//! Experimental Antigravity transcript normalizer (antigravity-events.ts).
//! The live native driver uses agy_events + the agy CLI; this covers the transcript fixtures.
use super::Event;
use crate::runtime::events::text;
use serde_json::{Value, json};
use std::collections::HashSet;

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn obj(v: &Value) -> bool {
    v.is_object()
}

pub struct TranscriptNormalizer {
    seen: HashSet<i64>,
    count: usize,
    pub latest_final: bool,
}

impl Default for TranscriptNormalizer {
    fn default() -> Self {
        Self {
            seen: HashSet::new(),
            count: 0,
            latest_final: false,
        }
    }
}

impl TranscriptNormalizer {
    fn event(&mut self, kind: &str, payload: Value) -> Event {
        self.count += 1;
        Event {
            kind: kind.into(),
            payload,
            thread: None,
        }
    }
    pub fn accept_line(&mut self, raw: &str) -> Vec<Event> {
        let parsed: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        if !obj(&parsed) {
            return vec![];
        }
        let status = s(&parsed, "status");
        if status != "DONE" && !status.is_empty() {
            return vec![];
        }
        let index = parsed["step_index"].as_i64().unwrap_or(-1);
        if index >= 0 && !self.seen.insert(index) {
            return vec![];
        }
        let typ = s(&parsed, "type");
        let source = s(&parsed, "source");
        if typ == "USER_INPUT" || typ == "CHECKPOINT" {
            return vec![];
        }
        if typ == "SYSTEM_MESSAGE" || source == "SYSTEM" {
            // Host follow-up echo — no event.
            if s(&parsed, "content").contains("SYSTEM_MESSAGE") || typ == "SYSTEM_MESSAGE" {
                return vec![];
            }
        }
        if typ == "PLANNER_RESPONSE" {
            let has_tools = parsed["tool_calls"]
                .as_array()
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let content = s(&parsed, "content");
            if !has_tools && !content.is_empty() {
                self.latest_final = true;
                return vec![self.event(
                    "message.delta",
                    json!({
                        "itemId": format!("msg_{index}"),
                        "operation": "replace",
                        "text": text(content, 131072),
                        "final": true,
                        "evidence": {"runtimeEventType":"PLANNER_RESPONSE","redacted":true}
                    }),
                )];
            }
            // Reasoning-only: step without text.
            if parsed.get("thinking").is_some() {
                return vec![
                    self.event("step.started", json!({"stepKind":"reasoning","evidence":{"runtimeEventType":"PLANNER_RESPONSE","redacted":true}})),
                    self.event("step.completed", json!({"stepKind":"reasoning","evidence":{"runtimeEventType":"PLANNER_RESPONSE","redacted":true}})),
                ];
            }
        }
        if typ == "GENERIC" {
            return vec![self.event("adapter.diagnostic", json!({
                "level":"info","code":"antigravity.tool_result","message":"Antigravity tool result (transcript).",
                "terminal":false,"evidence":{"runtimeEventType":"GENERIC","redacted":true}
            }))];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lines(name: &str) -> Vec<String> {
        std::fs::read_to_string(format!(
            "{}/../scripts/fixtures/antigravity/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
    }
    #[test]
    fn plain_reply_fixture_yields_final_message() {
        let mut n = TranscriptNormalizer::default();
        let mut events = Vec::new();
        for line in lines("transcript-plain-reply.jsonl") {
            events.extend(n.accept_line(&line));
        }
        assert!(n.latest_final);
        assert!(
            events
                .iter()
                .any(|e| e.kind == "message.delta" && e.payload["final"] == true)
        );
    }
}
