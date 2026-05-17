$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$packageRoot = Split-Path -Parent $scriptDir
$binarySource = Join-Path $packageRoot "cli4all.exe"
$dataSource = Join-Path $packageRoot "data"
$installRoot = Join-Path $env:LOCALAPPDATA "CLI4ALL"
$binDir = Join-Path $installRoot "bin"
$dataDir = Join-Path $installRoot "data"

if (-not (Test-Path $binarySource)) {
    throw "cli4all.exe was not found next to install_windows.ps1"
}

if (-not (Test-Path $dataSource)) {
    throw "data directory was not found next to install_windows.ps1"
}

New-Item -ItemType Directory -Force $binDir | Out-Null
New-Item -ItemType Directory -Force $dataDir | Out-Null

Copy-Item $binarySource (Join-Path $binDir "cli4all.exe") -Force
Copy-Item (Join-Path $packageRoot "README.md") (Join-Path $installRoot "README.md") -Force
Copy-Item (Join-Path $packageRoot "PACKAGING.md") (Join-Path $installRoot "PACKAGING.md") -Force
Copy-Item (Join-Path $dataSource "commands.yaml") (Join-Path $dataDir "commands.yaml") -Force
Copy-Item (Join-Path $dataSource "risks.yaml") (Join-Path $dataDir "risks.yaml") -Force

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = @()

if ($userPath) {
    $pathEntries = $userPath.Split(";") | Where-Object { $_ -ne "" }
}

if ($pathEntries -notcontains $binDir) {
    $newPath = (($pathEntries + $binDir) | Select-Object -Unique) -join ";"
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Output "Added $binDir to the user PATH."
}
else {
    Write-Output "$binDir is already on the user PATH."
}

Write-Output "Installed cli4all.exe to $binDir"
Write-Output "Installed YAML data to $dataDir"
Write-Output "Restart PowerShell or open a new terminal to use the updated PATH."
