//! A ROUTINE'S RUN, AS THE STORE KEEPS IT (main/routine-store.ts: `parsedRoutine`, `parsedExecution`, `recordRun`,
//! `saveProgress`, `clearProgress`, `keepSchedule`, `abandon`, `settleStaged`, `recordMiss`).
//!
//! Every write compares attempt ids, so a stale decision cannot acknowledge a newer run, and counting a run and
//! clearing its receipt are one write. What the runner reads is the routine as Electron's store would parse it: a
//! hand-off list, goal, waiting copy or last failure that does not read is dropped, never the routine, and a receipt
//! that does not read becomes a visible hold rather than being dropped and replayed.
use super::{UNREADABLE, WRITES, list, now, schedule};
use crate::store::Store;
use serde_json::{Value, json};

pub const MAX_GOAL_TRIES: u64 = 5;
const HISTORY_CAP: usize = 30;

/// `safeId`: letters, digits, `_` and `-`, 1 to 64 of them.
pub fn safe_id(v: &Value) -> bool {
    v.as_str().is_some_and(|s| (1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
}

fn instant(v: &Value) -> bool {
    v.as_str().is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
}

/// Who takes each step (0.435): one entry per step, each naming a teammate or nobody, at most a checker or a wait.
pub fn valid_hand_offs(value: &Value, steps: usize) -> bool {
    value.as_array().is_some_and(|entries| {
        entries.len() == steps
            && entries.iter().all(|entry| {
                entry.as_object().is_some_and(|o| {
                    o.keys().all(|k| k == "teammateId" || k == "check" || k == "askFirst")
                        && o.get("teammateId").is_none_or(safe_id)
                        && o.get("check").is_none_or(|c| c == true)
                        && o.get("askFirst").is_none_or(|c| c == true)
                })
            })
    })
}

/// Stored only when some step goes to another teammate, checks, or waits for an OK (step 2 on: step 1 never waits).
pub fn kept_hand_offs(hand_offs: &Value, owner: &str) -> Option<Value> {
    let cleaned: Vec<Value> = hand_offs
        .as_array()?
        .iter()
        .enumerate()
        .map(|(at, entry)| {
            let mut kept = serde_json::Map::new();
            if let Some(t) = entry["teammateId"].as_str().filter(|t| *t != owner) {
                kept.insert("teammateId".into(), json!(t));
            }
            if entry["check"] == true {
                kept.insert("check".into(), json!(true));
            }
            if entry["askFirst"] == true && at > 0 {
                kept.insert("askFirst".into(), json!(true));
            }
            Value::Object(kept)
        })
        .collect();
    cleaned.iter().any(|e| e.as_object().is_some_and(|o| !o.is_empty())).then(|| json!(cleaned))
}

/// A standing goal the store will keep: 1 to 5 fixes (0.534).
pub fn valid_goal(value: &Value) -> bool {
    value["tries"].as_u64().is_some_and(|t| (1..=MAX_GOAL_TRIES).contains(&t)) && value["tries"].is_u64()
}

/// What a finished copy run left, as the store will keep it (0.533).
pub fn valid_staged(value: &Value) -> bool {
    let paths = |list: &Value| {
        list.as_array().is_some_and(|l| l.len() <= 2_000 && l.iter().all(|p| p.as_str().is_some_and(|p| !p.is_empty() && p.len() <= 1_024)))
    };
    safe_id(&value["attemptId"])
        && instant(&value["finishedAt"])
        && value["folder"].as_str().is_some_and(|f| !f.is_empty() && f.len() <= 4_096)
        && paths(&value["changed"])
        && paths(&value["deleted"])
}

/// A last failure is a line of the run's own words, bounded (0.536).
pub fn valid_failed(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.trim().is_empty() && s.encode_utf16().count() <= 600)
}

fn valid_steps(v: &Value) -> bool {
    v.as_array().is_some_and(|s| !s.is_empty() && s.len() <= 12 && s.iter().all(|s| s.as_str().is_some_and(|s| !s.trim().is_empty())))
}

fn valid_route(v: &Value) -> bool {
    v["runtime"].as_str().is_some_and(|s| !s.is_empty()) && v["mode"].is_string()
}

/// `parsedExecution`: the receipt as it was, or a visible, non-resumable hold in place of one that does not read.
pub fn parsed_execution(item: &Value, steps: &Value, route: &Value) -> Value {
    let int = |v: &Value| v.as_u64();
    let opt = |key: &str, ok: &dyn Fn(&Value) -> bool| item.get(key).is_none_or(ok);
    let valid = safe_id(&item["attemptId"])
        && ["dispatching", "running", "held", "abandoned"].iter().any(|s| item["status"] == *s)
        && int(&item["step"]).is_some_and(|s| s >= 1)
        && valid_steps(&item["steps"])
        && int(&item["of"]) == item["steps"].as_array().map(|s| s.len() as u64)
        && int(&item["step"]) <= int(&item["of"])
        && valid_route(&item["route"])
        && safe_id(&item["workspaceId"])
        && instant(&item["startedAt"])
        && instant(&item["updatedAt"])
        && ["missionId", "runId", "followUpOf"].iter().all(|k| opt(k, &safe_id))
        && opt("reason", &|v| v.as_str().is_some_and(|s| s.encode_utf16().count() <= 4000))
        && opt("canContinue", &Value::is_boolean)
        && opt("recovered", &Value::is_boolean)
        && opt("settledAtDispatch", &Value::is_boolean)
        && opt("waitsForOk", &|v| v == true)
        && opt("inCopy", &|v| v == true)
        && opt("untilCheck", &valid_goal)
        && opt("goalTry", &|v| v.as_u64().is_some_and(|n| (1..=MAX_GOAL_TRIES).contains(&n)));
    if valid {
        let mut kept = item.clone();
        // A receipt's hand-offs that do not read are dropped, never the receipt (0.435).
        if let Some(h) = item.get("handOffs")
            && !valid_hand_offs(h, item["steps"].as_array().map_or(0, Vec::len))
            && let Some(o) = kept.as_object_mut()
        {
            o.remove("handOffs");
        }
        return kept;
    }
    json!({"attemptId": "invalid_receipt", "status": "held", "step": 1, "of": steps.as_array().map_or(0, Vec::len),
        "startedAt": "1970-01-01T00:00:00.000Z", "updatedAt": "1970-01-01T00:00:00.000Z", "steps": steps, "route": route,
        "workspaceId": "unknown", "reason": "The saved execution receipt is invalid. Review external work before abandoning this attempt.",
        "canContinue": false})
}

/// The routine as the runner reads it (`parsedRoutine`'s run fields), or None for one that cannot run at all.
pub fn parsed(r: &Value) -> Option<Value> {
    if !safe_id(&r["routineId"]) || !safe_id(&r["teammateId"]) || !valid_steps(&r["steps"]) || !valid_route(&r["route"]) {
        return None;
    }
    if r.get("inputs").is_some_and(|v| !v.is_null() && !crate::routine_inputs::valid_inputs(v)) {
        return None;
    }
    let mut out = r.clone();
    let o = out.as_object_mut()?;
    let steps = r["steps"].as_array().map_or(0, Vec::len);
    if o.get("handOffs").is_some_and(|h| !valid_hand_offs(h, steps)) {
        o.remove("handOffs");
    }
    if o.get("inCopy").is_some_and(|v| v != true) {
        o.remove("inCopy");
    }
    if o.get("untilCheck").is_some_and(|v| !valid_goal(v)) {
        o.remove("untilCheck");
    }
    if o.get("staged").is_some_and(|v| !valid_staged(v)) {
        o.remove("staged");
    }
    if o.get("lastFailed").is_some_and(|v| !valid_failed(v)) {
        o.remove("lastFailed");
    }
    if o.get("schedule").is_some_and(|s| !schedule::valid(s)) {
        o.remove("schedule");
    }
    if o.get("paused").is_some_and(|v| v != true) {
        o.remove("paused");
    }
    if let Some(e) = o.get("execution").filter(|e| !e.is_null()).cloned() {
        o.insert("execution".into(), parsed_execution(&e, &r["steps"], &r["route"]));
    } else {
        o.remove("execution");
    }
    Some(out)
}

pub fn get(store: &Store, id: &str) -> Result<Option<Value>, String> {
    Ok(list(store)?["routines"].as_array().and_then(|rs| rs.iter().find(|r| r["routineId"] == id)).and_then(parsed))
}

pub fn all(store: &Store) -> Result<Vec<Value>, String> {
    Ok(list(store)?["routines"].as_array().map(|rs| rs.iter().filter_map(parsed).collect()).unwrap_or_default())
}

/// One routine, parsed, changed and written back in one write. `f` returns false to write nothing.
fn change(store: &Store, id: &str, f: impl FnOnce(&mut Value) -> Result<bool, String>) -> Result<(), String> {
    let _guard = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = list(store)?;
    let rows = file["routines"].as_array_mut().ok_or(UNREADABLE)?;
    let Some(at) = rows.iter().position(|r| r["routineId"] == id) else {
        return f(&mut Value::Null).map(|_| ());
    };
    let mut row = parsed(&rows[at]).unwrap_or_else(|| rows[at].clone());
    if !f(&mut row)? {
        return Ok(());
    }
    rows[at] = row;
    store.save_json("routines.json", &file)
}

fn changed() -> String {
    "Routine execution changed; reload before continuing.".into()
}

/// Count a finished run and clear its receipt in ONE write: a crash cannot count twice. A copy's waiting changes and
/// a last failure are recorded in the same write; a clock time that passed while it ran is recorded as missed.
pub fn record_run(store: &Store, id: &str, attempt: &str, staged: Option<Value>, failed: Option<String>) -> Result<(), String> {
    if !safe_id(&json!(id)) {
        return Ok(());
    }
    change(store, id, |held| {
        if held.is_null() {
            return Ok(false);
        }
        let e = &held["execution"];
        if e["attemptId"] != attempt
            || e["step"] != e["of"]
            || e.get("missionId").is_none()
            || !(e["status"] == "running" || e["status"] == "held")
        {
            return Err("Routine completion receipt changed; reload before continuing.".into());
        }
        if staged.as_ref().is_some_and(|s| !valid_staged(s)) {
            return Err("Routine copy changes are invalid".into());
        }
        let ended = chrono::Local::now();
        let started = e["startedAt"].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&chrono::Local));
        let passed = match (held.get("schedule"), started) {
            (Some(s), Some(started)) => schedule::slot_passed_during(s, started, ended),
            _ => None,
        };
        let o = held.as_object_mut().ok_or(UNREADABLE)?;
        for key in ["execution", "lastFailed", "missedAt"] {
            o.remove(key);
        }
        let runs = o.get("runs").and_then(Value::as_u64).unwrap_or(0);
        o.insert("runs".into(), json!(runs + 1));
        o.insert("lastRunAt".into(), json!(now()));
        if let Some(passed) = passed {
            let due = passed.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            o.insert("missedAt".into(), json!(due));
            let mut history = o.get("history").and_then(Value::as_array).cloned().unwrap_or_default();
            history.push(json!({"kind": "missed", "dueAt": due, "recordedAt": now()}));
            let skip = history.len().saturating_sub(HISTORY_CAP);
            o.insert("history".into(), json!(history[skip..]));
        }
        if let Some(staged) = staged {
            o.insert("staged".into(), staged);
        }
        if let Some(failed) = failed.filter(|f| valid_failed(&json!(f))) {
            o.insert("lastFailed".into(), json!(failed));
        }
        Ok(true)
    })
}

/// Write a run's progress, only over the attempt the caller last saw (`expected`), or over none when starting.
pub fn save_progress(store: &Store, id: &str, progress: &Value, expected: Option<&str>) -> Result<(), String> {
    change(store, id, |held| {
        if held.is_null() {
            return Err(changed());
        }
        let current = held.get("execution");
        let stale = match expected {
            None => current.is_some_and(|c| c["status"] != "abandoned"),
            Some(attempt) => current.is_none_or(|c| c["attemptId"] != attempt),
        };
        if stale {
            return Err(changed());
        }
        if parsed_execution(progress, &held["steps"], &held["route"])["attemptId"] != progress["attemptId"] {
            return Err("Invalid routine progress".into());
        }
        held["execution"] = progress.clone();
        Ok(true)
    })
}

/// A start the runtime refused before anything ran: the receipt goes, as if it was never written.
pub fn clear_progress(store: &Store, id: &str, attempt: &str) -> Result<(), String> {
    change(store, id, |held| {
        if held.is_null() || held["execution"]["attemptId"] != attempt {
            return Err("Routine execution changed.".into());
        }
        held.as_object_mut().ok_or(UNREADABLE)?.remove("execution");
        Ok(true)
    })
}

/// Keep: the held attempt cleared, the routine as it was, and its clock started at the decision (0.404).
pub fn keep_schedule(store: &Store, id: &str, attempt: &str, kept_at: &str) -> Result<(), String> {
    change(store, id, |held| {
        if held.is_null() || held["execution"]["attemptId"] != attempt || held["execution"]["status"] != "held" {
            return Err(changed());
        }
        let o = held.as_object_mut().ok_or(UNREADABLE)?;
        o.remove("execution");
        o.insert("lastRunAt".into(), json!(kept_at));
        Ok(true)
    })
}

/// Abandon: durable acknowledgement, and the schedule removed in the same write so nothing replays step 1. Refused
/// while the attempt's own turn is still running here (native's rule since p11; Electron has no such turn to ask).
pub fn abandon(store: &Store, id: &str, attempt: &str) -> Result<(), String> {
    change(store, id, |held| {
        if held.is_null() || held["execution"]["attemptId"] != attempt || held["execution"]["status"] != "held" {
            return Err(changed());
        }
        if held["execution"]["missionId"].as_str().is_some_and(|m| crate::engine_api::api().owns(m)) {
            return Err("Stop the running turn before abandoning this attempt.".into());
        }
        let e = &mut held["execution"];
        e["status"] = json!("abandoned");
        e["canContinue"] = json!(false);
        e["updatedAt"] = json!(now());
        e["reason"] = json!("Abandoned by you; schedule removed. This does not stop an external runtime or undo its work.");
        held.as_object_mut().ok_or(UNREADABLE)?.remove("schedule");
        Ok(true)
    })
}

/// A copy run's waiting changes, settled by Keep or Discard.
pub fn settle_staged(store: &Store, id: &str, attempt: &str) -> Result<(), String> {
    change(store, id, |held| {
        if held.is_null() || held["staged"]["attemptId"] != attempt {
            return Err("Those changes were already kept or discarded; reload Routines.".into());
        }
        held.as_object_mut().ok_or(UNREADABLE)?.remove("staged");
        Ok(true)
    })
}

/// A slot that passed before this process opened: missed, recorded, never started.
pub fn record_miss(store: &Store, id: &str, due: &str, recorded: &str) -> Result<(), String> {
    if !safe_id(&json!(id)) || !instant(&json!(due)) || !instant(&json!(recorded)) {
        return Ok(());
    }
    change(store, id, |held| {
        if held.is_null() || held.get("execution").is_some_and(|e| e["status"] != "abandoned") {
            return Ok(false);
        }
        let mut history = held["history"].as_array().cloned().unwrap_or_default();
        history.push(json!({"kind": "missed", "dueAt": due, "recordedAt": recorded}));
        let skip = history.len().saturating_sub(HISTORY_CAP);
        held["history"] = json!(history[skip..]);
        held["lastRunAt"] = json!(due);
        held["missedAt"] = json!(due);
        Ok(true)
    })
}

/// The native store as the runner's [`crate::routine_runner::Routines`].
pub struct StoreRoutines(pub Store);

impl crate::routine_runner::Routines for StoreRoutines {
    fn get(&self, id: &str) -> Result<Option<Value>, String> {
        get(&self.0, id)
    }
    fn list(&self) -> Result<Vec<Value>, String> {
        all(&self.0)
    }
    fn record_run(&self, id: &str, attempt: &str, staged: Option<Value>, failed: Option<String>) -> Result<(), String> {
        record_run(&self.0, id, attempt, staged, failed)
    }
    fn save_progress(&self, id: &str, execution: &Value, expected: Option<&str>) -> Result<(), String> {
        save_progress(&self.0, id, execution, expected)
    }
    fn clear_progress(&self, id: &str, attempt: &str) -> Result<(), String> {
        clear_progress(&self.0, id, attempt)
    }
    fn abandon(&self, id: &str, attempt: &str) -> Result<(), String> {
        abandon(&self.0, id, attempt)
    }
    fn keep_schedule(&self, id: &str, attempt: &str, kept_at: &str) -> Result<(), String> {
        keep_schedule(&self.0, id, attempt, kept_at)
    }
    fn record_miss(&self, id: &str, due: &str, recorded: &str) -> Result<(), String> {
        record_miss(&self.0, id, due, recorded)
    }
}
