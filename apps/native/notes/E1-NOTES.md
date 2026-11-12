# E1: one door to the engine's turns (12 Nov 2026)

Plan step E1 (`docs/PLAN-fable-2026-11-12.md`, Phase 2B), on `exec/native-engine`. It changes no behaviour.

## Why

A turn lives in whichever process started it. `core/src/native.rs` keeps the running turns and the waiting cards in
statics, and the window, Quick Ask and `--ask` each reached them directly. So a card raised in one process could not be
seen in another, Stop could miss, and the cap of eight counted per process. E2 and E3 put one engine process behind a
socket. E1 first gathers every window-side use behind one trait, so that change touches one place.

## What was built

- **`core/src/engine_api.rs`.** The `EngineApi` trait covers everything the window asks of the engine's turns: send,
  stop, owns and `owned_key`, the running turns, the waiting cards, deciding a card, saving a rule and deciding,
  carrying on, shutting down and waiting for it. `InProcess` forwards each call to `native.rs` exactly as the callers
  did. `api()` returns the engine this process talks to. `install()` chooses it once at startup (E3 will install a
  client there); a second call is refused.
- **Every window-side caller goes through `api()`:** the window (`main.rs`), Quick Ask and `--ask` (`quick.rs`), the
  approval card, the Board, the attention watch, the scenes and the window tests. That is 31 call sites.
- **`src/engine_calls.rs` pins what is left.** It counts direct uses of the turn-state functions (`native::send`,
  `stop`, `owns`, `running_ids`, `waiting_approvals`, `decide_approval`, `carry`, the shutdown pair,
  `approvals_channel` and the rest) in `src/`, by file. The count is exact, so it can only fall on purpose. One use is
  left: a Board test that answers two cards on the shared channel at once.

## Left for later steps

- The door (`core/src/door.rs`) and routines still call `native.rs` directly. They are engine-side and move into the
  engine process with E4.
- `approval::can_save_allow_rule` reads the channel's teammate for a card. Over a socket the card will carry it (E2).

## How it was checked

- The full suite: 473 engine tests and 208 window tests, all passing. `cargo build --release` passed with no warnings.
- `engine_api::tests::the_default_engine_is_the_one_in_this_process`.
- One full run, with the machine's load average at 10–13 from other work, failed
  `approval::tests::claude_always_is_only_for_the_chosen_tool_in_this_turn`, which has a 5-second wait. It passed in
  the next four runs. It is a timing flake under load, not this change: the test is in `approval.rs`, which E1 does
  not touch.
