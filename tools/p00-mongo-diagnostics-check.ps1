[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$checks = [System.Collections.Generic.List[object]]::new()
function Invoke-DiagnosticCheck([string]$Name, [string[]]$Arguments) {
    $stamp = [DateTime]::UtcNow.ToString('o')
    $prior = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = (& cargo @Arguments 2>&1 | ForEach-Object { $_.ToString() } | Out-String)
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $prior }
    $artifact = "reports/raw/$Name.txt"
    $output | Set-Content -Encoding utf8 $artifact
    $checks.Add([pscustomobject]@{name=$Name;command=('cargo '+($Arguments -join ' '));status=$(if($code -eq 0){'PASS'}else{'FAIL'});exit_code=$code;timestamp=$stamp;artifact_path=$artifact})
    Write-Host "$Name exit=$code"
    return $code
}
try {
    $version = & rustc +stable --version
    if ($LASTEXITCODE -ne 0 -or $version -notmatch '^rustc 1\.100\.0 ') { throw 'Pinned Rust 1.100.0 required' }
    & "$PSScriptRoot\doctor.ps1"
    $doctor = Get-Content reports/doctor.json -Raw | ConvertFrom-Json
    foreach ($required in @('Rust compiler','Cargo','MSVC installation','Windows SDK')) {
        if (($doctor | Where-Object name -eq $required).status -ne 'PASS') { throw "Diagnostic prerequisite blocked: $required" }
    }
    $manifest = @('--manifest-path','spikes/p00/Cargo.toml')
    if ((Invoke-DiagnosticCheck 'mongo-diagnostics-unit' (@('+stable','test') + $manifest + @('--offline','--locked','--features','mongo','--lib','mongo_diagnostics::tests'))) -ne 0) { throw 'Diagnostic tests failed' }
    if ((Invoke-DiagnosticCheck 'mongo-diagnostics-build' (@('+stable','build') + $manifest + @('--offline','--locked','--features','mongo','--bin','mongo-smoke'))) -ne 0) { throw 'Probe build failed' }
    [void](Invoke-DiagnosticCheck 'mongo-diagnostics-clippy' (@('+stable','clippy') + $manifest + @('--offline','--locked','--features','mongo,fault-injection','--all-targets','--','-D','warnings')))
} finally {
    $checks | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 reports/mongo-diagnostics-checks.json
}
if ($checks | Where-Object status -ne 'PASS') { exit 2 }
