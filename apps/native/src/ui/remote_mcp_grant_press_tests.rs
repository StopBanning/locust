//! The window answers a real pending request, over a fake relay and with no tool execution.

use super::*;
use locust_core::{
    door_tools::ToolResult,
    relay_crypto::sha256_b64url,
    remote_mcp::{AskKind, McpRelay, RemoteMcpAsk, RemoteMcpReply},
};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf, sync::mpsc};

thread_local! {
    static LOCAL: RefCell<Option<Arc<RemoteMcp>>> = const { RefCell::new(None) };
}

pub(super) fn scoped_service() -> Option<Arc<RemoteMcp>> {
    LOCAL.with(|cell| cell.borrow().clone())
}

struct Scope {
    previous: Option<Arc<RemoteMcp>>,
    remote: Arc<RemoteMcp>,
    root: PathBuf,
}

impl Drop for Scope {
    fn drop(&mut self) {
        LOCAL.with(|cell| cell.replace(self.previous.take()));
        self.remote.dispose();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Relay;
impl McpRelay for Relay {
    fn address(&self) -> Option<String> {
        Some("https://relay.test/v1/room/fake/mcp".into())
    }

    fn open(&self, _: bool) -> bool {
        true
    }
}

const VERIFIER: &str = "FixtureVerifierNotAnAccountOrPassword1234567890";
const CLIENT: &str = "fixture-client";
const REDIRECT: &str = "https://fixture-app.test/return";

fn ask(kind: AskKind, data: Value) -> RemoteMcpAsk {
    RemoteMcpAsk {
        ask: "fixture-request".into(),
        kind,
        body: data.to_string(),
        auth: None,
    }
}

fn press_pending_card(on_settings: bool, allow: bool) {
    let root = std::env::temp_dir().join(format!(
        "locust-grant-press-{}-{}",
        std::process::id(),
        locust_core::invariants::unique_nanos(),
    ));
    let (asked, pending) = mpsc::channel();
    let mut options = Options::new(
        root.join(REMOTE_MCP_FILE),
        Arc::new(Relay),
        Arc::new(|_: &str, _: &Value| ToolResult::refused("No tools in this fixture.")),
    );
    options.asked = Arc::new(move |name| {
        let _ = asked.send(name.to_owned());
    });
    let remote = Arc::new(RemoteMcp::new(options));
    let _scope = Scope {
        previous: LOCAL.with(|cell| cell.replace(Some(remote.clone()))),
        remote: remote.clone(),
        root,
    };
    assert!(remote.set_enabled(&json!(true)).enabled);

    let (settled, reply) = mpsc::channel::<RemoteMcpReply>();
    let remote_worker = remote.clone();
    let worker = std::thread::spawn(move || {
        let result = remote_worker.answer(&ask(
            AskKind::Connect,
            json!({
                "client_id": CLIENT,
                "client": "Fixture app",
                "redirect": REDIRECT,
                "challenge": sha256_b64url(VERIFIER),
                "match": "482913",
            }),
        ));
        let _ = settled.send(result);
    });
    let wait = std::time::Duration::from_secs(10);
    assert_eq!(pending.recv_timeout(wait).unwrap(), "Fixture app");

    let mut state = State::default();
    if on_settings {
        state.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
    }
    assert!(poll(&mut state));
    let mut window = rune::Runtime::new(
        state,
        crate::view,
        (1280.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    window.frame(0.05);
    let said = window
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        said.contains("Fixture app")
            && said.contains("482 913")
            && said.contains("fixture-app.test"),
        "{said}"
    );
    assert!(matches!(reply.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(remote.look().apps.is_empty());

    let control = if allow {
        "remote-connect-allow"
    } else {
        "remote-connect-deny"
    };
    let (x, y) = window.find(control).expect("the card's decision button");
    window.pointer_down(x, y);
    window.pointer_up(x, y);
    window.frame(0.05);
    assert!(
        remote.look().asking.is_empty(),
        "the press must clear its own pending card"
    );
    assert!(window.find("remote-connect").is_none());
    let result = reply
        .recv_timeout(wait)
        .expect("press must answer the relay request");
    worker.join().unwrap();
    let body: Value = serde_json::from_str(&result.body).unwrap();
    if allow {
        assert_eq!(result.status, 200);
        let code = body["code"].as_str().expect("Allow makes a code");
        let trade = remote.answer(&ask(
            AskKind::Token,
            json!({
                "grant_type": "authorization_code", "client_id": CLIENT, "redirect_uri": REDIRECT,
                "code": code, "code_verifier": VERIFIER,
            }),
        ));
        assert_eq!(trade.status, 200, "the fake app must get its grant");
        let apps = remote.look().apps;
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Fixture app");
    } else {
        assert_eq!((result.status, body), (403, json!({"error":"denied"})));
        assert!(remote.look().apps.is_empty());
    }
}

#[test]
fn pressing_allow_on_home_completes_the_waiting_apps_grant() {
    press_pending_card(false, true);
}

#[test]
fn pressing_deny_in_settings_ends_the_request_without_a_grant() {
    press_pending_card(true, false);
}
