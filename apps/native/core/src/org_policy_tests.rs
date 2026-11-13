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

#[test]
fn uses_the_machine_wide_path_on_each_platform() {
    assert_eq!(policy_path("win32", Some("D:\\ProgramData")), "D:\\ProgramData\\Locust\\policy.json");
    assert_eq!(policy_path("darwin", None), "/Library/Application Support/Locust/policy.json");
    assert_eq!(policy_path("linux", None), "/etc/locust/policy.json");
}

#[test]
fn leaves_a_missing_file_unrestricted() {
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
