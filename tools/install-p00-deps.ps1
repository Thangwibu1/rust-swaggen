[CmdletBinding()]
param([switch]$CompareUI)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:CARGO_NET_RETRY = '0'
$env:CARGO_HTTP_TIMEOUT = '15'
New-Item -ItemType Directory -Force .local/p00-dependencies/src,reports/raw | Out-Null
$checks = [System.Collections.Generic.List[object]]::new()
function Invoke-InstallStep([string]$Name, [string]$Executable, [string[]]$Arguments) {
    $stamp = [DateTime]::UtcNow.ToString('o')
    $prior = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = (& $Executable @Arguments 2>&1 | Out-String)
        $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $prior }
    $artifact = "reports/raw/install-$Name.txt"
    $output | Set-Content -Encoding utf8 $artifact
    $checks.Add([pscustomobject]@{name=$Name;command=($Executable+' '+($Arguments -join ' '));exit_code=$code;status=$(if($code -eq 0){'PASS'}else{'BLOCKED'});timestamp=$stamp;artifact_path=$artifact})
    Write-Host "$Name exit=$code ($artifact)"
    return $code
}
try {
    # Reuse exact installed compiler rather than reinstalling the toolchain unnecessarily.
    if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        throw 'Rustup unavailable. Install from https://rust-lang.org/tools/install/ then rerun.'
    }
    $installedCompiler = & rustc +stable --version 2>$null
    $selectedToolchain = 'stable'
    if ($LASTEXITCODE -ne 0 -or $installedCompiler -notmatch '^rustc 1\.100\.0 ') {
        if ((Invoke-InstallStep 'rust-toolchain' 'rustup' @('toolchain','install','1.100.0','--profile','minimal','--component','rustfmt,clippy')) -ne 0) {
            throw 'Exact Rust toolchain installation blocked; inspect report.'
        }
        $selectedToolchain = '1.100.0'
    }
    if ((Invoke-InstallStep 'windows-target' 'rustup' @('target','list','--installed','--toolchain',$selectedToolchain)) -ne 0) {
        throw 'Unable to inspect installed Windows target.'
    }
    $targets = Get-Content reports/raw/install-windows-target.txt -Raw
    if ($targets -notmatch 'x86_64-pc-windows-msvc') {
        if ((Invoke-InstallStep 'add-windows-target' 'rustup' @('target','add','x86_64-pc-windows-msvc','--toolchain',$selectedToolchain)) -ne 0) { throw 'Windows target installation blocked.' }
    }
    foreach ($component in @('rustfmt','clippy')) {
        if ((Invoke-InstallStep $component 'rustup' @('component','add',$component,'--toolchain',$selectedToolchain)) -ne 0) { throw "$component installation blocked." }
    }
    # Download candidates in a disposable compatibility package, never scaffold the product.
    # Cargo.lock is generated only by a successful registry resolution, never fabricated.
    @'
[package]
name = "swaggen-p00-dependency-fetch"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
iced = { version = "=0.14.0", features = ["tokio", "highlighter", "svg"] }
axum = "=0.8.9"
mongodb = { version = "=3.10.2", features = ["redact-errors"] }
tokio = { version = "=1.54.1", features = ["macros", "rt-multi-thread", "net", "time"] }
serde_json = "=1.0.154"
'@ | Set-Content -Encoding utf8 .local/p00-dependencies/Cargo.toml
    if ($CompareUI) {
        # Native comparison candidate only, after the first Iced accessibility probe.
        'eframe = "=0.37.0"' | Add-Content -Encoding utf8 .local/p00-dependencies/Cargo.toml
    }
    '// Dependency fetch/compatibility fixture only; not application code.' | Set-Content -Encoding utf8 .local/p00-dependencies/src/lib.rs
    if ((Invoke-InstallStep 'candidate-crates' 'cargo' @("+$selectedToolchain",'fetch','--manifest-path','.local/p00-dependencies/Cargo.toml','--target','x86_64-pc-windows-msvc')) -ne 0) {
        throw 'Dependency download blocked. No install success or compatibility acceptance claimed.'
    }
    Write-Host 'Candidate crate sources fetched. Run versioned API/license/advisory/native smoke checks before approving P00.'
} finally {
    $checks | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 reports/dependency-install.json
}
