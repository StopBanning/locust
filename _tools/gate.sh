#!/usr/bin/env bash
# The one gate before a Locust commit: the type configs, then every suite.
# Exits non-zero on the first failure, so read `$?` (or `gate.sh && commit`)
# and a red gate can never be committed.
#
#   bash _tools/gate.sh > gate.log 2>&1; echo "gate exit $?"; grep -E "Tests |GATE" gate.log
#
# Run from anywhere: the repository is found from this file's own place.
#
# Exit codes -- commit on 0 and on nothing else:
#    0   every check passed ("GATE PASSED")
#   75   refused to start: the machine was not quiet (under 2 GB of free
#        virtual memory, or another vitest running anywhere on it). Nothing
#        was run, so nothing is red; run it again when the other work is done.
#    1   something failed (a type check keeps tsc's own non-zero code)
#
# A red gate says why (2026-10-05). It used to keep `grep "×|FAIL"`: the name
# of each failed test and not the line under it, so a timeout and an assertion
# looked the same and the reason was gone. Now every failed test is a `GATE:`
# line with the reason vitest gave, vitest's own failure section follows, and
# the suites' full logs are kept in the folder the last line names.
#
# One retry, of one kind. If the desktop suite fails and EVERY failed test
# failed by timing out, only those files are run once more, and the gate prints
# that it did and which. An assertion failure, a file that would not load, an
# unhandled error, or anything the log cannot account for is never retried.
# The log is read by _tools/gate-support.mjs, which also makes the quiet-machine
# check (what it found is printed, as `GATE quiet:` lines, at the top).
# _tools/gate-selftest.sh runs this script against a pretend npx and checks each
# way it can end (green, retried, refused, red); run it after changing this file.
set -e
W="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUPPORT="$W/_tools/gate-support.mjs"
# Scratch files inside one folder (2026-09-30): the suites make thousands of
# temp folders, and an antivirus scanning each one in %TEMP% pinned the CPU
# and timed tests out. Clear old locust-* folders there before a long run.
SCRATCH="${LOCUST_GATE_TMP:-$(cd "$W/.." && pwd)/.tmp}"
mkdir -p "$SCRATCH"
export TMP="$SCRATCH" TEMP="$SCRATCH" TMPDIR="$SCRATCH"
echo "GATE: $(basename "$W") $(date '+%Y-%m-%d %H:%M:%S')"
# Is the machine quiet enough to trust a run? Two suites at once, or too little
# memory, is how a test that passes alone times out in the gate. It prints what
# it found and exits 75 (nothing run) when the answer is no.
node "$SUPPORT" quiet "$W" "$SCRATCH" || exit $?
LOGS="$(mktemp -d)"
# The recall model its tests run (no network once staged; hashes checked).
node "$W/_tools/vendor-recall.mjs" > /dev/null
cd "$W/packages/runtime-adapters" && npx tsc -p tsconfig.json
cd "$W/packages/mission-store" && npx tsc -p tsconfig.json
cd "$W/packages/cli" && npx tsc -p tsconfig.json
cd "$W/apps/desktop"
npx tsc --noEmit -p tsconfig.node.json
npx tsc --noEmit -p tsconfig.web.json

# One vitest run: its whole report goes to $LOGS/<name>.log; what follows the
# directory is passed on (the retry names files). The reporter is NAMED because
# the helper reads this one: left to itself vitest 4 picks a terse reporter when
# it sees an AI agent run it (CLAUDECODE, AI_AGENT), and the format a parser
# reads should not depend on who started the gate.
suite() { (cd "$2" && npx vitest run --reporter=default "${@:3}" > "$LOGS/$1.log" 2>&1); }
# Red: say why, say where the logs are, and stop. $3, when given, is said in
# the parentheses after FAILED.
red() {
  node "$SUPPORT" failures "$2" "$LOGS/$1.log" || true
  echo "GATE: $2 FAILED (${3:+$3; }full logs kept in $LOGS)"
  exit 1
}

# The packages, built first (0.668): the desktop imports their dist/, and a
# change to a package's source was tested by the desktop against the old build.
(cd "$W" && pnpm -r --filter "./packages/**" run build > "$LOGS/pkg.log" 2>&1) || { tail -20 "$LOGS/pkg.log"; echo "GATE: package build FAILED (full logs kept in $LOGS)"; exit 1; }
suite ra "$W/packages/runtime-adapters" || red ra adapters
suite ms "$W/packages/mission-store" || red ms mission-store
suite cli "$W/packages/cli" || red cli cli
RETRIED=
if ! suite d "$W/apps/desktop"; then
  # A retry that names no file is red: run with no file, vitest runs the whole suite, and a second green would hide
  # the first run's assertion (2026-10-31: the helper had exited 0 unasked, from a folder reached through a link).
  if again="$(node "$SUPPORT" retry-files "$LOGS/d.log")" && [ -n "$again" ]; then
    files=()
    while IFS= read -r file; do files+=("$file"); done <<< "$again"
    # Green or not, what timed out is kept in the log: a flake that passed on
    # the second go is still a flake worth reading about.
    node "$SUPPORT" failures desktop "$LOGS/d.log" digest || true
    echo "GATE: desktop failed ONLY by timing out; running ${#files[@]} file(s) once more (an assertion failure is never retried):"
    printf 'GATE:   %s\n' "${files[@]}"
    suite d2 "$W/apps/desktop" "${files[@]}" || red d2 desktop "the retry of the timed-out files failed too"
    RETRIED=1
  else
    red d desktop
  fi
fi

# The app's own bundles, built (0.668): a renderer import that pulled a Node
# module into the window (0.666's provider-error) passed every test and both
# typechecks, and only electron-vite at the ship refused it.
(cd "$W/apps/desktop" && npx electron-vite build > "$LOGS/build.log" 2>&1) || { grep -E "error|rror during|is not exported" "$LOGS/build.log" | head -10; echo "GATE: app build FAILED (full logs kept in $LOGS)"; exit 1; }
echo "GATE: app build ok"

if [ -z "$RETRIED" ]; then
  grep -h "Tests " "$LOGS/ra.log" "$LOGS/ms.log" "$LOGS/cli.log" "$LOGS/d.log"
else
  grep -h "Tests " "$LOGS/ra.log" "$LOGS/ms.log" "$LOGS/cli.log"
  echo "GATE: desktop first run -- $(grep -h '^ *Tests ' "$LOGS/d.log" | sed 's/^ *//')"
  echo "GATE: desktop retry run -- $(grep -h '^ *Tests ' "$LOGS/d2.log" | sed 's/^ *//')  (${#files[@]} timed-out file(s), run again)"
  echo "GATE: note -- the desktop suite needed one timeout-only retry; the files are named above"
fi
rm -rf "$LOGS"
echo "GATE PASSED"
