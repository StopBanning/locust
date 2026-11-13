//! The window's own functions (`send`, `poll`, `can_stop`, `stop`) on the native engine, against the fake
//! app-server and a temporary data folder. No window, no Codex, no Electron.
use super::*;
use crate::{ledger::Item, ledger_write::LedgerWriter, store::Store};
use serde_json::json;
use std::path::PathBuf;

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup() -> (Dirs, Store, State) {
    let root = std::env::temp_dir().join(format!(
        "locust-m2-window-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    apply_team(
        &mut s,
        vec![
            Teammate {
                id: "tm_codec".into(),
                name: "Codec".into(),
                role: "Code & Migrations".into(),
                runtime: Some("codex".into()),
                model: Some("synthetic-model".into()),
                account: None,
                busy: false,
                runs_in: "ask".into(),
                folder: Some(root.join("work").to_string_lossy().into()),
            },
            Teammate {
                id: "tm_argon".into(),
                name: "Argon".into(),
                role: "Code & Migrations".into(),
                runtime: Some("antigravity".into()),
                model: Some("m".into()),
                account: None,
                busy: false,
                runs_in: native::IN_LOCUST.into(),
                folder: None,
            },
        ],
    );
    (Dirs(root), store, s)
}

fn fake(scenario: &str) -> Option<(PathBuf, Vec<String>)> {
    Some((
        PathBuf::from("python3"),
        vec![
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/fixtures/m1-app-server.py"
            )
            .into(),
            scenario.into(),
        ],
    ))
}

/// Pump `poll` the way the window does until `done`, or fail.
fn pump(s: &mut State, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut now = 0.0;
    let mut tx = Tx::default();
    while !done(s) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}: status {:?}",
            s.status
        );
        poll(s, now, &mut tx);
        now += 0.3;
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn answers(s: &State) -> usize {
    s.ledger.as_ref().map_or(0, |p| {
        p.items
            .iter()
            .filter(|i| matches!(i, Item::Assistant { text, .. } if text == "Hello world"))
            .count()
    })
}
fn idle(s: &State) -> bool {
    s.flight.is_none()
        && s.ledger
            .as_ref()
            .is_some_and(|p| !p.loading && !p.snapshot.running && p.after_tip.is_none())
}

#[test]
fn stopped_queue_waits_for_explicit_send_and_is_removed_before_dispatch() {
    let (_dirs, store, mut s) = setup();
    s.data = Some(store.clone());
    let _g = native::hooks::install(store.clone(), fake("success"));
    s.draft.replace_text("Synthetic prompt");
    send(&mut s, &mut Tx::default());
    pump(&mut s, "first answer", idle);
    *crate::invariants::lock(&native::hooks::STATE) = (Some(store.clone()), fake("resume"));
    assert!(enqueue_follow_up(
        &mut s,
        "Synthetic follow-up".into(),
        vec![]
    ));
    s.queue_rows[0].restored = true;
    drain_queue_after_turn(&mut s, &mut Tx::default());
    assert!(s.sending.is_pending() == false);
    assert_eq!(s.queue_rows.len(), 1);
    s.draft.replace_text("Unrelated draft");
    s.queue_send_now = queue_key_now(&s);
    drain_queue_after_turn(&mut s, &mut Tx::default());
    assert_eq!(s.draft.text, "Unrelated draft");
    assert!(s.queue_rows.is_empty());
    assert!(composer::read_queue(&store).unwrap().is_empty());
    assert!(s.sending.is_pending());
    // Polling again while acceptance is unknown cannot submit this row a second time.
    drain_queue_after_turn(&mut s, &mut Tx::default());
    pump(&mut s, "queued answer", |s| idle(s) && answers(s) >= 2);
    assert!(s.queue_rows.is_empty());
}

#[test]
fn a_native_window_starts_continues_and_stops_a_codex_conversation_without_electron() {
    let (_dirs, store, mut s) = setup();
    let _g = native::hooks::install(store, fake("success"));
    let mut tx = Tx::default();

    // Start. The status line names the engine; Argon (not Codex) is marked as running in Locust for now.
    assert_eq!(s.engine.unwrap().label(), "Engine: native");
    assert_eq!(
        agent_line(&s, &s.teammates[1]),
        "Antigravity · m · runs in Locust for now"
    );
    assert_eq!(agent_line(&s, &s.teammates[0]), "Codex · synthetic-model");
    s.draft.replace_text("Synthetic prompt");
    assert!(can_send(&s));
    send(&mut s, &mut tx);
    pump(&mut s, "the first answer", |s| answers(s) == 1 && idle(s));
    let first = s
        .ledger
        .as_ref()
        .unwrap()
        .conversation
        .conversation_id
        .clone()
        .unwrap();
    assert!(first.starts_with("mission_native_"));
    assert!(!can_stop(&s), "a finished turn has no Stop");

    // Continue: the same conversation, a second prompt, thread/resume under the fake's checks.
    *crate::invariants::lock(&native::hooks::STATE) = (native::store(), fake("resume"));
    s.draft.replace_text("Synthetic follow-up");
    send(&mut s, &mut tx);
    pump(&mut s, "the continued answer", |s| {
        answers(s) == 2 && idle(s)
    });
    let pane = s.ledger.as_ref().unwrap();
    assert_eq!(
        pane.conversation.conversation_id.as_deref(),
        Some(first.as_str())
    );
    assert_ne!(
        pane.snapshot.tip, first,
        "a continuation is a new ledger file chained to the first"
    );
    let users: Vec<_> = pane
        .items
        .iter()
        .filter_map(|i| {
            if let Item::User(u) = i {
                Some(u.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(users, ["Synthetic prompt", "Synthetic follow-up"]);

    // Stop: only for the turn this window started.
    *crate::invariants::lock(&native::hooks::STATE) = (native::store(), fake("resume-interrupt"));
    s.draft.replace_text("Synthetic follow-up");
    send(&mut s, &mut tx);
    pump(&mut s, "Stop to appear", can_stop);
    assert!(s.own_turn.is_some());
    stop(&mut s);
    pump(&mut s, "the stopped turn", |s| {
        s.stopping.is_pending() == false
            && s.ledger.as_ref().is_some_and(|p| !p.snapshot.running)
            && s.ledger.as_ref().is_some_and(|p| {
                p.items
                    .iter()
                    .any(|i| matches!(i, ledger::Item::Note { text, .. } if text == "Stopped"))
            })
    });
    assert!(s.own_turn.is_none() && !can_stop(&s));
    assert!(
        !s.ledger.as_ref().unwrap().snapshot.failed,
        "stopped is cancelled, not failed"
    );

    // Recent shows the native conversation, marked.
    s.recents.request((), move |()| {
        let listed = native::list_recent(12, true).ok_or_else(|| "none".to_string())?;
        Ok(RecentBatch {
            rows: listed
                .summaries
                .iter()
                .map(|m| ConversationRow {
                    id: m.mission_id.clone(),
                    teammate: "Codex".into(),
                    title: m.title.clone(),
                    started_by: if listed.native.contains(&m.mission_id) {
                        "native"
                    } else {
                        "ledger"
                    }
                    .into(),
                    state: "completed".into(),
                    updated: m.updated.clone(),
                })
                .collect(),
            summaries: HashMap::new(),
            local: true,
        })
    });
    pump(&mut s, "recents", |s| !s.recent.is_empty());
    assert!(s.recent.iter().all(|r| r.started_by == "native"));
    assert!(
        s.recent.len() >= 3,
        "every native turn is a row: {}",
        s.recent.len()
    );
}

#[test]
fn stop_is_never_offered_for_a_turn_another_process_started() {
    let (_dirs, store, mut s) = setup();
    let _g = native::hooks::install(store.clone(), None);
    // A running ledger this process did not start (no registry entry).
    let mut w = LedgerWriter::create(&store, "someone_elses", "Prompt", "m", "x").unwrap();
    w.append("run.started", json!({"runtimeThreadId":"t"}), Some("t"))
        .unwrap();
    let header = s.teammates[0].clone();
    begin_ledger(
        &mut s,
        "someone_elses".into(),
        String::new(),
        header,
        None,
        &mut Tx::default(),
    );
    pump(&mut s, "the live view", |s| {
        s.ledger.as_ref().is_some_and(|p| p.snapshot.running)
    });
    assert!(!can_stop(&s));
    // Even a forged ownership record is not enough on the native engine: the process must hold the turn.
    s.own_turn = Some(OwnTurn {
        conversation: "someone_elses".into(),
        tip: Some("someone_elses".into()),
        before: None,
    });
    assert!(
        !can_stop(&s),
        "native ownership is the registry, not a flag in the window"
    );
    assert!(locust_core::engine_api::api().stop("someone_elses").is_err());
}

#[test]
fn stop_survives_reopening_an_owned_native_turn_from_recent() {
    let (_dirs, store, mut s) = setup();
    let _g = native::hooks::install(store, fake("resume-interrupt"));
    // An earlier native conversation to continue.
    let mut tx = Tx::default();
    *crate::invariants::lock(&native::hooks::STATE) = (native::store(), fake("success"));
    s.draft.replace_text("Synthetic prompt");
    send(&mut s, &mut tx);
    pump(&mut s, "the first answer", |s| answers(s) == 1 && idle(s));
    let anchor = s
        .ledger
        .as_ref()
        .unwrap()
        .conversation
        .conversation_id
        .clone()
        .unwrap();
    // Start a long turn and wait for Stop.
    *crate::invariants::lock(&native::hooks::STATE) = (native::store(), fake("resume-interrupt"));
    s.draft.replace_text("Synthetic follow-up");
    send(&mut s, &mut tx);
    pump(&mut s, "Stop to appear", can_stop);
    // Leave the view (another teammate), then reopen the running turn from Recent.
    select_teammate(&mut s, "tm_argon", &mut tx);
    assert!(s.ledger.is_none() && !can_stop(&s));
    let listed = native::list_recent(12, true).unwrap();
    let running = listed
        .summaries
        .iter()
        .find(|m| m.running)
        .expect("a running native turn")
        .clone();
    s.ledger_summaries
        .insert(running.mission_id.clone(), running.clone());
    open_recent(
        &mut s,
        ConversationRow {
            id: running.mission_id.clone(),
            teammate: "Codex".into(),
            title: running.title.clone(),
            started_by: "native".into(),
            state: "running".into(),
            updated: running.updated.clone(),
        },
        &mut tx,
    );
    assert!(
        s.own_turn.is_none(),
        "the window's own flag was cleared by leaving the view"
    );
    pump(&mut s, "the reopened turn to stream", |s| {
        s.ledger.as_ref().is_some_and(|p| p.snapshot.running) && s.flight.is_some()
    });
    assert!(
        can_stop(&s),
        "ownership is the process registry: Stop stays for an owned turn after reopening"
    );
    stop(&mut s);
    pump(&mut s, "the stopped turn", |s| {
        s.ledger.as_ref().is_some_and(|p| !p.snapshot.running)
            && s.ledger.as_ref().is_some_and(|p| {
                p.items
                    .iter()
                    .any(|i| matches!(i, ledger::Item::Note { text, .. } if text == "Stopped"))
            })
    });
    assert!(!can_stop(&s));
    let _ = anchor;
}

#[test]
fn a_stale_completed_pane_cannot_send_beside_a_turn_started_elsewhere() {
    let (dirs, store, mut s) = setup();
    let _g = native::hooks::install(store.clone(), fake("resume"));
    // Electron's completed conversation, shown in the pane.
    let ledger = dirs.0.join("electron/mission-ledger");
    let mk = |id: &str, parent: Option<&str>, thread: &str, done: bool| {
        let mut meta = json!({"missionId":id,"prompt":format!("prompt {id}"),"runtime":"codex","model":"synthetic-model","createdAt":"2026-11-01T00:00:00.000Z"});
        if let Some(p) = parent {
            meta["continuesFrom"] = json!({"missionId":p,"checkpointEpoch":1,"reason":"follow-up"});
        }
        let mut lines = vec![
            json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"metadata":meta}),
            json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-11-01T00:00:01.000Z","type":"run.started","payload":{"runtimeThreadId":thread}}}),
        ];
        if done {
            lines.push(json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":2,"occurredAt":"2026-11-01T00:00:02.000Z","type":"run.completed","payload":{"runtimeThreadId":thread}}}));
        }
        std::fs::write(
            ledger.join(format!("{id}.jsonl")),
            lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
        )
        .unwrap();
    };
    mk("win_a", None, "thread_old", true);
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(ledger.join("win_a.jsonl"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    let header = s.teammates[0].clone();
    let mut tx = Tx::default();
    begin_ledger(&mut s, "win_a".into(), String::new(), header, None, &mut tx);
    pump(&mut s, "the completed view", |s| {
        s.ledger.as_ref().is_some_and(|p| !p.loading)
    });
    assert_eq!(s.ledger.as_ref().unwrap().snapshot.tip, "win_a");
    // Electron starts child B while the idle pane has stopped polling.
    mk("win_b", Some("win_a"), "thread_fake", false);
    s.draft.replace_text("Synthetic follow-up");
    assert!(
        can_send(&s),
        "the pane still looks idle: the window cannot know"
    );
    send(&mut s, &mut tx);
    pump(&mut s, "the refusal", |s| {
        s.sending.is_pending() == false && s.flight.is_none() && s.status.contains("still running")
    });
    assert!(s.status.contains("still running"), "{}", s.status);
    assert_eq!(
        s.status_tone as u8,
        StatusTone::Red as u8,
        "a refusal is shown, not swallowed"
    );
    assert_eq!(
        s.draft.text, "Synthetic follow-up",
        "the unsent message is put back"
    );
    assert!(
        !dirs.0.join("native/mission-ledger").exists()
            || std::fs::read_dir(dirs.0.join("native/mission-ledger"))
                .unwrap()
                .count()
                == 0
    );
}

#[test]
fn a_teammate_native_cannot_run_sends_nothing() {
    let (dirs, store, mut s) = setup();
    let _g = native::hooks::install(store, fake("success"));
    select_teammate(&mut s, "tm_argon", &mut Tx::default());
    s.draft.replace_text("Synthetic prompt");
    assert!(native_blocked(&s));
    assert!(
        !can_send(&s),
        "Send is off for a teammate that runs in Locust for now"
    );
    // Even if a send were forced, the engine refuses and creates no ledger.
    let refused = locust_core::engine_api::api().send(
        native::Send {
            teammate: s.teammates[1].clone(),
            conversation: None,
            prompt: "x".into(),
            model: None,
            effort: None,
        },
        native::Origin::default(),
    );
    assert!(refused.unwrap_err().contains("runs in Locust for now"));
    assert!(!dirs.0.join("native/mission-ledger").exists());
}

#[test]
fn door_engine_still_goes_through_the_door_path() {
    let (_dirs, _store, mut s) = setup();
    s.engine = Some(Kind::Door);
    select_teammate(&mut s, "tm_argon", &mut Tx::default());
    s.draft.replace_text("hello");
    assert!(
        !native_blocked(&s),
        "on the door every teammate sends as before"
    );
    assert!(can_send(&s));
    assert_eq!(agent_line(&s, &s.teammates[1]), "Antigravity · m");
    assert!(!can_stop(&s));
}
