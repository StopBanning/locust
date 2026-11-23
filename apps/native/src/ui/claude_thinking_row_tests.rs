//! claude-thinking-appears-in-the-thinking-row.test.tsx (Electron 1697b996): the real Claude reader feeds the existing
//! thinking row; Claude needs no drawing of its own.

use super::*;
use locust_core::runtime::claude_events::ClaudeNormalizer;
use serde_json::json;

// case: renders the shown thinking from the adapter in one expandable row
#[test]
fn renders_the_shown_thinking_from_the_adapter_in_one_expandable_row() {
    let mut claude = ClaudeNormalizer::default();
    let events: Vec<_> = [
        json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}}),
        json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Read the configuration before changing the port."}}}),
        json!({"type":"stream_event","event":{"type":"content_block_stop","index":0}}),
    ]
    .iter()
    .flat_map(|v| claude.accept(v))
    .collect();
    // Through the ledger, as a run's events are kept and read back.
    let rows: String = events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            format!(
                "{}\n",
                json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":i + 1,"occurredAt":"2026-11-10T12:00:00Z","type":e.kind,"payload":e.payload}})
            )
        })
        .collect();
    let dir = std::env::temp_dir().join(format!("locust-claude-thought-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ledger.jsonl");
    std::fs::write(&path, rows).unwrap();
    let items = ledger::read_transcript(&path).items;
    let _ = std::fs::remove_dir_all(&dir);
    let thoughts: Vec<_> = items.iter().filter(|i| matches!(i, ledger::Item::Thought { .. })).collect();
    assert_eq!(thoughts.len(), 1, "{items:?}");
    let mut s = State::default();
    s.activity_open.insert(format!("conv/group/{}", activity_id(&items[0])), true);
    let mut rt = rune::Runtime::new(
        (s, items),
        |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"),
        (700.0, 500.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let said = rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(said.contains("Read the configuration before changing the port."), "{said}");
}
