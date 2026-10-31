#!/bin/bash
# win-attach-dl.sh VERSION [RUN_ID] (2026-10-30): VERSION's Windows installer from StopBanning's public ci.yml, the way
# mac-attach-dl.sh takes the Mac images, now that the PC that built it is retired. Requires the Windows job to have
# PASSED its checks in its own log -- the window zip made, and the archive, installer, feed, files beside the archive
# and every changelog marker checked (_tools/check-artifact.mjs) -- then puts the installer, its block map, its feed
# and its window zip where publish-downloads.mjs reads them, publishes them to dl.locust.lol and promotes them.
#
# It starts the run when there is none for VERSION's commit, handing it the markers from LOCUST_MARKERS (one a line),
# so start it first: mac-attach-dl.sh and linux-attach-dl.sh then reuse the same run. publish-ci.sh does all of that.
V="$1"; RUN="$2"
PUB=StopBanning/locust
# Where this runs: the Mac, or the PC until it retires. The repository is the one holding this script.
R=$(cd "$(dirname "$0")/.." && pwd)
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) SCRATCH=/c/work/scratch ;; *) SCRATCH="$HOME/work/scratch" ;; esac
TEMP="${TEMP:-${TMPDIR:-/tmp}}"; TEMP="${TEMP%/}"
[ -n "$V" ] || { echo "usage: win-attach-dl.sh VERSION [RUN_ID]"; exit 1; }
cd "$R" || exit 1
if curl -sfI "https://dl.locust.lol/Locust-$V.yml" > /dev/null; then echo "WIN: $V is already on dl.locust.lol"; exit 0; fi
git fetch -q stopbanning
# SHA: the release commit, by its tag (publish-ci.sh tags it at its push), so work committed while CI builds is never
# taken for the release; HEAD only before there is a tag.
SHA=${SHA:-$(git rev-list -n 1 "refs/tags/$V" 2>/dev/null || git rev-parse HEAD)}
git show "$SHA:apps/desktop/package.json" | grep -q "\"version\": \"$V\"" || { echo "WIN: $SHA is not $V"; exit 1; }
dispatched() {
  gh run list -R "$PUB" -w ci.yml -L 10 --json databaseId,headSha,event -q "[.[] | select(.headSha==\"$SHA\" and .event==\"workflow_dispatch\")][0].databaseId // empty"
}
if [ -z "$RUN" ]; then
  RUN=$(dispatched)
  if [ -z "$RUN" ]; then
    [ "$(git rev-parse stopbanning/main)" = "$SHA" ] || { echo "WIN: stopbanning/main is not $V's commit $SHA"; exit 1; }
    gh workflow run ci.yml --ref main -R "$PUB" -f only=all -f markers="${LOCUST_MARKERS:-}" || { echo "WIN: could not start ci.yml"; exit 1; }
    for i in $(seq 1 30); do
      sleep 5
      RUN=$(dispatched)
      [ -n "$RUN" ] && break
    done
  fi
fi
[ -n "$RUN" ] || { echo "WIN: no ci.yml run found for $SHA"; exit 1; }
HEAD_OF_RUN=$(gh run view "$RUN" -R "$PUB" --json headSha -q .headSha)
[ "$HEAD_OF_RUN" = "$SHA" ] || { echo "WIN: run $RUN built $HEAD_OF_RUN, not $V's commit $SHA"; exit 1; }
echo "WIN: run $RUN on $SHA"
# The Windows package job's own result, as mac-attach-dl.sh reads the macOS job's. Polled, never `gh run watch` (10/16).
# "package (windows-latest)": the test job also runs on Windows, but its name is only "test".
WINJOB='[.jobs[] | select(.name|test("package.*windows"))][0]'
until gh run view "$RUN" -R "$PUB" --json status,jobs -q "(.status == \"completed\") or (($WINJOB.status // \"\") == \"completed\")" 2>/dev/null | grep -qx true; do sleep 60; done
[ "$(gh run view "$RUN" -R "$PUB" --json jobs -q "$WINJOB.conclusion // \"never ran\"")" = success ] || { echo "WIN: the Windows job of run $RUN did not succeed"; exit 1; }
JOB=$(gh run view "$RUN" -R "$PUB" --json jobs -q "$WINJOB.databaseId")
LOG="$TEMP/win-$V.log"
gh api --allow-escape-sequences "repos/$PUB/actions/jobs/$JOB/logs" > "$LOG" 2>&1
grep -q "WINDOWS ARTIFACT PASSED: $V" "$LOG" || { echo "WIN: the Windows build's checks did not pass (see $LOG)"; exit 1; }
echo "WIN: $(grep -c '  ok    ' "$LOG") checks ok on the Windows build, $(grep -c '  FAIL  ' "$LOG") FAIL"
D=$SCRATCH/win-$V
rm -rf "$D"; mkdir -p "$D"
gh run download "$RUN" -R "$PUB" -n locust-windows-latest -D "$D" || { echo "WIN: download failed"; exit 1; }
EXE="Locust-$V-setup.exe"
for f in "$EXE" "$EXE.blockmap" latest.yml "Locust-$V-window.zip"; do
  [ -f "$D/$f" ] || { echo "WIN: the build has no $f: $(ls "$D")"; exit 1; }
done
# An installer is a Windows program (it starts "MZ"); a zip starts "PK". A cut-off download is caught by
# publish-downloads.mjs, which holds the installer to latest.yml's sha512.
[ "$(head -c 2 "$D/$EXE")" = MZ ] || { echo "WIN: $EXE is not a Windows program"; exit 1; }
[ "$(head -c 2 "$D/Locust-$V-window.zip")" = PK ] || { echo "WIN: the window zip is not a zip"; exit 1; }
grep -qx "version: $V" <(tr -d '\r' < "$D/latest.yml") || { echo "WIN: latest.yml is not for $V"; exit 1; }
REL="$R/apps/desktop/release"
mkdir -p "$REL"
for f in "$EXE" "$EXE.blockmap" latest.yml "Locust-$V-window.zip"; do cp "$D/$f" "$REL/$f"; done
node "$R/_tools/publish-downloads.mjs" "$V" || { echo "WIN: upload failed"; exit 1; }
node "$R/_tools/publish-downloads.mjs" "$V" --promote || { echo "WIN: promote failed"; exit 1; }
rm -rf "$D"
echo "WIN ATTACHED: $V (installer, block map, feed, window)"
