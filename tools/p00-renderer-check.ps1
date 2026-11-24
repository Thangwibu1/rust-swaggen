[CmdletBinding()]
param([switch]$Release)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$checks = [System.Collections.Generic.List[object]]::new()
New-Item -ItemType Directory -Force reports/raw | Out-Null
function Invoke-RendererCheck([string]$Name, [string[]]$Arguments) {
    $stamp = [DateTime]::UtcNow.ToString('o')
    $prior = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $lines = (& cargo @Arguments 2>&1 | ForEach-Object { $_.ToString() } | Out-String)
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $prior }
    $artifact = "reports/raw/$Name.txt"
    $lines | Set-Content -Encoding utf8 $artifact
    $checks.Add([pscustomobject]@{name=$Name;status=$(if($code -eq 0){'PASS'}else{'FAIL'});command=('cargo '+($Arguments -join ' '));exit_code=$code;timestamp=$stamp;artifact_path=$artifact})
    Write-Host "$Name exit=$code"
    return $code
}
try {
    $version = & rustc +stable --version
    if ($LASTEXITCODE -ne 0 -or $version -notmatch '^rustc 1\.100\.0 ') { throw 'Pinned compiler 1.100.0 required' }
    & "$PSScriptRoot\doctor.ps1"
    $doctor = Get-Content reports/doctor.json -Raw | ConvertFrom-Json
    foreach ($required in @('Rust compiler','Cargo','Rustup target','MSVC installation','Windows SDK')) {
        if (($doctor | Where-Object name -eq $required).status -ne 'PASS') { throw "Renderer prerequisite blocked: $required" }
    }
    $manifest = @('--manifest-path','spikes/p00-native/Cargo.toml')
    $targetArgs = @('--offline','--locked','--no-default-features','--features','software','--target','x86_64-pc-windows-msvc','--target-dir','spikes/p00-native/target-software')
    $buildArgs = @('+stable','build') + $manifest + $targetArgs + @('--bin','native-gate')
    if ($Release) { $buildArgs += '--release' }
    $flavor = if ($Release) { 'release' } else { 'debug' }
    if ((Invoke-RendererCheck "software-build-$flavor" $buildArgs) -ne 0) { throw 'Software renderer build failed' }
    [void](Invoke-RendererCheck 'software-unit' (@('+stable','test') + $manifest + $targetArgs + @('--lib')))
    [void](Invoke-RendererCheck 'software-clippy' (@('+stable','clippy') + $manifest + $targetArgs + @('--all-targets','--','-D','warnings')))
    $metadataArgs = @('+stable','metadata') + $manifest + @('--offline','--locked','--no-default-features','--features','software','--filter-platform','x86_64-pc-windows-msvc','--format-version','1')
    if ((Invoke-RendererCheck 'software-metadata' $metadataArgs) -ne 0) { throw 'Software feature metadata failed' }
    Copy-Item -LiteralPath reports/raw/software-metadata.txt -Destination reports/raw/software-metadata.json
    if ((Invoke-RendererCheck 'software-tree' (@('+stable','tree') + $manifest + @('--offline','--locked','--no-default-features','--features','software','--target','x86_64-pc-windows-msvc','--edges','normal,build','--prefix','none','--format','{p}'))) -ne 0) { throw 'Selected software dependency tree failed' }
    Write-Host "Software renderer binary built ($flavor); native launch/graphics acceptance still requires an observed interactive run."
} finally {
    $checks | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 reports/software-checks.json
}
if ($checks | Where-Object status -ne 'PASS') { exit 2 }
