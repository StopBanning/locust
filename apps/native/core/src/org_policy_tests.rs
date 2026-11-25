//! Electron's tests: an-organization-policy-is-read-before-work-starts, a-managed-turn-is-refused-before-an-agent-starts
//! (the gate's words), a-person-cannot-lift-the-organizations-spend-limit (the effective limit) and the shared policy's
//! own rules.
use super::*;

fn state(policy: Value) -> PolicyState {
    let mut skipped = Vec::new();
    PolicyState { path: "/admin/policy.json".into(), present: true, policy: parse_policy(&policy, &mut |k| skipped.push(k.to_owned())).unwrap(), error: None }
}

fn read_text(text: &'static str) -> (PolicyState, Vec<String>) {
    let mut notes = Vec::new();
    let s = read_policy("/admin/policy.json", &move |_| Ok(text.as_bytes().to_vec()), &mut |n| notes.push(n));
    (s, notes)
}

fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |key| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned())
}

fn options<'a>(packaged: bool, platform: &'a str, env: &'a dyn Fn(&str) -> Option<String>, common: &'a dyn Fn() -> Result<String, String>) -> PolicyOptions<'a> {
    PolicyOptions { packaged, platform, env, common_data: common }
}

/// A folder of the test's own, removed when the test ends.
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new(contents: Option<&[u8]>) -> Self {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("locust-policy-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(bytes) = contents {
            std::fs::write(dir.join("policy.json"), bytes).unwrap();
        }
        Fixture(dir)
    }
    fn path(&self) -> String {
        self.0.join("policy.json").to_string_lossy().into_owned()
    }
    /// The app's own launch read, with the development override pointing here (tests are an unpackaged build).
    fn read(&self) -> (PolicyState, Vec<String>) {
        let path = self.path();
        let mut notes = Vec::new();
        let s = read_machine_policy(&move |k| (k == "LOCUST_POLICY_FILE").then(|| path.clone()), &|p| std::fs::read(p), &mut |n| notes.push(n));
        (s, notes)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn uses_the_machine_wide_path_on_each_platform() {
    // case: uses the machine-wide path on each platform and ignores the test override in a packaged app
    assert_eq!(policy_path("win32", Some("D:\\ProgramData")), "D:\\ProgramData\\Locust\\policy.json");
    assert_eq!(policy_path("darwin", None), "/Library/Application Support/Locust/policy.json");
    assert_eq!(policy_path("linux", None), "/etc/locust/policy.json");
    let env = env_of(&[("LOCUST_POLICY_FILE", "/person/escape.json"), ("ProgramData", "C:\\person\\escape")]);
    let known = || Ok("D:\\ProgramData".to_owned());
    assert_eq!(organization_policy_path(&options(true, "win32", &env, &known)).unwrap(), "D:\\ProgramData\\Locust\\policy.json");
    assert_eq!(organization_policy_path(&options(true, "darwin", &env, &known)).unwrap(), "/Library/Application Support/Locust/policy.json");
    assert_eq!(organization_policy_path(&options(true, "linux", &env, &known)).unwrap(), "/etc/locust/policy.json");
}

#[test]
fn uses_the_windows_known_folder_even_if_the_process_changes_program_data() {
    let env = env_of(&[("ProgramData", "C:\\person\\escape")]);
    let known = || Ok("C:\\ProgramData".to_owned());
    assert_eq!(organization_policy_path(&options(true, "win32", &env, &known)).unwrap(), "C:\\ProgramData\\Locust\\policy.json");
    // The registry's own answer is what names the folder; a share or a relative answer is refused.
    let said = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders\r\n    Common AppData    REG_SZ    C:\\ProgramData\r\n\r\n";
    assert_eq!(common_app_data_from_reg(said).as_deref(), Some("C:\\ProgramData"));
    assert_eq!(settle_common_data(common_app_data_from_reg(said)).unwrap(), "C:\\ProgramData");
    assert!(settle_common_data(Some("\\\\server\\share".into())).is_err());
    assert!(settle_common_data(Some("ProgramData".into())).is_err());
    assert_eq!(settle_common_data(None).unwrap_err(), "Windows did not identify its machine-wide application data folder.");
    // Windows not naming it is a closed gate, never the per-process variable.
    let unnamed = || Err("Windows did not identify its machine-wide application data folder.".to_owned());
    let s = read_organization_policy(&options(true, "win32", &env, &unnamed), &|_| panic!("nothing is read"), &mut |_| {});
    assert!(s.error.unwrap().contains("Windows did not identify"));
}

#[test]
fn reads_a_development_override_once_without_changing_its_bytes() {
    let contents = b"{ \"version\": 1, \"highestMode\": \"plan\", \"otherApps\": false }\n";
    let f = Fixture::new(Some(contents));
    let (loaded, notes) = f.read();
    std::fs::write(f.path(), "{}").unwrap();
    assert_eq!(Value::Object(loaded.policy), json!({"version": 1, "highestMode": "plan", "otherApps": false}));
    assert_eq!(loaded.path, f.path());
    assert!(notes.is_empty());
    std::fs::write(f.path(), contents).unwrap();
    f.read();
    assert_eq!(std::fs::read(f.path()).unwrap(), contents);
    // A packaged app never consults it.
    let path = f.path();
    let env = move |k: &str| (k == "LOCUST_POLICY_FILE").then(|| path.clone());
    let known = || Ok("C:\\ProgramData".to_owned());
    assert_eq!(organization_policy_path(&options(true, "linux", &env, &known)).unwrap(), "/etc/locust/policy.json");
}

#[test]
fn leaves_a_missing_file_unrestricted_and_never_creates_it() {
    let f = Fixture::new(None);
    let (s, _) = f.read();
    assert_eq!(s, PolicyState { path: f.path(), present: false, policy: Map::new(), error: None });
    assert!(!std::path::Path::new(&f.path()).exists());
    assert_eq!(turn_refusal(Some(&s), "codex", None, "auto", false, false), None);
}

#[test]
fn accepts_a_utf_8_byte_order_mark_from_windows_deployment_tools() {
    let f = Fixture::new(Some("\u{feff}{\"phone\":false}".as_bytes()));
    let (s, _) = f.read();
    assert_eq!((Value::Object(s.policy), s.error), (json!({"phone": false}), None));
}

#[test]
fn skips_an_unknown_key_with_one_log_line() {
    let f = Fixture::new(Some(br#"{"futureKey":true,"phone":false}"#));
    let (s, notes) = f.read();
    assert_eq!(Value::Object(s.policy), json!({"phone": false}));
    assert_eq!(notes, [format!("Unknown setting \"futureKey\" in {} was skipped.", f.path())]);
}

#[test]
fn fails_closed_on_an_unreadable_file_and_an_invalid_override() {
    let env = env_of(&[]);
    let known = || Ok("C:\\ProgramData".to_owned());
    let s = read_organization_policy(&options(true, "linux", &env, &known), &|_| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)), &mut |_| {});
    let error = s.error.unwrap();
    assert!(error.contains("/etc/locust/policy.json") && error.contains("could not be read"), "{error}");
    let relative = env_of(&[("LOCUST_POLICY_FILE", "relative.json")]);
    let mut notes = Vec::new();
    let s = read_organization_policy(&options(false, "linux", &relative, &known), &|_| panic!("nothing is read"), &mut |n| notes.push(n));
    let error = s.error.unwrap();
    assert!(error.contains("absolute path") && error.contains("relative.json"), "{error}");
    assert_eq!(notes, [error]);
}

#[test]
fn reports_an_invalid_known_setting_and_an_oversized_file_as_blocking_errors() {
    let invalid = Fixture::new(Some(br#"{"phone":"false"}"#));
    let error = invalid.read().0.error.unwrap();
    assert!(error.contains("phone setting is invalid") && error.contains(&invalid.path()), "{error}");
    let large = Fixture::new(Some(" ".repeat(1024 * 1024 + 1).as_bytes()));
    assert!(large.read().0.error.unwrap().contains("exceeds 1 MB"));
}

#[test]
fn validates_every_optional_setting_and_rejects_a_malformed_known_key() {
    let policy = json!({"version": 1, "allowedAgents": POLICY_RUNTIME_IDS, "allowedModels": {"acp": ["model-*"], "codex": ["*"]}, "highestMode": "approve-each", "monthlyLimitUsd": 12.5, "otherApps": false, "phone": true, "chatApps": false, "cloud": false, "terminal": true, "browser": false, "accounts": true});
    let mut unknown: Vec<String> = Vec::new();
    assert_eq!(Value::Object(parse_policy(&policy, &mut |k| unknown.push(k.into())).unwrap()), policy);
    assert!(unknown.is_empty(), "every known key is known: {unknown:?}");
    assert_eq!(Value::Object(parse_policy(&json!({}), &mut |_| {}).unwrap()), json!({}));
    assert_eq!(Value::Object(parse_policy(&json!({"monthlyLimitUsd": 0}), &mut |_| {}).unwrap()), json!({"monthlyLimitUsd": 0}));
    // JSON has no NaN; every other malformed value of Electron's list is refused.
    for bad in [
        json!(null), json!([]), json!(1), json!({"version": 2}), json!({"allowedAgents": ["typo"]}), json!({"allowedAgents": "codex"}),
        json!({"allowedModels": []}), json!({"allowedModels": {"typo": ["*"]}}), json!({"allowedModels": {"codex": "*"}}), json!({"allowedModels": {"codex": [""]}}),
        json!({"highestMode": "edit"}), json!({"monthlyLimitUsd": -1}), json!({"monthlyLimitUsd": "5"}), json!({"otherApps": 1}), json!({"phone": null}),
        json!({"chatApps": "no"}), json!({"cloud": "false"}),
    ] {
        assert!(parse_policy(&bad, &mut |_| {}).is_err(), "{bad}");
    }
}

#[test]
fn enforces_every_mode_ceiling_in_the_documented_authority_order() {
    assert_eq!(POLICY_MODE_ORDER, ["ask", "plan", "approve-each", "accept-edits", "auto"]);
    for (ceiling, highest) in POLICY_MODE_ORDER.iter().enumerate() {
        for (rank, mode) in POLICY_MODE_ORDER.iter().enumerate() {
            assert_eq!(mode_allowed(Some(&state(json!({"highestMode": highest}))), mode), rank <= ceiling, "{highest}: {mode}");
        }
    }
}

#[test]
fn allows_only_listed_agents_including_all_own_acp_agents_under_one_runtime_id() {
    for runtime in POLICY_RUNTIME_IDS {
        let refused = turn_refusal(Some(&state(json!({"allowedAgents": ["acp"]}))), runtime, Some("acp_goose::m"), "approve-each", false, false);
        assert_eq!(refused.is_none(), runtime == "acp", "{runtime}");
    }
    assert!(turn_refusal(Some(&state(json!({"allowedAgents": []}))), "codex", None, "ask", false, false).unwrap().contains("allowedAgents"));
}

#[test]
fn matches_only_allowed_model_ids_with_literal_punctuation_and_star_wildcards() {
    let policy = state(json!({"allowedModels": {"codex": ["gpt-6.*", "exact"], "claude": [], "acp": ["safe-*"]}}));
    assert!(model_allowed(Some(&policy), "codex", Some("gpt-6.1")));
    assert!(!model_allowed(Some(&policy), "codex", Some("gpt-6X1")));
    assert!(model_allowed(Some(&policy), "codex", Some("exact")));
    assert!(!model_allowed(Some(&policy), "claude", Some("sonnet")));
    assert!(model_allowed(Some(&policy), "muse", Some("any")));
    assert!(model_allowed(Some(&policy), "acp", Some("acp_goose::safe-a")));
    assert!(!model_allowed(Some(&policy), "acp", Some("acp_goose::other")));
}

#[test]
fn cannot_use_an_unresolved_default_to_escape_a_model_allowlist() {
    let policy = state(json!({"allowedModels": {"codex": ["account-default", "account-*", "gpt-*"], "acp": ["safe-*"]}}));
    assert!(!model_allowed(Some(&policy), "codex", None));
    assert!(!model_allowed(Some(&policy), "codex", Some("account-default")));
    assert!(!model_allowed(Some(&policy), "acp", Some("acp_goose")));
    assert!(model_allowed(Some(&state(json!({"allowedModels": {"codex": ["*"]}}))), "codex", None));
}

#[test]
fn keeps_only_selectable_family_ids_and_their_efforts() {
    let models = vec![json!({"runtime": "cursor", "id": "family", "displayName": "Family", "description": "", "supportedEfforts": ["low", "high"], "defaultEffort": "high", "variants": {"low": "family-low", "high": "family-high"}})];
    let filtered = policy_models(Some(&state(json!({"allowedModels": {"cursor": ["family-low"]}}))), models.clone());
    assert_eq!(filtered.len(), 1);
    assert_eq!((&filtered[0]["id"], &filtered[0]["variants"], &filtered[0]["supportedEfforts"], &filtered[0]["defaultEffort"]), (&json!("family-low"), &json!({"low": "family-low"}), &json!(["low"]), &json!("low")));
    assert_eq!(policy_models(Some(&state(json!({"allowedAgents": []}))), models.clone()), Vec::<Value>::new());
    assert_eq!(policy_models(None, models.clone()), models);
}

#[test]
fn keeps_each_connection_off_only_when_the_organization_disables_it() {
    for key in ["phone", "chatApps", "otherApps", "cloud"] {
        assert!(feature_refusal(Some(&state(Value::Object(Map::from_iter([(key.to_owned(), json!(false))])))), key).unwrap().contains(key));
        assert_eq!(feature_refusal(Some(&state(Value::Object(Map::from_iter([(key.to_owned(), json!(true))])))), key), None);
        assert_eq!(feature_refusal(Some(&state(json!({}))), key), None);
    }
    assert!(turn_refusal(Some(&state(json!({"cloud": false}))), "codex", None, "ask", true, false).unwrap().contains("cloud"));
    assert_eq!(turn_refusal(Some(&state(json!({"cloud": false}))), "codex", None, "ask", false, false), None);
}

#[test]
fn caps_an_absent_or_higher_personal_limit_while_keeping_a_lower_one() {
    let policy = state(json!({"monthlyLimitUsd": 10}));
    assert_eq!(monthly_limit(Some(&policy), None), Some(10.0));
    assert_eq!(monthly_limit(Some(&policy), Some(50.0)), Some(10.0));
    assert_eq!(monthly_limit(Some(&policy), Some(5.0)), Some(5.0));
    assert_eq!(monthly_limit(Some(&state(json!({"monthlyLimitUsd": 0}))), Some(5.0)), Some(0.0));
    assert_eq!(monthly_limit(Some(&state(json!({}))), None), None);
}

#[test]
fn a_missing_file_read_by_path_is_unrestricted() {
    let s = read_policy("/admin/policy.json", &|_| Err(std::io::Error::from(std::io::ErrorKind::NotFound)), &mut |_| {});
    assert_eq!(s, PolicyState { path: "/admin/policy.json".into(), present: false, policy: Map::new(), error: None });
    assert_eq!(turn_refusal(Some(&s), "codex", None, "auto", false, false), None);
}

#[test]
fn reads_a_policy_accepts_a_byte_order_mark_and_skips_an_unknown_key_with_one_line() {
    let (s, notes) = read_text(r#"{"version":1,"highestMode":"plan","otherApps":false}"#);
    assert_eq!(Value::Object(s.policy), json!({"version": 1, "highestMode": "plan", "otherApps": false}));
    assert!(notes.is_empty());
    let (s, _) = read_text("\u{feff}{\"phone\":false}");
    assert_eq!((Value::Object(s.policy), s.error), (json!({"phone": false}), None));
    let (s, notes) = read_text(r#"{"phone":false,"futureKey":1}"#);
    assert_eq!(Value::Object(s.policy), json!({"phone": false}));
    assert_eq!(notes, ["Unknown setting \"futureKey\" in /admin/policy.json was skipped."]);
}

#[test]
fn fails_closed_for_broken_json_and_names_its_file_in_every_refusal() {
    let (s, _) = read_text("{ not json");
    let error = s.error.clone().expect("closed");
    assert!(error.contains("/admin/policy.json") && error.contains("The file is not valid JSON.") && error.contains("No turns or connections will start"), "{error}");
    assert_eq!(turn_refusal(Some(&s), "codex", Some("gpt"), "ask", false, false), Some(error.clone()));
    assert!(!agent_allowed(Some(&s), "codex") && !mode_allowed(Some(&s), "ask"));
    let (bad, _) = read_text(r#"{"highestMode":"wild"}"#);
    assert!(bad.error.unwrap().contains("The highestMode setting is invalid."));
    let (bad, _) = read_text(r#"{"allowedAgents":["codex","nope"]}"#);
    assert!(bad.error.unwrap().contains("The allowedAgents setting is invalid."));
    let (bad, _) = read_text(r#"{"monthlyLimitUsd":-3}"#);
    assert!(bad.error.unwrap().contains("The monthlyLimitUsd setting is invalid."));
}

#[test]
fn refuses_agents_models_and_modes_in_electrons_words() {
    let s = state(json!({"allowedAgents": ["codex", "claude"], "allowedModels": {"codex": ["gpt-*"]}, "highestMode": "accept-edits"}));
    assert_eq!(
        turn_refusal(Some(&s), "cursor", None, "ask", false, false).unwrap(),
        "Cursor Agent is not an allowed AI agent. Set by your organization (allowedAgents in /admin/policy.json). Nothing was started."
    );
    assert!(turn_refusal(Some(&s), "codex", Some("o9"), "ask", false, false).unwrap().contains("The codex model o9 is not allowed."));
    // A finite allowlist needs an explicit model: the account's default is unknown until it runs.
    assert!(turn_refusal(Some(&s), "codex", None, "ask", false, false).unwrap().contains("account default"));
    assert_eq!(turn_refusal(Some(&s), "codex", Some("gpt-6-astra"), "accept-edits", false, false), None);
    assert_eq!(
        turn_refusal(Some(&s), "codex", Some("gpt-6"), "auto", false, false).unwrap(),
        "Auto exceeds the highest allowed mode, Accept edits. Set by your organization (highestMode in /admin/policy.json). Nothing was started."
    );
    // A runtime with no model list is open.
    assert_eq!(turn_refusal(Some(&s), "claude", None, "ask", false, false), None);
    let empty = state(json!({"allowedAgents": []}));
    assert!(turn_refusal(Some(&empty), "codex", None, "ask", false, false).is_some(), "an empty allowlist denies everything");
}

#[test]
fn turns_off_features_and_cloud_and_accounts() {
    let s = state(json!({"cloud": false, "accounts": false, "phone": false}));
    assert!(turn_refusal(Some(&s), "codex", None, "ask", true, false).unwrap().starts_with("Cloud computers"));
    assert!(turn_refusal(Some(&s), "codex", None, "ask", false, true).unwrap().contains("(accounts in /admin/policy.json)"));
    assert_eq!(feature_refusal(Some(&s), "phone").unwrap(), "Phone pairing is off. Set by your organization (phone in /admin/policy.json). Nothing was started.");
    assert_eq!(feature_refusal(Some(&s), "terminal"), None);
}

#[test]
fn matches_model_patterns_with_only_a_star_special() {
    assert!(model_matches("gpt-*", "gpt-6-astra"));
    assert!(model_matches("*astra", "gpt-6-astra"));
    assert!(model_matches("gpt-*-astra", "gpt-6-astra"));
    assert!(!model_matches("gpt-?", "gpt-6"));
    assert!(model_matches("gpt-.", "gpt-."));
    assert!(!model_matches("gpt", "gpt-6"));
}

#[test]
fn the_effective_limit_is_the_cap_or_a_lower_personal_amount() {
    let s = state(json!({"monthlyLimitUsd": 10}));
    assert_eq!(monthly_limit(Some(&s), None), Some(10.0));
    assert_eq!(monthly_limit(Some(&s), Some(50.0)), Some(10.0));
    assert_eq!(monthly_limit(Some(&s), Some(3.0)), Some(3.0));
    assert_eq!(monthly_limit(None, Some(3.0)), Some(3.0));
}

#[test]
fn says_each_setting_in_words() {
    assert_eq!(setting_words("allowedAgents", &json!(["codex", "acp", "omniroute"])), "Codex CLI, Agents you added, OmniRoute");
    assert_eq!(setting_words("allowedAgents", &json!([])), "None: no AI agent can run");
    assert_eq!(setting_words("highestMode", &json!("accept-edits")), "Accept edits");
    assert_eq!(setting_words("monthlyLimitUsd", &json!(10)), "$10 a month");
    assert_eq!(setting_words("monthlyLimitUsd", &json!(0)), "$0, so nothing that spends can start");
    assert_eq!(setting_words("phone", &json!(false)), "Off");
}

#[test]
fn a_policy_on_one_thread_is_not_seen_on_another_and_leaves_when_cleared() {
    let mine = PolicyState { path: "/this/thread.json".into(), present: true, policy: Map::new(), error: None };
    use_policy_on_this_thread(Some(mine));
    assert_eq!(current().path, "/this/thread.json");
    let elsewhere = std::thread::spawn(|| current().path).join().unwrap();
    assert_ne!(elsewhere, "/this/thread.json");
    use_policy_on_this_thread(None);
    assert_ne!(current().path, "/this/thread.json");
}

// case: loads once at launch, keeps the test seam out of packaged builds and exposes errors in the window
#[test]
fn the_launch_read_happens_once_and_its_seam_is_only_for_unpackaged_builds() {
    let f = Fixture::new(Some(br#"{"phone":false}"#));
    // This test build is unpackaged, so the app's own launch read takes the override...
    assert!(cfg!(debug_assertions));
    assert_eq!(Value::Object(f.read().0.policy), json!({"phone": false}));
    // ...which a packaged app never consults.
    let path = f.path();
    let env = move |k: &str| (k == "LOCUST_POLICY_FILE").then(|| path.clone());
    let known = || Ok("C:\\ProgramData".to_owned());
    assert_eq!(read_organization_policy(&options(true, "linux", &env, &known), &|_| Err(std::io::Error::from(std::io::ErrorKind::NotFound)), &mut |_| {}).path, "/etc/locust/policy.json");
    // Read once: what the process holds stays as it was read (the window shows its error: settings_panel's broken-policy alert).
    assert_eq!(current(), current());
}

// case: refuses an unmetered launch when the monthly cap cannot be enforced
#[test]
fn refuses_an_unmetered_launch_when_the_monthly_cap_cannot_be_enforced() {
    let capped = state(json!({"monthlyLimitUsd": 5}));
    let refused = unmetered_refusal(Some(&capped), "claude", None, "auto", false, false)
        .expect("unmetered launch refused under monthly cap");
    assert!(refused.contains("monthlyLimitUsd"), "{refused}");
    assert_eq!(unmetered_refusal(Some(&state(json!({}))), "claude", None, "auto", false, false), None);
}

// case: refuses an unrestricted cloud terminal when it cannot enforce the managed route
#[test]
fn refuses_an_unrestricted_cloud_terminal_when_it_cannot_enforce_the_managed_route() {
    let by_agent = state(json!({"allowedAgents": ["codex"]}));
    assert!(cloud_terminal_refusal(Some(&by_agent)).unwrap().contains("allowedAgents"));

    let by_model = state(json!({"allowedModels": {"claude": ["sonnet"]}}));
    assert!(cloud_terminal_refusal(Some(&by_model)).unwrap().contains("allowedModels"));

    let by_mode = state(json!({"highestMode": "plan"}));
    assert!(cloud_terminal_refusal(Some(&by_mode)).unwrap().contains("highestMode"));

    assert_eq!(cloud_terminal_refusal(Some(&state(json!({})))), None);
}
