# Cross-task context

How to work on Locust, for a person or an agent session arriving cold.
Updated 2026-10-04, at 0.619; the rules still hold at 0.778 (2026-10-15), and
two were added since: a ledger field is fitted, not refused, by
`packages/mission-store/src/fit.ts`, so an adapter need not bound its own
sizes; and the reasoning rule in AGENTS.md now says what is kept and why. Every rule here names the file that holds it;
when the file and this page disagree, the file is right and this page needs
an edit.

## Read first

1. `AGENTS.md` and `CONTRIBUTING.md`: the standing rules.
2. `PROJECT.md`: what Locust is and where it stands.
3. `docs/ARCHITECTURE.md`: how it is built, and what was planned instead.
4. `docs/WHAT-LOCUST-CAN-STOP.md` and `docs/NETWORK.md`: the two documents
   held to the code by tests.
5. `CHANGELOG.md`: what shipped, build by build.
6. The source. The repository is authoritative for what is implemented; a
   document records intent and where things stood when it was written.

In the development repository, the newest `docs/PLAN-*` file and
`docs/PRD-2026-10-04-LOCUST-1.0.md` say what is being worked on. They are not
in the public mirror.

## Where work happens

The repository root, wherever it is checked out. The public copy is a mirror
and has no canonical path; no document names a machine's folder.

**Worktrees.** A session that changes source works in a git worktree of its
own, on a branch of its own (the convention is `exec/<item>`), never in the
checkout a tester is using. Record the branch and the commit it is based on
in the handoff; the main agent reviews and merges. Never assume another
session's uncommitted changes are in your worktree, and never reset, stash
or clean up changes you do not own. Inspect `git status` before editing.

**Who publishes.** One main agent pushes, bumps the version, writes the
changelog entry and publishes. An executor session commits on its branch and
stops: no version bump, no `CHANGELOG.md` edit, no push, no release.

## The gate

```
bash _tools/gate.sh > gate.log 2>&1; echo "gate exit $?"; grep -E "Tests |GATE" gate.log
```

`_tools/gate.sh` runs the type configs and every suite and exits non-zero on
the first failure. Commit on exit 0 and on nothing else. Its own header says
the rest: 75 means it refused to start because the machine was not quiet
(another vitest running, or too little free memory) and nothing was run; a
desktop suite that fails only by timing out is run once more, those files
only, and the gate says so; an assertion failure is never retried; every
failed test is a `GATE:` line with vitest's reason. Scratch files go under
one folder (`LOCUST_GATE_TMP`, by default `.tmp` beside the repository)
because an antivirus scanning thousands of temp folders timed tests out. On
Windows run it from Git Bash; `_tools/gate-selftest.sh` checks the gate
itself after a change to it.

`pnpm check` (build, typecheck, every suite) is the broader run `CONTRIBUTING.md`
asks for. `git diff --check` before a commit.

## Tests

**Named as sentences.** A test file in `apps/desktop/src/main/` is a sentence
that is true of the product: `a-routine-due-while-closed-is-missed.test.ts`,
`the-host-refuses-always-for-a-reaching-command.test.ts`. The sentence is the
claim; the file is the proof. Older files keep module names
(`updates.test.ts`); new ones take a sentence.

**Every test asserts** (`main/tests-assert-something.test.ts` walks every
test body and fails on one with no assertion).

**Controls.** A green suite is not the evidence. Each package has
`test/mutation-control.mjs`, which breaks one behaviour at a time and
requires the NAMED test to fail, rejecting any mutation that stops the file
running; run it from the package folder with `node test/mutation-control.mjs`.
A new derived claim gets a test and a mutation. A smoke carries a negative
control (`_smoke/README.md`); a drive that cannot fail on the build before the
fix is not trusted.

**Documents held by tests.** `docs/WHAT-LOCUST-CAN-STOP.md` is written from
`shared/what-locust-can-stop.ts`; edit the source and regenerate with the
command in the document's first lines. `docs/NETWORK.md` is held by
`main/the-network-is-written-down.test.ts`: a new place in the main process
that reaches the network fails until the document lists it. A saved record
is held to the ledger by the golden file beside
`a-saved-record-says-only-what-the-ledger-holds.test.ts`.

**Source checks.** Sources under `apps/desktop/src` and `packages` are LF
(`sources-are-lf.test.ts`); every CSS class and token used is defined; every
link out is one the host names (`no-dead-links.test.ts`); a failure sentence
says what is still true (`says-what-is-still-true.test.ts`). Read the test
when one of these goes red; each says why it exists.

## Drives, smokes, probes and looks

All under `_tools/` and `_smoke/`, none part of `pnpm check`.

**Drives** (`_tools/drive-*.mjs`, on `_tools/drive-lib.mjs`) launch the built
app on a throwaway profile, do what a person does over CDP, and keep what the
screen showed at each step: text, a screenshot, the renderer's error count.
The judging is done afterwards by reading the record. Each picks its own
debugging port and refuses to start when it is taken. Records go under a
capture folder in `docs/` (`docs/user-session/` by default; `LOCUST_DRIVE_OUT`
moves them elsewhere); the public export
leaves every such folder out by name, and the ship gate's clean-tree check
exempts them, because a capture is evidence about a build and never part of
one. Drives
default to the free OpenCode route (`FREE_ROUTE`; `LOCUST_FREE_MODEL` picks
another free model when it is down); a drive that spends a paid account is
opt-in with `LOCUST_SPEND=1` and refuses otherwise. `--packaged <exe>` runs
one against the installer's build, which is what a release is judged on;
`_tools/sweep-drives.mjs` runs every free packaged drive in turn.

**Smokes** (`_smoke/*-smoke.mjs`) assert, against a real agent or a real
build, and exit non-zero on a failed assertion. `_smoke/README.md` says what
each proves, what it cannot see, and which spend no quota.

**Probes and looks** (`_tools/probe-*.mjs`, `_tools/look-*.mjs`) measure one
thing or render one surface to be looked at. Measure before changing a
hot path; a number from a probe goes in the changelog entry or the plan,
not in a document that will outlive it.

## Shipping

`docs/SHIPPING.md` is the recipe, and `_tools/ship.mjs` is the gate it
describes: typecheck both configs, the unit suite, the version not already on
the releases repository, a changelog entry for it, a clean working tree (only
paths the installer cannot contain are exempt, and each is printed), then
package, then check the installer itself: `app.asar` present, the setup exe
present, `latest.yml` naming the version, every `--marker` string in the
asar, every file beside the asar as staged. `python _tools/bump-version.py
<version> entry.md` moves the version and puts the entry on top of
`CHANGELOG.md`. `_tools/publish-release.mjs` drafts, uploads each file as its
own command, reads the release back, and publishes only when every file is
there; `_tools/promote-release.mjs` marks one build a day as latest for the
tester lane; `_tools/publish-mirror.mjs` brings the public copy up to the
release.

**The changelog** is written for the person using the app, under `### New`,
`### Improved` and `### Fixed`, quoting the words the screen now says. Those
words are the markers. No testers' names.

## The public copy

`_tools/public-export.mjs` builds the mirror from a commit, never from the
working tree. Under `docs/` only the files on its `DOCS_PUBLISHED` list are
published; everything else there (plans, findings, briefs, screenshot
folders, anything named `HANDOFF-*`) stays out, so a new folder of captures
needs no one to remember to exclude it. The export rewrites the owner's
account folder to `<home>` in every text file and refuses to finish if it
remains anywhere; with `--deny <file outside the repository>` it also refuses
if any term from that file (the owner's own details) appears. So in a
published document: no machine paths, no names of people other than public
authors, no account details, no email addresses or phone numbers. The export
is tested by `_tools/public-export.test.mjs`.

## Handoff

Before ending a session that changes the project: re-read the diff and
preserve unrelated work; run the gate; update the document that records what
you changed, if one does; add an entry below if another session must
continue; report exact validation results, and say what was not run.

```markdown
### YYYY-MM-DD — task title (who)

- Scope: branch, base commit, what was and was not touched
- Files changed:
- Outcome:
- Validation: observed results, with what was not run
- Decisions made:
- Known issues:
- Recommended next step:
```

## Handoff log

### 2026-10-21 — A comparison judge reads the work

- Scope: `exec/judge-reads-the-work`, based on `9da0a00d` (0.809.0). No changes to runtime routes, tune,
  dependencies, versions or release files.
- File-changing comparisons hand each judge the same blind answers and per-column diffs, against the comparison's
  recorded git base, including committed, staged, unsaved and new files. Reading the work never stages it. Paths
  are project-relative; copy roots, branch names and comparison ids are not supplied as judge metadata.
- Each column's diff is capped at 6,000 characters with a cut line. Binary and oversized files are named, not
  inlined. The long-brief answer files carry the same bounded evidence. No changes is said explicitly; unavailable
  originals refuse the judge instead of pretending nothing changed. Plain copies have only baseline hashes, so
  their source files must still match those hashes. Shared-folder comparisons explicitly judge answers only.
- The prompt checks task, constraints, claims against changes, consistency, usability and completeness in that
  order. It asks for the decisive point and the runner-up's specific shortfall; the scorecard syntax is unchanged.
  Google Research's Apache-2.0 Harness² is credited for the approach, with Locust's own wording.
- Validation: the gate passed with 1,025 adapters tests (five skipped), 168 mission-store, 32 CLI and 11,684
  desktop tests, plus both type configs and the app build. Fourteen mutation controls were caught. The free
  two-column/one-judge development drive passed 13/13; all seven successful frames and the three failed initial
  navigation frames were inspected. No packaged, paid or live three-judge-panel run was made.

### 2026-10-21 — OpenCode 2 desktop ACP routing

- Scope: `feat/opencode-2-desktop-wiring`, based on upstream `602606e`
  (0.808.0). No dependency, version, changelog, release or credential changes.
- OpenCode 2 discovery checks its ACP help before any V1 probe. The desktop
  selects the existing ACP profile for exact major 2; OpenCode 1 keeps its
  serve/run routes and original guard. Future majors stay unsupported.
  V1 setup warmup and command-list probes are skipped for V2, clearing stale
  command suggestions instead of starting a server that V2 no longer has.
- The model picker opens a bounded, prompt-free ACP session in an empty
  folder and reads its advertised model options, including grouped/free rows.
  It never substitutes V1 hints for a failed V2 list or assigns one model's
  effort options to another. An explicitly saved effort is validated after
  selecting the model using the updated session advertisement.
- Each V2 turn selects a host-owned `locust` mode with native permission
  rules. Approve each uses the existing cards and only sends allow-once;
  unattended modes preserve the existing allow/refuse policy. The separate
  V2 guard is a directory plugin using native tool and shell hooks.
- Ask/Plan disable project configuration and Claude compatibility roots and
  use an empty host-owned config directory, keeping credentials in OpenCode's
  own data store. Per-run plugin policy allows the exact guard directory.
  System/organization and well-known policies remain OpenCode's authority;
  this is native permission enforcement, not an OS sandbox. A model configured
  only in the excluded personal/project config may therefore be unavailable
  in Ask/Plan; a requested model is refused rather than silently replaced.
- Owned V2 sessions continue over ACP. A protocol change or imported session
  is briefed into a fresh session to avoid inheriting outside permissions.
  A side question also uses a freshly briefed session, never the parent's.
  The cloud route remains V1-only and a V2 cloud start is refused before work.
- Validation: focused discovery, catalog, permission, route, cancellation and
  guard tests; dedicated mutation controls; a real OpenCode 2.1.1 prompt-free
  handshake in an isolated home (including registered guard hooks and excluded
  repository/personal plugin fixtures). Final full-gate results and logs are
  recorded in the desktop wiring handoff outside the source checkout.
- Not claimed: a live/paid provider turn, packaged UI drive, older V2 binary
  execution, macOS/Linux execution or V2 cloud support. No message to the owner
  or pull request is part of this task; the requested feature branch is pushed
  after the local gate passes.

### 2026-10-19 — point at a part of the browser page

- Scope: `exec/page-point`, based on `18702215`. The incoming `docs/HANDOFF-page-point.md` and the drive captures in
  `docs/point-at-the-browser-2026-10-19/` stay out of the commit. No version, changelog, installer, site, or push.
- The browser toolbar points at the page in that page's own WebContents. One pick carries the address, a short
  selector, scrubbed HTML, the styles that matter for a visual fix, a picture taken after the outline is gone, and
  a source file only when the page itself names one. Several picks wait as chips, each with a note, until Send.
  Removing a chip removes its picture. The message marks the block as page content. Typed form values are left out.
  A password, payment, or one-time-code field is not pictured. A page that will not run the picker says so on the
  button. Organization policy `browser: false` turns pointing off too.
- Observed: `pnpm typecheck` passed. `_tools/gate.sh` passed. The desktop suite's first run had two timeouts in
  `a-conversation-can-be-pinned.test.ts` (11263 passed); the retry of that file passed 3. The app build passed.
  `node apps/desktop/test/mutation-control.mjs page-point` caught 7 of 7. `_tools/drive-point-at-the-browser.mjs`
  on the dev build at 1200x720, with no model turn, passed every check: the button's chip had a picture, its styles,
  and `src/SaveButton.tsx:18`; the button's own click did not run; the password chip had no value and no picture;
  a note was on the chip; nothing was sent.
- Not run: macOS, Linux, the packaged app, and a paid model turn.
- The browser page shows up on the devtools port at its real address, but that socket's default script context is
  still `about:blank`. The drive evaluates in the context whose origin is `http://127.0.0.1`.
### 2026-10-19 — import OpenCode and Cursor Agent sessions (Codex)

- Scope: `exec/import-more-sessions`, based on `fba73fb8` (0.797.0). No version,
  changelog, installer, site, dependency or global configuration change; no push.
- `main/agent-session-import.ts` reads OpenCode through its documented JSON
  list/export commands. After the person's explicit authorization, Cursor is
  read through chat metadata and read-only SQLite message blobs, never its
  configuration or sign-in. The existing import writer retains session IDs
  and account-default routes; Cursor resumes omit `--model`.
- Import rows name and mark all four supported sources. Missing folders are
  visible and disabled, with a host recheck for the new sources at import time.
  Antigravity and Muse were left out because their help documents no structured
  session listing. Automatic terminal catch-up remains Claude/Codex only.
- Validation and limits: `docs/REPORT-import-more-sessions.md` records the
  exact commands, fixtures, controls, UI evidence and final gate result.
- Not run: a live continuation/model turn, real-user-profile import, packaged
  app or macOS/Linux run. Cursor's private storage format and chat-level
  timestamps are explicit limits. Next: review this committed branch.

### 2026-10-18 — browser teammates can use

- Scope: `exec/agent-browser`, based on `c0a3439b`. The incoming untracked `docs/HANDOFF-agent-browser.md` and
  `.locust/` stay out of the commit. No version, changelog, installer, site or global configuration change; no paid
  model turn or push.
- Main owns the isolated Browser panel (`browser-host.ts`, `browser-url.ts`, `browser-ipc.ts`) in its own Chromium
  partition (`persist:locust-browser`, separate from the person's Chrome profile and cookies) and the loopback MCP
  server (`/browser/call`) paired with `resources/locust-browser-bridge.mjs`. Views are created lazily on first open
  or tool use (`viewCount === 0` at rest).
- Per-run MCP tools (`browser_open`, `browser_read`, `browser_click`, `browser_type`, `browser_press`,
  `browser_select`, `browser_scroll`, `browser_screenshot`, `browser_console`, `browser_network`, `browser_back`,
  `browser_forward`, `browser_tabs`) are passed per run across `claude`, `codex`, `opencode`, `copilot`, and `acp`,
  never written to global agent configs. Runtimes without per-run MCP (`cursor`, `antigravity`, `muse`) display an
  explicit note in the panel.
- URL containment allows `http:`, `https:`, `localhost`/`127.0.0.1`, and `file:` inside the conversation folder
  (resolved with `realpathSync` so symlinks/junctions cannot escape). Ask-first gates (`credential`, `submit`,
  `purchase`, `send`, `download`, `leave-origin`) raise Locust's approval card (`ApprovalChannel`) before acting.
  `browser: false` joins organization policy (`organization-policy.ts`, `docs/ADMIN.md`).
- Observed: `pnpm typecheck`, `agent-browser.test.ts` (including negative mutation control), `_tools/gate.sh`, and
  packaged Windows drive (`_tools/drive-agent-browser.mjs` against `release/win-unpacked/Locust.exe`, 12/12 checks,
  zero model spend) all passed.
- Not run: macOS/Linux packaged runs, live paid cloud model turns.

### 2026-10-18 — terminal beside the conversation

- Scope: `exec/terminal-panel`, based on `226c38ef`. The incoming untracked `HANDOFF-terminal-panel.md` stays out
  of the commit. No version, changelog, installer, site or global configuration change; no model turn or push.
- Main owns personal pseudo-terminals (`terminal-host.ts`), identity-to-folder resolution (`terminal-folder.ts`)
  and window-only validated IPC (`terminal-ipc.ts`). Terminal text never enters a mission or a model. Shells and
  xterm screens remain while the panel is hidden; tab close and app quit end owned process trees.
- `@lydell/node-pty` is pinned to 1.2.0-beta.15, xterm to 6.0.0. Prebuilds ship unpacked, with no dependency rebuild.
  Both CPUs are installed on the build OS for the two Mac images. Public CI loads the packaged native module.
- `terminal: false` joins organization policy. A personal shell also refuses policy restrictions it cannot enforce.
  New tabs resolve only the displayed conversation, selected teammate's known folder/worktree, or current project.
- Observed: repository typecheck and full gate passed; the gate's one existing pinning timeout passed its prescribed
  isolated retry. All 12 controls caught their named failures with 21 terminal tests collected; packaged Windows
  native loading and 14 UI checks passed, with zero captured renderer errors and no captured process left after
  ordinary quit. Focused Home CPU: 0.1% at rest, 0.0% with an idle terminal, rounded to one decimal.
- Not run: Mac/Linux execution or remote CI, other shell variants, native OS clipboard round trips. Packaged
  native-module checks are prepared in the public CI workflow.

### 2026-10-19 — opt-in OpenCode ACP preparation

- Scope: `feat/opencode-2-acp`, independently based on upstream `main`
  at `f9c8657da8510cb364b421ae8adccfaa6317cd3a` (desktop 0.802.0).
  Tool-step replay is already merged upstream as PR #5. This branch adds
  only the separate ACP preparation.
- The package exposes an OpenCode profile for `opencode acp`, ACP version 1
  over stdio. It reuses the shared handshake, session loading, permissions,
  tool updates, usage and event normalizer. The desktop's existing OpenCode
  route remains selected as before; enabling the profile is separate work.
- Only pre-prompt transport failures may restart the owned process: two
  retries by default, capped at three, with bounded exponential backoff and
  one sixty-second startup budget. Prompt admission is marked before write;
  an uncertain prompt or permission response is never resent. Hard agent,
  protocol, authentication and required-setting errors do not retry.
- The optional stdout-close signal preserves older process implementations.
  Shared ACP runs settle on EOF/error, retain a last-chunk final reply, and
  cannot continue a stopped handshake. Finish cancels permission waits and
  clears abort listeners and timers. A failed native tree-release promise
  falls back to ending the child.
- The profile waits at most one second by default for parent-process exit
  before replacing it or reporting cleanup. Missing acknowledgement is
  recorded as unconfirmed and prevents replacement. This does not prove
  arbitrary descendant termination: the native tree primitive remains best
  effort, particularly POSIX children that ignore SIGTERM.
- Explicit profile configuration must be advertised and accepted; legacy
  ACP callers retain their existing optional-setting behavior. Runtime
  identity remains OpenCode throughout, without an automatic route switch,
  new telemetry, credential access, dependencies, version or changelog edit.
- Both the live record queue and pre-prompt receipt staging are bounded;
  overflow stops the run without replacing the process or leaking abandoned
  startup receipts into the ledger.
- Controlled fake-agent tests, dedicated mutation controls and the local
  gate are recorded in the accompanying ACP handoff. Live OpenCode 2 turns,
  packaged UI, macOS/Linux execution and arbitrary descendant cleanup are
  not claimed by that validation. Desktop routing needs its own integration.

### 2026-10-18 — machine-wide organization policy

- Scope: `exec/org-policy`, based on `c3174add`. No version, changelog,
  dependency, installer, site or global configuration change; no push.
- Decision: the main process reads an administrator-owned `policy.json` once
  at launch. No file preserves personal settings; an unreadable or invalid
  policy blocks starts and connections and is shown in the window. Packaged
  builds ignore `LOCUST_POLICY_FILE`; Windows resolves the OS known folder,
  rather than trusting a process's `ProgramData` override.
- `shared/organization-policy.ts` holds the rules, separately from Electron
  and React. Both mission services enforce them before discovery; handoff
  preflight checks before stopping the old run. Separate background, cloud,
  voice and terminal paths are guarded too. MCP and phone hosts check before
  saved opt-ins can open listeners. Renderer controls read the same snapshot.
- Effective monthly limits are the lower of personal and organization limits,
  overlaid without rewriting personal preferences. Unowned work uses the
  whole month's dollar receipts. An unreadable spending record blocks a
  managed start. Unmetered launches are refused when they cannot enforce the
  cap; unknown model defaults are refused under a finite model allowlist.
- `docs/ADMIN.md` records paths, schema, deployment and the limits of receipt
  accounting, runtime capabilities and launches outside Locust.
- Validation: pending the final gate; the focused policy and regression runs
  have passed. Final command results are recorded before this branch commits.
- Not run: live or paid agent turns, real policy deployment, packaged UI,
  macOS or Linux execution. No subscription credentials were inspected.

### 2026-10-19 — OpenCode chronological tool-step recovery

- Scope: `feat/opencode-tool-steps-replay`, based on upstream `main` at
  `e90b079611efe68372924b3c77847980622b5373` (desktop 0.789.0, including PR #4).
- EOF recovery reuses `checkBegun`, collects all assistant messages tied to
  the streamed prompt by `parentID`, and validates the latest message's stop
  before replay. Messages follow creation time then ID; OpenCode parts follow
  ID order. Imported parts use available timestamps within step boundaries,
  retaining response order where ordering evidence is absent.
- Replay uses the live `accept` path, so tools, failures, text and step
  token/cost receipts are recorded once. ID-less parts use a full stable
  payload hash, replacing the colliding 80-character fallback. History and
  foreign-session/message parts remain excluded.
- Status polling recognizes only `idle`, `busy` and `retry`. The existing
  ten-second request bound, five-minute recovery bound, cancellation and
  process-exit handling remain in force. No prompt replay or automatic retry.
- The full gate exposed an upstream scanner false positive in the release
  formatter's raw-backtick regex. A one-line equivalent `\x60` escape fixes
  it without exempting the file; all seven formatter tests pass unchanged.
- Validation: focused recovery suite 67/67 with no skips; workspace
  typechecks and package builds passed; all 108 runtime-adapter mutation
  controls caught (95 existing, 13 new), none skipped or invalid. Full gate
  exit 0: 956 adapter tests passed (five existing optional skips), 168 store,
  32 CLI and 11,068 desktop tests passed; all application bundles built.
  The gate used scratch outside Git, two Vitest workers, CI timeouts and a
  process-local en-US locale preload. Reproductions, logs and limits are in
  `PR_TOOL_STEPS_REPLAY.md` outside the source tree.
- No version or changelog change, paid/live provider turn, push or PR.

### 2026-10-18 — OpenCode multi-turn recovery

- Scope: `feat/opencode-multiturn-recovery`, based on upstream `main` at
  `367b3f50a204a037d223ff5feb0fdb4b178b594f` (desktop 0.787.0).
- Reuses `checkBegun` for EOF recovery. The first new user message streamed
  after sending the prompt supplies its ID; a recovered assistant answer must
  name that ID as its `parentID`. This permits resumed turns and replies whose
  assistant event was lost, while rejecting old answers and other sessions.
  A fresh session without that receipt retains the observed-reply fallback.
- Completion still requires the same session, an assistant without an error,
  a finite completed timestamp, and matching terminal stop evidence. Recovered
  messages and parts use the existing handler and output deduplication.
- Busy, working and retry statuses keep polling beyond ten seconds, within
  a fixed five-minute recovery ceiling from EOF. Shared status/message checks,
  including their bodies, have separate ten-second abort deadlines. Finish,
  cancellation and server exit clear the timers and abort outstanding requests.
- Adds 19 cases to the existing fake-server suite (55 total), ten mutation
  controls, and updated anchors for two existing controls. An incomplete
  mutation baseline now fails instead of being reported green.
- Validation: all 20 OpenCode controls caught their named regressions with
  949 tests collected per run, then source was restored. Full Git Bash gate
  exited 0: adapters 944 passed / 5 existing skips, mission-store 168 passed,
  desktop 10,988 passed; both desktop type configs, package builds and app
  bundles passed. The extended fake-server test file also passes strict
  TypeScript checking. Native locale, existing `CI=1` timeout setting, and
  scratch outside Git; no timeout-only retry was needed. Earlier setup runs
  were interrupted for dependency-cache and scratch-location issues; they
  are not counted as successful gates. No new tests are skipped.
- No prompt or command replay, version, changelog, dependency, permission,
  provider route, installer, push or publication changes. No live model turn
  or packaged UI drive. Next: review the focused feature branch.

### 2026-10-18 — OpenCode premature stream close (MeetGodhani)

- Scope: `fix/opencode-stream-hang`, based on upstream `3c05f54`, including
  0.784.0 and 0.785.0. Adapter, existing watchdog test, mutation controls and this handoff
  only; no version, dependency, lockfile or changelog change.
- Outcome: EOF before `session.idle` calls the existing `checkBegun` status
  and message check, including an in-flight watchdog query. Busy/retry states
  poll every 250 ms within a 10-second recovery budget. Confirmed terminal
  output from a fresh session uses the existing part handler; uncertain
  completion, unreadable responses, process exit or deadline expiry fails
  cleanly. Resumed history alone cannot establish this turn's completion.
- Teardown flushes paced text and aborts pending requests. A late session
  response cannot send a prompt after failure; cancellation has its own
  deadline even when the server never answers abort. No prompt or command
  is replayed, and no event-stream reconnect or permission change is added.
- Tests: 33 added cases alongside the existing three watchdog tests, using
  their shared fake server; all 36 pass. Ten new mutation controls each fail
  their named assertion with all 930 adapter tests collected, then restore
  source. This includes the original hang, stale output, cancellation and
  overlapping request/timer regressions. The other mutation controls were
  not individually rerun.
- Verification: full `_tools/gate.sh` passed (exit 0) on the 0.785 base
  with the test environment below: adapters 925 passed / 5 skipped,
  mission-store 168 passed, desktop 10,967 passed. Package and desktop bundle
  builds and both desktop TypeScript configs passed. The adapter count
  includes 36 compiled copies of source tests from the normal package build.
  Strict checking of the extended test file, mutation-script syntax and
  `git diff --check` passed.
- Test environment: the first native en-IN desktop run had four date-format
  assertion failures and three timeouts. The four locale failures reproduce
  with the upstream adapter restored. All seven files pass (251 tests) with
  a process-local en-US Date/Intl test preload and the existing `CI=1`
  timeout configuration; this does not change OS settings or product source.
- Limits: controlled fake-server and captured-part tests, not a live provider
  turn, packaged UI, process-tree termination drive, macOS test or release.
  Next: review the focused patch for Issue #2 and run ordinary upstream CI.
- Merged: squashed as `8b8fc884` after the PR's CI (tests and public guard)
  passed and a local rerun on main (the 36 tests, the adapter suite and all
  ten mutation controls), for 0.786.

### 2026-10-17 — MCP follow-up startup claim (MeetGodhani)

- Scope: `fix/mcp-async-handling`, based on `b510ad94` (0.783.0).
  No version, dependency, lockfile, changelog, retry or permission changes.
- Fixed: `send_message` now awaits `started(...)` inside its existing
  `try/finally`, retaining the conversation claim through saved-mode lookup
  and asynchronous startup. Concurrent sends and polls keep the pending-start
  response; success and rejection both release the claim.
- Tests: three deferred-start regressions cover pending startup and polling,
  pending saved-mode lookup, and cleanup after rejection. Two `mcp-startup`
  mutation controls require named assertion failures when the await or cleanup
  is removed.
- Validation: MCP tools, host-token and stdio-bridge suites passed 48/48 with the normal
  Vitest config. Focused mutation verification collected 27 tests each time:
  baseline/restored all pass; removing await fails three assertions; removing
  cleanup fails two. The repository mutation runner caught both controls.
  Both package builds and both desktop TypeScript checks passed. The full
  gate exited 1 before its suites: the pinned recall-model download ended
  with UND_ERR_SOCKET (other side closed). No full-suite pass is claimed;
  full-suite validation remains pending in CI.
- Limits: controlled offline callbacks, not live agent runs or a performance
  benchmark. The mission service has another conversation claim; duplicate
  real runtime launches were not established. Bug 2 is reported separately; no
  OpenCode source change is included in this branch.
- Merged: squashed as `a1003a71` after the PR's CI (tests and public guard)
  passed, for 0.784.

### 2026-10-07 — opt-in MCP access from other AI apps (Codex)

- Scope: `exec/mcp-server`, source commit `5fccf37c`, from desktop 0.689. No version, changelog, dependency,
  installer, client-config or push changes. All `docs/` changes remain uncommitted
  as requested; the network inventory update must accompany integration.
- Built: a separate account-token-authenticated listener on `127.0.0.1`, off by
  default, and a dependency-free stdio bridge shipped beside the app. Settings >
  General shows setup text for Claude Code and Codex, never writes their configs,
  and never sends the token to the renderer. Off closes the listener and revokes
  the token; re-enabling rotates it. Windows account-only ACL is established on
  the empty temporary file before any token bytes are written. Profile backup
  excludes the connection file and opt-in.
- Tools reuse the window's roster, mission start and ledger readers. Starts and
  continuations always use Ask, preserve the saved teammate route, and return
  monthly-limit refusals unchanged. Send/read accept MCP-origin conversations
  only; a busy turn is reported, never queued. Ledger schema 23 preserves the
  other-app origin across restarts, with a visible thread marker.
- Decision from the person: no automatic handoffs from MCP turns. Shares remain
  posted in the workroom, but the relay never starts, defers, steers or interrupts
  another teammate because of such a share. The thread records why no automatic
  reply was started. Ordinary person-origin relay behavior is unchanged.
- Validation: 37 tests in five new MCP files, five additional cases in existing
  mission/relay files, and all selected regression/source checks passed. Final
  unique-file total: 1,285 tests in 21 single-file runs (including the 879-test
  assertion guard). Runtime-adapters and mission-store builds, desktop build,
  both desktop type configs, bridge syntax and diff whitespace checks passed.
  Four temporary mutations failed their named tests, then were restored and
  their full files rerun green: early body reading, non-loopback bind, Auto
  starts, and automatic MCP handoffs. Stdio tests launch real Node and Electron
  processes against the real authenticated listener, with fake mission actions.
- Not verified: a real Claude Code/Codex/Cursor client, a paid or live model turn,
  a live desktop UI drive, installer packaging, another Windows user's access,
  or macOS. The full gate was not run because this handoff requests single-file
  suites on the shared machine. Existing renderer chunk-size warnings remain.

Entries since 2026-10-03, newest first. Earlier entries (2026-08-30 to
2026-09-09) are in this file's history in git.

### 2026-10-04 — the docs say what is built (Fable)

- Scope: `exec/docs-truth`, based on `3beebae9` (0.619). Documents only:
  `docs/ARCHITECTURE.md`, `PROJECT.md`, `docs/CROSS_TASK_CONTEXT.md`
  rewritten as the built product. No code, version, changelog, push or
  publication.
- Verified by grep on this build: nothing under `apps/`, `packages/` or
  `_tools/` imports `@teammate/contracts` or `@teammate/runtime-core`; the
  only mentions are inside `attic/`. The handoff asked for this check against
  `packages/`; the two had already been moved to `attic/` at 0.589.
- Validation: `_tools/public-export.test.mjs` and `_tools/gate.sh`, results in
  the session's report.
- Recommended next step: review and merge; the PRD's R28 row ("the docs
  describe the built product") can then move.

### 2026-10-03 — W11 Remote Control Settings switch (Casper)

- Scope: `exec/w11-casper`, a fresh worktree based on `origin/main` at
  `edb05f22141f253d461cf4e73681b1fd28af8d7d`. No version or CHANGELOG change,
  packaging, push or publication.
- Built: an off-by-default Settings > AI agents switch; host-owned `claude
  remote-control --spawn worktree --name Locust` in the current project
  folder; boolean-only validated IPC; first stdout/stderr shown verbatim,
  separately capped at 64 Ki characters with a visible truncation notice. No
  persistence, automatic retry or stdin answers.
- Lifecycle: off/quit kill only the owned process tree and await close.
  Dispose latches against later starts; a pending discovery is invalidated by
  off or quit. A termination deadline reports failure rather than claiming
  the process ended.
- Trust/setup: trust prompts, `Workspace not trusted`, and the one-time enable
  prompt stop the server before opening normal interactive Claude Code for the
  person. The person completes setup and explicitly enables the switch again.
  Windows/macOS terminal launch is unit-tested with mocks; other platforms
  receive manual guidance.
- Observed: 40/40 new unit tests in four files passed with fake processes.
  Four deliberate regressions each failed the named assertion, then were
  restored. Full `_tools/gate.sh` via Git Bash passed, exit 0. `git diff
  --check` passed. Windows' default `bash` selected an unavailable WSL
  distribution; Git Bash ran the gate, with `LOCUST_GATE_TMP` set to a scratch
  folder of its own.
- Not run: real `claude remote-control`, account/sign-in checks, trust or
  enable answers, live UI, packaged UI, or a remote phone session. Review and
  merge before any release; W11 is built, not live.

### 2026-10-03 — comparison and drive polish (Codex)

- Ready for review: `exec/polish-batch`, based on `06e4a6c9`. Full file list,
  evidence paths and limitations are in `docs/HANDOFF-2026-10-03-POLISH.md`
  (development repository only).
- Four requested fixes: full comparison route tooltip (owned by
  Composer.tsx), styled judge select, stdout route probe, and a seeded Copy
  assertion with `--copy-only` mode.
- Gate exit 0; runtime-core 4/4 separately. Five new tests pass and all four
  requested regressions fail their named tests. Built-app drives: comparison
  8/8, seeded Copy 1/1, zero captured renderer errors.
- No version/CHANGELOG change, paid turn, packaging, push or publication.
  Live model round trips and packaged checks remain unverified.

### 2026-10-03 — research 3.2 real Codex fixtures and schema drift (Casper)

- Scope: `exec/3-2-casper`, fresh from `origin/main` at
  `06e4a6c9aa648ab1d391ef7a95ca096aa36a9592`. No production adapter change,
  push, publication, packaging, version bump or CHANGELOG edit.
- Observed live: installed Codex 0.160.0. Three retained recordings in
  `packages/runtime-adapters/test/fixtures/codex`: one approval declined, a
  file changed with host read-back, and an accepted mid-command steer. Paths,
  ids, machine metadata and timestamps scrubbed before persistence. The
  recorder is `_tools/record-codex-app-server-fixtures.mjs`, gated by
  `LOCUST_SPEND=1`.
- Observed tests: 9 new offline tests passed, one installed-CLI schema test
  skipped by default. Four negative controls failed their named assertions,
  then were restored. Final gate exit 0. `git diff --check` passed.
- Observed opt-in check: `LOCUST_CODEX_SCHEMA=1` generated the installed
  0.160.0 schema without a model call; the one installed-CLI test failed for
  exactly `ConfigWarningNotification.message` (the notification now carries
  `summary`). Fixed in the next entry.
- Not run: an exhausted provider limit, live configWarning, other Codex
  versions, packaged UI, or a release.

### 2026-10-03 — Codex app-server config warning fields (Gemini Flash)

- Scope: `exec/codex-warning`. Files: `packages/runtime-adapters/src/app-server-events.ts`,
  `packages/runtime-adapters/test/codex-schema-read-fields.ts`,
  `packages/runtime-adapters/test/a-config-warning-says-its-summary.test.ts`,
  this document.
- Outcome: `configWarning` notifications were dropped when `message` was
  missing. The normalizer now reads `params.summary` with optional
  `params.details` appended, falling back to `params.message`, and the
  read-fields inventory tracks `['summary', 'details']` so a later removal is
  caught.
- Validation: the new test failed 2 assertions on the un-fixed code, then
  passed 6/6; the opt-in schema test passed 5/5 against installed Codex CLI
  0.160.0; gate exit 0; `git diff --check` clean.
- Not run: a live session with a broken config making app-server push a
  `configWarning`; no model calls.
- Recommended next step: review and merge `exec/codex-warning` into main.

## First-launch teammate faces (2026-10-05)

- Branch: `exec/first-launch-blocked`. In renderer `status.ts`,
  `runtimeReach` leaves the global unusable verdict unknown while any check
  is pending, and `teammateStatusView` does not block on a pending own runtime.
  Checked missing or signed-out runtimes still block; this does not make a
  pending runtime usable for sending work.
- Regression tests: `a-teammate-is-not-stuck-while-agents-are-still-being-found.test.ts`.
  `_tools/control-first-launch-blocked.mjs` checks four deliberate regressions.
- `_tools/drive-pet-puppets.mjs --startup-only` records fresh-start faces and
  held discovery answers before `ready()`; `--observe-startup` also runs the
  full puppet checks. Neither sends a turn. The normal drive is unchanged
  unless an observation option is supplied.
- Evidence and verification limits: `docs/REPORT-2026-10-05-first-launch-blocked.md`.

## Conversation references (2026-10-19)

- `main/conversation-references.ts` holds host-picked identities and conversation roots in the profile's
  `conversation-references.json`, separately from teammates. Every normal turn uses the current selection,
  including resumed turns; an empty selection explicitly withdraws earlier references from the brief.
- A reference must never narrow the chosen mode's tools. Auto keeps its existing access and the chip warns
  that it can change the reference. Codex gets a brief, never `--add-dir` (which would widen writes).
  Claude Code, OpenCode, Copilot and Antigravity add outside folders only in their existing read-only modes;
  in other confined modes the chip explains why that extra grant is omitted. Other runtimes get a brief only,
  with a chip saying their existing permissions still apply. Local paths are not sent to a cloud computer.
- GitHub references are shallow host copies under `reference-repositories`, capped at 500 MiB and refreshed
  after an hour when reopened, never during an active turn. Replaced or released copies move to
  `reference-trash`; chosen local folders are never moved. Soft-deleted conversations retain references for
  restore; permanently emptying the conversation trash releases them. These machine-local records and copies
  are excluded from profile backups.
- `_tools/drive-conversation-references.mjs` uses one free OpenCode Ask turn and a separate reference folder.
  Its `holdCursor: false` option avoids reading or changing Cursor's global defaults.

## Instruction trials for a teammate

- `/tune` in a finished teammate conversation authorizes one side turn to suggest two edits: procedure and
  domain knowledge, each tied to a moment in that conversation. The card discloses that turn. Invalid output or
  an unsupported side conversation leaves editable copies of Current. No three-way comparison starts until
  Run all three is pressed, and no saved teammate instructions change when a column is kept.
- `CompareRoute.instructions` is a bounded, per-column replacement; an empty string explicitly removes the
  teammate's own instructions for that column. It travels through the run slot into `ConversationHint`, not a
  mutable global or a compare-store lookup from the brief builder. Follow-ups, retries, restarts and reruns keep
  the column's snapshot. Ordinary duplicate-model picker rules remain; explicit instruction trials may share a
  model, including copies of Current when an angle recommends no edit. Blind letters are shuffled in main.
  Instruction trials do not count as model-against-model votes in the model records.
- The kept output offers Review instructions, Use these instructions for the teammate, and Not now. Only the
  explicit Use button calls the existing teammate update path; empty clears, and the change briefs future turns.
  `_tools/drive-tune-a-teammate.mjs` uses a free route and does not hold or change Cursor's global default.


## GitHub comment asks (2026-10-22)

- Scout's optional comment watch uses only the signed-in person's newly created `/locust` issue and pull request
  conversation comments. It has no timer of its own. `main/scout-comments.ts` reads through the installed GitHub CLI;
  activation time, repository cursors, remembered comment ids and pending answer mappings live in Scout's own file.
- Named teammates are matched without a model. Scout may route an unnamed ask, but cannot replace its deterministic
  draft or turn the issue's other people's words into instructions. Successful person-origin starts bind the card to
  the new turn in the host; terminal answers reconcile after a fast completion or restart.
- Posting is a separate explicit card press. Only the saved, displayed body goes to GitHub over stdin; the durable
  attempt marker must be written before the call. Neither errors nor uncertain results are retried, even on restart.
- `_tools/drive-locust-in-a-github-comment.mjs` uses a free model and a fake `gh.exe` ahead of PATH. It was written for
  the owner to run; this task did not launch Locust or run any drive. The scoped controls are
  `_tools/control-locust-comments.mjs`.
