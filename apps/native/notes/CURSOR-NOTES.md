# Cursor Default-Model Guard & Connector Allow Rules (18 Nov 2026)

## What was built
- **`core/src/cursor_config.rs` & `core/src/cursor_config_tests.rs`**:
  - `CursorDefaultModel`: saves the three model fields (`model`, `selectedModel`, `modelSelectionHistory`) from `~/.cursor/cli-config.json` before a `--model` Cursor run and puts them back when it ends, preserving sign-in (`authInfo`) and user changes, handling overlapping runs, and recovering on next start via `kept_file` (`cursor-default-model.json`) if Locust quit mid-run.
  - `CursorRunGuard` & `recover_on_startup`: process-wide guards tying together `CursorDefaultModel`, `CursorConnectorKeeper`, and `CursorConnectorCache`.
- **`core/src/cursor_config_allow.rs` & `core/src/cursor_config_allow_tests.rs`**:
  - `merged_cursor_config`, `cursor_allow_rule`, `without_rules`, `AllowIo` (`FsAllowIo`), `HoldStore` (`FileHoldStore`), and `CursorConnectorKeeper`: outside Auto (`workspace-write`), merges `Mcp(<server>:*)` into `<workspace>/.cursor/cli.json`'s `permissions.allow` without touching `deny` or unreadable files; releases only the rules this run added (and deletes the file / `.cursor/` if created by us and now empty), with refcounting for overlapping runs and crash recovery via `cursor-connector-holds.json`.
  - `parse_cursor_mcp_list`, `cursor_connector_sentence`, `cursor_ready_connector_line`, and `CursorConnectorCache`: parses `cursor-agent mcp list`, warms in the background on startup, deduplicates concurrent refreshes, and answers from stale readings while refreshing in the background.
- **Minimal hooks in shared files**:
  - `core/src/native.rs`: one call at start (`let cursor_guard = crate::cursor_config::CursorRunGuard::begin(...)` right before `run_turn`) and one call at end (`cursor_guard.end()` right after `run_turn`).
  - `src/main.rs`: one call at startup (`locust_core::cursor_config::recover_on_startup(store.root())`).
  - `core/src/lib.rs`: added `pub mod cursor_config;` and `pub mod cursor_config_allow;`.

## Tests & Mutation Breaks
- 45 unit tests in `cursor_config::tests` (9) and `cursor_config_allow::tests` (36) covering all 7 Electron test files (`a-cursor-run-leaves-cursor-as-it-was`, `cursor-connector-allow`, `a-cursor-run-takes-back-the-rules-it-added`, `a-cursor-mission-gives-its-connector-rules-back-when-it-ends`, `a-cursor-turn-does-not-wait-for-its-connector-list`, `a-cursor-teammate-gets-its-skills`, `a-cursor-level-becomes-its-id-at-the-start`).
- 4 mutation breaks run via `~/work/scratch/maxlane/mutsafe.py`:
  1. `cursor_config.rs` (`if !named { ==> if false {`) -> killed by `leaves_it_alone_when_the_person_chose_another_model_themselves_during_the_run`
  2. `cursor_config.rs` (`Some(i) => model[..i]... ==> Some(_) => model...`) -> killed by `reads_a_model_with_bracketed_options_as_its_plain_id`, `matches_a_model_to_the_id_it_is_saved_under_and_nothing_else`, `with_runs_overlapping_puts_back_the_state_before_the_first_when_the_last_ends`
  3. `cursor_config_allow.rs` (`.filter(|v| v.as_str().is_none_or(|s| !gone.contains(s))) ==> .filter(|_| false)`) -> killed by `leaves_the_rules_that_were_already_in_the_file_and_its_own_other_settings`, `leaves_a_rule_the_person_added_while_the_run_was_going`, `does_not_take_back_a_connector_rule_the_person_had_already_allowed_though_the_run_asked_for_it_too`
  4. `cursor_config_allow.rs` (`if count > 0 { ==> if false {`) -> killed by `keeps_a_rule_until_the_last_of_two_overlapping_runs_ends`, `keeps_the_rule_when_the_run_that_added_it_ends_first_and_the_other_is_still_going`, `does_nothing_twice_when_the_run_is_released_twice`, `says_whether_a_live_run_holds_rules_in_a_folder_until_the_last_one_ends`

## Finished by the Mac chat (18 Nov, after the lane stopped on a malformed Hydrogen tool call)
- Test builds: `CursorRunGuard::begin` and `recover_on_startup` now return early under `cfg!(any(test, feature = "test-hooks"))`, not only `cfg!(test)`: the app crate's tests build locust-core with `test-hooks` and without `cfg(test)`, so a window test running a Cursor turn could have touched the real `~/.cursor`.
- `adds_connector_rules(runtime, mode)`: codex-mission.ts's `runtime === 'cursor' && effectiveSandbox === 'workspace-write'`, pulled out of the guard so a test can see it. My break (`if true`) had survived; `adds_connector_rules_only_for_a_cursor_edit_run` catches both breaks now.
- My breaks also caught: the rule's `:*`, the `Mcp(*:*)` already-allowed check, an unreadable file kept as it is.
