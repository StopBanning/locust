//! Model names earned from ledger evidence, never inferred from an alias.
use crate::ledger::Summary;
use serde_json::Value;
use std::collections::HashMap;
pub fn reported_model(events: &[Value], requested: &str) -> Option<String> {
    let completed = events.iter().find(|e| e["type"] == "run.completed");
    let started = events.iter().find(|e| e["type"] == "run.started");
    completed.and_then(|e| e["payload"]["resolvedModel"].as_str())
        .or_else(|| started.and_then(|e| e["payload"]["reportedModel"].as_str().or_else(|| e["payload"]["evidence"]["raw"]["model"].as_str())))
        .filter(|name| !name.is_empty() && *name != requested).map(crate::ledger::scrub)
}
pub fn resolved_model_names<'a>(missions: impl IntoIterator<Item = &'a Summary>) -> HashMap<String, String> {
    let mut held: HashMap<String, (i64, String)> = HashMap::new();
    for m in missions {
        let Some(name) = &m.reported_model else { continue };
        let key = format!("{}:{}", m.runtime, m.model);
        let at = chrono::DateTime::parse_from_rfc3339(&m.created_at).ok().map(|t| t.timestamp_millis()).unwrap_or(0);
        if held.get(&key).is_none_or(|(old, _)| at > *old) {
            held.insert(key, (at, name.clone()));
        }
    }
    held.into_iter().map(|(key, (_, name))| (key, name)).collect()
}

/// One run as `live_model_names` reads it: its runtime, requested model, and events so far.
pub struct LiveRun<'a> {
    pub runtime: Option<&'a str>,
    pub model: Option<&'a str>,
    pub events: &'a [Value],
}

/// `missionView.ts` `liveModelNames`: which model a run under way is on, from its start record.
pub fn live_model_names<'a>(runs: impl IntoIterator<Item = LiveRun<'a>>) -> HashMap<String, String> {
    let mut named = HashMap::new();
    for run in runs {
        let (Some(runtime), Some(model)) = (run.runtime, run.model) else { continue };
        let Some(started) = run.events.iter().find(|e| e["type"] == "run.started") else { continue };
        let Some(raw_model) = started["payload"]["evidence"]["raw"]["model"].as_str() else { continue };
        if !raw_model.is_empty() && raw_model != model {
            named.insert(format!("{runtime}:{model}"), crate::ledger::scrub(raw_model));
        }
    }
    named
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn ran_on(id: &str, model: &str, resolved: Option<&str>, at: &str) -> Summary {
        let events = vec![json!({"type":"run.started","payload":{"runtimeThreadId":"thread","evidence":{"redacted":true,"raw":resolved.map(|model|json!({"model":model})).unwrap_or(json!({}))}}})];
        Summary { mission_id:id.into(), runtime:"claude".into(), model:model.into(), created_at:at.into(), reported_model:reported_model(&events, model), ..Default::default() }
    }
    #[test]
    fn learns_the_real_name_the_runtime_reported_for_an_alias() {
        let m = ran_on("m1", "fable", Some("claude-fable-5-1"), "2026-09-01T10:00:00.000Z");
        assert_eq!(resolved_model_names([&m]).get("claude:fable").map(String::as_str), Some("claude-fable-5-1"));
    }
    #[test]
    fn prefers_the_newest_mission_so_a_new_release_replaces_an_old_name() {
        let new = ran_on("new", "fable", Some("claude-fable-5-1"), "2026-09-01T10:00:00.000Z");
        let old = ran_on("old", "fable", Some("claude-fable-5"), "2026-08-01T10:00:00.000Z");
        assert_eq!(resolved_model_names([&new, &old]).get("claude:fable").map(String::as_str), Some("claude-fable-5-1"));
    }
    #[test]
    fn says_nothing_about_an_alias_nobody_has_run_or_one_that_taught_it_nothing() {
        assert!(resolved_model_names([]).is_empty());
        assert!(resolved_model_names([&ran_on("m1", "fable", None, "2026-09-01T10:00:00.000Z")]).is_empty());
        assert!(resolved_model_names([&ran_on("m2", "fable", Some("fable"), "2026-09-01T10:00:00.000Z")]).is_empty());
    }
    // missionView.test.ts: "the model an alias turned out to mean (0.35.2)".
    fn completed_mission(resolved: Option<&str>) -> Summary {
        let mut payload = json!({});
        if let Some(model) = resolved { payload["resolvedModel"] = json!(model); }
        let events = vec![json!({"type":"run.completed", "occurredAt":"2026-09-06T05:00:09.000Z", "payload":payload})];
        Summary { mission_id:"m1".into(), runtime:"claude".into(), model:"sonnet".into(),
            created_at:"2026-09-06T05:00:00.000Z".into(),
            reported_model:reported_model(&events, "sonnet"), ..Default::default() }
    }
    #[test]
    fn learns_it_from_the_result_which_is_the_only_record_that_states_it() {
        let m = completed_mission(Some("claude-sonnet-5"));
        assert_eq!(resolved_model_names([&m]).get("claude:sonnet").map(String::as_str), Some("claude-sonnet-5"));
    }
    #[test]
    fn learns_nothing_from_a_run_that_never_named_one() {
        let m = completed_mission(None);
        assert_eq!(resolved_model_names([&m]).get("claude:sonnet"), None);
    }

    // a-default-route-names-its-model.test.tsx
    #[test]
    fn names_the_model_of_a_run_under_way_from_its_start_record() {
        let started = json!({
            "type": "run.started",
            "payload": { "runtimeThreadId": "s1", "evidence": { "redacted": true, "raw": { "model": "claude-sonnet-5-5" } } }
        });
        let live = live_model_names([
            LiveRun { runtime: Some("claude"), model: Some("account-default"), events: std::slice::from_ref(&started) },
            // An alias the start only repeats teaches nothing; a run not yet started says nothing.
            LiveRun { runtime: Some("claude"), model: Some("claude-sonnet-5-5"), events: std::slice::from_ref(&started) },
            LiveRun { runtime: Some("codex"), model: Some("account-default"), events: &[] },
        ]);
        assert_eq!(live.len(), 1);
        assert_eq!(live.get("claude:account-default").map(String::as_str), Some("claude-sonnet-5-5"));
    }

}
