//! Electron missionView.test.ts through the tool and diagnostic rows that reach the thread.
use super::*;
use rune::{Env, Runtime};
use serde_json::{Value, json};

const PS: &str = r#""C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe""#;
const AT: &str = "2026-09-03T20:00:00.000Z";

fn window(items: Vec<ledger::Item>) -> Runtime<State> {
    let mut s = tests_live_state();
    let pane = s.ledger.as_mut().unwrap();
    pane.snapshot.running = false;
    pane.snapshot.live = None;
    pane.conversation.conversation_id = Some("mission_1".into());
    pane.items = items;
    let mut rt = Runtime::new(
        s,
        |s, _| {
            message_column(
                &s.ledger.as_ref().unwrap().items,
                "Codec",
                &Palette::new(true),
                s,
            )
        },
        (900.0, 1000.0),
        Env::new(true, 1.0),
    );
    frame(&mut rt);
    rt
}

fn frame(rt: &mut Runtime<State>) {
    for _ in 0..8 {
        rt.frame(0.05);
    }
}

fn command_row(input: &str, expected: &str) {
    let mut rt = window(vec![
        ledger::Item::User("Run this".into()),
        ledger::Item::Tool {
            name: "shell".into(),
            summary: input.into(),
            status: ledger::Status::Done,
            detail: Box::new(ledger::ToolDetail {
                id: "cmd".into(),
                exit_code: Some(0),
                finished: true,
                ..Default::default()
            }),
        },
    ]);
    let (x, y) = rt
        .find("fold/mission_1/group/cmd")
        .expect("the actual activity disclosure");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    frame(&mut rt);
    let words: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| e.text)
        .collect();
    assert!(words.iter().any(|w| w == expected), "{words:?}");
    if input != expected {
        assert!(!words.iter().any(|w| w.contains(input)), "{words:?}");
    }
}

// case: shows what ran, not the host that ran it
#[test]
fn shell_row_shows_the_command_not_the_host() {
    command_row(&format!(r#"{PS} -Command "npm test""#), "npm test");
    command_row(
        &format!("{PS} -NoProfile -NonInteractive -Command 'node --test'"),
        "node --test",
    );
}

// case: keeps a command that was never wrapped
#[test]
fn shell_row_keeps_an_unwrapped_command() {
    command_row("pnpm build", "pnpm build");
    command_row("git diff -- src/streak.js", "git diff -- src/streak.js");
}

// case: unwraps a cmd.exe host too
#[test]
fn shell_row_unwraps_cmd() {
    command_row(
        r#"C:\Windows\System32\cmd.exe /d /s /c "npm run build""#,
        "npm run build",
    );
}

// case: undoes the quote doubling the host introduced
#[test]
fn shell_row_undoes_the_hosts_doubled_quotes() {
    command_row(
        &format!(r#"{PS} -Command "rg -n ""streak"" src""#),
        r#"rg -n "streak" src"#,
    );
}

// case: shows the command as the shell got it, when Codex escaped every backslash
#[test]
fn shell_row_shows_the_real_backslashes_of_a_fully_escaped_command() {
    command_row(
        r#""C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "Get-Content 'C:\\Users\\me\\.codex\\skills\\imagegen\\SKILL.md'; Get-Content notes.md""#,
        r"Get-Content 'C:\Users\me\.codex\skills\imagegen\SKILL.md'; Get-Content notes.md",
    );
}

// case: leaves a command alone when one backslash is not half of a pair
#[test]
fn shell_row_keeps_backslashes_that_were_not_escaped() {
    command_row(
        r#""C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "Get-Content C:\Users\me\notes.md""#,
        r"Get-Content C:\Users\me\notes.md",
    );
}

// case: still undoes an escaped quote inside an escaped payload
#[test]
fn shell_row_undoes_escaped_quotes_inside_the_escaped_payload() {
    command_row(
        r#""C:\\Windows\\powershell.exe" -Command "rg \"streak\" C:\\work""#,
        r#"rg "streak" C:\work"#,
    );
}

fn notices(events: &[(&str, Value)]) -> Vec<String> {
    let mut transcript = ledger::Transcript::default();
    transcript.apply(
        events
            .iter()
            .enumerate()
            .map(|(n, (kind, payload))| {
                json!({
                    "recordType":"mission.event","event":{
                        "id":format!("e{n}"),"runId":"run_1","missionId":"mission_1","sequence":n+1,
                        "type":kind,"occurredAt":AT,"sourceAdapter":"codex","payload":payload
                    }
                })
            })
            .collect(),
    );
    window(transcript.items)
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect()
}

// case: names why a run failed before it said anything
#[test]
fn a_failed_turn_without_work_has_the_actual_reason_in_its_thread() {
    let why = "Antigravity could not run it: model no-such-model is not recognized";
    let shown = notices(&[(
        "run.failed",
        json!({"kind":"process-failed","message":why,"runtimeTerminal":"failed","process":{}}),
    )]);
    assert!(shown.iter().any(|t| t == why), "{shown:?}");
}

// case: does not say a failure twice when a diagnostic already carries it (control)
#[test]
fn one_runtime_failure_is_not_drawn_twice_in_the_thread() {
    let why = "The provider turned the request away.";
    let shown = notices(&[
        (
            "adapter.diagnostic",
            json!({"level":"error","code":"codex.runtime_error","message":why,"terminal":true}),
        ),
        (
            "run.failed",
            json!({"kind":"process-failed","message":why,"runtimeTerminal":"failed","process":{}}),
        ),
    ]);
    assert_eq!(
        shown.iter().filter(|t| t.as_str() == why).count(),
        1,
        "{shown:?}"
    );
}

fn printed(output: &str) -> Vec<String> {
    let mut rt = window(vec![
        ledger::Item::User("Print the run output".into()),
        ledger::Item::Tool {
            name: "shell".into(),
            summary: "printrun".into(),
            status: ledger::Status::Done,
            detail: Box::new(ledger::ToolDetail {
                id: "cmd".into(),
                exit_code: Some(0),
                finished: true,
                output: output.into(),
                output_reported: true,
                ..Default::default()
            }),
        },
    ]);
    for key in ["fold/mission_1/group/cmd", "mission_1/cmd"] {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        frame(&mut rt);
    }
    rt.elements()
        .into_iter()
        .filter(|el| !el.exiting)
        .filter_map(|el| el.text)
        .collect()
}

fn numbered(count: usize) -> String {
    (1..=count)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// case: keeps a short output whole, and asks for no elision control
#[test]
fn shell_drawer_keeps_short_output_without_a_more_button() {
    let text = printed("one\ntwo\nthree");
    assert!(text.iter().any(|el| el == "one\ntwo\nthree"), "{text:?}");
    assert!(!text.iter().any(|el| el.contains("more lines")), "{text:?}");
}

// case: keeps BOTH ends of a long one, as two halves the caller can put a control between
#[test]
fn shell_drawer_keeps_eight_lines_at_both_ends_with_a_control_between() {
    let text = printed(&numbered(1_200));
    let head = text
        .iter()
        .position(|el| el.starts_with("line 1\n"))
        .unwrap();
    let control = text.iter().position(|el| el == "1184 more lines").unwrap();
    let tail = text
        .iter()
        .position(|el| el.ends_with("line 1200"))
        .unwrap();
    assert!(head < control && control < tail, "{text:?}");
    assert_eq!(text[head].lines().count(), 8);
    assert_eq!(text[tail].lines().count(), 8);
}

// case: bounds by LINES rather than pixels, so nothing needs an inner scroller
#[test]
fn shell_drawer_folds_at_seventeen_lines_and_not_sixteen() {
    let sixteen = numbered(16);
    let short = printed(&sixteen);
    assert!(short.contains(&sixteen), "{short:?}");
    assert!(
        !short.iter().any(|el| el.contains("more lines")),
        "{short:?}"
    );
    let long = printed(&numbered(17));
    assert!(long.iter().any(|el| el == "1 more lines"), "{long:?}");
}

// case: does not end on the blank line almost every command leaves behind
#[test]
fn shell_drawer_does_not_spend_its_last_line_on_the_trailing_newline() {
    let text = printed(&format!("{}\n", numbered(400)));
    assert!(text.iter().any(|el| el.ends_with("line 400")), "{text:?}");
}

// case: says how much it left out rather than trailing off
#[test]
fn shell_drawer_names_the_actual_omitted_count() {
    let text = printed(&numbered(500));
    assert!(text.iter().any(|el| el == "484 more lines"), "{text:?}");
}

// case: says capacity exhaustion in English rather than passing the jargon through alone
#[test]
fn capacity_failure_keeps_the_runtimes_own_words_beneath_plain_english() {
    let shown = notices(&[(
        "run.failed",
        json!({
            "message":"Cursor Agent ended without a terminal result record.",
            "process":{"stderr":"RetriableError: [resource_exhausted] Error\n"}
        }),
    )]);
    let words = shown.join("\n");
    assert!(
        words.contains("out of capacity right now") && words.contains("resource_exhausted"),
        "{words:?}"
    );
}
