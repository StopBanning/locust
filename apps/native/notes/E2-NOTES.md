# E2: the engine as a process (12 Nov 2026)

Plan step E2 (`docs/PLAN-fable-2026-11-12.md`, Phase 2B), on `exec/native-engine`. The window does not use it yet;
that is E3.

## What was built

- **`locust-native --engine`.** The same binary, with no window. It answers `EngineApi` (E1) on a Unix socket,
  `engine.sock`, in the native data folder.
- **`core/src/engine_process.rs`:**
  - `Server::start` takes an exclusive `flock` on `engine.lock`, so a second engine refuses: "Another Locust engine is
    already running." Only the process holding the lock replaces a socket file a crash left behind.
  - The socket is made 0600, so only the owner can use it.
  - Each request gets its own connection: one JSON line in, one out. A half-read reply can never be taken for the
    next one.
  - `Client` is `EngineApi` over the socket.
  - A send is never retried. An error comes back in the engine's own words, and the caller decides what to do.
- **The wire is written by hand.** The engine has no serde derive dependency, so each type gets explicit to/from-JSON
  functions: the send, its origin (mode, folder, who started it, runtime, references), a started turn, a card with all
  its fields, an answer and a rule. An unknown answer, mode, rule or card kind is refused, never guessed.
- **Only state crosses the socket.** Windows keep following the ledger files for streaming.

## How it was checked

- `the_wire_carries_a_send_an_origin_and_a_card_whole`: a round trip of each type. What is left out stays left out,
  so a window's plain send never arrives looking like a door's.
- `a_second_engine_refuses_to_start_while_one_runs`: covers the 0600 socket, and a restart over the stale socket file.
- `a_turn_sent_through_the_socket_runs_and_its_card_is_answered_through_it`: the fake app-server in Approve each. A
  send through the client is owned and listed as running. Its card is answered through the client, and the turn
  completes. A second answer is refused, with the refusal passed back through the socket.
- `with_no_engine_running_the_client_says_so`: "Locust's engine is not running."
- **By hand, with the release binary over a temporary data folder:**
  - the first `--engine` printed where it listens;
  - a second `--engine` printed "Another Locust engine is already running." and exited 1;
  - the socket's mode was `srw-------`.
- The full suite passed (511 engine tests and 208 window tests), and so did `cargo build --release`.

## Left for E3 to E5

- **Bad replies.** In the client, `owns`, `running` and `waiting` read a failed call as false or empty. E3 starts or
  joins the engine before the window uses it, so "not running" should not arise there; it still needs a visible
  state.
- **Killing the engine** (SIGTERM) writes no endings. E5 covers the engine dying mid-turn.
- **The door and routines** still run in the window's process (E4).
