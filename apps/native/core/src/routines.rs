//! Native-owned routine definitions. Unknown runtime state is preserved and blocks removal/replay.
use crate::store::Store;
use serde_json::{Value, json};
use std::sync::Mutex;
pub mod schedule;
static WRITES: Mutex<()> = Mutex::new(());
pub const UNREADABLE: &str = "ROUTINES_UNREADABLE";
fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn control(s: &str, multiline: bool) -> bool {
    s.chars().any(|c| {
        (c <= '\u{1f}' || c == '\u{7f}') && !(multiline && (c == '\n' || c == '\t' || c == '\r'))
    })
}
pub fn list(store: &Store) -> Result<Value, String> {
    let value = store
        .seeded_json("routines.json", json!({"schemaVersion":1,"routines":[]}))
        .map_err(|_| UNREADABLE)?;
    if value["schemaVersion"] != 1 || !value["routines"].is_array() {
        return Err(UNREADABLE.into());
    }
    Ok(value)
}
pub fn awaits_review(r: &Value) -> bool {
    r.get("execution")
        .is_some_and(|e| e["status"] != "abandoned")
        || r.get("staged").is_some()
}
pub fn save(store: &Store, mut r: Value) -> Result<(), String> {
    let _guard = WRITES.lock().map_err(|e| e.to_string())?;
    let name = r["name"].as_str().unwrap_or("").trim();
    if name.is_empty() || name.encode_utf16().count() > 80 || control(name, false) {
        return Err("Routine name is invalid".into());
    }
    r["name"] = json!(name);
    let steps = r["steps"]
        .as_array()
        .ok_or("Routine steps are invalid: 1 to 12 non-empty steps")?;
    if steps.is_empty()
        || steps.len() > 12
        || steps.iter().any(|s| {
            s.as_str()
                .is_none_or(|s| s.trim().is_empty() || control(s, true))
        })
    {
        return Err("Routine steps are invalid: 1 to 12 non-empty steps".into());
    }
    for (i, step) in steps.iter().enumerate() {
        let n = step.as_str().unwrap_or("").encode_utf16().count();
        if n > 8000 {
            return Err(format!(
                "Step {} is {} characters; a step can be at most 8,000. Shorten it and save again.",
                i + 1,
                comma(n)
            ));
        }
    }
    if r.get("inputs").is_none() && steps.iter().filter_map(Value::as_str).any(|step|regex::Regex::new(r"\{\{[^{}]*\}\}").is_ok_and(|re|re.is_match(step))) {
        return Err("Routine input forms are not available in native yet. Declare inputs in Electron before importing this routine.".into());
    }
    if let Some(s) = r.get("schedule")
        && !s.is_null()
        && !schedule::valid(s)
    {
        return Err("The schedule is not one Locust can keep: every 1 to 168 hours, daily or on set days at HH:MM, once at a date and time, on a new file in a folder inside the project, or when a link is called.".into());
    }
    let mut file = list(store)?;
    let rows = file["routines"].as_array_mut().ok_or(UNREADABLE)?;
    if let Some(at) = rows
        .iter()
        .position(|held| held["routineId"] == r["routineId"] && r["routineId"].is_string())
    {
        // Editor changes only these fields; never accept replacement progress or reset run counts.
        for key in ["name", "steps", "schedule"] {
            if r.get(key).is_none() { continue; }
            if r[key].is_null() {
                rows[at].as_object_mut().ok_or(UNREADABLE)?.remove(key);
            } else {
                rows[at][key] = r[key].clone();
            }
        }
    } else {
        if rows.len() >= 64 {
            return Err("Too many routines".into());
        }
        let team = crate::people::team(store)?;
        let owner = team["teammates"]
            .as_array()
            .and_then(|ts| ts.iter().find(|t| t["teammateId"] == r["teammateId"]))
            .ok_or("Teammate id is invalid")?;
        let teammate=crate::native::parse_roster(&team.to_string())?.into_iter().find(|t|r["teammateId"]==t.id).ok_or("Teammate id is invalid")?;
        let folder=crate::native::folder_for(&teammate,store.electron())?;
        r["workspaceId"]=json!(crate::people::workspace_id(&folder));
        r["route"] = owner["route"].clone();
        r["routineId"] = json!(format!(
            "rt_{}",
            crate::native::uuid_v4()
                .map_err(|e| e.to_string())?
                .replace('-', "")
                .chars()
                .take(24)
                .collect::<String>()
        ));
        r["createdAt"] = json!(now());
        r["runs"] = json!(0);
        r["learnedFrom"] = json!([]);
        rows.push(r);
    }
    store.save_json("routines.json", &file)
}
fn comma(n: usize) -> String {
    if n < 1000 {
        n.to_string()
    } else {
        format!("{},{:03}", n / 1000, n % 1000)
    }
}
/// Pause or resume. On resume, `at` (RFC3339) becomes `resumedAt` so the clock counts from then.
pub fn pause(store: &Store, id: &str, paused: bool, at: &str) -> Result<(), String> {
    if !paused
        && chrono::DateTime::parse_from_rfc3339(at).is_err()
    {
        return Err("Resume time is invalid".into());
    }
    mutate(store, id, |r| {
        if paused {
            r["paused"] = json!(true);
        } else {
            r.as_object_mut().ok_or(UNREADABLE)?.remove("paused");
            r["resumedAt"] = json!(at);
        }
        Ok(())
    })
}
pub fn remove(store: &Store, id: &str) -> Result<(), String> {
    let _guard = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = list(store)?;
    let rows = file["routines"].as_array_mut().ok_or(UNREADABLE)?;
    if rows
        .iter()
        .any(|r| r["routineId"] == id && awaits_review(r))
    {
        return Err("That routine is waiting for your review. Check what its last attempt did and abandon it first.".into());
    }
    rows.retain(|r| r["routineId"] != id);
    store.save_json("routines.json", &file)
}
fn mutate(
    store: &Store,
    id: &str,
    f: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let _guard = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = list(store)?;
    let r = file["routines"]
        .as_array_mut()
        .and_then(|rs| rs.iter_mut().find(|r| r["routineId"] == id))
        .ok_or("Routine not found")?;
    f(r)?;
    store.save_json("routines.json", &file)
}
#[cfg(test)]
mod tests;
/// Persist intent before native runtime dispatch. An uncertain result is never replayed.
pub fn run(store: &Store, id: &str) -> Result<(), String> {
    let file = list(store)?;
    let r = file["routines"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["routineId"] == id))
        .ok_or("Routine not found")?;
    if awaits_review(r) {
        return Err("That routine is waiting for your review.".into());
    }
    // These paths must not silently run against the original folder or omit inputs/checks.
    if r["inCopy"] == true
        || r.get("untilCheck").is_some()
        || r.get("inputs").is_some()
        || r.get("handOffs").is_some()
    {
        return Err("This routine needs copy, input, check or handoff support that is not available in native yet.".into());
    }
    let team = crate::native::parse_roster(&crate::people::team(store)?.to_string())?;
    let mut t = team
        .into_iter()
        .find(|t| r["teammateId"] == t.id)
        .ok_or("Teammate id is invalid")?;
    t.runs_in = r["route"]["mode"].as_str().unwrap_or("ask").into();
    let folder = crate::native::folder_for(&t, store.electron())?;
    let workspace = crate::people::workspace_id(&folder);
    if r["workspaceId"].as_str().is_some_and(|w| w != workspace) {
        return Err("This routine belongs to another folder.".into());
    }
    let steps = r["steps"].as_array().ok_or("Routine steps are invalid")?;
    if steps.len() != 1 {
        return Err("Automatic continuation of multi-step routines is not available in native yet.".into());
    }
    if steps.is_empty() || steps.len() > 12 {
        return Err("Routine steps are invalid".into());
    }
    let prompt = steps[0]
        .as_str()
        .ok_or("Routine steps are invalid")?
        .to_owned();
    if prompt.encode_utf16().count() > 8000 {
        return Err("Shorten step 1 before running this routine.".into());
    }
    let attempt = format!(
        "ra_{}",
        crate::native::uuid_v4()
            .map_err(|e| e.to_string())?
            .replace('-', "")
    );
    let execution = json!({"attemptId":attempt,"status":"dispatching","step":1,"of":steps.len(),"startedAt":now(),"updatedAt":now(),"steps":steps,"route":r["route"],"workspaceId":workspace,"canContinue":false});
    mutate(store, id, |held| {
        if awaits_review(held) {
            return Err("Routine execution changed; reload before continuing.".into());
        }
        held["execution"] = execution;
        Ok(())
    })?;
    let result = crate::engine_api::api().send(
        crate::native::Send { model: t.model.clone(), effort: None, teammate: t, conversation: None, prompt },
        crate::native::Origin::default(),
    );
    mutate(store, id, |held| {
        if held["execution"]["attemptId"] != attempt {
            return Err("Routine execution changed.".into());
        }
        held["execution"]["status"] = json!("held");
        held["execution"]["updatedAt"] = json!(now());
        match result {
            Ok(started) => {
                held["execution"]["missionId"] = json!(started.conversation_id);
                held["execution"]["reason"] = json!(
                    "Review this step's conversation before starting any further work. Native automatic continuation is not available yet."
                );
            }
            Err(error) => {
                held["execution"]["reason"] = json!(error);
            }
        }
        Ok(())
    })
}
/// Abandon a held attempt and remove its schedule. Never retries work.
pub fn acknowledge(store: &Store, id: &str, attempt: &str) -> Result<(), String> {
    mutate(store, id, |r| {
        if r["execution"]["attemptId"] != attempt || r["execution"]["status"] != "held" {
            return Err("Routine execution changed; reload before continuing.".into());
        }
        if r["execution"]["missionId"]
            .as_str()
            .is_some_and(|m| crate::engine_api::api().owns(m))
        {
            return Err("Stop the running turn before abandoning this attempt.".into());
        }
        r["execution"]["status"] = json!("abandoned");
        r["execution"]["canContinue"] = json!(false);
        r["execution"]["updatedAt"] = json!(now());
        r["execution"]["reason"] = json!(
            "Abandoned by you; schedule removed. This does not stop an external runtime or undo its work."
        );
        r.as_object_mut().ok_or(UNREADABLE)?.remove("schedule");
        Ok(())
    })
}
/// Keep the routine and its schedule; clear the held attempt. Clock starts at `kept_at` (decision).
pub fn keep(store: &Store, id: &str, attempt: &str, kept_at: &str) -> Result<(), String> {
    if chrono::DateTime::parse_from_rfc3339(kept_at).is_err() {
        return Err("Keep time is invalid".into());
    }
    mutate(store, id, |r| {
        if r["execution"]["attemptId"] != attempt || r["execution"]["status"] != "held" {
            return Err("Routine execution changed; reload before continuing.".into());
        }
        if r["execution"]["missionId"]
            .as_str()
            .is_some_and(|m| crate::engine_api::api().owns(m))
        {
            return Err("Stop the running turn before keeping this attempt.".into());
        }
        r.as_object_mut().ok_or(UNREADABLE)?.remove("execution");
        r["lastRunAt"] = json!(kept_at);
        Ok(())
    })
}
/// Record missed time slots without dispatch. Scheduled execution remains unavailable until
/// completion reconciliation can prove both the terminal result and question receipts.
pub fn record_misses(
    store: &Store,
    opened: chrono::DateTime<chrono::Local>,
) -> Result<bool, String> {
    let _guard = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = list(store)?;
    let mut changed = false;
    for r in file["routines"].as_array_mut().ok_or(UNREADABLE)? {
        if ["dispatching", "running"].iter().any(|status|r["execution"]["status"]==*status) {
            r["execution"]["status"]=json!("held");
            r["execution"]["canContinue"]=json!(false);
            r["execution"]["reason"]=json!("The previous dispatch needs review. Its result is uncertain; this step will not be retried.");
            changed=true;
        }
        if r["paused"] == true || awaits_review(r) {
            continue;
        }
        if let Some(last) = schedule::base(r)
            && let Some(due) = schedule::missed(&r["schedule"], last, opened, chrono::Local::now())
        {
            let due = due.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            r["lastRunAt"] = json!(due);
            r["missedAt"] = json!(due);
            let mut history = r["history"].as_array().cloned().unwrap_or_default();
            history.push(json!({"kind":"missed","dueAt":due,"recordedAt":now()}));
            if history.len() > 30 {
                history.drain(..history.len() - 30);
            }
            r["history"] = json!(history);
            changed = true;
        }
    }
    if changed {
        store.save_json("routines.json", &file)?;
    }
    Ok(changed)
}
