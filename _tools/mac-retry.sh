#!/bin/bash
# mac-retry.sh RUN [TRIES] (2026-11-01): rerun a release run's macOS job while GitHub has no arm64 runner for it --
# only when the last attempt was cancelled with no runner assigned, never after a real failure. Prints one line when it
# ends. Then mac-attach-dl.sh VERSION RUN and github-release.mjs VERSION.
RUN="$1"; TRIES="${2:-2}"; REPO=StopBanning/locust
mac_state() { gh run view "$RUN" -R "$REPO" --json jobs --jq '.jobs[] | select(.name|test("macos")) | "\(.status) \(.conclusion)"' 2>/dev/null; }
last_runner() {
  local attempt; attempt=$(gh run view "$RUN" -R "$REPO" --json attempt --jq .attempt 2>/dev/null)
  gh api "repos/$REPO/actions/runs/$RUN/attempts/$attempt/jobs" --jq '.jobs[] | select(.name|test("macos")) | .runner_name // ""' 2>/dev/null
}
for try in $(seq 1 "$TRIES"); do
  for i in $(seq 1 60); do
    s=$(mac_state); case "$s" in completed*) break;; esac; sleep 60
  done
  case "$s" in
    "completed success") echo "MAC JOB SUCCEEDED after $try attempt(s) of this loop"; exit 0;;
    "completed cancelled")
      if [ -n "$(last_runner)" ]; then echo "MAC JOB CANCELLED ON A RUNNER (not a shortage): stopping"; exit 1; fi
      sleep 600
      gh run rerun "$RUN" --failed -R "$REPO" >/dev/null 2>&1 || { echo "RERUN REFUSED"; exit 1; }
      sleep 20;;
    *) echo "MAC JOB ENDED: $s (not a shortage): stopping"; exit 1;;
  esac
done
echo "MAC JOB STILL HAS NO RUNNER after $TRIES reruns"
exit 1
