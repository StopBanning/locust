use super::*;
use crate::ledger::ToolDetail;
fn detail(exit: Option<i64>, settled: bool, failed: bool) -> Item {
    Item::Tool {
        name: "shell".into(),
        summary: "pnpm test".into(),
        status: if failed {
            Status::Failed
        } else if settled {
            Status::Done
        } else {
            Status::Running
        },
        detail: Box::new(ToolDetail {
            exit_code: exit,
            ..Default::default()
        }),
    }
}
fn good() -> Item {
    detail(None, true, false)
}
fn words(items: &[Item], names: &[&str]) -> Words {
    commands_run_text(
        commands_run(items, true),
        &names.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
    .unwrap()
}
#[test]
fn says_nothing_at_all_when_nothing_was_run() {
    assert_eq!(commands_run_text(commands_run(&[], true), &[]), None);
    assert_eq!(
        commands_run(&[], true),
        CommandsRun {
            ran: 0,
            non_zero: 0,
            unsettled: 0
        }
    );
}
#[test]
fn names_the_commands_rather_than_grading_them() {
    assert_eq!(
        words(&[good(), good()], &["pnpm check", "tsc --noEmit"]),
        Words {
            text: "ran pnpm and tsc".into(),
            amber: false
        }
    );
}
#[test]
fn names_two_and_counts_the_rest_because_four_command_lines_do_not_fit() {
    assert_eq!(
        words(
            &[good(), good(), good(), good()],
            &["pnpm check", "tsc", "node x.mjs", "git status"]
        )
        .text,
        "ran 4 commands: pnpm, tsc and 2 more"
    );
}
#[test]
fn names_each_kind_once_so_twelve_commands_of_two_kinds_read_as_twelve() {
    let mut names = vec!["mkdir -p batch && printf 'a' > batch/a.txt".to_owned()];
    names.extend(
        "bcdefghijkl"
            .chars()
            .map(|c| format!("printf '{c}' > batch/{c}.txt")),
    );
    assert_eq!(
        commands_run_text(
            commands_run(&(0..12).map(|_| good()).collect::<Vec<_>>(), true),
            &names
        )
        .unwrap()
        .text,
        "ran 12 commands: mkdir and printf"
    );
}
#[test]
fn reads_past_claude_codes_cd_in_front_of_a_command() {
    assert_eq!(
        words(
            &[good(), good(), good()],
            &[
                "cd C:\\work\\app && npm test",
                "cd \"C:\\work\\my app\" && git status",
                "cd /work/app; npm run build"
            ]
        )
        .text,
        "ran 3 commands: npm and git"
    );
}
#[test]
fn falls_back_to_a_count_when_no_command_name_was_recorded() {
    assert_eq!(words(&[good(), good()], &[]).text, "ran 2 commands");
    assert_eq!(words(&[good()], &[]).text, "ran 1 command");
}
#[test]
fn says_nothing_about_how_they_came_out_when_they_all_came_out_fine() {
    let said = words(&[good(), good()], &["pnpm check", "tsc"])
        .text
        .to_lowercase();
    for verdict in ["exit 0", "pass", "ok", "success", "checked", "verified"] {
        assert!(!said.contains(verdict), "{verdict}");
    }
}
#[test]
fn counts_a_non_zero_exit_and_says_so_in_amber() {
    let run = commands_run(
        &[detail(Some(0), true, false), detail(Some(1), true, false)],
        true,
    );
    assert_eq!((run.ran, run.non_zero), (2, 1));
    assert_eq!(
        commands_run_text(run, &[]),
        Some(Words {
            text: "ran 2 commands · 1 exited non-zero".into(),
            amber: true
        })
    );
}
#[test]
fn counts_a_command_that_failed_outright_as_non_zero_however_it_failed() {
    assert_eq!(commands_run(&[detail(None, true, true)], true).non_zero, 1);
}
#[test]
fn does_not_call_a_command_that_never_reported_a_pass() {
    let run = commands_run(
        &[detail(Some(0), true, false), detail(None, false, false)],
        true,
    );
    assert_eq!(
        run,
        CommandsRun {
            ran: 2,
            non_zero: 0,
            unsettled: 1
        }
    );
    assert_eq!(
        commands_run_text(run, &[]),
        Some(Words {
            text: "ran 2 commands · 1 did not report".into(),
            amber: true
        })
    );
}
// case: does not count an unfinished turnu2019s open command as unreported
#[test]
fn does_not_count_an_unfinished_turns_open_command_as_unreported() {
    assert_eq!(
        commands_run(&[detail(None, false, false)], false).unsettled,
        0
    );
}
#[test]
fn a_non_zero_exit_outranks_an_unreported_one_the_failure_is_the_news() {
    assert_eq!(
        words(
            &[detail(Some(2), true, false), detail(None, false, false)],
            &[]
        )
        .text,
        "ran 2 commands · 1 exited non-zero"
    );
}
#[test]
fn a_refused_command_never_ran() {
    let mut i = good();
    if let Item::Tool { detail, .. } = &mut i {
        detail.runtime_status = "refused".into();
    }
    assert_eq!(commands_run(&[i], true), CommandsRun::default());
}
