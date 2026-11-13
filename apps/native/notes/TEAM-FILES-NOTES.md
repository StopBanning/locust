# Starter teams, team cards, and routine files/inputs in Rust (Hydrogen, 13 Nov 2026)

Notes on porting three PARITY rows:
- **PARITY row 44 ("Starter teams and reusable routine chains")**:
  - `apps/desktop/src/renderer/src/components/TeamTemplates.tsx`, `apps/desktop/src/shared/team-templates.ts`, `apps/desktop/src/main/routine-templates.ts`, `apps/desktop/src/shared/chain-proposal.ts`
- **PARITY row 52 ("Team image import/export with routines")**:
  - `apps/desktop/src/renderer/src/components/TeamCard.tsx`, `apps/desktop/src/shared/team-card.ts`, `apps/desktop/src/main/team-card-routines.ts`, `apps/desktop/src/main/png-text.ts`, `apps/desktop/src/main/index.ts` (team card export/import)
- **PARITY row 100 ("Routine input forms, Markdown import/export and starter templates")**:
  - `apps/desktop/src/renderer/src/components/RoutineInputs.tsx`, `apps/desktop/src/renderer/src/components/RoutineFileDialog.tsx`, `apps/desktop/src/renderer/src/components/RoutineTemplates.tsx`, `apps/desktop/src/main/routine-file.ts`, `apps/desktop/src/main/routine-io.ts`, `apps/desktop/src/shared/routine-inputs.ts`, `apps/desktop/src/main/routine-runner.ts` (W7 inputs)

## What was built

- **Starter teams & teammate persistence (`src/starters.rs`, `src/ui/home_panel.rs`, `core/src/people.rs`)**:
  - Fixed `home_panel.rs` `apply_template` to omit the empty `route.model` so starter template teammates save cleanly with their `hue` and `starters` intact.
  - Updated `people::save_teammate_id` to validate `starters` (1..=4 items, each <=300 UTF-16 units), preserve existing `starters` on edit when omitted, preserve `hue`, and allow omitted/`null` `route`.
- **Team image export/import with routines (`core/src/png_text.rs`, `core/src/team_card.rs`, `src/ui/people_panel.rs`, `src/ui/home_panel.rs`)**:
  - Ported PNG signature check, `tEXt` chunk read/write with CRC32 (`with_text_chunk`, `read_text_chunk`), and a pure-Rust uncompressed-DEFLATE RGBA PNG renderer (`render_team_card_png`).
  - Ported `team_card_of` (strips ids, folders, limits, worktree, own models), `team_card_routines` (workspace-scoped, turns absolute paths into inputs, detects connectors), `read_team_card` (downgrades `auto` to `accept-edits`), `export_team_card`, `import_team_card_bytes`, and `import_team_card_file`.
  - Wired `"Share your team"` / `"Save image"` and `"Add team from image"` in `people_panel.rs` and replaced the `TEAM_FROM_IMAGE` stub in `home_panel.rs`.
- **Routine inputs, Markdown/JSON import/export & starter templates (`core/src/routine_inputs.rs`, `core/src/routine_file.rs`, `core/src/routine_templates.rs`, `core/src/routine_io.rs`, `core/src/routines.rs`, `core/src/routine_runner.rs`, `assets/routines/*.locust-routine.json`, `src/ui/routines_panel.rs`)**:
  - Ported `RoutineInputKind` (`text`, `long-text`, `choice`, `folder`), `placeholders_in`, `undeclared_placeholders`, `inputs_refusal`, `resolve_values`, and `substitute_steps`.
  - Ported YAML-frontmatter Markdown and JSON routine parsing/export (`parse_routine_file`, `routine_markdown`, `routine_file_text`, `routine_export_text`, `routine_file_name`), `absolute_paths_in`, `paths_as_inputs`, `connectors_named_in`, and `when: hook` schedule handling.
  - Bundled all 16 shipped `.locust-routine.json` templates in `assets/routines/` via `include_str!` with `list_shipped_templates`, `read_shipped_template`, `template_meta`, `templates_line`, `propose_teammates`, and `proposed_hand_offs`.
  - Ported host-side `RoutineIO` with one-use preview tokens and chosen-folder receipts, and wired `RoutineRunner::run_with_inputs` (W7) so manual runs validate inputs and persist filled steps across edits while automatic runs use defaults or emit `Blocked`.
  - Wired the Inputs editor, `Run` input dialog (with folder picker), `"Start from a template"` + empty shelf starter list, `"Open routine"`, and `"Save as file"` (with absolute-path dialog) in `routines_panel.rs`.

## Electron's special cases & break checks

1. **`routine-file.ts:116,154,296` (`when: hook`)**:
   - `schedule: { kind: "hook" }` travels in routine files as `when: hook` in Markdown frontmatter with nothing else in `schedule`; any other schedule object or extra field is refused with `"That routine file has a trigger Locust does not read."`.
   - **Break check**: Temporarily removed the `schedule` validation in `core/src/routine_file.rs` (`parse_routine_file`), ran `hook_schedule_carries_and_other_schedules_are_refused`, observed failure (`assertion failed: parse_routine_file(extra_sched).is_err()`), and restored it.
2. **`routine-io.ts:76` (folder picker receipt)**:
   - A `folder` input must be chosen via the host folder picker (`RoutineIO::register_folder`); a typed path is refused with `"Choose \"<label>\" with the folder picker before running."`.
   - **Break check**: Temporarily disabled `!self.folder_was_chosen(path)` in `core/src/routine_io.rs` (`validate_run_inputs`), ran `refuses_typed_folder_paths_until_picker_selected_them`, observed failure (`unwrap_err()` on `Ok`), and restored it.
3. **`team-card.ts:113` (`auto` -> `accept-edits` safety downgrade)**:
   - A teammate read from a team card never arrives in `auto`; its mode becomes `accept-edits`.
   - **Break check**: Temporarily removed the `auto` -> `accept-edits` rewrite in `core/src/team_card.rs` (`read_team_card`), ran `auto_mode_on_team_card_arrives_as_accept_edits`, observed failure (`left: String("auto"), right: "accept-edits"`), and restored it.

## Which TypeScript tests came over

- `apps/desktop/src/main/a-team-template-makes-ordinary-teammates.test.ts` → `core/src/people.rs` (`template_hue_and_starters_are_preserved_and_invalid_starters_rejected`), `src/starters.rs`, and `src/ui/window_drive_tests.rs` (`starter_team_makes_its_teammates`).
- `apps/desktop/src/main/the-chain-templates-hand-work-on.test.ts` → `core/src/routine_templates.rs` (`tests`).
- `apps/desktop/src/main/the-starter-routines-are-routines.test.ts` → `core/src/routine_templates.rs` & `core/src/routine_io.rs` (`template_previews_and_imports_once_in_ask_mode_requiring_workspace`).
- `apps/desktop/src/renderer/src/a-new-shelf-offers-starter-routines.test.tsx` → `core/src/routine_templates.rs` (`template_meta` / `templates_line` tests) & `src/ui/routines_panel.rs`.
- `apps/desktop/src/renderer/src/a-team-without-a-coder-gets-its-own-starters.test.tsx` → `src/starters.rs` (`tests`).
- `apps/desktop/src/main/a-routine-is-a-markdown-file.test.ts` → `core/src/routine_file.rs` (`tests`) & `src/ui/window_drive_tests.rs` (`routine_saved_as_markdown_and_opened_again_is_the_same_routine`).
- `apps/desktop/src/main/a-routine-asks-for-inputs-and-travels-as-a-file.test.ts` → `core/src/routine_inputs.rs`, `core/src/routine_io.rs`, & `src/ui/window_drive_tests.rs` (`routine_with_inputs_asks_for_them_and_runs_with_them`).
- `apps/desktop/src/main/a-team-card-carries-its-routines.test.ts` → `core/src/team_card.rs`, `core/src/png_text.rs`, & `src/ui/window_drive_tests.rs` (`team_card_exported_and_imported_brings_routines_and_auto_arrives_in_edit`).
- `apps/desktop/src/renderer/src/a-team-card-says-what-it-waits-on.test.tsx` → `core/src/team_card.rs` & `src/ui/people_panel.rs` (`share_card_view`).
- `apps/desktop/src/main/routine-runner.test.ts` (W7) → `core/src/routine_runner_tests.rs` (`w7_*` tests).

## What was left out

- No live-window / real-agent run was performed for these three rows (all headless window tests use `rune::Runtime` and `scripts/fixtures/m1-app-server.py`).
- Team card PNG rendering (`render_team_card_png`) draws a valid 720×320 RGBA PNG card carrying the `tEXt` metadata chunk without a full rasterized typeface/avatar renderer.
