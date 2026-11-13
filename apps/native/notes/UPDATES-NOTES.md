# Keeping Codex and Copilot current (13 Nov 2026)

Electron `main/runtime-updates.ts` (548 lines), its row's words in `renderer/src/agentUpdates.ts`, and the Settings row and
switch in `Screens.tsx`. On `exec/native-updates`.

## What was ported

1. **The rule and the service** (`core/src/agent_updates.rs`): `compare_versions`, `decide`, `saved_updates_from` and
   `Saved::to_json` (the file is Electron's own shape, `runtime-updates.json` in the native store's folder),
   `npm_package_dir`, `held_from`, `release_from` (npm's answer), `may_update_agents`, and the `Updates` service: `state`,
   `set_automatic`, `update_now`, `tick`, `updating`. Timings: first look 45 s after launch, a tick every hour that goes to
   npm only when six hours have passed, a release must be 12 hours old.
2. **Every effect behind `Effects`**: discover, npm root, a link's real path, latest release, held versions, in-use,
   install, load, save, updated, changed, now. `Machine` is the real one: it refuses under `cfg(test)` (nothing is
   discovered, run, fetched or installed; only its own file reads and writes). The install is
   `agent_install::for_this_machine().install(id, name, Some(version))`, the one installer Install already uses.
3. **The row** (`src/ui/agents_section.rs`): the line (`update_line`), an Update button where Electron offers it
   (`offers_update`), and the switch with Electron's two sentences (`keep_current_note`). A copy that is not the installed
   app (`os_hosts::packaged()`, or a scripted launch) never ticks and says why instead of showing a switch (`heldHere`).
   Everything runs in one `Latest` job (Read, Tick, Press, Automatic); drawing reads memory only, and the lines are worded
   when the state changes, not while drawing. While a job runs the poll takes `peek()` (memory) so "Updating to ..." shows.
   When an agent was updated the window asks discovery again.

## Tests

`core/src/agent_updates_tests.rs` (40): all 26 it() of runtime-updates.test.ts, the in-use test's promise on a Mac, all 6 of
the renderer test, and ones the port added (npm's answer, held versions kept across a look that cannot reach the check, a
past update said again, the switch kept as a choice, a peek while updating, real effects refuse in a test).
`window_drive_tests.rs update_installs_the_newer_version_and_the_row_says_updated`: a fake service, the row says 0.156.1 is
out, Update calls the fake installer with `codex@0.156.1`, the row says updated, Update is gone.

## Electron's special cases, where they live, and what fails when they break

| Special case | Rust line | Test that fails when it is broken |
|---|---|---|
| A release must be out 12 h; no date is "too new" | `decide`, `RELEASE_AGE_MS` | `waits_on_a_release_out_for_less_than_twelve_hours_or_one_npm_gives_no_date_for` (broke: age 1 ms) |
| Off, a newer one only waits for the person | `decide` (`Why::Ask`) | `asks_first_a_newer_release_waits_for_the_person...`, `turned_off_only_looks...` |
| A version Locust's check could not read is held, never installed on its own, still installed on Update | `decide` (`Why::Held`), `run` | `holds_back_a_version_locusts_release_check...` (broke: held ignored, 3 fail) |
| Only a choice the person made is kept (0.303 saved off for everyone) | `saved_updates_from` (`chosen`) | `updates_on_its_own_unless_the_person_turned_that_off...` (broke: chosen not required) |
| Npm's folder only: a Mac link into the package, Homebrew's Cellar left alone | `npm_package_dir` (`real_path`) | `is_npms_on_a_mac_or_linux_too...` (broke: link not followed) |
| One lock for a tick and a manual Update (L12): a tick during Update waits, installs once | `Updates::tick`, `update_now` (`ticking`) | `a_tick_during_a_manual_update_waits_for_it_and_installs_once` (broke: tick runs after waiting) |
| The process table is read only when an update would otherwise go ahead | `run` (`provisional`) | `does_not_go_to_the_network_when_the_last_look_is_recent...` (broke: always read; 2 fail) |
| No network inside six hours | `run` (`CHECK_EVERY_MS`) | same test |
| A scripted launch / a copy never updates the machine | `may_update_agents` | `a_scripted_launch_never_changes_the_machine...` |
| Anything unreadable about "in use" reads as in use | `unwrap_or(true)` in `run`, `press_update` | not tested: no Electron it() for it, and the fake never errs |
| Update on the press refuses while in use and says so | `press_update` | `does_not_update_on_the_press_while_something_is_using_it...` |
| A failed update is kept and said again; a landed one is said until something newer | `update_one`, `run` | `says_when_an_update_failed...`, `a_past_update_is_said_again...` |
| The in-use check is a case-insensitive substring of the folder, read as UTF-8 | `processes_using_bytes` | `the_in_use_check_finds_a_process_whose_path_has_a_non_ascii_letter` |
| The row's words, Update not offered for "too new" | `update_line`, `offers_update` | the six renderer-port tests |

Four of these were broken on purpose (six, in fact) and restored from `~/work/scratch/brk/`; each named test failed.

## What differs

- **Windows**: the two Windows-layout `npm_package_dir` tests exist only under `#[cfg(windows)]` (Electron skips them off
  win32 too); the PowerShell/`cmd.exe` process table and `npm` through `cmd.exe` are not built. The in-use test of
  Electron is Windows-only; its Mac twin runs a real `sh` with a non-ASCII folder on its command line.
- **Blocking, not async**: the service is blocking and runs in a job; a tick that finds another running waits for it and
  returns (Electron returns the same promise).
- **`set_automatic` ticks inside the call** when turned on (Electron fires `tick()` and returns), so its job ends when
  the update does; the window shows progress meanwhile from `peek()`.
- **`agents::discover()`** runs all nine agents' checks on a tick, not just the two (a tick is hourly, in a job).
- The canary file is fetched with `/usr/bin/curl --silent --fail --max-time 10 <url>`, no credential.
- Times in the row are en-US style ("at 11:05 AM", "on Sep 22"), not the computer's locale.
- No `agentsStay` disable on the Update button (that is Electron's Home-screen mission gate, not built here).
- Electron also tells the model catalog to forget (`models-changed`); here the window asks discovery again, and the model
  lists are read live elsewhere in this port.
