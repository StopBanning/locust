# B4, first part: the model list (12 Nov 2026)

The native engine had no model list: the window said choosing a model "runs in Locust for now", and the door refused
a model or effort. It now builds Electron's catalog itself.

## What was built

- **`core/src/models.rs`**, from model-catalog.ts and runtime-adapters' parsers: Codex's `model/list` from an
  app-server started for it (no turn), Claude Code's aliases from its `--help` (each alias named by the version it
  means, then the older fixed versions), Cursor's `--list-models` grouped into one model per family with its levels
  and the variant id each means, OpenCode 1's `models --verbose` with each model's variants, Antigravity's
  `agy models` with its efforts beside each model, Copilot's Auto, Muse's two models, and each runtime's
  "Account default" row with the levels all its current models share. Read in parallel, within 20 seconds each,
  kept ten minutes. `locust-native --models` prints it.
- **The door**: a model and effort for one turn (choiceOf: only ones the teammate's agent offers here, the saved
  effort kept for a model asked for alone, Electron's words for each refusal), `list_models` for a teammate or for
  every agent, and `carry_on` checks a named model against the list.
- **Cursor's level** goes into the model id it names at the start of a turn (cursorChoiceFor), as Electron's host does.
- **The window's model picker** on the native engine reads the same list; so does the carry-on card's menu.

## How it was checked

- `scripts/model-goldens.mjs` runs Electron's parsers and catalog builders over the real list output of Claude Code,
  Cursor, OpenCode, Antigravity and Codex on the Mac mini (`parity/models/`). Rust matches every parsed list and the
  whole catalog, model for model: 101 models here, 95 for a machine with Muse and no Codex.
- `locust-native --models` on this Mac: the same 101 models in 3.6 s.
- Live through the door's bridge: Scout's 13 models listed, a turn on a named free model answered, an unknown model
  refused with nothing started.
- Tests: 453 in the engine, 190 in the window. The isolated test runner gives tests their own home folder, so a live
  read inside a test cannot see Cursor's or Antigravity's sign-in; the check runs through the binary instead.

## The picker, restyled (same night)

RoutePicker.tsx's look: a 420-wide card on the raised fill with a strong edge and a menu shadow; each agent's models
under its mark and name, one row per model with a dot, its name, the levels it takes, and ACTIVE on the one in use;
"Teammate's own" first to go back to the saved route; the level for the turn chosen at the foot, apart from the
model, as Electron chose it. It was one row per model and level. Scenes `models` and `models-light`.

## Not done

- The picker has no search box and no "N more · type to search" cap yet: Cursor's 56 models scroll in the list.
- OpenCode 2's and Kilo's models (an ACP session), the models Claude Code's handshake lists, and usage readings.
- The catalog treats an installed agent as ready; Electron also checks each one's sign-in first.
