//! Electron's main/a-model-of-your-own-is-priced-as-listed.test.ts (0.810): a model of your own at its provider's own
//! address, under the id the provider lists, is told its listed price from OpenCode's copy of models.dev's list, and
//! OpenCode reports its runs in dollars. Anything less than that exact match is not priced, and reports tokens as before.
use super::*;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

fn haiku() -> Value {
    json!({"input": 0.1, "output": 0.5, "cache_read": 0.01, "cache_write": 0.125, "tiers": [{"input": 0.5, "output": 2.5, "cache_read": 0.05, "cache_write": 0.625, "tier": {"type": "context", "size": 100000}}]})
}

fn list() -> Value {
    json!({
        "anthropic": {
            "id": "anthropic",
            "name": "Anthropic",
            "npm": "@ai-sdk/anthropic",
            "models": {
                "claude-haiku-5-5": {"id": "claude-haiku-5-5", "cost": haiku()},
                "claude-sonnet-5-5": {"id": "claude-sonnet-5-5", "cost": {"input": 2, "output": 10, "cache_read": 0.1, "cache_write": 2.5}},
                "claude-unpriced": {"id": "claude-unpriced"}
            }
        },
        "deepseek": {"api": "https://api.deepseek.com", "models": {"deepseek-chat": {"cost": {"input": 0.28, "output": 0.42}}}},
        // A pay-as-you-go account and a plan at one host, listing one model at two prices.
        "zai": {"api": "https://api.z.ai/api/paas/v4", "models": {"glm-5": {"cost": {"input": 1, "output": 3.2}}}},
        "zai-coding-plan": {"api": "https://api.z.ai/api/coding/paas/v4", "models": {"glm-5": {"cost": {"input": 0, "output": 0}}}}
    })
}

// case: is its provider's listed one, at the provider's own address, tiers and all
#[test]
fn is_its_providers_listed_one_at_the_providers_own_address_tiers_and_all() {
    assert_eq!(listed_price(&list(), "https://api.anthropic.com/v1", "claude-haiku-5-5"), Some(haiku()));
    assert_eq!(listed_price(&list(), "https://API.anthropic.com/v1/", "claude-sonnet-5-5"), Some(json!({"input": 2, "output": 10, "cache_read": 0.1, "cache_write": 2.5})));
    // A provider the list gives an address for is matched by it.
    assert_eq!(listed_price(&list(), "https://api.deepseek.com/v1", "deepseek-chat"), Some(json!({"input": 0.28, "output": 0.42})));
}

// case: is nothing anywhere less than an exact match
#[test]
fn is_nothing_anywhere_less_than_an_exact_match() {
    // The same id at a gateway, a reseller, or a server of the person's own.
    assert_eq!(listed_price(&list(), "https://llm.acme.example/v1", "claude-sonnet-5-5"), None);
    assert_eq!(listed_price(&list(), "http://127.0.0.1:11434/v1", "claude-sonnet-5-5"), None);
    // An id the provider does not list, or lists with no price.
    assert_eq!(listed_price(&list(), "https://api.anthropic.com/v1", "claude-sonnet-5-5-custom"), None);
    assert_eq!(listed_price(&list(), "https://api.anthropic.com/v1", "claude-unpriced"), None);
    // No list, or a list that is not one.
    assert_eq!(listed_price(&Value::Null, "https://api.anthropic.com/v1", "claude-sonnet-5-5"), None);
    assert_eq!(listed_price(&json!([]), "https://api.anthropic.com/v1", "claude-sonnet-5-5"), None);
    // A port is part of the host: the list's own address at another port is somewhere else.
    let at_port = json!({"local": {"api": "http://127.0.0.1:8080/v1", "models": {"m": {"cost": {"input": 1, "output": 2}}}}});
    assert_eq!(listed_price(&at_port, "http://127.0.0.1:8080/v1", "m"), Some(json!({"input": 1, "output": 2})));
    assert_eq!(listed_price(&at_port, "http://127.0.0.1:9090/v1", "m"), None);
}

// case: tells a plan and an account at one host apart by their paths, and prices neither when it cannot
#[test]
fn tells_a_plan_and_an_account_at_one_host_apart_by_their_paths_and_prices_neither_when_it_cannot() {
    assert_eq!(listed_price(&list(), "https://api.z.ai/api/paas/v4", "glm-5"), Some(json!({"input": 1, "output": 3.2})));
    assert_eq!(listed_price(&list(), "https://api.z.ai/api/coding/paas/v4", "glm-5"), Some(json!({"input": 0, "output": 0})));
    // A last slash is not a different path.
    assert_eq!(listed_price(&list(), "https://api.z.ai/api/paas/v4/", "glm-5"), Some(json!({"input": 1, "output": 3.2})));
    assert_eq!(listed_price(&list(), "https://api.z.ai/other/v1", "glm-5"), None);
    // Two at one host that agree are one price.
    let agreeing = json!({"a": {"api": "https://api.z.ai/a", "models": {"glm-5": {"cost": {"input": 1, "output": 2}}}}, "b": {"api": "https://api.z.ai/b", "models": {"glm-5": {"cost": {"input": 1, "output": 2}}}}});
    assert_eq!(listed_price(&agreeing, "https://api.z.ai/c", "glm-5"), Some(json!({"input": 1, "output": 2})));
}

// case: drops what is not a price rather than passing it to OpenCode
#[test]
fn drops_what_is_not_a_price_rather_than_passing_it_to_opencode() {
    let odd = json!({"anthropic": {"models": {
        "m": {"cost": {"input": 1, "output": "two"}},
        "n": {"cost": {"input": 1, "output": 2, "cache_read": -1, "tiers": [{"input": 3}, {"input": 4, "output": 5, "tier": {"type": "context", "size": 1000}}, {"input": 6, "output": 7, "tier": {"type": "tokens", "size": 1000}}, {"input": 8, "output": 9, "tier": {"type": "context", "size": "big"}}]}}
    }}});
    assert_eq!(listed_price(&odd, "https://api.anthropic.com/v1", "m"), None);
    assert_eq!(listed_price(&odd, "https://api.anthropic.com/v1", "n"), Some(json!({"input": 1, "output": 2, "tiers": [{"input": 4, "output": 5, "tier": {"type": "context", "size": 1000}}]})));
    // A price over 200k of context is kept when it is whole, dropped when it is not.
    let over = json!({"anthropic": {"models": {
        "o": {"cost": {"input": 1, "output": 2, "context_over_200k": {"input": 2, "output": 4, "cache_read": 0.2}}},
        "p": {"cost": {"input": 1, "output": 2, "context_over_200k": {"input": 2}}}
    }}});
    assert_eq!(listed_price(&over, "https://api.anthropic.com/v1", "o"), Some(json!({"input": 1, "output": 2, "context_over_200k": {"input": 2, "output": 4, "cache_read": 0.2}})));
    assert_eq!(listed_price(&over, "https://api.anthropic.com/v1", "p"), Some(json!({"input": 1, "output": 2})));
}

// case: is read from OpenCode's cache folder, once while the file is unchanged
#[test]
fn is_read_from_opencodes_cache_folder_once_while_the_file_is_unchanged() {
    assert_eq!(opencode_price_list_path(None, Path::new("C:/Users/someone")), Path::new("C:/Users/someone").join(".cache").join("opencode").join("models.json"));
    assert_eq!(opencode_price_list_path(Some(""), Path::new("/home/me")), Path::new("/home/me").join(".cache").join("opencode").join("models.json"));
    assert_eq!(opencode_price_list_path(Some("/var/cache/me"), Path::new("/home/me")), Path::new("/var/cache/me").join("opencode").join("models.json"));
    let reads = Arc::new(AtomicU32::new(0));
    let at = Arc::new(AtomicU64::new(1));
    let (r, a) = (reads.clone(), at.clone());
    let prices = PriceList::with(
        "models.json".into(),
        Box::new(move |_| {
            r.fetch_add(1, Ordering::SeqCst);
            Ok(list().to_string())
        }),
        Box::new(move |_| Ok(a.load(Ordering::SeqCst) as u128)),
    );
    assert_eq!(prices.price_at("https://api.anthropic.com/v1", "claude-sonnet-5-5").unwrap()["input"], 2);
    assert_eq!(prices.price_at("https://api.anthropic.com/v1", "claude-haiku-5-5").unwrap()["input"], 0.1);
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    at.store(2, Ordering::SeqCst);
    prices.price_at("https://api.anthropic.com/v1", "claude-haiku-5-5");
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    // No list -- OpenCode never run here -- is no price, not an error.
    let none = PriceList::new(std::env::temp_dir().join(format!("locust-no-models-{}-{}.json", std::process::id(), crate::invariants::unique_nanos())));
    assert_eq!(none.price_at("https://api.anthropic.com/v1", "claude-sonnet-5-5"), None);
    // A real file is read, and a list that is not JSON is no price either.
    let file = std::env::temp_dir().join(format!("locust-models-{}-{}.json", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::write(&file, list().to_string()).unwrap();
    assert_eq!(PriceList::new(file.clone()).price_at("https://api.deepseek.com/v1", "deepseek-chat"), Some(json!({"input": 0.28, "output": 0.42})));
    std::fs::write(&file, "{not json").unwrap();
    assert_eq!(PriceList::new(file.clone()).price_at("https://api.deepseek.com/v1", "deepseek-chat"), None);
    let _ = std::fs::remove_file(&file);
}

struct OpenBox;
impl SecretBox for OpenBox {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        Ok(sealed.to_owned())
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn priced_store(id: &'static str) -> (Scratch, OwnModels) {
    let root = std::env::temp_dir().join(format!("locust-own-priced-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let prices = PriceList::with("models.json".into(), Box::new(|_| Ok(list().to_string())), Box::new(|_| Ok(1)));
    let models = OwnModels::with(&root, Arc::new(OpenBox), Box::new(|| "2026-10-21T09:00:00.000Z".into()), Box::new(move || id.into())).with_prices(Arc::new(prices));
    (Scratch(root), models)
}

// case: carries the listed price into OpenCode's config, where OpenCode prices each step with it
#[test]
fn carries_the_listed_price_into_opencodes_config_where_opencode_prices_each_step_with_it() {
    let (_s, models) = priced_store("0a1b2c3d");
    let haiku_model = models.add("claude-haiku-5-5", "https://api.anthropic.com/v1", "claude-haiku-5-5", Some("sk-test"), false).unwrap();
    let found = models.provider_for(&own_route_model(&haiku_model.own_id, &haiku_model.model)).unwrap().unwrap();
    assert_eq!(found.1.prices, Some(Map::from_iter([("claude-haiku-5-5".to_owned(), haiku())])));
    let config = with_opencode_providers(json!({}), &[found]).unwrap();
    assert_eq!(config["provider"]["own-0a1b2c3d"]["models"], json!({"claude-haiku-5-5": {"name": "claude-haiku-5-5", "cost": haiku()}}));
    // A run's own models are read with OpenCode's list, the one this process keeps.
    let root = Scratch(std::env::temp_dir().join(format!("locust-own-priced-store-{}-{}", std::process::id(), crate::invariants::unique_nanos())));
    let store = crate::store::Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    assert!(for_store(&store).prices.as_ref().is_some_and(|kept| Arc::ptr_eq(kept, &opencode_prices())));
}

// case: carries none for a model its provider does not list, and its config is as it was
#[test]
fn carries_none_for_a_model_its_provider_does_not_list_and_its_config_is_as_it_was() {
    let (_s, models) = priced_store("0a1b2c3e");
    let acme = models.add("Acme Chat", "https://llm.acme.example/v1", "acme-70b", None, false).unwrap();
    let found = models.provider_for(&own_route_model(&acme.own_id, &acme.model)).unwrap().unwrap();
    assert_eq!(found.1.prices, None);
    let config = with_opencode_providers(json!({}), &[found]).unwrap();
    assert_eq!(config["provider"]["own-0a1b2c3e"]["models"], json!({"acme-70b": {"name": "acme-70b"}}));
}
