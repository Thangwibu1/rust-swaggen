[CmdletBinding()]
param([switch]$UseLocalDatabase)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
$priorCargoHome = $env:CARGO_HOME
$priorPath = $env:PATH
$priorRetry = $env:CARGO_NET_RETRY
$priorTimeout = $env:CARGO_HTTP_TIMEOUT
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $compiler = & rustc +stable --version
    if ($LASTEXITCODE -ne 0 -or $compiler -notmatch '^rustc 1\.100\.0 ') { throw 'Pinned Rust 1.100.0 required' }
    $env:CARGO_HOME = Join-Path (Get-Location) '.local/p00-audit-cargo'
    $env:CARGO_NET_RETRY = '0'
    $env:CARGO_HTTP_TIMEOUT = '15'
    $tool = Join-Path (Get-Location) '.local/p00-audit-tools/bin/cargo-audit.exe'
    if (-not (Test-Path -LiteralPath $tool)) {
        $prior = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            $output = & cargo +stable install cargo-audit --version 0.23.0 --locked --root .local/p00-audit-tools 2>&1 | Out-String
            $installCode = $LASTEXITCODE
        } finally { $ErrorActionPreference = $prior }
        $output | Set-Content -Encoding utf8 reports/raw/audit-install-attempt.txt
        Write-Host "Pinned audit installation exit=$installCode"
        if ($installCode -ne 0) {
            & python -X utf8 tools/p00-audit.py
            & python -X utf8 tools/p00-report.py
            exit 2
        }
    }
    $version = & $tool --version
    if ($LASTEXITCODE -ne 0 -or $version -notmatch '0\.23\.0(?:\s|$)') { throw 'Expected cargo-audit 0.23.0' }
    $version | Set-Content -Encoding utf8 reports/raw/audit-tool-version.txt
    # The install cache is isolated. Resolve application metadata using its
    # normal dependency cache, rather than the cargo-audit installation cache.
    $env:CARGO_HOME = $priorCargoHome
    if ($UseLocalDatabase) {
        & python -X utf8 tools/p00-audit.py --no-fetch
    } else {
        & python -X utf8 tools/p00-audit.py
    }
    $auditCode = $LASTEXITCODE
    & python -X utf8 tools/p00-report.py
    if ($LASTEXITCODE -ne 0) { exit 2 }
    & python -X utf8 tools/p00-evidence-audit.py
    if ($LASTEXITCODE -ne 0) { exit 2 }
    Write-Host 'Acceptance uses adopted Windows graphs. Original cross-platform and rejected Iced FAIL results remain visible; see reports/p00-security-disposition.md.'
    exit $auditCode
} finally {
    $env:CARGO_HOME = $priorCargoHome
    $env:PATH = $priorPath
    $env:CARGO_NET_RETRY = $priorRetry
    $env:CARGO_HTTP_TIMEOUT = $priorTimeout
}
