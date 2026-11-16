#!/usr/bin/env python3
"""Run one renderer regression against the workspace, restoring source even on failure.

Run from apps/native: python3 scripts/fixtures/renderer-mutation.py name
No filtered cargo runs. A compiler error does not count as catching a mutation.
"""
import os
from pathlib import Path
import subprocess
import sys

CONTROLS = {
    "work-owner": (
        "core/src/teammate_work.rs",
        'owners.get(&m.mission_id).is_some_and(|owner| owner == id)',
        'owners.get(&m.mission_id).is_some()',
        "counts_only_the_missions_the_host_recorded_as_theirs",
    ),
    "recorded-runtime": (
        "src/ui/home_panel.rs",
        '.or(route)\n}',
        '; route\n}',
        "still_prefers_a_running_mission_then_a_past_one_over_the_route",
    ),
    "plain-ghost": (
        "src/ui/messages.rs",
        'hue: "pearl".into(), parts: None, bot: Some(("ghost".into(), "eyes".into())),',
        'hue: "lime".into(), parts: None, bot: Some(("swarm".into(), "eyes".into())),',
        "beside_a_reply_the_ghost_when_the_conversation_is_nobodys_the_teammates_own_bot_when_it_has_one",
    ),
    "idle-roster-cache": (
        "src/main.rs",
        "s.people_panel.team = team;",
        "let _ = team;",
        "roster_refresh_keeps_the_template_starters_before_the_team_screen_is_opened",
    ),
    "pending-reach": (
        "core/src/teammate_reach.rs",
        "(usable || !runtimes.iter().any(|r| r.checking)).then_some(usable)",
        "Some(usable)",
        "has_no_global_blocked_verdict_while_the_only_possible_agent_is_still_checking",
    ),
    "idle-own-starters": (
        "src/ui/idle_teammate.rs",
        "own.unwrap_or_else(|| role_starters(role))",
        "role_starters(role)",
        "offers_a_template_teammates_own_first_messages",
    ),
    "routine-owner": (
        "src/ui/routines_panel.rs",
        'if !s.teammates.iter().any(|t| t.id == owner) {',
        'if false {',
        "will_not_save_until_that_is_answered",
    ),
    "newest-budget": (
        "core/src/thread_view.rs",
        'used.saturating_add(sizes[first - 1]) <= budget',
        'used.saturating_add(sizes[first - 1]) <= usize::MAX',
        "the_newest_turns_whose_rows_fit_oldest_first_index",
    ),
    "working-cycle": (
        "core/src/bot_rig.rs",
        'WORKING_BEATS[(start + count as usize) % WORKING_BEATS.len()]',
        'WORKING_BEATS[start % WORKING_BEATS.len()]',
        "goes_round_all_of_them_in_turn_each_face_from_a_place_of_its_own",
    ),
    "ring-plan": (
        "core/src/benchmark.rs",
        'if shown == "plan" || shown.is_empty() { return None; }',
        'if shown.is_empty() { return None; }',
        "gives_the_ring_nothing_to_say_about_cost_only_the_context",
    ),
    "plan-format": (
        "core/src/benchmark.rs",
        'if c.plan {',
        'if false {',
        "says_on_its_receipt_that_the_plan_covered_it",
    ),
    "queue-isolation": (
        "src/composer/queue.rs",
        'if next.key != front.key',
        'if false',
        "never_folds_into_another_conversations_message",
    ),
    "plan-receipt": (
        "core/src/inspector.rs",
        '&& cost.plan && cost.usd.is_none() && cost.premium_requests.is_none()',
        '&& false && cost.usd.is_none() && cost.premium_requests.is_none()',
        "says_on_its_receipt_that_the_plan_covered_it",
    ),
    "prompt-rail-threshold": (
        "src/ui/prompt_rail.rs",
        'messages >= PROMPT_RAIL_MIN && scrollable && !compare && room >= PROMPT_RAIL_ROOM',
        'messages > PROMPT_RAIL_MIN && scrollable && !compare && room >= PROMPT_RAIL_ROOM',
        "gives_rail_to_four_messages_when_scrollable_and_room_suffices",
    ),
    "glyph-weight": (
        "core/src/bot_spec.rs",
        '"o" => GlyphShape { lines: LINES_O, ring: Some(3.9), weight: 3.8, closed: false },',
        '"o" => GlyphShape { lines: LINES_O, ring: Some(3.9), weight: 3.7, closed: false },',
        "is_as_heavy_as_the_eye_it_stands_for",
    ),
    "conversation-step": (
        "src/ui/conversation_list.rs",
        'if Some(index) != at && eligible(&drawn[index])',
        'if Some(index) != at',
        "with_shift_pass_over_every_conversation_that_is_not_waiting_for_you",
    ),
    "conversation-model-mark": (
        "src/ui/sidebar.rs",
        '!is_own_mark_model(&m.model)',
        'true',
        "keeps_the_empty_place_for_a_model_of_your_own_or_a_runtime_not_yet_known",
    ),
    "board-working": (
        "src/team_board.rs",
        "Some(Status::Working) => Section::Working,",
        "Some(Status::Working) => Section::Ready,",
        "an_unseen_finish_outranks_idle_never_a_live_run_or_a_wait",
    ),
    "removed-selection": (
        "src/main.rs",
        'Some(id) => s.teammates.iter().position(|t| t.id == id).unwrap_or(usize::MAX),',
        'Some(id) => s.teammates.iter().position(|t| t.id == id).unwrap_or(0),',
        "is_nobody_once_that_teammate_is_removed_never_whoever_is_first",
    ),
    "tooltip-default": (
        "core/src/teammate_tooltip.rs",
        'if model == "account-default" {',
        'if false {',
        "names_the_model_as_the_chip_does_never_by_its_id",
    ),
    "routine-step-words": (
        "core/src/conversation_turns.rs",
        'return crate::hand_off::step_words_of(&m.prompt);',
        'return &m.prompt;',
        "are_the_step_not_the_answer_quoted_above_it",
    ),
    "prompt-screen": (
        "src/look/faces.rs",
        'if locust_core::bot_spec::outline_of(shape).screen {',
        'if false {',
        "is_never_a_screen_with_terminal_faces_off_but_prompts_whose_face_it_is",
    ),
    "glad-screen": (
        "core/src/bot_rig.rs",
        'let glad = if screen {',
        'let glad = if false {',
        "leaves_a_screens_eyes_to_its_glyphs_carets_say_glad_there",
    ),
    "runtime-gap": (
        "src/ui/reply_actions.rs",
        '.strip_prefix("<truncated ").and_then(|t| t.strip_suffix(" bytes>"))',
        '.strip_prefix("<never-truncated ").and_then(|t| t.strip_suffix(" bytes>"))',
        "draws_the_marker_as_a_note",
    ),
    "screen-eyes": (
        "src/look/bot_stage.rs",
        'if screen && !matches!(activity, FaceActivity::Idle) {',
        'if !matches!(activity, FaceActivity::Idle) {',
        "is_drawn_on_a_screen_only_a_face_with_no_screen_keeps_its_own_eyes",
    ),
    "long-root": (
        "src/ui/sidebar.rs",
        "if !by_id.contains_key(parent) || !seen.insert(parent) {",
        "if seen.len() > 32 || !by_id.contains_key(parent) || !seen.insert(parent) {",
        "finds_the_real_root_36_turns_back",
    ),
    "streaming-file": (
        "src/ui/messages.rs",
        "let files = if finished {",
        "let files = if true {",
        "waits_for_the_message_to_finish_before_drawing_a_button",
    ),
    "tag-context": (
        "core/src/conversation_turns.rs",
        'prompt.split_once("\\n\\n(You were tagged in ").map_or(prompt, |(words, _)| words)',
        'prompt',
        "a_tagged_conversation_is_titled_by_the_message",
    ),
    "anonymous-title": (
        "src/ui/decision_card.rs",
        'None => "A decision is needed to continue".into(),',
        'None => "The runtime needs a decision to continue".into(),',
        "says_a_decision_is_needed_and_no_word_for_the_machinery_when_nobody_asked_by_name",
    ),
    "free-name": (
        "core/src/people.rs",
        'EXAMPLE_NAMES.into_iter().find(|name| !taken.iter().any(|t| t.trim().to_lowercase() == name.to_lowercase())).unwrap_or("Wren")',
        'let _ = taken; "Wren"',
        "skips_every_name_already_on_the_team_whatever_its_case",
    ),
    "fresh-colour": (
        "core/src/people.rs",
        'hues.into_iter().find(|h| !taken.contains(h)).unwrap_or(hues[taken.len() % hues.len()])',
        'hues[taken.len() % hues.len()]',
        "is_the_first_one_nobody_wears_then_round_again",
    ),
}
path, before, after, named = CONTROLS[sys.argv[1]]
path = Path(path)
original = path.read_bytes()
source = original.decode()
assert source.count(before) == 1, "mutation must match exactly once"
log = Path("notes") / f"renderer-control-{sys.argv[1]}.log"
env = dict(os.environ, LOCUST_TEST_TS_READER=str(Path("dist/thread-7-ts-reader/index.js").resolve()))
try:
    path.write_text(source.replace(before, after, 1))
    with log.open("w") as out:
        result = subprocess.run(["timeout", "5400", "python3", "scripts/gentle-cargo.py", "--timeout", "5380", "test", "--release", "--workspace", "--no-fail-fast"], stdout=out, stderr=subprocess.STDOUT, env=env)
    output = log.read_text()
    assert result.returncode != 0, "mutation survived"
    assert "Test isolation guard refused" not in output, "test isolation failure is not a caught mutation"
    assert any(line.startswith("test ") and named in line and line.endswith(" ... FAILED") for line in output.splitlines()), f"named assertion failure absent: {log}"
    print(f"CAUGHT {sys.argv[1]}: {named}", flush=True)
finally:
    path.write_bytes(original)
    assert path.read_bytes() == original
    print(f"RESTORED {path}", flush=True)
