# P6 — native people and memory

P6 is **partial, not an Area 3 parity sign-off**. The native editing/persistence and prompt paths below are implemented and synthetically tested. The remaining behavior and test gaps are explicit below; they are not classified as inapplicable and none of these composite rows is marked done. No milestone exit test is claimed.

## What changed

- `src/store.rs`: native `teammates.json` and `memories.json` seed once from Electron, read-only, when first needed. Subsequent reads and writes use native files. Missing source creates an empty schema-1 file; an unreadable file refuses changes. Temporary-file replacement uses mode 0600 and Store's directory checks. Unknown teammate fields/settings survive edits.
- `src/people.rs`: Electron-shaped teammate creation/edit/removal, trimmed and bounded names, duplicate-name refusals, standing instructions, route/runtime/model/effort/mode/folder; removal clears mission ownership. About you lives in `teammates.json.settings.aboutYou`; suggestions retain Electron's `id/text/by/at` shape and need the person's Add/Dismiss action.
- `src/ui/people_panel.rs`: sidebar entry opens the native editor, with Teammates and Memory tabs. The panel labels the copy shown. Native roster chrome also says “Native team copy” when the native engine is selected. A successful native edit selects the native roster/engine for subsequent work.
- Memory: named replacement preserves identity and previous text; ask-mode replacement proposes beside the kept memory; Keep applies it. Unnamed normalized duplicates do not promote proposals. Edit, enable/disable, remove, seven-day Recently forgotten and Restore are implemented. Rewrite/merge/retire proposals apply through a SHA-256 basis gate and stale proposals are dropped with Electron's refusal. The screen offers a manual rewrite proposal after selecting Edit.
- `src/native.rs`: all the native runtime branches receive one composed runtime prompt containing the selected teammate's saved instructions, About you, and applicable enabled kept memories. The ledger's user message remains the person's original prompt. Saved effort is used when the turn has no effort override. Workspace IDs use Electron's `ws_` + first 32 SHA-256 hex digits.
- After a native runtime returns, its latest assistant reply is inspected for `<locust-memory>` operations and `<locust-tidy>` proposals. Failed/running/stopped turns do not record memories. About-you suggestions are considered even with teammate memory off. Recording errors reach stderr.
- `src/ui/sidebar.rs`: Recent uses an inline `⋯` button on hover or selection, with `Actions for ${title}` as its accessible label, rather than a permanently visible “Actions” line. Space is reserved so hover does not grow the row.

Electron source was read under `/Users/locust/work/locust/apps/desktop/src`; no Electron source or data writes were made. The tests use synthetic stores and subprocess fixtures. There was no real runtime turn and no window launch.

## Verification

Final logs are retained in `p6-verification/`. The hover replay script is `p6-verification/recent-hover.json`.

Final commands run from `/Users/locust/Documents/Locust/native-review/p6/native`, with:

```
PATH=$HOME/.cargo/bin:$PATH
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p6
```

- `cargo test --release`, three runs: **338 passed, 0 failed, 13 ignored** each (351 total). Fourteen P6 tests were added; none of the new tests is ignored.
- `cargo build --release`: succeeds, **zero compiler warnings**.
- `cargo clippy --release --all-targets`: succeeds. Existing diagnostics remain: four production warnings (argument counts in approval, ledger writer and native command/run helpers), eighteen test-build warnings including those four duplicates. No warning points to new people/store/panel logic. No new lint suppression was added.
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py`: synthetic CLI scenarios pass, including cancellation and permission bridge cases.
- Headless PNG scenes in `/Users/locust/work/scratch/p6-snaps/`, at `--size 900x900 --scale 1`: people dark/light, memory dark/light, Recent dark/light and hover dark/light. Inspected images for legibility and layout; the hover capture also verifies that “Actions” is no longer rendered as a word.
- Existing thirteen ignored tests are the Electron MCP live-team check, real-ledger read, real Codex continuation, real Codex/Claude approval checks, and eight real runtime one-line checks (ACP, Antigravity, Copilot, Cursor, Kilo, Muse, OpenCode, Claude). They were intentionally not run, per the no-real-turn instruction.

The release tests initially exposed an absent `forgotten` field becoming JSON null through mutable indexing; that was fixed with optional field access. Runtime fixtures also caught a composed prompt being sent for a synthetic teammate absent from the roster; the empty-context path now preserves the original prompt.

## TypeScript behavior carried over

Matching Rust tests live in `src/people.rs::tests`; the pre-existing native roster tests continue to cover Electron roster parsing and read-only behavior.

| Electron test source (under desktop/src) | Rust coverage added |
| --- | --- |
| `main/teammate-store.test.ts`, `main/two-teammates-may-not-share-a-name.test.ts` | Trimmed, UTF-16-bounded names; duplicate refusal; persisted creation/edit; identity/route/folder preservation; removed ownership. |
| `main/a-teammate-has-its-own-instructions.test.ts`, `main/about-you-is-read-by-every-teammate.test.ts` | Saved instructions and About you reach the native brief; About you survives memory off; protocol tags in the note are disarmed. |
| `main/a-teammate-suggests-a-line-for-about-you.test.ts` | Suggestions do not change the person's existing note. |
| `main/a-named-memory-replaces-the-one-before-it.test.ts` | Same named fact replaces in place; previous text retained; ask mode waits for Keep; unnamed duplicate does not promote. |
| `main/memory-file.test.ts`, `main/memory-store.test.ts` | Torn native memory refuses mutation; native seeding occurs once; Electron bytes remain unchanged. |
| `main/a-forgotten-memory-can-come-back.test.ts` | Remove holds a kept memory for Restore, preserving ID. |
| `shared/memory.test.ts` | Fenced examples ignored; four-operation cap; named parser form. |
| Memory tidy behavior in `main/memory-store.test.ts` | Stale basis drops proposal; merge preserves first and bins rest. |
| `main/a-teammates-memory-is-chosen-by-what-was-asked.test.ts` | Only the **scope/enabled/kept filtering subset** is covered, not relevance scoring. |

## Remaining work — not silently skipped

The requested full TypeScript suite has **not** been ported test-for-test. The following are unresolved gaps, not “does not apply” exceptions:

1. **Teammates**: full avatar/hue/custom-title validation and UI, budget controls/enforcement, exact platform/runtime picker availability and account-route handling, missing/removed-record diagnostics, all role and folder edge cases. `NewTeammateDialog.tsx` is represented by editable text fields, not Electron's picker interactions. `InstructionsCard.tsx` interview-save receipts are absent; native interviews are outside this implementation. Existing optional fields are preserved, but preservation is not an implementation of their controls. Store limits/schema diagnostics need the remaining `teammate-store.test.ts` cases. Full secret recognition/refusal text from `shared/secrets.ts` is not ported; current guards cover common API key/private key shapes only.
2. **About you**: full `about-you-is-read-by-every-teammate`, `a-key-is-never-about-you`, `a-teammate-suggests-a-line-for-about-you`, `the-about-you-line-is-taught-once`, and `a-changed-memory-list-replaces-the-old-one` coverage remains. The editor lacks Electron's saved/removed notices and change-in-conversation replacement bookkeeping. Suggestions are stored and can be approved/dismissed, but their source/time display and all cap/secret cases are not complete.
3. **Memory store/parser**: parsed-memory field validation/deduplication, complete Unicode normalization and significant-word containment forget matching, complete ask-mode proposal deduplication/settling, expiry boundary/cap/duplicate restore cases, per-memory mission provenance, brief tracking and unused-memory indicators, clear-everything confirmation, relevance ranking and brief budgets, and every malformed/capped/fenced parser edge case remain. The screen's Add currently adds a global unnamed memory; folder/name selection controls are absent. The tests in `memory-store.test.ts`, `memory-file.test.ts`, `memory-reader.test.ts`, `forget-hits-one-memory.test.ts`, `a-named-memory-replaces-the-one-before-it.test.ts`, `a-forgotten-memory-can-come-back.test.ts`, `memory-says-how-old.test.ts`, `a-memory-nobody-is-given-says-so.test.ts`, `shared/memory.test.ts`, and `shared/a-rewritten-memory-is-new.test.ts` therefore have uncovered cases.
4. **Memory UI/turn receipts**: `MemoryCard.tsx` turn notices and renderer tests (`memory-card-survives-a-reply`, `a-forgotten-memory-shows-with-restore`, `a-memory-shows-what-it-said-before`, `a-memory-says-when-it-may-be-out-of-date`, `a-memory-nobody-is-given-says-so-on-screen`) are not fully carried over. There is no full Tidy up teammate picker/run flow or all merge/retire controls. Parsed tidy proposals can be kept or removed from the screen, with stale-basis protection.
5. **Runtime briefing**: this adds teammate instructions, the person's note and applicable kept memory to all native branches. It is not the entire Electron `workroom-briefing.ts` protocol: peer exchange, workroom briefs, reference folders, standing room/project instructions, semantic recall, skills and memory-file handover remain outside this change. The running slot is held through final memory processing; there is still no durable memory receipt/replay mechanism for a process crash. Recording refusals currently go to stderr, not a window card. No live end-to-end prompt or memory-write test was run.
6. **Recent**: headless hover paint inspected; native accessibility tree/keyboard focus and live pointer behavior have not been tested. The conditional button is present on hover or selected rows, with Electron's accessible name.

Because these gaps remain, PARITY Area 3 has **2 done, 3 partial, 9 missing**, and the overall recount is **25 done, 21 partial, 107 missing, 153 total**. M4 remains unreached; no window/real-turn milestone is claimed.

## Review and fixes

Parity pass against Electron (`NewTeammateDialog.tsx`, `InstructionsCard.tsx`, `main/teammate-store.ts`, `shared/teammate-instructions.ts`, `shared/about-you.ts`, `shared/memory.ts`, `main/workspace-brief.ts`, `MemoryScreen.tsx`, `MemoryCard.tsx`, `main/memory-store.ts`, and the per-runtime brief path in `main/index.ts`). Fixes landed in this copy only. No PARITY row was promoted to done.

### Electron folder stays read-only

- Added `every_store_operation_leaves_electron_teammates_and_memory_bytes_unchanged`: fingerprints a fake Electron folder (teammates.json, memories.json, sentinel) before and after seed, create/edit/remove teammate, About you settings, add/edit/tidy/remove/restore memory, and `record_reply`. Bytes, sizes and relative paths must match.
- Existing seed-once coverage remains; Store still refuses writes whose resolved path is inside Electron’s data folder.

### Runtime brief words match Electron

- Native `people::brief` was a shortened paraphrase. It now composes Electron’s section builders for the P6 subset: `whereSection`, `teammateSection`, `aboutYouSection`, and `memorySection` (folder notes, standing instructions, About you teaching form, memory list + rules + write block, `BLOCK_PLACEMENT`, age words, protocol defang of tag openers only).
- Ledger user message remains the person’s bare prompt; only the runtime command receives the composed brief.
- Covered by `brief_matches_electron_section_words_byte_for_byte` (synthetic teammate, fixed clock). Workroom peer identity, LOCUST.md, group instructions, keys, semantic recall, and `.locust/memory.md` handover remain outside this round (still listed under Remaining work).

### Validation and refusal words

- About you / memory / instructions secret refusals now use Electron’s sentences from `shared/secrets.ts` and `InstructionsCard.tsx` (`aboutYouSecretRefusal`, `secretRefusal`, instructions “They hold … Take it out first.” with the curly apostrophe), for the common key shapes native already detects.
- Covered by `secret_refusal_words_match_electron`. Full `secretIn` shape matrix is still not ported.

### Fixture follow-through

- `scripts/fixtures/m1-app-server.py` and `m3-claude.py` accept a briefed turn whose text ends with the synthetic person prompt, matching ledger-vs-runtime split.

### Left unchanged (still partial)

- Remaining work items 1–6 above (avatar/budget/UI pickers, full TS memory suites, MemoryCard receipts, workroom briefing, live pointer/a11y).
- Live clipboard/window/exit and real runtime one-line turns were not run (13 ignored tests stay ignored).

### Verification this review

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p6 cargo test --release
# ×3 after last source edit → each: 341 passed; 0 failed; 13 ignored

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p6 cargo build --release
# zero compiler warnings

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p6 cargo clippy --release --all-targets
# existing findings only; no finding names new people/store brief logic

scripts/test-m1-cli.py /Users/locust/work/scratch/native-target-p6/release/locust-native
scripts/test-m3a-cli.py /Users/locust/work/scratch/native-target-p6/release/locust-native
# synthetic CLI scenarios pass, including cancellation and the permission bridge
```

- Eight scenes re-snapped and inspected at 900×900, scale 1 under `/Users/locust/work/scratch/p6-snaps/`: people dark/light, memory dark/light, Recent dark/light, Recent hover dark/light. Hover shows the `⋯` control and does not paint the word “Actions”. Logs in `p6-verification/`.

Not verified: live window with Electron quit, real runtime turns (`LOCUST_REAL_*`), OS pointer hover/focus a11y tree, full secret-shape matrix, workroom/LOCUST.md/memory-file briefing, and the rest of the TypeScript suites listed under Remaining work.
