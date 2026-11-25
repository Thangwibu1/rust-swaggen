param(
    [string]$Version = "0.1.3",
    [string]$BundleDir = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($BundleDir)) {
    $BundleDir = Join-Path $RepoRoot "result\v$Version"
}
if (-not (Test-Path (Join-Path $BundleDir "bin\swaggen-desktop.exe"))) {
    throw "Bundle directory missing swaggen-desktop.exe: $BundleDir. Run tools\bundle-result.ps1 -Version $Version first."
}

$InstallerOutDir = Join-Path $BundleDir "installer"
New-Item -ItemType Directory -Force -Path $InstallerOutDir | Out-Null

# 1. Build standalone self-extracting Windows Setup EXE using built-in Windows iexpress.exe (zero external deps!)
$StageDir = Join-Path $RepoRoot ".local\installer-stage-v$Version"
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null

$PayloadZip = Join-Path $StageDir "swaggen-payload.zip"
$PayloadFiles = @(
    (Join-Path $BundleDir "bin"),
    (Join-Path $BundleDir "config"),
    (Join-Path $BundleDir "docs"),
    (Join-Path $BundleDir "Swaggen.cmd"),
    (Join-Path $BundleDir "Swaggen-Portable.cmd"),
    (Join-Path $BundleDir "README.txt"),
    (Join-Path $BundleDir "version.json")
) | Where-Object { Test-Path $_ }
Compress-Archive -Path $PayloadFiles -DestinationPath $PayloadZip -Force

$InstallPs1 = Join-Path $StageDir "install-swaggen.ps1"
Copy-Item -Path (Join-Path $RepoRoot "installer\install-swaggen.ps1") -Destination $InstallPs1 -Force

$SetupExe = Join-Path $InstallerOutDir "Swaggen-Setup-$Version-x64.exe"
$SedFile = Join-Path $StageDir "swaggen.sed"
$SedContent = @"
[Version]
Class=IEXPRESS
SEDVersion=3
[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=1
HideExtractAnimation=0
UseLongFileName=1
InsideCompressed=0
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=N
InstallPrompt=Install Swaggen v$Version (Native Windows API Studio)?
DisplayLicense=
FinishMessage=
TargetName=$SetupExe
FriendlyName=Swaggen Setup v$Version
AppLaunched=powershell.exe -NoProfile -ExecutionPolicy Bypass -File install-swaggen.ps1 -Version $Version
PostInstallCmd=<None>
AdminQuietInstCmd=powershell.exe -NoProfile -ExecutionPolicy Bypass -File install-swaggen.ps1 -Version $Version -Silent
UserQuietInstCmd=powershell.exe -NoProfile -ExecutionPolicy Bypass -File install-swaggen.ps1 -Version $Version -Silent
SourceFiles=SourceFiles
[Strings]
FILE0="swaggen-payload.zip"
FILE1="install-swaggen.ps1"
[SourceFiles]
SourceFiles0=$StageDir\
[SourceFiles0]
%FILE0%=
%FILE1%=
"@
Set-Content -Path $SedFile -Value $SedContent -Encoding ASCII

Write-Host "Building native Windows Setup EXE via iexpress.exe..."
$ix = Start-Process -FilePath "iexpress.exe" -ArgumentList "/N", "/Q", $SedFile -Wait -PassThru
if ($ix.ExitCode -ne 0 -or -not (Test-Path $SetupExe)) {
    throw "iexpress.exe failed with exit code $($ix.ExitCode)"
}
$SetupHash = (Get-FileHash -Path $SetupExe -Algorithm SHA256).Hash
$SetupSizeMb = [math]::Round((Get-Item $SetupExe).Length / 1MB, 2)
Write-Host "Created Windows Setup EXE: $SetupExe ($SetupSizeMb MB, SHA256=$SetupHash)" -ForegroundColor Green

# 2. Optional Inno Setup 6 build if ISCC.exe is installed
$Iscc = Get-Command "iscc" -ErrorAction SilentlyContinue
if (-not $Iscc -and (Test-Path "C:\Program Files (x86)\Inno Setup 6\ISCC.exe")) {
    $Iscc = Get-Item "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
}
if ($Iscc) {
    $IsccPath = if ($Iscc.Source) { $Iscc.Source } else { $Iscc.FullName }
    Write-Host "ISCC.exe detected ($IsccPath) - building Inno Setup installer..."
    & $IsccPath "/DAppVersion=$Version" "/DBundleDir=$BundleDir" "/O$InstallerOutDir" (Join-Path $RepoRoot "installer\swaggen.iss")
}

# 3. Optional WiX v4 MSI build if wix.exe is installed
$Wix = Get-Command "wix" -ErrorAction SilentlyContinue
if ($Wix) {
    $MsiOut = Join-Path $InstallerOutDir "Swaggen-$Version-x64.msi"
    Write-Host "wix.exe detected - building MSI installer..."
    & $Wix.Source build -arch x64 -d "AppVersion=$Version.0" -d "BundleDir=$BundleDir" -o $MsiOut (Join-Path $RepoRoot "installer\swaggen.wxs")
}

# 4. Write SHA256SUMS.txt for distribution verification
$Sums = Get-ChildItem -Path $InstallerOutDir -File | Where-Object { $_.Name -ne "SHA256SUMS.txt" } | ForEach-Object {
    "$((Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash)  $($_.Name)"
}
Set-Content -Path (Join-Path $InstallerOutDir "SHA256SUMS.txt") -Value $Sums -Encoding UTF8

Remove-Item -Recurse -Force $StageDir -ErrorAction SilentlyContinue
Write-Host "Installer artifacts ready in: $InstallerOutDir" -ForegroundColor Cyan
