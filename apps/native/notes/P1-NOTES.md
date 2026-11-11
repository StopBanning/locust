# Native parity round 1

This round implements the four requested activity surfaces. It does **not** establish complete Electron visual/behavior parity: three checklist rows remain partial for the concrete gaps below. All edits are in this native copy. No runtime or ledger mutation was added, and no real ledger text was used.

## Changes by file

- `src/ledger.rs`: added structured plans, public thoughts with duration, and tool details (arguments/command, output, reported-empty output, exit code, elapsed time, helper type, parent ID, background lifecycle/progress and children). Keeps the latest plan and preserves its actual step states. Completion updates the existing tool row; helper calls nest by `parentItemId`, including nested helpers. IDs are qualified by mission so continuation turns can reuse runtime IDs; ended turns retain their ended state. The module still only reads production ledger files. Existing test helpers write synthetic temporary files as before.
- `src/ledger.rs`: drops raw evidence and raw reasoning fields at JSONL decode. Thoughts consume only normalized `message`, never raw `content`, raw items or evidence. Public display strings are scrubbed using the adapter's secret patterns, then capped at 131,072 UTF-16 units with a truncation marker. Plans cap at 256 steps. Output previews keep eight head and eight tail lines; explicit expansion shows the retained output. Synthetic JSONL regression tests cover tool arguments/output and elapsed time across midnight, plan replacement and step spelling/state variants, nested helpers separated by unrelated work, background helper completion, public reasoning and duration, secret scrubbing/bounds, and hidden reasoning that must not produce text.
- `src/main.rs`: conversation/item keyed click disclosures, folded adjacent activity groups, plan counts and states, helper children/report drawers, thought duration and text drawers, and output expansion. Groups/tools/helpers/thoughts start folded; plans show their steps initially. Duration-only thoughts have no empty inner drawer. Finished unsettled work says `did not report`; failed, declined, refused, background and exit statuses follow Electron's words. Added pure status/default-state regression checks. Added synthetic `thread-rich` and `thread-rich-light`, plus `thread-rich-open` and `thread-rich-open-light` for inspecting expanded content without changing production defaults.
- `Cargo.toml`, `Cargo.lock`: added `regex` (and its resolved transitive crates) to apply the runtime adapter's secret-redaction patterns at the native display boundary. No date/time or UI dependency was added.
- `PARITY.md`: updated only the four requested Area 1 rows; remaining parity gaps are explicit.

## Electron source evidence

Paths below are relative to `/Users/locust/work/locust` and were read as references only.

- `packages/runtime-adapters/src/app-server-events.ts:426–440`: Codex app-server reasoning exposes **summary only** as normalized `message`. Its raw `content` is never kept. Native follows the normalized event rather than reinterpreting a raw summary/item.
- `packages/runtime-adapters/src/codex-events.ts:563–571` (`sanitizeReasoningRecord`) and `:959–975`: the older exec reasoning item is stripped, and its normalized reasoning steps contain no text. Native never mines the raw/evidence item for words. The synthetic hidden case also confirms a duration-only step can have a row without exposing hidden text.
- `packages/runtime-adapters/src/codex-events.ts:453–475`: secret-redaction patterns copied for display scrubbing. `ToolPayload` in that file defines output, command, duration, background and `parentItemId`; `packages/mission-store/src/index.ts` validates normalized step/tool payloads.
- `apps/desktop/src/renderer/src/missionView.ts:1396` (`readPlan`): plan spellings and state normalization. `:1795` (`durationText`): rounding and elapsed-time wording. `:3378–3540`: own/helper call separation and start/completion updates. `:3596–3728`: background progress/end events, normalized reasoning messages and explicit-duration/observed-time fallback. A thought without words renders only at one second or longer.
- `apps/desktop/src/renderer/src/components/ActivityCard.tsx:38` (`thoughtLine`), `:426–449` (entry defaults), `:643–685` (helper rows), `:690–868` (shell/thought drawers), `:1166–1260` (helper/shell result words). `missionView.ts:6005–6050` supplies eight-head/eight-tail output bounding; ActivityCard's middle control says `N more lines`.
- `apps/desktop/src/renderer/src/components/Thread.tsx:477–506`: step groups start closed, including the newest group. `ThreadItems.tsx:710–880`: `PLAN`, `N of M done`, step states, and `not checked off` for ended unfinished plans. Native preserves unfinished states rather than marking them done at run completion.

## Verification

Used `/Users/locust/.cargo/bin` in PATH because the shell's initial PATH did not contain cargo. Ran the requested release test command with `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p1`: 64 passed, 0 failed, 2 existing live-data tests ignored. Ran `cargo build --release` with the same target directory.

Headless snapshots use the release executable, `--size 900x900 --scale 2`. The requested `--snap /tmp/p1.png --scene thread-rich` and the light/open variants are generated and visually inspected. No window was opened. The first expanded snapshot exposed overflow on a helper summary; its heading now has a constrained wrapping width.

## Fixes

Review findings fixed in this folder (`src/ledger.rs`, `src/main.rs`), each with a synthetic test:

1. **Display strings scrubbed** — user prompts, `message.delta`, diagnostics, route-limit and `run.failed` notes now pass through the TS secret patterns. Test: `display_strings_and_json_arguments_follow_ts_secret_rules`.
2. **SENSITIVE_KEY + multi-word JSON values** — tool argument objects are walked with `sanitizeJson`-style key redaction (`passwd`/`password`/… → `[redacted]`) and capped before `to_string_pretty`. Test: same as (1), plus `event_map_is_bounded_and_json_is_capped_before_pretty_print`.
3. **Title over command** — a later `tool.completed` with only `command` no longer overwrites an earlier `title`. Test: `tool_keeps_title_when_completion_only_has_command`.
4. **Orphaned `parentItemId` children dropped** — children whose helper parent is missing are removed, matching `missionView.ts`. Test: `orphaned_parent_item_id_children_are_dropped`.
5. **Step start times cleared on completion** — reused/`itemId`-less reasoning steps no longer inherit the previous start. Test: `step_start_times_clear_so_reused_ids_do_not_inherit_duration`.
6. **Event map bounded; JSON capped before pretty-print** — `Transcript::events` keeps at most `MAX_EVENTS` (2000); argument JSON is depth/key/array-capped before serialization. Test: `event_map_is_bounded_and_json_is_capped_before_pretty_print`.
7. **Unique plan drawer keys** — plans use `{scope}/plan/{id}` so they do not collide with a tool whose `itemId` is `"plan"`. Test: `plan_drawer_keys_do_not_collide_with_tool_item_ids`.
8. **Thought headlines** — single-thought groups strip the headline in the body (`without_headline`); multi-item thought rows show `Thought for … · {headline}` and keep full text in the drawer. Test: `thought_headline_helpers_match_activity_card`.
9. **Background / 0 calls / no output / duration wording** — helpers omit the shell background badge suffix and omit `0 calls`; `no output` and elapsed time are shell-only; declined/refused shells omit duration. Test: `tool_row_label_omits_zero_calls_background_dupes_and_non_shell_no_output`.

## Remaining gaps and unverified behavior

- Tools: all requested retained arguments/output are inspectable, but Electron's full `stepsLine` grammar, shell-command interpretation, tool-kind classification, consecutive read/search aggregation, adaptive shorter labels and specialized file/diff rows are not ported. Native groups adjacent tool/thought blocks and can use a thought headline; this is not the full Electron grouping algorithm.
- Plans: states/counts and expand/collapse work; the animated Rubik header and live-line plan peek are absent. Native uses a static active marker. It does not let the person alter the runtime's checklist.
- Helpers: nested calls and report text are inspectable, including further nested helpers. Native uses the common drawer mechanism rather than Electron's exact helper row/transcript presentation. It does not yet apply all of Electron's Markdown-to-one-line summary formatting or adaptive group labels.
- Reasoning: duration/public-text folding and hidden-text exclusion are implemented and synthetic-tested. Renderer-wide semantic/pixel equivalence with Electron is not established.
- Tests exercise decoding, status words, synthetic scene defaults and independent drawer state; they do not dispatch a real mouse click through Rune. No live engine, real-ledger replay, animated timing, accessibility interaction or Electron side-by-side screenshot comparison was tested. The two pre-existing ignored tests remain ignored. No claim is made about runtimes beyond their normalized event shapes.
