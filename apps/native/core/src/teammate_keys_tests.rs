//! Electron's main/a-teammates-keys-are-kept-for-it-alone.test.ts, with a box that visibly changes the text.
use super::*;

const VALUE: &str = "locust-made-up-key-value-0042";

struct Box_ {
    available: bool,
    forgotten: Mutex<Vec<String>>,
}
impl SecretBox for Box_ {
    fn available(&self) -> bool {
        self.available
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        Ok(format!("sealed:{}", text.bytes().map(|b| format!("{:02x}", b ^ 0x5a)).collect::<String>()))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        let hex = sealed.strip_prefix("sealed:").ok_or("not sealed")?;
        let bytes: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map(|b| b ^ 0x5a).map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }
    fn forget(&self, sealed: &str) {
        self.forgotten.lock().unwrap().push(sealed.to_owned());
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store(available: bool) -> (Scratch, TeammateKeys, Arc<Box_>) {
    let root = std::env::temp_dir().join(format!("locust-keys-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let secrets = Arc::new(Box_ { available, forgotten: Mutex::new(vec![]) });
    (Scratch(root.clone()), TeammateKeys::new(&root, secrets.clone()), secrets)
}

fn key(name: &str, value: &str) -> RunKey {
    RunKey { name: name.into(), value: value.into() }
}

#[test]
fn are_named_as_a_variable_is_and_never_as_one_an_agent_signs_in_with() {
    assert_eq!(key_name_refusal("STRIPE_KEY"), None);
    assert_eq!(key_name_refusal("SLACK_WEBHOOK_URL"), None);
    for bad in ["stripe_key", "1KEY", "A", "MY-KEY", "KEY NAME"] {
        assert!(key_name_refusal(bad).unwrap().contains("capitals, digits and underscores"), "{bad}");
    }
    for reserved in ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY", "GITHUB_TOKEN", "GH_TOKEN", "CODEX_HOME", "PATH", "LOCUST_TELEGRAM_API", "DYLD_INSERT_LIBRARIES"] {
        assert!(key_name_refusal(reserved).unwrap().contains("an agent or this computer reads for itself"), "{reserved}");
    }
    assert_eq!(key_name_refusal("OPENAI_API_KEY").unwrap(), "OPENAI_API_KEY is a name an agent or this computer reads for itself, so a key can\u{2019}t take it. Pick another, like MY_API_KEY.");
    assert_eq!(key_value_refusal("").unwrap(), "Paste the key first.");
    assert!(key_value_refusal("two\nlines").unwrap().contains("one line"));
    assert!(key_value_refusal(&"x".repeat(5_000)).unwrap().contains("longer than any key"));
}

#[test]
fn are_kept_encrypted_listed_by_name_given_back_only_by_teammate_and_gone_with_it() {
    let (s, keys, secrets) = store(true);
    assert_eq!(keys.set("tm_mira", "STRIPE_KEY", &format!("  {VALUE} ")), Ok(vec!["STRIPE_KEY".to_owned()]));
    assert!(keys.set("tm_mira", "OPENAI_API_KEY", VALUE).is_err());
    let file = std::fs::read_to_string(s.0.join(FILE)).unwrap();
    assert!(!file.contains(VALUE) && !file.contains(&VALUE[8..]));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(s.0.join(FILE)).unwrap().permissions().mode() & 0o777, 0o600, "this account's alone");
    assert_eq!(keys.names("tm_mira"), vec!["STRIPE_KEY"]);
    assert!(keys.names("tm_ada").is_empty());
    assert_eq!(keys.run_keys("tm_mira"), vec![key("STRIPE_KEY", VALUE)]);
    assert!(keys.run_keys("tm_ada").is_empty());
    assert!(keys.keeps_key());
    assert_eq!(keys.remove("tm_mira", "STRIPE_KEY"), Ok(vec![]));
    assert_eq!(secrets.forgotten.lock().unwrap().len(), 1, "a removed key leaves the keychain too");
    keys.set("tm_mira", "STRIPE_KEY", VALUE).unwrap();
    keys.forget("tm_mira");
    assert!(keys.run_keys("tm_mira").is_empty());
    assert!(!keys.keeps_key());
}

#[test]
fn are_refused_where_the_computer_cannot_encrypt_them_and_past_twenty() {
    let (_s, plain, _) = store(false);
    assert_eq!(plain.set("tm_mira", "STRIPE_KEY", VALUE).unwrap_err().0, "This computer cannot encrypt the key, so it was not kept.");
    let (_s2, keys, _) = store(true);
    for index in 0..20 {
        assert!(keys.set("tm_mira", &format!("KEY_{index}"), VALUE).is_ok());
    }
    assert!(keys.set("tm_mira", "ONE_MORE", VALUE).is_err());
    assert!(keys.set("tm_mira", "KEY_3", "a-new-value-for-it").is_ok(), "replacing one is not one more");
}

#[test]
fn are_asked_for_by_name_in_the_answer_outside_code_three_at_most() {
    let text = "I need two keys.\n\n<locust-key name=\"STRIPE_KEY\">To make test charges</locust-key>\n<locust-key name=\"SLACK_WEBHOOK_URL\">\n  To post the summary\n</locust-key>\n\n```\n<locust-key name=\"IN_CODE\">no</locust-key>\n```\n<locust-key name=\"STRIPE_KEY\">twice</locust-key>";
    assert_eq!(
        key_requests_in(text),
        vec![KeyRequest { name: "STRIPE_KEY".into(), why: "To make test charges".into() }, KeyRequest { name: "SLACK_WEBHOOK_URL".into(), why: "To post the summary".into() }]
    );
    let four = (0..4).map(|i| format!("<locust-key name=\"K_{i}\">w</locust-key>")).collect::<Vec<_>>().join("\n");
    assert_eq!(key_requests_in(&four).len(), 3);
}

#[test]
fn are_asked_for_in_the_answer_as_one_line_naming_the_key_its_words_left_to_the_card() {
    use crate::protocol_tags::unwrap_protocol_tags;
    assert_eq!(unwrap_protocol_tags("I need one.\n\n<locust-key name=\"WEATHER_KEY\">the key for calling the weather service</locust-key>"), "I need one.\n\nAsks for the key WEATHER_KEY (below).");
    assert_eq!(unwrap_protocol_tags("Write `<locust-key name=\"X\">why</locust-key>` to ask."), "Write `<locust-key name=\"X\">why</locust-key>` to ask.");
}

#[test]
fn are_named_in_the_brief_never_valued_with_how_to_ask_for_another() {
    let brief = keys_section(&["STRIPE_KEY".into()], true);
    assert!(brief.contains("STRIPE_KEY") && brief.contains("$STRIPE_KEY"));
    assert!(brief.contains("Never print, echo, log or write a key's value"));
    assert!(brief.contains("<locust-key name=\"NAME\">"));
    assert!(!keys_section(&[], true).contains("You have these keys"));
    assert!(keys_section(&["STRIPE_KEY".into()], false).contains("Locust does not give keys to this agent yet"));
    assert_eq!(keys_section(&[], false), "");
}

#[test]
fn have_their_values_taken_out_of_anything_longest_first_deep_in_an_event() {
    let keys = [key("SHORT", "abc"), key("A", "secret-value-1"), key("B", "secret-value-12345")];
    assert_eq!(without_key_values("x secret-value-12345 y secret-value-1 abc", &keys), "x [key B] y [key A] abc");
    let event = json!({"type": "tool.completed", "payload": {"output": ["secret-value-1"], "evidence": {"raw": {"text": "ran secret-value-12345"}}, "count": 3}});
    assert_eq!(scrub_key_values(&event, &keys), json!({"type": "tool.completed", "payload": {"output": ["[key A]"], "evidence": {"raw": {"text": "ran [key B]"}}, "count": 3}}));
    assert_eq!(scrub_key_values(&event, &[]), event);
}

#[test]
fn a_keys_value_never_shows_in_a_debug_print() {
    assert_eq!(format!("{:?}", key("STRIPE_KEY", VALUE)), "RunKey(STRIPE_KEY)");
}

#[test]
fn reach_only_the_agents_measured_to_pass_them_on() {
    assert!(reaches("claude") && reaches("cursor") && reaches("opencode"));
    assert!(!reaches("codex") && !reaches("antigravity") && !reaches("copilot"));
}
