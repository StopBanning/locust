//! Electron's `main/approval-channel.test.ts`: describe what a Codex app server asked, wait for a person, and match the
//! answer to the question, without a mission (`approval.rs`). Electron's `withFileChanges`, which rewrites a fileChange
//! tool row from the item's changes, is `approval::with_file_changes`, tested on a Codex app server's wire in
//! `runtime/codex_file_change_rows_tests.rs`.

use crate::approval::{approval_kind_for, describe_codex, protocol_decision_for, questions_of, Answer, Channel, Decision, Kind, QuestionOption, Request};
use crate::approval_patch::{FileChangeKind, FileChangeRecord};
use crate::store::Store;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A channel, with a temporary store for the saved rules it reads before raising a card.
struct Host {
    channel: Arc<Channel>,
    root: PathBuf,
    _hook: crate::native::hooks::Guard,
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn channel() -> Host {
    let root = std::env::temp_dir().join(format!("locust-approval-channel-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let hook = crate::native::hooks::install(Store::new(root.join("native"), root.join("electron")).unwrap(), None);
    Host { channel: Channel::new(), root, _hook: hook }
}

type Changes = Arc<Mutex<HashMap<String, Vec<FileChangeRecord>>>>;

/// Ask, on a thread of its own, and hand back what the runtime will be answered.
fn ask(host: &Host, mission: &str, method: &str, params: Value, changes: Option<Changes>) -> JoinHandle<Result<Value, String>> {
    let (channel, mission, method) = (host.channel.clone(), mission.to_owned(), method.to_owned());
    std::thread::spawn(move || {
        channel.ask_codex_with_changes(&format!("run_{mission}"), &mission, "codex", &method, &params, json!(1), changes.as_deref(), &|| false)
    })
}

/// What an ask was answered with, within the load wait: a request that is never answered fails here instead of hanging.
fn answered(handle: JoinHandle<Result<Value, String>>) -> Result<Value, String> {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the request was never answered");
        std::thread::sleep(Duration::from_millis(5));
    }
    handle.join().unwrap()
}

fn cards(host: &Host, n: usize) -> Vec<Request> {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    loop {
        let waiting = host.channel.waiting();
        if waiting.len() >= n {
            return waiting;
        }
        assert!(Instant::now() < deadline, "{} of {n} cards were raised", waiting.len());
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The decision a runtime was answered with. On a run's end Rust also sends why, which Electron does not (see the notes).
fn decision_of(answered: Result<Value, String>) -> String {
    answered.unwrap()["decision"].as_str().unwrap_or_default().to_owned()
}

fn decide(host: &Host, card: &Request, decision: Decision) -> Result<crate::approval::Recorded, String> {
    host.channel.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision, reason: None })
}

const COMMAND: &str = "item/commandExecution/requestApproval";
const FILE_CHANGE: &str = "item/fileChange/requestApproval";
const QUESTION: &str = "item/tool/requestUserInput";

// ---- approval descriptions ----

#[test]
fn recognizes_the_three_protocol_methods_and_nothing_else() {
    assert_eq!(approval_kind_for(COMMAND), Some(Kind::Command));
    assert_eq!(approval_kind_for(FILE_CHANGE), Some(Kind::FileChange));
    assert_eq!(approval_kind_for(QUESTION), Some(Kind::Question));
    assert_eq!(approval_kind_for("item/somethingNew/requestApproval"), None);
}

#[test]
fn carries_the_exact_command_so_the_card_can_say_what_would_happen() {
    let (kind, _, detail, cwd) = describe_codex(COMMAND, &json!({"command": "rm -rf build", "cwd": "C:\\work"})).unwrap();
    assert_eq!((kind, detail.as_str(), cwd.as_deref()), (Kind::Command, "rm -rf build", Some("C:\\work")));
}

#[test]
fn says_so_plainly_when_the_runtime_described_nothing() {
    let (_, summary, _, _) = describe_codex(COMMAND, &json!({})).unwrap();
    assert!(summary.contains("did not describe"), "{summary}");
}

#[test]
fn counts_file_changes_rather_than_pasting_a_diff() {
    let (_, summary, detail, _) = describe_codex(FILE_CHANGE, &json!({"changes": [1, 2], "summary": "update config"})).unwrap();
    assert_eq!((summary.as_str(), detail.as_str()), ("Change 2 files", "update config"));
}

#[test]
fn shows_a_long_command_whole_and_counts_what_is_past_the_cap() {
    let long = describe_codex(COMMAND, &json!({"command": "x".repeat(9_000)})).unwrap().2;
    assert_eq!(long, "x".repeat(9_000));
    let huge = describe_codex(COMMAND, &json!({"command": "x".repeat(25_000)})).unwrap().2;
    assert!(huge.contains("[5,000 more characters not shown here"), "{}", &huge[huge.len() - 120..]);
}

#[test]
fn drops_a_question_with_no_id_because_an_answer_would_have_nowhere_to_go() {
    let questions = questions_of(&json!({"questions": [
        {"id": "q1", "question": "Which branch?", "options": [{"label": "main"}]},
        {"question": "Nameless"}
    ]}));
    assert_eq!(questions.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(), ["q1"]);
    assert_eq!(questions[0].options, vec![QuestionOption { label: "main".into(), description: None }]);
}

// ---- decisions ----

#[test]
fn maps_the_product_answers_onto_the_protocol() {
    assert_eq!(protocol_decision_for(&Decision::ApproveOnce), "accept");
    // Session-scoped, never durable: a forever-grant is a Settings decision.
    assert_eq!(protocol_decision_for(&Decision::ApproveAlways), "acceptForSession");
    assert_eq!(protocol_decision_for(&Decision::Deny), "decline");
}

// ---- the approval round trip ----

#[test]
fn surfaces_a_request_to_the_person_and_answers_the_runtime_when_decided() {
    let host = channel();
    let waiting = ask(&host, "mission_1", COMMAND, json!({"command": "pnpm build", "cwd": "C:\\work"}), None);
    let card = cards(&host, 1).remove(0);
    assert_eq!((card.mission_id.as_str(), card.runtime.as_str(), &card.kind), ("mission_1", "codex", &Kind::Command));
    assert_eq!(host.channel.pending_count(), 1);
    assert!(decide(&host, &card, Decision::ApproveOnce).is_ok());
    assert_eq!(waiting.join().unwrap(), Ok(json!({"decision": "accept"})));
    assert_eq!(host.channel.pending_count(), 0);
}

#[test]
fn ignores_a_decision_for_an_unknown_or_already_answered_approval() {
    let host = channel();
    let waiting = ask(&host, "mission_1", FILE_CHANGE, json!({"changes": [1]}), None);
    let card = cards(&host, 1).remove(0);
    assert!(decide(&host, &card, Decision::Deny).is_ok());
    // A double click on the card must not take the run down.
    assert!(decide(&host, &card, Decision::ApproveOnce).is_err());
    assert!(host.channel.decide(Answer::Decision { approval_id: "nope".into(), decision: Decision::ApproveOnce, reason: None }).is_err());
    assert_eq!(waiting.join().unwrap(), Ok(json!({"decision": "decline"})));
}

#[test]
fn refuses_a_request_it_does_not_understand_rather_than_guessing_and_says_so() {
    let host = channel();
    // The channel's refusal, for any caller that hands it a method it does not know. Codex's own session never does:
    // it answers an unknown request with JSON-RPC -32601 and its own notice before the channel (runtime/codex.rs).
    let said = ask(&host, "mission_1", "item/futureThing/requestApproval", json!({}), None).join().unwrap().unwrap_err();
    assert!(said.contains("does not recognise"), "{said}");
    assert_eq!(said, "Locust denied a request it does not recognise (item/futureThing/requestApproval) because it could not say what would happen. This build is older than the runtime's protocol.");
    // And it never reaches the person, because the card could not describe it.
    assert!(host.channel.waiting().is_empty());
}

#[test]
fn refuses_rather_than_queueing_without_limit_when_too_many_are_waiting() {
    let host = channel();
    let asked: Vec<_> = (0..crate::approval::MAX_PENDING).map(|i| ask(&host, "mission_1", COMMAND, json!({"command": format!("step {i}")}), None)).collect();
    cards(&host, crate::approval::MAX_PENDING);
    let too_many = ask(&host, "mission_1", COMMAND, json!({"command": "one too many"}), None);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !too_many.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if !too_many.is_finished() {
        host.channel.release("mission_1");
        panic!("the 65th request was queued instead of refused");
    }
    let said = answered(too_many).unwrap_err();
    assert_eq!(host.channel.pending_count(), 64);
    assert!(said.contains("already waiting on you"), "{said}");
    assert_eq!(said, "Locust denied an action because 64 approvals are already waiting on you. Answer some of them and ask again.");
    host.channel.release("mission_1");
    for a in asked {
        assert_eq!(decision_of(a.join().unwrap()), "decline");
    }
}

// a-card-answer-is-recorded-past-64-open-cards.test.ts
#[test]
fn still_has_the_first_card_when_its_answer_comes_after_80_more_were_raised_and_answered() {
    let host = channel();
    let first = ask(&host, "mission_1", COMMAND, json!({"command": "step 0"}), None);
    let first_card = cards(&host, 1).remove(0);
    for n in 1..=80 {
        let later = ask(&host, "mission_1", COMMAND, json!({"command": format!("step {n}")}), None);
        let card = cards(&host, 2).into_iter().find(|c| c.approval_id != first_card.approval_id).unwrap();
        decide(&host, &card, Decision::ApproveOnce).unwrap();
        assert_eq!(decision_of(later.join().unwrap()), "accept");
    }
    assert_eq!(host.channel.pending_count(), 1);
    decide(&host, &first_card, Decision::ApproveOnce).unwrap();
    assert_eq!(decision_of(first.join().unwrap()), "accept");
}

#[test]
fn forgets_an_answered_card_and_keeps_who_it_was_raised_for_until_then() {
    // case: forgets an answered card, keeps who it was raised for until then, and an unknown id is nothing
    let host = channel();
    host.channel.register_teammate("mission_1", "tm_wren");
    let waiting = ask(&host, "mission_1", COMMAND, json!({"command": "echo 1"}), None);
    let card = cards(&host, 1).remove(0);
    assert_eq!(host.channel.teammate_for(&card.mission_id).as_deref(), Some("tm_wren"));
    decide(&host, &card, Decision::ApproveOnce).unwrap();
    assert!(host.channel.waiting().is_empty());
    assert!(decide(&host, &card, Decision::ApproveOnce).is_err(), "an answered card is forgotten");
    assert!(host.channel.decide(Answer::Decision { approval_id: "ap_nope".into(), decision: Decision::ApproveOnce, reason: None }).is_err());
    assert_eq!(host.channel.pending_count(), 0);
    assert_eq!(decision_of(answered(waiting)), "accept");
}

/// Electron guards index.ts's answerApproval by its source: it records the answer, then forgets the card. Rust's
/// funnel is native::decide_approval (what the window and the engine call), and the same holds by behaviour: once it
/// returns, the answer waits for the ledger and the card is gone.
#[test]
fn the_answer_funnel_records_the_answer_and_forgets_the_card_at_once() {
    // case: the answer funnel forgets the card right after recording it (source guard)
    let _host = channel();
    let channel = crate::native::approvals_channel();
    let asking = channel.clone();
    let waiting = std::thread::spawn(move || {
        asking.ask_codex_with_changes("run_mission_funnel", "mission_funnel", "codex", COMMAND, &json!({"command": "echo 1"}), json!(1), None, &|| false)
    });
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let card = loop {
        if let Some(card) = crate::native::waiting_approvals_for("mission_funnel").pop() {
            break card;
        }
        assert!(Instant::now() < deadline, "the card was never raised");
        std::thread::sleep(Duration::from_millis(5));
    };
    crate::native::decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    assert!(crate::native::waiting_approvals_for("mission_funnel").is_empty(), "the card is forgotten");
    let recorded = channel.take_recorded_for("mission_funnel");
    assert_eq!(recorded.iter().map(|r| r.approval_id.as_str()).collect::<Vec<_>>(), [card.approval_id.as_str()], "the answer is recorded");
    assert!(crate::native::decide_approval(&card.approval_id, Decision::ApproveOnce, None).is_err());
    assert_eq!(decision_of(answered(waiting)), "accept");
}

#[test]
fn will_not_let_a_question_be_answered_with_a_decision_or_the_reverse() {
    let host = channel();
    let waiting = ask(&host, "mission_1", QUESTION, json!({"questions": [{"id": "q1", "question": "Which branch?", "options": [{"label": "main"}]}]}), None);
    let card = cards(&host, 1).remove(0);
    let said = decide(&host, &card, Decision::ApproveOnce).unwrap_err();
    assert!(said.contains("answered rather than approved"), "{said}");
    assert_eq!(waiting.join().unwrap(), Ok(json!({"answers": {"q1": {"answers": []}}})));
    // And the reverse: answers sent to an authorization.
    let waiting = ask(&host, "mission_1", COMMAND, json!({"command": "ls"}), None);
    let card = cards(&host, 1).remove(0);
    let said = host.channel.decide(Answer::Answers { approval_id: card.approval_id.clone(), answers: vec![("q1".into(), vec!["main".into()])] }).unwrap_err();
    assert!(said.contains("belongs to a question"), "{said}");
    assert_eq!(waiting.join().unwrap(), Ok(json!({"decision": "decline"})));
}

#[test]
fn carries_a_question_and_its_answers_through_in_the_shape_the_protocol_wants() {
    let host = channel();
    let waiting = ask(&host, "mission_1", QUESTION, json!({"questions": [{"id": "q1", "question": "Which branch?", "options": [{"label": "main"}]}]}), None);
    let card = cards(&host, 1).remove(0);
    assert_eq!(card.questions.iter().map(|q| (q.id.as_str(), q.question.as_str())).collect::<Vec<_>>(), [("q1", "Which branch?")]);
    assert!(host.channel.decide(Answer::Answers { approval_id: card.approval_id.clone(), answers: vec![("q1".into(), vec!["main".into()])] }).is_ok());
    assert_eq!(waiting.join().unwrap(), Ok(json!({"answers": {"q1": {"answers": ["main"]}}})));
}

#[test]
fn releases_what_a_finished_run_was_still_asking_so_no_card_waits_forever() {
    let host = channel();
    let waiting = ask(&host, "mission_1", COMMAND, json!({"command": "x"}), None);
    let card = cards(&host, 1).remove(0);
    host.channel.release("mission_1");
    assert_eq!(decision_of(waiting.join().unwrap()), "decline");
    assert_eq!(host.channel.pending_count(), 0);
    // And the card is dead: answering it now changes nothing.
    assert!(decide(&host, &card, Decision::ApproveOnce).is_err());
}

#[test]
fn releases_only_the_run_that_ended_and_leaves_another_run_its_waiting_card() {
    let host = channel();
    let ending = ask(&host, "mission_1", COMMAND, json!({"command": "x"}), None);
    let other = ask(&host, "mission_2", COMMAND, json!({"command": "y"}), None);
    cards(&host, 2);
    host.channel.release("mission_1");
    assert_eq!(decision_of(ending.join().unwrap()), "decline");
    // Wren's is still waiting on the person, and only the person may answer.
    assert_eq!(host.channel.pending_count(), 1);
    let left = cards(&host, 1).remove(0);
    assert_eq!(left.mission_id, "mission_2");
    decide(&host, &left, Decision::Deny).unwrap();
    other.join().unwrap().unwrap();
}

#[test]
fn shows_the_diff_for_the_item_the_card_is_about() {
    let host = channel();
    let changes: Changes = Arc::new(Mutex::new(HashMap::from([(
        "item_fc".to_owned(),
        vec![FileChangeRecord { path: "C:\\work\\HELLO.txt".into(), kind: FileChangeKind::Add, move_path: None, diff: "hello\n".into() }],
    )])));
    let waiting = ask(&host, "mission_1", FILE_CHANGE, json!({"itemId": "item_fc", "changes": [1], "cwd": "C:\\work"}), Some(changes));
    let card = cards(&host, 1).remove(0);
    assert_eq!(card.summary, "Change 1 file");
    let patch = card.patch.clone().expect("a patch");
    assert!(patch.text.contains("+hello"), "{}", patch.text);
    decide(&host, &card, Decision::Deny).unwrap();
    waiting.join().unwrap().unwrap();
}
