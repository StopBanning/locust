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
    assert any(line.startswith("test ") and named in line and line.endswith(" ... FAILED") for line in output.splitlines()), f"named assertion failure absent: {log}"
    print(f"CAUGHT {sys.argv[1]}: {named}", flush=True)
finally:
    path.write_bytes(original)
    assert path.read_bytes() == original
    print(f"RESTORED {path}", flush=True)
