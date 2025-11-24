#!/bin/bash
set -e

# Build macOS .app bundle for macport
# This creates a proper app bundle required for launch-at-login functionality

APP_NAME="macport"
BUNDLE_ID="com.github.${USER}.macport"
VERSION="0.1.0"
BUILD_DIR="target/release"
APP_DIR="${BUILD_DIR}/${APP_NAME}.app"

echo "🔨 Building macport.app bundle..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Step 1: Build release binary
echo "📦 Building release binary..."
cargo build --release

# Step 2: Create app bundle structure
echo "📁 Creating app bundle structure..."
rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/Contents/MacOS"
mkdir -p "${APP_DIR}/Contents/Resources"

# Step 3: Copy binary
echo "📋 Copying binary..."
cp "${BUILD_DIR}/${APP_NAME}" "${APP_DIR}/Contents/MacOS/"

# Step 4: Create Info.plist
echo "📝 Creating Info.plist..."
cat > "${APP_DIR}/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# Step 5: Code sign (ad-hoc for local testing)
echo "✍️  Code signing (ad-hoc)..."
codesign --force --deep --sign - "${APP_DIR}"

# Step 6: Verify
echo "✅ Verifying bundle..."
codesign --verify --verbose "${APP_DIR}"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✨ Success! App bundle created at:"
echo "   ${APP_DIR}"
echo ""
echo "📍 To install and test:"
echo "   1. Copy to Applications:"
echo "      cp -r ${APP_DIR} /Applications/"
echo ""
echo "   2. Run from Applications:"
echo "      open /Applications/${APP_NAME}.app"
echo ""
echo "   3. Or run directly:"
echo "      open ${APP_DIR}"
echo ""
echo "💡 Bundle ID: ${BUNDLE_ID}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
