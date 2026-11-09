#!/bin/sh
# Builds "Locust Native.app" next to this crate's target folder: the release binary, Locust's own icon, ad-hoc signed.
# Local use only: the app isn't notarized, so macOS asks once (Control-click, Open).
set -e
HERE="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$HERE/target}"
(cd "$HERE" && cargo build --release)
APP="$TARGET/Locust Native.app"
rm -rf "$APP" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET/release/locust-native" "$APP/Contents/MacOS/locust-native"
# The icon comes from the desktop app's own art, turned into an .icns with the system's sips and iconutil.
SET="$(mktemp -d)/icon.iconset" && mkdir -p "$SET"
SRC="$REPO/design/locust-desktop/brand/app-icon/icon-1024.png"
for s in 16 32 128 256 512; do
  sips -z $s $s "$SRC" --out "$SET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) "$SRC" --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o "$APP/Contents/Resources/icon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Locust Native</string>
<key>CFBundleDisplayName</key><string>Locust Native</string>
<key>CFBundleIdentifier</key><string>lol.locust.native.preview</string>
<key>CFBundleExecutable</key><string>locust-native</string>
<key>CFBundleIconFile</key><string>icon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$APP" >/dev/null 2>&1
echo "$APP ($(du -sh "$APP" | cut -f1))"
