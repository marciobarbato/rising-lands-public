#!/bin/sh
# Sound-event parity on macOS (no Windows machine needed): the engine's ordered
# sound requests (kind-10 PostSound/StopSound/StopAllSound copies plus the
# PlaySnd_Manage ticks) as the Windows driver (rlw_driver.c) and the macOS
# driver (macos/src, compiled read-only) publish them, over the same
# native engine: MONDE01 for N host ticks with the fixed synthetic input, then
# the scripted menu walk (rlw_menu_demo.c). Both streams must be identical and
# the play run must hold sound requests. With the original sound banks
# (<Dados>/sound or $RL_CD_ROOT/INSTALHD/SOUND) every sample decoded by
# rlw_sound.c must match the Audio catalogue's decoded SHA-256 (hashes only; the
# catalogue lives in the private research repository, so without it this step is
# skipped), and the play
# run is rendered through RlwAudio to a WAV whose statistics are checked
# (non-silent, no clipping, 1/30 s per tick). Prints SONS OK.
#
#   windows/audio_parity.sh --cd /caminho/RISING_LANDS_BR_.cue [ticks]
#   windows/audio_parity.sh --data /pasta-com-Dados [ticks]
#   windows/audio_parity.sh --build-only
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/.." && pwd)
OUT=${OUT:-$HERE/build/audio-parity}
MAC="$REPO/macos/src"
CD="" DATA="" TICKS=600 BUILD_ONLY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --cd) CD=$2; shift 2 ;;
    --data) DATA=$2; shift 2 ;;
    --build-only) BUILD_ONLY=1; shift ;;
    *) TICKS=$1; shift ;;
  esac
done
mkdir -p "$OUT"
# The engine build is shared with parity.sh (same crate, flags and target).
ENGINE_TARGET=${ENGINE_TARGET:-$HERE/build/parity/engine-target}
echo "== engine (host)"
CARGO_TARGET_DIR="$ENGINE_TARGET" cargo rustc --release --locked \
  --manifest-path "$REPO/engine/Cargo.toml" -p rl-ffi --lib --crate-type staticlib
LIB="$ENGINE_TARGET/release/librising_engine.a"
CC=${HOST_CC:-cc}
cc_host() { "$CC" -std=c11 -O2 -Wall -Wextra -I"$REPO/engine/include" -I"$HERE/src" "$@"; }
echo "== audio-parity-windows / audio-parity-macos / rlw-import"
S="$HERE/src"
cc_host -I"$MAC" "$HERE/tests/audio_parity.c" "$S/rlw_audio.c" "$S/rlw_sound.c" "$S/rlw_cdmusic.c" "$S/rlw_movie.c" "$MAC/macos_audio_mapper.c" "$S/rlw_driver.c" "$S/rlw_menu_ops.c" "$S/rlw_menu.c" "$S/rlw_menu_demo.c" "$S/rlw_autosave.c" "$S/rlw_saves.c" "$S/rlw_flc.c" "$S/rlw_fs.c" "$S/rlw_sha256.c" \
  "$LIB" -lm -o "$OUT/audio-parity-windows"
cc_host -Wno-deprecated-declarations -Wno-unused-parameter -Wno-sign-compare -Wno-missing-field-initializers \
  -DPARITY_MACOS -DMACOS_STATIC_ENGINE -DMACOS_EVENT_FLC -I"$MAC" \
  "$HERE/tests/audio_parity.c" "$MAC/macos_live_driver.c" "$MAC/macos_projection_codec.c" "$MAC/macos_audio_mapper.c" \
  "$MAC/events_host_file.c" "$S/rlw_menu.c" "$S/rlw_menu_demo.c" "$S/rlw_autosave.c" "$S/rlw_saves.c" "$S/rlw_flc.c" "$S/rlw_fs.c" "$S/rlw_sha256.c" "$LIB" -o "$OUT/audio-parity-macos"
"$CC" -std=c11 -O2 -I"$HERE/src" "$HERE/tests/import_cli.c" "$S/rlw_cd.c" "$S/rlw_fs.c" "$S/rlw_sha256.c" -o "$OUT/rlw-import"
[ "$BUILD_ONLY" = 1 ] && exit 0
if [ -n "$CD" ]; then
  DATA="$OUT/data"; "$OUT/rlw-import" "$CD" "$DATA"
  # The two CD-audio tracks as pinned WAVs (170 MB; removed after the check).
  "$OUT/audio-parity-windows" music "$CD" "$DATA" && rm -rf "$DATA/Dados/music"
fi
[ -n "$DATA" ] || { echo "use --cd CAMINHO ou --data PASTA" >&2; exit 64; }
# The macOS driver reads the FLC headers from <native-inputs>/../events.
[ -e "$DATA/events" ] || ln -s Dados/events "$DATA/events"
SOUND=""
for candidate in "$DATA/Dados/sound" "${RL_CD_ROOT:-}/INSTALHD/SOUND"; do
  [ -f "$candidate/P.SND" ] && { SOUND=$candidate; break; }
done
WAVLINE=""
if [ -n "$SOUND" ]; then
  CATALOGUE="$REPO/pesquisas-paralelas/audio/catalogo-amostras.json"
  "$OUT/audio-parity-windows" samples "$SOUND" "$OUT/samples.txt"
  if [ ! -f "$CATALOGUE" ]; then echo "(sem o catálogo do Audio: amostras decodificadas não comparadas)"; else
  python3 -I - "$CATALOGUE" "$OUT/samples.txt" "$SOUND" <<'PY' || exit 1
import hashlib, json, os, sys
catalogue = json.load(open(sys.argv[1]))['sounds']
got = {}
for line in open(sys.argv[2]):
    bank, entry, variant, frames, sha = line.split()
    got[(bank, int(entry), int(variant))] = (int(frames), sha)
def expected(s):
    # Plain PCM with an odd byte count: the last half sample is not played (NativeCDImport
    # pcm-wav keeps whole frames too). The catalogue hashes the whole SND window; check
    # that window, then expect the hash of its whole-frame prefix.
    if s['format'] == 0 and s['decoded_bytes'] % 2:
        with open(os.path.join(sys.argv[3], s['bank'] + '.SND'), 'rb') as f:
            f.seek(s['snd_offset']); window = f.read(s['decoded_bytes'])
        if hashlib.sha256(window).hexdigest() != s['decoded_sha256']:
            return None
        return (len(window) // 2, hashlib.sha256(window[:-1]).hexdigest())
    return (s['decoded_bytes'] // 2, s['decoded_sha256'])
bad = [s for s in catalogue if got.get((s['bank'], s['index'], s['variant'])) != expected(s)]
if bad or len(got) != len(catalogue):
    print(f"AMOSTRAS FALHARAM: {len(bad)} diferentes, {len(got)} decodificadas, {len(catalogue)} no catálogo", bad[:3]); sys.exit(1)
print(f"amostras: {len(got)} decodificadas = catálogo do Audio (SHA-256 dos bytes decodificados)")
PY
  fi
  "$OUT/audio-parity-windows" "$DATA/Dados" "$TICKS" "$OUT/windows-play.txt" --wav "$SOUND" "$OUT/windows-play.wav" | tee "$OUT/wav.txt"
  WAVLINE=$(grep '^wav:' "$OUT/wav.txt")
  echo "$WAVLINE" | awk -v ticks="$TICKS" '{ if ($2 + 0 != (ticks + 1) * 1470 || $7 + 0 < 1000 || $13 + 0 != 0 || $15 + 0 < 1 || $17 + 0 != 0) { print "WAV FALHOU: " $0; exit 1 } }' || exit 1
else
  echo "(sem bancos de som: amostras e WAV não verificados)"
  "$OUT/audio-parity-windows" "$DATA/Dados" "$TICKS" "$OUT/windows-play.txt"
fi
"$OUT/audio-parity-macos" "$DATA/Dados" "$TICKS" "$OUT/macos-play.txt"
posts=$(grep -c '^[0-9]* 1 ' "$OUT/windows-play.txt" || true)
if ! cmp -s "$OUT/windows-play.txt" "$OUT/macos-play.txt"; then
  echo "SONS FALHARAM (jogo)"; diff "$OUT/windows-play.txt" "$OUT/macos-play.txt" | head -20; exit 1
fi
[ "$posts" -gt 0 ] || { echo "SONS FALHARAM: nenhum PostSound em $TICKS ticks"; exit 1; }
"$OUT/audio-parity-windows" "$DATA/Dados" menu "$OUT/windows-menu.txt"
"$OUT/audio-parity-macos" "$DATA/Dados" menu "$OUT/macos-menu.txt"
if ! cmp -s "$OUT/windows-menu.txt" "$OUT/macos-menu.txt"; then
  echo "SONS FALHARAM (menus)"; diff "$OUT/windows-menu.txt" "$OUT/macos-menu.txt" | head -20; exit 1
fi
# Movies: rlw_movie.c against the macOS app's own decoder (NativeDefeatMovie.swift, read-only),
# every frame and the whole audio of each original AVI (needs swiftc and the CD's EVENTS).
MOVIELINE=""
AVIS=""
for candidate in "$DATA/Dados/movies" "${RL_CD_ROOT:-}/EVENTS"; do
  ls "$candidate"/*.AVI >/dev/null 2>&1 && { AVIS=$candidate; break; }
done
if [ -n "$AVIS" ] && command -v swiftc >/dev/null 2>&1; then
  # Top-level code must live in a file named main.swift.
  mkdir -p "$OUT/movie-swift" && cp "$HERE/tests/movie_parity.swift" "$OUT/movie-swift/main.swift"
  swiftc -O -o "$OUT/movie-parity-macos" "$OUT/movie-swift/main.swift" "$MAC/NativeDefeatMovie.swift"
  "$OUT/audio-parity-windows" movies "$AVIS"/*.AVI > "$OUT/windows-movies.txt"
  "$OUT/movie-parity-macos" "$AVIS"/*.AVI > "$OUT/macos-movies.txt"
  if ! cmp -s "$OUT/windows-movies.txt" "$OUT/macos-movies.txt"; then
    echo "FILMES FALHARAM"; diff "$OUT/windows-movies.txt" "$OUT/macos-movies.txt"; exit 1
  fi
  MOVIELINE="filmes: $(wc -l < "$OUT/windows-movies.txt" | tr -d ' ') AVI idênticos ao decodificador do macOS ($(awk '{f+=$2} END {print f}' "$OUT/windows-movies.txt") quadros)"
  echo "$MOVIELINE"
else
  echo "(sem AVI ou sem swiftc: filmes não comparados)"
fi
echo "SONS OK: jogo $(grep -c '' "$OUT/windows-play.txt") linhas idênticas ($posts PostSound; $(tail -1 "$OUT/windows-play.txt")), menus $(grep -c '' "$OUT/windows-menu.txt") linhas idênticas ($(tail -1 "$OUT/windows-menu.txt"))${WAVLINE:+; $WAVLINE}${MOVIELINE:+; $MOVIELINE}"
