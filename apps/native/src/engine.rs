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
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Application Support/@teammate/desktop"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("@teammate").join("desktop"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home.map(|h| h.join(".config"))).map(|c| c.join("@teammate/desktop"))
    }
}

/// The connection file: `LOCUST_MCP_CONNECTION` when set (tests, a second profile), else Locust's own.
pub fn connection_path() -> Option<PathBuf> {
    std::env::var_os("LOCUST_MCP_CONNECTION").map(PathBuf::from).or_else(|| default_data_folder().map(|f| f.join(CONNECTION_FILE)))
}

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
        connection_path().map(Self::new).ok_or_else(|| "Locust's data folder could not be found on this computer.".to_string())
    }

    fn door(&self) -> Result<(String, String), String> {
        read_door(&self.connection)
    }

    /// One tool call: Locust's own words back, or why the call didn't happen.
    pub fn call(&self, name: &str, arguments: Value) -> Result<ToolText, String> {
        let (url, token) = self.door()?;
        let response = self.agent.post(&url)
            .set("authorization", &format!("Bearer {token}"))
            .set("content-type", "application/json")
            .send_json(json!({ "name": name, "arguments": arguments }));
        let body: Value = match response {
            Ok(ok) => ok.into_json().map_err(|_| "Locust answered with something that isn't JSON.".to_string())?,
            Err(ureq::Error::Status(401, _)) => return Err("Locust refused the request: its connection has changed. Try again; if it keeps happening, restart Locust.".into()),
            Err(ureq::Error::Status(code, _)) => return Err(format!("Locust refused the request ({code}).")),
            Err(ureq::Error::Transport(_)) => return Err("Locust isn't running, or isn't taking requests from other apps right now. Open Locust and try again.".into()),
        };
        Ok(tool_text(&body))
    }

    pub fn teammates(&self) -> Result<Vec<Teammate>, String> {
        let said = self.call("list_teammates", json!({}))?;
        if said.is_error { return Err(said.text); }
        parse_teammates(&said.text)
    }

    pub fn start(&self, teammate_id: &str, message: &str) -> Result<Started, String> {
        let said = self.call("start_conversation", json!({ "teammate": teammate_id, "message": message }))?;
        parse_started(&said)
    }

    pub fn send(&self, conversation_id: &str, message: &str) -> Result<Started, String> {
        let said = self.call("send_message", json!({ "conversation_id": conversation_id, "message": message }))?;
        parse_started(&said)
    }

    /// The newest turn; waits up to `wait_seconds` (at most WAIT_LIMIT_S) inside Locust for it to end.
    pub fn reply(&self, conversation_id: &str, wait_seconds: u64) -> Result<Reply, String> {
        let said = self.call("read_reply", json!({ "conversation_id": conversation_id, "wait_seconds": wait_seconds.min(WAIT_LIMIT_S) }))?;
        Ok(classify_reply(said))
    }
}

/// The url and token from the connection file. The token is checked for its shape and goes nowhere but the header.
pub fn read_door(path: &Path) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(path).map_err(|_| NOT_ON.to_string())?;
    let value: Value = serde_json::from_str(&text).map_err(|_| NOT_ON.to_string())?;
    let url = value.get("url").and_then(Value::as_str).unwrap_or_default();
    let token = value.get("token").and_then(Value::as_str).unwrap_or_default();
    let local = url.starts_with("http://127.0.0.1:") && url.ends_with("/tools/call");
    let shaped = token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit());
    if !local || !shaped { return Err(NOT_ON.to_string()); }
    Ok((url.to_string(), token.to_string()))
}

/// An MCP tool result's text: `{ content: [{ type: "text", text }], isError? }`.
pub fn tool_text(body: &Value) -> ToolText {
    let text = body.get("content").and_then(Value::as_array).map(|parts| {
        parts.iter().filter_map(|part| part.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n")
    }).unwrap_or_default();
    ToolText { text, is_error: body.get("isError").and_then(Value::as_bool).unwrap_or(false) }
}

fn optional(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn parse_teammates(text: &str) -> Result<Vec<Teammate>, String> {
    let list: Value = serde_json::from_str(text).map_err(|_| "Locust's list of teammates could not be read.".to_string())?;
    let rows = list.as_array().ok_or_else(|| "Locust's list of teammates could not be read.".to_string())?;
    Ok(rows.iter().filter_map(|row| {
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
    }).collect())
}

pub fn parse_started(said: &ToolText) -> Result<Started, String> {
    if said.is_error { return Err(said.text.clone()); }
    // A turn still starting answers in words ("Still working: starting the next turn.").
    let value: Value = serde_json::from_str(&said.text).map_err(|_| said.text.clone())?;
    Ok(Started {
        conversation_id: optional(&value, "conversation_id").ok_or_else(|| said.text.clone())?,
        mode: optional(&value, "mode").unwrap_or_else(|| "ask".into()),
    })
}

pub fn classify_reply(said: ToolText) -> Reply {
    if said.is_error { return Reply::Failed(said.text); }
    if let Some(rest) = said.text.strip_prefix("Still working") {
        return Reply::Working(rest.trim_start_matches(':').trim().to_string());
    }
    if said.text.starts_with("Waiting for the person") { return Reply::WaitingForYou(said.text); }
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
                    if let Some(v) = lower.strip_prefix("authorization: ") { auth = line[15..].trim().to_string(); let _ = v; }
                    if let Some(v) = lower.strip_prefix("content-length: ") { length = v.trim().parse().unwrap(); }
                    if line == "\r\n" { break; }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                log.lock().unwrap().push((auth, serde_json::from_slice(&body).unwrap()));
                let text = answer.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", text.len(), text).unwrap();
            }
        });
        let dir = tempdir::Dir::new();
        let path = dir.path.join(CONNECTION_FILE);
        std::fs::write(&path, json!({ "url": format!("http://127.0.0.1:{port}/tools/call"), "token": TOKEN }).to_string()).unwrap();
        (path, seen, dir)
    }

    mod tempdir {
        pub struct Dir { pub path: std::path::PathBuf }
        impl Dir {
            pub fn new() -> Self {
                let path = std::env::temp_dir().join(format!("locust-native-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }
        }
        impl Drop for Dir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.path); } }
    }

    fn said(text: &str) -> Value { json!({ "content": [{ "type": "text", "text": text }] }) }

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
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0].0, format!("Bearer {TOKEN}"));
        assert_eq!(seen[0].1, json!({ "name": "list_teammates", "arguments": {} }));
        assert!(!seen[0].1.to_string().contains(TOKEN), "the token rides only in the header");
    }

    #[test]
    fn starts_a_conversation_then_reads_working_waiting_and_the_answer() {
        let (path, seen, _dir) = fake(vec![
            said(r#"{"conversation_id":"mission_9","mode":"ask","folder":"now-coding"}"#),
            said("Still working: Reading run.py"),
            said("Waiting for the person to approve or deny an action in Locust's window. This app cannot answer it; read_reply again later."),
            said("## Done\nAll set."),
        ]);
        let engine = Engine::new(path);
        let started = engine.start("tm_1", "Look at run.py").unwrap();
        assert_eq!(started, Started { conversation_id: "mission_9".into(), mode: "ask".into() });
        assert_eq!(engine.reply("mission_9", 99).unwrap(), Reply::Working("Reading run.py".into()));
        assert!(matches!(engine.reply("mission_9", 5).unwrap(), Reply::WaitingForYou(_)));
        assert_eq!(engine.reply("mission_9", 5).unwrap(), Reply::Answer("## Done\nAll set.".into()));
        let seen = seen.lock().unwrap();
        assert_eq!(seen[1].1["arguments"]["wait_seconds"], json!(WAIT_LIMIT_S), "a wait is never longer than Locust's limit");
    }

    #[test]
    fn says_locust_is_off_when_there_is_no_door_and_refusals_come_back_in_locusts_words() {
        let missing = Engine::new(std::env::temp_dir().join("no-such-locust").join(CONNECTION_FILE));
        assert_eq!(missing.teammates().unwrap_err(), NOT_ON);
        let refusal = json!({ "content": [{ "type": "text", "text": "Choose a teammate and give it a message of up to 8,000 characters. Nothing was started." }], "isError": true });
        let (path, _seen, _dir) = fake(vec![refusal]);
        let err = Engine::new(path).start("", "").unwrap_err();
        assert!(err.contains("Nothing was started"));
    }

    #[test]
    fn a_door_that_is_not_local_or_a_token_of_the_wrong_shape_is_refused() {
        let dir = tempdir::Dir::new();
        let path = dir.path.join(CONNECTION_FILE);
        std::fs::write(&path, json!({ "url": "http://example.com/tools/call", "token": TOKEN }).to_string()).unwrap();
        assert_eq!(read_door(&path).unwrap_err(), NOT_ON);
        std::fs::write(&path, json!({ "url": "http://127.0.0.1:5/tools/call", "token": "short" }).to_string()).unwrap();
        assert_eq!(read_door(&path).unwrap_err(), NOT_ON);
    }

    /// Against the Locust running on this computer: `cargo test -- --ignored real_locust`.
    #[test]
    #[ignore]
    fn real_locust_lists_its_team() {
        let team = Engine::local().unwrap().teammates().unwrap();
        assert!(!team.is_empty());
        for t in &team { println!("{} · {} · {}", t.name, t.runtime.as_deref().map(runtime_name).unwrap_or("no agent"), t.model.as_deref().unwrap_or("default")); }
    }

    #[test]
    fn names_each_agent_as_locust_does() {
        assert_eq!(runtime_name("codex"), "Codex");
        assert_eq!(runtime_name("antigravity"), "Antigravity");
        assert_eq!(runtime_name("newthing"), "newthing");
    }
}
