# Uninstall ScreenCapture (shortcuts, autostart entry, program files).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File uninstall.ps1
#   powershell -ExecutionPolicy Bypass -File uninstall.ps1 -PurgeConfig
#
# -PurgeConfig also deletes %APPDATA%\ScreenCapture (settings and hotkeys).
param(
    [switch]$PurgeConfig
)

$ErrorActionPreference = "SilentlyContinue"

$defaultDir = Join-Path $env:LOCALAPPDATA "Programs\ScreenCapture"
$dir = $PSScriptRoot
if (-not (Test-Path (Join-Path $dir "ScreenCapture.exe")) -and (Test-Path (Join-Path $defaultDir "ScreenCapture.exe"))) {
    $dir = $defaultDir
}

# Shortcuts
Remove-Item (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\ScreenCapture.lnk") -Force
Remove-Item (Join-Path ([Environment]::GetFolderPath("Desktop")) "ScreenCapture.lnk") -Force

# Autostart entry
Remove-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "ScreenCapture"

# Running process
Get-Process screencapture -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 400

# Program files (keep this script so the user can rerun it)
Get-ChildItem $dir -Force | Where-Object { $_.Name -ne "uninstall.ps1" } | Remove-Item -Recurse -Force

if ($PurgeConfig) {
    Remove-Item (Join-Path $env:APPDATA "ScreenCapture") -Recurse -Force
    Write-Host "Configuration removed."
}

Write-Host "ScreenCapture uninstalled. You can delete this folder manually: $dir"
