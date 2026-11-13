use super::*;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_SEQ: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "locust-{prefix}-{}-{}",
            std::process::id(),
            TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scripted(runs: Vec<CheckRun>) -> impl FnMut(&str, &Path) -> CheckRun {
    let mut at = 0usize;
    move |_cmd, _cwd| {
        let idx = at.min(runs.len() - 1);
        at += 1;
        runs[idx].clone()
    }
}

#[test]
fn edit_check_reads_command_of_conversations_own_folder() {
    let ec = EditCheck::new();
    let mut asked = Vec::new();
    let res = ec.after(
        Path::new("/tmp/work/a"),
        Some("mission-a"),
        Some("ws_folder_a"),
        |ws| {
            asked.push(ws.map(str::to_owned));
            Some("true".into())
        },
        |_| {},
        |_, _| CheckRun {
            exit_code: Some(0),
            output: "ok\n".into(),
            timed_out: false,
            error: None,
        },
    );
    assert!(res.is_some());
    assert_eq!(asked, vec![Some("ws_folder_a".to_owned())]);
}

#[test]
fn edit_check_says_nothing_when_no_command_is_set() {
    let ec = EditCheck::new();
    let res = ec.after(
        Path::new("/tmp/work"),
        None,
        None,
        |_| None,
        |_| panic!("must not start"),
        |_, _| panic!("must not run"),
    );
    assert!(res.is_none());
}

#[test]
fn edit_check_names_only_what_is_new_and_says_when_same_failure() {
    let ec = EditCheck::new();
    let mut run = scripted(vec![
        CheckRun {
            exit_code: Some(1),
            output: "FAIL a.test.ts\n  expected 1 to be 2\nDuration 3.2s\n".into(),
            timed_out: false,
            error: None,
        },
        CheckRun {
            exit_code: Some(1),
            output: "FAIL a.test.ts\n  expected 1 to be 2\nFAIL b.test.ts\n  cannot read x\nDuration 4.1s\n".into(),
            timed_out: false,
            error: None,
        },
        CheckRun {
            exit_code: Some(1),
            output: "FAIL a.test.ts\n  expected 1 to be 2\nFAIL b.test.ts\n  cannot read x\nDuration 2.0s\n".into(),
            timed_out: false,
            error: None,
        },
        CheckRun {
            exit_code: Some(0),
            output: "all passed\n".into(),
            timed_out: false,
            error: None,
        },
    ]);
    let r1 = ec.after(Path::new("/tmp/work"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert_eq!(r1.outcome, EditCheckOutcome::Failed);
    assert!(r1.first);
    assert_eq!(r1.new_lines, vec!["FAIL a.test.ts", "expected 1 to be 2"]);

    let r2 = ec.after(Path::new("/tmp/work"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert_eq!(r2.outcome, EditCheckOutcome::Failed);
    assert!(!r2.first);
    assert!(!r2.unchanged);
    assert_eq!(r2.new_lines, vec!["FAIL b.test.ts", "cannot read x"]);

    let r3 = ec.after(Path::new("/tmp/work"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert_eq!(r3.outcome, EditCheckOutcome::Failed);
    assert!(r3.unchanged);
    assert!(r3.new_lines.is_empty());

    let r4 = ec.after(Path::new("/tmp/work"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert_eq!(r4.outcome, EditCheckOutcome::Passed);
}

#[test]
fn edit_check_keeps_whole_failure_and_before_for_fix() {
    let ec = EditCheck::new();
    let mut run = scripted(vec![
        CheckRun { exit_code: Some(0), output: "ok 4 tests\n".into(), timed_out: false, error: None },
        CheckRun { exit_code: Some(1), output: "FAIL a.test.ts\nFAIL b.test.ts\n".into(), timed_out: false, error: None },
        CheckRun { exit_code: Some(1), output: "FAIL b.test.ts\n".into(), timed_out: false, error: None },
    ]);
    let passed = ec.after(Path::new("/tmp/work"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert!(passed.lines.is_none());

    let f1 = ec.after(Path::new("/tmp/work"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert_eq!(f1.lines.as_deref(), Some(&["FAIL a.test.ts".to_string(), "FAIL b.test.ts".to_string()][..]));
    assert_eq!(f1.before.as_deref(), Some(&["ok 4 tests".to_string()][..]));

    let f2 = ec.after(Path::new("/tmp/work"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert!(f2.unchanged);
    assert!(f2.new_lines.is_empty());
    assert_eq!(f2.lines.as_deref(), Some(&["FAIL b.test.ts".to_string()][..]));
    let mut before = f2.before.unwrap();
    before.sort();
    assert_eq!(before, vec!["FAIL a.test.ts", "FAIL b.test.ts"]);
}

#[test]
fn edit_check_keeps_each_folder_to_itself() {
    let ec = EditCheck::new();
    let mut run = scripted(vec![CheckRun { exit_code: Some(1), output: "broken\n".into(), timed_out: false, error: None }]);
    ec.after(Path::new("/tmp/work/a"), None, None, |_| Some("true".into()), |_| {}, &mut run);
    let b = ec.after(Path::new("/tmp/work/b"), None, None, |_| Some("true".into()), |_| {}, &mut run).unwrap();
    assert!(b.first);
    assert_eq!(b.new_lines, vec!["broken"]);
}

#[test]
fn edit_check_says_command_as_it_starts_and_skips_if_busy_in_same_folder() {
    let ec = Arc::new(EditCheck::new());
    let (tx, rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let ec1 = ec.clone();
    let handle = std::thread::spawn(move || {
        let mut started = Vec::new();
        let res = ec1.after(
            Path::new("/tmp/copies/cmp_y-a"),
            Some("mission-a"),
            None,
            |_| Some("true".into()),
            |cmd| {
                started.push(cmd.to_owned());
            },
            |_, _| {
                tx.send(()).unwrap();
                release_rx.recv().unwrap();
                CheckRun { exit_code: Some(0), output: String::new(), timed_out: false, error: None }
            },
        );
        (started, res)
    });

    rx.recv().unwrap();
    assert_eq!(ec.running_checks(), vec![("mission-a".to_owned(), "true".to_owned())]);
    // Second turn in the same folder while the first is checked runs no check.
    let second = ec.after(
        Path::new("/tmp/copies/cmp_y-a"),
        Some("mission-b"),
        None,
        |_| Some("true".into()),
        |_| panic!("must not start while busy"),
        |_, _| panic!("must not run while busy"),
    );
    assert!(second.is_none());

    release_tx.send(()).unwrap();
    let (started, first) = handle.join().unwrap();
    assert_eq!(started, vec!["true"]);
    assert_eq!(first.unwrap().outcome, EditCheckOutcome::Passed);
    assert!(ec.running_checks().is_empty());

    // Panic in on_start does not stop the check.
    let loud = ec.after(
        Path::new("/tmp/work"),
        Some("mission-d"),
        None,
        |_| Some("true".into()),
        |_| panic!("window gone"),
        |_, _| CheckRun { exit_code: Some(0), output: String::new(), timed_out: false, error: None },
    );
    assert_eq!(loud.unwrap().outcome, EditCheckOutcome::Passed);
}

#[test]
fn without_folder_normalizes_all_path_spellings() {
    let copy = r"C:\Users\someone\.locust\compare\cmp_1a2b\a";
    let printed = [
        "not ok 1 - adds two numbers",
        "  location: 'file:///C:/Users/someone/.locust/compare/cmp_1a2b/a/sum.test.mjs:3:1'",
        r"Error: Cannot find module 'c:\users\someone\.locust\compare\cmp_1a2b\a\node_modules\vitest'",
        "sh: /c/Users/someone/.locust/compare/cmp_1a2b/a/node_modules/.bin/vitest: No such file or directory",
        r"beside it, C:\Users\someone\.locust\compare\cmp_1a2b\ab\x.mjs is another folder",
    ]
    .join("\n");
    let got: Vec<&str> = without_folder(&printed, copy, true).leak().split('\n').collect();
    assert_eq!(
        got,
        vec![
            "not ok 1 - adds two numbers",
            "  location: './sum.test.mjs:3:1'",
            r"Error: Cannot find module '.\node_modules\vitest'",
            "sh: ./node_modules/.bin/vitest: No such file or directory",
            r"beside it, C:\Users\someone\.locust\compare\cmp_1a2b\ab\x.mjs is another folder",
        ]
    );
    assert_eq!(
        without_folder(
            r#"{"file":"C:\\Users\\someone\\.locust\\compare\\cmp_1a2b\\a\\sum.test.mjs"}"#,
            copy,
            true
        ),
        r#"{"file":".\\sum.test.mjs"}"#
    );
    assert_eq!(
        without_folder("at file:///home/me/My%20Work/t.mjs and /home/me/My Work/t.mjs", "/home/me/My Work/", false),
        "at ./t.mjs and ./t.mjs"
    );
    assert_eq!(without_folder("/Home/me/t.mjs", "/home/me", false), "/Home/me/t.mjs");
    assert_eq!(without_folder("/t.mjs", "/", false), "/t.mjs");
}

#[test]
fn comparable_lines_drops_ansi_timings_and_keeps_test_names() {
    assert_eq!(
        comparable_lines("\x1b[31mFAIL\x1b[39m x\n  Duration 3.21s\nTime: 12ms\n12 ms\nok\nok\n"),
        vec!["FAIL x", "ok"]
    );
    assert_eq!(
        comparable_lines("✖ adds two numbers (1.6709ms)\nℹ tests 1\nℹ duration_ms 76.857\n# duration_ms 12.5\n✓ src/a.test.ts (3 tests) 12ms\n"),
        vec!["✖ adds two numbers", "ℹ tests 1", "✓ src/a.test.ts (3 tests)"]
    );
}

#[test]
fn run_check_command_handles_nul_and_runs_in_temp_folder() {
    let dir = TempDir::new("edit-check");
    let nul = run_check_command("true\0", &dir.0, 5_000);
    assert!(nul.exit_code.is_none());
    assert!(nul.error.is_some());

    let passed = run_check_command("pwd", &dir.0, 10_000);
    assert_eq!(passed.exit_code, Some(0));
    let folder_name = dir.0.file_name().unwrap().to_string_lossy().to_lowercase();
    assert!(passed.output.to_lowercase().contains(&folder_name));

    let failed = run_check_command("echo BAD >&2; exit 3", &dir.0, 10_000);
    assert_eq!(failed.exit_code, Some(3));
    assert!(failed.output.contains("BAD"));

    let timed = run_check_command("sleep 10", &dir.0, 300);
    assert!(timed.timed_out);
}

#[test]
fn a_check_command_is_one_line() {
    assert_eq!(parsed_check_command("  npm test -- --run  "), Some("npm test -- --run".into()));
    assert_eq!(parsed_check_command(""), None);
    assert_eq!(parsed_check_command("   "), None);
    assert_eq!(parsed_check_command(&"x".repeat(501)), None);
    assert_eq!(parsed_check_command("npm\ntest"), None);
    assert_eq!(parsed_check_command("npm\rtest"), None);
    assert_eq!(parsed_check_command("npm\0test"), None);
}

fn person_turn() -> CheckedTurn {
    CheckedTurn {
        mission_id: "turn_1".into(),
        workspace_id: None,
        teammate_id: Some("tm_wren".into()),
        completed: true,
        apart: false,
        started_by_kind: None,
        started_by_fix: None,
        sent_by_person: true,
    }
}

fn fix_turn(mission_id: &str, fix: u32) -> CheckedTurn {
    CheckedTurn {
        mission_id: mission_id.into(),
        workspace_id: None,
        teammate_id: Some("tm_wren".into()),
        completed: true,
        apart: false,
        started_by_kind: Some("check-fix".into()),
        started_by_fix: Some(fix),
        sent_by_person: false,
    }
}

fn failed_check(lines: &[&str], before: Option<&[&str]>) -> CheckFixResultOrNoEdits {
    let lines_vec: Vec<String> = lines.iter().map(|s| (*s).to_owned()).collect();
    let before_vec = before.map(|b| b.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
    let fresh: Vec<String> = match &before_vec {
        None => lines_vec.clone(),
        Some(b) => lines_vec.iter().filter(|l| !b.contains(*l)).cloned().collect(),
    };
    let unchanged = before_vec.is_some() && fresh.is_empty();
    CheckFixResultOrNoEdits::Result(EditCheckResult {
        command: "npm test".into(),
        outcome: EditCheckOutcome::Failed,
        new_lines: fresh,
        unchanged,
        first: before_vec.is_none(),
        lines: Some(lines_vec),
        before: before_vec,
    })
}

fn passed_check() -> CheckFixResultOrNoEdits {
    CheckFixResultOrNoEdits::Result(EditCheckResult {
        command: "npm test".into(),
        outcome: EditCheckOutcome::Passed,
        new_lines: Vec::new(),
        unchanged: false,
        first: false,
        lines: None,
        before: None,
    })
}

#[test]
fn check_fix_sends_what_turn_broke_and_loops_until_pass_or_stop() {
    let cf = CheckFix::new();
    let mut started = Vec::new();
    let mut notes = Vec::new();
    let mut fixes = 0u32;

    cf.on_checked(
        &person_turn(),
        failed_check(&["FAIL adds two numbers", "Tests 1 failed"], Some(&["Tests 4 passed"])),
        |_| Some(3),
        |_| false,
        |_| "Wren".into(),
        |req| {
            started.push(req);
            fixes += 1;
            FixStartOutcome::Ok { mission_id: format!("fix_{fixes}") }
        },
        |id, msg, lvl| notes.push((id.to_owned(), msg.to_owned(), lvl)),
    );
    assert_eq!(started.len(), 1);
    let ask = &started[0];
    assert_eq!(ask.follow_up_of, "turn_1");
    assert_eq!(ask.teammate_id.as_deref(), Some("tm_wren"));
    assert_eq!(ask.started_by, CheckFixStarter { fix: 1, of: 3, command: "npm test".into() });
    assert!(ask.prompt.contains("The check `npm test` failed after your last turn, with problems it did not have before (fix 1 of 3)."));
    assert!(ask.prompt.contains("    FAIL adds two numbers"));
    assert!(!ask.prompt.contains("Tests 4 passed"));
    assert!(ask.prompt.contains("Do not weaken, skip or delete tests"));
    assert!(notes.is_empty());

    // Pass ends it with nothing more said.
    cf.on_checked(
        &fix_turn("fix_1", 1),
        passed_check(),
        |_| Some(3),
        |_| false,
        |_| "Wren".into(),
        |_| panic!("must not start"),
        |id, msg, lvl| notes.push((id.to_owned(), msg.to_owned(), lvl)),
    );
    assert_eq!(started.len(), 1);
    assert!(notes.is_empty());
}

#[test]
fn check_fix_ignores_off_non_failed_apart_incomplete_and_unchanged() {
    let cf = CheckFix::new();
    let mut started = 0;
    let mut notes = 0;

    // Off (most = None)
    cf.on_checked(&person_turn(), failed_check(&["FAIL x"], None), |_| None, |_| false, |_| "Wren".into(), |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } }, |_, _, _| notes += 1);
    // Non-failed outcomes
    for outcome in [EditCheckOutcome::Passed, EditCheckOutcome::TimedOut, EditCheckOutcome::CouldNotRun] {
        cf.on_checked(
            &person_turn(),
            CheckFixResultOrNoEdits::Result(EditCheckResult {
                command: "npm test".into(),
                outcome,
                new_lines: Vec::new(),
                unchanged: false,
                first: false,
                lines: None,
                before: None,
            }),
            |_| Some(2),
            |_| false,
            |_| "Wren".into(),
            |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } },
            |_, _, _| notes += 1,
        );
    }
    // Not sent by person / apart / incomplete / routine / carry-on
    cf.on_checked(&CheckedTurn { sent_by_person: false, ..person_turn() }, failed_check(&["FAIL x"], None), |_| Some(2), |_| false, |_| "Wren".into(), |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } }, |_, _, _| notes += 1);
    cf.on_checked(&CheckedTurn { apart: true, ..person_turn() }, failed_check(&["FAIL x"], None), |_| Some(2), |_| false, |_| "Wren".into(), |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } }, |_, _, _| notes += 1);
    cf.on_checked(&CheckedTurn { completed: false, ..person_turn() }, failed_check(&["FAIL x"], None), |_| Some(2), |_| false, |_| "Wren".into(), |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } }, |_, _, _| notes += 1);
    cf.on_checked(&CheckedTurn { started_by_kind: Some("routine".into()), ..person_turn() }, failed_check(&["FAIL x"], None), |_| Some(2), |_| false, |_| "Wren".into(), |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } }, |_, _, _| notes += 1);
    cf.on_checked(&CheckedTurn { started_by_kind: Some("carry-on".into()), ..person_turn() }, failed_check(&["FAIL x"], None), |_| Some(2), |_| false, |_| "Wren".into(), |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } }, |_, _, _| notes += 1);
    // Unchanged failure on person's turn
    cf.on_checked(&person_turn(), failed_check(&["FAIL old"], Some(&["FAIL old"])), |_| Some(2), |_| false, |_| "Wren".into(), |_| { started += 1; FixStartOutcome::Ok { mission_id: "f".into() } }, |_, _, _| notes += 1);

    assert_eq!(started, 0);
    assert_eq!(notes, 0);
}

#[test]
fn check_fix_partial_progress_and_stop_conditions() {
    // Partial progress sends only what is left beyond baseline.
    let cf = CheckFix::new();
    let mut started = Vec::new();
    let mut notes = Vec::new();
    let mut fixes = 0u32;
    cf.on_checked(
        &person_turn(),
        failed_check(&["FAIL old", "FAIL a", "FAIL b"], Some(&["FAIL old"])),
        |_| Some(3),
        |_| false,
        |_| "Wren".into(),
        |req| { started.push(req); fixes += 1; FixStartOutcome::Ok { mission_id: format!("fix_{fixes}") } },
        |id, msg, lvl| notes.push((id.to_owned(), msg.to_owned(), lvl)),
    );
    cf.on_checked(
        &fix_turn("fix_1", 1),
        failed_check(&["FAIL old", "FAIL b"], Some(&["FAIL old", "FAIL a", "FAIL b"])),
        |_| Some(3),
        |_| false,
        |_| "Wren".into(),
        |req| { started.push(req); fixes += 1; FixStartOutcome::Ok { mission_id: format!("fix_{fixes}") } },
        |id, msg, lvl| notes.push((id.to_owned(), msg.to_owned(), lvl)),
    );
    assert_eq!(started.len(), 2);
    assert_eq!(started[1].follow_up_of, "fix_1");
    assert_eq!(started[1].started_by, CheckFixStarter { fix: 2, of: 3, command: "npm test".into() });
    assert!(started[1].prompt.contains("    FAIL b"));
    assert!(!started[1].prompt.contains("FAIL a"));
    assert!(!started[1].prompt.contains("FAIL old"));
    assert!(notes.is_empty());

    // Fixed what it broke -> info note
    let cf2 = CheckFix::new();
    let mut notes2 = Vec::new();
    cf2.on_checked(&person_turn(), failed_check(&["FAIL old", "FAIL a"], Some(&["FAIL old"])), |_| Some(3), |_| false, |_| "Wren".into(), |_| FixStartOutcome::Ok { mission_id: "fix_1".into() }, |id, m, l| notes2.push((id.to_owned(), m.to_owned(), l)));
    cf2.on_checked(&fix_turn("fix_1", 1), failed_check(&["FAIL old"], Some(&["FAIL old", "FAIL a"])), |_| Some(3), |_| false, |_| "Wren".into(), |_| panic!("must not start"), |id, m, l| notes2.push((id.to_owned(), m.to_owned(), l)));
    assert_eq!(notes2, vec![("fix_1".into(), stopped_reason::fixed_what_it_broke(1), "info")]);

    // No progress -> warning note
    let cf3 = CheckFix::new();
    let mut notes3 = Vec::new();
    cf3.on_checked(&person_turn(), failed_check(&["FAIL a"], Some(&[])), |_| Some(3), |_| false, |_| "Wren".into(), |_| FixStartOutcome::Ok { mission_id: "fix_1".into() }, |id, m, l| notes3.push((id.to_owned(), m.to_owned(), l)));
    cf3.on_checked(&fix_turn("fix_1", 1), failed_check(&["FAIL a"], Some(&["FAIL a"])), |_| Some(3), |_| false, |_| "Wren".into(), |_| panic!("must not start"), |id, m, l| notes3.push((id.to_owned(), m.to_owned(), l)));
    assert_eq!(notes3, vec![("fix_1".into(), stopped_reason::no_progress(1), "warning")]);

    // Out of fixes -> warning note
    let cf4 = CheckFix::new();
    let mut notes4 = Vec::new();
    cf4.on_checked(&person_turn(), failed_check(&["FAIL a"], Some(&[])), |_| Some(1), |_| false, |_| "Wren".into(), |_| FixStartOutcome::Ok { mission_id: "fix_1".into() }, |id, m, l| notes4.push((id.to_owned(), m.to_owned(), l)));
    cf4.on_checked(&fix_turn("fix_1", 1), failed_check(&["FAIL c"], Some(&["FAIL a"])), |_| Some(1), |_| false, |_| "Wren".into(), |_| panic!("must not start"), |id, m, l| notes4.push((id.to_owned(), m.to_owned(), l)));
    assert_eq!(notes4, vec![("fix_1".into(), stopped_reason::out_of_fixes(1), "warning")]);
    assert!(stopped_reason::out_of_fixes(1).contains("after 1 fix,"));
    assert!(stopped_reason::out_of_fixes(2).contains("after 2 fixes,"));

    // No edits -> warning note
    let cf5 = CheckFix::new();
    let mut notes5 = Vec::new();
    cf5.on_checked(&person_turn(), failed_check(&["FAIL a"], Some(&[])), |_| Some(2), |_| false, |_| "Wren".into(), |_| FixStartOutcome::Ok { mission_id: "fix_1".into() }, |id, m, l| notes5.push((id.to_owned(), m.to_owned(), l)));
    cf5.on_checked(&fix_turn("fix_1", 1), CheckFixResultOrNoEdits::NoEdits, |_| Some(2), |_| false, |_| "Wren".into(), |_| panic!("must not start"), |id, m, l| notes5.push((id.to_owned(), m.to_owned(), l)));
    assert_eq!(notes5, vec![("fix_1".into(), stopped_reason::changed_nothing(1), "warning")]);

    // Busy -> info note; error -> warning note and retryable
    let cf6 = CheckFix::new();
    let mut notes6 = Vec::new();
    cf6.on_checked(&person_turn(), failed_check(&["FAIL a"], Some(&[])), |_| Some(2), |_| true, |_| "Wren".into(), |_| panic!("must not start"), |id, m, l| notes6.push((id.to_owned(), m.to_owned(), l)));
    assert_eq!(notes6, vec![("turn_1".into(), stopped_reason::busy("Wren"), "info")]);

    let cf7 = CheckFix::new();
    let mut notes7 = Vec::new();
    let mut started7 = 0;
    cf7.on_checked(&person_turn(), failed_check(&["FAIL a"], Some(&[])), |_| Some(2), |_| false, |_| "Wren".into(), |_| FixStartOutcome::Error { message: "Wren is signed out.".into() }, |id, m, l| notes7.push((id.to_owned(), m.to_owned(), l)));
    assert_eq!(notes7, vec![("turn_1".into(), stopped_reason::could_not_start("Wren is signed out."), "warning")]);
    cf7.on_checked(&person_turn(), failed_check(&["FAIL a"], Some(&[])), |_| Some(2), |_| false, |_| "Wren".into(), |_| { started7 += 1; FixStartOutcome::Ok { mission_id: "fix_1".into() } }, |_, _, _| {});
    assert_eq!(started7, 1);
}

#[test]
fn check_fix_bounds_prompt_and_parses_settings() {
    let cf = CheckFix::new();
    let lines: Vec<String> = (0..45)
        .map(|i| {
            if i == 44 {
                format!("FAIL test {i} {}", "x".repeat(400))
            } else {
                format!("FAIL test {i}")
            }
        })
        .collect();
    let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut started = Vec::new();
    cf.on_checked(&person_turn(), failed_check(&line_refs, Some(&[])), |_| Some(2), |_| false, |_| "Wren".into(), |req| { started.push(req); FixStartOutcome::Ok { mission_id: "f1".into() } }, |_, _, _| {});
    let told: Vec<&str> = started[0].prompt.lines().filter(|l| l.starts_with("    FAIL")).collect();
    assert_eq!(told.len(), 30);
    assert_eq!(told[0], "    FAIL test 15");
    assert!(told.last().unwrap().chars().count() <= 4 + 300);
    assert!(told.last().unwrap().ends_with('…'));

    let first = check_fix_prompt("npm test", &[], true, 1, 2);
    assert!(first.contains("The check `npm test` failed after your last turn (fix 1 of 2). What it printed:"));
    assert!(first.contains("    (it printed nothing)"));
    assert!(check_fix_prompt("npm test", &["FAIL a".into()], true, 2, 2).contains("still fails after your fix (fix 2 of 2). What it printed:"));

    assert_eq!(parsed_check_fix_tries(&json!(0)), None);
    assert_eq!(parsed_check_fix_tries(&json!(1)), Some(1));
    assert_eq!(parsed_check_fix_tries(&json!(5)), Some(5));
    assert_eq!(parsed_check_fix_tries(&json!(6)), None);
    assert_eq!(parsed_check_fix_tries(&json!(2.5)), None);
    assert_eq!(parsed_check_fix_tries(&json!("2")), None);
    assert_eq!(parsed_check_fixes(&json!({"ws_a": 2, "ws_b": 9, "ws_c": "x", "": 3})), BTreeMap::from([("ws_a".into(), 2)]));
}

#[test]
fn a_reopened_conversation_keeps_its_check() {
    let dir = TempDir::new("reopen-check");
    let store = Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    let mut writer = crate::ledger_write::LedgerWriter::create_runtime_with(
        &store,
        "mission_1",
        "Replace notes.txt with BROKEN.",
        "haiku",
        "claude",
        "claude",
        &[],
        None,
        &json!({"workspaceId": "ws_test"}),
    )
    .unwrap();
    writer
        .append(
            "run.started",
            json!({"runtimeThreadId": "t1", "evidence": {"redacted": true, "runtimeEventType": "test"}}),
            None,
        )
        .unwrap();
    writer
        .append(
            "run.completed",
            json!({
                "runtimeThreadId": "t1",
                "resolvedModel": "haiku",
                "process": {
                    "exitCode": 0,
                    "signal": null,
                    "stderr": "",
                    "stderrTruncated": false,
                    "recordCount": 1,
                    "inputDeliveryFailed": false,
                    "outputLimitExceeded": false,
                    "oversizedRecordsDropped": 0,
                    "forcedTerminationAttempted": false,
                    "terminationUnconfirmed": false,
                    "startedAt": "2026-09-25T17:00:00.000Z",
                    "finishedAt": "2026-09-25T17:00:01.000Z"
                }
            }),
            None,
        )
        .unwrap();
    drop(writer);

    let path = store.write_path("mission_1").unwrap();
    let r1 = EditCheckResult {
        command: "node check.js".into(),
        outcome: EditCheckOutcome::Failed,
        new_lines: vec!["FAIL: notes.txt says BROKEN".into()],
        unchanged: false,
        first: true,
        lines: None,
        before: None,
    };
    let r2 = EditCheckResult {
        command: "node check.js".into(),
        outcome: EditCheckOutcome::Passed,
        new_lines: vec![],
        unchanged: false,
        first: false,
        lines: None,
        before: None,
    };
    let r3 = EditCheckResult {
        command: "node check.js".into(),
        outcome: EditCheckOutcome::Failed,
        new_lines: vec!["FAIL: again".into()],
        unchanged: false,
        first: false,
        lines: None,
        before: None,
    };
    crate::ledger_write::append_edit_check(&path, &edit_check_record_json(&r1, "2026-09-25T17:00:00.000Z")).unwrap();
    crate::ledger_write::append_edit_check(&path, &edit_check_record_json(&r2, "2026-09-25T17:01:00.000Z")).unwrap();
    crate::ledger_write::append_edit_check(&path, &edit_check_record_json(&r3, "2026-09-25T17:02:00.000Z")).unwrap();

    // Fresh transcript read over the same file: what a reopened app reads.
    let transcript = crate::ledger::read_transcript(&path);
    assert!(matches!(
        transcript.items.last(),
        Some(crate::ledger::Item::EditCheck {
            command,
            outcome: EditCheckOutcome::Failed,
            new_lines,
            unchanged: false,
            first: false,
            ..
        }) if command == "node check.js" && new_lines == &vec!["FAIL: again".to_string()]
    ));

    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let status = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../scripts/check-m1-ledger.mjs"
            ))
            .arg(reader)
            .arg(store.ledger_dir().unwrap())
            .arg("mission_1")
            .status()
            .unwrap();
        assert!(status.success(), "TypeScript reader rejected mission_1 with edit_check");
    }
}
