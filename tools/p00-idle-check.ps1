[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
$expected = [System.IO.Path]::GetFullPath((Join-Path $root 'spikes/p00-egui/target/x86_64-pc-windows-msvc/release/swaggen-p00-egui.exe'))
$fixtureProcesses = @([System.Diagnostics.Process]::GetProcessesByName('swaggen-p00-egui') | Where-Object { $_.MainModule.FileName -eq $expected })
if ($fixtureProcesses.Count -ne 1) {
    [pscustomobject]@{name='p00-idle-observation';status='BLOCKED';timestamp=[DateTime]::UtcNow.ToString('o');exit_code=2;command='powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-idle-check.ps1';artifact_path='reports/idle-observation.json';scope='Process telemetry could not uniquely identify the project release fixture; no idle CPU/memory acceptance'} | ConvertTo-Json | Set-Content -Encoding utf8 reports/idle-observation.json
    Write-Host 'BLOCKED: process telemetry could not uniquely select the project release fixture'
    exit 2
}
$target = $fixtureProcesses[0]
$stamp = [DateTime]::UtcNow.ToString('o')
$samples = [System.Collections.Generic.List[object]]::new()
$clock = [System.Diagnostics.Stopwatch]::StartNew()
$startCpu = $target.TotalProcessorTime.TotalSeconds
for ($index = 0; $index -lt 30; $index++) {
    Start-Sleep -Seconds 2
    $target.Refresh()
    if ($target.HasExited) { throw 'Fixture exited during observation; no completed idle measurement' }
    $samples.Add([pscustomobject]@{elapsed_seconds=$clock.Elapsed.TotalSeconds;cpu_seconds=$target.TotalProcessorTime.TotalSeconds-$startCpu;private_bytes=$target.PrivateMemorySize64;working_set_bytes=$target.WorkingSet64})
}
$last = $samples[$samples.Count - 1]
$oneCorePercent = 100.0 * $last.cpu_seconds / $last.elapsed_seconds
$logical = [Environment]::ProcessorCount
$record = [pscustomobject]@{name='p00-idle-observation';status='PASS';timestamp=$stamp;exit_code=0;command='powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-idle-check.ps1';artifact_path='reports/idle-observation.json';scope='30 process telemetry samples over at least 60 seconds on observed hardware; collection only, not release budget acceptance';one_core_cpu_percent=$oneCorePercent;machine_cpu_percent=$oneCorePercent/$logical;logical_processors=$logical;samples=$samples.ToArray()}
$record | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 reports/idle-observation.json
Write-Host ('Observed {0:N1}s, machine CPU {1:N3}%, private {2:N1} MiB; no full performance acceptance' -f $last.elapsed_seconds, ($oneCorePercent/$logical), ($last.private_bytes/1MB))
