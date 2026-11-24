//! Electron's `the-flat-sidebar-is-written-down.test.ts`, "the title is cut by the column, not before it": a
//! conversation's title is what the person typed, its first line, and is not capped before the sidebar's column cuts it.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn turn(dir: &Path, id: &str, prompt: &str) {
    let at = "2026-09-20T12:00:00.000Z";
    let metadata = serde_json::json!({"missionId": id, "runId": format!("run_{id}"), "prompt": prompt, "runtime": "codex", "model": "account-default",
        "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0", "workspaceId": "ws_test",
        "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": at});
    let created = serde_json::json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": at, "metadata": metadata});
    std::fs::write(dir.join(format!("{id}.jsonl")), format!("{created}\n")).unwrap();
}

/// The titles of a listed folder of turns, as the window reads them (`try_list_recent`).
fn listed_titles(prompts: &[(&str, &str)]) -> Vec<(String, String)> {
    let dir = std::env::temp_dir().join(format!("locust-mission-title-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let _t = Temp(dir.clone());
    for (id, prompt) in prompts {
        turn(&dir, id, prompt);
    }
    let mut out: Vec<_> = try_list_recent(&dir, usize::MAX).unwrap().into_iter().map(|s| (s.mission_id, s.title)).collect();
    out.sort();
    out
}

const LONG: &str = "Go through every file under apps/native/src/ui and tell me which of them still draws the old two-line conversation row";

#[test]
fn does_not_cap_the_title_in_javascript() {
    // case: does not cap the title in JavaScript
    assert!(LONG.chars().count() > 60);
    assert_eq!(mission_title(LONG), LONG);
    // And the list the sidebar reads carries it whole.
    assert_eq!(listed_titles(&[("mission_long", LONG)]), vec![("mission_long".to_owned(), LONG.to_owned())]);
}

#[test]
fn still_takes_the_first_line_and_drops_the_attachment_preamble() {
    // case: still takes the first line and drops the attachment preamble
    let sent = "Read this file in the workspace before you answer:\n- notes/plan.md\n\nFix the parser.\nIt fails on tabs.";
    assert_eq!(mission_title(sent), "Fix the parser.");
    assert_eq!(mission_title("  \nWhat changed?\nAnd why?"), "What changed?");
    // A first line that is only a path is titled by its file (pathTitle.ts).
    assert_eq!(mission_title("/Users/locust/work/handoffs/HANDOFF-menu.md\nread it"), "HANDOFF-menu.md");
    assert_eq!(
        listed_titles(&[("mission_file", sent), ("mission_two", "First line\nsecond line")]),
        vec![("mission_file".to_owned(), "Fix the parser.".to_owned()), ("mission_two".to_owned(), "First line".to_owned())]
    );
}
