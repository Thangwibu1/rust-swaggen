# Shared by the runner and the PowerShell 5.1 regression check. No process execution.
function Save-P00Checks([string]$Path, [object[]]$Records) {
    $merged = [System.Collections.Generic.List[object]]::new()
    if (Test-Path -LiteralPath $Path) {
        $previous = Get-Content -Encoding utf8 -LiteralPath $Path -Raw | ConvertFrom-Json
        foreach ($item in $previous) {
            if (-not $item.name) { throw 'Invalid prior evidence record; preserve and repair it before rerunning' }
            if ($item.name -notin @($Records | ForEach-Object { $_.name })) { $merged.Add($item) }
        }
    }
    foreach ($item in $Records) {
        if (-not $item.name) { throw 'Invalid new evidence record' }
        $merged.Add($item)
    }
    ConvertTo-Json -InputObject $merged.ToArray() -Depth 8 | Set-Content -Encoding utf8 -LiteralPath $Path
}
