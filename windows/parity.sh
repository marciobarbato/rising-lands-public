#!/bin/sh
# Driver parity check on macOS (no Windows machine needed): runs MONDE01 for N
# host ticks with the fixed synthetic input through the Windows driver
# (rlw_driver.c) and through the macOS driver (macos/src, compiled
# read-only) over the same native engine, and compares the per-tick SHA-256 of
# the presented pixels and palette. Then the scripted menu walk
# (src/rlw_menu_demo.c: main menu, Novo jogo, briefing, play, pause, restart)
# through both drivers, compared step by step. Then the save walk (stage
# autosave, save, load and replay, the pause menu's save and load lists), whose
# lines include the SHA-256 of every slot and .view file each driver wrote.
#
#   windows/parity.sh --cd /caminho/RISING_LANDS_BR_.cue [ticks]
#   windows/parity.sh --data /pasta-com-Dados [ticks]
#   windows/parity.sh --build-only
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/.." && pwd)
OUT=${OUT:-$HERE/build/parity}
MAC="$REPO/macos/src"
CD="" DATA="" TICKS=300 BUILD_ONLY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --cd) CD=$2; shift 2 ;;
    --data) DATA=$2; shift 2 ;;
    --build-only) BUILD_ONLY=1; shift ;;
    *) TICKS=$1; shift ;;
  esac
done
mkdir -p "$OUT"
echo "== engine (host)"
CARGO_TARGET_DIR="$OUT/engine-target" cargo rustc --release --locked \
  --manifest-path "$REPO/engine/Cargo.toml" -p rl-ffi --lib --crate-type staticlib
LIB="$OUT/engine-target/release/librising_engine.a"
CC=${HOST_CC:-cc}
cc_host() { "$CC" -std=c11 -O2 -I"$REPO/engine/include" -I"$HERE/src" "$@"; }
echo "== parity-windows / parity-macos / rlw-import"
S="$HERE/src"
cc_host "$HERE/tests/parity.c" "$S/rlw_driver.c" "$S/rlw_menu_ops.c" "$S/rlw_menu.c" "$S/rlw_menu_demo.c" "$S/rlw_flc.c" "$S/rlw_fs.c" "$S/rlw_sha256.c" \
  "$LIB" -o "$OUT/parity-windows"
cc_host -Wno-deprecated-declarations -DPARITY_MACOS -DMACOS_STATIC_ENGINE -DMACOS_EVENT_FLC -I"$MAC" \
  "$HERE/tests/parity.c" "$MAC/macos_live_driver.c" "$MAC/macos_projection_codec.c" "$MAC/macos_audio_mapper.c" \
  "$MAC/events_host_file.c" "$S/rlw_menu.c" "$S/rlw_menu_demo.c" "$S/rlw_flc.c" "$S/rlw_fs.c" "$S/rlw_sha256.c" "$LIB" -o "$OUT/parity-macos"
cc_host "$HERE/tests/import_cli.c" "$HERE/src/rlw_cd.c" "$HERE/src/rlw_fs.c" "$HERE/src/rlw_sha256.c" -o "$OUT/rlw-import"
[ "$BUILD_ONLY" = 1 ] && exit 0
if [ -n "$CD" ]; then DATA="$OUT/data"; "$OUT/rlw-import" "$CD" "$DATA"; fi
[ -n "$DATA" ] || { echo "use --cd CAMINHO ou --data PASTA" >&2; exit 64; }
# The macOS driver reads the FLC headers from <native-inputs>/../events.
[ -e "$DATA/events" ] || ln -s Dados/events "$DATA/events"
"$OUT/parity-windows" "$DATA/Dados" "$TICKS" "$OUT/windows.txt" --demo-input
"$OUT/parity-macos" "$DATA/Dados" "$TICKS" "$OUT/macos.txt" --demo-input
if cmp -s "$OUT/windows.txt" "$OUT/macos.txt"; then
  echo "PARIDADE OK: $(wc -l < "$OUT/windows.txt" | tr -d ' ') ticks idênticos (último: $(tail -1 "$OUT/windows.txt"))"
else
  echo "PARIDADE FALHOU"; diff "$OUT/windows.txt" "$OUT/macos.txt" | head -20; exit 1
fi
"$OUT/parity-windows" "$DATA/Dados" menu "$OUT/windows-menu.txt"
"$OUT/parity-macos" "$DATA/Dados" menu "$OUT/macos-menu.txt"
if cmp -s "$OUT/windows-menu.txt" "$OUT/macos-menu.txt"; then
  echo "PARIDADE DOS MENUS OK: $(grep -c '' "$OUT/windows-menu.txt") linhas idênticas ($(tail -1 "$OUT/windows-menu.txt"))"
else
  echo "PARIDADE DOS MENUS FALHOU"; diff "$OUT/windows-menu.txt" "$OUT/macos-menu.txt" | head -20; exit 1
fi
rm -rf "$OUT/saves-windows" "$OUT/saves-macos"; mkdir -p "$OUT/saves-windows" "$OUT/saves-macos"
"$OUT/parity-windows" "$DATA/Dados" save "$OUT/windows-save.txt" "$OUT/saves-windows"
"$OUT/parity-macos" "$DATA/Dados" save "$OUT/macos-save.txt" "$OUT/saves-macos"
if cmp -s "$OUT/windows-save.txt" "$OUT/macos-save.txt"; then
  echo "PARIDADE DOS SAVES OK: $(grep -c '' "$OUT/windows-save.txt") linhas idênticas ($(tail -1 "$OUT/windows-save.txt"))"
else
  echo "PARIDADE DOS SAVES FALHOU"; diff "$OUT/windows-save.txt" "$OUT/macos-save.txt" | head -20; exit 1
fi
