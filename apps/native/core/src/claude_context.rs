//! Claude Code's own context estimate for the context ring's menu (Electron `main/claude-context.ts` and
//! `shared/context-breakdown.ts`, 0.806).
//!
//! The ring's menu shows what Claude Code's `/context` says fills the conversation, run with the turn's own flags after
//! the turn ends: free (0 turns, no model call), never while a turn runs, once per finished turn. MEASURED 2026-10-19 on
//! Claude Code 2.1.303: `/context` also lists "(deferred)" tools and the autocompact buffer, which are not in the window
//! -- with connectors on, "MCP tools (deferred)" read 200.7k while the whole conversation was 24k. Those never become a
//! bar or a sentence.
//!
//! This is the engine and the reader. Wiring it into a running window (remembering each turn's launch, building the
//! `/context` command from it, and drawing the rows under the ring) is not here yet.

use serde_json::{Map, Value, json};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// One row of `/context`'s category table: an estimate from Claude Code itself, never one of our own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCategory {
    pub name: String,
    pub tokens: u64,
}

impl ContextCategory {
    pub fn new(name: &str, tokens: u64) -> Self {
        Self { name: name.to_owned(), tokens }
    }
}

/// `ClaudeContextBreakdown`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeContextBreakdown {
    pub categories: Vec<ContextCategory>,
    pub used: u64,
    pub window: u64,
}

/// `contextTokens`: 24.5k, 1.2m, 400.
pub fn context_tokens(tokens: u64) -> String {
    let (scale, unit) = if tokens >= 1_000_000 {
        (1_000_000.0, "m")
    } else if tokens >= 1_000 {
        (1_000.0, "k")
    } else {
        (1.0, "")
    };
    let tenths = (tokens as f64 / scale * 10.0).round() / 10.0;
    let figure = if tenths.fract() == 0.0 { format!("{}", tenths as u64) } else { format!("{tenths}") };
    format!("{figure}{unit}")
}

/// `inContext`: deferred tools load only when used, and the autocompact buffer is space held back.
pub fn in_context(row: &ContextCategory) -> bool {
    let name = row.name.to_lowercase();
    !name.trim_end().ends_with("(deferred)") && name != "autocompact buffer"
}

/// `contextCategories`: the four biggest rows in the window, then free space, which is useful even when small.
pub fn context_categories(breakdown: &ClaudeContextBreakdown) -> Vec<ContextCategory> {
    let free = breakdown.categories.iter().position(|row| row.name.to_lowercase() == "free space");
    let mut largest: Vec<ContextCategory> = breakdown
        .categories
        .iter()
        .enumerate()
        .filter(|(i, row)| Some(*i) != free && in_context(row))
        .map(|(_, row)| row.clone())
        .collect();
    // Stable, like Array.prototype.sort: equal rows keep their order.
    largest.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    largest.truncate(4);
    if let Some(i) = free {
        largest.push(breakdown.categories[i].clone());
    }
    largest
}

/// `CONTEXT_ADVICE_SHARE`: below this share of the window a category is not worth a sentence.
pub const CONTEXT_ADVICE_SHARE: f64 = 0.1;

/// `contextAdvice`: point only at the largest category the person can change, once it matters.
pub fn context_advice(breakdown: &ClaudeContextBreakdown) -> Option<String> {
    const CONTROLLED: [&str; 5] = ["connectors", "messages", "skills", "memory files", "custom agents"];
    let mut controlled: Vec<(String, u64)> = Vec::new();
    for row in breakdown.categories.iter().filter(|row| in_context(row)) {
        let name = row.name.to_lowercase();
        let key = if name.starts_with("mcp ") { "connectors".to_owned() } else { name };
        if !CONTROLLED.contains(&key.as_str()) {
            continue;
        }
        match controlled.iter_mut().find(|(k, _)| *k == key) {
            Some((_, tokens)) => *tokens += row.tokens,
            None => controlled.push((key, row.tokens)),
        }
    }
    let threshold = breakdown.window as f64 * CONTEXT_ADVICE_SHARE;
    let mut big: Vec<(String, u64)> = controlled.into_iter().filter(|(_, tokens)| *tokens as f64 >= threshold).collect();
    big.sort_by(|a, b| b.1.cmp(&a.1));
    let (name, tokens) = big.into_iter().next()?;
    let amount = context_tokens(tokens);
    Some(match name.as_str() {
        "connectors" => format!("Your connectors take {amount}: untick ones this teammate does not need in Edit teammate."),
        "messages" => format!("Messages take {amount}. Compact this conversation to make room while keeping a summary."),
        "skills" => format!("Skills take {amount}. Review the project's skills and your Skills library for ones this teammate does not need."),
        "memory files" => format!("Memory files take {amount}. Shorten project instructions that repeat what is already in the conversation."),
        _ => format!("Custom agents take {amount}. Keep only the agent definitions this teammate needs."),
    })
}

fn token_count(value: &str) -> Option<u64> {
    let re = regex::Regex::new(r"(?i)^(\d[\d,]*(?:\.\d+)?)([km])?$").unwrap();
    let caps = re.captures(value.trim())?;
    let number: f64 = caps[1].replace(',', "").parse().ok()?;
    let scale = match caps.get(2).map(|m| m.as_str().to_lowercase()) {
        Some(ref u) if u == "m" => 1_000_000.0,
        Some(ref u) if u == "k" => 1_000.0,
        _ => 1.0,
    };
    let count = number * scale;
    (count.is_finite() && count >= 0.0).then(|| count.round() as u64)
}

/// `parseClaudeContext`: only the category table, never the per-tool tables after it. A missing row is not a zero.
///
/// Free means no turn and no model call. Not `total_cost_usd`: on a resumed session it is that session's earlier spend
/// (MEASURED 2026-10-19: 0.00107 with `num_turns` 0 and `duration_api_ms` 0).
pub fn parse_claude_context(json: &str) -> Option<ClaudeContextBreakdown> {
    let reply: Value = serde_json::from_str(json).ok()?;
    if reply.get("num_turns").and_then(Value::as_f64) != Some(0.0) {
        return None;
    }
    if reply.get("is_error") == Some(&Value::Bool(true)) {
        return None;
    }
    if reply.get("duration_api_ms").and_then(Value::as_f64).is_some_and(|ms| ms > 0.0) {
        return None;
    }
    let markdown = reply.get("result")?.as_str()?.replace('\r', "");
    let total = regex::Regex::new(r"(?i)\*\*Tokens:\*\*\s*([\d.,]+[km]?)\s*/\s*([\d.,]+[km]?)").unwrap();
    let caps = total.captures(&markdown)?;
    let used = token_count(&caps[1])?;
    let window = token_count(&caps[2])?;
    if window == 0 {
        return None;
    }
    let heading = regex::Regex::new(r"(?i)Estimated usage by category").unwrap();
    let mut parts = heading.split(&markdown);
    parts.next();
    let after = parts.next()?;
    let mut categories: Vec<ContextCategory> = Vec::new();
    let mut table = false;
    for line in after.split('\n') {
        if !line.trim().starts_with('|') {
            if table && !line.trim().is_empty() {
                break;
            }
            continue;
        }
        let pieces: Vec<&str> = line.split('|').collect();
        let cells: Vec<String> = if pieces.len() >= 2 {
            pieces[1..pieces.len() - 1].iter().map(|cell| cell.trim().replace("**", "")).collect()
        } else {
            Vec::new()
        };
        if !table {
            if cells.first().map(|c| c.to_lowercase()) != Some("category".to_owned()) {
                return None;
            }
            table = true;
            continue;
        }
        let Some(name) = cells.first() else { continue };
        let Some(tokens) = cells.get(1).and_then(|c| token_count(c)) else { continue };
        if name.is_empty() || name.chars().count() > 80 {
            continue;
        }
        if categories.len() >= 32 || categories.iter().any(|row| row.name.to_lowercase() == name.to_lowercase()) {
            return None;
        }
        categories.push(ContextCategory { name: name.clone(), tokens });
    }
    (!categories.is_empty()).then_some(ClaudeContextBreakdown { used, window, categories })
}

/// Why a `/context` run should stop: a starting turn, or its own time running out.
#[derive(Debug, Default)]
pub struct Stop(AtomicU8);

const RUNNING: u8 = 0;
const STOPPED: u8 = 1;
const TIMED_OUT: u8 = 2;

impl Stop {
    pub fn stop(&self) {
        let _ = self.0.compare_exchange(RUNNING, STOPPED, Ordering::SeqCst, Ordering::SeqCst);
    }
    fn time_out(&self) {
        let _ = self.0.compare_exchange(RUNNING, TIMED_OUT, Ordering::SeqCst, Ordering::SeqCst);
    }
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst) != RUNNING
    }
    fn timed_out(&self) -> bool {
        self.0.load(Ordering::SeqCst) == TIMED_OUT
    }
}

/// The `/context` process to run: Claude Code with the turn's own flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

const MAX_REPLY_BYTES: usize = 1_048_576;
pub const NOT_AVAILABLE: &str = "Context was not available.";

/// `runClaudeContext`: the lines the process printed, or "Context was not available." when it failed, printed more than
/// a megabyte, or was stopped. Stopping kills the process.
pub fn run_claude_context(command: &ContextCommand, stop: &Stop) -> Result<String, String> {
    let mut child = std::process::Command::new(&command.program)
        .args(&command.args)
        .current_dir(&command.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| NOT_AVAILABLE.to_owned())?;
    let mut stdout = child.stdout.take().ok_or_else(|| NOT_AVAILABLE.to_owned())?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut chunk = [0u8; 8192];
        let mut over = false;
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) if out.len() + n > MAX_REPLY_BYTES => over = true,
                Ok(n) if !over => out.extend_from_slice(&chunk[..n]),
                Ok(_) => {}
            }
        }
        (out, over)
    });
    let status = loop {
        if stop.is_stopped() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(NOT_AVAILABLE.to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return Err(NOT_AVAILABLE.to_owned()),
        }
    };
    let (out, over) = reader.join().map_err(|_| NOT_AVAILABLE.to_owned())?;
    if !status.success() || over || stop.is_stopped() {
        return Err(NOT_AVAILABLE.to_owned());
    }
    let text = String::from_utf8(out).map_err(|_| NOT_AVAILABLE.to_owned())?;
    Ok(text.lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>().join("\n"))
}

/// `ClaudeContextLaunch`: the turn's own paths and flags. No environment, tokens or transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeContextLaunch {
    pub cwd: String,
    pub executable_path: String,
    pub args: Vec<String>,
    /// "read-only", "workspace-write" or "full-access".
    pub sandbox: String,
    pub permission: Option<String>,
    pub browser: Option<String>,
    /// (name, dir).
    pub plugins: Vec<(String, String)>,
}

impl ClaudeContextLaunch {
    fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("cwd".into(), json!(self.cwd));
        o.insert("executablePath".into(), json!(self.executable_path));
        o.insert("args".into(), json!(self.args));
        o.insert("sandbox".into(), json!(self.sandbox));
        o.insert("plugins".into(), Value::Array(self.plugins.iter().map(|(name, dir)| json!({"name": name, "dir": dir})).collect()));
        if let Some(p) = &self.permission {
            o.insert("permission".into(), json!(p));
        }
        if let Some(b) = &self.browser {
            o.insert("browser".into(), json!(b));
        }
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            cwd: s("cwd")?,
            executable_path: s("executablePath")?,
            args: v.get("args")?.as_array()?.iter().filter_map(|a| a.as_str().map(str::to_owned)).collect(),
            sandbox: s("sandbox")?,
            permission: s("permission"),
            browser: s("browser"),
            plugins: v
                .get("plugins")
                .and_then(Value::as_array)
                .map(|ps| ps.iter().filter_map(|p| Some((p.get("name")?.as_str()?.to_owned(), p.get("dir")?.as_str()?.to_owned()))).collect())
                .unwrap_or_default(),
        })
    }
}

/// `ClaudeContextTurn`: a conversation's latest turn, as the ledger knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeContextTurn {
    pub mission_id: String,
    pub session_id: String,
    pub busy: bool,
    pub account: Option<String>,
    pub fallback: Option<ClaudeContextLaunch>,
}

/// A `/context` command made ready from a launch, and what tidies up after it.
pub struct PreparedContext {
    pub command: ContextCommand,
    pub dispose: Box<dyn FnOnce() + Send>,
}

type Resolve = dyn Fn(&str) -> Option<ClaudeContextTurn> + Send + Sync;
type Prepare = dyn Fn(&ClaudeContextLaunch, &ClaudeContextTurn) -> Result<PreparedContext, String> + Send + Sync;
type Run = dyn Fn(&ContextCommand, &Stop) -> Result<String, String> + Send + Sync;

/// What the reader is built from (`createClaudeContextReader`'s options).
pub struct ReaderOptions {
    /// Where each conversation's last launch is kept (paths and flags only).
    pub file: PathBuf,
    pub resolve: Arc<Resolve>,
    pub prepare: Arc<Prepare>,
    /// `runClaudeContext` unless a caller gives another.
    pub run: Option<Arc<Run>>,
    pub timeout: Duration,
}

type Answer = Option<ClaudeContextBreakdown>;

struct Pending {
    stop: Arc<Stop>,
    done: Arc<(Mutex<Option<Answer>>, Condvar)>,
}

#[derive(Default)]
struct Inner {
    cache: VecDeque<(String, Answer)>,
    pending: HashMap<String, Pending>,
    launches: Option<Vec<(String, ClaudeContextLaunch)>>,
}

/// `createClaudeContextReader`: no polling. One lazy, silent result per finished turn; a starting turn stops any
/// inspection first and waits for it.
pub struct ClaudeContextReader {
    options: ReaderOptions,
    inner: Mutex<Inner>,
    writes: Mutex<()>,
}

const CACHE_LIMIT: usize = 64;
const LAUNCH_LIMIT: usize = 128;

impl ClaudeContextReader {
    pub fn new(options: ReaderOptions) -> Self {
        Self { options, inner: Mutex::new(Inner::default()), writes: Mutex::new(()) }
    }

    fn load(&self, inner: &mut Inner) -> Result<(), String> {
        if inner.launches.is_some() {
            return Ok(());
        }
        let launches = match std::fs::read_to_string(&self.options.file) {
            Ok(text) => {
                let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                v.as_object()
                    .map(|o| o.iter().filter_map(|(k, l)| Some((k.clone(), ClaudeContextLaunch::from_json(l)?))).collect())
                    .unwrap_or_default()
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.to_string()),
        };
        inner.launches = Some(launches);
        Ok(())
    }

    /// `remember`: keep a turn's launch so `/context` can run with the same flags after it ends.
    pub fn remember(&self, mission_id: &str, launch: ClaudeContextLaunch) -> Result<(), String> {
        let _write = self.writes.lock().unwrap();
        let saved = {
            let mut inner = self.inner.lock().unwrap();
            self.load(&mut inner)?;
            let launches = inner.launches.as_mut().unwrap();
            launches.retain(|(id, _)| id != mission_id);
            launches.push((mission_id.to_owned(), launch));
            while launches.len() > LAUNCH_LIMIT {
                launches.remove(0);
            }
            Value::Object(launches.iter().map(|(id, l)| (id.clone(), l.to_json())).collect())
        };
        write_private(&self.options.file, &saved.to_string())
    }

    fn cache_result(inner: &mut Inner, key: &str, answer: Answer) {
        inner.cache.retain(|(k, _)| k != key);
        inner.cache.push_back((key.to_owned(), answer));
        while inner.cache.len() > CACHE_LIMIT {
            inner.cache.pop_front();
        }
    }

    /// `read`: the breakdown for a conversation's last finished turn, or nothing while a turn runs or when `/context`
    /// failed. Blocks until it is known; a second ask for the same turn waits on the first.
    pub fn read(&self, mission_id: &str) -> Answer {
        let turn = (self.options.resolve)(mission_id)?;
        if turn.busy {
            return None;
        }
        let key = format!("{}:{}", turn.mission_id, turn.session_id);
        let (stop, done) = {
            let mut inner = self.inner.lock().unwrap();
            if let Some((_, answer)) = inner.cache.iter().find(|(k, _)| *k == key) {
                return answer.clone();
            }
            if let Some(held) = inner.pending.get(&key) {
                let done = held.done.clone();
                drop(inner);
                return wait(&done);
            }
            let pending = Pending { stop: Arc::new(Stop::default()), done: Arc::new((Mutex::new(None), Condvar::new())) };
            let handles = (pending.stop.clone(), pending.done.clone());
            inner.pending.insert(key.clone(), pending);
            handles
        };

        let mut prepared: Option<PreparedContext> = None;
        let deadline = Instant::now() + self.options.timeout;
        let answer = self.inspect(mission_id, &turn, &key, &stop, deadline, &mut prepared);
        if let Some(p) = prepared.take() {
            (p.dispose)();
        }
        {
            let mut inner = self.inner.lock().unwrap();
            if stop.timed_out() {
                Self::cache_result(&mut inner, &key, None);
            }
            inner.pending.remove(&key);
        }
        let (slot, ready) = &*done;
        *slot.lock().unwrap() = Some(answer.clone());
        ready.notify_all();
        answer
    }

    fn inspect(
        &self,
        mission_id: &str,
        turn: &ClaudeContextTurn,
        key: &str,
        stop: &Arc<Stop>,
        deadline: Instant,
        prepared: &mut Option<PreparedContext>,
    ) -> Answer {
        let failed = |this: &Self| {
            // A stop for a starting turn is not a failure to remember: the next finished turn reads again.
            if !stop.is_stopped() || stop.timed_out() {
                Self::cache_result(&mut this.inner.lock().unwrap(), key, None);
            }
            None
        };
        let launch = {
            drop(self.writes.lock().unwrap());
            let mut inner = self.inner.lock().unwrap();
            if self.load(&mut inner).is_err() {
                drop(inner);
                return failed(self);
            }
            inner.launches.as_ref().unwrap().iter().find(|(id, _)| *id == turn.mission_id).map(|(_, l)| l.clone())
        };
        let Some(launch) = launch.or_else(|| turn.fallback.clone()) else { return None };
        match (self.options.prepare)(&launch, turn) {
            Ok(p) => *prepared = Some(p),
            Err(_) => return failed(self),
        }
        let still_finished = |this: &Self| {
            let current = (this.options.resolve)(mission_id);
            !stop.is_stopped() && current.is_some_and(|c| !c.busy && c.mission_id == turn.mission_id)
        };
        if !still_finished(self) {
            return None;
        }
        // The time limit, watched beside the run, so that a run that waits on its stop is ended by it too.
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let watch = {
            let (stop, finished) = (stop.clone(), finished.clone());
            std::thread::spawn(move || {
                while Instant::now() < deadline {
                    if finished.load(Ordering::SeqCst) || stop.is_stopped() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                stop.time_out();
            })
        };
        let command = &prepared.as_ref().unwrap().command;
        let raw = match &self.options.run {
            Some(run) => run(command, stop),
            None => run_claude_context(command, stop),
        };
        finished.store(true, Ordering::SeqCst);
        let _ = watch.join();
        let ran_out = stop.timed_out();
        let Ok(raw) = raw else { return failed(self) };
        if ran_out || !still_finished(self) {
            return None;
        }
        let answer = parse_claude_context(&raw);
        Self::cache_result(&mut self.inner.lock().unwrap(), key, answer.clone());
        answer
    }

    /// `beforeStart`: stop every inspection and wait for each to finish, before a turn starts.
    pub fn before_start(&self) {
        let waiting: Vec<Arc<(Mutex<Option<Answer>>, Condvar)>> = {
            let inner = self.inner.lock().unwrap();
            inner.pending.values().map(|p| {
                p.stop.stop();
                p.done.clone()
            }).collect()
        };
        for done in waiting {
            wait(&done);
        }
    }
}

fn wait(done: &(Mutex<Option<Answer>>, Condvar)) -> Answer {
    let (slot, ready) = done;
    let mut guard = slot.lock().unwrap();
    while guard.is_none() {
        guard = ready.wait(guard).unwrap();
    }
    guard.clone().unwrap()
}

/// Written beside itself and moved into place, readable by this user only.
fn write_private(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".{}.tmp", crate::invariants::unique_nanos()));
    let tmp = PathBuf::from(tmp);
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

#[cfg(test)]
#[path = "claude_context_tests.rs"]
mod tests;
