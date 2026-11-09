#!/bin/bash
# macOS only. Uses the system Ruby and standard library; never controls Electron.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
if [[ "${1:-}" == "--test" ]]; then
  exec /usr/bin/ruby "$HERE/scripts/bench.rb" --test
fi
[[ "$(uname -s)" == Darwin ]] || { echo 'bench.sh requires macOS' >&2; exit 1; }
exec /usr/bin/ruby "$HERE/scripts/bench.rb" "${1:-./bench-out}" "$HERE"
