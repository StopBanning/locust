    use locust_core::profile_sync::{HttpSyncRemote, SyncHttpFetch, SyncHttpReply};
    use locust_core::your_account::{AccountSyncState, YourAccountOptions};

    #[derive(Default)]
    struct ProfileRelay {
        calls: Mutex<Vec<(String, String)>>,
        objects: Mutex<std::collections::BTreeMap<(String, String), Vec<u8>>>,
    }

    impl SyncHttpFetch for ProfileRelay {
        fn send(&self, url: &str, method: &str, auth: &str, body: Option<&[u8]>) -> Result<SyncHttpReply, String> {
            self.calls.lock().unwrap().push((url.to_owned(), auth.to_owned()));
            let rest = url.strip_prefix("https://relay.test/v1/room/").expect("local test relay");
            let (room, tail) = rest.split_once("/sync").expect("sync route");
            let mut stored = self.objects.lock().unwrap();
            let mut body_out = Vec::new();
            let status = match (method, tail.trim_start_matches('/'), body) {
                ("PUT", id, Some(body)) => {
                    stored.insert((room.to_owned(), id.to_owned()), body.to_vec());
                    200
                }
                ("GET", "", _) => {
                    let mut by_room: std::collections::BTreeMap<&str, Vec<serde_json::Value>> = Default::default();
                    for ((owner, id), body) in stored.iter() {
                        by_room.entry(owner).or_default().push(serde_json::json!({
                            "id": id, "size": body.len(), "at": 1_761_933_600_000u64,
                        }));
                    }
                    let computers: Vec<_> = by_room.into_iter().map(|(room, files)| serde_json::json!({
                        "room": room, "bytes": files.iter().map(|f| f["size"].as_u64().unwrap()).sum::<u64>(),
                        "files": files,
                    })).collect();
                    body_out = serde_json::json!({"used":1234,"quota":1073741824,"computers":computers}).to_string().into_bytes();
                    200
                }
                ("GET", from_and_id, _) => {
                    let (from, id) = from_and_id.split_once('/').expect("copy from other room");
                    if let Some(bytes) = stored.get(&(from.to_owned(), id.to_owned())) {
                        body_out = bytes.clone();
                        200
                    } else {
                        404
                    }
                }
                _ => panic!("unexpected fake profile request: {method} {tail}"),
            };
            Ok(SyncHttpReply { status, body: body_out })
        }
    }

    struct ProfileKeyBox;
    impl locust_core::own_models::SecretBox for ProfileKeyBox {
        fn available(&self) -> bool { true }
        fn seal(&self, text: &str) -> Result<String, String> {
            Ok(text.bytes().map(|b| format!("{b:02x}")).collect())
        }
        fn open(&self, text: &str) -> Result<String, String> {
            let bytes: Result<Vec<_>, _> = text.as_bytes().chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16))
                .collect();
            String::from_utf8(bytes.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        }
    }

    fn account_window(store: locust_core::store::Store, relay: &FakeRelay, fetch: Arc<ProfileRelay>) -> rune::Runtime<State> {
        let mut s = State::default();
        s.engine = Some(Kind::Native);
        s.settings_panel.open = true;
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
        s.phone_anywhere_link.set_test_relay(relay.connector(), "https://relay.test", None);
        let profile = store.root().to_path_buf();
        let options = YourAccountOptions {
            profile: store.root().to_path_buf(),
            link: Arc::new(|| panic!("use the live phone host, not the fake factory's own link")),
            secrets: Arc::new(ProfileKeyBox),
            computer: Arc::new(|| "Colin's PC".into()),
            app_version: "0.845.0".into(),
            busy: Arc::new(|| false),
            remote: Some(Arc::new(move |link| Arc::new(HttpSyncRemote::new(
                &link.relay, &link.room, &link.token, fetch.clone(),
            )))),
            now: None,
            quiet_ms: Some(0),
            copies_fresh_ms: Some(0),
            ask_for_ms: None,
            changed: None,
            restore: Some(Arc::new(move |folder| {
                locust_core::profile_backup::request_restore(folder, &profile, "0.845.0", 0, chrono::Utc::now())
                    .map(|_| ())
            })),
        };
        s.data = Some(store);
        let mut rt = rune::Runtime::new(s, |s, _| {
            col().child(section(s, &Palette::new(true)))
                .child(crate::your_account_section::section(s, &Palette::new(true)))
        }, (1200.0, 4400.0), rune::Env::new(true, 1.0));
        setup(&mut rt.state, Arc::new(|| {}));
        // choose_engine opens the native account host after normal window setup.
        rt.state.settings_panel.account.service_fixture = Some(options);
        rt.frame(0.0);
        rt
    }

    #[test]
    fn account_window_uses_only_the_signed_in_phone_room_and_seals_after_opt_in() {
        let (_root, store) = temp_store("profile-contract");
        let root = store.root().to_path_buf();
        std::fs::write(root.join("memories.json"), r#"{"schemaVersion":2,"memories":[{"text":"Colin ships on Mondays"}]}"#).unwrap();
        let relay = FakeRelay::new();
        let fetch = Arc::new(ProfileRelay::default());
        let mut rt = account_window(store, &relay, fetch.clone());

        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "phone connection", |s| s.phone_anywhere.as_ref().is_some_and(|p| p.connected));
        assert!(rt.find("account-switch").is_none());
        assert!(fetch.calls.lock().unwrap().is_empty());

        relay.push_frame(serde_json::json!({"t":"github","account":{"user":7,"login":"colin"}}));
        pump_until(&mut rt, "account follows the phone sign-in", |s| {
            s.settings_panel.account.status.signed_in.as_ref().is_some_and(|a| a.login == "colin")
        });
        assert!(fetch.objects.lock().unwrap().is_empty());
        assert!(!root.join("account-sync.json").exists());

        click_key(&mut rt, "account-switch");
        pump_until(&mut rt, "sealed copy and a one-time code", |s| {
            s.settings_panel.account.status.sync == AccountSyncState::On && s.settings_panel.account.code.is_some()
        });
        let code = rt.state.settings_panel.account.code.as_ref().unwrap();
        assert_eq!(code.split('-').count(), 11);
        assert!(code.split('-').all(|p| p.len() == 5 && p.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())));
        let raw = std::fs::read_to_string(root.join("account-sync.json")).unwrap();
        assert!(!raw.contains(code));
        let key = locust_core::profile_sync::profile_key_from_code(code).unwrap();
        assert!(!raw.contains(&locust_core::relay_crypto::to_base64url(&key)));
        assert_eq!(serde_json::from_str::<serde_json::Value>(&raw).unwrap()["account"], 7);
        let objects = fetch.objects.lock().unwrap();
        assert!(!objects.is_empty());
        assert!(objects.values().all(|b| !String::from_utf8_lossy(b).contains("Colin ships on Mondays")));
        drop(objects);

        let auth = relay.frames.lock().unwrap().iter().find(|j| j["t"] == "auth").unwrap().clone();
        let saved_room: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(PHONE_ANYWHERE_FILE)).unwrap(),
        ).unwrap();
        let expected_base = format!("https://relay.test/v1/room/{}/sync", saved_room["room"].as_str().unwrap());
        let expected_auth = format!("Bearer {}", auth["token"].as_str().unwrap());
        let asked = fetch.calls.lock().unwrap();
        assert!(!asked.is_empty());
        assert!(asked.iter().all(|(url, auth)| (url == &expected_base || url.starts_with(&format!("{expected_base}/"))) && auth == &expected_auth));
        drop(asked);

        click_key(&mut rt, "phone-github-btn");
        pump_until(&mut rt, "signed out everywhere on this window", |s| s.settings_panel.account.status.signed_in.is_none());
        assert!(rt.find("account-switch").is_none());
        assert!(rt.state.settings_panel.account.code.is_none());
    }

    #[test]
    fn account_window_recovers_the_other_room_only_after_a_second_press() {
        let (_source, source_store) = temp_store("account-recovery-source");
        let (_here, here_store) = temp_store("account-recovery-here");
        let here = here_store.root().to_path_buf();
        let source = Arc::new(ProfileRelay::default());
        let source_relay = FakeRelay::new();
        let here_relay = FakeRelay::new();
        std::fs::write(source_store.root().join("teammates.json"), r#"{"schemaVersion":15,"teammates":[{"name":"Wren"}]}"#).unwrap();
        std::fs::write(here.join("teammates.json"), r#"{"schemaVersion":15,"teammates":[{"name":"Atlas"}]}"#).unwrap();

        let mut pc = account_window(source_store, &source_relay, source.clone());
        click_key(&mut pc, "phone-switch");
        pump_until(&mut pc, "PC connection", |s| s.phone_anywhere.as_ref().is_some_and(|p| p.connected));
        source_relay.push_frame(serde_json::json!({"t":"github","account":{"user":7,"login":"colin"}}));
        pump_until(&mut pc, "PC account", |s| s.settings_panel.account.status.signed_in.is_some());
        click_key(&mut pc, "account-switch");
        pump_until(&mut pc, "source recovery code", |s| s.settings_panel.account.code.is_some());
        let code = pc.state.settings_panel.account.code.clone().unwrap().to_lowercase().replace('-', " ");

        let mut mac = account_window(here_store, &here_relay, source.clone());
        click_key(&mut mac, "phone-switch");
        pump_until(&mut mac, "new computer's connection", |s| s.phone_anywhere.as_ref().is_some_and(|p| p.connected));
        here_relay.push_frame(serde_json::json!({"t":"github","account":{"user":7,"login":"colin"}}));
        pump_until(&mut mac, "copy from another computer", |s| {
            s.settings_panel.account.status.elsewhere.as_ref().is_some_and(|c| c.computers == 1)
        });
        assert!(all_text(&mac.elements()).contains("Another computer keeps a copy of your Locust"));
        let mut misspelled = code.clone();
        let wrong = if &code[2..3] == "0" { "1" } else { "0" };
        misspelled.replace_range(2..3, wrong);
        let count_before_wrong = source.calls.lock().unwrap().len();
        mac.state.settings_panel.account.recovery_input.replace_text(misspelled);
        click_key(&mut mac, "account-bring-button");
        click_key(&mut mac, "account-bring-button");
        pump_until(&mut mac, "bad recovery code presented locally", |s| {
            s.settings_panel.account.failure.as_deref() == Some("That code does not read right. It is 55 letters and digits in groups of five; check each group.")
        });
        assert_eq!(source.calls.lock().unwrap().len(), count_before_wrong);
        assert!(!here.join("account-sync.json").exists());

        mac.state.settings_panel.account.recovery_input.replace_text(code);
        click_key(&mut mac, "account-bring-button");
        let pending = here.join(locust_core::profile_backup::PENDING_RESTORE_FILE);
        assert!(!pending.exists());
        assert!(all_text(&mac.elements()).contains("Prepare the copy"), "first press only prepares confirmation");
        click_key(&mut mac, "account-bring-button");
        pump_until(&mut mac, "staged copy", |s| s.settings_panel.account.bringing.as_deref() == Some("Colin's PC"));
        assert!(pending.exists());
        assert!(std::fs::read_to_string(here.join("teammates.json")).unwrap().contains("Atlas"));
        let applied = locust_core::profile_backup::apply_pending_restore(&here, "0.845.0", chrono::Local::now()).unwrap();
        assert!(applied.ok, "{:?}", applied.reason);
        assert!(std::fs::read_to_string(here.join("teammates.json")).unwrap().contains("Wren"));
        assert!(std::fs::read_to_string(std::path::PathBuf::from(applied.aside.unwrap()).join("teammates.json")).unwrap().contains("Atlas"));
    }
