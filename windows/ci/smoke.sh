#!/bin/sh
# Runs a built RisingLands.exe without game data: the engine ABI self-test, then
# the importer must refuse a file that is not the CD (exit code 2).
#   windows/ci/smoke.sh path/to/RisingLands.exe
set -eu
EXE=$1
WORK=${RUNNER_TEMP:-${TMPDIR:-/tmp}}/rl-smoke-$$
mkdir -p "$WORK"
echo "== $EXE --selftest"
"$EXE" --selftest
echo "== $EXE refuses a non-CD source"
printf 'not a cd' > "$WORK/nada.bin"
set +e
"$EXE" --headless --frames 1 --data "$WORK/data" --cd "$WORK/nada.bin"
rc=$?
set -e
echo "exit $rc"
rm -rf "$WORK"
test "$rc" = 2
