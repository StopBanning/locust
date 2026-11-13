//! A PERSON'S DECISIONS ON A ROUTINE: a held attempt continued, kept or abandoned (main/routine-recovery-ipc.ts,
//! shared/routine-recovery.ts), and a copy run's changes kept or discarded (main/index.ts, `ROUTINE_SETTLE_CHANNEL`).
//!
//! Electron validates these in main because the renderer is not trusted. Native has no renderer process, so there is
//! no sender to check; the request's own shape is still checked, and a failure to record is said in Electron's words.
use crate::routine_copy::{BringIn, RoutineCopies};
use crate::routine_runner::{Decision, RecoveryRequest, RoutineRunner};
use crate::store::Store;
use serde_json::Value;

fn id_ok(v: &Value) -> bool {
    v.as_str().is_some_and(|s| (1..=100).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
}

/// `isRoutineRecoveryRequest`.
pub fn request_of(v: &Value) -> Option<RecoveryRequest> {
    if !id_ok(&v["routineId"]) || !id_ok(&v["attemptId"]) {
        return None;
    }
    let step = v["step"].as_u64().filter(|s| (1..=12).contains(s))?;
    let decision = match v["decision"].as_str()? {
        "continue" => Decision::Continue,
        "abandon" => Decision::Abandon,
        "keep" => Decision::Keep,
        _ => return None,
    };
    Some(RecoveryRequest { routine_id: v["routineId"].as_str()?.into(), attempt_id: v["attemptId"].as_str()?.into(), step, decision })
}

/// `decideRoutineRecovery`, without the sender check native has no use for.
pub fn decide(input: &Value, workspace_chosen: bool, runner: Option<&RoutineRunner>) -> Result<(), String> {
    let Some(request) = request_of(input) else {
        return Err("Invalid routine recovery request.".into());
    };
    let Some(runner) = runner.filter(|_| workspace_chosen) else {
        return Err("Open the original project folder before reviewing this routine.".into());
    };
    match runner.recover(&request) {
        Ok(answer) => answer,
        Err(_) => Err("Recovery could not be recorded or started. Check disk access and the saved mission, then reload routines. No automatic replay will occur.".into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settle {
    Keep,
    Discard,
}

/// A copy run's changes, settled by the person. Keep writes them into the folder the copy was made from -- and
/// nothing at all when the person has since changed any of those same files; Discard throws the copy away. Ok is
/// what to say; Err is the refusal, said.
pub fn settle(store: &Store, copies: &RoutineCopies, routine_id: &str, decision: Settle) -> Result<String, String> {
    let routine = crate::routines::progress::get(store, routine_id).ok().flatten();
    let Some((routine, staged)) = routine.and_then(|r| r.get("staged").cloned().map(|s| (r, s))) else {
        return Err("Those changes were already kept or discarded.".into());
    };
    let name = routine["name"].as_str().unwrap_or("");
    let attempt = staged["attemptId"].as_str().unwrap_or("");
    if decision == Settle::Discard {
        crate::routines::progress::settle_staged(store, routine_id, attempt)?;
        let _ = copies.discard(routine_id);
        return Ok(format!("Discarded. Nothing from that run of \"{name}\" reached the folder."));
    }
    let kept = copies.keep(routine_id)?;
    if let BringIn::YourChanges(files) = &kept {
        let named = if files.len() <= 3 { files.join(", ") } else { format!("{} and {} more", files[..3].join(", "), files.len() - 3) };
        return Err(format!("Not kept: you have changed {named} since it ran, so nothing was written. Discard its changes, or put those files back and Keep again."));
    }
    crate::routines::progress::settle_staged(store, routine_id, attempt)?;
    let _ = copies.discard(routine_id);
    Ok(match kept {
        BringIn::Brought(files) => format!("Kept: {} file{} written into the folder.", files.len(), if files.len() == 1 { "" } else { "s" }),
        _ => "Nothing to write: the folder already matches.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // routine-recovery.test.ts, "main IPC validates the sender, workspace, request and reports persistence failures":
    // the request and the workspace; native has no sender to validate.
    #[test]
    fn validates_the_workspace_and_the_request_and_reports_persistence_failures() {
        let request = json!({"routineId": "rt_x", "attemptId": "attempt_1", "step": 1, "decision": "abandon"});
        assert_eq!(request_of(&request), Some(RecoveryRequest { routine_id: "rt_x".into(), attempt_id: "attempt_1".into(), step: 1, decision: Decision::Abandon }));
        assert_eq!(decide(&request, false, None).unwrap_err(), "Open the original project folder before reviewing this routine.");
        for bad in [json!({"routineId": "rt_x", "attemptId": "attempt_1", "step": 1, "decision": "replay"}), Value::Null, json!({"routineId": "../x", "attemptId": "a", "step": 1, "decision": "keep"}), json!({"routineId": "rt_x", "attemptId": "a", "step": 13, "decision": "keep"})] {
            assert_eq!(decide(&bad, true, None).unwrap_err(), "Invalid routine recovery request.");
        }
        // A store that cannot be read: the decision is not recorded, and said so.
        struct Broken;
        impl crate::routine_runner::Routines for Broken {
            fn get(&self, _: &str) -> Result<Option<Value>, String> { Err("disk full".into()) }
            fn list(&self) -> Result<Vec<Value>, String> { Err("disk full".into()) }
            fn record_run(&self, _: &str, _: &str, _: Option<Value>, _: Option<String>) -> Result<(), String> { Err("disk full".into()) }
            fn save_progress(&self, _: &str, _: &Value, _: Option<&str>) -> Result<(), String> { Err("disk full".into()) }
            fn clear_progress(&self, _: &str, _: &str) -> Result<(), String> { Err("disk full".into()) }
            fn abandon(&self, _: &str, _: &str) -> Result<(), String> { Err("disk full".into()) }
            fn keep_schedule(&self, _: &str, _: &str, _: &str) -> Result<(), String> { Err("disk full".into()) }
        }
        let host = std::sync::Arc::new(crate::routine_runner::tests::Fake::default());
        let runner = RoutineRunner::new(std::sync::Arc::new(Broken), host);
        assert!(decide(&request, true, Some(&runner)).unwrap_err().contains("Check disk access"));
    }
}
