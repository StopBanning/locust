//! Every channel the window can call is in exactly one of `desk_access`'s lists, and every
//! browser-refused channel is refused through `DeskHost` (`renderer/src/every-call-a-browser-is-refused-is-decided.test.ts`).

use crate::desk::{
    ACT_CHANNELS, CONDITIONAL_CHANNELS, DESK_EVENTS, DESK_TAKE_BACK_CHANNEL, DESK_WHERE_CHANNEL,
    DeskAccess, PHONE_DESK_CHANNEL, READ_CHANNELS, WINDOW_ONLY, WINDOW_ONLY_EVENTS,
    WINDOW_ONLY_SENDS, WINDOW_ONLY_SAYS, desk_access, is_desk_event,
};
use crate::desk_host::{
    DeskAsk, DeskHandler, DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskTell, DeskValue,
    DeskWindow, MISSION_APPROVAL_CHANNEL, desk_encode_args,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn all_call_channels() -> Vec<&'static str> {
    READ_CHANNELS
        .iter()
        .chain(ACT_CHANNELS.iter())
        .chain(CONDITIONAL_CHANNELS.iter())
        .chain(WINDOW_ONLY.iter())
        .copied()
        .collect()
}

fn make_host(handlers: HashMap<String, DeskHandler>) -> (DeskHost, Arc<Mutex<Vec<DeskTell>>>) {
    let told: Arc<Mutex<Vec<DeskTell>>> = Arc::new(Mutex::new(Vec::new()));
    let told_c = told.clone();
    let win = DeskWindow {
        destroyed: false,
        web_contents: DeskSender {
            id: 1,
            main_frame_id: 2,
            process_id: 3,
            destroyed: false,
        },
    };
    let host = DeskHost::new(DeskHostDeps {
        handlers: Arc::new(Mutex::new(handlers)),
        window: Arc::new(move || Some(win.clone())),
        tell: Arc::new(move |_, items| {
            told_c.lock().unwrap().extend_from_slice(items);
            true
        }),
        cover: Arc::new(|_| {}),
        hello: Arc::new(|| {
            Ok(DeskHelloInfo {
                version: "0.850.0".into(),
                computer: "DESK-PC".into(),
                theme: "dark".into(),
            })
        }),
        waiting: Arc::new(Vec::new),
        changed: None,
    });
    (host, told)
}

// case: reads every call off the API, and finds the window making them
#[test]
fn reads_every_call_off_the_api_and_finds_the_window_making_them() {
    let all = all_call_channels();
    let unique: HashSet<&str> = all.iter().copied().collect();
    assert!(
        unique.len() > 300,
        "expected >300 decided channels, got {}",
        unique.len()
    );
    assert!(WINDOW_ONLY.contains(&"workspace:reveal"));
    assert!(ACT_CHANNELS.contains(&"voice:transcribe"));
    assert!(
        WINDOW_ONLY.len() > 80,
        "expected >80 window-only channels, got {}",
        WINDOW_ONLY.len()
    );
}

// case: is decided where the window makes it: held, left out, made to work, or quiet for a reason written here
#[test]
fn is_decided_where_the_window_makes_it_held_left_out_made_to_work_or_quiet_for_a_reason_written_here()
{
    let lists: [(&str, &[&str]); 4] = [
        ("READ_CHANNELS", READ_CHANNELS),
        ("ACT_CHANNELS", ACT_CHANNELS),
        ("CONDITIONAL_CHANNELS", CONDITIONAL_CHANNELS),
        ("WINDOW_ONLY", WINDOW_ONLY),
    ];

    let mut seen: HashMap<&str, &str> = HashMap::new();
    for (list_name, list) in lists {
        let mut local: HashSet<&str> = HashSet::new();
        for &channel in list {
            assert!(
                local.insert(channel),
                "duplicate channel {channel:?} inside {list_name}"
            );
            if let Some(prev) = seen.insert(channel, list_name) {
                panic!("channel {channel:?} appears in both {prev} and {list_name}");
            }
        }
    }

    let mut events_seen: HashMap<&str, &str> = HashMap::new();
    for (list_name, list) in [
        ("DESK_EVENTS", DESK_EVENTS),
        ("WINDOW_ONLY_EVENTS", WINDOW_ONLY_EVENTS),
    ] {
        let mut local: HashSet<&str> = HashSet::new();
        for &channel in list {
            assert!(
                local.insert(channel),
                "duplicate event {channel:?} inside {list_name}"
            );
            if let Some(prev) = events_seen.insert(channel, list_name) {
                panic!("event {channel:?} appears in both {prev} and {list_name}");
            }
        }
    }
    for &channel in DESK_EVENTS {
        assert!(is_desk_event(channel), "{channel} should be a desk event");
    }
    for &channel in WINDOW_ONLY_EVENTS {
        assert!(
            !is_desk_event(channel),
            "{channel} should not be a desk event"
        );
    }

    let mut sends_seen: HashSet<&str> = HashSet::new();
    for &channel in WINDOW_ONLY_SENDS {
        assert!(
            sends_seen.insert(channel),
            "duplicate send channel {channel:?} in WINDOW_ONLY_SENDS"
        );
    }

    for &channel in READ_CHANNELS {
        assert_eq!(desk_access(channel, &[]), Some(DeskAccess::Read), "{channel}");
    }
    for &channel in ACT_CHANNELS {
        assert_eq!(desk_access(channel, &[]), Some(DeskAccess::Act), "{channel}");
    }
    for &channel in WINDOW_ONLY {
        assert_eq!(desk_access(channel, &[]), None, "{channel}");
    }

    let conditional_cases: [(&str, Vec<Value>, DeskAccess, Vec<Value>); 6] = [
        (
            "agent-accounts:call",
            vec![json!({ "kind": "list" })],
            DeskAccess::Read,
            vec![json!({ "kind": "add" })],
        ),
        (
            "mission-approval:decide",
            vec![json!({ "decision": "approve-once" })],
            DeskAccess::Act,
            vec![json!({ "decision": "approve-always" })],
        ),
        (
            "teammates:update",
            vec![json!({ "teammateId": "t", "name": "Ada" })],
            DeskAccess::Act,
            vec![json!({ "teammateId": "t", "monthlyLimitUsd": 500 })],
        ),
        (
            "scout:set",
            vec![json!({ "everyMinutes": 30 })],
            DeskAccess::Act,
            vec![json!({ "fixes": true })],
        ),
        (
            "folder:commit",
            vec![json!("msg"), json!("commit")],
            DeskAccess::Act,
            vec![json!("msg"), json!("push")],
        ),
        (
            "routines:settle",
            vec![json!({ "decision": "keep" })],
            DeskAccess::Act,
            vec![json!({ "decision": "open" })],
        ),
    ];
    let tested_conditionals: HashSet<&str> = conditional_cases.iter().map(|(c, ..)| *c).collect();
    let declared_conditionals: HashSet<&str> = CONDITIONAL_CHANNELS.iter().copied().collect();
    assert_eq!(
        tested_conditionals, declared_conditionals,
        "CONDITIONAL_CHANNELS must match desk_access conditional arms"
    );
    for (channel, allowed_args, expected, refused_args) in conditional_cases {
        assert_eq!(desk_access(channel, &allowed_args), Some(expected), "{channel}");
        assert_eq!(desk_access(channel, &refused_args), None, "{channel}");
    }
}

// case: is made only where it was looked at: a new call to one fails until it is decided and listed
#[test]
fn is_made_only_where_it_was_looked_at_a_new_call_to_one_fails_until_it_is_decided_and_listed() {
    for channel in [DESK_TAKE_BACK_CHANNEL, DESK_WHERE_CHANNEL, PHONE_DESK_CHANNEL] {
        assert!(WINDOW_ONLY.contains(&channel), "{channel}");
        assert_eq!(desk_access(channel, &[]), None, "{channel}");
    }
    assert!(DESK_EVENTS.contains(&MISSION_APPROVAL_CHANNEL));
    assert_eq!(desk_access("unlisted:new-channel", &[]), None);

    // Every WINDOW_ONLY channel, plus an unlisted channel, is refused through DeskHost::handle.
    let invoked = Arc::new(AtomicUsize::new(0));
    let mut handlers: HashMap<String, DeskHandler> = HashMap::new();
    for &channel in WINDOW_ONLY.iter().chain(["unlisted:new-channel"].iter()) {
        let invoked_c = invoked.clone();
        handlers.insert(
            channel.to_owned(),
            Arc::new(move |_, _| {
                invoked_c.fetch_add(1, Ordering::SeqCst);
                Ok(DeskValue::Json(json!({ "ok": true })))
            }),
        );
    }

    let (host, told) = make_host(handlers);
    host.handle("browser-1", &[DeskAsk::Take { id: "take".into() }]);
    let asks: Vec<DeskAsk> = WINDOW_ONLY
        .iter()
        .chain(["unlisted:new-channel"].iter())
        .enumerate()
        .map(|(idx, &channel)| DeskAsk::Call {
            id: format!("refuse-{idx}"),
            channel: channel.to_owned(),
            args: desk_encode_args(&[]),
        })
        .collect();
    let expected_count = asks.len();
    host.handle("browser-1", &asks);
    host.flush();

    assert_eq!(invoked.load(Ordering::SeqCst), 0);
    let replies = told.lock().unwrap();
    let refused_count = replies
        .iter()
        .filter(|tell| {
            matches!(
                tell,
                DeskTell::ResErr {
                    message,
                    refused: true,
                    ..
                } if message == WINDOW_ONLY_SAYS
            )
        })
        .count();
    assert_eq!(refused_count, expected_count);
}
