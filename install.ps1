$ErrorActionPreference = 'Stop'

$repo = 'BG2MKJ/WinMosh'
$installDir = "$env:LOCALAPPDATA\WinMosh"
$binDir = $installDir

Write-Host 'WinMosh Installer' -ForegroundColor Cyan
Write-Host "Install directory: $installDir" -ForegroundColor Gray

# Create install directory
New-Item -ItemType Directory -Force -Path $installDir | Out-Null

# Download latest release
$releaseUrl = "https://api.github.com/repos/$repo/releases/latest"
Write-Host 'Fetching latest release...' -ForegroundColor Gray
$release = Invoke-RestMethod -Uri $releaseUrl -Headers @{ 'Accept' = 'application/vnd.github+json' }

$asset = $release.assets | Where-Object { $_.name -eq 'winmosh.exe' }
if (-not $asset) {
    Write-Host 'ERROR: winmosh.exe not found in latest release. Trying zip fallback...' -ForegroundColor Yellow
    $zipAsset = $release.assets | Where-Object { $_.name -like 'winmosh-windows-x86_64.zip' }
    if (-not $zipAsset) {
        Write-Host 'ERROR: No release asset found.' -ForegroundColor Red
        exit 1
    }
    Write-Host "Downloading $($zipAsset.name)..." -ForegroundColor Gray
    $zipPath = Join-Path $env:TEMP 'winmosh-release.zip'
    Invoke-WebRequest -Uri $zipAsset.browser_download_url -OutFile $zipPath
    Write-Host 'Extracting...' -ForegroundColor Gray
    Expand-Archive -Path $zipPath -DestinationPath $installDir -Force
    Remove-Item $zipPath
} else {
    Write-Host "Downloading $($asset.name) ($('{0:N0}' -f $asset.size) bytes)..." -ForegroundColor Gray
    $exePath = Join-Path $installDir 'winmosh.exe'
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exePath
}

# Verify installation
$finalExe = Join-Path $installDir 'winmosh.exe'
if (-not (Test-Path $finalExe)) {
    Write-Host 'ERROR: Installation failed - winmosh.exe not found.' -ForegroundColor Red
    exit 1
}

$version = & $finalExe version 2>&1
Write-Host "Installed: $version" -ForegroundColor Green

# Add to PATH
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$installDir*") {
    Write-Host 'Adding to user PATH...' -ForegroundColor Gray
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$installDir", 'User')
    Write-Host 'PATH updated. Restart your terminal or run: refreshenv' -ForegroundColor Yellow
    # Also update current session
    $env:Path = "$env:Path;$installDir"
} else {
    Write-Host 'Already in PATH.' -ForegroundColor Gray
}

Write-Host ''
Write-Host 'WinMosh installed successfully!' -ForegroundColor Green
Write-Host "Run 'winmosh user@host' to connect." -ForegroundColor White
Write-Host "Run 'winmosh alias add myserver user@host' to save a connection." -ForegroundColor White
