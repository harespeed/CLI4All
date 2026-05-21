$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$OutputDir = Join-Path $RepoRoot "data/raw/generated"
$OutputPath = Join-Path $OutputDir "windows_cmd_commands.json"
$SeedPath = Join-Path $PSScriptRoot "windows_cmd_seed.txt"

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$records = New-Object System.Collections.Generic.List[object]
$seen = @{}

function Add-Record {
    param(
        [string]$Name,
        [string]$Kind,
        [string]$DetectedFrom,
        [string]$Notes
    )

    if ([string]::IsNullOrWhiteSpace($Name)) {
        return
    }

    $key = "$($Name.ToLowerInvariant())|$($Kind.ToLowerInvariant())|$DetectedFrom|$Notes"
    if ($seen.ContainsKey($key)) {
        return
    }
    $seen[$key] = $true

    $record = [ordered]@{
        name            = $Name
        source_platform = "windows"
        shell           = "cmd"
        kind            = $Kind
        detected_from   = $DetectedFrom
    }
    if ($Notes) {
        $record["notes"] = $Notes
    }
    $records.Add([pscustomobject]$record)
}

if (Test-Path $SeedPath) {
    Get-Content $SeedPath | ForEach-Object {
        Add-Record -Name $_ -Kind "internal" -DetectedFrom "windows_cmd_seed.txt" -Notes ""
    }
}

try {
    cmd /c help 2>$null | ForEach-Object {
        if ($_ -match '^\s{2,}([A-Z0-9][A-Z0-9._-]+)\s') {
            Add-Record -Name $matches[1].ToLowerInvariant() -Kind "internal" -DetectedFrom "cmd /c help" -Notes ""
        }
    }
} catch {
}

Get-Command -CommandType Application | ForEach-Object {
    Add-Record -Name $_.Name -Kind "external" -DetectedFrom "Get-Command -CommandType Application" -Notes $_.Source
}

$records |
    Sort-Object name, kind, detected_from |
    ConvertTo-Json -Depth 6 |
    Set-Content -Encoding UTF8 $OutputPath

Write-Host "Wrote $($records.Count) records to $OutputPath"
