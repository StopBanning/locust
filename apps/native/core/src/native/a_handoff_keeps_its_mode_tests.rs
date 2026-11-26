//! Electron `main/a-handoff-keeps-its-mode.test.ts` (M26), the two cases about how a mode is read: one Locust does not
//! know (missing, malformed) is read-only Ask, read by `requested_mode` (mission-mode.ts `parsedMissionMode`), the one
//! reading a start (`send_from`, for the teammate's own mode when the window names none), a handoff and a resume
//! (`carry_with`) share. A handoff or a resume reads the stopped turn's recorded mode, and a record whose mode is not one
//! Locust knows is never written and is not read (the ledger refuses it), so it is refused there, never widened.
use super::provider_carry_on::{carried_from, stopped_in, team};
use super::*;
use crate::carry_on::CarryOnAlso;

fn started_ev() -> Value {
    json!({"type": "run.started", "payload": {}})
}

fn quota_ev() -> Value {
    json!({"type": "route.limit_detected", "payload": {"kind": "quota-exhausted"}})
}

/// A start on the one teammate `team` writes, whose own saved mode is `runs_in`, naming no mode of its own.
fn start_with_own_mode(t: &Temp, runs_in: &str) -> Value {
    let job = Send {
        teammate: crate::engine::Teammate {
            id: "tm_carry".into(),
            name: "Carry".into(),
            role: String::new(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: runs_in.into(),
            folder: Some(t.0.join("work").to_string_lossy().into_owned()),
        },
        conversation: None,
        prompt: "Say hello.".into(),
        model: None,
        effort: None,
    };
    let started = send_from(job, Origin::default()).unwrap();
    finished(&started.conversation_id);
    records(&t.store().write_path(&started.conversation_id).unwrap())[0]["metadata"]["mode"].clone()
}

// case: is read-only Ask when it is missing or malformed
#[test]
fn is_read_only_ask_when_it_is_missing_or_malformed() {
    // Electron's values: undefined, null, '', 'AUTO', 'full-access' (42 is no string, so here it is none).
    for value in [None, Some(""), Some("AUTO"), Some("full-access")] {
        assert_eq!(requested_mode(value), Mode::Ask, "{value:?}");
    }
    // Through a start: a teammate whose own mode is missing or malformed runs read-only.
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    for own in ["", "AUTO", "full-access"] {
        assert_eq!(start_with_own_mode(&t, own), "ask", "{own:?}");
    }
}

// case: is read the same way by start, handoff and resume
#[test]
fn is_read_the_same_way_by_start_handoff_and_resume() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);

    // A mode Locust knows travels as itself on all three, not as the teammate's own Accept edits.
    assert_eq!(start_with_own_mode(&t, "plan"), "plan", "a start");
    stopped_in(&t, "mission_mode_handoff", "codex", "plan", &[quota_ev()]);
    let (id, _, mode) = carry("mission_mode_handoff", "claude", None).unwrap();
    finished(&id);
    assert_eq!((mode.as_str(), &carried_from(&t, "mission_mode_handoff")[0]["mode"]), ("plan", &json!("plan")), "a handoff");
    stopped_in(&t, "mission_mode_resume", "codex", "ask", &[started_ev()]);
    let (id, _, mode) =
        carry_with("mission_mode_resume", "codex", None, &CarryOnAlso { cut_off: true, message: None }).unwrap();
    finished(&id);
    assert_eq!((mode.as_str(), &carried_from(&t, "mission_mode_resume")[0]["mode"]), ("ask", &json!("ask")), "a resume");

    // A stopped turn whose recorded mode is not one Locust knows: neither handed off nor resumed in the teammate's own
    // mode; nothing starts at all.
    stopped_in(&t, "mission_mode_bad", "codex", "accept-edits", &[started_ev()]);
    let path = path_of(&t, "mission_mode_bad");
    let text = fs::read_to_string(&path).unwrap();
    let (first, rest) = text.split_once('\n').unwrap();
    let mut header: Value = serde_json::from_str(first).unwrap();
    header["metadata"]["mode"] = json!("full-access");
    fs::write(&path, format!("{header}\n{rest}")).unwrap();
    assert!(carry_with("mission_mode_bad", "claude", None, &CarryOnAlso { cut_off: true, message: None }).is_err(), "a handoff");
    assert!(carry_with("mission_mode_bad", "codex", None, &CarryOnAlso { cut_off: true, message: None }).is_err(), "a resume");
    assert!(carried_from(&t, "mission_mode_bad").is_empty());
}

/// carry-on.ts `modeOf`: a turn recorded before modes existed is read by its sandbox, read-only as Ask and anything
/// else as Accept edits, on a handoff as on a resume.
#[test]
fn a_turn_recorded_before_modes_is_read_by_its_sandbox() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    for (id, sandbox, want) in [("mission_old_read_only", "read-only", "ask"), ("mission_old_writes", "workspace-write", "accept-edits")] {
        stopped_in(&t, id, "codex", "plan", &[quota_ev()]);
        let path = path_of(&t, id);
        let text = fs::read_to_string(&path).unwrap();
        let (first, rest) = text.split_once('\n').unwrap();
        let mut header: Value = serde_json::from_str(first).unwrap();
        header["metadata"].as_object_mut().unwrap().remove("mode");
        header["metadata"]["sandbox"] = json!(sandbox);
        fs::write(&path, format!("{header}\n{rest}")).unwrap();
        let (carried, _, mode) = carry(id, "claude", None).unwrap();
        finished(&carried);
        assert_eq!(mode, want, "{sandbox}");
    }
}
