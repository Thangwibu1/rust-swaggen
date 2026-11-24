[CmdletBinding()]
param([switch]$Release)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$checks = [System.Collections.Generic.List[object]]::new()
function Invoke-ComparisonCheck([string]$Name, [string[]]$Arguments) {
    $stamp = [DateTime]::UtcNow.ToString('o')
    $prior = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = (& cargo @Arguments 2>&1 | Out-String)
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $prior }
    $artifact = "reports/raw/$Name.txt"
    $output | Set-Content -Encoding utf8 $artifact
    $checks.Add([pscustomobject]@{name=$Name;status=$(if($code -eq 0){'PASS'}else{'FAIL'});command=('cargo '+($Arguments -join ' '));exit_code=$code;timestamp=$stamp;artifact_path=$artifact})
    Write-Host "$Name exit=$code"
    return $code
}
try {
    $compiler = & rustc +stable --version
    if ($LASTEXITCODE -ne 0 -or $compiler -notmatch '^rustc 1\.100\.0 ') { throw 'Pinned Rust 1.100.0 required' }
    & "$PSScriptRoot\doctor.ps1"
    $doctor = Get-Content reports/doctor.json -Raw | ConvertFrom-Json
    foreach ($required in @('Rust compiler','Cargo','Rustup target','MSVC installation','Windows SDK')) {
        if (($doctor | Where-Object name -eq $required).status -ne 'PASS') { throw "Comparison prerequisite blocked: $required" }
    }
    $manifest = @('--manifest-path','spikes/p00-egui/Cargo.toml')
    $target = @('--offline','--locked','--target','x86_64-pc-windows-msvc')
    if ((Invoke-ComparisonCheck 'egui-format' (@('+stable','fmt') + $manifest + @('--','--check'))) -ne 0) { throw 'Comparison formatting failed' }
    if ((Invoke-ComparisonCheck 'egui-unit' (@('+stable','test') + $manifest + $target + @('--lib'))) -ne 0) { throw 'Comparison editor model tests failed' }
    if ((Invoke-ComparisonCheck 'egui-build' (@('+stable','build') + $manifest + $target)) -ne 0) { throw 'Comparison build failed' }
    if ($Release) {
        if ((Invoke-ComparisonCheck 'egui-release-build' (@('+stable','build') + $manifest + $target + @('--release'))) -ne 0) { throw 'Comparison release build failed' }
    }
    if ((Invoke-ComparisonCheck 'egui-clippy' (@('+stable','clippy') + $manifest + $target + @('--all-targets','--','-D','warnings'))) -ne 0) { throw 'Comparison Clippy failed' }
    if ((Invoke-ComparisonCheck 'egui-metadata' (@('+stable','metadata') + $manifest + @('--offline','--locked','--filter-platform','x86_64-pc-windows-msvc','--format-version','1'))) -ne 0) { throw 'Comparison metadata failed' }
    Write-Host 'Native comparison compiled; actual accessibility/IME/Narrator/UI acceptance remains separate.'
} finally {
    $checks | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 reports/egui-checks.json
}
if ($checks | Where-Object status -ne 'PASS') { exit 2 }
