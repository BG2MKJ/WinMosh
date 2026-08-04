$ErrorActionPreference = 'Stop'
if ($env:MSYSTEM) {
    Write-Host 'Git Bash detected. Add this to ~/.bashrc:' -ForegroundColor Yellow
    Write-Host '  export MSYS_NO_PATHCONV=1' -ForegroundColor White
    Write-Host 'Otherwise Unix paths like /home/user/bin will be converted to Windows paths.' -ForegroundColor Gray
    Write-Host ''
}
$host.UI.RawUI.WindowTitle = 'WinMosh Installer'

# ── ASCII Art ───────────────────────────────────────────
$art = @'

  [36m╔╗ ╦ ╔╗ ╔╗  ╔╗ ╔╗ ╔══╗ ╔═╗
  ║║ ║ ║║ ║║  ║║ ║║ ║╔╗║ ║╔╝
  ║║ ║ ║║ ║║  ║║║║║ ║║║║ ║╚╗
  ╚╝ ╚ ╚╝ ╚╝  ╚╝╚╝╚ ╚╝╚╝ ╚═╝  v__VERSION__
  [0m
  Native Windows Mosh Client  |  github.com/BG2MKJ/WinMosh
'@ -replace '__VERSION__', '0.1.2'

Clear-Host
Write-Host $art

$repo    = 'BG2MKJ/WinMosh'
$dir     = "$env:LOCALAPPDATA\WinMosh"
$exe     = Join-Path $dir 'winmosh.exe'
$wm      = Join-Path $dir 'wm.exe'

# ── Spinner function ────────────────────────────────────
$spinFrames = @('|', '/', '-', '\')
$spinIdx    = 0
function Spin-Step {
    $script:spinIdx = ($script:spinIdx + 1) % 4
    Write-Host "`r  $($spinFrames[$script:spinIdx])  $($args[0])" -NoNewline -ForegroundColor Cyan
}
function Spin-Done {
    Write-Host "`r  +  $($args[0])" -ForegroundColor Green
}
function Spin-Fail {
    Write-Host "`r  x  $($args[0])" -ForegroundColor Red
}

# ── Install ─────────────────────────────────────────────
Spin-Step 'Creating install directory...'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Spin-Done "Install directory: $dir"

Spin-Step 'Fetching latest release info...'
$releaseJson = Invoke-RestMethod `
    -Uri "https://api.github.com/repos/$repo/releases/latest" `
    -Headers @{ 'Accept' = 'application/vnd.github+json' }

$asset = $releaseJson.assets | Where-Object { $_.name -eq 'winmosh.exe' }
$isZip = $false
if (-not $asset) {
    $asset = $releaseJson.assets | Where-Object { $_.name -like 'winmosh-windows-x86_64.zip' }
    $isZip = $true
}
if (-not $asset) {
    Spin-Fail 'No release asset found.'
    exit 1
}
Spin-Done "Found: $($asset.name) ($('{0:N0}' -f $asset.size) bytes)"

Spin-Step 'Downloading...'
$tmp = Join-Path $env:TEMP 'winmosh-dl'
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$dlPath = Join-Path $tmp $asset.name
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $dlPath
Spin-Done 'Download complete'

if ($isZip) {
    Spin-Step 'Extracting...'
    Expand-Archive -Path $dlPath -DestinationPath $dir -Force
    Remove-Item $dlPath
} else {
    Move-Item -Force $dlPath $exe
}
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue

$ver = & $exe version 2>&1
Spin-Done "Installed: $ver"

Spin-Step 'Creating wm alias...'
Copy-Item -Force $exe $wm
Spin-Done 'wm.exe ready'

Spin-Step 'Updating PATH...'
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$dir*") {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$dir", 'User')
    $env:Path = "$env:Path;$dir"
    Spin-Done 'PATH updated'
} else {
    Spin-Done 'Already in PATH'
}

# ── Done ────────────────────────────────────────────────
Write-Host ''
Write-Host '  WinMosh installed!' -ForegroundColor Green
Write-Host ''
Write-Host '  Usage:' -ForegroundColor White
Write-Host '    winmosh user@host         Connect to a server'
Write-Host '    wm     user@host         Same, shorter alias'
Write-Host '    winmosh alias add ...    Save a connection'
Write-Host '    winmosh update --download  Update to latest'
Write-Host '    winmosh --uninstall       Remove everything'
Write-Host ''
Write-Host '  Restart your terminal or run: refreshenv' -ForegroundColor Yellow
Write-Host ''
