#!/bin/sh
# Standalone component tests of the macOS host (no game data needed).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
SRC="$HERE/../src"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
run() {
    name=$1; shift
    sources=""
    for s in "$@"; do sources="$sources $s.swift"; done
    # shellcheck disable=SC2086
    (cd "$SRC" && xcrun swiftc -swift-version 5 -module-cache-path "$TMP/modules" $sources "$HERE/$name.swift" -o "$TMP/$name")
    "$TMP/$name"
}
run HotkeyBindingsTests NativeHotkeyBindings
run HotkeyInputRouterTests NativeHotkeyBindings NativeHotkeyInputRouter
run HotkeyPreferencesModelTests NativeHotkeyBindings NativeHotkeyPreferences
run ModernHotkeysTests NativeHotkeyBindings NativeHotkeyInputRouter NativeModernHotkeys
run ViewportPolicyTests NativeViewportPolicy
run GameSpeedTests NativeGameSpeed
run PlayerOptionsTests NativePlayerOptions
run BenchmarkArgumentsTests NativeBenchmarkArguments
# Production view objects (NativeMetalGameView, layout, mouse and focus handling) linked against the native driver.
# Opens no window and needs no game data.
ENGINE_INCLUDE="$HERE/../../engine/include"
xcrun clang -O3 -dynamiclib -arch arm64 -mmacosx-version-min=13.0 -Wno-deprecated-declarations -I "$ENGINE_INCLUDE" \
    "$SRC/macos_projection_codec.c" "$SRC/macos_live_driver.c" "$SRC/macos_audio_mapper.c" "$SRC/events_host_file.c" \
    -install_name @rpath/librising_macos_live.dylib -o "$TMP/librising_macos_live.dylib"
consumer=""
for s in NativeCampaignMenu NativeAudio NativeCDMusicDisc NativeCDMusicPlayer NativeCDMusicHost NativeHotkeyBindings \
         NativeHotkeyInputRouter NativeModernHotkeys NativeHotkeyPreferences NativeHotkeyLabels NativeViewportPolicy NativeGameSpeed \
         NativePlayerOptions NativeDefeatMovie NativePlayerEdition NativeFLC NativeEventFLCPlayback NativeLiveWindow; do
    consumer="$consumer $s.swift"
done
cd "$SRC"
# shellcheck disable=SC2086
xcrun swiftc -swift-version 5 -target arm64-apple-macosx13.0 -module-cache-path "$TMP/modules" -I "$ENGINE_INCLUDE" \
    -import-objc-header "$SRC/MacOSLiveDriver.h" $consumer "$HERE/ControlsConsumerTests.swift" \
    -framework AppKit -framework Metal -framework QuartzCore -framework AVFoundation -framework Carbon \
    "$TMP/librising_macos_live.dylib" -Xlinker -rpath -Xlinker "$TMP" -o "$TMP/ControlsConsumerTests"
"$TMP/ControlsConsumerTests"
echo "All macOS component tests passed."
