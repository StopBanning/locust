// Carries packages/runtime-adapters/test/a-large-cursor-result-keeps-its-terminal-status.test.ts (Electron 4ce2f37a):
// Cursor repeats its answer in its `result` record, and an answer past the line limit took the run's ending with it.
use super::{
    cursor::{ProjectedOutput, ResultProjector},
    cursor_events::CursorNormalizer,
    process::read_line,
};
use serde_json::{Value, json};
use std::io::{BufReader, Read};

/// A pipe that hands over at most 997 bytes a read, as Electron's test feeds its runner.
struct Chunks {
    data: Vec<u8>,
    at: usize,
}
impl Read for Chunks {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(997).min(self.data.len() - self.at);
        buf[..n].copy_from_slice(&self.data[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

fn received(line: &str) -> String {
    let mut reader = BufReader::new(ProjectedOutput::new(Chunks { data: line.as_bytes().to_vec(), at: 0 }));
    let mut raw = Vec::new();
    read_line(&mut reader, &mut raw).unwrap();
    String::from_utf8(raw).unwrap()
}

#[test]
fn a_large_cursor_result_keeps_success_and_failure_across_chunks_and_without_a_line_end() {
    for is_error in [false, true] {
        let subtype = if is_error { "error" } else { "success" };
        let line = json!({ "type": "result", "result": format!("{}\n\"tail\"", "x".repeat(9 * 1024 * 1024)), "is_error": is_error,
            "subtype": subtype, "usage": { "inputTokens": 3, "outputTokens": 2 } })
        .to_string();
        let raw = received(&line);
        assert!(raw.len() < 70_000, "{} bytes", raw.len());
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!((parsed["type"].as_str(), parsed["is_error"].as_bool(), parsed["subtype"].as_str()), (Some("result"), Some(is_error), Some(subtype)));
        let mut cursor = CursorNormalizer::default();
        cursor.accept_line(&raw);
        let ended = cursor.finish(false, Some(0));
        assert_eq!(ended.payload["process"]["oversizedRecordsDropped"], 0);
        assert_eq!(ended.kind, if is_error { "run.failed" } else { "run.completed" }, "{}", ended.payload);
    }
}

// Rust's own: what the projector leaves alone.
#[test]
fn a_cursor_record_that_is_not_the_result_passes_through_whole() {
    let line = format!("{}\n", json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "y".repeat(100_000) }] } }));
    assert_eq!(received(&line), line);
}

#[test]
fn a_cursor_result_keeps_an_escape_whole_and_a_short_last_line() {
    let mut out = Vec::new();
    let mut projector = ResultProjector::default();
    let line = r#"{"type":"result","result":"aé\"b","is_error":false}"#;
    for piece in line.as_bytes().chunks(3) {
        projector.project(piece, &mut out);
    }
    projector.finish(&mut out);
    assert_eq!(String::from_utf8(out).unwrap(), line);
}

// Rust's own: a last line shorter than the projector's 64-byte look, with no line end, still reaches the reader.
#[test]
fn a_short_last_cursor_line_without_a_line_end_still_reaches_the_reader() {
    let line = r#"{"type":"result","is_error":false}"#;
    assert_eq!(received(line), line);
}

// Rust's own: a \u escape that would cross the 64 Ki cut is dropped whole, so the record still reads as JSON.
#[test]
fn a_unicode_escape_at_the_cut_is_dropped_whole() {
    let head = "x".repeat(64 * 1024 - 3);
    let line = format!(r#"{{"type":"result","result":"{head}\u00e9y","is_error":false}}"#);
    let parsed: Value = serde_json::from_str(&received(&line)).unwrap();
    assert_eq!(parsed["result"].as_str(), Some(head.as_str()));
    assert_eq!(parsed["is_error"], false);
}

// The same, through the driver's own reader: a Cursor process that sends a 9 MiB answer in its result, no line end.
#[test]
fn a_cursor_run_with_a_result_past_the_line_limit_still_completes() {
    use super::{Runtime, Turn, cursor::Cursor};
    use std::time::{Duration, Instant};
    let folder = std::env::temp_dir().join(format!("locust-large-cursor-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let script = "import json,sys\nsys.stdin.read()\nsys.stdout.write(json.dumps({'type':'result','subtype':'success','is_error':False,'result':'x'*(9*1024*1024)+'\\n\"tail\"'}))\nsys.stdout.flush()\n";
    let mut command = std::process::Command::new("python3");
    command.arg("-c").arg(script).current_dir(&folder);
    let mut cursor = Cursor::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Say it.".into(),
        folder: folder.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: crate::approval::Mode::Ask,
        approvals: None,
        mission_id: "m_large".into(),
        run_id: "r_large".into(),
    };
    cursor.start(&turn).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let ended = loop {
        assert!(Instant::now() < deadline, "the run did not end");
        if let Some(e) = cursor.next_event().unwrap()
            && matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled")
        {
            break e;
        }
    };
    let _ = std::fs::remove_dir_all(&folder);
    assert_eq!(ended.kind, "run.completed", "{}", ended.payload);
}
