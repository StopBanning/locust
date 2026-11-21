//! Electron's `main/usage-carried-from-another-computer.test.ts` (4 cases). Fixed 20 Nov: Rust never read the
//! numbers-only `usage-carried.jsonl`, so Usage left out every turn carried from another computer.
use super::*;
use crate::usage::{UsageRange, usage_summary};
use serde_json::json;

fn turn() -> Value {
    json!({"at": "2026-10-01T10:00:00.000Z", "runtime": "claude", "model": "claude-opus-5-5", "conversation": "mission_a",
        "inputTokens": 1200, "outputTokens": 300, "cacheReadTokens": 900, "plan": true})
}

fn with(key: &str, value: Value) -> Value {
    let mut row = turn();
    row[key] = value;
    row
}

fn expected() -> UsageTurn {
    UsageTurn {
        at: "2026-10-01T10:00:00.000Z".into(),
        runtime: "claude".into(),
        model: "claude-opus-5-5".into(),
        conversation: "mission_a".into(),
        input_tokens: Some(1200),
        output_tokens: Some(300),
        cache_read_tokens: Some(900),
        cache_write_tokens: None,
        usd: None,
        premium_requests: None,
        plan: true,
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let dir = std::env::temp_dir().join(format!("locust-usage-carried-{tag}-{}", crate::invariants::unique_nanos()));
    fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

// case: keeps a turn of numbers, and nothing it does not name
#[test]
fn keeps_a_turn_of_numbers_and_nothing_it_does_not_name() {
    let mut row = turn();
    row["prompt"] = json!("secret words");
    row["teammate"] = json!("Wren");
    assert_eq!(carried_turn(&row), Some(expected()));
    // The numbers it does name all come through; money and premium requests too.
    let mut all = turn();
    all["cacheWriteTokens"] = json!(40);
    all["usd"] = json!(0.25);
    all["premiumRequests"] = json!(2);
    let kept = carried_turn(&all).unwrap();
    assert_eq!((kept.cache_write_tokens, kept.usd, kept.premium_requests), (Some(40), Some(0.25), Some(2.0)));
    // A turn with no plan says so.
    let mut unplanned = turn();
    unplanned.as_object_mut().unwrap().remove("plan");
    assert!(!carried_turn(&unplanned).unwrap().plan);
}

// case: leaves out a line that is not a turn
#[test]
fn leaves_out_a_line_that_is_not_a_turn() {
    assert_eq!(carried_turn(&with("at", json!("yesterday"))), None);
    assert_eq!(carried_turn(&with("runtime", json!("Claude Code"))), None);
    assert_eq!(carried_turn(&with("inputTokens", json!(-1))), None);
    assert_eq!(carried_turn(&with("usd", json!("free"))), None);
    assert_eq!(carried_turn(&with("plan", json!("yes"))), None);
    assert_eq!(carried_turn(&Value::Null), None);
    // And the lengths usage-carried.ts allows: a time past 40 characters, a model or conversation past 200, or none.
    assert_eq!(carried_turn(&with("at", json!(format!("2026-10-01T10:00:00.000Z{}", " ".repeat(20))))), None);
    assert_eq!(carried_turn(&with("model", json!("m".repeat(201)))), None);
    assert!(carried_turn(&with("model", json!("m".repeat(200)))).is_some());
    assert_eq!(carried_turn(&with("conversation", json!(""))), None);
    assert_eq!(carried_turn(&with("runtime", json!("r".repeat(41)))), None);
    assert_eq!(carried_turn(&with("plan", json!(false))), None);
    assert_eq!(carried_turn(&json!([turn()])), None);
}

// case: reads every good line of the file, and no file as none
#[test]
fn reads_every_good_line_of_the_file_and_no_file_as_none() {
    let dir = scratch("file");
    let path = dir.0.join(USAGE_CARRIED_FILE);
    assert_eq!(carried_usage(&path), Vec::new());
    let lines = [turn().to_string(), "not json".into(), with("at", json!("x")).to_string(), String::new(),
        with("conversation", json!("mission_b")).to_string()];
    fs::write(&path, lines.join("\n")).unwrap();
    let carried = carried_usage(&path);
    assert_eq!(carried.len(), 2);
    let now = chrono::TimeZone::from_utc_datetime(&chrono::Local, &chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap().and_hms_opt(0, 0, 0).unwrap());
    assert_eq!(usage_summary(&carried, UsageRange::All, now).turns, 2);
}

// case: is added to the ledger where Usage is read
#[test]
fn is_added_to_the_ledger_where_usage_is_read() {
    let dir = scratch("store");
    for folder in ["native", "electron"] {
        fs::create_dir_all(dir.0.join(folder)).unwrap();
    }
    let store = Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    fs::write(store.electron().join(USAGE_CARRIED_FILE), format!("{}\n", turn())).unwrap();
    fs::write(store.root().join(USAGE_CARRIED_FILE), format!("{}\n", with("conversation", json!("mission_here")))).unwrap();
    let ledger_only = usage_turns(&store).unwrap();
    let with_carried = usage_turns_with_carried(&store).unwrap();
    assert_eq!(with_carried.len(), ledger_only.len() + 2);
    let conversations: Vec<&str> = with_carried.iter().map(|t| t.conversation.as_str()).collect();
    assert!(conversations.contains(&"mission_a") && conversations.contains(&"mission_here"), "{conversations:?}");
    // Wherever Usage is read: the Usage panel and Home's usage line, never the ledger-only turns.
    for (name, source) in [("usage_panel", include_str!("../../src/ui/usage_panel.rs")), ("usage_line", include_str!("../../src/ui/usage_line.rs"))] {
        assert!(source.contains("usage_ledger::usage_turns_with_carried("), "{name} leaves out carried usage");
        assert!(!source.contains("usage_ledger::usage_turns("), "{name} still reads the ledger alone");
    }
}
