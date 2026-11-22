use std::path::Path;

use super::{
    CloudEnvironment, CloudStartProblem, CloudState, CloudStatus, FolderOnGitHub, Ran, StartAnswer,
    environment_answer, folder_on_github, github_repo_of, refusal_for, start_answer, status_answer,
};

fn ran(stdout: &str, code: i32, stderr: &str) -> Ran {
    Ran::new(code, stdout, stderr)
}

#[test]
// case: is named owner/repo from its remote, https or ssh
fn is_named_owner_repo_from_its_remote_https_or_ssh() {
    assert_eq!(
        github_repo_of("https://github.com/locust-lol/locust-cloud-test.git").as_deref(),
        Some("locust-lol/locust-cloud-test")
    );
    assert_eq!(
        github_repo_of("git@github.com:acme/widgets.git").as_deref(),
        Some("acme/widgets")
    );
    assert_eq!(
        github_repo_of("https://token@github.com/acme/widgets").as_deref(),
        Some("acme/widgets")
    );
    assert_eq!(
        github_repo_of("https://gitlab.com/acme/widgets.git"),
        None
    );
}

#[test]
// case: says its branch, what GitHub does not have yet, and uncommitted work
fn says_its_branch_what_github_does_not_have_yet_and_uncommitted_work() {
    let git = |args: &[&str], _cwd: &Path| -> Result<Ran, String> {
        Ok(match args.first().copied() {
            Some("remote") => ran("https://github.com/acme/widgets.git\n", 0, ""),
            Some("rev-parse") => ran("main\n", 0, ""),
            Some("rev-list") => ran("2\n", 0, ""),
            _ => ran(" M greet.js\n", 0, ""),
        })
    };
    assert_eq!(
        folder_on_github(Path::new("/w"), &git),
        FolderOnGitHub {
            repo: Some("acme/widgets".into()),
            branch: Some("main".into()),
            unpushed: Some(2),
            dirty: true,
        }
    );
}

#[test]
// case: has no upstream, or no remote, said as unknown rather than guessed
fn has_no_upstream_or_no_remote_said_as_unknown_rather_than_guessed() {
    let git = |args: &[&str], _cwd: &Path| -> Result<Ran, String> {
        Ok(if args.first().copied() == Some("rev-parse") {
            ran("main\n", 0, "")
        } else {
            ran("", 128, "fatal")
        })
    };
    assert_eq!(
        folder_on_github(Path::new("/w"), &git),
        FolderOnGitHub {
            repo: None,
            branch: Some("main".into()),
            unpushed: None,
            dirty: false,
        }
    );
}

#[test]
// case: finds the task in what Codex printed
fn finds_the_task_in_what_codex_printed() {
    assert_eq!(
        start_answer(
            &ran(
                "https://chatgpt.com/codex/tasks/task_e_6abd55697ae88328aeb7ba44897c5cf2\n",
                0,
                ""
            ),
            "acme/widgets"
        ),
        StartAnswer::Ok {
            task_id: "task_e_6abd55697ae88328aeb7ba44897c5cf2".into(),
            url: "https://chatgpt.com/codex/tasks/task_e_6abd55697ae88328aeb7ba44897c5cf2".into(),
        }
    );
}

#[test]
// case: says a missing environment in terms of what to do about it
fn says_a_missing_environment_in_terms_of_what_to_do_about_it() {
    let missing = start_answer(
        &ran(
            "",
            1,
            "Error: environment 'acme/widgets' not found; run `codex cloud` to list available environments",
        ),
        "acme/widgets",
    );
    assert_eq!(
        missing,
        StartAnswer::Err(CloudStartProblem::NoEnvironment {
            repo: "acme/widgets".into()
        })
    );
    if let StartAnswer::Err(problem) = missing {
        assert!(
            refusal_for(&problem)
                .contains("Legacy Codex Cloud and create one for that repository")
        );
    }
    assert_eq!(
        start_answer(
            &ran(
                "",
                1,
                "Error: no cloud environments are available for this workspace"
            ),
            "acme/widgets"
        ),
        StartAnswer::Err(CloudStartProblem::NoEnvironment {
            repo: "acme/widgets".into()
        })
    );
}

#[test]
// case: passes on anything else in Codex's own last words, never its config warnings
fn passes_on_anything_else_in_codexs_own_last_words_never_its_config_warnings() {
    let other = start_answer(
        &ran(
            "",
            1,
            "Error: rate limited, try later\nwarning: `computer_use.windows.always_allowed_app_ids` is ignored.",
        ),
        "acme/widgets",
    );
    assert_eq!(
        other,
        StartAnswer::Err(CloudStartProblem::Other {
            said: "Error: rate limited, try later".into()
        })
    );
}

#[test]
// case: reads what `codex cloud list --env` answered on 2026-09-30
fn reads_what_codex_cloud_list_env_answered_on_2026_09_30() {
    assert_eq!(
        environment_answer(&ran(
            "https://chatgpt.com/codex/tasks/task_e_1\n  [READY] Add farewell function to greet.js\n",
            0,
            ""
        )),
        CloudEnvironment::Ready
    );
    assert_eq!(
        environment_answer(&ran(
            "",
            1,
            "Error: environment 'locust-lol/ai-teammate-platform' not found; run `codex cloud` to list available environments"
        )),
        CloudEnvironment::Missing
    );
    assert_eq!(
        environment_answer(&ran(
            "",
            1,
            "Error: no cloud environments are available for this workspace"
        )),
        CloudEnvironment::Missing
    );
}

#[test]
// case: says unknown, not missing, for anything else -- a network failure is not a missing environment
fn says_unknown_not_missing_for_anything_else_a_network_failure_is_not_a_missing_environment() {
    assert_eq!(
        environment_answer(&ran("", 1, "Error: request timed out")),
        CloudEnvironment::Unknown
    );
}

#[test]
// case: reads a finished task as Codex prints it, on three lines, with its change counted
fn reads_a_finished_task_as_codex_prints_it_on_three_lines_with_its_change_counted() {
    assert_eq!(
        status_answer(
            "[READY] Add farewell function to greet.js\nlocust-lol/locust-cloud-test  •  4m ago\n+9/-1 • 2 files\n"
        ),
        CloudStatus {
            state: CloudState::Ready,
            title: Some("Add farewell function to greet.js".into()),
            added: Some(9),
            removed: Some(1),
            files: Some(2),
        }
    );
}

#[test]
// case: reads the same on one line
fn reads_the_same_on_one_line() {
    assert_eq!(
        status_answer(
            "[READY] Update greeting in greet.js locust-lol/locust-cloud-test  •  1m ago +2/-2 • 2 files"
        ),
        CloudStatus {
            state: CloudState::Ready,
            title: Some("Update greeting in greet.js".into()),
            added: Some(2),
            removed: Some(2),
            files: Some(2),
        }
    );
}

#[test]
// case: reads one still going, with nothing to count yet
fn reads_one_still_going_with_nothing_to_count_yet() {
    let status = status_answer(
        "[PENDING] Update greeting in greet.js locust-lol/locust-cloud-test  •  16s ago no diff",
    );
    assert_eq!(status.state, CloudState::Pending);
    assert_eq!(status.added, None);
    assert_eq!(status.files, None);
}

#[test]
// case: reads anything it does not know as unknown
fn reads_anything_it_does_not_know_as_unknown() {
    assert_eq!(
        status_answer("something else entirely").state,
        CloudState::Unknown
    );
}
