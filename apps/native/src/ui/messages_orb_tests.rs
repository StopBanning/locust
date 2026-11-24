use super::*;
use locust_core::thread_view::live_line_in;
use serde_json::{Value, json};

const AT: &str = "2026-09-20T10:00:00.000Z";

fn started(id: &str, name: &str, kind: &str, command: Option<&str>) -> Value {
    json!({"type": "tool.started", "occurredAt": AT, "payload": {
        "itemId": id, "name": name, "toolKind": kind, "command": command
    }})
}

fn completed(id: &str) -> Value {
    json!({"type": "tool.completed", "occurredAt": AT, "payload": {"itemId": id, "status": "completed"}})
}

fn thread(events: &[Value], plan_mode: bool) -> rune::Runtime<State> {
    let mut s = tests_live_state();
    let pane = s.ledger.as_mut().unwrap();
    pane.items = vec![ledger::Item::User("Run the tests".into())];
    pane.snapshot.live = live_line_in(events, true, AT, false, None, plan_mode);
    let mut rt = rune::Runtime::new(
        s,
        |s, cx| {
            s.reduced_motion.set(cx.is_reduced_motion());
            message_column(
                &s.ledger.as_ref().unwrap().items,
                "Codec",
                &Palette::new(true),
                s,
            )
        },
        (800.0, 800.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

fn orb(elements: &[rune::ElementInfo]) -> Option<&rune::ElementInfo> {
    let mut orbs = elements
        .iter()
        .filter(|e| e.trace_name == Some("thinking-orb"));
    let first = orbs.next();
    assert!(orbs.next().is_none(), "the live thread draws only one orb");
    first
}

fn orb_state(elements: &[rune::ElementInfo]) -> Option<&'static str> {
    orb(elements)
        .and_then(|e| e.trace_namespace)
        .map(|(_, state)| state)
}

fn label(elements: &[rune::ElementInfo]) -> &str {
    elements
        .iter()
        .find(|e| e.key.as_deref() == Some("live-step-label"))
        .and_then(|e| e.text.as_deref())
        .expect("the live label")
}

// case: a shell command that is still open draws the working orb
#[test]
fn a_shell_command_that_is_still_open_draws_the_working_orb() {
    let rt = thread(
        &[started(
            "t1",
            "shell",
            "command_execution",
            Some("pnpm test billing"),
        )],
        false,
    );
    assert_eq!(
        rt.state
            .ledger
            .as_ref()
            .unwrap()
            .snapshot
            .live
            .as_ref()
            .unwrap()
            .register,
        "tool"
    );
    let elements = rt.elements();
    assert_eq!(orb_state(&elements), Some("connecting"));
    assert_eq!(label(&elements), "Running pnpm test billing");
    let image = orb(&elements).unwrap();
    let words = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("live-step-label"))
        .unwrap();
    assert_eq!(&image.rect[2..], &[26.0, 26.0]);
    assert_eq!(image.parent, words.parent);
    assert!(image.rect[0] + image.rect[2] <= words.rect[0]);
}

// case: a read that is still open draws the searching orb
#[test]
fn a_read_that_is_still_open_draws_the_searching_orb() {
    let rt = thread(&[started("t1", "read_file", "", None)], false);
    assert_eq!(orb_state(&rt.elements()), Some("searching"));
}

// case: a subagent that is still out draws the weaving orb
#[test]
fn a_subagent_that_is_still_out_draws_the_weaving_orb() {
    let rt = thread(&[started("t1", "Task", "", None)], false);
    assert_eq!(orb_state(&rt.elements()), Some("weaving"));
}

// case: stops claiming the work the moment the tool closes
#[test]
fn stops_claiming_the_work_the_moment_the_tool_closes() {
    for (start, open_orb) in [
        (started("t1", "read_file", "", None), "searching"),
        (
            started("t1", "shell", "command_execution", Some("ls")),
            "connecting",
        ),
    ] {
        let mut rt = thread(std::slice::from_ref(&start), false);
        assert_eq!(orb_state(&rt.elements()), Some(open_orb));
        rt.update(|s, tx| {
            tx.instant();
            s.ledger.as_mut().unwrap().snapshot.live =
                live_line_in(&[start, completed("t1")], true, AT, false, None, false);
        });
        rt.frame(0.016);
        let elements = rt.elements();
        assert_eq!(orb_state(&elements), Some("composing"));
        assert_eq!(label(&elements), "Working…");
    }
}

// case: a Plan-mode turn with no tool open draws the solving orb
#[test]
fn a_plan_mode_turn_with_no_tool_open_draws_the_solving_orb() {
    let rt = thread(&[], true);
    assert_eq!(orb_state(&rt.elements()), Some("solving"));
}

// case: an ordinary turn with no tool open still draws the floor
#[test]
fn an_ordinary_turn_with_no_tool_open_still_draws_the_floor() {
    let rt = thread(&[], false);
    let elements = rt.elements();
    assert_eq!(orb_state(&elements), Some("breathing"));
    let image = orb(&elements).unwrap();
    let slot = &elements[image.parent.unwrap()];
    assert_eq!(slot.key.as_deref(), Some("live-step-orb"));
    assert_eq!(&slot.rect[2..], &[26.0, 26.0]);
    assert_eq!(&image.rect[2..], &[20.0, 20.0]);
    assert_eq!(image.rect[0] - slot.rect[0], 3.0);
}

// case: a runtime that reports its tools only on completion still shows it is alive
#[test]
fn a_runtime_that_reports_its_tools_only_on_completion_still_shows_it_is_alive() {
    let events = [
        started("t1", "shell", "command_execution", Some("echo orb")),
        completed("t1"),
    ]
    .map(|mut event| {
        event["sourceAdapter"] = json!("opencode");
        event
    });
    let rt = thread(&events, false);
    assert_eq!(orb_state(&rt.elements()), Some("composing"));
}

#[test]
fn when_the_live_step_closes_the_orb_disappears_and_the_thread_stops_waking() {
    let mut rt = thread(&[started("t1", "read_file", "", None)], false);
    assert_eq!(orb_state(&rt.elements()), Some("searching"));
    assert!(live_sweeping(&rt.state));
    rt.update(|s, tx| {
        tx.instant();
        // A completed run can leave a cached projection until its ledger refresh.
        s.ledger.as_mut().unwrap().snapshot.running = false;
    });
    rt.frame(0.016);
    assert!(orb(&rt.elements()).is_none());
    assert!(!live_sweeping(&rt.state));
    assert!(!rt.frame(0.2));
    assert_eq!(rt.wake_deadline(), None);
}

mod mapping {
    use super::*;

    fn drawn(events: &[Value], plan_mode: bool) -> Vec<rune::ElementInfo> {
        thread(events, plan_mode).elements()
    }

    fn assert_line(elements: &[rune::ElementInfo], words: &str, shape: &str) {
        assert_eq!(label(elements), words);
        assert_eq!(orb_state(elements), Some(shape), "beside {words:?}");
    }

    fn step(kind: &str, item: &str, words: &str) -> Value {
        json!({"type": "step.started", "occurredAt": AT, "payload": {
            "stepKind": kind, "itemType": item, "message": words
        }})
    }

    fn writing() -> Value {
        json!({"type": "message.delta", "occurredAt": AT, "payload": {"itemId": "reply", "final": false}})
    }

    // case: a shell command is working
    #[test]
    fn a_shell_command_is_working() {
        let events = [started(
            "t1",
            "shell",
            "command_execution",
            Some("pnpm test billing"),
        )];
        assert_line(
            &drawn(&events, false),
            "Running pnpm test billing",
            "connecting",
        );
    }

    // case: a subagent is weaving, and a connector is connecting
    #[test]
    fn a_subagent_and_a_connector_are_told_apart() {
        let helper = [started("t1", "Task", "tool_use", None)];
        let connector = [started("t1", "gmail__send", "tool_use", None)];
        assert_line(&drawn(&helper, false), "Using a tool · Task", "weaving");
        assert_line(
            &drawn(&connector, false),
            "Using a connector · send",
            "solving",
        );
    }

    // case: reading a file is searching
    #[test]
    fn reading_a_file_is_searching() {
        for name in ["read_file", "view_file", "Grep", "listDirectory", "search"] {
            let events = [started("t1", name, "tool_use", None)];
            assert_line(
                &drawn(&events, false),
                &format!("Using a tool · {name}"),
                "searching",
            );
        }
    }

    // case: a Plan-mode turn with nothing open is solving
    #[test]
    fn a_plan_mode_turn_with_nothing_open_is_solving() {
        assert_line(&drawn(&[], true), "Starting…", "solving");
    }

    // case: writing a file is working -- a tool IS running, and that is all it says
    #[test]
    fn a_file_writing_tool_gets_the_generic_tool_orb() {
        let events = [started("t1", "write_file", "write_to_file", None)];
        assert_line(&drawn(&events, false), "Writing a file", "connecting");
    }

    // case: starting is breathing: alive, nothing back yet
    #[test]
    fn starting_is_breathing_before_the_runtime_speaks() {
        let starting = [json!({"type": "run.started", "occurredAt": AT, "payload": {}})];
        assert_line(&drawn(&starting, false), "Starting…", "breathing");
        assert_line(&drawn(&[], false), "Starting…", "breathing");
    }

    // case: EVERY word change changes the orb
    #[test]
    fn every_word_change_changes_the_orb_in_the_same_thread() {
        let mut rt = thread(&[], false);
        let mut events = Vec::new();
        assert_line(&rt.elements(), "Starting…", "breathing");
        for (event, words, shape) in [
            (
                step("reasoning", "reasoning", "Thinking"),
                "Thinking…",
                "listening",
            ),
            (step("turn", "turn", "Working"), "Working…", "composing"),
            (writing(), "Writing…", "shaping"),
            (
                started("t1", "gmail__send", "tool_use", None),
                "Using a connector · send",
                "solving",
            ),
        ] {
            events.push(event);
            rt.update(|s, tx| {
                tx.instant();
                s.ledger.as_mut().unwrap().snapshot.live =
                    live_line_in(&events, true, AT, false, None, false);
            });
            rt.frame(0.016);
            assert_line(&rt.elements(), words, shape);
        }
    }

    // case: waiting on a model to speak is listening, and a model writing is composing
    #[test]
    fn thinking_and_writing_draw_their_own_shapes() {
        let thinking = [step("reasoning", "reasoning", "Thinking")];
        assert_line(&drawn(&thinking, false), "Thinking…", "listening");
        assert_line(&drawn(&[writing()], false), "Writing…", "shaping");
    }

    // case: a step that says `tool` never shows the waiting ring
    #[test]
    fn a_tool_step_without_details_still_draws_connecting() {
        let events = [step("tool", "commandExecution", "Running a tool")];
        assert_line(
            &drawn(&events, false),
            "Using a tool · Running a tool",
            "connecting",
        );
    }

    // case: a connector call is connecting, because it left the machine
    #[test]
    fn a_connector_draws_solving_beside_its_tool_and_server() {
        let events = [started("t1", "gmail__send", "tool_use", None)];
        let elements = drawn(&events, false);
        assert_line(&elements, "Using a connector · send", "solving");
        let server = elements
            .iter()
            .find(|e| e.key.as_deref() == Some("live-step-peek"))
            .unwrap();
        assert_eq!(server.text.as_deref(), Some(" · gmail"));
    }

    // case: an unclassified open tool is the generic tool orb, never a specific one
    #[test]
    fn an_unclassified_open_tool_draws_connecting() {
        let events = [started("t1", "todowrite", "tool_use", None)];
        assert_line(
            &drawn(&events, false),
            "Using a tool · todowrite",
            "connecting",
        );
    }

    // case: never claims a SPECIFIC kind of work that is not happening
    #[test]
    fn generic_work_does_not_claim_reading_or_a_connector() {
        for event in [
            started("t1", "apply_patch", "edit", None),
            started("t1", "unknown_thing", "tool_use", None),
            started("t1", "shell", "command_execution", Some("ls")),
        ] {
            assert_eq!(orb_state(&drawn(&[event], false)), Some("connecting"));
        }
        assert_eq!(orb_state(&drawn(&[], false)), Some("breathing"));
    }

    // case: always returns something, so a live row is never bare
    #[test]
    fn a_live_row_always_draws_an_orb() {
        for (event, shape) in [
            (
                started("t1", "shell", "command_execution", Some("ls")),
                "connecting",
            ),
            (started("t1", "task", "tool_use", None), "weaving"),
            (started("t1", "read", "tool_use", None), "searching"),
            (started("t1", "apply_patch", "edit", None), "connecting"),
            (
                started("t1", "unknown_thing", "tool_use", None),
                "connecting",
            ),
        ] {
            assert_eq!(orb_state(&drawn(&[event], false)), Some(shape));
        }
        assert_eq!(orb_state(&drawn(&[], true)), Some("solving"));
        assert_eq!(orb_state(&drawn(&[], false)), Some("breathing"));
    }

    // case: plan mode never overrides a tool that is actually running
    #[test]
    fn a_running_tool_takes_precedence_over_plan_mode() {
        for (event, words, shape) in [
            (
                started("t1", "shell", "command_execution", Some("git status")),
                "Running git status",
                "connecting",
            ),
            (
                started("t1", "read_file", "tool_use", None),
                "Using a tool · read_file",
                "searching",
            ),
            (
                started("t1", "Task", "tool_use", None),
                "Using a tool · Task",
                "weaving",
            ),
        ] {
            assert_line(&drawn(&[event], true), words, shape);
        }
    }
}
