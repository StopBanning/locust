#!/bin/bash
# linux-attach-dl.sh VERSION [RUN_ID] (0.789): VERSION's Linux packages from the public ci.yml run that built its Mac
# images (mac-attach-dl.sh starts it; one is started here only if there is none), required to have PASSED their checks
# in the Linux job's log -- the .deb installed with apt and stopped mid-turn, the AppImage opened on a stock Ubuntu and
# updated itself -- then uploaded to dl.locust.lol with their feed, and promoted. publish-dl.sh runs it after the Mac images.
V="$1"; RUN="$2"
PUB=StopBanning/locust
R=/c/work/locust
[ -n "$V" ] || { echo "usage: linux-attach-dl.sh VERSION [RUN_ID]"; exit 1; }
cd "$R" || exit 1
if curl -sfI "https://dl.locust.lol/Locust-$V-linux.json" > /dev/null; then echo "LINUX: $V already has its Linux packages"; exit 0; fi
git fetch -q stopbanning
# SHA: the release commit, which a later commit on main may follow (the version is read from that commit).
SHA=${SHA:-$(git rev-parse HEAD)}
git show "$SHA:apps/desktop/package.json" | grep -q "\"version\": \"$V\"" || { echo "LINUX: $SHA is not $V"; exit 1; }
dispatched() {
  gh run list -R "$PUB" -w ci.yml -L 10 --json databaseId,headSha,event -q "[.[] | select(.headSha==\"$SHA\" and .event==\"workflow_dispatch\")][0].databaseId // empty"
}
if [ -z "$RUN" ]; then
  RUN=$(dispatched)
  if [ -z "$RUN" ]; then
    [ "$(git rev-parse stopbanning/main)" = "$SHA" ] || { echo "LINUX: stopbanning/main is not $V's commit $SHA"; exit 1; }
    gh workflow run ci.yml --ref main -R "$PUB" || { echo "LINUX: could not start ci.yml"; exit 1; }
    for i in $(seq 1 30); do
      sleep 5
      RUN=$(dispatched)
      [ -n "$RUN" ] && break
    done
  fi
fi
[ -n "$RUN" ] || { echo "LINUX: no ci.yml run found for $SHA"; exit 1; }
HEAD_OF_RUN=$(gh run view "$RUN" -R "$PUB" --json headSha -q .headSha)
[ "$HEAD_OF_RUN" = "$SHA" ] || { echo "LINUX: run $RUN built $HEAD_OF_RUN, not $V's commit $SHA"; exit 1; }
echo "LINUX: run $RUN on $SHA"
# The Linux job's own result, as mac-attach-dl.sh reads the macOS job's. Polled, never `gh run watch` (10/16).
LINUXJOB='[.jobs[] | select(.name|test("ubuntu"))][0]'
until gh run view "$RUN" -R "$PUB" --json status,jobs -q "(.status == \"completed\") or (($LINUXJOB.status // \"\") == \"completed\")" 2>/dev/null | grep -qx true; do sleep 60; done
[ "$(gh run view "$RUN" -R "$PUB" --json jobs -q "$LINUXJOB.conclusion // \"never ran\"")" = success ] || { echo "LINUX: the Linux job of run $RUN did not succeed"; exit 1; }
JOB=$(gh run view "$RUN" -R "$PUB" --json jobs -q "$LINUXJOB.databaseId")
LOG="$TEMP/linux-$V.log"
# The job's own log (0.789): `gh run view --log` answers only once the WHOLE run is done, and the run also
# builds the other systems, so it read "still in progress" here and failed a job that had passed.
gh api --allow-escape-sequences "repos/$PUB/actions/jobs/$JOB/logs" > "$LOG" 2>&1
if ! grep -q "LINUX STOP SMOKE PASSED" "$LOG" || ! grep -q "LINUX LAUNCH SMOKE PASSED" "$LOG" || ! grep -q "LINUX UPDATE SMOKE PASSED" "$LOG"; then echo "LINUX: a Linux check did not pass (see $LOG)"; exit 1; fi
echo "LINUX: $(grep -c '\[PASS\]' "$LOG") checks PASS on Linux, $(grep -c '\[FAIL\]' "$LOG") FAIL"
# C:, not D: -- D: damages files at rest (memory d-drive-ntfs-corruption); these are deleted as soon as they are up.
D=/c/work/scratch/linux-$V
rm -rf "$D"; mkdir -p "$D"
gh run download "$RUN" -R "$PUB" -n locust-ubuntu-24.04 -D "$D" || { echo "LINUX: download failed"; exit 1; }
IMG="$D/Locust-$V-linux-x86_64.AppImage"; DEB="$D/Locust-$V-linux-amd64.deb"; RPM="$D/Locust-$V-linux-x86_64.rpm"
[ -f "$IMG" ] && [ -f "$DEB" ] && [ -f "$RPM" ] && [ -f "$D/latest-linux.yml" ] || { echo "LINUX: the packages are not named for $V: $(ls "$D")"; exit 1; }
# Each by its own signature: an ELF that says it is a type-2 AppImage, an ar archive, an RPM lead. The AppImage's
# whole length is held to the feed's sha512 by publish-downloads.mjs.
hex() { od -An -tx1 -j "$2" -N "$3" "$1" | tr -d ' \n'; }
[ "$(hex "$IMG" 0 4)" = 7f454c46 ] && [ "$(hex "$IMG" 8 3)" = 414902 ] || { echo "LINUX: $(basename "$IMG") is not an AppImage"; exit 1; }
[ "$(hex "$DEB" 0 8)" = 213c617263683e0a ] || { echo "LINUX: $(basename "$DEB") is not a Debian package"; exit 1; }
[ "$(hex "$RPM" 0 4)" = edabeedb ] || { echo "LINUX: $(basename "$RPM") is not an RPM"; exit 1; }
node "$R/_tools/publish-downloads.mjs" "$V" --linux "$D" || { echo "LINUX: upload failed"; exit 1; }
node "$R/_tools/publish-downloads.mjs" "$V" --promote || { echo "LINUX: the Linux feed was not promoted"; exit 1; }
rm -rf "$D"
echo "LINUX ATTACHED: $V (AppImage, .deb, .rpm)"
