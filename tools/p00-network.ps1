[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
$checks = foreach ($hostname in @('index.crates.io', 'coolify.be-learning.io.vn')) {
    $timestamp = [DateTime]::UtcNow.ToString('o')
    try {
        $answer = Resolve-DnsName $hostname -ErrorAction Stop
        [pscustomobject]@{name=$hostname;status='PASS';command="Resolve-DnsName $hostname";exit_code=0;timestamp=$timestamp;detail='DNS resolved; TCP/auth/transaction not implied'}
    } catch {
        [pscustomobject]@{name=$hostname;status='BLOCKED';command="Resolve-DnsName $hostname";exit_code=$null;timestamp=$timestamp;detail='DNS lookup failed in execution environment; no URI or credentials logged'}
    }
}
$checks | ConvertTo-Json | Set-Content -Encoding utf8 reports/network-checks.json
$checks | Select-Object name,status | Format-Table
if ($checks | Where-Object status -ne 'PASS') { exit 2 }
