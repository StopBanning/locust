//! Let a Cursor run outside Auto call the connectors the person configured, and take the rules back when it ends
//! (Electron's `main/cursor-connector-allow.ts`, `main/cursor-connector-notice.ts`, `runtime-adapters/connectors.ts`).
//!
//! At the start of a Cursor run outside Auto (`workspace-write`), `Mcp(<server>:*)` for each configured server is
//! merged into `<workspace>/.cursor/cli.json`'s `permissions.allow`. `deny` is never touched, and an unreadable file is
//! left alone. When the run ends (completed, failed, cancelled, or never started), `release` removes only the rules THIS
//! run added, and deletes the file (and `.cursor/` if created by us) when nothing else is left.

use crate::invariants::regex;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub const CURSOR_PROJECT_CONFIG: &str = ".cursor/cli.json";

pub fn cursor_allow_rule(server: &str) -> String {
    format!("Mcp({server}:*)")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedCursorConfig {
    /// The file to write, or `None` when nothing needs writing.
    pub text: Option<String>,
    pub added: Vec<String>,
}

/// The file with the missing rules added, or `None` when the file cannot be read as the config it is supposed to be.
/// An absent or whitespace-only string is treated as an empty object.
pub fn merged_cursor_config(text: Option<&str>, servers: &[&str]) -> Option<MergedCursorConfig> {
    let mut record = match text.map(str::trim).filter(|s| !s.is_empty()) {
        None => Map::new(),
        Some(s) => match serde_json::from_str::<Value>(s).ok()? {
            Value::Object(m) => m,
            _ => return None,
        },
    };
    let mut held = match record.get("permissions") {
        None => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return None,
    };
    let mut allow = match held.get("allow") {
        None => Vec::new(),
        Some(Value::Array(a)) => a.clone(),
        Some(_) => return None,
    };
    let mut added = Vec::new();
    for server in servers {
        let rule = cursor_allow_rule(server);
        let already = allow
            .iter()
            .any(|v| v.as_str() == Some(rule.as_str()) || v.as_str() == Some("Mcp(*:*)"));
        if !already {
            allow.push(Value::String(rule.clone()));
            added.push(rule);
        }
    }
    if added.is_empty() {
        return Some(MergedCursorConfig { text: None, added });
    }
    held.insert("allow".into(), Value::Array(allow));
    if !held.contains_key("deny") {
        held.insert("deny".into(), Value::Array(Vec::new()));
    }
    record.insert("permissions".into(), Value::Object(held));
    let mut out = serde_json::to_string_pretty(&Value::Object(record)).ok()?;
    out.push('\n');
    Some(MergedCursorConfig { text: Some(out), added })
}

pub trait AllowIo: Send + Sync {
    fn read_file(&self, path: &Path) -> io::Result<String>;
    fn write_file(&self, path: &Path, text: &str) -> io::Result<()>;
    fn mkdir(&self, path: &Path) -> io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn remove_dir(&self, path: &Path) -> io::Result<()>;
}

pub struct FsAllowIo;

impl AllowIo for FsAllowIo {
    fn read_file(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
    fn write_file(&self, path: &Path, text: &str) -> io::Result<()> {
        std::fs::write(path, text)
    }
    fn mkdir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }
    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir(path)
    }
}

pub trait HoldStore: Send + Sync {
    fn read(&self) -> Option<String>;
    fn write(&self, text: &str) -> io::Result<()>;
}

pub struct FileHoldStore(pub PathBuf);

impl HoldStore for FileHoldStore {
    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.0).ok()
    }
    fn write(&self, text: &str) -> io::Result<()> {
        if let Some(parent) = self.0.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.0, text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Original {
    created_file: bool,
    created_dir: bool,
    had_permissions: bool,
    had_allow: bool,
    had_deny: bool,
}

impl Original {
    fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("createdFile".into(), Value::Bool(self.created_file));
        m.insert("createdDir".into(), Value::Bool(self.created_dir));
        m.insert("hadPermissions".into(), Value::Bool(self.had_permissions));
        m.insert("hadAllow".into(), Value::Bool(self.had_allow));
        m.insert("hadDeny".into(), Value::Bool(self.had_deny));
        Value::Object(m)
    }

    fn from_value(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let flag = |k: &str| obj.get(k).and_then(Value::as_bool) == Some(true);
        Some(Self {
            created_file: flag("createdFile"),
            created_dir: flag("createdDir"),
            had_permissions: flag("hadPermissions"),
            had_allow: flag("hadAllow"),
            had_deny: flag("hadDeny"),
        })
    }
}

#[derive(Clone)]
struct FolderHold {
    original: Original,
    holds: BTreeMap<String, usize>,
}

const HOLD_STORE_VERSION: i64 = 1;

struct StoredHold {
    original: Original,
    rules: Vec<String>,
}

fn parse_stored_holds(text: Option<&str>) -> Option<BTreeMap<PathBuf, StoredHold>> {
    let Some(s) = text.map(str::trim).filter(|s| !s.is_empty()) else {
        return Some(BTreeMap::new());
    };
    let Value::Object(root) = serde_json::from_str::<Value>(s).ok()? else {
        return None;
    };
    if root.get("version").and_then(Value::as_i64) != Some(HOLD_STORE_VERSION) {
        return None;
    }
    let Value::Object(folders_obj) = root.get("folders")? else {
        return None;
    };
    let mut folders = BTreeMap::new();
    for (path, held) in folders_obj {
        let Some(obj) = held.as_object() else { continue };
        let Some(rules_arr) = obj.get("rules").and_then(Value::as_array) else { continue };
        let Some(original) = obj.get("original").and_then(Original::from_value) else { continue };
        let rules: Vec<String> = rules_arr.iter().filter_map(Value::as_str).map(str::to_owned).collect();
        if rules.is_empty() {
            continue;
        }
        folders.insert(PathBuf::from(path), StoredHold { original, rules });
    }
    Some(folders)
}

enum WithoutRules {
    DeleteFile,
    Write(String),
    Unreadable,
}

fn without_rules(text: &str, rules: &[String], original: &Original) -> WithoutRules {
    let Ok(Value::Object(mut record)) = serde_json::from_str::<Value>(text) else {
        return WithoutRules::Unreadable;
    };
    let mut permissions = match record.get("permissions") {
        None => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return WithoutRules::Unreadable,
    };
    match permissions.get("allow") {
        None => {}
        Some(Value::Array(arr)) => {
            let gone: BTreeSet<&str> = rules.iter().map(String::as_str).collect();
            let kept: Vec<Value> = arr
                .iter()
                .filter(|v| v.as_str().is_none_or(|s| !gone.contains(s)))
                .cloned()
                .collect();
            permissions.insert("allow".into(), Value::Array(kept));
        }
        Some(_) => return WithoutRules::Unreadable,
    }
    if permissions.get("allow").and_then(Value::as_array).is_some_and(Vec::is_empty) && !original.had_allow {
        permissions.shift_remove("allow");
    }
    if permissions.get("deny").and_then(Value::as_array).is_some_and(Vec::is_empty) && !original.had_deny {
        permissions.shift_remove("deny");
    }
    if permissions.is_empty() && !original.had_permissions {
        record.shift_remove("permissions");
    } else {
        record.insert("permissions".into(), Value::Object(permissions));
    }
    if record.is_empty() && original.created_file {
        return WithoutRules::DeleteFile;
    }
    let Ok(mut out) = serde_json::to_string_pretty(&Value::Object(record)) else {
        return WithoutRules::Unreadable;
    };
    out.push('\n');
    WithoutRules::Write(out)
}

pub struct CursorConnectorGrant {
    pub added: Vec<String>,
    release_fn: Option<Box<dyn Fn() + Send + Sync>>,
    released: AtomicBool,
}

impl CursorConnectorGrant {
    pub fn none() -> Self {
        Self { added: Vec::new(), release_fn: None, released: AtomicBool::new(true) }
    }

    /// Idempotent. Never panics. Call when the run ends, however it ended.
    pub fn release(&self) {
        if self.released.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(f) = &self.release_fn {
            f();
        }
    }
}

struct KeeperInner {
    io: Arc<dyn AllowIo>,
    store: Option<Arc<dyn HoldStore>>,
    folders: Mutex<BTreeMap<PathBuf, FolderHold>>,
    path_locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl KeeperInner {
    fn lock_for(&self, path: &Path) -> Arc<Mutex<()>> {
        let mut map = self.path_locks.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn remember(&self) {
        let Some(store) = &self.store else { return };
        let folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
        let mut folders_map = Map::new();
        for (path, folder) in folders.iter() {
            let mut entry = Map::new();
            entry.insert("original".into(), folder.original.to_value());
            let rules: Vec<Value> = folder.holds.keys().cloned().map(Value::String).collect();
            entry.insert("rules".into(), Value::Array(rules));
            folders_map.insert(path.to_string_lossy().into_owned(), Value::Object(entry));
        }
        let mut root = Map::new();
        root.insert("version".into(), Value::Number(HOLD_STORE_VERSION.into()));
        root.insert("folders".into(), Value::Object(folders_map));
        if let Ok(mut out) = serde_json::to_string_pretty(&Value::Object(root)) {
            out.push('\n');
            let _ = store.write(&out);
        }
    }

    fn give_back(&self, path: &Path, mine: &[String]) {
        let lock = self.lock_for(path);
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        let (original, last) = {
            let mut folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
            let Some(folder) = folders.get_mut(path) else { return };
            let mut last = Vec::new();
            for rule in mine {
                let count = folder.holds.get(rule).copied().unwrap_or(0).saturating_sub(1);
                if count > 0 {
                    folder.holds.insert(rule.clone(), count);
                } else {
                    folder.holds.remove(rule);
                    last.push(rule.clone());
                }
            }
            let original = folder.original.clone();
            if folder.holds.is_empty() {
                folders.remove(path);
            }
            (original, last)
        };
        self.remember();
        if last.is_empty() {
            return;
        }
        let Ok(text) = self.io.read_file(path) else { return };
        match without_rules(&text, &last, &original) {
            WithoutRules::Unreadable => {}
            WithoutRules::DeleteFile => {
                let _ = self.io.remove_file(path);
                if original.created_dir
                    && let Some(dir) = path.parent()
                {
                    let _ = self.io.remove_dir(dir);
                }
            }
            WithoutRules::Write(next) => {
                if next != text {
                    let _ = self.io.write_file(path, &next);
                }
            }
        }
    }
}

pub struct CursorConnectorKeeper {
    inner: Arc<KeeperInner>,
}

impl CursorConnectorKeeper {
    pub fn new(io: Arc<dyn AllowIo>, store: Option<Arc<dyn HoldStore>>) -> Self {
        Self {
            inner: Arc::new(KeeperInner {
                io,
                store,
                folders: Mutex::new(BTreeMap::new()),
                path_locks: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn allow(&self, workspace: &Path, servers: &[&str]) -> CursorConnectorGrant {
        if servers.is_empty() {
            return CursorConnectorGrant::none();
        }
        let path = workspace.join(CURSOR_PROJECT_CONFIG);
        let lock = self.inner.lock_for(&path);
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        let text = match self.inner.io.read_file(&path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(_) => return CursorConnectorGrant::none(),
        };
        let held_rules: Vec<String> = {
            let folders = self.inner.folders.lock().unwrap_or_else(|e| e.into_inner());
            match folders.get(&path) {
                Some(held) => servers
                    .iter()
                    .map(|s| cursor_allow_rule(s))
                    .filter(|r| held.holds.contains_key(r))
                    .collect(),
                None => Vec::new(),
            }
        };
        let Some(merged) = merged_cursor_config(text.as_deref(), servers) else {
            return CursorConnectorGrant::none();
        };
        if !merged.added.is_empty() {
            let dir = path.parent().unwrap_or(workspace);
            let mut folders = self.inner.folders.lock().unwrap_or_else(|e| e.into_inner());
            if !folders.contains_key(&path) {
                let parsed = text
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .and_then(|s| serde_json::from_str::<Value>(s).ok());
                let perms = parsed.as_ref().and_then(|v| v.get("permissions")).and_then(Value::as_object);
                let original = Original {
                    created_file: text.is_none(),
                    created_dir: text.is_none() && !self.inner.io.exists(dir),
                    had_permissions: perms.is_some(),
                    had_allow: perms.is_some_and(|p| p.contains_key("allow")),
                    had_deny: perms.is_some_and(|p| p.contains_key("deny")),
                };
                folders.insert(path.clone(), FolderHold { original, holds: BTreeMap::new() });
            }
            drop(folders);
            if self.inner.io.mkdir(dir).is_err() {
                return CursorConnectorGrant::none();
            }
            if let Some(next_text) = &merged.text
                && self.inner.io.write_file(&path, next_text).is_err()
            {
                return CursorConnectorGrant::none();
            }
        }
        let mut mine = merged.added.clone();
        mine.extend(held_rules);
        if mine.is_empty() {
            return CursorConnectorGrant::none();
        }
        {
            let mut folders = self.inner.folders.lock().unwrap_or_else(|e| e.into_inner());
            let Some(folder) = folders.get_mut(&path) else {
                return CursorConnectorGrant::none();
            };
            for rule in &mine {
                *folder.holds.entry(rule.clone()).or_insert(0) += 1;
            }
        }
        self.inner.remember();
        let inner = Arc::clone(&self.inner);
        let path_owned = path;
        CursorConnectorGrant {
            added: merged.added,
            release_fn: Some(Box::new(move || inner.give_back(&path_owned, &mine))),
            released: AtomicBool::new(false),
        }
    }

    pub fn recover(&self) -> Vec<PathBuf> {
        let Some(store) = &self.inner.store else { return Vec::new() };
        let Some(stored) = parse_stored_holds(store.read().as_deref()) else {
            return Vec::new();
        };
        let mut changed = Vec::new();
        for (path, held) in &stored {
            let lock = self.inner.lock_for(path);
            let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
            if self.inner.folders.lock().unwrap_or_else(|e| e.into_inner()).contains_key(path) {
                continue;
            }
            let Ok(text) = self.inner.io.read_file(path) else { continue };
            match without_rules(&text, &held.rules, &held.original) {
                WithoutRules::Unreadable => {}
                WithoutRules::DeleteFile => {
                    let _ = self.inner.io.remove_file(path);
                    if held.original.created_dir
                        && let Some(dir) = path.parent()
                    {
                        let _ = self.inner.io.remove_dir(dir);
                    }
                    changed.push(path.clone());
                }
                WithoutRules::Write(next) => {
                    if next != text && self.inner.io.write_file(path, &next).is_ok() {
                        changed.push(path.clone());
                    }
                }
            }
        }
        if !stored.is_empty() {
            self.inner.remember();
        }
        changed
    }

    pub fn holds(&self, workspace: &Path) -> bool {
        let path = workspace.join(CURSOR_PROJECT_CONFIG);
        self.inner
            .folders
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&path)
            .is_some_and(|f| !f.holds.is_empty())
    }
}

pub fn allow_cursor_connectors(
    workspace: &Path,
    servers: &[&str],
    io: Arc<dyn AllowIo>,
) -> CursorConnectorGrant {
    CursorConnectorKeeper::new(io, None).allow(workspace, servers)
}

// --- Cursor connector listing & non-blocking cache (`cursor-connector-notice.ts`, `connectors.ts`) ---

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorConnector {
    pub name: String,
    pub status: String,
    pub needs_authentication: bool,
}

/// `name: status` per line. Anything that is not that shape is not a server.
pub fn parse_cursor_mcp_list(text: &str) -> Vec<CursorConnector> {
    let line_re = regex(r"^\s*([A-Za-z0-9][\w .-]*?)\s*:\s*(\S.*?)\s*$");
    let auth_re = regex(r"(?i)requires?[_\s-]*auth");
    let mut out = Vec::new();
    for line in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let Some(caps) = line_re.captures(line) else { continue };
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let status = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        if name.is_empty() || status.is_empty() {
            continue;
        }
        out.push(CursorConnector {
            name: name.to_owned(),
            status: status.to_owned(),
            needs_authentication: auth_re.is_match(status),
        });
    }
    out
}

pub fn cursor_connector_sentence(connectors: &[CursorConnector]) -> Option<String> {
    let names: Vec<&str> = connectors
        .iter()
        .filter(|c| c.needs_authentication)
        .map(|c| c.name.as_str())
        .collect();
    if names.is_empty() {
        return None;
    }
    let list = if names.len() == 1 {
        names[0].to_owned()
    } else {
        format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1])
    };
    let pronoun = if names.len() == 1 { "it" } else { "them" };
    Some(format!(
        "The Cursor CLI has no credential for {list}, so a Cursor teammate cannot call {pronoun}. Signing in inside the Cursor app does not cover the CLI; they keep separate credentials."
    ))
}

pub fn cursor_ready_connector_line(connectors: &[CursorConnector]) -> Option<String> {
    let ready_re = regex(r"(?i)ready|connected");
    let names: Vec<&str> = connectors
        .iter()
        .filter(|c| !c.needs_authentication && ready_re.is_match(&c.status))
        .map(|c| c.name.as_str())
        .collect();
    if names.is_empty() {
        return None;
    }
    let list = if names.len() == 1 {
        names[0].to_owned()
    } else {
        format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1])
    };
    Some(format!(
        "Connectors you can call on this machine, by name: {list}. Use these exact names -- another server with a similar name may exist and not work."
    ))
}

const TTL_MS: u64 = 5 * 60_000;
const LIST_TIMEOUT_MS: u64 = 8_000;

pub type McpLister = Arc<dyn Fn() -> Option<String> + Send + Sync>;
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

struct CacheState {
    held: Option<(u64, String)>,
    refreshing: bool,
}

pub struct CursorConnectorCache {
    lister: McpLister,
    now: Clock,
    state: Arc<(Mutex<CacheState>, Condvar)>,
}

impl CursorConnectorCache {
    pub fn new(lister: McpLister, now: Clock) -> Self {
        Self {
            lister,
            now,
            state: Arc::new((Mutex::new(CacheState { held: None, refreshing: false }), Condvar::new())),
        }
    }

    fn spawn_refresh(&self) {
        let mut s = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.refreshing {
            return;
        }
        s.refreshing = true;
        drop(s);
        let lister = Arc::clone(&self.lister);
        let now = Arc::clone(&self.now);
        let state = Arc::clone(&self.state);
        std::thread::Builder::new()
            .name("cursor-mcp-list".into())
            .spawn(move || {
                let text = (lister)();
                let ts = (now)();
                let mut s = state.0.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(t) = text {
                    s.held = Some((ts, t));
                }
                s.refreshing = false;
                state.1.notify_all();
            })
            .ok();
    }

    pub fn warm(&self) {
        let s = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.held.is_none() && !s.refreshing {
            drop(s);
            self.spawn_refresh();
        }
    }

    pub fn cached_list(&self) -> Option<String> {
        let s = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, text)) = s.held.clone() {
            let stale = (self.now)().saturating_sub(at) >= TTL_MS;
            let refreshing = s.refreshing;
            drop(s);
            if stale && !refreshing {
                self.spawn_refresh();
            }
            return Some(text);
        }
        let need_spawn = !s.refreshing;
        drop(s);
        if need_spawn {
            self.spawn_refresh();
        }
        let mut s = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        while s.refreshing && s.held.is_none() {
            s = self.state.1.wait(s).unwrap_or_else(|e| e.into_inner());
        }
        s.held.as_ref().map(|(_, t)| t.clone())
    }

    pub fn configured_connector_names(&self) -> Vec<String> {
        let Some(text) = self.cached_list() else { return Vec::new() };
        parse_cursor_mcp_list(&text)
            .into_iter()
            .filter(|c| !c.needs_authentication)
            .map(|c| c.name)
            .collect()
    }

    /// cursor-connector-notice.ts `cursorConnectorsNeedingLogin`: the sentence naming connectors the CLI has no
    /// credential for. A reading that failed says nothing and is not kept, so the next run asks again.
    pub fn connectors_needing_login(&self) -> Option<String> {
        let text = self.cached_list()?;
        cursor_connector_sentence(&parse_cursor_mcp_list(&text))
    }

    pub fn ready_connectors(&self) -> Option<String> {
        let text = self.cached_list()?;
        cursor_ready_connector_line(&parse_cursor_mcp_list(&text))
    }

    pub fn forget(&self) {
        let mut s = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        s.held = None;
        s.refreshing = false;
        self.state.1.notify_all();
    }
}

pub fn run_cursor_mcp_list() -> Option<String> {
    let exe = crate::runtime::discovery::find_named("cursor-agent").ok()?;
    let mut child = Command::new(exe)
        .args(["mcp", "list"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_millis(LIST_TIMEOUT_MS);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut buf = String::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = io::Read::read_to_string(&mut out, &mut buf);
                }
                if !buf.trim().is_empty() || status.success() {
                    return Some(buf);
                }
                return None;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
#[path = "cursor_config_allow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cursor_connectors_tests.rs"]
mod cursor_connectors_tests;
