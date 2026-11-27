#!/bin/sh
# Approve the look of locked scenes: re-render each named scene into pictures/NAME.png, the reference the picture
# tests (src/picture_tests.rs) hold it to. Run it only after you have looked at the render and want it kept
# (notes/PICTURES.md); the tests never write a reference themselves.
#
#   scripts/picture-lock.sh settings people-light      approve two scenes
#   LOCUST_NATIVE_BIN=path/to/locust-native scripts/picture-lock.sh empty
#
# It renders with the release binary (build it first: python3 scripts/gentle-cargo.py build --release), at the time
# and scale the tests use, through scripts/isolated-test-runner.py: the empty HOME and data folders every test gets,
# so nothing of this Mac's own Locust reaches the picture (26 Nov: with the real HOME the Electron settings seeded a
# code font of Paper Mono into the render). Only scenes the tests lock are accepted: add a new one to LOCKED in
# src/picture_tests.rs first.
set -eu
cd "$(dirname "$0")/.."

if [ $# -eq 0 ]; then
    echo "usage: scripts/picture-lock.sh SCENE..." >&2
    exit 2
fi
for scene in "$@"; do
    # A LOCKED line is ("NAME", its_scene_fn), as UNLOCKED's ("KEY", "why") lines are not.
    if ! grep -Eq "^ *\(\"$scene\", [a-z_:]*scene\),\$" src/picture_tests.rs; then
        echo "$scene is not a locked scene (LOCKED in src/picture_tests.rs)" >&2
        exit 2
    fi
done

bin=${LOCUST_NATIVE_BIN:-${CARGO_TARGET_DIR:-$HOME/work/scratch/native-target}/release/locust-native}
if [ ! -x "$bin" ]; then
    echo "no release binary at $bin: build it (python3 scripts/gentle-cargo.py build --release) or set LOCUST_NATIVE_BIN" >&2
    exit 1
fi
mkdir -p pictures
for scene in "$@"; do
    python3 scripts/isolated-test-runner.py "$bin" --scene "$scene" --snap "$PWD/pictures/$scene.png" --at 1 --scale 1 >/dev/null
    rm -f "pictures/$scene.new.png" "pictures/$scene.diff.png"
    echo "approved pictures/$scene.png"
done
