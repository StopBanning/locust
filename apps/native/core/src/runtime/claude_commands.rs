//! Claude Code's commands before any run (claude-commands.ts `readClaudeCommands`, 0.428 and 0.694). Claude Code is
//! started as every run outside Auto is (restricted, nothing kept of the session), sent the `initialize` control
//! request its own SDK sends -- a handshake, not a message: no turn, nothing spent -- and listened to: the answer's
//! `commands` (and `models`), or, from an older Claude Code, its `commands_changed` records. The last list is taken once
//! it has stood quiet a moment, and Claude Code is stopped.
use crate::runtime_commands::{CommandInfo, commands_from};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// CLAUDE_COMMANDS_REQUEST, without its newline.
pub const REQUEST: &str = r#"{"type":"control_request","request_id":"locust_commands","request":{"subtype":"initialize"}}"#;

/// createClaudeCommandListCommand's arguments.
pub const LIST_ARGS: [&str; 8] =
    ["--restricted", "--print", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--no-session-persistence"];

/// How long a list must stand before it is taken, and how long one is waited for at all.
#[derive(Clone, Copy)]
pub struct Wait {
    pub settle: Duration,
    pub timeout: Duration,
}

impl Default for Wait {
    fn default() -> Self {
        Self { settle: Duration::from_millis(1_500), timeout: Duration::from_secs(20) }
    }
}

/// claudeListedModelsFrom: each model Claude Code's own picker lists, as `{value, resolvedModel, displayName, efforts?}`.
pub fn listed_models_from(value: &Value) -> Vec<Value> {
    let Some(rows) = value.as_array() else {
        return vec![];
    };
    let named = |v: &Value| v.as_str().filter(|t| !t.is_empty() && t.chars().count() <= 200).map(str::to_owned);
    let level = crate::invariants::regex(r"^[a-z]{1,16}$");
    rows.iter()
        .filter_map(|entry| {
            let id = named(&entry["value"])?;
            let display = named(&entry["displayName"])?;
            let resolved = entry["resolvedModel"].as_str().filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| id.clone());
            let mut model = json!({"value": id, "resolvedModel": resolved, "displayName": display});
            if entry["supportsEffort"] == true
                && let Some(levels) = entry["supportedEffortLevels"].as_array()
            {
                let efforts: Vec<&str> = levels.iter().filter_map(Value::as_str).filter(|l| level.is_match(l)).collect();
                if !efforts.is_empty() {
                    model["efforts"] = json!(efforts);
                }
            }
            Some(model)
        })
        .collect()
}

/// Reads Claude Code's commands: `claude` is the executable as Locust starts it, `folder` the workspace. `on_models`
/// is told the models an answer lists, when it lists any.
pub fn read_commands(mut claude: Command, folder: &Path, wait: Wait, on_models: &mut dyn FnMut(Vec<Value>)) -> Result<Vec<CommandInfo>, String> {
    claude.args(LIST_ARGS).current_dir(folder).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = claude.spawn().map_err(|e| format!("Claude Code could not be started: {e}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        // An older Claude Code announces its list unasked; the wait below still hears it.
        let _ = stdin.write_all(format!("{REQUEST}\n").as_bytes()).and_then(|_| stdin.flush());
    }
    let (send, chunks) = mpsc::channel::<Vec<u8>>();
    let mut stdout = child.stdout.take().expect("stdout was piped");
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = stdout.read(&mut chunk) {
            if n == 0 || send.send(chunk[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + wait.timeout;
    let mut quiet: Option<Instant> = None;
    let mut latest: Option<Vec<CommandInfo>> = None;
    let mut buffer: Vec<u8> = vec![];
    let failure = loop {
        let until = quiet.map_or(deadline, |q| q.min(deadline));
        let now = Instant::now();
        if now >= until {
            break if quiet.is_some_and(|q| q <= now) { None } else { Some("Claude Code did not list its commands in time.") };
        }
        let chunk = match chunks.recv_timeout(until - now) {
            Ok(chunk) => chunk,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break Some("Claude Code exited before it listed its commands."),
        };
        buffer.extend(chunk);
        while let Some(at) = buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=at).collect();
            let Ok(record) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            if !record.is_object() {
                continue;
            }
            // The answer to `initialize`: its list is `response.response.commands`, its models `response.response.models`.
            let answer = (record["type"] == "control_response" && record["response"].is_object()).then(|| &record["response"]["response"]);
            let answered = answer.map(|a| &a["commands"]).filter(|c| !c.is_null());
            let models = answer.map(|a| listed_models_from(&a["models"])).unwrap_or_default();
            if !models.is_empty() {
                on_models(models);
            }
            if answered.is_none() && !(record["type"] == "system" && record["subtype"] == "commands_changed") {
                continue;
            }
            let listed = commands_from(answered.unwrap_or(&record["commands"]));
            if listed.is_empty() {
                continue;
            }
            latest = Some(listed);
            quiet = Some(Instant::now() + wait.settle);
        }
        // A line that never ends is not a list.
        if buffer.len() > 2_000_000 {
            break Some("Claude Code sent more than a command list.");
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    match latest {
        Some(listed) => Ok(listed),
        None => Err(failure.unwrap_or("Claude Code did not list its commands.").to_owned()),
    }
}

#[cfg(test)]
#[path = "claude_commands_tests.rs"]
mod claude_commands_tests;
