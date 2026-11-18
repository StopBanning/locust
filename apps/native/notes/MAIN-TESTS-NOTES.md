# Electron main-process tests carried into Rust

Lane: `~/work/locust-wt/maxlane`, branch `exec/claude-tests`, cut from native-engine at 6cd55444. Each batch takes about
five `apps/desktop/src/main/*.test.ts` files. For each case whose behaviour Rust has, there is one Rust test with
Electron's values and words, and a break check where the named test must fail. Rows are in `parity/tests.tsv`.

Break checks ran through `~/work/scratch/maxlane/mutall.py`. It is a copy of `~/work/scratch/mut6.py` that prints
every failing test name, not just the first three. That way "caught by the named test" can be read off the output.

## Batch 1: handoff, mode, handover note, route at start, remembered effort

| File | Row | Rust tests |
| --- | --- | --- |
| `handoff.test.ts` | carried (8/8) | `core/src/handoff_brief_tests.rs` |
| `a-handoff-keeps-its-mode.test.ts` | partial (1/3) | `core/src/mission_mode_tests.rs` |
| `a-handoff-without-a-note-says-so.test.ts` | carried (4/4) | `core/src/handover_note_tests.rs` |
| `route-at-start.test.ts` | partial (16/17) | `core/src/route_at_start_tests.rs` |
| `route-remembers-effort.test.ts` | waiting | none |

Bugs found and fixed:
- **The brief had no `kept`.** Electron's `composeHandoffPrompt` returns the sections it carried, in reading order and
  without the person's next words. The preview under the box reads that list. `carry_on::Briefing` now has `kept`.
- **The saved route dropped the request's account.** When a start matched the teammate's saved route, Rust took the
  saved route's account even when that was none. Electron sets the saved account only when there is one, so the
  request's own account stays. Fixed in `route_at_start`.

Found and not fixed (outside these files):
- `compose_handoff_prompt_with` leaves out the sections the person chose to drop, but it does not add Electron's
  `choiceNotice` line, "(The person chose to leave out ...)". Electron also takes that line's length out of the
  budget. This belongs to `a-person-can-leave-out-part-of-a-brief.test.ts`.

Why some rows are not carried:
- Mode: Electron's `parsedMissionMode` reads a missing or malformed mode as Ask. Rust has no lenient reader. Its
  request paths take a typed mode, and the door refuses one it does not know. The third case checks Electron's
  `index.ts` source text, so it has no Rust counterpart.
- Route at start: "returns no permission mode" holds by type, because `StartRoute` has no mode field. No test was
  written, so the row is partial.
- Remembered effort: Rust never records the route a start used. There is no `rememberRoute`, and a teammate's route
  changes only through the people panel. The teammate-store reader (`native::route_of`) also leaves effort out of the
  `Route` it builds. The window reads effort straight from the JSON instead (`native.rs` job.effort).

Checked for M26 and found to match:
- A carried-on turn reads the stopped turn's recorded mode as it is. Electron does this (`carry-on.ts` `modeOf`, sent
  straight to `codexMissions.start`, without `parsedMissionMode`), and so does Rust (`native::carry`). An earlier
  version of this note said they differed. That came from reading only the IPC payload path, and it was wrong.
- In Rust, a mode `Mode::parse` does not know becomes None, and the turn falls back to the teammate's saved mode.
  This applies to a carried turn and to a relay request (`native.rs`). Whether Electron's relay start does the same
  was not checked. The engine wire (`engine_process::origin_from`) and the door refuse an unknown mode.

Break checks (all caught by the named test): `kept` including next; a blank next counted as an instruction; next
not charged against the budget (refuses); newest-that-fit taking the oldest; the clip notice's words; stopped partway
with work in flight; the clean-finish wording; the request's account; the override for another teammate; the saved
effort; `modelChoice` ignored; the conversation's own route; the pick key; Kilo's Auto Free; an explicit pick
ignored; a share to anyone counted as a note; any share counted as a note to the owner; the no-note row's words; the
board brief's words; `plan` read as Ask.
A follow-up made one break for each of the 9 tests the first round never saw fail. All 9 were caught: next words
moved up front; the picker override ignored (both override tests); for nobody, the request's effort dropped; a
seeded account default promoted to a pick; the default runtime; the saved identity's effort change; a new chat not
on the saved model; home's pick key.

## Batch 2: model lists, free routes, models of your own

| File | Row | Rust tests |
| --- | --- | --- |
| `a-model-list-missing-one-is-asked-again.test.ts` | carried (2/2) | `core/src/model_lists_tests.rs` |
| `free-routes.test.ts` | partial (3/6) | `core/src/model_lists_tests.rs` |
| `a-model-of-your-own-is-unpriced-not-free.test.ts` | partial (2/3) | `core/src/model_lists_tests.rs` |
| `a-model-of-your-own-is-priced-as-listed.test.ts` | waiting | none |

Bug found and fixed:
- **An incomplete model list was kept for ten minutes.** This is Colin's 2026-10-02 report: "only account default is
  popping up for opencode". `models::local` kept whatever the sweep returned. Now `models::kept_catalog` keeps a
  catalog only when `lists_its_models_but_listed_none` is false. That function is Electron's rule: an OpenCode,
  Cursor or Claude Code that is ready and listed no models (for Claude Code, no aliases either). `read_listed_checked`
  counts an agent as ready when it is installed. An OpenCode 2 is not counted, because its list comes from an ACP
  session this sweep does not read. Electron's other branch (Antigravity readiness `unknown`, a `check-pending`
  diagnostic) is not carried, because Rust's sweep has no such states.

Not built, so not carried:
- `freeRoutesOnly`: holding a launch with `--remote-debugging-port`, `LOCUST_SPEND` or `LOCUST_FREE_ONLY` to free routes.
- `spendByTeammate` counting `unpricedRuns`: `spend::spent_by` adds up dollars only.
- Listed prices (`listed-prices.ts`): the OpenCode `models.json` price list, and own models told their price.

How "ready" is decided: Rust has no discovery readiness here, so `read_listed_checked` counts an agent as ready when
it is installed (`find_named`). Two edges differ from Electron's `readiness === 'ready'`:
- An installed but signed-out Cursor or Claude Code that lists nothing keeps the catalog from being kept. Every
  `models::local()` then reads again. That costs one sweep per call, but `local()` runs only for a Cursor start
  (`native.rs`) and the door's model list. It does not run on a poll.
- An `opencode --version` that times out counts as "not a 1.x". Its empty list is then kept.

`model-catalog.test.ts` (33 cases) was left for a batch of its own.

Break checks (all caught by the named test): an incomplete catalog kept; Copilot counted as listing models; OpenCode's
free rule loosened to any "free"; Kilo's `:free` suffix dropped; an own model on any runtime; unpriced runs counted
toward the limit. No break was made for the OpenCode 1.18.27 list on its own. Its six ids go through the same rule
as the first free-route case.

## Batch 3: worktrees, an npm lock, git without a shell

| File | Row | Rust tests |
| --- | --- | --- |
| `a-half-made-worktree-is-finished-not-trusted.test.ts` | partial (2/3) | `core/src/worktree_checkout_tests.rs` |
| `a-worktree-survives-only-a-finished-checkout.test.ts` | partial (6 tests) | `core/src/worktree_checkout_tests.rs` |
| `a-locked-file-is-not-a-permission-problem.test.ts` | carried (3/3) | `core/src/install_lock_tests.rs` |
| `a-planted-git-is-never-run.test.ts` | waiting | none |
| `git-reaches-a-run-with-no-shell.test.ts` | waiting | none |

No bugs were found. The worktree tests use real git and real `post-checkout` hooks with Electron's hook scripts, and
Rust behaved as Electron does in every case written.

Not carried, and why:
- Electron's worktree manager takes an injected `runGit` and a diagnostic `note` callback. Rust's `Manager` has
  neither. So these four cases have no Rust test: "no git at all for a finished tree", "an unreadable HEAD after a
  nonzero add", "a timeout is not a finished hook failure", and "a failing note callback". Neither does the note
  assertion in the first hook case. Rust drops the hook's stderr when it accepts the tree.
- `NoDefaultCurrentDirectoryInExePath` is Windows-only, and Rust does not set it.
- The `locust-git-reads` server (git for Ask and Plan runs with no shell) is not built.

Break checks, run through `~/work/scratch/maxlane/mutsafe.py` (all caught by the named test): `index.lock` left;
`reset --hard` instead of `reset`; an admin folder outside the repository accepted; the recorded branch not checked;
an `initializing` lock not checked; a usable tree refused; any failed hook accepted; the add writing
`core.longpaths`; the longpaths flag off Windows; a lock read off Windows; EPERM-on-rename not a lock; EACCES not a
folder problem.

**Why `mutsafe.py`.** One mut6 run was cut off by its outer `timeout 590` while another lane held the build lock.
That killed the Python but not its cargo child, and the break was left applied in `worktrees.rs`. I found it at
once, stopped my own orphaned run (the `sol` lane's suite was left alone) and checked the file out clean.
`mutsafe.py` restores the file on SIGTERM and SIGINT, and runs cargo in its own process group. It kills that group
at its own limit, so it needs no outer `timeout`.
`worktree_checkout_tests` is registered only for Unix (`cfg(all(test, unix))`), because it makes hooks executable
with `PermissionsExt`. `worktrees_tests.rs` does the same without the guard, so CI's Windows `cargo check
--all-targets` already stops there.
Two tests outside these files failed once while a break was applied, and passed in every full run:
`native::tests::relay_roundtrip_starts_recipient_records_peer_links_remembers_hub_and_returns_to_origin_thread` (batch
1, with the room_store note break) and
`door::tests::the_door_is_off_until_turned_on_and_answers_only_its_current_token`, with the longpaths break applied.

## Round 2 (16 Nov 2026, afternoon)

Branch moved up to 9b886986. Break checks ran through `~/work/scratch/maxlane/mutfilter.py`, a copy of `mutsafe.py`
that runs only a nextest filter (the batch's tests) instead of all of `locust-core`. A full-package run took 5 minutes
a break; a filtered one takes the build and a few seconds. It restores the file on SIGTERM/SIGINT and kills its own
cargo group, as `mutsafe.py` does. A `TIMEOUT` or `(build error)` was never counted as caught: each was run again,
or the break was rewritten, until the named test failed.

## Batch 4: mission history, a turn's receipt on its own branch

Commits be71cd42, a75d9d2e.

| File | Row | Rust tests |
| --- | --- | --- |
| `mission-history.test.ts` | partial (17/30) | `core/src/mission_history_tests.rs` |
| `a-turn-on-its-own-branch-leaves-a-receipt.test.ts` | carried (8/8) | `core/src/native/tests.rs`, `core/src/worktrees_tests.rs` |
| `missions-can-be-deleted-together.test.ts` | waiting | none |
| `the-current-turn-is-keyed-by-its-mission.test.ts` | waiting | none |
| `torn-ledger.test.ts` | waiting | none |

Bug found and fixed:
- **A turn with a slot of its own could commit to a teammate's branch.** Rust skipped the turn-end commit only for a
  side question, or a slot with a `cwd` of its own. A compare column with no copy, or the scout chat (its slot never
  has a `cwd`), ran in the teammate's copy and was committed there, with that ask as the subject. Electron commits
  only when `mission.slot === undefined`. `native.rs` now checks `!is_slot` too.

Found and not fixed:
- **A very long reply reopens without its front.** `ledger::Transcript::apply` keeps the newest `MAX_EVENTS` (2000)
  events by sequence and joins a message's fragments afterwards. A reply streamed in more than 2000 fragments
  therefore loses its opening, which is the bug Electron fixed by joining before the window (mission 645f02a4).
  Electron also keeps the opening quarter of a 3000-event window, not only the newest.
  - Confirmed by a throwaway test, which was then removed: a reply streamed in 2,500 fragments reopens starting
    "w500 w501 …", so its first 500 fragments are gone.
  - The fix touches the reader's replay rule ("sequence keys make duplicate records replay correctly") and the
    door's `latest_final`, which reads the same events. It needs a batch of its own.
- Deleting: the window says ELECTRON_ONLY for a missing record, not Electron's `NOT_FOUND` "That mission does not
  exist.". An unsafe id is refused with "missionId is invalid". The behaviour (refused, nothing touched) matches.

Why some are not carried:
- Mission history: `publicRecoveredMission` (eventCount, eventsTruncated, the first-and-latest window, host failure,
  issue count), `withinByteBudget`, `HISTORY_UNAVAILABLE` and the workroom join are the IPC payload. The native window
  reads the ledger in process and has none of them.
- The other three batch-4 files check renderer source text (`Screens.tsx`, `Thread.tsx`), or the mission-store
  reader's issue codes, which the Rust reader does not have.

Break checks (all caught by the named test): the folder id's length and its hash; a reply keeping only its last
fragment; final from the first fragment; a replace ignored; two messages keyed as one; a running mission deleted; a
missing one counted as deleted; an unsafe id let through. Then the limit word; a temporary limit counted; a completion
not clearing; the newest by time; the source adapter ignored; the store not read; the window's "from a run at"; the
warning's reset; the temporary-limit reading; the newest window by time. For the receipt: the `!is_slot` fix; the
shared folder; silence on a clean turn; the warning level; the answer's first line as the body.
- One survivor came first. "Last read wins", in place of newest by time, passed Electron's own case. The test now
  lists the two missions both ways round, and the break is caught.

## Batch 5: permissions (and the start files)

Commits 0e6a6e45, e0128771, and the follow-up that tells Claude Locust's own refusals bare.

| File | Row | Rust tests |
| --- | --- | --- |
| `permission-host.test.ts` | partial (13/16) | `core/src/permission_host_tests.rs` |
| `approval-channel.test.ts` | partial (16/19) | `core/src/approval_channel_tests.rs` |
| `a-copilot-permission-is-a-card.test.ts` | partial (6/7) | `core/src/copilot_permission_tests.rs` |
| `the-permission-bridge-keeps-its-characters.test.ts` | carried (1/1) | `core/src/runtime/permission_bridge.rs` |
| `approvals-come-only-from-the-window.test.ts` | partial (1/2) | `core/src/approvals_from_the_window_tests.rs` |
| `starting-one-mission-probes-one-runtime.test.ts` | does-not-apply | none: a native start sweeps nothing |
| `the-start-says-its-phase-on-every-route.test.ts` | waiting | none: no starting-runtime phase in native |
| `antigravity-mission.test.ts` | waiting | none: native has only the agy CLI route |
| `codex-mission.test.ts` | waiting | 137 cases, left for a batch of its own |
| `a-carried-conversation-keeps-its-route-and-reports-its-status.test.ts` | waiting | the file is not in this tree's `apps/desktop/src/main`, though `tests.tsv` has a row for it; nothing to carry |

Bugs found and fixed:
- **Claude was told a denial in other words.** Rust sent the person's bare reason, or "The person declined this in
  Locust.". Electron's host sends "Denied in Locust.", or "Denied in Locust. The person declined this, and said:
  <reason>". A saved rule's denial reads the same way (see the Electron export golden in `mission_export.rs`).
  - A follow-up fixes what 0e6a6e45 got wrong: it wrapped every reason as the person's, so the command guard's sentence
    (and a turn's end) reached Claude as "the person said". `ask_by` now says whose an answer is. Only a card or a
    saved rule is wrapped; Locust's own reason goes as it is
    (`locusts_own_refusal_is_not_told_to_claude_as_the_persons_words`).
- **A question's answer aimed at a Claude permission** told Claude only "no". Claude is now told Electron's "Locust
  did not send that answer: it belongs to a question, and the runtime was waiting on a permission.".
- **A connector's card showed its input as compact JSON.** Electron's `wholeInput` is `JSON.stringify(input, null,
  2)`, and nothing when there is no input.
- **Any `tool_use_id` was kept for a card.** Electron keeps only `/^[A-Za-z0-9_-]{1,128}$/`.
- **The channel refused an unknown request silently.** It now returns Electron's words to its caller: "Locust denied
  a request it does not recognise (<method>) because it could not say what would happen. This build is older than the
  runtime's protocol."
  - Correction to e0128771's message: Codex's own session never hands the channel an unknown method. Its dispatcher
    (`runtime/codex.rs` `notification`) answers it with JSON-RPC -32601 and its own notice, "Codex requested an
    unsupported interaction; native declined it.". Electron gives it to the channel and says the sentence above.
  - OpenCode's caller still turns the channel's refusal into a plain reject without saying it. Neither difference
    was changed.
- **Sixteen waiting cards** now refuse with Electron's "Locust denied an action because 16 approvals are already
  waiting on you. Answer some of them and ask again.", not "Too many approvals waiting.".
- **Copilot's (ACP) cards were Rust's own.** `ask_acp_runtime` now builds Electron's `acpPermissionRequest` and goes
  the Codex-shaped way every other card goes. Three cases changed:
  - a shell that did not say its command reads "Run a command it did not describe";
  - an edit is "Change 1 file", named relative to the folder, with its patch on the card;
  - an untitled call reads "Use <kind>".
- `permission_bridge::serve` now takes its reader and writer (`serve_from`), so the split-character case is tested
  against the real decoder.

Found and not fixed:
- **Codex is sent a reason Electron never sends.** On a run's end, and on a person's denial, Rust's
  `protocol_answer_for` puts a `reason` on Codex's answer. Electron sends Codex `{decision}` only, and gives a denial's
  reason to the run as its next input (`onDeniedSaying`). That path is not built in Rust.
  - Stripping the field now would lose the person's reason, so it was left.
  - `approval_channel_tests` reads only the decision on release.
- `beforeApproval` (a Claude card waits for the run's pending text to be written) is not built. Neither is
  `serverPrefixOf`, nor `withFileChanges` (a fileChange activity row rewritten from the item's changes).
- The token, the one shared listener and Electron's `bridgeConfig` env belong to Electron's loopback HTTP host. The
  native bridge is one Unix socket per run, in a 0700 folder.
- "Always" for the same tool again is answered by the one channel (earlier-always). Electron's host raises it and its
  main process answers it.

Test safety: the permission tests install a temporary store through `native::hooks`. Without one,
`native::store()` falls back to the person's own store, and the saved-rule lookup reads it, or writes `used` on a
match. The existing `permission_bridge` test still runs without one.

Break checks (all caught by the named test, 44 in all): the deny sentence with and without a reason; pretty JSON; the
id shape; the question refusal; the Always key by tool; the connector summary; Always per run; the peer-closed
withdraw; the config dir on drop; a non-call raising a card; the recorded answer; the server name; the method kinds;
the command detail and cwd; "did not describe"; file counts; the detail cap; a nameless question; the protocol
decisions; the unknown-method words; the pending limit; question against decision; the answers' shape; release by
mission; the item's changes; split-character decoding; eight ACP breaks; two new `decide` callers.

Two breaks hung rather than failed (a request left unanswered), so the tests were given limits:
- `permission_host_tests::answer` reads with `UNDER_LOAD`;
- the 17th request in `approval_channel_tests` waits 5 s.

macOS returns EINVAL for `set_read_timeout` on a socket its peer already closed, so that error is ignored.

## Round 3 (16 Nov 2026, evening)

Branch at eb7284a7. Break checks ran through `~/work/scratch/maxlane/mutfilter.py`, as in round 2.

## Batch 6: the model catalog, a usage reading kept

| File | Row | Rust tests |
| --- | --- | --- |
| `model-catalog.test.ts` | partial (27/32) | `core/src/model_catalog_tests.rs` |
| `a-usage-reading-outlives-the-newest-twenty.test.ts` | carried (2/2) | `core/src/usage_readings_kept_tests.rs` |
| `codex-usage-is-read-with-the-models.test.ts` | waiting | none: no `account/rateLimits/read` |
| `antigravity-usage-is-read-without-a-turn.test.ts` | waiting | none: no agy usage read |

Bugs found and fixed:
- **A catalog that listed nothing was kept for ten minutes.** `models::kept_catalog` kept any catalog not missing a
  ready runtime's list, including an empty one. Electron answers an empty list with `MODELS_UNAVAILABLE` and never
  caches a failure. Now an empty catalog is read again on the next call.
- **Two callers at once swept twice.** `kept_catalog` let go of its cache while it read, so a second caller started
  its own 20-second sweep of every agent. Electron answers the second caller from the first probe. The cache now
  stays locked while it reads: the second caller waits and is answered from the first read.
  - The callers are a Cursor start (`native.rs`), the door's model list and `--models`. None runs on the window's
    thread, so the wait blocks no frame.
  - One difference is left. When the first read lists nothing, the waiting caller reads again. Electron's would
    get the first read's failure.
- **A usage reading went with its conversation.** `usage_ledger::usage_windows` read `usage-readings.json` (this
  app's and Electron's), but nothing in Rust wrote it. Deleting the conversation that last said an agent's window
  lost the reading. It now keeps the merge in this app's file, but only when the merge changed. A newer reading from
  the ledgers takes an agent's place, and wins a tie, as in Electron.
  - Rust reads every ledger, not the newest twenty, so Electron's backfill (conversations 21 to 80) is not needed.

Why some cases are not carried:
- Rust's lists carry no readiness. `cursor_models_from` and `claude_models_from` take what was listed, and a runtime
  that is not installed lists nothing. So the "signed out" and "not ready" halves of two cases have no Rust test,
  and neither does the "not ready Codex fails soft without spawning" case.
- The probe's spawn is not injectable: `codex_model_list` starts the real `codex app-server`. The "reads the list and
  shuts the server down" case is tested for the three rows only.
- `claudeModelsFrom`'s second argument, the models Claude Code's handshake listed (0.697), is not built:
  `claude_models_from` reads only `--help`.
- Codex's usage beside its models (0.390) and Antigravity's usage without a turn (0.709) are not built.

Break checks (all caught by the named test, 33 in all): the hidden flag; the effort's field; the missing id; the data
key; the account default's place; kept no time; kept too long; an empty catalog kept; the lock let go during the
read; the default effort; extra-high's level; the shared name's tail; nothing shared; Haiku measured; Haiku only
beside advertised aliases; the version name; the alias's description; the older versions' own levels; current
models only; the shared levels; an earlier default row; the row's name; the second default; each runtime's own
levels; a default for every runtime; Claude without Codex; a Cursor and a Claude that listed nothing. For the usage
reading: not written; not read back; a newer reading not taking the place.
- One break survived, and it is equivalent. With `thinking` after a level not taken off (`split_cursor_model_id`),
  the word stays among the family's words, and the family comes out the same. No input tells the two apart.

## Batch 7: `codex-mission.test.ts`, first three groups

`codex-mission.test.ts` has 137 cases in 22 groups. It is carried a group at a time, in
`core/src/native/codex_mission_tests.rs`, a child of `native::tests` so it shares that module's temp store, fake
app-server and `send` helpers. The row stays `partial` with a running count: 12 of 137 so far.

| Group | Carried | Rust tests |
| --- | --- | --- |
| mission sandbox | 6 of 8, 2 half | `codex_mission_tests` |
| model selection | 3 of 3 | `codex_mission_tests` (a new `no-model` scenario in the fake app-server asserts no model is sent) |
| a model of your own (0.357) | 3 of 3 | `codex_mission_tests` (removed: refused, nothing written); `own_models_tests` (env only; other models untouched) |
| a file the runtime named but the host could not read (0.597) | 0 of 1 | not built: native writes no observed edits from disk |

Bug found and fixed:
- **A teammate's limit that could not be checked refused the run.** `native::spend_gate` refused with "The
  spending record could not be read." Electron's personal limit fails open: `spendRefusalFor` logs "could not check
  <id>'s monthly limit: ..." and the run starts. Only an organization's cap closes on an unreadable record, and
  `org_spend_gate` still does. Native now writes Electron's line to stderr and lets the run start. It has no
  diagnostic log file to write it to, so that half of the case is not tested.

Not carried, and why:
- `Started` reports the turn's mode, not its sandbox, so "the UI states the real posture" has no counterpart. The
  ledger header carries the sandbox, and that part is tested.
- The model of your own's key in the tests is not Electron's: its value is secret-shaped, and such literals stay out
  of this public repository.

Break checks (all caught by the named test, 12 in all): own branch skipped for another app's start; the limit
skipped for another app's start; the limit's comparison; the fail-open put back; the header's sandbox; an unknown
mode read as accept-edits; `startedBy` not recorded; account-default sent as a model; account-default not recorded;
the turn's model not sent; a removed model of your own let through; a model invented when none was chosen.

## Batch 8: `codex-mission.test.ts`, what a start refuses and the mode it records

Six more of the "Codex mission service" group: 18 of 137 so far.

Bug found and fixed:
- **A blank prompt started a turn.** Native's `send_from` had no prompt check. Electron's `validPrompt` refuses a
  prompt that is blank, longer than 8,000 characters (counted in UTF-16 units, as JavaScript counts) or holds a NUL,
  saying "Enter a mission between 1 and 8,000 characters.". It does so before anything is read or recorded. The
  window's composer never sent a blank one, but the door and the relay reach `send_from` too.

Carried: an invalid prompt refused; a plan recorded as `plan` and held read-only; Auto refused with the workspace's
switch off and with no switch ever set, nothing recorded; Auto on the widest sandbox (`danger-full-access` on the
thread, `full-access` in the header) once allowed, and the next Auto refused once the switch is taken back.
- The fake app-server gains an `auto` scenario that expects `danger-full-access`. It answers only its own prompt
  ("Synthetic prompt"), so the Auto run sends that instead of Electron's "First, while it is on.".

Break checks (all caught by the named test): the check skipped; the length doubled; NUL let through; a plan
recorded as ask; a plan given workspace-write; Auto always allowed; Auto given workspace-write.
- One run was cut short, then run again. Its breaks waited on the build lock while the `opus2` lane's release suite
  and the `native-engine` lane's tests ran. Each break hit its 240-300 s limit and counted as TIMEOUT, never as
  caught. The run was stopped (`mutfilter.py` puts the file back on SIGTERM) and the four breaks it had not reached
  were run again once the lock was free. All four were caught.

## Batch 9: `a-person-can-leave-out-part-of-a-brief.test.ts`

Begun before the 22:47 stop, finished on 17 Nov.

| File | Row | Rust tests |
| --- | --- | --- |
| `a-person-can-leave-out-part-of-a-brief.test.ts` | carried (6/6) | `core/src/leave_out_brief_tests.rs`, `native::tests::a_route_switch_records_what_its_brief_left_out_not_what_was_sent` |

Bugs found and fixed. Native already took the left-out sections out of the brief, but:
- **The brief never said the person chose it.** Electron ends the brief with "(The person chose to leave out its last
  reply.)", after any "did not fit" notice, so the new runtime knows it has less than the whole story. Native said
  nothing. `carry_on::choice_notice` is new, and `compose_handoff_prompt_with` adds the notice.
- **The notice was not charged against the bound.** Electron holds its length back from the budget first, so a long
  task gives way for it. Pinned at Electron's own answers: a 3,900-character task beside a 3,773-character reply fits
  whole at 7,880; a 3,800-character reply clips the task to 7,879. Both sides were run through Electron's
  `composeHandoffPrompt` (`~/work/scratch/maxlane/b9/leaveout*.mjs`).
- **The switch's link recorded the list as it was sent.** `route_switch` wrote the window's `leftOutByYou` whole,
  including sections that may not go and ones that were not there. It now records `Briefing::left_out_by_you`,
  the sections actually left out, in reading order.
- **The engine wire took any strings.** `engine_process::origin_from` now keeps the list to the sections that may
  go (`carry_on::chosen_leave_out`, Electron's `chosenLeaveOut`).

Two tests beyond Electron's six pin Electron's values: the notice's charge against the bound (above), and the order of
the two notices (a 6,000-character reply: "did not fit ... summary", then the person's choice, 6,357 characters).

Break checks (all 11 caught by the named test): the window's list not kept to what may go; kept in the order sent;
a section that may not go let go; a section that was not there named; the section not taken out; the notice not
added; the notice not charged; the notice's words; the two notices swapped; the engine wire unfiltered; the link
recording what was sent.

## Batch 10: what a switch brief says, an attached name, a dead session, a refusal in Ask

| File | Row | Rust tests |
| --- | --- | --- |
| `a-switch-brief-says-what-was-done.test.ts` | carried (10/10) | `core/src/switch_brief_tests.rs` |
| `a-file-name-cannot-write-into-the-brief.test.ts` | carried (2/2) | `core/src/file_name_brief_tests.rs` |
| `a-session-that-ended-is-not-resumed.test.ts` | carried (2/2) | `core/src/native/session_ended_tests.rs` |
| `a-refusal-in-ask-says-the-mode.test.ts` | carried (2/2) | `core/src/runtime/opencode_refusal_tests.rs`, `runtime::opencode::tests::an_ask_or_plan_run_refuses_opencodes_question_naming_its_mode` |

Bug found and fixed:
- **An Ask or Plan run refused OpenCode without naming its mode.** Native answered every unasked refusal with
  "this teammate's mode does not allow it". Electron's `openCodeReadOnlyRefusal` (0.822) says "this teammate is in
  Ask mode, which reads and changes nothing" (Plan: "which reads and plans") for a read-only run nobody answers for.
  `opencode_serve::rejection_message` now picks between the person's decline, the read-only refusal and the
  plain unasked one, as Electron does.
  - Electron's second case is a source check (`codex-mission.ts` passes the refusal only when nobody answers and
    the sandbox is read-only). It is carried as behaviour: `rejection_message` for every mode, and a run in Ask and
    in Plan over the fake OpenCode server, whose reply body is read back. The fixture
    (`scripts/fixtures/m4-opencode-serve.py`) now logs each permission reply to `opencode-requests.jsonl`.

`carry_on` changes, no behaviour change: `omission_notice` is a function of its own (Electron's `omissionNotice`),
and `NOTICE_BUDGET` and `KEEP_ORDER` (Electron's `OPTIONAL_SECTION_NAMES`) are visible to the tests, so "the notice
fits its reserve with every optional section named" has a test.

Break checks (20 of 21 caught by the named test): the Plan words; the Ask words; Plan not read-only; the call
site's message; the person's decline; a name's quoting; the list's `- ` kept; `sessionEnded` ignored; every failure
ending the session; the tool named twice; the command not used; the forty cap; "other" swapped; "and N earlier"; the
asked clip; the brief's no-reply words; the usage-limit words; a completed turn said to fail; the notice's reserve;
the keep order.
- The one survivor is not this batch's: the first copy of "no reply was recorded" is `compose_cold_follow_up`'s,
  which these files never reach. The brief's own copy was broken next and caught.

## Batch 11: `a-provider-that-fails-first-is-carried-on.test.ts`

| File | Row | Rust tests |
| --- | --- | --- |
| `a-provider-that-fails-first-is-carried-on.test.ts` | partial (5 of 6, and half the 6th) | `core/src/provider_carry_on_tests.rs`, `core/src/native/provider_carry_on_tests.rs` |

No bug: native already classified and carried a provider failure as Electron does. `carry_on::stopped_for` and
`ended_on_plan_limit` (Electron's `stoppedFor` and `endedOnPlanLimit`) and `phase_of` are new. `native::carry`
and `carry_on_without_asking` each had their own copy of that rule, and now use the new functions.
- Electron passes a mission's phase. Native reads it from the turn's last terminal event (`phase_of`), so "a
  cancelled phase over a failed event" is tested through `stopped_for` only.
- The carry tests run against the fake app-server (`fake("success")`) for both Codex and Claude: only the new
  turn's header is read (its runtime, mode, `startedBy`, and the carry-on words in its prompt), not its run.
- `native::CARRIED` is shared by the whole process, so each stopped turn has an id of its own
  (`mission_provider_*`).
- Not carried: the last case's divider sentence (`src/ui/handoff_divider.rs`) and Settings' heading and
  "provider error" (`src/settings_pages.rs`). Those are the renderer lane's.

Break checks (all 17 caught by the named test): a tool run not counted; a completion not clearing the failure; the
sign-in kind; stderr's first line in place of its last; the "overloaded" pattern; any message read as trouble; the
plan's reason skipped; a cancelled phase carried; any reason read as the plan; the provider words; the same agent
let through; the reason forced in `carry`; `startedBy` without `from`; the plan's words in a provider carry; the
note's words; Without asking inverted; no reason in `carry_on_without_asking`.

Noise: `window_drive_tests::relay_stops_at_hop_cap_and_exchange_strip_stop_ends_live_run` failed once in the first
full run ("Stop button disappears once no run is live"). It then passed three times alone, and the second full
run (3,253 tests) passed whole.

## Batch 12: `a-plan-that-runs-out-is-carried-on.test.ts`

| File | Row | Rust tests |
| --- | --- | --- |
| `a-plan-that-runs-out-is-carried-on.test.ts` | partial (8 of 11, two halves) | `core/src/plan_carry_on_tests.rs`, `core/src/native/plan_carry_on_tests.rs` |

No bug found. The carry tests share batch 11's harness (`stopped`, `stopped_in` for a mode, `team`,
`carried_from`). "A start that fails" is a roster whose teammate names no model: native refuses it with
"Carry runs in Locust for now.", and the turn can be pressed again once the roster is put right.
- `team` rewrites `teammates.json`, which also holds the turns' owners. A test that calls it again records the
  owner again.
- The harness's teammate is on `haiku`, so a pick's model (`sonnet`) is told apart from the teammate's own.

Halves and gaps:
- "Only for a turn the person sent": native decides it at the run's end (`person_sent`, a turn with no
  `startedBy`) before it calls `carry_on_without_asking`. No fake runtime here ends a turn on a used-up plan, so
  that gate has no test.
- The record: native writes schema 27 with `startedBy {kind: carry-on, from}`, and that is tested. Electron's
  reader refusing a v23 file that says carry-on, and its writer refusing a starter that names no agent, are
  mission-store rules that native's ledger reader and writer do not have.
- Not carried: `index.ts` runs the carry before the limit toast, and the toast says nothing for a carried turn.
  That is a source-order check, and native's toast is `src/notify.rs`.

`a-carried-conversation-keeps-its-route-and-reports-its-status.test.ts` (a round-2 file) is not in this branch's
`apps/desktop/src/main`. It was added after the branch was cut and is in `~/work/locust`. Its row says so.

Break checks (20 of 22 caught by the named test): `route.limit_detected` not read; a completion not clearing the
limit; any kind read as the plan; the plan words; a second press let through; a failed start not let go; the
plan's same-agent words; OpenCode refused under approve-each; Claude let through under approve-each; the mode not
handed down; the refused note's reason; the plan note's words; `automatic` read loosely; any runtime accepted;
`account-default` kept as a model; the pick being the agent that stopped; a pick that is not ready; the order's
first two; the pick's model dropped (both lines at once).
- Two survivors, each equivalent alone: `carry` sets the pick's model on the teammate and on the job, and either
  one carries it into the turn. Broken together, they were caught.

## Batch 13: `workspace-brief.test.ts`

| File | Row | Rust tests |
| --- | --- | --- |
| `workspace-brief.test.ts` | partial (2 of 6, four halves) | `core/src/workspace_brief_tests.rs` |

**A gap, not fixed here: native gives a folder's `LOCUST.md` to no run.** Electron puts it in every run's brief
(`briefSection`: 'Instructions for the folder "shop", from its LOCUST.md. Every teammate on every runtime is given
these...', and "(LOCUST.md is longer than 200 lines; the rest was not loaded.)" when it was cut). Native reads the
file only for the Setup card's count (`teammate_setup::read_workspace_brief`, which gives lines and whether it was
cut, and no text). Building it means adding a section to native's run brief, and that is a feature of its own,
beyond a test batch. The two cases about the section's words wait for it.

Carried whole: `brief_for_runtime`, each runtime its own section, and tags left alone (no runtime, fenced,
unclosed). Carried as lines and `truncated`: read from the root with one line ending; none or empty is no brief;
bounded by lines (200) and by bytes (24 lines of 1,000 bytes fit in 24 KiB, 25 do not); bounded after the other
runtimes' sections are gone. Electron's no-folder and empty-folder reads have no counterpart, because
`read_workspace_brief` takes a `&Path`.

Break checks (all 10 caught by the named test): the trailing empty line kept; whitespace read as a brief; the line
cap; the byte check; the byte cut; another runtime's section given; a fenced tag read; an unclosed tag's line
dropped; `details` read as a runtime; the runtime filter skipped.


## Batch 14: `codex-mission.test.ts`, continuing a conversation

13 of the group's 16 cases (6 of them half): 31 of 137 so far. Tests in `core/src/native/codex_mission_tests.rs`. They
run against `scripts/fixtures/commands-app-server.py`, which logs every request it gets, so a test reads whether the
thread was started or resumed, by which id, in which sandbox, and what the turn was told. The earlier turns are written
by hand with Electron's `finished()` values (`mission_prior`, "check the google stock price", `thread-prior`), and aged
an hour so a turn with no terminal record reads as finished.

Bugs found and fixed. Native's same-agent follow-up is now Electron's one rule (`codex-mission.ts` 2100-2125) for
every runtime, not a branch per runtime:
- **A reply in another mode resumed the old session.** A resumed session keeps the tools it was built with. A
  conversation begun in Ask and continued in Accept edits ran with no write tools while the chip said Accept edits
  (Electron's outside tester, 0.38.7). Now a changed mode starts cold. A header with no mode is not a change. `Prior`
  carries the header's `mode`.
- **A conversation with no session was refused.** Native said "This conversation has no Codex session to continue."
  (and the same for OpenCode, Kilo, Cursor, Copilot, Antigravity, Muse and ACP). Electron continues it cold, recorded
  as a follow-up with no `runtimeThreadId` (Colin, 2026-09-03: a reply after a failure opened a second row).
- **A cold reply was told nothing.** Native's Claude started a new session silently, and the others refused. Now a
  cold start is told the conversation so far through `compose_cold_follow_up` ("This conversation has earlier turns,
  but not in your session..."), and the record keeps the person's words.
- **Send again** (`cold_earlier`): the newest turn's words are not quoted back when it went unanswered and this reply
  is the same words.
- The old test `a_conversation_with_nothing_to_resume_or_still_running...` asserted the refusal. It now keeps only its
  "still running" half (`a_conversation_still_running_is_refused_without_starting_anything`).

Halves and gaps:
- Native's `Started` has no `followsUp` or `switchedFrom`, so the receipt halves of three cases are read from the
  new turn's header instead.
- Electron's edit case also checks the terminal is not caught up (`catchUpTerminal`). Native has no counterpart.
- Electron's A2.11 case switches over a turn that continues itself. Native's start refuses that history before any
  brief, with "This conversation has an invalid history. Its references were not changed." That is the same check
  as Electron's own `conversation-references.ts` `rootOf`; Electron's test service wires no references. So the
  case is two tests: the switch brief over a sound chain, and `earlier_turns` stopping at the loop.
- The route switch's link says checkpoint epoch 1: native's first checkpoint of a turn. Electron's 2 is its fake's.
- The "unsafe" refusal is reached through a torn last line, and says native's reason.
- H6, resuming an interrupted mission (`service.resume`), is not built. That is 3 cases.

One test beyond Electron's: `the_same_words_after_an_answered_turn_are_told_that_turn` (the answered half of Send
again survived its break without it).

Break checks (all 14 caught by the named test): no mode read as a change; a changed mode still resuming; no session
refused; the cold start not told; Send again off; Send again ignoring the reply; the cold link carrying a thread; an
edit told as a plain reply; the walk's loop guard; the header's mode not read; the newest turn not quoted; the unsafe
refusal; the switch brief without earlier turns; the checkpoint not written back.
- One break was a build error at first (the loop guard). It was rewritten (`&& false`) and then caught.

## Batch 15: `codex-mission.test.ts`, the workroom around a mission

8 of the group's 12 cases (4 of them half): 39 of 137 so far. (Commit 3be39b9e's subject says 9; the 9th test it
counted pins Electron's words and is not one of Electron's cases. The row was put right in the next commit.) The tests are in `core/src/native/codex_mission_tests.rs`,
with Wren and Atlas on the roster and the relay off, so a share waits in the workroom instead of starting Atlas. The
recording fake app-server (`scripts/fixtures/commands-app-server.py`) now reads an optional `commands-reply.json` from
its folder, the turn's text and status, so a test can make a turn share or fail.

Bugs found and fixed:
- **A Plan turn was never told to plan.** Native ran Plan read-only but sent no plan instruction. Electron appends
  `planSection` ("PLAN FIRST. ... numbered steps ...") after everything else, the references line included, so it is
  the instruction closest to the answer. It checks the sandbox as well as the mode. `briefing::plan_section` is
  new, and `send_from` appends it.
- **A read-only OpenCode run was not told it has no shell.** In Ask or Plan, OpenCode refuses every shell call and
  the run ends there. Electron tells it so (`openCodeReadOnlySection`, A2.20), before the plan instruction. Electron
  leaves the line out when the run has Locust's git tool (0.818). Native has no git tool, so it always says it.
- Neither section follows a command the person typed, which still goes to the runtime alone.

Halves and gaps:
- Native's `Started` carries no `peerMessages`, `peerDeliveryFailed` or peer-message updates, so the workroom and
  the ledger's `mission.peer` links are read instead. The roster refusal is a relay notice, not a
  `peer-share-failed` update.
- **Found, not fixed:** a received link the ledger cannot write. Electron refuses the run ("The messages this mission
  was shown could not be recorded in the durable local ledger."), starts nothing and consumes nothing. Native ignores
  the failure (`let _ = writer.write_peer_link(...)`). The link is written after the turn has registered its slot,
  so refusing there needs the slot let go too. That is left for a change of its own.
- Not built: Locust's git tool for a read-only OpenCode run (round 1 found the `locust-git-reads` server is not
  built), and a run that belongs to nobody (a native turn always has a teammate). The sealed look's OpenCode
  permissions were not carried here.
- `peer_exchange.rs` has the roster refusal twice: `share_reply` and `share_reply_for_relay`. A native turn uses the
  second, and the first break, made in the first, survived for that reason. The break in the second was caught.

Break checks (13 behaviours, 11 caught by the named test at first, all 13 in the end): the no-shell line for
every OpenCode run; for every read-only run; the plan section for every read-only run; the plan section first; a typed
command briefed; the plan words; the received link's teammate; a posted link recorded as received; a share from a run
that did not complete; the plan section's last line; an unreadable workroom refusing the run.
- Survivors, then caught: the no-shell words, whose test compared against the function, so a test now pins Electron's
  words for both sections; and the roster refusal, whose break was in the copy native does not use (above).

## Batch 16: `codex-mission.test.ts`, Codex over app-server

All 7 of the group's cases (2 of them half): 46 of 137 so far. No bug: native's Codex path already did what
Electron's does. The recording fake app-server's `commands-reply.json` can now also hold `deltas` (pieces streamed
before the whole text) and `config` (what `config/read` answers).

Halves:
- The plan tool (0.153) is tested on `runtime::codex::app_server_args`. Under test the hook replaces the command, so
  the record holds the fake's argv. That argv is still what ran, which is Electron's claim.
- The command guard is tested on `command_guard::codex_config_for_home` with a temporary Codex home, with no hook
  beside it and with a `hooks.json` that declares one. A run reads `CODEX_HOME` or `~/.codex`, which is the
  machine's own, so the guard on a run's `thread/start` is not asserted.

Break checks (all 12 caught by the named test): the plan tool's first release; its setting; the settings read for
every mode but Edit; for Edit too; an address server switched off when unsealed; the sealed shell; the sealed web
search; the sealed config not used; a program left on; the guard beside another hook; the guard's bypass; Edit's
sandbox widened.

## After batch 16: what else was checked, and why it was left

- Batch 14's rule is proven on Codex and, since this follow-up, on Claude Code
  (`a_claude_reply_with_no_session_to_resume_is_told_what_was_said_before`, its break caught). OpenCode, Kilo,
  Cursor, Copilot, Antigravity, Muse and ACP share the same branch now; a reply with no session on them starts cold
  instead of being refused, and no test runs that path on them yet.
- `the_earlier_turns_walk_stops_at_a_turn_that_continues_itself` now runs under the recording fake and asserts that
  nothing was started, so a regression there cannot reach a real Codex.
- `codex-mission.test.ts` groups scouted and not carried:
  - **Mid-mission handoff** (9 cases): native has no live handoff. A route switch is made only from a finished turn,
    and nothing stops a running turn to restart it on another runtime.
  - **Steering a Codex run** (1 case): Codex's runtime has no steer. Only ACP's does (`runtime/acp.rs`).
  - **Missions side by side** (4 cases): the cap's words and count are already tested
    (`native::tests::a_ninth_turn_at_once_is_refused_in_electrons_words`), and so is a stop
    (`stop_interrupts_the_runtime_records_cancelled_and_only_stops_what_this_window_started`). They are not
    credited to this file. A reply to a turn still running is refused in native's words, "A turn is already running
    in this conversation.". Electron says "<Name> is still answering in this conversation. Your message goes when
    that turn finishes.". It was not changed, because `rooms.rs` `classify_send_error` keys on native's string, and
    the window queues a reply itself before it gets there. The handoff of one run while another runs is not built.
  - **Stopping a Codex run over app-server** (1 case): covered in spirit by the stop test above; not credited.
- Next groups with native code: "runtime selection" (16 cases) and "Approve-each is a mode of this service" (5): batch 17.

## Batch 17: `codex-mission.test.ts`, runtime selection and Approve-each

Begun before the 04:22 stop on 17 Nov, finished after it. 15 more of the file's cases (1 half): 61 of 137 so far. The
branch was fast-forwarded to native-engine's head (6242dde6, which holds eb7284a7 and this lane's 31e878ed) before
committing, with no change to these files.

| Group | Carried | Rust tests |
| --- | --- | --- |
| runtime selection | 11 of 16, 1 half | `codex_mission_tests` |
| Approve-each is a mode of this service | 4 of 5 | `codex_mission_tests` |

Bug found and fixed:
- **A Cursor turn given an effort left a record of a run that never existed.** `send_from` wrote the turn's ledger,
  and only then did the Cursor driver refuse the effort ("Cursor Agent takes no effort level"). Electron refuses
  options that cannot become a command before the mission is written. `send_from` now asks `cursor::refusal` first,
  so nothing is started or recorded. A Cursor effort with no model no longer reads the model catalog either: with no
  model there is no family to look in.

How they are run:
- Runtime selection: a fake that writes down its argv (`argv_fake`) stands in for Claude Code, Cursor and Copilot. Under
  the test hook the runtime's own command is the fake's, so the Cursor and Copilot argv Electron checks is read from
  their builders (`cursor::print_args`, `copilot::prompt_args`); what a turn adds after that (Claude Code's argv,
  Copilot's `--plugin-dir`) reaches the fake.
- Approve-each: the recording fake app-server (`scripts/fixtures/commands-app-server.py`) can now ask: `ask` in
  `commands-reply.json` is sent as `srv-1`, the turn waits on it, and the answer is written to
  `commands-replies.jsonl`. It also answers `turn/interrupt`.

Halves and gaps:
- A runtime native cannot read (Gemini) is refused before anything starts, in native's words for a teammate it cannot
  run ("Codec runs in Locust for now."), not Electron's "Gemini CLI is installed and signed in, ..." line.
- Not built: a message handed to a running Claude turn (A2.10; only ACP's runtime steers), Copilot's own MCP servers
  switched off in Ask (0.824, no `--disable-mcp-server` in core), a Cursor Ask held back beside an Edit run that holds
  connector rules (0.825), Windows' Cursor ask without `--sandbox`, and the unavailable runtime named (discovery is
  not injectable under the hook).
- Approve-each's first case (the pending text is on disk before the card) has no counterpart: native's ledger writer
  has no text deadline to race.

Break checks (all 25 behaviours caught by the named test):
- Runtime selection (15): stream-json changed; `--sandbox` added to Claude; acceptEdits sent as default; the header's
  sandbox forced read-only and forced workspace-write for Claude; Gemini let through; Cursor's ask sent as plan; the
  session id dropped; `sourceAdapter` always codex; Copilot's `--effort` renamed; the guard plugin for every runtime
  but Kilo; `--plugin-dir` renamed; `--mode` always sent; `--force` for every writing run; the new refusal skipped.
  Their first run's logs were lost in the 04:22 stop; all 15 were run again here and each was caught.
- Approve-each (10): the policy `never`; the thread's sandbox read-only; the header's sandbox read-only; every
  approval declined; the card's runtime; the card filed under the run, not the conversation; approve-once sent as
  decline; any answer taken; a stop's cancel not seen by the waiting card (the test fails after 128 s: the run hangs
  until the test's waits give up).
  - Three survivors, each redundant alone: a card is let go when its wait is cancelled, when the native loop sees the
    terminal event, and when Codex's reader sees it. Broken together, they were caught.
- Several breaks first hit their 130 s limit while other lanes held the build lock; each was run again alone and
  caught. Logs: `~/work/scratch/maxlane/b17`, `~/work/scratch/maxlane/b18`.

## Round 4 (17 Nov 2026, night)

Branch at f72f97c7.

## Batch 18: OpenCode serve busy-model and never-begun, plus command-guard tidy

- **`a-busy-model-served-says-so.test.ts`** (carried, 1/1): `a_busy_model_through_opencode_serve_is_said_once_in_the_providers_words` in `core/src/runtime/opencode_tests.rs` using `scripts/fixtures/m4-opencode-serve.py` (`busy-model`).
- **`a-message-taken-and-never-begun-ends-the-run.test.ts`** (partial, 3 cases): the 3 "a message OpenCode took and never began" cases (`a_message_opencode_took_and_never_began_ends_the_run_saying_so`, `a_message_opencode_took_waits_on_while_the_server_says_it_is_busy`, `a_message_opencode_took_asks_nothing_once_the_model_has_begun`), with `#[cfg(test)] with_begin_check` on `OpenCode` / `Serve` and new scenarios (`never-begun`, `never-begun-busy`, `begun-no-idle`) in `m4-opencode-serve.py`. EOF stream-recovery and follow-up parentID groups were not carried here.
- **Command-guard tidy** (`ENGINE-PACKAGES-NOTES.md`, Colin's decision): `approval.rs` no longer builds or queues a `"command-guard"` record on a guard refusal; `saved_deny_and_guard_precede_session_always` now asserts `ch.take_recorded().is_empty()` and uses a watcher thread so a missing guard refusal fails immediately instead of blocking on a card.
- Break checks (all 6 caught by the named test): `session.status` retry ignored, `locust.retry` deduplication dropped, `busy | retry` ignored in `recover`, `self.begun = true` dropped on assistant `message.updated`, watchdog `!self.begun` check disabled, and `approval.rs` guard `Kind::Command` check disabled.
