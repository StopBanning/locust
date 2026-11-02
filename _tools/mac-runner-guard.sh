#!/bin/bash
# mac-runner-guard.sh (2026-11-02): the job-started hook of the GitHub runner on Colin's Mac mini
# (ACTIONS_RUNNER_HOOK_JOB_STARTED in ~/work/gh-runner/.env). The repository is public, and a pull request can change
# a workflow's runs-on to this Mac, so the runner itself refuses every job but ci.yml's, started by hand on main of
# StopBanning/locust by its owner. A refused job fails before its first step. Copied beside the runner by
# _tools/mac-runner-setup.sh; the copy there is the one that runs.
want_repo=StopBanning/locust
fail() { echo "mac-runner-guard: refused: $1"; exit 1; }
[ "${GITHUB_REPOSITORY:-}" = "$want_repo" ] || fail "repository ${GITHUB_REPOSITORY:-none}"
[ "${GITHUB_EVENT_NAME:-}" = workflow_dispatch ] || fail "event ${GITHUB_EVENT_NAME:-none}"
[ "${GITHUB_REF:-}" = refs/heads/main ] || fail "ref ${GITHUB_REF:-none}"
case "${GITHUB_WORKFLOW_REF:-}" in
  "$want_repo/.github/workflows/ci.yml@refs/heads/main") ;;
  *) fail "workflow ${GITHUB_WORKFLOW_REF:-none}" ;;
esac
[ "${GITHUB_ACTOR:-}" = StopBanning ] && [ "${GITHUB_TRIGGERING_ACTOR:-}" = StopBanning ] || fail "actor ${GITHUB_ACTOR:-none}/${GITHUB_TRIGGERING_ACTOR:-none}"
echo "mac-runner-guard: ci.yml on main, started by StopBanning: allowed"
