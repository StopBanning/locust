# Cutover 1: the Rust app gets ready to be Colin's Locust on his Mac (25 Nov 2026)

From Big Brother's review of 25 Nov (`docs/REVIEW-big-brother-port-2026-11-25.md`, findings 2 and 3 and the cutover
checklist). Colin decided on 25 Nov: his data is re-seeded into a fresh Rust folder on switch day (Electron's folder is
left untouched, so Electron stays the way back), and the version keeps Electron's numbering. None of these rows is a
TypeScript test row, so they are recorded here and not in `parity/tests.tsv`.

Break runs: `~/work/scratch/mut6.py`, each list and its output kept side by side in `~/work/scratch/evidence/cutover-1/`.

## Row 1: the native engine by default

**What changed.** `core/src/native.rs` `forced()` now says native unless `LOCUST_NATIVE_ENGINE` is exactly `0`
(unset, empty, `1` or anything else all mean native); the reading is a private pure function, `native_by_default`.
`choose` is unchanged in shape and stays pure: with `forced` true it never asks Electron's door. So a double-clicked
app, which has no environment, runs on its own engine even while Electron is open and its door answers.
`LOCUST_NATIVE_ENGINE=0` is the one way back to Electron's engine (the door while it answers, native only when it
cannot be reached, as before).

**Call sites** (both read `forced()`, so both follow): `src/main.rs:844` (`choose_engine`, the window) and
`src/quick.rs:328` (`reach`, Quick Ask).

**Tests.**
- `core/src/native/tests.rs` `native_is_the_engine_unless_locust_native_engine_is_zero`: unset, `1`, empty, `yes`,
  `00` give native; only `0` gives the door.
- `core/src/native/tests.rs` `engine_choice_forced_door_down_door_up_and_door_refusal`: comments updated for the new
  default; the cases are the same.
- `src/engine_default_tests.rs` `the_window_runs_on_its_own_engine_while_electrons_door_is_up`: a made-up door (a
  connection file pointing at a local listener that counts connections) is up; `choose_engine` in a child copy of the
  test binary, with `LOCUST_MCP_CONNECTION` set on the child alone, chooses native and never asks the door; with
  `LOCUST_NATIVE_ENGINE=0` the door is asked. Before this change the default asked the door.

**Breaks.** 5 of 5 caught by name.
- `row1-core.entries` / `row1-core.out` (locust-core): 2 of 2, both by `native_is_the_engine_unless_locust_native_engine_is_zero`
  (the old `== "1"` reading; `is_none()`).
- `row1-native.entries` / `row1-native.out` (locust-native): 3 of 3, all by
  `engine_default_tests::the_window_runs_on_its_own_engine_while_electrons_door_is_up`: the call site in `choose_engine`
  bypassed (`native::forced()` replaced by `false`), `forced` ignoring the variable, and the old reading.

**Not changed (not this lane's files).** These still describe the old default: the comment above `choose_engine`
(`src/main.rs:840`), `src/quick.rs:323`, `README.md:23` and `PARITY.md:11`.

## Row 2: the version follows Electron's numbering

**What changed.** `apps/native/Cargo.toml` and `apps/native/core/Cargo.toml` say `0.883.0`, the next number after the
Electron Locust installed on this Mac (`/Applications/Locust.app` reports 0.882.0; this branch's
`apps/desktop/package.json` is older and says 0.870.0). `Cargo.lock` follows. Every `env!("CARGO_PKG_VERSION")` now
reads 0.883.0: the backup's read, write and restore (`src/ui/backup_section.rs:223,258,298`), the pending restore the
engine applies (`core/src/engine_process.rs:497`), the diagnostic log, feedback, the record files, and the version
the app gives agents (`codex_voice.rs`, `acp_agent.rs`). So a backup Electron 0.882.0 made is read; one from a newer
Locust is still refused with Electron's words.

**Call site.** `src/ui/backup_section.rs:258` (`preview_restore`, reached from the restore folder picker at
`backup_section.rs:248`).

**Tests.**
- `core/src/profile_backup.rs` `a_backup_from_electron_0_882_0_is_read_and_one_from_0_884_0_is_refused_with_electrons_words`.
- `src/ui/backup_section.rs` `electron_backup_tests::the_window_previews_a_backup_from_electron_0_882_0_and_refuses_one_from_0_884_0`:
  through `preview_restore`, as the window does.

**Breaks.** 4 of 4 caught by name.
- `row2-core.entries` / `row2-core.out` (locust-core): the core manifest back at `0.1.0`, caught by
  `a_backup_from_electron_0_882_0_is_read_and_one_from_0_884_0_is_refused_with_electrons_words`; the version
  comparison turned round, caught by it and `reader_validates_manifest_hashes_versions_and_paths`.
- `row2-native.entries` / `row2-native.out` (locust-native): the window manifest back at `0.1.0`, and the call site
  in `preview_restore` passing `"0.1.0"` instead of `CARGO_PKG_VERSION`, both caught by
  `electron_backup_tests::the_window_previews_a_backup_from_electron_0_882_0_and_refuses_one_from_0_884_0`.

**What still says 0.1.0 (grep of the tree; none in this lane's files).**
- **The title bar, Settings and the updater do not read the Cargo version.** `src/ui/home_panel.rs:184-190` sets
  `s.home.current_version` from the first heading of the native changelog, `src/changelog.rs:19` `## 0.1.0`; the
  title (`src/ui/rail.rs:669`, "Locust 0.1.0"), Settings' version line (`settings_panel.rs:148,950`) and the update
  service's current version (`settings_panel.rs:149`) all use it. Until the changelog gains a `0.883.0` entry, or
  `home_panel.rs` falls back to `CARGO_PKG_VERSION`, the app still calls itself 0.1.0 there. What's new is still the
  native app's own notes, not Electron's (eyes-3 finding 6, a decision for Colin).
- `scripts/bundle-mac.sh:36`: `CFBundleShortVersionString` 0.1.0 (and the bundle id `lol.locust.native.preview`).
- `core/src/updates.rs:190`: `AppUpdateState::default()`'s `current_version` (a placeholder the window replaces).
- Fixed strings that are not this app's version: `models.rs:779,992` and `runtime/codex.rs:573` send
  `clientInfo.version "0.1.0"` to agents; test-only values in `mission_export.rs` and `home_panel.rs` tests.

## Row 3: seed again from Electron

**What changed.** `core/src/store.rs` gains `SEEDED_FILES` (the nine files `Store::seeded_json` seeds: teammates,
memories, folders, routines, approval rules, conversation references, appearance, terminal imports, folder cautions;
checked against every `seeded_json(` caller in `core/src` and `src`, browser_panel's two callers read
`teammates.json`) and `Store::reseed_from_electron(stamp)`, which returns a `Reseeded` (copied, moved aside, the
`seeded-before-<stamp>/` folder, unchanged, skipped with the reason). Every Electron file is read (2 MiB cap) and
parsed as JSON before anything is replaced; a file that is missing, unreadable, too large or not JSON is skipped and
its Rust copy is left as it is. A copy that is already identical is left alone. Otherwise the fresh bytes are written
to a temp file in the Rust folder, the old Rust copy is moved (renamed, never deleted) into
`<root>/seeded-before-<stamp>/`, and the temp file is renamed into place. It takes the engine lock and refuses
("Locust is running on this data folder") while an engine holds it, since a running engine would write its own copies
back. Nothing is ever written in Electron's folder (`store.rs` `refuse` is unchanged).

**Call site.** `src/main.rs`: `main` calls `cutover_flag` (before `shell_path::adopt_terminal_path`), which handles
`--seed-from-electron` through `seed_from_electron`, which calls `Store::reseed_from_electron` on `Store::local()`
(LOCUST_NATIVE_DATA, or the default Rust folder), prints what was copied, moved aside, unchanged and skipped, and exits
(1 when the folder cannot be used or an engine holds it).

**Tests.**
- `core/src/store_reseed_tests.rs`: `a_fresh_copy_replaces_a_stale_one_and_the_stale_one_is_kept_aside`,
  `an_unreadable_or_invalid_electron_file_leaves_the_rust_copy_untouched`, `electrons_folder_is_never_written`,
  `seeding_again_is_refused_while_an_engine_holds_the_folder`.
- `src/cutover_flags_tests.rs`: `seed_from_electron_copies_electrons_files_into_the_rust_folder_and_keeps_the_old_ones`
  (a child copy of the test binary, with HOME and LOCUST_NATIVE_DATA set on the child alone, runs the flag),
  `main_hands_its_arguments_to_cutover_flag_before_anything_slow`.

**Breaks.** 10 of 10 caught by name.
- `row3-core.entries` / `row3-core.out` (locust-core): 7 of 7 KILLED: the move aside turned into a delete, the JSON
  check on a made-up body, the "already the same" test disabled, the engine-lock check disabled, the size limit
  doubled, the temp file made in Electron's folder, and the old-copy branch disabled.
- `row3-native.entries` / `row3-native.out` (locust-native): 3 of 3 KILLED: the call to `reseed_from_electron`
  replaced by an empty result, the flag's match arm returning 0 without running, and `main` not calling `cutover_flag`.

## Row 4: Colin's real history, read once, read-only

**What changed.** `core/src/history_check.rs` (new): `check_history(dir)` reads every `*.jsonl` file directly in a
folder, and in its `.trash/` when there is one, with the Rust ledger reader (`ledger::read_ledger`, then
`read_transcript`), and sorts each by what it found: refused (the name is not a conversation id, the file could not be
read, or `read_ledger` says `unreadable`, with the issue codes), damaged (read, but only the part before the fault:
`truncated-tail`, `invalid-record`), or read with zero turns (nothing after the opening prompt), and the rest as read
whole. It only opens files to read them. The reader was NOT changed.

**Call site.** `src/main.rs`: `main` -> `cutover_flag` -> `--check-history <folder>` -> `check_history` (main.rs), which
calls `locust_core::history_check::check_history` and prints the files by reason. For the real run, the same
`cutover_flag` is reached by the `#[ignore]`d test `cutover_flags_tests::check_history_over_a_real_copy`, which takes
the folder from `LOCUST_HISTORY_COPY`, so no second binary had to be built.

**Tests.**
- `core/src/history_check_tests.rs` (8): a whole file reports nothing; a cut-off last line and a bad record are damaged;
  no header, an empty file and a newer schema are refused with their reasons; a bad file name; a conversation with only
  its opening prompt; the trash is checked and says so; a missing folder is an error; the check writes nothing (every
  file's bytes and modified time are the same after).
- `src/cutover_flags_tests.rs`: `check_history_reports_by_reason_and_writes_nothing`,
  `check_history_without_a_folder_or_with_a_missing_one_exits_1_and_says_why`.

**Breaks.** `row4-core.entries` / `row4-core.out` (locust-core, 8 breaks) and `row4-native.entries` /
`row4-native.out` (locust-native, 3 breaks, two of them on the call site: the flag's match arm, and the call to
`history_check::check_history`). All 11 KILLED by name (8 of 8 core, 3 of 3 native).

**The run (26 Nov 2026).**
- Copy: `cp -c -R "$HOME/Library/Application Support/@teammate/desktop/mission-ledger" /Volumes/Files/scratch-main/history-copy-20261126`
  (an APFS clone, 584 MB; only `mission-ledger`; 60 conversation files, 545 in its `.trash/`, plus `.writable`).
  SHA-256 of every file in the copy before and after the run is identical
  (`~/work/scratch/evidence/cutover-1/row4-copy-before.sha`): the check wrote nothing in it. Output:
  `~/work/scratch/evidence/cutover-1/row4-real-run.txt`.
- **Files read: 605 (the 60 and the 545 in the trash). Read whole with turns: 605.**
- **Refused: none. Damaged (truncated or invalid record, unsupported schema, too large): none. Read with zero turns:
  none.** No `.jsonl` file in the copy was skipped for its name either.
- It took 174 s in a debug build on this Mac (the reader parses every record of 584 MB).
- What this does not say: it checks that the ledger reader keeps every record and that each file has a header and at
  least one thing after the prompt. It does not compare what the window would draw with Electron's drawing of the same
  conversation, and it does not read the other state in Electron's folder.

# Cutover 2b: a release the old app's updater installs, and the Rust app that keeps updating itself (26 Nov 2026)

Rows 3 and 4 of `HANDOFF-cutover-2.md` (rows 1 and 2, the bring-over card and saved keys, are another lane's). Not
TypeScript test rows, so recorded here. Break runs: `~/work/scratch/mut6.py`, each list and its output kept side by
side in `~/work/scratch/evidence/cutover-2b/`.

## Row 3: a release the old app's updater installs

**What changed.** `scripts/bundle-mac.sh` gains `--release` (and `--out <folder>`). Release mode makes `Locust.app`
with Electron's bundle id `com.automatedworkflow.locust` (so macOS's microphone, screen and folder permissions stay
with the same app) and the `locust://` link claim Electron's Locust has, and packs it with `hdiutil` into
`Locust-<version>-mac-<arm64|x64>.dmg` with `Locust.app` at its root (plus an `Applications` link for a drag-install),
the exact shape Electron's Mac updater takes (`apps/desktop/src/main/mac-self-update.ts`, `mac-release.ts`: the newest
release asset named `Locust-<x.y.z>-mac-<arm64|x64>.dmg`, mounted, `Locust.app` at its root, no bundle id or signature
checked). With no argument it still makes `Locust Native.app`, `lol.locust.native.preview`, no links unless
`LOCUST_NATIVE_LINKS=1`, no disk image. In both modes `CFBundleShortVersionString` (and `CFBundleVersion`) is the
crate's version from `Cargo.toml` (it said `0.1.0` before), `LOCUST_BUILD_COMMIT` is handed to the build when set, and
an unknown argument stops the script before anything is built. Nothing is uploaded.

**Call site.** A script, not a function: run by hand (`scripts/bundle-mac.sh --release`) when Colin cuts a Mac release.

**Tests** (`core/src/bundle_mac_tests.rs`, macOS only; a stand-in `cargo` first on PATH stands for the release build, the
rest of the script is the real one, with the real `sips`, `iconutil`, `codesign`, `hdiutil`; the expected names and
ids are Electron's, from `electron-builder.yml` and `mac-self-update.ts`, not read from the script):
- `release_mode_makes_a_locust_app_with_electrons_id_and_this_crates_version_in_the_disk_image_electrons_updater_takes`:
  the app, its plist (id, name, version, the link claim), then the image mounted read-only: the exact image name,
  `Locust.app` at its root (only it and `Applications`), the same id and version inside.
- `the_preview_keeps_its_own_name_and_id_takes_the_crate_version_too_and_makes_no_disk_image`.
- `an_argument_it_does_not_know_stops_it_before_anything_is_built`.
The first run of these (`run-a.log`, the earlier account's) failed all three: the tests had been written before the
script's release mode. They pass against the script as it stands (3 of 3, this session).

**Breaks.** `row3.entries` / `row3.out` (locust-core): 9 of 9 KILLED by name, every one by a `bundle_mac_tests` test
(the id, the name, the version, the commit hand-over, `--release` ignored, the image name without the chip, the app
not at the image's root, the link claim, the unknown argument). The earlier run's list was checked against the script
first: no broken line was left in it.

## Row 4: the Rust app keeps updating itself

**What changed.** `core/src/mac_self_update.rs`: `this_mac` (the self-updater for this copy) now fills every seam with the
real one, where it used to leave download, disk-image commands and swap unset (so a switched copy could only look).
Electron's `main/mac-self-update.ts` is the model for each:
- `download_release` (`createMacUpdater`'s `download`): GETs the release's address, writes the file in 64 KB steps, says
  the percent each time it changes, `The download answered <code>.` on a refusal, and removes a half-written file if
  the body stops short. `MacUpdater::prepare` still checks size and SHA-256 against the release afterwards, as before.
- `run_command` (`runCommand`): runs `hdiutil`, `ditto`, `xattr`; returns the exit code (None if it cannot start or is
  killed by a signal) and stdout+stderr.
- `start_swap`: starts `/bin/sh <script> <pid> <app> <staged>` in its own process group with no stdio, so the app
  quitting does not take the swap down (Electron's `detached: true, stdio: 'ignore'`).
- the quit seam is `std::process::exit(0)`: once the update service has flushed (`updates.rs` `request_quit` then
  `quit_and_install`) and the swap is handed over, the app ends so the script's wait for its pid finishes.
- Test builds only: `fetch_address` sends Locust's downloads address (`updates::DOWNLOADS`) to the server named by
  `LOCUST_TEST_SELF_UPDATE_DOWNLOADS`. A release build has no such seam. The module doc's old "never fetches" safety
  rule is replaced by what is true now: still behind the self-update switch and feed (`LOCUST_NATIVE_SELF_UPDATE=1` and
  `LOCUST_NATIVE_UPDATE_FEED`), which stay off by default.

**Call sites** (outside tests). `download_release` `mac_self_update.rs:622`, `run_command` `:623`, `start_swap` `:624`,
all inside `this_mac` (`:603`), which is called by `updates::this_copys_update_service` (`updates.rs:532`) through
`app_updater`. Honest gap: nothing in the window or `main.rs` calls `this_copys_update_service` yet, so the switch has
no reader in the running app; that wiring is not in this lane's files (and the switch stays off).

**Tests** (`core/src/mac_self_update_live_tests.rs`, macOS only, declared at the end of `mac_self_update.rs` as
`live_tests`; a file server on 127.0.0.1 in the test process; no network, nothing in `/Applications`):
- `the_app_downloads_a_newer_locust_from_its_feed_swaps_it_in_after_it_quits_and_opens_it`: a copy of the test binary
  inside a temp `Locust.app` runs as the app: `this_mac` finds its bundle, reads the releases list from the local
  server, downloads a real `hdiutil`-made image (100%), checks it against the stated size and SHA-256, attaches it,
  `ditto`s its `Locust.app` beside the running one, `quit_and_install` ends the process with success, the swap script
  (real, detached) puts the new app in the old one's place, removes the staged copy and `.replaced`, and calls `open`
  (a stand-in on PATH that writes down what it was asked to open). The download stayed in the app's own data folder
  (`LOCUST_NATIVE_DATA`, a temp folder).
- `the_real_download_writes_every_byte_and_counts_up_to_100`, `a_download_that_is_not_there_says_what_answered_and_leaves_no_file`,
  `a_download_that_stops_half_way_says_so_and_leaves_no_half_file`, `the_real_run_says_how_a_command_ended_and_what_it_said`,
  `the_swap_runs_apart_from_the_app_with_its_arguments_so_the_app_quitting_does_not_take_it_down` (its process group is
  not the app's).

**Breaks.** `row4.entries` / `row4.out` (locust-core): 11 of 11 KILLED by name. The three seams in `this_mac`, each
`None`d (`download`, `run`, `start_swap`), and the quit made a no-op, are killed by the end-to-end test: that is the call
site's proof. The rest are killed by the unit-level live tests: the percent arithmetic, a short write, the refusal's
status code, the half-file clean-up, the process group, the arguments handed to the swap, and the exit code.
