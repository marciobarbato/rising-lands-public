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
  -I"$REPO/engine/include" -I"$HERE/src" \
  "$HERE/src/rlw_main.c" "$HERE/src/rlw_driver.c" "$HERE/src/rlw_cd.c" "$HERE/src/rlw_fs.c" "$HERE/src/rlw_sha256.c" \
  "$HERE/src/rlw_menu.c" "$HERE/src/rlw_menu_ops.c" "$HERE/src/rlw_menu_demo.c" "$HERE/src/rlw_flc.c" \
  "$LIB" \
  -static -static-libgcc \
  -lgdi32 -lcomdlg32 -lshell32 -lole32 -luuid -lwinmm \
  -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp \
  -o "$OUT/RisingLands.exe"
echo "== $OUT/RisingLands.exe"
