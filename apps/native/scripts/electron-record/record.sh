#!/bin/bash
# Records Electron's answers for the Rust port to replay: bash scripts/electron-record/record.sh [picker]
# Bundles scripts/electron-record/<name>.ts with esbuild (from an Electron checkout's node_modules) and runs it in Node;
# writes parity/recorded/<name>.jsonl. LOCUST_ELECTRON_CHECKOUT names a checkout with node_modules (default ~/work/locust).
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
name="${1:-picker}"
electron="${LOCUST_ELECTRON_CHECKOUT:-$HOME/work/locust}"
esbuild="$(ls -d "$electron"/node_modules/.pnpm/esbuild@*/node_modules/esbuild/bin/esbuild | sort -V | tail -1)"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
"$esbuild" "$here/scripts/electron-record/$name.ts" --bundle --platform=node --format=esm --log-level=warning \
  --outfile="$out/$name.mjs" --external:electron
mkdir -p "$here/parity/recorded"
ELECTRON_RENDERER_SRC="$(cd "$here/../desktop/src/renderer/src" && pwd)" node "$out/$name.mjs" > "$here/parity/recorded/$name.jsonl"
echo "$(wc -l < "$here/parity/recorded/$name.jsonl" | tr -d ' ') cases recorded in parity/recorded/$name.jsonl"
