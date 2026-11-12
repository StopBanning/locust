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
Then the search (`.lc-picker__head`: a mono slash and "Search runtimes and models", matching agent and model names
the way routeSearchText reads them), and each agent's group capped at six rows (twelve while searching) with the one in
use always kept and "N more models · type to search them" under a capped group, as capRouteRows does.

## Not done

- (Fixed the same night) `opencode_acp_success_sets_locust_mode` hung once under a full parallel run: its helper thread
  answered the approval card within 5 seconds or gave up quietly, leaving the turn waiting forever. The three helpers
  that answer cards from a thread (OpenCode, Copilot, ACP) now wait 60 seconds.
- OpenCode 2's and Kilo's models (an ACP session), the models Claude Code's handshake lists, and usage readings.
- The catalog treats an installed agent as ready; Electron also checks each one's sign-in first.

## Second part (12 Nov): notifications and the folder picker

- **Notifications** (`core/src/attention.rs`, from attention.ts and finishedToast.ts, in Electron's words): only while
  Locust's window is not in front (`on_window_focus`), for a card waiting for an answer ("Codec needs your approval"),
  a turn that hit its plan's limit, and a turn you started that ran at least a minute and ended ("Wren finished",
  with its first line and how long it took). Finishes within four seconds of each other are said as one ("3 teammates
  are done"). `src/notify.rs` posts them through macOS's notification center, which needs the app bundle
  (`scripts/bundle-mac.sh`); a bare binary shows none. The first one asks macOS for permission.
- **The folder picker**: Home's Choose folder opens the system folder dialog (it used to make a folder under the data
  folder), and the folder chosen becomes the native app's own open folder (`workspace.json` in its data folder, in
  Electron's shape), which turns now use before Electron's. A project named in a message opens the same way.
- Not done: clicking a notification does not open its conversation yet; no notification for a question asked in a
  reply (`locust-ask`); none on Linux or Windows; not seen live (a test would put a permission prompt on screen).
