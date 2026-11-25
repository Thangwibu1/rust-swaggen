param(
    [string]$Version = "0.1.3",
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Swaggen"),
    [switch]$Silent
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms -ErrorAction SilentlyContinue

$PayloadZip = Join-Path $PSScriptRoot "swaggen-payload.zip"
if (-not (Test-Path $PayloadZip)) {
    throw "Installer payload not found: $PayloadZip"
}

if (-not $Silent) {
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  Swaggen Desktop Installer v$Version" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "Installing to: $InstallDir"
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$BinDir = Join-Path $InstallDir "bin"
if (Test-Path $BinDir) {
    Get-Process -Name "swaggen-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
}

Expand-Archive -Path $PayloadZip -DestinationPath $InstallDir -Force

$DesktopExe = Join-Path $InstallDir "bin\swaggen-desktop.exe"
$UninstallScript = Join-Path $InstallDir "uninstall-swaggen.ps1"
if (-not (Test-Path $DesktopExe)) {
    throw "Expected binary missing after extraction: $DesktopExe"
}

# Create Desktop & Start Menu shortcuts via WScript.Shell
$WshShell = New-Object -ComObject WScript.Shell

$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Swaggen"
New-Item -ItemType Directory -Force -Path $StartMenuDir | Out-Null

$ShortcutTargets = @(
    (Join-Path $StartMenuDir "Swaggen.lnk"),
    (Join-Path ([Environment]::GetFolderPath("Desktop")) "Swaggen.lnk")
)
foreach ($lnkPath in $ShortcutTargets) {
    $sc = $WshShell.CreateShortcut($lnkPath)
    $sc.TargetPath = $DesktopExe
    $sc.WorkingDirectory = $InstallDir
    $sc.Description = "Swaggen - Native Windows API Client & Collaboration Studio"
    $sc.Save()
}

# Write per-user Uninstall script & register in Windows Settings -> Installed Apps (no admin/UAC required)
$UninstallBody = @"
param([switch]`$KeepData)
`$ErrorActionPreference = "SilentlyContinue"
Get-Process -Name "swaggen-desktop" | Stop-Process -Force
Start-Sleep -Milliseconds 400
Remove-Item -Force "$($ShortcutTargets[0])"
Remove-Item -Force "$($ShortcutTargets[1])"
Remove-Item -Recurse -Force "$StartMenuDir"
Remove-Item -Recurse -Force "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Swaggen"
if (-not `$KeepData) {
    Remove-Item -Recurse -Force "$InstallDir\bin", "$InstallDir\docs", "$InstallDir\config", "$InstallDir\Launch-Swaggen.cmd", "$InstallDir\manifest.json", "$InstallDir\README.txt"
}
Write-Host "Swaggen has been uninstalled."
"@
Set-Content -Path $UninstallScript -Value $UninstallBody -Encoding UTF8

$RegPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Swaggen"
New-Item -Path $RegPath -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "DisplayName" -Value "Swaggen" -PropertyType String -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "DisplayVersion" -Value $Version -PropertyType String -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "Publisher" -Value "Swaggen" -PropertyType String -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "InstallLocation" -Value $InstallDir -PropertyType String -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "DisplayIcon" -Value "$DesktopExe,0" -PropertyType String -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "UninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$UninstallScript`"" -PropertyType String -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "NoModify" -Value 1 -PropertyType DWord -Force | Out-Null
New-ItemProperty -Path $RegPath -Name "NoRepair" -Value 1 -PropertyType DWord -Force | Out-Null

# Add bin\ to per-user PATH so `swaggen-cli` works in PowerShell/CMD
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ([string]::IsNullOrWhiteSpace($UserPath)) {
    [Environment]::SetEnvironmentVariable("Path", $BinDir, "User")
} elseif ($UserPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$BinDir", "User")
}

if (-not $Silent) {
    Write-Host "Installed Swaggen v$Version cleanly to $InstallDir" -ForegroundColor Green
    $res = [System.Windows.Forms.MessageBox]::Show(
        "Swaggen v$Version has been installed to:`n$InstallDir`n`nDesktop and Start Menu shortcuts were created.`n`nLaunch Swaggen now?",
        "Swaggen Setup Complete",
        [System.Windows.Forms.MessageBoxButtons]::YesNo,
        [System.Windows.Forms.MessageBoxIcon]::Information
    )
    if ($res -eq [System.Windows.Forms.DialogResult]::Yes) {
        Start-Process -FilePath $DesktopExe -WorkingDirectory $InstallDir
    }
}
