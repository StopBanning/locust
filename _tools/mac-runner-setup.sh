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
# 0.856's first Mac job (2026-11-02): under svc.sh's agent, the job's git could not resolve github.com, though the
# runner's own downloads worked; from a Terminal it checked out fine. svc.sh's SessionCreate was suspected, but git
# under the agent with it resolves (this script's own check). So each start first asks GitHub through git, as a job
# would, into _diag/lookup.log, which this script reads back.
PLIST="$HOME/Library/LaunchAgents/actions.runner.StopBanning-locust.mac-mini.plist"
cat > start.sh <<'START'
#!/bin/bash
cd "$(dirname "$0")"
{ date -u; /usr/bin/git ls-remote https://github.com/StopBanning/locust HEAD 2>&1 && echo "LOOKUP OK" || echo "LOOKUP FAILED"; } > _diag/lookup.log
exec ./runsvc.sh
START
chmod 755 start.sh
plutil -replace ProgramArguments -json "[\"$DIR/start.sh\"]" "$PLIST"
rm -f _diag/lookup.log
./svc.sh start
./svc.sh status | tail -3
for i in $(seq 1 20); do grep -q "LOOKUP" _diag/lookup.log 2>/dev/null && break; sleep 1; done
cat _diag/lookup.log 2>/dev/null || echo "no lookup log"
