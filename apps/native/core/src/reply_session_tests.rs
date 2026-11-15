//! missionView's three resumableSessionOf cases, through native's record reader.
use crate::runtime_terminal::session;
use serde_json::{json, Value};
fn event(ty: &str, payload: Value) -> Value { json!({"event":{"type":ty,"payload":payload}}) }
fn started(id: Option<&str>) -> Value { event("run.started", json!({"runtimeThreadId":id.unwrap_or("")})) }
#[test]
fn names_the_session_a_reply_would_resume() {
    assert_eq!(session(&[started(Some("thread-7")),event("run.completed",json!({}))]).as_deref(),Some("thread-7"));
}
#[test]
fn has_nothing_to_resume_when_the_run_failed_before_its_runtime_started() {
    assert_eq!(session(&[event("run.failed",json!({"kind":"process-failed","message":"Codex CLI is not ready."}))]),None);
    assert_eq!(session(&[]),None);
}
#[test]
fn ignores_an_empty_session_id_rather_than_treating_it_as_one() {
    assert_eq!(session(&[started(None)]),None);
}
