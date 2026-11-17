//! Electron's `main/codex-mission.test.ts`, carried a `describe` group at a time, each case a test with Electron's
//! values and words. The fake app-server (`scripts/fixtures/m1-app-server.py`) asserts the sandbox each scenario must
//! be started with, so a turn that completes proves what `thread/start` was sent.
use super::*;

/// The turn's durable header: `mission.created`'s metadata.
fn header(path: &Path) -> Value {
    records(path).into_iter().find(|r| r["recordType"] == "mission.created").map(|r| r["metadata"].clone()).unwrap_or(Value::Null)
}

/// Waits for the turn to end and gives its event kinds.
fn ended(t: &Temp, id: &str) -> Vec<String> {
    finished(id);
    kinds(&path_of(t, id))
}

fn wren_and_its_limit(t: &Temp) {
    t.store()
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "monthlyLimitUsd": 5}],
                "missionOwners": {"mission_spent": "tm_codec"}, "settings": {}}),
        )
        .unwrap();
}

/// A finished run of this month that cost $5.02, in Electron's ledger.
fn spent_this_month(t: &Temp) {
    let now = chrono::Local::now().to_rfc3339();
    let records = [
        json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {"missionId": "mission_spent", "runtime": "codex", "model": "gpt-6", "createdAt": now}}),
        json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"type": "run.completed", "occurredAt": now, "sequence": 1,
            "payload": {"usage": {"usd": 5.02, "inputTokens": 10, "outputTokens": 2}}}}),
    ];
    fs::write(t.electron().join("mission-ledger/mission_spent.jsonl"), records.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
}

fn juno(t: &Temp) -> Send {
    Send { teammate: Teammate { id: "tm_juno".into(), name: "Juno".into(), role: "Reviewer".into(), ..codex(t) }, ..job(t, None, "Synthetic prompt") }
}

// ---- mission sandbox ----

#[test]
fn a_teammate_whose_own_branch_could_not_be_made_is_refused_whoever_started_it() {
    let t = Temp::new();
    let store = t.store();
    store
        .save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "worktree": true}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let _hook = hooks::install(store, fake("success"));
    let said = "Codec is set to work on its own branch, but the project folder is not a git repository.";
    for started_by in [None, Some(json!({"kind": "room", "roomId": "room_1"})), Some(json!({"kind": "mcp"})), Some(json!({"kind": "routine", "routineId": "routine_1"}))] {
        let origin = Origin { started_by: started_by.clone(), ..Default::default() };
        assert_eq!(send_from(job(&t, None, "Fix the typo."), origin).unwrap_err(), said, "{started_by:?}");
    }
    assert!(ledgers(&t).is_empty(), "nothing was started");
}

#[test]
fn a_teammate_at_the_monthly_limit_is_refused_whoever_started_it_before_anything_is_recorded() {
    let t = Temp::new();
    wren_and_its_limit(&t);
    spent_this_month(&t);
    let _hook = hooks::install(t.store(), fake("success"));
    let said = format!(
        "Codec has reached this month's limit: $5.02 of $5.00. Raise the limit by editing Codec, or it starts again on {}.",
        crate::spend::next_month_starts(chrono::Local::now())
    );
    let mut mine = job(&t, None, "Fix the typo.");
    mine.teammate.runs_in = "accept-edits".into();
    assert_eq!(send(mine).unwrap_err(), said);
    let from_another_app = Origin { mode: Some(Mode::Ask), started_by: Some(json!({"kind": "mcp"})), ..Default::default() };
    assert_eq!(send_from(job(&t, None, "Fix the typo."), from_another_app).unwrap_err(), said);
    assert!(ledgers(&t).is_empty(), "nothing was recorded");
    // Another teammate starts as before.
    let started = send(juno(&t)).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()));
}

#[test]
fn a_run_starts_when_its_teammates_limit_cannot_be_checked() {
    let t = Temp::new();
    wren_and_its_limit(&t);
    // Electron's ledger cannot be listed: it is a file where the folder should be.
    fs::remove_dir_all(t.electron().join("mission-ledger")).unwrap();
    fs::write(t.electron().join("mission-ledger"), "not a folder").unwrap();
    assert!(crate::usage_ledger::usage_turns(&t.store()).is_err(), "the spending record is unreadable");
    let _hook = hooks::install(t.store(), fake("success"));
    let started = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()));
}

#[test]
fn an_ask_run_is_started_read_only_and_recorded_so() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let started = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only read-only");
    assert_eq!(header(&path_of(&t, &started.conversation_id))["sandbox"], "read-only");
}

#[test]
fn an_ask_run_another_app_started_is_recorded_with_its_read_only_sandbox_and_who_started_it() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let origin = Origin { mode: Some(Mode::Ask), started_by: Some(json!({"kind": "mcp"})), ..Default::default() };
    let started = send_from(job(&t, None, "Synthetic prompt"), origin).unwrap();
    assert_eq!(started.mode, "ask");
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()));
    let head = header(&path_of(&t, &started.conversation_id));
    assert_eq!((&head["sandbox"], &head["mode"], &head["startedBy"]), (&json!("read-only"), &json!("ask"), &json!({"kind": "mcp"})));
}

#[test]
fn a_run_writes_in_its_folder_only_when_edits_were_accepted_and_records_that_sandbox() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("cards-edit-check"));
    let mut mine = job(&t, None, "Write the notes. Then stop.");
    mine.teammate.runs_in = "accept-edits".into();
    let started = send(mine).unwrap();
    assert_eq!(started.mode, "accept-edits");
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only workspace-write");
    assert_eq!(header(&path_of(&t, &started.conversation_id))["sandbox"], "workspace-write");
}

#[test]
fn a_mode_it_does_not_recognise_never_widens_the_sandbox() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let mut mine = job(&t, None, "Synthetic prompt");
    mine.teammate.runs_in = "whatever".into();
    let started = send(mine).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only read-only");
    assert_eq!(header(&path_of(&t, &started.conversation_id))["sandbox"], "read-only");
}

// ---- model selection ----

#[test]
fn a_chosen_model_is_sent_to_the_runtime_and_recorded() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let started = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only synthetic-model");
    assert_eq!(header(&path_of(&t, &started.conversation_id))["model"], "synthetic-model");
}

#[test]
fn account_default_is_sending_no_model_and_is_recorded_as_account_default() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("no-model"));
    let mut mine = job(&t, None, "Synthetic prompt");
    mine.model = Some("account-default".into());
    let started = send(mine).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only no model");
    assert_eq!(header(&path_of(&t, &started.conversation_id))["model"], "account-default");
}

#[test]
fn no_model_is_sent_when_none_was_chosen() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("no-model"));
    let mut mine = job(&t, None, "Synthetic prompt");
    mine.model = None;
    mine.teammate.model = None;
    let started = send(mine).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only no model");
}

// ---- a model of your own (0.357) ----

#[test]
fn a_model_of_your_own_that_was_removed_is_refused_in_words_before_anything_is_written() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let mut mine = job(&t, None, "Say hello.");
    mine.teammate.runtime = Some("opencode".into());
    mine.teammate.runs_in = "accept-edits".into();
    mine.model = Some("own-a1b2c3d4/acme-70b".into());
    let refused = send(mine).unwrap_err();
    assert!(refused.contains("no longer in Your own models"), "{refused}");
    assert!(ledgers(&t).is_empty(), "nothing was written");
}
