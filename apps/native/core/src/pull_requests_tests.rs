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
    time::Duration,
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

fn link_body(over: serde_json::Map<String, Value>) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("state".into(), json!("OPEN"));
    obj.insert("isDraft".into(), json!(false));
    obj.insert("mergedAt".into(), Value::Null);
    obj.insert("title".into(), json!("Fix the cart"));
    obj.insert("statusCheckRollup".into(), json!([]));
    for (k, v) in over {
        obj.insert(k, v);
    }
    Value::Object(obj).to_string()
}

#[test]
fn github_links_conversation_pieces_and_request_validation() {
    assert_eq!(
        github_link_of("https://github.com/acme/app/pull/42"),
        Some(GitHubConversationLink {
            kind: GitHubLinkKind::Pull,
            address: PullLinkAddress { owner: "acme".into(), repo: "app".into(), number: 42 },
        })
    );
    assert!(github_link_of("https://github.com/acme/app/pull/42/").is_some());
    assert!(github_link_of("https://www.github.com/acme/app/pull/42?w=1#discussion").is_some());
    assert_eq!(
        github_link_of("https://github.com/acme/app/issues/3").map(|l| l.kind),
        Some(GitHubLinkKind::Issue)
    );

    for bad in [
        "https://github.com/acme/app/commit/abcdef1",
        "https://github.com/acme/app/blob/main/README.md",
        "https://github.com/acme/app/pull/42/files",
        "http://github.com/acme/app/pull/42",
        "https://github.com.evil/acme/app/pull/42",
        "https://evil.example/acme/app/pull/42",
        "https://user:token@github.com/acme/app/pull/42",
        "https://github.com/-acme/app/pull/1",
        "https://github.com/acme/app/pull/0",
        "https://github.com/acme/../pull/1",
    ] {
        assert!(github_link_of(bad).is_none(), "expected None for {bad}");
    }

    let pieces = conversation_pieces(
        "Opened https://github.com/acme/app/pull/42 today. See https://github.com/acme/app/issues/3.",
    );
    assert_eq!(
        pieces,
        vec![
            ConversationPiece::Text("Opened ".into()),
            ConversationPiece::Link {
                text: "https://github.com/acme/app/pull/42".into(),
                href: "https://github.com/acme/app/pull/42".into(),
                pull: Some(PullLinkAddress { owner: "acme".into(), repo: "app".into(), number: 42 }),
                issue: None,
            },
            ConversationPiece::Text(" today. See ".into()),
            ConversationPiece::Link {
                text: "https://github.com/acme/app/issues/3".into(),
                href: "https://github.com/acme/app/issues/3".into(),
                pull: None,
                issue: Some(PullLinkAddress { owner: "acme".into(), repo: "app".into(), number: 3 }),
            },
            ConversationPiece::Text(".".into()),
        ]
    );

    assert_eq!(
        conversation_pieces("Run `https://github.com/acme/app/pull/42` first."),
        vec![ConversationPiece::Text("Run `https://github.com/acme/app/pull/42` first.".into())]
    );

    // Request validation
    assert!(pull_link_request(&json!({ "owner": "acme", "repo": "app", "number": 42 })).is_some());
    assert!(pull_link_request(&json!({ "owner": "acme", "repo": "app", "number": 42, "cwd": "C:/work" })).is_none());
    assert!(pull_link_request(&json!({ "owner": "acme", "repo": "app", "number": 42, "args": ["auth", "token"] })).is_none());
    assert!(pull_link_request(&json!({ "owner": "acme", "repo": "app", "number": "42" })).is_none());
    assert!(pull_link_request(&json!({ "owner": "-acme", "repo": "app", "number": 1 })).is_none());
    assert!(pull_link_request(&json!({ "owner": "acme", "repo": "..", "number": 1 })).is_none());
    assert!(pull_link_request(&json!({ "owner": "acme", "repo": "app", "number": 0 })).is_none());
    assert!(pull_link_request(&json!("gh pr view 42 --repo acme/app")).is_none());
    assert!(pull_link_request(&json!(["acme", "app", 42])).is_none());
}

#[test]
fn linked_pull_request_parsing_and_hovers() {
    assert_eq!(
        linked_pull_request_of(&link_body(Default::default())).unwrap().state,
        PullRequestState::Open
    );
    let mut draft = serde_json::Map::new();
    draft.insert("isDraft".into(), json!(true));
    assert_eq!(linked_pull_request_of(&link_body(draft)).unwrap().state, PullRequestState::Draft);

    let mut closed_merged = serde_json::Map::new();
    closed_merged.insert("state".into(), json!("CLOSED"));
    closed_merged.insert("mergedAt".into(), json!("2026-10-01T00:00:00Z"));
    assert_eq!(linked_pull_request_of(&link_body(closed_merged)).unwrap().state, PullRequestState::Merged);

    let mut closed = serde_json::Map::new();
    closed.insert("state".into(), json!("CLOSED"));
    closed.insert("mergedAt".into(), json!("  "));
    assert_eq!(linked_pull_request_of(&link_body(closed)).unwrap().state, PullRequestState::Closed);

    assert!(linked_pull_request_of("no pull request").is_none());
    assert!(linked_pull_request_of("[]").is_none());

    let mut running_map = serde_json::Map::new();
    running_map.insert(
        "statusCheckRollup".into(),
        json!([
            run_check("SUCCESS", "COMPLETED"),
            run_check("SUCCESS", "COMPLETED"),
            run_check("SUCCESS", "COMPLETED"),
            run_check("", "IN_PROGRESS"),
        ]),
    );
    let running = linked_pull_request_of(&link_body(running_map)).unwrap();
    assert_eq!(running.checks, PullLinkChecks::Running);
    assert_eq!(pull_link_hover(&running), "Open · 3 of 4 checks passed · 1 running");

    let mut failing_map = serde_json::Map::new();
    failing_map.insert(
        "statusCheckRollup".into(),
        json!([run_check("FAILURE", "COMPLETED"), run_check("SUCCESS", "COMPLETED")]),
    );
    let failing = linked_pull_request_of(&link_body(failing_map)).unwrap();
    assert_eq!(failing.checks, PullLinkChecks::Failing);
    assert_eq!(pull_link_hover(&failing), "Open · 1 of 2 checks passed · 1 failing");

    let mut one_pass = serde_json::Map::new();
    one_pass.insert("statusCheckRollup".into(), json!([run_check("SUCCESS", "COMPLETED")]));
    assert_eq!(pull_link_hover(&linked_pull_request_of(&link_body(one_pass)).unwrap()), "Open · 1 of 1 check passed");
}

#[test]
fn pull_link_reader_dedupes_caches_bounds_concurrency_and_caches_errors() {
    struct MockGh {
        seen: Mutex<Vec<Vec<String>>>,
        current: AtomicUsize,
        max: AtomicUsize,
        delay_ms: u64,
        fail: bool,
    }
    impl Network for MockGh {
        fn run(&self, _program: &str, args: &[String], _cwd: &Path) -> Result<String, String> {
            lock(&self.seen).push(args.to_vec());
            let c = self.current.fetch_add(1, Ordering::SeqCst) + 1;
            self.max.fetch_max(c, Ordering::SeqCst);
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
            self.current.fetch_sub(1, Ordering::SeqCst);
            if self.fail {
                Err("spawn gh ENOENT".into())
            } else {
                Ok(link_body({
                    let mut m = serde_json::Map::new();
                    m.insert("statusCheckRollup".into(), json!([run_check("SUCCESS", "COMPLETED")]));
                    m
                }))
            }
        }
    }

    // 1. Asks gh for that pull request and nothing else
    let gh1 = Arc::new(MockGh {
        seen: Mutex::new(Vec::new()),
        current: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
        delay_ms: 0,
        fail: false,
    });
    let reader1 = PullLinkReader::new(gh1.clone());
    let req = json!({ "owner": "acme", "repo": "app", "number": 42 });
    assert_eq!(reader1.read(&req).unwrap().state, PullRequestState::Open);
    assert_eq!(
        *lock(&gh1.seen),
        vec![vec!["pr", "view", "42", "--repo", "acme/app", "--json", LINK_PULL_FIELDS]]
    );

    // 2. Reads once when many threads ask together, and keeps the answer for a minute
    let time = Arc::new(AtomicU64::new(10_000));
    let gh2 = Arc::new(MockGh {
        seen: Mutex::new(Vec::new()),
        current: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
        delay_ms: 30,
        fail: false,
    });
    let reader2 = Arc::new(PullLinkReader::with_options(
        gh2.clone(),
        {
            let time = time.clone();
            Arc::new(move || time.load(Ordering::SeqCst))
        },
        PULL_LINK_CACHE_MS,
        PULL_LINK_AT_ONCE,
    ));
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let r = reader2.clone();
            let req = req.clone();
            std::thread::spawn(move || r.read(&req))
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap().unwrap().checks, PullLinkChecks::Passing);
    }
    assert_eq!(lock(&gh2.seen).len(), 1);
    time.fetch_add(59_000, Ordering::SeqCst);
    assert!(reader2.read(&req).is_some());
    assert_eq!(lock(&gh2.seen).len(), 1);
    time.fetch_add(2_000, Ordering::SeqCst);
    assert!(reader2.read(&req).is_some());
    assert_eq!(lock(&gh2.seen).len(), 2);

    // 3. Does not ask 12 times at once (max concurrency = 2)
    let gh3 = Arc::new(MockGh {
        seen: Mutex::new(Vec::new()),
        current: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
        delay_ms: 25,
        fail: false,
    });
    let reader3 = Arc::new(PullLinkReader::new(gh3.clone()));
    let handles3: Vec<_> = (1..=12)
        .map(|n| {
            let r = reader3.clone();
            std::thread::spawn(move || r.read(&json!({ "owner": "acme", "repo": "app", "number": n })))
        })
        .collect();
    for h in handles3 {
        assert!(h.join().unwrap().is_some());
    }
    assert_eq!(gh3.max.load(Ordering::SeqCst), 2);

    // 4. Reads nothing when gh cannot, and does not ask again for a minute
    let gh4 = Arc::new(MockGh {
        seen: Mutex::new(Vec::new()),
        current: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
        delay_ms: 0,
        fail: true,
    });
    let reader4 = PullLinkReader::with_options(gh4.clone(), Arc::new(|| 5_000), PULL_LINK_CACHE_MS, 2);
    assert!(reader4.read(&req).is_none());
    assert!(reader4.read(&req).is_none());
    assert!(reader4.read(&json!({ "owner": "acme", "repo": "app", "number": 42, "token": "secret" })).is_none());
    assert_eq!(lock(&gh4.seen).len(), 1);
}

// --- a-pull-link-is-read-once.test.ts ---

#[test]
fn link_watch_reads_once_refreshes_on_focus_after_minute_and_polls_only_while_running() {
    let addr = PullLinkAddress { owner: "acme".into(), repo: "app".into(), number: 42 };
    let passing = LinkedPullRequest {
        state: PullRequestState::Open,
        checks: PullLinkChecks::Passing,
        passed: 2,
        failed: 0,
        running: 0,
        total: 2,
    };
    let running = LinkedPullRequest {
        state: PullRequestState::Open,
        checks: PullLinkChecks::Running,
        passed: 3,
        failed: 0,
        running: 1,
        total: 4,
    };

    let mut time = 1_000u64;
    let mut w: LinkWatch<LinkedPullRequest> = LinkWatch::default();
    assert_eq!(w.again_ms(), 180_000);

    let (id1, r1) = w.watch(&addr, time, is_pull_link_moving);
    let (id2, r2) = w.watch(&addr, time, is_pull_link_moving);
    let (id3, r3) = w.watch(&addr, time, is_pull_link_moving);
    assert!(r1);
    assert!(r2);
    assert!(r3);
    w.finish_read(&addr, Some(passing.clone()), time, is_pull_link_moving);
    assert!(!w.timer_armed());
    assert!(w.on_focus(time).is_empty());

    time += 60_000;
    assert_eq!(w.on_focus(time), vec![addr.clone()]);
    w.unwatch(&addr, id1, is_pull_link_moving);
    w.unwatch(&addr, id2, is_pull_link_moving);
    w.unwatch(&addr, id3, is_pull_link_moving);
    assert!(!w.focus_armed());

    // Polls only while checks are running, stops when link is gone
    let mut w2: LinkWatch<LinkedPullRequest> = LinkWatch::default();
    let (id, should_read) = w2.watch(&addr, 1_000, is_pull_link_moving);
    assert!(should_read);
    w2.finish_read(&addr, Some(running), 1_000, is_pull_link_moving);
    assert!(w2.timer_armed());
    assert_eq!(w2.on_tick(is_pull_link_moving), vec![addr.clone()]);
    w2.unwatch(&addr, id, is_pull_link_moving);
    assert!(!w2.timer_armed());
    assert!(!w2.focus_armed());
    assert!(w2.on_tick(is_pull_link_moving).is_empty());
}
