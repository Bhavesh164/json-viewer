#!/usr/bin/env bash
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$DIR"

echo "==> Building JSONViewer in Release mode..."
swift build -c release

APP_BUNDLE="build/JSONViewer.app"
CONTENTS="$APP_BUNDLE/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

echo "==> Creating macOS App Bundle structure..."
rm -rf "$APP_BUNDLE"
mkdir -p "$MACOS"
mkdir -p "$RESOURCES"

echo "==> Copying binary and metadata..."
cp ".build/release/JSONViewer" "$MACOS/JSONViewer"
cp "Resources/Info.plist" "$CONTENTS/Info.plist"

# Generate .icns if AppIcon.png exists
if [ -f "Resources/AppIcon.png" ]; then
    echo "==> Creating iconset..."
    ICONSET="build/AppIcon.iconset"
    mkdir -p "$ICONSET"
    sips -z 16 16     Resources/AppIcon.png --out "$ICONSET/icon_16x16.png" >/dev/null 2>&1 || true
    sips -z 32 32     Resources/AppIcon.png --out "$ICONSET/icon_16x16@2x.png" >/dev/null 2>&1 || true
    sips -z 32 32     Resources/AppIcon.png --out "$ICONSET/icon_32x32.png" >/dev/null 2>&1 || true
    sips -z 64 64     Resources/AppIcon.png --out "$ICONSET/icon_32x32@2x.png" >/dev/null 2>&1 || true
    sips -z 128 128   Resources/AppIcon.png --out "$ICONSET/icon_128x128.png" >/dev/null 2>&1 || true
    sips -z 256 256   Resources/AppIcon.png --out "$ICONSET/icon_128x128@2x.png" >/dev/null 2>&1 || true
    sips -z 256 256   Resources/AppIcon.png --out "$ICONSET/icon_256x256.png" >/dev/null 2>&1 || true
    sips -z 512 512   Resources/AppIcon.png --out "$ICONSET/icon_256x256@2x.png" >/dev/null 2>&1 || true
    sips -z 512 512   Resources/AppIcon.png --out "$ICONSET/icon_512x512.png" >/dev/null 2>&1 || true
    sips -z 1024 1024 Resources/AppIcon.png --out "$ICONSET/icon_512x512@2x.png" >/dev/null 2>&1 || true
    iconutil -c icns "$ICONSET" -o "$RESOURCES/AppIcon.icns" >/dev/null 2>&1 || true
    cp Resources/AppIcon.png "$RESOURCES/AppIcon.png" || true
    rm -rf "$ICONSET"
fi

echo "==> Setting permissions..."
chmod +x "$MACOS/JSONViewer"

echo "==> Successfully created $APP_BUNDLE"

if [ "$1" == "run" ]; then
    echo "==> Launching JSONViewer.app..."
    open "$APP_BUNDLE"
fi
