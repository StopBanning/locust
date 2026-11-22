use super::*;
use crate::folder_commit::FolderCommits;
use serde_json::json;
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn repository() -> TempDir {
    let dir = std::env::temp_dir().join(format!(
        "locust-pr-ws-{}-{}",
        std::process::id(),
        TEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.email", "you@example.test"]);
    git(&dir, &["config", "user.name", "You"]);
    std::fs::write(dir.join("cart.py"), "def total(items):\n    return 0\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["remote", "add", "origin", "https://github.com/someone/pebble.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&dir, &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
    TempDir(dir)
}

fn run_check(conclusion: &str, status: &str) -> Value {
    json!({ "__typename": "CheckRun", "conclusion": conclusion, "status": status, "name": "CI" })
}

fn merged_json() -> String {
    json!({
        "isDraft": false,
        "number": 1,
        "reviewDecision": "",
        "state": "MERGED",
        "statusCheckRollup": [
            { "__typename": "CheckRun", "conclusion": "SUCCESS", "name": "Lint, Typecheck, Test", "status": "COMPLETED", "workflowName": "CI" },
            { "__typename": "StatusContext", "context": "Review bot", "state": "SUCCESS", "targetUrl": "" }
        ],
        "title": "oxc stack",
        "url": "https://github.com/someone/pebble/pull/1"
    })
    .to_string()
}

fn open_json(checks: Vec<Value>, extra: serde_json::Map<String, Value>) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("isDraft".into(), json!(false));
    obj.insert("number".into(), json!(17));
    obj.insert("reviewDecision".into(), json!(""));
    obj.insert("state".into(), json!("OPEN"));
    obj.insert("statusCheckRollup".into(), Value::Array(checks));
    obj.insert("title".into(), json!("Fix the cart total"));
    obj.insert("url".into(), json!("https://github.com/someone/pebble/pull/17"));
    for (k, v) in extra {
        obj.insert(k, v);
    }
    Value::Object(obj).to_string()
}

struct FakeHeaderGh {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    signed_in: bool,
}

impl Network for FakeHeaderGh {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> Result<String, String> {
        let mut call = vec![program.to_owned()];
        call.extend(args.iter().cloned());
        lock(&self.calls).push(call);
        if program != "gh" {
            return Err("unexpected program".into());
        }
        if args.first().map(String::as_str) == Some("auth") {
            return if self.signed_in {
                Ok("Logged in".into())
            } else {
                Err("not logged in".into())
            };
        }
        if args.first().map(String::as_str) == Some("pr")
            && args.get(1).map(String::as_str) == Some("view")
        {
            let branch = git(cwd, &["symbolic-ref", "--short", "HEAD"]).trim().to_owned();
            if branch == "locust/fix-the-cart-total" {
                return Ok(open_json(vec![run_check("SUCCESS", "COMPLETED")], Default::default()));
            }
            return Err(format!("no pull requests found for branch \"{branch}\""));
        }
        Err("unexpected gh call".into())
    }
}

// --- a-pull-request-shows-in-the-header.test.ts ---

// case: reads its state and how its checks stand
// case: takes only a GitHub pull request address
// case: says it in a line
#[test]
fn a_pull_request_read_from_gh_reads_state_checks_and_line() {
    let m = pull_request_of(&merged_json()).unwrap();
    assert_eq!(
        m,
        FolderPullRequest {
            number: 1,
            title: "oxc stack".into(),
            url: "https://github.com/someone/pebble/pull/1".into(),
            state: PullRequestState::Merged,
            checks: PullRequestChecks::Passing,
            check_count: 2,
            not_passed: 0,
            review: None,
            conflicts: false,
            updated_at: None,
        }
    );

    let failing = pull_request_of(&open_json(
        vec![
            run_check("SUCCESS", "COMPLETED"),
            run_check("SKIPPED", "COMPLETED"),
            run_check("CANCELLED", "COMPLETED"),
        ],
        Default::default(),
    ))
    .unwrap();
    assert_eq!(failing.state, PullRequestState::Open);
    assert_eq!(failing.checks, PullRequestChecks::Failing);
    assert_eq!(failing.check_count, 3);
    assert_eq!(failing.not_passed, 1);

    let pending = pull_request_of(&open_json(
        vec![run_check("SUCCESS", "COMPLETED"), run_check("", "IN_PROGRESS")],
        Default::default(),
    ))
    .unwrap();
    assert_eq!(pending.checks, PullRequestChecks::Pending);
    assert_eq!(pending.not_passed, 1);

    let mut extra = serde_json::Map::new();
    extra.insert("isDraft".into(), json!(true));
    extra.insert("reviewDecision".into(), json!("APPROVED"));
    let draft = pull_request_of(&open_json(vec![], extra)).unwrap();
    assert_eq!(draft.state, PullRequestState::Draft);
    assert_eq!(draft.checks, PullRequestChecks::None);
    assert_eq!(draft.review, Some(PullRequestReview::Approved));

    // Takes only a GitHub pull request address
    let mut bad_url = serde_json::Map::new();
    bad_url.insert("url".into(), json!("https://evil.example/someone/pebble/pull/17"));
    assert!(pull_request_of(&open_json(vec![], bad_url)).is_none());
    assert!(pull_request_of("no pull requests found for branch \"main\"").is_none());

    // Says it in a line
    assert_eq!(pull_request_line(&m), "Merged");
    assert_eq!(
        pull_request_line(
            &pull_request_of(&open_json(
                vec![run_check("SUCCESS", "COMPLETED"), run_check("SUCCESS", "COMPLETED")],
                Default::default(),
            ))
            .unwrap()
        ),
        "Open · all 2 checks passed"
    );
    assert_eq!(
        pull_request_line(
            &pull_request_of(&open_json(
                vec![
                    run_check("SUCCESS", "COMPLETED"),
                    run_check("FAILURE", "COMPLETED"),
                    run_check("", "QUEUED"),
                ],
                Default::default(),
            ))
            .unwrap()
        ),
        "Open · 2 of 3 checks not passed"
    );
    let mut cr = serde_json::Map::new();
    cr.insert("reviewDecision".into(), json!("CHANGES_REQUESTED"));
    assert_eq!(
        pull_request_line(
            &pull_request_of(&open_json(vec![run_check("", "IN_PROGRESS")], cr)).unwrap()
        ),
        "Open · checks running · changes requested"
    );
}

// case: is the one for the branch the folder is on, asked of gh from that branch (0.731: a fork’s too)
// case: is nothing on the default branch, on a branch with none, or when gh is signed out -- and gh is not asked on main
// case: is kept a minute, asked again for another branch, and asked again after a sign-in
#[test]
fn folder_pull_request_reads_for_branch_skips_main_and_caches_for_a_minute() {
    let repo = repository();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let commits = FolderCommits::new(Box::new(FakeHeaderGh {
        calls: calls.clone(),
        signed_in: true,
    }));
    // Nothing on the default branch, and gh pr is not asked on main.
    assert!(commits.pull_request(&repo.0).is_none());
    assert!(!lock(&calls).iter().any(|c| c.get(1).map(String::as_str) == Some("pr")));

    git(&repo.0, &["checkout", "-q", "-b", "other-work"]);
    assert!(commits.pull_request(&repo.0).is_none());

    git(&repo.0, &["checkout", "-q", "-b", "locust/fix-the-cart-total"]);
    let pr = commits.pull_request(&repo.0).unwrap();
    assert_eq!(pr.number, 17);
    assert_eq!(pr.state, PullRequestState::Open);
    assert_eq!(pr.checks, PullRequestChecks::Passing);

    let pr_calls: Vec<_> = lock(&calls)
        .iter()
        .filter(|c| c.get(1).map(String::as_str) == Some("pr"))
        .cloned()
        .collect();
    assert_eq!(
        pr_calls.last().unwrap(),
        &vec!["gh", "pr", "view", "--json", PULL_REQUEST_FIELDS]
    );

    // Cached for a minute
    let count_before = pr_calls.len();
    assert!(commits.pull_request(&repo.0).is_some());
    let count_after = lock(&calls)
        .iter()
        .filter(|c| c.get(1).map(String::as_str) == Some("pr"))
        .count();
    assert_eq!(count_after, count_before);

    // Asked again after forget_sign_in
    commits.forget_sign_in();
    assert!(commits.pull_request(&repo.0).is_some());
    let count_after_forget = lock(&calls)
        .iter()
        .filter(|c| c.get(1).map(String::as_str) == Some("pr"))
        .count();
    assert_eq!(count_after_forget, count_before + 1);

    // Asked again for another branch
    git(&repo.0, &["checkout", "-q", "-b", "another-work"]);
    assert!(commits.pull_request(&repo.0).is_none());
    let count_after_branch = lock(&calls)
        .iter()
        .filter(|c| c.get(1).map(String::as_str) == Some("pr"))
        .count();
    assert_eq!(count_after_branch, count_before + 2);

    // Signed out returns None
    git(&repo.0, &["checkout", "-q", "locust/fix-the-cart-total"]);
    let signed_out = FolderCommits::new(Box::new(FakeHeaderGh {
        calls: Arc::new(Mutex::new(Vec::new())),
        signed_in: false,
    }));
    assert!(signed_out.pull_request(&repo.0).is_none());
}

// --- a-pull-request-can-be-watched.test.ts ---

const WATCH_URL: &str = "https://github.com/acme/shop/pull/12";

fn sample_pr() -> FolderPullRequest {
    FolderPullRequest {
        number: 12,
        title: "Fix totals".into(),
        url: WATCH_URL.into(),
        state: PullRequestState::Open,
        checks: PullRequestChecks::Pending,
        check_count: 3,
        not_passed: 3,
        review: None,
        conflicts: false,
        updated_at: None,
    }
}

// case: a check that fails, a review that asks for changes, a conflict: each something to fix
// case: checks passing, an approval, a merge: news, nothing to fix
// case: nothing changed, or a check still failing, is not news again
// case: reads a conflict off gh’s own field
// case: looks at once, then on each tick, and says only what changed
// case: stops watching a merged one once it has said so, and the timer with it
// case: has no timer while nothing is watched
#[test]
fn pull_request_news_and_watch_behave_as_electron_pins() {
    assert_eq!(PULL_REQUEST_WATCH_MS, 180_000);

    let mut failing = sample_pr();
    failing.checks = PullRequestChecks::Failing;
    failing.not_passed = 1;
    assert_eq!(
        pull_request_news(&sample_pr(), &failing),
        Some(PullRequestNews {
            number: 12,
            url: WATCH_URL.into(),
            said: "1 of 3 checks did not pass".into(),
            needs_work: true,
        })
    );

    let mut passing = sample_pr();
    passing.checks = PullRequestChecks::Passing;
    passing.not_passed = 0;
    let mut changes_req = passing.clone();
    changes_req.review = Some(PullRequestReview::ChangesRequested);
    assert_eq!(
        pull_request_news(&passing, &changes_req),
        Some(PullRequestNews {
            number: 12,
            url: WATCH_URL.into(),
            said: "Changes were requested".into(),
            needs_work: true,
        })
    );

    let mut conflicting = sample_pr();
    conflicting.conflicts = true;
    assert_eq!(
        pull_request_news(&sample_pr(), &conflicting),
        Some(PullRequestNews {
            number: 12,
            url: WATCH_URL.into(),
            said: "It conflicts with its base branch".into(),
            needs_work: true,
        })
    );

    assert_eq!(
        pull_request_news(&sample_pr(), &passing),
        Some(PullRequestNews {
            number: 12,
            url: WATCH_URL.into(),
            said: "All 3 checks passed".into(),
            needs_work: false,
        })
    );

    let mut approved = passing.clone();
    approved.review = Some(PullRequestReview::Approved);
    assert_eq!(
        pull_request_news(&passing, &approved),
        Some(PullRequestNews {
            number: 12,
            url: WATCH_URL.into(),
            said: "It was approved".into(),
            needs_work: false,
        })
    );

    let mut merged = sample_pr();
    merged.state = PullRequestState::Merged;
    assert_eq!(
        pull_request_news(&sample_pr(), &merged),
        Some(PullRequestNews {
            number: 12,
            url: WATCH_URL.into(),
            said: "It was merged".into(),
            needs_work: false,
        })
    );

    assert!(pull_request_news(&sample_pr(), &sample_pr()).is_none());
    let mut failing2 = failing.clone();
    failing2.not_passed = 2;
    assert!(pull_request_news(&failing, &failing2).is_none());

    // Conflict from gh JSON
    let c1 = pull_request_of(
        &json!({ "number": 12, "url": WATCH_URL, "state": "OPEN", "mergeable": "CONFLICTING", "statusCheckRollup": [] }).to_string(),
    )
    .unwrap();
    assert!(c1.conflicts);
    let c2 = pull_request_of(
        &json!({ "number": 12, "url": WATCH_URL, "state": "OPEN", "mergeable": "MERGEABLE", "statusCheckRollup": [] }).to_string(),
    )
    .unwrap();
    assert!(!c2.conflicts);

    // Watch: looks at once, then on each tick, says only what changed
    let reads = Arc::new(Mutex::new(vec![sample_pr(), sample_pr(), failing.clone()]));
    let said = Arc::new(Mutex::new(Vec::<String>::new()));
    let timer_events = Arc::new(Mutex::new(Vec::<bool>::new()));
    let watch = PullRequestWatch::with_options(
        {
            let reads = reads.clone();
            move || {
                let mut r = lock(&reads);
                if r.is_empty() { None } else { Some(r.remove(0)) }
            }
        },
        {
            let said = said.clone();
            move |news| lock(&said).push(news.said)
        },
        PULL_REQUEST_WATCH_MS,
        Some(Box::new({
            let timer_events = timer_events.clone();
            move |armed, ms| {
                assert_eq!(ms, 180_000);
                lock(&timer_events).push(armed);
            }
        })),
    );
    watch.watch(WATCH_URL, true);
    assert!(watch.timer_armed());
    watch.look();
    assert!(lock(&said).is_empty());
    watch.look();
    assert_eq!(*lock(&said), vec!["1 of 3 checks did not pass"]);

    // Stops watching a merged one once it has said so, and the timer with it
    let reads2 = Arc::new(Mutex::new(vec![sample_pr(), merged]));
    let said2 = Arc::new(Mutex::new(Vec::<String>::new()));
    let watch2 = PullRequestWatch::new(
        {
            let reads2 = reads2.clone();
            move || {
                let mut r = lock(&reads2);
                if r.is_empty() { None } else { Some(r.remove(0)) }
            }
        },
        {
            let said2 = said2.clone();
            move |news| lock(&said2).push(news.said)
        },
    );
    watch2.watch(WATCH_URL, true);
    watch2.look();
    assert_eq!(*lock(&said2), vec!["It was merged"]);
    assert!(!watch2.watching(WATCH_URL));
    assert!(!watch2.timer_armed());

    // Has no timer while nothing is watched
    let cleared = Arc::new(AtomicUsize::new(0));
    let watch3 = PullRequestWatch::with_options(
        || None,
        |_| {},
        PULL_REQUEST_WATCH_MS,
        Some(Box::new({
            let cleared = cleared.clone();
            move |armed, _| {
                if !armed {
                    cleared.fetch_add(1, Ordering::SeqCst);
                }
            }
        })),
    );
    watch3.watch(WATCH_URL, true);
    watch3.watch(WATCH_URL, false);
    assert!(!watch3.timer_armed());
    assert_eq!(cleared.load(Ordering::SeqCst), 1);
}

// --- a-pull-request-link-wears-its-state.test.ts ---

// --- a-pull-link-is-read-once.test.ts ---

// Carried case by case in pull_link_read_once_tests.rs.
