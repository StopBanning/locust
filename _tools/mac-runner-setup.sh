#!/bin/bash
# mac-runner-setup.sh (2026-11-02): makes Colin's Mac mini the runner for ci.yml's Mac build. GitHub's macos-15 queue
# gave 0.851-0.855 no runner for hours (0.855: thirteen attempts, each a failure email); Colin: "let's definitely do it
# ourselves". Run it from the repository, on that Mac, once GitHub's runner is unpacked in ~/work/gh-runner
# (github.com/actions/runner releases, osx-arm64, checksum from the release notes). Again is safe: it replaces the
# registration and the hook.
#
# - Registered to StopBanning/locust only, labelled locust-mac (ci.yml's runs-on).
# - Its job-started hook is mac-runner-guard.sh, copied beside it: every job but ci.yml, started by hand on main by
#   StopBanning, fails before its first step. Pull requests never reach this Mac.
# - Runs as a LaunchAgent of this user (svc.sh), so it starts at login; no sudo.
set -euo pipefail
REPO=StopBanning/locust
DIR="$HOME/work/gh-runner"
cd "$(dirname "$0")/.."
[ -x "$DIR/config.sh" ] || { echo "no runner in $DIR"; exit 1; }
cp _tools/mac-runner-guard.sh "$DIR/guard.sh"
chmod 755 "$DIR/guard.sh"
cd "$DIR"
if [ -f .runner ]; then
  # Registered already: keep it, and only bring the hook and the service up to date.
  ./svc.sh stop >/dev/null 2>&1 || true
else
  # The registration token is asked for here and handed straight to config.sh: never printed, never kept.
  ./config.sh --unattended --replace --url "https://github.com/$REPO" --name mac-mini --labels locust-mac --work _work \
    --token "$(gh api -X POST "repos/$REPO/actions/runners/registration-token" --jq .token)"
fi
grep -v '^ACTIONS_RUNNER_HOOK_JOB_STARTED=' .env > .env.new 2>/dev/null || true
echo "ACTIONS_RUNNER_HOOK_JOB_STARTED=$DIR/guard.sh" >> .env.new
mv .env.new .env
./svc.sh install >/dev/null 2>&1 || true
./svc.sh start
./svc.sh status | tail -3
