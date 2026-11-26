//! `a-streamed-batch-is-written-before-it-is-shown.test.ts`: the run loop (`drive_runtime`) holds text for one window
//! and writes it in one go, ahead of any other event; a batch is on disk before anything shows it. Electron drives its
//! batcher on fake timers; these drive the run loop with a scripted runtime that looks at the ledger between events,
//! and the batcher on a clock the test moves, over a real ledger in a temporary data folder.
use super::*;
use crate::long_answer_batch::{STREAM_BATCH_MS, StreamBatcher, StreamedUpdate, persist_event_updates};
use crate::runtime::{Event, Runtime};
use std::collections::VecDeque;
use std::io::Write as _;
use std::time::{Duration, Instant};

fn delta(n: u64) -> Event {
    Event {
        kind: "message.delta".into(),
        payload: json!({"itemId": "answer", "operation": "append", "text": format!("word{n} "), "final": false}),
        thread: None,
    }
}

fn step() -> Event {
    Event { kind: "step.started".into(), payload: json!({"itemId": "tool", "stepKind": "item"}), thread: None }
}

fn ended() -> Event {
    Event { kind: "run.completed".into(), payload: json!({"process": {"exitCode": 0}}), thread: None }
}

fn writer(t: &Temp, mission: &str) -> LedgerWriter {
    LedgerWriter::create_runtime(&t.store(), mission, "Say a few words.", "account-default", "codex", "codex", &[], None).unwrap()
}

fn records(path: &Path) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn sequences(path: &Path) -> Vec<u64> {
    records(path).iter().filter_map(|r| r["event"]["sequence"].as_u64()).collect()
}

enum Step {
    Give(Event),
    Wait(Duration),
    Look,
}

/// A runtime that gives its script, and between events records which sequences the ledger holds.
struct Script {
    steps: VecDeque<Step>,
    ledger: PathBuf,
    looks: Arc<Mutex<Vec<Vec<u64>>>>,
}
impl Runtime for Script {
    fn start(&mut self, _turn: &crate::runtime::Turn) -> io::Result<()> {
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        match self.steps.pop_front() {
            Some(Step::Give(event)) => Ok(Some(event)),
            Some(Step::Wait(how_long)) => {
                std::thread::sleep(how_long);
                Ok(None)
            }
            Some(Step::Look) => {
                self.looks.lock().unwrap().push(sequences(&self.ledger));
                Ok(None)
            }
            None => Ok(Some(ended())),
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Drive a turn of `mission` through the run loop with `steps`; give back the looks and the ledger's path.
fn drive_script(t: &Temp, mission: &str, steps: Vec<Step>) -> (Vec<Vec<u64>>, PathBuf) {
    let store = t.store();
    let mut w = writer(t, mission);
    let looks = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = Script { steps: steps.into(), ledger: w.path.clone(), looks: looks.clone() };
    let flag = Arc::new(AtomicBool::new(false));
    let mut registered = Some(Registered(Vec::new(), None, None));
    let channel = Channel::new();
    drive_runtime(&mut runtime, &mut w, &flag, &mut registered, &channel, mission, &store);
    let seen = looks.lock().unwrap().clone();
    (seen, w.path.clone())
}

// case: flushes text ahead of activity immediately and preserves order across both batches
#[test]
fn text_is_written_with_the_activity_that_follows_it_in_order() {
    let t = Temp::new();
    let (looks, path) = drive_script(&t, "mission_activity", vec![
        Step::Give(delta(1)),
        Step::Give(delta(2)),
        Step::Look,
        Step::Give(step()),
        Step::Look,
        Step::Give(delta(4)),
    ]);
    assert_eq!(looks[0], Vec::<u64>::new(), "text waits for its window");
    assert_eq!(looks[1], [1, 2, 3], "a step writes the text ahead of it, at once");
    assert_eq!(sequences(&path), [1, 2, 3, 4, 5], "and the run's end writes the last fragment");
    let kinds: Vec<String> = records(&path).iter().filter_map(|r| r["event"]["type"].as_str().map(str::to_owned)).collect();
    assert_eq!(kinds, ["message.delta", "message.delta", "step.started", "message.delta", "run.completed"]);
}

// case: delivers a lone delta at the deadline without another record or a run ending
#[test]
fn a_lone_delta_is_written_when_its_window_ends() {
    let t = Temp::new();
    let window = Duration::from_millis(STREAM_BATCH_MS);
    let (looks, _path) = drive_script(&t, "mission_lone", vec![
        Step::Give(delta(1)),
        Step::Look,
        Step::Wait(window + Duration::from_millis(30)),
        Step::Wait(Duration::from_millis(1)),
        Step::Look,
    ]);
    assert_eq!(looks[0], Vec::<u64>::new(), "held inside its window");
    assert_eq!(looks[1], [1], "written once the window passed, with nothing after it");
}

// case: writes pending text before a run ending and drains the last fragment on process exit
// case: collapses a burst of records into one durable append
#[test]
fn text_waiting_is_written_before_the_ending_and_the_last_on_flush() {
    let t = Temp::new();
    let (_looks, path) = drive_script(&t, "mission_end", vec![Step::Give(delta(1)), Step::Give(delta(2))]);
    assert_eq!(sequences(&path), [1, 2, 3], "the ending writes the text ahead of it");
    // The batcher Electron drives: one append of three, shown as the text and then the ending.
    let mut w = writer(&t, "mission_end_batch");
    let (mut writes, mut sent) = (Vec::<usize>::new(), Vec::<StreamedUpdate>::new());
    let mut batcher = StreamBatcher::default();
    let now = Instant::now();
    let mut write = |batch: Vec<Event>, writes: &mut Vec<usize>, sent: &mut Vec<StreamedUpdate>| {
        writes.push(batch.len());
        persist_event_updates(&mut w, &batch, |_| Ok(()), |update| sent.push(update))
    };
    batcher.push(vec![delta(1), delta(2)], now, |b| write(b, &mut writes, &mut sent)).unwrap();
    batcher.push(vec![ended()], now, |b| write(b, &mut writes, &mut sent)).unwrap();
    batcher.flush(|b| write(b, &mut writes, &mut sent)).unwrap();
    assert_eq!(writes, [3]);
    assert_eq!(sent.iter().map(StreamedUpdate::kind).collect::<Vec<_>>(), ["message-deltas", "event"]);
    assert_eq!(sent.iter().flat_map(StreamedUpdate::sequences).collect::<Vec<_>>(), [1, 2, 3]);
    batcher.push(vec![delta(4)], now, |b| write(b, &mut writes, &mut sent)).unwrap();
    batcher.flush(|b| write(b, &mut writes, &mut sent)).unwrap();
    assert_eq!(writes, [3, 1]);
    assert_eq!(sent.iter().flat_map(StreamedUpdate::sequences).collect::<Vec<_>>(), [1, 2, 3, 4]);
    assert_eq!(batcher.deadline(), None, "no window left running");
}

// case: measures frame to fifty millisecond windows on the same paced answer and keeps its first-fragment deadline
// case: replays the same paced answer with fewer durable appends and sends while keeping every fragment
#[test]
fn paced_text_in_frame_and_fifty_millisecond_windows() {
    let t = Temp::new();
    let count = 1_000u64;
    let mut measured = Vec::new();
    for window_ms in [16u64, 33, STREAM_BATCH_MS] {
        let mut w = writer(&t, &format!("mission_paced_{window_ms}"));
        let mut batcher = StreamBatcher::with_window(Duration::from_millis(window_ms));
        let (mut appends, mut sends, mut max_delay) = (0usize, 0usize, Duration::ZERO);
        let mut arrivals: Vec<Instant> = Vec::new();
        let t0 = Instant::now();
        let mut write = |batch: Vec<Event>, at: Instant, arrivals: &[Instant], appends: &mut usize, sends: &mut usize, max_delay: &mut Duration| {
            *appends += 1;
            persist_event_updates(&mut w, &batch, |_| Ok(()), |update| {
                *sends += 1;
                for sequence in update.sequences() {
                    *max_delay = (*max_delay).max(at - arrivals[sequence as usize - 1]);
                }
            })
        };
        for i in 0..count {
            let now = t0 + Duration::from_millis(2 * i);
            // A window that ended between two fragments is written at its end, as a timer would.
            if let Some(due) = batcher.deadline().filter(|due| *due <= now) {
                batcher.tick(due, |b| write(b, due, &arrivals, &mut appends, &mut sends, &mut max_delay)).unwrap();
            }
            arrivals.push(now);
            batcher.push(vec![delta(i + 1)], now, |b| write(b, now, &arrivals, &mut appends, &mut sends, &mut max_delay)).unwrap();
        }
        let last = t0 + Duration::from_millis(2 * (count - 1));
        batcher.flush(|b| write(b, last, &arrivals, &mut appends, &mut sends, &mut max_delay)).unwrap();
        let path = w.path.clone();
        assert_eq!(sequences(&path), (1..=count).collect::<Vec<_>>(), "every fragment kept, in order");
        let texts: Vec<String> = records(&path).iter().filter_map(|r| r["event"]["payload"]["text"].as_str().map(str::to_owned)).collect();
        assert_eq!(texts, (1..=count).map(|n| format!("word{n} ")).collect::<Vec<_>>());
        assert!(max_delay <= Duration::from_millis(window_ms), "window {window_ms}: a fragment waited {max_delay:?}");
        measured.push((window_ms, appends, sends, max_delay));
        assert_eq!(batcher.deadline(), None);
    }
    eprintln!("STREAM WINDOWS: {measured:?}");
    let fifty = measured.iter().find(|m| m.0 == STREAM_BATCH_MS).unwrap();
    assert!(fifty.1 as u64 <= count / 10 && fifty.2 as u64 <= count / 10, "{fifty:?}");
}

// case: reads the entire batch from disk at send time and recovers it after a crash before send
// case: still persists every event before any of them is emitted
#[test]
fn a_batch_is_on_disk_when_shown_and_survives_a_crash_before() {
    let t = Temp::new();
    let mut w = writer(&t, "mission_stream");
    let path = w.path.clone();
    let mut sent: Vec<Vec<u64>> = Vec::new();
    persist_event_updates(&mut w, &[delta(1), delta(2)], |_| Ok(()), |update| sent.push(update.sequences())).unwrap();
    assert_eq!(sent, [vec![1, 2]]);
    assert_eq!(sequences(&path), [1, 2]);
    // A fresh read of the disk IN the send proves the boundary, not that the disk caught up by the time of the assert.
    let mut at_send = String::new();
    persist_event_updates(&mut w, &[delta(3), delta(4)], |_| Ok(()), |_| at_send = fs::read_to_string(&path).unwrap()).unwrap();
    assert!(at_send.contains(&format!("\"{}:app:4\"", w.run)), "the whole batch was on disk when it was shown");
    let crashed = persist_event_updates(
        &mut w,
        &[delta(5), delta(6)],
        |_| Err(io::Error::other("crash after write, before send")),
        |_| panic!("send must not run"),
    );
    assert_eq!(crashed.unwrap_err().to_string(), "crash after write, before send");
    let lines = records(&path);
    assert_eq!(sequences(&path), [1, 2, 3, 4, 5, 6], "a fresh read recovers the batch");
    assert_eq!(lines.len(), 7, "the mission's first record and six events, every line whole");
}

// case: sends nothing when the durable append fails
#[test]
fn a_failed_append_shows_nothing() {
    let t = Temp::new();
    let mut w = writer(&t, "mission_refused");
    // Something else wrote to the file: the writer refuses to extend it.
    fs::OpenOptions::new().append(true).open(&w.path).unwrap().write_all(b"{}\n").unwrap();
    let (mut tracked, mut emitted) = (false, false);
    let failed = persist_event_updates(&mut w, &[delta(1)], |_| {
        tracked = true;
        Ok(())
    }, |_| emitted = true);
    assert!(failed.is_err());
    assert!(!tracked && !emitted, "nothing known, nothing shown");
}
