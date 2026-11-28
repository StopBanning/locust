use super::*;
use locust_core::runtime::codex_voice::CallHost;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command, time::Instant};

struct Fake(PathBuf);
impl Drop for Fake {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
impl CallHost for Fake {
    fn discover(&self) -> Option<Command> {
        let mut command = Command::new("python3");
        command.env_clear()
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/fixtures/voice-app-server.py"))
            .arg(&self.0).arg("");
        Some(command)
    }
}

#[test]
fn b09_call_words_pressing_deny_declines_on_the_live_wire() {
    let root = std::env::temp_dir().join(format!("locust-b09-wire-{}", locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("notifications"), b"").unwrap();
    let fake = Arc::new(Fake(root.clone()));
    let service = Arc::new(VoiceCalls::new(fake.clone()));
    let options = CallOptions {
        folder: root.clone(), offer: "v=0\r\noffer".into(), teammate: None, voice: None,
        preview: None, connectors: true, anchor_turn: None,
    };
    let live = service.start(options).unwrap();
    live.started.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
    let (mut s, _unused) = fixture();
    s.voice.call.as_mut().unwrap().state = TalkState::Connected(Box::new(Peer), live, true);
    s.voice.service = Some(service.clone());
    let mut rt = thread(s);
    let request = json!({"id": 71, "method":"mcpServer/elicitation/request","params":{
        "threadId":"thread_1","serverName":"remote_notes","mode":"form",
        "_meta":{"codex_approval_kind":"mcp_tool_call","tool_params_display":[{"display_name":"text","value":"hello"}]},
        "message":"Allow the remote_notes MCP server to run tool \"send_note\"?"
    }});
    std::fs::write(root.join("notifications"), format!("{request}\n")).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while rt.state.voice.questions.is_empty() && Instant::now() < until {
        poll(&mut rt);
        std::thread::sleep(Duration::from_millis(10));
    }
    let question = rt.state.voice.questions.first().expect("real service delivered a question");
    let key = format!("voice-deny-{}", question.id);
    rt.update(|s, _| s.voice.questions[0].at = Instant::now() - Duration::from_secs(1));
    settle(&mut rt);
    let (x, y) = rt.find(&key).expect("Deny on the rendered thread card");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    let until = Instant::now() + Duration::from_secs(6);
    let mut answer = None;
    while answer.is_none() && Instant::now() < until {
        answer = std::fs::read_to_string(root.join("wire")).unwrap_or_default().lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|row| row["id"] == 71 && row.get("result").is_some());
        poll(&mut rt);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(answer.expect("the clicked Deny reached the running call")["result"], json!({"action":"decline"}));
    service.stop();
    let until = Instant::now() + Duration::from_secs(5);
    while rt.state.voice.call.is_some() && Instant::now() < until { poll(&mut rt); }
}
