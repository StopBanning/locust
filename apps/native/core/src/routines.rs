//! Native-owned routine definitions. Unknown runtime state is preserved and blocks removal/replay.
use crate::store::Store;
use serde_json::{Value, json};
use std::sync::Mutex;
pub mod progress;
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
    let step_strings: Vec<String> = steps
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect();
    if let Some(inputs_val) = r.get("inputs") {
        let check_val = if inputs_val.is_null() { json!([]) } else { inputs_val.clone() };
        if let Some(refusal) = crate::routine_inputs::inputs_refusal(&check_val, Some(&step_strings)) {
            return Err(refusal);
        }
    }
    if let Some(s) = r.get("schedule")
        && !s.is_null()
        && !schedule::valid(s)
    {
        return Err("The schedule is not one Locust can keep: every 1 to 168 hours, daily or on set days at HH:MM, once at a date and time, on a new file in a folder inside the project, or when a link is called.".into());
    }
    // Who takes each step, whether it works in a copy, and its standing goal (routine-store.ts create/update).
    let count = steps.len();
    if let Some(h) = r.get("handOffs").filter(|h| !h.is_null())
        && !progress::valid_hand_offs(h, count)
    {
        return Err("Who takes each step does not line up with the steps".into());
    }
    if let Some(g) = r.get("untilCheck").filter(|g| !g.is_null())
        && !progress::valid_goal(g)
    {
        return Err("Routine goal is invalid".into());
    }
    if let Some(m) = r.get("mode")
        && m.as_str().and_then(crate::approval::Mode::parse).is_none()
    {
        return Err("Routine route is invalid".into());
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
        let owner = rows[at]["teammateId"].as_str().unwrap_or("").to_owned();
        let held = rows[at].as_object_mut().ok_or(UNREADABLE)?;
        match r.get("inputs") {
            Some(v) if v.is_null() => {
                held.remove("inputs");
            }
            Some(v) => {
                let parsed = crate::routine_inputs::parse_inputs_array(v)
                    .ok_or_else(|| "Routine inputs are invalid".to_string())?;
                held.insert(
                    "inputs".into(),
                    crate::routine_inputs::kept_inputs_value(&parsed),
                );
            }
            None => {
                if let Some(existing) = held.get("inputs")
                    && let Some(refusal) =
                        crate::routine_inputs::inputs_refusal(existing, Some(&step_strings))
                {
                    return Err(refusal);
                }
            }
        }
        // Hand-offs not named are kept only while they still line up with the steps.
        let hand_offs = match r.get("handOffs") {
            Some(h) if !h.is_null() => progress::kept_hand_offs(h, &owner),
            Some(_) => None,
            None => held.get("handOffs").filter(|h| h.as_array().is_some_and(|h| h.len() == count)).cloned(),
        };
        match hand_offs {
            Some(h) => held.insert("handOffs".into(), h),
            None => held.remove("handOffs"),
        };
        if let Some(c) = r.get("inCopy") {
            if c == true { held.insert("inCopy".into(), json!(true)) } else { held.remove("inCopy") };
        }
        match r.get("untilCheck") {
            Some(g) if !g.is_null() => { held.insert("untilCheck".into(), json!({"tries": g["tries"]})); }
            Some(_) => { held.remove("untilCheck"); }
            None => {}
        }
        if let Some(m) = r.get("mode") {
            held.entry("route").or_insert_with(|| json!({}))["mode"] = m.clone();
        }
    } else {
        if rows.len() >= 64 {
            return Err("Too many routines".into());
        }
        if r.get("inputs").is_none()
            && let Some(refusal) =
                crate::routine_inputs::inputs_refusal(&json!([]), Some(&step_strings))
        {
            return Err(refusal);
        }
        if let Some(inputs_val) = r.get("inputs").filter(|v| !v.is_null()) {
            let parsed = crate::routine_inputs::parse_inputs_array(inputs_val)
                .ok_or_else(|| "Routine inputs are invalid".to_string())?;
            r["inputs"] = crate::routine_inputs::kept_inputs_value(&parsed);
        } else if let Some(o) = r.as_object_mut() {
            o.remove("inputs");
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
        // Where a routine saved from a conversation came from (routine-store.ts validLearnedFrom): at most 64 safe ids.
        let learned = match r.get("learnedFrom").filter(|v| !v.is_null()) {
            None => json!([]),
            Some(v) => match v.as_array() {
                Some(ids) if ids.len() <= 64 && ids.iter().all(|id| id.as_str().is_some_and(crate::ledger::safe_id)) => v.clone(),
                _ => return Err("Routine provenance is invalid".into()),
            },
        };
        r["learnedFrom"] = learned;
        let o = r.as_object_mut().ok_or(UNREADABLE)?;
        match o.remove("handOffs").filter(|h| !h.is_null()).and_then(|h| progress::kept_hand_offs(&h, teammate.id.as_str())) {
            Some(h) => { o.insert("handOffs".into(), h); }
            None => {}
        }
        if o.get("inCopy").is_some_and(|c| c != true) { o.remove("inCopy"); }
        if o.get("untilCheck").is_some_and(Value::is_null) { o.remove("untilCheck"); }
        if let Some(m) = o.remove("mode") { o.entry("route").or_insert_with(|| json!({}))["mode"] = m; }
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
/// Record calls a private routine link could not run (expired or dropped past 200 letters).
pub fn record_hook_loss(store: &Store, id: &str, count: u64, at: &str) -> Result<(), String> {
    if !progress::safe_id(&json!(id)) || count == 0 || chrono::DateTime::parse_from_rfc3339(at).is_err() {
        return Ok(());
    }
    let _guard = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = list(store)?;
    let Some(r) = file["routines"].as_array_mut().and_then(|rs| rs.iter_mut().find(|r| r["routineId"] == id)) else {
        return Ok(());
    };
    let mut history = r["history"].as_array().cloned().unwrap_or_default();
    history.push(json!({"kind": "missed", "dueAt": at, "recordedAt": at, "hookCalls": count}));
    if history.len() > 30 {
        history.drain(..history.len() - 30);
    }
    r["history"] = json!(history);
    store.save_json("routines.json", &file)
}
#[cfg(test)]
mod tests;
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

/// A teammate and the model their routine steps run on now: "Wren (Grok 4.7 High)" (`routineRunner`).
pub fn routine_runner(teammate_id: &str, team: &[crate::engine::Teammate]) -> String {
    let Some(mate) = team.iter().find(|m| m.id == teammate_id) else { return "someone removed".into() };
    match (mate.runtime.as_deref(), mate.model.as_deref()) {
        (Some(runtime), Some(model)) => format!("{} ({})", mate.name, model_said(runtime, model)),
        _ => mate.name.clone(),
    }
}

fn model_said(runtime: &str, model: &str) -> String {
    if model == "account-default" {
        format!("{} default", crate::inspector::runtime_display_name(runtime))
    } else {
        crate::teammate_tooltip::route_model_name(runtime, model)
    }
}

/// The routine row's chain with each teammate's model (`routineChain(routine, team, true)`, 0.493): one teammate's
/// routine names it with its model; a chain names each, except that one shared route is named once, after the chain,
/// "both on" or "all on" it (0.636), so "(checks)" stays on the line. The same model through two tools is two routes.
pub fn routine_chain_with_models(routine: &Value, team: &[crate::engine::Teammate]) -> String {
    let owner = routine["teammateId"].as_str().unwrap_or("");
    let handoffs = routine["handOffs"].as_array();
    if !handoffs.is_some_and(|h| h.iter().any(|e| e["teammateId"].is_string() || e["check"] == true)) {
        return routine_runner(owner, team);
    }
    let handoffs = handoffs.unwrap();
    let mut steps: Vec<(&str, bool)> = Vec::new();
    let mut last: Option<&str> = None;
    for (i, _) in routine["steps"].as_array().map(Vec::as_slice).unwrap_or_default().iter().enumerate() {
        let entry = handoffs.get(i).unwrap_or(&Value::Null);
        let runner = entry["teammateId"].as_str().unwrap_or(owner);
        let check = entry["check"] == true;
        if last == Some(runner) && !check {
            continue;
        }
        steps.push((runner, check));
        last = Some(runner);
    }
    let link = |check: bool, label: String| if check { format!("{label} (checks)") } else { label };
    let name = |id: &str| team.iter().find(|m| m.id == id).map_or("someone removed".to_owned(), |m| m.name.clone());
    let routes: Vec<Option<(&str, &str)>> = steps
        .iter()
        .map(|(id, _)| team.iter().find(|m| m.id == *id).and_then(|m| Some((m.runtime.as_deref()?, m.model.as_deref()?))))
        .collect();
    let shared = routes.first().copied().flatten().filter(|first| routes.iter().all(|r| r.as_ref() == Some(first)));
    if let Some((runtime, model)) = shared
        && steps.len() > 1
    {
        let chain = steps.iter().map(|(id, check)| link(*check, name(id))).collect::<Vec<_>>().join(" → ");
        return format!("{chain}, {} on {}", if steps.len() == 2 { "both" } else { "all" }, model_said(runtime, model));
    }
    steps.iter().map(|(id, check)| link(*check, routine_runner(id, team))).collect::<Vec<_>>().join(" → ")
}

/// A hand-off chain in step order; adjacent repeated runners collapse unless checking.
pub fn routine_chain(routine: &Value, team: &[crate::engine::Teammate]) -> Option<String> {
    let handoffs = routine["handOffs"].as_array()?;
    if !handoffs.iter().any(|h| h["teammateId"].is_string() || h["check"] == true) { return None; }
    let owner = routine["teammateId"].as_str().unwrap_or("");
    let mut last = None;
    let mut links = Vec::new();
    for (i, _) in routine["steps"].as_array()?.iter().enumerate() {
        let entry = handoffs.get(i).unwrap_or(&Value::Null);
        let runner = entry["teammateId"].as_str().unwrap_or(owner);
        let check = entry["check"] == true;
        if last == Some(runner) && !check { continue; }
        let name = team.iter().find(|m| m.id == runner).map(|m| m.name.as_str()).unwrap_or("someone removed");
        links.push(if check { format!("{name} (checks)") } else { name.to_owned() });
        last = Some(runner);
    }
    Some(links.join(" → "))
}

/// The folder a routine runs in, among the folders Locust knows (AutomationsScreen, 0.512): its name and its whole
/// path. A routine keeps the folder only as an id (`workspaceId`, the folder's path hashed), so a folder Locust does not
/// know is not guessed at: None, and the card says nothing.
pub fn routine_folder(routine: &Value, known: &[std::path::PathBuf]) -> Option<(String, String)> {
    let id = routine["workspaceId"].as_str()?;
    let path = known.iter().find(|p| crate::people::workspace_id(p) == id)?;
    let whole = path.to_string_lossy().into_owned();
    let name = path.file_name().map_or_else(|| whole.clone(), |n| n.to_string_lossy().into_owned());
    Some((name, whole))
}

/// `routineRunSummary`: how many steps, then where a run of it stands -- in progress, waiting for your OK, waiting for
/// review, or its last attempt abandoned, with its completed runs -- else how often it has run.
pub fn run_summary(routine: &Value) -> String {
    let count = routine["steps"].as_array().map_or(0, Vec::len);
    let steps = format!("{count} step{}", if count == 1 { "" } else { "s" });
    let runs = routine["runs"].as_u64().unwrap_or(0);
    if let Some(e) = routine.get("execution").filter(|e| e.is_object()) {
        let state = if e["status"] == "abandoned" {
            "last attempt abandoned"
        } else if e["status"] == "running" {
            "in progress"
        } else if e["waitsForOk"] == true {
            "waiting for your OK"
        } else {
            "waiting for review"
        };
        return format!("{steps} · {state} · {runs} completed runs");
    }
    if runs == 0 {
        return format!("{steps} · not run yet");
    }
    format!("{steps} · run {runs} time{}", if runs == 1 { "" } else { "s" })
}

/// `routineStepLabel`: what the sidebar says under a teammate replaying a routine.
pub fn step_label(step: u64, of: u64) -> String {
    format!("routine · step {step} of {of}")
}

/// `routineWaitsForYou` (0.591): a scheduled routine on a teammate in Approve each, fired while nobody is at the
/// window, stops on its first card and waits there. Said where the schedule is set; None for every other mode and for
/// a routine that only runs when pressed (someone is there).
pub fn waits_for_you(routine: &Value) -> Option<&'static str> {
    if routine.get("schedule").is_none_or(Value::is_null) || routine["route"]["mode"] != "approve-each" {
        return None;
    }
    Some("Runs in Approve each: fired while you are away, it waits for you on its first card. Ask or Accept edits runs through.")
}
