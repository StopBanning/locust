//! The engine behind its socket, with the fake app-server: no agent, no credentials, no network, no window.
use super::*;
use crate::{native, store::Store};
use std::time::Instant;

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(p.join("electron/mission-ledger")).unwrap();
        std::fs::create_dir_all(p.join("work")).unwrap();
        Temp(p)
    }
    fn store(&self) -> Store {
        Store::new(self.0.join("native"), self.0.join("electron")).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fake(scenario: &str) -> Option<(PathBuf, Vec<String>)> {
    Some((
        PathBuf::from("python3"),
        vec![concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py").into(), scenario.into()],
    ))
}

fn codec(t: &Temp, mode: &str) -> Teammate {
    Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        busy: false,
        runs_in: mode.into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    }
}

fn kinds(store: &Store, mission: &str) -> Vec<String> {
    std::fs::read_to_string(store.ledger_dir().unwrap().join(format!("{mission}.jsonl")))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|r| r["event"]["type"].as_str().or(r["recordType"].as_str()).map(str::to_owned))
        .collect()
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn the_wire_carries_a_send_an_origin_and_a_card_whole() {
    let t = Temp::new();
    let job = Send {
        teammate: codec(&t, "approve-each"),
        conversation: Some("mission_a".into()),
        prompt: "Synthetic prompt".into(),
        model: Some("synthetic-model".into()),
        effort: Some("high".into()),
    };
    let back = send_from(&send_json(&job)).unwrap();
    assert_eq!(back.teammate, job.teammate);
    assert_eq!((back.conversation, back.prompt, back.model, back.effort), (job.conversation, job.prompt, job.model, job.effort));

    let origin = Origin {
        mode: Some(Mode::parse("plan").unwrap()),
        folder: Some(t.0.join("work")),
        started_by: Some(json!({"kind": "mcp"})),
        runtime: Some("claude".into()),
        reference_ids: Some(vec!["ref_1".into()]),
    };
    let o = origin_from(&origin_json(&origin)).unwrap();
    assert_eq!(o.mode.map(Mode::as_str), Some("plan"));
    assert_eq!((o.folder, o.started_by, o.runtime, o.reference_ids), (origin.folder, origin.started_by, origin.runtime, origin.reference_ids));
    // Left out stays left out: a window's plain send must not arrive as a door's.
    let plain = origin_from(&origin_json(&Origin::default())).unwrap();
    assert!(plain.mode.is_none() && plain.started_by.is_none() && plain.reference_ids.is_none());

    let card = Request {
        approval_id: "ap_1".into(),
        mission_id: "mission_a".into(),
        runtime: "claude".into(),
        kind: Kind::FileChange,
        summary: "Edit hello.txt".into(),
        detail: "hello.txt".into(),
        cwd: Some("/work".into()),
        requested_at: "2026-11-12T00:00:00Z".into(),
        always_covers: Some("Edits in this folder".into()),
        always_key: Some("edit:/work".into()),
        data_sent_says: None,
        reversible_says: Some("Undo with git".into()),
        claude_input: Some(json!({"file_path": "hello.txt"})),
    };
    let c = card_from(&card_json(&card)).unwrap();
    assert_eq!(card_json(&c), card_json(&card));
    for d in [Decision::ApproveOnce, Decision::ApproveAlways, Decision::Deny] {
        assert_eq!(decision_from(decision_word(&d)).unwrap(), d);
    }
    assert!(decision_from("yes").is_err(), "an answer the engine does not know is refused, never guessed");
}

#[test]
fn a_second_engine_refuses_to_start_while_one_runs() {
    let t = Temp::new();
    let dir = t.0.join("native");
    let first = Server::start(&dir).unwrap();
    assert_eq!(Server::start(&dir).err().as_deref(), Some("Another Locust engine is already running."));
    let mode = std::fs::metadata(first.socket()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the socket is the owner's alone");
    drop(first);
    // Once it has gone, the next one starts, over the socket file a crash would have left. A process another test
    // forks at that moment holds a copy of the lock's descriptor until its exec closes it, so allow two seconds; a
    // lock that never frees still fails here.
    let end = Instant::now() + Duration::from_secs(2);
    let again = loop {
        match Server::start(&dir) {
            Ok(server) => break server,
            Err(_) if Instant::now() < end => std::thread::sleep(Duration::from_millis(20)),
            Err(why) => panic!("the engine did not start again: {why}"),
        }
    };
    assert!(again.socket().exists());
}

#[test]
fn a_turn_sent_through_the_socket_runs_and_its_card_is_answered_through_it() {
    let t = Temp::new();
    let store = t.store();
    let _hook = native::hooks::install(store.clone(), fake("approve-allow"));
    let server = Server::start(store.root()).unwrap();
    let engine = Client::new(server.socket().to_path_buf());

    let started = engine
        .send(
            Send { teammate: codec(&t, "approve-each"), conversation: None, prompt: "Synthetic prompt".into(), model: None, effort: None },
            Origin { started_by: Some(json!({"kind": "mcp"})), ..Default::default() },
        )
        .unwrap();
    let mission = native::real_tip(&started.conversation_id).unwrap_or(started.conversation_id.clone());
    assert!(engine.owns(&started.conversation_id), "the engine owns the turn it started");
    assert!(engine.running().contains(&started.conversation_id));

    until("the card", || !engine.waiting().is_empty());
    let card = engine.waiting().remove(0);
    assert_eq!(card.mission_id, mission);
    engine.decide(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    until("the turn's end", || kinds(&store, &mission).iter().any(|k| k == "run.completed"));
    until("the turn to leave the running list", || !engine.owns(&started.conversation_id));
    // An answer is given once: the engine's refusal comes back through the socket in its own words.
    assert!(engine.decide(&card.approval_id, Decision::ApproveOnce, None).is_err());
    assert!(engine.stop(&started.conversation_id).is_err(), "nothing left to stop");
}

#[test]
fn with_no_engine_running_the_client_says_so() {
    let t = Temp::new();
    let engine = Client::new(t.0.join("native").join(SOCKET));
    let refused = engine
        .send(
            Send { teammate: codec(&t, "ask"), conversation: None, prompt: "x".into(), model: None, effort: None },
            Origin::default(),
        )
        .unwrap_err();
    assert_eq!(refused, "Locust's engine is not running.");
    assert!(engine.running().is_empty() && engine.waiting().is_empty() && !engine.owns("mission_a"));
}

#[test]
fn a_turn_quick_ask_starts_is_answered_and_stopped_from_the_window() {
    // Plan E3's check: two processes on one engine. One client stands for Quick Ask, the other for the window.
    let t = Temp::new();
    let store = t.store();
    let _hook = native::hooks::install(store.clone(), fake("approve-allow"));
    let server = Server::start(store.root()).unwrap();
    let quick = Client::new(server.socket().to_path_buf());
    let window = Client::new(server.socket().to_path_buf());

    // A turn Quick Ask starts raises its card where the window reads cards, and the window's answer finishes it.
    let asked = quick
        .send(
            Send { teammate: codec(&t, "approve-each"), conversation: None, prompt: "Synthetic prompt".into(), model: None, effort: None },
            Origin::default(),
        )
        .unwrap();
    let mission = native::real_tip(&asked.conversation_id).unwrap_or(asked.conversation_id.clone());
    until("the card in the window", || window.waiting().iter().any(|c| c.mission_id == mission));
    let card = window.waiting().into_iter().find(|c| c.mission_id == mission).unwrap();
    window.decide(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    until("the answered turn's end", || kinds(&store, &mission).iter().any(|k| k == "run.completed"));

    // A long turn Quick Ask starts is the window's to stop: it owns it, and Stop ends it as cancelled.
    *crate::invariants::lock(&native::hooks::STATE) = (Some(store.clone()), fake("interrupt"));
    let long = quick
        .send(
            Send { teammate: codec(&t, "ask"), conversation: None, prompt: "Synthetic prompt".into(), model: None, effort: None },
            Origin::default(),
        )
        .unwrap();
    let long_mission = native::real_tip(&long.conversation_id).unwrap_or(long.conversation_id.clone());
    assert!(window.owns(&long.conversation_id), "the window sees the turn as one it can stop");
    window.stop(&long.conversation_id).unwrap();
    until("the stopped turn's ending", || kinds(&store, &long_mission).iter().any(|k| k == "run.cancelled"));
    until("the stopped turn to leave the running list", || !window.owns(&long.conversation_id));
}

#[test]
fn join_or_start_finds_the_engine_that_answers() {
    let t = Temp::new();
    let dir = t.0.join("native");
    let server = Server::start(&dir).unwrap();
    // The engine is running: joining it starts nothing (the path given would fail if it were run).
    let joined = join_or_start(&dir, Path::new("/nonexistent/locust-native")).unwrap();
    // Asked through it, the engine answers (other tests' turns may be running, so ask about one that cannot exist).
    assert!(!joined.owns("mission_join_check_none"));
    assert!(joined.stop("mission_join_check_none").is_err());
    drop(server);
    // None running, and the binary cannot start: said in words, not hung.
    let refused = join_or_start(&dir, Path::new("/nonexistent/locust-native")).err().unwrap();
    assert!(refused.starts_with("Locust's engine could not be started"), "{refused}");
}
