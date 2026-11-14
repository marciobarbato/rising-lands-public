#!/bin/sh
# Builds the Windows host: the Rust engine as a static library for a Windows
# target, then the Win32 host linked against it into one RisingLands.exe.
# Works as a cross build (macOS/Linux with mingw-w64) and under MSYS2 on Windows.
#
#   windows/build.sh            # release build, x86_64-pc-windows-gnu
#   windows/build.sh test       # also builds and runs the portable unit tests
#
# Environment: TARGET (Rust target, default x86_64-pc-windows-gnu), CC (default
# x86_64-w64-mingw32-gcc), OUT (default windows/build), HOST_CC for tests.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/.." && pwd)
TARGET=${TARGET:-x86_64-pc-windows-gnu}
CC=${CC:-x86_64-w64-mingw32-gcc}
OUT=${OUT:-$HERE/build}
mkdir -p "$OUT"

if [ "${1:-}" = test ]; then
  HOST_CC=${HOST_CC:-cc}
  TEST_LIBS=""
  case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) TEST_LIBS="-lshell32 -lole32 -luuid" ;; esac
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -DRLW_TESTING -I"$HERE/src" \
    "$HERE/tests/test_cd.c" "$HERE/src/rlw_cd.c" "$HERE/src/rlw_fs.c" "$HERE/src/rlw_sha256.c" \
    $TEST_LIBS -o "$OUT/test_cd"
  "$OUT/test_cd" "$OUT/test-work"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" -I"$REPO/engine/include" \
    "$HERE/tests/test_menu.c" "$HERE/src/rlw_menu.c" "$HERE/src/rlw_flc.c" "$HERE/src/rlw_fs.c" \
    $TEST_LIBS -o "$OUT/test_menu"
  "$OUT/test_menu"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" \
    "$HERE/tests/test_saves.c" "$HERE/src/rlw_saves.c" "$HERE/src/rlw_fs.c" \
    $TEST_LIBS -o "$OUT/test_saves"
  "$OUT/test_saves" "$OUT"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" \
    "$HERE/tests/test_savemgr.c" "$HERE/src/rlw_saves.c" "$HERE/src/rlw_savemgr.c" "$HERE/src/rlw_autosave.c" "$HERE/src/rlw_fs.c" \
    $TEST_LIBS -o "$OUT/test_savemgr"
  "$OUT/test_savemgr" "$OUT"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" \
    "$HERE/tests/test_gamemenu.c" "$HERE/src/rlw_gamemenu.c" "$HERE/src/rlw_saves.c" "$HERE/src/rlw_savemgr.c" "$HERE/src/rlw_autosave.c" "$HERE/src/rlw_fs.c" \
    $TEST_LIBS -o "$OUT/test_gamemenu"
  "$OUT/test_gamemenu" "$OUT"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" \
    "$HERE/tests/test_icon.c" "$HERE/src/rlw_icon.c" "$HERE/src/rlw_fs.c" \
    $TEST_LIBS -o "$OUT/test_icon"
  "$OUT/test_icon" "$OUT"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" \
    "$HERE/tests/test_reimport.c" "$HERE/src/rlw_reimport.c" "$HERE/src/rlw_fs.c" \
    $TEST_LIBS -o "$OUT/test_reimport"
  "$OUT/test_reimport" "$OUT"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" \
    "$HERE/tests/test_hotkeys.c" "$HERE/src/rlw_hotkeys.c" "$HERE/src/rlw_modern.c" \
    $TEST_LIBS -o "$OUT/test_hotkeys"
  "$OUT/test_hotkeys"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -I"$HERE/src" \
    "$HERE/tests/test_markers.c" "$HERE/src/rlw_markers.c" -o "$OUT/test_markers"
  "$OUT/test_markers"
  MAC="$REPO/macos/src"
  "$HOST_CC" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-format-truncation -Wno-misleading-indentation -DRLW_TESTING -I"$HERE/src" -I"$MAC" \
    "$HERE/tests/test_audio.c" "$HERE/src/rlw_audio.c" "$HERE/src/rlw_sound.c" "$HERE/src/rlw_sound_import.c" "$HERE/src/rlw_cdmusic.c" "$HERE/src/rlw_eventflc.c" "$HERE/src/rlw_flc.c" "$HERE/src/rlw_movie.c" \
    "$HERE/src/rlw_cd.c" "$HERE/src/rlw_fs.c" "$HERE/src/rlw_sha256.c" "$MAC/macos_audio_mapper.c" \
    $TEST_LIBS -lm -o "$OUT/test_audio"
  "$OUT/test_audio" "$OUT/test-audio"
fi

echo "== engine ($TARGET)"
CARGO_TARGET_DIR="$OUT/engine-target" cargo rustc --release --locked \
  --manifest-path "$REPO/engine/Cargo.toml" -p rl-ffi --lib --crate-type staticlib --target "$TARGET"
LIB="$OUT/engine-target/$TARGET/release/librising_engine.a"
test -f "$LIB"

echo "== host ($CC)"
# Native libraries the Rust std needs on windows-gnu (cargo --print native-static-libs),
# plus the Win32 libraries of the host.
"$CC" -std=c11 -O2 -Wall -Wextra -Wno-format-truncation -Wno-misleading-indentation -municode -mwindows \
  -I"$REPO/engine/include" -I"$HERE/src" -I"$REPO/macos/src" \
  "$HERE/src/rlw_main.c" "$HERE/src/rlw_driver.c" "$HERE/src/rlw_cd.c" "$HERE/src/rlw_fs.c" "$HERE/src/rlw_sha256.c" \
  "$HERE/src/rlw_menu.c" "$HERE/src/rlw_menu_ops.c" "$HERE/src/rlw_menu_demo.c" "$HERE/src/rlw_flc.c" \
  "$HERE/src/rlw_saves.c" "$HERE/src/rlw_hotkeys.c" "$HERE/src/rlw_modern.c" "$HERE/src/rlw_keys_dialog.c" \
  "$HERE/src/rlw_markers.c" "$HERE/src/rlw_markers_gdi.c" "$HERE/src/rlw_smoke.c" "$HERE/src/rlw_dpi.c" \
  "$HERE/src/rlw_savemgr.c" "$HERE/src/rlw_savemgr_dialog.c" "$HERE/src/rlw_autosave.c" "$HERE/src/rlw_gamemenu.c" "$HERE/src/rlw_gamemenu_win.c" "$HERE/src/rlw_icon.c" "$HERE/src/rlw_icon_win.c" "$HERE/src/rlw_reimport.c" "$HERE/src/rlw_reimport_win.c" \
  "$HERE/src/rlw_audio.c" "$HERE/src/rlw_audio_win32.c" "$HERE/src/rlw_audio_ui.c" "$HERE/src/rlw_sound.c" "$HERE/src/rlw_sound_import.c" "$HERE/src/rlw_cdmusic.c" "$HERE/src/rlw_eventflc.c" "$HERE/src/rlw_movie.c" \
  "$REPO/macos/src/macos_audio_mapper.c" "$LIB" \
  -static -static-libgcc \
  -lgdi32 -lcomdlg32 -lcomctl32 -lshell32 -lole32 -luuid -lwinmm \
  -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp \
  -o "$OUT/RisingLands.exe"
echo "== $OUT/RisingLands.exe"
