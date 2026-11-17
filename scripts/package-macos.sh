#!/usr/bin/env bash
# 构建 macOS .app 应用包，并生成 zip（可选 dmg）。
#
# 用法：
#   bash scripts/package-macos.sh            # 版本号自动读取 Cargo.toml
#   bash scripts/package-macos.sh 0.8.0      # 指定版本号
#   TARGET_ARCH=universal bash scripts/package-macos.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)}"
APP_NAME="ScreenCapture"
BUNDLE_ID="com.screencapture.app"
ARCH="${TARGET_ARCH:-$(uname -m)}"

echo "Building $APP_NAME $VERSION ($ARCH) ..."
cargo build --release

BIN="target/release/screencapture"
if [ ! -f "$BIN" ]; then
    echo "找不到构建产物：$BIN" >&2
    exit 1
fi

APP_DIR="dist/$APP_NAME.app"
CONTENTS="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES"
cp "$BIN" "$MACOS_DIR/$APP_NAME"
chmod +x "$MACOS_DIR/$APP_NAME"
cp "assets/icon.icns" "$RESOURCES/$APP_NAME.icns"

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>$APP_NAME.icns</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHumanReadableCopyright</key>
    <string>© 2026 风不休 &lt;dreign@qq.com&gt; · https://hefeng.chat/</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
PLIST

ZIP="dist/$APP_NAME-$VERSION-macos-$ARCH.zip"
rm -f "$ZIP"
(cd dist && ditto -c -k --keepParent "$APP_NAME.app" "$(basename "$ZIP")")
echo "Package created: $ZIP"

# 安装了 create-dmg 时额外生成 dmg（brew install create-dmg）
if command -v create-dmg >/dev/null 2>&1; then
    DMG="dist/$APP_NAME-$VERSION-macos-$ARCH.dmg"
    rm -f "$DMG"
    create-dmg --volname "$APP_NAME" --app-drop-link 480 200 "$DMG" "$APP_DIR" >/dev/null \
        && echo "DMG created: $DMG"
fi
