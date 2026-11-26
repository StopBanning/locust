# The port's tests on Linux (cloud lane "Linux 1", 26 Nov 2026)

Cloud lanes do all their work on Linux, where CI ran no test, and report tests that "fail here and nowhere else". This
is what fails here, found by running every test of both crates twice. No test was fixed and no Rust code changed.
`parity/linux-not-yet.txt` lists the failures and `scripts/linux-tests.sh` runs everything else (CI job `linux-tests`).

## How it was run

- Branch `cloud/linux-1` at `d59074cf` (cut from `exec/native-engine`), in a cloud container: Ubuntu 24.04, x86_64,
  4 CPUs, 15 GB, rustc/cargo 1.97.0, **running as root**, `/bin/sh` is dash.
- `apt-get install libasound2-dev libxkbcommon-dev` first. `CLAUDE_CONFIG_DIR` not set. `LOCUST_TEST_TS_READER` not
  set (the ledger tests skip the TypeScript reader's check without it, as they do anywhere it is unset).
- `cargo test --workspace --no-fail-fast` (debug, libtest, default thread count), twice, one after the other. Through
  `.cargo/config.toml`'s runner, `scripts/isolated-test-runner.py`, as on the Mac. cargo-nextest was not available
  here, so each test binary ran as one process, as the `mac` job runs it.
- The debug build of both crates' tests took 12 min; each run 9.5 min (core 2.2 min, native 7.2 min).

| Run | Passed | Failed | Ignored |
|-----|-------:|-------:|--------:|
| 1   | 10068 (core 7551, native 2517) | 19 (core 11, native 8) | 27 |
| 2   | 10069 (core 7553, native 2516) | 18 (core 9, native 9)  | 27 |

20 tests failed in at least one run. 3 of them (session_import) only for want of the `sqlite3` program, which this
container lacked and GitHub's ubuntu-24.04 image has; they are not on the list. The other 17 are.

## Each failing test

Labels: **Mac only by design** (it says so, or needs a macOS API), **flaky here** (passed in one of the two runs),
**fails here, cause not known**. Where the cause was found after all, it is said under the label.

### Mac only by design

- `chat_apps::tests::reads_a_name_and_a_colon_or_a_comma_or_an_at_in_any_case_and_without_its_accents` (both runs)
  `panicked at core/src/chat_apps.rs:274:9: assertion left == right failed; left: None; right: Some(AddressedTeammate { teammate_id: "t_zoe", rest: "hello" })`.
  `fold_nfkd` decomposes accents with `NSString::decomposedStringWithCompatibilityMapping` on macOS only; elsewhere
  the text is kept as it is, so "Zoë" never folds to "zoe".
- `chat_telegram_fmt::tests::names_each_teammate_by_a_command_telegram_allows` (both runs)
  `panicked at core/src/chat_telegram_fmt.rs:499:9: left: [.., "zo_ng_smith", ..] right: [.., "zoe_ng_smith", ..]`.
  The same `fold_nfkd`.
- `claude_cloud_panel::claude_cloud_panel_rows_tests::a_follow_up_on_this_computer_runs_claude_code_for_its_receipt` (both runs)
  `panicked at src/ui/claude_cloud_panel_rows_tests.rs:356:5: left: Some("Claude's cloud opens from here on Windows and macOS so far. Run claude --cloud in a terminal in this folder.") right: Some("Sent. Check again in a moment to read its reply.")`.
  The app says so itself (`core/src/claude_cloud.rs:689`).

### Fails here, cause not known (cause found: the container runs as root)

Both pass when the same test binary is run as `nobody` (`setpriv --reuid=65534`), and fail as root: a file or folder
set to mode 000 or 555 does not stop root. GitHub's runner is not root, so these would likely pass in CI; they are
listed because the lanes that run the script are root.

- `checkpoints::tests::unreadable_paths_are_skipped_not_fatal` (both runs)
  `panicked at core/src/checkpoints.rs:1159:9: assertion failed: !snap.files.contains_key("secret")`.
- `edit_earlier::tests::a_write_that_fails_puts_the_ones_already_written_back_as_they_were` (both runs)
  `panicked at core/src/edit_earlier_tests.rs:293:5: left: Array [String("a.txt"), String("sub/b.txt")] right: Array []`.

### Flaky here

- `history_cache::unreadable_count_tests::counts_a_file_whose_header_is_unreadable_and_only_that_file` (run 1 only)
  `panicked at core/src/unreadable_count_tests.rs:93:5: left: ["m_01"] right: ["m_02"]`.
  Alone: 3 of 5 passed as root, 4 of 5 as `nobody`.
- `ledger::fall_off_tests::still_finds_an_ancestor_off_the_end_of_the_list` (run 1 only)
  `panicked at core/src/fall_off_tests.rs:69:5: assertion failed: listed.iter().any(|s| s.mission_id == "mission_tip")`.
  Alone: 0 of 5 passed as root, 0 of 5 as `nobody` (one earlier single run as `nobody` passed).
- `agents_section::agents_artifacts_fold_tests::the_list_is_read_again_when_which_agents_are_there_changes` (run 2 only)
  `panicked at src/ui/agents_artifacts_fold_tests.rs:178:5: Codex CLI | 1.0.0 | Sign in again | No connectors, hooks, skills or agents set up | READY`.
  Alone: 5 of 5 passed.

The two history/ledger tests order files by modification time written 2 ms apart (`unreadable_count_tests.rs`), or set
by `set_modified` (`fall_off_tests.rs`); that is where to look first, not checked.

### Fails here, cause not known

- `history_cache::unreadable_count_tests::does_not_call_a_damaged_but_recovered_mission_unreadable_when_it_falls_off_the_page` (both runs)
  `panicked at core/src/unreadable_count_tests.rs:71:5: assertion failed: !page.iter().any(|id| id == "m_00")`.
  Failed in both whole runs, but alone it passed 2 of 5 as root and 2 of 5 as `nobody`: flaky in fact.
- `runtime::process::tests::stop_kills_a_grandchild_the_run_started_not_just_the_process_it_launched` (both runs)
  `panicked at core/src/runtime/process.rs:515:9: detached grandchild 3752 survived interrupt_process`.
  Alone: 0 of 5, as root and as `nobody`. Not zombies (none are left; PID 1 reaps). A guess, not checked: the
  grandchild is a `/bin/sh` subshell waiting on `sleep 3`, and `interrupt_process` sends it SIGINT only; dash (Linux's
  `/bin/sh`) may not act on it until `sleep` ends, where macOS's sh (bash) does.
- `phone_section::tests::the_routines_links_reach_the_relay_through_the_phones_own_link_and_its_notice_refreshes_them` (both runs)
  `panicked at src/ui/phone_section.rs:1713:10: called Result::unwrap() on an Err value: "Teammate id is invalid"`.
  Alone: 0 of 5. The routine names teammate `tm_a`, written to `electron/teammates.json` just before; `routines::save`
  does not find it in the team (`core/src/routines.rs:229-239`).
- The window tests that wait for a fake Codex, below.

### Window tests with a fake Codex: each alone, five times

Each run alone: `cargo test -p locust-native --bin locust-native -- --exact window_drive_tests::<name>`, one after
the other, nothing else running. A race is worth knowing about on any platform.

| Test (`window_drive_tests::`) | Whole runs | Alone, passed of 5 | Failure |
|---|---|---:|---|
| `a_copy_runs_changes_are_kept_into_the_folder_or_discarded` | failed both | 0 | `timed out waiting for its changes to wait: status "Routine \"Notes\" stopped at step 1 of 1: that run failed."` (alone it got one step further: "...to wait again") |
| `a_failing_check_shows_its_card_and_sends_back_to_fix` | failed both | 0 | `timed out waiting for failing check and fix turn: status "Ready"` |
| `a_note_on_a_changed_line_goes_with_the_next_message_and_shows_under_the_bubble` | failed both | 0 | `timed out waiting for the reply: status "Ready"` |
| `a_pointed_part_goes_with_the_next_message_after_the_words_and_leaves_the_box` | failed both | 1 | `timed out waiting for the reply: status "Ready"` |
| `a_turn_started_outside_the_window_shows_its_card_on_the_board_and_allow_once_finishes_it` | failed both | **5** | `timed out waiting for the card: status "Ready"` |
| `scout_fixes_settings_apply_and_skip_work_from_window_in_isolated_fix_root` | failed both | 0 | `fix became Unfixed: Wren's turn failed: Codex stdout closed before the turn completed (model_said=None)` |

All time out at `src/ui/window_drive_tests.rs:111` (the pump's 60 s wait) except scout (`:5938`). So:

- `a_turn_started_outside_the_window_...` is a race under load: it passes every time alone and fails every time among
  the other tests (cargo test runs the window tests four at a time here; nextest's `window` group would hold them to
  three).
- `a_pointed_part_...` is a race even alone (1 of 5).
- The other four never passed here: not a race, something about Linux or this container. Not looked into further.

## The two known ones

- `feedback_dialog::a_beta_reports_itself_tests::a_delete_is_remembered_and_written`, known to fail only inside a
  whole-package `cargo test` (process-global diagnostic log): **not seen**. It passed in both whole-workspace runs.
- `live2_claude_account_tests::new_claude_turn_without_teammate_uses_the_for_new_signed_in_account`, known to fail when
  `CLAUDE_CONFIG_DIR` is set: **seen when set, not otherwise**. Unset (as in both runs) it passes; run alone with
  `CLAUDE_CONFIG_DIR=/tmp/...` it fails: `panicked at core/src/live2_claude_account_tests.rs:181:5: assertion failed:
  logged[1]["CLAUDE_CONFIG_DIR"].is_null()`. The isolated runner replaces HOME but passes `CLAUDE_CONFIG_DIR` through.
  `scripts/linux-tests.sh` unsets it, so it is not on the list.

## Not on the list: the `sqlite3` program

- `session_import::tests::imports_cursor_and_opencode_as_resumable_on_account_default_and_refuses_duplicates`
- `session_import::tests::lists_huge_cursor_stores_from_metadata_and_keeps_missing_folders_visible`
- `session_import::tests::reads_cursor_metadata_and_ordered_messages_without_changing_store_or_opening_config`

Each failed both runs with `panicked at core/src/session_import_tests.rs:287:10: called Result::unwrap() on an Err
value: Os { code: 2, kind: NotFound, message: "No such file or directory" }`: the test builds its Cursor store with the
`sqlite3` command, which this container did not have. With `apt-get install sqlite3` all seven `session_import` tests
pass. `scripts/linux-tests.sh` stops at once, saying so, when `sqlite3` is missing; the CI job installs it.
