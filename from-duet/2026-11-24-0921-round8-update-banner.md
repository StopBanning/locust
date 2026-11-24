# Duet: round 8, update banner gaps (24 Nov 2026)

**Short version.** All three survivors are now caught. Branch `duet/update-reopens`, new commit `eda90c7b` on top of e2297aca (plain push, no rebase). Only `src/ui/update_banner_tests.rs` changed, and the app code is untouched. Each break you named now fails a test. Round 5 B batch 2 and C batch 2 are still in progress.

## New tests (window tests, through the drawn banner)
`window(svc)` now calls a new helper, `window_with(setup)`, so a test can set the update state without a service.
1. `is_not_drawn_while_a_turn_runs_and_is_there_again_the_moment_it_ends`: with an update ready and Codec's turn running, the composer is on screen with no banner and no "Restart and install". When the turn ends, the banner says "Locust 0.406.0 is downloaded and ready. Locust closes while it installs and opens again by itself in a minute or two."
2. `a_second_press_while_it_installs_asks_for_nothing_more`: two presses call the service's install once (counted through request_quit). The line then reads "Installing Locust 0.406.0. It opens again by itself in a minute or two; there is no need to open it."
3. `says_nothing_until_the_version_is_downloaded_and_ready`: Idle, Checking, Current, Available, Downloading, Failed and Unsupported each draw no banner and no line. Ready draws Electron's exact words (from UpdateBanner in Screens.tsx and its test). This is tested through the window, so there's no separate core test.

## Your breaks (each applied, run, restored; each run 4 passed, 1 failed)
1. `if turn_running(s) {` changed to `if false {`: test 1 failed at update_banner_tests.rs:196, "no banner while Codec's turn runs".
2. `if !s.settings_panel.update_installing {` changed to `if true {`: test 2 failed at :220, "the service is asked to install once" (left 2, right 1).
3. `update.filter(|u| u.phase == AppUpdatePhase::Ready)?` changed to `update?`: test 3 failed at :251, "no banner at Idle".

## Runs (Linux, shared target)
- `cargo test -p locust-native --bin locust-native update_banner`: 5 passed, exit 0.
- `cargo test -p locust-core updates`: 89 passed, exit 0.
- Full `cargo test -p locust-native --bin locust-native`: 1764 passed, 6 failed, exit 101. The 6 are the known Linux window_drive_tests: copy runs, failing check, note on a changed line, pointed part, turn started outside, scout isolated fix root.
- Ledger: case-map --write and recount, plus `--check` on recount, marks and test-ledger, all exit 0, and none of them changed a file.

## Not run
On a Mac.
