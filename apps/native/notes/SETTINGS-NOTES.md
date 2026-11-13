# Saved appearance, startup/keep-awake, and feedback/problem reporting, in Rust (13 Nov 2026)

Three PARITY rows on `exec/native-settings` (from `exec/native-engine`):
1. **Saved appearance choice and window look settings** (area 14)
2. **Startup/login item, keep running/tray and keep awake** (area 13)
3. **Feedback and private report export/send controls** (area 11)

## What was built

1. **Appearance & Look (`core/src/workspace_settings.rs`, `src/ui/settings_panel.rs`, `src/ui/composer_wiring.rs`)**
   - `LookChoice` (`Standard`, `Liquid`), `GlassGround` (`Desktop`, `Rainbow`), `AppearancePrefs`, `load_appearance`, `save_appearance`, `load_appearance_for_platform`.
   - `first_theme(platform, profile_is_new)`, `resolved_theme`, `window_ground`, `theme_from_arguments`, `liquid_from_arguments`, `look_lede`, `ground_lede`.
   - Settings > Appearance renders the Theme segmented control (with `settings-theme-dark`, `settings-theme-light`, `settings-theme-system`) and, when `liquid_offered` is `true`, renders the `Look` setting and `Behind the glass` sub-setting.
   - `composer_wiring::load_composer_store` restores saved `theme`, `look`, `glass_ground`, `keep_running`, and `stay_awake` on startup.

2. **Login item, keep running & keep awake (`core/src/login_item.rs`, `core/src/keep_awake.rs`)**
   - `LoginItem` with `LoginItemHost` seam: handles Windows (`--background` query) vs macOS (`SMAppService` no-query, `enabled` / `requires-approval` status), and refuses development builds or unsupported platforms.
   - `read_keep_running` / `write_keep_running` (`background.json`) and `read_stay_awake` / `write_stay_awake` (`awake.json`) with atomic `.tmp` + rename writes.
   - `KeepAwake<B: PowerBlocker>` state machine: acquires one hold on `live_runs > 0 || standing`, releases when neither holds, retries on `start()` failure, and releases permanently on `dispose()`.
   - Settings > General > `Startup` section (`src/ui/quit_guard_dialog.rs`) renders the three controls.

3. **Quit guard (`core/src/quit_guard.rs`, `src/ui/quit_guard_dialog.rs`)**
   - `should_ask_before_closing`, `hide_to_tray_on_close`, `close_question`, `CLOSE_BUTTONS`, `tray_menu_labels`, `tray_line`.
   - `quit_guard_dialog::request_close` checks live runs and opens the Quit Guard modal (`"Keep working in the background"`, `"Quit anyway"`, `"Cancel"`).

4. **Feedback & problem report (`core/src/report_problem.rs`, `src/ui/feedback_dialog.rs`)**
   - `SUPPORT_ADDRESS`, `REPORT_DESTINATION`, `MAX_REPORT_URL`, `MAX_MAILTO_URL`, `MAX_DESCRIPTION`, `system_name`, `windows_name`, `report_claim`, `report_subject`, `report_file_text`, `support_mailto_url`, `feedback_url`, `conversation_text`, plus diagnostic log helpers (`should_roll`, `diagnostic_line`, `startup_detail`).
   - All report builders scrub `description` and `conversation` through `locust_core::secrets::scrub_secrets` before building a URL, mailto, or file.
   - `FeedbackDialog` modal and Settings > General > `Report a problem` section (`Show the log`, `Send feedback`).

## Special cases and the test that fails without each

- **`appearance.ts:35` (new profile on Linux starts on `system`; elsewhere `None`):** `workspace_settings::tests::first_theme_starts_system_only_on_new_linux_profile` fails if `first_theme` returns `None` on Linux or `Some` on Windows/Mac/existing profiles.
- **`login-item.ts:53, 59` (only Windows and Mac register, Mac uses its own no-query path):** `login_item::tests::does_not_register_development_build_or_unsupported_platform` and `login_item::tests::registers_app_itself_on_mac_and_reports_requires_approval` fail if `registers` or `is_mac` is wrong.
- **`login-item.ts:67` (Mac `"requires-approval"` state):** `login_item::tests::registers_app_itself_on_mac_and_reports_requires_approval` fails if `approval` is not set when `status == Some("requires-approval")`.
- **Login item registers only packaged app:** `login_item::tests::does_not_register_development_build_or_unsupported_platform` fails if `packaged == false` calls `set_login_item_settings`.
- **Quit guard asks while turns run:** `quit_guard::tests::asks_only_when_someone_is_working_and_person_is_closing` and `window_drive_tests::quitting_with_a_turn_running_says_so_first` fail if the guard stops asking when a teammate is busy.
- **Report never sends without press, says what it sends, and scrubs secrets:** `report_problem::tests::report_scrubs_secrets_before_leaving` fails if `scrub_secrets` is removed; `window_drive_tests::report_shows_what_it_sends_and_sends_nothing_until_pressed` fails if `last_sent` is populated before clicking Send or if secrets leak into the URL.

## TypeScript tests ported

- `main/a-theme-is-the-persons-choice.test.ts` -> `core/src/workspace_settings.rs` (`saved_theme_defaults_and_reads_persons_choice`, `first_theme_starts_system_only_on_new_linux_profile`, `theme_resolution_window_ground_and_cli_args`) + `window_drive_tests::choosing_a_theme_is_kept_after_restart`
- `renderer/src/the-theme-control-says-what-it-chose.test.tsx` -> `core/src/workspace_settings.rs` (`theme_and_look_ledes_match_electron`) + `window_drive_tests::choosing_a_theme_is_kept_after_restart`
- `renderer/src/the-look-is-offered-only-where-it-can-be-drawn.test.tsx` -> `core/src/workspace_settings.rs` (`theme_and_look_ledes_match_electron`) + `settings_panel.rs` (`liquid_offered` gate)
- `main/a-login-item-registers-only-the-packaged-app.test.ts` -> `core/src/login_item.rs` (`sets_background_on_packaged_windows_executable_and_reads_back`, `does_not_register_development_build_or_unsupported_platform`, `registers_app_itself_on_mac_and_reports_requires_approval`)
- `main/quit-guard.test.ts` -> `core/src/quit_guard.rs` + `window_drive_tests::quitting_with_a_turn_running_says_so_first`
- `main/keep-awake.test.ts` -> `core/src/keep_awake.rs` (`holds_on_first_live_run_once_and_releases_when_none`, `dispose_releases_for_good`, `failure_to_start_leaves_unheld_and_retries_next_beat`)
- `main/a-plugged-in-computer-stays-awake-for-what-is-scheduled.test.ts` -> `core/src/keep_awake.rs` (`plugged_in_standing_holds_without_runs_and_releases_on_battery`, `stay_awake_and_keep_running_files_default_false_and_persist`)
- `main/report-problem.test.ts` -> `core/src/report_problem.rs` (`opens_single_destination_and_carries_words_facts_and_conversation`, `keeps_end_of_long_conversation_and_bounds_url_length`, `names_windows_11_and_macos_correctly`)
- `main/a-report-can-reach-support-by-email-or-file.test.ts` -> `core/src/report_problem.rs` (`support_email_and_file_work_and_keep_uncut_conversation_in_file`)
- `main/a-beta-can-report-itself.test.ts` -> `core/src/report_problem.rs` (`diagnostic_log_and_conversation_text_helpers_match_electron`) + Settings > General > `Report a problem`
- `renderer/src/feedback-says-what-it-sends.test.tsx` -> `core/src/report_problem.rs` + `window_drive_tests::report_shows_what_it_sends_and_sends_nothing_until_pressed`

## What was left out or not wired

- **macOS `SMAppService` / Windows registry:** `objc2-service-management` is not in `Cargo.toml`; real OS login-item registration is behind the `LoginItemHost` trait (`UnwiredLoginItemHost` at runtime).
- **macOS `IOPMAssertion` / `caffeinate`:** real OS power assertion is behind the `PowerBlocker` trait (`UnwiredPowerBlocker` at runtime) and is never called in tests.
- **macOS Liquid glass rendering:** `liquid_offered` defaults to `false` because Rune does not yet expose macOS 26 glass/vibrancy; the `Look` and `Behind the glass` UI and persistence are wired behind `liquid_offered`.
- **OS window close button & system tray menu:** Rune's `WindowEvent::CloseRequested` unconditionally closes the window; `quit_guard_dialog::request_close` provides the guarded close/quit path, and the system tray menu is not wired in Rune yet.
