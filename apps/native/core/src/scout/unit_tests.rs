use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset};
use serde_json::json;

use crate::pull_requests::{FolderPullRequest, PullRequestChecks, PullRequestReview, PullRequestState};
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

fn open_pr(
    number: u64,
    checks: PullRequestChecks,
    check_count: usize,
    not_passed: usize,
    review: Option<PullRequestReview>,
    conflicts: bool,
    updated_at: Option<&str>,
) -> FolderPullRequest {
    FolderPullRequest {
        number,
        title: "Keep SSE open".to_owned(),
        url: format!("https://github.com/acme/repo/pull/{number}"),
        state: PullRequestState::Open,
        checks,
        check_count,
        not_passed,
        review,
        conflicts,
        updated_at: updated_at.map(str::to_owned),
    }
}

// ---- shared/scout-notices-what-is-new.test.ts (16 cases) ----

#[test]
fn only_tells_pull_requests_that_need_work_first_time_and_changes_afterwards() {
    let clean = open_pr(14, PullRequestChecks::Passing, 2, 0, None, false, None);
    let broken = open_pr(15, PullRequestChecks::Failing, 3, 1, None, false, None);
    let now = dt("2026-10-12T10:00:00Z");

    let first = pull_request_facts(&HashMap::new(), &[clean.clone(), broken.clone()], "look-1", now);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].urgency, ScoutUrgency::Now);
    assert!(first[0].said.contains("#15"));
    assert!(first[0].said.contains("1 of 3 checks did not pass"));

    let mut before = HashMap::new();
    before.insert(clean.url.clone(), clean.clone());
    before.insert(broken.url.clone(), broken);

    let same = pull_request_facts(&before, &[clean.clone()], "look-2", now);
    assert!(same.is_empty());

    let approved = FolderPullRequest {
        review: Some(PullRequestReview::Approved),
        ..clean
    };
    let next = pull_request_facts(&before, &[approved], "look-3", now);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].urgency, ScoutUrgency::Later);
    assert!(next[0].said.contains("it was approved"));
}

#[test]
fn notices_pull_request_passing_checks_waiting_ready_waiting_days_then_only_once() {
    let pr = open_pr(
        2,
        PullRequestChecks::Passing,
        1,
        0,
        None,
        false,
        Some("2026-10-08T00:00:00Z"),
    );
    let now = dt("2026-10-12T12:00:00Z");
    let first = pull_request_facts(&HashMap::new(), &[pr.clone()], "look-1", now);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].key, format!("pr:{}:ready", pr.url));
    assert_eq!(first[0].urgency, ScoutUrgency::Later);
    assert!(first[0].said.contains("#2"));
    assert!(first[0].said.contains("waited 4 days"));

    // Three days after the first look, same key means it stays told once.
    let mut before = HashMap::new();
    before.insert(
        pr.url.clone(),
        FolderPullRequest {
            updated_at: Some("2026-10-09T12:00:00Z".to_owned()),
            ..pr.clone()
        },
    );
    let younger = FolderPullRequest {
        updated_at: Some("2026-10-09T12:00:00Z".to_owned()),
        ..pr
    };
    let at_two_days = pull_request_facts(&before, &[younger.clone()], "look-2", dt("2026-10-11T12:00:00Z"));
    assert!(at_two_days.is_empty());
    let at_three_days = pull_request_facts(&before, &[younger], "look-3", dt("2026-10-12T13:00:00Z"));
    assert_eq!(at_three_days.len(), 1);
    assert_eq!(at_three_days[0].key, first[0].key);
    assert!(at_three_days[0].said.contains(&format!("waited {READY_WAITING_DAYS} days")));
}

#[test]
fn notices_new_issues_within_a_week_newest_first_capped_at_three_and_more_overflow() {
    let raw = serde_json::to_string(&json!([
        {"number":1,"title":"Too old","url":"https://github.com/acme/repo/issues/1","createdAt":"2026-10-01T00:00:00Z","author":{"login":"a"}},
        {"number":2,"title":"Two","url":"https://github.com/acme/repo/issues/2","createdAt":"2026-10-10T00:00:00Z","author":{"login":"b"}},
        {"number":3,"title":"Three","url":"https://github.com/acme/repo/issues/3","createdAt":"2026-10-11T00:00:00Z","author":{"login":"c"}},
        {"number":4,"title":"Four","url":"https://github.com/acme/repo/issues/4","createdAt":"2026-10-12T00:00:00Z","author":{"login":"d"}},
        {"number":5,"title":"Five","url":"https://github.com/acme/repo/issues/5","createdAt":"2026-10-12T09:00:00Z","author":{"login":"e"}}
    ]))
    .unwrap();
    let parsed = issues_of(&raw);
    assert_eq!(parsed.len(), 5);
    let now = dt("2026-10-12T10:00:00Z");

    let split = new_issues(&parsed, now, &HashSet::new());
    assert_eq!(split.facts.len(), 3);
    assert_eq!(split.facts.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(), vec![
        "issue:https://github.com/acme/repo/issues/5",
        "issue:https://github.com/acme/repo/issues/4",
        "issue:https://github.com/acme/repo/issues/3",
    ]);
    assert_eq!(split.more.len(), 1);
    assert_eq!(split.more[0].key, "issue:https://github.com/acme/repo/issues/2");

    // Next look picks up #2 once #3-#5 are in told.
    let mut told = HashSet::new();
    for f in &split.facts {
        told.insert(f.key.clone());
    }
    let next = new_issues(&parsed, now, &told);
    assert_eq!(next.facts.len(), 4);
    assert!(next.more.is_empty());
    assert!(next.facts.iter().any(|f| f.key == "issue:https://github.com/acme/repo/issues/2"));
}

#[test]
fn review_requests_and_failed_branch_runs_produce_expected_facts() {
    let reviews = reviews_asked_of(&json!([
        {"number":21,"title":"Fix auth","url":"https://github.com/acme/repo/pull/21","author":{"login":"maya"}},
        {"number":0,"title":"Invalid","url":"https://github.com/acme/repo/pull/0"}
    ]).to_string());
    assert_eq!(reviews.len(), 1);
    let rfacts = review_facts(&reviews);
    assert_eq!(rfacts[0].key, "review:https://github.com/acme/repo/pull/21");
    assert_eq!(rfacts[0].said, "maya asked you to review pull request #21, Fix auth.");

    let runs = branch_runs_of(&json!([
        {"databaseId":101,"status":"completed","conclusion":"failure","workflowName":"CI","url":"https://github.com/acme/repo/actions/runs/101"},
        {"databaseId":100,"status":"completed","conclusion":"success","workflowName":"CI","url":"https://github.com/acme/repo/actions/runs/100"},
        {"databaseId":102,"status":"in_progress","conclusion":"","workflowName":"Lint","url":"https://github.com/acme/repo/actions/runs/102"},
        {"databaseId":103,"status":"completed","conclusion":"timed_out","workflowName":"Nightly","url":"https://github.com/acme/repo/actions/runs/103"}
    ]).to_string());
    assert_eq!(runs.len(), 2);
    let bfacts = branch_facts("main", &runs);
    assert_eq!(bfacts.len(), 2);
    assert_eq!(bfacts[0].key, "branch:101");
    assert_eq!(bfacts[0].said, "CI failed on main.");
    assert_eq!(bfacts[1].said, "Nightly failed on main.");
}

#[test]
fn waiting_cards_only_notify_after_threshold_and_keep_command_in_quote_not_detail() {
    let now = dt("2026-10-12T10:15:00Z");
    let cards = vec![
        WaitingCard {
            approval_id: "ap_new".into(),
            mission_id: "m_1".into(),
            teammate_id: Some("tm_wren".into()),
            name: Some("Wren".into()),
            summary: "TOKEN=secret rm -rf foo".into(),
            requested_at: "2026-10-12T10:10:00Z".into(),
        },
        WaitingCard {
            approval_id: "ap_old".into(),
            mission_id: "m_2".into(),
            teammate_id: Some("tm_wren".into()),
            name: Some("Wren".into()),
            summary: "npm publish".into(),
            requested_at: "2026-10-12T10:00:00Z".into(),
        },
    ];
    let facts = waiting_facts(&cards, now, SCOUT_WAITING_MS);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].key, "waiting:ap_old");
    assert_eq!(facts[0].urgency, ScoutUrgency::Now);
    assert_eq!(facts[0].said, "Wren has waited 15 minutes for your OK.");
    assert_eq!(facts[0].quote.as_deref(), Some("npm publish"));
    assert_eq!(facts[0].detail, None);
}

#[test]
fn failed_turn_facts_lead_with_why_and_include_prompt_in_detail() {
    let facts = failed_facts(&[FailedTurn {
        mission_id: "m_fail".into(),
        teammate_id: Some("tm_ada".into()),
        name: Some("Ada".into()),
        asked: "Check why deploy stalled".into(),
        why: Some("model not found".into()),
    }]);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].said, "Ada's turn failed: model not found");
    assert_eq!(facts[0].detail.as_deref(), Some("It was asked: Check why deploy stalled"));
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
fn scout_prompt_and_answers_and_note_parsing_match_electron() {
    let facts = vec![
        ScoutFact {
            detail: Some("It was asked: fix tests".into()),
            ..ScoutFact::simple("failed:m1", ScoutUrgency::Soon, "Ada's turn failed")
        },
        ScoutFact::simple("check:1:fails", ScoutUrgency::Now, "The check npm test fails"),
    ];
    let prompt = scout_prompt(
        &facts,
        &[PromptTeammate {
            name: "Wren",
            role: "Code",
        }],
        Some("Prioritize CI"),
        &["2026-10-11: Ignore docs PRs".into()],
    );
    assert!(prompt.contains("keep watch for this: Prioritize CI"));
    assert!(prompt.contains("Your notes from earlier looks"));
    assert!(prompt.contains("- Wren, Code"));

    let raw = r#"
**ITEM 1:**
**TAKE**: Ada hit a flaky test in CI.
**HAND TO**: Wren.
DRAFT: Re-run the suite on Ada's branch and fix any flake.

ITEM 99:
TAKE: Ignored out-of-range item.

NOTE: Keep an eye on flaky CI tests.
"#;
    let team = [AnswerTeammate {
        teammate_id: "tm_wren",
        name: "Wren",
    }];
    let answers = scout_answers(raw, facts.len(), &team);
    assert_eq!(answers.len(), 1);
    let a1 = answers.get(&1).unwrap();
    assert_eq!(a1.take.as_deref(), Some("Ada hit a flaky test in CI."));
    let draft = a1.draft.as_ref().unwrap();
    assert_eq!(draft.teammate_id, "tm_wren");
    assert_eq!(draft.message, "Re-run the suite on Ada's branch and fix any flake.");
    assert!(!draft.message.contains("NOTE:"));
    assert_eq!(scout_note_of(raw).as_deref(), Some("Keep an eye on flaky CI tests."));
    assert_eq!(scout_note_of("NOTE: none.").as_deref(), None);
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

#[test]
fn with_new_cards_caps_at_max_scout_cards_keeping_newest_first_and_filters_bad_issues() {
    assert!(issues_of("not json").is_empty());
    let partial_issues = issues_of(
        &json!([
            {"number": "nope", "title": "bad"},
            {"number": 9, "title": "Good", "url": "https://github.com/acme/repo/issues/9", "createdAt": "2026-10-12T00:00:00Z"}
        ])
        .to_string(),
    );
    assert_eq!(partial_issues.len(), 1);
    assert_eq!(partial_issues[0].number, 9);

    let existing: Vec<ScoutCard> = (0..MAX_SCOUT_CARDS)
        .map(|i| ScoutCard {
            card_id: format!("old_{i}"),
            at: "2026-10-12T00:00:00.000Z".into(),
            reading: false,
            take: None,
            raw_fix: None,
            fact: ScoutFact::simple(
                format!("old:{i}"),
                ScoutUrgency::Later,
                format!("old {i}"),
            ),
        })
        .collect();
    let newest = ScoutCard {
        card_id: "new_1".into(),
        at: "2026-10-12T01:00:00.000Z".into(),
        reading: false,
        take: None,
        raw_fix: None,
        fact: ScoutFact::simple("new:1", ScoutUrgency::Now, "brand new"),
    };
    let kept = with_new_cards(&existing, &[newest]);
    assert_eq!(kept.len(), MAX_SCOUT_CARDS);
    assert_eq!(kept[0].card_id, "new_1");
    assert_eq!(kept[MAX_SCOUT_CARDS - 1].card_id, format!("old_{}", MAX_SCOUT_CARDS - 2));
}
