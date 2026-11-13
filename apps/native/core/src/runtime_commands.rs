//! EACH RUNTIME'S OWN SLASH COMMANDS, IN THE `/` MENU (0.426, main/runtime-commands.ts).
//!
//! Claude Code lists every command it has, its own and the person's (`.claude/commands`, skills), with a description,
//! in the `commands_changed` records it sends at the start of a run. Locust used to drop that list; it is kept here,
//! once, outside every mission's record, and read by the composer's `/` menu. A message that starts with one of them
//! is sent as the command itself (`native::send_from`). OpenCode's list comes from its server (0.427); Codex never
//! lists its own, so its three are fixed here (0.428).
//!
//! Left out on purpose: the ones that change the person's own Claude Code setup outside the conversation. Model and
//! effort are the teammate's, set in Locust; config, mcp, output-style, agents and fast would rewrite settings.
use crate::store::Store;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The file, in the native data folder (Electron keeps `runtime-commands.json` in its own).
pub const FILE: &str = "runtime-commands.json";

/// Claude Code's commands that would change the person's own setup (HIDDEN_COMMANDS.claude).
pub const HIDDEN_CLAUDE: &[&str] = &[
    "model", "effort", "fast", "config", "mcp", "output-style", "agents", "login", "logout", "permissions", "hooks", "statusline",
    "theme", "vim", "terminal-setup", "privacy-settings", "upgrade", "install-github-app", "ide", "keybindings", "exit", "resume",
    "sandbox", "plugin", "add-dir", "memory", "auto-mode-setup",
];

/// The runtimes whose commands Locust can list and send (OpenCode's are all prompt templates, so none is hidden).
pub const COMMAND_RUNTIMES: [&str; 2] = ["claude", "opencode"];

/// What `/init` asks Codex to do: Locust's words, to the same end as its terminal's.
pub const CODEX_INIT_PROMPT: &str = "Create an AGENTS.md file at the root of this repository: a short guide for coding agents working here. Look at the project first. Cover how it is laid out, how to build, test and run it, the conventions its code follows, and anything a newcomer would likely get wrong. Keep it concise and specific to this repository. If an AGENTS.md already exists, improve it rather than replacing it.";

/// One of a runtime's own slash commands, as the CLI names and describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub argument_hint: String,
}

impl CommandInfo {
    fn new(name: &str, description: &str, argument_hint: &str) -> Self {
        Self { name: name.into(), description: description.into(), argument_hint: argument_hint.into() }
    }
    fn to_json(&self) -> Value {
        json!({"name": self.name, "description": self.description, "argumentHint": self.argument_hint})
    }
}

/// Codex's own commands (0.428), which Codex never lists: its terminal draws them and sends each as a request of its
/// own. Only the ones that do work in a conversation are here.
pub fn codex_commands() -> Vec<CommandInfo> {
    vec![
        CommandInfo::new("review", "Review your current changes and find issues", "[what to review]"),
        CommandInfo::new("compact", "Summarize the conversation to prevent hitting the context limit", ""),
        CommandInfo::new("init", "Create an AGENTS.md file with instructions for Codex", ""),
    ]
}

fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-')
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric()) && name.chars().count() <= 64 && name.chars().all(name_char)
}

/// The command a message names, when it starts with `/name` (and the name ends at a space or the end).
pub fn command_named(prompt: &str) -> Option<String> {
    let rest = prompt.trim_start().strip_prefix('/')?;
    let run: String = rest.chars().take_while(|c| name_char(*c)).collect();
    if !valid_name(&run) {
        return None;
    }
    match rest[run.len()..].chars().next() {
        None => Some(run),
        Some(c) if c.is_whitespace() => Some(run),
        Some(_) => None,
    }
}

/// What follows the command's name, trimmed: what a runtime is sent as the command's own message.
pub fn after_name(prompt: &str, name: &str) -> String {
    let trimmed = prompt.trim_start();
    trimmed.get(name.len() + 1..).unwrap_or("").trim().to_owned()
}

/// The list as a CLI sent it, bounded: at most 400, names a command could have, text cut short.
pub fn commands_from(value: &Value) -> Vec<CommandInfo> {
    let Some(rows) = value.as_array() else {
        return Vec::new();
    };
    let text = |field: &Value, max: usize| -> String {
        let Some(raw) = field.as_str() else {
            return String::new();
        };
        raw.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect()
    };
    rows.iter()
        .take(400)
        .filter_map(|entry| {
            let name = entry.as_object()?.get("name")?.as_str()?.trim();
            let name = name.strip_prefix('/').unwrap_or(name);
            valid_name(name).then(|| CommandInfo {
                name: name.to_owned(),
                description: text(&entry["description"], 240),
                argument_hint: text(&entry["argumentHint"], 80),
            })
        })
        .collect()
}

fn visible(runtime: &str, commands: &[CommandInfo]) -> Vec<CommandInfo> {
    commands.iter().filter(|c| !(runtime == "claude" && HIDDEN_CLAUDE.contains(&c.name.as_str()))).cloned().collect()
}

/// Writes of the file go one at a time: a run thread learning a list and a window reading it never see half of it.
static WRITING: Mutex<()> = Mutex::new(());

/// The kept lists, in a store.
pub struct RuntimeCommands<'a> {
    store: &'a Store,
}

pub fn for_store(store: &Store) -> RuntimeCommands<'_> {
    RuntimeCommands { store }
}

impl RuntimeCommands<'_> {
    /// None yet, or unreadable: the menu offers Locust's own until the next Claude run lists them.
    fn held(&self) -> BTreeMap<String, Vec<CommandInfo>> {
        let mut held = BTreeMap::new();
        // The native file once one was written; until then what Electron's app kept (read only, as its ledgers are).
        let Ok(body) = std::fs::read_to_string(self.store.root().join(FILE)).or_else(|_| std::fs::read_to_string(self.store.electron().join(FILE))) else {
            return held;
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&body) else {
            return held;
        };
        for runtime in COMMAND_RUNTIMES {
            let Some(kept) = parsed["byRuntime"][runtime].as_array() else {
                continue;
            };
            let rows = kept
                .iter()
                .filter_map(|entry| {
                    let name = entry.as_object()?.get("name")?.as_str()?;
                    let string = |key: &str| entry[key].as_str().unwrap_or("").to_owned();
                    Some(CommandInfo { name: name.to_owned(), description: string("description"), argument_hint: string("argumentHint") })
                })
                .collect();
            held.insert(runtime.to_owned(), rows);
        }
        held
    }

    /// What a runtime's list holds, hidden ones and all, as it was kept (a test's way to say the same list again).
    #[cfg(test)]
    pub fn held_for_test(&self, runtime: &str) -> Vec<CommandInfo> {
        self.held().get(runtime).cloned().unwrap_or_default()
    }

    /// What each runtime offers, the hidden ones left out; Codex's three are always there.
    pub fn list(&self) -> BTreeMap<String, Vec<CommandInfo>> {
        let mut out: BTreeMap<String, Vec<CommandInfo>> =
            self.held().into_iter().map(|(runtime, commands)| (runtime.clone(), visible(&runtime, &commands))).collect();
        out.insert("codex".into(), codex_commands());
        out
    }

    /// Take a runtime's list as its CLI sent it; true when it changed.
    pub fn set(&self, runtime: &str, commands: &[CommandInfo]) -> Result<bool, String> {
        if !COMMAND_RUNTIMES.contains(&runtime) {
            return Err(format!("{runtime} lists no commands of its own."));
        }
        let _writing = crate::invariants::lock(&WRITING);
        let mut held = self.held();
        if held.get(runtime).map(Vec::as_slice).unwrap_or(&[]) == commands {
            return Ok(false);
        }
        held.insert(runtime.to_owned(), commands.to_vec());
        let by_runtime: serde_json::Map<String, Value> =
            held.iter().map(|(k, v)| (k.clone(), Value::Array(v.iter().map(CommandInfo::to_json).collect()))).collect();
        self.store.save_json(FILE, &json!({"schemaVersion": 1, "byRuntime": by_runtime}))?;
        Ok(true)
    }

    /// Whether a message is one of this runtime's commands, and so is sent as it is.
    pub fn is_command(&self, runtime: Option<&str>, prompt: &str) -> bool {
        let Some(name) = command_named(prompt) else {
            return false;
        };
        match runtime {
            Some("codex") => codex_commands().iter().any(|c| c.name == name),
            Some(runtime) if COMMAND_RUNTIMES.contains(&runtime) => {
                visible(runtime, self.held().get(runtime).map(Vec::as_slice).unwrap_or(&[])).iter().any(|c| c.name == name)
            }
            _ => false,
        }
    }
}

/// A list a run learned, kept in the engine's store. A store that cannot be written is not the run's trouble.
pub fn keep_learned(runtime: &str, commands: &[CommandInfo]) -> bool {
    let Some(store) = crate::native::store() else {
        return false;
    };
    for_store(&store).set(runtime, commands).unwrap_or(false)
}

#[cfg(test)]
#[path = "runtime_commands_tests.rs"]
mod tests;
