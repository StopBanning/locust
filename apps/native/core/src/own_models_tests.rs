//! Electron's main/your-own-model.test.ts (the store) and packages/runtime-adapters/test/your-own-model-is-a-provider.test.ts
//! (the OpenCode config), with a stand-in for the operating system's protection that visibly changes the text.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

struct Box_ {
    available: bool,
    forgotten: Mutex<Vec<String>>,
}
impl SecretBox for Box_ {
    fn available(&self) -> bool {
        self.available
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        Ok(format!("sealed:{}", text.chars().rev().collect::<String>()))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        Ok(sealed.trim_start_matches("sealed:").chars().rev().collect())
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

fn store(available: bool) -> (Scratch, OwnModels, Arc<Box_>) {
    let root = std::env::temp_dir().join(format!("locust-own-models-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let secrets = Arc::new(Box_ { available, forgotten: Mutex::new(vec![]) });
    let id = AtomicU32::new(0);
    let models = OwnModels::with(&root, secrets.clone(), Box::new(|| "2026-09-26T09:00:00.000Z".into()), Box::new(move || format!("a1b2c3d{}", id.fetch_add(1, Ordering::SeqCst))));
    (Scratch(root), models, secrets)
}

fn acme(m: &OwnModels) -> Result<PublicOwnModel, String> {
    m.add("Acme Chat", "https://llm.acme.example/v1/", "acme-70b", Some("acme-made-up-key"), false)
}

#[test]
fn is_kept_with_its_key_sealed_and_the_window_is_told_only_that_a_key_is_kept() {
    let (s, models, _) = store(true);
    let added = acme(&models).unwrap();
    assert_eq!(
        added,
        PublicOwnModel { own_id: "a1b2c3d0".into(), name: "Acme Chat".into(), base_url: "https://llm.acme.example/v1".into(), model: "acme-70b".into(), has_key: true, chat_only: false, created_at: "2026-09-26T09:00:00.000Z".into() }
    );
    assert!(!format!("{:?}", models.list().unwrap()).contains("acme-made-up-key"));
    let on_disk = std::fs::read_to_string(s.0.join(FILE)).unwrap();
    assert!(!on_disk.contains("acme-made-up-key") && on_disk.contains("sealed:yek-pu-edam-emca"));
}

#[test]
fn gives_a_run_the_provider_it_needs_key_opened_and_nothing_for_one_no_longer_kept() {
    let (_s, models, secrets) = store(true);
    let added = acme(&models).unwrap();
    let route = own_route_model(&added.own_id, &added.model);
    assert_eq!(route, "own-a1b2c3d0/acme-70b");
    assert!(is_own_route_model(&route));
    assert_eq!(
        models.provider_for(&route).unwrap(),
        Some(("own-a1b2c3d0".into(), Provider { name: "Acme Chat".into(), base_url: "https://llm.acme.example/v1".into(), models: vec!["acme-70b".into()], api_key: Some("acme-made-up-key".into()), tool_calls: true }))
    );
    models.remove(&added.own_id).unwrap();
    assert_eq!(models.provider_for(&route).unwrap(), None);
    assert_eq!(secrets.forgotten.lock().unwrap().clone(), vec!["sealed:yek-pu-edam-emca".to_owned()], "its key goes with it");
}

#[test]
fn is_in_the_model_list_under_opencode_by_its_own_name_marked_as_yours() {
    let (_s, models, _) = store(true);
    models.add("Acme Chat", "https://llm.acme.example/v1/", "acme-70b", None, false).unwrap();
    assert_eq!(
        models.catalog().unwrap(),
        vec![json!({"id": "own-a1b2c3d0/acme-70b", "runtime": "opencode", "displayName": "Acme Chat", "description": "Your model \u{b7} llm.acme.example", "supportedEfforts": [], "own": true})]
    );
}

#[test]
fn one_that_only_chats_gives_its_runs_no_tools_and_says_so_in_the_list() {
    let (_s, models, _) = store(true);
    let plain = models.add("Acme Chat", "https://llm.acme.example/v1/", "acme-70b", Some("acme-made-up-key"), true).unwrap();
    assert!(plain.chat_only);
    let route = own_route_model(&plain.own_id, &plain.model);
    assert!(!models.provider_for(&route).unwrap().unwrap().1.tool_calls);
    let listed = &models.catalog().unwrap()[0];
    assert_eq!(listed["description"], "Your model \u{b7} llm.acme.example \u{b7} chat only");
    assert_eq!(listed["chatOnly"], true);
    assert!(!models.set_chat_only(&plain.own_id, false).unwrap().chat_only);
    assert!(models.provider_for(&route).unwrap().unwrap().1.tool_calls);
    assert!(models.set_chat_only("nobody00", true).unwrap_err().contains("no longer kept"));
}

#[test]
fn keeps_no_key_on_a_machine_that_cannot_protect_one_and_says_so() {
    let (_s, models, _) = store(false);
    assert!(acme(&models).unwrap_err().contains("cannot protect a key"));
    assert!(!models.add("Acme Chat", "https://llm.acme.example/v1/", "acme-70b", Some(""), false).unwrap().has_key);
}

#[test]
fn refuses_what_is_not_a_name_an_address_or_a_model_id_in_words() {
    let (_s, models, _) = store(true);
    assert!(models.add("", "https://llm.acme.example/v1", "acme-70b", None, false).unwrap_err().contains("name"));
    assert!(models.add("Acme", "ftp://llm.example", "acme-70b", None, false).unwrap_err().contains("http:// or https://"));
    assert!(models.add("Acme", "https://llm.example", "two words", None, false).unwrap_err().contains("model is the id"));
    acme(&models).unwrap();
    assert!(acme(&models).unwrap_err().contains("already have a model called Acme Chat"));
}

// ---- the OpenCode config (your-own-model-is-a-provider.test.ts) ----

fn acme_provider() -> Provider {
    Provider { name: "Acme Chat".into(), base_url: "https://llm.acme.example/v1".into(), models: vec!["acme-70b".into()], api_key: Some("acme-made-up-key".into()), tool_calls: true }
}

#[test]
fn is_declared_as_an_openai_compatible_provider_beside_the_runs_own_permissions() {
    let config = with_opencode_providers(json!({"permission": {"edit": "deny"}}), &[("own-a1b2".into(), acme_provider())]).unwrap();
    assert_eq!(
        config["provider"]["own-a1b2"],
        json!({"npm": "@ai-sdk/openai-compatible", "name": "Acme Chat", "options": {"baseURL": "https://llm.acme.example/v1", "apiKey": "acme-made-up-key"}, "models": {"acme-70b": {"name": "acme-70b"}}})
    );
    assert_eq!(config["permission"]["edit"], "deny", "a provider is added, nothing replaced");
}

#[test]
fn sends_a_placeholder_to_an_endpoint_that_asks_for_no_key() {
    let local = Provider { name: "Local".into(), base_url: "http://127.0.0.1:11434/v1".into(), models: vec!["llama3".into()], api_key: None, tool_calls: true };
    assert_eq!(with_opencode_providers(json!({}), &[("own-local".into(), local)]).unwrap()["provider"]["own-local"]["options"]["apiKey"], "not-needed");
}

#[test]
fn turns_every_tool_off_for_a_model_that_only_chats_and_only_for_it() {
    let chat = Provider { tool_calls: false, ..acme_provider() };
    assert_eq!(with_opencode_providers(json!({}), &[("own-chat".into(), chat)]).unwrap()["tools"], json!({"*": false}));
    assert!(with_opencode_providers(json!({}), &[("own-a1b2".into(), acme_provider())]).unwrap().get("tools").is_none());
}

#[test]
fn changes_nothing_without_a_model_of_your_own_and_refuses_a_bad_id_or_address() {
    assert_eq!(with_opencode_providers(json!({"permission": {}}), &[]).unwrap(), json!({"permission": {}}));
    assert!(with_opencode_providers(json!({}), &[("Bad Id".into(), acme_provider())]).unwrap_err().contains("provider id"));
    let file = Provider { base_url: "file:///etc/passwd".into(), ..acme_provider() };
    assert!(with_opencode_providers(json!({}), &[("own-x".into(), file)]).unwrap_err().contains("http(s)"));
}

#[test]
fn the_real_keychain_is_never_touched_in_a_test() {
    let touched = AtomicBool::new(MacKeychain.available());
    assert!(!touched.load(Ordering::SeqCst));
    assert!(MacKeychain.seal("x").is_err() && MacKeychain.open("keychain:x").is_err());
}

#[test]
fn a_run_on_your_own_model_carries_its_provider_in_the_environment_only() {
    let root = std::env::temp_dir().join(format!("locust-own-run-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    use_secrets(Arc::new(Box_ { available: true, forgotten: Mutex::new(vec![]) }));
    let added = for_store(&store).add("Acme Chat", "https://llm.acme.example/v1", "acme-70b", Some("acme-made-up-key"), false).unwrap();
    let route = own_route_model(&added.own_id, &added.model);
    let mut command = std::process::Command::new("opencode");
    command.args(["run", "--model", &route]).env("OPENCODE_CONFIG_CONTENT", json!({"permission": {"edit": "deny"}}).to_string());
    add_provider_env(&mut command, &store, &route).unwrap();
    let env: Value = serde_json::from_str(&command.get_envs().find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT").unwrap().1.unwrap().to_string_lossy()).unwrap();
    assert_eq!(env["provider"][format!("own-{}", added.own_id)]["options"]["apiKey"], "acme-made-up-key");
    assert_eq!(env["permission"]["edit"], "deny");
    assert!(!command.get_args().any(|a| a.to_string_lossy().contains("acme-made-up-key")), "never in the arguments");
    for_store(&store).remove(&added.own_id).unwrap();
    let mut again = std::process::Command::new("opencode");
    assert_eq!(add_provider_env(&mut again, &store, &route).unwrap_err(), "That model is no longer in Your own models. Choose another model for this teammate. Nothing was started.");
    let _ = std::fs::remove_dir_all(&root);
}
