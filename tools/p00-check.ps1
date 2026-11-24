[CmdletBinding()]
param([switch]$Mongo, [switch]$Native, [switch]$NativeLibrary, [switch]$Durability)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
New-Item -ItemType Directory -Force reports/raw | Out-Null
$records = [System.Collections.Generic.List[object]]::new()
. "$PSScriptRoot\p00-evidence-store.ps1"
function Invoke-Evidence([string]$Name, [string]$Executable, [string[]]$Arguments) {
    $stamp = [DateTime]::UtcNow.ToString('o')
    $artifact = "reports/raw/$Name.txt"
    $priorPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $result = (& $Executable @Arguments 2>&1 | ForEach-Object { $_.ToString() } | Out-String)
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $priorPreference }
    $status = if ($code -eq 0) { 'PASS' } else { 'FAIL' }
    if ($Name -eq 'mongo-runtime' -and $code -ne 0) {
        $diagnostic = $result -split "`r?`n" | Where-Object { $_ -match '^\{.*"status"' } | Select-Object -Last 1
        if ($diagnostic) {
            try {
                $category = ($diagnostic | ConvertFrom-Json).status
                if ($category -eq 'BLOCKED') { $status = 'BLOCKED' }
            } catch { } # Keep FAIL for malformed or unexpected diagnostic output.
        }
    }
    $result | Set-Content -Encoding utf8 $artifact
    $records.Add([pscustomobject]@{name=$Name;status=$status;command=($Executable+' '+($Arguments -join ' '));exit_code=$code;timestamp=$stamp;artifact_path=$artifact})
    Write-Host "$Name exit=$code"
    return $code
}
try {
    # Alias override is safe only if this installed stable is the exact pinned compiler.
    $compiler = & rustc +stable --version
    if ($LASTEXITCODE -ne 0 -or $compiler -notmatch '^rustc 1\.100\.0 ') { throw 'Pinned Rust 1.100.0 unavailable; install exact toolchain before running' }
    & "$PSScriptRoot\doctor.ps1"
    # The doctor intentionally reports remaining GUI/research blockers; verify required local tools below.
    $doctor = Get-Content reports/doctor.json -Raw | ConvertFrom-Json
    foreach ($required in @('Rust compiler','Cargo','Rustup target','MSVC installation','Windows SDK')) {
        if (($doctor | Where-Object name -eq $required).status -ne 'PASS') { throw "Build prerequisite blocked: $required" }
    }
    $manifest = @('--manifest-path','spikes/p00/Cargo.toml')
    if ((Invoke-Evidence 'format' 'cargo' (@('+stable','fmt') + $manifest + @('--','--check'))) -ne 0) { throw 'Format check failed' }
    if ((Invoke-Evidence 'journal-unit' 'cargo' (@('+stable','test') + $manifest + @('--offline','--locked','--lib'))) -ne 0) { throw 'Journal tests failed' }
    if ((Invoke-Evidence 'journal-process-crash' 'cargo' (@('+stable','run') + $manifest + @('--offline','--locked','--bin','journal-smoke'))) -ne 0) { throw 'Process crash smoke failed' }
    if ($Durability) {
        if ((Invoke-Evidence 'journal-crash-matrix' 'cargo' (@('+stable','run') + $manifest + @('--offline','--locked','--features','fault-injection','--bin','journal-crash-matrix'))) -ne 0) { throw 'Compaction crash matrix failed' }
    }
    if ((Invoke-Evidence 'mongo-build' 'cargo' (@('+stable','build') + $manifest + @('--offline','--locked','--features','mongo'))) -ne 0) { throw 'Mongo smoke compile failed' }
    $lintFeatures = if ($Durability) { 'mongo,fault-injection' } else { 'mongo' }
    if ((Invoke-Evidence 'clippy' 'cargo' (@('+stable','clippy') + $manifest + @('--offline','--locked','--features',$lintFeatures,'--all-targets','--','-D','warnings'))) -ne 0) { throw 'Clippy failed' }
    if ($Native -or $NativeLibrary) {
        $nativeManifest = @('--manifest-path','spikes/p00-native/Cargo.toml')
        if ((Invoke-Evidence 'native-format' 'cargo' (@('+stable','fmt') + $nativeManifest + @('--','--check'))) -ne 0) { throw 'Native formatting failed' }
        if ($Native) {
            if ((Invoke-Evidence 'native-build' 'cargo' (@('+stable','build') + $nativeManifest + @('--offline','--locked'))) -ne 0) { throw 'Native build failed' }
        }
        [void](Invoke-Evidence 'native-unit' 'cargo' (@('+stable','test') + $nativeManifest + @('--offline','--locked','--lib')))
        if ($Native) { [void](Invoke-Evidence 'native-http' 'cargo' (@('+stable','run') + $nativeManifest + @('--offline','--locked','--bin','http-gate'))) }
        [void](Invoke-Evidence 'native-clippy' 'cargo' (@('+stable','clippy') + $nativeManifest + @('--offline','--locked','--all-targets','--','-D','warnings')))
    }
    if ($Mongo) {
        if (-not $env:SWAGGEN_MONGO_URI) { throw 'Set SWAGGEN_MONGO_URI privately; it is never persisted by this script' }
        # Driver internal timeouts only; never cancel/drop a driver future.
        [void](Invoke-Evidence 'mongo-runtime' 'cargo' (@('+stable','run') + $manifest + @('--offline','--locked','--features','mongo','--bin','mongo-smoke')))
    }
} finally {
    # Keep evidence for scopes not rerun (especially Mongo and the crash matrix).
    # A rerun replaces its own scope even when it fails; never retain an older PASS instead.
    Save-P00Checks 'reports/p00-checks.json' $records.ToArray()
}
if ($records | Where-Object status -ne 'PASS') { exit 2 }
# Local checks alone do not satisfy GUI/topology/advisory/full durability gate.
Write-Host 'Local checks completed; P00 acceptance remains separate in reports/preflight.md.'
