# Install ScreenCapture into the current user's local programs folder.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File install.ps1
#   powershell -ExecutionPolicy Bypass -File install.ps1 -DesktopShortcut
#
# The release zip must be extracted first; this script copies ScreenCapture.exe
# next to it into %LOCALAPPDATA%\Programs\ScreenCapture and creates shortcuts.
param(
    [switch]$NoShortcut,
    [switch]$DesktopShortcut
)

$ErrorActionPreference = "Stop"

$sourceExe = Join-Path $PSScriptRoot "ScreenCapture.exe"
if (-not (Test-Path $sourceExe)) {
    throw "ScreenCapture.exe not found next to install.ps1. Extract the zip first."
}

$targetDir = Join-Path $env:LOCALAPPDATA "Programs\ScreenCapture"
New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

$targetExe = Join-Path $targetDir "ScreenCapture.exe"
Copy-Item $sourceExe $targetExe -Force

$guide = Join-Path $PSScriptRoot "docs\user-guide.md"
if (Test-Path $guide) {
    Copy-Item $guide (Join-Path $targetDir "user-guide.md") -Force
}

$uninstaller = Join-Path $PSScriptRoot "uninstall.ps1"
if (Test-Path $uninstaller) {
    Copy-Item $uninstaller (Join-Path $targetDir "uninstall.ps1") -Force
}

if (-not $NoShortcut) {
    $shell = New-Object -ComObject WScript.Shell

    $startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\ScreenCapture.lnk"
    $shortcut = $shell.CreateShortcut($startMenu)
    $shortcut.TargetPath = $targetExe
    $shortcut.WorkingDirectory = $targetDir
    $shortcut.Description = "ScreenCapture 截图工具"
    $shortcut.Save()

    if ($DesktopShortcut) {
        $desktop = Join-Path ([Environment]::GetFolderPath("Desktop")) "ScreenCapture.lnk"
        $desktopShortcut = $shell.CreateShortcut($desktop)
        $desktopShortcut.TargetPath = $targetExe
        $desktopShortcut.WorkingDirectory = $targetDir
        $desktopShortcut.Save()
    }
}

Write-Host "Installed to: $targetDir"
Write-Host "Start it from the Start Menu shortcut, or run: $targetExe"
$uninstallPath = Join-Path $targetDir "uninstall.ps1"
Write-Host "Uninstall with: powershell -ExecutionPolicy Bypass -File `"$uninstallPath`""
