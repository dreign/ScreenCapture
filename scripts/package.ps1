# Build a release binary and produce a portable zip package.
#
# Usage (from any directory):
#   powershell -ExecutionPolicy Bypass -File scripts\package.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\package.ps1 -Version 0.8.0
param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $Version) {
    $line = Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    $Version = $line.Matches[0].Groups[1].Value
}

Write-Host "Building ScreenCapture $Version ..."
cargo build --release

$dist = Join-Path $root "dist"
$pkgName = "ScreenCapture-$Version-win64"
$pkgDir = Join-Path $dist $pkgName
if (Test-Path $pkgDir) {
    Remove-Item $pkgDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $pkgDir | Out-Null

Copy-Item "target\release\screencapture.exe" (Join-Path $pkgDir "ScreenCapture.exe")
Copy-Item "README.md" $pkgDir
Copy-Item "scripts\install.ps1" $pkgDir
Copy-Item "scripts\uninstall.ps1" $pkgDir
New-Item -ItemType Directory -Force -Path (Join-Path $pkgDir "docs") | Out-Null
Copy-Item "docs\user-guide.md" (Join-Path $pkgDir "docs\user-guide.md")

$zip = Join-Path $dist "$pkgName.zip"
if (Test-Path $zip) {
    Remove-Item $zip -Force
}
Compress-Archive -Path $pkgDir -DestinationPath $zip -CompressionLevel Optimal

Write-Host "Package created: $zip"
