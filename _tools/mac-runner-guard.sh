#!/bin/bash
# mac-runner-guard.sh (2026-11-02): the job-started hook of the GitHub runner on Colin's Mac mini
# (ACTIONS_RUNNER_HOOK_JOB_STARTED in ~/work/gh-runner/.env). The repository is public, and a pull request can change
# a workflow's runs-on to this Mac, so the runner itself refuses every job but two, both by the owner of
# StopBanning/locust: ci.yml started by hand on main, and native.yml on exec/native. A refused job fails before its
# first step. Copied beside the runner by _tools/mac-runner-setup.sh; the copy there is the one that runs.
want_repo=StopBanning/locust
fail() { echo "mac-runner-guard: refused: $1"; exit 1; }
[ "${GITHUB_REPOSITORY:-}" = "$want_repo" ] || fail "repository ${GITHUB_REPOSITORY:-none}"
[ "${GITHUB_ACTOR:-}" = StopBanning ] && [ "${GITHUB_TRIGGERING_ACTOR:-}" = StopBanning ] || fail "actor ${GITHUB_ACTOR:-none}/${GITHUB_TRIGGERING_ACTOR:-none}"
case "${GITHUB_WORKFLOW_REF:-}|${GITHUB_REF:-}|${GITHUB_EVENT_NAME:-}" in
  # The release build: ci.yml, started by hand on main.
  "$want_repo/.github/workflows/ci.yml@refs/heads/main|refs/heads/main|workflow_dispatch")
    echo "mac-runner-guard: ci.yml on main, started by StopBanning: allowed" ;;
  # The Rust port's tests (2026-11-11, Colin's go-ahead after GitHub's Mac runners gave the job none three times):
  # native.yml on exec/native, pushed or started by the owner. Only someone with push access can make either event
  # on that branch; a pull request's event and ref match neither line.
  "$want_repo/.github/workflows/native.yml@refs/heads/exec/native|refs/heads/exec/native|push" | \
  "$want_repo/.github/workflows/native.yml@refs/heads/exec/native|refs/heads/exec/native|workflow_dispatch")
    echo "mac-runner-guard: native.yml on exec/native, by StopBanning: allowed" ;;
  *) fail "workflow ${GITHUB_WORKFLOW_REF:-none}, ref ${GITHUB_REF:-none}, event ${GITHUB_EVENT_NAME:-none}" ;;
esac
