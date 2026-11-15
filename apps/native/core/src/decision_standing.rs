//! The decision card states guarantees and observed work, never guesses from tool names.
use serde_json::Value;
pub fn decision_standing(sandbox: Option<&str>, events: &[Value]) -> &'static str {
    if sandbox == Some("read-only") { return "stopped here · nothing was changed"; }
    let wrote = events.iter().any(|e| matches!(e["type"].as_str(), Some("tool.completed" | "tool.failed")) && e["payload"].get("patch").is_some());
    if wrote { "stopped here · work already done is kept" } else { "stopped here · this run could edit files" }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn claims_nothing_changed_only_when_the_run_could_not_write() {
        assert!(decision_standing(Some("read-only"), &[]).contains("nothing was changed"));
    }
    #[test]
    fn says_work_is_kept_when_a_patch_actually_came_back() {
        let wrote = [json!({"type":"tool.completed","payload":{"itemId":"t1","toolKind":"edit","name":"apply","phase":"completed","patch":"diff"}})];
        assert!(decision_standing(Some("workspace-write"), &wrote).contains("work already done is kept"));
    }
    #[test]
    fn never_claims_nothing_changed_for_a_run_that_was_allowed_to_write() {
        let said = decision_standing(Some("workspace-write"), &[]);
        assert!(!said.contains("nothing was changed"));
        assert!(said.contains("could edit files"));
    }
    #[test]
    fn does_not_claim_nothing_changed_when_the_mode_is_unknown() {
        assert!(!decision_standing(None, &[]).contains("nothing was changed"));
    }
    #[test]
    fn a_failed_tool_patch_is_evidence_but_a_started_or_merely_named_edit_is_not() {
        assert_eq!(decision_standing(None, &[json!({"type":"tool.failed","payload":{"patch":null}})]), "stopped here · work already done is kept");
        for event in [json!({"type":"tool.started","payload":{"patch":"diff"}}), json!({"type":"tool.completed","payload":{"name":"write"}})] {
            assert_eq!(decision_standing(Some("full-access"), &[event]), "stopped here · this run could edit files");
        }
    }
}
