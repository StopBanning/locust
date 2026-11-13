//! Your own models (Electron `main/own-models.ts`, 0.357): any endpoint that speaks the OpenAI chat API (a company
//! gateway, vLLM, Ollama's /v1, LM Studio, Anthropic's compatible endpoint) runs through the OpenCode path as a provider
//! of its own. This keeps the list, and the keys.
//!
//! A key is kept by the operating system, never in the clear: sealed before anything touches the disk, opened only to
//! start a run, and never handed to the window, which is told only that one is kept. A machine that cannot protect a
//! key keeps none, and says so.

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const FILE: &str = "own-models.json";
pub const MAX_OWN_MODELS: usize = 20;
const OWN_PREFIX: &str = "own-";

/// What the operating system's own protection is asked to do with a key: the file keeps only what `seal` returns.
pub trait SecretBox: Send + Sync {
    fn available(&self) -> bool;
    fn seal(&self, text: &str) -> Result<String, String>;
    fn open(&self, sealed: &str) -> Result<String, String>;
    /// Let a kept key go when its model is removed.
    fn forget(&self, _sealed: &str) {}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnModel {
    pub own_id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub created_at: String,
    /// What the box sealed, never the key.
    pub sealed_key: Option<String>,
    /// A model that only chats: its runs get no tools (0.358).
    pub chat_only: bool,
}

/// What the window is told: everything but the key, and whether one is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicOwnModel {
    pub own_id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
    pub chat_only: bool,
    pub created_at: String,
}

/// An OpenCode provider for one run (OpenCodeProvider).
#[derive(Clone, Debug, PartialEq)]
pub struct Provider {
    pub name: String,
    pub base_url: String,
    pub models: Vec<String>,
    pub api_key: Option<String>,
    /// false: a model that only chats, so the run gets no tools.
    pub tool_calls: bool,
}

/// The address as it is kept: http(s), no trailing slash, no spaces.
pub fn own_model_address(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed.len() > 500 || trimmed.contains(char::is_whitespace) {
        return None;
    }
    let url = url::Url::parse(trimmed).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| trimmed.to_owned())
}

/// A model id as endpoints name them: `acme-70b`, `llama3:8b`, `meta-llama/Llama-3.1-8B`.
pub fn own_model_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && trimmed.len() <= 200 && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || ".:/@+-".contains(c))).then(|| trimmed.to_owned())
}

fn own_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && trimmed.chars().count() <= 60 && !trimmed.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f)).then(|| trimmed.to_owned())
}

pub fn own_route_model(own_id: &str, model: &str) -> String {
    format!("{OWN_PREFIX}{own_id}/{model}")
}

pub fn is_own_route_model(model: &str) -> bool {
    model.starts_with(OWN_PREFIX)
}

fn host_of(base_url: &str) -> String {
    url::Url::parse(base_url).ok().and_then(|u| u.host_str().map(|h| match u.port() { Some(p) => format!("{h}:{p}"), None => h.to_owned() })).unwrap_or_else(|| base_url.to_owned())
}

fn parsed(v: &Value) -> Option<OwnModel> {
    let own_id = v["ownId"].as_str().filter(|id| id.len() == 8 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))?;
    let created_at = v["createdAt"].as_str().filter(|t| chrono::DateTime::parse_from_rfc3339(t).is_ok())?;
    Some(OwnModel {
        own_id: own_id.into(),
        name: own_name(v["name"].as_str()?)?,
        base_url: own_model_address(v["baseUrl"].as_str()?)?,
        model: own_model_id(v["model"].as_str()?)?,
        created_at: created_at.into(),
        sealed_key: v["key"].as_str().filter(|k| !k.is_empty()).map(str::to_owned),
        chat_only: v["chatOnly"] == true,
    })
}

fn stored(m: &OwnModel) -> Value {
    let mut v = json!({"ownId": m.own_id, "name": m.name, "baseUrl": m.base_url, "model": m.model, "createdAt": m.created_at});
    if let Some(key) = &m.sealed_key {
        v["key"] = json!(key);
    }
    if m.chat_only {
        v["chatOnly"] = json!(true);
    }
    v
}

impl OwnModel {
    pub fn public(&self) -> PublicOwnModel {
        PublicOwnModel {
            own_id: self.own_id.clone(),
            name: self.name.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            has_key: self.sealed_key.is_some(),
            chat_only: self.chat_only,
            created_at: self.created_at.clone(),
        }
    }
}

pub struct OwnModels {
    path: PathBuf,
    secrets: Arc<dyn SecretBox>,
    now: Box<dyn Fn() -> String + Send + Sync>,
    create_id: Box<dyn Fn() -> String + Send + Sync>,
    serial: Mutex<()>,
}

impl OwnModels {
    pub fn new(root: &Path, secrets: Arc<dyn SecretBox>) -> Self {
        Self::with(
            root,
            secrets,
            Box::new(|| chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            Box::new(|| crate::native::uuid_v4().map(|u| u.replace('-', "")[..8].to_owned()).unwrap_or_else(|_| format!("{:08x}", crate::invariants::unique_nanos() as u32))),
        )
    }

    pub fn with(root: &Path, secrets: Arc<dyn SecretBox>, now: Box<dyn Fn() -> String + Send + Sync>, create_id: Box<dyn Fn() -> String + Send + Sync>) -> Self {
        Self { path: root.join(FILE), secrets, now, create_id, serial: Mutex::new(()) }
    }

    fn read(&self) -> Result<Vec<OwnModel>, String> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err("Your own models could not be read.".into()),
        };
        let v: Value = serde_json::from_str(&text).map_err(|_| "Your own models could not be read.".to_owned())?;
        let models = v["models"].as_array().filter(|_| v["schemaVersion"] == 1).ok_or("Your own models could not be read.")?;
        Ok(models.iter().filter_map(parsed).collect())
    }

    fn write(&self, models: &[OwnModel]) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let body = json!({"schemaVersion": 1, "models": models.iter().map(stored).collect::<Vec<_>>()}).to_string();
        let temp = self.path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temp, body).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &self.path).map_err(|e| e.to_string())
    }

    pub fn list(&self) -> Result<Vec<PublicOwnModel>, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read()?.iter().map(OwnModel::public).collect())
    }

    pub fn add(&self, name: &str, base_url: &str, model: &str, key: Option<&str>, chat_only: bool) -> Result<PublicOwnModel, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        let name = own_name(name).ok_or("Give it a name of up to 60 characters.")?;
        let base_url = own_model_address(base_url).ok_or("The address must be an http:// or https:// address, like https://llm.example.com/v1.")?;
        let model = own_model_id(model).ok_or("The model is the id the endpoint knows it by, like acme-70b or llama3:8b.")?;
        let key = key.map(str::trim).filter(|k| !k.is_empty());
        if key.is_some_and(|k| k.len() > 4000) {
            return Err("That key is longer than any key an endpoint issues.".into());
        }
        if key.is_some() && !self.secrets.available() {
            return Err("This machine cannot protect a key, so it was not kept. Add the model without one, or on a machine that can.".into());
        }
        let mut models = self.read()?;
        if models.len() >= MAX_OWN_MODELS {
            return Err(format!("Up to {MAX_OWN_MODELS} models of your own can be kept."));
        }
        if models.iter().any(|m| m.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("You already have a model called {name}."));
        }
        let sealed_key = match key {
            Some(k) => Some(self.secrets.seal(k).map_err(|_| "This machine could not protect the key, so it was not kept. Nothing was added.".to_owned())?),
            None => None,
        };
        let added = OwnModel { own_id: (self.create_id)(), name, base_url, model, created_at: (self.now)(), sealed_key, chat_only };
        models.push(added.clone());
        self.write(&models)?;
        Ok(added.public())
    }

    pub fn set_chat_only(&self, own_id: &str, chat_only: bool) -> Result<PublicOwnModel, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        let mut models = self.read()?;
        let kept = models.iter_mut().find(|m| m.own_id == own_id).ok_or("That model is no longer kept.")?;
        kept.chat_only = chat_only;
        let public = kept.public();
        self.write(&models)?;
        Ok(public)
    }

    pub fn remove(&self, own_id: &str) -> Result<(), String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        let models = self.read()?;
        let (gone, kept): (Vec<OwnModel>, Vec<OwnModel>) = models.into_iter().partition(|m| m.own_id == own_id);
        self.write(&kept)?;
        for model in gone {
            if let Some(sealed) = &model.sealed_key {
                self.secrets.forget(sealed);
            }
        }
        Ok(())
    }

    /// The provider a run on this route needs, its key opened; None for one no longer kept.
    pub fn provider_for(&self, route_model: &str) -> Result<Option<(String, Provider)>, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        if !is_own_route_model(route_model) {
            return Ok(None);
        }
        let rest = &route_model[OWN_PREFIX.len()..];
        let own_id = rest.split('/').next().unwrap_or("");
        let Some(kept) = self.read()?.into_iter().find(|m| m.own_id == own_id) else { return Ok(None) };
        let api_key = match &kept.sealed_key {
            Some(sealed) => Some(self.secrets.open(sealed).map_err(|_| format!("The key kept for {} could not be opened, so nothing was started.", kept.name))?),
            None => None,
        };
        Ok(Some((format!("{OWN_PREFIX}{}", kept.own_id), Provider { name: kept.name, base_url: kept.base_url, models: vec![kept.model], api_key, tool_calls: !kept.chat_only })))
    }

    /// The models the picker offers under OpenCode, each by its own name, marked as yours.
    pub fn catalog(&self) -> Result<Vec<Value>, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self
            .read()?
            .iter()
            .map(|m| {
                let mut v = json!({"id": own_route_model(&m.own_id, &m.model), "runtime": "opencode", "displayName": m.name,
                    "description": format!("Your model \u{b7} {}{}", host_of(&m.base_url), if m.chat_only { " \u{b7} chat only" } else { "" }),
                    "supportedEfforts": [], "own": true});
                if m.chat_only {
                    v["chatOnly"] = json!(true);
                }
                v
            })
            .collect())
    }
}

/// An OpenCode provider id it can take.
fn provider_id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// OpenCode's config with each provider declared as `@ai-sdk/openai-compatible` (withOpenCodeProviders). The key goes
/// into the config, which goes into the run's environment, never its arguments; an endpoint that asks for no key is
/// sent a placeholder, which the SDK needs and the server ignores. A model that only chats turns every tool off.
pub fn with_opencode_providers(mut config: Value, providers: &[(String, Provider)]) -> Result<Value, String> {
    if providers.is_empty() {
        return Ok(config);
    }
    let mut declared = Map::new();
    for (id, p) in providers {
        if !provider_id_ok(id) {
            return Err(format!("\"{id}\" is not a provider id OpenCode can take"));
        }
        let lower = p.base_url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) || p.base_url.contains(char::is_whitespace) {
            return Err(format!("{}'s address is not an http(s) address", p.name));
        }
        let models: Map<String, Value> = p.models.iter().map(|m| (m.clone(), json!({"name": m}))).collect();
        declared.insert(id.clone(), json!({"npm": "@ai-sdk/openai-compatible", "name": p.name, "options": {"baseURL": p.base_url, "apiKey": p.api_key.clone().unwrap_or_else(|| "not-needed".into())}, "models": models}));
    }
    if !config.is_object() {
        config = json!({});
    }
    let provider = config["provider"].as_object().cloned().unwrap_or_default();
    let mut merged = provider;
    merged.extend(declared);
    config["provider"] = Value::Object(merged);
    if providers.iter().any(|(_, p)| !p.tool_calls) {
        config["tools"] = json!({"*": false});
    }
    Ok(config)
}

/// This Mac's keychain, through Apple's own `security` tool: an item it makes trusts it, so reading one back asks for
/// nothing, and the key travels on its standard input, never in its arguments.
pub struct MacKeychain;

const KEYCHAIN_SERVICE: &str = "Locust Native: your own models";

impl SecretBox for MacKeychain {
    fn available(&self) -> bool {
        cfg!(target_os = "macos") && !cfg!(test)
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        use std::io::Write;
        if !self.available() {
            return Err("No keychain here.".into());
        }
        let account = format!("key-{:x}", crate::invariants::unique_nanos());
        let quoted = format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
        let mut child = std::process::Command::new("/usr/bin/security")
            .arg("-i")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let line = format!("add-generic-password -U -s \"{KEYCHAIN_SERVICE}\" -a \"{account}\" -w {quoted}\n");
        child.stdin.take().ok_or("no input")?.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("The keychain did not keep it.".into());
        }
        Ok(format!("keychain:{account}"))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        if !self.available() {
            return Err("No keychain here.".into());
        }
        let account = sealed.strip_prefix("keychain:").ok_or("Not a keychain item.")?;
        let out = std::process::Command::new("/usr/bin/security")
            .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", account, "-w"])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err("The keychain does not have it.".into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_owned())
    }
    fn forget(&self, sealed: &str) {
        if let (true, Some(account)) = (self.available(), sealed.strip_prefix("keychain:")) {
            let _ = std::process::Command::new("/usr/bin/security").args(["delete-generic-password", "-s", KEYCHAIN_SERVICE, "-a", account]).output();
        }
    }
}

static SECRETS: Mutex<Option<Arc<dyn SecretBox>>> = Mutex::new(None);

/// The box this process keeps keys with: this Mac's keychain, unless a test put its own in.
pub fn secrets() -> Arc<dyn SecretBox> {
    SECRETS.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(|| Arc::new(MacKeychain))
}

/// Whether a stand-in has been put in place of the keychain (the window's tests refuse the real one without it).
pub fn has_stand_in() -> bool {
    SECRETS.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// A box that protects nothing, so keeps no key: what a test gets without a stand-in of its own.
pub struct NoBox;
impl SecretBox for NoBox {
    fn available(&self) -> bool {
        false
    }
    fn seal(&self, _: &str) -> Result<String, String> {
        Err("No box.".into())
    }
    fn open(&self, _: &str) -> Result<String, String> {
        Err("No box.".into())
    }
}

/// Tests keep keys in a stand-in, never the real keychain.
pub fn use_secrets(secrets: Arc<dyn SecretBox>) {
    *SECRETS.lock().unwrap_or_else(|e| e.into_inner()) = Some(secrets);
}

/// The models of your own kept in a native data folder.
pub fn for_store(store: &crate::store::Store) -> OwnModels {
    OwnModels::new(store.root(), secrets())
}

/// A run's OpenCode config with the provider its own model needs, in the command's environment (never its arguments).
pub fn add_provider_env(command: &mut std::process::Command, store: &crate::store::Store, model: &str) -> Result<(), String> {
    let Some(provider) = for_store(store).provider_for(model)? else {
        return Err("That model is no longer in Your own models. Choose another model for this teammate. Nothing was started.".into());
    };
    let current = command
        .get_envs()
        .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
        .and_then(|(_, v)| v)
        .and_then(|v| serde_json::from_str::<Value>(&v.to_string_lossy()).ok())
        .unwrap_or_else(|| json!({}));
    let merged = with_opencode_providers(current, &[provider])?;
    command.env("OPENCODE_CONFIG_CONTENT", merged.to_string());
    Ok(())
}

#[cfg(test)]
#[path = "own_models_tests.rs"]
mod tests;
