$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\..\p00-evidence-store.ps1"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$directory = Join-Path $root ('.local/evidence-store-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $directory -Force | Out-Null
$path = Join-Path $directory 'checks.json'
Save-P00Checks $path @([pscustomobject]@{name='mongo-runtime';status='PASS'}, [pscustomobject]@{name='format';status='PASS'})
Save-P00Checks $path @([pscustomobject]@{name='format';status='FAIL'}, [pscustomobject]@{name='journal-unit';status='PASS'})
$saved = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
if ($saved.Count -ne 3) { throw 'Expected exactly three flat records' }
if (($saved | Where-Object name -eq 'mongo-runtime').status -ne 'PASS') { throw 'Unrerun Mongo evidence was lost' }
if (($saved | Where-Object name -eq 'format').status -ne 'FAIL') { throw 'New failure must replace old pass' }
Save-P00Checks (Join-Path $directory 'single.json') @([pscustomobject]@{name='only';status='PASS'})
$single = Get-Content -LiteralPath (Join-Path $directory 'single.json') -Raw
if (-not $single.TrimStart().StartsWith('[')) { throw 'Single records must remain a JSON array' }
Write-Host 'PASS: flat JSON array, unchanged Mongo evidence, failure supersession and single-record array on this PowerShell runtime'
