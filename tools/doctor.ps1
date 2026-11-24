# Read-only developer preflight. Does not install software, change settings or deploy.
[CmdletBinding()]
param([string]$OutputDirectory = 'reports', [switch]$ServerTools)
$ErrorActionPreference = 'Stop'
$checks = [System.Collections.Generic.List[object]]::new()
function Add-Check([string]$Name, [string]$Status, [string]$Detail, [string]$Command = 'PowerShell presence inspection; see tools/doctor.ps1', $ExitCode = $null) {
    $checks.Add([pscustomobject]@{name=$Name; status=$Status; detail=$Detail; command=$Command; exit_code=$ExitCode; timestamp=[DateTime]::UtcNow.ToString('o')})
}
function Check-Tool([string]$Name, [string]$Command, [string[]]$Arguments) {
    $cmd = Get-Command $Command -ErrorAction SilentlyContinue
    if (-not $cmd) { Add-Check $Name 'BLOCKED' "$Command not found"; return }
    try {
        $text = (& $cmd.Source @Arguments 2>&1 | Out-String).Trim()
        $code = $LASTEXITCODE
        Add-Check $Name $(if ($code -eq 0) {'PASS'} else {'FAIL'}) $text ($Command+' '+($Arguments -join ' ')) $code
    } catch { Add-Check $Name 'FAIL' $_.Exception.Message }
}
Add-Check 'Host' 'PASS' ([System.Runtime.InteropServices.RuntimeInformation]::OSDescription)
Check-Tool 'Rust compiler' 'rustc' @('+stable','-Vv')
Check-Tool 'Cargo' 'cargo' @('+stable','--version')
Check-Tool 'Rustup target' 'rustup' @('target','list','--installed','--toolchain','stable')
Check-Tool 'Rustfmt' 'cargo' @('+stable','fmt','--version')
Check-Tool 'Clippy' 'cargo' @('+stable','clippy','--version')
Check-Tool 'Git' 'git' @('--version')
if ($env:OS -eq 'Windows_NT') {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $vc = (& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Out-String).Trim()
        Add-Check 'MSVC installation' $(if ($vc) {'PASS'} else {'BLOCKED'}) $(if ($vc) {$vc} else {'C++ x64 tools not detected'})
    } else { Add-Check 'MSVC installation' 'BLOCKED' 'vswhere unavailable; inspect Visual Studio Build Tools manually' }
    $sdk = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots' -ErrorAction SilentlyContinue
    $sdkRoot = if ($sdk) {$sdk.KitsRoot10} else {$null}
    Add-Check 'Windows SDK' $(if ($sdkRoot -and (Test-Path $sdkRoot)) {'PASS'} else {'BLOCKED'}) $(if ($sdkRoot) {$sdkRoot} else {'Windows SDK not detected'})
    Add-Check 'MSVC link smoke test' 'BLOCKED' 'Build and launch a pinned Rust native executable from a configured MSVC environment; presence checks alone are insufficient'
    Add-Check 'Windows GUI/IME/DPI/Narrator' 'BLOCKED' 'Run P00 native UI spike and record measured evidence'
} else { Add-Check 'Windows native checks' 'BLOCKED' 'This host cannot establish Windows GUI/runtime acceptance' }
if ($ServerTools) {
    Add-Check 'Container tooling' 'NOT_APPLICABLE' 'User explicitly prohibited Docker on 2026-11-24; no Docker/Compose checks or execution'
    Check-Tool 'Mongo shell' 'mongosh' @('--version')
    Check-Tool 'Mongo backup tool' 'mongodump' @('--version')
    Check-Tool 'Mongo restore tool' 'mongorestore' @('--version')
    Add-Check 'MongoDB replica-set transactions' 'BLOCKED' 'Run real commit, rollback, retry and lost-ACK tests with a scoped disposable database; ping is insufficient'
}
Add-Check 'Dependency compatibility and licenses' 'BLOCKED' 'Resolve official exact versions/MSRV/targets/features and advisories; fill dependency-matrix.md before scaffold'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$checks | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory 'doctor.json')
$md = @('# Developer preflight', '', '| Check | Status | Detail |', '| --- | --- | --- |')
foreach ($check in $checks) { $detail = $check.detail.Replace('|','\|').Replace("`r",'').Replace("`n",'<br>'); $md += "| $($check.name) | $($check.status) | $detail |" }
$md | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory 'doctor.md')
Write-Host 'Developer prerequisite snapshot only. Build/runtime results are reconciled in reports/preflight.md after the checks.'
$checks | Format-Table name,status -AutoSize
if ($checks | Where-Object { $_.status -in @('FAIL','BLOCKED') }) { exit 2 }
exit 0
