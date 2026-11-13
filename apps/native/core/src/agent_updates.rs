//! Keeping Codex and Copilot current (Electron `main/runtime-updates.ts`, and the row's words in
//! `renderer/src/agentUpdates.ts`).
//!
//! Colin, 2026-09-23: "is there a way to make it so the models will automatically update without messing up load times
//! or interfering with the app". The model lists are read live from each agent; what goes stale is the agent. Claude
//! Code, OpenCode and Cursor Agent keep themselves current; Codex and Copilot, installed with npm, never do. So, for
//! those two:
//!
//! - **Look, never on the way up.** The first look waits [`FIRST_LOOK_AFTER_MS`] after launch, and a look is due at most
//!   every [`CHECK_EVERY_MS`]: one `npm view` per agent, kept on disk, so a launch inside the window goes to the network
//!   not at all.
//! - **Update only what nothing is using.** No process may run from the agent's own folder. And only a release out for
//!   [`RELEASE_AGE_MS`]: one pulled within hours of going out never lands here.
//! - **Only what npm put there.** An agent found somewhere npm did not install it is left alone.
//! - **A release Locust's own check could not read is held back** (0.501), and installed only when the person asks.
//! - Updating on its own is a switch, on unless the person turned it off; only a choice the person made is kept.
//!
//! Every effect (discovering, npm, the network, the process table, the install, the disk, the clock) is behind
//! [`Effects`], so the rule is tested alone on fakes. The real ones, [`Machine`], refuse to run under `cfg(test)`.
//! Electron's service is async; this one is blocking and runs in a job off the window's thread.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The two agents npm installs and never updates.
pub const KEPT_CURRENT: [&str; 2] = ["codex", "copilot"];
pub const FIRST_LOOK_AFTER_MS: u64 = 45_000;
/// The window ticks this often; a tick goes to the network only when a look is due (Electron: hourly).
pub const TICK_EVERY_MS: u64 = 60 * 60 * 1000;
pub const CHECK_EVERY_MS: i64 = 6 * 60 * 60 * 1000;
pub const RELEASE_AGE_MS: i64 = 12 * 60 * 60 * 1000;
/// Where Locust's release check publishes what it found, beside the installers (0.501).
pub const CANARY_VERDICTS_URL: &str = "https://dl.locust.lol/runtime-canary.json";
/// The file in the native store's own folder (Electron: `runtime-updates.json` in userData).
pub const FILE: &str = "runtime-updates.json";

/// What a row waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// Updating on its own is off: the person presses Update.
    Ask,
    InUse,
    TooNew,
    /// Locust's release check could not read this version.
    Held,
}

impl Why {
    pub fn as_str(self) -> &'static str {
        match self {
            Why::Ask => "ask",
            Why::InUse => "in use",
            Why::TooNew => "too new",
            Why::Held => "held",
        }
    }
}

/// RuntimeUpdateStatus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Current,
    Waiting { version: String, why: Why },
    Updating { version: String },
    Updated { from: String, to: String, at: String },
    Failed { version: String, what: String, at: String },
}

/// What [`decide`] says: a status to show, or an update to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decided {
    Say(Status),
    Update(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// When npm says it went out; None when it did not say.
    pub published_at: Option<String>,
}

/// What is kept on disk between launches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    /// Updates without being asked. On unless the person turned it off.
    pub automatic: bool,
    /// Whether `automatic` is the person's own choice rather than the default.
    pub chosen: bool,
    pub checked_at: Option<i64>,
    /// The newest release seen, by package.
    pub latest: BTreeMap<String, Release>,
    /// What last happened to each agent that is worth saying again: an update, or a failed one.
    pub last: BTreeMap<String, Status>,
    /// Versions held back, by package (0.501); None when the check could not be reached.
    pub held: Option<BTreeMap<String, Vec<String>>>,
}

pub fn nothing_saved() -> Saved {
    Saved { automatic: true, chosen: false, checked_at: None, latest: BTreeMap::new(), last: BTreeMap::new(), held: None }
}

impl Saved {
    /// The file's shape, Electron's field for field.
    pub fn to_json(&self) -> Value {
        let latest: Map<String, Value> = self
            .latest
            .iter()
            .map(|(name, r)| {
                let mut o = Map::new();
                o.insert("version".into(), json!(r.version));
                if let Some(p) = &r.published_at {
                    o.insert("publishedAt".into(), json!(p));
                }
                (name.clone(), Value::Object(o))
            })
            .collect();
        let last: Map<String, Value> = self
            .last
            .iter()
            .filter_map(|(runtime, s)| {
                let v = match s {
                    Status::Updated { from, to, at } => json!({ "kind": "updated", "from": from, "to": to, "at": at }),
                    Status::Failed { version, what, at } => json!({ "kind": "failed", "version": version, "what": what, "at": at }),
                    _ => return None,
                };
                Some((runtime.clone(), v))
            })
            .collect();
        let mut o = Map::new();
        if let Some(h) = &self.held {
            o.insert("held".into(), json!(h));
        }
        o.insert("automatic".into(), json!(self.automatic));
        o.insert("chosen".into(), json!(self.chosen));
        if let Some(c) = self.checked_at {
            o.insert("checkedAt".into(), json!(c));
        }
        o.insert("latest".into(), Value::Object(latest));
        o.insert("last".into(), Value::Object(last));
        Value::Object(o)
    }
}

/// What a saved file holds, checked field by field: anything else is nothing saved (savedUpdatesFrom).
pub fn saved_updates_from(value: &Value) -> Option<Saved> {
    let empty = Map::new();
    let record = match value {
        Value::Object(m) => m,
        // JavaScript's typeof calls an array an object: its fields are simply absent.
        Value::Array(_) => &empty,
        _ => return None,
    };
    let mut latest = BTreeMap::new();
    if let Some(Value::Object(m)) = record.get("latest") {
        for (name, release) in m {
            let Value::Object(r) = release else { continue };
            if let Some(Value::String(version)) = r.get("version") {
                latest.insert(name.clone(), Release { version: version.clone(), published_at: r.get("publishedAt").and_then(Value::as_str).map(str::to_owned) });
            }
        }
    }
    let mut last = BTreeMap::new();
    if let Some(Value::Object(m)) = record.get("last") {
        for (runtime, status) in m {
            let Value::Object(e) = status else { continue };
            let text = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_owned);
            match e.get("kind").and_then(Value::as_str) {
                Some("updated") => {
                    if let (Some(from), Some(to), Some(at)) = (text("from"), text("to"), text("at")) {
                        last.insert(runtime.clone(), Status::Updated { from, to, at });
                    }
                }
                Some("failed") => {
                    if let (Some(version), Some(what), Some(at)) = (text("version"), text("what"), text("at")) {
                        last.insert(runtime.clone(), Status::Failed { version, what, at });
                    }
                }
                _ => {}
            }
        }
    }
    // What the person chose, when they chose; the default otherwise -- 0.303 wrote `automatic: false` for everyone, and
    // 0.302 its own `enabled`.
    let chosen = record.get("chosen") == Some(&Value::Bool(true)) && record.get("automatic").is_some_and(Value::is_boolean);
    let mut held = BTreeMap::new();
    if let Some(Value::Object(m)) = record.get("held") {
        for (name, versions) in m {
            if let Value::Array(a) = versions {
                held.insert(name.clone(), a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect::<Vec<_>>());
            }
        }
    }
    Some(Saved {
        held: if held.is_empty() { None } else { Some(held) },
        automatic: if chosen { record.get("automatic") == Some(&Value::Bool(true)) } else { nothing_saved().automatic },
        chosen,
        checked_at: record.get("checkedAt").and_then(Value::as_f64).filter(|n| n.is_finite()).map(|n| n as i64),
        latest,
        last,
    })
}

/// Whether this launch may change the machine's agents at all (mayUpdateAgents). A script drives the app with
/// `--remote-debugging-port`, and a drive that updated the Codex every other session on this machine would be a test
/// with a side effect no one asked for. `LOCUST_UPDATE_AGENTS=1` lets a drive that is about updating do it. A copy that
/// is not the installed Locust (`installed` false) changes nothing on the machine either (0.514).
pub fn may_update_agents(argv: &[String], environment: &dyn Fn(&str) -> Option<String>, installed: bool) -> bool {
    if environment("LOCUST_UPDATE_AGENTS").as_deref() == Some("1") {
        return true;
    }
    installed && !argv.iter().any(|a| a.starts_with("--remote-debugging-port"))
}

fn parse_leading_int(part: &str) -> i64 {
    // Number.parseInt(part, 10) || 0
    let digits: String = part.trim_start().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(0)
}

/// Newer, older or the same: dotted numbers compared as numbers, and a prerelease below its release
/// (`0.158.0-alpha.4` < `0.158.0`).
pub fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering::{Equal, Greater, Less};
    fn split(version: &str) -> (Vec<i64>, Option<String>) {
        let v = version.trim();
        let v = v.strip_prefix(['v', 'V']).unwrap_or(v);
        let mut it = v.split('-');
        let core = it.next().unwrap_or("");
        let rest: Vec<&str> = it.collect();
        (core.split('.').map(parse_leading_int).collect(), if rest.is_empty() { None } else { Some(rest.join("-")) })
    }
    let (a, b) = (split(left), split(right));
    for i in 0..a.0.len().max(b.0.len()) {
        let d = a.0.get(i).copied().unwrap_or(0) - b.0.get(i).copied().unwrap_or(0);
        if d != 0 {
            return d.cmp(&0);
        }
    }
    let (ap, bp) = match (&a.1, &b.1) {
        (None, None) => return Equal,
        (None, Some(_)) => return Greater,
        (Some(_), None) => return Less,
        (Some(x), Some(y)) if x == y => return Equal,
        (Some(x), Some(y)) => (x.split('.').collect::<Vec<_>>(), y.split('.').collect::<Vec<_>>()),
    };
    // Prerelease identifiers one by one, numbers as numbers: alpha.10 is after alpha.9 (2026-10-10 sweep).
    for i in 0..ap.len().max(bp.len()) {
        let (Some(x), Some(y)) = (ap.get(i), bp.get(i)) else {
            return if ap.get(i).is_none() { Less } else { Greater };
        };
        let (xn, yn) = (!x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()), !y.is_empty() && y.bytes().all(|c| c.is_ascii_digit()));
        if xn && yn {
            let (xv, yv) = (x.parse::<f64>().unwrap_or(0.0), y.parse::<f64>().unwrap_or(0.0));
            if xv != yv {
                return if xv < yv { Less } else { Greater };
            }
        }
        if xn != yn {
            return if xn { Less } else { Greater };
        }
        if !xn && x != y {
            return if x < y { Less } else { Greater };
        }
    }
    Equal
}

/// Date.parse for what npm sends (an ISO stamp), in milliseconds.
fn parse_ms(stamp: &str) -> Option<i64> {
    use chrono::{DateTime, NaiveDate, NaiveDateTime};
    if let Ok(t) = DateTime::parse_from_rfc3339(stamp) {
        return Some(t.timestamp_millis());
    }
    if let Ok(t) = NaiveDateTime::parse_from_str(stamp, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(t.and_utc().timestamp_millis());
    }
    NaiveDate::parse_from_str(stamp, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)).map(|t| t.and_utc().timestamp_millis())
}

pub struct DecideInput<'a> {
    pub installed: Option<&'a str>,
    pub latest: Option<&'a Release>,
    pub automatic: bool,
    pub in_use: bool,
    pub now: i64,
    /// Locust's release check could not read this version (0.501).
    pub held: bool,
}

/// What to do about one agent, from what is known: the whole rule, alone.
pub fn decide(input: &DecideInput) -> Decided {
    let (Some(installed), Some(latest)) = (input.installed, input.latest) else { return Decided::Say(Status::Current) };
    if compare_versions(&latest.version, installed) != std::cmp::Ordering::Greater {
        return Decided::Say(Status::Current);
    }
    let waiting = |why| Decided::Say(Status::Waiting { version: latest.version.clone(), why });
    // Not automatic: it waits for the person to press Update.
    if !input.automatic {
        return waiting(Why::Ask);
    }
    // A release Locust could not read is never installed on its own, however old.
    if input.held {
        return waiting(Why::Held);
    }
    // A release npm gives no date for is treated as brand new: it waits.
    match latest.published_at.as_deref().and_then(parse_ms) {
        Some(published) if input.now - published >= RELEASE_AGE_MS => {}
        _ => return waiting(Why::TooNew),
    }
    if input.in_use {
        return waiting(Why::InUse);
    }
    Decided::Update(latest.version.clone())
}

/// One agent as discovery found it, with what the folder rule needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Discovered {
    pub id: String,
    pub version: Option<String>,
    pub executable: Option<Executable>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Executable {
    pub discovered_path: String,
    pub executable_path: String,
    pub prefix_args: Vec<String>,
}

fn dirname(path: &str) -> String {
    let at = if cfg!(windows) { path.rfind(['/', '\\']) } else { path.rfind('/') };
    match at {
        Some(0) => "/".into(),
        Some(i) => path[..i].to_owned(),
        None => ".".into(),
    }
}

fn normal(path: &str) -> String {
    let mut out = String::new();
    let mut slash = false;
    for c in path.to_lowercase().chars() {
        if c == '/' || c == '\\' {
            if !slash {
                out.push('/');
            }
            slash = true;
        } else {
            out.push(c);
            slash = false;
        }
    }
    out
}

/// Where npm keeps an agent it installed, when it did: the package's own folder.
///
/// And on a Mac or Linux (the first Mac run, 2026-10-30): npm's shim there is a link in `<prefix>/bin` to the package's
/// own script, with `npm root -g` at `<prefix>/lib/node_modules`, so the Windows test -- a shim beside node_modules --
/// never matched, and codex from npm was never updated. The link is followed instead. Homebrew's own codex sits in the
/// same bin, but its link leads into Homebrew's Cellar, not npm's folder, so it is left alone.
pub fn npm_package_dir(discovery: &Discovered, npm_root: &str, real_path: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let package = crate::agents::install_package(&discovery.id)?;
    let executable = discovery.executable.as_ref()?;
    // Joined as npm_root is spelled, so a Mac's root read on any machine stays a Mac's.
    let sep = if (npm_root.contains('/') && !npm_root.contains('\\')) || !cfg!(windows) { "/" } else { "\\" };
    let folder = std::iter::once(npm_root).chain(package.split('/')).collect::<Vec<_>>().join(sep);
    let prefix = dirname(npm_root);
    let inside = |path: &str| normal(path).starts_with(&format!("{}/", normal(&folder)));
    // npm's own shim in its own prefix, a path inside the package itself, or a link that leads into it.
    let npm_put = normal(&dirname(&executable.discovered_path)) == normal(&prefix)
        || std::iter::once(&executable.executable_path).chain(executable.prefix_args.iter()).any(|p| inside(p))
        || inside(&real_path(&executable.discovered_path).unwrap_or_default());
    npm_put.then_some(folder)
}

/// The versions Locust's release check ran and could not read, by package (heldFrom): a few kilobytes, asked with each
/// look.
pub fn held_from(value: &Value) -> BTreeMap<String, Vec<String>> {
    let mut held = BTreeMap::new();
    let Some(Value::Object(packages)) = value.get("packages") else { return held };
    for (name, versions) in packages {
        let Value::Object(versions) = versions else { continue };
        let failed: Vec<String> = versions.iter().filter(|(_, v)| v.get("ok") == Some(&Value::Bool(false))).map(|(version, _)| version.clone()).collect();
        if !failed.is_empty() {
            held.insert(name.clone(), failed);
        }
    }
    held
}

/// npm's answer to `npm view <package> dist-tags.latest time --json`, as a release (npmRelease).
pub fn release_from(text: &str) -> Option<Release> {
    let parsed: Value = serde_json::from_str(text).ok()?;
    let version = parsed.get("dist-tags.latest")?.as_str().filter(|v| !v.is_empty())?;
    let published_at = parsed.get("time").and_then(|t| t.get(version)).and_then(Value::as_str).map(str::to_owned);
    Some(Release { version: version.to_owned(), published_at })
}

/// Whether the process table (`ps -eo command`, as bytes) shows anything running from inside `dir`: its executable
/// there, or -- for an agent that is a node script -- that script on its command line. A case-insensitive substring,
/// of whatever the path holds: read as UTF-8, so a folder with a non-ASCII letter matches (Electron's L13).
pub fn processes_using_bytes(dir: &str, process_table: &[u8]) -> bool {
    String::from_utf8_lossy(process_table).to_lowercase().contains(&dir.to_lowercase())
}

/// One view of one agent's row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub runtime: String,
    pub installed: String,
    pub latest: Option<String>,
    pub status: Status,
}

/// RuntimeUpdatesState.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct State {
    pub automatic: bool,
    /// When npm was last asked (ISO), None before the first look.
    pub checked_at: Option<String>,
    pub agents: Vec<View>,
}

impl State {
    pub fn view(&self, runtime: &str) -> Option<&View> {
        self.agents.iter().find(|v| v.runtime == runtime)
    }
}

/// Everything the service does to the world, so none of it is done in a test.
pub trait Effects: Send + Sync {
    fn discover(&self) -> Vec<Discovered>;
    /// npm's global folder (`npm root -g`); None when there is no npm to ask.
    fn npm_root(&self) -> Option<String>;
    /// Where a link really points, or None when it cannot be read.
    fn real_path(&self, path: &str) -> Option<String>;
    fn latest(&self, package: &str) -> Option<Release>;
    /// The versions Locust's release check could not read, by package; None when it could not be asked, which holds
    /// nothing back.
    fn held_versions(&self) -> Option<BTreeMap<String, Vec<String>>>;
    /// Whether any process on this machine is running from inside `dir`; Err when it could not be read (read as in use).
    fn in_use(&self, dir: &str) -> Result<bool, String>;
    fn install(&self, runtime: &str, version: &str) -> Result<(), String>;
    fn load(&self) -> Option<Saved>;
    fn save(&self, saved: &Saved);
    /// An agent was updated: ask the machine again, re-read the models.
    fn updated(&self, runtime: &str);
    /// Anything a window shows changed.
    fn changed(&self);
    fn now(&self) -> i64;
}

struct Agent {
    id: String,
    installed: String,
    dir: String,
    package: String,
}

fn iso(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Updates {
    fx: Box<dyn Effects>,
    saved: Mutex<Option<Saved>>,
    current: Mutex<Option<String>>,
    views: Mutex<Vec<View>>,
    /// One lock for a tick and for a manual Update (L12).
    ticking: Mutex<()>,
    updated: AtomicBool,
}

impl Updates {
    pub fn new(fx: Box<dyn Effects>) -> Self {
        Updates { fx, saved: Mutex::new(None), current: Mutex::new(None), views: Mutex::new(vec![]), ticking: Mutex::new(()), updated: AtomicBool::new(false) }
    }

    fn saved_now(&self) -> Saved {
        if lock(&self.saved).is_none() {
            let loaded = self.fx.load().unwrap_or_else(nothing_saved);
            lock(&self.saved).get_or_insert(loaded);
        }
        lock(&self.saved).clone().unwrap_or_else(nothing_saved)
    }

    fn keep(&self, next: Saved) {
        *lock(&self.saved) = Some(next.clone());
        self.fx.save(&next);
    }

    /// What is known, from memory alone: safe to ask while a job runs, and from a window's poll.
    pub fn peek(&self) -> State {
        let saved = lock(&self.saved).clone().unwrap_or_else(nothing_saved);
        State { automatic: saved.automatic, checked_at: saved.checked_at.map(iso), agents: lock(&self.views).clone() }
    }

    pub fn state(&self) -> State {
        self.saved_now();
        self.peek()
    }

    /// Whether an agent was updated since this was last asked: the window then asks discovery again.
    pub fn take_updated(&self) -> bool {
        self.updated.swap(false, Ordering::SeqCst)
    }

    /// The agent being updated right now, if one is: a mission must not start on it.
    pub fn updating(&self) -> Option<String> {
        lock(&self.current).clone()
    }

    fn agents_now(&self) -> Option<Vec<Agent>> {
        let runtimes = self.fx.discover();
        let root = self.fx.npm_root()?;
        let real = |p: &str| self.fx.real_path(p);
        Some(
            KEPT_CURRENT
                .iter()
                .filter_map(|id| {
                    let d = runtimes.iter().find(|r| r.id == *id)?;
                    let dir = npm_package_dir(d, &root, &real)?;
                    Some(Agent { id: (*id).to_owned(), installed: d.version.clone()?, dir, package: crate::agents::install_package(id)? })
                })
                .collect(),
        )
    }

    /// One update, reported as it starts and as it ends; the view it leaves.
    fn update_one(&self, agent: &Agent, version: &str, others: &[View]) -> View {
        *lock(&self.current) = Some(agent.id.clone());
        *lock(&self.views) = others.iter().cloned().chain([View { runtime: agent.id.clone(), installed: agent.installed.clone(), latest: Some(version.to_owned()), status: Status::Updating { version: version.to_owned() } }]).collect();
        self.fx.changed();
        let outcome = self.fx.install(&agent.id, version);
        *lock(&self.current) = None;
        let at = iso(self.fx.now());
        let ok = outcome.is_ok();
        let status = match outcome {
            Ok(()) => Status::Updated { from: agent.installed.clone(), to: version.to_owned(), at },
            Err(what) => Status::Failed { version: version.to_owned(), what, at },
        };
        let mut state = self.saved_now();
        state.last.insert(agent.id.clone(), status.clone());
        self.keep(state);
        if ok {
            self.updated.store(true, Ordering::SeqCst);
            self.fx.updated(&agent.id);
        }
        View { runtime: agent.id.clone(), installed: if ok { version.to_owned() } else { agent.installed.clone() }, latest: Some(version.to_owned()), status }
    }

    fn run(&self) {
        let mut state = self.saved_now();
        let Some(agents) = self.agents_now() else { return };
        // Look, if due.
        if state.checked_at.is_none_or(|at| self.fx.now() - at >= CHECK_EVERY_MS) {
            let mut latest = state.latest.clone();
            for agent in &agents {
                if let Some(release) = self.fx.latest(&agent.package) {
                    latest.insert(agent.package.clone(), release);
                }
            }
            let held = self.fx.held_versions();
            state = Saved { checked_at: Some(self.fx.now()), latest, held: held.or(state.held.clone()), ..state };
            self.keep(state.clone());
        }
        let mut next: Vec<View> = vec![];
        for agent in &agents {
            let release = state.latest.get(&agent.package);
            let ask = |in_use: bool| {
                decide(&DecideInput {
                    installed: Some(&agent.installed),
                    latest: release,
                    automatic: state.automatic,
                    in_use,
                    now: self.fx.now(),
                    held: release.is_some_and(|r| state.held.as_ref().and_then(|h| h.get(&agent.package)).is_some_and(|v| v.contains(&r.version))),
                })
            };
            // The machine's process table is read only when it is the last question: an update would go ahead if
            // nothing were using the agent.
            let provisional = ask(false);
            let verdict = if matches!(provisional, Decided::Update(_)) { ask(self.fx.in_use(&agent.dir).unwrap_or(true)) } else { provisional };
            match verdict {
                Decided::Update(version) => {
                    let view = self.update_one(agent, &version, &next);
                    next.push(view);
                    state = self.saved_now();
                }
                Decided::Say(status) => {
                    // A past update is still worth saying until something newer is out.
                    let status = match (&status, state.last.get(&agent.id)) {
                        (Status::Current, Some(last)) => last.clone(),
                        _ => status,
                    };
                    next.push(View { runtime: agent.id.clone(), installed: agent.installed.clone(), latest: release.map(|r| r.version.clone()), status });
                }
            }
        }
        *lock(&self.views) = next;
        self.fx.changed();
    }

    /// Look, if a look is due; then update whatever is behind and not in use. A tick that comes while another runs
    /// waits for it and does nothing more.
    pub fn tick(&self) {
        match self.ticking.try_lock() {
            Ok(_g) => self.run(),
            Err(std::sync::TryLockError::Poisoned(e)) => {
                let _g = e.into_inner();
                self.run()
            }
            Err(std::sync::TryLockError::WouldBlock) => drop(lock(&self.ticking)),
        }
    }

    /// The person turned the switch. Turned on, whatever was waiting on it goes now, not at the next look.
    pub fn set_automatic(&self, automatic: bool) -> State {
        let state = self.saved_now();
        self.keep(Saved { automatic, chosen: true, ..state });
        if automatic {
            self.tick();
        }
        self.state()
    }

    /// The person pressed Update: this agent, now, unless something is using it. One lock for both (L12): this waits for
    /// a tick, then holds the lock itself, so a tick meanwhile waits for it.
    pub fn update_now(&self, runtime: &str) -> State {
        let _g = lock(&self.ticking);
        self.press_update(runtime)
    }

    fn press_update(&self, runtime: &str) -> State {
        let agent = self.agents_now().and_then(|a| a.into_iter().find(|a| a.id == runtime));
        let state = self.saved_now();
        let release = agent.as_ref().and_then(|a| state.latest.get(&a.package)).cloned();
        let (Some(agent), Some(release)) = (agent, release) else { return self.state() };
        if compare_versions(&release.version, &agent.installed) != std::cmp::Ordering::Greater || lock(&self.current).is_some() {
            return self.state();
        }
        let others: Vec<View> = lock(&self.views).iter().filter(|v| v.runtime != runtime).cloned().collect();
        let with = |view: View| others.iter().cloned().chain([view]).collect::<Vec<_>>();
        if self.fx.in_use(&agent.dir).unwrap_or(true) {
            *lock(&self.views) = with(View { runtime: runtime.to_owned(), installed: agent.installed.clone(), latest: Some(release.version.clone()), status: Status::Waiting { version: release.version.clone(), why: Why::InUse } });
            self.fx.changed();
            return self.state();
        }
        let view = self.update_one(&agent, &release.version, &others);
        *lock(&self.views) = with(view);
        self.fx.changed();
        self.state()
    }
}

// ---- the real effects ----

/// One command, no shell, its output, and a time limit. Refuses under test.
fn capture(command: &str, args: &[&str], limit: std::time::Duration) -> Result<Vec<u8>, String> {
    use std::io::Read;
    if cfg!(test) {
        return Err("no process runs in a test".into());
    }
    let mut child = std::process::Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("{command} could not be started: {e}"))?;
    let mut out = child.stdout.take().ok_or("no output")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = (&mut out).take(4_000_000).read_to_end(&mut bytes);
        let _ = tx.send(bytes);
    });
    match rx.recv_timeout(limit) {
        Ok(bytes) => match child.wait() {
            Ok(status) if status.success() => Ok(bytes),
            Ok(status) => Err(format!("{command} exited {}", status.code().map_or("?".into(), |c| c.to_string()))),
            Err(e) => Err(e.to_string()),
        },
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(format!("{command} took longer than {} s", limit.as_secs()))
        }
    }
}

/// This machine: discovery, npm, the network, the process table, the installer, one file, the clock.
pub struct Machine {
    file: std::path::PathBuf,
}

impl Machine {
    /// `root` is the native store's own folder.
    pub fn new(root: &std::path::Path) -> Self {
        Machine { file: root.join(FILE) }
    }
}

impl Effects for Machine {
    fn discover(&self) -> Vec<Discovered> {
        if cfg!(test) {
            return vec![];
        }
        crate::agents::discover()
            .into_iter()
            .filter(|f| KEPT_CURRENT.contains(&f.id.as_str()))
            .map(|f| {
                let path = crate::agents::command_of(&f.id).and_then(|c| crate::runtime::discovery::find_named(c).ok()).map(|p| p.to_string_lossy().into_owned());
                let version = f.version.as_deref().and_then(|v| crate::invariants::regex(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?").find(v).map(|m| m.as_str().to_owned()));
                Discovered { id: f.id, version, executable: path.map(|p| Executable { discovered_path: p.clone(), executable_path: p, prefix_args: vec![] }) }
            })
            .collect()
    }

    fn npm_root(&self) -> Option<String> {
        let text = capture("npm", &["root", "-g"], std::time::Duration::from_secs(30)).ok()?;
        String::from_utf8_lossy(&text).trim().lines().next_back().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty())
    }

    fn real_path(&self, path: &str) -> Option<String> {
        std::fs::canonicalize(path).ok().map(|p| p.to_string_lossy().into_owned())
    }

    fn latest(&self, package: &str) -> Option<Release> {
        let text = capture("npm", &["view", package, "dist-tags.latest", "time", "--json"], std::time::Duration::from_secs(60)).ok()?;
        release_from(&String::from_utf8_lossy(&text))
    }

    fn held_versions(&self) -> Option<BTreeMap<String, Vec<String>>> {
        // curl with the one address and no credential: nothing about this person is sent.
        let curl = if std::path::Path::new("/usr/bin/curl").exists() { "/usr/bin/curl" } else { "curl" };
        let text = capture(curl, &["--silent", "--fail", "--max-time", "10", CANARY_VERDICTS_URL], std::time::Duration::from_secs(12)).ok()?;
        Some(held_from(&serde_json::from_slice(&text).ok()?))
    }

    fn in_use(&self, dir: &str) -> Result<bool, String> {
        let table = capture("ps", &["-eo", "command"], std::time::Duration::from_secs(20))?;
        Ok(processes_using_bytes(dir, &table))
    }

    fn install(&self, runtime: &str, version: &str) -> Result<(), String> {
        if cfg!(test) {
            return Err("no install runs in a test".into());
        }
        let name = crate::agents::known().find(|(id, _)| *id == runtime).map_or(runtime, |(_, n)| n);
        crate::agent_install::for_this_machine().install(runtime, name, Some(version), &mut |_| {}).map_err(|f| f.what)
    }

    fn load(&self) -> Option<Saved> {
        saved_updates_from(&serde_json::from_slice(&std::fs::read(&self.file).ok()?).ok()?)
    }

    fn save(&self, saved: &Saved) {
        let _ = std::fs::write(&self.file, saved.to_json().to_string());
    }

    fn updated(&self, _runtime: &str) {}

    fn changed(&self) {}

    fn now(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}

// ---- the row's words (renderer/src/agentUpdates.ts) ----

/// "at 4:42 PM" today, "on Sep 22" before.
fn when(at: &str, now: chrono::DateTime<chrono::Local>) -> String {
    use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
    let then: Option<DateTime<Local>> = DateTime::parse_from_rfc3339(at)
        .map(|t| t.with_timezone(&Local))
        .ok()
        .or_else(|| NaiveDateTime::parse_from_str(at, "%Y-%m-%dT%H:%M:%S%.f").ok().and_then(|n| Local.from_local_datetime(&n).single()));
    let Some(then) = then else { return String::new() };
    if then.date_naive() == now.date_naive() { format!("at {}", then.format("%-I:%M %p")) } else { format!("on {}", then.format("%b %-d")) }
}

/// What a runtime's row says about keeping it current: one line, and none at all when it is current (updateLine).
pub fn update_line(view: Option<&View>, now: chrono::DateTime<chrono::Local>) -> Option<String> {
    Some(match &view?.status {
        Status::Current => return None,
        Status::Waiting { version, why } => match why {
            Why::Ask => format!("{version} is out."),
            Why::InUse => format!("{version} is out. It can update once nothing is using it."),
            Why::Held => format!("{version} is out, and held back: Locust's check of new versions could not read it yet. Update to try it anyway."),
            Why::TooNew => format!("{version} is out. It updates once it has been out 12 hours."),
        },
        Status::Updating { version } => format!("Updating to {version}\u{2026}"),
        Status::Updated { from, to, at } => format!("Updated from {from} to {to} {}.", when(at, now)),
        Status::Failed { version, what, at } => format!("Could not update to {version} {}: {what}", when(at, now)),
    })
}

/// Whether a row offers Update: a newer version is out and waiting on the person, or on nothing using it.
pub fn offers_update(view: Option<&View>) -> bool {
    view.is_some_and(|v| matches!(&v.status, Status::Waiting { why, .. } if *why != Why::TooNew))
}

/// The switch's own sentence, saying what an update costs either way.
pub fn keep_current_note(automatic: bool) -> &'static str {
    if automatic {
        "Updating on their own. A newer Codex CLI or Copilot CLI is downloaded when it is out and nothing is using it \u{2014} a big download, which slows the connection while it runs."
    } else {
        "Updating when you press Update. A newer Codex CLI or Copilot CLI is shown on its row, and the download \u{2014} Codex CLI's is about 160 MB \u{2014} starts only when you ask."
    }
}

/// Said instead of the switch in a copy that is not the installed Locust (0.514).
pub const HELD_HERE_NOTE: &str = "This copy is not the installed Locust, so it never updates the agents on this machine on its own. The installed Locust keeps them current.";

#[cfg(test)]
#[path = "agent_updates_tests.rs"]
mod tests;
