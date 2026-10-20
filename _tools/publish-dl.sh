#!/bin/bash
export PNPM_CONFIG_STORE_DIR=C:/work/pnpm-store
# publish-dl.sh NEW PREV: publish NEW to Locust's own downloads (dl.locust.lol), promote it for testers, run the
# update smoke and the differential check, push the code to StopBanning/locust, then the Mac images.
# Since 0.783: GitHub holds only code, CI and the Mac build (locust-lol was suspended 2026-10-16).
# Run after `node _tools/ship.mjs --marker "<a string the release adds>"` passed, with TEMP set as it was for ship:
#   TEMP=<the folder ship used> TMP=<the same> bash _tools/publish-dl.sh 0.784.0 0.783.0
#
# A QUIET GITHUB (2026-10-20). Each release is a push, its test and guard runs, a dispatched package run and, with a
# Releases page, a second push for the tag: six releases in a day is dozens of automated runs. So, unless asked:
#   - one release a day: this refuses within 20 hours of PREV's tag (LOCUST_RELEASE_ANYWAY=1 overrides);
#   - no Releases page: dl.locust.lol serves every file (LOCUST_GITHUB_RELEASE=1 makes one, as before).
NEW="$1"; PREV="$2"
[ -n "$NEW" ] && [ -n "$PREV" ] || { echo "usage: publish-dl.sh NEW PREV"; exit 1; }
R=/c/work/locust
G='credential.helper=!gh auth git-credential'
cd "$R" || exit 1
if [ "$LOCUST_RELEASE_ANYWAY" != 1 ]; then
  PREV_AT=$(git log -1 --format=%ct "refs/tags/$PREV" 2>/dev/null)
  [ -n "$PREV_AT" ] || { echo "no tag $PREV to measure a day from; LOCUST_RELEASE_ANYWAY=1 publishes anyway"; exit 1; }
  HOURS=$(( ($(date +%s) - PREV_AT) / 3600 ))
  [ "$HOURS" -ge 20 ] || { echo "REFUSED: $PREV went out ${HOURS}h ago; one release a day (LOCUST_RELEASE_ANYWAY=1 overrides)"; exit 1; }
fi
node _tools/publish-downloads.mjs "$NEW" > "$TEMP/pub.log" 2>&1 || { echo "publish FAILED"; tail -5 "$TEMP/pub.log"; exit 1; }
tail -1 "$TEMP/pub.log"
node _tools/publish-downloads.mjs "$NEW" --promote > "$TEMP/promote.log" 2>&1 || { echo "promote FAILED"; tail -5 "$TEMP/promote.log"; exit 1; }
tail -1 "$TEMP/promote.log"
node _smoke/update-smoke.mjs > "$TEMP/us.log" 2>&1 || { echo "update smoke FAILED"; tail -5 "$TEMP/us.log"; exit 1; }
tail -1 "$TEMP/us.log"
cd "$R/apps/desktop" || exit 1
# The last release's installer, kept by the previous publish, is what the differential check updates from.
CACHE=/c/work/scratch/installers
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
# Only when asked since 2026-10-20 (A QUIET GITHUB, above).
if [ "$LOCUST_GITHUB_RELEASE" = 1 ]; then
  echo "GitHub release: see $TEMP/github-$NEW.out"
  node "$R/_tools/github-release.mjs" "$NEW" > "$TEMP/github-$NEW.out" 2>&1 < /dev/null
  tail -1 "$TEMP/github-$NEW.out"
else
  echo "GitHub release page: skipped (LOCUST_GITHUB_RELEASE=1 makes one)"
fi
