# Tune Teammate, Combine & Lessons/Skills Notes (PARITY Row 54)

Implemented PARITY row 54 ("Tune teammate, combine alternatives and keep measured lessons/skills") in `apps/native`:

## What Was Built

1. **Core Tune Domain & Stores (`apps/native/core/src/`)**
   - `skill_runtimes.rs`: Canonical skill runtime registry (`SKILL_RUNTIMES`: `claude`, `codex`, `opencode`, `copilot`, `antigravity`, `cursor`), `takes_skills`, `SKILL_RUNTIME_NAMES`, `SKILL_RUNTIME_ANY`.
   - `tune.rs`: `TuneSkill`, `TuneEdit`, `TuneDrafts`, `TuneMeasured`, `TuneEditVerdict`, `TuneLessonConfidence`, `TuneEditOutcome`, `TuneLessonDraft`, `ParsedTuneLesson`, `PublicTuneLesson`, `PublicTeammateSkill`, `TuneCombineSetups`, `TuneLessonColumnEntry`; prompt builders (`tune_draft_prompt`, `tune_lesson_prompt`, `tune_combine_prompt`); validators/parsers (`validate_tune_skill`, `skill_markdown`, `parse_tune_drafts`, `parse_tune_combine`, `tune_lesson_line`, `parse_tune_lesson`); helpers (`repeats_of`, `tune_routes`, `kept_skill`, `other_skill`, `combine_setups`, `tune_measured_of`).
   - `tune_lesson_store.rs`: Native `tune-lessons.json` store with Electron fallback read (`add`, `list`, `remove`, `remove_for_teammate`), per-teammate cap of 20 newest-last, dedupe on `teammateId:compareId`, and control-char stripping via `clean_field`.
   - `teammate_skills.rs`: Native `teammate-skills/skills.json` + `<teammate>/<name>/SKILL.md` store with atomic staging/write (`add`, `list`, `remove`, `remove_for_teammate`, `folder_of`, `skill_names_for`, `kept_skill_text`), cap of 20 per teammate, and seeding from Electron's read-only store on first native write.
   - `compare.rs` & `compare_copies.rs`: Added `CompareRoute.skill`, `column_instructions`, `setup_name`, `compare_instructions_refusal`, `compare_skill_refusal`, and finished all remaining ported test cases from `a-comparison-keeps-each-columns-instructions`, `a-column-can-be-tried-again`, and `a-copy-column-brings-its-changes-in`.
   - `skills.rs`, `teammate_setup.rs`, `people.rs`, `native.rs`: Added `Sources.candidate` (wins collisions and is scoped strictly to the candidate column's run), `sources_for_store` (reading native `teammate-skills` before Electron), `people::brief_with_instructions` (never includes `tune-lessons.json`), cleanup of lessons/skills in `people::remove_teammate`, and `SlotOrigin.skill` suppressed on `sealed: true` side turns.

2. **UI & Engine Wiring (`apps/native/src/ui/`)**
   - `tune_teammate.rs`, `app_state.rs`, `composer_wiring.rs`, `main.rs`: Added `/tune` (including `/tune <ask>`), `TuneCardState`, 3-column preview (`Current`, `How it works`, `What it knows`), repeat-setup chips, `TuneSkill` toggle/stashing across `skill_aside`, and launch of blind shuffled 3-column comparisons (`changes: true, blind: true`, same model/route across all 3).
   - `compare_view.rs`: Added kept-tune banner & strip (`Use these instructions for <name>`, `Use the combined instructions for <name>`, `Combine both edits`, `Use this skill for <name>`, replacement notice, `Review the skill` / `Review the instructions`, `Note what this tune showed` with measured `TuneMeasured` benchmark context, `Remove`, `Not now`), plus `"not by setup"` / setup-only suppression on kept tune benchmarks.
   - `people_panel.rs`: Added per-teammate `Tune lessons` and `Skills` sections with `Remove` buttons in Edit teammate.

## Special Cases Verified (Break-Checks)

Each guard was temporarily broken, verified to fail its pinning test, and restored:
- **`tune_lesson_line` vague-phrase filter** (`core/src/tune.rs`): Disabling the vague-phrase check caused `tune::tests::a_lesson_reads_as_one_sentence` to fail on `"When using this setup, answers improved."`.
- **`combine_setups` Current exclusion** (`core/src/tune.rs`): Removing the `Current` label guard caused `tune::tests::combine_setups_and_parser_reject_duplicates_or_baseline` to fail (`combine_setups(&kept_cur).is_none()`).
- **`skills::prepare_run` candidate collision priority** (`core/src/skills.rs`): Skipping the candidate pre-copy step caused `skills::tests::tune_candidate_skill_is_handed_to_run_and_claude_and_wins_name_collision` to fail (`run_md.contains("Check candidate rule first.")`).
- **`parse_tune_drafts` NUL & 4,000-char instruction guards** (`core/src/tune.rs`): Removing `\0` and length checks caused `tune::tests::drafts_parser_handles_plain_and_skill_variants_and_repeats` to fail.
- **`tune_routes` `auto`-mode gate** (`core/src/tune.rs`): Forwarding `base_mode` unconditionally caused `tune::tests::drafts_parser_handles_plain_and_skill_variants_and_repeats` to fail (`r_ask.iter().all(|r| r.mode.is_none())`).

## TypeScript Tests Carried & Left Out

- **Carried (11 Tune files + 3 completed Compare files)**:
  - `main/a-tune-can-give-a-claude-teammate-a-skill.test.ts`
  - `main/a-tune-is-measured-before-one-is-kept.test.ts`
  - `main/combining-a-tunes-two-edits.test.ts`
  - `main/earlier-tunes-reach-only-the-next-tune.test.ts`
  - `renderer/src/a-tune-can-give-a-claude-teammate-a-skill.test.tsx`
  - `renderer/src/a-tune-is-measured-on-screen.test.tsx`
  - `renderer/src/a-tune-names-its-setups.test.tsx`
  - `renderer/src/a-tune-says-when-a-setup-repeats.test.tsx`
  - `renderer/src/combining-a-tunes-two-edits.test.tsx`
  - `renderer/src/noting-what-a-tune-showed-is-explicit.test.tsx`
  - `renderer/src/tuning-never-applies-instructions-by-itself.test.tsx`
  - Plus remaining cases of `main/a-comparison-keeps-each-columns-instructions.test.ts`, `main/a-column-can-be-tried-again.test.ts`, and `main/a-copy-column-brings-its-changes-in.test.ts`.
- **Partial**:
  - `main/codex-and-opencode-teammates-get-their-skills.test.ts` -> Candidate skill delivery, collision priority, `skill_runtimes`, native rules/cleanup, and Claude/Codex wiring carried; cloud-computer exclusion, symlink/case-disk copies, and OpenCode/Copilot/Antigravity config/CLI flag delivery wait on their runtime adapters.
- **Left out (`waiting`)**:
  - `main/scout-brings-a-weekly-tune-up.test.ts` -> Depends on Scout (PARITY row 84, not yet built).

## Standalone Headless Window Tests (`src/ui/window_drive_tests.rs`)

- `slash_tune_opens_drafts_two_setups_and_saves_nothing`
- `tune_combine_edits_shows_combined_instructions_and_saves_only_when_pressed`
- `tune_note_lesson_stores_lesson_and_next_tune_draft_prompt_includes_it`

## Review (13 Nov)

The scripted review passed: 1,245 tests, no warnings, and the three window tests alone and together three times.
Checked by hand:
- nothing is applied by itself (drafting, keeping and combining save nothing until pressed);
- earlier lessons reach only `/tune`, never an ordinary brief;
- the model-or-account refusal uses Electron's words;
- the prompts quote data as JSON and cut at 4,000 and 400 characters.

Found and fixed:
- **Noting a lesson did not wait for the kept run to finish.** It could hand the lesson turn a half-written answer.
  `start_tune_note` now refuses with Electron's words, "Wait for the kept run to finish first. Nothing was started.",
  and `tune_note_lesson_stores_lesson_and_next_tune_draft_prompt_includes_it` checks it before noting.
- **Two files were marked carried on one test each.** `a-tune-is-measured-before-one-is-kept` (7 cases, 5
  assertions) and `a-tune-is-measured-on-screen` (4 cases, sharing a test with `a-tune-names-its-setups`) are now
  partial.
