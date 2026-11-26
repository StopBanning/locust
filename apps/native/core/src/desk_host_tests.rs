use crate::desk::WINDOW_ONLY_SAYS;
use crate::desk_host::{
    DeskAsk, DeskEvent, DeskHandler, DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskTell,
    DeskValue, DeskWindow, HandlerRegistry, MISSION_APPROVAL_CHANNEL, desk_decode, desk_encode_args,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const CODEX_MISSION_START_CHANNEL: &str = "codex-mission:start";
const CODEX_MISSION_UPDATE_CHANNEL: &str = "codex-mission:update";
const MISSION_HISTORY_CHANNEL: &str = "mission-history:list";
const FOLDER_PULL_REQUEST_CHANNEL: &str = "folder:pull-request";
const GITHUB_SIGN_IN_CODE_CHANNEL: &str = "locust:github-sign-in-code";
const TERMINAL_CREATE: &str = "terminal:create";

struct TestDesk {
    host: DeskHost,
    told: Arc<Mutex<Vec<(String, Vec<DeskTell>)>>>,
    covers: Arc<Mutex<Vec<bool>>>,
    waiting: Arc<Mutex<Vec<Value>>>,
    sender: DeskSender,
}

impl TestDesk {
    fn new(handlers: HashMap<String, DeskHandler>) -> Self {
        let told = Arc::new(Mutex::new(Vec::new()));
        let covers = Arc::new(Mutex::new(Vec::new()));
        let waiting = Arc::new(Mutex::new(Vec::new()));
        let sender = DeskSender {
            id: 101,
            main_frame_id: 202,
            process_id: 7,
            destroyed: false,
        };
        let win = DeskWindow {
            destroyed: false,
            web_contents: sender.clone(),
        };
        let told_c = told.clone();
        let covers_c = covers.clone();
        let waiting_c = waiting.clone();
        let host = DeskHost::new(DeskHostDeps {
            handlers: Arc::new(Mutex::new(handlers)),
            window: Arc::new(move || Some(win.clone())),
            tell: Arc::new(move |device, items| {
                told_c
                    .lock()
                    .unwrap()
                    .push((device.to_owned(), items.to_vec()));
                true
            }),
            cover: Arc::new(move |on| covers_c.lock().unwrap().push(on)),
            hello: Arc::new(|| {
                Ok(DeskHelloInfo {
                    version: "0.830.0".into(),
                    computer: "DESK-PC".into(),
                    theme: "dark".into(),
                })
            }),
            waiting: Arc::new(move || waiting_c.lock().unwrap().clone()),
            changed: None,
        });
        Self {
            host,
            told,
            covers,
            waiting,
            sender,
        }
    }

    fn flush_all(&self) -> Vec<DeskTell> {
        self.host.flush();
        self.told
            .lock()
            .unwrap()
            .iter()
            .flat_map(|(_, items)| items.clone())
            .collect()
    }

    fn to(&self, device: &str) -> Vec<DeskTell> {
        self.told
            .lock()
            .unwrap()
            .iter()
            .filter(|(d, _)| d == device)
            .flat_map(|(_, items)| items.clone())
            .collect()
    }
}

fn call_ask(id: &str, channel: &str, args: &[DeskValue]) -> DeskAsk {
    DeskAsk::Call {
        id: id.into(),
        channel: channel.into(),
        args: desk_encode_args(args),
    }
}

fn find_answer<'a>(items: &'a [DeskTell], id: &str) -> Option<&'a DeskTell> {
    items.iter().find(|item| match item {
        DeskTell::ResOk { id: i, .. } | DeskTell::ResErr { id: i, .. } => i == id,
        _ => false,
    })
}

#[test]
// case: says what the computer is, and who has the window
fn says_what_the_computer_is_and_who_has_the_window() {
    let d = TestDesk::new(HashMap::new());
    d.host.handle("phone1", &[DeskAsk::Hello { id: "h".into() }]);
    let items = d.flush_all();
    let Some(DeskTell::ResOk { value, .. }) = find_answer(&items, "h") else {
        panic!("expected ResOk");
    };
    assert_eq!(
        desk_decode(value).unwrap(),
        DeskValue::Json(json!({
            "version": "0.830.0",
            "computer": "DESK-PC",
            "theme": "dark",
            "active": "computer"
        }))
    );
}

#[test]
// case: answers a call with the window’s own handler, made as from the window, once it has the window
fn answers_a_call_with_the_windows_own_handler_made_as_from_the_window_once_it_has_the_window() {
    let start_calls: Arc<Mutex<Vec<(DeskEvent, Value)>>> = Arc::new(Mutex::new(Vec::new()));
    let sc = start_calls.clone();
    let mut handlers: HashMap<String, DeskHandler> = HashMap::new();
    handlers.insert(
        CODEX_MISSION_START_CHANNEL.into(),
        Arc::new(move |ev, args| {
            let req = args.first().map(DeskValue::to_json_lossy).unwrap_or(Value::Null);
            sc.lock().unwrap().push((ev.clone(), req.clone()));
            Ok(DeskValue::Json(json!({
                "ok": true,
                "missionId": "m1",
                "request": req
            })))
        }),
    );
    handlers.insert(
        MISSION_HISTORY_CHANNEL.into(),
        Arc::new(|_, _| Ok(DeskValue::Json(json!({ "ok": true, "missions": [] })))),
    );
    handlers.insert(
        FOLDER_PULL_REQUEST_CHANNEL.into(),
        Arc::new(|_, _| Ok(DeskValue::Undefined)),
    );

    let d = TestDesk::new(handlers);
    // Reading needs no window of its own; acting does.
    d.host.handle(
        "laptop",
        &[
            call_ask("r1", MISSION_HISTORY_CHANNEL, &[]),
            call_ask("a1", CODEX_MISSION_START_CHANNEL, &[DeskValue::Json(json!({ "prompt": "hi" }))]),
        ],
    );
    let items = d.flush_all();
    assert!(matches!(find_answer(&items, "r1"), Some(DeskTell::ResOk { .. })));
    assert_eq!(
        find_answer(&items, "a1"),
        Some(&DeskTell::ResErr {
            id: "a1".into(),
            message: "Locust is open somewhere else now. Open it here again to do that.".into(),
            refused: true,
        })
    );
    assert!(start_calls.lock().unwrap().is_empty());

    d.host.handle(
        "laptop",
        &[
            DeskAsk::Take { id: "t".into() },
            call_ask("a2", CODEX_MISSION_START_CHANNEL, &[DeskValue::Json(json!({ "prompt": "hi" }))]),
            call_ask("u", FOLDER_PULL_REQUEST_CHANNEL, &[]),
        ],
    );
    let items = d.flush_all();
    assert_eq!(*d.covers.lock().unwrap(), vec![true]);
    {
        let calls = start_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0.sender_id, d.sender.id);
        assert_eq!(calls[0].0.sender_frame_id, d.sender.main_frame_id);
        assert_eq!(calls[0].0.process_id, d.sender.process_id);
    }
    let Some(DeskTell::ResOk { value, .. }) = find_answer(&items, "a2") else {
        panic!("expected a2 ok");
    };
    assert_eq!(
        desk_decode(value).unwrap(),
        DeskValue::Json(json!({ "ok": true, "missionId": "m1", "request": { "prompt": "hi" } }))
    );
    // An answer of nothing is Undefined, not null.
    let Some(DeskTell::ResOk { value: u_val, .. }) = find_answer(&items, "u") else {
        panic!("expected u ok");
    };
    assert_eq!(desk_decode(u_val).unwrap(), DeskValue::Undefined);
}

#[test]
// case: refuses in words what stays on this computer, a call it does not know, and says a handler’s own failure
fn refuses_in_words_what_stays_on_this_computer_a_call_it_does_not_know_and_says_a_handlers_own_failure()
{
    let term_calls = Arc::new(AtomicUsize::new(0));
    let tc = term_calls.clone();
    let mut handlers: HashMap<String, DeskHandler> = HashMap::new();
    handlers.insert(
        TERMINAL_CREATE.into(),
        Arc::new(move |_, _| {
            tc.fetch_add(1, Ordering::SeqCst);
            Ok(DeskValue::Undefined)
        }),
    );
    handlers.insert(
        MISSION_HISTORY_CHANNEL.into(),
        Arc::new(|_, _| Err("The ledger could not be read.".into())),
    );
    let d = TestDesk::new(handlers);
    d.host.handle(
        "laptop",
        &[
            DeskAsk::Take { id: "t".into() },
            call_ask("w", TERMINAL_CREATE, &[DeskValue::Json(json!({}))]),
            call_ask("x", "no:such-call", &[]),
            call_ask("e", MISSION_HISTORY_CHANNEL, &[]),
        ],
    );
    let items = d.flush_all();
    assert_eq!(
        find_answer(&items, "w"),
        Some(&DeskTell::ResErr {
            id: "w".into(),
            message: WINDOW_ONLY_SAYS.into(),
            refused: true,
        })
    );
    assert_eq!(term_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        find_answer(&items, "x"),
        Some(DeskTell::ResErr { refused: true, .. })
    ));
    assert_eq!(
        find_answer(&items, "e"),
        Some(&DeskTell::ResErr {
            id: "e".into(),
            message: "The ledger could not be read.".into(),
            refused: false,
        })
    );
}

#[test]
// case: tells the browser that has the window what the window is told, and nothing it may not hear
fn tells_the_browser_that_has_the_window_what_the_window_is_told_and_nothing_it_may_not_hear() {
    let d = TestDesk::new(HashMap::new());
    d.host.mirror(
        CODEX_MISSION_UPDATE_CHANNEL,
        &[DeskValue::Json(json!({ "kind": "event" }))],
    );
    d.flush_all();
    assert!(d.told.lock().unwrap().is_empty());

    d.host.handle("laptop", &[DeskAsk::Take { id: "t".into() }]);
    d.host.mirror(
        CODEX_MISSION_UPDATE_CHANNEL,
        &[DeskValue::Json(json!({ "kind": "event", "n": 1 }))],
    );
    d.host.mirror(
        GITHUB_SIGN_IN_CODE_CHANNEL,
        &[DeskValue::Json(json!({ "code": "ABCD-1234" }))],
    );
    let items = d.flush_all();
    let events: Vec<&DeskTell> = items
        .iter()
        .filter(|i| matches!(i, DeskTell::Ev { .. }))
        .collect();
    assert_eq!(
        events,
        vec![&DeskTell::Ev {
            channel: CODEX_MISSION_UPDATE_CHANNEL.into(),
            args: desk_encode_args(&[DeskValue::Json(json!({ "kind": "event", "n": 1 }))]),
        }]
    );
    let debug = format!("{items:?}");
    assert!(!debug.contains("ABCD-1234"));
}

#[test]
// case: lets one window act at a time: another browser takes it, this computer takes it back, each is told
fn lets_one_window_act_at_a_time_another_browser_takes_it_this_computer_takes_it_back_each_is_told()
{
    let d = TestDesk::new(HashMap::new());
    d.host.handle("laptop", &[DeskAsk::Take { id: "t1".into() }]);
    d.host.handle("tablet", &[DeskAsk::Take { id: "t2".into() }]);
    d.flush_all();
    assert!(d.to("laptop").contains(&DeskTell::Taken { by: "browser".into() }));
    assert_eq!(d.host.active().as_deref(), Some("tablet"));
    assert_eq!(*d.covers.lock().unwrap(), vec![true]);

    d.host.take_back();
    d.flush_all();
    assert!(d.to("tablet").contains(&DeskTell::Taken { by: "computer".into() }));
    assert_eq!(*d.covers.lock().unwrap(), vec![true, false]);
    assert_eq!(d.host.active(), None);
}

#[test]
// case: lets the window go when the browser that has it may no longer have it
fn lets_the_window_go_when_the_browser_that_has_it_may_no_longer_have_it() {
    let d = TestDesk::new(HashMap::new());
    d.host.handle("laptop", &[DeskAsk::Take { id: "t".into() }]);
    d.host.forget("laptop");
    assert_eq!(d.host.active(), None);
    assert_eq!(*d.covers.lock().unwrap(), vec![true, false]);
}

#[test]
// case: gives a window just opened the approval cards already waiting
fn gives_a_window_just_opened_the_approval_cards_already_waiting() {
    let d = TestDesk::new(HashMap::new());
    d.waiting
        .lock()
        .unwrap()
        .push(json!({ "approvalId": "ap1", "summary": "npm test" }));
    d.host.handle(
        "laptop",
        &[DeskAsk::Replay {
            id: "r0".into(),
            what: "approvals".into(),
        }],
    );
    let first = d.flush_all();
    assert!(!first.iter().any(|i| matches!(i, DeskTell::Ev { .. })));

    d.host.handle(
        "laptop",
        &[
            DeskAsk::Take { id: "t".into() },
            DeskAsk::Replay {
                id: "r1".into(),
                what: "approvals".into(),
            },
        ],
    );
    let items = d.flush_all();
    assert!(items.contains(&DeskTell::Ev {
        channel: MISSION_APPROVAL_CHANNEL.into(),
        args: desk_encode_args(&[DeskValue::Json(
            json!({ "approvalId": "ap1", "summary": "npm test" })
        )]),
    }));
}

#[test]
// case: keeps every handler ipcMain is given, as it is given them
fn keeps_every_handler_ipc_main_is_given_as_it_is_given_them() {
    let ipc = HandlerRegistry::new();
    let kept = ipc.kept();
    let handler: DeskHandler = Arc::new(|_, _| Ok(DeskValue::Json(json!(1))));
    ipc.handle("a:b", handler);
    assert!(ipc.registered_has("a:b"));
    assert!(kept.lock().unwrap().contains_key("a:b"));
    ipc.remove_handler("a:b");
    assert!(!kept.lock().unwrap().contains_key("a:b"));
}

// ---- on the phone link: a browser's ask, read as shared/desk.ts `isDeskAsk` reads it ---------------------------------

#[test]
fn reads_a_browsers_ask_as_isdeskask_does_by_its_id_its_kind_and_a_calls_channel() {
    use crate::desk_host::desk_ask_of;
    let id64 = "a".repeat(64);
    assert_eq!(desk_ask_of(&json!({ "kind": "hello", "id": id64 })), Some(DeskAsk::Hello { id: id64.clone() }));
    assert_eq!(desk_ask_of(&json!({ "kind": "take", "id": "t1" })), Some(DeskAsk::Take { id: "t1".into() }));
    // ask.id.length counts UTF-16 units: 32 emoji are 64, 33 are 66.
    assert!(desk_ask_of(&json!({ "kind": "hello", "id": "\u{1F600}".repeat(32) })).is_some());
    assert_eq!(desk_ask_of(&json!({ "kind": "hello", "id": "\u{1F600}".repeat(33) })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "hello", "id": "a".repeat(65) })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "hello", "id": "" })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "hello", "id": 7 })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "hello" })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "nonsense", "id": "x1" })), None);
    assert_eq!(desk_ask_of(&json!("hello")), None);

    // replay: only what is 'approvals'
    assert_eq!(
        desk_ask_of(&json!({ "kind": "replay", "id": "r1", "what": "approvals" })),
        Some(DeskAsk::Replay { id: "r1".into(), what: "approvals".into() })
    );
    assert_eq!(desk_ask_of(&json!({ "kind": "replay", "id": "r1", "what": "everything" })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "replay", "id": "r1" })), None);

    // call: channel 1 to 120 long, args a string
    let channel120 = "c".repeat(120);
    assert_eq!(
        desk_ask_of(&json!({ "kind": "call", "id": "c1", "channel": channel120, "args": "[]" })),
        Some(DeskAsk::Call { id: "c1".into(), channel: channel120.clone(), args: "[]".into() })
    );
    assert_eq!(desk_ask_of(&json!({ "kind": "call", "id": "c1", "channel": "c".repeat(121), "args": "[]" })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "call", "id": "c1", "channel": "", "args": "[]" })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "call", "id": "c1", "channel": "appearance:get" })), None);
    assert_eq!(desk_ask_of(&json!({ "kind": "call", "id": "c1", "channel": "appearance:get", "args": [] })), None);
}

#[test]
fn tells_a_browser_in_the_four_shapes_shared_desk_ts_names() {
    use crate::desk_host::desk_tell_json;
    assert_eq!(
        desk_tell_json(&DeskTell::ResOk { id: "c1".into(), value: "true".into() }),
        json!({ "kind": "res", "id": "c1", "ok": true, "value": "true" })
    );
    assert_eq!(
        desk_tell_json(&DeskTell::ResErr { id: "c2".into(), message: "No such call.".into(), refused: false }),
        json!({ "kind": "res", "id": "c2", "ok": false, "message": "No such call." })
    );
    assert_eq!(
        desk_tell_json(&DeskTell::ResErr { id: "c3".into(), message: "This works in Locust on your computer.".into(), refused: true }),
        json!({ "kind": "res", "id": "c3", "ok": false, "message": "This works in Locust on your computer.", "refused": true })
    );
    assert_eq!(
        desk_tell_json(&DeskTell::Ev { channel: "codex-mission:update".into(), args: "[1]".into() }),
        json!({ "kind": "ev", "channel": "codex-mission:update", "args": "[1]" })
    );
    assert_eq!(desk_tell_json(&DeskTell::Taken { by: "browser".into() }), json!({ "kind": "taken", "by": "browser" }));
}
