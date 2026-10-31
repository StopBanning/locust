#!/bin/bash
# publish-ci.sh NEW PREV (2026-10-30): a whole release from the Mac, every system built by StopBanning's public ci.yml,
# now that the PC that built Windows releases is retired. publish-dl.sh is the PC's way, and stays for it.
#
#   LOCUST_MARKERS=$'a string the release adds\nanother one' bash _tools/publish-ci.sh 0.844.0 0.843.0
#
# Run at the release commit on main, with its CHANGELOG entry and What's new already committed. In order:
#   1. ship.mjs --check-only: the typecheck, the tests, a version not yet published, a clean tree, a CHANGELOG entry;
#   2. main pushed to StopBanning/locust and tagged NEW, because CI builds the public main;
#   3. win-attach-dl.sh starts one ci.yml run with the markers, requires the Windows build's checks, then publishes the
#      installer, block map, feed and window to dl.locust.lol and promotes them;
#   4. the differential check: electron-updater's own download from PREV's installer to NEW, over the published block
#      maps, checked against the published sha512 (the update smoke's Windows half stays with the PC);
#   5. mac-attach-dl.sh and linux-attach-dl.sh, from the same run;
#   6. github-release.mjs: the Releases page, unless LOCUST_GITHUB_RELEASE=0.
# Each step stops the release on a failure, saying which; a step run again finds what is already done.
NEW="$1"; PREV="$2"
[ -n "$NEW" ] && [ -n "$PREV" ] || { echo "usage: publish-ci.sh NEW PREV"; exit 1; }
R=$(cd "$(dirname "$0")/.." && pwd)
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) SCRATCH=/c/work/scratch ;; *) SCRATCH="$HOME/work/scratch" ;; esac
TEMP="${TEMP:-${TMPDIR:-/tmp}}"; TEMP="${TEMP%/}"
export TEMP
G='credential.helper=!gh auth git-credential'
cd "$R" || exit 1
[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || { echo "publish from main"; exit 1; }
grep -q "\"version\": \"$NEW\"" apps/desktop/package.json || { echo "apps/desktop/package.json is not $NEW"; exit 1; }

MARKS=()
while IFS= read -r line; do [ -n "$line" ] && MARKS+=(--marker "$line"); done <<< "${LOCUST_MARKERS:-}"
node _tools/ship.mjs --check-only "${MARKS[@]}" > "$TEMP/ship-$NEW.log" 2>&1 || { echo "the gates FAILED"; grep -E "FAIL|failed" "$TEMP/ship-$NEW.log" | head -8; exit 1; }
echo "gates pass"

git -c credential.helper= -c "$G" push -q stopbanning HEAD:main || { echo "push FAILED"; exit 1; }
# The tag stays local: the attach scripts read it, and github-release.mjs puts tags up its own way.
git tag "$NEW" HEAD 2>/dev/null || echo "tag $NEW was already made"
echo "pushed $(git log --oneline -1) to StopBanning/locust"

echo "Windows: see $TEMP/win-$NEW.out"
bash "$R/_tools/win-attach-dl.sh" "$NEW" > "$TEMP/win-$NEW.out" 2>&1 < /dev/null
tail -1 "$TEMP/win-$NEW.out"
grep -q "WIN ATTACHED\|already on dl.locust.lol" "$TEMP/win-$NEW.out" || { echo "Windows FAILED, so nothing more was published"; tail -5 "$TEMP/win-$NEW.out"; exit 1; }

# The previous installer comes from dl.locust.lol, which keeps every version's by name.
mkdir -p "$SCRATCH"
OLD="$SCRATCH/Locust-$PREV-setup.exe"
curl -sfL -o "$OLD" "https://dl.locust.lol/Locust-$PREV-setup.exe" || { echo "Locust-$PREV-setup.exe could not be downloaded for the differential check"; exit 1; }
(cd "$R/apps/desktop" && npx electron "$R/_tools/probe-differential-update.cjs" "$PREV" "$NEW" "$OLD") > "$TEMP/diff-$NEW.log" 2>&1
d=$?
rm -f "$OLD"
[ $d -eq 0 ] || { echo "differential FAILED"; tail -5 "$TEMP/diff-$NEW.log"; exit 1; }
grep -E "arrived|DIFFERENTIAL" "$TEMP/diff-$NEW.log"

echo "Mac images: see $TEMP/mac-$NEW.out"
bash "$R/_tools/mac-attach-dl.sh" "$NEW" > "$TEMP/mac-$NEW.out" 2>&1 < /dev/null
tail -1 "$TEMP/mac-$NEW.out"
echo "Linux packages: see $TEMP/linux-$NEW.out"
bash "$R/_tools/linux-attach-dl.sh" "$NEW" > "$TEMP/linux-$NEW.out" 2>&1 < /dev/null
tail -1 "$TEMP/linux-$NEW.out"
if [ "$LOCUST_GITHUB_RELEASE" != 0 ]; then
  echo "GitHub release: see $TEMP/github-$NEW.out"
  node "$R/_tools/github-release.mjs" "$NEW" > "$TEMP/github-$NEW.out" 2>&1 < /dev/null
  tail -1 "$TEMP/github-$NEW.out"
else
  echo "GitHub release page: skipped (LOCUST_GITHUB_RELEASE=0)"
fi
echo "RELEASED $NEW"
