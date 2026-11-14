use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset};
use serde_json::json;

use crate::pull_requests::{FolderPullRequest, PullRequestChecks, PullRequestReview, PullRequestState, pull_requests_of};
use crate::store::Store;

use super::folder::folder_mark_of;
use super::github::{AWAY_WORKFLOW_NAME, GhError, ScoutGithubUnread, github_unread_why, read_scout_github};
use super::log::*;
use super::store::*;
use super::types::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "locust-scout-test-{tag}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn dt(rfc3339: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(rfc3339).unwrap()
}

const PR_URL: &str = "https://github.com/acme/app/pull/212";

fn sample_pr() -> FolderPullRequest {
    FolderPullRequest {
        number: 212,
        title: "Talk starts from any app".to_owned(),
        url: PR_URL.to_owned(),
        state: PullRequestState::Open,
        checks: PullRequestChecks::Passing,
        check_count: 9,
        not_passed: 0,
        review: None,
        conflicts: false,
        updated_at: None,
    }
}

// ---- shared/scout-notices-what-is-new.test.ts (15 cases, 1:1) ----

#[test]
fn pull_requests_seen_for_the_first_time_are_news_only_for_what_needs_work_now_keyed_by_their_state() {
    let now = dt("2026-10-12T15:00:00.000Z");
    assert!(pull_request_facts(&HashMap::new(), &[sample_pr()], "look1", now).is_empty());

    let failing_pr = FolderPullRequest {
        checks: PullRequestChecks::Failing,
        not_passed: 2,
        ..sample_pr()
    };
    let failing = pull_request_facts(&HashMap::new(), &[failing_pr], "look1", now);
    assert_eq!(failing.len(), 1);
    assert_eq!(failing[0].urgency, ScoutUrgency::Now);
    assert_eq!(
        failing[0].said,
        "Your pull request #212, Talk starts from any app: 2 of 9 checks did not pass."
    );
    assert_eq!(
        failing[0].open,
        Some(ScoutOpen::Url {
            url: PR_URL.to_owned(),
            label: "Open the pull request".to_owned(),
        })
    );
    assert_eq!(failing[0].key, format!("pr:{PR_URL}:failing::"));

    let changes_and_conflicts = FolderPullRequest {
        review: Some(PullRequestReview::ChangesRequested),
        conflicts: true,
        ..sample_pr()
    };
    let soon = pull_request_facts(&HashMap::new(), &[changes_and_conflicts], "look1", now);
    assert_eq!(soon.len(), 1);
    assert_eq!(soon[0].urgency, ScoutUrgency::Soon);
    assert!(
        soon[0]
            .said
            .contains("changes were asked for, and it conflicts with its base branch")
    );
}

#[test]
fn pull_requests_tell_what_changed_since_the_last_look_keyed_by_the_look_so_the_same_state_coming_back_is_news_again() {
    let now = dt("2026-10-12T15:00:00.000Z");
    let mut before = HashMap::new();
    before.insert(PR_URL.to_owned(), sample_pr());

    let failing_1 = FolderPullRequest {
        checks: PullRequestChecks::Failing,
        not_passed: 1,
        ..sample_pr()
    };
    let news = pull_request_facts(&before, &[failing_1.clone()], "look7", now);
    assert_eq!(news.len(), 1);
    assert_eq!(news[0].key, format!("pr:{PR_URL}:look7"));
    assert_eq!(news[0].urgency, ScoutUrgency::Now);
    assert_eq!(
        news[0].said,
        "Your pull request #212, Talk starts from any app: 1 of 9 checks did not pass."
    );

    let mut before_failing = HashMap::new();
    before_failing.insert(PR_URL.to_owned(), failing_1);
    let passed = pull_request_facts(&before_failing, &[sample_pr()], "look8", now);
    assert_eq!(passed.len(), 1);
    assert_eq!(passed[0].key, format!("pr:{PR_URL}:look8"));
    assert_eq!(passed[0].urgency, ScoutUrgency::Later);
    assert!(passed[0].said.contains("all 9 checks passed"));

    assert!(pull_request_facts(&before, &[sample_pr()], "look9", now).is_empty());
}

#[test]
fn pull_requests_ready_and_waiting_passing_mergeable_nothing_asked_untouched_three_days_are_said_once_for_later() {
    let at = dt("2026-10-12T15:00:00.000Z");
    let waited = FolderPullRequest {
        updated_at: Some("2026-08-28T05:03:26Z".to_owned()),
        ..sample_pr()
    };
    let ready = pull_request_facts(&HashMap::new(), &[waited.clone()], "look1", at);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].key, format!("pr:{PR_URL}:ready"));
    assert_eq!(ready[0].urgency, ScoutUrgency::Later);
    assert_eq!(
        ready[0].said,
        "Your pull request #212, Talk starts from any app, passes its checks and has waited 45 days. It can be merged, or closed."
    );
    assert_eq!(
        ready[0].open,
        Some(ScoutOpen::Url {
            url: PR_URL.to_owned(),
            label: "Open the pull request".to_owned(),
        })
    );

    let mut known_waited = HashMap::new();
    known_waited.insert(PR_URL.to_owned(), waited.clone());
    let keys: Vec<String> = pull_request_facts(&known_waited, &[waited.clone()], "look2", at)
        .into_iter()
        .map(|f| f.key)
        .collect();
    assert_eq!(keys, vec![format!("pr:{PR_URL}:ready")]);

    let two_days = FolderPullRequest {
        updated_at: Some("2026-10-10T16:00:00Z".to_owned()),
        ..sample_pr()
    };
    assert!(pull_request_facts(&HashMap::new(), &[two_days], "look1", at).is_empty());

    let draft = FolderPullRequest {
        updated_at: waited.updated_at.clone(),
        state: PullRequestState::Draft,
        ..sample_pr()
    };
    assert!(pull_request_facts(&HashMap::new(), &[draft], "look1", at).is_empty());

    let conflicted = FolderPullRequest {
        updated_at: waited.updated_at.clone(),
        conflicts: true,
        ..sample_pr()
    };
    let mut known_conflicted = HashMap::new();
    known_conflicted.insert(PR_URL.to_owned(), conflicted.clone());
    assert!(pull_request_facts(&known_conflicted, &[conflicted], "look2", at).is_empty());

    let mut before_failing = HashMap::new();
    before_failing.insert(
        PR_URL.to_owned(),
        FolderPullRequest {
            checks: PullRequestChecks::Failing,
            not_passed: 1,
            ..sample_pr()
        },
    );
    let passed_again: Vec<String> = pull_request_facts(&before_failing, &[waited], "look3", at)
        .into_iter()
        .map(|f| f.key)
        .collect();
    assert_eq!(passed_again, vec![format!("pr:{PR_URL}:look3")]);
}

#[test]
fn pull_requests_are_read_from_gh_pr_list_and_anything_that_is_not_one_is_left_out() {
    let raw = json!([
        {
            "number": 1,
            "title": "One",
            "url": "https://github.com/acme/app/pull/1",
            "state": "OPEN",
            "isDraft": false,
            "statusCheckRollup": [{ "status": "COMPLETED", "conclusion": "FAILURE" }],
            "reviewDecision": "",
            "mergeable": "MERGEABLE"
        },
        { "number": "two", "url": "https://github.com/acme/app/pull/2" },
        "nothing"
    ])
    .to_string();
    let mapped: Vec<(u64, PullRequestChecks)> = pull_requests_of(&raw)
        .into_iter()
        .map(|one| (one.number, one.checks))
        .collect();
    assert_eq!(mapped, vec![(1, PullRequestChecks::Failing)]);
    assert!(pull_requests_of("not json").is_empty());
    assert!(pull_requests_of(r#"{"number":1}"#).is_empty());
}

fn sample_issue(number: u64, created_at: &str) -> IssueOpened {
    IssueOpened {
        number,
        title: format!("Issue {number}"),
        url: format!("https://github.com/acme/app/issues/{number}"),
        created_at: created_at.to_owned(),
        author: Some("dana".to_owned()),
    }
}

#[test]
fn issues_opened_this_week_are_news_newest_first_three_a_look_each_said_once_by_its_address() {
    let at = dt("2026-10-12T15:00:00.000Z");
    let issues = [
        sample_issue(1, "2026-10-01T00:00:00Z"),
        sample_issue(2, "2026-10-11T00:00:00Z"),
        sample_issue(3, "2026-10-12T10:00:00Z"),
        sample_issue(4, "2026-10-10T00:00:00Z"),
        sample_issue(5, "2026-10-09T00:00:00Z"),
    ];
    let facts = issue_facts(&issues, at, &HashSet::new());
    let keys: Vec<&str> = facts.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "issue:https://github.com/acme/app/issues/3",
            "issue:https://github.com/acme/app/issues/2",
            "issue:https://github.com/acme/app/issues/4",
        ]
    );
    assert_eq!(facts[0].urgency, ScoutUrgency::Soon);
    assert_eq!(facts[0].said, "dana opened issue #3: Issue 3.");
    assert_eq!(
        facts[0].open,
        Some(ScoutOpen::Url {
            url: "https://github.com/acme/app/issues/3".to_owned(),
            label: "Open the issue".to_owned(),
        })
    );
}

#[test]
fn issues_are_read_from_gh_issue_list_and_anything_that_is_not_one_is_left_out() {
    let raw = json!([
        {
            "number": 7,
            "title": "It  crashes\\non start",
            "url": "https://github.com/acme/app/issues/7",
            "createdAt": "2026-10-12T00:00:00Z",
            "author": { "login": "dana" }
        },
        {
            "number": 8,
            "title": "A pull request",
            "url": "https://github.com/acme/app/pull/8",
            "createdAt": "2026-10-12T00:00:00Z"
        },
        {
            "number": 9,
            "title": "No date",
            "url": "https://github.com/acme/app/issues/9"
        },
        "nothing"
    ])
    .to_string();
    assert_eq!(
        issues_of(&raw),
        vec![IssueOpened {
            number: 7,
            title: "It crashes\\non start".to_owned(),
            url: "https://github.com/acme/app/issues/7".to_owned(),
            created_at: "2026-10-12T00:00:00Z".to_owned(),
            author: Some("dana".to_owned()),
        }]
    );
    assert!(issues_of("not json").is_empty());
}

#[test]
fn a_review_asked_of_the_person_names_who_asked() {
    let asked = reviews_asked_of(
        &json!([
            {
                "number": 214,
                "title": "Rename the call log",
                "url": "https://github.com/acme/app/pull/214",
                "author": { "login": "ada" }
            },
            {
                "number": 5,
                "url": "https://evil.example/pull/5"
            }
        ])
        .to_string(),
    );
    assert_eq!(
        asked,
        vec![ReviewAsked {
            number: 214,
            title: "Rename the call log".to_owned(),
            url: "https://github.com/acme/app/pull/214".to_owned(),
            author: Some("ada".to_owned()),
        }]
    );
    let facts = review_facts(&asked);
    assert_eq!(facts[0].key, "review:https://github.com/acme/app/pull/214");
    assert_eq!(facts[0].urgency, ScoutUrgency::Soon);
    assert_eq!(
        facts[0].said,
        "ada asked you to review pull request #214, Rename the call log."
    );
}

#[test]
fn the_newest_finished_run_of_each_workflow_says_how_the_branch_stands_only_a_failed_one_is_news() {
    let runs = branch_runs_of(
        &json!([
            {
                "databaseId": 9,
                "status": "in_progress",
                "conclusion": "",
                "workflowName": "ci",
                "url": "https://github.com/acme/app/actions/runs/9"
            },
            {
                "databaseId": 8,
                "status": "completed",
                "conclusion": "failure",
                "workflowName": "ci",
                "url": "https://github.com/acme/app/actions/runs/8"
            },
            {
                "databaseId": 7,
                "status": "completed",
                "conclusion": "success",
                "workflowName": "ci",
                "url": "https://github.com/acme/app/actions/runs/7"
            },
            {
                "databaseId": 6,
                "status": "completed",
                "conclusion": "success",
                "workflowName": "docs",
                "url": "https://github.com/acme/app/actions/runs/6"
            }
        ])
        .to_string(),
    );
    let summary: Vec<(&str, u64, bool)> = runs
        .iter()
        .map(|run| (run.workflow.as_str(), run.id, run.failed))
        .collect();
    assert_eq!(summary, vec![("ci", 8, true), ("docs", 6, false)]);
    let facts = branch_facts("main", &runs);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].key, "branch:8");
    assert_eq!(facts[0].said, "ci failed on main.");
}

#[test]
fn teammates_waiting_on_the_person_is_news_after_ten_minutes_and_needs_them_now() {
    let card = WaitingCard {
        approval_id: "ap_1".to_owned(),
        mission_id: "mis_1".to_owned(),
        teammate_id: Some("tm_atlas".to_owned()),
        name: Some("Atlas".to_owned()),
        summary: "Run pnpm test".to_owned(),
        requested_at: "2026-10-12T05:00:00.000Z".to_owned(),
    };
    assert!(
        waiting_facts(
            std::slice::from_ref(&card),
            dt("2026-10-12T05:09:00.000Z"),
            SCOUT_WAITING_MS
        )
        .is_empty()
    );
    let waiting = waiting_facts(
        &[card],
        dt("2026-10-12T05:25:00.000Z"),
        SCOUT_WAITING_MS,
    );
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].key, "waiting:ap_1");
    assert_eq!(waiting[0].urgency, ScoutUrgency::Now);
    assert_eq!(waiting[0].said, "Atlas has waited 25 minutes for your OK.");
    assert_eq!(
        waiting[0].open,
        Some(ScoutOpen::Conversation {
            mission_id: "mis_1".to_owned(),
            label: "Open it".to_owned(),
        })
    );
    // What it waits to do can be a command with a secret in it: the person sees it, Scout's model never does.
    assert_eq!(waiting[0].quote.as_deref(), Some("Run pnpm test"));
    assert!(!scout_prompt(&waiting, &[], None, &[]).contains("pnpm test"));
}

#[test]
fn a_turn_that_failed_says_whose_and_why_in_the_runtimes_own_words_with_what_was_asked_for_the_model() {
    let f1 = failed_facts(&[FailedTurn {
        mission_id: "mis_9".to_owned(),
        teammate_id: Some("tm_wren".to_owned()),
        name: Some("Wren".to_owned()),
        asked: "Fix the save".to_owned(),
        why: Some("Model not found: opencode/no-such-model-free.".to_owned()),
    }]);
    assert_eq!(f1[0].key, "failed:mis_9");
    assert_eq!(
        f1[0].said,
        "Wren's turn failed: Model not found: opencode/no-such-model-free."
    );
    assert_eq!(f1[0].detail.as_deref(), Some("It was asked: Fix the save"));
    assert_eq!(f1[0].teammate_id.as_deref(), Some("tm_wren"));

    let f2 = failed_facts(&[FailedTurn {
        mission_id: "mis_8".to_owned(),
        teammate_id: None,
        name: None,
        asked: "Fix the save".to_owned(),
        why: None,
    }]);
    assert_eq!(f2[0].said, "A teammate's turn failed.");
}

#[test]
fn scouts_model_is_asked_about_the_new_facts_and_the_team_read_only_in_one_form() {
    let facts = failed_facts(&[FailedTurn {
        mission_id: "mis_9".to_owned(),
        teammate_id: None,
        name: Some("Wren".to_owned()),
        asked: "Fix the save".to_owned(),
        why: Some("Overloaded.".to_owned()),
    }]);
    let prompt = scout_prompt(
        &facts,
        &[PromptTeammate {
            name: "Atlas",
            role: "Reviews",
        }],
        None,
        &[],
    );
    assert!(prompt.contains(
        "Use only what is below: do not use any tools, open files, run commands or change anything."
    ));
    assert!(prompt.contains("1. Wren's turn failed: Overloaded.\n   It was asked: Fix the save"));
    assert!(prompt.contains("- Atlas, Reviews"));
    assert!(prompt.contains("ITEM <its number>"));
    assert!(prompt.contains("Say nothing that is not in what is above."));
}

#[test]
fn scouts_model_is_read_loosely_bold_labels_a_colon_after_the_number_a_draft_over_several_lines() {
    let team = [
        AnswerTeammate {
            teammate_id: "tm_wren",
            name: "Wren",
        },
        AnswerTeammate {
            teammate_id: "tm_atlas",
            name: "Atlas",
        },
    ];
    let answer = scout_answers(
        &[
            "Here is what I think.",
            "",
            "**ITEM 1:**",
            "**TAKE:** The timeout is new since this morning.",
            "HAND TO: Wren.",
            "DRAFT: Find why the save waits on Mac.",
            "Run the test three times.",
            "",
            "ITEM 2",
            "TAKE: Approved and clean.",
            "HAND TO: none",
        ]
        .join("\n"),
        2,
        &team,
    );
    assert_eq!(
        answer.get(&1),
        Some(&ScoutAnswer {
            take: Some("The timeout is new since this morning.".to_owned()),
            draft: Some(ScoutDraft {
                teammate_id: "tm_wren".to_owned(),
                name: "Wren".to_owned(),
                message: "Find why the save waits on Mac.\nRun the test three times.".to_owned(),
            }),
        })
    );
    assert_eq!(
        answer.get(&2),
        Some(&ScoutAnswer {
            take: Some("Approved and clean.".to_owned()),
            draft: None,
        })
    );
}

#[test]
fn scouts_model_is_kept_strictly_no_item_that_was_not_asked_no_teammate_who_is_not_on_the_team_no_draft_with_nobody_to_send_it_to() {
    let team = [
        AnswerTeammate {
            teammate_id: "tm_wren",
            name: "Wren",
        },
        AnswerTeammate {
            teammate_id: "tm_atlas",
            name: "Atlas",
        },
    ];
    let answer = scout_answers(
        &[
            "ITEM 3",
            "TAKE: Out of range.",
            "ITEM 1",
            "HAND TO: Mallory",
            "DRAFT: Delete everything.",
            "ITEM 2",
            "TAKE: Kept.",
            "HAND TO: Mallory",
            "DRAFT: Send nothing.",
        ]
        .join("\n"),
        2,
        &team,
    );
    assert!(!answer.contains_key(&3));
    assert!(!answer.contains_key(&1));
    assert_eq!(
        answer.get(&2),
        Some(&ScoutAnswer {
            take: Some("Kept.".to_owned()),
            draft: None,
        })
    );
    assert!(scout_answers("nothing in the form", 2, &team).is_empty());
}

#[test]
fn cards_keep_the_newest_at_most_max_scout_cards() {
    let fact = failed_facts(&[FailedTurn {
        mission_id: "mis_1".to_owned(),
        teammate_id: None,
        name: None,
        asked: "x".to_owned(),
        why: None,
    }])
    .into_iter()
    .next()
    .unwrap();
    let many: Vec<ScoutCard> = (0..MAX_SCOUT_CARDS)
        .map(|index| card_of(fact.clone(), format!("card_{index}"), dt("1970-01-01T00:00:00.000Z"), false))
        .collect();
    let added = card_of(fact, "card_new", dt("1970-01-01T00:00:00.001Z"), true);
    let kept = with_new_cards(&many, &[added]);
    assert_eq!(kept.len(), MAX_SCOUT_CARDS);
    assert_eq!(kept[0].card_id, "card_new");
    assert!(kept[0].reading);
    assert_eq!(
        kept[MAX_SCOUT_CARDS - 1].card_id,
        format!("card_{}", MAX_SCOUT_CARDS - 2)
    );
}

#[test]
fn settings_take_only_what_a_person_could_have_set() {
    let defaults = ScoutSettings::default();
    assert_eq!(scout_settings_from(&serde_json::Value::Null, &defaults), defaults);
    assert_eq!(
        scout_settings_from(
            &json!({
                "on": true,
                "everyMinutes": 30,
                "watches": { "github": false },
                "notify": false,
                "teammateId": "tm_scout"
            }),
            &defaults,
        ),
        ScoutSettings {
            on: true,
            teammate_id: Some("tm_scout".to_owned()),
            every_minutes: 30,
            watches: ScoutWatches {
                github: false,
                teammates: true,
                check: false,
                comments: true,
            },
            notify: false,
            wait_while_working: true,
            ..defaults.clone()
        }
    );
    assert_eq!(
        scout_settings_from(
            &json!({ "everyMinutes": 1, "teammateId": "../x", "on": "yes" }),
            &defaults,
        ),
        defaults
    );
}

#[test]
fn new_issues_overflow_holds_fourth_issue_until_earlier_three_are_told() {
    let at = dt("2026-10-12T15:00:00.000Z");
    let issues = [
        sample_issue(1, "2026-10-01T00:00:00Z"),
        sample_issue(2, "2026-10-11T00:00:00Z"),
        sample_issue(3, "2026-10-12T10:00:00Z"),
        sample_issue(4, "2026-10-10T00:00:00Z"),
        sample_issue(5, "2026-10-09T00:00:00Z"),
    ];
    let split = new_issues(&issues, at, &HashSet::new());
    assert_eq!(split.facts.len(), 3);
    assert_eq!(split.more.len(), 1);
    assert_eq!(split.more[0].key, "issue:https://github.com/acme/app/issues/5");
    let told: HashSet<String> = split.facts.iter().map(|f| f.key.clone()).collect();
    let next = new_issues(&issues, at, &told);
    assert_eq!(next.facts.len(), 4);
    assert!(next.more.is_empty());
}

#[test]
fn check_facts_report_only_transitions_and_tail_of_output() {
    let fail = FolderCheckResult {
        command: "npm test".into(),
        passed: false,
        lines: vec!["ok 1".into(), "FAIL tests/a.test.ts".into()],
    };
    let pass = FolderCheckResult {
        command: "npm test".into(),
        passed: true,
        lines: vec!["all good".into()],
    };
    // Passing on first run is quiet.
    assert!(check_facts(None, &pass, "m1").is_empty());
    // Failing on first run raises.
    let f1 = check_facts(None, &fail, "m1");
    assert_eq!(f1.len(), 1);
    assert_eq!(f1[0].key, "check:m1:fails");
    assert_eq!(f1[0].urgency, ScoutUrgency::Now);
    assert_eq!(f1[0].said, "The check npm test fails: ok 1");
    assert!(f1[0].detail.as_deref().unwrap().contains("FAIL tests/a.test.ts"));
    // Staying failed is quiet.
    assert!(check_facts(Some(false), &fail, "m2").is_empty());
    // Turning passing raises later.
    let p1 = check_facts(Some(false), &pass, "m3");
    assert_eq!(p1.len(), 1);
    assert_eq!(p1[0].key, "check:m3:passes");
    assert_eq!(p1[0].said, "The check npm test passes again.");
    // Turning failing after passing includes "since the folder changed".
    let f2 = check_facts(Some(true), &fail, "m4");
    assert!(f2[0].said.contains("since the folder changed"));
}

#[test]
fn quiet_hours_and_hour_words_handle_overnight_and_daytime_ranges() {
    let overnight = Some(ScoutQuietHours { from: 22, to: 7 });
    assert!(in_quiet_hours(overnight, dt("2026-10-12T22:00:00-04:00")));
    assert!(in_quiet_hours(overnight, dt("2026-10-13T06:59:00-04:00")));
    assert!(!in_quiet_hours(overnight, dt("2026-10-13T07:00:00-04:00")));
    assert!(!in_quiet_hours(overnight, dt("2026-10-12T21:59:00-04:00")));
    let daytime = Some(ScoutQuietHours { from: 9, to: 17 });
    assert!(in_quiet_hours(daytime, dt("2026-10-12T09:00:00-04:00")));
    assert!(!in_quiet_hours(daytime, dt("2026-10-12T17:00:00-04:00")));
    assert_eq!(hour_words(0), "midnight");
    assert_eq!(hour_words(12), "noon");
    assert_eq!(hour_words(7), "7 AM");
    assert_eq!(hour_words(22), "10 PM");
}

#[test]
fn notes_from_settings_brief_span_and_week_fact_match_electron() {
    let notes = notes_from("- first\n  \n• second\n* third");
    assert_eq!(notes, vec!["first", "second", "third"]);
    let many: String = (0..50).map(|i| format!("n{i}")).collect::<Vec<_>>().join("\n");
    assert_eq!(notes_from(&many).len(), MAX_SCOUT_NOTES);
    assert_eq!(notes_from(&many).first().unwrap(), "n10");

    assert_eq!(brief_span(dt("2026-10-11T08:00:00Z"), dt("2026-10-12T08:00:00Z")), "Since yesterday");
    assert_eq!(brief_span(dt("2026-10-09T08:00:00Z"), dt("2026-10-12T08:00:00Z")), "Since Friday");
    assert_eq!(brief_span(dt("2026-10-01T08:00:00Z"), dt("2026-10-12T08:00:00Z")), "Since October 1");

    let mut week = empty_week("2026-10-05");
    week.looks = 20;
    week.asked = 4;
    week.github = ScoutWeekWatch { raised: 4, used: 0, dismissed: 4 };
    week.dismissed = 4;
    let fact = week_fact(&week, 10, ScoutWatches::default());
    assert_eq!(fact.key, "week:2026-10-05");
    assert_eq!(fact.said, "This week: 20 looks, 4 of them read by its model.");
    assert_eq!(fact.tune.as_ref().unwrap().watch_off, Some(ScoutWatchKey::Github));

    let quiet_week = empty_week("2026-10-05");
    let quiet_fact = week_fact(&quiet_week, 10, ScoutWatches::default());
    assert_eq!(quiet_fact.tune.as_ref().unwrap().every_minutes, Some(30));
    assert_eq!(days_between("2026-10-05", "2026-10-12"), 7);
}

#[test]
fn settings_from_clips_goal_and_validates_choices() {
    let base = ScoutSettings::default();
    let next = scout_settings_from(
        &json!({
            "on": true,
            "everyMinutes": 30,
            "quiet": {"from": 22, "to": 7},
            "briefAt": 8,
            "goal": "  watch\n  CI  ",
            "watches": {"check": true}
        }),
        &base,
    );
    assert!(next.on);
    assert_eq!(next.every_minutes, 30);
    assert_eq!(next.quiet, Some(ScoutQuietHours { from: 22, to: 7 }));
    assert_eq!(next.brief_at, Some(8));
    assert_eq!(next.goal.as_deref(), Some("watch CI"));
    assert!(next.watches.check);

    let cleared = apply_settings_change(
        &next,
        &ScoutChange {
            goal: Some("".into()),
            quiet: Some(None),
            brief_at: Some(None),
            ..Default::default()
        },
    );
    assert_eq!(cleared.goal, None);
    assert_eq!(cleared.quiet, None);
    assert_eq!(cleared.brief_at, None);
}

// ---- main/scout-reads-what-github-has.test.ts (2 cases) ----

#[test]
fn read_scout_github_asks_for_updated_at_and_issues_with_cli_in_folder() {
    let mut calls: Vec<(PathBuf, Vec<String>)> = Vec::new();
    let answers = [
        ("pr list --author", json!([{"number":2,"title":"Stop claiming empty results","url":"https://github.com/acme/hearth/pull/2","state":"OPEN","isDraft":false,"statusCheckRollup":[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","mergeable":"MERGEABLE","updatedAt":"2026-08-28T05:03:26Z"}]).to_string()),
        ("pr list --search", "[]".to_owned()),
        ("repo view", "main\n".to_owned()),
        ("run list", json!([
            {"databaseId":10,"status":"completed","conclusion":"failure","workflowName":AWAY_WORKFLOW_NAME,"url":"https://github.com/acme/hearth/actions/runs/10"},
            {"databaseId":11,"status":"completed","conclusion":"success","workflowName":"Tests","url":"https://github.com/acme/hearth/actions/runs/11"}
        ]).to_string()),
        ("issue list", json!([{"number":5,"title":"Crashes on start","url":"https://github.com/acme/hearth/issues/5","createdAt":"2026-10-12T09:00:00Z","author":{"login":"dana"}}]).to_string()),
    ];
    let folder = Path::new("/tmp/hearth");
    let mut unread_reason = None;
    let mut cb = |why: ScoutGithubUnread| unread_reason = Some(why);
    let read = read_scout_github(
        folder,
        |args, cwd| {
            calls.push((cwd.to_path_buf(), args.iter().map(|s| (*s).to_owned()).collect()));
            let joined = args.join(" ");
            Ok(answers
                .iter()
                .find(|(k, _)| joined.starts_with(k))
                .map(|(_, v)| v.clone())
                .unwrap_or_default())
        },
        None,
        Some(&mut cb),
    )
    .expect("github read");

    assert!(unread_reason.is_none());
    assert!(calls.iter().all(|(cwd, _)| cwd == folder));
    let author_call = calls.iter().find(|(_, a)| a.len() >= 3 && a[0] == "pr" && a[2] == "--author").unwrap();
    assert_eq!(
        author_call.1.last().unwrap(),
        "number,title,url,state,isDraft,statusCheckRollup,reviewDecision,mergeable,updatedAt"
    );
    assert_eq!(read.prs.len(), 1);
    assert_eq!(read.prs[0].updated_at.as_deref(), Some("2026-08-28T05:03:26Z"));
    assert_eq!(read.issues.len(), 1);
    assert_eq!(read.issues[0].author.as_deref(), Some("dana"));
    // Filters out Locust Scout away workflow run!
    assert_eq!(read.branch.as_ref().unwrap().runs.len(), 1);
    assert_eq!(read.branch.as_ref().unwrap().runs[0].workflow, "Tests");
}

#[test]
fn read_scout_github_asks_nothing_more_when_first_read_fails_and_classifies_reason() {
    let mut calls = 0usize;
    let mut why = None;
    let mut cb = |r: ScoutGithubUnread| why = Some(r);
    let read = read_scout_github(
        Path::new("/tmp/plain"),
        |_args, _| {
            calls += 1;
            Err(GhError::Failed("not a git repository".into()))
        },
        None,
        Some(&mut cb),
    );
    assert!(read.is_none());
    assert_eq!(calls, 1);
    assert_eq!(why, Some(ScoutGithubUnread::NotOnGithub));

    assert_eq!(github_unread_why(&GhError::NotFound("enoent".into())), ScoutGithubUnread::NoCli);
    assert_eq!(github_unread_why(&GhError::Failed("HTTP 401 bad credentials".into())), ScoutGithubUnread::SignedOut);
    assert_eq!(github_unread_why(&GhError::Failed("gh: command not found".into())), ScoutGithubUnread::NoCli);
    assert_eq!(github_unread_why(&GhError::Failed("timeout".into())), ScoutGithubUnread::Unreadable);
}

// ---- main/scout-folder.ts folder_mark_of ----

#[test]
fn folder_mark_changes_when_head_or_file_mtime_changes_and_none_outside_git() {
    let dir = TempDir::new("foldermark");
    let f = dir.0.join("a.txt");
    fs::write(&f, "hello").unwrap();

    assert_eq!(folder_mark_of(&dir.0, |_, _| Err("not git".into())), None);
    let m1 = folder_mark_of(&dir.0, |args, _| {
        Ok(match args[0] {
            "rev-parse" => "abc123\n".into(),
            _ => " M a.txt\0R  new.txt\0old.txt\0".into(),
        })
    })
    .unwrap();
    assert_eq!(m1.len(), 32);

    fs::write(&f, "hello world").unwrap();
    let m2 = folder_mark_of(&dir.0, |args, _| {
        Ok(match args[0] {
            "rev-parse" => "abc123\n".into(),
            _ => " M a.txt\0".into(),
        })
    })
    .unwrap();
    assert_ne!(m1, m2);

    // Identical output produces identical mark even with mtime preserved.
    let same = folder_mark_of(&dir.0, |args, _| {
        Ok(match args[0] {
            "rev-parse" => "abc123\n".into(),
            _ => " M a.txt\0".into(),
        })
    })
    .unwrap();
    assert_eq!(m2, same);
}

// ---- main/a-scout-file-it-cannot-read-is-never-overwritten.test.ts (3 cases) + Store safety ----

#[test]
fn unreadable_or_corrupt_scout_file_is_never_overwritten() {
    let now = dt("2026-10-15T10:00:00Z");

    // 1. Busy/transient read error retries 3 times, then falls back to memory-only (save_to_disk = false).
    let dir = TempDir::new("busy");
    let file = dir.0.join("scout.json");
    let untouched = r#"{"settings":{"on":true},"cards":[],"seen":[],"lookIds":["important"],"prs":[]}"#;
    fs::write(&file, untouched).unwrap();
    let attempts = AtomicUsize::new(0);
    let mut sleeps = Vec::new();
    let loaded = load_kept_with(
        &ScoutFiles::Paths { file: file.clone(), log_file: None },
        now,
        |_| {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::from(std::io::ErrorKind::ResourceBusy))
        },
        |d| sleeps.push(d),
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
    assert_eq!(sleeps, vec![Duration::from_millis(200), Duration::from_millis(400), Duration::from_millis(800)]);
    assert!(!loaded.save_to_disk);
    assert_eq!(fs::read_to_string(&file).unwrap(), untouched);

    // 2. Corrupt JSON is moved aside to scout.json.unreadable-<stamp> and starts fresh.
    let dir2 = TempDir::new("corrupt");
    let file2 = dir2.0.join("scout.json");
    fs::write(&file2, "{ not valid json").unwrap();
    let files2 = ScoutFiles::Paths { file: file2.clone(), log_file: None };
    let loaded2 = load_kept_with(&files2, now, |p| fs::read_to_string(p), |_| {});
    assert!(loaded2.save_to_disk);
    assert!(!loaded2.kept.settings.on);
    assert!(!file2.exists());
    let backups: Vec<_> = fs::read_dir(&dir2.0)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("scout.json.unreadable-"))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::read_to_string(backups[0].path()).unwrap(), "{ not valid json");

    // 3. Directory at scout.json (cannot be read or renamed over cleanly on Unix -> actually rename succeeds on dir unless non-empty, wait: reader fails with IsADirectory, retried 3 times -> save_to_disk = false!).
    let dir3 = TempDir::new("isdir");
    let file3 = dir3.0.join("scout.json");
    fs::create_dir_all(file3.join("keep")).unwrap();
    let files3 = ScoutFiles::Paths { file: file3.clone(), log_file: None };
    let loaded3 = load_kept_with(&files3, now, |p| fs::read_to_string(p), |_| {});
    assert!(!loaded3.save_to_disk);
    assert!(file3.join("keep").exists());
}

#[test]
fn store_reads_electron_scout_read_only_and_writes_only_native() {
    let dir = TempDir::new("store-iso");
    let native = dir.0.join("native");
    let electron = dir.0.join("electron");
    fs::create_dir_all(&native).unwrap();
    fs::create_dir_all(&electron).unwrap();
    let el_scout = electron.join("scout.json");
    let el_body = json!({"settings":{"on":true,"everyMinutes":30},"notes":["From Electron"]}).to_string();
    fs::write(&el_scout, &el_body).unwrap();

    let store = Store::new(native.clone(), electron.clone()).unwrap();
    let files = ScoutFiles::Store(store.clone());
    let now = dt("2026-10-22T12:00:00Z");
    let loaded = load_kept_with(&files, now, |p| fs::read_to_string(p), |_| {});
    assert!(loaded.save_to_disk);
    assert!(loaded.kept.settings.on);
    assert_eq!(loaded.kept.settings.every_minutes, 30);
    assert_eq!(loaded.kept.notes, vec!["From Electron"]);

    // Saving writes native/scout.json and leaves electron/scout.json untouched.
    let mut updated = loaded.kept;
    updated.notes.push("From Native".into());
    files
        .save_json_atomic(&files.scout_path().unwrap(), SCOUT_FILE, &updated.to_json())
        .unwrap();
    assert!(native.join("scout.json").exists());
    assert_eq!(fs::read_to_string(&el_scout).unwrap(), el_body);
}

// ---- shared/scout-log.ts ----

#[test]
fn scout_log_merges_quiet_looks_prunes_old_and_renders_brief() {
    let tz = FixedOffset::west_opt(4 * 3600).unwrap();
    let t1 = DateTime::parse_from_rfc3339("2026-10-22T21:40:00-04:00").unwrap();
    let t2 = DateTime::parse_from_rfc3339("2026-10-22T21:50:00-04:00").unwrap();
    assert_eq!(look_time(t1), "Thu Oct 22, 9:40 PM");

    let e1 = look_entry(&LookEntryInput {
        at: t1,
        raised: &[],
        aside: &[ScoutLogAside::new("pr:1", "Still failing", ScoutAsideWhy::Told)],
        off: &[ScoutLogWatch::Check],
        github_unread: false,
        github: None,
        skipped_quiet_hours: false,
        away: false,
    });
    let e2 = look_entry(&LookEntryInput {
        at: t2,
        ..LookEntryInput {
            at: t1,
            raised: &[],
            aside: &[ScoutLogAside::new("pr:1", "Still failing", ScoutAsideWhy::Told)],
            off: &[ScoutLogWatch::Check],
            github_unread: false,
            github: None,
            skipped_quiet_hours: false,
            away: false,
        }
    });
    let merged = with_look(&[e1], e2, t2);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].looks, 2);
    assert_eq!(look_name(&merged[0], tz), "The 2 looks from Thu Oct 22, 9:40 PM to Thu Oct 22, 9:50 PM");
    assert!(set_something_aside(&merged[0]));

    let brief = scout_log_brief(
        &merged,
        &[ScoutHeldAlert {
            at: t2.timestamp_millis(),
            title: "Build broken".into(),
            why: HeldWhy::QuietHours,
            about: None,
        }],
        t2,
    );
    assert!(brief.contains("The 2 looks from Thu Oct 22, 9:40 PM to Thu Oct 22, 9:50 PM:"));
    assert!(brief.contains("Set aside, told you already: Still failing"));
    assert!(brief.contains("Not read, its watch is off: the folder’s check."));
    assert!(brief.contains("Alerts kept off the phone since Locust started:"));

    // Roundtrip through JSON keeps away: true and all fields.
    let away_entry = look_entry(&LookEntryInput {
        at: t1,
        raised: &[ScoutLogItem::new("pr:9", "PR #9 failed")],
        aside: &[],
        off: &[],
        github_unread: false,
        github: None,
        skipped_quiet_hours: false,
        away: true,
    });
    let roundtrip = scout_log_from(&serde_json::Value::Array(vec![away_entry.to_json()]));
    assert_eq!(roundtrip, vec![away_entry]);

    // Old entries (>14 days) are pruned on next write.
    let fifteen_days_later = DateTime::parse_from_rfc3339("2026-11-06T22:00:00-04:00").unwrap();
    let e_new = look_entry(&LookEntryInput {
        at: fifteen_days_later,
        raised: &[],
        aside: &[],
        off: &[],
        github_unread: false,
        github: None,
        skipped_quiet_hours: true,
        away: false,
    });
    let pruned = with_look(&merged, e_new, fifteen_days_later);
    assert_eq!(pruned.len(), 1);
    assert!(pruned[0].skipped_quiet_hours);

    let _ = SystemTime::now().duration_since(UNIX_EPOCH);
}

// Review (13 Nov): the two breaks no test caught.

#[test]
fn quiet_hours_from_and_to_the_same_hour_are_never_quiet() {
    let same = ScoutQuietHours { from: 9, to: 9 };
    for hour in [0, 8, 9, 10, 23] {
        assert!(!in_quiet_hours(Some(same), local_dt_for_review(hour)), "{hour}");
    }
}

#[test]
fn an_interval_settings_does_not_offer_is_never_kept() {
    let base = ScoutSettings::default();
    for refused in [5, 15, 120, 0] {
        assert_eq!(scout_settings_from(&json!({ "everyMinutes": refused }), &base).every_minutes, base.every_minutes, "{refused}");
    }
    for offered in SCOUT_EVERY_MINUTES {
        assert_eq!(scout_settings_from(&json!({ "everyMinutes": offered }), &base).every_minutes, offered);
    }
}

fn local_dt_for_review(hour: u32) -> chrono::DateTime<chrono::FixedOffset> {
    use chrono::TimeZone;
    chrono::FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 10, 22, hour, 30, 0).unwrap()
}
