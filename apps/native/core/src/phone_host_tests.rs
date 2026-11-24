//! Carries:
//! - `apps/desktop/src/main/a-phone-is-paired-by-its-code.test.ts` (all 9 tests)
//! - `apps/desktop/src/main/a-managed-turn-is-refused-before-an-agent-starts.test.ts` (phone host test)
//! - `apps/desktop/src/main/scouts-cards-are-on-your-phone.test.ts` (tests 1 & 2)
//! - `apps/desktop/src/renderer/src/a-phone-lists-conversations.test.ts` (all 4 tests)
//! - `apps/desktop/src/main/the-phone-reads-history-once-per-change.test.ts` (both tests)
//! - SecretBox & error-scrubbing invariants for the pairing key.

use super::*;
use crate::phone_page::{PhonePageVia, phone_page};
use crate::scout::types::{ScoutDraft, ScoutFact};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU16, AtomicUsize, Ordering};

// ---- Fake SecretBox (never touches real keychain) ------------------------------------------------------------------

struct FakeSecretBox {
    available: bool,
    fail_seal: AtomicBool,
    sealed: Mutex<HashMap<String, String>>,
    next: AtomicUsize,
}

impl FakeSecretBox {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            available: true,
            fail_seal: AtomicBool::new(false),
            sealed: Mutex::new(HashMap::new()),
            next: AtomicUsize::new(1),
        })
    }

    fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            available: false,
            fail_seal: AtomicBool::new(false),
            sealed: Mutex::new(HashMap::new()),
            next: AtomicUsize::new(1),
        })
    }
}

impl SecretBox for FakeSecretBox {
    fn available(&self) -> bool {
        self.available
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        if !self.available || self.fail_seal.load(Ordering::Relaxed) {
            return Err("fake box failed".into());
        }
        let id = format!("fake-box:{}", self.next.fetch_add(1, Ordering::Relaxed));
        self.sealed
            .lock()
            .unwrap()
            .insert(id.clone(), text.to_owned());
        Ok(id)
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        if !self.available {
            return Err("unavailable".into());
        }
        self.sealed
            .lock()
            .unwrap()
            .get(sealed)
            .cloned()
            .ok_or_else(|| "missing key".into())
    }
    fn forget(&self, sealed: &str) {
        self.sealed.lock().unwrap().remove(sealed);
    }
}

// ---- Fake Listener (never binds a real port) -----------------------------------------------------------------------

struct FakeBound {
    port: u16,
    stopped: AtomicBool,
}

impl PhoneBound for FakeBound {
    fn port(&self) -> u16 {
        self.port
    }
    fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

struct FakeListenerFactory {
    next_port: AtomicU16,
    active_handler: Mutex<Option<Arc<PhoneRequestHandler>>>,
    bind_count: AtomicUsize,
}

impl FakeListenerFactory {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            next_port: AtomicU16::new(47740),
            active_handler: Mutex::new(None),
            bind_count: AtomicUsize::new(0),
        })
    }

    fn request(&self, req: PhoneHttpRequest) -> PhoneHttpResponse {
        let handler = self
            .active_handler
            .lock()
            .unwrap()
            .clone()
            .expect("listener is bound");
        handler.handle(&req)
    }
}

impl PhoneListenerFactory for FakeListenerFactory {
    fn bind(
        &self,
        port: u16,
        handler: Arc<PhoneRequestHandler>,
    ) -> Result<Box<dyn PhoneBound>, String> {
        self.bind_count.fetch_add(1, Ordering::Relaxed);
        let assigned = if port == 0 {
            self.next_port.fetch_add(1, Ordering::Relaxed)
        } else {
            port
        };
        *self.active_handler.lock().unwrap() = Some(handler);
        Ok(Box::new(FakeBound {
            port: assigned,
            stopped: AtomicBool::new(false),
        }))
    }
}

// ---- Fake PhoneCalls -----------------------------------------------------------------------------------------------

struct FakeCalls {
    decided: Mutex<Vec<(String, bool)>>,
    sent: Mutex<Vec<(String, String)>>,
    dismissed: Mutex<Vec<String>>,
}

impl FakeCalls {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            decided: Mutex::new(Vec::new()),
            sent: Mutex::new(Vec::new()),
            dismissed: Mutex::new(Vec::new()),
        })
    }
}

impl PhoneCalls for FakeCalls {
    fn state(&self) -> PhoneState {
        PhoneState {
            waiting: vec![PhoneWaiting {
                approval_id: "a1".into(),
                mission_id: "m1".into(),
                summary: "Run npm test".into(),
                requested_at: "2026-10-15T12:00:00.000Z".into(),
                name: None,
                exact: None,
                plain: None,
            }],
            conversations: Vec::new(),
            scout: None,
        }
    }
    fn turns(&self, mission_id: &str) -> Option<Vec<PhoneTurn>> {
        (mission_id == "m1").then(|| {
            vec![PhoneTurn {
                mission_id: "m1".into(),
                asked: Some("hi".into()),
                answer: Some("hello".into()),
                agent: "Codex CLI".into(),
                at: "2026-10-15T12:00:00.000Z".into(),
                phase: "completed".into(),
            }]
        })
    }
    fn answer_card(&self, approval_id: &str, approve: bool) -> bool {
        self.decided.lock().unwrap().push((approval_id.to_owned(), approve));
        true
    }
    fn send(&self, mission_id: &str, text: &str) -> PhoneSent {
        self.sent.lock().unwrap().push((mission_id.to_owned(), text.to_owned()));
        PhoneSent::Ok
    }
    fn dismiss_scout(&self, card_id: &str) -> Option<bool> {
        self.dismissed.lock().unwrap().push(card_id.to_owned());
        Some(true)
    }
}

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "locust-phone-host-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn harness() -> (
    TempDir,
    PhoneHost,
    Arc<FakeCalls>,
    Arc<FakeSecretBox>,
    Arc<FakeListenerFactory>,
    Arc<AtomicI64>,
) {
    let dir = TempDir::new("h");
    let calls = FakeCalls::new();
    let secrets = FakeSecretBox::new();
    let listener = FakeListenerFactory::new();
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let clock_clone = clock.clone();
    let host = PhoneHost::new(PhoneHostOptions {
        directory: dir.0.clone(),
        page: Arc::new(|| phone_page(PhonePageVia::Lan, None)),
        calls: calls.clone(),
        secrets: secrets.clone(),
        listener: listener.clone(),
        refusal: Arc::new(|| None),
        addresses: Arc::new(|| vec!["127.0.0.1".into()]),
        port: 0,
        now_ms: Arc::new(move || clock_clone.load(Ordering::Relaxed)),
    });
    (dir, host, calls, secrets, listener, clock)
}

fn key_of(url: Option<&str>) -> String {
    url.unwrap_or("")
        .split('#')
        .nth(1)
        .unwrap_or("")
        .to_owned()
}

fn port_of(url: Option<&str>) -> u16 {
    let u = url::Url::parse(url.unwrap_or("")).unwrap();
    u.port().unwrap()
}

fn get_req(port: u16, path: &str, key: Option<&str>) -> PhoneHttpRequest {
    let mut headers = vec![("host".into(), format!("127.0.0.1:{port}").into_bytes())];
    if let Some(k) = key {
        headers.push(("authorization".into(), format!("Bearer {k}").into_bytes()));
    }
    PhoneHttpRequest {
        method: "GET".into(),
        path: path.into(),
        headers,
        body: Vec::new(),
        remote_addr: "192.168.1.50".into(),
    }
}

fn post_req(
    port: u16,
    path: &str,
    key: &str,
    body: &Value,
    content_type: &str,
) -> PhoneHttpRequest {
    PhoneHttpRequest {
        method: "POST".into(),
        path: path.into(),
        headers: vec![
            ("host".into(), format!("127.0.0.1:{port}").into_bytes()),
            ("authorization".into(), format!("Bearer {key}").into_bytes()),
            ("content-type".into(), content_type.as_bytes().to_vec()),
        ],
        body: body.to_string().into_bytes(),
        remote_addr: "192.168.1.50".into(),
    }
}

fn header<'a>(resp: &'a PhoneHttpResponse, name: &str) -> Option<&'a str> {
    resp.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn body_json(resp: &PhoneHttpResponse) -> Value {
    serde_json::from_slice(&resp.body).unwrap()
}

// ---- a-phone-is-paired-by-its-code.test.ts (9 tests) ---------------------------------------------------------------

#[test]
fn is_off_until_turned_on_and_off_means_no_address_at_all() {
    let (_dir, host, _, _, _, _) = harness();
    let settings = host.load();
    assert!(!settings.enabled);
    assert_eq!(settings.url, None);
}

#[test]
fn serves_the_page_to_anyone_and_nothing_else_without_the_key_from_the_code() {
    let (_dir, host, _, _, listener, _) = harness();
    let on = host.set_enabled(true);
    let url = on.url.as_deref().unwrap();
    let key = key_of(Some(url));
    assert!(url.starts_with("http://127.0.0.1:"));
    assert!(valid_pairing_key(&key), "key was {key}");
    let port = port_of(Some(url));

    let page = listener.request(get_req(port, "/", None));
    assert_eq!(page.status, 200);
    assert!(header(&page, "content-security-policy").unwrap().contains("default-src 'none'"));
    assert!(String::from_utf8_lossy(&page.body).contains("Pair this phone with Locust"));

    assert_eq!(listener.request(get_req(port, "/api/state", None)).status, 401);
    assert_eq!(
        listener
            .request(get_req(port, "/api/state", Some(&"x".repeat(32))))
            .status,
        401
    );
    let state = listener.request(get_req(port, "/api/state", Some(&key)));
    assert_eq!(state.status, 200);
    assert_eq!(body_json(&state)["waiting"].as_array().unwrap().len(), 1);
    host.dispose();
}

#[test]
fn refuses_a_request_that_names_this_computer_by_a_name_as_a_rebinding_page_would() {
    let (_dir, host, _, _, listener, _) = harness();
    let on = host.set_enabled(true);
    let key = key_of(on.url.as_deref());
    let port = port_of(on.url.as_deref());

    let mut evil = get_req(port, "/api/state", Some(&key));
    evil.headers[0] = ("host".into(), format!("evil.example:{port}").into_bytes());
    assert_eq!(listener.request(evil).status, 421);

    let good = get_req(port, "/api/state", Some(&key));
    assert_eq!(listener.request(good).status, 200);
    host.dispose();
}

#[test]
fn answers_a_card_and_sends_a_message_only_as_json_with_the_key() {
    let (_dir, host, calls, _, listener, _) = harness();
    let on = host.set_enabled(true);
    let key = key_of(on.url.as_deref());
    let port = port_of(on.url.as_deref());

    let decided = listener.request(post_req(
        port,
        "/api/decide",
        &key,
        &json!({"approvalId": "a1", "approve": false}),
        "application/json",
    ));
    assert_eq!(decided.status, 200);
    assert_eq!(body_json(&decided), json!({"ok": true}));
    assert_eq!(*calls.decided.lock().unwrap(), vec![("a1".into(), false)]);

    assert_eq!(
        listener
            .request(post_req(
                port,
                "/api/send",
                &key,
                &json!({"missionId": "m1", "text": "and again"}),
                "text/plain"
            ))
            .status,
        415
    );
    assert_eq!(
        listener
            .request(post_req(
                port,
                "/api/send",
                &key,
                &json!({"missionId": "m1", "text": "   "}),
                "application/json"
            ))
            .status,
        400
    );
    let sent = listener.request(post_req(
        port,
        "/api/send",
        &key,
        &json!({"missionId": "m1", "text": " and again "}),
        "application/json",
    ));
    assert_eq!(body_json(&sent), json!({"ok": true}));
    assert_eq!(
        *calls.sent.lock().unwrap(),
        vec![("m1".into(), "and again".into())]
    );

    let turns = listener.request(get_req(port, "/api/turns?id=m1", Some(&key)));
    assert_eq!(body_json(&turns)["turns"].as_array().unwrap().len(), 1);
    assert_eq!(
        listener
            .request(get_req(port, "/api/turns?id=gone", Some(&key)))
            .status,
        404
    );
    host.dispose();
}

#[test]
fn stops_listening_to_an_address_after_too_many_wrong_keys() {
    let (_dir, host, _, _, listener, _) = harness();
    let on = host.set_enabled(true);
    let key = key_of(on.url.as_deref());
    let port = port_of(on.url.as_deref());
    for _ in 0..20 {
        let resp = listener.request(get_req(port, "/api/state", Some(&"y".repeat(32))));
        assert_eq!(resp.status, 401);
    }
    // Even the right key, from there, for a while.
    assert_eq!(
        listener.request(get_req(port, "/api/state", Some(&key))).status,
        429
    );
    host.dispose();
}

#[test]
fn answers_a_key_of_the_right_length_in_characters_but_not_in_bytes_and_counts_it_as_wrong() {
    let (_dir, host, _, _, listener, _) = harness();
    let on = host.set_enabled(true);
    let key = key_of(on.url.as_deref());
    let port = port_of(on.url.as_deref());

    // Latin-1 header bytes (32 chars, 64 bytes in UTF-8); must return 401 without panicking.
    let mut latin1_header = b"Bearer ".to_vec();
    latin1_header.extend(std::iter::repeat_n(0xe9u8, 32));
    let req = PhoneHttpRequest {
        method: "GET".into(),
        path: "/api/state".into(),
        headers: vec![
            ("host".into(), format!("127.0.0.1:{port}").into_bytes()),
            ("authorization".into(), latin1_header),
        ],
        body: Vec::new(),
        remote_addr: "192.168.1.50".into(),
    };
    assert_eq!(listener.request(req).status, 401);

    // Also 32 UTF-8 characters of 'é' (64 bytes).
    assert_eq!(
        listener
            .request(get_req(port, "/api/state", Some(&"é".repeat(32))))
            .status,
        401
    );
    // Control: the right key still works.
    assert_eq!(
        listener.request(get_req(port, "/api/state", Some(&key))).status,
        200
    );
    host.dispose();
}

#[test]
fn keeps_its_key_across_a_restart_so_a_paired_phone_stays_paired() {
    let (dir, host, calls, secrets, _, clock) = harness();
    let on = host.set_enabled(true);
    host.dispose();

    let listener2 = FakeListenerFactory::new();
    let again = PhoneHost::new(PhoneHostOptions {
        directory: dir.0.clone(),
        page: Arc::new(|| phone_page(PhonePageVia::Lan, None)),
        calls,
        secrets,
        listener: listener2,
        refusal: Arc::new(|| None),
        addresses: Arc::new(|| vec!["127.0.0.1".into()]),
        port: 0,
        now_ms: Arc::new(move || clock.load(Ordering::Relaxed)),
    });
    let loaded = again.load();
    assert!(loaded.enabled);
    assert_eq!(key_of(loaded.url.as_deref()), key_of(on.url.as_deref()));
    again.dispose();
}

#[test]
fn off_unpairs_every_phone_and_a_new_code_does_it_while_staying_on() {
    let (dir, host, _, secrets, listener, _) = harness();
    let first = host.set_enabled(true);
    let first_key = key_of(first.url.as_deref());
    let renewed = host.forget();
    let renewed_key = key_of(renewed.url.as_deref());
    let port = port_of(renewed.url.as_deref());
    assert_ne!(renewed_key, first_key);
    assert_eq!(
        listener
            .request(get_req(port, "/api/state", Some(&first_key)))
            .status,
        401
    );
    assert_eq!(
        listener
            .request(get_req(port, "/api/state", Some(&renewed_key)))
            .status,
        200
    );

    host.set_enabled(false);
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.0.join(PHONE_FILE)).unwrap()).unwrap();
    assert_eq!(saved, json!({ "enabled": false }));
    assert!(secrets.sealed.lock().unwrap().is_empty());

    let back = host.set_enabled(true);
    assert_ne!(key_of(back.url.as_deref()), renewed_key);
    host.dispose();
}

#[test]
fn offers_home_network_addresses_never_a_virtual_switch_a_phone_cannot_reach() {
    let entry = |name: &str, address: &str, is_ipv4: bool, internal: bool| NetworkInterface {
        name: name.into(),
        address: address.into(),
        is_ipv4,
        internal,
    };
    let found = phone_addresses(&[
        entry("vEthernet (WSL)", "172.20.0.1", true, false),
        entry("Loopback", "127.0.0.1", true, true),
        entry("Tailscale", "100.101.1.2", true, false),
        entry("Wi-Fi", "192.168.1.23", true, false),
        entry("Wi-Fi", "fe80::1", false, false),
        entry("Ethernet", "8.8.8.8", true, false),
    ]);
    assert_eq!(found, vec!["192.168.1.23", "100.101.1.2"]);
}

// ---- Policy refusal + SecretBox + scrubbing ------------------------------------------------------------------------

#[test]
fn ignores_a_saved_phone_opt_in_and_refuses_forged_pairing_without_a_listener_or_key() {
    let dir = TempDir::new("policy");
    std::fs::write(
        dir.0.join(PHONE_FILE),
        json!({ "enabled": true, "key": "a".repeat(32) }).to_string(),
    )
    .unwrap();
    let secrets = FakeSecretBox::new();
    let listener = FakeListenerFactory::new();
    let host = PhoneHost::new(PhoneHostOptions {
        directory: dir.0.clone(),
        page: Arc::new(String::new),
        calls: FakeCalls::new(),
        secrets: secrets.clone(),
        listener: listener.clone(),
        refusal: Arc::new(|| Some("Phone pairing is off. Set by your organization (phone in /admin/policy.json). Nothing was started.".into())),
        addresses: Arc::new(Vec::new),
        port: 0,
        now_ms: Arc::new(|| 0),
    });
    let loaded = host.load();
    assert!(!loaded.enabled);
    assert!(loaded.addresses.is_empty());
    assert!(loaded.message.as_deref().unwrap().contains("phone"));
    assert!(!host.set_enabled(true).enabled);
    assert_eq!(host.settings().url, None);
    assert_eq!(listener.bind_count.load(Ordering::Relaxed), 0);
    assert!(secrets.sealed.lock().unwrap().is_empty());
    host.dispose();
}

#[test]
fn pairing_key_is_sealed_in_secret_box_never_plaintext_and_scrubbed_from_errors() {
    let (dir, host, _, secrets, _, _) = harness();
    let on = host.set_enabled(true);
    let key = key_of(on.url.as_deref());
    let disk = std::fs::read_to_string(dir.0.join(PHONE_FILE)).unwrap();
    assert!(!disk.contains(&key), "phone.json must never hold plaintext key");
    let saved: Value = serde_json::from_str(&disk).unwrap();
    assert!(saved["key"].as_str().unwrap().starts_with("fake-box:"));
    assert_eq!(secrets.sealed.lock().unwrap().len(), 1);

    let pk = PairingKey(key.clone());
    assert!(!format!("{pk:?}").contains(&key));
    let scrubbed = scrub_phone_error(&format!("failed with key {key} and sk-ant-{}", "a".repeat(25)), Some(&key));
    assert!(!scrubbed.contains(&key));
    assert!(!scrubbed.contains("sk-ant-"));
    host.dispose();

    // Unavailable SecretBox refuses to turn on or write a plaintext key.
    let dir2 = TempDir::new("nobox");
    let listener2 = FakeListenerFactory::new();
    let host2 = PhoneHost::new(PhoneHostOptions {
        directory: dir2.0.clone(),
        page: Arc::new(String::new),
        calls: FakeCalls::new(),
        secrets: FakeSecretBox::unavailable(),
        listener: listener2.clone(),
        refusal: Arc::new(|| None),
        addresses: Arc::new(|| vec!["127.0.0.1".into()]),
        port: 0,
        now_ms: Arc::new(|| 0),
    });
    let off = host2.set_enabled(true);
    assert!(!off.enabled);
    assert_eq!(off.url, None);
    assert_eq!(listener2.bind_count.load(Ordering::Relaxed), 0);
    assert!(!dir2.0.join(PHONE_FILE).exists());
}

// ---- scouts-cards-are-on-your-phone.test.ts (tests 1 & 2) ----------------------------------------------------------

fn make_scout_card(n: u32, extra: impl FnOnce(&mut ScoutCard)) -> ScoutCard {
    let fact = ScoutFact::simple(format!("issue:{n}"), ScoutUrgency::Soon, format!("Card {n}."));
    let mut card = ScoutCard {
        card_id: format!("card_{n:012}"),
        at: format!("2026-10-23T{n:02}:00:00.000Z"),
        reading: false,
        take: None,
        fix: None,
        fact,
    };
    extra(&mut card);
    card
}

#[test]
fn scout_cards_in_locusts_words_newest_first_with_github_link_only_and_nothing_the_window_keeps() {
    let cards = vec![
        make_scout_card(1, |c| {
            c.fact.urgency = ScoutUrgency::Now;
            c.fact.quote = Some("rm -rf build".into());
            c.take = Some("The model thinks…".into());
            c.fact.draft = Some(ScoutDraft {
                teammate_id: "tm_ada".into(),
                name: "Ada".into(),
                message: "Fix it".into(),
            });
            c.fact.open = Some(ScoutOpen::Url {
                url: "https://github.com/acme/app/pull/7".into(),
                label: "Open".into(),
            });
        }),
        make_scout_card(3, |c| {
            c.fact.open = Some(ScoutOpen::Url {
                url: "https://evil.example/x".into(),
                label: "Open".into(),
            });
            c.fact.lines = Some(vec!["One line.".into()]);
        }),
        make_scout_card(2, |c| {
            c.fact.open = Some(ScoutOpen::Conversation {
                mission_id: "m1".into(),
                label: "Open it".into(),
            });
        }),
    ];
    let shown = phone_scout_cards(&cards);
    assert_eq!(
        shown,
        vec![
            PhoneScoutCard {
                card_id: "card_000000000003".into(),
                said: "Card 3.".into(),
                lines: Some(vec!["One line.".into()]),
                now: false,
                at: "2026-10-23T03:00:00.000Z".into(),
                url: None,
            },
            PhoneScoutCard {
                card_id: "card_000000000002".into(),
                said: "Card 2.".into(),
                lines: None,
                now: false,
                at: "2026-10-23T02:00:00.000Z".into(),
                url: None,
            },
            PhoneScoutCard {
                card_id: "card_000000000001".into(),
                said: "Card 1.".into(),
                lines: None,
                now: true,
                at: "2026-10-23T01:00:00.000Z".into(),
                url: Some("https://github.com/acme/app/pull/7".into()),
            },
        ]
    );
    let dumped = Value::Array(shown.iter().map(PhoneScoutCard::to_json).collect()).to_string();
    assert!(!dumped.contains("rm -rf") && !dumped.contains("model thinks") && !dumped.contains("Fix it"));

    let many: Vec<ScoutCard> = (0..15).map(|n| make_scout_card(n, |_| {})).collect();
    assert_eq!(phone_scout_cards(&many).len(), PHONE_MAX_SCOUT_CARDS);
}

#[test]
fn dismisses_a_scout_card_the_phone_names_and_nothing_that_is_not_a_cards_id() {
    let calls = FakeCalls::new();
    let good = "card_000000000004";
    assert_eq!(
        answer_phone(
            calls.as_ref(),
            "POST",
            "/api/scout/dismiss",
            Some(&json!({ "cardId": good }))
        ),
        Some(PhoneAnswer {
            status: 200,
            data: json!({ "ok": true }),
        })
    );
    assert_eq!(*calls.dismissed.lock().unwrap(), vec![good.to_owned()]);
    assert_eq!(
        answer_phone(
            calls.as_ref(),
            "POST",
            "/api/scout/dismiss",
            Some(&json!({ "cardId": "../scout.json" }))
        ),
        None
    );
    assert_eq!(calls.dismissed.lock().unwrap().len(), 1);
}

// ---- a-phone-lists-conversations.test.ts (all 4 tests) -------------------------------------------------------------

fn said_event(mission_id: &str, seq: u64, text: &str, fin: bool) -> Value {
    json!({
        "id": format!("{mission_id}:{seq}"),
        "runId": format!("r-{mission_id}"),
        "missionId": mission_id,
        "sequence": seq,
        "type": "message.delta",
        "occurredAt": "2026-10-15T12:00:00.000Z",
        "sourceAdapter": "codex",
        "payload": { "itemId": "i1", "operation": "append", "text": text, "final": fin }
    })
}

fn mission(id: &str, at: &str, prompt: &str) -> PhoneMission {
    PhoneMission {
        mission_id: id.into(),
        prompt: prompt.into(),
        runtime: "codex".into(),
        created_at: at.into(),
        last_updated_at: at.into(),
        phase: "completed".into(),
        continues_from_mission_id: None,
        continues_from_reason: None,
        started_by_kind: None,
        events: vec![said_event(id, 1, &format!("answer of {id}"), true)],
    }
}

#[test]
// case: is one row per conversation, newest first, named by the first thing asked
fn conversations_are_one_row_per_conversation_newest_first_named_by_the_first_thing_asked() {
    let first = mission("a1", "2026-10-15T10:00:00.000Z", "Fix the login bug");
    let mut reply = mission("a2", "2026-10-15T10:05:00.000Z", "and add a test");
    reply.continues_from_mission_id = Some("a1".into());
    reply.continues_from_reason = Some("follow-up".into());
    let mut carried = mission("a3", "2026-10-15T10:09:00.000Z", "Codex CLI ran out of its plan…");
    carried.runtime = "opencode".into();
    carried.continues_from_mission_id = Some("a2".into());
    carried.continues_from_reason = Some("route-switch".into());
    carried.started_by_kind = Some("carry-on".into());
    let other = mission("b1", "2026-10-15T09:00:00.000Z", "Explain   this\nproject");
    let mut side = mission("s1", "2026-10-15T11:00:00.000Z", "side");
    side.started_by_kind = Some("side".into());

    let (list, _) = phone_conversations(&[other, first, reply, carried, side], &["a3".into()], None);
    let summary: Vec<(&str, &str, &str, &str)> = list
        .iter()
        .map(|r| (r.mission_id.as_str(), r.title.as_str(), r.agent.as_str(), r.phase.as_str()))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("a3", "Fix the login bug", "OpenCode", "running"),
            ("b1", "Explain this project", "Codex CLI", "completed"),
        ]
    );
    assert_eq!(list[0].snippet.as_deref(), Some("answer of a3"));
}

#[test]
// case: names the teammate who answered, as the window does
fn conversations_name_the_teammate_who_answered_as_the_window_does() {
    let first = mission("a1", "2026-10-15T10:00:00.000Z", "Fix the login bug");
    let mut reply = mission("a2", "2026-10-15T10:05:00.000Z", "and add a test");
    reply.continues_from_mission_id = Some("a1".into());
    reply.continues_from_reason = Some("follow-up".into());
    let names = |id: &str| (id == "a1" || id == "a2").then(|| "Mira".to_owned());
    let (list, chains) = phone_conversations(&[first, reply], &[], Some(&names));
    assert_eq!(list[0].agent, "Mira");
    let agents: Vec<String> = phone_turns(&chains, "a2", &[], Some(&|_| Some("Mira".into())))
        .unwrap()
        .into_iter()
        .map(|t| t.agent)
        .collect();
    assert_eq!(agents, vec!["Mira", "Mira"]);
}

#[test]
// case: shows each turn’s answer, and the person’s words only where the person typed them
fn turns_show_each_turns_answer_and_the_persons_words_only_where_the_person_typed_them() {
    let first = mission("a1", "2026-10-15T10:00:00.000Z", "Fix the login bug");
    let mut reply = mission("a2", "2026-10-15T10:05:00.000Z", "and add a test");
    reply.continues_from_mission_id = Some("a1".into());
    reply.continues_from_reason = Some("follow-up".into());
    let mut carried = mission("a3", "2026-10-15T10:09:00.000Z", "Codex CLI ran out of its plan…");
    carried.runtime = "opencode".into();
    carried.continues_from_mission_id = Some("a2".into());
    carried.continues_from_reason = Some("route-switch".into());
    carried.started_by_kind = Some("carry-on".into());

    let (_, chains) = phone_conversations(&[first, reply, carried], &[], None);
    let turns = phone_turns(&chains, "a1", &[], None).unwrap();
    let pairs: Vec<(Option<&str>, Option<&str>)> = turns
        .iter()
        .map(|t| (t.asked.as_deref(), t.answer.as_deref()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            (Some("Fix the login bug"), Some("answer of a1")),
            (Some("and add a test"), Some("answer of a2")),
            (None, Some("answer of a3")),
        ]
    );
    assert!(phone_turns(&chains, "nobody", &[], None).is_none());
}

#[test]
// case: reads a streamed answer whole, and one still arriving
fn reads_a_streamed_answer_whole_and_one_still_arriving() {
    assert_eq!(
        answer_of(&[said_event("m", 1, "Hel", false), said_event("m", 2, "lo.", true)]).as_deref(),
        Some("Hello.")
    );
    assert_eq!(
        answer_of(&[said_event("m", 1, "Still writ", false)]).as_deref(),
        Some("Still writ")
    );
    assert_eq!(answer_of(&[]), None);
}

// ---- the-phone-reads-history-once-per-change.test.ts (both tests) --------------------------------------------------

#[test]
fn ledger_write_counter_counts_writes_before_and_after_and_never_reads() {
    let counter = LedgerWriteCounter::new();
    assert_eq!(counter.writes(), 0);
    counter.begin_write();
    assert_eq!(counter.writes(), 1);
    counter.end_write();
    assert_eq!(counter.writes(), 2);
    counter.record_write(|| {});
    assert_eq!(counter.writes(), 4);
}

#[test]
fn keyed_once_builds_once_per_key_shares_a_build_in_flight_and_retries_after_a_failure() {
    let builds = AtomicUsize::new(0);
    let once: KeyedOnce<usize> = KeyedOnce::new();
    let build = |fail: bool| -> Result<usize, &'static str> {
        let n = builds.fetch_add(1, Ordering::Relaxed) + 1;
        if fail { Err("no") } else { Ok(n) }
    };
    // Two asks for "a" at once: the second arrives while the first is still building, and shares it.
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let both = std::thread::scope(|s| {
        let first = s.spawn(|| {
            once.get_or_build("a", || {
                started_tx.send(()).unwrap();
                go_rx.lock().unwrap().recv().unwrap();
                build(false)
            })
        });
        started_rx.recv().unwrap();
        let second = s.spawn(|| once.get_or_build("a", || build(false)));
        std::thread::sleep(std::time::Duration::from_millis(50));
        go_tx.send(()).unwrap();
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(both, [Ok(1), Ok(1)]);
    assert_eq!(once.get_or_build("a", || build(false)), Ok(1));
    assert_eq!(once.get_or_build("b", || build(false)), Ok(2));
    assert_eq!(once.get_or_build("c", || build(true)), Err("no"));
    assert_eq!(once.get_or_build("c", || build(false)), Ok(4));
    assert_eq!(builds.load(Ordering::Relaxed), 4);
}

/// phone-host.ts checks the key again once the body has arrived (`key() !== expected` -> "This phone was unpaired."):
/// a phone unpaired while its message was still arriving is refused, and nothing it asked for is done.
#[test]
fn a_phone_unpaired_while_its_message_was_arriving_is_refused_and_nothing_is_done() {
    let calls = FakeCalls::new();
    let key = "k".repeat(32);
    let asked = Arc::new(AtomicUsize::new(0));
    let (key_for, asked_for) = (key.clone(), asked.clone());
    // The first look (before the body) finds the key; every look after it finds the phone unpaired.
    let handler = PhoneRequestHandler::new(
        Arc::new(move || (asked_for.fetch_add(1, Ordering::SeqCst) == 0).then(|| key_for.clone())),
        Arc::new(String::new),
        calls.clone(),
        Arc::new(|| 1_700_000_000_000),
    );
    let answer = handler.handle(&post_req(8080, "/api/decide", &key, &json!({"approvalId": "a1", "approve": true}), "application/json"));
    assert_eq!(answer.status, 401);
    assert_eq!(body_json(&answer), json!({"message": "This phone was unpaired."}));
    assert!(calls.decided.lock().unwrap().is_empty());
}
