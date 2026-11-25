//! Parity tests for `apps/desktop/src/main/no-teammate-ends-your-browser.test.ts` (19 cases).

use super::*;
use crate::approval::Mode;
use crate::runtime::claude::{self, PrintOptions};
use crate::runtime::claude_events::{LOCUST_GUARD_SAID, guarded_sentence};
use std::collections::BTreeMap;

fn empty_ctx() -> GuardContext {
    GuardContext { fix_copy: false, started_pids: Vec::new() }
}

fn row(ppid: u32, name: &str, line: &str, created: u64) -> GuardedProcess {
    GuardedProcess { ppid, name: name.into(), line: line.into(), created }
}

fn table() -> BTreeMap<u32, GuardedProcess> {
    BTreeMap::from([
        (6000, row(5900, "explorer.exe", "", 1000)),
        (10544, row(6000, "chrome.exe", r#""C:\Program Files\Google\Chrome\Application\chrome.exe""#, 2000)),
        (29988, row(10544, "chrome.exe", r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --type=crashpad-handler"#, 2001)),
        (12420, row(6000, "chrome.exe", r#""C:\Program Files\Google\Chrome\Application\chrome.exe""#, 5000)),
        (11000, row(1111, "chrome.exe", "chrome.exe", 1500)),
        (8200, row(6000, "node.exe", "node server.js", 2500)),
        (8300, row(6000, "Code.exe", "", 2600)),
        (7000, row(6000, "Locust.exe", "", 3000)),
        (7100, row(7000, "claude.exe", "", 4000)),
        (7200, row(7100, "bash.exe", "", 4100)),
        (7300, row(7200, "chrome.exe", r"chrome.exe --headless=new --user-data-dir=C:\tmp\shot", 4200)),
        (7310, row(7300, "chrome.exe", "chrome.exe --type=renderer", 4201)),
        (7400, row(7200, "node.exe", "node dev-server.mjs", 4300)),
        (7500, row(7777, "chrome.exe", r"chrome.exe --user-data-dir=C:\tmp\headed", 4500)),
        (7510, row(7500, "chrome.exe", "chrome.exe --type=gpu-process", 4501)),
        (7600, row(7650, "vivaldi.exe", "vivaldi.exe keep.js --user-data-dir=./test-profile", 3900)),
        (8100, row(7000, "node.exe", r"node C:\Users\jane\AppData\Roaming\npm\node_modules\@anthropic-ai\claude-code\cli.js", 3500)),
        (8150, row(8100, "chrome.exe", r"chrome.exe --user-data-dir=C:\tmp\theirs", 3550)),
        (8400, row(6000, "msedge.exe", "msedge.exe --headless --dump-dom", 3600)),
        (8999, row(7100, "bash.exe", "", 6000)),
        (9000, row(8999, "electron.exe", "", 6001)),
    ])
}

fn look() -> Option<ProcessLook> {
    Some(ProcessLook { table: table(), root: 7000, agent: Some(7100), above: vec![8999, 7100, 7000, 6000] })
}

fn no_table() -> Option<ProcessLook> {
    None
}

// case: is stopped as round 4 ran it, and in every spelling of the same thing
#[test]
fn stopped_by_name_in_every_spelling() {
    for command in [
        "taskkill //F //IM msedge.exe",
        "taskkill //F //IM chrome.exe",
        "taskkill /F /IM chrome.exe /T",
        "taskkill /IM chrome.exe /F",
        "TASKKILL //IM CHROME.EXE //F",
        "cd out && taskkill //F //IM msedge.exe 2>/dev/null; echo done",
        r#"taskkill /F /FI "IMAGENAME eq chrome.exe""#,
        r#"cmd //c "taskkill /F /IM msedge.exe""#,
        r#"powershell -NoProfile -Command "Stop-Process -Name chrome -Force""#,
        r#"powershell -c "Stop-Process -Name node,msedge -ErrorAction SilentlyContinue""#,
        r#"powershell -c "Get-Process msedge | Stop-Process -Force""#,
        r#"powershell -c "(Get-Process -Name chrome).Kill()""#,
        r#"pkill -f "Google Chrome""#,
        "killall Safari",
        "pkill node",
        "pgrep -f chrome | xargs kill -9",
        "ps aux | grep firefox | awk '{print $2}' | xargs kill",
        "kill $(pgrep chromium)",
        r#"wmic process where "name='chrome.exe'" delete"#,
        r#"powershell -c "Get-CimInstance Win32_Process -Filter \"Name = 'msedge.exe'\" | Invoke-CimMethod -MethodName Terminate""#,
        r#"osascript -e 'quit app "Google Chrome"'"#,
        "taskkill /F /IM node.exe",
        "taskkill /F /IM Locust.exe",
        "taskkill /F /IM explorer.exe",
        "taskkill /F /IM msedgewebview2.exe",
        "taskkill /F /IM claude.exe",
        r#"for p in $(tasklist //FI "IMAGENAME eq chrome.exe" //NH | awk '{print $2}'); do taskkill //F //PID $p; done"#,
        r#"powershell -c "foreach ($p in Get-Process chrome) { $p.Kill() }""#,
    ] {
        let got = stopped_because_with(command, &empty_ctx(), Some(&no_table));
        assert!(
            got.as_deref().is_some_and(|s| s.starts_with("Locust stopped this command before it ran: it ends ")),
            "{command}: got {got:?}"
        );
    }
}

// case: says what it would have ended, to leave it running, and never to go and find its ids
#[test]
fn says_what_it_would_have_ended_and_never_mentions_pid() {
    assert_eq!(
        stopped_because_with("taskkill //F //IM chrome.exe", &empty_ctx(), Some(&no_table)).as_deref(),
        Some(
            "Locust stopped this command before it ran: it ends Chrome by name, which ends every Chrome on this computer, the person's own windows too. Leave the person's Chrome running. A test browser you start needs a profile of its own (--user-data-dir=<a new folder>), so it never meets theirs."
        )
    );
    assert_eq!(
        stopped_because_with("taskkill /F /IM node.exe", &empty_ctx(), Some(&no_table)).as_deref(),
        Some(
            "Locust stopped this command before it ran: it ends Node by name, which ends every Node on this computer, the person's own programs and other teammates' work too. Leave the person's Node running. End only the processes this run started; Locust checks."
        )
    );
    let chrome = stopped_because_with("taskkill //F //IM chrome.exe", &empty_ctx(), Some(&no_table)).unwrap();
    assert!(!chrome.to_lowercase().contains("pid") && !chrome.to_lowercase().contains("process id"));
}

// case: is stopped when the id is the person’s Chrome, as it was ended on 2026-10-10
#[test]
fn stopped_when_id_is_the_persons_chrome() {
    assert_eq!(
        stopped_because_with(
            "taskkill //F //PID 10544 >/dev/null 2>&1; taskkill //F //PID 29988 >/dev/null 2>&1; sleep 1",
            &empty_ctx(),
            Some(&look),
        )
        .as_deref(),
        Some(
            "Locust stopped this command before it ran: process 10544 is Chrome, which no teammate started, so it is the person's own. Leave it running. A test browser you start needs a profile of its own (--user-data-dir=<a new folder>), so it never meets theirs."
        )
    );
    for command in [
        r#"powershell -c "Stop-Process -Id 10544 -Force""#,
        r#"powershell -c "Stop-Process -Id 7300,10544""#,
        "kill -9 29988",
        r#"powershell -c "Stop-Process 10544""#,
        r#"node -e "process.kill(10544)""#,
        "wmic process where processid=10544 delete",
    ] {
        let got = stopped_because_with(command, &empty_ctx(), Some(&look)).unwrap_or_default();
        assert!(
            got.starts_with("Locust stopped this command before it ran: process 10544 is Chrome")
                || got.starts_with("Locust stopped this command before it ran: process 29988 is Chrome"),
            "{command}: {got}"
        );
    }
    assert!(
        stopped_because_with("taskkill //F //PID 12420", &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .starts_with("Locust stopped this command before it ran: process 12420 is Chrome")
    );
    assert!(
        stopped_because_with("taskkill //F //PID 11000", &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .starts_with("Locust stopped this command before it ran: process 11000 is Chrome")
    );
}

// case: is stopped for the person’s editor, another teammate’s agent, this run’s own agent, and Locust
#[test]
fn stopped_for_editor_other_agent_own_agent_and_locust() {
    assert!(
        stopped_because_with("taskkill //F //PID 8300", &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .contains("process 8300 is VS Code, which no teammate started, so it is the person's own.")
    );
    assert!(
        stopped_because_with("taskkill //F //PID 8100", &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .contains("process 8100 is an AI agent on Node, which this run did not start, so it is another teammate's or the person's.")
    );
    assert!(
        stopped_because_with("taskkill //F //PID 7100", &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .contains("process 7100 is the agent this run is")
    );
    assert!(
        stopped_because_with("taskkill //F //PID 7000", &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .contains("process 7000 is Locust itself")
    );
}

// case: runs when a teammate started it -- its own test browser, its own server -- or it is nobody’s to protect
#[test]
fn runs_when_teammate_started_it_or_unprotected() {
    for command in [
        "taskkill //F //PID 7300",
        "taskkill //F //PID 7310 //T",
        "kill 7400",
        "taskkill //F //PID 7500",
        "taskkill //F //PID 7510",
        "taskkill //F //PID 7600",
        "taskkill //F //PID 8150",
        "taskkill //F //PID 8400",
        r#"powershell -c "Stop-Process -Id 7400 -Force""#,
        "taskkill //PID 8200 //F",
        "taskkill //PID 99999 //F",
        "taskkill //F //PID $CHROME_PID",
        r#"tasklist //FI "IMAGENAME eq chrome.exe"; taskkill //F //PID 7300"#,
    ] {
        assert_eq!(stopped_because_with(command, &empty_ctx(), Some(&look)), None, "{command}");
    }
    assert_eq!(stopped_because_with("taskkill //F //PID 10544", &empty_ctx(), Some(&no_table)), None);
}

// case: reads the ids as each tool writes them
#[test]
fn reads_ids_as_each_tool_writes_them() {
    assert_eq!(ended_ids("taskkill //F //PID 10544 >/dev/null 2>&1; taskkill //F //PID 29988"), vec![10544, 29988]);
    assert_eq!(ended_ids("taskkill /PID 1 /PID 2 /T /F"), vec![1, 2]);
    assert_eq!(ended_ids(r#"powershell -c "Stop-Process -Id 3,4 -Force""#), vec![3, 4]);
    assert_eq!(ended_ids("kill -9 5 6 && kill -s TERM 7"), vec![5, 6, 7]);
    assert_eq!(ended_ids("kill %1; pkill 8; taskkill //IM 9.exe"), Vec::<u32>::new());
    assert_eq!(ended_ids(r#"node -e "process.kill(10)""#), vec![10]);
}

// case: reads ids written out and ended through a variable
#[test]
fn reads_ids_written_out_and_ended_through_variable() {
    assert_eq!(ended_ids("for p in 28700 30432 2888; do taskkill //F //PID $p >/dev/null 2>&1; done; sleep 1"), vec![28700, 30432, 2888]);
    assert_eq!(ended_ids(r#"for p in 11 12; do kill -9 "$p"; done"#), vec![11, 12]);
    assert_eq!(ended_ids(r#"powershell -c "foreach ($p in 13,14) { Stop-Process -Id $p -Force }""#), vec![13, 14]);
    assert_eq!(ended_ids(r#"powershell -c "15,16 | ForEach-Object { Stop-Process -Id $_ }""#), vec![15, 16]);
    assert_eq!(ended_ids("echo 17 18 | xargs kill"), vec![17, 18]);
    assert_eq!(ended_ids("for p in 19 20; do echo $p; done"), Vec::<u32>::new());
    assert!(
        stopped_because_with("for p in 7300 10544; do taskkill //F //T //PID $p; done", &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .contains("process 10544 is Chrome, which no teammate started")
    );
    assert_eq!(
        stopped_because_with("for p in 7300 7310 7500; do taskkill //F //T //PID $p; done", &empty_ctx(), Some(&look)),
        None
    );
}

const SONNET_FILTER: &str = r#"powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { \$_.Name -eq 'chrome.exe' -and \$_.CommandLine -like '*C:\tmp\headed*' } | ForEach-Object { Stop-Process -Id \$_.ProcessId -Force }""#;
const WQL_FILTER: &str = r#"powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='chrome.exe' AND CommandLine LIKE '%tmp\\shot%'\" | Invoke-CimMethod -MethodName Terminate""#;

// case: runs when every process the filter matches is one a teammate started
#[test]
fn narrowed_filter_runs_when_every_match_is_a_teammates() {
    assert_eq!(stopped_because_with(SONNET_FILTER, &empty_ctx(), Some(&look)), None);
    assert_eq!(stopped_because_with(WQL_FILTER, &empty_ctx(), Some(&look)), None);
}

// case: is stopped when the filter also matches the person’s own
#[test]
fn narrowed_filter_stopped_when_it_also_matches_the_persons_own() {
    let wide = r#"powershell -c "Get-CimInstance Win32_Process | Where-Object { \$_.Name -eq 'chrome.exe' -and \$_.CommandLine -like '*Chrome*' } | ForEach-Object { Stop-Process -Id \$_.ProcessId }""#;
    let got = stopped_because_with(wide, &empty_ctx(), Some(&look)).unwrap_or_default();
    assert!(
        ["10544", "29988", "12420", "11000"].iter().any(|pid| {
            got.starts_with(&format!(
                "Locust stopped this command before it ran: its command-line filter also matches process {pid}, Chrome that no teammate started"
            ))
        }),
        "got: {got}"
    );
}

// case: is stopped as before when the narrowing cannot be read exactly, or the table cannot be read
#[test]
fn narrowed_filter_falls_back_to_by_name_when_inexact_or_no_table() {
    for command in [
        SONNET_FILTER.replace(r"-like '*C:\tmp\headed*'", "-match 'headed'"),
        SONNET_FILTER.replace(r"-like '*C:\tmp\headed*'", "-like '*[h]eaded*'"),
        SONNET_FILTER.replace(" -and ", " -or "),
        format!("{SONNET_FILTER}; taskkill //F //IM chrome.exe"),
    ] {
        let got = stopped_because_with(&command, &empty_ctx(), Some(&look)).unwrap_or_default();
        assert!(
            got.starts_with("Locust stopped this command before it ran: it ends Chrome by name"),
            "{command}: {got}"
        );
    }
    assert!(
        stopped_because_with(SONNET_FILTER, &empty_ctx(), Some(&no_table))
            .unwrap_or_default()
            .starts_with("Locust stopped this command before it ran: it ends Chrome by name")
    );
}

// case: never lets a filter reach Locust or this run’s own agent
#[test]
fn filter_never_reaches_locust_or_own_agent() {
    let locust = r#"powershell -c "Get-CimInstance Win32_Process | Where-Object { \$_.Name -eq 'Locust.exe' -and \$_.CommandLine -like '*' } | ForEach-Object { Stop-Process -Id \$_.ProcessId }""#;
    assert!(
        stopped_because_with(locust, &empty_ctx(), Some(&look))
            .unwrap_or_default()
            .contains("also matches process 7000, Locust")
    );
}

// case: runs: starting a browser, and the ordinary work of a turn
#[test]
fn ordinary_work_and_starting_a_browser_run() {
    for command in [
        r#""C:/Program Files/Google/Chrome/Application/chrome.exe" --headless --screenshot=shot.png index.html"#,
        "start msedge http://localhost:8080",
        "node play.mjs bot.js example-bot.js",
        "npx playwright test",
        "git status && npm test",
        "grep -rn chrome src/ | head",
        r#"tasklist //FI "IMAGENAME eq chrome.exe""#,
        "ls ~/locust-scratch/node_modules",
        "codex --version",
        "",
    ] {
        assert_eq!(stopped_because_with(command, &empty_ctx(), Some(&look)), None, "{command}");
    }
}

// case: reads commands, not meaning: one that only writes such a command down is stopped too
#[test]
fn reads_commands_not_meaning() {
    assert!(stopped_because_with(r#"echo "taskkill /IM chrome.exe" >> notes.txt"#, &empty_ctx(), Some(&no_table)).is_some());
}

// case: is let through when the question cannot be read
#[test]
fn lets_unreadable_question_through() {
    assert_eq!(stopped_because_with("", &empty_ctx(), Some(&no_table)), None);
    assert_eq!(hook_reply("{}", "/"), None);
    assert_eq!(hook_reply(r#"{"tool_input":{"command":123}}"#, "/"), None);
}

// case: answers a refused command with a deny Claude Code reads, and anything else with nothing
#[test]
fn hook_answers_refused_with_deny_and_rest_with_nothing() {
    let refused = hook_reply(
        &serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": "taskkill //F //IM msedge.exe", "description": "Close the test browser" }
        })
        .to_string(),
        "/",
    )
    .expect("refused");
    let parsed: serde_json::Value = serde_json::from_str(&refused).unwrap();
    assert_eq!(
        parsed,
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": stopped_because_with("taskkill //F //IM msedge.exe", &empty_ctx(), Some(&no_table)).unwrap()
            }
        })
    );
    assert_eq!(
        hook_reply(
            &serde_json::json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"npm test"}}).to_string(),
            "/",
        ),
        None
    );
    assert_eq!(hook_reply("not json", "/"), None);
}

// case: reads this machine’s real process table when an id is ended, and leaves an id nobody runs alone
#[test]
fn hook_reads_real_process_table_and_leaves_absent_id_alone() {
    let seen = look_at_processes().expect("live process table");
    assert!(seen.table.contains_key(&std::process::id()));
    assert_eq!(
        hook_reply(
            &serde_json::json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"taskkill //F //PID 999999"}}).to_string(),
            "/",
        ),
        None
    );
}

// case: is given the guard in every mode, Auto included
#[test]
fn claude_run_is_given_the_guard_in_every_mode_auto_included() {
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits, Mode::Auto] {
        let args = claude::print_args(&PrintOptions { mode, guard_settings: Some("/data/command-guard.json"), ..Default::default() }).unwrap();
        let at = args.iter().position(|a| a == "--settings").expect("has --settings");
        assert_eq!(&args[at..at + 2], &["--settings", "/data/command-guard.json"], "{mode:?}");
    }
    let without = claude::print_args(&PrintOptions { mode: Mode::Auto, ..Default::default() }).unwrap();
    assert!(!without.iter().any(|a| a == "--settings"));
}

// case: names the guard as a hook its shell can start, on Locust’s own binary run as node, with Locust’s own id
#[test]
fn claude_settings_names_guard_with_locust_binary_and_parent_id() {
    let settings: serde_json::Value = serde_json::from_str(&claude_settings_with_parent(
        std::path::Path::new(r"C:\Users\Jane Doe\AppData\Local\Programs\Locust\Locust.exe"),
        4242,
    ))
    .unwrap();
    assert_eq!(
        settings,
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash|PowerShell",
                    "hooks": [{
                        "type": "command",
                        "command": r#"LOCUST_GUARD_PARENT=4242 "C:/Users/Jane Doe/AppData/Local/Programs/Locust/Locust.exe" --command-guard"#,
                        "timeout": 15
                    }]
                }]
            }
        })
    );
    let root = std::env::temp_dir().join(format!("lcg-parent-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let written = write_claude_settings(&root, std::path::Path::new("/Apps/locust-native")).unwrap();
    let on_disk = std::fs::read_to_string(&written).unwrap();
    assert!(on_disk.contains(&format!("LOCUST_GUARD_PARENT={}", std::process::id())), "{on_disk}");
    let _ = std::fs::remove_dir_all(&root);
}

// case: is read from the guard’s own words, so the two cannot drift apart
#[test]
fn guarded_sentence_reads_from_the_guards_own_words() {
    let by_name = stopped_because_with("taskkill //F //IM chrome.exe", &empty_ctx(), Some(&no_table)).unwrap();
    assert!(by_name.starts_with(LOCUST_GUARD_SAID));
    assert_eq!(
        guarded_sentence(&[("Bash `taskkill //F //IM chrome.exe`".into(), Some(by_name))]),
        "Locust stopped Bash `taskkill //F //IM chrome.exe` before it ran: it would have ended every Chrome on this computer, your own windows too. A teammate may end only what it started itself."
    );
    let by_id = stopped_because_with("taskkill //F //PID 10544", &empty_ctx(), Some(&look)).unwrap();
    assert!(by_id.starts_with(LOCUST_GUARD_SAID));
    assert_eq!(
        guarded_sentence(&[("Bash `taskkill //F //PID 10544`".into(), Some(by_id))]),
        "Locust stopped Bash `taskkill //F //PID 10544` before it ran: process 10544 is Chrome, which no teammate started, so it is yours. A teammate may end only what it started itself."
    );
}
