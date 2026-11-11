# P4: Conversation organization (parity with Electron)

Native sidebar organization, in-thread find, cross-conversation search, command palette, and beside view in this copy. No window for real turns; no live approve/exit. Area 1 / Area 13 rows closed below are the ones this piece owns.

## Changes by file and TypeScript ports

| File | Change and TypeScript reference |
| --- | --- |
| `src/ui/conversation_list.rs` | `conversationList.ts`: `conversation_keys`, `held_for`, `by_live_then_recent`, `conversation_rows`, `narrowing_line`, `conversation_title_for`, `unreadable_sentence`, `pinned_and_recent`. |
| `src/ui/settled.rs` | `settled.ts`: `SettledEntry`, `is_put_away`, `snooze_choices` (Electron snooze labels). |
| `src/ui/conversation_search.rs` | `status.ts` `missionsMatching` (title, id, `said`, teammate name). |
| `src/ui/thread_find.rs` | `threadFind.ts`: `find_in`, `find_count_words`, `step_hit`, `MAX_FIND_HITS=2000`. |
| `src/ui/thread_find_bar.rs` | `ThreadFind.tsx` words: Find in this conversation / No matches / N of M / Previous match / Next match / Close find / Also search the N earlier turn(s). |
| `src/ui/org.rs` | `main/teammate-store.ts` + `main/group-store.ts` shapes: read Electron `teammates.json` / `groups.json` read-only; write native copies only via `Store`. Pin / rename / settle / snooze / group assign / create group. |
| `src/store.rs` | `teammates_json_path`, `groups_json_path` (native root only). |
| `src/ui/sidebar.rs` | Pinned / Recents|Ungrouped / project sections, Search conversations, Actions menu (Open beside, Rename, Pin to top/Unpin, Settle/Bring back, Snooze, Move to project), rename field. |
| `src/ui/command_palette.rs` | `CommandPalette.tsx` filter/group order + Electron action words; Cmd/Ctrl+K. |
| `src/ui/beside.rs` | `BesideConversation.tsx` chrome: Beside: … / Open it here / Close; no composer. |
| `src/ui/app_state.rs`, `src/main.rs`, `src/ui/scenes.rs`, `src/ui/composer_wiring.rs` | State, keys (Cmd/Ctrl+F/K, Escape clears find/palette/search), view wiring, org scenes, load org with composer store. |

## TypeScript tests ported or skipped

| TS test | Rust | Notes |
| --- | --- | --- |
| `a-conversation-can-be-pinned.test.tsx` | `conversation_list::a_conversation_can_be_pinned_order` | Pinned newest-first, Recents rest, no sections when empty. |
| `main/a-conversation-can-be-pinned.test.ts` | `org::a_conversation_can_be_pinned_persists`, `pin_rejects_bad_id_and_caps_at_fifty`, `deleted_conversation_clears_pin_and_settled` | Cap 50, Electron words on refuse. |
| `a-conversation-can-be-put-away.test.tsx` | `settled::a_conversation_can_be_put_away`, `snooze_times_offered` + sidebar hide via `sidebar_missions_now` | Settle/snooze/expiry/activity; snooze labels. |
| `main/a-conversation-can-be-put-away.test.ts` | `org::a_conversation_can_be_put_away_persists` | Persist settle/snooze/`until`; clear; bad id/time. |
| `a-conversation-keeps-its-name-and-group.test.ts` | `conversation_list::a_conversation_keeps_its_name_and_group` | Keys / heldFor / title across turn ids. Skipped: App.tsx raw string assert (no TS App in native). |
| `a-conversation-can-be-searched.test.ts` | `thread_find::a_conversation_can_be_searched` | findIn / count words / stepHit. |
| `search-finds-every-turn.test.tsx` (missionsMatching) | `conversation_search::a_sidebar_search_finds_title_id_said_and_teammate` | Title/id/said/teammate. Skipped: Escape/Ctrl-K DOM wiring (covered by native `on_key`). |
| `a-conversation-opens-beside-another.test.tsx` | `beside::a_conversation_opens_beside_another_words` | Chrome words + never-duplicate-middle guard. Skipped: App.tsx panel priority vs file/review/inspector (those panels are not in this piece). |
| `main/a-conversation-can-be-named.test.ts` | exercised via `org::rename_mission` in store path | Titles only in native `teammates.json` overlay. |
| `main/groups-hold-conversations.test.ts` | partial via `create_group` / `assign_to_group` | Full instructions/route/left[] history and Project settings dialog not ported. |
| `main/groups-remember-leaving.test.ts` | **skipped** | `left[]` leave-briefing lines need thread join/leave UI (not this piece). |
| `groups-unreadable-is-a-line.test.tsx` | words in sidebar when `groups_unreadable` | Same Electron sentences. |
| `group-settings-dialog.test.tsx` / `group-boundary.test.ts` / `group-leaving-line.test.ts` / `group-join-line-survives-leaving.test.ts` | **skipped** | Project settings dialog and briefing boundary lines are out of this piece’s ask. |
| `main/palette-has-no-inner-ring.test.ts` | **skipped** | CSS-only; not org behaviour. |

## Persistence

- **Read (Electron, read-only):** `{userData}/teammates.json` fields `missionTitles`, `missionPins`, `missionSettled`; `{userData}/groups.json` groups + members.
- **Write (native only):** same filenames under `LOCUST_NATIVE_DATA` via `store::Store::teammates_json_path` / `groups_json_path`. Electron’s files are never written; a byte-identical check is in `org::electron_pins_show_until_native_writes`.

## Scenes / snaps

Snapped to `/Users/locust/work/scratch/p4-snaps/` at `--size 900x900 --scale 1`, dark + light:

- `org-sidebar` — Pinned / Ungrouped / project Locust, Search conversations
- `org-search` — narrowing line `"deploy" — 1 of 4`
- `org-find` — Find in this conversation, `1 of 2`, Previous/Next/Close find
- `org-palette` — Command palette, Electron placeholder and groups
- `org-beside` — Beside: …, Open it here, Close, no composer

## Verification

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p4 cargo test --release
# ×3 → each: 301 passed; 0 failed; 13 ignored

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p4 cargo build --release
# zero warnings

python3 scripts/test-m1-cli.py $HOME/work/scratch/native-target-p4/release/locust-native
python3 scripts/test-m3a-cli.py $HOME/work/scratch/native-target-p4/release/locust-native
```

## Unconfirmed / remaining

- Live window: Cmd/Ctrl+F against a streaming ledger, Cmd/Ctrl+K run actions that navigate screens not yet built.
- Full Delete / Delete for good? flow and trash; Project settings… dialog; groups `left[]` briefing lines in the thread.
- Rooms folded into the sidebar list (`withRoomsFolded`) and the compact rail.
- CSS Highlight-style match painting inside Markdown (Rune has no Highlight API); find counts and steps are real, highlights are not painted on glyphs.
- Prompt rail (listed in the suggested round table) was not in this ask and stays missing.

## Review and fixes

Reviewed by Codec, 10 November 2026. The earlier 301-test verification above describes the incoming implementation, not this review. **P4 still has parity gaps; this review does not close all of Area 1 / Area 13.** Reference source was read from `/Users/locust/work/locust/apps/desktop/src`; it was not edited. There is no `renderer/src/conversationSearch.ts` in that checkout: sidebar matching is in `status.ts`, and find is in `threadFind.ts` / `components/ThreadFind.tsx`.

### Fixed

- **Cleared imports returned after restart.** Native organization maps now supersede imported maps instead of merging deleted names, settled marks, and memberships back from Electron. Tests clear imported values, reload, and compare Electron's source bytes.
- **Organization saves discarded unrelated data.** Writes now retain the existing native roster, owners, settings, group routes, historical leaves, and unchanged membership timestamps; first writes seed these from the read-only Electron file. Moving out of a group records its name, instructions, original join moment and departure time, retaining the last eight leaves. All mutation helpers commit UI state only after a successful save. Unreadable native files are reported and refused rather than overwritten. Tests exercise both file types and failed writes.
- **Conversation identity was only a leaf ID.** Local ledger summaries now retain parent IDs and full prompt words. Sidebar rows collapse chains, use the root title and newest leaf state, keep all identities, and search all turn prompts. The UI no longer requests only twelve local turns. Pin, rename, settle and project actions write a stable root ID; clearing an old turn's marks clears its aliases. The opener uses the same chosen title as the sidebar. Tests exercise a three-turn chain, fallback title/group lookup, second-turn-only search, teammate search, cycle termination and clearing old identities.
- **Pinned project rows appeared twice and section headings were misplaced.** Project lists now take only unpinned rows, and Ungrouped/Recents headings precede their actual rows after projects. The actual sidebar chain order is tested; refreshed scenes were inspected.
- **Snooze and Move to project silently selected a default.** The menu now exposes each snooze time, each project, Ungrouped, and New project…; naming a new project precedes creation. No implicit “Ungrouped project” is created. Native name cleaning normalizes whitespace, IDs accept Electron's 128-character bound, and title limits count UTF-16 units. Refusal words for pin IDs, pin capacity, renamed-conversation capacity and group operations were corrected.
- **Palette rows and beside navigation did not execute.** Palette conversation selection and Open it here now call the sidebar's real opener instead of setting an asynchronous request tag. Stop calls the existing stop action. Commands for absent screens are no longer advertised. Up/Down wrap through the grouped palette; Enter executes; find Enter/Shift+Enter step; the inputs have key handlers and shortcut focus. Beside loads the chain's user/assistant text instead of opening empty. Navigation clears stale find state.
- **Find offsets broke after Unicode case expansion.** Literal Unicode case-insensitive matching now searches original text and reports UTF-16 offsets, matching the TS representation. Added tests cover astral text, accents, Greek sigma, Kelvin sign and dotted capital I. Find counts refresh from current conversation text, and a changed query resets the selection.
- Removed P4 clippy findings (unit bindings, redundant closure, nested conditions, complex tuple spelling and error cleanup). No lint suppression was added for new findings.

### TypeScript coverage corrections and remaining unported cases

The original test table remains a record of the incoming port. Its claims that all DOM wiring was covered, and that group persistence was complete, were too strong. Eight review regressions were added: four persistence tests, one Unicode find test, two actual sidebar/key-handler tests, and one identity-clearing test. These are in `org.rs`, `thread_find.rs` and `sidebar.rs`.

| TS file / cases not fully ported | Reason / remaining boundary |
| --- | --- |
| `a-conversation-can-be-pinned.test.tsx` | Helper ordering and actual unpinned group input are present; React markup, context-menu placement and interactions are not reproduced as DOM tests. No real-window pin interaction was run. |
| `a-conversation-can-be-put-away.test.tsx` | Timing/filter primitives are ported; the full React rendering and real-clock wakeup interaction are not independently driven. |
| `a-conversation-keeps-its-name-and-group.test.ts` | Chain lookup is now tested through real native rows. Raw `App.tsx` wiring assertions have no native TS counterpart. |
| `search-finds-every-turn.test.tsx` | Prompt aggregation now reaches the actual sidebar. Escape/palette keys are tested in native handlers; browser focus/DOM propagation and live window typing remain unverified. Palette teammate keywords still need full wiring beyond sidebar matching. |
| `a-conversation-can-be-searched.test.ts` | Pure find/count/step cases are ported. Markdown glyph highlighting, active-hit scrolling, initial visible-hit choice, and earlier-turn expansion are still absent. Native search text is not a DOM text-node walk. |
| `a-conversation-opens-beside-another.test.tsx` | Chrome words and middle-ID guard are ported; live stream adoption, rich ledger rendering, and priority against review/file/inspector panels remain absent. Static scenes do not prove live beside behavior. |
| `missions-lists-conversations.test.tsx` | Chain grouping/root/leaf behavior is covered through native sidebar rows. A Missions screen, its count/age/turn metadata, and whole-chain deletion are not implemented in this round. |
| `the-flat-sidebar.test.ts` | Native ordering helpers and chain rows cover part of the list contract. Face filter, nested relayed conversations, compact flyout and all TS presentation assertions remain unported. |
| `a-teammate-who-just-finished-last-worked-just-now.test.ts` | `liveRunLastAt` is not separately ported; native local summaries use ledger event times. Live teammate-card age presentation is outside this implementation. |
| `a-routine-run-is-marked.test.tsx` | Routine attribution/badges are not wired in this round. |
| `a-room-stays-in-the-sidebar.test.ts` | `withRoomsFolded` / `roomLastAt` and empty-room placement are not implemented. |
| `a-long-list-is-drawn-as-far-as-it-is-read.test.tsx` | The twelve-turn request cap was removed, but TS incremental list-window drawing is not implemented. Large histories have not been performance-tested. |
| `groups-unreadable-is-a-line.test.tsx` | Exact sentences and combined unreadable-store reporting are present; React markup checks do not apply. Native refusal is regression-tested. |
| `main/a-conversation-can-be-named.test.ts` | Native clear/reload, old turn IDs, preservation and UTF-16 bounds are tested; Electron IPC integration assertions are not replicated. A TS slice can retain a lone UTF-16 surrogate at the boundary; Rust keeps a valid Unicode prefix instead. |
| `main/groups-hold-conversations.test.ts` | Core create/rename/assign/ungroup persistence and route preservation are tested. Full route/instructions editing and validation, menu checked state, newest duplicate-name matching, project folding and project settings are not ported. Membership reads are not a complete port of the TS sanitizer. |
| `main/groups-remember-leaving.test.ts` | Departure preservation and eight-entry retention are implemented; exhaustive leave/move/remove/bound tests and exact same-moment join/leave parity remain unported. |
| `group-settings-dialog.test.tsx`, `group-boundary.test.ts`, `group-leaving-line.test.ts`, `group-join-line-survives-leaving.test.ts` | Project settings and thread briefing boundary rendering remain absent. Preserving the records does not implement their UI. |
| `main/palette-has-no-inner-ring.test.ts` | Electron CSS-only assertions; native palette was visually inspected instead. |
| `main/a-saved-record-says-only-what-the-ledger-holds.test.ts` | Export/save-record behavior is not part of this port. The inert Save the record… palette entry was removed. |

Other tests importing `Sidebar.tsx` concern roster cards, runtime marks, calls, routines, boards, compact rail, pets or CSS; those broader surfaces are not claimed as ported by P4. Likewise, importing `teammate-store.ts` does not make its unrelated credential, routing, template, spending or personal-instruction tests organization tests.

### Verification and limits

Final results and snapshot inspection are recorded below. Logs are retained under `review-artifacts/`. Refreshed PNGs are under `review-artifacts/snaps/` to honor this turn's restriction to write only inside `/Users/locust/Documents/Locust/native-review/p4/native`; the supplied `/Users/locust/work/scratch/p4-snaps/` remains the prior snapshot reference.

All five scenes were rendered in dark and light at 900×900 / scale 1 and visually inspected. The palette now contains only executable commands; find shows its count and controls; beside retains a separate readable column without a composer. These are static scene checks, not a live interaction claim. The inherited find scene uses synthetic words distinct from the visible example conversation and cannot prove highlighting.

Live window/exit with Electron quit, live find during streaming, highlight painting/scrolling, full Delete / Delete for good? / trash, project settings, room folding and compact rail remain unverified or unimplemented as distinguished above. Tests use synthetic Electron directories for byte comparisons; this is not a whole-profile live filesystem audit. No reference project files or real Electron data files were intentionally written.

Final verification (after the last source edit):

- `PATH=$HOME/.cargo/bin:$PATH LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p4 cargo test --release` ran **three times**: **309 passed, 0 failed, 13 ignored** each. Complete output: `review-artifacts/test-1.log`, `test-2.log`, `test-3.log`. No tests were disabled or newly ignored during review.
- `cargo build --release` with the same PATH/target directory: exit 0, **zero warnings** (`review-artifacts/build.log`).
- `cargo clippy --release`: exit 0, **no new findings**. Four remaining `too_many_arguments` findings are in unchanged `approval.rs::ask_claude`, `ledger_write.rs::create_runtime`, `native.rs::command_for_turn`, and `native.rs::run_turn`; there are none in the reviewed organization modules (`review-artifacts/clippy.log`). This is not a claim of globally warning-free clippy.
- Both requested CLI scripts exited 0: `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py`, using `/Users/locust/work/scratch/native-target-p4/release/locust-native`. Logs: `review-artifacts/m1.log`, `m3a.log`.
- The final binary regenerated all ten snapshots successfully. Their SHA-256 hashes were identical to the ten images visually inspected during this review. Reproduction scripts are `review-artifacts/review_checks.py` and `review-artifacts/review_snaps.py`.
- The 13 existing ignored tests require live services/credentials/history: real team listing, real ledger reading, Codex continuation, Codex and Claude approval turns, and one-line turns for ACP, Antigravity, Copilot, Cursor, Kilo, Muse, OpenCode and Claude. They were not run. Synthetic TypeScript reader checks did run using the requested `LOCUST_TEST_TS_READER`.
