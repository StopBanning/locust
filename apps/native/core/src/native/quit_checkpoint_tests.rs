//! codex-mission.test.ts, `dispose`: quitting checkpoints each turn it cut short with reason "shutdown", once that turn's
//! own records have landed; none when nothing was running; and a checkpoint that cannot be written never stops the quit.
//! Synthetic data and the fake app-server only: no agent, no network.
use super::*;

fn checkpoints_in(path: &Path) -> Vec<Value> {
    records(path).into_iter().filter(|r| r["recordType"] == "mission.checkpoint").map(|r| r["checkpoint"].clone()).collect()
}

/// A second teammate, so two turns can run at once.
fn other_job(t: &Temp, prompt: &str) -> Send {
    let mut job = job(t, None, prompt);
    job.teammate.id = "tm_other".into();
    job.teammate.name = "Other".into();
    job
}

/// A turn this process ran earlier in the conversation, which the person stopped: its ledger, in native's folder.
fn stopped_native_turn(t: &Temp, thread: &str) -> (String, PathBuf) {
    let id = format!("mission_native_{}_{}", std::process::id(), crate::invariants::unique_nanos());
    let lines = [
        json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":"2026-11-26T00:00:00.000Z","metadata":{"missionId":id,"runId":format!("run_{id}"),"prompt":"Earlier question","runtime":"codex","model":"synthetic-model","createdAt":"2026-11-26T00:00:00.000Z"}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":2,"occurredAt":"2026-11-26T00:00:01.000Z","event":{"id":"e1","sequence":1,"occurredAt":"2026-11-26T00:00:01.000Z","type":"run.started","payload":{"runtimeThreadId":thread}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":3,"occurredAt":"2026-11-26T00:00:02.000Z","event":{"id":"e2","sequence":2,"occurredAt":"2026-11-26T00:00:02.000Z","type":"run.cancelled","payload":{"runtimeThreadId":thread}}}),
    ];
    let path = path_of(t, &id);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    (id, path)
}

#[test]
fn checkpoints_the_mission_it_cut_short_after_that_run_has_finished_settling() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("resume-interrupt"));
    // The conversation's first turn ran here and the person stopped it; its follow-up is running when Locust quits.
    let (first, first_path) = stopped_native_turn(&t, "thread_fake");
    let first_before = fs::read(&first_path).unwrap();
    send(job(&t, Some(&first), "Synthetic follow-up")).unwrap();
    let tip = real_tip(&first).unwrap();
    assert_ne!(tip, first);
    let path = path_of(&t, &tip);
    wait("the turn to start", || path.exists() && started(&path));
    assert!(owns(&tip) && owns(&first), "the turn runs under its own id and its conversation's");

    shutdown_in_background_for(Duration::from_secs(8));
    wait_for_shutdown();

    assert!(!owns(&tip), "the quit waited for the turn to settle");
    let all = records(&path);
    let cancelled = all.iter().position(|r| r["event"]["type"] == "run.cancelled").expect("the turn's own ending landed");
    let at = all.iter().position(|r| r["recordType"] == "mission.checkpoint").expect("the turn the quit cut short is checkpointed");
    assert!(cancelled < at, "the checkpoint follows the turn's own ending");
    let checkpoint = &all[at]["checkpoint"];
    // Electron: createCheckpoint(missionId, 'shutdown') for the mission it cut short.
    assert_eq!(checkpoint["missionId"], json!(tip));
    assert_eq!(checkpoint["reason"], "shutdown");
    assert_eq!(checkpoint["epoch"], 1);
    // Reconciled from the settled ledger, every event the turn wrote included, not raced against it.
    let last_event = all.iter().filter_map(|r| r["event"]["sequence"].as_u64()).max();
    assert_eq!(checkpoint["reconciledThroughSequence"].as_u64(), last_event);
    assert_eq!(checkpoints_in(&path).len(), 1, "one checkpoint for the one turn cut short");
    // The turn the person stopped earlier is not one the quit cut short: never offered a resume (recovery.ts).
    assert_eq!(fs::read(&first_path).unwrap(), first_before, "the earlier turn is left as it was");
    // The next launch offers to resume from it: the app closed underneath the run (recovery.ts).
    assert!(matches!(ledger::read_transcript(&path).recovery, Some(crate::recovery::Offer::Resume { epoch: 1, .. })));
}

#[test]
fn does_not_checkpoint_when_no_mission_was_running() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let done = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&done.conversation_id);
    let path = path_of(&t, &done.conversation_id);
    let before = fs::read(&path).unwrap();

    shutdown_in_background_for(Duration::from_secs(8));
    wait_for_shutdown();

    assert_eq!(fs::read(&path).unwrap(), before, "a turn that had already finished is left as it was");
    assert!(checkpoints_in(&path).is_empty());
}

#[test]
fn still_shuts_down_when_the_checkpoint_write_fails() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("interrupt"));
    let first = send(job(&t, None, "Synthetic prompt")).unwrap();
    let first_path = path_of(&t, &real_tip(&first.conversation_id).unwrap());
    wait("the first turn to start", || first_path.exists() && started(&first_path));
    let second = send(other_job(&t, "Synthetic prompt")).unwrap();
    let second_path = path_of(&t, &real_tip(&second.conversation_id).unwrap());
    wait("the second turn to start", || second_path.exists() && started(&second_path));
    // The first turn's ledger can no longer be opened to append: its checkpoint write fails (Electron: "disk full").
    // The turn's own writer holds the file open, so its ending still lands.
    let mut read_only = fs::metadata(&first_path).unwrap().permissions();
    read_only.set_readonly(true);
    fs::set_permissions(&first_path, read_only).unwrap();

    let begun = Instant::now();
    shutdown_in_background_for(Duration::from_secs(8));
    wait_for_shutdown();

    assert!(begun.elapsed() < Duration::from_secs(8), "the failed write did not hold the quit: {:?}", begun.elapsed());
    assert!(!owns(&first.conversation_id) && !owns(&second.conversation_id), "both turns settled");
    assert_eq!(kinds(&first_path).iter().filter(|k| *k == "run.cancelled").count(), 1, "{:?}", kinds(&first_path));
    assert!(checkpoints_in(&first_path).is_empty(), "the write that failed left nothing");
    // The quit went on past the failure: the turn after it is still checkpointed.
    let second_points = checkpoints_in(&second_path);
    assert_eq!(second_points.len(), 1, "{second_points:?}");
    assert_eq!(second_points[0]["reason"], "shutdown");
    #[allow(clippy::permissions_set_readonly_false)]
    {
        let mut writable = fs::metadata(&first_path).unwrap().permissions();
        writable.set_readonly(false);
        fs::set_permissions(&first_path, writable).unwrap();
    }
}

/// Beyond Electron, whose quit waits for every run however long: Rust's quit leaves at its deadline, and a turn still
/// running then has not landed its records, so it gets no checkpoint (one written now would describe a ledger still
/// being written).
#[test]
fn a_turn_still_running_at_the_quits_deadline_is_not_checkpointed() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), None);
    let id = format!("mission_native_{}_{}", std::process::id(), crate::invariants::unique_nanos());
    let path = path_of(&t, &id);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let lines = [
        json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":"2026-11-26T00:00:00.000Z","metadata":{"missionId":id,"runId":format!("run_{id}"),"prompt":"Synthetic prompt","runtime":"codex","model":"synthetic-model","createdAt":"2026-11-26T00:00:00.000Z"}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":2,"occurredAt":"2026-11-26T00:00:01.000Z","event":{"id":"e1","sequence":1,"occurredAt":"2026-11-26T00:00:01.000Z","type":"run.started","payload":{}}}),
    ];
    fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    with_running(|m| m.insert(id.clone(), flag.clone()));

    shutdown_in_background_for(Duration::from_millis(300));
    wait_for_shutdown();
    with_running(|m| m.remove(&id));

    assert!(flag.load(Ordering::Relaxed), "the turn was told to stop");
    assert!(checkpoints_in(&path).is_empty(), "no checkpoint for a turn whose records had not landed");
}
