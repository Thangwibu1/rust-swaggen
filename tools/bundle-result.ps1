param(
    [string]$Version = "",
    [switch]$SkipBuild,
    [switch]$Online
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $RepoRoot

if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargoToml = Get-Content (Join-Path $RepoRoot "Cargo.toml") -Raw
    if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        throw "Could not determine version from Cargo.toml"
    }
}

$Target = "x86_64-pc-windows-msvc"
if (-not $SkipBuild) {
    Write-Host "Building release binaries ($Target)..."
    $BuildArgs = @("build", "--release", "--locked", "-p", "swaggen-desktop", "-p", "swaggen-cli", "-p", "swaggen-server", "--target", $Target)
    if (-not $Online -and $env:CI -ne "true") {
        $BuildArgs = @("+stable") + $BuildArgs + @("--offline")
    }
    cargo @BuildArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo release build failed with exit code $LASTEXITCODE" }
}

$ReleaseDir = Join-Path $RepoRoot "target\$Target\release"
$ResultRoot = Join-Path $RepoRoot "result"
$VerTag = "v$Version"
$VerDir = Join-Path $ResultRoot $VerTag
$BinDir = Join-Path $VerDir "bin"
$SymDir = Join-Path $VerDir "symbols"
$CfgDir = Join-Path $VerDir "config"
$DocDir = Join-Path $VerDir "docs"
$PkgDir = Join-Path $VerDir "packages"

foreach ($d in @($ResultRoot, $VerDir, $BinDir, $SymDir, $CfgDir, $DocDir, $PkgDir)) {
    New-Item -ItemType Directory -Force -Path $d | Out-Null
}

$Bins = @("swaggen-desktop.exe", "swaggen-cli.exe", "swaggen-server.exe")
foreach ($b in $Bins) {
    $src = Join-Path $ReleaseDir $b
    if (-not (Test-Path $src)) { throw "Missing release binary: $src" }
    Copy-Item -Force $src (Join-Path $BinDir $b)
}

$Pdbs = @("swaggen_desktop.pdb", "swaggen_cli.pdb", "swaggen_server.pdb")
foreach ($p in $Pdbs) {
    $src = Join-Path $ReleaseDir $p
    if (Test-Path $src) {
        Copy-Item -Force $src (Join-Path $SymDir $p)
    }
}

foreach ($f in @(".env.example", "docker-compose.yml", "Dockerfile", ".dockerignore")) {
    $src = Join-Path $RepoRoot $f
    if (Test-Path $src) {
        Copy-Item -Force $src (Join-Path $CfgDir $f)
    }
}

$CoolifyGuide = Join-Path $RepoRoot "07-operations\coolify-deploy-steps.md"
if (Test-Path $CoolifyGuide) {
    Copy-Item -Force $CoolifyGuide (Join-Path $DocDir "coolify-deploy-steps.md")
}

$OpenApiPath = Join-Path $DocDir "server-openapi.json"
& (Join-Path $BinDir "swaggen-server.exe") export-openapi $OpenApiPath | Out-Null
if ($LASTEXITCODE -ne 0) { throw "export-openapi failed" }

@'
@echo off
setlocal
start "" "%~dp0bin\swaggen-desktop.exe" %*
'@ | Set-Content -Encoding ASCII (Join-Path $VerDir "Swaggen.cmd")

@'
@echo off
setlocal
set "DATA_DIR=%~dp0data"
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"
start "" "%~dp0bin\swaggen-desktop.exe" --data-dir "%DATA_DIR%" %*
'@ | Set-Content -Encoding ASCII (Join-Path $VerDir "Swaggen-Portable.cmd")

$ReadmeText = @"
Swaggen $VerTag (Windows x86_64 - Unsigned Portable Bundle)
===========================================================

Cau truc thu muc:
- Swaggen.cmd               : Mo app Windows GUI (luu du lieu tai %LOCALAPPDATA%\Swaggen).
- Swaggen-Portable.cmd      : Mo app Windows GUI doc lap (luu du lieu ngay trong .\data\).
- bin\swaggen-desktop.exe   : Ung dung Windows Desktop native (Rust eframe + wgpu, khong can WebView2).
- bin\swaggen-cli.exe       : CLI runner cho CI/CD va chay bo collection.
- bin\swaggen-server.exe    : Backend server Windows (can SWAGGEN_MONGO_URI hoac --in-memory-dev).
- symbols\                  : File debug .pdb phuc vu dieu tra crash.
- config\                   : .env.example, Dockerfile, .dockerignore, docker-compose.yml cho Coolify/Docker.
- docs\                     : server-openapi.json (OpenAPI 3.1 contract) va coolify-deploy-steps.md.
- packages\                 : Ban nen .zip portable de gui hoac luu tru.
- version.json              : Thong tin phien ban, Git commit va SHA-256 checksums.
"@
$ReadmeText | Set-Content -Encoding UTF8 (Join-Path $VerDir "README.txt")

$GitCommit = (git rev-parse --short HEAD 2>$null)
if ([string]::IsNullOrWhiteSpace($GitCommit)) { $GitCommit = "unknown" }
$GitCommitFull = (git rev-parse HEAD 2>$null)
if ([string]::IsNullOrWhiteSpace($GitCommitFull)) { $GitCommitFull = "unknown" }
$RustVer = (rustc +stable --version 2>$null)
$BuiltAt = (Get-Date).ToString("o")

$Hashes = [ordered]@{}
foreach ($b in $Bins) {
    $fp = Join-Path $BinDir $b
    $Hashes[$b] = [ordered]@{
        size_bytes = (Get-Item $fp).Length
        sha256     = (Get-FileHash $fp -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

$ZipName = "Swaggen-$Version-win64-portable.zip"
$ZipPath = Join-Path $PkgDir $ZipName
if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }

$StageDir = Join-Path $RepoRoot ".local\stage-$VerTag"
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null

foreach ($item in @("bin", "config", "docs", "Swaggen.cmd", "Swaggen-Portable.cmd", "README.txt")) {
    Copy-Item -Recurse -Force (Join-Path $VerDir $item) (Join-Path $StageDir $item)
}

$VersionObj = [ordered]@{
    version        = $Version
    tag            = $VerTag
    target         = $Target
    built_at       = $BuiltAt
    git_commit     = $GitCommitFull
    git_short      = $GitCommit
    rustc          = $RustVer
    signing_status = "UNSIGNED_PORTABLE"
    binaries       = $Hashes
    zip_file       = "packages/$ZipName"
}
$VersionObj | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $StageDir "version.json")

Compress-Archive -Path (Join-Path $StageDir "*") -DestinationPath $ZipPath -CompressionLevel Optimal
Remove-Item -Recurse -Force $StageDir

$VersionObj["zip_size_bytes"] = (Get-Item $ZipPath).Length
$VersionObj["zip_sha256"] = (Get-FileHash $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$VersionObj | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $VerDir "version.json")

# Rebuild result/versions.json and result/VERSIONS.md index across all version folders
$AllVersions = @()
Get-ChildItem -Directory $ResultRoot -Filter "v*" | Sort-Object Name -Descending | ForEach-Object {
    $vf = Join-Path $_.FullName "version.json"
    if (Test-Path $vf) {
        $AllVersions += (Get-Content $vf -Raw | ConvertFrom-Json)
    }
}
ConvertTo-Json -InputObject @($AllVersions) -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $ResultRoot "versions.json")

$MdLines = @(
    '# Swaggen Build Releases (`result/`)',
    "",
    "| Phien ban | Thoi gian build | Git Commit | Desktop EXE (MB) | ZIP Portable (MB) | Thu muc |",
    "|---|---|---|---:|---:|---|"
)
foreach ($v in $AllVersions) {
    $deskMb = [math]::Round(($v.binaries.'swaggen-desktop.exe'.size_bytes / 1MB), 2)
    $zipMb = [math]::Round(($v.zip_size_bytes / 1MB), 2)
    $MdLines += "| $($v.tag) | $($v.built_at) | ``$($v.git_short)`` | $deskMb | $zipMb | ``result/$($v.tag)/`` |"
}
$MdLines | Set-Content -Encoding UTF8 (Join-Path $ResultRoot "VERSIONS.md")

Write-Host "Bundled $VerTag into $VerDir"
