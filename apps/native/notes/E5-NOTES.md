# E5: endings (12 Nov 2026)

Plan step E5 (`docs/PLAN-fable-2026-11-12.md`, Phase 2B), the last of the engine round, on `exec/native-engine`.

## Each ending

- **The window closing** follows decision 3's default, built in E3: quitting stops its turns. The engine writes every
  ending, then exits.
- **The engine dying mid-turn:**
  - **The open conversation reads its turn as stopped at once.** A turn whose process died has no ending, and its
    ledger may have been written a moment ago. The reader used only the fifteen-minute quiet window, so the
    conversation would have said "running" for up to fifteen minutes. `Session::poll` now also asks
    `native::orphaned(tip)`: a `mission_native_<pid>_…` turn whose process is gone is not running. The sidebar already
    did this.
  - **The phase reads as interrupted,** so the recovery card offers what the checkpoint holds, as for any interrupted
    run.
  - **The window starts the engine again when it finds it gone.** The client from `join_or_start` restarts it on a
    failed connection, at most once every ten seconds, so a binary that cannot start never becomes a spawn storm.
  - **That is not a retry.** A request that found no engine never reached one, so nothing it asked can have happened.
    A request that did reach the engine is never sent twice.
  - **The window says so once:** "Locust's engine had stopped, so it was started again. A turn it was running shows
    where it stopped." (`EngineApi::notice`, shown in the status line).
- **The cap of eight is counted once.** Every entry point (window, Quick Ask, `--ask`, the door, routines) now sends
  through the one engine, and its one list of running turns holds the cap. Nothing had to change for that.

## How it was checked

- `a_turn_whose_engine_is_gone_reads_as_stopped_at_once`: a ledger of a dead pid, written just now, reads as not
  running, while a live pid's still reads as running. With the new check taken out, it fails.
- `a_client_starts_the_engine_again_once_when_it_finds_it_gone_and_says_so`:
  - with no engine, a call starts one and is answered by it;
  - the notice comes once;
  - when the engine is gone again straight away, it is not started a second time within ten seconds.
- The full suite passed (523 engine tests and 211 window tests), and so did `cargo build --release`.

## Not checked

- **Killing a real `--engine` mid-turn with a real agent and reading the window.** The window cannot be driven on the
  person's screen. The pieces are tested separately.
- **The benchmark with two processes** (window and engine). Measure it before `exec/native-engine` merges into
  `exec/native`, after the 18th.
