#!/bin/sh
# Build the Rust engine and the macOS player app (RisingLands.app) from this repository.
# Usage: ./build-macos.sh [output-directory]   (default: ./build)
# Requires: macOS 13+ on Apple Silicon, Xcode Command Line Tools, Rust (rustup).
# The app bundle contains code only; the game data is imported from the player's own CD at first run.
set -eu
ROOT=$(cd "$(dirname "$0")" && pwd)
OUT=${1:-"$ROOT/build"}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
ENGINE="$ROOT/engine"
SRC="$ROOT/macos/src"
TARGET="$OUT/engine-target"
OBJ="$OUT/obj"
APP="$OUT/RisingLands.app"
VERSION=${RISING_VERSION:-0.1.0}
BUNDLE_ID=${RISING_BUNDLE_ID:-local.risinglands.ptbr.player}
BUILD_NUMBER=${RISING_BUILD_NUMBER:-1}
REVISION=$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)

echo "==> Engine (Rust)"
cargo build --release --locked --manifest-path "$ENGINE/Cargo.toml" --target-dir "$TARGET"
cargo rustc --release --locked -p rl-ffi --lib --crate-type staticlib \
    --manifest-path "$ENGINE/Cargo.toml" --target-dir "$TARGET"
STATIC="$TARGET/release/librising_engine.a"

EVENT_FLC=""
if grep -q rl_eventos_flc_v1 "$ENGINE/include/rising_engine.h"; then EVENT_FLC="-DMACOS_EVENT_FLC"; fi

echo "==> Native host objects (C)"
mkdir -p "$OBJ"
CFLAGS="-O3 -c -arch arm64 -mmacosx-version-min=13.0 -Wno-deprecated-declarations"
# shellcheck disable=SC2086
xcrun clang $CFLAGS -DMACOS_STATIC_ENGINE $EVENT_FLC -I "$ENGINE/include" "$SRC/macos_live_driver.c" -o "$OBJ/macos-live-static.o"
# shellcheck disable=SC2086
xcrun clang $CFLAGS "$SRC/macos_projection_codec.c" -o "$OBJ/macos-codec-static.o"
# shellcheck disable=SC2086
xcrun clang $CFLAGS "$SRC/macos_audio_mapper.c" -o "$OBJ/macos-audio-static.o"
# shellcheck disable=SC2086
xcrun clang $CFLAGS -I "$ENGINE/include" "$SRC/events_host_file.c" -o "$OBJ/macos-events-static.o"

echo "==> Player app (Swift)"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
SWIFT_SOURCES=""
for name in NativeCampaignMenu NativeAudio NativeCDMusicDisc NativeCDMusicPlayer NativeCDMusicHost \
            NativeHotkeyBindings NativeHotkeyInputRouter NativeModernHotkeys NativeHotkeyPreferences NativeHotkeyLabels \
            NativeViewportPolicy NativeGameSpeed NativePlayerOptions NativeBenchmarkArguments NativeDefeatMovie \
            NativeFLC NativeEventFLCPlayback NativeSaveLibrary NativeLiveWindow NativePlayerEdition NativeCDImport \
            NativePlayerImport NativePlayerApp NativeFirstUseSmoke NativeSaveManager NativeSaveManagerSmoke SmokeReadiness main; do
    SWIFT_SOURCES="$SWIFT_SOURCES $name.swift"
done
# Source names are relative to $SRC (no spaces), so the list may be split by the shell.
cd "$SRC"
# shellcheck disable=SC2086
xcrun swiftc -swift-version 5 -O -target arm64-apple-macosx13.0 -DRISING_PLAYER_EDITION \
    -module-cache-path "$OUT/module-cache" \
    -framework AppKit -framework WebKit -framework Carbon -framework Metal -framework QuartzCore -framework AVFoundation \
    -I "$ENGINE/include" -import-objc-header "$SRC/MacOSLiveDriver.h" \
    "$OBJ/macos-live-static.o" "$OBJ/macos-codec-static.o" "$OBJ/macos-audio-static.o" "$OBJ/macos-events-static.o" \
    "$STATIC" $SWIFT_SOURCES -o "$APP/Contents/MacOS/RisingLands"
cd "$ROOT"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>RisingLands</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleName</key><string>Rising Lands</string>
  <key>CFBundleDisplayName</key><string>Rising Lands</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$BUILD_NUMBER</string>
  <key>CFBundleDevelopmentRegion</key><string>pt-BR</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.strategy-games</string>
  <key>RisingLandsSourceCommit</key><string>$REVISION</string>
  <key>RisingLandsBuildMode</key><string>public</string>
  <key>RisingLandsEdition</key><string>player</string>
  <key>RisingLandsEventsDirectory</key><string>C:\\RISING</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSArchitecturePriority</key><array><string>arm64</string></array>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST

echo "==> CD import recipes (metadata only: CD file names, SHA-256, sample offsets)"
mkdir -p "$APP/Contents/Resources/player"
cp -R "$ROOT/macos/player/." "$APP/Contents/Resources/player/"

# Ad hoc by default. Set RISING_SIGN_IDENTITY (for example "Developer ID Application: Name (TEAMID)")
# to sign for distribution: hardened runtime and a secure timestamp, as notarization requires.
SIGN_IDENTITY=${RISING_SIGN_IDENTITY:--}
if [ "$SIGN_IDENTITY" = "-" ]; then
    codesign --force --sign - --options runtime --timestamp=none "$APP"
else
    codesign --force --sign "$SIGN_IDENTITY" --options runtime --timestamp "$APP"
fi
codesign --verify --deep --strict "$APP"
echo "==> $APP"
