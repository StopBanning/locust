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

// case: lets the run start when the limit cannot be checked, and says why in the log
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
    // Said in the log a tester sends back (Electron's `note`), one line after its timestamp.
    let log = fs::read_to_string(crate::diagnostic_log::log_path(t.store().root())).unwrap_or_default();
    let said: Vec<&str> = log.lines().filter_map(|l| l.split_once(' ').map(|(_, rest)| rest)).filter(|l| l.starts_with("spend-limit:")).collect();
    assert_eq!(said, ["spend-limit: could not check tm_codec's monthly limit: The spending record could not be read."], "{log}");
}

// case: runs read-only when the mode is ask
#[test]
fn an_ask_run_is_started_read_only_and_recorded_so() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let started = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only read-only");
    assert_eq!(header(&path_of(&t, &started.conversation_id))["sandbox"], "read-only");
}

// case: records an MCP-origin Ask run with the actual read-only sandbox
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

// case: runs workspace-write only when edits were explicitly accepted
// case: records the sandbox it actually used in the durable header
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

// case: sends a chosen model to the runtime and records it
#[test]
fn a_chosen_model_is_sent_to_the_runtime_and_recorded() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let started = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(ended(&t, &started.conversation_id).contains(&"run.completed".to_owned()), "the fake accepted only synthetic-model");
    assert_eq!(header(&path_of(&t, &started.conversation_id))["model"], "synthetic-model");
}

// case: treats account-default as "send no model", not as a model id
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

// case: is refused in words, before anything is written, once it has been removed
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

// case: rejects invalid prompts before discovery or process launch
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

// case: records that a plan was a plan, while still holding it read-only
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

// case: refuses an Auto mission when nobody wired the switch at all
#[test]
fn an_auto_run_is_refused_when_the_switch_was_never_set() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("auto"));
    assert_eq!(send(in_mode(&t, "auto", "Tidy my whole machine.")).unwrap_err(), AUTO_OFF_REFUSAL);
    assert!(ledgers(&t).is_empty());
}

// case: runs an Auto mission on the widest sandbox once the workspace allows it
// case: refuses the NEXT Auto run once the switch is taken back, on the same service
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
    let work = t.0.join("work").canonicalize().unwrap_or_else(|_| t.0.join("work"));
    let mut meta = json!({"missionId": "mission_prior", "runId": "run_prior", "prompt": "check the google stock price",
        "runtime": "codex", "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
        "cliVersion": null, "workspaceId": crate::workspace_settings::workspace_id_for(&work), "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": NOW});
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

// case: A2.11: carries the conversationu2019s earlier turns into the switch, oldest first, and stops at a loop
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

// case: records a resume on another runtime as a route switch, not a follow-up
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

// case: starts a fresh conversation when nothing is being continued
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

// case: quotes a waiting message into the prompt as a claim, records it, and marks it delivered only once the runtime starts
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

// case: runs without the channel when it cannot be read, and reports that on the receipt
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
    // Electron's receipt says `peerDeliveryFailed` and its window says so beside the turn; native says it there.
    let said: Vec<(String, String)> = relay_live_state(std::slice::from_ref(&id)).notices.into_iter().map(|n| (n.message, n.level)).collect();
    assert_eq!(said, [(WORKROOM_UNREADABLE.to_owned(), "warning".to_owned())]);
    assert_eq!(WORKROOM_UNREADABLE, "Messages from teammates could not be read for this mission. Whatever was waiting is still waiting.");
    // A workroom that reads says nothing of the kind.
    fs::remove_file(room.join(crate::workroom::FILE_NAME)).unwrap();
    let fine = wren_ran(&t, "ask", "Task.");
    assert!(relay_live_state(&[fine]).notices.is_empty());
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

// ---- missions side by side ----

// case: runs two teammatesu2019 missions at once, and stops each by its own run id
#[test]
fn two_teammates_turns_run_at_once_and_a_stop_ends_only_the_one_it_names() {
    let t = Temp::new();
    wren_and_atlas(&t);
    // The fake runs until it is interrupted, and takes only its own synthetic words.
    let _hook = hooks::install(t.store(), fake("interrupt"));
    let wren_turn = send(wren(&t, "ask", "Synthetic prompt")).unwrap();
    let mut atlas_job = job(&t, None, "Synthetic prompt");
    atlas_job.teammate.id = "tm_atlas".into();
    atlas_job.teammate.name = "Atlas".into();
    let atlas_turn = send(atlas_job).unwrap();
    assert_ne!(wren_turn.conversation_id, atlas_turn.conversation_id);
    let (wren_path, atlas_path) = (path_of(&t, &wren_turn.conversation_id), path_of(&t, &atlas_turn.conversation_id));
    wait("both turns to start", || wren_path.exists() && atlas_path.exists() && started(&wren_path) && started(&atlas_path));
    stop(&atlas_turn.conversation_id).unwrap();
    finished(&atlas_turn.conversation_id);
    assert_eq!(kinds(&atlas_path).last().map(String::as_str), Some("run.cancelled"));
    // Wren's turn was not touched by Atlas's stop.
    assert!(owns(&wren_turn.conversation_id));
    assert!(!kinds(&wren_path).iter().any(|k| matches!(k.as_str(), "run.cancelled" | "run.completed" | "run.failed")), "{:?}", kinds(&wren_path));
    stop(&wren_turn.conversation_id).unwrap();
    finished(&wren_turn.conversation_id);
    assert_eq!(kinds(&wren_path).last().map(String::as_str), Some("run.cancelled"));
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

// case: launches a server rather than an exec, and turns its deltas into message events
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

// case: runs one fixed read-only Codex route and streams normalized events
#[test]
fn a_fixed_read_only_codex_mission_keeps_its_route_and_writes_the_stream_and_final_answer() {
    let t = Temp::new();
    // Electron's fixture supplies one completed item, not an extra delta repeating the whole answer.
    // Send the two app-server fragments followed by just that completed item.
    let server = r#"
import json, sys
from pathlib import Path

def send(**message):
    print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

for line in sys.stdin:
    request = json.loads(line)
    method = request["method"]
    if method == "initialized":
        continue
    with Path("commands-requests.jsonl").open("a", encoding="utf-8") as log:
        log.write(line)
    id = request["id"]
    if method == "initialize":
        send(id=id, result={})
    elif method == "config/read":
        send(id=id, result={"config": {}})
    elif method == "thread/start":
        send(id=id, result={"thread": {"id": "thread-live"}})
    elif method == "turn/start":
        def notify(method, **params):
            send(method=method, params={"threadId": "thread-live", **params})

        send(id=id, result={"turn": {"id": "turn-live"}})
        notify("turn/started", turn={"id": "turn-live"})
        for part in ["Safe ", "result"]:
            notify("item/agentMessage/delta", itemId="answer", turnId="turn-live", delta=part)
        notify("item/completed", item={"id": "answer", "type": "agentMessage", "text": "Safe result"})
        notify("turn/completed", turn={"id": "turn-live", "status": "completed"})
    else:
        raise AssertionError(method)
"#;
    let _hook = hooks::install(t.store(), Some((PathBuf::from("python3"), vec!["-c".into(), server.into()])));
    const PROMPT: &str = "Inspect the workspace without changing it.";
    let mut mine = job(&t, None, PROMPT);
    mine.model = Some("account-default".into());
    let started = send(mine).unwrap();
    assert_eq!(started.mode, "ask");
    finished(&started.conversation_id);

    // The public window send starts one app-server thread/turn in native. Electron's older exec
    // sends an argv sandbox and emits step events; this transport sends the policy and prompt as JSON.
    let requests = codex_requests(&t);
    assert_eq!(methods(&requests), ["initialize", "config/read", "thread/start", "turn/start"]);
    let open = &requests[2]["params"];
    assert_eq!(open["cwd"], json!(t.0.join("work").to_string_lossy()));
    assert_eq!(open["sandbox"], "read-only");
    assert_eq!(open["approvalPolicy"], "never");
    assert!(open.get("model").is_none(), "account-default never names a model");
    assert!(!serde_json::to_string(&requests).unwrap().contains("danger-full-access"));
    let turn = &requests[3]["params"];
    assert_eq!(turn["threadId"], "thread-live");
    assert_eq!(turn["approvalPolicy"], "never");
    assert!(turn.get("model").is_none());
    let input = &turn["input"][0];
    assert_eq!(input["type"], "text");
    // Native can add its own briefing; the question belongs in the turn, never in the process arguments.
    assert!(input["text"].as_str().is_some_and(|asked| asked.contains(PROMPT)));

    let path = path_of(&t, &started.conversation_id);
    assert_eq!(ledgers(&t), [path.clone()], "one turn made one durable mission");
    let head = header(&path);
    assert_eq!(head["missionId"], started.conversation_id);
    assert_eq!(head["prompt"], PROMPT);
    assert_eq!(head["runtime"], "codex");
    assert_eq!(head["model"], "account-default");
    assert_eq!(head["sandbox"], "read-only");
    assert!(head["command"]["args"].as_array().unwrap().iter().all(|arg| arg != PROMPT));

    // Native's window reads this ledger. The hook gives no Codex CLI version; don't invent one.
    let durable = records(&path);
    let events: Vec<_> = durable.iter().filter(|r| r["recordType"] == "mission.event").map(|r| &r["event"]).collect();
    let position = |kind| events.iter().position(|e| e["type"] == kind).expect("expected event in the durable ledger");
    let deltas: Vec<_> = events.iter().enumerate().filter(|(_, e)| e["type"] == "message.delta").collect();
    assert_eq!(deltas.len(), 3, "the two fragments and the final answer must be saved: {events:?}");
    let (first, a) = deltas[0];
    let (second, b) = deltas[1];
    for (event, fragment) in [(a, "Safe "), (b, "result")] {
        assert_eq!(event["payload"]["operation"], "append");
        assert_eq!(event["payload"]["text"], fragment);
        assert_eq!(event["payload"]["final"], false);
    }
    let finals: Vec<_> = deltas.iter().copied().filter(|(_, e)| e["payload"]["final"] == true).collect();
    assert_eq!(finals.len(), 1, "exactly one durable final answer: {deltas:?}");
    let (final_at, answer) = finals[0];
    assert_eq!(answer["payload"]["operation"], "replace");
    assert_eq!(answer["payload"]["text"], "Safe result");
    for event in [a, b, answer] {
        assert_eq!(event["payload"]["itemId"], "answer");
    }
    assert!(position("run.started") < first && first < second && second < final_at && final_at < position("run.completed"));
    let transcript = ledger::read_transcript(&path);
    let answers: Vec<_> = transcript.items.iter().filter_map(|item| match item {
        ledger::Item::Assistant { text, .. } => Some(text.as_str()),
        _ => None,
    }).collect();
    assert_eq!(answers, ["Safe result"], "opening the ledger must not show an echo of the stream");
}

// case: starts none of the MCP servers or plugins the person’s Codex runs as programs, in Ask and Plan only (0.824)
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

// case: starts every Codex thread with Locust’s command guard, for its own folder, and without it when the guard says so (0.720)
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

// case: refuses a runtime whose event stream it cannot read yet, by name, recording nothing
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

// case: runs a read-only Cursor mission where the sandbox is real, and asks for it
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
    // The host's start-timing note follows the run's own events, as Cursor's too.
    assert_eq!(kinds, ["run.started", "message.delta", "run.completed", "adapter.diagnostic"], "{events:?}");
    assert_eq!(events[3]["payload"]["code"], "host.start-timing");
    assert!(events[3]["payload"]["message"].as_str().unwrap().contains("; Cursor took "), "{events:?}");
    assert!(events.iter().all(|e| e["sourceAdapter"] == "cursor"), "{events:?}");
    assert_eq!(events[0]["runtimeThreadId"], "cursor-session-1");
}

#[test]
fn a_copilot_teammate_is_sent_the_effort_it_was_given() {
    let args = crate::runtime::copilot::prompt_args("Say hi.", Mode::Ask, None, Some("high"), None, None, &[], None);
    let at = args.iter().position(|a| a == "--effort").expect("--effort is sent");
    assert_eq!(args[at..at + 2], ["--effort", "high"]);
}

// case: is given on the print route and the Approve-each route alike, and only when there is one
// (Here the plugin is added for every Copilot start before its route is chosen; this runs the print route.)
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

// case: refuses what a stopped run was still asking, so no card waits on a run that has gone
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

// Native's approval channel is the engine's own and always there, so what is left to prove is the half that matters:
// an answer with no card behind it is refused, never reported as taken.
// case: answers nothing when no channel is wired, rather than pretending it did
#[test]
fn an_answer_to_a_card_nobody_raised_is_not_taken() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    assert!(decide_approval("ap_1", Decision::ApproveOnce, None).is_err());
}

// ---- a conversation runs in its own folder (main/a-conversation-runs-in-its-own-folder.test.ts) ----

fn folder_setup(t: &Temp, folder_of_earlier: &str, known: &[(&str, &str)]) -> (PathBuf, PathBuf, PathBuf) {
    let here = t.0.join("here");
    let there = t.0.join("there");
    fs::create_dir_all(&here).unwrap();
    fs::create_dir_all(&there).unwrap();
    let store = t.store();
    crate::native::set_open_folder(&store, &here).unwrap();
    let entries: Vec<(String, String)> = known
        .iter()
        .map(|(id, p)| ((*id).to_owned(), (*p).to_owned()))
        .collect();
    crate::folders::learn(&store, &entries).unwrap();
    let prior = earlier(
        t,
        json!({
            "missionId": "mission_earlier",
            "runId": "run_earlier",
            "workspaceId": crate::workspace_settings::workspace_id_for(Path::new(folder_of_earlier)),
        }),
        &[],
    );
    (here, there, prior)
}

fn folder_job(t: &Temp, here: &Path, conversation: Option<&str>, prompt: &str) -> Send {
    let mut j = job(t, conversation, prompt);
    j.teammate.folder = Some(here.to_string_lossy().into_owned());
    j
}

#[test]
fn runs_in_its_conversations_folder_not_the_windows_and_is_recorded_there() {
    let t = Temp::new();
    let here = t.0.join("here");
    let there = t.0.join("there");
    let there_s = there.to_string_lossy().into_owned();
    let ws_there = crate::workspace_settings::workspace_id_for(&there);
    let (_, _, prior) = folder_setup(&t, &there_s, &[(&ws_there, &there_s)]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = send(folder_job(&t, &here, Some("mission_earlier"), "And the tests?")).unwrap();
    finished(&started.conversation_id);
    let head = new_turn(&t, &[&prior]);
    assert_eq!(head["command"]["cwd"], there_s);
    assert_eq!(head["workspaceId"], ws_there);
}

#[test]
fn continues_a_restored_windows_conversation_in_the_chosen_mac_folder() {
    let t = Temp::new();
    let here = t.0.join("here");
    let there = t.0.join("there");
    let there_s = there.to_string_lossy().into_owned();
    let old = "C:\\work\\demo";
    let ws_old = crate::workspace_settings::workspace_id_for(Path::new(old));
    let ws_there = crate::workspace_settings::workspace_id_for(&there);
    let (_, _, prior) = folder_setup(&t, old, &[(&ws_old, &there_s)]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = send(folder_job(&t, &here, Some("mission_earlier"), "And the tests?")).unwrap();
    finished(&started.conversation_id);
    let head = new_turn(&t, &[&prior]);
    assert_eq!(head["command"]["cwd"], there_s);
    assert_eq!(head["workspaceId"], ws_there);
}

#[test]
fn a_new_conversation_still_starts_in_the_windows_folder() {
    let t = Temp::new();
    let (here, there, _) = folder_setup(&t, &t.0.join("there").to_string_lossy(), &[]);
    let there_s = there.to_string_lossy().into_owned();
    let ws_there = crate::workspace_settings::workspace_id_for(&there);
    crate::folders::learn(&t.store(), &[(ws_there, there_s)]).unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = send(folder_job(&t, &here, None, "Start something")).unwrap();
    finished(&started.conversation_id);
    let head = header(&path_of(&t, &started.conversation_id));
    let here_canon = here.canonicalize().unwrap();
    assert_eq!(head["command"]["cwd"], here_canon.to_string_lossy().as_ref());
    assert_eq!(head["workspaceId"], crate::workspace_settings::workspace_id_for(&here_canon));
}

#[test]
fn is_refused_not_run_here_when_its_folder_is_not_known() {
    let t = Temp::new();
    let here = t.0.join("here");
    let there = t.0.join("there");
    let there_s = there.to_string_lossy().into_owned();
    folder_setup(&t, &there_s, &[]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let before = ledgers(&t).len();
    let err = send(folder_job(&t, &here, Some("mission_earlier"), "And the tests?")).unwrap_err();
    assert!(err.contains("does not know which folder this conversation ran in"), "{err}");
    assert_eq!(ledgers(&t).len(), before);
}

#[test]
fn is_refused_not_run_here_when_its_folder_is_gone() {
    let t = Temp::new();
    let here = t.0.join("here");
    let gone = t.0.join("there").join("deleted-project");
    let gone_s = gone.to_string_lossy().into_owned();
    let ws_gone = crate::workspace_settings::workspace_id_for(&gone);
    folder_setup(&t, &gone_s, &[(&ws_gone, &gone_s)]);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let before = ledgers(&t).len();
    let err = send(folder_job(&t, &here, Some("mission_earlier"), "And the tests?")).unwrap_err();
    assert!(err.contains("that folder is not there any more"), "{err}");
    assert_eq!(ledgers(&t).len(), before);
}

// case: adds an observed edit row, after the terminal event, for a file no tool named
#[test]
fn a_write_capable_turn_records_observed_edit_for_a_file_modified_on_disk_without_a_tool_event() {
    let t = Temp::new();
    asks(&t, "item/commandExecution/requestApproval", json!({"command": "python3 build.py", "cwd": "C:\\work"}));
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = approve_each(&t, "Build it.");
    let card = card_for(&started.conversation_id);
    fs::write(t.0.join("work/unreported.txt"), "hello from disk\n").unwrap();
    decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    finished(&started.conversation_id);
    let recs = records(&path_of(&t, &started.conversation_id));
    let observed: Vec<&Value> = recs
        .iter()
        .filter_map(|r| r.get("event"))
        .filter(|e| e["type"] == "tool.completed" && e["payload"]["toolKind"] == "observed_edit")
        .collect();
    assert_eq!(observed.len(), 1, "{recs:?}");
    assert_eq!(observed[0]["payload"]["command"], "unreported.txt");
    assert_eq!(observed[0]["payload"]["status"], "observed on disk");
    assert_eq!(observed[0]["payload"]["patch"]["added"], 1);
    // two-teammates-hear-about-each-others-edits: what the run changed is kept for another teammate's next brief here.
    let who = codex(&t).id;
    assert!(!who.is_empty());
    let folder = t.0.join("work").to_string_lossy().into_owned();
    crate::recent_edits::record(&folder, "tm_other_hand", "Other Hand", &["unreported.txt".to_owned()]);
    let note = crate::recent_edits::overlap_for(&folder, &who).unwrap_or_default();
    assert!(note.contains("- Other Hand changed unreported.txt (last"), "{note}");
}

// ---- start-timing.test.ts: through the mission service. The run's end writes where its start's seconds went. ----
fn start_timing_note(recs: &[Value]) -> (usize, Value) {
    let events: Vec<&Value> = recs.iter().filter_map(|r| r.get("event")).collect();
    let at = events.iter().position(|e| e["payload"]["code"] == "host.start-timing").unwrap_or_else(|| panic!("no start-timing note: {recs:?}"));
    assert_eq!(events.iter().filter(|e| e["payload"]["code"] == "host.start-timing").count(), 1, "one note per run");
    (events.len() - 1 - at, events[at].clone())
}

// case: says looking, briefing, reading the folder and starting, in that order, and the end writes the note
#[test]
fn a_writing_run_ends_with_a_note_naming_each_phase_of_its_start() {
    let t = Temp::new();
    fs::write(t.0.join("work/commands-reply.json"), json!({"text": "Tidied.", "status": "completed"}).to_string()).unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let mut mine = job(&t, None, "Tidy it.");
    mine.teammate.runs_in = "approve-each".into();
    let started = send(mine).unwrap();
    finished(&started.conversation_id);
    let recs = records(&path_of(&t, &started.conversation_id));
    let (after, note) = start_timing_note(&recs);
    assert_eq!(after, 0, "written after the run's own events");
    assert_eq!(note["type"], "adapter.diagnostic");
    assert_eq!(note["payload"]["level"], "info");
    assert_eq!(note["sourceAdapter"], "codex");
    let last_seq = recs.iter().filter_map(|r| r.get("event")).filter(|e| e["payload"]["code"] != "host.start-timing").filter_map(|e| e["sequence"].as_u64()).max().unwrap();
    assert_eq!(note["sequence"].as_u64(), Some(last_seq + 1));
    let said = note["payload"]["message"].as_str().unwrap();
    assert!(said.starts_with("Started in "), "{said}");
    for phase in ["looked for Codex ", "briefed ", "read the folder ", "; Codex took "] {
        assert!(said.contains(phase), "{phase:?} in {said}");
    }
    assert!(said.find("looked for").unwrap() < said.find("briefed").unwrap() && said.find("briefed").unwrap() < said.find("read the folder").unwrap());
}

// case: reads no folder for a read-only start, and says so by leaving the phase out
#[test]
fn a_read_only_runs_note_reads_no_folder() {
    let t = Temp::new();
    fs::write(t.0.join("work/commands-reply.json"), json!({"text": "A readme.", "status": "completed"}).to_string()).unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let started = send(job(&t, None, "What is in here?")).unwrap();
    finished(&started.conversation_id);
    let (_, note) = start_timing_note(&records(&path_of(&t, &started.conversation_id)));
    let said = note["payload"]["message"].as_str().unwrap();
    assert!(said.contains("briefed ") && said.contains("Codex took "), "{said}");
    assert!(!said.contains("read the folder"), "{said}");
}

// ---- a-resumed-session-is-told-only-what-changed.test.ts: through the mission service (A2.5) ----
#[test]
fn briefs_a_resumed_turn_with_what_changed_and_in_full_again_after_the_session_compacted() {
    const ROSTER_RULE: &str = "Writing a teammate's name in your reply does NOT reach them.";
    const STILL_HOLDS: &str = "You were given standing instructions earlier in this conversation";
    let t = Temp::new();
    t.store()
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "role": "Code & Migrations"},
                {"teammateId": "tm_atlas", "name": "Atlas", "role": "Research & Briefs"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let sent = |t: &Temp| -> Vec<String> {
        codex_requests(t)
            .iter()
            .filter(|r| r["method"] == "turn/start")
            .map(|r| r["params"]["input"][0]["text"].as_str().unwrap().to_owned())
            .collect()
    };
    // Each turn's own ledger: the one that was not there before it started (a reply answers with the conversation).
    let turn = |t: &Temp, conversation: Option<&str>, words: &str| -> PathBuf {
        let before = ledgers(t);
        let started = send(job(t, conversation, words)).unwrap();
        let path = ledgers(t).into_iter().find(|p| !before.contains(p)).expect("a turn was started");
        finished(path.file_stem().unwrap().to_str().unwrap());
        let _ = started;
        path
    };
    let one = turn(&t, None, "First.");
    let conversation = one.file_stem().unwrap().to_str().unwrap().to_owned();
    let two = turn(&t, Some(&conversation), "Second.");
    // The session summarized itself during turn two.
    let compacted = json!({"schemaVersion": 27, "recordType": "mission.event", "occurredAt": NOW, "event": {"id": "e_compact", "sequence": 99,
        "type": "adapter.diagnostic", "occurredAt": NOW, "sourceAdapter": "codex",
        "payload": {"code": "codex.context_compacted", "level": "info", "message": "x", "terminal": false, "evidence": {"redacted": true}}}});
    let mut ledger = fs::OpenOptions::new().append(true).open(&two).unwrap();
    std::io::Write::write_all(&mut ledger, format!("{compacted}\n").as_bytes()).unwrap();
    turn(&t, Some(&conversation), "Third.");

    let told = sent(&t);
    assert_eq!(told.len(), 3, "{told:?}");
    assert!(told[0].contains(ROSTER_RULE));
    // Resumed, not compacted: the line, not the rules.
    assert!(told[1].contains(STILL_HOLDS), "{}", told[1]);
    assert!(!told[1].contains(ROSTER_RULE));
    assert!(told[1].split("\n\nReference folders").next().unwrap().ends_with("Second."), "{}", told[1]);
    // The whole brief again.
    assert!(told[2].contains(ROSTER_RULE), "{}", told[2]);
    assert!(!told[2].contains(STILL_HOLDS));
}

// ---- memory round 1: what a real turn does with memory before it starts ----
#[test]
fn a_turn_writes_the_memory_file_where_it_stands_and_notes_what_its_brief_was_given() {
    let t = Temp::new();
    crate::people::add_memory(&t.store(), "The build is pnpm check.", None, "global", "ws_any", json!({"name": "you"}), "kept").unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let before = ledgers(&t);
    send(job(&t, None, "What is the build?")).unwrap();
    let path = ledgers(&t).into_iter().find(|p| !before.contains(p)).expect("a turn was started");
    finished(path.file_stem().unwrap().to_str().unwrap());
    let file = fs::read_to_string(t.0.join("work").join(crate::memory_file::MEMORY_FILE_IN_FOLDER)).expect("the memory file was written");
    let kept = crate::people::memories(&t.store()).unwrap();
    let id = kept["memories"][0]["memoryId"].as_str().unwrap().to_owned();
    assert!(file.contains(&format!("- The build is pnpm check. (by the person -- ")) && file.contains(&format!("[{id}]")), "{file}");
    assert!(kept["memories"][0]["lastBriefedAt"].is_string(), "{kept}");
    assert!(kept["briefTrackingSince"].is_string(), "{kept}");
}

/// A turn that answers `reply` to `prompt`, and the notices it left for its conversation.
fn memory_notices(t: &Temp, prompt: &str, reply: &str) -> Vec<RelayNotice> {
    crate::people::add_memory(&t.store(), "Tests run with pnpm test.", None, "global", "ws_any", json!({"name": "you"}), "kept").unwrap();
    fs::write(t.0.join("work/commands-reply.json"), json!({"text": reply, "status": "completed"}).to_string()).unwrap();
    let _hook = hooks::install(t.store(), codex_commands_hook(t));
    let before = ledgers(t);
    send(job(t, None, prompt)).unwrap();
    let path = ledgers(t).into_iter().find(|p| !before.contains(p)).expect("a turn was started");
    let id = path.file_stem().unwrap().to_str().unwrap().to_owned();
    finished(&id);
    relay_live_state(&[id]).notices
}

#[test]
fn a_forget_that_missed_is_said_in_the_thread_in_amber() {
    let t = Temp::new();
    let said = memory_notices(&t, "Clean up.", "Done.\n<locust-memory>\nforget :: anything at all\n</locust-memory>");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].message.contains("tried to forget something and could not."), "{said:?}");
    assert_eq!(said[0].level, "warning");
}

#[test]
fn a_tidy_pass_that_put_nothing_to_the_person_says_so_quietly_in_the_thread() {
    let t = Temp::new();
    let said = memory_notices(&t, &crate::memory_tidy::tidy_prompt(), "Summary: the port memory is stale.");
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!((said[0].message.as_str(), said[0].level.as_str()), ("Nothing from this tidy pass waits on the Memory screen.", "info"));
}

// ---- round 18: cases that hold, named ----

#[test]
fn shares_nothing_from_a_mission_that_belongs_to_nobody() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    the_turn_says(&t, "<locust-share to=\"Atlas\">\nfinding\n</locust-share>", "completed");
    let started = send_from(
        Send { teammate: nobody(Some("codex".into()), None), conversation: None, prompt: "Task.".into(), model: None, effort: None },
        Origin { mode: Some(Mode::Ask), folder: Some(t.0.join("work")), runtime: Some("codex".into()), ..Default::default() },
    )
    .unwrap();
    finished(&started.conversation_id);
    assert!(kinds(&path_of(&t, &started.conversation_id)).contains(&"run.completed".to_owned()));
    assert!(crate::workroom::Workroom::for_store(&t.store()).read().messages.is_empty(), "nothing was posted");
    // It is briefed, and not as anybody: the person's words come last, and no roster or share block is in it.
    // (Native adds the reference-folders line after them; `told` reads up to it.)
    let told = told(&t);
    assert!(told.ends_with("Task."), "{told}");
    assert!(told.contains("<locust-ask>"), "{told}");
    assert!(!told.contains("Atlas") && !told.contains("<locust-share"), "{told}");
}

/// Wren's writing turn that shares with Atlas: the fake stops to ask, the test changes `src/app.ts` while it waits.
fn wren_writes_and_shares(t: &Temp, mode: &str) -> Vec<crate::workroom::Message> {
    let reply = json!({"text": "Done.\n<locust-share to=\"Atlas\">I fixed app.ts.</locust-share>", "status": "completed",
        "ask": {"method": "item/commandExecution/requestApproval", "params": {"command": "python3 fix.py", "cwd": "C:\\work"}}});
    let plain = json!({"text": "Done.\n<locust-share to=\"Atlas\">I fixed app.ts.</locust-share>", "status": "completed"});
    fs::create_dir_all(t.0.join("work/src")).unwrap();
    fs::write(t.0.join("work/src/app.ts"), "one\n").unwrap();
    let writing = mode != "ask";
    fs::write(t.0.join("work/commands-reply.json"), if writing { reply } else { plain }.to_string()).unwrap();
    let started = send(wren(t, mode, "Fix app.ts and tell Atlas.")).unwrap();
    if writing {
        let card = card_for(&started.conversation_id);
        fs::write(t.0.join("work/src/app.ts"), "two\n").unwrap();
        fs::write(t.0.join("work/src/new.ts"), "new\n").unwrap();
        decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    }
    finished(&started.conversation_id);
    crate::workroom::Workroom::for_store(&t.store()).unread("tm_atlas", 5, &[]).unwrap().messages
}

#[test]
fn attaches_the_paths_the_host_saw_a_writing_run_change() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let posts = wren_writes_and_shares(&t, "approve-each");
    assert_eq!(posts.len(), 1);
    let mut observed = posts[0].observed.clone().unwrap_or_default();
    observed.sort();
    assert_eq!(observed, ["src/app.ts", "src/new.ts"]);
}

#[test]
fn attaches_nothing_from_a_read_only_run_which_the_host_never_looked_at() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let posts = wren_writes_and_shares(&t, "ask");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].observed, None);
}

// case: A2.9: tells a teammate, in their next brief, that another changed a file they changed too
#[test]
fn a_teammate_is_told_in_their_next_brief_that_another_changed_a_file_they_changed_too() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    fs::create_dir_all(t.0.join("work/src")).unwrap();
    fs::write(t.0.join("work/src/app.ts"), "zero\n").unwrap();
    // Every run finds src/app.ts changed by the time it ends.
    let run_as = |id: &str, name: &str, words: &str| {
        fs::write(
            t.0.join("work/commands-reply.json"),
            json!({"text": "Done.", "status": "completed", "ask": {"method": "item/commandExecution/requestApproval", "params": {"command": "tidy", "cwd": "C:\\work"}}}).to_string(),
        )
        .unwrap();
        let mut mine = wren(&t, "approve-each", "Tidy app.ts.");
        mine.teammate.id = id.into();
        mine.teammate.name = name.into();
        let started = send(mine).unwrap();
        let card = card_for(&started.conversation_id);
        fs::write(t.0.join("work/src/app.ts"), format!("{words}\n")).unwrap();
        decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
        finished(&started.conversation_id);
    };
    run_as("tm_wren", "Wren", "one");
    run_as("tm_atlas", "Atlas", "two");
    run_as("tm_wren", "Wren", "three");
    let briefs: Vec<String> = codex_requests(&t)
        .iter()
        .filter(|r| r["method"] == "turn/start")
        .map(|r| r["params"]["input"][0]["text"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(briefs.len(), 3);
    assert!(!briefs[0].contains("also changed by another teammate"), "{}", briefs[0]);
    assert!(briefs[2].contains("- Atlas changed src/app.ts"), "{}", briefs[2]);
}

#[test]
fn claims_nothing_from_the_tree_when_another_writing_run_shared_the_folder() {
    let t = Temp::new();
    wren_and_atlas(&t);
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    fs::create_dir_all(t.0.join("work/src")).unwrap();
    fs::write(t.0.join("work/src/notes.ts"), "one\n").unwrap();
    asks(&t, "item/commandExecution/requestApproval", json!({"command": "cat README.md", "cwd": "C:\\work"}));
    let observed_rows = |id: &str| -> usize {
        records(&path_of(&t, id)).iter().filter_map(|r| r.get("event")).filter(|e| e["payload"]["status"] == "observed on disk").count()
    };
    let shared_notices = |id: &str| -> Vec<String> {
        records(&path_of(&t, id))
            .iter()
            .filter_map(|r| r.get("event"))
            .filter(|e| e["type"] == "adapter.diagnostic" && e["payload"]["code"] == "host.shared_workspace")
            .map(|e| e["payload"]["message"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    // Two teammates writing in one folder at once; both end looking at the same changed file.
    let first = send(wren(&t, "approve-each", "write one poem")).unwrap();
    let mut atlas_job = wren(&t, "approve-each", "write another poem");
    atlas_job.teammate.id = "tm_atlas".into();
    atlas_job.teammate.name = "Atlas".into();
    let second = send(atlas_job).unwrap();
    let (one, two) = (card_for(&first.conversation_id), card_for(&second.conversation_id));
    fs::write(t.0.join("work/src/notes.ts"), "two\n").unwrap();
    decide_approval(&one.approval_id, Decision::ApproveOnce, None).unwrap();
    decide_approval(&two.approval_id, Decision::ApproveOnce, None).unwrap();
    finished(&first.conversation_id);
    finished(&second.conversation_id);
    for id in [&first.conversation_id, &second.conversation_id] {
        assert_eq!(observed_rows(id), 0, "{:?}", kinds(&path_of(&t, id)));
        let said = shared_notices(id);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("Another teammate was working in this folder"), "{}", said[0]);
        assert!(said[0].contains("1 file in the folder is different"), "{}", said[0]);
        assert!(said[0].contains("it is not counted as this run's work"), "{}", said[0]);
    }
    // The negative control: one run alone, the same change, still gets its row.
    let solo = send(wren(&t, "approve-each", "write one poem")).unwrap();
    let card = card_for(&solo.conversation_id);
    fs::write(t.0.join("work/src/notes.ts"), "three\n").unwrap();
    decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    finished(&solo.conversation_id);
    assert!(observed_rows(&solo.conversation_id) > 0, "{:?}", kinds(&path_of(&t, &solo.conversation_id)));
}

/// THE CHIP IS THE RUN (run_effort.rs, App.tsx runEffortFor): what a Claude teammate's command carries for its level,
/// with `catalog` as the catalogue this process has read. The chip's side of each case is in
/// src/ui/the_chip_is_the_run_tests.rs.
fn claude_effort_sent(t: &Temp, catalog: Vec<Value>, model: &str, effort: Option<&str>) -> Option<String> {
    crate::run_effort::use_catalog_on_this_thread(Some(catalog));
    struct Forget;
    impl Drop for Forget {
        fn drop(&mut self) {
            crate::run_effort::use_catalog_on_this_thread(None);
        }
    }
    let _forget = Forget;
    let _hook = hooks::install(t.store(), argv_fake(t, "claude", &claude_answers(), true));
    let mut mine = claude_job(t, None, "Say hi.");
    mine.teammate.model = Some(model.into());
    mine.effort = effort.map(str::to_owned);
    let started = send(mine).unwrap();
    finished(&started.conversation_id);
    let args = argv_of(t, "claude");
    args.iter().position(|a| a == "--effort").map(|at| args[at + 1].clone())
}

fn sonnet_lists_levels() -> Vec<Value> {
    vec![json!({"id": "sonnet", "runtime": "claude", "displayName": "Sonnet", "description": "", "supportedEfforts": ["low", "medium", "high"]})]
}

// case: agree when nobody has chosen a level
// The run's half: status.test.ts expects `medium` for a model listing low, medium and high, and the chip says the same.
#[test]
fn a_teammate_with_no_level_chosen_is_sent_the_level_its_chip_shows() {
    let t = Temp::new();
    assert_eq!(claude_effort_sent(&t, sonnet_lists_levels(), "sonnet", None).as_deref(), Some("medium"));
}

// case: agree when someone has
// The run's half: a level chosen is the level sent (status.test.ts `high`); the chip's half is in the window's tests.
#[test]
fn a_level_someone_chose_is_the_level_the_run_is_sent() {
    let t = Temp::new();
    assert_eq!(claude_effort_sent(&t, sonnet_lists_levels(), "sonnet", Some("high")).as_deref(), Some("high"));
}

// case: agree on a model with no levels: both say nothing, and nothing is sent
#[test]
fn a_model_that_lists_no_levels_is_sent_no_level_when_nobody_chose_one() {
    let t = Temp::new();
    let none = vec![json!({"id": "sonnet", "runtime": "claude", "displayName": "Sonnet", "description": "", "supportedEfforts": []})];
    assert_eq!(claude_effort_sent(&t, none, "sonnet", None), None);
}

// case: agree on a Cursor family, whose id already names a level
// The run's half: Cursor puts the level in the model's id (and refuses one beside it), so the variant started is the
// level sent. A family standing on
// its low variant is not run at medium (status.test.ts), and a level chosen picks its own variant.
#[test]
fn a_cursor_family_is_run_on_the_level_its_id_names_when_nobody_chose_one() {
    let family = json!({"id": "grok-4.7-low", "runtime": "cursor", "displayName": "Grok 4.7", "description": "",
        "supportedEfforts": ["low", "medium", "high"],
        "variants": {"low": "grok-4.7-low", "medium": "grok-4.7-medium", "high": "grok-4.7-high"}});
    struct Forget;
    impl Drop for Forget {
        fn drop(&mut self) {
            crate::run_effort::use_catalog_on_this_thread(None);
            use_cursor_catalog_on_this_thread(None);
        }
    }
    let _forget = Forget;
    crate::run_effort::use_catalog_on_this_thread(Some(vec![family.clone()]));
    use_cursor_catalog_on_this_thread(Some(vec![family]));
    for (chosen, started_on) in [(None, "grok-4.7-low"), (Some("high"), "grok-4.7-high")] {
        let t = Temp::new();
        let printed = [
            json!({"type": "system", "subtype": "init", "session_id": "cursor-session-1", "model": "Grok 4.7", "permissionMode": "default"}),
            json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok"}),
        ];
        let _hook = hooks::install(t.store(), argv_fake(&t, "cursor", &printed, false));
        let mut mine = on(&t, "cursor", "ask", "Say hi.");
        mine.teammate.model = Some("grok-4.7-low".into());
        mine.effort = chosen.map(str::to_owned);
        let started = send(mine).unwrap();
        finished(&started.conversation_id);
        // The hook stands in for Cursor's whole command, so the model it was started on is read from the record.
        let head = header(&path_of(&t, &started.conversation_id));
        assert_eq!(head["model"], started_on, "{chosen:?}: {head}");
    }
}

// ---- What each agent is actually sent (status.ts startRoute, run_effort.rs start_route) ----

// case: never sends an effort to a model that advertises none
// A leftover level (here the teammate's own, kept from another model) is not sent to a model whose levels the catalogue
// knows to be none (status.test.ts: `{ model }` alone).
#[test]
fn a_level_is_never_sent_to_a_model_the_catalogue_lists_with_none() {
    let t = Temp::new();
    let none = vec![json!({"id": "sonnet", "runtime": "claude", "displayName": "Sonnet", "description": "", "supportedEfforts": []})];
    assert_eq!(claude_effort_sent(&t, none, "sonnet", Some("medium")), None);
}

// case: still sends an effort for a model the catalogue has never heard of
#[test]
fn a_level_is_still_sent_for_a_model_the_catalogue_does_not_list() {
    let t = Temp::new();
    assert_eq!(claude_effort_sent(&t, sonnet_lists_levels(), "account-default", Some("high")).as_deref(), Some("high"));
}

/// The model a Cursor run on `model` at `effort` starts on, or the refusal, with `family` as both catalogues.
fn cursor_started_on(family: Value, model: &str, effort: &str) -> Result<String, String> {
    struct Forget;
    impl Drop for Forget {
        fn drop(&mut self) {
            crate::run_effort::use_catalog_on_this_thread(None);
            use_cursor_catalog_on_this_thread(None);
        }
    }
    let _forget = Forget;
    crate::run_effort::use_catalog_on_this_thread(Some(vec![family.clone()]));
    use_cursor_catalog_on_this_thread(Some(vec![family]));
    let t = Temp::new();
    let printed = [
        json!({"type": "system", "subtype": "init", "session_id": "cursor-session-1", "model": "Grok 4.6", "permissionMode": "default"}),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok"}),
    ];
    let _hook = hooks::install(t.store(), argv_fake(&t, "cursor", &printed, false));
    let mut mine = on(&t, "cursor", "ask", "Say hi.");
    mine.teammate.model = Some(model.into());
    mine.effort = Some(effort.into());
    let started = send(mine)?;
    finished(&started.conversation_id);
    Ok(header(&path_of(&t, &started.conversation_id))["model"].as_str().unwrap_or_default().to_owned())
}

/// status.test.ts's Cursor family: never listed without a level, so its id is the first variant seen.
fn grok_family() -> Value {
    json!({"id": "cursor-grok-4.6-high-fast", "runtime": "cursor", "displayName": "cursor-grok-4.6", "description": "",
        "supportedEfforts": ["low", "high", "high-fast"],
        "variants": {"low": "cursor-grok-4.6-low", "high": "cursor-grok-4.6-high", "high-fast": "cursor-grok-4.6-high-fast"}})
}

// case: drops an effort the variants do not name rather than refuse the run
#[test]
fn a_cursor_level_its_variants_do_not_name_is_dropped_and_the_run_still_starts() {
    assert_eq!(cursor_started_on(grok_family(), "cursor-grok-4.6-high-fast", "medium").as_deref(), Ok("cursor-grok-4.6-high-fast"));
}

// case: finds the family when the id in hand is one of its variants, not its own id
// And whichever variant it starts from, Cursor is never handed a loose level, which is what refuses a run.
#[test]
fn a_cursor_run_on_a_sibling_variant_starts_on_the_variant_its_level_names() {
    assert_eq!(cursor_started_on(grok_family(), "cursor-grok-4.6-low", "high").as_deref(), Ok("cursor-grok-4.6-high"));
    for from in ["cursor-grok-4.6-low", "cursor-grok-4.6-high", "cursor-grok-4.6-high-fast"] {
        for effort in ["low", "high", "high-fast", "medium", "nonsense"] {
            assert!(cursor_started_on(grok_family(), from, effort).is_ok(), "{from} at {effort}");
        }
    }
}
