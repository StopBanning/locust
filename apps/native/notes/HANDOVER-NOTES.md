# Shared handover helpers parity notes

Carried three shared Electron test files into `apps/native/core/` on branch `exec/native-handover` with one Rust `#[test]` per TypeScript `it(...)` case and exact Electron names/values:

1. `apps/desktop/src/shared/handover.test.ts` (19 cases) -> `apps/native/core/src/protocol_tags.rs` (`41180c94`, also touching `core/src/briefing.rs` and `core/src/relay_tests.rs`).
2. `apps/desktop/src/shared/runtime-install.test.ts` (5 cases) -> `apps/native/core/src/agents.rs` and `core/src/agents_tests.rs` (`594ac4a3`, also wiring `FREE_START_RUNTIME` into `core/src/connectors_off.rs` and `src/ui/agents_section.rs`).
3. `apps/desktop/src/shared/kilo-models.test.ts` (5 cases) -> `apps/native/core/src/kilo_models.rs` and `core/src/lib.rs` (`4d81eeab`).

## Special-cases audit (`scripts/special-cases.py`)

- `apps/desktop/src/shared/handover.ts`: 0 special cases reported.
- `apps/desktop/src/shared/runtime-install.ts`: 2 special cases reported and carried in `agents.rs`:
  - `:161` (`kind !== 'npm'`): `install_command` returns `None` for `RuntimeInstall::Vendor(_)`.
  - `:172` (`kind === 'vendor'`): `install_sentence` formats the vendor install URL sentence plus optional sign-in note for `RuntimeInstall::Vendor(url)` instead of falling through to npm copy.
- `apps/desktop/src/shared/kilo-models.ts`: 2 special cases reported and carried in `kilo_models.rs`:
  - `:48` (`runtime !== 'kilo' || model !== 'account-default'`): `kilo_start_model` returns `None` for non-Kilo runtimes or explicit models.
  - `:49` (`entry.runtime === 'kilo' && entry.id === KILO_AUTO_FREE_MODEL`): `kilo_start_model` checks both runtime and ID on listed models (including a non-Kilo `opencode` entry case in `moves_a_new_conversation_off_the_account_default_once_auto_free_is_listed`).

## Break-check verification (`cargo test --offline -p locust-core`)

Every mutation was applied singly inside a temporary restore guard, tested against the full `locust-core` unit test suite, confirmed to fail, and reverted with a clean `git diff`.

### File 1 (`handover.test.ts` in `protocol_tags.rs`, `briefing.rs`, `relay.rs`)

- Verified before commit `41180c94`: mutating `MAX_FILES_PER_REPLY`, `MAX_FILE_NOTE_LENGTH`, `FILE_BLOCK_EXAMPLE_PATH`, `FILE_TAG`, the 400-character limit in `acceptable_file_path`, and `strip_file_blocks` inside `shown_text` each failed the core suite and restored cleanly.
- `relay::without_protocol_blocks` (`relay.rs:427` `strip_file_blocks` removed): failed `relay::tests::without_protocol_blocks_strips_every_tag_and_defangs_strays` (`1640 passed; 1 failed; 20 ignored`).

### File 2 (`runtime-install.test.ts` in `agents.rs`, `agents_tests.rs`, `connectors_off.rs`, `ui/agents_section.rs`)

| Mutation | Failed test(s) | Result |
| --- | --- | --- |
| `FREE_START_RUNTIME` `"opencode"` -> `"claude"` (`agents.rs`) | `agents::tests::knows_the_one_runtime_that_needs_no_account_and_that_the_others_do`, `connectors_off::tests::dropped_free_model_matches_electron_rules` | `1645 passed; 2 failed; 20 ignored` |
| `install_command` `RuntimeInstall::Vendor(url) => Some(url.to_owned())` (`agents.rs`) | `agents::tests::offers_no_command_for_the_ones_that_do_not_install_from_npm`, `agents::tests::install_and_sign_in_lines_are_electrons` | `1645 passed; 2 failed; 20 ignored` |
| `install_sentence` `RuntimeInstall::Vendor` branch fallthrough (`agents.rs`) | `agents::tests::says_what_to_do_next_not_only_what_is_missing`, `agents::tests::install_and_sign_in_lines_are_electrons`, `agents::tests::every_row_says_what_electrons_agents_screen_says` | `1644 passed; 3 failed; 20 ignored` |
| `sign_in_opened_line` omitting `.strip_prefix("then ")` (`agents.rs`) | `agents::tests::sign_in_opened_line_notes_in_window_account_switch_only_when_signing_in_again` | `1646 passed; 1 failed; 20 ignored` |

### File 3 (`kilo-models.test.ts` in `kilo_models.rs`)

| Mutation | Failed test(s) | Result |
| --- | --- | --- |
| `KILO_AUTO_FREE_MODEL` -> `"kilo/other/free"` (`kilo_models.rs`) | `kilo_models::tests::uses_the_same_auto_free_id_the_run_starts_on`, `kilo_models::tests::moves_a_new_conversation_off_the_account_default_once_auto_free_is_listed`, `kilo_models::tests::keeps_kilos_word_on_its_own_router_so_the_chip_never_says_a_bare_auto` | `1649 passed; 3 failed; 20 ignored` |
| `kilo_start_model` dropping `runtime != "kilo"` guard (`kilo_models.rs`) | `kilo_models::tests::moves_a_new_conversation_off_the_account_default_once_auto_free_is_listed` | `1651 passed; 1 failed; 20 ignored` |
| `kilo_start_model` dropping `model != "account-default"` guard (`kilo_models.rs`) | `kilo_models::tests::moves_a_new_conversation_off_the_account_default_once_auto_free_is_listed` | `1651 passed; 1 failed; 20 ignored` |
| `kilo_start_model` dropping `entry.runtime == "kilo"` check (`kilo_models.rs`) | `kilo_models::tests::moves_a_new_conversation_off_the_account_default_once_auto_free_is_listed` | `1651 passed; 1 failed; 20 ignored` |
| `kilo_model_description` free regex -> `(?i)^free$` (`kilo_models.rs`) | `kilo_models::tests::says_the_privacy_cost_on_auto_free_only` | `1651 passed; 1 failed; 20 ignored` |
| `kilo_model_name` skipping `^Kilo Gateway/` strip (`kilo_models.rs`) | `kilo_models::tests::names_a_model_without_the_gateway_every_kilo_row_would_repeat`, `kilo_models::tests::keeps_kilos_word_on_its_own_router_so_the_chip_never_says_a_bare_auto` | `1650 passed; 2 failed; 20 ignored` |
| `kilo_model_name` dropping `"Kilo "` prefix on `^kilo/kilo-` (`kilo_models.rs`) | `kilo_models::tests::keeps_kilos_word_on_its_own_router_so_the_chip_never_says_a_bare_auto` | `1651 passed; 1 failed; 20 ignored` |

## Judgement calls

- **1:1 test split without losing existing coverage**: In `protocol_tags.rs`, split `handover.test.ts` into its 19 named tests while keeping `shown_text_removes_handed_file_blocks_alongside_other_blocks` and `drops_a_path_over_four_hundred_characters` as explicit native extras (21 handover tests total).
- **`runtime-install` exports and wiring**: Exported `FREE_START_RUNTIME`, `RuntimeInstall`, `RuntimeInstallFacts`, `runtime_install_facts`, and `sign_in_opened_line` on `agents.rs`; replaced the duplicate `"opencode"` sort check in `src/ui/agents_section.rs` and re-exported `FREE_START_RUNTIME` in `core/src/connectors_off.rs` rather than leaving a private copy; kept `sign_in_opened_line_notes_in_window_account_switch_only_when_signing_in_again` in `agents_tests.rs`.
- **Standalone `kilo_models` module**: Kept `kilo_models.rs` self-contained against `crate::runtime::kilo::AUTO_FREE_MODEL` and `crate::invariants::regex` without wiring unused UI picker copy into `locust-native`, leaving future UI parity work decoupled.
- **No exclusions**: None of the 29 cases across the three TypeScript test files were marked `does-not-apply`.

## Final verification

- `python3 scripts/recount.py --check`: `159 carried, 62 does-not-apply, 190 partial, 898 waiting (1309 in all)`.
- `python3 scripts/gentle-cargo.py test --offline --release --workspace`:
  - `locust-core`: `1652 passed; 0 failed; 20 ignored; 0 measured; 0 filtered out` (up from `1621 passed; 0 failed; 20 ignored` before this work).
  - `locust-native`: `407 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out` — failed test is `window_drive_tests::a_note_on_a_changed_line_goes_with_the_next_message_and_shows_under_the_bubble` (`app.ts:1 | Not changed | const port = 3001 | Read it from PORT.`), which originates on base branch `exec/native-review-notes` (`9e5882d7`) and is unrelated to these handover modules.
- `python3 scripts/gentle-cargo.py build --offline --release`: succeeded (`Finished release profile [optimized] target(s) in 1m 28s`).
