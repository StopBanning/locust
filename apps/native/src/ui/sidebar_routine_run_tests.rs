//! Electron's `a-routine-run-is-marked.test.tsx`, case by case, on the drawn sidebar (a routine's run wears the
//! Routines clock and says whose it is), the conversation's header, and conversation_list.rs's `routine_of`.
use super::*;
use crate::conversation_list::{routine_of, routine_step_phrase};
use crate::org::RoutineName;

const AT: &str = "2026-09-23T05:00:00.000Z";
const WORDS: &str = "Read the README and list the TODOs";

fn wren() -> Teammate {
    Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: Some("claude".into()),
        model: None, account: None, busy: false, runs_in: "ask".into(), folder: None }
}

/// A profile whose saved routines are `routines`, read the way the sidebar reads them (org.rs `load_org`).
fn with_routines(routines: &str) -> State {
    let temp = std::env::temp_dir().join(format!("locust-sb-routine-run-{}-{}", std::process::id(), routines.len()));
    let _ = std::fs::remove_dir_all(&temp);
    let (electron, native) = (temp.join("electron"), temp.join("native"));
    std::fs::create_dir_all(&electron).unwrap();
    std::fs::create_dir_all(&native).unwrap();
    std::fs::write(native.join("routines.json"), format!(r#"{{"schemaVersion":1,"routines":[{routines}]}}"#)).unwrap();
    let store = crate::store::Store::new(native, electron).unwrap();
    let mut s = State::default();
    s.demo = true;
    s.org = org::load_org(&store);
    s.teammates.push(wren());
    s
}

const MORNING: &str = r#"{"routineId":"rt_morning","name":"Morning check","steps":["a","b","c"]}"#;

/// A conversation of Wren's; `routine` is the `startedBy` of a routine's replay, read from its recorded header.
fn turn(s: &mut State, id: &str, state: &str, routine: Option<(&str, u64)>) {
    s.recent.push(ConversationRow { id: id.into(), title: WORDS.into(), teammate: "Wren".into(),
        started_by: "native".into(), state: state.into(), updated: AT.into() });
    s.mission_owners.insert(id.into(), "tm_wren".into());
    let started_by = routine.map_or(serde_json::Value::Null, |(rid, step)| serde_json::json!({"kind": "routine", "routineId": rid, "step": step}));
    let meta = ledger::metadata(&serde_json::json!({"recordType": "mission.created",
        "metadata": {"missionId": id, "prompt": WORDS, "runtime": "claude", "createdAt": AT, "startedBy": started_by}})).unwrap();
    s.ledger_summaries.insert(id.into(), ledger::Summary { mission_id: id.into(), title: WORDS.into(), runtime: "claude".into(),
        created_at: AT.into(), updated: AT.into(), running: state == "running", started_by_kind: meta.started_by_kind,
        started_by_routine: meta.started_by_routine, started_by_step: meta.started_by_step, ..Default::default() });
}

fn draw(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

fn clocks(rt: &rune::Runtime<State>) -> Vec<(String, Option<String>)> {
    rt.elements().into_iter()
        .filter(|e| !e.exiting && e.role == "img" && e.key.as_deref().is_some_and(|k| k.starts_with("conversation-routine-")))
        .map(|e| (e.key.unwrap(), e.label)).collect()
}

fn help_of(rt: &rune::Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key)).and_then(|e| e.help)
}

fn said(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_routines_run_wears_the_clock_its_source_conversation_does_not() {
    // case: carries the Routines clock, and the conversation it came from does not
    let mut s = with_routines(MORNING);
    turn(&mut s, "mission_person", "completed", None);
    turn(&mut s, "mission_replay", "completed", Some(("rt_morning", 1)));
    let rt = draw(s);
    assert_eq!(clocks(&rt), [("conversation-routine-mission_replay".to_owned(), Some("From the routine Morning check".to_owned()))]);
    // Before the title, as the plug of an app-started row is.
    assert!(rt.find("conversation-routine-mission_replay").unwrap().0 < rt.find("conversation-title-mission_replay").unwrap().0);
}

#[test]
fn every_row_says_whose_it_is_and_which_routine_on_hover() {
    // case: says whose each row is, and which routine, on hover
    let mut s = with_routines(MORNING);
    turn(&mut s, "mission_person", "completed", None);
    turn(&mut s, "mission_replay", "completed", Some(("rt_morning", 1)));
    let rt = draw(s);
    assert_eq!(help_of(&rt, "recent-mission_person").as_deref(), Some("Read the README and list the TODOs · Wren"));
    assert_eq!(help_of(&rt, "recent-mission_replay").as_deref(), Some("Read the README and list the TODOs · Wren · from the routine Morning check"));
    // The face names its teammate rather than hiding from a screen reader.
    assert!(rt.elements().iter().any(|e| !e.exiting && e.label.as_deref() == Some("Wren")), "the face says Wren");
}

#[test]
fn a_run_whose_routine_was_deleted_is_still_marked() {
    // case: still marks a run whose routine was since deleted
    let mut s = with_routines("");
    turn(&mut s, "mission_replay", "completed", Some(("rt_gone", 1)));
    let rt = draw(s);
    assert_eq!(clocks(&rt), [("conversation-routine-mission_replay".to_owned(), Some("From a routine".to_owned()))]);
    assert_eq!(help_of(&rt, "recent-mission_replay").as_deref(), Some("Read the README and list the TODOs · Wren · from a routine"));
}

#[test]
fn a_running_routine_row_names_its_step_where_its_age_goes() {
    // case: says which step it is on while it runs, where its age would be
    let mut s = with_routines(MORNING);
    turn(&mut s, "mission_replay", "running", Some(("rt_morning", 2)));
    let rt = draw(s);
    let age = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("conversation-age-mission_replay")).expect("the step");
    assert_eq!(age.text.as_deref(), Some("step 2 of 3"));
    assert_eq!(age.help.as_deref(), Some("Morning check: step 2 of 3"));
    // Finished, it is back to its age.
    let mut s = with_routines(MORNING);
    turn(&mut s, "mission_replay", "completed", Some(("rt_morning", 2)));
    assert!(!said(&draw(s)).contains("step 2 of 3"));
}

#[test]
fn a_subagent_at_work_is_said_on_the_row_that_sent_it() {
    // case: says a subagent is working on the conversation that sent it (Yurt's #17)
    let running = |activity| {
        let mut s = with_routines("");
        turn(&mut s, "mission_person", "running", None);
        s.seen_live.push("mission_person".into());
        s.live_faces.by_mission.insert("mission_person".into(), activity);
        said(&draw(s))
    };
    assert!(running(locust_core::bot_rig::LiveActivity::Delegating).contains("subagent working"));
    assert!(!running(locust_core::bot_rig::LiveActivity::Working).contains("subagent working"));
}

#[test]
fn the_header_names_the_routine_step_where_it_said_running() {
    // case: names the step in the header, in place of "running"
    let morning = [RoutineName { id: "rt_morning".into(), name: "Morning check".into(), steps: 3, ..Default::default() }];
    assert_eq!(routine_step_phrase(Some("routine"), Some("rt_morning"), Some(2), &morning).as_deref(), Some("routine Morning check, step 2 of 3"));
    assert_eq!(routine_step_phrase(Some("routine"), Some("rt_gone"), Some(1), &[]).as_deref(), Some("routine, step 1"));
    assert_eq!(routine_step_phrase(Some("relay"), None, None, &[]), None);
    assert_eq!(routine_step_phrase(None, None, None, &[]), None);
    // In the window: the conversation's header says it where a running turn says "running".
    let header = |routine: Option<(&str, u64)>| {
        let mut s = with_routines(MORNING);
        s.selected = 0;
        s.conversations.insert("tm_wren".into(), Conversation::default());
        let started_by = routine.map_or(serde_json::Value::Null, |(rid, step)| serde_json::json!({"kind": "routine", "routineId": rid, "step": step}));
        let mut snap = ledger::Snapshot::default();
        snap.running = true;
        snap.turn_metas.push(ledger::metadata(&serde_json::json!({"recordType": "mission.created",
            "metadata": {"missionId": "mission_replay", "prompt": WORDS, "startedBy": started_by}})).unwrap());
        s.ledger = Some(LedgerPane { cold_tip: None, header: s.teammates[0].clone(), conversation: Conversation::default(), items: vec![],
            dir: std::path::PathBuf::new(), worker: None, snapshot: snap, due: f64::INFINITY, loading: false, after_tip: None,
            after_tip_deadline: None, backoff: 0.0 });
        let mut rt = rune::Runtime::new(s, |s, _| crate::thread_view(s, &Palette::new(true)), (1000.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some("thread-header-route")).and_then(|e| e.text).unwrap_or_default()
    };
    assert!(header(Some(("rt_morning", 2))).ends_with(" · routine Morning check, step 2 of 3"), "{}", header(Some(("rt_morning", 2))));
    assert!(header(None).ends_with(" · running"), "{}", header(None));
}

#[test]
fn a_routine_is_read_only_from_a_routines_starter() {
    // case: reads a routine from a live starter or a recorded one, and nothing else
    assert_eq!(routine_of(Some("routine"), Some("rt_morning")).as_deref(), Some("rt_morning"));
    assert_eq!(routine_of(Some("relay"), None), None);
    assert_eq!(routine_of(None, None), None);
    // A recorded one: the turn's header in the ledger.
    let recorded = |started_by: serde_json::Value| ledger::metadata(&serde_json::json!({"recordType": "mission.created",
        "metadata": {"missionId": "m_1", "prompt": "x", "startedBy": started_by}})).unwrap();
    let m = recorded(serde_json::json!({"kind": "routine", "routineId": "rt_morning", "step": 1}));
    assert_eq!(routine_of(m.started_by_kind.as_deref(), m.started_by_routine.as_deref()).as_deref(), Some("rt_morning"));
    let m = recorded(serde_json::json!({"kind": "relay", "routineId": "rt_morning"}));
    assert_eq!(routine_of(m.started_by_kind.as_deref(), m.started_by_routine.as_deref()), None);
}
