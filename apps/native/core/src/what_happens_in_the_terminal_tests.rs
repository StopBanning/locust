//! what-happens-in-the-terminal-comes-back.test.ts (0.391): what is typed in a conversation's terminal comes back as
//! its next turns. The shapes are Electron's own fixtures: Claude's `entrypoint` says where a prompt was typed (`cli`
//! the terminal, `sdk-cli` Locust's print mode), and a Codex rollout is ordered by time.
use super::*;
use serde_json::json;

const SESSION: &str = "5f0c2a1e-8d4b-4c6a-9e2f-1b3c4d5e6f70";
const T0: &str = "2026-09-27T10:00:00.000Z";

fn claude_line(mut value: Value) -> String {
    let obj = value.as_object_mut().unwrap();
    obj.insert("sessionId".into(), json!(SESSION));
    obj.insert("cwd".into(), json!("C:\\work"));
    obj.entry("isSidechain").or_insert(json!(false));
    value.to_string()
}
fn typed_extra(text: &str, at: &str, entrypoint: &str, extra: Value) -> String {
    let mut v = json!({"type":"user","entrypoint":entrypoint,"timestamp":at,"uuid":format!("u-{at}"),"message":{"role":"user","content":text}});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    claude_line(v)
}
fn typed(text: &str, at: &str, entrypoint: &str) -> String {
    typed_extra(text, at, entrypoint, json!({}))
}
fn blocks(text: &str, at: &str) -> String {
    claude_line(json!({"type":"user","entrypoint":"cli","timestamp":at,"uuid":format!("b-{at}"),"message":{"role":"user","content":[{"type":"text","text":text}]}}))
}
fn answer_stop(text: &str, at: &str, stop: &str) -> String {
    claude_line(json!({"type":"assistant","timestamp":at,"uuid":format!("a-{at}"),"message":{"role":"assistant","model":"claude-opus-5-5","stop_reason":stop,"content":[{"type":"text","text":text}]}}))
}
fn answer(text: &str, at: &str) -> String {
    answer_stop(text, at, "end_turn")
}
fn tool_result(at: &str) -> String {
    claude_line(json!({"type":"user","entrypoint":"cli","timestamp":at,"message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}))
}

fn claude_session() -> String {
    [
        // Locust's own turn, in print mode, before the terminal.
        typed("Who takes what?", "2026-09-27T09:58:00.000Z", "sdk-cli"),
        answer("Wren takes the migrations.", "2026-09-27T09:59:00.000Z"),
        // In the terminal, after.
        typed("Now add tests for the migrations.", "2026-09-27T10:05:00.000Z", "cli"),
        answer_stop("Looking at the migration files first.", "2026-09-27T10:05:10.000Z", "tool_use"),
        tool_result("2026-09-27T10:05:20.000Z"),
        answer("Added three tests; all pass.", "2026-09-27T10:06:00.000Z"),
        blocks("[Request interrupted by user]", "2026-09-27T10:06:05.000Z"),
        // A command Claude did not answer, then a compacted session's summary.
        typed("<command-name>/model</command-name>", "2026-09-27T10:06:10.000Z", "cli"),
        typed("<local-command-stdout>Set model to Opus</local-command-stdout>", "2026-09-27T10:06:11.000Z", "cli"),
        typed_extra(
            "This session is being continued from a previous conversation.",
            "2026-09-27T10:06:20.000Z",
            "cli",
            json!({"isCompactSummary":true,"isVisibleInTranscriptOnly":true}),
        ),
        // A command it did answer: a skill.
        typed(
            "<command-message>review</command-message>\n<command-name>/review</command-name>\n<command-args>the diff</command-args>",
            "2026-09-27T10:06:40.000Z",
            "cli",
        ),
        typed_extra("Review the diff carefully.", "2026-09-27T10:06:41.000Z", "cli", json!({"isMeta":true})),
        answer("The diff looks right.", "2026-09-27T10:06:50.000Z"),
        typed_extra("Commit them.", "2026-09-27T10:07:00.000Z", "cli", json!({"promptSource":"queued"})),
        answer("Committed as \"Add migration tests\".", "2026-09-27T10:07:30.000Z"),
        typed_extra("Something the system said.", "2026-09-27T10:07:40.000Z", "cli", json!({"promptSource":"system"})),
    ]
    .join("\n")
}

fn pairs(got: &[Exchange]) -> Vec<(String, Option<String>)> {
    got.iter().map(|e| (e.prompt.clone(), e.answer.clone())).collect()
}
fn some(s: &str) -> Option<String> {
    Some(s.to_owned())
}

#[test]
fn are_the_prompts_typed_in_the_terminal_after_the_turn_each_with_its_last_words_in_answer() {
    // case: are the prompts typed in the terminal after the turn, each with its last words in answer
    let got = exchanges("claude", &claude_session(), T0);
    assert_eq!(
        pairs(&got),
        [
            ("Now add tests for the migrations.".to_owned(), some("Added three tests; all pass.")),
            ("/review the diff".to_owned(), some("The diff looks right.")),
            ("Commit them.".to_owned(), some("Committed as \"Add migration tests\".")),
        ]
    );
    assert_eq!(got[0].started, "2026-09-27T10:05:00.000Z");
    assert_eq!(got[0].finished, "2026-09-27T10:06:00.000Z");
    assert_eq!(got[0].model.as_deref(), Some("claude-opus-5-5"));
}

#[test]
fn never_take_locusts_own_print_mode_turn_or_its_answer_for_the_terminals() {
    // case: never take Locust's own print-mode turn, or its answer, for the terminal's
    let with_locust_after = [
        claude_session(),
        typed("Locust again.", "2026-09-27T10:09:00.000Z", "sdk-cli"),
        answer("Locust's answer.", "2026-09-27T10:09:30.000Z"),
    ]
    .join("\n");
    let answers: Vec<_> = exchanges("claude", &with_locust_after, T0).into_iter().map(|e| e.answer).collect();
    assert!(!answers.contains(&some("Locust's answer.")));
    assert_eq!(answers.last().cloned().flatten().as_deref(), Some("Committed as \"Add migration tests\"."));
    let later: Vec<_> = exchanges("claude", &claude_session(), "2026-09-27T10:06:55.000Z").into_iter().map(|e| e.prompt).collect();
    assert_eq!(later, ["Commit them."]);
}

#[test]
fn hold_back_an_exchange_still_going_so_no_answer_comes_back_half_written() {
    // case: hold back an exchange still going, so no answer comes back half-written
    let going = [
        claude_session(),
        typed("One more thing.", "2026-09-27T10:08:00.000Z", "cli"),
        answer_stop("Reading the file.", "2026-09-27T10:08:10.000Z", "tool_use"),
    ]
    .join("\n");
    assert!(!exchanges("claude", &going, T0).iter().any(|e| e.prompt == "One more thing."));
    let finished = [going, tool_result("2026-09-27T10:08:20.000Z"), answer("Done.", "2026-09-27T10:08:30.000Z")].join("\n");
    let last = exchanges("claude", &finished, T0).pop().unwrap();
    assert_eq!((last.prompt.as_str(), last.answer.as_deref()), ("One more thing.", Some("Done.")));
}

#[test]
fn count_an_exchange_over_when_its_turn_closed_or_the_person_stopped_it() {
    // case: count an exchange over when its turn closed, or the person stopped it
    let closed = [
        typed("Check the build.", "2026-09-27T10:05:00.000Z", "cli"),
        answer_stop("Checking.", "2026-09-27T10:05:10.000Z", "tool_use"),
        claude_line(json!({"type":"system","subtype":"turn_duration","timestamp":"2026-09-27T10:05:30.000Z"})),
    ]
    .join("\n");
    let answers: Vec<_> = exchanges("claude", &closed, T0).into_iter().map(|e| e.answer).collect();
    assert_eq!(answers, [some("Checking.")]);
    let stopped = [
        typed("Check the build.", "2026-09-27T10:05:00.000Z", "cli"),
        answer_stop("Checking.", "2026-09-27T10:05:10.000Z", "tool_use"),
        blocks("[Request interrupted by user for tool use]", "2026-09-27T10:05:20.000Z"),
    ]
    .join("\n");
    let answers: Vec<_> = exchanges("claude", &stopped, T0).into_iter().map(|e| e.answer).collect();
    assert_eq!(answers, [some("Checking.")]);
}

fn codex_line(at: &str, kind: &str, payload: Value) -> String {
    json!({"timestamp":at,"type":kind,"payload":payload}).to_string()
}
fn codex_said(at: &str, role: &str, text: &str) -> String {
    let kind = if role == "user" { "input_text" } else { "output_text" };
    codex_line(at, "response_item", json!({"type":"message","role":role,"content":[{"type":kind,"text":text}]}))
}
fn codex_rollout() -> String {
    [
        codex_said("2026-09-27T09:58:00.000Z", "user", "Who takes what?"),
        codex_said("2026-09-27T09:59:00.000Z", "assistant", "Wren takes the migrations."),
        codex_line("2026-09-27T10:05:00.000Z", "turn_context", json!({"model":"gpt-6-sol"})),
        codex_said("2026-09-27T10:05:00.300Z", "user", "# AGENTS.md instructions for C:\\work\n\n<INSTRUCTIONS>\nBe brief.\n</INSTRUCTIONS>"),
        codex_said("2026-09-27T10:05:00.500Z", "user", "<environment_context>cwd</environment_context>"),
        codex_said("2026-09-27T10:05:01.000Z", "user", "Add tests."),
        codex_said("2026-09-27T10:05:30.000Z", "assistant", "Reading first."),
        codex_said("2026-09-27T10:06:00.000Z", "assistant", "Tests added."),
        codex_line("2026-09-27T10:06:00.500Z", "event_msg", json!({"type":"task_complete"})),
    ]
    .join("\n")
}

#[test]
fn are_the_typed_turns_after_the_conversations_last_each_with_its_last_answer() {
    // case: are the typed turns after the conversation’s last, each with its last answer; injected context is not a prompt
    let got: Vec<_> = exchanges("codex", &codex_rollout(), T0).into_iter().map(|e| (e.prompt, e.answer, e.model)).collect();
    assert_eq!(got, [("Add tests.".to_owned(), some("Tests added."), some("gpt-6-sol"))]);
}

#[test]
fn hold_back_a_codex_turn_still_going() {
    // case: hold back a turn still going
    let rollout = codex_rollout();
    let lines: Vec<_> = rollout.lines().collect();
    let going = lines[..lines.len() - 1].join("\n");
    assert!(exchanges("codex", &going, T0).is_empty());
}

fn temp_root(what: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("locust-{what}-{}", crate::native::uuid_v4().unwrap()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn is_found_by_the_sessions_own_name_and_read_again_only_when_it_changed() {
    // case: is found by the session’s own name, and read again only when it changed
    let root = temp_root("transcripts");
    let claude_file = root.join("claude/projects/C--work").join(format!("{SESSION}.jsonl"));
    let codex_file = root.join("codex/sessions/2026/09/27").join(format!("rollout-2026-09-27T09-58-00-{SESSION}.jsonl"));
    std::fs::create_dir_all(root.join("claude/projects/C--other")).unwrap();
    std::fs::create_dir_all(claude_file.parent().unwrap()).unwrap();
    std::fs::create_dir_all(codex_file.parent().unwrap()).unwrap();
    std::fs::write(&claude_file, claude_session()).unwrap();
    std::fs::write(&codex_file, codex_rollout()).unwrap();
    let read = TranscriptReader::new(Some(root.join("claude")), Some(root.join("codex")));
    assert_eq!(read.read("claude", SESSION), Some(claude_session()));
    assert_eq!(read.read("claude", SESSION), None, "unchanged, not read again");
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(&claude_file).unwrap();
    write!(f, "\n{}", typed("More.", "2026-09-27T10:09:00.000Z", "cli")).unwrap();
    drop(f);
    assert!(read.read("claude", SESSION).unwrap().contains("More."));
    assert_eq!(read.read("codex", SESSION), Some(codex_rollout()));
    assert_eq!(read.read("claude", "not a session"), None);
    let _ = std::fs::remove_dir_all(root);
}

/// A conversation with one turn Locust ran on the session, a teammate who owns it, and the terminal's transcript.
struct Conversation {
    root: std::path::PathBuf,
    store: crate::store::Store,
    reader: TranscriptReader,
    _hook: crate::native::hooks::Guard,
}
impl Drop for Conversation {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Conversation {
    fn facts(&self) -> CatchUpFacts<'_> {
        CatchUpFacts { store: &self.store, live: vec![], reader: &self.reader }
    }
    fn ledger(&self) -> std::path::PathBuf {
        self.store.ledger_dir().unwrap()
    }
    /// A turn Locust ran on the session, as the ledger records one; still running without `finished`.
    fn record_turn(&self, runtime: &str, mission: &str, started: &str, finished: Option<&str>, continues_from: Option<&str>) {
        let run = format!("run_{mission}");
        let mut meta = json!({"missionId":mission,"runId":run,"prompt":"Who takes what?","runtime":runtime,"model":"opus",
            "requestedRouteId":format!("{runtime}:opus"),"resolvedRouteId":format!("{runtime}:opus"),"cliVersion":"2.1.283",
            "workspaceId":"ws_1","sandbox":"workspace-write","executionPolicyVersion":1,"createdAt":started,"mode":"ask",
            "command":{"args":["-p"]}});
        if let Some(from) = continues_from {
            meta["continuesFrom"] = json!({"missionId":from,"checkpointEpoch":1,"reason":"follow-up","runtimeThreadId":SESSION});
        }
        let mut lines = vec![json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":started,"metadata":meta})];
        let event = |seq: usize, at: &str, kind: &str, payload: Value| {
            json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":seq+1,"occurredAt":at,"event":{"id":format!("{run}:{seq}"),
                "runId":run,"missionId":mission,"sequence":seq,"occurredAt":at,"sourceAdapter":runtime,"runtimeThreadId":SESSION,"type":kind,"payload":payload}})
        };
        lines.push(event(1, started, "run.started", json!({"runtimeThreadId":SESSION,"evidence":{"redacted":true}})));
        if let Some(finished) = finished {
            lines.push(event(2, finished, "run.completed", json!({"runtimeThreadId":SESSION,"process":{"exitCode":0,"signal":null,"stderr":"",
                "stderrTruncated":false,"recordCount":2,"inputDeliveryFailed":false,"outputLimitExceeded":false,"oversizedRecordsDropped":0,
                "forcedTerminationAttempted":false,"terminationUnconfirmed":false,"startedAt":started,"finishedAt":finished}})));
        }
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(self.ledger().join(format!("{mission}.jsonl")), body).unwrap();
    }
    /// The terminal's turns in the ledger, by their exchange number: (id, header metadata, records).
    fn terminal_turns(&self) -> Vec<(String, Value, Vec<Value>)> {
        let mut turns: Vec<_> = std::fs::read_dir(self.ledger())
            .unwrap()
            .filter_map(|e| {
                let path = e.ok()?.path();
                let id = path.file_stem()?.to_str()?.to_owned();
                let (meta, rows) = mission(&id).ok()?;
                (meta["startedBy"]["kind"] == "terminal").then_some((id, meta, rows))
            })
            .collect();
        turns.sort_by_key(|(_, meta, _)| meta["startedBy"]["exchange"].as_u64());
        turns
    }
}

fn with_conversation(runtime: &str, transcript: &str) -> Conversation {
    let root = temp_root("catch-up");
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let hook = crate::native::hooks::install(store.clone(), None);
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren"}],"missionOwners":{"mission_first":"tm_wren"},"settings":{}}),
        )
        .unwrap();
    let (claude, codex) = (root.join("claude"), root.join("codex"));
    let file = if runtime == "claude" {
        claude.join("projects/C--work").join(format!("{SESSION}.jsonl"))
    } else {
        codex.join("sessions/2026/09/27").join(format!("rollout-2026-09-27T09-58-00-{SESSION}.jsonl"))
    };
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, transcript).unwrap();
    let conversation = Conversation { root, store, reader: TranscriptReader::new(Some(claude), Some(codex)), _hook: hook };
    conversation.record_turn(runtime, "mission_first", "2026-09-27T09:58:00.000Z", Some(T0), None);
    conversation
}

fn nothing(latest: &str) -> CaughtUp {
    CaughtUp { imported: 0, latest: latest.to_owned(), owners: Default::default() }
}

#[test]
fn takes_a_prompt_longer_than_the_ledger_keeps_bounded_and_the_exchanges_after_it_too() {
    // case: takes a prompt longer than the ledger keeps, bounded, and the exchanges after it too (2026-10-10 sweep)
    let long = [
        typed(&"x".repeat(9_000), "2026-09-27T10:05:00.000Z", "cli"),
        answer("Read it.", "2026-09-27T10:05:30.000Z"),
        typed("And this one.", "2026-09-27T10:06:00.000Z", "cli"),
        answer("Done.", "2026-09-27T10:06:30.000Z"),
    ]
    .join("\n");
    let c = with_conversation("claude", &long);
    assert_eq!(catch_up_with("mission_first", &c.facts()).unwrap().imported, 2);
    let turns = c.terminal_turns();
    let prompt = turns[0].1["prompt"].as_str().unwrap();
    assert_eq!(prompt.chars().count(), 8_000, "bounded to what the ledger keeps");
    assert!(prompt.ends_with('…'));
    assert_eq!(turns[1].1["prompt"], "And this one.");
}

#[test]
fn records_each_terminal_exchange_as_the_next_turn_owned_by_its_teammate_readable_by_the_ledger() {
    // case: records each terminal exchange as the next turn of the conversation, owned by its teammate, readable by the ledger
    let c = with_conversation("claude", &claude_session());
    let result = catch_up_with("mission_first", &c.facts()).unwrap();
    let turns = c.terminal_turns();
    let ids: Vec<_> = turns.iter().map(|(id, _, _)| id.clone()).collect();
    assert_eq!(result.imported, 3);
    assert_eq!(ids.len(), 3);
    assert_eq!(result.latest, ids[2]);
    assert_eq!(result.owners, ids.iter().map(|id| (id.clone(), "tm_wren".to_owned())).collect());
    let (first_id, first, first_rows) = &turns[0];
    let recovered = crate::ledger::read_ledger(&c.ledger().join(format!("{first_id}.jsonl"))).unwrap();
    assert!(recovered.issues.is_empty(), "{:?}", recovered.issues);
    assert_eq!(first_rows.last().unwrap()["event"]["type"], "run.completed", "completed");
    assert_eq!(first["prompt"], "Now add tests for the migrations.");
    assert_eq!(first["runtime"], "claude");
    assert_eq!(first["startedBy"], json!({"kind":"terminal","exchange":1}));
    assert_eq!(first["continuesFrom"]["missionId"], "mission_first");
    assert_eq!(first["continuesFrom"]["reason"], "follow-up");
    assert_eq!(first["continuesFrom"]["runtimeThreadId"], SESSION);
    assert!(first.get("command").is_none() && first.get("mode").is_none());
    let (last_id, last, last_rows) = &turns[2];
    assert!(crate::ledger::read_ledger(&c.ledger().join(format!("{last_id}.jsonl"))).unwrap().issues.is_empty());
    assert_eq!(last["startedBy"], json!({"kind":"terminal","exchange":3}));
    assert_eq!(last["continuesFrom"]["missionId"], ids[1].as_str());
    let said = last_rows.iter().find(|r| r["event"]["type"] == "message.delta").unwrap();
    assert_eq!(said["event"]["payload"]["text"], "Committed as \"Add migration tests\".");
    let read = crate::ledger::read_transcript(c.ledger().join(format!("{last_id}.jsonl")));
    assert!(read.items.iter().any(|i| matches!(i, crate::ledger::Item::Assistant { text, .. } if text == "Committed as \"Add migration tests\".")));
    // It continues the same session, so a follow-up here resumes it.
    assert_eq!(session(last_rows).as_deref(), Some(SESSION));
    for id in &ids {
        assert_eq!(crate::people::owner_of(&c.store, id).as_deref(), Some("tm_wren"), "{id} is Wren's");
    }
    assert_eq!(crate::native::real_tip("mission_first").unwrap(), ids[2]);
}

#[test]
fn brings_nothing_back_twice_whichever_turn_it_is_asked_from() {
    // case: brings nothing back twice, whichever turn it is asked from
    let c = with_conversation("claude", &claude_session());
    let first = catch_up_with("mission_first", &c.facts()).unwrap();
    assert_eq!(first.imported, 3);
    // A fresh reader each time, so it is the record that holds them back, not an unchanged file.
    let again = TranscriptReader::new(Some(c.root.join("claude")), None);
    let facts = CatchUpFacts { store: &c.store, live: vec![], reader: &again };
    assert_eq!(catch_up_with("mission_first", &facts).unwrap(), nothing(&first.latest));
    let fresh = TranscriptReader::new(Some(c.root.join("claude")), None);
    let facts = CatchUpFacts { store: &c.store, live: vec![], reader: &fresh };
    assert_eq!(catch_up_with(&first.latest, &facts).unwrap(), nothing(&first.latest));
    assert_eq!(c.terminal_turns().len(), 3);
}

#[test]
fn brings_nothing_back_twice_when_asked_twice_at_once() {
    // case: brings nothing back twice when asked twice at once
    let c = with_conversation("claude", &claude_session());
    let facts = c.facts();
    let mut both: Vec<_> = std::thread::scope(|scope| {
        let a = scope.spawn(|| catch_up_with("mission_first", &facts).unwrap().imported);
        let b = scope.spawn(|| catch_up_with("mission_first", &facts).unwrap().imported);
        vec![a.join().unwrap(), b.join().unwrap()]
    });
    both.sort();
    assert_eq!(both, [0, 3]);
    assert_eq!(c.terminal_turns().len(), 3);
}

#[test]
fn leaves_a_running_conversation_alone() {
    // case: leaves a running conversation alone
    let c = with_conversation("claude", &claude_session());
    let facts = CatchUpFacts { live: vec!["mission_first".into()], ..c.facts() };
    assert_eq!(catch_up_with("mission_first", &facts).unwrap(), nothing("mission_first"));
    assert!(c.terminal_turns().is_empty());
}

#[test]
fn reads_codexs_rollout_the_same_way() {
    // case: reads Codex's rollout the same way
    let c = with_conversation("codex", &codex_rollout());
    let result = catch_up_with("mission_first", &c.facts()).unwrap();
    assert_eq!(result.imported, 1);
    let (meta, _) = mission(&result.latest).unwrap();
    assert!(crate::ledger::read_ledger(&c.ledger().join(format!("{}.jsonl", result.latest))).unwrap().issues.is_empty());
    assert_eq!(meta["prompt"], "Add tests.");
    let held = c.store.seeded_json("terminal-imports.json", json!({})).unwrap();
    assert_eq!(held[SESSION]["through"], "2026-09-27T10:06:00.000Z");
}

#[test]
fn never_takes_locusts_own_later_codex_turn_for_the_terminals_when_an_older_turn_is_named() {
    // case: never takes Locust's own later Codex turn for the terminal's, when an older turn is named
    // The rollout's "Add tests." was Locust's own follow-up: a window that still names the first turn must not bring it
    // back as the terminal's.
    let c = with_conversation("codex", &codex_rollout());
    c.record_turn("codex", "mission_second", "2026-09-27T10:05:00.000Z", Some("2026-09-27T10:06:00.000Z"), Some("mission_first"));
    assert_eq!(catch_up_with("mission_first", &c.facts()).unwrap(), nothing("mission_first"));
}

#[test]
fn waits_while_the_newest_turn_is_still_running_even_when_an_older_one_is_named() {
    // case: waits while the newest turn is still running, even when an older one is named
    let c = with_conversation("codex", &codex_rollout());
    c.record_turn("codex", "mission_second", "2026-09-27T10:05:00.000Z", None, Some("mission_first"));
    let facts = CatchUpFacts { live: vec!["mission_second".into()], ..c.facts() };
    assert_eq!(catch_up_with("mission_first", &facts).unwrap(), nothing("mission_first"));
}

#[test]
fn does_not_bring_back_a_terminal_turn_the_person_deleted() {
    // case: does not bring back a terminal turn the person deleted
    let c = with_conversation("claude", &claude_session());
    catch_up_with("mission_first", &c.facts()).unwrap();
    for (id, _, _) in c.terminal_turns() {
        std::fs::remove_file(c.ledger().join(format!("{id}.jsonl"))).unwrap();
    }
    let fresh = TranscriptReader::new(Some(c.root.join("claude")), None);
    let facts = CatchUpFacts { store: &c.store, live: vec![], reader: &fresh };
    assert_eq!(catch_up_with("mission_first", &facts).unwrap(), nothing("mission_first"));
    assert!(c.terminal_turns().is_empty());
}

#[test]
fn is_found_from_any_turn_of_it_the_latest_made_where_it_branches() {
    // case: is found from any turn of it, the latest-made where it branches
    let c = with_conversation("claude", "");
    c.record_turn("claude", "mission_second", "2026-09-27T10:05:00.000Z", Some("2026-09-27T10:06:00.000Z"), Some("mission_first"));
    c.record_turn("claude", "mission_third", "2026-09-27T10:07:00.000Z", Some("2026-09-27T10:08:00.000Z"), Some("mission_first"));
    c.record_turn("claude", "mission_fourth", "2026-09-27T10:09:00.000Z", Some("2026-09-27T10:10:00.000Z"), Some("mission_third"));
    assert_eq!(crate::native::real_tip("mission_first").unwrap(), "mission_fourth");
    assert_eq!(crate::native::real_tip("mission_second").unwrap(), "mission_second");
    assert_eq!(crate::native::real_tip("mission_fourth").unwrap(), "mission_fourth");
}

// A session's name is 8 to 80 letters, digits or dashes; anything else is never looked up, even when a file has that name.
#[test]
fn a_name_too_short_or_too_long_is_never_looked_for() {
    let root = temp_root("transcript-names");
    let dir = root.join("claude/projects/C--work");
    std::fs::create_dir_all(&dir).unwrap();
    let long = "a".repeat(81);
    for name in ["abcdefg", long.as_str(), "abcdefgh"] {
        std::fs::write(dir.join(format!("{name}.jsonl")), claude_session()).unwrap();
    }
    let read = TranscriptReader::new(Some(root.join("claude")), None);
    assert_eq!(read.read("claude", "abcdefg"), None, "seven letters");
    assert_eq!(read.read("claude", &long), None, "eighty-one letters");
    assert_eq!(read.read("claude", "abcdefgh"), Some(claude_session()), "eight letters is a name");
    let _ = std::fs::remove_dir_all(root);
}

// A link in Claude Code's folders is never followed: the file read is always one Claude Code wrote there.
#[test]
fn a_transcript_that_is_a_link_is_not_read() {
    let root = temp_root("transcript-link");
    let dir = root.join("claude/projects/C--work");
    std::fs::create_dir_all(&dir).unwrap();
    let elsewhere = root.join("elsewhere.jsonl");
    std::fs::write(&elsewhere, claude_session()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir.join(format!("{SESSION}.jsonl"))).unwrap();
    let read = TranscriptReader::new(Some(root.join("claude")), None);
    assert_eq!(read.read("claude", SESSION), None);
    let _ = std::fs::remove_dir_all(root);
}
