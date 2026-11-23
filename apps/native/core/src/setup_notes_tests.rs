//! Electron's `renderer/src/a-notice-is-said-once.test.ts`, "a CLI's remark about its own setup": known for what it is,
//! and what its runtime's row in Settings shows, from its newest run. The turn half is `ledger::tests::notice_once_tests`.
use super::*;
use serde_json::json;

const SKILLS: &str = "Skill descriptions were shortened to fit the skills context budget.";
const REMARK: &str = "The model is answering from a fallback region.";
const IGNORING: &str = "Codex is ignoring 1 unrecognized configuration setting. Check for typos or deprecated settings. user (config.toml): `computer_use.windows.always_allowed_app_ids` is ignored.";

fn said(messages: &[&str]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| json!({ "type": "adapter.diagnostic", "payload": { "code": "codex.item_notice", "level": "warning", "terminal": false, "message": message } }))
        .collect()
}

// case: is known for what it is, and nothing else is
#[test]
fn is_known_for_what_it_is_and_nothing_else_is() {
    assert!(is_setup_note(SKILLS));
    assert!(is_setup_note(IGNORING));
    assert!(is_setup_note("Codex is ignoring 2 unrecognized configuration settings."));
    assert!(is_setup_note("  skill descriptions were shortened to fit the budget"));
    assert!(!is_setup_note("Reconnecting... 2/5"));
    assert!(!is_setup_note(REMARK));
    assert!(!is_setup_note("Skill descriptions were shortened."));
}

// case: is what its runtime's row in Settings shows, from its newest run
#[test]
fn is_what_its_runtimes_row_in_settings_shows_from_its_newest_run() {
    let (older, newer, newest) = (said(&[SKILLS]), said(&[IGNORING, SKILLS, IGNORING]), said(&[REMARK]));
    let notes = latest_setup_notes([
        Run { runtime: "codex", created_at: "2026-09-23T10:00:00.000Z", events: &older },
        Run { runtime: "codex", created_at: "2026-09-23T12:00:00.000Z", events: &newer },
        Run { runtime: "codex", created_at: "2026-09-23T11:00:00.000Z", events: &newest },
    ]);
    assert_eq!(notes.get("codex").map(Vec::as_slice), Some([IGNORING.to_owned(), SKILLS.to_owned()].as_slice()));
    assert!(!notes.contains_key("claude"));
    // Read in the other order, the same newest run.
    let notes = latest_setup_notes([
        Run { runtime: "codex", created_at: "2026-09-23T12:00:00.000Z", events: &newer },
        Run { runtime: "codex", created_at: "2026-09-23T10:00:00.000Z", events: &older },
    ]);
    assert_eq!(notes["codex"], [IGNORING, SKILLS]);
    // The row says three at most, each in its runtime's name.
    let many: Vec<String> = (1..=5).map(|n| format!("Codex is ignoring {n} unrecognized configuration settings.")).collect();
    assert_eq!(row_lines("Codex", &many).len(), 3);
    assert_eq!(row_lines("Codex", &notes["codex"])[0], format!("Codex says: {IGNORING}"));
}
