//! Codex 0.172.0: ToolRequestUserInputParams and ToolRequestUserInputResponse.
//! These wire fixtures are from the generated schema, independent of Locust's serializer.
use super::codex_stand_in::*;
use super::*;
use crate::approval::{Answer, Decision};

const QUESTION: &str = "item/tool/requestUserInput";

fn request(thread: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": "server-question-1", "method": QUESTION,
        "params": {"threadId": thread, "turnId": "turn_1", "itemId": "question_1",
            "isBlocking": true, "questions": [{"id": "db", "header": "Database",
                "question": "Which database?", "options": null}]}})
}

fn refusal_wire(
    thread: &str,
    channel: Option<Arc<Channel>>,
    cancel: Option<Arc<AtomicBool>>,
) -> Value {
    let (mut codex, told) = stand_in(json!({
        "answer": [{"send": {"method": "turn/completed",
            "params": {"threadId": "thread_9", "turn": {"id": "turn_1", "status": "completed"}}}}],
    }));
    codex.normal.thread = Some("thread_9".into());
    codex.normal.turn = Some("turn_1".into());
    codex.mission_id = "question-test-mission".into();
    codex.run_id = "turn_1".into();
    codex.approvals = channel;
    codex.cancel = cancel;
    codex.notification(request(thread)).unwrap();
    codex.cancel = None; // Let the fake server finish the reply handshake after the test cancels.
    // Wait for the stand-in to acknowledge and record the real wire reply.
    to_the_end(&mut codex);
    let replies = told.replies();
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], "server-question-1");
    assert!(replies[0].get("error").is_none());
    replies.into_iter().next().unwrap()["result"].clone()
}

fn expected_unanswered() -> Value {
    json!({"answers": {"db": {"answers": []}}})
}

fn wait_for_card(channel: &Channel) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(card) = channel.waiting().first() {
            return card.approval_id.clone();
        }
        assert!(Instant::now() < deadline, "the runtime never asked its question");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn ending_a_run_with_a_waiting_question_sends_unanswered_questions() {
    let channel = Channel::new();
    let waiter = channel.clone();
    let worker = std::thread::spawn(move || {
        wait_for_card(&waiter);
        waiter.release("question-test-mission");
    });
    let result = refusal_wire("thread_9", Some(channel.clone()), None);
    worker.join().unwrap();
    assert_eq!(result, expected_unanswered());
    assert!(channel.waiting().is_empty());
}

#[test]
fn a_decision_on_a_waiting_question_sends_unanswered_questions() {
    let channel = Channel::new();
    let waiter = channel.clone();
    let worker = std::thread::spawn(move || {
        let id = wait_for_card(&waiter);
        assert!(waiter.decide(Answer::Decision {
            approval_id: id, decision: Decision::ApproveOnce, reason: None
        }).is_err());
    });
    let result = refusal_wire("thread_9", Some(channel.clone()), None);
    worker.join().unwrap();
    assert_eq!(result, expected_unanswered());
    assert!(channel.waiting().is_empty());
}

#[test]
fn a_question_arriving_after_release_gets_an_unanswered_result() {
    let channel = Channel::new();
    channel.release("question-test-mission");
    assert_eq!(refusal_wire("thread_9", Some(channel.clone()), None), expected_unanswered());
    assert!(channel.waiting().is_empty());
}

#[test]
fn a_question_without_a_card_host_gets_an_unanswered_result() {
    assert_eq!(refusal_wire("thread_9", None, None), expected_unanswered());
}

#[test]
fn another_threads_question_gets_an_unanswered_result() {
    let channel = Channel::new();
    assert_eq!(refusal_wire("thread_other", Some(channel.clone()), None), expected_unanswered());
    assert!(channel.waiting().is_empty());
}

#[test]
fn cancelling_a_waiting_question_sends_unanswered_questions() {
    let channel = Channel::new();
    let waiter = channel.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let worker = std::thread::spawn(move || {
        wait_for_card(&waiter);
        flag.store(true, Ordering::Relaxed);
    });
    let result = refusal_wire("thread_9", Some(channel.clone()), Some(cancel));
    worker.join().unwrap();
    assert_eq!(result, expected_unanswered());
    assert!(channel.waiting().is_empty());
}
