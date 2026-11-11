# P11 — Routines first; incomplete parity implementation

This round adds a native routine shelf/editor, not completed Area 9 parity. Compare/Blind was deliberately not started. No row is promoted to done and M5 is not reached.

## Implemented

- `src/routines.rs`: native-owned `routines.json`, seeded once from Electron and subsequently independent; serialized atomic mutations through `Store`; name/step bounds measured as UTF-16; create/edit/remove, pause/resume, preservation of progress and run counters. Unknown records are retained rather than erased. Active/uncertain/staged work prevents removal.
- `src/routines/schedule.rs`: clock schedule validation and calculations for intervals, daily, chosen days and one-off dates; calendar-day arithmetic, local DST overlap/gap handling, schedule labels, resumed clock bases and missed closed slots. Arriving-file/hook schedules are recognized but have no driver.
- `src/ui/routines_panel.rs`: Automations navigation, per-step text fields, add/remove step, schedule choices/day controls, pause/resume, retained history, review acknowledgements and links to recent conversations. Native-unavailable execution is stated visibly.
- Single-step runs call `native::send`, using the supplied native store's current teammate route/model and saved permission mode. Dispatch intent is saved first. Every dispatch is held for review and never automatically retried. No completion is claimed. Restart turns dispatching/running receipts into non-continuable holds. Abandon requires explicit review in the UI and removes the schedule; it refuses a still-owned native turn.
- Unsupported copy/input/handoff/check/multi-step runs are refused before dispatch. They do not silently run in the original folder, omit inputs or skip steps.

## Still required for the requested routines parity

The scheduler does NOT execute due routines while the app runs. Startup can account for closed slots, but the running-app scheduler and long-running slot accounting are unfinished. Native completion reconciliation, completed-run accounting, multi-step continuation, durable question receipts, capacity/busy handling, route-switch events and full recovery decisions remain. Native's generic JSON seeding limit is 2 MiB versus Electron routine-store's 4 MiB. Routine entry/route validation and malformed-record handling are not full TS parity. Imported advanced definitions can be retained/edited but are not executed; advanced edit validation remains incomplete.

Input forms/substitution, Markdown/JSON import/export, starter templates, copy creation/change review/keep/discard, retry-until-check, handed-off steps and wait-for-OK gates are unimplemented. Retry never proceeds after an uncertain result. Neither the copy row nor the recovery composite row is promoted on the strength of safe refusal alone. Full Electron UI words, history presentation, routing labels, source-conversation saving and UI interaction assertions still need porting.

Electron comparison copies default to `~/.locust/compare`, following `LOCUST_COMPARE_ROOT`; routine copies default to `~/.locust/routines` (`main/compare-copies.ts`, `main/routine-copy.ts`). A future native implementation must put their corresponding roots under native data. No native comparison code or copy folders were created in this round.

## TypeScript test disposition

The Rust tests are in `src/routines/tests.rs`. Schedule validation, interval/daily/weekly/once calculations, exact schedule labels, pause clock base, seed independence, pause persistence, edit counter preservation, unsafe removal/replay refusal, stale acknowledgement, UTF-16 step budget and unreadable-file preservation have matching assertions. A synthetic native Codex fixture exercises single-step dispatch and refusal to replay. Restart holds and paused missed-slot accounting are tested.

This is NOT a claim that the TS files below are fully ported. Each file remains applicable. The gaps below prevent the associated composite parity rows from being done. Electron/React/preload-specific wiring needs equivalent native interaction tests, rather than being silently marked inapplicable.

| TS test file (relative to desktop/src) | Disposition |
| --- | --- |
| `main/a-called-link-starts-its-routine.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `main/a-daily-routine-keeps-its-time.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `main/a-file-arriving-starts-its-routine.test.ts` | Unported and applicable: routine input/file/template or trigger behavior remains outside the implemented subset. |
| `main/a-paused-routine-waits-to-be-resumed.test.ts` | Partially ported: pause/resume persistence, clock base and no missed slot while paused; scheduled dispatch/tray behavior remains. |
| `main/a-routine-asks-for-inputs-and-travels-as-a-file.test.ts` | Unported and applicable: routine input/file/template or trigger behavior remains outside the implemented subset. |
| `main/a-routine-at-the-monthly-limit-says-so.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `main/a-routine-awaiting-review-is-not-removed.test.ts` | Partially ported: unsafe removal refused and abandonment permits removal; exhaustive imported execution-state cases remain. |
| `main/a-routine-due-while-closed-is-missed.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `main/a-routine-hands-off-to-the-next-teammate.test.ts` | Unported and applicable: check retries, handoff and wait-for-OK execution/UI remains unavailable. |
| `main/a-routine-is-a-markdown-file.test.ts` | Unported and applicable: routine input/file/template or trigger behavior remains outside the implemented subset. |
| `main/a-routine-keeps-going-until-its-check-passes.test.ts` | Unported and applicable: check retries, handoff and wait-for-OK execution/UI remains unavailable. |
| `main/a-routine-runs-on-the-route-its-teammate-runs-on.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `main/a-routine-step-too-long-is-refused-before-it-starts.test.ts` | Partially ported: UTF-16 save budget and refusal; full stored legacy-step/continuation cases remain. |
| `main/a-routine-step-waits-for-your-ok.test.ts` | Unported and applicable: check retries, handoff and wait-for-OK execution/UI remains unavailable. |
| `main/a-routine-time-passed-while-it-ran-is-missed.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `main/a-routine-works-in-a-copy.test.ts` | Unported and applicable: routine copy creation/review/keep/discard remains unavailable. |
| `main/a-team-card-carries-its-routines.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `main/routine-recovery.test.ts` | Partially ported: pre-dispatch intent, stale acknowledgement and no replay; terminal reconciliation, completion accounting and continuation/recovery IPC remain. |
| `main/routine-runner.test.ts` | Partially ported: one synthetic step uses native runtime; automatic advancement, scheduler, dynamic routes and question receipt matrix remain. |
| `main/routine-store.test.ts` | Partially ported: basic validation/create/edit/remove/read-back; run completion, full schema filtering, route/provenance validation and advanced definitions remain. |
| `main/the-starter-routines-are-routines.test.ts` | Unported and applicable: routine input/file/template or trigger behavior remains outside the implemented subset. |
| `preload/routine-inputs.test.ts` | Unported and applicable: routine input/file/template or trigger behavior remains outside the implemented subset. |
| `preload/routine-recovery.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/a-missed-routine-says-missed-and-run-now.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/a-new-shelf-offers-starter-routines.test.tsx` | Unported and applicable: routine input/file/template or trigger behavior remains outside the implemented subset. |
| `renderer/src/a-paused-routine-says-so.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/a-routine-keeps-what-you-keep.test.tsx` | Unported and applicable: routine copy creation/review/keep/discard remains unavailable. |
| `renderer/src/a-routine-names-its-folder.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/a-routine-previews-its-inputs-and-file.test.tsx` | Unported and applicable: routine input/file/template or trigger behavior remains outside the implemented subset. |
| `renderer/src/a-routine-run-is-marked.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/a-routine-says-what-it-may-do.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/a-routine-says-what-it-runs-on.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/a-routine-step-can-go-to-another-teammate.test.tsx` | Unported and applicable: check retries, handoff and wait-for-OK execution/UI remains unavailable. |
| `renderer/src/a-routine-waiting-for-keep-is-on-the-board.test.tsx` | Unported and applicable: routine copy creation/review/keep/discard remains unavailable. |
| `renderer/src/a-routine-waiting-for-your-ok-asks-plainly.test.tsx` | Unported and applicable: check retries, handoff and wait-for-OK execution/UI remains unavailable. |
| `renderer/src/nobodys-conversation-can-be-a-routine.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/routine-dialog-names-the-model.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/routine-recovery.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/routines-shelf.test.tsx` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `renderer/src/routines.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |
| `shared/routine-schedule.test.ts` | Partially ported: validation, next-run and label cases match; full schedule helper inventory and DST oracle cases remain. |
| `shared/routines-and-rooms-from-a-browser.test.ts` | Unported and applicable: full scheduler, route, presentation or integration behavior remains; this round does not claim this file passed. |

## Verification

Final verification results are recorded below. No TS test runner was invoked; the existing release suite invokes Electron's ledger reader using the supplied LOCUST_TEST_TS_READER. No window was opened and no real agent turn was run. The added runtime test uses only the Python app-server fixture.

An initial fixture incorrectly lacked its native directory, then its synthetic runtime model did not match the fake server's required model; these were corrected. Another failed test caught a default-store roster lookup and that lookup was replaced with the explicitly supplied Store before the final runs. That failed attempt did not reach a real runtime dispatch. Metadata inspection confirmed that the failed default-store lookup created `/Users/locust/Library/Application Support/@teammate/desktop-native/teammates.json` at 2026-11-10 23:23:08 local time. This was an unintended write outside the authorized copy. The file was left in place because removal would be another out-of-folder mutation. The caller now reads the explicitly supplied Store. Electron’s source folder remained read-only.

Formatting: cargo fmt was initially run across the crate, so pre-existing formatting in `src/keyboard_shortcuts.rs`, `src/terminal.rs`, `src/own_command.rs`, `src/settings_pages.rs`, plus module ordering in `src/main.rs`, was normalized. These are formatting-only changes outside the routine wiring. This workspace has no .git metadata to provide a reliable original diff or restore those formatting changes.

### Final results

- Required release test command, with `LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js`, `PATH=$HOME/.cargo/bin:$PATH`, and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p11`: **459 passed, 0 failed, 13 existing live tests ignored**, three consecutive final runs. Eleven new routine tests; none ignored. Logs: `work/p11-test-1.log`, `work/p11-test-2.log`, `work/p11-test-3.log`.
- `cargo build --release`: success, **zero warnings** (`work/p11-build.log`).
- `cargo clippy --release`: success, four pre-existing `too_many_arguments` findings in `approval.rs`, `ledger_write.rs`, and two in `native.rs`; **no new findings** (`work/p11-clippy-final.log`).
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py`: passed against the release binary, synthetic subprocesses only (`work/p11-m1.log`, `work/p11-m3a.log`).
- Headless Rune snapshots: `/Users/locust/work/scratch/p11-snaps/routines-dark.png` and `routines-light.png`, both **900×900 at scale 1**, rendered and visually inspected. Shelf labels, action row, paused/missed state and native limitation text are legible without clipping. Editor/recovery interaction and native window behavior remain unverified.
- No new process-spawning path bypasses `src/runtime/process.rs`: the routine dispatch uses the existing native runtime. No production `unwrap()`, `allow(dead_code)`, or new ignored test was added.
- PARITY.md Section 1 and Section 3 recounted from rows: **153 total, 28 done, 33 partial, 92 missing**. Area 9: **0 done, 1 partial, 9 missing**. No milestone advanced.

Not run: TS test runner, real agent turns, live scheduler, native window, real copy/change application, actual Markdown import/export, real handoffs/gates, or compare. These are unfinished or unverified, not passed by the synthetic suite.

## Review and fixes

Parity pass against Electron (`shared/routine-schedule.ts`, `main/routine-store.ts` pause/keep/abandon, `main/routine-runner.ts` / recovery holds, and the schedule/pause/missed TS tests beside them). Fixes landed in this copy only. No PARITY row was promoted to done. Antigravity’s earlier build notes above stay as the incomplete-parity record; this section is the review pass.

### Test isolation guard (real Application Support)

- A failed default-store path during Codec’s build had seeded `~/Library/Application Support/@teammate/desktop-native/teammates.json`. That file was left in place (removing it would be another out-of-folder mutation).
- Ported p12’s isolation stack: `scripts/isolated-test-runner.py` remaps `HOME` / `LOCUST_NATIVE_DATA` / `XDG_CONFIG_HOME` / `APPDATA` / `CODEX_HOME` / `TMPDIR` under a disposable `LOCUST_TEST_ROOT` and refuses `LOCUST_REAL_*`; `.cargo/config.toml` runs every `cargo test` binary through it; `src/test_isolation.rs` `require_temporary` is called from `Store::with_codex` and `engine::default_data_folder` under `#[cfg(test)]`.
- Crate-local Store fixtures (runtime `.m4-*` temps, approval channel-test, settings fingerprint, usage-windows) now use `std::env::temp_dir()` so they stay under the remapped root. Routines fixture likewise.

### Schedule maths

- `once` card labels use Electron’s fixed English day/month names and the raw `on` time slice (spring-forward gap keeps `02:30` in the string).
- `files` folder validation rejects drive-letter / absolute paths the way `watchedFolderValid` does (`C:/inbox`, `C:inbox`).
- Added New York DST wall-clock oracle (skip when the host is not EDT for that instant), closed-vs-open miss cases (`missed` uses process `opened_at`, not “now”), and `slot_passed_during` maths under `#[cfg(test)]` until completion reconciliation can record it on run end.

### Pause / resume and review holds

- `pause` takes an RFC3339 `at` and stores it as `resumedAt` on resume (Electron `setPaused`).
- `keep` clears execution, keeps the schedule, sets `lastRunAt` to the decision time; `acknowledge` remains abandon (schedule removed). UI offers Keep the schedule / Keep the routine plus Abandon behind the same review checkbox, with Electron’s keep/abandon explanation line.
- Uncertain / held progress still refuses `run` and `remove`; stale acknowledgement still fails; successful synthetic dispatch still ends held with no automatic replay.
- Automations poll passes `Panel.opened_at` (set at process start) into `record_misses`, matching Electron’s `appOpenedAt`.

### Left unchanged (still partial)

- Live scheduler tick, completion reconciliation / run counting, multi-step continuation, durable question receipts, capacity/busy handling, copy/input/handoff/check paths, Markdown import/export, starter templates, and the unported TS rows in the disposition table above.

### Verification this review

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p11 cargo test --release
# ×3 after last source edit → each: 464 passed; 0 failed; 13 ignored
# (isolated runner on every binary)

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p11 cargo build --release
# zero compiler warnings

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p11 cargo clippy --release --bin locust-native
# four pre-existing too_many_arguments findings only; no new findings in routines/schedule/isolation

python3 scripts/isolated-test-runner.py python3 scripts/test-m1-cli.py $HOME/work/scratch/native-target-p11/release/locust-native
python3 scripts/isolated-test-runner.py python3 scripts/test-m3a-cli.py $HOME/work/scratch/native-target-p11/release/locust-native
# synthetic CLI scenarios pass
```

- Dark/light routines scenes re-snapped at **900×900, scale 1** under `/Users/locust/work/scratch/p11-snaps/` (`routines-dark.png`, `routines-light.png`): Automations title, native-limitation line, paused/missed card and Edit/Resume/Remove/Run row are legible; no window opened.

Not verified: live window with Electron quit, real agent turns (`LOCUST_REAL_*`), live scheduler, Keep/Abandon in a live window, and the rest of the TypeScript suites listed under the disposition table.
