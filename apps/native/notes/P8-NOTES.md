# P8 — approvals and the command guard

P8 is **partial for Area 5**, with one composite row promoted to **done** (saved approval rules list/remove/remove all). Approvals must not auto-allow something Electron would ask about: compound commands, reach, secrets, outside paths, questions, and unreadable rule files all ask (or hard-deny via the guard). No milestone exit test is claimed. No window and no real turn were run.

## What changed

- `src/secrets.rs`: one shared module matching Electron `shared/secrets.ts` (`secret_in`, `scrub_secrets` with `[secret-shaped text removed]`, refusal sentences). `people.rs`, `ledger.rs`, and `runtime/events.rs` call it instead of per-module regex lists. Display/evidence still uses `redact_secrets` (`[redacted]`) from the same module.
- `src/approval_rules.rs` + `src/approval_rule_store.rs`: Electron `approval-rules.json` schemaVersion 1, seed-once from Electron via `Store::seeded_json`, list/add/remove/remove all/used. Secret-bearing patterns refused with Electron’s words. Cap 200. Unreadable never treated as empty.
- `src/command_reach.rs`, `src/command_plain.rs`, `src/command_guard.rs`: reach classifier, In short sentences, and hard-deny-by-name / Scout fix-copy publish refusals with Electron copy.
- `src/attach_outside.rs`, `src/workspace_settings.rs`: attachment destination/exclude and `settings_change_kept` (browser may change only grant-nothing settings).
- Approval card: In short, Reaches, “Yes, and don't ask again”, “Never allow this (saves a rule)”; Always/rule offers hidden when reach hits. Channel consults saved rules and the command guard before raising a card; `enforced_answer` downgrades Always on reach.
- People panel: Saved approvals tab with Electron lede/note/empty/remove/remove-all words.

Electron source was read under `/Users/locust/work/locust/apps/desktop/src` (and `resources/locust-command-guard.mjs`); no Electron source or data writes were made. Stay-out of Area 6 git/diff/changes code was observed.

## Verification

Commands run from `/Users/locust/Documents/Locust/native-review/p8/native`, with:

```
PATH=$HOME/.cargo/bin:$PATH
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p8
```

- `cargo test --release`, three runs: **383 passed, 0 failed, 13 ignored** each.
- `cargo build --release`: succeeds, **zero compiler warnings**.
- `cargo clippy --release --all-targets`: succeeds. Existing diagnostics remain (argument-count / test-file style). New modules add collapsible-if / too-many-arguments style findings; no new lint suppression was added beyond intentional `dead_code` on ported APIs awaiting further wiring.
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py`: synthetic CLI scenarios pass, including permission bridge.
- Headless PNG scenes in `/Users/locust/work/scratch/p8-snaps/` at `--size 900x900 --scale 1`: `approval`, `approval-light`, `approval-reach`, `approval-reach-light`, `p8-rules`, `p8-rules-light`. Inspected: In short on ordinary commands; Reaches + no Always/Don't-ask-again on `taskkill`; Saved approvals list/remove/remove-all chrome.

Ignored thirteen tests stay ignored (live MCP/ledger/Codex/Claude/runtime one-liners), per no-real-turn.

## TypeScript behavior carried over

| Electron test source (under desktop/src) | Rust coverage |
| --- | --- |
| `shared/approval-rules.test.ts` | Exact/prefix allow; compound never allowed; deny wins; outside paths ask; secret candidate refused |
| `main/a-command-with-a-key-is-never-saved-as-a-rule.test.ts` | No candidate; store refusal words; disk holds only ordinary rules |
| `main/every-saved-rule-can-go-at-once.test.ts`, `main/a-full-rule-store-keeps-every-rule.test.ts` | remove all; 201st refused; duplicate OK when full |
| `shared/a-command-that-reaches-other-programs-is-named.test.ts` | REACHES / DOES_NOT tables + said sentences |
| `shared/a-command-says-what-it-does-in-plain-words.test.ts` | Push/force/delete/download/looking/silence cases |
| `main/no-teammate-ends-your-browser.test.ts` (subset) | Chrome-by-name and fix-copy publish refusals |
| `main/a-secret-is-never-remembered.test.ts`, `main/a-scrub-replaces-what-is-shaped-like-a-key.test.ts`, `main/a-key-is-never-about-you.test.ts` (subset) | `secret_in` / `scrub_secrets` / refusal words |
| `main/attach-outside.test.ts` | Destination collision, traversal refuse, exclude line |
| `main/a-browser-changes-only-what-grants-nothing.test.ts` (subset) | `settings_change_kept` strips Auto/reach/swarm |

## Remaining work — not silently skipped

1. **Question form** on approval cards; connector/browser permission cards; Board/sidebar/taskbar/phone approval alerts; organization policy/spend.
2. **Command guard**: by-id process-table lookup against a live process list; full Claude/Codex/Copilot/OpenCode PreToolUse writer parity beyond the OpenCode-bundled JS asset and the Rust classifier used in-channel.
3. **Permission-host** outside summary for every tool; attach-outside picker wiring into the composer; agent-settings-folder confirmation UI beyond the kept-settings filter.
4. **Secrets**: Save-record / share export path should call `scrub_secrets`; full spoken/card shape matrix vs every Electron fixture; AWS lookaround shapes approximated without lookaround (Rust `regex` crate).
5. Live window with Electron quit, and `LOCUST_REAL_*` turns, were not run.

Because gaps remain, PARITY Area 5 is **1 done, 5 partial, 3 missing**, and the overall recount is **26 done, 24 partial, 103 missing, 153 total**.

## Not verified

Live window/exit, real runtime one-line turns, OS process-table by-id guard checks, full permission-host matrix, Save-record export scrubbing end-to-end.


## Review and fixes

Reviewed in `/Users/locust/Documents/Locust/native-review/p8/native`, against the Electron sources named in the review request (renderer components live under `src/renderer/src/components`), plus `shared/who-decides.ts` and `shared/approval-detail.ts`. Reference source and Electron data were not edited. This section supersedes earlier claims about placeholder APIs and lint suppression. Area 5 remains partial; no milestone exit is claimed.

Fixed:

- Session Always previously ran before the hard command guard and saved rules. Guard and saved deny/allow now run first. Questions never use session Always. An unreadable rule store blocks session Always rather than acting like an empty store.
- Native missions now register their teammate with the channel. Rule evaluation and card-created rules use that teammate and the request folder. Saving re-reads the pending request under the channel lock, refuses an ended/stale card or unknown teammate, and ignores stale window command text. This prevents a card's rule from silently becoming an any-teammate rule.
- Glob matching removes exactly one leading `./`, as Electron does. Repeated prefixes no longer broaden allow rules. Unicode path prefixes cannot panic at a byte boundary. Candidate length and stored usage-counter validation use JavaScript UTF-16 / safe-integer limits.
- Command normalization now uses JavaScript whitespace, rather than Rust's broader whitespace set. Secret word boundaries are ASCII as in JavaScript: a key directly after a Unicode letter is still refused. Compound command, reach, outside path, secret-pattern and deny-first behavior remain covered by the existing evaluator/classifier tests.
- Exact detail now uses Electron's 20,000-character cap, line-ending normalization and counted omission sentence. Claude file paths are no longer silently clipped at 600 characters before rule evaluation. Production `unwrap()` calls in the rule-store candidate construction were removed.
- Scout fix-copy detection now feeds the live in-channel guard instead of always passing false.

Removed unused APIs instead of retaining promises of future wiring: `attach_outside.rs`, `workspace_settings.rs`, export-only `Scrubbed`/`scrub_secrets`, the Electron/Node hook-settings generator, the unconstructed ruled Read action, and the unused Removing label. Read-kind rules still parse and display but no native request is falsely presented as a supported read action. All `allow(dead_code)` annotations are gone from `src`. Other unused suppressed APIs/fields were deleted; helpers actually exercised only by fixtures (including verdict/reach string helpers, legacy org fixtures, ledger convenience reads and ACP steering fixtures) compile only in tests. No runtime feature was newly claimed from them.

Test accounting, explicitly: deleted attachment helpers remove 11 synthetic tests, the absent browser-settings endpoint removes 2, and the absent export scrub path removes 1. These features remain missing, not tested or completed. Four new regression tests bring the previous 383 passing tests to 373. No test was silently ignored or converted into a live test.

Additional TypeScript correspondence:

| Electron source/test area | Review coverage / limit |
| --- | --- |
| `shared/who-decides.ts` | Channel regression: scoped saved denial and guard beat session Always; unreadable store raises a card. |
| `shared/approval-rules.test.ts` | Existing exact/prefix/compound/path/secret tests retained; added repeated-`./`, Unicode path, JavaScript whitespace and key-boundary regressions. |
| `main/an-approval-shows-the-whole-action.test.ts` | New detail normalization/counting and long Claude path regression; full tool-input/question matrix still partial. |
| `main/the-host-refuses-always-for-a-reaching-command.test.ts` | Existing host downgrade/refusal tests retained; card saving uses the host's pending request. |
| `main/attach-outside.test.ts` | Not applicable to a shipped native picker flow: placeholder API and its 11 tests deleted; outside attachment copying remains missing. |
| `main/a-browser-changes-only-what-grants-nothing.test.ts` | No native browser settings endpoint; placeholder filter and its 2 tests deleted. |
| `main/a-scrub-replaces-what-is-shaped-like-a-key.test.ts` | No native Save-record/share export flow; placeholder scrub API and its test deleted. Display/evidence redaction and secret refusal tests remain. |
| `main/command-guard.ts` hook writers / by-id process table | Still missing; the deleted unused Node hook generator did not implement these native runtime paths. In-channel guard tests do not prove guard coverage in Auto mode or where a runtime never asks. |

Final verification used the requested PATH, LOCUST_TEST_TS_READER and CARGO_TARGET_DIR, from this folder:

- `cargo test --release` three times: **373 passed, 0 failed, 13 ignored** each; TypeScript reader checks ran. Logs: `work/review/test-1.log`, `test-2.log`, `test-3.log`.
- `cargo build --release`: success, **zero compiler warnings** (`work/review/build-final.log`).
- `cargo clippy --release`: success; no findings in the new P8 modules. Four existing argument-count findings remain in `approval.rs::ask_claude`, `ledger_write.rs::create_runtime`, and `native.rs::command_for_turn` / `run_turn` (`work/review/clippy-final.log`).
- Both `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py` pass, including the permission bridge; logs in `work/review/`.
- Re-rendered and visually inspected all six 900×900, scale-1 PNGs under `/Users/locust/work/scratch/p8-snaps/`: ordinary approval and reach approval, dark/light; saved rules, dark/light. Text and buttons fit; reach cards omit Always and save-rule offers; saved-rule remove/reset controls remain readable.

Not verified/completed: live window/exit with Electron quit, the 13 ignored live tests, real runtime turns, OS process-table guard enforcement and hook installation across all runtimes, complete permission-host/outside summaries, question-answer UI, attachment copying, browser settings, and Save-record/share export. No claim of full Area 5 parity or a complete Electron test matrix.
