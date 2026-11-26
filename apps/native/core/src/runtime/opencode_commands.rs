//! OPENCODE'S OWN COMMANDS, FOR THE `/` MENU (opencode-commands.ts `readOpenCodeCommands`, 0.427).
//!
//! `opencode run` never lists them; its server does, at `GET /command`: `{name, description, source, template,
//! hints}` for each (1.18.27, measured 2026-09-28), `init` and `review` built in and every skill beside them. So a
//! server is started for as long as it takes to ask, in the person's folder (a folder's own `.opencode/command` files
//! are listed there), with a random password like every server Locust starts (opencode_serve), and with plugins off
//! (`OPENCODE_PURE`): listing commands is no reason to run code a repository chose. The template is never read here;
//! it is the command's whole prompt, and the menu needs its name. OpenCode 2 has no such server, and lists none.
use crate::runtime_commands::{CommandInfo, commands_from};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long the server is given to start and answer (Electron's `timeoutMs` default).
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// What goes after a command, from OpenCode's `hints`: `$ARGUMENTS` says only that it takes some.
fn hint_of(hints: &Value) -> String {
    let Some(hints) = hints.as_array() else {
        return String::new();
    };
    let named: Vec<&str> = hints.iter().filter_map(Value::as_str).filter(|h| *h != "$ARGUMENTS").collect();
    if !named.is_empty() {
        named.join(" ")
    } else if hints.iter().any(|h| h == "$ARGUMENTS") {
        "[arguments]".into()
    } else {
        String::new()
    }
}

/// OpenCode's commands, from a server started by `command` (opencode::command's for the folder, which it runs in).
/// `v2`: OpenCode 2, which starts nothing and lists nothing. The server is stopped whatever it answered.
pub fn read_commands(mut command: Command, v2: bool, timeout: Duration) -> Result<Vec<CommandInfo>, String> {
    if v2 {
        return Ok(vec![]);
    }
    let password = super::opencode_serve::password().map_err(|e| e.to_string())?;
    let folder = command.get_current_dir().map(|d| d.to_string_lossy().into_owned());
    command
        .env("OPENCODE_SERVER_PASSWORD", &password)
        .env("OPENCODE_PURE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| format!("OpenCode could not be started: {e}"))?;
    let stdout = child.stdout.take().expect("stdout was piped");
    let (send, lines) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + timeout;
    let listed = (|| {
        let listening = crate::invariants::regex(r"listening on (http://127\.0\.0\.1:[0-9]+)");
        let late = || "OpenCode's server did not answer in time.".to_owned();
        let mut said = String::new();
        let base = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match lines.recv_timeout(left) {
                Ok(line) => {
                    said.push_str(&line);
                    said.push('\n');
                    if said.len() > 4_000 {
                        said = said[said.len() - 4_000..].to_owned();
                    }
                    if let Some(found) = listening.captures(&said) {
                        break found[1].to_owned();
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(late()),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("OpenCode's server exited before it listed its commands.".to_owned());
                }
            }
        };
        let in_folder = folder.map(|f| format!("?directory={}", super::opencode_serve::encoded(&f))).unwrap_or_default();
        let auth = format!("Basic {}", super::opencode_serve::base64(format!("opencode:{password}").as_bytes()));
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(late());
        }
        let answer = ureq::AgentBuilder::new()
            .redirects(0)
            .build()
            .get(&format!("{base}/command{in_folder}"))
            .set("authorization", &auth)
            .timeout(left)
            .call();
        let listed: Value = match answer {
            Ok(response) => response.into_json().map_err(|_| "OpenCode's server could not be reached.".to_owned())?,
            Err(ureq::Error::Status(code, _)) => {
                return Err(format!("OpenCode's server answered {code} for its commands."));
            }
            Err(_) => return Err("OpenCode's server could not be reached.".to_owned()),
        };
        let rows: Vec<Value> = listed
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|entry| {
                        if entry.is_object() {
                            json!({"name": entry["name"], "description": entry["description"], "argumentHint": hint_of(&entry["hints"])})
                        } else {
                            entry.clone()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(commands_from(&Value::Array(rows)))
    })();
    let _ = child.kill();
    let _ = child.wait();
    listed
}

#[cfg(test)]
#[path = "opencode_commands_case_tests.rs"]
mod tests;
