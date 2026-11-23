//! `a-refusal-offers-the-mode-that-works.test.ts`, drawn: a run its mode refused is not offered Run it again (Sol's
//! beta finding 3: pressing it would refuse again, identically, for ever, beside a sentence admitting it repeats
//! nothing); it is offered the mode that works, Run again with edits allowed. A run that failed for anything else
//! still is offered Run it again. The reading's own cases are in core/src/mode_refusal_tests.rs.
use super::*;
use serde_json::{json, Value};

/// A failed OpenCode run that never started, read by the ledger's own reader, drawn under the person's words.
fn drawn(message: &str, tweak: impl FnOnce(&mut State)) -> rune::Runtime<State> {
    let failed: Value = json!({ "type": "run.failed", "payload": { "message": message } });
    let record = json!({ "event": { "id": "evt_1", "runId": "run_1", "missionId": "mission_1", "sequence": 1,
        "occurredAt": "2026-09-21T12:00:00.000Z", "sourceAdapter": "opencode", "type": "run.failed", "payload": failed["payload"] } });
    let mut transcript = ledger::Transcript::default();
    transcript.apply(vec![record]);
    let items: Vec<ledger::Item> = std::iter::once(ledger::Item::User("Create notes.txt".into())).chain(transcript.items).collect();
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_codec".into(), name: "Codec".into(), role: "Engineer".into(), runtime: Some("opencode".into()), model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("mission_1".into()), turns: Vec::new() },
        items, dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            failed: transcript.failed,
            never_started: locust_core::thread_view::runtime_never_started([&failed]),
            tip: "mission_1".into(),
            turn_metas: vec![ledger::Metadata { mission_id: "mission_1".into(), prompt: "Create notes.txt".into(), ..Default::default() }],
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    tweak(&mut s);
    let mut rt = rune::Runtime::new(
        s,
        |s, _| message_column(&s.ledger.as_ref().unwrap().items, "Codec", &Palette::new(true), s),
        (800.0, 1200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.05);
    rt
}

fn texts(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

const REFUSED: &str = "The mode this run is in does not allow write, so OpenCode stopped when it tried to use it. Nothing was changed.";

// case: reads a mode refusal as a refusal
#[test]
fn a_run_its_mode_refused_is_offered_the_mode_that_works_not_run_it_again() {
    let rt = drawn(REFUSED, |_| {});
    let t = texts(&rt);
    assert!(!t.contains("Run it again"), "{t}");
    assert!(!t.contains("cannot repeat anything"), "{t}");
    assert!(t.contains(locust_core::thread_view::MODE_REFUSED_SENTENCE), "{t}");
    assert!(t.contains("Run again with edits allowed"), "{t}");
    // The press: the mode switched on the message box, and the same words sent in it; what is in the box stays.
    let mut rt = drawn(REFUSED, |s| s.draft.replace_text("half a thought"));
    let (x, y) = rt.find("run-with-edits-button").expect("no Run again with edits allowed");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..40 {
        rt.frame(0.05);
    }
    assert_eq!(rt.state.composer_mode.as_deref(), Some("accept-edits"));
    let flight = rt.state.flight.as_ref().expect("Run again with edits allowed sent nothing");
    assert_eq!((flight.teammate_id.as_str(), flight.message.as_deref()), ("tm_codec", Some("Create notes.txt")));
    assert_eq!(rt.state.draft.text, "half a thought");
}

// case: reads everything else as something that actually went wrong
#[test]
fn a_run_that_failed_for_anything_else_is_still_offered_run_it_again() {
    for said in ["OpenCode exited with code 1.", "OpenCode asked for C:/other, which is outside the folder this run may use, and stopped."] {
        let t = texts(&drawn(said, |_| {}));
        assert!(t.contains("Run it again"), "{said}: {t}");
        assert!(!t.contains("Run again with edits allowed"), "{said}: {t}");
    }
}
