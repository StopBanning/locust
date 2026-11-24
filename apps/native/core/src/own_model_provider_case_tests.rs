// Carries the five cases of packages/runtime-adapters/test/your-own-model-is-a-provider.test.ts (Electron 4ce2f37a) not
// yet named, through what a run on a model of your own is given: OpenCode's served command for its mode
// (`runtime::opencode::command_for_version`, as `command_for_turn` builds it for OpenCode 1) and then the provider in
// its environment (`add_provider_env`, as native.rs adds it), and the config merge itself (`with_opencode_providers`).
use super::*;
use crate::approval::Mode;

/// The same stand-in for the operating system's protection as own_models_tests.rs (one is shared by every test).
struct Stand;
impl SecretBox for Stand {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        Ok(format!("sealed:{}", text.chars().rev().collect::<String>()))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        Ok(sealed.trim_start_matches("sealed:").chars().rev().collect())
    }
    fn forget(&self, _sealed: &str) {}
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A store holding Acme's model, and its route.
fn kept() -> (Scratch, crate::store::Store, String) {
    let root = std::env::temp_dir().join(format!("locust-own-case-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    use_secrets(Arc::new(Stand));
    let added = for_store(&store).add("Acme Chat", "https://llm.acme.example/v1", "acme-70b", Some(&key()), false).unwrap();
    let route = own_route_model(&added.own_id, &added.model);
    (Scratch(root), store, route)
}
fn key() -> String {
    format!("{}-acme-secret", "made-up")
}
/// The served OpenCode 1 command a run in `mode` starts, with the provider added as native.rs adds it.
fn served(store: &crate::store::Store, folder: &Path, mode: Mode, route: &str) -> std::process::Command {
    let mut command = crate::runtime::opencode::command_for_version(PathBuf::from("opencode"), "1.18.27", folder, mode, store, false).unwrap();
    add_provider_env(&mut command, store, route).unwrap();
    command
}
fn config_of(command: &std::process::Command) -> Value {
    let text = command.get_envs().find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT").and_then(|(_, v)| v).unwrap();
    serde_json::from_str(&text.to_string_lossy()).unwrap()
}
fn acme() -> Provider {
    Provider { name: "Acme Chat".into(), base_url: "https://llm.acme.example/v1".into(), models: vec!["acme-70b".into()], api_key: Some(key()), tool_calls: true, prices: None }
}

// case: puts the key in the environment only -- never in the arguments
#[test]
fn an_own_models_key_goes_in_the_runs_environment_and_never_its_arguments() {
    let (s, store, route) = kept();
    let command = served(&store, &s.0.join("work"), Mode::AcceptEdits, &route);
    assert!(!command.get_args().any(|a| a.to_string_lossy().contains(&key())), "{:?}", command.get_args().collect::<Vec<_>>());
    assert!(config_of(&command).to_string().contains(&key()));
}

// case: works for a run that stops to ask, too
#[test]
fn an_own_model_is_declared_for_an_approve_each_run_too() {
    let (s, store, route) = kept();
    let config = config_of(&served(&store, &s.0.join("work"), Mode::ApproveEach, &route));
    let id = route.split('/').next().unwrap();
    assert_eq!(config["provider"][id]["options"]["baseURL"], "https://llm.acme.example/v1");
    assert_eq!(config["permission"]["edit"], "ask");
}

// case: declares a model's listed price, which OpenCode prices each step with, and none where none is given (0.810)
#[test]
fn an_own_models_listed_price_is_declared_and_none_without_one() {
    let price = json!({ "input": 2, "output": 10, "cache_read": 0.1, "cache_write": 2.5 });
    let priced = Provider { prices: Some(Map::from_iter([("acme-70b".to_owned(), price.clone())])), ..acme() };
    let config = with_opencode_providers(json!({}), &[("own-a1b2".into(), priced)]).unwrap();
    assert_eq!(config["provider"]["own-a1b2"]["models"], json!({ "acme-70b": { "name": "acme-70b", "cost": price } }));
    let unpriced = with_opencode_providers(json!({}), &[("own-a1b2".into(), acme())]).unwrap();
    assert_eq!(unpriced["provider"]["own-a1b2"]["models"], json!({ "acme-70b": { "name": "acme-70b" } }));
}

// case: changes nothing when there is no model of your own
#[test]
fn a_run_without_a_model_of_your_own_keeps_its_config_as_it_was() {
    assert_eq!(with_opencode_providers(json!({ "permission": {} }), &[]).unwrap(), json!({ "permission": {} }));
    // A run on a model that is not one of yours is given no provider: native.rs adds one only for an own route.
    assert!(!is_own_route_model("opencode/some-free-model"));
}

// case: refuses an id OpenCode could not take, and an address that is not http(s)
#[test]
fn a_provider_id_opencode_cannot_take_or_an_address_that_is_not_http_is_refused() {
    assert!(with_opencode_providers(json!({}), &[("Bad Id".into(), acme())]).unwrap_err().contains("provider id"));
    let file = Provider { base_url: "file:///etc/passwd".into(), ..acme() };
    assert!(with_opencode_providers(json!({}), &[("own-x".into(), file)]).unwrap_err().contains("http(s)"));
}
