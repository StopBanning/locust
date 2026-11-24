//! What each agent has set up for itself: its MCP servers, hooks, skills and agents, read from the agent's own files and
//! shown, never changed (Electron `main/runtime-setup.ts`, the AI agents screen's `RuntimeSetupLine`).
//!
//! Colin, 2026-09-05, on hooks: "make sure the user actually knows what's going on". A hook fires shell commands at
//! points in a run; an MCP server gives the agent tools. Both are the agent's own configuration, so Locust adds none and
//! says what is there. Files are read with a byte bound and parsed leniently: a file that cannot be read is named as
//! unreadable, never guessed at. Only names are kept -- a server's command line and a hook's script can hold secrets.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_NAMES: usize = 40;

/// `PublicRuntimeSetup`: one agent's own setup, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSetup {
    pub mcp_servers: Vec<String>,
    pub hooks: Vec<String>,
    pub skills: Vec<String>,
    pub agents: Vec<String>,
    /// Files and folders read.
    pub sources: Vec<String>,
    /// Files and folders that exist but could not be read or parsed.
    pub unreadable: Vec<String>,
}

/// Reads a file's text. The test seam for `fs::read_to_string`.
pub type ReadFile<'a> = &'a dyn Fn(&Path) -> std::io::Result<String>;
/// A folder's entries, names only, a folder's name ending in "/". The test seam for `fs::read_dir`.
pub type ListFolder<'a> = &'a dyn Fn(&Path) -> std::io::Result<Vec<String>>;

/// `RuntimeSetupOptions`.
pub struct SetupOptions<'a> {
    pub workspace: Option<PathBuf>,
    pub home: PathBuf,
    pub read: ReadFile<'a>,
    pub list: ListFolder<'a>,
}

/// The machine's own files.
pub fn read_from_disk(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

/// The machine's own folders.
pub fn list_from_disk(dir: &Path) -> std::io::Result<Vec<String>> {
    Ok(std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) { format!("{name}/") } else { name }
        })
        .collect())
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Folders,
    Markdown,
}

/// The names in a folder an agent keeps its skills (one folder each) or agents (one .md each) in. Absent is nothing;
/// unreadable is said. Dot-names, like Codex's `.system` built-ins, are left out.
fn names(dir: &Path, o: &SetupOptions, found: &mut RuntimeSetup, kind: Kind) -> Vec<String> {
    match (o.list)(dir) {
        Ok(entries) => {
            found.sources.push(shown(dir));
            entries
                .into_iter()
                .filter(|e| if kind == Kind::Folders { e.ends_with('/') } else { e.to_lowercase().ends_with(".md") })
                .map(|e| {
                    let e = e.strip_suffix('/').unwrap_or(&e).to_owned();
                    if e.to_lowercase().ends_with(".md") { e[..e.len() - 3].to_owned() } else { e }
                })
                .filter(|e| !e.starts_with('.'))
                .collect()
        }
        Err(e) => {
            if e.kind() != ErrorKind::NotFound {
                found.unreadable.push(shown(dir));
            }
            Vec::new()
        }
    }
}

fn add_names(into: &mut Vec<String>, names: impl IntoIterator<Item = String>) {
    for name in names {
        if name.trim().is_empty() || into.contains(&name) {
            continue;
        }
        if into.len() >= MAX_NAMES {
            return;
        }
        into.push(name.chars().take(80).collect());
    }
}

fn read_text(path: &Path, o: &SetupOptions, found: &mut RuntimeSetup) -> Option<String> {
    match (o.read)(path) {
        Ok(text) if text.len() > MAX_FILE_BYTES => {
            found.unreadable.push(shown(path));
            None
        }
        Ok(text) => Some(text),
        Err(e) => {
            // Absent is the common case and not a finding; anything else is.
            if e.kind() != ErrorKind::NotFound {
                found.unreadable.push(shown(path));
            }
            None
        }
    }
}

fn read_json(path: &Path, o: &SetupOptions, found: &mut RuntimeSetup) -> Option<Map<String, Value>> {
    let text = read_text(path, o, found)?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => {
            found.sources.push(shown(path));
            Some(map)
        }
        _ => {
            found.unreadable.push(shown(path));
            None
        }
    }
}

fn keys_of(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Object(map)) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

/// Hook events with how many entries each: "Stop (1)".
fn hook_labels(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(map)) = value else { return Vec::new() };
    map.iter()
        .map(|(event, entries)| {
            let count = match entries {
                Value::Array(a) => a.len(),
                Value::Object(o) => o.len(),
                _ => 1,
            };
            format!("{event} ({count})")
        })
        .collect()
}

/// Claude Code's per-project record, for either slash style the file uses.
fn claude_project_record<'v>(projects: Option<&'v Value>, ws: &Path) -> Option<&'v Value> {
    let Some(Value::Object(projects)) = projects else { return None };
    let ws = ws.to_string_lossy();
    let wanted = [ws.to_string(), ws.replace('\\', "/"), ws.replace('/', "\\")].map(|p| p.to_lowercase());
    projects.iter().find(|(k, v)| wanted.contains(&k.to_lowercase()) && v.is_object()).map(|(_, v)| v)
}

fn get<'m>(map: &'m Option<Map<String, Value>>, key: &str) -> Option<&'m Value> {
    map.as_ref().and_then(|m| m.get(key))
}

fn claude(o: &SetupOptions, home: &Path, ws: Option<&Path>) -> RuntimeSetup {
    let mut found = RuntimeSetup::default();
    let user = read_json(&home.join(".claude").join("settings.json"), o, &mut found);
    add_names(&mut found.hooks, hook_labels(get(&user, "hooks")));
    if let Some(ws) = ws {
        for name in ["settings.json", "settings.local.json"] {
            let project = read_json(&ws.join(".claude").join(name), o, &mut found);
            add_names(&mut found.hooks, hook_labels(get(&project, "hooks")));
        }
    }
    let global = read_json(&home.join(".claude.json"), o, &mut found);
    add_names(&mut found.mcp_servers, keys_of(get(&global, "mcpServers")));
    if let Some(ws) = ws {
        add_names(&mut found.mcp_servers, keys_of(claude_project_record(get(&global, "projects"), ws).and_then(|r| r.get("mcpServers"))));
        let project_mcp = read_json(&ws.join(".mcp.json"), o, &mut found);
        add_names(&mut found.mcp_servers, keys_of(get(&project_mcp, "mcpServers")));
    }
    let skills = names(&home.join(".claude").join("skills"), o, &mut found, Kind::Folders);
    add_names(&mut found.skills, skills);
    let agents = names(&home.join(".claude").join("agents"), o, &mut found, Kind::Markdown);
    add_names(&mut found.agents, agents);
    if let Some(ws) = ws {
        let skills = names(&ws.join(".claude").join("skills"), o, &mut found, Kind::Folders);
        add_names(&mut found.skills, skills);
        let agents = names(&ws.join(".claude").join("agents"), o, &mut found, Kind::Markdown);
        add_names(&mut found.agents, agents);
    }
    found
}

fn codex(o: &SetupOptions, home: &Path, ws: Option<&Path>) -> RuntimeSetup {
    let mut found = RuntimeSetup::default();
    let header = regex::Regex::new(r"(?m)^\s*\[mcp_servers\.([^\]\s.]+)\]").unwrap();
    let notify = regex::Regex::new(r"(?m)^\s*notify\s*=").unwrap();
    let mut paths = vec![home.join(".codex").join("config.toml")];
    paths.extend(ws.map(|ws| ws.join(".codex").join("config.toml")));
    for path in paths {
        let Some(text) = read_text(&path, o, &mut found) else { continue };
        found.sources.push(shown(&path));
        // Table headers only: [mcp_servers.<name>] and nothing beneath them.
        add_names(&mut found.mcp_servers, header.captures_iter(&text).map(|c| c[1].to_owned()));
        if notify.is_match(&text) {
            add_names(&mut found.hooks, ["notify (1)".to_owned()]);
        }
    }
    // Where Codex itself finds skills (its app-server's skills/list, MEASURED 2026-09-24): never ~/.claude/skills.
    let mut folders = vec![home.join(".codex").join("skills"), home.join(".agents").join("skills")];
    folders.extend(ws.map(|ws| ws.join(".agents").join("skills")));
    for folder in folders {
        let skills = names(&folder, o, &mut found, Kind::Folders);
        add_names(&mut found.skills, skills);
    }
    found
}

fn cursor(o: &SetupOptions, home: &Path, ws: Option<&Path>) -> RuntimeSetup {
    let mut found = RuntimeSetup::default();
    let roots: Vec<&Path> = std::iter::once(home).chain(ws).collect();
    for root in &roots {
        let dir = root.join(".cursor");
        let mcp = read_json(&dir.join("mcp.json"), o, &mut found);
        add_names(&mut found.mcp_servers, keys_of(get(&mcp, "mcpServers")));
        let hooks = read_json(&dir.join("hooks.json"), o, &mut found);
        add_names(&mut found.hooks, hook_labels(get(&hooks, "hooks")));
    }
    // Cursor's own skill roots (read from its code): third-party folders and its `skills-cursor` built-ins are not listed.
    for root in &roots {
        for folder in [root.join(".cursor").join("skills"), root.join(".agents").join("skills")] {
            let skills = names(&folder, o, &mut found, Kind::Folders);
            add_names(&mut found.skills, skills);
        }
    }
    found
}

fn opencode(o: &SetupOptions, home: &Path, ws: Option<&Path>) -> RuntimeSetup {
    let mut found = RuntimeSetup::default();
    let mut paths = vec![home.join(".config").join("opencode").join("opencode.json")];
    if let Some(ws) = ws {
        paths.extend([ws.join("opencode.json"), ws.join(".opencode").join("opencode.json")]);
    }
    for path in paths {
        let config = read_json(&path, o, &mut found);
        add_names(&mut found.mcp_servers, keys_of(get(&config, "mcp")));
    }
    // OpenCode's skills (read from its source): Claude Code's and the shared .agents folders, and its own.
    let mut folders = vec![
        home.join(".claude").join("skills"),
        home.join(".agents").join("skills"),
        home.join(".config").join("opencode").join("skill"),
        home.join(".config").join("opencode").join("skills"),
    ];
    if let Some(ws) = ws {
        folders.extend([
            ws.join(".claude").join("skills"),
            ws.join(".agents").join("skills"),
            ws.join(".opencode").join("skill"),
            ws.join(".opencode").join("skills"),
        ]);
    }
    for folder in folders {
        let skills = names(&folder, o, &mut found, Kind::Folders);
        add_names(&mut found.skills, skills);
    }
    found
}

fn copilot(o: &SetupOptions, home: &Path, ws: Option<&Path>) -> RuntimeSetup {
    let mut found = RuntimeSetup::default();
    let config = read_json(&home.join(".copilot").join("mcp-config.json"), o, &mut found);
    add_names(&mut found.mcp_servers, keys_of(get(&config, "mcpServers")));
    // Where `copilot skill --help` (1.0.88) says it looks: the Claude folder only in the project.
    let mut folders = vec![home.join(".copilot").join("skills"), home.join(".agents").join("skills")];
    if let Some(ws) = ws {
        folders.extend([ws.join(".github").join("skills"), ws.join(".agents").join("skills"), ws.join(".claude").join("skills")]);
    }
    for folder in folders {
        let skills = names(&folder, o, &mut found, Kind::Folders);
        add_names(&mut found.skills, skills);
    }
    found
}

/// `copilotMcpServerNames`: the MCP servers in the person's own Copilot settings, the ones an Ask or Plan run on
/// Copilot starts none of (0.824). None when the file is absent or does not read as settings.
pub fn copilot_mcp_server_names(home: &Path, read: ReadFile) -> Vec<String> {
    let list: ListFolder = &|_| Err(ErrorKind::NotFound.into());
    let o = SetupOptions { workspace: None, home: home.to_owned(), read, list };
    let config = read_json(&home.join(".copilot").join("mcp-config.json"), &o, &mut RuntimeSetup::default());
    keys_of(get(&config, "mcpServers"))
}

/// `readRuntimeSetup`: every agent's own setup, by agent id.
pub fn read_runtime_setup(o: &SetupOptions) -> BTreeMap<String, RuntimeSetup> {
    let ws = o.workspace.as_deref().filter(|p| !p.as_os_str().is_empty()).map(|p| {
        if p.is_absolute() { p.to_owned() } else { std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_owned()) }
    });
    let ws = ws.as_deref();
    let home = o.home.as_path();
    BTreeMap::from([
        ("claude".to_owned(), claude(o, home, ws)),
        ("codex".to_owned(), codex(o, home, ws)),
        ("cursor".to_owned(), cursor(o, home, ws)),
        ("opencode".to_owned(), opencode(o, home, ws)),
        ("copilot".to_owned(), copilot(o, home, ws)),
    ])
}

/// Screens.tsx `fewNames`: five names, then how many more.
pub fn few_names(names: &[String]) -> String {
    const LIMIT: usize = 5;
    if names.len() <= LIMIT { names.join(", ") } else { format!("{} +{}", names[..LIMIT].join(", "), names.len() - LIMIT) }
}

/// Screens.tsx `RuntimeSetupLine`: the one line under an agent's row.
pub fn setup_line(setup: &RuntimeSetup) -> String {
    let mut parts = Vec::new();
    if !setup.mcp_servers.is_empty() {
        parts.push(format!("Connectors (MCP): {}", setup.mcp_servers.join(", ")));
    }
    if !setup.hooks.is_empty() {
        parts.push(format!("Hooks: {}", setup.hooks.join(", ")));
    }
    if !setup.skills.is_empty() {
        parts.push(format!("Skills: {}", few_names(&setup.skills)));
    }
    if !setup.agents.is_empty() {
        parts.push(format!("Agents: {}", few_names(&setup.agents)));
    }
    if !setup.unreadable.is_empty() {
        let n = setup.unreadable.len();
        parts.push(format!("{n} config file{} could not be read", if n == 1 { "" } else { "s" }));
    }
    if parts.is_empty() { "No connectors, hooks, skills or agents set up".to_owned() } else { parts.join(" · ") }
}

/// The line's hover: each file read, and each that could not be.
pub fn setup_title(setup: &RuntimeSetup) -> String {
    let lines: Vec<String> = setup
        .sources
        .iter()
        .map(|p| format!("read: {p}"))
        .chain(setup.unreadable.iter().map(|p| format!("unreadable: {p}")))
        .collect();
    if lines.is_empty() { "No configuration files found".to_owned() } else { lines.join("\n") }
}

#[cfg(test)]
#[path = "runtime_setup_tests.rs"]
mod tests;
