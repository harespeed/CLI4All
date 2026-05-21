$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$OutputDir = Join-Path $RepoRoot "data/raw/generated"
$OutputPath = Join-Path $OutputDir "powershell_commands.json"

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$records = Get-Command * | ForEach-Object {
    $record = [ordered]@{
        name            = $_.Name
        source_platform = "windows"
        shell           = "powershell"
        kind            = $_.CommandType.ToString().ToLowerInvariant()
        detected_from   = "Get-Command *"
    }

    if ($_.Source) {
        $record["notes"] = "source=$($_.Source)"
    }
    if ($_.ModuleName) {
        $record["module_name"] = $_.ModuleName
    }
    if ($_.Version) {
        $record["version"] = $_.Version.ToString()
    }

    [pscustomobject]$record
} | Sort-Object name, kind -Unique

$records | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $OutputPath
Write-Host "Wrote $($records.Count) records to $OutputPath"
