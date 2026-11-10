//! THE LOCUST ENGINE, REACHED THROUGH ITS OWN DOOR.
//!
//! Locust Native draws a window; it runs no agent, keeps no ledger and answers no approval card. Everything it shows
//! comes from the Locust desktop app already running on this computer, through the local MCP server that app opens
//! when "Let your other AI apps use Locust" is on: `POST http://127.0.0.1:<port>/tools/call` with a bearer token, the
//! same door Claude Code and Codex use (`apps/desktop/resources/locust-mcp-bridge.mjs`, `locust-mcp-host.ts`).
//!
//! The connection file names the port and token. It is read fresh for every call, because Locust writes a new token
//! each time it starts or the switch is turned on, and the token is never logged, printed or kept anywhere else.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The file Locust writes beside its settings (`MCP_CONNECTION_FILE` in locust-mcp-host.ts).
pub const CONNECTION_FILE: &str = "locust-mcp-connection.json";
/// The longest `read_reply` waits inside one call (`MCP_WAIT_LIMIT_S` in locust-mcp-tools.ts).
pub const WAIT_LIMIT_S: u64 = 20;

/// What Locust's tools say, as plain text: Locust words its own answers and refusals for people.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolText {
    pub text: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Teammate {
    pub id: String,
    pub name: String,
    pub role: String,
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub busy: bool,
    pub runs_in: String,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub efforts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Models {
    pub teammate: Option<String>,
    pub runtime: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub models: Vec<Model>,
}

/// Both teammate replies and the all-agent array use the same agent/model rows.
pub fn parse_models(text: &str) -> Result<Vec<Models>, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| text.to_string())?;
    let rows = value
        .as_array()
        .cloned()
        .or_else(|| {
            value
                .get("agents")
                .or_else(|| value.get("runtimes"))
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_else(|| vec![value.clone()]);
    rows.iter()
        .map(|row| {
            let runtime = optional(row, "runtime")
                .ok_or_else(|| "Locust's models could not be read.".to_string())?;
            let models = row
                .get("models")
                .and_then(Value::as_array)
                .ok_or_else(|| "Locust's models could not be read.".to_string())?;
            Ok(Models {
                teammate: optional(row, "teammate"),
                runtime,
                model: optional(row, "model")
                    .or_else(|| optional(row, "current_model"))
                    .or_else(|| row.get("current").and_then(|v| optional(v, "model"))),
                effort: optional(row, "effort")
                    .or_else(|| optional(row, "current_effort"))
                    .or_else(|| row.get("current").and_then(|v| optional(v, "effort"))),
                models: models
                    .iter()
                    .filter_map(|m| {
                        let id = m
                            .as_str()
                            .map(str::to_string)
                            .or_else(|| optional(m, "id"))
                            .or_else(|| optional(m, "model"))?;
                        Some(Model {
                            name: optional(m, "name")
                                .or_else(|| optional(m, "label"))
                                .unwrap_or_else(|| id.clone()),
                            id,
                            efforts: m
                                .get("efforts")
                                .and_then(Value::as_array)
                                .map(|a| {
                                    a.iter()
                                        .filter_map(Value::as_str)
                                        .filter(|s| !s.is_empty())
                                        .map(str::to_string)
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                    })
                    .collect(),
            })
        })
        .collect()
}

/// One recent conversation, as `list_conversations` gives it: its newest turn, newest first.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationRow {
    pub id: String,
    pub teammate: String,
    pub title: String,
    /// "the person" or "an app".
    pub started_by: String,
    /// completed, running, failed, cancelled ... as the ledger says it.
    pub state: String,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Started {
    pub conversation_id: String,
    pub mode: String,
}

/// A conversation's newest turn, as `read_reply` gives it.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    /// Still running; the words say what it is doing.
    Working(String),
    /// A card waits in Locust's window for the person: only they can answer it, and only there.
    WaitingForYou(String),
    /// The finished answer.
    Answer(String),
    /// Locust's own words for why there is no answer.
    Failed(String),
}

/// Where Locust keeps its data on this platform (Electron's `userData` for the package `@teammate/desktop`).
pub fn default_data_folder() -> Option<PathBuf> {
    // An empty variable counts as unset, as Electron and the XDG rules read it (Argon's review, 8 Nov).
    let set = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    let home = set("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Application Support/@teammate/desktop"))
    } else if cfg!(target_os = "windows") {
        set("APPDATA").map(|a| PathBuf::from(a).join("@teammate").join("desktop"))
    } else {
        set("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".config")))
            .map(|c| c.join("@teammate/desktop"))
    }
}

/// The connection file: `LOCUST_MCP_CONNECTION` when set (tests, a second profile), else Locust's own.
pub fn connection_path() -> Option<PathBuf> {
    std::env::var_os("LOCUST_MCP_CONNECTION")
        .map(PathBuf::from)
        .or_else(|| default_data_folder().map(|f| f.join(CONNECTION_FILE)))
}

/// What a call says when nothing answers at the door (Locust not running).
pub const DOWN: &str = "Locust isn't running, or isn't taking requests from other apps right now. Open Locust and try again.";

pub const NOT_ON: &str = "Locust isn't taking requests from other apps. Open Locust, then in Settings, General, turn on \"Let your other AI apps use Locust\".";

#[derive(Debug, Clone)]
pub struct Engine {
    connection: PathBuf,
    agent: ureq::Agent,
}

impl Engine {
    pub fn new(connection: PathBuf) -> Self {
        // A read_reply may wait up to WAIT_LIMIT_S inside Locust; leave room for the answer to travel.
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout_read(Duration::from_secs(WAIT_LIMIT_S + 15))
            .build();
        Self { connection, agent }
    }

    /// The engine on this computer, or why it can't be reached.
    pub fn local() -> Result<Self, String> {
        connection_path()
            .map(Self::new)
            .ok_or_else(|| "Locust's data folder could not be found on this computer.".to_string())
    }

    fn door(&self) -> Result<(String, String), String> {
        read_door(&self.connection)
    }

    /// One tool call: Locust's own words back, or why the call didn't happen.
    pub fn call(&self, name: &str, arguments: Value) -> Result<ToolText, String> {
        let (url, token) = self.door()?;
        let response = self
            .agent
            .post(&url)
            .set("authorization", &format!("Bearer {token}"))
            .set("content-type", "application/json")
            .send_json(json!({ "name": name, "arguments": arguments }));
        let body: Value = match response {
            Ok(ok) => ok.into_json().map_err(|_| "Locust answered with something that isn't JSON.".to_string())?,
            Err(ureq::Error::Status(401, _)) => return Err("Locust refused the request: its connection has changed. Try again; if it keeps happening, restart Locust.".into()),
            Err(ureq::Error::Status(code, _)) => return Err(format!("Locust refused the request ({code}).")),
            Err(ureq::Error::Transport(_)) => return Err(DOWN.into()),
        };
        Ok(tool_text(&body))
    }

    pub fn teammates(&self) -> Result<Vec<Teammate>, String> {
        let said = self.call("list_teammates", json!({}))?;
        if said.is_error {
            return Err(said.text);
        }
        parse_teammates(&said.text)
    }

    /// Recent conversations in the folder Locust has open, the person's own too, when they let apps see them; else
    /// Locust's own words for why not (its "Let apps see your conversations" switch).
    pub fn conversations(&self) -> Result<Vec<ConversationRow>, String> {
        let said = self.call("list_conversations", json!({}))?;
        if said.is_error {
            return Err(said.text);
        }
        parse_conversations(&said.text)
    }

    pub fn start(
        &self,
        teammate_id: &str,
        message: &str,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<Started, String> {
        let mut args = json!({ "teammate": teammate_id, "message": message });
        turn_options(&mut args, model, effort);
        let said = self.call("start_conversation", args)?;
        parse_started(&said)
    }

    pub fn send(
        &self,
        conversation_id: &str,
        message: &str,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<Started, String> {
        let mut args = json!({ "conversation_id": conversation_id, "message": message });
        turn_options(&mut args, model, effort);
        let said = self.call("send_message", args)?;
        parse_started(&said)
    }

    pub fn list_models(&self, teammate: Option<&str>) -> Result<Vec<Models>, String> {
        let args = teammate
            .map(|id| json!({ "teammate": id }))
            .unwrap_or_else(|| json!({}));
        let said = self.call("list_models", args)?;
        if said.is_error {
            return Err(said.text);
        }
        parse_models(&said.text)
    }

    pub fn stop_turn(&self, id: &str) -> Result<String, String> {
        let said = self.call("stop_turn", json!({ "conversation_id": id }))?;
        if said.is_error {
            Err(said.text)
        } else {
            Ok(said.text)
        }
    }

    pub fn carry_on(&self, id: &str, runtime: &str, model: &str) -> Result<Started, String> {
        parse_started(&self.call(
            "carry_on",
            json!({ "conversation_id": id, "runtime": runtime, "model": model }),
        )?)
    }

    /// The newest turn; waits up to `wait_seconds` (at most WAIT_LIMIT_S) inside Locust for it to end.
    pub fn reply(&self, conversation_id: &str, wait_seconds: u64) -> Result<Reply, String> {
        let said = self.call("read_reply", json!({ "conversation_id": conversation_id, "wait_seconds": wait_seconds.min(WAIT_LIMIT_S) }))?;
        Ok(classify_reply(said))
    }
}

fn turn_options(args: &mut Value, model: Option<&str>, effort: Option<&str>) {
    if let Some(model) = model {
        args["model"] = json!(model);
    }
    if let Some(effort) = effort {
        args["effort"] = json!(effort);
    }
}

/// The url and token from the connection file. The token is checked for its shape and goes nowhere but the header.
pub fn read_door(path: &Path) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(path).map_err(|_| NOT_ON.to_string())?;
    let value: Value = serde_json::from_str(&text).map_err(|_| NOT_ON.to_string())?;
    let url = value.get("url").and_then(Value::as_str).unwrap_or_default();
    let token = value
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default();
    // The part between must be a port and nothing else: `http://127.0.0.1:80@elsewhere/tools/call` would carry the
    // token to another host (Argon's review, 8 Nov).
    let local = url
        .strip_prefix("http://127.0.0.1:")
        .and_then(|rest| rest.strip_suffix("/tools/call"))
        .is_some_and(|port| {
            !port.is_empty()
                && port.bytes().all(|b| b.is_ascii_digit())
                && port.parse::<u16>().is_ok()
        });
    let shaped = token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit());
    if !local || !shaped {
        return Err(NOT_ON.to_string());
    }
    Ok((url.to_string(), token.to_string()))
}

/// An MCP tool result's text: `{ content: [{ type: "text", text }], isError? }`.
pub fn tool_text(body: &Value) -> ToolText {
    let text = body
        .get("content")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    ToolText {
        text,
        is_error: body
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn optional(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn parse_teammates(text: &str) -> Result<Vec<Teammate>, String> {
    let list: Value = serde_json::from_str(text)
        .map_err(|_| "Locust's list of teammates could not be read.".to_string())?;
    let rows = list
        .as_array()
        .ok_or_else(|| "Locust's list of teammates could not be read.".to_string())?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            Some(Teammate {
                id: optional(row, "id")?,
                name: optional(row, "name")?,
                role: optional(row, "role").unwrap_or_default(),
                runtime: optional(row, "runtime"),
                model: optional(row, "model"),
                busy: row.get("busy").and_then(Value::as_bool).unwrap_or(false),
                runs_in: optional(row, "runs_in").unwrap_or_else(|| "ask".into()),
                folder: optional(row, "folder"),
            })
        })
        .collect())
}

pub fn parse_conversations(text: &str) -> Result<Vec<ConversationRow>, String> {
    // Off, Locust answers in words ("Off: this app sees only the conversations it started ..."): that is the reason.
    let list: Value = serde_json::from_str(text).map_err(|_| text.to_string())?;
    let rows = list.as_array().ok_or_else(|| text.to_string())?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            Some(ConversationRow {
                id: optional(row, "conversation_id")?,
                teammate: optional(row, "teammate").unwrap_or_default(),
                title: optional(row, "title").unwrap_or_else(|| "Untitled".into()),
                started_by: optional(row, "started_by").unwrap_or_default(),
                state: optional(row, "state").unwrap_or_default(),
                updated: optional(row, "updated").unwrap_or_default(),
            })
        })
        .collect())
}

pub fn parse_started(said: &ToolText) -> Result<Started, String> {
    if said.is_error {
        return Err(said.text.clone());
    }
    // A turn still starting answers in words ("Still working: starting the next turn.").
    let value: Value = serde_json::from_str(&said.text).map_err(|_| said.text.clone())?;
    Ok(Started {
        conversation_id: optional(&value, "conversation_id").ok_or_else(|| said.text.clone())?,
        mode: optional(&value, "mode").unwrap_or_else(|| "ask".into()),
    })
}

pub fn classify_reply(said: ToolText) -> Reply {
    if said.is_error {
        return Reply::Failed(said.text);
    }
    if let Some(rest) = said.text.strip_prefix("Still working") {
        return Reply::Working(rest.trim_start_matches(':').trim().to_string());
    }
    if said.text.starts_with("Waiting for the person") {
        return Reply::WaitingForYou(said.text);
    }
    Reply::Answer(said.text)
}

/// The agent a teammate runs on, as Locust's own picker names it.
pub fn runtime_name(runtime: &str) -> &str {
    match runtime {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "cursor" => "Cursor",
        "copilot" => "GitHub Copilot",
        "opencode" => "OpenCode",
        "antigravity" => "Antigravity",
        "muse" => "Muse Code",
        "kilo" => "Kilo",
        "acp" => "ACP agent",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// A one-request-at-a-time stand-in for Locust's door: checks the token, records each body, answers in turn.
    fn fake(answers: Vec<Value>) -> (PathBuf, Arc<Mutex<Vec<(String, Value)>>>, tempdir::Dir) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (stream, answer) in listener.incoming().zip(answers) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let (mut auth, mut length) = (String::new(), 0usize);
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("authorization: ") {
                        auth = line[15..].trim().to_string();
                        let _ = v;
                    }
                    if let Some(v) = lower.strip_prefix("content-length: ") {
                        length = v.trim().parse().unwrap();
                    }
                    if line.is_empty() || line == "\r\n" || line == "\n" {
                        break;
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                // Refused as Locust refuses it: a call without the right token gets 401, so a test can't pass without it.
                if auth != format!("Bearer {TOKEN}") {
                    write!(stream, "HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").unwrap();
                    continue;
                }
                log.lock()
                    .unwrap()
                    .push((auth, serde_json::from_slice(&body).unwrap()));
                let text = answer.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", text.len(), text).unwrap();
            }
        });
        let dir = tempdir::Dir::new();
        let path = dir.path.join(CONNECTION_FILE);
        std::fs::write(
            &path,
            json!({ "url": format!("http://127.0.0.1:{port}/tools/call"), "token": TOKEN })
                .to_string(),
        )
        .unwrap();
        (path, seen, dir)
    }

    mod tempdir {
        pub struct Dir {
            pub path: std::path::PathBuf,
        }
        impl Dir {
            pub fn new() -> Self {
                let path = std::env::temp_dir().join(format!(
                    "locust-native-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    fn said(text: &str) -> Value {
        json!({ "content": [{ "type": "text", "text": text }] })
    }

    #[test]
    fn lists_the_team_with_the_token_in_the_header_and_nowhere_else() {
        let team = r#"[{"id":"tm_1","name":"Codec","role":"Code & Migrations","folder":"now-coding","runtime":"codex","model":"gpt-6.1-sol","effort":"high","busy":false,"runs_in":"accept-edits"},{"id":"tm_2","name":"Scout","role":"Custom","runtime":null,"model":null,"busy":true,"runs_in":"ask"}]"#;
        let (path, seen, _dir) = fake(vec![said(team)]);
        let engine = Engine::new(path);
        let teammates = engine.teammates().unwrap();
        assert_eq!(teammates.len(), 2);
        assert_eq!(teammates[0].name, "Codec");
        assert_eq!(teammates[0].runtime.as_deref(), Some("codex"));
        assert_eq!(teammates[1].runtime, None);
        assert!(teammates[1].busy);
        let seen = crate::invariants::lock(&seen);
        assert_eq!(seen[0].0, format!("Bearer {TOKEN}"));
        assert_eq!(
            seen[0].1,
            json!({ "name": "list_teammates", "arguments": {} })
        );
        assert!(
            !seen[0].1.to_string().contains(TOKEN),
            "the token rides only in the header"
        );
    }

    #[test]
    fn starts_a_conversation_then_reads_working_waiting_and_the_answer() {
        let (path, seen, _dir) = fake(vec![
            said(r#"{"conversation_id":"mission_9","mode":"ask","folder":"now-coding"}"#),
            said("Still working: Reading run.py"),
            said(
                "Waiting for the person to approve or deny an action in Locust's window. This app cannot answer it; read_reply again later.",
            ),
            said("## Done\nAll set."),
        ]);
        let engine = Engine::new(path);
        let started = engine.start("tm_1", "Look at run.py", None, None).unwrap();
        assert_eq!(
            started,
            Started {
                conversation_id: "mission_9".into(),
                mode: "ask".into()
            }
        );
        assert_eq!(
            engine.reply("mission_9", 99).unwrap(),
            Reply::Working("Reading run.py".into())
        );
        assert!(matches!(
            engine.reply("mission_9", 5).unwrap(),
            Reply::WaitingForYou(_)
        ));
        assert_eq!(
            engine.reply("mission_9", 5).unwrap(),
            Reply::Answer("## Done\nAll set.".into())
        );
        let seen = crate::invariants::lock(&seen);
        assert_eq!(
            seen[1].1["arguments"]["wait_seconds"],
            json!(WAIT_LIMIT_S),
            "a wait is never longer than Locust's limit"
        );
    }

    #[test]
    fn says_locust_is_off_when_there_is_no_door_and_refusals_come_back_in_locusts_words() {
        let missing = Engine::new(
            std::env::temp_dir()
                .join("no-such-locust")
                .join(CONNECTION_FILE),
        );
        assert_eq!(missing.teammates().unwrap_err(), NOT_ON);
        let refusal = json!({ "content": [{ "type": "text", "text": "Choose a teammate and give it a message of up to 8,000 characters. Nothing was started." }], "isError": true });
        let (path, _seen, _dir) = fake(vec![refusal]);
        let err = Engine::new(path).start("", "", None, None).unwrap_err();
        assert!(err.contains("Nothing was started"));
    }

    #[test]
    fn a_door_that_is_not_local_or_a_token_of_the_wrong_shape_is_refused() {
        let dir = tempdir::Dir::new();
        let path = dir.path.join(CONNECTION_FILE);
        std::fs::write(
            &path,
            json!({ "url": "http://example.com/tools/call", "token": TOKEN }).to_string(),
        )
        .unwrap();
        assert_eq!(read_door(&path).unwrap_err(), NOT_ON);
        std::fs::write(
            &path,
            json!({ "url": "http://127.0.0.1:5/tools/call", "token": "short" }).to_string(),
        )
        .unwrap();
        assert_eq!(read_door(&path).unwrap_err(), NOT_ON);
        // A host smuggled in after the port, or no port at all.
        for url in [
            "http://127.0.0.1:80@example.com/tools/call",
            "http://127.0.0.1:/tools/call",
            "http://127.0.0.1:99999/tools/call",
            "http://127.0.0.1:80/x/tools/call",
        ] {
            std::fs::write(&path, json!({ "url": url, "token": TOKEN }).to_string()).unwrap();
            assert_eq!(read_door(&path).unwrap_err(), NOT_ON, "{url}");
        }
        std::fs::write(
            &path,
            json!({ "url": "http://127.0.0.1:64838/tools/call", "token": TOKEN }).to_string(),
        )
        .unwrap();
        assert!(
            read_door(&path).is_ok(),
            "the control: a real door is taken"
        );
    }

    #[test]
    fn reads_recent_conversations_or_says_why_it_cannot() {
        let rows = parse_conversations(r#"[{"conversation_id":"mission_1","teammate":"Scout","title":"Hello","started_by":"an app","runtime":"opencode","model":"m","state":"completed","updated":"2026-11-08T21:23:37.380Z"}]"#).unwrap();
        assert_eq!(
            rows,
            vec![ConversationRow {
                id: "mission_1".into(),
                teammate: "Scout".into(),
                title: "Hello".into(),
                started_by: "an app".into(),
                state: "completed".into(),
                updated: "2026-11-08T21:23:37.380Z".into()
            }]
        );
        let off = "Off: this app sees only the conversations it started. The person can turn on \"Let apps see your conversations\" in Locust's Settings.";
        assert_eq!(parse_conversations(off).unwrap_err(), off);
    }

    #[test]
    fn daily_controls_use_the_mcp_door_and_optional_fields() {
        let (path, seen, _dir) = fake(vec![
            said(
                r#"{"runtime":"sample","model":"small","effort":"high","models":[{"id":"small","efforts":["high"]}]}"#,
            ),
            said("[]"),
            said(r#"{"conversation_id":"synthetic-one"}"#),
            said(r#"{"conversation_id":"synthetic-one"}"#),
            said("Stopped."),
            said(r#"{"conversation_id":"synthetic-two"}"#),
        ]);
        let e = Engine::new(path);
        assert_eq!(
            e.list_models(Some("synthetic-agent")).expect("models")[0]
                .model
                .as_deref(),
            Some("small")
        );
        assert!(e.list_models(None).expect("all agents").is_empty());
        e.start(
            "synthetic-agent",
            "Synthetic prompt",
            Some("small"),
            Some("high"),
        )
        .expect("start");
        e.send("synthetic-one", "Synthetic follow-up", None, None)
            .expect("send");
        e.stop_turn("synthetic-one").expect("stop");
        assert_eq!(
            e.carry_on("synthetic-one", "sample", "small")
                .expect("carry")
                .conversation_id,
            "synthetic-two"
        );
        let calls = seen.lock().expect("calls");
        assert_eq!(
            calls[0].1["arguments"],
            json!({"teammate":"synthetic-agent"})
        );
        assert_eq!(calls[1].1["arguments"], json!({}));
        assert_eq!(calls[2].1["arguments"]["effort"], "high");
        assert!(calls[3].1["arguments"].get("model").is_none());
        assert_eq!(
            calls[4].1,
            json!({"name":"stop_turn","arguments":{"conversation_id":"synthetic-one"}})
        );
        assert_eq!(
            calls[5].1,
            json!({"name":"carry_on","arguments":{"conversation_id":"synthetic-one","runtime":"sample","model":"small"}})
        );
    }

    /// Against the Locust running on this computer: `cargo test -- --ignored real_locust`.
    #[test]
    #[ignore]
    fn real_locust_lists_its_team() {
        assert_eq!(
            std::env::var("LOCUST_REAL_LOCUST").ok().as_deref(),
            Some("1"),
            "set LOCUST_REAL_LOCUST=1 to run this real test"
        );
        let team = Engine::local().unwrap().teammates().unwrap();
        assert!(!team.is_empty());
        for t in &team {
            println!(
                "{} · {} · {}",
                t.name,
                t.runtime.as_deref().map(runtime_name).unwrap_or("no agent"),
                t.model.as_deref().unwrap_or("default")
            );
        }
    }

    #[test]
    fn names_each_agent_as_locust_does() {
        assert_eq!(runtime_name("codex"), "Codex");
        assert_eq!(runtime_name("antigravity"), "Antigravity");
        assert_eq!(runtime_name("newthing"), "newthing");
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    #[test]
    fn parses_teammate_models_efforts_and_current_selection() {
        let a = parse_models(r#"{"runtime":"sample","model":"small","effort":"high","models":[{"id":"small","name":"Small","efforts":["low","high"]}]}"#).expect("synthetic models");
        assert_eq!(a[0].teammate, None);
        assert_eq!(a[0].model.as_deref(), Some("small"));
        assert_eq!(a[0].effort.as_deref(), Some("high"));
        assert_eq!(a[0].models[0].efforts, ["low", "high"]);
        assert_eq!(a[0].models[0].name, "Small");
        assert!(parse_models("unavailable").is_err());
        assert!(parse_models("{}").is_err());
        assert!(parse_models(r#"{"models":[]}"#).is_err());
    }
    #[test]
    fn parses_all_agents_and_skips_malformed_model_rows() {
        let a = parse_models(r#"[{"runtime":"first","models":[null,{"id":"one"}]},{"runtime":"second","models":[{"id":"two","efforts":[]}]}]"#).expect("synthetic agents");
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].models.len(), 1);
        assert_eq!(a[1].runtime, "second");
        assert!(a[1].models[0].efforts.is_empty());
    }
    #[test]
    fn parses_live_teammate_and_all_runtime_list_models_shapes() {
        let teammate_reply = r#"{"teammate":"Scout","runtime":"opencode","model":"opencode/deepseek-v4.1-flash-free","effort":null,"models":[{"id":"account-default","name":"Account default","efforts":[]},{"id":"opencode/beam-free","name":"beam-free","efforts":["low","medium","high","xhigh","max"]},{"id":"opencode/deepseek-v4.1-flash-free","name":"deepseek-v4.1-flash-free","efforts":["low","high","max"]}]}"#;
        let parsed = parse_models(teammate_reply).expect("live teammate shape");
        assert_eq!(
            parsed,
            vec![Models {
                teammate: Some("Scout".into()),
                runtime: "opencode".into(),
                model: Some("opencode/deepseek-v4.1-flash-free".into()),
                effort: None,
                models: vec![
                    Model {
                        id: "account-default".into(),
                        name: "Account default".into(),
                        efforts: vec![]
                    },
                    Model {
                        id: "opencode/beam-free".into(),
                        name: "beam-free".into(),
                        efforts: vec![
                            "low".into(),
                            "medium".into(),
                            "high".into(),
                            "xhigh".into(),
                            "max".into()
                        ],
                    },
                    Model {
                        id: "opencode/deepseek-v4.1-flash-free".into(),
                        name: "deepseek-v4.1-flash-free".into(),
                        efforts: vec!["low".into(), "high".into(), "max".into()],
                    },
                ],
            }]
        );

        let all_reply = r#"[{"runtime":"codex","models":[{"id":"account-default","name":"Account default"},{"id":"gpt-6.1-sol","name":"GPT-6.1-Sol"}]},{"runtime":"claude","models":[{"id":"account-default","name":"Account default"},{"id":"sonnet","name":"Sonnet 5.5"}]},{"runtime":"kilo","models":[{"id":"kilo/kilo-auto/free","name":"Kilo Auto Free"}]}]"#;
        let all = parse_models(all_reply).expect("live all-runtime shape");
        assert_eq!(
            all,
            vec![
                Models {
                    teammate: None,
                    runtime: "codex".into(),
                    model: None,
                    effort: None,
                    models: vec![
                        Model {
                            id: "account-default".into(),
                            name: "Account default".into(),
                            efforts: vec![]
                        },
                        Model {
                            id: "gpt-6.1-sol".into(),
                            name: "GPT-6.1-Sol".into(),
                            efforts: vec![]
                        },
                    ],
                },
                Models {
                    teammate: None,
                    runtime: "claude".into(),
                    model: None,
                    effort: None,
                    models: vec![
                        Model {
                            id: "account-default".into(),
                            name: "Account default".into(),
                            efforts: vec![]
                        },
                        Model {
                            id: "sonnet".into(),
                            name: "Sonnet 5.5".into(),
                            efforts: vec![]
                        },
                    ],
                },
                Models {
                    teammate: None,
                    runtime: "kilo".into(),
                    model: None,
                    effort: None,
                    models: vec![Model {
                        id: "kilo/kilo-auto/free".into(),
                        name: "Kilo Auto Free".into(),
                        efforts: vec![]
                    },],
                },
            ]
        );
    }
    #[test]
    fn sends_only_explicit_turn_options_and_uses_existing_control_tools() {
        let mut args = json!({});
        turn_options(&mut args, None, None);
        assert_eq!(args, json!({}));
        turn_options(&mut args, Some("sample"), Some("high"));
        assert_eq!(args, json!({"model":"sample","effort":"high"}));
    }
}
