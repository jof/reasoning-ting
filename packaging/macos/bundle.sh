#!/usr/bin/env bash
# Build, bundle, sign, notarize, and DMG the macOS menu-bar app.
# Run on a Mac with Xcode command-line tools + a Developer ID certificate.
#
#   SIGN_ID="Developer ID Application: Your Name (TEAMID)" \
#   NOTARY_PROFILE="some-ting"   # from: xcrun notarytool store-credentials \
#   ./packaging/macos/bundle.sh
#
# Skips signing/notarization if SIGN_ID is unset (produces an unsigned .app for
# local testing — Gatekeeper will require a right-click→Open).
set -euo pipefail
cd "$(dirname "$0")/../.."

APP="dist/some-ting.app"
BIN="listener/target/release/some-ting"
PLIST="packaging/macos/Info.plist"
ENTS="packaging/macos/entitlements.plist"

echo "==> building release (gui)"
( cd listener && cargo build --release --features gui --bin some-ting )

echo "==> assembling $APP"
rm -rf "$APP" dist/some-ting.dmg
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/some-ting"
cp "$PLIST" "$APP/Contents/Info.plist"
# Optional icon: packaging/macos/AppIcon.icns -> Resources (add CFBundleIconFile to Info.plist)
[ -f packaging/macos/AppIcon.icns ] && cp packaging/macos/AppIcon.icns "$APP/Contents/Resources/"

if [ -n "${SIGN_ID:-}" ]; then
    echo "==> codesign (hardened runtime)"
    codesign --force --options runtime --timestamp \
        --entitlements "$ENTS" --sign "$SIGN_ID" "$APP"
    codesign --verify --strict --verbose=2 "$APP"

    echo "==> dmg"
    hdiutil create -volname some-ting -srcfolder "$APP" -ov -format UDZO dist/some-ting.dmg

    if [ -n "${NOTARY_PROFILE:-}" ]; then
        echo "==> notarize + staple"
        xcrun notarytool submit dist/some-ting.dmg --keychain-profile "$NOTARY_PROFILE" --wait
        xcrun stapler staple "$APP"
        hdiutil create -volname some-ting -srcfolder "$APP" -ov -format UDZO dist/some-ting.dmg
    else
        echo "(NOTARY_PROFILE unset — skipping notarization)"
    fi
else
    echo "(SIGN_ID unset — unsigned .app at $APP; right-click→Open to run locally)"
fi
echo "done: $APP"
