#!/bin/sh
# Builds the Mac app from this crate's release binary, with Locust's own icon, ad-hoc signed. Nothing is uploaded.
#
#   scripts/bundle-mac.sh                "Locust Native.app", bundle id lol.locust.native.preview: beside Electron's.
#   scripts/bundle-mac.sh --release      "Locust.app", Electron's bundle id com.automatedworkflow.locust (so macOS's
#                                        microphone, screen and folder permissions stay with the same app), packed into
#                                        Locust-<version>-mac-<arm64|x64>.dmg with Locust.app at its root: the shape
#                                        Electron's Mac updater installs (apps/desktop/src/main/mac-self-update.ts).
#   --out <folder>                       where the app (and the disk image) go; this crate's target folder by default.
#
# In every mode CFBundleShortVersionString is this crate's version (Cargo.toml), and LOCUST_BUILD_COMMIT, when set, is
# handed to the build. Tested by core/src/bundle_mac_tests.rs.
# Local use only: the app isn't notarized, so macOS asks once (Control-click, Open).
set -e
HERE="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$HERE/target}"
MODE=preview
OUT="$TARGET"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --release) MODE=release ;;
    --out)
      [ -n "${2:-}" ] || { echo "bundle-mac.sh: --out needs a folder" >&2; exit 2; }
      OUT="$2"; shift ;;
    *) echo "bundle-mac.sh: unknown argument $1 (it takes --release and --out <folder>)" >&2; exit 2 ;;
  esac
  shift
done
VERSION="$(sed -n '/^\[package\]/,/^\[/s/^version *= *"\([^"]*\)".*/\1/p' "$HERE/Cargo.toml" | head -n 1)"
[ -n "$VERSION" ] || { echo "bundle-mac.sh: no [package] version in $HERE/Cargo.toml" >&2; exit 1; }

if [ -n "${LOCUST_BUILD_COMMIT:-}" ]; then
  export LOCUST_BUILD_COMMIT
  echo "building $VERSION at commit $LOCUST_BUILD_COMMIT"
fi
(cd "$HERE" && cargo build --release)

if [ "$MODE" = release ]; then
  NAME="Locust"; ID="com.automatedworkflow.locust"
  # Electron's Locust claims locust:// links (electron-builder.yml protocols); the copy that takes its place does too.
  LINKS=1
else
  NAME="Locust Native"; ID="lol.locust.native.preview"
  # locust:// links (a message ready in the box, core/src/locust_link.rs). Only when asked: while the Electron app is
  # installed beside it, both claiming the scheme would leave which one opens a link to the system.
  LINKS="${LOCUST_NATIVE_LINKS:-}"
fi
mkdir -p "$OUT" && OUT="$(cd "$OUT" && pwd)"
APP="$OUT/$NAME.app"
rm -rf "$APP" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET/release/locust-native" "$APP/Contents/MacOS/locust-native"
# The icon comes from the desktop app's own art, turned into an .icns with the system's sips and iconutil.
ICONS="$(mktemp -d)"
SET="$ICONS/icon.iconset" && mkdir -p "$SET"
SRC="$REPO/design/locust-desktop/brand/app-icon/icon-1024.png"
for s in 16 32 128 256 512; do
  sips -z $s $s "$SRC" --out "$SET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) "$SRC" --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o "$APP/Contents/Resources/icon.icns"
rm -rf "$ICONS"
URLS=""
if [ "$LINKS" = "1" ]; then
  URLS="<key>CFBundleURLTypes</key><array><dict><key>CFBundleURLName</key><string>Locust link</string><key>CFBundleURLSchemes</key><array><string>locust</string></array></dict></array>"
fi
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>$NAME</string>
<key>CFBundleDisplayName</key><string>$NAME</string>
<key>CFBundleIdentifier</key><string>$ID</string>
<key>CFBundleExecutable</key><string>locust-native</string>
<key>CFBundleIconFile</key><string>icon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$VERSION</string>
<key>CFBundleVersion</key><string>$VERSION</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSMicrophoneUsageDescription</key><string>Locust uses the microphone only while you use voice typing or a voice call.</string>
$URLS
</dict></plist>
PLIST
codesign --force --sign - "$APP" >/dev/null 2>&1
echo "$APP ($VERSION, $ID, $(du -sh "$APP" | cut -f1))"

if [ "$MODE" = release ]; then
  case "$(uname -m)" in
    arm64) ARCH=arm64 ;;
    x86_64) ARCH=x64 ;;
    *) echo "bundle-mac.sh: no Locust disk image for $(uname -m)" >&2; exit 1 ;;
  esac
  DMG="$OUT/Locust-$VERSION-mac-$ARCH.dmg"
  # The image's root: Locust.app (the one thing Electron's updater looks for) and, for a person's drag-install, a link
  # to /Applications. ditto keeps the app's signature as it is.
  STAGE="$(mktemp -d)"
  ditto "$APP" "$STAGE/Locust.app"
  ln -s /Applications "$STAGE/Applications"
  rm -f "$DMG"
  hdiutil create -quiet -volname Locust -srcfolder "$STAGE" -format UDZO -ov "$DMG"
  rm -rf "$STAGE"
  echo "$DMG ($(du -sh "$DMG" | cut -f1))"
fi
