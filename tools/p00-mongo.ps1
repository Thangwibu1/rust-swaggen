[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
$priorUri = $env:SWAGGEN_MONGO_URI
$secureUri = $null
$pointer = [IntPtr]::Zero
try {
    if (-not $env:SWAGGEN_MONGO_URI) {
        $secureUri = Read-Host 'MongoDB URI for the isolated P00 fixture (hidden; not saved)' -AsSecureString
        $pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureUri)
        $env:SWAGGEN_MONGO_URI = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    }
    & "$PSScriptRoot\p00-check.ps1" -Mongo
    $result = $LASTEXITCODE
    & python -X utf8 "$PSScriptRoot\p00-report.py"
    Write-Host 'Mongo result is recorded in reports/p00-checks.json. No acceptance is implied by this message.'
    exit $result
} finally {
    if ($pointer -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer) }
    if ($secureUri) { $secureUri.Dispose() }
    if ($null -eq $priorUri) { Remove-Item Env:SWAGGEN_MONGO_URI -ErrorAction SilentlyContinue }
    else { $env:SWAGGEN_MONGO_URI = $priorUri }
}
