#!/bin/bash
# publish-dl.sh NEW PREV: publish NEW to Locust's own downloads (dl.locust.lol), promote it for testers, run the
# update smoke and the differential check, push the code to StopBanning/locust, then the Mac images.
# Since 0.783: GitHub holds only code, CI and the Mac build (locust-lol was suspended 2026-10-16).
# Run after `node _tools/ship.mjs --marker "<a string the release adds>"` passed, with TEMP set as it was for ship:
#   TEMP=<the folder ship used> TMP=<the same> bash _tools/publish-dl.sh 0.784.0 0.783.0
#
# THE RELEASES PAGE IS BACK (2026-10-22). For two days (A QUIET GITHUB, 2026-10-20) a release made no page on
# GitHub, and the Releases tab stopped at 0.808 (Colin: "you havent been updating the releases tab"). Every release
# makes its page again, with its six files; LOCUST_GITHUB_RELEASE=0 skips it. The one-release-a-day gate is gone too
# (Colin, 2026-10-21: "Doesn't need to be one release a day anymore"); a release goes when ready.
NEW="$1"; PREV="$2"
[ -n "$NEW" ] && [ -n "$PREV" ] || { echo "usage: publish-dl.sh NEW PREV"; exit 1; }
# Where this runs (2026-10-30): the PC, until it retires, or the Mac. The repository is the one holding this script.
R=$(cd "$(dirname "$0")/.." && pwd)
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) SCRATCH=/c/work/scratch; export PNPM_CONFIG_STORE_DIR=C:/work/pnpm-store ;; *) SCRATCH="$HOME/work/scratch" ;; esac
TEMP="${TEMP:-${TMPDIR:-/tmp}}"; TEMP="${TEMP%/}"
G='credential.helper=!gh auth git-credential'
cd "$R" || exit 1
node _tools/publish-downloads.mjs "$NEW" > "$TEMP/pub.log" 2>&1 || { echo "publish FAILED"; tail -5 "$TEMP/pub.log"; exit 1; }
tail -1 "$TEMP/pub.log"
node _tools/publish-downloads.mjs "$NEW" --promote > "$TEMP/promote.log" 2>&1 || { echo "promote FAILED"; tail -5 "$TEMP/promote.log"; exit 1; }
tail -1 "$TEMP/promote.log"
node _smoke/update-smoke.mjs > "$TEMP/us.log" 2>&1 || { echo "update smoke FAILED"; tail -5 "$TEMP/us.log"; exit 1; }
tail -1 "$TEMP/us.log"
cd "$R/apps/desktop" || exit 1
# The last release's installer, kept by the previous publish, is what the differential check updates from.
CACHE=$SCRATCH/installers
[ -f "$CACHE/Locust-$PREV-setup.exe" ] || { echo "no cached Locust-$PREV-setup.exe for the differential check"; exit 1; }
cp "$CACHE/Locust-$PREV-setup.exe" release/
npx electron "$R/_tools/probe-differential-update.cjs" "$PREV" "$NEW" "release/Locust-$PREV-setup.exe" > "$TEMP/diff.log" 2>&1
d=$?
rm -f "release/Locust-$PREV-setup.exe"
[ $d -eq 0 ] || { echo "differential FAILED"; tail -5 "$TEMP/diff.log"; exit 1; }
grep -E "arrived|DIFFERENTIAL" "$TEMP/diff.log"
cd "$R" || exit 1
git -c credential.helper= -c "$G" push -q stopbanning HEAD:main || { echo "push FAILED"; exit 1; }
echo "pushed $(git log --oneline -1) to StopBanning/locust"
# The build's tag, at the commit it was made from; github-release.mjs below puts it on GitHub (0.790).
git tag "$NEW" HEAD 2>/dev/null || echo "tag $NEW was already made"
mkdir -p "$CACHE" && rm -f "$CACHE"/Locust-*-setup.exe && cp "$R/apps/desktop/release/Locust-$NEW-setup.exe" "$CACHE/" && echo "cached Locust-$NEW-setup.exe for the next release check"
echo "ALL PUBLISHED"
echo "Mac images: see $TEMP/mac-$NEW.out"
bash "$(dirname "$0")/mac-attach-dl.sh" "$NEW" > "$TEMP/mac-$NEW.out" 2>&1 < /dev/null
tail -1 "$TEMP/mac-$NEW.out"
# The Linux packages, from the same CI run the Mac images came from (0.789).
echo "Linux packages: see $TEMP/linux-$NEW.out"
bash "$(dirname "$0")/linux-attach-dl.sh" "$NEW" > "$TEMP/linux-$NEW.out" 2>&1 < /dev/null
tail -1 "$TEMP/linux-$NEW.out"
# The Releases page on StopBanning/locust: the same six files and their sums, once dl.locust.lol has them all (0.790).
# Every release since 2026-10-22 (above); LOCUST_GITHUB_RELEASE=0 skips it.
if [ "$LOCUST_GITHUB_RELEASE" != 0 ]; then
  echo "GitHub release: see $TEMP/github-$NEW.out"
  node "$R/_tools/github-release.mjs" "$NEW" > "$TEMP/github-$NEW.out" 2>&1 < /dev/null
  tail -1 "$TEMP/github-$NEW.out"
else
  echo "GitHub release page: skipped (LOCUST_GITHUB_RELEASE=0)"
fi
