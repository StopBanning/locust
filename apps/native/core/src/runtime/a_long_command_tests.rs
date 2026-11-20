//! Electron's `a-long-command-does-not-stop-the-run.test.ts` (0.610). A teammate wrote a page through one Bash heredoc
//! of 29,322 characters; Claude Code's adapter announced the call again once its input was whole, with the command
//! unbounded, and Electron's ledger refused the event and stopped the run. The whole path is checked here: Claude
//! Code's stream records, with a command longer than that one, through `ClaudeNormalizer` into a real ledger file.
//!
//! Rust's ledger writer does not refuse a field over 16,384 characters (it refuses a record over 512 KiB), so the bound
//! is the normalizer's to keep: every event that names the command keeps it within 16,384 UTF-16 units, both ends
//! kept, and every event is written and read back with the run still going.
use super::*;
use crate::{ledger_write::LedgerWriter, store::Store};

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn writer() -> (Temp, LedgerWriter) {
    let root = std::env::temp_dir().join(format!("locust-long-command-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let w = LedgerWriter::create_runtime(&store, "mission_long_command", "Make a landing page in one file.", "fable", "claude", "claude", &[], None).unwrap();
    (Temp(root), w)
}

// case: is recorded, cut to fit with both ends kept, and the run goes on
#[test]
fn a_claude_code_call_with_a_very_long_command_is_recorded_cut_to_fit_with_both_ends_kept_and_the_run_goes_on() {
    let (_t, mut w) = writer();
    let page = format!("<!doctype html>\n{}", "<p class=\"plan\">Coffee, every week.</p>\n".repeat(1200));
    let command = format!("cat > index.html <<'EOF'\n{page}EOF");
    // The control: the command really is past the ledger's limit.
    assert!(command.encode_utf16().count() > 16_384);

    let mut claude = ClaudeNormalizer::default();
    let mut events = Vec::new();
    for record in [
        json!({"type": "system", "subtype": "init", "session_id": "s1"}),
        json!({"type": "stream_event", "event": {"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "toolu_1", "name": "Bash"}}}),
        json!({"type": "assistant", "message": {"id": "msg_1", "content": [{"type": "tool_use", "id": "toolu_1", "name": "Bash", "input": {"command": command, "description": "Write the landing page"}}]}}),
        json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": ""}]}}),
    ] {
        events.extend(claude.accept(&record));
    }

    let named: Vec<&Event> = events
        .iter()
        .filter(|e| (e.kind == "tool.started" || e.kind == "tool.completed") && e.payload["command"].is_string())
        .collect();
    // The restated start, once the input is whole, and the call's end both name it.
    assert!(named.iter().any(|e| e.kind == "tool.started"), "{:?}", events.iter().map(|e| &e.kind).collect::<Vec<_>>());
    assert!(named.iter().any(|e| e.kind == "tool.completed"), "{:?}", events.iter().map(|e| &e.kind).collect::<Vec<_>>());
    for event in &named {
        let kept = event.payload["command"].as_str().unwrap();
        assert!(kept.encode_utf16().count() <= 16_384, "{} kept {} units", event.kind, kept.encode_utf16().count());
        // Both ends: what it ran, and how it ended.
        assert!(kept.starts_with("cat > index.html <<'EOF'"), "{}", event.kind);
        assert!(kept.ends_with("EOF"), "{}", event.kind);
    }

    for event in &events {
        w.append(&event.kind, event.payload.clone(), event.thread.as_deref()).unwrap();
    }
    let back = crate::ledger::read_transcript(&w.path);
    assert_eq!(back.error, None);
    assert!(!back.failed, "the run is not stopped");
    let written = std::fs::read_to_string(&w.path).unwrap().lines().filter(|l| l.contains("\"mission.event\"")).count();
    assert_eq!(written, events.len());
}
