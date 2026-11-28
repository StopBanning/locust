//! Phone MCP command routing with an already-authenticated, in-memory socket.
use super::*;
use crate::phone_host::{PhoneSent, PhoneState, PhoneTurn};

const CURRENT: u64 = 2;

struct NoCalls;
impl PhoneCalls for NoCalls {
    fn state(&self) -> PhoneState {
        PhoneState { waiting: Vec::new(), conversations: Vec::new(), scout: None }
    }
    fn turns(&self, _: &str) -> Option<Vec<PhoneTurn>> { None }
    fn answer_card(&self, _: &str, _: bool) -> bool { false }
    fn send(&self, _: &str, _: &str) -> PhoneSent { PhoneSent::Ok }
}

struct AcknowledgingWire {
    owner: std::sync::Weak<PhoneAnywhereInner>,
    sent: Mutex<Vec<String>>,
}
impl RelaySocket for AcknowledgingWire {
    fn send(&self, text: String) -> Result<(), String> {
        let frame: Value = serde_json::from_str(&text).unwrap();
        self.sent.lock().unwrap().push(frame["t"].as_str().unwrap().to_owned());
        let reply = json!({ "t": "done", "id": frame["id"] });
        self.owner.upgrade().unwrap().heard_frame(CURRENT, &reply.to_string());
        Ok(())
    }
    fn close(&self) {}
    fn set_callbacks(&self, _: Arc<dyn RelaySocketCallbacks>) {}
}

fn authenticated() -> (PhoneAnywhere, Arc<AcknowledgingWire>) {
    let phone = PhoneAnywhere::new(PhoneAnywhereOptions {
        directory: PathBuf::new(),
        calls: Arc::new(NoCalls),
        relay: Arc::new(|| "https://relay.invalid".into()),
        refusal: None, changed: None,
        connect: Arc::new(|_| panic!("the socket is already installed")),
        now: None, protect_file: None, retry_ms: None, away: None, hooks: None,
        desk: None, notes: None, github_user: None, computer_name: None, mcp: None,
    });
    let wire = Arc::new(AcknowledgingWire { owner: Arc::downgrade(&phone.inner), sent: Mutex::new(Vec::new()) });
    {
        let mut st = phone.inner.state.lock().unwrap();
        st.socket = Some((CURRENT, wire.clone()));
        st.connected = true;
        st.saved = Some(Saved {
            relay: "https://relay.invalid".into(), room: "fixture room".into(), token: String::new(),
            private_key: None, public_key: String::new(), devices: Vec::new(), night_approvals: false,
            notes_taken: Vec::new(), closing: false, github: None,
        });
    }
    (phone, wire)
}

#[test]
fn a_stale_mcp_command_never_writes_to_its_replacement_socket() {
    let (phone, wire) = authenticated();
    let send = |generation| phone.inner.answer_of_on(json!({ "t": "mcp-open" }), Some(generation));
    // The command captured generation 1; only generation 2 is now authenticated.
    assert!(matches!(send(CURRENT - 1), CommandAnswer::Failed));
    assert!(wire.sent.lock().unwrap().is_empty(), "no bytes cross to the new socket");
    assert!(matches!(send(CURRENT), CommandAnswer::Ok));
    assert_eq!(wire.sent.lock().unwrap().as_slice(), ["mcp-open"]);
}

#[test]
fn a_deduplicated_mcp_open_still_reports_success_without_writing() {
    let (phone, wire) = authenticated();
    assert!(phone.mcp_open(true), "the relay acknowledged the first open");
    assert!(phone.mcp_open(true), "the same socket's acknowledged open is still success");
    assert_eq!(wire.sent.lock().unwrap().as_slice(), ["mcp-open"]);
}
