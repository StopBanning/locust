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

/// The same rule on Claude Code: a reply to a Claude turn that recorded no session used to start a new one told
/// nothing. It starts cold and is told what was said.
#[test]
fn a_claude_reply_with_no_session_to_resume_is_told_what_was_said_before() {
    let t = Temp::new();
    let prior = earlier(&t, json!({"runtime": "claude", "resolvedRouteId": "claude-account:default"}), &[]);
    let fake = commands_claude(&t);
    let _hook = hooks::install(t.store(), Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let started = send(claude_job(&t, Some("mission_prior"), "go on")).unwrap();
    finished(&started.conversation_id);
    let sent = sent_log(&t, "claude-sent.jsonl");
    let told = sent[0].as_str().unwrap();
    assert!(told.contains("This conversation has earlier turns, but not in your session"), "{told}");
    assert!(told.contains("Asked: \"check the google stock price\" -- no reply was recorded."), "{told}");
    let head = new_turn(&t, &[&prior]);
    assert_eq!(head["continuesFrom"], json!({"missionId": "mission_prior", "checkpointEpoch": 1, "reason": "follow-up"}));
    assert_eq!(head["prompt"], "go on");
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
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let prior = turn_records("mission_prior").unwrap();
    let asked: Vec<String> = earlier_turns(&prior.metadata).into_iter().map(|turn| turn.asked).collect();
    assert_eq!(asked, ["build a price report", "then make it weekly"]);
    assert_eq!(send(job(&t, Some("mission_prior"), "now in euros")).unwrap_err(), "This conversation has an invalid history. Its references were not changed.");
    assert!(codex_requests(&t).is_empty(), "nothing was started");
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

// ---- the workroom around a mission ----

/// Wren and Atlas on the roster, the relay off (a share waits in the workroom rather than starting Atlas).
fn wren_and_atlas(t: &Temp) {
    let mate = |id: &str, name: &str, role: &str| json!({"teammateId": id, "name": name, "role": role, "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}});
    t.store()
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [mate("tm_wren", "Wren", "Code & Migrations"), mate("tm_atlas", "Atlas", "Research & Briefs")],
                "missionOwners": {}, "settings": {"relay": false}}),
        )
        .unwrap();
}

fn wren(t: &Temp, mode: &str, prompt: &str) -> Send {
    let mut mine = job(t, None, prompt);
    mine.teammate.id = "tm_wren".into();
    mine.teammate.name = "Wren".into();
    mine.teammate.runs_in = mode.into();
    mine
}

/// Runs Wren's turn to its end and gives the turn's own mission id.
fn wren_ran(t: &Temp, mode: &str, prompt: &str) -> String {
    let started = send(wren(t, mode, prompt)).unwrap();
    finished(&started.conversation_id);
    started.conversation_id
}

/// Everything the runtime was told on its turn, the reference-folders line and what follows it included.
fn told_whole(t: &Temp) -> String {
    let asked = codex_requests(t);
    let turn = asked.iter().rev().find(|r| r["method"] == "turn/start").expect("a turn was sent");
    turn["params"]["input"][0]["text"].as_str().unwrap().to_owned()
}

fn the_turn_says(t: &Temp, text: &str, status: &str) {
    fs::write(t.0.join("work/commands-reply.json"), json!({"text": text, "status": status}).to_string()).unwrap();
}

fn atlas() -> crate::workroom::Party {
    crate::workroom::Party { teammate_id: "tm_atlas".into(), name: "Atlas".into() }
}

fn peer_links(path: &Path) -> Vec<Value> {
    records(path).into_iter().filter(|r| r["recordType"] == "mission.peer").map(|r| r["link"].clone()).collect()
}

#[test]
fn a_waiting_message_is_quoted_as_a_claim_recorded_and_delivered_once_the_run_starts() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let workroom = crate::workroom::Workroom::for_store(&t.store());
    let wren_party = crate::workroom::Party { teammate_id: "tm_wren".into(), name: "Wren".into() };
    let waiting = workroom.post(&atlas(), "mission_a", &wren_party, "pnpm check runs everything.", None).unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let id = wren_ran(&t, "ask", "Which command runs the checks?");
    let sent = told(&t);
    assert!(sent.ends_with("Which command runs the checks?"), "{sent}");
    assert!(sent.contains("CLAIMS from other agents"), "{sent}");
    assert!(sent.contains("pnpm check runs everything."), "{sent}");
    assert!(sent.contains("<locust-share to=\"Atlas\">"), "{sent}");
    assert_eq!(
        peer_links(&path_of(&t, &id)),
        vec![json!({"direction": "received", "messageId": waiting.message_id, "peerTeammateId": "tm_atlas", "occurredAt": waiting.posted_at})]
    );
    let deliveries = workroom.read().deliveries;
    assert_eq!(deliveries.len(), 1);
    assert_eq!((deliveries[0].message_id.as_str(), deliveries[0].mission_id.as_str()), (waiting.message_id.as_str(), id.as_str()));
}

#[test]
fn a_plan_run_is_told_to_plan_last_and_only_where_the_sandbox_refuses_writes() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let planned = wren_ran(&t, "plan", "Add retries to the fetch helper.");
    assert_eq!(header(&path_of(&t, &planned))["sandbox"], "read-only");
    assert_eq!(thread_request(&t)["params"]["sandbox"], "read-only");
    let with_plan = told_whole(&t);
    assert!(with_plan.contains("Add retries to the fetch helper."));
    assert!(with_plan.ends_with(&crate::briefing::plan_section()), "{with_plan}");
    assert!(with_plan.find("Add retries to the fetch helper.").unwrap() > with_plan.find("<locust-ask>").unwrap());
    assert!(with_plan.contains("PLAN FIRST") && with_plan.contains("numbered steps"));
    // A run that may edit, and an ordinary read-only one, are not told to plan.
    wren_ran(&t, "accept-edits", "Add retries to the fetch helper.");
    assert!(!told_whole(&t).contains("PLAN FIRST"));
    wren_ran(&t, "ask", "Add retries to the fetch helper.");
    assert!(!told_whole(&t).contains("PLAN FIRST"));
}

#[test]
fn a_read_only_opencode_run_is_told_it_has_no_shell_and_no_other_run_is() {
    let t = Temp::new();
    let opencode_sent = |mode: &str| -> String {
        *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), opencode_hook("success"));
        let mut mine = opencode_job(&t, "What does LOCUST.md say?");
        mine.teammate.runs_in = mode.into();
        mine.model = Some("opencode/muse-spark-1.3-contributor-free".into());
        let started = send(mine).unwrap();
        finished(&started.conversation_id);
        let asked = sent_log(&t, "opencode-requests.jsonl");
        asked.iter().rev().find(|r| r["path"] == "prompt_async").unwrap()["body"]["parts"][0]["text"].as_str().unwrap().to_owned()
    };
    let _hook = hooks::install(t.store(), opencode_hook("success"));
    let no_shell = crate::briefing::opencode_read_only_section();
    assert!(opencode_sent("ask").contains(&no_shell));
    // Plan too, and the plan instruction still comes last.
    let planned = opencode_sent("plan");
    assert!(planned.contains(&no_shell), "{planned}");
    assert!(planned.ends_with(&crate::briefing::plan_section()), "{planned}");
    // A run that may edit has its shell, and a read-only Codex run runs read-only commands in its sandbox.
    assert!(!opencode_sent("accept-edits").contains(&no_shell));
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), codex_commands_hook(&t));
    let started = send(job(&t, None, "What does LOCUST.md say?")).unwrap();
    finished(&started.conversation_id);
    assert!(!told_whole(&t).contains(&no_shell));
}

#[test]
fn the_persons_own_words_are_the_recorded_prompt_not_the_briefing() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let id = wren_ran(&t, "ask", "Look around.");
    assert_eq!(header(&path_of(&t, &id))["prompt"], "Look around.");
    assert_ne!(told(&t), "Look around.", "the runtime was briefed");
}

#[test]
fn a_run_goes_on_without_the_workroom_when_it_cannot_be_read_and_can_still_share() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let room = t.store().root().join(crate::workroom::FOLDER);
    fs::create_dir_all(&room).unwrap();
    fs::write(room.join(crate::workroom::FILE_NAME), "channel damaged\n").unwrap();
    assert!(crate::workroom::Workroom::for_store(&t.store()).unread("tm_wren", 5, &[]).is_err(), "the workroom cannot be read");
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let id = wren_ran(&t, "ask", "Task.");
    assert_eq!(kinds(&path_of(&t, &id)).last().map(String::as_str), Some("run.completed"));
    assert!(told(&t).contains("<locust-share to=\"Atlas\">"), "the share form still goes");
}

#[test]
fn a_share_block_is_posted_to_the_named_teammate_attributed_to_this_mission_and_linked() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    the_turn_says(&t, "The gate is pnpm check.\n\n<locust-share to=\"Atlas\">\npnpm check runs build, typecheck and tests.\n</locust-share>", "completed");
    let id = wren_ran(&t, "ask", "Find the check command.");
    let workroom = crate::workroom::Workroom::for_store(&t.store());
    let posted = workroom.unread("tm_atlas", 5, &[]).unwrap().messages;
    assert_eq!(posted.len(), 1);
    let one = &posted[0];
    assert_eq!(
        (one.from.teammate_id.as_str(), one.from.name.as_str(), one.from_mission.as_str(), one.to.teammate_id.as_str(), one.to.name.as_str(), one.text.as_str()),
        ("tm_wren", "Wren", id.as_str(), "tm_atlas", "Atlas", "pnpm check runs build, typecheck and tests.")
    );
    assert_eq!(
        peer_links(&path_of(&t, &id)),
        vec![json!({"direction": "posted", "messageId": one.message_id, "peerTeammateId": "tm_atlas", "occurredAt": one.posted_at})]
    );
}

#[test]
fn a_share_to_someone_not_on_the_roster_is_refused_out_loud() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    the_turn_says(&t, "<locust-share to=\"Mallory\">\nsecret\n</locust-share>", "completed");
    let id = wren_ran(&t, "ask", "Task.");
    assert!(crate::workroom::Workroom::for_store(&t.store()).read().messages.is_empty(), "nothing was sent");
    let said: Vec<String> = relay_live_state(std::slice::from_ref(&id)).notices.into_iter().map(|n| n.message).collect();
    assert_eq!(said, ["Wren addressed a message to \"Mallory\", who is not on the roster. Nothing was sent."]);
}

#[test]
fn a_run_that_did_not_complete_shares_nothing() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    the_turn_says(&t, "<locust-share to=\"Atlas\">\nhalf a claim\n</locust-share>", "failed");
    let id = wren_ran(&t, "ask", "Task.");
    assert!(kinds(&path_of(&t, &id)).contains(&"run.failed".to_owned()), "{:?}", kinds(&path_of(&t, &id)));
    assert!(crate::workroom::Workroom::for_store(&t.store()).read().messages.is_empty());
}

/// Electron's `openCodeReadOnlySection` and `planSection`, word for word (workroom-briefing.ts).
#[test]
fn the_no_shell_and_plan_sections_are_electrons_words() {
    assert_eq!(
        crate::briefing::opencode_read_only_section(),
        "You have no shell in this mode -- not git, not cat, not ls: every shell command is refused. Use your read, grep, glob and list tools, and answer from what the files show."
    );
    assert_eq!(
        crate::briefing::plan_section(),
        "PLAN FIRST. Do not change anything in this workspace on this turn, and do not run commands that change state.\nRead what you need, then answer with the plan itself: numbered steps, in the order you would do them, each one concrete enough to check off.\nName anything you had to assume, and anything you found that changes what was asked.\nIf the work is small enough that a plan would be longer than doing it, say so in one line and give the steps anyway -- brevity is a fine plan.\nThe person reads this and decides whether to have you carry it out; you are not being asked to start."
    );
}

// ---- Codex over app-server ----

fn codex_settings(t: &Temp, chosen: Value) {
    fs::write(t.0.join("work/commands-reply.json"), chosen.to_string()).unwrap();
}

fn index_of(requests: &[Value], method: &str) -> Option<usize> {
    requests.iter().position(|r| r["method"] == method)
}

#[test]
fn codexs_plan_tool_is_switched_on_from_0_153_so_its_plan_reaches_the_panel() {
    use crate::runtime::codex::app_server_args;
    assert_eq!(app_server_args(Some("0.153.0")), ["app-server", "-c", "tools.update_plan.enabled=true"]);
    // The older runtime Electron's other tests run keeps plain `app-server`, as does one that did not say its version.
    assert_eq!(app_server_args(Some("0.151.0-alpha")), ["app-server"]);
    assert_eq!(app_server_args(None), ["app-server"]);
}

#[test]
fn codex_runs_as_a_server_its_pieces_become_message_events_and_the_mode_earns_its_policy() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    codex_settings(&t, json!({"deltas": ["Two ", "files."], "text": "Two files."}));
    let mut mine = job(&t, None, "How many files?");
    mine.teammate.runs_in = "accept-edits".into();
    let started = send(mine).unwrap();
    finished(&started.conversation_id);
    let path = path_of(&t, &started.conversation_id);
    let events = kinds(&path);
    assert!(events.iter().filter(|k| *k == "message.delta").count() > 1, "{events:?}");
    assert_eq!(events.last().map(String::as_str), Some("run.completed"));
    assert!(fs::read_to_string(&path).unwrap().contains("Two "));
    let opened = thread_request(&t);
    assert_eq!(opened["method"], "thread/start");
    assert_eq!(
        (&opened["params"]["cwd"], &opened["params"]["sandbox"], &opened["params"]["approvalPolicy"]),
        (&json!(t.0.join("work").to_string_lossy()), &json!("workspace-write"), &json!("never"))
    );
}

#[test]
fn a_read_only_mode_stays_read_only_and_never_asks_for_full_access() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = send(job(&t, None, "Just look.")).unwrap();
    finished(&started.conversation_id);
    assert_eq!(thread_request(&t)["params"]["sandbox"], "read-only");
    assert!(!serde_json::to_string(&codex_requests(&t)).unwrap().contains("danger-full-access"));
}

#[test]
fn ask_and_plan_start_none_of_the_persons_codex_programs_and_edit_leaves_codex_as_it_is() {
    let settings = json!({"mcp_servers": {"node_repl": {"command": "node", "args": ["repl.mjs"]}, "docs": {"url": "https://docs.example.com/mcp"}}});
    for mode in ["ask", "plan"] {
        let t = Temp::new();
        let _hook = hooks::install(t.store(), codex_commands_hook(&t));
        codex_settings(&t, json!({"config": settings}));
        let mut mine = job(&t, None, "Just look.");
        mine.teammate.runs_in = mode.into();
        finished(&send(mine).unwrap().conversation_id);
        let asked = codex_requests(&t);
        // Read first, which starts nothing; then the thread, with the programs off and the one at an address kept.
        assert!(index_of(&asked, "config/read").unwrap() < index_of(&asked, "thread/start").unwrap(), "{mode}");
        let config = &thread_request(&t)["params"]["config"];
        assert_eq!((&config["features.plugins"], &config["mcp_servers.node_repl.enabled"]), (&json!(false), &json!(false)), "{mode}");
        assert!(config.get("mcp_servers.docs.enabled").is_none(), "{mode}");
    }
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    codex_settings(&t, json!({"config": settings}));
    let mut mine = job(&t, None, "Change it.");
    mine.teammate.runs_in = "accept-edits".into();
    finished(&send(mine).unwrap().conversation_id);
    let asked = codex_requests(&t);
    assert_eq!(index_of(&asked, "config/read"), None, "its settings are not asked for");
    assert!(!serde_json::to_string(&asked).unwrap().contains("features.plugins"));
}

#[test]
fn a_sealed_look_starts_with_no_shell_web_search_apps_or_servers_connectors_included() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    codex_settings(&t, json!({"config": {"mcp_servers": {"node_repl": {"command": "node"}, "gmail": {"url": "https://mail.example.com/mcp"}}}}));
    let origin = Origin { slot: Some(SlotOrigin { key: "scout:tm_scout:codex".into(), sealed: true, ..Default::default() }), ..Default::default() };
    let started = send_from(job(&t, None, "Scout found: PR #7."), origin).unwrap();
    finished(&started.conversation_id);
    let config = &thread_request(&t)["params"]["config"];
    for (key, value) in [
        ("features.apps", json!(false)),
        ("features.shell_tool", json!(false)),
        ("web_search", json!("disabled")),
        ("features.plugins", json!(false)),
        ("mcp_servers.node_repl.enabled", json!(false)),
        ("mcp_servers.gmail.enabled", json!(false)),
    ] {
        assert_eq!(config[key], value, "{key}: {config}");
    }
}

#[test]
fn a_codex_thread_gets_locusts_command_guard_for_its_folder_and_none_when_codex_would_load_another_hook() {
    let t = Temp::new();
    let home = t.0.join("codex-home");
    fs::create_dir_all(&home).unwrap();
    let folder = t.0.join("work");
    let guard = crate::command_guard::codex_config_for_home(&folder, Some(&home)).expect("no other hook: the guard goes");
    assert_eq!(guard["bypass_hook_trust"], true);
    let hook = &guard["hooks"]["PreToolUse"][0];
    assert_eq!((&hook["matcher"], &hook["hooks"][0]["type"], &hook["hooks"][0]["timeout"]), (&json!("Bash"), &json!("command"), &json!(15)));
    assert!(hook["hooks"][0]["command"].as_str().unwrap().ends_with(" --command-guard"));
    // Another hook Codex would load: no guard, and so no bypass either.
    fs::write(home.join("hooks.json"), r#"{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "theirs"}]}]}}"#).unwrap();
    assert_eq!(crate::command_guard::codex_config_for_home(&folder, Some(&home)), None);
}

#[test]
fn every_other_runtime_keeps_the_transport_it_had() {
    let t = Temp::new();
    let fake = commands_claude(&t);
    let _hook = hooks::install(t.store(), Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let started = send(claude_job(&t, None, "Read this.")).unwrap();
    finished(&started.conversation_id);
    assert_eq!(sent_log(&t, "claude-sent.jsonl").len(), 1, "Claude Code was sent the turn");
    assert!(codex_requests(&t).is_empty(), "no app-server was started");
}

// ---- runtime selection ----
//
// Under the test hook the runtime's own command is the fake's, so a Cursor or Copilot argv is read from its builder
// (`cursor::print_args`, `copilot::prompt_args`). What a turn adds to the command after that (Claude Code's whole
// argv, Copilot's guard plugin) reaches the fake, which writes its argv down.

/// A runtime that writes down the argv it was started with (`<name>-argv.jsonl` in the work folder), reads what it is
/// given, and prints `records`.
fn argv_fake(t: &Temp, name: &str, records: &[Value], read_line: bool) -> Option<(PathBuf, Vec<String>)> {
    let fake = t.0.join(format!("{name}_writes_its_argv.py"));
    let lines: Vec<String> = records.iter().map(|r| r.to_string()).collect();
    fs::write(
        &fake,
        format!(
            r#"import json, sys
with open("{name}-argv.jsonl", "a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
if {read}:
    sys.stdin.readline()
else:
    try:
        sys.stdin.read()
    except Exception:
        pass
for line in {lines:?}:
    print(line, flush=True)
if {read}:
    try:
        sys.stdin.read()
    except Exception:
        pass
"#,
            read = if read_line { "True" } else { "False" },
        ),
    )
    .unwrap();
    Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()]))
}

fn argv_of(t: &Temp, name: &str) -> Vec<String> {
    let logged = sent_log(t, &format!("{name}-argv.jsonl"));
    serde_json::from_value(logged.last().expect("the runtime was started").clone()).unwrap()
}

/// Claude Code's three records for a turn that answers "OK".
fn claude_answers() -> Vec<Value> {
    vec![
        json!({"type": "system", "subtype": "init", "session_id": "33b21513-a4fb-478d-ad84-21cb7ef30571", "cwd": "/tmp", "tools": ["Read"], "mcp_servers": [], "model": "synthetic-model", "permissionMode": "default", "slash_commands": [], "claude_code_version": "2.1.252", "agents": [], "skills": []}),
        json!({"type": "assistant", "message": {"id": "msg_c", "content": [{"type": "text", "text": "OK"}]}}),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": "OK"}),
    ]
}

fn claude_in(t: &Temp, mode: &str, prompt: &str) -> (Started, Vec<String>, Value) {
    let _hook = hooks::install(t.store(), argv_fake(t, "claude", &claude_answers(), true));
    let mut mine = claude_job(t, None, prompt);
    mine.teammate.runs_in = mode.into();
    let started = send(mine).unwrap();
    finished(&started.conversation_id);
    let head = header(&path_of(t, &started.conversation_id));
    (started, argv_of(t, "claude"), head)
}

#[test]
fn claude_is_launched_with_its_own_restricted_argv_when_claude_is_chosen() {
    let t = Temp::new();
    let (_, args, head) = claude_in(&t, "ask", "Summarize this repo.");
    assert_eq!(head["runtime"], "claude");
    for flag in ["--print", "--output-format", "stream-json"] {
        assert!(args.iter().any(|a| a == flag), "{flag}: {args:?}");
    }
    assert!(args.windows(2).any(|w| w == ["--output-format", "stream-json"]), "{args:?}");
    // Claude's command takes no sandbox flag; the Codex one must not leak in.
    assert!(!args.iter().any(|a| a == "--sandbox"), "{args:?}");
}

#[test]
fn a_claude_run_that_may_edit_is_recorded_as_one_that_may_edit() {
    let t = Temp::new();
    let (started, _, head) = claude_in(&t, "accept-edits", "Edit something.");
    assert_eq!(started.mode, "accept-edits");
    assert_eq!((&head["runtime"], &head["sandbox"]), (&json!("claude"), &json!("workspace-write")));
}

#[test]
fn claude_is_sent_the_mode_it_was_started_in_not_a_default() {
    let t = Temp::new();
    let (_, args, _) = claude_in(&t, "accept-edits", "Edit something.");
    let line = args.join(" ");
    assert!(line.contains("--permission-mode acceptEdits"), "{line}");
    assert!(line.contains("Edit,Write,NotebookEdit,Bash"), "{line}");
}

#[test]
fn a_claude_run_asked_to_read_is_still_recorded_read_only() {
    let t = Temp::new();
    let (_, _, head) = claude_in(&t, "ask", "Read something.");
    assert_eq!(head["sandbox"], "read-only");
}

#[test]
fn a_runtime_whose_event_stream_native_cannot_read_is_refused_before_anything_is_started_or_recorded() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), argv_fake(&t, "gemini", &[], false));
    let mut mine = job(&t, None, "Do work.");
    mine.teammate.runtime = Some("gemini".into());
    // Native names the teammate, not Electron's "Gemini CLI is installed and signed in, but Locust cannot read its
    // event stream yet.": the window says the same words of a teammate it cannot run.
    assert_eq!(send(mine).unwrap_err(), "Codec runs in Locust for now.");
    assert!(sent_log(&t, "gemini-argv.jsonl").is_empty(), "nothing was started");
    assert!(ledgers(&t).is_empty(), "nothing was recorded");
}

fn on(t: &Temp, runtime: &str, mode: &str, prompt: &str) -> Send {
    let mut mine = job(t, None, prompt);
    mine.teammate.runtime = Some(runtime.into());
    mine.teammate.model = None;
    mine.model = None;
    mine.teammate.runs_in = mode.into();
    mine
}

#[test]
fn a_read_only_cursor_mission_runs_under_its_own_command_where_the_sandbox_is_real() {
    let args = crate::runtime::cursor::print_args(Path::new("/workspace"), Mode::Ask, None, None, None).unwrap();
    for (i, flag) in ["--print", "--trust", "--mode", "ask", "--sandbox", "enabled"].iter().enumerate() {
        assert!(args.iter().any(|a| a == flag), "{i} {flag}: {args:?}");
    }
    assert!(args.windows(2).any(|w| w == ["--mode", "ask"]) && args.windows(2).any(|w| w == ["--sandbox", "enabled"]), "{args:?}");
    assert!(!args.iter().any(|a| a == "--force"), "{args:?}");
}

#[test]
fn a_cursor_mission_is_read_by_cursors_own_normalizer_and_recorded_read_only() {
    let t = Temp::new();
    // Two records as Cursor prints them, so the normalizer's signature on the events is the fact under test.
    let printed = [
        json!({"type": "system", "subtype": "init", "session_id": "cursor-session-1", "model": "Composer 2.5", "permissionMode": "default"}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "pebble"}]}}),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": "pebble"}),
    ];
    let _hook = hooks::install(t.store(), argv_fake(&t, "cursor", &printed, false));
    let started = send(on(&t, "cursor", "ask", "Do work.")).unwrap();
    finished(&started.conversation_id);
    let path = path_of(&t, &started.conversation_id);
    let head = header(&path);
    assert_eq!((&head["runtime"], &head["sandbox"]), (&json!("cursor"), &json!("read-only")));
    let events: Vec<Value> = records(&path).into_iter().filter(|r| r["recordType"] == "mission.event").map(|r| r["event"].clone()).collect();
    let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["run.started", "message.delta", "run.completed"], "{events:?}");
    assert!(events.iter().all(|e| e["sourceAdapter"] == "cursor"), "{events:?}");
    assert_eq!(events[0]["runtimeThreadId"], "cursor-session-1");
}

#[test]
fn a_copilot_teammate_is_sent_the_effort_it_was_given() {
    let args = crate::runtime::copilot::prompt_args("Say hi.", Mode::Ask, None, Some("high"), None, None, &[], None);
    let at = args.iter().position(|a| a == "--effort").expect("--effort is sent");
    assert_eq!(args[at..at + 2], ["--effort", "high"]);
}

#[test]
fn every_copilot_run_gets_locusts_command_guard_as_a_plugin_and_no_other_runtime_that_plugin() {
    let t = Temp::new();
    let plugin = t.0.join("native/guard/copilot-command-guard").to_string_lossy().into_owned();
    {
        let _hook = hooks::install(t.store(), argv_fake(&t, "copilot", &[], false));
        let started = send(on(&t, "copilot", "ask", "Say hi.")).unwrap();
        finished(&started.conversation_id);
        let args = argv_of(&t, "copilot");
        let at = args.iter().position(|a| a == "--plugin-dir").expect("the guard's plugin folder is given");
        assert_eq!(args[at..at + 2], ["--plugin-dir".to_owned(), plugin.clone()]);
    }
    let _hook = hooks::install(t.store(), argv_fake(&t, "cursor", &[], false));
    let started = send(on(&t, "cursor", "ask", "Say hi.")).unwrap();
    finished(&started.conversation_id);
    assert!(!argv_of(&t, "cursor").contains(&plugin), "{:?}", argv_of(&t, "cursor"));
}

#[test]
fn a_cursor_mission_may_edit_when_asked_and_its_commands_are_never_forced() {
    let args = crate::runtime::cursor::print_args(Path::new("/workspace"), Mode::AcceptEdits, None, None, None).unwrap();
    assert!(!args.iter().any(|a| a == "--mode"), "{args:?}");
    assert!(!args.iter().any(|a| a == "--force"), "{args:?}");
}

#[test]
fn nothing_is_recorded_when_the_chosen_options_cannot_be_turned_into_a_command() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), argv_fake(&t, "cursor", &[], false));
    // Cursor takes no effort level.
    let mut mine = on(&t, "cursor", "ask", "Do work.");
    mine.effort = Some("high".into());
    assert_eq!(send(mine).unwrap_err(), "Cursor Agent takes no effort level");
    assert!(sent_log(&t, "cursor-argv.jsonl").is_empty(), "nothing was started");
    assert!(ledgers(&t).is_empty(), "nothing was recorded");
}

// ---- Approve-each is a mode of this service ----
//
// The recording fake app-server asks what `commands-reply.json`'s `ask` names, as `srv-1`, and writes the answer it
// gets to `commands-replies.jsonl`.

fn asks(t: &Temp, method: &str, params: Value) {
    fs::write(t.0.join("work/commands-reply.json"), json!({"ask": {"method": method, "params": params}}).to_string()).unwrap();
}

fn approve_each(t: &Temp, prompt: &str) -> Started {
    let mut mine = job(t, None, prompt);
    mine.teammate.runs_in = "approve-each".into();
    send(mine).unwrap()
}

fn card_for(mission: &str) -> ApprovalRequest {
    let mut card = None;
    wait("a card is raised", || {
        card = waiting_approvals_for(mission).into_iter().next();
        card.is_some()
    });
    card.unwrap()
}

#[test]
fn approve_each_runs_write_capable_and_stops_to_ask_rather_than_falling_through_to_read_only() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = approve_each(&t, "Do something consequential.");
    assert_eq!(started.mode, "approve-each");
    finished(&started.conversation_id);
    // The record says what it was allowed, and approvals only mean something when the run could otherwise act.
    let head = header(&path_of(&t, &started.conversation_id));
    assert_eq!((&head["sandbox"], &head["mode"]), (&json!("workspace-write"), &json!("approve-each")));
    // The policy that makes the server ask at all.
    let requests = codex_requests(&t);
    assert!(requests.iter().any(|r| r["method"] == "turn/start"));
    let thread = thread_request(&t);
    assert_eq!((&thread["params"]["sandbox"], &thread["params"]["approvalPolicy"]), (&json!("workspace-write"), &json!("untrusted")));
}

#[test]
fn a_card_is_raised_mid_run_and_the_persons_answer_goes_back_to_the_runtime() {
    let t = Temp::new();
    asks(&t, "item/commandExecution/requestApproval", json!({"command": "pnpm build", "cwd": "C:\\work"}));
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = approve_each(&t, "Build it.");
    let card = card_for(&started.conversation_id);
    assert_eq!((card.kind.as_str(), card.detail.as_str(), card.runtime.as_str()), ("command", "pnpm build", "codex"));
    // The run it belongs to, so the card can be filed under the conversation.
    assert_eq!(card.mission_id, started.conversation_id);
    decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    finished(&started.conversation_id);
    let replies = sent_log(&t, "commands-replies.jsonl");
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["result"]["decision"], "accept", "{replies:?}");
}

#[test]
fn a_stopped_run_leaves_no_card_waiting_on_it() {
    let t = Temp::new();
    asks(&t, "item/fileChange/requestApproval", json!({"changes": [1]}));
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = approve_each(&t, "Build it.");
    let card = card_for(&started.conversation_id);
    assert_eq!(waiting_approvals_for(&started.conversation_id).len(), 1);
    stop(&started.conversation_id).unwrap();
    finished(&started.conversation_id);
    // What is asserted is that nothing is left waiting, not that a refusal reached the wire: stopping ends the server.
    assert!(waiting_approvals_for(&started.conversation_id).is_empty());
    assert!(decide_approval(&card.approval_id, Decision::ApproveOnce, None).is_err());
}

#[test]
fn an_answer_to_a_card_nobody_raised_is_not_taken() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    assert!(decide_approval("ap_1", Decision::ApproveOnce, None).is_err());
}
