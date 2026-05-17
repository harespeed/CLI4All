$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$distDir = Join-Path $repoRoot "dist"
$tempRoot = Join-Path $repoRoot "target\package"
$stageDir = Join-Path $tempRoot "cli4all-windows-x86_64"
$archivePath = Join-Path $distDir "cli4all-windows-x86_64.zip"

New-Item -ItemType Directory -Force $distDir | Out-Null
New-Item -ItemType Directory -Force $tempRoot | Out-Null

Push-Location $repoRoot
try {
    cargo build --release
}
finally {
    Pop-Location
}

if (Test-Path $stageDir) {
    Remove-Item $stageDir -Recurse -Force
}

New-Item -ItemType Directory -Force $stageDir | Out-Null
New-Item -ItemType Directory -Force (Join-Path $stageDir "data") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $stageDir "scripts") | Out-Null
Copy-Item (Join-Path $repoRoot "target\release\cli4all.exe") (Join-Path $stageDir "cli4all.exe") -Force
Copy-Item (Join-Path $repoRoot "README.md") (Join-Path $stageDir "README.md") -Force
Copy-Item (Join-Path $repoRoot "PACKAGING.md") (Join-Path $stageDir "PACKAGING.md") -Force
Copy-Item (Join-Path $repoRoot "data\commands.yaml") (Join-Path $stageDir "data\commands.yaml") -Force
Copy-Item (Join-Path $repoRoot "data\risks.yaml") (Join-Path $stageDir "data\risks.yaml") -Force
Copy-Item (Join-Path $repoRoot "scripts\install_windows.ps1") (Join-Path $stageDir "scripts\install_windows.ps1") -Force

if (Test-Path $archivePath) {
    Remove-Item $archivePath -Force
}

Compress-Archive -Path $stageDir -DestinationPath $archivePath -Force

Write-Output "Created $archivePath"
