//! The line to the relay over a WebSocket client (Electron `webRelaySocket` in main/phone-anywhere.ts): a fake client
//! here, never the network.

use super::relay_connector;
use crate::chat_http::{WsCallbacks, WsClient, WsConnection};
use crate::phone_anywhere::{PhoneAnywhere, PhoneAnywhereOptions, RelaySocketCallbacks};
use crate::phone_host::{PhoneCalls, PhoneSent, PhoneState, PhoneTurn};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A WebSocket client that connects nowhere: it keeps what it was given and what was sent on it.
#[derive(Default)]
struct FakeClient {
    urls: Mutex<Vec<String>>,
    heard: Mutex<Vec<Arc<dyn WsCallbacks>>>,
    sent: Arc<Mutex<Vec<String>>>,
    closed: Arc<Mutex<Vec<u16>>>,
    /// Answers the PC's auth as the relay does, on the line it came on.
    relay: bool,
    refuse: bool,
}

struct FakeConnection {
    sent: Arc<Mutex<Vec<String>>>,
    closed: Arc<Mutex<Vec<u16>>>,
    heard: Arc<dyn WsCallbacks>,
    relay: bool,
}

impl WsConnection for FakeConnection {
    fn send_text(&self, text: &str) -> Result<(), String> {
        self.sent.lock().unwrap().push(text.to_owned());
        if self.relay && serde_json::from_str::<Value>(text).is_ok_and(|frame| frame["t"] == "auth") {
            let heard = self.heard.clone();
            std::thread::spawn(move || heard.on_message(json!({ "t": "ok", "as": "pc", "devices": [] }).to_string()));
        }
        Ok(())
    }
    fn close(&self, code: u16, _reason: &str) {
        self.closed.lock().unwrap().push(code);
    }
}

impl WsClient for FakeClient {
    fn connect(&self, url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
        if self.refuse {
            return Err("offline".into());
        }
        self.urls.lock().unwrap().push(url.to_owned());
        self.heard.lock().unwrap().push(callbacks.clone());
        Ok(Arc::new(FakeConnection { sent: self.sent.clone(), closed: self.closed.clone(), heard: callbacks, relay: self.relay }))
    }
}

impl FakeClient {
    fn line(&self, n: usize) -> Arc<dyn WsCallbacks> {
        self.heard.lock().unwrap()[n].clone()
    }
}

/// What the phone link is told, in order.
#[derive(Default)]
struct Told(Mutex<Vec<String>>);

impl RelaySocketCallbacks for Told {
    fn on_open(&self) {
        self.0.lock().unwrap().push("open".into());
    }
    fn on_message(&self, text: String) {
        self.0.lock().unwrap().push(text);
    }
    fn on_close(&self) {
        self.0.lock().unwrap().push("close".into());
    }
}

impl Told {
    fn said(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

#[test]
fn is_told_it_opened_once_when_its_callbacks_are_set_and_hears_a_frame_from_before_then_in_order() {
    let client = Arc::new(FakeClient::default());
    let socket = relay_connector(client.clone())("wss://relay.test/v1/room/r").unwrap();
    assert_eq!(client.urls.lock().unwrap().as_slice(), ["wss://relay.test/v1/room/r"]);
    let line = client.line(0);
    line.on_message("first".into());
    line.on_message("second".into());
    let told = Arc::new(Told::default());
    socket.set_callbacks(told.clone());
    until("the open and the kept frames", || told.said().len() == 3);
    line.on_message("third".into());
    until("the frame after", || told.said().len() == 4);
    // Set again, the line does not open again.
    socket.set_callbacks(told.clone());
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(told.said(), ["open", "first", "second", "third"]);
}

#[test]
fn is_told_its_close_once_and_nothing_heard_after_it() {
    let client = Arc::new(FakeClient::default());
    let socket = relay_connector(client.clone())("wss://relay.test/v1/room/r").unwrap();
    let line = client.line(0);
    line.on_message("before".into());
    line.on_close(1006);
    let told = Arc::new(Told::default());
    socket.set_callbacks(told.clone());
    until("the close", || told.said().last().is_some_and(|last| last == "close"));
    line.on_close(1000);
    line.on_message("late".into());
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(told.said(), ["open", "before", "close"]);
}

#[test]
fn sends_and_closes_on_its_websocket_and_a_line_that_cannot_be_made_is_an_error() {
    let client = Arc::new(FakeClient::default());
    let socket = relay_connector(client.clone())("wss://relay.test/v1/room/r").unwrap();
    socket.send("ping".into()).unwrap();
    socket.close();
    assert_eq!(client.sent.lock().unwrap().as_slice(), ["ping"]);
    assert_eq!(client.closed.lock().unwrap().as_slice(), [1000]);
    let refusing = Arc::new(FakeClient { refuse: true, ..FakeClient::default() });
    assert!(relay_connector(refusing)("wss://relay.test/v1/room/r").is_err());
}

struct NoCalls;
impl PhoneCalls for NoCalls {
    fn state(&self) -> PhoneState {
        PhoneState { waiting: Vec::new(), conversations: Vec::new(), scout: None }
    }
    fn turns(&self, _mission_id: &str) -> Option<Vec<PhoneTurn>> {
        None
    }
    fn answer_card(&self, _approval_id: &str, _approve: bool) -> bool {
        false
    }
    fn send(&self, _mission_id: &str, _text: &str) -> PhoneSent {
        PhoneSent::Ok
    }
}

#[test]
fn the_phone_link_over_it_signs_in_as_this_pc_once_the_line_opens_and_connects_again_after_it_closes() {
    let dir = std::env::temp_dir().join(format!("locust-relay-socket-{}", crate::relay_crypto::random_text(10)));
    std::fs::create_dir_all(&dir).unwrap();
    let client = Arc::new(FakeClient { relay: true, ..FakeClient::default() });
    let changes = Arc::new(AtomicUsize::new(0));
    let counted = changes.clone();
    let pc = PhoneAnywhere::new(PhoneAnywhereOptions {
        directory: dir.clone(),
        calls: Arc::new(NoCalls),
        relay: Arc::new(|| "https://relay.test".to_owned()),
        refusal: None,
        changed: Some(Arc::new(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
        })),
        connect: relay_connector(client.clone()),
        now: None,
        protect_file: Some(Arc::new(|_| Ok(()))),
        retry_ms: Some(Arc::new(|_| 20)),
        away: None,
        hooks: None,
        desk: None,
        notes: None,
        github_user: None,
        computer_name: None,
    });
    pc.set_enabled(true);
    until("the relay's ok", || pc.connected());
    let room = pc.room().unwrap().1;
    // Electron: `${relay.replace(/^http/, 'ws')}/v1/room/${room}`, then { t: 'auth', as: 'pc', token } on open.
    assert_eq!(client.urls.lock().unwrap()[0], format!("wss://relay.test/v1/room/{room}"));
    let auth: Value = serde_json::from_str(&client.sent.lock().unwrap()[0]).unwrap();
    assert_eq!((auth["t"].as_str(), auth["as"].as_str()), (Some("auth"), Some("pc")));
    assert!(auth["token"].as_str().is_some_and(|token| !token.is_empty()));
    client.line(0).on_close(1006);
    until("a second line", || client.urls.lock().unwrap().len() == 2 && pc.connected());
    assert_eq!(client.sent.lock().unwrap().iter().filter(|text| text.contains("\"auth\"")).count(), 2);
    pc.dispose();
    let _ = std::fs::remove_dir_all(&dir);
}
