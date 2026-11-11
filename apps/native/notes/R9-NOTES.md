# Round 9

Implemented the daily-path controls through the existing MCP door. No changes to `remote-mcp.ts`, dependencies, or Locust itself.

## Changes per file

- `src/engine.rs`: Added typed model/effort listings and current selections, optional per-turn model/effort arguments for start/send, and wrappers for `stop_turn` and `carry_on`. Omitted options are absent from tool arguments. Added synthetic parser and HTTP-door tests, including tool names and arguments.
- `src/main.rs`: Added a composer model chip and scrollable model/effort menu. Picks belong to the selected teammate and are consumed on the next send; the chip labels an override “this turn,” then returns to the teammate's selection. Model requests run off the UI thread. Added independent ownership tracking so following an external working turn never grants Stop. Accepted sends bind ownership to their new ledger tip; Stop uses the conversation ID. Added carry-on cards with AI-agent/model choices from an unscoped model listing and routed accepted carry-on turns through the existing follow machinery. Added ownership and one-turn-pick reset tests.
- `src/ledger.rs`: Added per-turn carry-on items for `route.limit_detected` and failed runs with limit/provider details. Completion, cancellation, or restart clears the trigger. Cancelled turns render “Stopped.” Added synthetic trigger and cancellation tests.
- `src/quick.rs`: Updated its existing start call to omit the new optional model/effort parameters; its behavior is unchanged.

## Verification

From `native/`, with the installed Cargo directory on PATH:

- `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-r9 cargo test --release`: **51 passed, 0 failed, 2 ignored** (53 total; eight added tests). The ignored tests access a real Locust instance or real ledgers and were not run.
- `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-r9 cargo build --release`: passed.
- `$HOME/work/scratch/native-target-r9/release/locust-native --snap /tmp/r9.png --scene ledger-live --size 900x640 --scale 2`: passed headlessly. Inspected the rendered frame: the model chip fits beside the composer, the transcript remains visible, and the unowned demo turn does not expose Stop.

## Limits of verification

No live provider turn was started, stopped, or carried on. The tool calls were verified against a synthetic local MCP server. The supplied `remote-mcp.ts` declares tool names and argument schemas, but delegates tool execution and does not contain the `list_models` response implementation; its live JSON response shape remains unverified. The parser accepts agent arrays or individual agent objects, with model rows, effort lists, and current model/effort selections, and reports unreadable replies without panicking.

The headless frame verifies the existing `ledger-live` scene; the open menus and carry-on card were not exercised interactively. No window was opened for manual testing.
