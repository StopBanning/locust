//! Electron's main/codex-usage-is-read-with-the-models.test.ts (0.390): the catalog already starts Codex's app-server
//! for `model/list`, and in the same session it asks `account/rateLimits/read` -- no turn spent -- whose answer is the
//! snapshot a turn pushes, said in a turn's words with when it was read. The fake server is
//! `scripts/fixtures/m1-app-server.py` (`models-usage`, `models-usage-refused`, `models-usage-silent`).
use super::*;

const RESETS: i64 = 1790363478;

/// The session against the fake server, and the methods it was asked, in order.
fn read(scenario: &str) -> (Option<(Value, Option<String>)>, String) {
    let asked = std::env::temp_dir().join(format!("locust-codex-asked-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let mut command = Command::new("python3");
    command.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py")).arg(scenario).env("M1_ASKED_FILE", &asked);
    let read = codex_session(command, Duration::from_secs(20));
    let said = std::fs::read_to_string(&asked).unwrap_or_default();
    let _ = std::fs::remove_file(&asked);
    (read, said)
}

fn iso(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

// case: comes back as a reading beside the models, in the words a turn’s reading uses
#[test]
fn comes_back_as_a_reading_beside_the_models_in_the_words_a_turns_reading_uses() {
    let (read, asked) = read("models-usage");
    let (models, codex_usage) = read.expect("the models were read");
    let listed = Listed { codex: Some(models), codex_usage, ..Default::default() };
    assert!(catalog(&listed).iter().any(|m| m["id"] == "gpt-6-luna"));
    // 0.406: read from the account, and said so with when -- " · as of <now>".
    let reading = usage_windows(&listed).remove("codex").unwrap_or_default();
    let head = format!("weekly window 71% used \u{b7} resets {} \u{b7} 5-hour window 34% used \u{b7} resets {} \u{b7} as of ", iso(RESETS + 86400), iso(RESETS));
    assert!(reading.starts_with(&head), "{reading}");
    let as_of = chrono::DateTime::parse_from_rfc3339(&reading[head.len()..]).expect("as of a time");
    assert!((chrono::Utc::now() - as_of.with_timezone(&chrono::Utc)).num_milliseconds().abs() < 60_000);
    // In the same server session as the models: one server, two questions.
    assert_eq!(asked, "initialize\nmodel/list\naccount/rateLimits/read\n");
}

// case: leaves the reading out -- and keeps the models -- when Codex cannot say
#[test]
fn leaves_the_reading_out_and_keeps_the_models_when_codex_cannot_say() {
    let (refused, _) = read("models-usage-refused");
    let (models, codex_usage) = refused.expect("the models are kept");
    let listed = Listed { codex: Some(models), codex_usage, ..Default::default() };
    assert!(catalog(&listed).iter().any(|m| m["id"] == "gpt-6-luna"));
    assert!(usage_windows(&listed).is_empty());

    let (silent, _) = read("models-usage-silent");
    let (_, codex_usage) = silent.expect("the models are kept");
    assert_eq!(codex_usage, None);
}
