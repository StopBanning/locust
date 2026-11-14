# Scout part 1: core watch, folder check, brief, tune-up, notes & host engine (13 Nov 2026)

Ported the engine half of Scout (area 6, row 84) on `exec/native-scout` without Electron or React dependencies. Part 2 items (`/locust` GitHub comments, away GitHub workflow, phone cards, fixes in copies, and Scout UI views `ScoutCards.tsx` / `ScoutSetting.tsx`) remain out of scope for this round.

## What was built

1. **Shared types, prompts, parser, and watch rules** (`core/src/scout/types.rs`, from `shared/scout.ts` and `shared/scout-log.ts`):
   - `ScoutState`, `ScoutSettings`, `ScoutCard`, `ScoutFact`, `ScoutWeek`, `ScoutBrief`, `ScoutTune`, `ScoutNote`, `ScoutChange`, and `apply_settings_change` (`every_minutes` only one of Electron's choices, 10, 30 or 60; `goal` trimmed and capped at 600 chars; `quiet` hours and `brief_at` only from their choices).
   - Watch rule evaluators: `pull_request_facts` (`READY_WAITING_DAYS = 3`, state-keyed PR transitions), `review_facts`, `branch_facts`, `new_issues` (`NEW_ISSUE_DAYS = 7`, `ISSUES_A_LOOK = 3`, overflow notice), `waiting_facts` (`WAIT_NOTIFY_MS`), `failed_facts`, `check_facts` (transitions only, clipped tail).
   - Brief and weekly tune-up renderers: `brief_fact`, `brief_span`, `week_fact`, `days_between`, `in_quiet_hours` (daytime and midnight-crossing windows).
   - Sealed prompt composer and loose answer/note parsers: `scout_prompt`, `scout_answers`, `scout_note_of`, `notes_from` (`MAX_SCOUT_NOTES = 40`), `with_new_cards` (`MAX_SCOUT_CARDS = 30`), strict team-member matching (`HAND TO` / `DRAFT` dropped unless matching a real teammate).

2. **Look log** (`core/src/scout/log.rs`, from `shared/scout-log.ts`):
   - `look_entry`, `with_look`, `scout_log_brief`, `why_brief` (merges adjacent quiet looks, prunes entries older than `SCOUT_LOG_DAYS = 14`, renders readable look reasons and held phone alerts).

3. **Folder mark & GitHub reader** (`core/src/scout/folder.rs`, `core/src/scout/github.rs`, from `main/scout-folder.ts`, `main/scout-github.ts`):
   - `folder_mark_of`: 32-hex SHA-256 over git `HEAD`, `status --porcelain -z` paths (up to 200 files), sizes, and mtimes; `None` outside git checkouts.
   - `read_scout_github` and `github_unread_why`: fetches open PRs (`updatedAt`), review requests, default-branch workflow runs (excluding `AWAY_WORKFLOW_NAME`), and open issues via `gh`; stops on first error and classifies `NotOnGithub`, `SignedOut`, `NoCli`, and `Unreadable`.

4. **Durable storage & scheduler** (`core/src/scout/store.rs`, `core/src/scout/scheduler.rs`):
   - `ScoutFiles`: reads Electron `scout.json` / `scout-looks.json` read-only on first run; writes atomically ONLY to the native `Store`. Retries busy read errors before falling back to memory-only without overwriting an unreadable file; renames corrupt JSON aside (`*.unreadable-<stamp>`).
   - `ScoutScheduler`: trait with `RealScheduler` (production background timer thread) and `FakeScheduler` (deterministic virtual clock for tests).

5. **Stateful Scout engine** (`core/src/scout.rs`, from `main/scout.ts`):
   - `Scout`: turn on/off (creating/reusing a `Custom` / `Keeps watch` teammate), periodic and deferred looks (`wait_while_working`, `SCOUT_WAIT_MOST_MS = 30m` ceiling, `WAIT_STEP_MS = 60s`), `quiet` hours suppression vs manual `look`, folder-check gating (only when `folder_mark` moves and never while busy; discards result and retries when a turn begins mid-check).
   - Model turns (`ask`), `ANSWER_WAIT_MS = 3m` answer timeout, `MAX_FALLBACKS = 2` free-model fallbacks (`next_free_model`), `provider_refused` retry trigger, dismissal tracking, daily morning brief, weekly tune-up card + `tune` actions, and dated `notes` deduplication/editing.

6. **Runtime sealing & native host wiring** (`core/src/scout/host.rs`, `core/src/native.rs`, `core/src/ledger.rs`, `core/src/runtime/{codex,opencode,opencode_config}.rs`):
   - `SlotOrigin.sealed` passed through `native::send_from`, `engine_process`, Codex (`codex::sealed_config`: disables `features.shell_tool`, `features.apps`, `features.plugins`, `web_search`, local/remote MCP; omits workspace brief, skills, developer instructions), and OpenCode (`opencode_config::sealed_config`: refuses every tool inside the turn).
   - `native.rs` guards `record_reply` and `share_reply_for_relay` with `!sealed`, forwards `adapter.diagnostic` matching `^opencode\.runtime_error$|\.provider_busy\.runtime_error$` to `scout::host::on_diagnostic`, and calls `scout::host::on_run_ended` on every finished/failed/cancelled turn.
   - `ledger::try_list_recent` hides turns where `m.started_by_kind.as_deref() == Some("scout")`.
   - `ScoutHostDeps` connects `Scout` to `Store`, `engine_api::api()`, `people`, `ledger::read_metadata`, `edit_check`, `gh` (`GH_PROMPT_DISABLED=1`, `GH_NO_UPDATE_NOTIFIER=1`, 30s timeout), `git` (10s timeout), and `is_free_route` / `FREE_MODELS_BEST_FIRST`.

## Judgement calls

- **Split engine vs UI/Part-2 cleanly via `ScoutDeps` and `ScoutHost`**: `Scout` accepts `Arc<dyn ScoutDeps>` and `Arc<dyn ScoutScheduler>`, allowing all 58 stateful TypeScript test behaviors + helper suites to run deterministically with virtual time (`FakeScheduler`) while `ScoutHostDeps` + `hooks::install` verify real native `engine_api` execution end-to-end.
- **Keep free-route detection aligned with `free-routes.ts`**: ported `is_free_route` in `scout/host.rs` for `opencode`, `kilo`, and `acp` (`acp_...::opencode/...-free`), ensuring `next_free_model` never escalates to a paid route.
- **Check `startedBy.kind == "scout"` in `try_list_recent`**: Electron hides Scout looks via `side || startedBy?.kind === 'scout'`. Native sets `started_by: {"kind": "scout"}` without forcing `is_side`, so `try_list_recent` filters both `m.side` and `m.started_by_kind.as_deref() == Some("scout")` while keeping `!sealed` memory isolation in `native.rs`.

## Special cases audit (`scripts/special-cases.py apps/desktop/src/main/scout.ts`)

`special-cases.py` reported 7 `kind` switches in `main/scout.ts`. All 7 belong to Part 2 and do not apply in Part 1:

- `main/scout.ts:940, 941` (in `still`), `:1102, 1104, 1106` (in `card`): `outcome.kind` / `brought.kind` for `ScoutFix` preparation and copy application (`main/scout-fix.ts`, Part 2).
- `main/scout.ts:1021` (in `post`): `letter.kind === 'look'` for `scout-away` absorbed letters (`main/scout-away.ts`, Part 2).
- `main/scout.ts:1142` (in `onRunEnded`): `commentState` check for `/locust` comment turns (`main/scout-comments.ts`, Part 2).

## Break checks (`cargo test -p locust-core --lib`)

Verified one-by-one (restoring each mutation before the next check and confirming clean `git diff`):

| Mutation | Failed test(s) caught |
| --- | --- |
| `SCOUT_WAIT_MOST_MS` (`30 * 60_000` -> `0`) in `scout/types.rs` | `holds_due_look_while_person_works_and_looks_once_it_ends`, `looks_anyway_after_half_an_hour_so_a_long_run_never_silences_it` (2 failed) |
| `MAX_FALLBACKS` (`2` -> `0`) in `scout.rs` | `does_not_wait_forever_on_a_model_that_never_ends_its_turn`, `provider_refused_stops_run_and_asks_next_free_model_at_once`, `tries_next_free_models_then_leaves_cards_in_locusts_words_and_says_why` (3 failed) |
| `ANSWER_WAIT_MS` (`3 * 60_000` -> `10`) in `scout.rs` | `does_not_wait_forever_on_a_model_that_never_ends_its_turn`, `host_end_to_end_turn_on_runs_sealed_look_and_populates_cards_and_notes` (2 failed) |
| `!sealed` memory guard removed in `native.rs` (`if completed && !is_side`) | `host_end_to_end_turn_on_runs_sealed_look_and_populates_cards_and_notes` (`memories.json` created unexpectedly) |
| `started_by_kind == Some("scout")` filter removed in `ledger.rs` `try_list_recent` | `host_end_to_end_turn_on_runs_sealed_look_and_populates_cards_and_notes` (Scout look appeared in recent list) |
| `scout::host::on_run_ended` removed in `native.rs` | `host_end_to_end_turn_on_runs_sealed_look_and_populates_cards_and_notes` (timed out waiting for look answer) |
| `READY_WAITING_DAYS` (`3` -> `30`) in `scout/types.rs` | `notices_pull_request_passing_checks_waiting_ready_waiting_days_then_only_once` (1 failed) |
| `MAX_SCOUT_NOTES` (`40` -> `2`) in `scout/types.rs` | `notes_from_settings_brief_span_and_week_fact_match_electron` (1 failed) |
| `NEW_ISSUE_DAYS` (`7` -> `0`) in `scout/types.rs` | `log_writes_each_reason_as_it_happens`, `notices_new_issues_within_a_week_newest_first_capped_at_three_and_more_overflow` (2 failed) |
| `codex::sealed_config` switched to `thread_config(settings, false)` | `sealed_slot_sends_plain_prompt_without_tools_or_briefs_and_roundtrips_origin`, `codex_sealed_config_turns_off_shell_web_apps_and_connectors`, `host_end_to_end_turn_on_runs_sealed_look_and_populates_cards_and_notes` (3 failed) |

## Which TypeScript tests came over

| TypeScript file | TS `it` count | Rust destination |
| --- | --- | --- |
| `apps/desktop/src/main/scout-keeps-its-goal-hours-and-word.test.ts` | 17 | `core/src/scout/runner_tests.rs` |
| `apps/desktop/src/main/scout-keeps-watch.test.ts` | 16 | `core/src/scout/runner_tests.rs` |
| `apps/desktop/src/main/scout-brings-a-morning-brief.test.ts` | 8 | `core/src/scout/brief_tests.rs` |
| `apps/desktop/src/main/scout-brings-a-weekly-tune-up.test.ts` | 7 | `core/src/scout/brief_tests.rs` |
| `apps/desktop/src/main/scout-says-what-it-saw.test.ts` | 5 | `core/src/scout/brief_tests.rs` |
| `apps/desktop/src/main/scout-keeps-notes-you-can-read.test.ts` | 5 | `core/src/scout/brief_tests.rs` |
| `apps/desktop/src/main/scout-runs-the-check-only-when-the-folder-moved.test.ts` | 2 | `core/src/scout/runner_tests.rs` |
| `apps/desktop/src/main/a-scout-file-it-cannot-read-is-never-overwritten.test.ts` | 3 | `core/src/scout/unit_tests.rs` |
| `apps/desktop/src/main/scout-reads-what-github-has.test.ts` | 2 | `core/src/scout/unit_tests.rs` |
| `apps/desktop/src/main/scouts-look-is-sealed.test.ts` | 2 | `core/src/native/tests.rs`, `core/src/runtime/{tests,opencode_tests}.rs`, `core/src/scout/host_tests.rs` |
| `apps/desktop/src/shared/scout-notices-what-is-new.test.ts` | 15 | `core/src/scout/unit_tests.rs`, `core/src/scout/brief_tests.rs`, `core/src/scout/runner_tests.rs` |

Additional native integration & helper tests in `core/src/scout/host_tests.rs` (8 tests) verify `is_free_route`, `next_free_model`, `make_teammate` / `teammate_exists`, `brief_facts` 80-mission cap, `run_check` tail clipping, `on_diagnostic` filtering, and end-to-end sealed turn completion via `native::hooks`.

## What remains (Part 2)

- UI components (`ScoutCards.tsx`, `ScoutSetting.tsx` equivalent under `src/ui/`).
- `/locust` GitHub comment watch & reply workflow (`main/scout-comments.ts`).
- Away workflow sync & absorbed letters (`main/scout-away.ts`, `main/scouts-away-look-tells-only-what-is-new.test.ts`).
- Phone alerts (`main/phone-scout-cards.ts`, `main/scouts-cards-are-on-your-phone.test.ts`).
- Fixes prepared in isolated copies (`main/scout-fix.ts`, `main/scout-prepares-the-fix.test.ts`).
