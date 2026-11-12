# E3: the window, Quick Ask and `--ask` share one engine (12 Nov 2026)

Plan step E3 (`docs/PLAN-fable-2026-11-12.md`, Phase 2B), on `exec/native-engine`.

## What was built

- **Joining.** `engine_process::join_or_start(dir, exe)` uses the engine answering on `engine.sock`. When none answers,
  it starts `<exe> --engine`, with stderr going to `engine.log`, and waits up to 5 s for a ping. If two callers start
  at once, the lock means one engine wins and both end up on it.
- **Who joins:**
  - the window, once it chooses the native engine (`choose_engine`);
  - Quick Ask and `--ask`, on their direct path (`quick::reach`).
  Each installs the client once (`engine_api::install`). If the engine cannot be joined, turns run in that process as
  before, and the reason goes to stderr.
- **The fence comes down when the engine is shared.** Quick Ask and `--ask` no longer hold their own turns to Ask or
  refuse every card. A card from a Quick Ask turn waits in the window and is answered there. The old fence (Ask only,
  `QUICK_DENY`, and `STOP_ELSEWHERE` for Stop) remains only for the case where joining failed.
- **The door's and the routines' turns go to the engine too.** `door.rs` and `routines.rs` call `engine_api::api()`.
  Otherwise a turn another app started through the window's door would live in the window's process, unseen by the
  engine. The door itself is still served by the window; serving it from the engine is E4.
- **Endings, with decision 3's default.** Quitting the window stops its turns. The window's shutdown asks the engine
  to stop every turn and write its ending, on its own thread so the closing window's UI thread never waits. Once the
  endings are written and the window has been told, `--engine` exits. The next window, Quick Ask or `--ask` starts a
  fresh one.
- **Tests keep the engine in their own process.** `join_engine` does nothing under `cfg(test)`, so the window tests
  and their fakes are unchanged.

## How it was checked

- `a_turn_quick_ask_starts_is_answered_and_stopped_from_the_window`: two clients on one engine.
  - A turn the "Quick Ask" client starts raises its card where the "window" client reads cards. The window's answer
    finishes it (`run.completed`).
  - A long turn Quick Ask starts is owned as far as the window is concerned, and the window's Stop ends it as
    `run.cancelled`.
- `join_or_start_finds_the_engine_that_answers`: a running engine is joined without starting anything. With none
  running and no binary to start, the refusal is said in words, not left hanging.
- **For real, release binary, temporary data folder, no window,** using
  `LOCUST_NATIVE_ENGINE=1 locust-native --ask Scout "Reply with exactly these two words: ENGINE OK"`:
  - an `--engine` process started;
  - the turn ran inside it (its id, `mission_native_66020_…`, carries the engine's pid);
  - the answer was "ENGINE OK".
  A shut-down request over the socket then answered `{"ok":null}`, and the engine exited.
- The full suite passed (513 engine tests and 208 window tests), and so did `cargo build --release`.

## Left for E4 and E5, and to measure

- **An idle engine started by Quick Ask or `--ask` stays running** until a window quits (or until E5 adds an idle
  rule).
- **`approval::can_save_allow_rule`** reads the card's teammate from the local channel. In a client process it finds
  none, so a rule saved from the window for a Quick Ask turn has no teammate. The card should carry its teammate over
  the wire (E4).
- **The engine dying mid-turn,** and the cap of eight counted once across everything (E5).
- **The benchmark.** The Rust app is now two processes while a native turn runs: the window and the engine. The
  README's numbers were measured before this, on `exec/native`. Measure again before this merges there, after the
  18th.
- **A build folder shared by two worktrees building at once** gave a stale engine crate: the window's tests could not
  find `engine_api::shared`, which was there in the source. Touching the file fixed it.
