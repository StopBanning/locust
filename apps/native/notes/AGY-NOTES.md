# Antigravity in the Rust app — notes (18 Nov 2026)

Lane: `~/work/locust-wt/agy`, branch `exec/native-agy`.

## What was built

- **Cascade question & planner-text client (`core/src/antigravity_cascade.rs`)**:
  - `pending_question_in`, `planner_texts_in`, `question_interaction`, `plain_question`, `responses_from_answers`.
  - `CascadeClient` / `CascadePost` trait + `UreqCascadePost` (plain HTTP to `127.0.0.1`, passing over the TLS port's 400 `"HTTP request to an HTTPS server"`, remembering the answering port, 5 s timeout, 8 MiB response cap, `x-codeium-csrf-token` never logged or included in errors).
- **CLI routing, tier names, and resume (`core/src/runtime/antigravity.rs`, `core/src/native.rs`, `core/src/teammate_tooltip.rs`)**:
  - `agy_route` maps `flash` / `pro` / `flash_lite` and `-(low|medium|high|max)` suffixes, defaulting bare `gemini-*` models to `medium` effort; wired into `print_args`.
  - `ANTIGRAVITY_TIER_NAMES` / `antigravity_tier_name` wired into `teammate_tooltip::route_model_name` and `teammate_tooltip`.
  - `native::prior_from_records` / `prior_of` skips `antigravity:hub` routes and Antigravity `trajectory not found` failures on resume.
- **Non-blocking discovery (`core/src/agents.rs`, `core/src/models.rs`)**:
  - `discover_antigravity_concurrent` runs `agy --version` and `agy models` concurrently via `std::thread::scope`.
  - `AntigravityProbe` coalesces concurrent checks and exposes `pending_record`.
  - `models::antigravity_model_list_is_pending` distinguishes pending (`ProbeFailed` while installed) from `Ready` / `AuthRequired` / `Missing`.
- **Activity rows (`core/src/tool_fold.rs`, `src/ui/activity_drawers.rs`)**:
  - `manage_task` renders as `"Checked on a command"` instead of `"manage_task"`; refused commands already render `"refused"` without counting as failed.
- **Live `/usage` reading & AT LIMIT lift (`core/src/antigravity_usage.rs`, `core/src/limits.rs`)**:
  - `is_supported_usage_version` (`>= 1.1.11`, no prerelease, no `--disable-slash-commands`), `read_antigravity_usage_with`, `read_antigravity_usage`, `antigravity_usage_text` (Gemini + lower-remaining groups, rejecting invalid/duplicate/unsafe names), and `read_usage_without_holding` (1.5 s deadline + late callback).
  - `limits::lift_limits_from_readings` and `limits::limited_runtimes_at` lift Antigravity's limit when a cached live `/usage` reading has quota left (`reading_has_quota_left`).

## What `core/src/approval.rs` lacks (not edited per handoff)

1. `approval::Request` has no `skippable: bool` or `answer_in: Option<String>` fields, so `src/ui/question_card.rs` cannot render a `Skip` button or the read-only `"Answer it in Antigravity"` card with no buttons.
2. `approval::Channel` has no `withdraw(&self, approval_id: &str)` method to take down a single card when answered in Antigravity's own window without calling `release(mission_id)` (which marks the mission ended).

## What was not built

- **Windows `antigravity-mission` IDE service (`apps/desktop/src/main/antigravity-mission.ts`, `antigravity-host.ts` Windows `language_server.exe` probe)**: Rust currently drives Antigravity through `agy` CLI (`core/src/runtime/antigravity.rs`) and has no Windows `language_server.exe` hub polling or live transcript gap-filling loop.
