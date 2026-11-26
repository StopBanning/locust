#!/usr/bin/env bash
# Every test of both crates (locust-core and locust-native) on Linux, except the ones parity/linux-not-yet.txt names.
#
#   bash scripts/linux-tests.sh       # from anywhere: it works in apps/native
#
# Fails if any test not on the list fails, or if a name on the list is no longer a test (a renamed or removed test
# leaves the list with it). A listed test is not run at all, so one that passes again is not reported: take it off the
# list when it is fixed (notes/LINUX-NOTES.md says how each was found). The list may only shrink.
#
# Plain `cargo test` (libtest), as the mac job runs it: one process per test binary, each through
# scripts/isolated-test-runner.py (.cargo/config.toml). The CI job (native.yml, linux-tests) runs this on ubuntu-24.04
# with libasound2-dev, libxkbcommon-dev and sqlite3 installed.
set -euo pipefail

cd "$(dirname "$0")/.."
list=parity/linux-not-yet.txt

# The session_import tests build a Cursor store with the sqlite3 program (GitHub's image has it; a bare container may
# not). Say so now, not as three failures ten minutes in.
if ! command -v sqlite3 > /dev/null; then
  echo "linux-tests: the sqlite3 program is needed (session_import tests): apt-get install sqlite3"
  exit 1
fi

# A Claude config folder set in the shell that runs this is not the tests': the isolated runner replaces HOME but
# passes CLAUDE_CONFIG_DIR through, and live2_claude_account_tests then sees it (notes/LINUX-NOTES.md).
unset CLAUDE_CONFIG_DIR

names=()
while IFS= read -r line || [ -n "$line" ]; do
  line="${line%%#*}"
  line="${line#"${line%%[![:space:]]*}"}"
  line="${line%"${line##*[![:space:]]}"}"
  [ -n "$line" ] && names+=("$line")
done < "$list"
echo "linux-tests: $list names ${#names[@]} tests, which are not run"

cargo test --workspace --no-run

# Every name on the list must still be a test.
all="$(mktemp)"
trap 'rm -f "$all"' EXIT
cargo test --workspace -q -- --list --format terse 2> /dev/null | sed -n 's/: test$//p' | sort -u > "$all"
if [ ! -s "$all" ]; then
  echo "linux-tests: cargo test -- --list listed no tests"
  exit 1
fi
missing=0
for name in "${names[@]}"; do
  if ! grep -qxF -- "$name" "$all"; then
    echo "linux-tests: $list names \"$name\", which is no longer a test: take it off the list"
    missing=1
  fi
done
[ "$missing" -eq 0 ] || exit 1
echo "linux-tests: every listed name is a test ($(wc -l < "$all" | tr -d ' ') tests in all)"

skips=()
for name in "${names[@]}"; do
  skips+=(--skip "$name")
done

# --exact makes each --skip match one whole test name, not every name containing it.
set +e
cargo test --workspace --no-fail-fast -- --exact "${skips[@]}"
status=$?
set -e
if [ "$status" -ne 0 ]; then
  echo "linux-tests: FAILED: a test not on $list failed (cargo test exit $status); its name is under failures: above"
  exit "$status"
fi
echo "linux-tests: every test not on $list passed (${#names[@]} listed, not run)"
