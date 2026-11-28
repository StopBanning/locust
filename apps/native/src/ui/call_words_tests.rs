use super::*;
use rune::{Env, Runtime};
use serde_json::json;

fn saved_call() -> serde_json::Value {
    json!({
        "callId": "call_1", "teammateId":"tm_ada", "name":"Ada", "startedAt":1000, "endedAt":104000,
        "afterMissionId":"m_typed",
        "lines":[
            {"role":"you","text":"Ada, check the readme.","at":10000},
            {"role":"voice","text":"Let me check.","at":12000},
            {"role":"you","text":"So what did you find?","at":60000},
            {"role":"voice","text":"It is cardamom.","at":62000}
        ],
        "turns":[{"missionId":"m_asked","at":20000}]
    })
}

fn state(origin: &[bool]) -> State {
    let ids = ["m_typed", "m_asked", "m_after"];
    let turn_metas: Vec<_> = origin.iter().enumerate().map(|(at, voice)| {
        ledger::metadata(&json!({
            "recordType":"mission.created",
            "metadata":{
                "missionId":ids[at], "prompt":format!("Turn {}.", ids[at]),
                "startedBy": if *voice { json!({"kind":"mcp","via":"voice"}) } else { serde_json::Value::Null }
            }
        })).unwrap()
    }).collect();
    let items = turn_metas
        .iter()
        .flat_map(|meta| {
            [
                ledger::Item::User(meta.prompt.clone()),
                ledger::Item::Assistant {
                    item_id: meta.mission_id.clone(),
                    text: "The answer.".into(),
                    r#final: true,
                },
            ]
        })
        .collect();
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        header: Teammate {
            id: "tm_ada".into(),
            name: "Ada".into(),
            role: "Engineer".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        },
        conversation: Conversation {
            conversation_id: Some("m_typed".into()),
            turns: Vec::new(),
        },
        items,
        dir: Default::default(),
        worker: None,
        snapshot: ledger::Snapshot {
            turn_metas,
            ..Default::default()
        },
        cold_tip: None,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    crate::messages::message_column(
        &s.ledger.as_ref().unwrap().items,
        "Ada",
        &Palette::new(true),
        s,
    )
}

fn thread(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, view, (960.0, 1800.0), Env::new(true, 1.0));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..4 {
        rt.frame(0.05);
    }
}

fn words(rt: &Runtime<State>) -> String {
    let mut items = rt.elements();
    items.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    items
        .into_iter()
        .filter_map(|item| item.text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

// case: ended: folds to one line that says how long and how much, and opens to every word
#[test]
fn saved_call_folds_and_places_its_words_around_the_turn() {
    let mut s = state(&[false, true]);
    s.calls.saved = vec![saved_call()];
    let mut rt = thread(s);
    let drawn = words(&rt);
    assert!(drawn.contains("Call with Ada · 1:43"));
    assert!(drawn.contains("4 lines said · asked Ada 1 time"));
    assert!(drawn.contains("Show what was said"));
    assert!(!drawn.contains("Let me check."));
    click(&mut rt, "call-words-call_1");
    let drawn = words(&rt);
    assert!(drawn.contains("Hide"));
    assert!(drawn.contains("CALL ENDED · 1:43"));
    let before = drawn.find("Let me check.").unwrap();
    let turn = drawn.find("Turn m_asked.").unwrap();
    let after = drawn.find("It is cardamom.").unwrap();
    assert!(before < turn && turn < after, "{drawn}");
}

// case: is not said of a conversation the person typed, that a call joined later
#[test]
fn the_newest_turn_does_not_replace_the_original_origin() {
    for origin in [&[false, true][..], &[false, true, true][..]] {
        assert!(!words(&thread(state(origin))).contains("STARTED FROM A VOICE CALL"));
    }
}

// case: is said of one a call began, however many turns follow, and of a call that is its only turn
#[test]
fn the_first_turn_marks_a_voice_conversation() {
    for origin in [&[true][..], &[true, false][..], &[true, false, true][..]] {
        assert!(words(&thread(state(origin))).contains("STARTED FROM A VOICE CALL"));
    }
}

// case: is never said of a conversation with no call in it
#[test]
fn a_conversation_without_voice_has_no_voice_marker() {
    for origin in [&[false][..], &[false, false][..]] {
        assert!(!words(&thread(state(origin))).contains("STARTED FROM"));
    }
}

fn todo(id: &str, name: &str, task: &str) -> serde_json::Value {
    json!({"missionId":id, "teammateId":format!("tm_{name}"), "name":name, "task":task, "at":20000})
}

fn with_todos(handed: &[serde_json::Value]) -> State {
    let mut s = state(&[false, true]);
    let mut call = saved_call();
    call["handed"] = json!(handed);
    s.calls.saved = vec![call];
    s
}

// case: each say who has it, what was asked and how it is going, with Open for one not on screen
#[test]
fn open_appears_only_for_the_other_conversation() {
    let mut s = with_todos(&[
        todo("m_asked", "Ada", "Fix login"),
        todo("m_note", "Wren", "Draft a note"),
    ]);
    s.ledger_summaries
        .insert("m_asked".into(), ledger::Summary::default());
    s.home.waiting_for.insert("m_note".into(), "Wren".into());
    let rt = thread(s);
    let shown = words(&rt);
    for word in [
        "Ada",
        "Fix login",
        "Done",
        "Wren",
        "Draft a note",
        "Needs you",
    ] {
        assert!(shown.contains(word), "{word}: {shown}");
    }
    assert!(rt.find("call-todo-open-m_note").is_some());
    assert!(rt.find("call-todo-open-m_asked").is_none());
}

// case: read as starting until the window has seen them run
#[test]
fn an_unseen_handoff_starts_out_starting() {
    let rt = thread(with_todos(&[todo("m_note", "Wren", "Draft a note")]));
    assert!(words(&rt).contains("Starting"));
}

// case: is under the call once it has ended, folded or open
#[test]
fn the_summary_remains_visible_when_the_words_are_folded() {
    let s = with_todos(&[
        todo("m_asked", "Ada", "Fix login"),
        todo("m_note", "Wren", "Draft a note"),
    ]);
    let mut rt = thread(s);
    assert!(words(&rt).contains("From the call · 2 to-dos"));
    click(&mut rt, "call-words-call_1");
    let drawn = words(&rt);
    assert!(drawn.find("CALL ENDED").unwrap() < drawn.find("From the call").unwrap());
}

// case: sums up: one to-do in another conversation is said, one whose turn is right above is not repeated
#[test]
fn a_single_handoff_already_in_this_thread_needs_no_summary() {
    let there = thread(with_todos(&[todo("m_note", "Wren", "Draft a note")]));
    assert!(words(&there).contains("From the call · 1 to-do"));
    let here = thread(with_todos(&[todo("m_asked", "Ada", "Fix login")]));
    assert!(!words(&here).contains("From the call"));
}

// case: is not there while the call is on, at a piece that is not its last, or for a call kept before it
#[test]
fn b09_call_words_summary_only_follows_a_kept_last_piece_with_todos() {
    let s = with_todos(&[todo("m_note", "Wren", "Draft the release note")]);
    let call = &s.calls.saved[0];
    for (live, last, has_handoff) in [(true, true, true), (false, false, true), (false, true, false)] {
        let mut fixture = call.clone();
        if live { fixture["partial"] = json!({"you":"","voice":""}); }
        if !has_handoff { fixture.as_object_mut().unwrap().remove("handed"); }
        fixture["fixtureLast"] = json!(last);
        let mut s = state(&[false, true]);
        s.calls.saved = vec![fixture];
        let mut rt = Runtime::new(s, |s, _| {
            let call = &s.calls.saved[0];
            let piece = Piece { call, lines: Vec::new(), first: true, last: call["fixtureLast"] == true };
            col().children(draw_with_live(s, &Palette::new(true), &Place::End, &[piece], call.get("partial").map(|_| call)))
        }, (960., 1200.), Env::new(true, 1.));
        settle(&mut rt);
        assert!(!words(&rt).contains("From the call"), "live={live} last={last} has_handoff={has_handoff}");
    }
}

// case: follows its run while the window holds it, else the record; waiting on you outranks both
#[test]
fn b09_call_words_live_phase_and_waiting_override_the_record() {
    for (live, record, waiting, expected) in [
        (None, None, false, "Starting"),
        (Some("starting"), None, false, "Starting"),
        (Some("running"), None, false, "Working"),
        (Some("cancelling"), None, false, "Working"),
        (Some("running"), None, true, "Needs you"),
        (Some("completed"), None, true, "Needs you"),
        (Some("completed"), None, false, "Done"),
        (None, Some("completed"), false, "Done"),
        (Some("running"), Some("completed"), false, "Working"),
        (None, Some("failed"), false, "Failed"),
        (None, Some("cancelled"), false, "Stopped"),
        (Some("interrupted"), None, false, "Stopped"),
    ] {
        assert_eq!(todo_status(waiting, live, record), expected);
    }
    let mut s = with_todos(&[todo("m_note", "Wren", "Draft the release note")]);
    s.recent.push(ConversationRow { id: "m_note".into(), title: "Note".into(), teammate: "Wren".into(),
        started_by: "voice".into(), state: "interrupted".into(), updated: String::new() });
    let mut rt = thread(s);
    assert!(words(&rt).contains("Stopped"));
    rt.update(|s,_| { s.home.waiting_for.insert("m_note".into(), "Wren".into()); });
    settle(&mut rt);
    assert!(words(&rt).contains("Needs you"));
    assert!(!words(&rt).contains("Stopped"));
}
