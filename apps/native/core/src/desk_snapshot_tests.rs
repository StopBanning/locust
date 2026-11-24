//! Electron `renderer/src/a-browser-opens-on-what-it-last-saw.test.ts`:
//! what the window read from the computer is kept, sealed, and shown at once the next time:
//! each read by its call and arguments, newest kept when there is too much, one too long not kept,
//! never readable without its own key, and saying when it is from (with the day when not today).

use crate::desk_host::{
    DeskAsk, DeskHandler, DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskValue, DeskWindow,
    desk_encode_args,
};
use crate::desk_snapshot::{
    SNAPSHOT_ENTRY_CHARS, SNAPSHOT_TOTAL_CHARS, SnapshotAnswer, generate_snapshot_key,
    open_snapshot, seal_snapshot, snapshot_key, snapshot_when, snapshot_with,
};
use chrono::{Local, TimeZone};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn big(chars: usize) -> String {
    "x".repeat(chars)
}

fn host_with_readers() -> DeskHost {
    let mut handlers: HashMap<String, DeskHandler> = HashMap::new();
    handlers.insert(
        "teammates:list".into(),
        Arc::new(|_, _| Ok(DeskValue::Json(json!([{ "name": "Ada" }])))),
    );
    handlers.insert(
        "mission-history:list".into(),
        Arc::new(|_, _| Ok(DeskValue::Json(json!({ "missions": ["m1"] })))),
    );
    handlers.insert(
        "mission-history:read".into(),
        Arc::new(|_, args| {
            let id = args.first().map(DeskValue::to_json_lossy).unwrap_or(serde_json::Value::Null);
            Ok(DeskValue::Json(json!({ "mission": id })))
        }),
    );
    DeskHost::new(DeskHostDeps {
        handlers: Arc::new(Mutex::new(handlers)),
        window: Arc::new(|| {
            Some(DeskWindow {
                destroyed: false,
                web_contents: DeskSender {
                    id: 1,
                    main_frame_id: 2,
                    process_id: 3,
                    destroyed: false,
                },
            })
        }),
        tell: Arc::new(|_, _| true),
        cover: Arc::new(|_| {}),
        hello: Arc::new(|| {
            Ok(DeskHelloInfo {
                version: "0.834.0".into(),
                computer: "DESK-PC".into(),
                theme: "dark".into(),
            })
        }),
        waiting: Arc::new(Vec::new),
        changed: None,
    })
}

#[test]
// case: keeps each read by its call and its arguments
fn keeps_each_read_by_its_call_and_its_arguments() {
    assert_ne!(
        snapshot_key("mission:history", &[]),
        snapshot_key("mission:get", &[DeskValue::Json(json!("m1"))])
    );
    assert_ne!(
        snapshot_key("mission:get", &[DeskValue::Json(json!("m1"))]),
        snapshot_key("mission:get", &[DeskValue::Json(json!("m2"))])
    );
    let key = snapshot_key("mission:get", &[DeskValue::Json(json!("m1"))]);
    let first = snapshot_with(None, &key, json!({ "turns": 3 }), 1_000);
    let kept = snapshot_with(Some(&first), &key, json!({ "turns": 4 }), 2_000);
    assert_eq!(
        kept.answers.get(&key),
        Some(&SnapshotAnswer {
            value: json!({ "turns": 4 }),
            at: 2_000,
        })
    );
    assert_eq!(kept.at, 2_000);
}

#[test]
// case: keeps the newest when there is too much, and does not keep one too long
fn keeps_the_newest_when_there_is_too_much_and_does_not_keep_one_too_long() {
    let third = SNAPSHOT_TOTAL_CHARS / 3;
    let step = third.min(SNAPSHOT_ENTRY_CHARS - 100);
    let mut kept = snapshot_with(None, "a", json!(big(step)), 1);
    for index in 0..12u64 {
        kept = snapshot_with(Some(&kept), &format!("k{index}"), json!(big(step)), 10 + index);
    }
    assert!(!kept.answers.contains_key("a"));
    assert!(kept.answers.contains_key("k11"));
    let total_json: usize = kept
        .answers
        .values()
        .map(|entry| entry.value.to_string().len())
        .sum();
    assert!(total_json <= SNAPSHOT_TOTAL_CHARS + 2_000);

    let before = snapshot_with(None, "huge", json!("small"), 1);
    let after = snapshot_with(Some(&before), "huge", json!(big(SNAPSHOT_ENTRY_CHARS + 1)), 2);
    assert!(!after.answers.contains_key("huge"));
}

#[test]
// case: is sealed: opened with its own key, and with no other
fn is_sealed_opened_with_its_own_key_and_with_no_other() {
    let key = generate_snapshot_key();
    let other = generate_snapshot_key();
    let snapshot = snapshot_with(
        None,
        &snapshot_key("teammates:list", &[]),
        json!([{ "name": "Ada" }]),
        5,
    );
    let sealed = seal_snapshot(&key, &snapshot).expect("seal snapshot");
    let raw_text = String::from_utf8_lossy(&sealed.ciphertext);
    assert!(!raw_text.contains("Ada"));
    assert_eq!(open_snapshot(&key, &sealed), Some(snapshot));
    assert_eq!(open_snapshot(&other, &sealed), None);

    // Caller path through DeskHost: reads are cached, sealed, and opened with the host's methods.
    let host = host_with_readers();
    host.handle(
        "laptop",
        &[DeskAsk::Call {
            id: "r1".into(),
            channel: "teammates:list".into(),
            args: desk_encode_args(&[]),
        }],
    );
    let host_sealed = host
        .sealed_snapshot(&key)
        .expect("host sealed snapshot after read");
    assert!(!String::from_utf8_lossy(&host_sealed.ciphertext).contains("Ada"));
    let opened = host
        .open_cached_snapshot(&key, &host_sealed)
        .expect("open host snapshot with own key");
    let expected_key = snapshot_key("teammates:list", &[]);
    assert_eq!(
        opened.answers.get(&expected_key).map(|a| &a.value),
        Some(&json!([{ "name": "Ada" }]))
    );
    assert_eq!(host.open_cached_snapshot(&other, &host_sealed), None);
}

#[test]
fn desk_host_keeps_two_reads_on_one_channel_with_different_args_apart_in_the_sealed_snapshot() {
    let key = generate_snapshot_key();
    let host = host_with_readers();
    let m1_args = [DeskValue::Json(json!("m1"))];
    let m2_args = [DeskValue::Json(json!("m2"))];
    host.handle(
        "laptop",
        &[
            DeskAsk::Call {
                id: "r1".into(),
                channel: "mission-history:read".into(),
                args: desk_encode_args(&m1_args),
            },
            DeskAsk::Call {
                id: "r2".into(),
                channel: "mission-history:read".into(),
                args: desk_encode_args(&m2_args),
            },
        ],
    );

    let sealed = host
        .sealed_snapshot(&key)
        .expect("host sealed snapshot after two reads");
    let opened = host
        .open_cached_snapshot(&key, &sealed)
        .expect("open sealed snapshot");

    let k1 = snapshot_key("mission-history:read", &m1_args);
    let k2 = snapshot_key("mission-history:read", &m2_args);
    assert_ne!(k1, k2);
    assert_eq!(opened.answers.len(), 2);
    assert_eq!(
        opened.answers.get(&k1).map(|a| &a.value),
        Some(&json!({ "mission": "m1" }))
    );
    assert_eq!(
        opened.answers.get(&k2).map(|a| &a.value),
        Some(&json!({ "mission": "m2" }))
    );
}

#[test]
// case: says when it is from, with the day when it was not today
fn says_when_it_is_from_with_the_day_when_it_was_not_today() {
    let now = Local
        .with_ymd_and_hms(2026, 10, 24, 15, 0, 0)
        .single()
        .unwrap()
        .timestamp_millis();
    let same_day = Local
        .with_ymd_and_hms(2026, 10, 24, 10, 32, 0)
        .single()
        .unwrap()
        .timestamp_millis();
    let earlier_day = Local
        .with_ymd_and_hms(2026, 10, 22, 10, 32, 0)
        .single()
        .unwrap()
        .timestamp_millis();
    assert!(!snapshot_when(same_day, now).contains(" on "));
    assert!(snapshot_when(earlier_day, now).contains(" on "));

    // Caller path through DeskHost::snapshot_label.
    let host = host_with_readers();
    host.handle(
        "laptop",
        &[DeskAsk::Call {
            id: "r2".into(),
            channel: "mission-history:list".into(),
            args: desk_encode_args(&[]),
        }],
    );
    let label = host.snapshot_label(now).expect("snapshot label");
    assert!(!label.is_empty());
}
