#!/bin/sh
# Runs a built RisingLands.exe without game data (both CI jobs: mingw x64 and
# MSVC x64). Each case checks the exit code and the
# message, and that a refused import leaves no Dados folder behind.
#   windows/ci/smoke.sh path/to/RisingLands.exe [--no-ui]
# --no-ui skips the case that opens the import dialogs (needs a desktop session).
set -eu
EXE=$1
UI=1
[ "${2:-}" = --no-ui ] && UI=0
WORK=${RUNNER_TEMP:-${TMPDIR:-/tmp}}/rl-smoke-$$
rm -rf "$WORK"; mkdir -p "$WORK"
FAILED=0

# expect NAME CODE PATTERN -- command...: exit code and a line of its output.
expect() {
  name=$1 code=$2 pattern=$3; shift 4
  echo "== $name"
  set +e
  "$@" > "$WORK/out.txt" 2>&1
  rc=$?
  set -e
  sed 's/^/   /' "$WORK/out.txt"
  if [ "$rc" != "$code" ]; then echo "FALHA: $name: código $rc, esperado $code"; FAILED=1; return 0; fi
  if [ -n "$pattern" ] && ! grep -q -- "$pattern" "$WORK/out.txt"; then echo "FALHA: $name: saída sem \"$pattern\""; FAILED=1; return 0; fi
  echo "   ok (código $rc)"
}
no_dados() {
  if [ -e "$1/Dados" ]; then echo "FALHA: $1/Dados existe depois de uma recusa"; FAILED=1; fi
  if ls "$1" 2>/dev/null | grep -q "Dados.importando"; then echo "FALHA: pasta temporária deixada em $1"; FAILED=1; fi
}

expect "self-test (ABI do motor, SHA-256)" 0 "selftest ok" -- "$EXE" --selftest
expect "argumento desconhecido -> uso" 64 "" -- "$EXE" --nao-existe
expect "dificuldade fora de 0..2 -> uso" 64 "" -- "$EXE" --difficulty 3 --headless
expect "velocidade fora de 0..3 -> uso" 64 "" -- "$EXE" --speed 4 --headless
expect "opção sem valor -> uso" 64 "" -- "$EXE" --cd
expect "smoke do primeiro início sem --data -> recusado" 64 "use só --data" -- "$EXE" --smoke-import-prompt

# First start without a CD in a smoke run: refused at once, nothing imported.
expect "sem CD (headless)" 2 "CD não importado" -- "$EXE" --headless --frames 1 --data "$WORK/vazio"
no_dados "$WORK/vazio"

# A file that is not the CD.
printf 'not a cd' > "$WORK/nada.bin"
expect "arquivo que não é o CD" 2 "não parece ser o CD" -- "$EXE" --headless --frames 1 --data "$WORK/d1" --cd "$WORK/nada.bin"
no_dados "$WORK/d1"

# A .CUE whose image is missing, and one that points outside its folder.
printf 'FILE "SUMIU.BIN" BINARY\r\n  TRACK 01 MODE1/2352\r\n    INDEX 01 00:00:00\r\n' > "$WORK/sumiu.cue"
expect ".cue sem a imagem" 2 "importação:" -- "$EXE" --import-only --data "$WORK/d2" --cd "$WORK/sumiu.cue"
no_dados "$WORK/d2"
printf 'FILE "../fora.bin" BINARY\r\n  TRACK 01 MODE1/2352\r\n' > "$WORK/fora.cue"
expect ".cue com caminho para fora" 2 "não é uma folha .CUE válida" -- "$EXE" --import-only --data "$WORK/d3" --cd "$WORK/fora.cue"
no_dados "$WORK/d3"

# A folder without the CD's INSTALHD.
mkdir -p "$WORK/pasta-vazia"
expect "pasta sem INSTALHD" 2 "não contém os arquivos do CD" -- "$EXE" --import-only --data "$WORK/d4" --cd "$WORK/pasta-vazia"
no_dados "$WORK/d4"

# A fake minimal CD layout (right names, wrong bytes): refused by the SHA-256
# pin of the executable, with the file named.
mkdir -p "$WORK/cd-falso/INSTALHD" "$WORK/cd-falso/WORLD" "$WORK/cd-falso/EVENTS"
for f in RISING.EXE RISING.INI RISING.RUL RISING.SBL TABLEAU.COL COULEUR.MAP PLATGAME.PIE MONSTRE.INI TEXTES.TXT EVENTS.INI; do
  printf 'falso %s' "$f" > "$WORK/cd-falso/INSTALHD/$f"
done
printf 'falso' > "$WORK/cd-falso/WORLD/MONDE01.ETA"
expect "CD falso com os nomes certos" 2 "RISING.EXE do CD escolhido é diferente" -- "$EXE" --import-only --data "$WORK/d5" --cd "$WORK/cd-falso"
no_dados "$WORK/d5"

# Order queue markers through the real GDI path (memory DC, synthetic picture).
expect "marcadores da fila de ordens (GDI)" 0 "smoke markers ok" -- "$EXE" --smoke-markers

# DPI mixed mode and the fullscreen fit on real windows (rlw_dpi.h).
expect "janela: DPI por monitor e tela cheia" 0 "smoke window ok" -- "$EXE" --smoke-window

# The player's first start: the import notice and the CD dialog open; both are
# cancelled by a watcher thread; the program ends cleanly without Dados.
if [ "$UI" = 1 ]; then
  expect "primeira execução: aviso e diálogo do CD" 0 "smoke import prompt ok" -- "$EXE" --smoke-import-prompt --data "$WORK/d6"
  no_dados "$WORK/d6"
fi

rm -rf "$WORK"
if [ "$FAILED" != 0 ]; then echo "smoke: FALHOU"; exit 1; fi
echo "smoke: todos os casos ok"
