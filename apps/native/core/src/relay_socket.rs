//! THE LINE TO LOCUST'S RELAY (Electron `main/phone-anywhere.ts`, `webRelaySocket`, 0.826).
//!
//! One connection to the relay carries the phone, the routine links and other apps. Electron opens it with Node's own
//! WebSocket; here it is any [`WsClient`] -- on macOS the NSURLSession client the chat apps use -- turned into the phone
//! link's [`RelaySocket`].
//!
//! A `WsClient` hands its frames to callbacks it is given when it connects, and says nothing when the line opens; the
//! phone link gives its callbacks only once the connection is made. So the line is told it opened once, when its
//! callbacks are set, a frame heard before then is kept and delivered after that in the order it came, and the close is
//! told once. An error is always followed by a close, which is where it is handled.

use crate::chat_http::{WsCallbacks, WsClient, WsConnection};
use crate::phone_anywhere::{RelayConnector, RelaySocket, RelaySocketCallbacks};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// The phone link's connector over a WebSocket client: each call opens one line to `url`.
pub fn relay_connector(client: Arc<dyn WsClient>) -> RelayConnector {
    Arc::new(move |url: &str| {
        let line = Arc::new(Line::default());
        let connection = client.connect(url, Arc::new(Heard(line.clone())))?;
        Ok(Arc::new(Socket { connection, line }) as Arc<dyn RelaySocket>)
    })
}

enum Event {
    Open,
    Message(String),
    Close,
}

#[derive(Default)]
struct Waiting {
    callbacks: Option<Arc<dyn RelaySocketCallbacks>>,
    events: VecDeque<Event>,
    /// One delivery at a time, so the order heard is the order told, whichever thread hears it.
    delivering: bool,
    closed: bool,
}

#[derive(Default)]
struct Line {
    waiting: Mutex<Waiting>,
}

impl Line {
    /// Kept until the callbacks are set, then told in order; a close is told once.
    fn heard(&self, event: Event) {
        {
            let mut waiting = self.waiting.lock().unwrap_or_else(|e| e.into_inner());
            if waiting.closed {
                return;
            }
            if matches!(event, Event::Close) {
                waiting.closed = true;
            }
            waiting.events.push_back(event);
            if waiting.callbacks.is_none() || waiting.delivering {
                return;
            }
            waiting.delivering = true;
        }
        self.deliver();
    }

    fn deliver(&self) {
        loop {
            let (callbacks, event) = {
                let mut waiting = self.waiting.lock().unwrap_or_else(|e| e.into_inner());
                match (waiting.callbacks.clone(), waiting.events.pop_front()) {
                    (Some(callbacks), Some(event)) => (callbacks, event),
                    _ => {
                        waiting.delivering = false;
                        return;
                    }
                }
            };
            match event {
                Event::Open => callbacks.on_open(),
                Event::Message(text) => callbacks.on_message(text),
                Event::Close => callbacks.on_close(),
            }
        }
    }
}

/// What the client hears on the line.
struct Heard(Arc<Line>);

impl WsCallbacks for Heard {
    fn on_message(&self, text: String) {
        self.0.heard(Event::Message(text));
    }
    fn on_close(&self, _code: u16) {
        self.0.heard(Event::Close);
    }
}

struct Socket {
    connection: Arc<dyn WsConnection>,
    line: Arc<Line>,
}

impl RelaySocket for Socket {
    fn send(&self, text: String) -> Result<(), String> {
        self.connection.send_text(&text)
    }

    fn close(&self) {
        self.connection.close(1000, "");
    }

    fn set_callbacks(&self, callbacks: Arc<dyn RelaySocketCallbacks>) {
        {
            let mut waiting = self.line.waiting.lock().unwrap_or_else(|e| e.into_inner());
            if waiting.callbacks.is_some() {
                return;
            }
            waiting.callbacks = Some(callbacks);
            // Opened first, then whatever was heard before now, in the order it came.
            waiting.events.push_front(Event::Open);
            waiting.delivering = true;
        }
        // The open is answered with a send, which waits for the line: never on the caller's thread, which may be the
        // window's.
        let line = self.line.clone();
        std::thread::spawn(move || line.deliver());
    }
}

#[cfg(test)]
#[path = "relay_socket_tests.rs"]
mod tests;
