use super::*;
use locust_core::approval::{Answer, Channel};
use rune::{Env, Runtime};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

pub(super) fn pane(mission: &str, name: &str) -> LedgerPane {
    LedgerPane {
        header: Teammate {
            id: format!("tm_{name}"), name: name.into(), role: "Writer".into(), runtime: Some("claude".into()),
            model: None, account: None, busy: false, runs_in: "local".into(), folder: Some("/work/app".into()),
        },
        conversation: Conversation { conversation_id: Some(mission.into()), ..Default::default() },
        items: vec![ledger::Item::User(format!("{name}'s request"))],
        dir: std::env::temp_dir(),
        worker: None,
        snapshot: ledger::Snapshot { tip: mission.into(), ..Default::default() },
        cold_tip: None,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    }
}

pub(super) fn settle(rt: &mut Runtime<State>) {
    for _ in 0..14 {
        rt.frame(0.05);
    }
}

fn ask(ch: Arc<Channel>, mission: String, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<Result<Value, String>> {
    std::thread::spawn(move || ch.ask_claude(
        &mission, &mission, "permission", "mcp__claude_ai_Gmail__send_message", &json!({}), "/work/app",
        &|| stop.load(Ordering::SeqCst),
    ))
}

fn wait_for_card(ch: &Channel, mission: &str) -> approval::Request {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(card) = ch.waiting_for_mission(mission).pop() { return card }
        assert!(Instant::now() < until, "card for {mission} never arrived");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is missing"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn drawn(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| !e.exiting && e.key.as_deref() == Some(key))
}

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) { self.0.store(true, Ordering::SeqCst); }
}

// case: carries the person’s reason for a denial, when they gave one (0.374)
#[test]
fn approval_routes_a_denial_to_the_active_thread_and_only_its_request() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let ch = native::approvals_channel();
    let stop = Stop(Arc::new(AtomicBool::new(false)));
    let suffix = locust_core::invariants::unique_nanos();
    let first = format!("approval_wren_{suffix}");
    let second = format!("approval_fern_{suffix}");
    let wren = ask(ch.clone(), first.clone(), stop.0.clone());
    let fern = ask(ch.clone(), second.clone(), stop.0.clone());
    let wren_card = wait_for_card(&ch, &first);
    let fern_card = wait_for_card(&ch, &second);
    let wren_key = format!("approval-{}", wren_card.approval_id);
    let fern_key = format!("approval-{}", fern_card.approval_id);
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.ledger = Some(pane(&first, "Wren"));
    sync_approvals(&mut s);
    let mut rt = Runtime::new(s, crate::view, (1440., 1500.), Env::new(true, 1.));
    settle(&mut rt);
    assert!(drawn(&rt, &wren_key) && !drawn(&rt, &fern_key), "only Wren's request is visible on Wren's thread");

    press(&mut rt, "deny");
    press(&mut rt, "approval-reason");
    rt.paste("wrong thread");
    settle(&mut rt);
    rt.update(|s, _| s.ledger = Some(pane(&second, "Fern")));
    settle(&mut rt);
    assert!(!drawn(&rt, &wren_key), "Wren's card must not appear on Fern before the next poll");
    assert!(!drawn(&rt, "approval-reason"), "Wren's denial draft must not appear on Fern");
    rt.update(|s, _| decide_pending(s, approval::Decision::Deny));
    assert_eq!(ch.waiting_for_mission(&first).len(), 1, "a late click must not deny the other thread's card");

    rt.update(|s, _| { sync_approvals(s); });
    settle(&mut rt);
    assert!(!drawn(&rt, &wren_key) && drawn(&rt, &fern_key), "the real poll selects Fern's card");
    press(&mut rt, "deny");
    press(&mut rt, "approval-reason");
    rt.paste("draft it instead of sending");
    settle(&mut rt);
    let [x, y, w, h] = rt.elements().into_iter().find(|e| !e.exiting && e.text.as_deref() == Some("Deny and say why"))
        .expect("confirmation when a reason is entered").rect;
    rt.pointer_down(x + w / 2., y + h / 2.);
    rt.pointer_up(x + w / 2., y + h / 2.);
    settle(&mut rt);
    let until = Instant::now() + Duration::from_secs(5);
    while !fern.is_finished() && Instant::now() < until { std::thread::sleep(Duration::from_millis(5)); }
    assert!(fern.is_finished(), "Deny and say why must answer Fern's original request");
    assert_eq!(fern.join().unwrap().unwrap(), json!({
        "behavior": "deny", "message": "Denied in Locust. The person declined this, and said: draft it instead of sending"
    }));
    assert!(!drawn(&rt, &fern_key));
    assert!(ch.waiting_for_mission(&second).is_empty());
    assert_eq!(ch.waiting_for_mission(&first).len(), 1);
    let recorded = ch.take_recorded_for(&second);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].approval_id, fern_card.approval_id);
    assert_eq!(recorded[0].answer, "denied");
    assert_eq!(recorded[0].words.as_deref(), Some("draft it instead of sending"));
    assert!(ch.take_recorded_for(&first).is_empty(), "Fern's denial must never be recorded for Wren");

    ch.decide(Answer::Decision { approval_id: wren_card.approval_id, decision: approval::Decision::Deny, reason: None }).unwrap();
    wren.join().unwrap().unwrap();
    ch.take_recorded_for(&first);
}

// case: sends answers keyed by question id, not a decision
#[test]
fn approval_a_question_sends_its_answers_only_from_its_own_thread() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let ch = native::approvals_channel();
    let stop = Stop(Arc::new(AtomicBool::new(false)));
    let mission = format!("approval_question_{}", locust_core::invariants::unique_nanos());
    let handle = {
        let (ch, mission, stop) = (ch.clone(), mission.clone(), stop.0.clone());
        std::thread::spawn(move || ch.ask_codex(&mission, &mission, "item/tool/requestUserInput", &json!({
            "questions": [{ "id": "database", "question": "Which database?", "options": [
                {"label": "SQLite", "description": "Use a file."}, {"label": "Postgres", "description": "Use a server."}
            ] }, { "id": "notes", "question": "Anything to keep?", "options": [] }]
        }), json!(4), &|| stop.load(Ordering::SeqCst)))
    };
    let card = wait_for_card(&ch, &mission);
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.ledger = Some(pane(&mission, "Wren"));
    sync_approvals(&mut s);
    let mut rt = Runtime::new(s, crate::view, (1440., 1500.), Env::new(true, 1.));
    rt.on_key = Some(crate::on_key);
    settle(&mut rt);
    assert!(drawn(&rt, "questions") && !drawn(&rt, "approve-once"));
    rt.update(|s, _| s.ledger = Some(pane("another_question_thread", "Fern")));
    settle(&mut rt);
    rt.key(KeyEvent { key: Key::Text("1".into()), mods: Default::default() });
    settle(&mut rt);
    assert!(rt.state.question_answers.chosen.is_empty(), "a number key on Fern must not choose an answer for Wren");

    rt.update(|s, _| { sync_approvals(s); });
    rt.update(|s, _| { s.ledger = Some(pane(&mission, "Wren")); sync_approvals(s); });
    settle(&mut rt);
    press(&mut rt, "question-database-option-0");
    press(&mut rt, "question-notes-free");
    rt.paste("Keep the existing migration names.");
    settle(&mut rt);
    press(&mut rt, "send-answers");
    let until = Instant::now() + Duration::from_secs(5);
    while !handle.is_finished() && Instant::now() < until { std::thread::sleep(Duration::from_millis(5)); }
    assert!(handle.is_finished(), "Send answer must reach the question");
    assert_eq!(handle.join().unwrap().unwrap(), json!({"answers": {"database": {"answers": ["SQLite"]}, "notes": {"answers": ["Keep the existing migration names."]}}}));
    let record = ch.take_recorded_for(&mission);
    assert_eq!(record.len(), 1);
    assert_eq!(record[0].approval_id, card.approval_id);
    assert_eq!(record[0].answer, "answered");
}
