//! The live line of a running turn, with Electron's values and words: `the-live-line-says-which-plan-step.test.ts`
//! (missionView.ts `buildThread`'s `live-step`) and `the-live-word-says-three-dots.test.ts` (`sweepText`).
use crate::thread_view::{live_line, sweep_text};
use serde_json::{Value, json};

fn event(n: usize, kind: &str, payload: Value) -> Value {
    let mut payload = payload;
    payload["evidence"] = json!({"redacted": true});
    json!({"id": format!("e{n}"), "runId": "r", "missionId": "m", "sequence": n, "occurredAt": "2026-09-30T09:00:00.000Z",
        "sourceAdapter": "codex", "type": kind, "payload": payload})
}

fn plan(states: &[&str]) -> Value {
    let steps: Vec<Value> = states.iter().enumerate().map(|(i, s)| json!({"step": format!("Step {}", i + 1), "status": s})).collect();
    event(2, "plan.updated", json!({"plan": {"plan": steps}}))
}

/// The live line's (action, detail) for these events, running.
fn live(events: &[Value]) -> (Option<String>, Option<String>) {
    let line = live_line(events, true, "", false).expect("a running turn has a live line");
    (line.action, line.detail)
}

fn started() -> Value {
    event(1, "run.started", json!({}))
}

#[test]
fn names_the_step_underway_where_the_eye_is() {
    let tool = event(3, "tool.started", json!({"itemId": "t", "toolKind": "command_execution", "name": "shell", "command": "npm test", "phase": "started"}));
    assert_eq!(live(&[started(), plan(&["completed", "in_progress", "pending"]), tool]).1.as_deref(), Some("step 2 of 3"));
}

#[test]
fn leads_with_what_the_open_call_is_doing_in_words() {
    let action = |open: Value| {
        let mut payload = json!({"itemId": "t", "phase": "started"});
        for (k, v) in open.as_object().unwrap() {
            payload[k] = v.clone();
        }
        live(&[started(), plan(&["completed", "in_progress", "pending"]), event(3, "tool.started", payload)]).0
    };
    assert_eq!(action(json!({"toolKind": "command_execution", "name": "shell", "command": "npm test"})).as_deref(), Some("Running npm test"));
    assert_eq!(action(json!({"toolKind": "command_execution", "name": "shell", "command": "cat notes.txt"})).as_deref(), Some("Reading notes.txt"));
    assert_eq!(action(json!({"toolKind": "command_execution", "name": "shell", "command": "rg kiln src"})).as_deref(), Some("Searching for kiln"));
    assert_eq!(action(json!({"toolKind": "tool_use", "name": "Bash", "command": "npm test", "title": "Run the test suite"})).as_deref(), Some("Run the test suite"));
    assert_eq!(action(json!({"toolKind": "write_to_file", "name": "write_to_file", "command": "C:\\work\\summary.txt"})).as_deref(), Some("Editing summary.txt"));
}

#[test]
fn names_the_plans_step_under_way_when_no_call_is_open_to_say_more() {
    assert_eq!(live(&[started(), plan(&["completed", "in_progress", "pending"])]), (Some("Step 2".into()), Some("step 2 of 3".into())));
}

#[test]
fn says_nothing_of_a_plan_with_no_step_underway() {
    assert_eq!(live(&[started(), plan(&["pending", "pending"])]).1, None);
}

#[test]
fn drops_the_steps_own_full_stop_step_1_of_3_follows_it() {
    let steps = event(2, "plan.updated", json!({"plan": {"plan": [
        {"step": "Inspect the screenshot and relevant visual-design notes.", "status": "in_progress"},
        {"step": "Generate a mockup...", "status": "pending"}
    ]}}));
    assert_eq!(live(&[started(), steps]), (Some("Inspect the screenshot and relevant visual-design notes".into()), Some("step 1 of 2".into())));
}

#[test]
fn keeps_an_ellipsis_the_model_wrote() {
    let steps = event(2, "plan.updated", json!({"plan": {"plan": [{"step": "Generate a mockup...", "status": "in_progress"}]}}));
    assert_eq!(live(&[started(), steps]).0.as_deref(), Some("Generate a mockup..."));
}

// the-live-word-says-three-dots.test.ts

#[test]
fn the_live_word_capitalises_and_ends_in_one_ellipsis() {
    assert_eq!(sweep_text("thinking"), "Thinking\u{2026}");
    assert_eq!(sweep_text("using a connector"), "Using a connector\u{2026}");
}

#[test]
fn the_live_word_never_trails_a_full_stop_after_the_ellipsis() {
    for word in ["starting", "working", "writing", "using a tool"] {
        assert!(!sweep_text(word).ends_with("\u{2026}."));
        assert!(sweep_text(word).ends_with('\u{2026}'));
    }
}

#[test]
fn the_live_word_leaves_the_rest_of_the_phrase_alone() {
    assert_eq!(sweep_text("using a tool"), "Using a tool\u{2026}");
}

/// missionView's `clip`: a name at exactly its room is shown whole; one character more is cut with an ellipsis.
#[test]
fn shows_a_file_name_whole_at_exactly_forty_characters_and_cuts_one_longer() {
    let reading = |name: &str| {
        let tool = event(3, "tool.started", json!({"itemId": "t", "toolKind": "command_execution", "name": "shell", "command": format!("cat {name}"), "phase": "started"}));
        live(&[started(), tool]).0.unwrap()
    };
    let forty = "a".repeat(36) + ".txt";
    assert_eq!(reading(&forty), format!("Reading {forty}"));
    let forty_one = "b".repeat(37) + ".txt";
    assert_eq!(reading(&forty_one), format!("Reading {}\u{2026}", &forty_one[..39]));
}

// case: leads with the phase, swept, in the place of the word "starting"
#[test]
fn leads_with_the_phase_swept_in_the_place_of_the_word_starting() {
    let line = crate::thread_view::live_line_starting(&[started()], true, "", false, Some("Reading the folder")).unwrap();
    assert_eq!(sweep_text(&line.label), "Reading the folder\u{2026}");
    assert_eq!(line.register, "starting");
    assert!(line.waiting);
}

// case: names the runtime in the runtime's phase
#[test]
fn names_the_runtime_in_the_runtimes_phase() {
    let line = crate::thread_view::live_line_starting(&[started()], true, "", false, Some("Starting OpenCode")).unwrap();
    assert_eq!(sweep_text(&line.label), "Starting OpenCode\u{2026}");
}

// case: still says "Starting…" when no phase has been sent
#[test]
fn still_says_starting_when_no_phase_has_been_sent() {
    for none in [None, Some("")] {
        let line = crate::thread_view::live_line_starting(&[started()], true, "", false, none).unwrap();
        assert_eq!(sweep_text(&line.label), "Starting\u{2026}");
    }
}

// case: buildThread attaches the plan to live-step only when step N of M is present
#[test]
fn attaches_the_plan_to_the_live_line_only_when_step_n_of_m_is_present() {
    let tool = event(3, "tool.started", json!({"itemId": "t", "toolKind": "command_execution", "name": "shell", "command": "npm test", "phase": "started"}));
    let line = live_line(&[started(), plan(&["completed", "in_progress", "pending"]), tool], true, "", false).unwrap();
    assert_eq!(line.detail.as_deref(), Some("step 2 of 3"));
    let steps = line.plan.expect("a plan is attached");
    assert_eq!(steps.len(), 3);
    assert_eq!(steps.iter().map(|s| s.state).collect::<Vec<_>>(), ["done", "running", "pending"]);
    assert_eq!(steps[0].text, "Step 1");
    // CONTROL: a run without a plan has none attached.
    let bare = event(2, "tool.started", json!({"itemId": "t2", "name": "shell", "phase": "started"}));
    assert_eq!(live_line(&[started(), bare], true, "", false).unwrap().plan, None);
}

fn orb_of(events: &[Value]) -> (String, &'static str, String) {
    let line = live_line(events, true, "", false).unwrap();
    (line.register, line.orb, line.label)
}

fn delta(n: usize, text: &str, op: &str, final_: bool) -> Value {
    event(n, "message.delta", json!({"itemId": "m1", "operation": op, "text": text, "final": final_}))
}

// case: shows the working orb through an OpenCode tool call, because the tool is never open
#[test]
fn shows_the_working_orb_through_an_opencode_tool_call_because_the_tool_is_never_open() {
    let got = orb_of(&[
        event(1, "step.started", json!({"stepKind": "turn"})),
        event(2, "tool.started", json!({"itemId": "t1", "name": "grep", "toolKind": "grep"})),
        event(3, "tool.completed", json!({"itemId": "t1", "status": "completed"})),
    ]);
    assert_eq!((got.0.as_str(), got.1), ("working", "composing"));
}

// case: shows the tool orb on Antigravity, where the start and the result are apart
#[test]
fn shows_the_tool_orb_on_antigravity_where_the_start_and_the_result_are_apart() {
    let got = orb_of(&[event(1, "tool.started", json!({"itemId": "t1", "name": "grep_search", "toolKind": "grep_search"}))]);
    assert_eq!((got.0.as_str(), got.1), ("tool", "searching"));
}

// case: reaches the writing orb on OpenCode, because text arriving IS writing
#[test]
fn reaches_the_writing_orb_on_opencode_because_text_arriving_is_writing() {
    let got = orb_of(&[event(1, "step.started", json!({"stepKind": "turn"})), delta(2, "hel", "append", false)]);
    assert_eq!((got.0.as_str(), got.1), ("writing", "shaping"));
}

// case: says Writing in words too, over a thought the runtime left open (0.586)
#[test]
fn says_writing_in_words_too_over_a_thought_the_runtime_left_open() {
    let thought = || event(1, "step.started", json!({"stepKind": "reasoning", "itemId": "r1"}));
    let got = orb_of(&[thought(), delta(2, "All four", "append", false)]);
    assert_eq!((got.0.as_str(), got.2.as_str()), ("writing", "Writing"));
    let got = orb_of(&[thought(), delta(2, "All four pass.", "append", true)]);
    assert_eq!((got.0.as_str(), got.2.as_str()), ("working", "Working"));
    let named = || vec![event(1, "step.started", json!({"stepKind": "turn", "message": "Running the tests"})), delta(2, "All four", "append", false)];
    let got = orb_of(&named());
    assert_eq!((got.0.as_str(), got.2.as_str()), ("writing", "Writing"));
    let mut done = named();
    done.push(delta(3, " pass.", "append", true));
    let got = orb_of(&done);
    assert_eq!((got.0.as_str(), got.2.as_str()), ("working", "Running the tests"));
}

// case: goes back to working when the text is complete
#[test]
fn goes_back_to_working_when_the_text_is_complete() {
    let got = orb_of(&[event(1, "step.started", json!({"stepKind": "turn"})), delta(2, "hel", "append", false), delta(3, "hello", "replace", true)]);
    assert_eq!((got.0.as_str(), got.1), ("working", "composing"));
}

// case: never overrides a step that said something more specific
#[test]
fn never_overrides_a_step_that_said_something_more_specific() {
    let got = orb_of(&[event(1, "tool.started", json!({"itemId": "t1", "name": "grep_search", "toolKind": "grep_search"})), delta(2, "hi", "append", false)]);
    assert_eq!(got.0, "tool");
}

// case: cannot hold the thinking orb when a reasoning step opens and closes at once
#[test]
fn cannot_hold_the_thinking_orb_when_a_reasoning_step_opens_and_closes_at_once() {
    let got = orb_of(&[event(1, "step.started", json!({"stepKind": "reasoning", "itemId": "r1"})), event(2, "step.completed", json!({"stepKind": "reasoning", "itemId": "r1"}))]);
    assert_ne!(got.1, "listening");
}

// case: times from the turn, not from the phase, before anything has happened
// case: does not restart when a step opens three seconds in
// case: does not restart when a tool opens either
#[test]
fn the_elapsed_counter_times_from_the_turn_and_never_restarts_with_the_phase() {
    const TURN: &str = "2026-09-15T10:00:00.000Z";
    let at = |kind: &str, when: &str, payload: Value| json!({"type": kind, "occurredAt": when, "payload": payload});
    assert_eq!(live_line(&[], true, TURN, false).unwrap().started_at, TURN);
    let step = [at("run.started", "2026-09-15T10:00:03.000Z", json!({}))];
    assert_eq!(live_line(&step, true, TURN, false).unwrap().started_at, TURN);
    let tool = [at("run.started", "2026-09-15T10:00:01.000Z", json!({})), at("tool.started", "2026-09-15T10:00:09.000Z", json!({"itemId": "t1", "name": "read", "command": "read"}))];
    assert_eq!(live_line(&tool, true, TURN, false).unwrap().started_at, TURN);
}
