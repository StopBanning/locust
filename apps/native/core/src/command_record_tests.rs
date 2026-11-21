//! command-record.test.ts (7 cases). Fixed 20 Nov: the record wrote the command's arguments verbatim, so a runtime
//! that takes the prompt as an argument (Copilot after -p, OpenCode as a positional) wrote the whole prompt into the
//! command a second time, unbounded. The executable and cwd half: Rust's record has no cwd, so that line is not here.
use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| (*a).to_owned()).collect()
}

// case: keeps the flags, which is the whole point
#[test]
fn keeps_the_flags() {
    let kept = recordable_args(&args(&["/d", "/s", "/c", "copilot.cmd", "--output-format", "json", "--deny-tool=write,shell"]), "Edit the file");
    assert!(kept.contains(&"--deny-tool=write,shell".to_owned()) && kept.contains(&"--output-format".to_owned()));
}

// case: never records the prompt, whatever flag introduced it
#[test]
fn never_records_the_prompt() {
    let secret = "Rewrite the billing handler and mention hunter2";
    assert_eq!(recordable_args(&args(&["-p", secret, "--no-color"]), secret), args(&["-p", "<prompt>", "--no-color"]));
    assert_eq!(recordable_args(&args(&["run", secret]), secret), args(&["run", "<prompt>"]));
}

// case: matches the prompt after trimming, the way a builder may have passed it
#[test]
fn matches_the_prompt_after_trimming() {
    assert_eq!(recordable_args(&args(&["-p", "  do the thing  "]), "do the thing"), args(&["-p", "<prompt>"]));
}

// case: leaves a flag alone that merely contains the prompt as a substring
#[test]
fn leaves_a_flag_that_contains_the_prompt() {
    assert_eq!(recordable_args(&args(&["--model", "auto-ok"]), "ok"), args(&["--model", "auto-ok"]));
}

// case: records nothing as the prompt when there is no prompt
#[test]
fn records_nothing_as_the_prompt_when_there_is_none() {
    assert_eq!(recordable_args(&args(&["--flag", ""]), "   "), args(&["--flag", ""]));
}

// case: truncates an overlong argument rather than dropping it
#[test]
fn truncates_an_overlong_argument() {
    let kept = recordable_args(&args(&["--config", &"x".repeat(900)]), "p");
    assert_eq!(kept[1].encode_utf16().count(), 512);
    assert!(kept[1].ends_with('…'));
    assert_eq!(recordable_args(&args(&["--config", &"x".repeat(512)]), "p")[1].len(), 512, "512 is kept whole");
}

// case: bounds how many arguments it will keep, and says how many it dropped
#[test]
fn bounds_how_many_arguments_it_keeps() {
    let many: Vec<String> = (0..70).map(|i| format!("--flag{i}")).collect();
    let kept = recordable_args(&many, "p");
    assert_eq!(kept.len(), 65);
    assert_eq!(kept.last().map(String::as_str), Some("… 6 more"));
    assert_eq!(recordable_args(&many[..64], "p").len(), 64, "64 says nothing more");
}

#[test]
fn the_ledger_records_the_command_without_its_prompt() {
    let root = std::env::temp_dir().join(format!("locust-command-record-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = crate::store::Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    let prompt = "Rewrite the billing handler and mention hunter2";
    let writer = LedgerWriter::create_runtime(&store, "mission_cmd", prompt, "account-default", "copilot", "copilot", &args(&["-p", prompt, "--no-color"]), None).unwrap();
    let first = std::fs::read_to_string(&writer.path).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    let created: serde_json::Value = serde_json::from_str(first.lines().next().unwrap()).unwrap();
    assert_eq!(created["metadata"]["command"]["args"], serde_json::json!(["-p", "<prompt>", "--no-color"]));
    assert_eq!(created["metadata"]["prompt"], prompt, "the prompt is kept once, as the turn's own");
}
