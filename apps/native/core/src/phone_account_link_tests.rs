    fn saved_room(relay: &str, closing: bool, github: Value) -> (std::path::PathBuf, PhoneAnywhere, Arc<AtomicBool>) {
        let dir = std::env::temp_dir().join(format!("locust-account-link-{}", random_text(12)));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(PHONE_ANYWHERE_FILE), json!({
            "v": 1,
            "relay": relay,
            "room": "R".repeat(22),
            "token": "t".repeat(43),
            "key": {},
            "publicKey": "P".repeat(43),
            "devices": [],
            "closing": closing,
            "github": github
        }).to_string()).unwrap();
        let off = Arc::new(AtomicBool::new(false));
        let policy = off.clone();
        let phone = PhoneAnywhere::new(PhoneAnywhereOptions {
            directory: dir.clone(),
            calls: Arc::new(NoopCalls),
            relay: Arc::new(|| unreachable!("only a restored room is used")),
            refusal: Some(Arc::new(move || policy.load(Ordering::SeqCst).then(|| "Phone is off.".into()))),
            changed: None,
            connect: Arc::new(|_| Err("fake: no line".into())),
            now: None,
            protect_file: Some(Arc::new(|_| Ok(()))),
            retry_ms: None,
            away: None,
            hooks: None,
            desk: None,
            notes: None,
            github_user: None,
            computer_name: None,
            mcp: None,
        });
        phone.load();
        (dir, phone, off)
    }

    #[test]
    fn account_link_keeps_the_saved_identity_when_offline_but_loses_it_on_dispose() {
        let (dir, phone, _) = saved_room("https://relay.test", false, json!({"user": 7, "login": "colin"}));
        assert!(!phone.connected());
        let link = phone.account_link().expect("local signed-in room");
        assert_eq!(link.user, 7);
        assert_eq!(link.login, "colin");
        assert_eq!(link.relay, "https://relay.test");
        assert_eq!(link.room, "R".repeat(22));
        assert_eq!(link.token, "t".repeat(43));

        phone.dispose();
        assert!(phone.account_link().is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn account_link_refuses_a_closing_room_plain_http_and_a_managed_phone() {
        let (dir, closing, _) = saved_room("https://relay.test", true, json!({"user": 7, "login": "colin"}));
        let (http_dir, http, _) = saved_room("http://relay.test", false, json!({"user": 7, "login": "colin"}));
        let (managed_dir, managed, off) = saved_room("https://relay.test", false, json!({"user": 7, "login": "colin"}));
        assert!(managed.account_link().is_some());
        off.store(true, Ordering::SeqCst);
        for (dir, phone) in [(dir, closing), (http_dir, http), (managed_dir, managed)] {
            let absent = phone.account_link().is_none();
            phone.dispose();
            fs::remove_dir_all(dir).unwrap();
            assert!(absent);
        }
    }

    #[test]
    fn account_link_refuses_invalid_saved_github_identity() {
        for bad in [json!({"user":0,"login":"colin"}), json!({"user":7,"login":"not a GitHub login"})] {
            let (dir, phone, _) = saved_room("https://relay.test", false, bad);
            let absent = phone.account_link().is_none();
            phone.dispose();
            fs::remove_dir_all(dir).unwrap();
            assert!(absent);
        }
    }
