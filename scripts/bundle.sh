#!/bin/zsh
# Builds target/GenUI.app: the release binary in a Mac app bundle, signed ad hoc.
set -euo pipefail
cd "${0:A:h}/.."
cargo build --release
app=target/GenUI.app
rm -rf $app
mkdir -p $app/Contents/MacOS
cp target/release/nsh-genui $app/Contents/MacOS/
id=$(sed -n 's/.*"id": *"\([^"]*\)".*/\1/p' shell/nsh.json)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cat > $app/Contents/Info.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>GenUI</string>
  <key>CFBundleDisplayName</key><string>GenUI</string>
  <key>CFBundleIdentifier</key><string>$id</string>
  <key>CFBundleExecutable</key><string>nsh-genui</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - $app
echo "built $app"
