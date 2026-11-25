//! The remaining two cases of `a-streamed-batch-is-written-before-it-is-shown.test.ts` on
//! [`StreamBatcher`]: a new text deadline after a flush, and stopping cleanly after a timer
//! write fails. Native's run loop writes synchronously, so at most one write runs at a time.
use super::{STREAM_BATCH_MS, StreamBatcher};
use crate::runtime::Event;
use serde_json::json;
use std::io;
use std::time::{Duration, Instant};

fn delta(seq: u64) -> Event {
    Event {
        kind: "message.delta".into(),
        payload: json!({"seq": seq, "text": format!("word{seq} ")}),
        thread: None,
    }
}

fn ended(seq: u64) -> Event {
    Event {
        kind: "run.completed".into(),
        payload: json!({"seq": seq}),
        thread: None,
    }
}

fn seqs(batch: &[Event]) -> Vec<u64> {
    batch.iter().filter_map(|e| e.payload["seq"].as_u64()).collect()
}

// case: starts a new text deadline after an approval flush waited on an in-flight write
#[test]
fn starts_a_new_text_deadline_after_an_approval_flush_waited_on_an_in_flight_write() {
    let mut batcher = StreamBatcher::default();
    let mut writes: Vec<Vec<u64>> = Vec::new();
    let t0 = Instant::now();

    batcher
        .push(vec![ended(1)], t0, |batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    batcher
        .flush(|batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    assert_eq!(writes, vec![vec![1]]);
    assert_eq!(batcher.deadline(), None);

    batcher
        .push(vec![delta(2)], t0, |batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    let due = t0 + Duration::from_millis(STREAM_BATCH_MS);
    assert_eq!(batcher.deadline(), Some(due));

    batcher
        .tick(due - Duration::from_millis(1), |batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    assert_eq!(writes, vec![vec![1]]);

    batcher
        .tick(due, |batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    assert_eq!(writes, vec![vec![1], vec![2]]);
    assert_eq!(batcher.deadline(), None);
}

// case: keeps one write in flight, awaits backpressure, and stops after a timer write fails
#[test]
fn keeps_one_write_in_flight_awaits_backpressure_and_stops_after_a_timer_write_fails() {
    let mut batcher = StreamBatcher::default();
    let mut writes: Vec<Vec<u64>> = Vec::new();
    let t0 = Instant::now();

    batcher
        .push(vec![ended(1)], t0, |batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    assert_eq!(writes, vec![vec![1]]);

    batcher
        .push(vec![delta(2), delta(3)], t0, |batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    assert_eq!(batcher.pending_count(), 2);

    let due = t0 + Duration::from_millis(STREAM_BATCH_MS);
    let err = batcher
        .tick(due, |batch| {
            writes.push(seqs(&batch));
            Err(io::Error::other("disk full"))
        })
        .unwrap_err();
    assert_eq!(err.to_string(), "disk full");
    assert_eq!(writes, vec![vec![1], vec![2, 3]]);
    assert_eq!(batcher.pending_count(), 0);
    assert_eq!(batcher.deadline(), None);

    // Once stopped by a failed write, further pushes and flushes do not queue or write.
    batcher
        .push(vec![delta(4)], due, |batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    assert_eq!(batcher.pending_count(), 0);
    assert_eq!(batcher.deadline(), None);
    batcher
        .flush(|batch| {
            writes.push(seqs(&batch));
            Ok(())
        })
        .unwrap();
    assert_eq!(writes.len(), 2);
    assert_eq!(batcher.deadline(), None);
}
