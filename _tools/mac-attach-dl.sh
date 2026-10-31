#!/bin/bash
# mac-attach-dl.sh VERSION [RUN_ID] (since 0.783): build VERSION's Mac images on StopBanning's PUBLIC ci.yml (free
# minutes; never the private mac-build.yml, frozen at 0.646), require both macOS smokes to have PASSED in the job log,
# and upload the two dmgs to dl.locust.lol. publish-dl.sh runs it after it has pushed main at VERSION's commit.
# Found 10/09: no release from 0.701 to 0.711 carried a dmg, so Mac copies sat on 0.700 and the README's
# "latest release" link offered Mac users nothing.
V="$1"; RUN="$2"
PUB=StopBanning/locust
# Where this runs (2026-10-30): the PC, until it retires, or the Mac. The repository is the one holding this script.
R=$(cd "$(dirname "$0")/.." && pwd)
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) SCRATCH=/c/work/scratch ;; *) SCRATCH="$HOME/work/scratch" ;; esac
TEMP="${TEMP:-${TMPDIR:-/tmp}}"; TEMP="${TEMP%/}"
[ -n "$V" ] || { echo "usage: mac-attach-dl.sh VERSION [RUN_ID]"; exit 1; }
cd "$R" || exit 1
if curl -sfI "https://dl.locust.lol/Locust-$V-mac.json" > /dev/null; then echo "MAC: $V already has its Mac images"; exit 0; fi
git fetch -q stopbanning
# SHA: the release commit, which a later commit on main may follow (the version is read from that commit): its tag,
# made by publish-dl.sh at its push, so work committed while the Mac and Linux builds run is never taken for the
# release (0.791's Linux packages were refused so); HEAD only before there is a tag.
SHA=${SHA:-$(git rev-list -n 1 "refs/tags/$V" 2>/dev/null || git rev-parse HEAD)}
git show "$SHA:apps/desktop/package.json" | grep -q "\"version\": \"$V\"" || { echo "MAC: $SHA is not $V"; exit 1; }
if [ -z "$RUN" ]; then
  [ "$(git rev-parse stopbanning/main)" = "$SHA" ] || { echo "MAC: stopbanning/main is not $V's commit $SHA"; exit 1; }
  # Only a run started by hand (workflow_dispatch) packages; a push runs the tests alone. Reuse a dispatched run of
  # this commit if there is one, otherwise start one.
  RUN=$(gh run list -R "$PUB" -w ci.yml -L 10 --json databaseId,headSha,event -q "[.[] | select(.headSha==\"$SHA\" and .event==\"workflow_dispatch\")][0].databaseId // empty")
  if [ -z "$RUN" ]; then
    gh workflow run ci.yml --ref main -R "$PUB" || { echo "MAC: could not start ci.yml"; exit 1; }
    for i in $(seq 1 30); do
      sleep 5
      RUN=$(gh run list -R "$PUB" -w ci.yml -L 3 --json databaseId,headSha,event -q "[.[] | select(.headSha==\"$SHA\" and .event==\"workflow_dispatch\")][0].databaseId // empty")
      [ -n "$RUN" ] && break
    done
  fi
fi
[ -n "$RUN" ] || { echo "MAC: no ci.yml run found for $SHA"; exit 1; }
HEAD_OF_RUN=$(gh run view "$RUN" -R "$PUB" --json headSha -q .headSha)
[ "$HEAD_OF_RUN" = "$SHA" ] || { echo "MAC: run $RUN built $HEAD_OF_RUN, not $V's commit $SHA"; exit 1; }
echo "MAC: run $RUN on $SHA"
# Not `gh run watch --exit-status`: on 10/16 it gave up on a run that then succeeded. Poll the run itself.
# The macOS job's own result, not the run's (0.789): the run also builds Linux, and a Linux check that fails says
# nothing about the Mac images. Done when that job is, or when the run ended without ever starting it.
MACJOB='[.jobs[] | select(.name|test("macos"))][0]'
until gh run view "$RUN" -R "$PUB" --json status,jobs -q "(.status == \"completed\") or (($MACJOB.status // \"\") == \"completed\")" 2>/dev/null | grep -qx true; do sleep 60; done
[ "$(gh run view "$RUN" -R "$PUB" --json jobs -q "$MACJOB.conclusion // \"never ran\"")" = success ] || { echo "MAC: the macOS job of run $RUN did not succeed"; exit 1; }
JOB=$(gh run view "$RUN" -R "$PUB" --json jobs -q "$MACJOB.databaseId")
LOG="$TEMP/mac-$V.log"
# The job's own log (0.789): `gh run view --log` answers only once the WHOLE run is done, and the run also
# builds the other systems, so it read "still in progress" here and failed a job that had passed.
gh api --allow-escape-sequences "repos/$PUB/actions/jobs/$JOB/logs" > "$LOG" 2>&1
if ! grep -q "MAC STOP SMOKE PASSED" "$LOG" || ! grep -q "MAC UPDATE SMOKE PASSED" "$LOG"; then echo "MAC: a macOS check did not pass (see $LOG)"; exit 1; fi
echo "MAC: $(grep -c '\[PASS\]' "$LOG") checks PASS on macOS, $(grep -c '\[FAIL\]' "$LOG") FAIL"
# C:, not D: -- D: damages files at rest (memory d-drive-ntfs-corruption); these are deleted as soon as they are up.
D=$SCRATCH/mac-$V
rm -rf "$D"; mkdir -p "$D"
gh run download "$RUN" -R "$PUB" -n locust-macos-14 -D "$D" || { echo "MAC: download failed"; exit 1; }
ARM="$D/Locust-$V-mac-arm64.dmg"; X64="$D/Locust-$V-mac-x64.dmg"
[ -f "$ARM" ] && [ -f "$X64" ] || { echo "MAC: the images are not named for $V: $(ls "$D")"; exit 1; }
# A disk image ends in its 512-byte "koly" trailer; a cut-off download does not.
for f in "$ARM" "$X64"; do
  [ "$(tail -c 512 "$f" | head -c 4)" = "koly" ] || { echo "MAC: $(basename "$f") has no disk-image trailer"; exit 1; }
done
node "$R/_tools/publish-downloads.mjs" "$V" --mac "$D" || { echo "MAC: upload failed"; exit 1; }
node "$R/_tools/publish-downloads.mjs" "$V" --promote || { echo "MAC: the Mac list was not updated"; exit 1; }
rm -rf "$D"
echo "MAC ATTACHED: $V (arm64 + x64)"
