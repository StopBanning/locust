# Project state

Updated 2026-10-15, at 0.778 (the agent list below; the rest was written at
0.619 and still holds). This file says what Locust is and where it
stands. `CHANGELOG.md` is the running record of each build, in the words a
person using the app would read; `docs/ARCHITECTURE.md` says how it is built;
`docs/CROSS_TASK_CONTEXT.md` says how to work on it. Earlier versions of this
file carried the August and September record, and git keeps them.

## What Locust is

A local-first desktop app that runs the AI coding agents a person already
has, as a team they can watch, on the runtimes and models they choose, with an
append-only record of everything the agents did. There is no model of
Locust's own, no telemetry, and no account needed (`docs/NETWORK.md`). An
account is optional: signing in with GitHub can keep a copy of a person's
Locust on Locust's relay, sealed on their computer with a key only their
computers hold, so the relay cannot read it (`docs/PLAN-2026-10-30-ACCOUNTS.md`).
It is open source under MIT (`LICENSE`); the public repository is a mirror
brought up at each release (`_tools/publish-mirror.mjs`), where issues are
welcome and code is by invitation (`CONTRIBUTING.md`).

## Where it is now

Seven AI agents own real conversations end to end: Claude Code, Codex CLI,
Cursor Agent, OpenCode, Copilot CLI, Antigravity and Muse Code
(`apps/desktop/src/shared/runtime-integration.ts` says how far each goes:
five live, Antigravity experimental, Muse Code preview). Since 0.777 a
person can add any other agent that speaks the Agent Client Protocol by its
command line (Settings, Your own agents); it runs in Approve each.

Since 0.619, among much else (CHANGELOG.md has all of it): a cloud computer
(an agent on a GitHub Codespace, 0.761-0.768), benchmarks and judges in
Compare (0.763-0.767), Scout, diagrams and locust:// links (0.770), carry-on
when a plan runs out (0.774), and the phone page on the home network (0.775,
off by default; docs/NETWORK.md). Each is found on the
machine without reading its credentials and offers the models its own CLI
lists. Gemini CLI is found and refused, with the reason said in the app.

Teammates (up to 64) each keep their own runtime, model, effort and mode; up
to 8 conversations run at once. Five modes: Ask, Plan, Accept edits, Approve
each, Auto. Where an agent asks before an action, a card waits, saved rules
answer first, and the record names who decided (`shared/who-decides.ts`,
0.616). Which agents ask, in which modes, is in
`docs/WHAT-LOCUST-CAN-STOP.md`, written from the list Settings shows.

Teams: shares between teammates through the workroom, a relay that lets them
answer each other, rooms, and hand-off chains with a checker. Routines on a
schedule or on a new file in a folder, running in the background if asked,
with "Since you were away" on return and starter routines shipped as files
(`apps/desktop/resources/routines/`). The Board. Compare across models, blind, with a judge. Memory with
recall by meaning on this machine. Your own models through OpenCode, keys
kept by the operating system. Save the record as Markdown that says what it
does and does not hold. Back up and restore from a folder, keys excluded.

The Windows installer updates itself with a differential download and is not
signed yet (`docs/CODE-SIGNING.md`). A Mac build is made in CI as a draft,
signed only ad hoc, not with a Developer ID. Codex CLI and Copilot CLI are kept current by Locust, twelve hours
after a release and only after a canary turn passed; the other agents update
themselves.

## Decisions that hold

- Local first. The agents run on the person's computer, in the folder they
  chose, with the accounts they signed into.
- Locust never reads, copies or pools an agent's credentials, and never runs
  code a model wrote (`docs/DECISION-2026-09-20-LOCUST-NEVER-RUNS-MODEL-CODE.md`,
  amended for a previewed web page in its own frame).
- The ledger is written before the screen, and it is the only evidence. The
  JSON stores beside it are setup, and a backup carries both.
- A mode is given to the agent in the agent's own terms, and the app says
  where an agent cannot be stopped rather than pretending a card.
- Every claim in a changelog entry is a string the shipped installer must
  contain (`_tools/ship.mjs --marker`). Every derived word on screen has a
  test, and the tests have mutation controls.
- Bugs before features. One item per release.

## Decisions that changed

The September design's tool broker, SQLite store, native runtime, OmniRoute
gateway and automatic fallback were not built; `docs/ARCHITECTURE.md` ends
with why. The packages that held the fallback policy and the hand-off state
machine sit under `attic/`, out of the workspace, unimported. The product is
the agents' own gates, the ledger, and the words that match it.

## Run and validate

Requirements: Node.js 22.22 or newer and pnpm 11 (`package.json`).

```powershell
pnpm install
pnpm dev
```

The gate before any commit is `bash _tools/gate.sh` (exit 0 and nothing
else; `docs/CROSS_TASK_CONTEXT.md` says how to read it). `pnpm check` builds,
typechecks and runs every suite. Counts are not written here: a number
written down is a number that disagrees with the run. `pnpm test` is the
truth, and `ls _smoke/*-smoke.mjs` and `ls _tools/drive-*.mjs` list the live
smokes and the drives, which need a signed-in agent and a desktop session and
are run by hand.

## What is open

In the development repository, `docs/PRD-2026-10-04-LOCUST-1.0.md` is the
proposed definition of 1.0 and lists each requirement with where it stands,
and the newest `docs/PLAN-*` file is where the day's work stands; neither is
in the public mirror, which publishes only the documents
`_tools/public-export.mjs` names. The handoff log in
`docs/CROSS_TASK_CONTEXT.md` names the branches waiting for review. Signing,
a Windows CI build, and whether the public mirror becomes the canonical
repository are the owner's to decide.
