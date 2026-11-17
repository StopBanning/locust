//! Electron `main/a-session-that-ended-is-not-resumed.test.ts` (M7): OpenCode's "the next message starts a fresh
//! session" was not true while the host resumed the same dead session. A failure marked `sessionEnded` leaves nothing
//! to resume. Native's `prior_of` ports `runtimeThreadIdOf`.
use super::*;

fn turn(t: &Temp, id: &str, events: &[(&str, Value)]) {
    let mut w = LedgerWriter::create_continuing(&t.store(), id, "p", "m", "x", None).unwrap();
    for (kind, payload) in events {
        w.append(kind, payload.clone(), None).unwrap();
    }
}

#[test]
fn a_failure_that_ended_the_session_leaves_none_to_resume_though_it_names_the_session() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), None);
    turn(
        &t,
        "mission_1",
        &[
            ("run.started", json!({"runtimeThreadId": "ses_1"})),
            ("run.failed", json!({"runtimeThreadId": "ses_1", "sessionEnded": true, "kind": "unknown", "message": "x"})),
        ],
    );
    assert_eq!(prior_of("mission_1").unwrap().thread, None);
}

#[test]
fn after_any_other_failure_the_last_session_named_is_still_resumed() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), None);
    turn(
        &t,
        "mission_1",
        &[
            ("run.started", json!({"runtimeThreadId": "ses_1"})),
            ("run.failed", json!({"runtimeThreadId": "ses_1", "kind": "unknown", "message": "x"})),
        ],
    );
    assert_eq!(prior_of("mission_1").unwrap().thread.as_deref(), Some("ses_1"));
}
