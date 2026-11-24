//! Keychain unlock note and launch gating — port of `main/keychain.ts` + `KeychainNote.tsx` (0.854).
//!
//! Nothing asks the keychain without a key to open. On a Mac's first launch of a build, when a key
//! is kept, the window says it first, waits briefly so the note is drawn, opens the keychain once,
//! takes the note down, and remembers this build. Never reads or copies real keychain items in tests.

use locust_core::own_models::SecretBox;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Exact Electron `KEYCHAIN_NOTE` (KeychainNote.tsx).
pub const KEYCHAIN_NOTE: &str = "macOS may ask for your Mac’s password so Locust can open the keys it keeps (“@teammate/desktop Safe Storage”). Type it and choose Always Allow. If you don’t see the box, it may be behind this window.";

/// Default time to wait for the note to be drawn before touching the keychain.
pub const DEFAULT_DRAW_MS: u64 = 400;

/// Default time to wait for the window to start listening before opening the keychain anyway.
pub const DEFAULT_LISTEN_MS: u64 = 20_000;

/// Whether the amber note should be shown. Never touches the OS keychain.
///
/// Quiet when: not Mac, no key is kept, or this build already opened the
/// keychain. An unknown `keeps_key` (caller passes true) shows the note rather
/// than freezing silently — matching Electron's unreadable-kept treatment.
pub fn should_show_note(may_ask: bool, keeps_key: bool, opened_by_matches_version: bool) -> bool {
    may_ask && keeps_key && !opened_by_matches_version
}

/// macOS may ask; elsewhere never.
pub fn may_ask_on_this_os() -> bool {
    cfg!(target_os = "macos")
}

/// Low-level keychain backend (port of Electron's `KeychainBackend` / `safeStorage`).
pub trait KeychainBackend: Send + Sync {
    fn is_encryption_available(&self) -> Result<bool, String>;
    fn seal(&self, text: &str) -> Result<String, String>;
    fn open(&self, sealed: &str) -> Result<String, String>;
}

/// One-shot signal resolved once the window is showing and can draw the note.
#[derive(Clone)]
pub struct WindowSignal {
    inner: Arc<(Mutex<bool>, Condvar)>,
}

impl WindowSignal {
    pub fn new() -> Self {
        Self { inner: Arc::new((Mutex::new(false), Condvar::new())) }
    }

    pub fn notify(&self) {
        let (lock, cvar) = &*self.inner;
        *lock.lock().unwrap_or_else(|e| e.into_inner()) = true;
        cvar.notify_all();
    }

    pub fn wait_timeout(&self, ms: u64) -> bool {
        let (lock, cvar) = &*self.inner;
        let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        if *guard {
            return true;
        }
        let (guard, _) = cvar
            .wait_timeout_while(guard, Duration::from_millis(ms), |ready| !*ready)
            .unwrap_or_else(|e| e.into_inner());
        *guard
    }
}

impl Default for WindowSignal {
    fn default() -> Self {
        Self::new()
    }
}

/// Options for [`Keychain::new`] (port of Electron `createKeychain`).
pub struct KeychainOptions {
    pub backend: Arc<dyn KeychainBackend>,
    /// A Mac reading its own keychain. Elsewhere, or with a mock keychain, nothing asks or is said.
    pub may_ask: bool,
    /// This launch's build version, and the one stored when the keychain was last opened here.
    pub version: String,
    pub opened_by: Arc<dyn Fn() -> Result<Option<String>, String> + Send + Sync>,
    pub remember_opened: Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>,
    /// Whether any store keeps a key. Errors count as true (unknown counts as kept).
    pub keeps_key: Arc<dyn Fn() -> Result<bool, String> + Send + Sync>,
    /// Returns a signal resolved once the window is showing and listening for the note.
    pub window_listening: Arc<dyn Fn() -> WindowSignal + Send + Sync>,
    /// Tells the window whether macOS may be asking now.
    pub say: Arc<dyn Fn(bool) + Send + Sync>,
    pub draw_ms: Option<u64>,
    pub listen_ms: Option<u64>,
    /// Custom wait hook for tests; `None` sleeps on the real clock.
    pub wait: Option<Arc<dyn Fn(u64) + Send + Sync>>,
}

struct StepState {
    started: bool,
    settled: bool,
}

/// Launch-time keychain coordinator: opens the keychain at most once, with the window note up first.
pub struct Keychain {
    opts: KeychainOptions,
    opened: Arc<AtomicBool>,
    asking: Arc<AtomicBool>,
    step: Arc<(Mutex<StepState>, Condvar)>,
}

struct TrackedSecrets {
    backend: Arc<dyn KeychainBackend>,
    opened: Arc<AtomicBool>,
}

impl SecretBox for TrackedSecrets {
    fn available(&self) -> bool {
        self.opened.store(true, Ordering::SeqCst);
        self.backend.is_encryption_available().unwrap_or(false)
    }

    fn seal(&self, text: &str) -> Result<String, String> {
        self.opened.store(true, Ordering::SeqCst);
        self.backend.seal(text)
    }

    fn open(&self, sealed: &str) -> Result<String, String> {
        self.opened.store(true, Ordering::SeqCst);
        self.backend.open(sealed)
    }
}

impl Keychain {
    pub fn new(opts: KeychainOptions) -> Arc<Self> {
        Arc::new(Self {
            opts,
            opened: Arc::new(AtomicBool::new(false)),
            asking: Arc::new(AtomicBool::new(false)),
            step: Arc::new((Mutex::new(StepState { started: false, settled: false }), Condvar::new())),
        })
    }

    /// What every store is given: straight through to the backend while marking the keychain opened.
    pub fn secrets(&self) -> Arc<dyn SecretBox> {
        Arc::new(TrackedSecrets {
            backend: self.opts.backend.clone(),
            opened: self.opened.clone(),
        })
    }

    /// Whether the window should be saying, now, that macOS may ask.
    pub fn asking(&self) -> bool {
        self.asking.load(Ordering::SeqCst)
    }

    /// Start the launch's step once; repeated or concurrent calls wait on the same step.
    pub fn begin(&self) {
        let should_run = {
            let (lock, _) = &*self.step;
            let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
            if st.started {
                false
            } else {
                st.started = true;
                true
            }
        };
        if !should_run {
            self.settled();
            return;
        }
        self.run();
        let (lock, cvar) = &*self.step;
        lock.lock().unwrap_or_else(|e| e.into_inner()).settled = true;
        cvar.notify_all();
    }

    /// Blocks until the launch's step is over, whether or not anything was asked.
    pub fn settled(&self) {
        let (lock, cvar) = &*self.step;
        let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
        while !st.settled {
            st = cvar.wait(st).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn wait_ms(&self, ms: u64) {
        if let Some(ref wait) = self.opts.wait {
            wait(ms);
        } else {
            std::thread::sleep(Duration::from_millis(ms));
        }
    }

    fn run(&self) {
        if !self.opts.may_ask || self.opened.load(Ordering::SeqCst) {
            return;
        }
        let opened_by_matches = (self.opts.opened_by)()
            .ok()
            .flatten()
            .as_deref()
            == Some(self.opts.version.as_str());
        // Unknown counts as kept: a note said for nothing is better than a window frozen without one.
        let keeps = (self.opts.keeps_key)().unwrap_or(true);
        if !should_show_note(self.opts.may_ask, keeps, opened_by_matches) {
            return;
        }
        let listen_ms = self.opts.listen_ms.unwrap_or(DEFAULT_LISTEN_MS);
        let signal = (self.opts.window_listening)();
        if let Some(ref wait) = self.opts.wait {
            wait(listen_ms);
        } else {
            let _ = signal.wait_timeout(listen_ms);
        }
        if self.opened.load(Ordering::SeqCst) {
            return;
        }
        self.asking.store(true, Ordering::SeqCst);
        (self.opts.say)(true);
        self.wait_ms(self.opts.draw_ms.unwrap_or(DEFAULT_DRAW_MS));
        self.opened.store(true, Ordering::SeqCst);
        let available = self.opts.backend.is_encryption_available().unwrap_or(false);
        self.asking.store(false, Ordering::SeqCst);
        (self.opts.say)(false);
        // Refused, or not there: not remembered, so the next launch says it again.
        if available {
            let _ = (self.opts.remember_opened)(&self.opts.version);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::chat_http::{HttpClient, HttpRequest, HttpResponse};
    use locust_core::chat_link::{ChatLinkOptions, ChatStart};
    use locust_core::chat_telegram::{TELEGRAM_FILE, TelegramOptions, create_telegram_link};
    use locust_core::teammate_keys::{RunKey, TeammateKeys};
    use locust_core::your_account::{ACCOUNT_SYNC_FILE, YourAccount, YourAccountOptions};
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

    struct Scratch(PathBuf);
    impl Scratch {
        fn new(tag: &str) -> Self {
            let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("locust-keychain-note-{tag}-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct CountedBox {
        asked: Mutex<Vec<String>>,
    }
    impl CountedBox {
        fn new() -> Arc<Self> {
            Arc::new(Self { asked: Mutex::new(Vec::new()) })
        }
        fn asked(&self) -> Vec<String> {
            self.asked.lock().unwrap().clone()
        }
        fn clear(&self) {
            self.asked.lock().unwrap().clear();
        }
    }
    impl SecretBox for CountedBox {
        fn available(&self) -> bool {
            self.asked.lock().unwrap().push("available".into());
            true
        }
        fn seal(&self, text: &str) -> Result<String, String> {
            self.asked.lock().unwrap().push("encrypt".into());
            let hex: String = text.bytes().map(|b| format!("{:02x}", b ^ 0x5a)).collect();
            Ok(format!("sealed:{hex}"))
        }
        fn open(&self, sealed: &str) -> Result<String, String> {
            self.asked.lock().unwrap().push("decrypt".into());
            let hex = sealed.strip_prefix("sealed:").ok_or("bad seal")?;
            let mut out = Vec::with_capacity(hex.len() / 2);
            for i in (0..hex.len()).step_by(2) {
                let b = u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| "bad hex")?;
                out.push(b ^ 0x5a);
            }
            String::from_utf8(out).map_err(|_| "bad utf8".into())
        }
    }

    struct UnusedHttp;
    impl HttpClient for UnusedHttp {
        fn send(&self, _req: HttpRequest) -> Result<HttpResponse, String> {
            Err("unused".into())
        }
    }

    enum Availability {
        Bool(bool),
        Throws,
    }

    struct StepBackend {
        steps: Arc<Mutex<Vec<String>>>,
        availability: Availability,
    }

    impl KeychainBackend for StepBackend {
        fn is_encryption_available(&self) -> Result<bool, String> {
            self.steps.lock().unwrap().push("keychain".into());
            match self.availability {
                Availability::Bool(v) => Ok(v),
                Availability::Throws => Err("User interaction is not allowed.".into()),
            }
        }
        fn seal(&self, text: &str) -> Result<String, String> {
            self.steps.lock().unwrap().push("keychain".into());
            Ok(text.to_owned())
        }
        fn open(&self, sealed: &str) -> Result<String, String> {
            self.steps.lock().unwrap().push("keychain".into());
            Ok(sealed.to_owned())
        }
    }

    struct MacFixture {
        opened_by: Option<String>,
        keeps_key: bool,
        /// The store could not say whether a key is kept (Electron: unknown counts as kept).
        keeps_key_unknown: bool,
        may_ask: bool,
        availability: Availability,
        listens: bool,
    }

    impl Default for MacFixture {
        fn default() -> Self {
            Self {
                opened_by: None,
                keeps_key: true,
                keeps_key_unknown: false,
                may_ask: true,
                availability: Availability::Bool(true),
                listens: true,
            }
        }
    }

    fn mac(input: MacFixture) -> (Arc<Keychain>, Arc<Mutex<Vec<String>>>) {
        let steps = Arc::new(Mutex::new(Vec::new()));
        let signal = WindowSignal::new();
        if input.listens {
            signal.notify();
        }
        let backend = Arc::new(StepBackend {
            steps: steps.clone(),
            availability: input.availability,
        });
        let opened_by = input.opened_by;
        let keeps_key = input.keeps_key;
        let keeps_key_unknown = input.keeps_key_unknown;
        let steps_remember = steps.clone();
        let steps_listen = steps.clone();
        let steps_say = steps.clone();
        let steps_wait = steps.clone();
        let keychain = Keychain::new(KeychainOptions {
            backend,
            may_ask: input.may_ask,
            version: "0.854.0".into(),
            opened_by: Arc::new(move || Ok(opened_by.clone())),
            remember_opened: Arc::new(move |version| {
                steps_remember.lock().unwrap().push(format!("remember {version}"));
                Ok(())
            }),
            keeps_key: Arc::new(move || if keeps_key_unknown { Err("unreadable".into()) } else { Ok(keeps_key) }),
            window_listening: Arc::new(move || {
                steps_listen.lock().unwrap().push("waits for the window".into());
                signal.clone()
            }),
            say: Arc::new(move |asking| {
                steps_say.lock().unwrap().push(if asking { "note up" } else { "note down" }.into());
            }),
            draw_ms: None,
            listen_ms: None,
            wait: Some(Arc::new(move |ms| {
                steps_wait.lock().unwrap().push(format!("wait {ms}"));
            })),
        });
        (keychain, steps)
    }

    // Same case as saying it first, when the store cannot say whether a key is kept: unknown counts as kept, so the
    // note still goes up before the keychain is opened (a note said for nothing beats a window frozen without one).
    #[test]
    fn an_unknown_key_still_says_it_in_the_window_first() {
        let (keychain, steps) = mac(MacFixture { keeps_key_unknown: true, ..MacFixture::default() });
        keychain.begin();
        let steps = steps.lock().unwrap().clone();
        let up = steps.iter().position(|s| s == "note up").expect("the note goes up");
        let opened = steps.iter().position(|s| s == "keychain").expect("the keychain is opened");
        assert!(up < opened, "{steps:?}");
    }

    #[test]
    fn quiet_without_key_or_when_already_opened() {
        assert!(!should_show_note(true, false, false));
        assert!(!should_show_note(true, true, true));
        assert!(!should_show_note(false, true, false));
        assert!(should_show_note(true, true, false));
    }

    #[test]
    fn note_words_exact() {
        assert!(KEYCHAIN_NOTE.contains("@teammate/desktop Safe Storage"));
        assert!(KEYCHAIN_NOTE.contains("Always Allow"));
        assert!(KEYCHAIN_NOTE.contains("behind this window"));
        let _ = may_ask_on_this_os();
    }

    // case: a run for a teammate with no keys asks nothing; one with a key opens it
    #[test]
    fn a_run_for_a_teammate_with_no_keys_asks_nothing_one_with_a_key_opens_it() {
        let dir = Scratch::new("teammate-keys");
        let lock = CountedBox::new();
        let keys = TeammateKeys::new(&dir.0, lock.clone());
        assert!(keys.run_keys("tm_wren").is_empty());
        assert!(!keys.keeps_key());
        assert!(lock.asked().is_empty());
        assert!(keys.set("tm_wren", "STRIPE_KEY", "kept-for-wren-alone").is_ok());
        assert!(keys.keeps_key());
        lock.clear();
        assert!(keys.run_keys("tm_booty").is_empty());
        assert!(lock.asked().is_empty());
        assert_eq!(
            keys.run_keys("tm_wren"),
            vec![RunKey { name: "STRIPE_KEY".into(), value: "kept-for-wren-alone".into() }]
        );
        assert!(lock.asked().iter().any(|s| s == "decrypt"));
    }

    // case: a chat app and your account say whether they keep a key without opening it
    #[test]
    fn a_chat_app_and_your_account_say_whether_they_keep_a_key_without_opening_it() {
        let dir = Scratch::new("chat-and-account");
        let lock = CountedBox::new();
        let telegram = create_telegram_link(
            TelegramOptions { http: Arc::new(UnusedHttp), api: Some(Arc::new(|| "http://127.0.0.1:1".into())), poll_seconds: None },
            ChatLinkOptions {
                directory: dir.0.clone(),
                secrets: lock.clone(),
                refusal: Arc::new(|| None),
                computer: Arc::new(|| "DESK-PC".into()),
                team: Arc::new(Vec::new),
                start: Arc::new(|_, _, _| ChatStart::Ok { mission_id: "mission_1".into() }),
                read: Arc::new(|_| None),
                live: Arc::new(|_| false),
                carried: Arc::new(|_| false),
                waiting: Arc::new(Vec::new),
                decide: Arc::new(|_| false),
                changed: None,
                note: None,
                now: None,
                typing_ms: None,
            },
        );
        assert!(!telegram.keeps_key());
        std::fs::write(
            dir.0.join(TELEGRAM_FILE),
            json!({ "schemaVersion": 1, "token": "c2VhbGVk", "bot": { "username": "locust_bot" } }).to_string(),
        )
        .unwrap();
        assert!(telegram.keeps_key());

        let account = YourAccount::new(YourAccountOptions {
            profile: dir.0.clone(),
            link: Arc::new(|| None),
            secrets: lock.clone(),
            computer: Arc::new(|| "Mac".into()),
            app_version: "0.854.0".into(),
            busy: Arc::new(|| false),
            remote: None,
            now: None,
            quiet_ms: None,
            copies_fresh_ms: None,
            ask_for_ms: None,
            changed: None,
            restore: None,
        });
        assert!(!account.keeps_key());
        std::fs::write(
            dir.0.join(ACCOUNT_SYNC_FILE),
            json!({ "v": 1, "on": false, "account": 7, "key": "c2VhbGVk" }).to_string(),
        )
        .unwrap();
        assert!(account.keeps_key());
        assert!(lock.asked().is_empty());
    }

    // case: says it in the window first, then opens the keychain once, then takes the note down and remembers the build
    #[test]
    fn says_it_in_the_window_first_then_opens_the_keychain_once_then_takes_the_note_down_and_remembers_the_build() {
        let (keychain, steps) = mac(MacFixture::default());
        keychain.begin();
        assert_eq!(
            *steps.lock().unwrap(),
            vec![
                "waits for the window",
                "wait 20000",
                "note up",
                "wait 400",
                "keychain",
                "note down",
                "remember 0.854.0",
            ]
        );
        assert!(!keychain.asking());
    }

    // case: a build that opened it before, nothing kept, or not a Mac: nothing said and nothing read
    #[test]
    fn a_build_that_opened_it_before_nothing_kept_or_not_a_mac_nothing_said_and_nothing_read() {
        let cases = [
            MacFixture { opened_by: Some("0.854.0".into()), ..Default::default() },
            MacFixture { keeps_key: false, ..Default::default() },
            MacFixture { may_ask: false, ..Default::default() },
        ];
        for cfg in cases {
            let (keychain, steps) = mac(cfg);
            keychain.begin();
            assert!(steps.lock().unwrap().is_empty());
        }
    }

    // case: refused, or failing: the note still comes down, and the build is not remembered, so the next launch says it again
    #[test]
    fn refused_or_failing_the_note_still_comes_down_and_the_build_is_not_remembered_so_the_next_launch_says_it_again() {
        let cases = [
            MacFixture { availability: Availability::Bool(false), ..Default::default() },
            MacFixture { availability: Availability::Throws, ..Default::default() },
        ];
        for cfg in cases {
            let (keychain, steps) = mac(cfg);
            keychain.begin();
            assert_eq!(
                *steps.lock().unwrap(),
                vec!["waits for the window", "wait 20000", "note up", "wait 400", "keychain", "note down"]
            );
            assert!(!keychain.asking());
        }
    }

    // case: a window that never listens is not waited on forever: the keys are opened anyway, for the readers that need them
    #[test]
    fn a_window_that_never_listens_is_not_waited_on_forever_the_keys_are_opened_anyway_for_the_readers_that_need_them() {
        let (keychain, steps) = mac(MacFixture { listens: false, ..Default::default() });
        keychain.begin();
        assert_eq!(
            *steps.lock().unwrap(),
            vec![
                "waits for the window",
                "wait 20000",
                "note up",
                "wait 400",
                "keychain",
                "note down",
                "remember 0.854.0",
            ]
        );
    }

    // case: a read before the step leaves nothing to say
    #[test]
    fn a_read_before_the_step_leaves_nothing_to_say() {
        let (keychain, steps) = mac(MacFixture::default());
        assert_eq!(keychain.secrets().open("sealed").unwrap(), "sealed");
        keychain.begin();
        assert_eq!(*steps.lock().unwrap(), vec!["keychain"]);
    }

    // case: is one step however often it is begun, and settled once it is over
    #[test]
    fn is_one_step_however_often_it_is_begun_and_settled_once_it_is_over() {
        let (keychain, steps) = mac(MacFixture::default());
        let settled = Arc::new(AtomicBool::new(false));
        let k_wait = keychain.clone();
        let s_wait = settled.clone();
        let waiter = std::thread::spawn(move || {
            k_wait.settled();
            s_wait.store(true, Ordering::SeqCst);
        });
        let k2 = keychain.clone();
        let t2 = std::thread::spawn(move || k2.begin());
        keychain.begin();
        t2.join().unwrap();
        waiter.join().unwrap();
        assert!(settled.load(Ordering::SeqCst));
        let keychain_calls = steps.lock().unwrap().iter().filter(|s| s.as_str() == "keychain").count();
        assert_eq!(keychain_calls, 1);
        // Repeated begin() even when nothing was opened still runs the step once.
        let checks = Arc::new(AtomicU64::new(0));
        let checks_cb = checks.clone();
        let once = Keychain::new(KeychainOptions {
            backend: Arc::new(StepBackend { steps: Arc::new(Mutex::new(Vec::new())), availability: Availability::Bool(true) }),
            may_ask: true,
            version: "0.854.0".into(),
            opened_by: Arc::new(|| Ok(None)),
            remember_opened: Arc::new(|_| Ok(())),
            keeps_key: Arc::new(move || { checks_cb.fetch_add(1, Ordering::SeqCst); Ok(false) }),
            window_listening: Arc::new(WindowSignal::new),
            say: Arc::new(|_| {}),
            draw_ms: None,
            listen_ms: None,
            wait: Some(Arc::new(|_| {})),
        });
        once.begin();
        once.begin();
        assert_eq!(checks.load(Ordering::SeqCst), 1);
    }
}
