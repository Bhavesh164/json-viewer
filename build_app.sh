#!/usr/bin/env bash
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$DIR"

echo "==> Building JSONViewer in Release mode..."
swift build -c release

# Use .noindex directory so macOS Spotlight and Launchpad never index intermediate build bundles
BUILD_DIR="build.noindex"
mkdir -p "$BUILD_DIR"
touch "$BUILD_DIR/.metadata_never_index"

APP_BUNDLE="$BUILD_DIR/JSONViewer.app"
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
    ICONSET="$BUILD_DIR/AppIcon.iconset"
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

echo "==> Setting permissions & ad-hoc code signing..."
chmod +x "$MACOS/JSONViewer"
codesign --force --deep --sign - "$APP_BUNDLE"

mkdir -p build
ZIP_BUNDLE="build/JSONViewer-macOS.zip"
echo "==> Packaging standalone zip: $ZIP_BUNDLE..."
rm -f "$ZIP_BUNDLE"
ditto -c -k --sequesterRsrc --keepParent "$APP_BUNDLE" "$ZIP_BUNDLE"

# Remove any old unindexed app in build/ so Spotlight never sees two apps
rm -rf build/JSONViewer.app

echo "==> Installing latest build to /Applications/JSONViewer.app..."
killall JSONViewer 2>/dev/null || true
rm -rf /Applications/JSONViewer.app
cp -R "$APP_BUNDLE" /Applications/JSONViewer.app
xattr -cr /Applications/JSONViewer.app

echo "==> Successfully installed latest JSONViewer to /Applications/JSONViewer.app"
echo "==> Standalone zip available at $ZIP_BUNDLE"

if [ "$1" == "run" ] || [ "$1" == "install" ]; then
    echo "==> Launching /Applications/JSONViewer.app..."
    open /Applications/JSONViewer.app
fi
