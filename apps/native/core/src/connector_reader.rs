//! Which connectors this machine has, and how each is (Electron `runtime-adapters` `parseClaudeConnectors`,
//! `main/connector-reader.ts`, `renderer/src/connectorHealth.ts`).
//!
//! What the person's Claude Code can reach, their teammates can reach: there is no per-teammate list. The account
//! connectors from claude.ai never appear in `~/.claude.json`, so the list is `claude mcp list`, the only source that
//! has all of them. That is slow (it health-checks every server), so it is read in the background and held for five
//! minutes, and a run never waits for it.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::Duration,
};

/// How long a reading stands before it is taken again.
pub const TTL_MS: i64 = 5 * 60_000;
/// A read that has not answered by here is not going to help this run.
pub const READ_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Connected,
    NeedsAuth,
    Failed,
    /// The last check hit the 20-second limit; the connectors are those of the last reading that answered.
    TimedOut,
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connector {
    pub name: String,
    pub location: String,
    pub status: Status,
    /// What the CLI said, for a state it did not name in words Locust knows.
    pub said: Option<String>,
    pub checked_in_ms: Option<i64>,
    /// When it was last seen connected, in milliseconds since the epoch.
    pub last_connected_ms: Option<i64>,
}

/// `claude mcp list`, one connector a line: `name: location - health`. Failure words are read first: "Disconnected"
/// holds "connected".
pub fn parse_claude_connectors(text: &str) -> Vec<Connector> {
    let mut found: Vec<Connector> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        let Some(cut) = line.rfind(" - ") else { continue };
        let (head, health) = (&line[..cut], line[cut + 3..].trim());
        let Some(colon) = head.find(": ").filter(|&c| c > 0) else { continue };
        let (name, location) = (head[..colon].trim(), head[colon + 2..].trim());
        if name.is_empty() || name.chars().count() > 120 || location.is_empty() || found.iter().any(|c| c.name == name) {
            continue;
        }
        let lower = health.to_lowercase();
        let status = if lower.contains("needs authentication") || lower.contains("need authentication") || lower.contains("authenticate") {
            Status::NeedsAuth
        } else if ["fail", "✗", "error", "could not", "unable", "disconnected", "timed out", "timeout", "time out"].iter().any(|w| lower.contains(w)) {
            Status::Failed
        } else if lower.contains("connected") {
            Status::Connected
        } else {
            Status::Unreadable
        };
        let said = (status == Status::Unreadable).then(|| health.chars().take(80).collect());
        found.push(Connector { name: name.into(), location: location.into(), status, said, checked_in_ms: None, last_connected_ms: None });
    }
    found
}

/// What a read gave: the listing's text, or that it hit the limit. None when there is no Claude Code to ask, which is
/// not a failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    Text(String),
    TimedOut,
}

type ReadFn = Box<dyn Fn(Duration) -> Option<Read> + Send + Sync>;
type NowFn = Box<dyn Fn() -> i64 + Send + Sync>;

#[derive(Default)]
struct Held {
    connectors: Vec<Connector>,
    taken_at: Option<i64>,
    last_check: Option<(i64, bool)>,
    last_connected: HashMap<String, i64>,
    generation: u64,
}

/// The connector list, read in the background and held.
pub struct Reader {
    read: ReadFn,
    now: NowFn,
    ttl_ms: i64,
    held: Mutex<Held>,
    /// One reading at a time, however many callers ask at once.
    reading: Mutex<()>,
}

impl Reader {
    pub fn new(read: ReadFn, now: NowFn) -> Self {
        Reader { read, now, ttl_ms: TTL_MS, held: Mutex::new(Held::default()), reading: Mutex::new(()) }
    }

    fn view(held: &Held) -> Vec<Connector> {
        held.connectors
            .iter()
            .map(|c| Connector {
                status: if held.last_check.is_some_and(|(_, timed_out)| timed_out) { Status::TimedOut } else { c.status },
                checked_in_ms: held.last_check.map(|(ms, _)| ms),
                last_connected_ms: held.last_connected.get(&c.name).copied(),
                ..c.clone()
            })
            .collect()
    }

    /// Every connector that answered, for the allow rules. Never waits: always the last reading that answered. A
    /// server the person has not finished signing into would produce a rule for tools that are not there.
    pub fn names(&self) -> Vec<String> {
        let held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        held.connectors.iter().filter(|c| c.status == Status::Connected).map(|c| c.name.clone()).collect()
    }

    /// The full reading, for anything that shows it.
    pub fn current(&self) -> Vec<Connector> {
        Self::view(&self.held.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Take a reading if the held one is stale, and return it once it settles.
    pub fn refresh(&self) -> Vec<Connector> {
        let fresh = |held: &Held| held.taken_at.is_some_and(|at| (self.now)() - at < self.ttl_ms);
        if fresh(&self.held.lock().unwrap_or_else(|e| e.into_inner())) {
            return self.current();
        }
        let _one = self.reading.lock().unwrap_or_else(|e| e.into_inner());
        // Another caller's reading may have just landed while this one waited its turn.
        let (asked, began) = {
            let held = self.held.lock().unwrap_or_else(|e| e.into_inner());
            if fresh(&held) {
                return Self::view(&held);
            }
            (held.generation, (self.now)())
        };
        let read = (self.read)(READ_TIMEOUT);
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        // A reading begun before a connector was added or taken back answers for a list that has since changed.
        if asked != held.generation {
            return Self::view(&held);
        }
        match read {
            // A read that could not run leaves the last good reading alone: a briefly busy Claude Code must not empty
            // the list and take everyone's connectors away mid-session.
            None => {}
            // Said, not hidden: the rules keep the last good reading, and Settings says it did not answer.
            Some(Read::TimedOut) => {
                let now = (self.now)();
                held.last_check = Some((now - began, true));
                held.taken_at = Some(now);
            }
            Some(Read::Text(text)) => {
                let now = (self.now)();
                held.connectors = parse_claude_connectors(&text);
                held.taken_at = Some(now);
                held.last_check = Some((now - began, false));
                let connected: Vec<String> = held.connectors.iter().filter(|c| c.status == Status::Connected).map(|c| c.name.clone()).collect();
                for name in connected {
                    held.last_connected.insert(name, now);
                }
            }
        }
        Self::view(&held)
    }

    /// Count the held reading as stale: a connector was just added or taken back, so the next look reads again.
    pub fn forget(&self) {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        held.generation += 1;
        held.taken_at = None;
    }
}

/// One connector's state in words, what to do about it, and its tone (MCP Inspector's vocabulary, said the way a
/// person acts on it).
pub fn state_words(c: &Connector) -> (&'static str, Option<String>, &'static str) {
    match c.status {
        Status::Connected => ("Connected", None, "good"),
        Status::NeedsAuth => ("Needs sign-in", Some(format!("Run /mcp in Claude Code and sign in to {}.", c.name)), "amber"),
        Status::Failed => ("Not working", Some("Claude Code could not start or reach it.".into()), "red"),
        Status::TimedOut => ("Did not answer", Some("It did not answer in 20 seconds. Check again in a moment.".into()), "amber"),
        Status::Unreadable => ("Unknown", Some(format!("Claude Code said: \u{201c}{}\u{201d}", c.said.as_deref().unwrap_or(""))), "amber"),
    }
}

/// When it last worked, said plainly.
pub fn last_worked(c: &Connector, now_ms: i64) -> String {
    let Some(at) = c.last_connected_ms else { return "Not seen connected since Locust started.".into() };
    let minutes = (now_ms - at).div_euclid(60_000);
    if minutes < 1 {
        "Connected just now.".into()
    } else if minutes < 60 {
        format!("Last connected {minutes} min ago.")
    } else {
        format!("Last connected {} h ago.", minutes / 60)
    }
}

/// The line under a connector's name, or nothing: a connected row checked a moment ago has nothing to add.
pub fn detail(c: &Connector, now_ms: i64) -> Option<String> {
    let (_, todo, _) = state_words(c);
    let when = last_worked(c, now_ms);
    if c.status == Status::Connected && when == "Connected just now." {
        return None;
    }
    Some(match todo {
        Some(todo) => format!("{todo} {when}"),
        None => when,
    })
}

/// Where it lives, as much as a report may say: a remote connector's host, never its path or query; a local one is
/// "a program on this computer", never its command, arguments or environment.
pub fn where_it_lives(location: &str) -> String {
    for scheme in ["https://", "http://"] {
        if let Some(rest) = location.strip_prefix(scheme) {
            let host = rest.split(['/', '?', '#']).next().unwrap_or("");
            let host = host.rsplit('@').next().unwrap_or(host);
            if !host.is_empty() {
                return host.to_owned();
            }
        }
    }
    "a program on this computer".into()
}

/// The text Copy report puts on the clipboard: names, states, timings and hosts. Nothing else.
pub fn report(connectors: &[Connector], now_iso: &str, now_ms: i64) -> String {
    let mut lines = vec![format!("Locust connector report, {now_iso}")];
    if let Some(ms) = connectors.iter().find_map(|c| c.checked_in_ms) {
        lines.push(format!("Last check took {:.1} s.", ms as f64 / 1000.0));
    }
    if connectors.is_empty() {
        lines.push("No connectors found.".into());
    }
    for c in connectors {
        let (state, _, _) = state_words(c);
        lines.push(format!("- {} ({}): {state}. {}", c.name, where_it_lives(&c.location), last_worked(c, now_ms)));
    }
    lines.join("\n")
}

/// The person's own Claude Code, asked `claude mcp list` with the time limit. None when there is no Claude Code.
pub fn read_with_claude(limit: Duration) -> Option<Read> {
    let claude = crate::runtime::discovery::find_named("claude").ok()?;
    let answer = crate::github_account::run_bounded(std::process::Command::new(claude).args(["mcp", "list"]), limit);
    match answer.exit {
        crate::github_account::Exit::Missing => None,
        _ if answer.stderr == "It took too long." => Some(Read::TimedOut),
        _ => Some(Read::Text(answer.stdout)),
    }
}

static SHARED: Mutex<Option<std::sync::Arc<Reader>>> = Mutex::new(None);

/// The one reader this process holds: the window's Settings and, later, the allow rules of a Claude Code turn.
pub fn shared() -> std::sync::Arc<Reader> {
    SHARED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(|| {
            std::sync::Arc::new(Reader::new(
                Box::new(read_with_claude),
                Box::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)),
            ))
        })
        .clone()
}

/// A reader of a test's own, in place of the person's Claude Code.
pub fn use_reader(reader: std::sync::Arc<Reader>) {
    *SHARED.lock().unwrap_or_else(|e| e.into_inner()) = Some(reader);
}

#[cfg(test)]
#[path = "connector_reader_tests.rs"]
mod tests;
