# Citadel standalone installer for Windows (PowerShell).
#
# Downloads a self-contained bundle from GitHub Releases.
# No Node.js, no build tools required.
#
#   irm https://raw.githubusercontent.com/antonygiomarxdev/citadel/main/install.ps1 | iex
#
# Re-run to upgrade.
#
# Environment:
#   CITADEL_VERSION          release tag to install (default: latest)
#   CITADEL_INSTALL_DIR      install location (default: %LOCALAPPDATA%\citadel)

$ErrorActionPreference = 'Stop'
$repo = 'antonygiomarxdev/citadel'
$installDir = if ($env:CITADEL_INSTALL_DIR) { $env:CITADEL_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'citadel' }

$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') { 'arm64' } else { 'x64' }
$target = "win32-$arch"

$version = $env:CITADEL_VERSION
if (-not $version) {
  $version = (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest").tag_name
}
if (-not $version) { throw "citadel: could not resolve latest version; set CITADEL_VERSION." }

$url = "https://github.com/$repo/releases/download/$version/citadel-$target.zip"
Write-Host "Installing Citadel $version ($target)..."
$tmp = Join-Path $env:TEMP ("citadel-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$zip = Join-Path $tmp 'citadel.zip'
Invoke-WebRequest -Uri $url -OutFile $zip

$dest = Join-Path $installDir 'current'
if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Expand-Archive -Path $zip -DestinationPath $dest -Force
$inner = Join-Path $dest "citadel-$target"
if (Test-Path $inner) {
  Get-ChildItem -Force $inner | Move-Item -Destination $dest -Force
  Remove-Item -Recurse -Force $inner
}
Remove-Item -Recurse -Force $tmp

$binDir = Join-Path $dest 'bin'
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $binDir) {
  [Environment]::SetEnvironmentVariable('Path', "$binDir;$userPath", 'User')
  Write-Host "Added $binDir to your PATH (restart your terminal to pick it up)."
}

Write-Host "Installed to $dest"
Write-Host "Run: citadel --help"
