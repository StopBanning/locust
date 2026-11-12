# E4: the engine serves the door for other apps (12 Nov 2026)

Plan step E4 (`docs/PLAN-fable-2026-11-12.md`, Phase 2B), on `exec/native-engine`.

## What was built

- **The door lives where the turns run.** The engine process keeps one door (`engine_process::DOOR`), made over its
  own data folder (`Door::for_store` with `native::store()`). It keeps everything it had: its switch, its token, its
  tools, and its rule that no tool answers a card.
- **A `door` request on the socket**: load, read, or set one of the switches (`engine_api::DoorChange`). It replies
  with the door's state, as Settings shows it. `EngineApi::door` is how a window asks. The in-process engine serves
  no door, so a window that could not join keeps its own.
- **The window no longer opens a door at launch.** Once it knows it is on the native engine, it joins
  (`join_engine`), then `open_door` opens the door as saved: through the engine, or through its own door if joining
  failed. A snapshot, video or script never opens one. Settings reads and flips the switches through `door_now`,
  wherever the door is served.
- **The door closes with the engine.** The shut-down request disposes it, so its connection file goes, and no app
  reaches a Locust that is not there.
- **Electron's engine in use:** when the window runs on Electron's engine (Electron open, with its door), the native
  door is no longer opened at all. Before, the window opened it whatever engine it used.

## How it was checked

`the_engine_serves_the_door_its_turns_are_the_engines_and_no_app_answers_a_card`:
- the saved switch is on, and the window's `door(Load)` through the socket opens it: the connection file appears, and
  the Claude Code line names `--door-bridge`;
- another app's `start_conversation`, posted over HTTP with the connection file's token, starts a turn whose ledger
  is in the engine's data folder, and the turn completes;
- `decide_approval` is refused with "That tool is not offered by Locust.";
- `door(Enabled(false))` turns it off, saves `enabled: false`, and removes the connection file.

The full suite passed (521 engine tests and 211 window tests), and so did `cargo build --release`.

## Left

- **One door per engine process** (a static). Only one test opens it.
- **A door kept by an engine that Quick Ask started.** The window asks for the door. Quick Ask and `--ask` never do,
  so their engine has none until a window joins.
- **E5:** the engine dying mid-turn, and the cap of eight counted once.
