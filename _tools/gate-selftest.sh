#!/usr/bin/env bash
# Does the gate do what it says when a suite goes red? (2026-10-05)
#
#   bash _tools/gate-selftest.sh
#
# Runs _tools/gate.sh in a throwaway copy of its own layout, with a pretend
# `npx` whose type checks always pass and whose vitest answers from a script,
# and checks the exit code and the words for each way a run can end: green; a
# timeout-only failure that goes green on the second go; one that does not; an
# assertion (never retried); a mix (never retried); a red package suite; and a
# machine that is not quiet. Seconds, no real suite. The retry is the one path
# a real gate only reaches after a flake, so it is checked here rather than
# waiting for one.
#
# The last case needs the quiet check to see a vitest process, so it starts a
# decoy; the rest need none running, and say so if one is.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(mktemp -d)"
trap 'rm -rf "$ROOT"' EXIT
FAKE="$ROOT/repo"
mkdir -p "$FAKE/_tools" "$FAKE/packages/runtime-adapters" "$FAKE/packages/mission-store" "$FAKE/packages/cli" "$FAKE/apps/desktop" "$ROOT/bin" "$ROOT/state" "$ROOT/scratch"
cp "$HERE/gate.sh" "$HERE/gate-support.mjs" "$FAKE/_tools/"
echo "process.exit(0)" > "$FAKE/_tools/vendor-recall.mjs"

# --- what the pretend vitest prints -------------------------------------------
GREEN='
 Test Files  3 passed (3)
      Tests  12 passed (12)'
TIMEOUTS=' ✓ src/main/fine.test.ts (3 tests) 12ms

⎯⎯⎯⎯⎯⎯⎯ Failed Tests 3 ⎯⎯⎯⎯⎯⎯⎯

 FAIL  src/main/slow.test.ts > a loop > goes on and on
Error: Test timed out in 60000ms.
If this is a long-running test, pass a timeout value as the last argument or configure it globally with "testTimeout".
 ❯ src/main/slow.test.ts:10:3

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[1/3]⎯

 FAIL  src/main/slow.test.ts > a loop > goes on and on
Error: ENOTEMPTY: directory not empty, rmdir '"'"'x'"'"'
⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[2/3]⎯

 FAIL  src/main/other.test.ts > a short one > waits on the disk
Error: Test timed out in 5000ms.
If this is a long-running test, pass a timeout value as the last argument or configure it globally with "testTimeout".
 ❯ src/main/other.test.ts:4:3

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[3/3]⎯


 Test Files  2 failed | 1 passed (3)
      Tests  2 failed | 10 passed (12)'
ASSERTION='
⎯⎯⎯⎯⎯⎯⎯ Failed Tests 1 ⎯⎯⎯⎯⎯⎯⎯

 FAIL  src/main/wrong.test.ts > a sum > adds
AssertionError: expected 3 to be 4 // Object.is equality
 ❯ src/main/wrong.test.ts:7:19

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[1/1]⎯


 Test Files  1 failed | 2 passed (3)
      Tests  1 failed | 11 passed (12)'
# A mix: one timeout and one assertion under a single summary.
MIXED='
⎯⎯⎯⎯⎯⎯⎯ Failed Tests 2 ⎯⎯⎯⎯⎯⎯⎯

 FAIL  src/main/slow.test.ts > a loop > goes on and on
Error: Test timed out in 60000ms.
If this is a long-running test, pass a timeout value as the last argument or configure it globally with "testTimeout".

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[1/2]⎯

 FAIL  src/main/wrong.test.ts > a sum > adds
AssertionError: expected 3 to be 4 // Object.is equality

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[2/2]⎯


 Test Files  2 failed | 1 passed (3)
      Tests  2 failed | 10 passed (12)'

# The pretend npx: type checks pass; vitest answers by suite and by run number.
# FAKE_SCENARIO names what each suite does: e.g. "d1=timeouts d2=green" is the
# desktop suite failing on timeouts, then passing when run again.
cat > "$ROOT/bin/npx" <<'PRETEND'
#!/usr/bin/env bash
if [ "$1" = "tsc" ] || [ "$1" = "electron-vite" ]; then exit 0; fi
shift # vitest
shift # run
suite="$(basename "$PWD")"
case "$suite" in desktop) key=d ;; mission-store) key=m ;; cli) key=c ;; *) key=a ;; esac
n=$(( $(cat "$FAKE_STATE/$key" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$FAKE_STATE/$key"
echo "$* " >> "$FAKE_STATE/$key.args"
what="green"
for pair in $FAKE_SCENARIO; do [ "${pair%%=*}" = "$key$n" ] && what="${pair#*=}"; done
case "$what" in
  green) printf '%s\n' "$FAKE_GREEN"; exit 0 ;;
  timeouts) printf '%s\n' "$FAKE_TIMEOUTS"; exit 1 ;;
  assertion) printf '%s\n' "$FAKE_ASSERTION"; exit 1 ;;
  mixed) printf '%s\n' "$FAKE_MIXED"; exit 1 ;;
esac
PRETEND
chmod +x "$ROOT/bin/npx"
export FAKE_GREEN="$GREEN" FAKE_TIMEOUTS="$TIMEOUTS" FAKE_ASSERTION="$ASSERTION" FAKE_MIXED="$MIXED" FAKE_STATE="$ROOT/state"

# Every case below goes through the real quiet check, so the machine has to be
# quiet for them to mean anything: ask first, and say so if it is not.
if ! node "$FAKE/_tools/gate-support.mjs" quiet "$FAKE" "$ROOT/scratch" > "$ROOT/quiet.out" 2>&1; then
  echo "SELFTEST NOT RUN: the machine is not quiet (that is the gate's own answer too). Try again when the other work is done:"
  sed 's/^/        | /' "$ROOT/quiet.out"
  exit 75
fi

failures=0
# What the pretend vitest was asked, one line per run: for the retry, exactly the failed files.
asked() { sed -n "${2}p" "$ROOT/state/$1.args" 2>/dev/null; }
# check <name> <what happened> <what should have>
check() {
  if [ "$2" = "$3" ]; then echo "PASS  $1"; else echo "FAIL  $1 -- got \"$2\" (wanted \"$3\")"; failures=$((failures + 1)); fi
}
# run <name> <scenario> <expected exit> <desktop runs expected> [text the output must contain ...]
run() {
  local name="$1" scenario="$2" want="$3" runs="$4"
  shift 4
  rm -f "$ROOT/state"/*
  local out code
  out="$(PATH="$ROOT/bin:$PATH" FAKE_SCENARIO="$scenario" LOCUST_GATE_TMP="$ROOT/scratch" bash "$FAKE/_tools/gate.sh" 2>&1)"
  code=$?
  printf '%s\n' "$out" > "$ROOT/last.out"
  local bad=""
  [ "$code" = "$want" ] || bad="$bad exit $code (wanted $want);"
  local got; got="$(cat "$ROOT/state/d" 2>/dev/null || echo 0)"
  [ "$got" = "$runs" ] || bad="$bad desktop ran $got time(s) (wanted $runs);"
  for text in "$@"; do
    printf '%s' "$out" | grep -qF -- "$text" || bad="$bad missing \"$text\";"
  done
  if [ -z "$bad" ]; then echo "PASS  $name"; else echo "FAIL  $name --$bad"; printf '%s\n' "$out" | sed 's/^/        | /' | head -40; failures=$((failures + 1)); fi
}

run "green: desktop runs once, no retry" "" 0 1 "GATE PASSED" "Tests  12 passed (12)"
check "green: the four suites' own Tests lines, as before" "$(grep -c 'Tests  12 passed (12)' "$ROOT/last.out")" "4"
check "green: vitest is asked for its default reporter, and for no particular file" "$(asked d 1)" "--reporter=default "
run "timeouts only, then green: retried once, says so, exits 0" "d1=timeouts d2=green" 0 2 \
  "GATE: desktop failed ONLY by timing out; running 2 file(s) once more" "GATE:   src/main/slow.test.ts" "GATE:   src/main/other.test.ts" \
  "[timeout] src/main/slow.test.ts" "GATE: desktop first run -- Tests  2 failed | 10 passed (12)" "GATE: desktop retry run" "GATE PASSED"
check "the retry is asked for exactly the failed files (the first run asked for none)" "$(asked d 2)" "--reporter=default src/main/slow.test.ts src/main/other.test.ts "
run "timeouts, and the retry times out too: red, exit 1" "d1=timeouts d2=timeouts" 1 2 "GATE: desktop FAILED (the retry of the timed-out files failed too"
run "an assertion: never retried, red, exit 1, the reason is in the log" "d1=assertion" 1 1 \
  "GATE: not retried:" "[assertion] src/main/wrong.test.ts > a sum > adds -- AssertionError: expected 3 to be 4" "GATE: desktop FAILED"
run "a timeout beside an assertion: never retried" "d1=mixed" 1 1 "GATE: not retried:" "[timeout] src/main/slow.test.ts" "[assertion] src/main/wrong.test.ts" "GATE: desktop FAILED"
run "a red package suite: stops there, desktop never runs" "a1=assertion" 1 0 "GATE: adapters FAILED" "[assertion] src/main/wrong.test.ts"

# A helper that answers "retry" and names no file: the whole suite is not run again, and the gate is red.
cp "$FAKE/_tools/gate-support.mjs" "$ROOT/gate-support.real"
echo "process.exit(0)" > "$FAKE/_tools/gate-support.mjs"
run "a retry that names no file: red, the suite not run again" "d1=assertion" 1 1 "GATE: desktop FAILED"
cp "$ROOT/gate-support.real" "$FAKE/_tools/gate-support.mjs"

# A machine that is not quiet: a decoy process with "vitest" on its command line.
node -e 'setTimeout(function () {}, 60000)' vitest-decoy &
decoy=$!
sleep 1
run "another vitest running: refused, exit 75, nothing run" "" 75 0 "GATE REFUSED: another vitest is running" "Nothing was run"
kill "$decoy" 2>/dev/null

if [ "$failures" = 0 ]; then echo "SELFTEST PASSED"; else echo "SELFTEST FAILED ($failures)"; exit 1; fi
