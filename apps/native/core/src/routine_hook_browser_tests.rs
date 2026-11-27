use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use serde_json::json;

use super::{ROUTINE_HOOK_NEW, ROUTINE_HOOK_READ};
use crate::desk_host::{
    DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskTell, DeskValue, DeskWindow,
    HandlerRegistry,
};
use crate::phone_anywhere::DeskCallbacks;

const BROWSER: &str = "test-browser";

fn assert_hook_stays_on_the_computer(channel: &str, take_window: bool) {
    let calls = Arc::new(AtomicUsize::new(0));
    let told = Arc::new(Mutex::new(Vec::new()));
    let handlers = HandlerRegistry::new();
    let hook_calls = calls.clone();
    handlers.handle(
        channel,
        Arc::new(move |_, _| {
            hook_calls.fetch_add(1, Ordering::Relaxed);
            Ok(DeskValue::Json(json!("fixture-handler-result")))
        }),
    );

    let answers = told.clone();
    let desk = DeskHost::new(DeskHostDeps {
        handlers: handlers.kept(),
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
        tell: Arc::new(move |browser, replies| {
            assert_eq!(browser, BROWSER);
            answers.lock().unwrap().extend_from_slice(replies);
            true
        }),
        cover: Arc::new(|_| {}),
        hello: Arc::new(|| {
            Ok(DeskHelloInfo {
                version: "test".into(),
                computer: "test".into(),
                theme: "dark".into(),
            })
        }),
        waiting: Arc::new(Vec::new),
        changed: None,
    });

    if take_window {
        desk.frame(BROWSER, vec![json!({"kind":"take","id":"take"})]);
        assert_eq!(desk.active().as_deref(), Some(BROWSER));
        assert!(matches!(
            told.lock().unwrap().as_slice(),
            [DeskTell::ResOk { id, .. }] if id == "take"
        ));
        told.lock().unwrap().clear();
    } else {
        assert_eq!(desk.active(), None);
    }

    // The paired-browser callback parses the frame, checks desk authority, and replies.
    desk.frame(
        BROWSER,
        vec![json!({"kind":"call","id":"hook","channel":channel,"args":"[\"rt_a\"]"})],
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "window handler was called"
    );
    assert_eq!(
        told.lock().unwrap().as_slice(),
        [DeskTell::ResErr {
            id: "hook".into(),
            message: "This works in Locust on your computer.".into(),
            refused: true,
        }]
    );
    assert!(desk.sealed_snapshot(&[0; 32]).is_none());
}

#[test]
// case: is not given to a browser that is only reading
fn is_not_given_to_a_browser_that_is_only_reading() {
    assert_hook_stays_on_the_computer(ROUTINE_HOOK_READ, false);
}

#[test]
// case: is not opened or replaced from a browser
fn is_not_opened_or_replaced_from_a_browser() {
    assert_hook_stays_on_the_computer(ROUTINE_HOOK_NEW, true);
}
