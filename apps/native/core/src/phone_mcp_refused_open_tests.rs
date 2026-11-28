//! An open of the apps channel that the relay refused is not remembered as open: the next open on the same
//! connection asks the relay again (the window's phone_section::tests::apps_can_be_enabled_after_the_relay_refuses_one_open
//! sees the same from the settings switch).
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

/// A relay that refuses the first command it is sent and acknowledges every one after it.
struct RefusesFirst {
    owner: std::sync::Weak<PhoneAnywhereInner>,
    sent: Mutex<Vec<String>>,
}
impl RelaySocket for RefusesFirst {
    fn send(&self, text: String) -> Result<(), String> {
        let frame: Value = serde_json::from_str(&text).unwrap();
        let first = {
            let mut sent = self.sent.lock().unwrap();
            sent.push(frame["t"].as_str().unwrap().to_owned());
            sent.len() == 1
        };
        let reply = if first {
            json!({ "t": "err", "id": frame["id"], "code": "refused" })
        } else {
            json!({ "t": "done", "id": frame["id"] })
        };
        self.owner.upgrade().unwrap().heard_frame(CURRENT, &reply.to_string());
        Ok(())
    }
    fn close(&self) {}
    fn set_callbacks(&self, _: Arc<dyn RelaySocketCallbacks>) {}
}

fn connected() -> (PhoneAnywhere, Arc<RefusesFirst>) {
    let phone = PhoneAnywhere::new(PhoneAnywhereOptions {
        directory: PathBuf::new(),
        calls: Arc::new(NoCalls),
        relay: Arc::new(|| "https://relay.invalid".into()),
        refusal: None, changed: None,
        connect: Arc::new(|_| panic!("the socket is already installed")),
        now: None, protect_file: None, retry_ms: None, away: None, hooks: None,
        desk: None, notes: None, github_user: None, computer_name: None, mcp: None,
    });
    let wire = Arc::new(RefusesFirst { owner: Arc::downgrade(&phone.inner), sent: Mutex::new(Vec::new()) });
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
fn an_open_the_relay_refused_is_asked_again_on_the_same_connection() {
    let (phone, wire) = connected();
    assert!(!phone.mcp_open(true), "the relay refused the first open");
    // Remembered as open, the second press would say yes without asking: apps "on" while the relay turns them away.
    assert!(phone.mcp_open(true), "the second open is the relay's own answer");
    assert_eq!(wire.sent.lock().unwrap().as_slice(), ["mcp-open", "mcp-open"]);
}
