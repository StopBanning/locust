//! codex-mission.test.ts, `dispose`: quitting checkpoints each turn it cut short with reason "shutdown", once that turn's
//! own records have landed; none when nothing was running; and a checkpoint that cannot be written never stops the quit.
//! Then `resume` (the card's press, through `carry`): such a turn is picked up on its own agent from a fresh checkpoint,
//! as a follow-up handed the brief, never its prompt again; a turn the card does not offer to resume is left as it was.
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

// ---- row 2: the card resumes (codex-mission.ts `resume`, on the agent the turn ran on) ----------------------------

/// The roster the resumed turn's owner is read from: Codec, in the work folder, on Codex in Ask.
fn codec_team(t: &Temp) {
    let work = t.0.join("work");
    t.store()
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "role": "Code & Migrations", "hue": "lime",
                "createdAt": "2026-11-26T00:00:00.000Z", "folder": work.to_string_lossy(), "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}}],
                "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
}

/// A real turn of Codec's, cut short by quitting: its ending and then its "shutdown" checkpoint are in its ledger.
fn cut_short_by_quit(t: &Temp) -> (String, PathBuf) {
    let g = hooks::install(t.store(), fake("interrupt"));
    codec_team(t);
    let first = send(job(t, None, "Synthetic prompt")).unwrap();
    let id = real_tip(&first.conversation_id).unwrap();
    let path = path_of(t, &id);
    wait("the turn to start", || path.exists() && started(&path));
    shutdown_in_background_for(Duration::from_secs(8));
    wait_for_shutdown();
    drop(g);
    assert_eq!(checkpoints_in(&path).iter().map(|c| c["reason"].clone()).collect::<Vec<_>>(), [json!("shutdown")]);
    (id, path)
}

/// The turns that continue `id`, by their first record's metadata.
fn continuing(t: &Temp, id: &str) -> Vec<Value> {
    ledgers(t).into_iter().map(|p| records(&p)).filter(|r| r[0]["metadata"]["continuesFrom"]["missionId"] == id).map(|r| r[0]["metadata"].clone()).collect()
}

// case: and on the same runtime, as a follow-up still
#[test]
fn and_on_the_same_runtime_as_a_follow_up_still() {
    let t = Temp::new();
    let (id, path) = cut_short_by_quit(&t);
    let _g = hooks::install(t.store(), fake("cards-share"));
    // The card's press: the turn's own agent, no model, no message.
    let (next, _, mode) = carry(&id, "codex", None).unwrap();
    finished(&next);
    // Electron: createCheckpoint(missionId, 'manual') on the same runtime, after the quit's own.
    let reasons: Vec<Value> = checkpoints_in(&path).iter().map(|c| c["reason"].clone()).collect();
    assert_eq!(reasons, [json!("shutdown"), json!("manual")]);
    let started = continuing(&t, &id);
    assert_eq!(started.len(), 1, "{started:?}");
    assert_eq!(started[0]["missionId"], json!(next));
    assert_eq!(started[0]["runtime"], "codex");
    assert_eq!(started[0]["continuesFrom"]["reason"], "follow-up");
    // Electron: start(..., { kind: 'resume', epoch: checkpoint.epoch }).
    assert_eq!(started[0]["startedBy"], json!({"kind": "resume", "epoch": 2}));
    assert_eq!(mode, "ask", "in the mode the turn ran in");
}

// case: runs in the teammateu2019s own branch, as theirs, under their one-run guard
#[test]
fn a_resume_runs_in_the_teammates_own_folder_as_theirs_under_their_one_run_guard() {
    let t = Temp::new();
    let (id, _) = cut_short_by_quit(&t);
    let _g = hooks::install(t.store(), fake("cards-streaming"));
    let work = t.0.join("work");
    fs::write(work.join("stream-hold.gate"), "").unwrap();
    let (next, _, _) = carry(&id, "codex", None).unwrap();
    let path = path_of(&t, &next);
    wait("the resumed turn to start", || path.exists() && started(&path));
    assert_eq!(crate::people::owner_of(&t.store(), &next).as_deref(), Some("tm_codec"), "theirs");
    let cwd = records(&path)[0]["metadata"]["command"]["cwd"].as_str().map(PathBuf::from).expect("the folder it runs in");
    assert_eq!(fs::canonicalize(cwd).unwrap(), fs::canonicalize(&work).unwrap(), "in their own folder");
    assert!(
        crate::invariants::lock(&ACTIVE_TEAMMATES).iter().any(|(teammate, mission, _)| teammate == "tm_codec" && *mission == next),
        "counted as their one run while it runs"
    );
    fs::write(work.join("stream-release.gate"), "").unwrap();
    finished(&next);
}

/// Beyond Electron's cases, its rule (codex-mission.ts `resume`, AGENTS.md): a resume never sends the cut-short prompt
/// again as if nothing had happened. The runtime is handed the brief from the checkpoint, which names the action that
/// never reported back for it to verify.
#[test]
fn a_resume_anyway_is_told_what_to_verify_and_never_sent_the_cut_short_prompt_bare() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("cards-share"));
    codec_team(&t);
    let id = format!("mission_native_{}_{}", std::process::id(), crate::invariants::unique_nanos());
    let lines = [
        json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":"2026-11-26T00:00:00.000Z","metadata":{"missionId":id,"runId":format!("run_{id}"),"prompt":"Publish the package","runtime":"codex","model":"synthetic-model","mode":"ask","sandbox":"read-only","resolvedRouteId":"codex:synthetic-model-account:default","createdAt":"2026-11-26T00:00:00.000Z"}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":2,"occurredAt":"2026-11-26T00:00:01.000Z","event":{"id":"e1","sequence":1,"occurredAt":"2026-11-26T00:00:01.000Z","type":"run.started","payload":{"runtimeThreadId":"thread_fake"}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":3,"occurredAt":"2026-11-26T00:00:02.000Z","event":{"id":"e2","sequence":2,"occurredAt":"2026-11-26T00:00:02.000Z","type":"tool.started","payload":{"itemId":"cmd1","toolKind":"command","name":"npm publish"}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":4,"occurredAt":"2026-11-26T00:00:03.000Z","event":{"id":"e3","sequence":3,"occurredAt":"2026-11-26T00:00:03.000Z","type":"run.cancelled","payload":{"runtimeThreadId":"thread_fake"}}}),
    ];
    let path = path_of(&t, &id);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    crate::people::record_owner(&t.store(), &id, "tm_codec").unwrap();
    // The quit's checkpoint, as the quit writes it.
    checkpoint_cut_short(std::slice::from_ref(&id));
    match ledger::read_transcript(&path).recovery {
        Some(crate::recovery::Offer::Resume { unverified, .. }) => assert_eq!(unverified, ["npm publish"]),
        other => panic!("the card offers Resume anyway: {other:?}"),
    }

    let (next, _, _) = carry(&id, "codex", None).unwrap();
    finished(&next);
    let sent = fs::read_to_string(t.0.join("last-prompt.txt")).unwrap();
    let recorded = records(&path_of(&t, &next))[0]["metadata"]["prompt"].as_str().unwrap().to_owned();
    for told in [&sent, &recorded] {
        assert_ne!(told.trim(), "Publish the package", "not the cut-short prompt again");
        assert!(told.contains("Publish the package"), "the task, inside the brief: {told}");
        assert!(told.contains("npm publish"), "the action to verify: {told}");
        assert!(told.contains("Codex CLI"), "who it continues from: {told}");
    }
    assert!(sent.contains(&recorded), "the runtime was handed the recorded brief");
}

/// Where the card says it cannot be resumed (recovery.ts: a turn with no ending that kept no checkpoint), or offers
/// nothing (the person pressed Stop), pressing on the same agent starts nothing and writes nothing: the carry is refused
/// as before.
#[test]
fn a_turn_the_card_does_not_offer_to_resume_is_not_resumed() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("cards-share"));
    codec_team(&t);
    let write = |ended: bool| {
        let id = format!("mission_native_{}_{}", std::process::id(), crate::invariants::unique_nanos());
        let mut lines = vec![
            json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":"2026-11-26T00:00:00.000Z","metadata":{"missionId":id,"runId":format!("run_{id}"),"prompt":"Synthetic prompt","runtime":"codex","model":"synthetic-model","mode":"ask","createdAt":"2026-11-26T00:00:00.000Z"}}),
            json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":2,"occurredAt":"2026-11-26T00:00:01.000Z","event":{"id":"e1","sequence":1,"occurredAt":"2026-11-26T00:00:01.000Z","type":"run.started","payload":{}}}),
        ];
        if ended {
            lines.push(json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":3,"occurredAt":"2026-11-26T00:00:02.000Z","event":{"id":"e2","sequence":2,"occurredAt":"2026-11-26T00:00:02.000Z","type":"run.cancelled","payload":{}}}));
        }
        let path = path_of(&t, &id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        crate::people::record_owner(&t.store(), &id, "tm_codec").unwrap();
        (id, path)
    };
    // Closed under with no ending and no checkpoint: the card says it cannot be resumed.
    let (unkept, unkept_path) = write(false);
    let shown = ledger::read_transcript(&unkept_path).recovery;
    assert!(matches!(shown, Some(crate::recovery::Offer::Refused(_))), "{shown:?}");
    // Stopped by the person: the card offers nothing.
    let (stopped, stopped_path) = write(true);
    assert_eq!(ledger::read_transcript(&stopped_path).recovery, None);
    for (id, path) in [(unkept, unkept_path), (stopped, stopped_path)] {
        let before = fs::read(&path).unwrap();
        assert!(carry(&id, "codex", None).is_err());
        assert_eq!(fs::read(&path).unwrap(), before, "its record is left as it was");
        assert!(continuing(&t, &id).is_empty(), "nothing started");
    }
}

/// Lane check (quit-1): native writes only its own ledgers; Electron's are read. A turn the quit cut short whose ledger
/// is in Electron's folder is never given a checkpoint there, and its file is left byte for byte as it was.
#[test]
fn a_turn_whose_ledger_is_electrons_gets_no_checkpoint_and_its_file_is_untouched() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let id = format!("mission_electron_{}_{}", std::process::id(), crate::invariants::unique_nanos());
    let lines = [
        json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":"2026-11-26T00:00:00.000Z","metadata":{"missionId":id,"runId":format!("run_{id}"),"prompt":"Electron's turn","runtime":"codex","model":"synthetic-model","createdAt":"2026-11-26T00:00:00.000Z"}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":2,"occurredAt":"2026-11-26T00:00:01.000Z","event":{"id":"e1","sequence":1,"occurredAt":"2026-11-26T00:00:01.000Z","type":"run.started","payload":{"runtimeThreadId":"thread_e"}}}),
    ];
    let dir = electron_ledger_dir().expect("the test store names Electron's folder");
    assert!(!t.store().ledger_dir().is_ok_and(|native| dir.starts_with(&native)), "Electron's folder is not native's");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{id}.jsonl"));
    fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(find_ledger(&id).as_deref(), Some(path.as_path()), "the turn is found in Electron's folder");

    let refused = checkpoint_on_quit(&id).expect_err("no checkpoint is written into Electron's ledger");
    assert_eq!(refused, "its ledger is not native's own");
    checkpoint_cut_short(&[id.clone()]);

    assert_eq!(fs::read(&path).unwrap(), before, "Electron's ledger is left byte for byte as it was");
    assert!(checkpoints_in(&path).is_empty());
}
