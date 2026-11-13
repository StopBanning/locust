# New-teammate interview, setup inspection, and pre-send warnings in Rust (Hydrogen, 13 Nov 2026)

Notes on porting:
- **PARITY row 53 ("New teammate interview and setup inspection")**:
  - `apps/desktop/src/shared/interview.ts`, `apps/desktop/src/renderer/src/components/InstructionsCard.tsx`, `apps/desktop/src/renderer/src/components/NewTeammateDialog.tsx`
  - `apps/desktop/src/shared/teammate-setup.ts`, `apps/desktop/src/main/teammate-setup.ts`, `apps/desktop/src/renderer/src/components/TeammateSetup.tsx`
- **PARITY row 40 ("Connector-off and unavailable-route pre-send warnings")**:
  - `apps/desktop/src/renderer/src/connectorsOff.ts`, `apps/desktop/src/renderer/src/components/Composer.tsx`, `apps/desktop/src/renderer/src/status.ts` (`droppedFreeModel`, `freeStartModel`)

## What was built

- **New teammate interview (`core/src/interview.rs`, `src/ui/instructions_card.rs`, `src/ui/people_panel.rs`, `src/ui/messages.rs`)**:
  - Ported `INSTRUCTIONS_TAG`, `INTERVIEW_OPENING`, `interview_prompt`, `is_interview`, `instructions_block` (last `<locust-instructions>` outside code, trimmed and UTF-16 bounded to 4000), and `instructions_refusal` (`secrets::secret_in` + `instructions_secret_refusal`).
  - Added `"Let {name} interview you instead"` / `"Let {name} interview you again"` in `people_panel.rs` `edit_form`; saves/creates the teammate via `people::save_teammate_id`, opens a fresh conversation for it, and sends `interview_prompt` in `Mode::Ask` for that first turn.
  - Added `instructions_card::card` in `messages.rs` under an interview's finished turn: editable draft, replacement notice when the teammate already has instructions, secret refusal check before saving, and `"Save as {name}’s instructions"` persisting to `teammates.json`.
- **Setup inspection (`core/src/teammate_setup.rs`, `src/ui/setup_card.rs`, `src/ui/people_panel.rs`, `src/ui/composer_wiring.rs`)**:
  - Ported `read_workspace_brief` (`LOCUST.md` with `<runtime>` section filtering and 200-line / 24 KiB bound), `runtime_file` (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`), `skill_folders` (`SKILL_FOLDERS` for `claude`, `codex`, `opencode`, `copilot`, `antigravity`, `cursor`), `skill_project_folders` (stopping at `.git`), `counted_skill_names` (following symlinks/junctions with canonicalized seen-set cycle detection and depth <= 6), `last_turn_of` (`run.started` `evidence.raw` counts), `gather_teammate_setup`, `setup_sections`, and `read_teammate_setup`.
  - Wired `/setup` in `composer_wiring.rs` (both slash popup and Enter) and a `"Setup"` button (`team-setup-{id}`) on each teammate card in `people_panel.rs` to open `setup_card::view`, with `"Close"` and `"Edit teammate"` actions.
- **Pre-send warnings (`core/src/connectors_off.rs`, `src/ui/thread_view.rs`, `src/ui/home_panel.rs`, `src/ui/people_panel.rs`)**:
  - Ported `connector_label`, `connector_mentioned` (whole-token regex, ignoring labels < 3 UTF-16 units), and `connectors_off_hint`. When a Claude Code teammate has no ticked connectors, `connector_names` is non-empty (loaded from `~/.claude.json` `mcpServers` when present), and the draft names a connector, `thread_view.rs` shows the hint above the composer without blocking send.
  - Ported `FREE_MODELS_BEST_FIRST`, `free_models_best_first`, `dropped_free_model`, `free_start_model`, and `opencode_model_display_name`. When a teammate or turn pick is on an `opencode/...-free` model no longer in OpenCode's catalog, `thread_view.rs` shows `"OpenCode no longer offers {model}."` and `"Switch {name} to {next}"` (updating `teammates.json` or turn pick), and `home_panel.rs` / `people_panel.rs` colour the card's route amber.
  - Unavailable routes (`!s.route_can_run`) continue to be caught by `send_blocked_now` and reported in `composer_note` on send.

## Electron's special cases

1. **`teammate-setup.ts:72` (`claude = runtime === 'claude'`)**:
   - Claude Code is handled specially for project/personal skill discovery, `found_itself`, and `connectors` (only Claude Code setups include `connectors`).
2. **`teammate-setup.ts:79` (whose ancestor skill folders are searched)**:
   - `codex` and `opencode` always search ancestor folders up to `.git`; `claude` only searches ancestors when `mode == Some("auto")`, otherwise CWD only; other skill runtimes (`copilot`, `antigravity`, `cursor`) search CWD only.
3. **`teammate-setup.ts:83` (`lent` personal skills)**:
   - `lent = !claude || claude_own_skills || mode == Some("auto")`, and `personal.found_itself = !claude`.
4. **`Composer.tsx` pre-send warnings on send**:
   - Neither `connectorsOff` nor `modelDropped` blocks sending; both are non-blocking notices shown above the composer before the send. Unavailable routes (`!s.route_can_run`) block send via `send_blocked_now`.

## Which TypeScript tests came over

- `apps/desktop/src/renderer/src/a-new-teammate-interviews-you.test.tsx` → `locust_core::interview::tests` (3 unit tests) + headless window test `creating_a_teammate_runs_the_interview`.
- `apps/desktop/src/main/a-teammates-setup-is-read-fresh.test.ts` & `apps/desktop/src/renderer/src/a-teammates-setup-is-one-command-away.test.tsx` → `locust_core::teammate_setup::tests` (5 unit tests) + headless window test `a_teammates_setup_opens_from_its_card`.
- `apps/desktop/src/renderer/src/a-draft-naming-an-off-connector-says-so.test.ts` → `locust_core::connectors_off::tests` (2 unit tests) + headless window test `a_send_with_the_connectors_off_says_so_first`.
- `apps/desktop/src/renderer/src/a-dropped-free-model-is-said-before-the-send.test.tsx` → `locust_core::connectors_off::tests` (1 unit test) + headless window test `a_dropped_free_model_is_said_before_the_send`.

## What was left out

- **Group instructions in `read_teammate_setup`**: Rust does not have conversation groups (`group-store`) yet, so `read_teammate_setup` passes `group: None` (while `gather_teammate_setup` and `setup_sections` support `group`).
- **Live `claude mcp list` subprocess for connectors**: `main.rs` reads `~/.claude.json` `mcpServers` keys locally without spawning `claude mcp list` (which can take several seconds).
- **`/tune` lessons UI**: `read_teammate_setup` counts matching `teammateId` entries in `tune-lessons.json` if present, though `/tune` itself is a separate parity row.

## Review (13 Nov)

- `scripts/review-branch.py` passed: 915 tests, a clean release build, and the four window tests alone and together.
- It flagged the setup card reading the real home folder. Under test, the card now reads an empty folder of the
  test's own (`setup_card::home_dir`).
- Break checks found two behaviours no test held:
  - **The interview's first turn runs in Ask.** It now goes through `people_panel::interview_origin`, and
    `an_interview_starts_in_ask_so_it_changes_nothing` pins it.
  - **No card outside an interview, nor while its turn runs** (Electron's own test, not carried before).
    `no_instructions_card_outside_an_interview_nor_while_its_turn_runs` fails without the guard.
