//! `renderer/src/a-queued-message-refused-as-busy-waits.test.ts` in the window: a message the queue sent that the host
//! refuses as busy goes back in line, pointed at the conversation it was typed in, and the queue does not send it again
//! before its retry time. The backoff's own cases are `src/composer/retried_after_busy_tests.rs`; these run the
//! window's `drain_queue_after_turn` and `poll`, with the send's result made locally as `ui/tests.rs` makes it.
use super::*;
use std::time::Duration;

const BUSY: &str = "A turn is already running in this conversation.";

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

struct Scene(std::path::PathBuf, State);
impl Drop for Scene {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl std::ops::Deref for Scene {
    type Target = State;
    fn deref(&self) -> &State {
        &self.1
    }
}
impl std::ops::DerefMut for Scene {
    fn deref_mut(&mut self) -> &mut State {
        &mut self.1
    }
}

static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The team on screen, not a demo, with a data folder of its own for the queue file.
fn scene() -> Scene {
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("locust-queued-busy-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let mut s = State::default();
    team_scene(&mut s);
    s.demo = false;
    s.data = Some(crate::store::Store::new(root.join("native"), root.join("electron")).unwrap());
    Scene(root, s)
}

fn row(key: &str) -> composer::QueuedRow {
    composer::QueuedRow {
        id: "q_0_run_4".into(),
        key: key.into(),
        text: "Create five.txt".into(),
        origin: composer::Origin::Person,
        attachments: Vec::new(),
        issue: None,
        restored: false,
        held: None,
        retry_at: None,
        tries: None,
    }
}

/// The send's result lands as `result` and the window polls once.
fn lands(s: &mut State, result: Result<Started, String>) {
    if s.flight.is_none() {
        let id = s.teammates[s.selected].id.clone();
        s.flight = Some(Flight { teammate_id: id, conversation_id: None, header: None, after_tip: None, message: Some("Create five.txt".into()) });
    }
    s.sending.request(result, |result| result);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while s.sending.is_pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!s.sending.is_pending(), "the result did not land");
    poll(s, 0.0, &mut Tx::default());
}

// case: goes back pointed at the conversation it was typed in, not before its retry time
#[test]
fn a_queued_message_refused_as_busy_goes_back_in_line_not_in_the_box() {
    let mut s = scene();
    s.queue_in_flight = Some(row("run_4"));
    let before = now_ms();
    lands(&mut s, Err(BUSY.into()));
    let back = s.queue_rows.first().expect("it went back in line");
    assert_eq!((back.key.as_str(), back.text.as_str(), back.tries), ("run_4", "Create five.txt", Some(1)));
    let at = back.retry_at.expect("a retry time");
    assert!(at >= before + 500 && at <= now_ms() + 500, "half a second on: {at} from {before}");
    assert!(s.draft.text.is_empty(), "the words are in the queue, not the box: {:?}", s.draft.text);
    assert!(s.queue_in_flight.is_none());
}

#[test]
fn a_queued_message_that_failed_for_another_reason_comes_back_in_the_box_as_before() {
    let mut s = scene();
    s.queue_in_flight = Some(row("run_4"));
    lands(&mut s, Err("send failed".into()));
    assert!(s.queue_rows.is_empty());
    assert_eq!(s.draft.text, "Create five.txt");
    assert!(s.queue_in_flight.is_none());
}

#[test]
fn a_message_the_person_typed_and_was_refused_as_busy_comes_back_in_the_box() {
    let mut s = scene();
    lands(&mut s, Err(BUSY.into()));
    assert!(s.queue_rows.is_empty());
    assert_eq!(s.draft.text, "Create five.txt");
}

#[test]
fn a_queued_message_that_started_is_no_longer_in_flight() {
    let mut s = scene();
    s.queue_in_flight = Some(row("run_4"));
    lands(&mut s, Ok(Started { conversation_id: "run_4".into(), mode: "ask".into() }));
    assert!(s.queue_in_flight.is_none(), "a later busy refusal cannot bring it back");
}

#[test]
fn the_persons_own_send_is_never_taken_for_a_queued_one() {
    let mut s = scene();
    s.queue_in_flight = Some(row("run_4"));
    s.draft.replace_text("Something else");
    submit_composer(&mut s, &mut Tx::default(), false);
    assert!(s.queue_in_flight.is_none());
}

#[test]
fn the_queue_waits_for_the_retry_time_then_sends_and_a_second_refusal_waits_longer() {
    let mut s = scene();
    let key = queue_key_now(&s).expect("a conversation on screen");
    s.queue_rows = vec![composer::QueuedRow { retry_at: Some(now_ms() + 60_000), tries: Some(1), ..row(&key) }];
    drain_queue_after_turn(&mut s, &mut Tx::default());
    assert_eq!(s.queue_rows.len(), 1, "not before its retry time");
    assert!(s.flight.is_none() && !s.sending.is_pending(), "nothing was sent");
    // Its time comes round: it goes, and the window remembers it went from the queue.
    s.queue_rows[0].retry_at = Some(now_ms().saturating_sub(1));
    drain_queue_after_turn(&mut s, &mut Tx::default());
    assert!(s.queue_rows.is_empty(), "it went");
    assert_eq!(s.queue_in_flight.as_ref().map(|r| r.tries), Some(Some(1)));
    // Refused again: back in line, waiting a second this time.
    let before = now_ms();
    lands(&mut s, Err(BUSY.into()));
    let back = s.queue_rows.first().expect("back in line again");
    assert_eq!(back.tries, Some(2));
    assert!(back.retry_at.unwrap() >= before + 1_000, "{back:?}");
}

// It keeps its place: a busy refusal goes back to the front of the line, ahead of what was queued after it.
#[test]
fn a_busy_refusal_goes_back_to_the_front_of_the_line() {
    let mut s = scene();
    s.queue_rows = vec![composer::QueuedRow { id: "q_1_run_4".into(), text: "Then six.txt".into(), ..row("run_4") }];
    s.queue_in_flight = Some(row("run_4"));
    lands(&mut s, Err(BUSY.into()));
    let texts: Vec<&str> = s.queue_rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, ["Create five.txt", "Then six.txt"]);
}
