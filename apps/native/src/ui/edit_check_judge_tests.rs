//! Electron's `main/edit-check.test.ts`, the three cases about a comparison's judge (JUDGES SEE THE CHECK, 0.810).
//! Electron's judge awaits `editCheck.afterTurn(missionId)`. Rust's window holds the judge while a column's check runs
//! (`judge_waiting`, asked again once every check has said) and reads each column's check with `snapshot_check`: the
//! check record after the column's newest turn, or the check still running for it. These tests hold that reading.
use super::*;
use locust_core::edit_check::EditCheckOutcome;
use std::collections::HashMap;

fn check_item(mission: &str, outcome: EditCheckOutcome, new_lines: &[&str]) -> ledger::Item {
    ledger::Item::EditCheck {
        mission_id: mission.into(),
        command: "npm test".into(),
        outcome,
        new_lines: new_lines.iter().map(|l| (*l).to_owned()).collect(),
        unchanged: false,
        first: true,
        fix: None,
    }
}

fn column(tip: &str, items: Vec<ledger::Item>) -> ledger::Snapshot {
    ledger::Snapshot { tip: tip.into(), items, ..Default::default() }
}

// case: lets a judge wait for the check after one turn, and says when none ran after it
#[test]
fn lets_a_judge_wait_for_the_check_after_one_turn_and_says_when_none_ran_after_it() {
    let mut running = HashMap::new();
    running.insert("mission-a".to_owned(), "npm test".to_owned());
    // While it runs, the judge finds it at once and waits (the window holds the judge on a running check).
    let waiting = column("mission-a", Vec::new());
    assert_eq!(snapshot_check(&waiting, &running).map(|c| c.outcome), Some(JudgedCheckOutcome::Running));
    // Once it has said, the judge reads what it said.
    let said = column("mission-a", vec![check_item("mission-a", EditCheckOutcome::Failed, &["FAIL totals.test.mjs", "summarize is not a function"])]);
    let read = snapshot_check(&said, &HashMap::new()).unwrap();
    assert_eq!(read.command, "npm test");
    assert_eq!(read.outcome, JudgedCheckOutcome::Failed);
    assert_eq!(read.new_lines, vec!["FAIL totals.test.mjs", "summarize is not a function"]);
    // A turn after which no check ran has none, and never an older turn's.
    let older = column("mission-b", vec![check_item("mission-a", EditCheckOutcome::Passed, &[])]);
    assert_eq!(snapshot_check(&older, &HashMap::new()), None);
}

// case: says a check still running at the end of the wait is running, not passed
#[test]
fn says_a_check_still_running_at_the_end_of_the_wait_is_running_not_passed() {
    let mut running = HashMap::new();
    running.insert("mission-b".to_owned(), "npm test".to_owned());
    // Even beside an earlier passed check of the same turn, a running one is running.
    let snap = column("mission-b", vec![check_item("mission-b", EditCheckOutcome::Passed, &[])]);
    let read = snapshot_check(&snap, &running).unwrap();
    assert_eq!(read.outcome, JudgedCheckOutcome::Running);
    assert_ne!(read.judged().outcome, JudgedCheckOutcome::Passed);
}

// case: is found by a judge even when no command is set, as no check at all
#[test]
fn is_found_by_a_judge_even_when_no_command_is_set_as_no_check_at_all() {
    // No command: EditCheck::after starts nothing and writes no record, so the column has no check and nothing runs.
    let ec = locust_core::edit_check::EditCheck::new();
    let ran = ec.after(std::path::Path::new("/tmp/copies/cmp_x-c"), Some("mission-c"), None, |_| None, |_| panic!("must not start"), |_, _| panic!("must not run"));
    assert!(ran.is_none());
    assert!(ec.running_checks().is_empty());
    assert_eq!(snapshot_check(&column("mission-c", Vec::new()), &HashMap::new()), None);
}
