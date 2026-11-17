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

// ---- Codex mission service: what a start refuses, and the mode it records ----

fn auto_allowed(t: &Temp, allowed: bool) {
    let settings = crate::workspace_settings::WorkspaceSettings { auto_mode: allowed, ..Default::default() };
    crate::workspace_settings::save(&t.store(), &settings).unwrap();
}

fn in_mode(t: &Temp, mode: &str, prompt: &str) -> Send {
    let mut mine = job(t, None, prompt);
    mine.teammate.runs_in = mode.into();
    mine
}

#[test]
fn an_invalid_prompt_is_refused_before_anything_is_started() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    for prompt in ["   ".to_owned(), "x".repeat(8_001), "a\0b".to_owned()] {
        assert_eq!(send(job(&t, None, &prompt)).unwrap_err(), "Enter a mission between 1 and 8,000 characters.");
    }
    assert!(ledgers(&t).is_empty(), "nothing was started");
    // At the limit is a prompt.
    assert!(valid_prompt(&"x".repeat(8_000)));
}

#[test]
fn a_plan_is_recorded_as_a_plan_and_held_read_only() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let started = send(in_mode(&t, "plan", "Synthetic prompt")).unwrap();
    assert_eq!(started.mode, "plan");
    let head = header(&path_of(&t, &started.conversation_id));
    assert_eq!((&head["mode"], &head["sandbox"]), (&json!("plan"), &json!("read-only")));
    finished(&started.conversation_id);
}

#[test]
fn an_auto_run_is_refused_when_the_workspace_has_auto_switched_off_and_nothing_is_recorded() {
    let t = Temp::new();
    auto_allowed(&t, false);
    let _hook = hooks::install(t.store(), fake("auto"));
    assert_eq!(
        send(in_mode(&t, "auto", "Tidy my whole machine.")).unwrap_err(),
        "Auto mode is switched off for this workspace. Turn it on in Settings to let a run work outside the workspace folder. Nothing was recorded."
    );
    assert!(ledgers(&t).is_empty());
}

#[test]
fn an_auto_run_is_refused_when_the_switch_was_never_set() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("auto"));
    assert_eq!(send(in_mode(&t, "auto", "Tidy my whole machine.")).unwrap_err(), AUTO_OFF_REFUSAL);
    assert!(ledgers(&t).is_empty());
}

#[test]
fn an_auto_run_has_the_widest_sandbox_once_allowed_and_the_next_is_refused_once_the_switch_is_taken_back() {
    let t = Temp::new();
    auto_allowed(&t, true);
    let _hook = hooks::install(t.store(), fake("auto"));
    // The fake answers only its own prompt, so the first run says that rather than Electron's words.
    let first = send(in_mode(&t, "auto", "Synthetic prompt")).unwrap();
    let path = path_of(&t, &first.conversation_id);
    assert!(ended(&t, &first.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only danger-full-access: {:?}", records(&path));
    assert_eq!(header(&path_of(&t, &first.conversation_id))["sandbox"], "full-access");
    auto_allowed(&t, false);
    assert_eq!(send(in_mode(&t, "auto", "Second, after taking it back.")).unwrap_err(), AUTO_OFF_REFUSAL);
    assert_eq!(ledgers(&t).len(), 1, "nothing more was launched");
}

// ---- continuing a conversation ----

const NOW: &str = "2026-11-16T12:00:00.000Z";

/// Electron's `finished()`: an earlier turn of the conversation as the record holds it, with `overrides` laid over its
/// metadata and the given events. Written to native's ledger folder (where a route switch checkpoints it) and aged an
/// hour, so a turn whose events end without a terminal record still reads as finished.
fn earlier(t: &Temp, overrides: Value, events: &[Value]) -> PathBuf {
    let mut meta = json!({"missionId": "mission_prior", "runId": "run_prior", "prompt": "check the google stock price",
        "runtime": "codex", "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
        "cliVersion": null, "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": NOW});
    for (k, v) in overrides.as_object().unwrap() {
        meta[k] = v.clone();
    }
    let id = meta["missionId"].as_str().unwrap().to_owned();
    let mut lines = vec![json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": NOW, "metadata": meta})];
    for (i, event) in events.iter().enumerate() {
        let mut event = event.clone();
        event["sequence"] = json!(i + 1);
        event["id"] = json!(format!("e{}", i + 1));
        event["occurredAt"] = json!(NOW);
        lines.push(json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": i + 2, "occurredAt": NOW, "event": event}));
    }
    let path = t.store().write_path(&id).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    fs::File::options().write(true).open(&path).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(3600)).unwrap();
    path
}

/// The run.started the earlier turn recorded, naming its session.
fn session_named(thread: &str) -> Value {
    json!({"type": "run.started", "payload": {"runtimeThreadId": thread, "evidence": {"redacted": true}}})
}

/// The recording fake app-server, and a teammate in `mode` replying in the conversation.
fn reply(t: &Temp, mode: &str, words: &str) -> Started {
    let mut mine = job(t, Some("mission_prior"), words);
    mine.teammate.runs_in = mode.into();
    let started = send(mine).unwrap();
    finished(&started.conversation_id);
    started
}

/// The turn this test started: the newest ledger that is not one of the earlier turns.
fn new_turn(t: &Temp, earlier: &[&PathBuf]) -> Value {
    let path = ledgers(t).into_iter().find(|p| !earlier.contains(&p)).expect("a turn was started");
    header(&path)
}

/// What the runtime was told on its turn, without the reference-folders line every turn ends with.
fn told(t: &Temp) -> String {
    let asked = codex_requests(t);
    let turn = asked.iter().find(|r| r["method"] == "turn/start").expect("a turn was sent");
    let text = turn["params"]["input"][0]["text"].as_str().unwrap();
    text.split("\n\nReference folders — for reading, not changing:").next().unwrap().to_owned()
}

fn thread_request(t: &Temp) -> Value {
    codex_requests(t).into_iter().find(|r| r["method"] == "thread/start" || r["method"] == "thread/resume").expect("a thread was opened")
}

#[test]
fn a_reply_resumes_the_earlier_turns_own_session_and_records_what_it_continued() {
    let t = Temp::new();
    let prior = earlier(&t, json!({}), &[session_named("thread-prior")]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "cant you look it up for me?");
    let opened = thread_request(&t);
    assert_eq!((opened["method"].as_str(), opened["params"]["threadId"].as_str()), (Some("thread/resume"), Some("thread-prior")));
    assert_eq!(
        new_turn(&t, &[&prior])["continuesFrom"],
        json!({"missionId": "mission_prior", "checkpointEpoch": 1, "reason": "follow-up", "runtimeThreadId": "thread-prior"})
    );
}

#[test]
fn a_reply_in_another_mode_starts_cold_so_the_run_is_the_mode_the_chip_shows() {
    let t = Temp::new();
    let prior = earlier(&t, json!({"mode": "ask"}), &[session_named("thread-prior")]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "accept-edits", "now actually write it");
    let opened = thread_request(&t);
    assert_eq!(opened["method"], "thread/start", "that session cannot be given write tools");
    assert!(!serde_json::to_string(&codex_requests(&t)).unwrap().contains("thread-prior"));
    assert_eq!(opened["params"]["sandbox"], "workspace-write");
    assert_eq!(new_turn(&t, &[&prior])["sandbox"], "workspace-write", "the run it did start is the one the chip promised");
}

#[test]
fn a_reply_that_cannot_resume_is_told_what_was_said_before() {
    let t = Temp::new();
    let prior = earlier(&t, json!({"mode": "ask"}), &[session_named("thread-prior")]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "accept-edits", "now actually write it");
    let sent = told(&t);
    assert!(sent.contains("This conversation has earlier turns, but not in your session"), "{sent}");
    assert!(sent.contains("The person now asks:"), "{sent}");
    assert!(sent.trim_end().ends_with("now actually write it"), "{sent}");
    assert_eq!(new_turn(&t, &[&prior])["prompt"], "now actually write it", "the record keeps the person's words");
}

#[test]
fn a_reply_in_the_mode_the_conversation_started_in_still_resumes() {
    let t = Temp::new();
    earlier(&t, json!({"mode": "ask"}), &[session_named("thread-prior")]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "and what did that say?");
    let opened = thread_request(&t);
    assert_eq!((opened["method"].as_str(), opened["params"]["threadId"].as_str()), (Some("thread/resume"), Some("thread-prior")));
}

#[test]
fn a_conversation_whose_earlier_turn_recorded_no_session_is_continued_cold_rather_than_not_at_all() {
    let t = Temp::new();
    let prior = earlier(&t, json!({}), &[]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "go on");
    // The record links the turns and claims no resume that did not happen.
    assert_eq!(new_turn(&t, &[&prior])["continuesFrom"], json!({"missionId": "mission_prior", "checkpointEpoch": 1, "reason": "follow-up"}));
    // And the run starts fresh.
    let methods: Vec<String> = codex_requests(&t).iter().map(|r| r["method"].as_str().unwrap().to_owned()).collect();
    assert!(methods.contains(&"thread/start".to_owned()) && !methods.iter().any(|m| m.contains("resume")), "{methods:?}");
}

#[test]
fn a_reply_picks_up_the_session_a_turn_stopped_before_naming_its_own_was_resuming() {
    let t = Temp::new();
    let link = json!({"missionId": "mission_older", "checkpointEpoch": 1, "reason": "follow-up", "runtimeThreadId": "thread-older"});
    let prior = earlier(&t, json!({"continuesFrom": link}), &[]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "go on");
    let opened = thread_request(&t);
    assert_eq!((opened["method"].as_str(), opened["params"]["threadId"].as_str()), (Some("thread/resume"), Some("thread-older")));
    assert!(!told(&t).contains("not in your session"));
    let link = &new_turn(&t, &[&prior])["continuesFrom"];
    assert_eq!((&link["missionId"], &link["runtimeThreadId"]), (&json!("mission_prior"), &json!("thread-older")));
}

#[test]
fn an_edited_message_starts_again_cold_never_in_the_session_that_holds_the_turns_set_aside() {
    let t = Temp::new();
    let prior = earlier(&t, json!({"mode": "ask"}), &[session_named("thread-prior")]);
    let link = json!({"missionId": "mission_prior", "checkpointEpoch": 1, "reason": "follow-up", "runtimeThreadId": "thread-prior"});
    let tip = earlier(&t, json!({"missionId": "mission_old_tip", "runId": "run_old_tip", "prompt": "and in euros?", "continuesFrom": link}), &[session_named("thread-prior")]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let origin = Origin {
        rewind: Some(Rewind { tip: "mission_old_tip".into(), before: Some("mission_prior".into()), set_aside: vec!["mission_old_tip".into()] }),
        ..Default::default()
    };
    let started = send_from(job(&t, Some("mission_prior"), "check the apple stock price instead"), origin).unwrap();
    finished(&started.conversation_id);
    assert!(!serde_json::to_string(&codex_requests(&t)).unwrap().contains("thread-prior"));
    let sent = told(&t);
    assert!(sent.contains("went back to an earlier point"), "{sent}");
    assert!(sent.contains("check the google stock price"), "{sent}");
    assert!(sent.trim_end().ends_with("check the apple stock price instead"), "{sent}");
    // Marked as an edit, so the version before can be found again after a restart.
    assert_eq!(
        new_turn(&t, &[&prior, &tip])["continuesFrom"],
        json!({"missionId": "mission_prior", "checkpointEpoch": 1, "reason": "follow-up", "edited": true})
    );
}

#[test]
fn sending_again_goes_as_the_same_words_not_as_an_earlier_turn_quoted_back() {
    let t = Temp::new();
    earlier(&t, json!({}), &[]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "check the google stock price");
    let sent = told(&t);
    assert!(!sent.contains("This conversation has earlier turns"), "{sent}");
    assert!(sent.contains("check the google stock price"), "{sent}");
}

/// mission_prior (on Claude) <- mission_middle <- mission_first, and what mission_first continues from.
fn three_turns_on_claude(t: &Temp, first_continues: Value) -> Vec<PathBuf> {
    let mut first = json!({"missionId": "mission_first", "runId": "run_first", "prompt": "build a price report"});
    if !first_continues.is_null() {
        first["continuesFrom"] = first_continues;
    }
    vec![
        earlier(
            t,
            json!({"runtime": "claude", "continuesFrom": {"missionId": "mission_middle", "checkpointEpoch": 1, "reason": "follow-up"}}),
            &[session_named("thread-prior")],
        ),
        earlier(
            t,
            json!({"missionId": "mission_middle", "runId": "run_middle", "prompt": "then make it weekly", "continuesFrom": {"missionId": "mission_first", "checkpointEpoch": 1, "reason": "follow-up"}}),
            &[],
        ),
        earlier(t, first, &[]),
    ]
}

const EARLIER_TWO: &str = "Earlier in this conversation, oldest first:\n- Asked: \"build a price report\" -- no reply was recorded.\n- Asked: \"then make it weekly\" -- no reply was recorded.";

#[test]
fn a_switch_carries_the_conversations_earlier_turns_oldest_first() {
    let t = Temp::new();
    three_turns_on_claude(&t, Value::Null);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "now in euros");
    let briefed = told(&t);
    assert!(briefed.contains(EARLIER_TWO), "{briefed}");
}

/// Electron's case runs the switch over a turn that continues itself. Native's start refuses that history before any
/// brief, in the words of Electron's `conversation-references.ts` `rootOf` (a fake service in Electron's test wires no
/// references), so the walk that stops at the loop is tested on its own.
#[test]
fn the_earlier_turns_walk_stops_at_a_turn_that_continues_itself() {
    let t = Temp::new();
    three_turns_on_claude(&t, json!({"missionId": "mission_first", "checkpointEpoch": 1, "reason": "follow-up"}));
    let _hook = hooks::install(t.store(), None);
    let prior = turn_records("mission_prior").unwrap();
    let asked: Vec<String> = earlier_turns(&prior.metadata).into_iter().map(|turn| turn.asked).collect();
    assert_eq!(asked, ["build a price report", "then make it weekly"]);
    assert_eq!(send(job(&t, Some("mission_prior"), "now in euros")).unwrap_err(), "This conversation has an invalid history. Its references were not changed.");
}

#[test]
fn another_runtimes_conversation_is_continued_from_a_checkpoint_with_the_reply_as_the_latest_word() {
    let t = Temp::new();
    let answer = json!({"type": "message.delta", "payload": {"itemId": "answer", "operation": "replace", "text": "The price was 181.", "final": true}});
    let prior = earlier(&t, json!({"runtime": "claude"}), &[session_named("thread-prior"), answer]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "now in euros");
    let checkpoints: Vec<Value> = records(&prior).into_iter().filter(|r| r["recordType"] == "mission.checkpoint").collect();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!((&checkpoints[0]["checkpoint"]["missionId"], &checkpoints[0]["checkpoint"]["reason"]), (&json!("mission_prior"), &json!("route-switch")));
    let opened: Vec<Value> = codex_requests(&t).into_iter().filter(|r| r["method"] == "turn/start").collect();
    assert_eq!(opened.len(), 1, "one run started");
    let briefed = told(&t);
    assert!(briefed.contains("another agent (Claude Code)"), "{briefed}");
    assert!(briefed.contains("check the google stock price"), "{briefed}");
    assert!(briefed.contains("The price was 181."), "{briefed}");
    assert!(briefed.ends_with("The person now asks:\n\nnow in euros"), "{briefed}");
    // Recorded the way the live handoff records. Native's first checkpoint of the turn is epoch 1 (Electron's fake said 2).
    assert_eq!(new_turn(&t, &[&prior])["continuesFrom"], json!({"missionId": "mission_prior", "checkpointEpoch": 1, "reason": "route-switch"}));
    // No resume: the other runtime's session is not this one's.
    assert_eq!(thread_request(&t)["method"], "thread/start");
}

#[test]
fn an_ordinary_reply_on_the_same_runtime_is_no_switch() {
    let t = Temp::new();
    let prior = earlier(&t, json!({}), &[session_named("thread-prior")]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "now in euros");
    assert!(records(&prior).iter().all(|r| r["recordType"] != "mission.checkpoint"), "nothing was reconciled");
    assert_eq!(new_turn(&t, &[&prior])["continuesFrom"]["reason"], "follow-up");
}

#[test]
fn a_continuation_on_another_runtime_is_refused_when_the_record_cannot_be_trusted() {
    let t = Temp::new();
    let prior = earlier(&t, json!({"runtime": "claude"}), &[session_named("thread-prior")]);
    // The ledger's tail is damaged: its last line is half a record.
    let mut text = fs::read_to_string(&prior).unwrap();
    text.push_str("{\"schemaVersion\":27,\"recordType\":\"mission.ev\n");
    fs::write(&prior, text).unwrap();
    fs::File::options().write(true).open(&prior).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(3600)).unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let refused = send(job(&t, Some("mission_prior"), "go on")).unwrap_err();
    assert!(refused.contains("cannot be continued safely") && refused.contains("stopped short of its end"), "{refused}");
    assert!(codex_requests(&t).is_empty(), "nothing was started");
    assert_eq!(ledgers(&t), vec![prior]);
}

#[test]
fn a_first_question_starts_a_fresh_conversation() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = send(job(&t, None, "a first question")).unwrap();
    finished(&started.conversation_id);
    assert_eq!(thread_request(&t)["method"], "thread/start");
    assert!(new_turn(&t, &[]).get("continuesFrom").is_none());
}

/// The other side of Send again: the same words after a turn that WAS answered are a new question, and the cold start
/// is told the answered one (codex-mission.ts `again` needs the latest reply empty).
#[test]
fn the_same_words_after_an_answered_turn_are_told_that_turn() {
    let t = Temp::new();
    let answer = json!({"type": "message.delta", "payload": {"itemId": "answer", "operation": "replace", "text": "The price was 181.", "final": true}});
    earlier(&t, json!({}), &[answer]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    reply(&t, "ask", "check the google stock price");
    let sent = told(&t);
    assert!(sent.contains("This conversation has earlier turns, but not in your session"), "{sent}");
    assert!(sent.contains("answered: \"The price was 181.\""), "{sent}");
}
