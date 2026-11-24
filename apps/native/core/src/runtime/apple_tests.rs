//! The apple runtime: Apple's on-device model through `locust-apple-model` (runtime/apple.rs). A synthetic helper
//! (scripts/fixtures/apple-model.py) stands in for the Swift one, which needs Apple Intelligence; the one test that asks
//! the real model is ignored unless asked for, and needs this Mac's model to be available.
use super::{
    Event, Runtime, Turn,
    apple::{
        self, Apple, AppleNormalizer, Probe, failure_words, read_probe, refuse_mode, unavailable_words,
    },
};
use crate::{approval::Mode, ledger_write::LedgerWriter, store::Store};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            ".apple-rt-{}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            crate::invariants::unique_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fake() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).join("scripts/fixtures/apple-model.py")
}

fn lines(normal: &mut AppleNormalizer, lines: &[Value], exit: Option<i32>) -> Vec<Event> {
    let mut out: Vec<Event> = lines.iter().flat_map(|l| normal.accept(l)).collect();
    out.extend(normal.finish(false, exit));
    out
}

fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

fn failure(events: &[Event]) -> String {
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    last.payload["message"].as_str().unwrap().to_owned()
}

fn turn(folder: PathBuf, prompt: &str, mode: Mode) -> Turn {
    Turn {
        prompt: prompt.into(),
        folder,
        model: Some(apple::APPLE_MODEL.into()),
        effort: None,
        resume: None,
        mode,
        approvals: None,
        mission_id: "apple_turn".into(),
        run_id: "run_apple_turn".into(),
    }
}

/// Drives a runtime to its last event, writing each into a ledger, within the load allowance.
fn drive(rt: &mut dyn Runtime, writer: &mut LedgerWriter) -> Vec<Event> {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the apple run did not end");
        if let Some(e) = rt.next_event().unwrap() {
            let terminal = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            writer.append(&e.kind, e.payload.clone(), e.thread.as_deref()).unwrap();
            events.push(e);
            if terminal {
                return events;
            }
        }
    }
}

/// Locust's own reader takes the ledger back: the turn ended in `phase`, and its answer reads as `answer`. (Electron's
/// reader is not asked: its ledger schema lists the runtimes it knows, and "apple" is not one until Electron runs it.)
fn reads_back(store: &Store, id: &str, phase: &str, answer: Option<&str>) {
    let mut session = crate::ledger::Session::new(store.ledger_dir().unwrap(), id.into());
    let snap = session.poll().unwrap();
    assert!(!snap.running);
    assert_eq!(session.tip_phase(), phase);
    assert_eq!(snap.turn_metas.last().unwrap().runtime, "apple");
    if let Some(answer) = answer {
        assert!(
            snap.items.iter().any(|i| matches!(i, crate::ledger::Item::Assistant { text, .. } if text == answer)),
            "no answer {answer:?} in {:?}",
            snap.items
        );
    }
}

// ---- reading its lines ----------------------------------------------------------------------------------------------

#[test]
fn an_answer_streams_as_it_comes_then_is_kept_whole_and_the_turn_completes() {
    let mut normal = AppleNormalizer::default();
    let events = lines(
        &mut normal,
        &[json!({"type":"delta","text":"Red, "}), json!({"type":"delta","text":"blue."}), json!({"type":"done","text":"Red, blue."})],
        Some(0),
    );
    assert_eq!(kinds(&events), ["step.started", "message.delta", "message.delta", "message.delta", "step.completed", "run.completed"]);
    assert_eq!(events[1].payload["operation"], "append");
    assert_eq!(events[1].payload["text"], "Red, ");
    assert_eq!(events[2].payload["text"], "blue.");
    assert_eq!(events[3].payload["operation"], "replace");
    assert_eq!(events[3].payload["text"], "Red, blue.");
    assert_eq!(events[3].payload["final"], true);
    // It keeps no session: every follow-up starts cold and is told what was said.
    assert!(events.iter().all(|e| e.thread.is_none()));
}

#[test]
fn an_answer_that_rewrites_its_earlier_words_replaces_them() {
    let mut normal = AppleNormalizer::default();
    let events = lines(
        &mut normal,
        &[json!({"type":"delta","text":"Red, blue"}), json!({"type":"replace","text":"Red, yellow, blue."}), json!({"type":"done","text":""})],
        Some(0),
    );
    assert_eq!(events[2].payload["operation"], "replace");
    assert_eq!(events[2].payload["final"], false);
    // A done with no words of its own keeps what was said.
    assert_eq!(events[3].payload["text"], "Red, yellow, blue.");
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn says_in_words_why_the_model_did_not_answer() {
    for (kind, reason, words) in [
        ("contextSizeExceeded", None, "more than Apple's on-device model can read at once"),
        ("guardrailViolation", None, "its safety rules stopped it"),
        ("refusal", None, "declined to answer this"),
        ("unsupportedLanguageOrLocale", None, "does not read this language yet"),
        ("rateLimited", None, "is busy"),
        ("assetsUnavailable", None, "still downloading"),
        ("unavailable", Some("appleIntelligenceNotEnabled"), "Apple Intelligence is turned off on this Mac"),
        ("whatever", None, "could not answer"),
    ] {
        let mut normal = AppleNormalizer::default();
        let mut line = json!({"type":"error","kind":kind,"message":"synthetic"});
        if let Some(r) = reason {
            line["reason"] = json!(r);
        }
        let said = failure(&lines(&mut normal, &[line], Some(1)));
        assert!(said.contains(words), "{kind}: {said}");
        assert_eq!(said, failure_words(kind, reason));
    }
}

#[test]
fn a_helper_that_ends_without_answering_is_a_failure_never_a_finished_turn() {
    let mut normal = AppleNormalizer::default();
    assert_eq!(failure(&lines(&mut normal, &[], Some(3))), "Apple's on-device model stopped before it answered.");
    let mut normal = AppleNormalizer::default();
    let said = failure(&lines(&mut normal, &[json!({"type":"done","text":"ok"})], Some(5)));
    assert!(said.contains("exited with code 5"), "{said}");
}

#[test]
fn a_stopped_turn_is_cancelled() {
    let mut normal = AppleNormalizer::default();
    normal.accept(&json!({"type":"delta","text":"Think"}));
    let end = normal.finish(true, None);
    assert_eq!(kinds(&end), ["run.cancelled"]);
    assert!(normal.finish(true, None).is_empty());
    assert!(normal.accept(&json!({"type":"delta","text":"more"})).is_empty());
}

// ---- whether it is there --------------------------------------------------------------------------------------------

#[test]
fn reads_the_probe_and_takes_anything_else_as_not_there() {
    assert_eq!(read_probe("{\"available\":true,\"contextSize\":8192}\n"), Probe::Available { context: 8192 });
    assert_eq!(
        read_probe("{\"available\":false,\"reason\":\"modelNotReady\"}"),
        Probe::Unavailable { reason: "modelNotReady".into() }
    );
    assert_eq!(read_probe("dyld: Library not loaded: FoundationModels"), Probe::Unavailable { reason: "unknown".into() });
    assert_eq!(read_probe(""), Probe::Unavailable { reason: "unknown".into() });
}

#[test]
fn says_what_to_do_for_each_reason_it_is_not_there() {
    assert!(unavailable_words("appleIntelligenceNotEnabled").contains("System Settings, under Apple Intelligence & Siri"));
    assert!(unavailable_words("modelNotReady").contains("still downloading"));
    assert_eq!(unavailable_words("deviceNotEligible"), "This Mac cannot run Apple Intelligence.");
    assert_eq!(unavailable_words("unknown"), "Apple's on-device model is not available on this Mac.");
}

#[test]
fn runs_in_ask_only_because_it_has_no_tools() {
    // Plan runs wherever Ask does (modeRunsOn): it changes nothing, and this model can change nothing.
    assert_eq!(refuse_mode(Mode::Ask), None);
    assert_eq!(refuse_mode(Mode::Plan), None);
    for mode in [Mode::AcceptEdits, Mode::ApproveEach, Mode::Auto] {
        assert!(refuse_mode(mode).is_some_and(|w| w.contains("Ask mode only")), "{mode:?}");
        assert!(apple::command(mode).is_err(), "{mode:?}");
    }
    assert!(crate::mode_facts::runs_on("ask", "apple"));
    assert!(crate::mode_facts::runs_on("plan", "apple"));
    for mode in ["accept-edits", "approve-each", "auto"] {
        assert!(!crate::mode_facts::runs_on(mode, "apple"), "{mode}");
    }
}

#[test]
fn is_offered_as_one_model_with_no_account_default_only_where_it_answers() {
    let none = crate::models::catalog(&crate::models::Listed::default());
    assert!(none.iter().all(|m| m["runtime"] != "apple"));
    let models = crate::models::catalog(&crate::models::Listed { apple: true, ..Default::default() });
    let apple: Vec<&Value> = models.iter().filter(|m| m["runtime"] == "apple").collect();
    assert_eq!(apple.len(), 1, "{apple:?}");
    assert_eq!(apple[0]["id"], "apple-on-device");
    assert_eq!(apple[0]["displayName"], "On-device model");
}

#[test]
fn says_it_may_not_touch_files_commands_or_the_web() {
    let may = crate::inspector::what_it_may_do("apple", "read-only", Some("ask"));
    assert!(may.rows.iter().all(|r| r.verdict == "deny"), "{:?}", may.rows.iter().map(|r| &r.text).collect::<Vec<_>>());
    let said: Vec<&str> = may.rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(said, ["reading or changing your files", "running commands", "searching the web or opening web pages"]);
    assert!(may.rest.contains("nothing leaves it"));
    assert_eq!(crate::carry_on::display_name("apple"), "Apple Intelligence");
}

// ---- the process ----------------------------------------------------------------------------------------------------

#[test]
fn sends_the_prompt_on_its_input_and_writes_the_answer_into_the_ledger() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let saw = temp.0.join("saw.jsonl");
    let mut command = Command::new("python3");
    command.arg(fake()).arg("echo").env("LOCUST_FAKE_APPLE_SAW", &saw).current_dir(&temp.0);
    let mut rt = Apple::spawn(&mut command).unwrap();
    let t = turn(temp.0.clone(), "What is a rebase?", Mode::Ask);
    let mut writer =
        LedgerWriter::create_runtime(&store, "apple_turn", &t.prompt, apple::APPLE_MODEL, "fake-apple", "apple", &[], None)
            .unwrap();
    rt.start(&t).unwrap();
    let events = drive(&mut rt, &mut writer);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(std::fs::read_to_string(&saw).unwrap().trim(), "\"What is a rebase?\"");
    let last_words = events.iter().rev().find(|e| e.kind == "message.delta").unwrap();
    assert_eq!(last_words.payload["text"], "You said: What is a rebase?");
    reads_back(&store, "apple_turn", "completed", Some("You said: What is a rebase?"));
}

#[test]
fn a_refusal_from_the_helper_ends_the_turn_with_its_words() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command.arg(fake()).arg("error:contextSizeExceeded").current_dir(&temp.0);
    let mut rt = Apple::spawn(&mut command).unwrap();
    let t = turn(temp.0.clone(), "A very long brief", Mode::Ask);
    let mut writer =
        LedgerWriter::create_runtime(&store, "apple_turn", &t.prompt, apple::APPLE_MODEL, "fake-apple", "apple", &[], None)
            .unwrap();
    rt.start(&t).unwrap();
    let said = failure(&drive(&mut rt, &mut writer));
    assert_eq!(said, failure_words("contextSizeExceeded", None));
    reads_back(&store, "apple_turn", "failed", None);
}

#[test]
fn stopping_it_mid_answer_cancels_the_turn() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command.arg(fake()).arg("slow").current_dir(&temp.0);
    let mut rt = Apple::spawn(&mut command).unwrap();
    let t = turn(temp.0.clone(), "Take your time", Mode::Ask);
    let mut writer =
        LedgerWriter::create_runtime(&store, "apple_turn", &t.prompt, apple::APPLE_MODEL, "fake-apple", "apple", &[], None)
            .unwrap();
    rt.start(&t).unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    loop {
        assert!(Instant::now() < deadline, "the first words never came");
        if let Some(e) = rt.next_event().unwrap() {
            writer.append(&e.kind, e.payload.clone(), None).unwrap();
            if e.kind == "message.delta" {
                break;
            }
        }
    }
    rt.stop().unwrap();
    let events = drive(&mut rt, &mut writer);
    assert_eq!(events.last().unwrap().kind, "run.cancelled");
}

#[test]
fn a_mode_with_tools_is_refused_before_anything_is_sent() {
    let temp = Temp::new();
    let mut command = Command::new("python3");
    command.arg(fake()).arg("echo").current_dir(&temp.0);
    let mut rt = Apple::spawn(&mut command).unwrap();
    let refused = rt.start(&turn(temp.0.clone(), "Fix the bug", Mode::AcceptEdits)).unwrap_err();
    assert!(refused.to_string().contains("Ask mode only"), "{refused}");
}

// ---- as a teammate --------------------------------------------------------------------------------------------------

#[test]
fn a_teammate_on_apple_answers_and_a_follow_up_is_told_what_was_said() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let saw = t.0.join("saw.jsonl");
    // `env` hands the fake where to write what it was sent; the hook replaces the program the run would start.
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "/usr/bin/env".into(),
            vec![format!("LOCUST_FAKE_APPLE_SAW={}", saw.display()), "python3".into(), fake().to_string_lossy().into_owned(), "echo".into()],
        )),
    );
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"a1","name":"Pippin","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-17T12:00:00Z","folder":t.0,"route":{"runtime":"apple","model":"apple-on-device","mode":"ask"}}]}).to_string(),
    )
    .unwrap();
    assert!(crate::native::supported(&team[0]));
    let first = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Name a primary colour".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    crate::load_waits::wait_for("the first Apple turn to finish", || !crate::native::owns(&first.conversation_id));
    let ledger = std::fs::read_to_string(store.ledger_dir().unwrap().join(format!("{}.jsonl", first.conversation_id))).unwrap();
    assert!(ledger.contains("run.completed"), "{ledger}");
    let second = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: Some(first.conversation_id.clone()),
        prompt: "And another".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    crate::load_waits::wait_for("the second Apple turn to finish", || !crate::native::owns(&second.conversation_id));
    let prompts: Vec<String> =
        std::fs::read_to_string(&saw).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert!(prompts[0].contains("Name a primary colour"));
    // No session to resume: the follow-up quotes the first turn, question and answer.
    assert!(prompts[1].contains("could not be resumed"), "{}", prompts[1]);
    assert!(prompts[1].contains("Name a primary colour"), "{}", prompts[1]);
    assert!(prompts[1].contains("You said:"), "{}", prompts[1]);
    assert!(prompts[1].contains("And another"), "{}", prompts[1]);
    // Its own short brief: who it is and what it cannot do, never the tag rules a coding agent is taught.
    for prompt in &prompts {
        assert!(prompt.starts_with("You are Pippin, a teammate in Locust (Test)."), "{prompt}");
        assert!(prompt.contains("You can only talk"), "{prompt}");
        assert!(!prompt.contains("<locust-"), "{prompt}");
    }
}

#[test]
fn its_brief_names_the_teammate_says_it_only_talks_and_keeps_the_persons_instructions() {
    let said = apple::brief("Pippin", "Researcher", Some("  Answer in French.  "), "What is a tag?");
    assert!(said.starts_with("You are Pippin, a teammate in Locust (Researcher). You run on Apple's on-device model"));
    assert!(said.contains("cannot read or change files, run commands, or open web pages"));
    assert!(said.contains("Your instructions from the person:\nAnswer in French."));
    assert!(said.ends_with("\n\nWhat is a tag?"));
    assert!(!apple::brief("Pippin", " ", None, "Hi").contains("Your instructions"));
    assert!(apple::brief("Pippin", " ", None, "Hi").starts_with("You are Pippin, a teammate in Locust. "));
}

// ---- the real model -------------------------------------------------------------------------------------------------

/// Asks this Mac's own model. Run with `--run-ignored only` and LOCUST_REAL_APPLE=1; needs Apple Intelligence on.
#[test]
#[ignore]
fn real_apple_one_line() {
    assert_eq!(std::env::var("LOCUST_REAL_APPLE").ok().as_deref(), Some("1"), "set LOCUST_REAL_APPLE=1 to run this real test");
    assert!(matches!(apple::probe(), Probe::Available { .. }), "Apple's model is not available here: {:?}", apple::probe());
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = apple::command(Mode::Ask).unwrap();
    let mut rt = Apple::spawn(&mut command).unwrap();
    let t = turn(temp.0.clone(), "Reply with exactly the word: ok", Mode::Ask);
    let mut writer =
        LedgerWriter::create_runtime(&store, "apple_turn", &t.prompt, apple::APPLE_MODEL, "apple", "apple", &[], None).unwrap();
    rt.start(&t).unwrap();
    let events = drive(&mut rt, &mut writer);
    assert_eq!(events.last().unwrap().kind, "run.completed", "{:?}", events.last().unwrap().payload);
    let said = events.iter().rev().find(|e| e.kind == "message.delta").unwrap().payload["text"].as_str().unwrap().to_lowercase();
    assert!(said.contains("ok"), "{said}");
    reads_back(&store, "apple_turn", "completed", None);
}

/// A real teammate turn, brief and all, then a follow-up told what was said: both fit in what this Mac's model reads.
/// Run with `--run-ignored only` and LOCUST_REAL_APPLE=1.
#[test]
#[ignore]
fn real_apple_teammate_turn_and_follow_up_fit() {
    assert_eq!(std::env::var("LOCUST_REAL_APPLE").ok().as_deref(), Some("1"), "set LOCUST_REAL_APPLE=1 to run this real test");
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(store.clone(), None);
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"a1","name":"Pippin","role":"Research & Briefs","roleTitle":"Researcher","hue":"lime","createdAt":"2026-11-17T12:00:00Z","folder":t.0,"route":{"runtime":"apple","model":"apple-on-device","mode":"ask"}}]}).to_string(),
    )
    .unwrap();
    let mut conversation = None;
    for prompt in ["In one sentence, what is a git branch?", "And in one sentence, how is that different from a tag?"] {
        let started = crate::native::send(crate::native::Send {
            teammate: team[0].clone(),
            conversation: conversation.clone(),
            prompt: prompt.into(),
            model: None,
            effort: None,
        })
        .unwrap();
        crate::load_waits::wait_for("the real Apple turn to finish", || !crate::native::owns(&started.conversation_id));
        let mut session = crate::ledger::Session::new(store.ledger_dir().unwrap(), started.conversation_id.clone());
        let snap = session.poll().unwrap();
        let answer = snap.items.iter().rev().find_map(|i| match i {
            crate::ledger::Item::Assistant { text, .. } => Some(text.clone()),
            _ => None,
        });
        eprintln!("{prompt}\n  -> {answer:?}");
        assert_eq!(session.tip_phase(), "completed", "{:?}", snap.items);
        conversation = Some(started.conversation_id);
    }
}

#[test]
fn has_a_row_among_the_agents_ready_or_saying_why_not_and_none_where_it_cannot_be_asked() {
    let ready = crate::agents::apple_found(&Probe::Available { context: 8192 }).unwrap();
    assert_eq!((ready.id.as_str(), ready.display_name.as_str()), ("apple", "Apple Intelligence"));
    assert!(ready.installed && ready.ready);
    assert_eq!(ready.status, crate::agents::Probe::Ready);
    assert_eq!(ready.auth, crate::agents::Auth::NotApplicable);
    let off = crate::agents::apple_found(&Probe::Unavailable { reason: "appleIntelligenceNotEnabled".into() }).unwrap();
    assert!(!off.ready);
    assert_eq!(off.status, crate::agents::Probe::Unsupported);
    assert_eq!(off.unsupported_note.as_deref(), Some(unavailable_words("appleIntelligenceNotEnabled")));
    assert!(crate::agents::apple_found(&Probe::Unavailable { reason: "unknown".into() }).is_none());
    assert_eq!(crate::agents::integration_of("apple"), crate::agents::Integration::Live);
}

#[test]
fn burst_one_under_cap_is_all_kept() {
    assert_eq!(apple::MAX_QUEUED_RECORDS, 2_048);
    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import json, sys\nline = json.dumps({'type':'delta','text':'a'})\nsys.stdout.write((line + '\\n') * 2047)\nsys.stdout.flush()\nsys.stdin.readline()",
    ]);
    let mut rt = Apple::spawn(&mut command).unwrap();
    // Let the producer fill the channel completely before the consumer drains a single record.
    std::thread::sleep(std::time::Duration::from_millis(200));
    let mut kept = 0usize;
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while kept < 2_047 {
        assert!(Instant::now() < deadline, "timed out after {kept} deltas");
        if let Some(ev) = rt.next_event().unwrap() {
            assert_ne!(ev.kind, "run.failed", "unexpected failure after {kept}: {:?}", ev.payload);
            if ev.kind == "message.delta" {
                kept += 1;
            }
        }
    }
    assert_eq!(kept, 2_047);
}

#[test]
fn burst_one_over_cap_ends_the_run_with_overflow_words() {
    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import json, sys\nline = json.dumps({'type':'delta','text':'a'})\nsys.stdout.write((line + '\\n') * 2049)\nsys.stdout.flush()\nsys.stdin.readline()",
    ]);
    let mut rt = Apple::spawn(&mut command).unwrap();
    // Let the 2049-record burst overflow the 2048-slot channel before draining.
    std::thread::sleep(std::time::Duration::from_millis(250));
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "2049-record burst did not fail");
        if let Some(ev) = rt.next_event().unwrap()
            && ev.kind == "run.failed"
        {
            assert_eq!(
                ev.payload["message"],
                "Apple's on-device model sent more than Locust could record"
            );
            break;
        }
    }
}
