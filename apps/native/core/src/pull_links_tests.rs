//! Electron main/a-pull-request-link-wears-its-state.test.ts and main/an-issue-link-wears-its-state.test.ts: a pull
//! request or issue link in a conversation, read with a stand-in gh (never the network).
use super::*;
use serde_json::json;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::Duration;

fn run(conclusion: &str, status: &str) -> Value {
    json!({ "__typename": "CheckRun", "conclusion": conclusion, "status": status, "name": "CI" })
}

fn done(conclusion: &str) -> Value {
    run(conclusion, "COMPLETED")
}

/// gh's answer for a pull request link, with `over` laid on Electron's defaults.
fn body(over: Value) -> String {
    let mut out = json!({ "state": "OPEN", "isDraft": false, "mergedAt": null, "title": "Fix the cart", "statusCheckRollup": [] });
    for (k, v) in over.as_object().unwrap() {
        out[k] = v.clone();
    }
    out.to_string()
}

/// gh's answer for an issue link.
fn issue(over: Value) -> String {
    let mut out = json!({ "state": "OPEN", "stateReason": "", "title": "Fix the cart" });
    for (k, v) in over.as_object().unwrap() {
        out[k] = v.clone();
    }
    out.to_string()
}

fn address() -> Value {
    json!({ "owner": "acme", "repo": "app", "number": 42 })
}

fn at(owner: &str, repo: &str, number: u64) -> PullLinkAddress {
    PullLinkAddress { owner: owner.into(), repo: repo.into(), number }
}

fn link(kind: GitHubLinkKind, number: u64) -> Option<GitHubConversationLink> {
    Some(GitHubConversationLink { kind, address: at("acme", "app", number) })
}

fn text(t: &str) -> ConversationPiece {
    ConversationPiece::Text(t.into())
}

fn piece(t: &str, href: &str, pull: Option<PullLinkAddress>, issue: Option<PullLinkAddress>) -> ConversationPiece {
    ConversationPiece::Link { text: t.into(), href: href.into(), pull, issue }
}

fn linked(state: PullRequestState, checks: PullLinkChecks, passed: usize, failed: usize, running: usize, total: usize) -> LinkedPullRequest {
    LinkedPullRequest { state, checks, passed, failed, running, total }
}

/// A stand-in gh: says `answer` for each call (or fails), and can hold every call until released.
struct Gh {
    seen: Mutex<Vec<Vec<String>>>,
    answer: Box<dyn Fn(&[String]) -> Result<String, String> + Send + Sync>,
    held: Mutex<bool>,
    release: Condvar,
    current: AtomicUsize,
    max: AtomicUsize,
    delay: Duration,
}

impl Gh {
    fn new(answer: impl Fn(&[String]) -> Result<String, String> + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            seen: Mutex::new(Vec::new()),
            answer: Box::new(answer),
            held: Mutex::new(false),
            release: Condvar::new(),
            current: AtomicUsize::new(0),
            max: AtomicUsize::new(0),
            delay: Duration::ZERO,
        })
    }

    fn calls(&self) -> usize {
        lock(&self.seen).len()
    }

    fn seen(&self) -> Vec<Vec<String>> {
        lock(&self.seen).clone()
    }
}

impl Network for Gh {
    fn run(&self, _program: &str, args: &[String], _cwd: &Path) -> Result<String, String> {
        lock(&self.seen).push(args.to_vec());
        let now = self.current.fetch_add(1, Ordering::SeqCst) + 1;
        self.max.fetch_max(now, Ordering::SeqCst);
        let mut held = lock(&self.held);
        while *held {
            held = self.release.wait(held).unwrap_or_else(|e| e.into_inner());
        }
        drop(held);
        std::thread::sleep(self.delay);
        self.current.fetch_sub(1, Ordering::SeqCst);
        (self.answer)(args)
    }
}

fn clock(start: u64) -> (Arc<AtomicU64>, Arc<dyn Fn() -> u64 + Send + Sync>) {
    let time = Arc::new(AtomicU64::new(start));
    let read = time.clone();
    (time, Arc::new(move || read.load(Ordering::SeqCst)))
}

/// Three readers asking together while gh holds its answer; gh is asked once.
fn three_together<T: Send + 'static>(gh: &Arc<Gh>, ask: impl Fn() -> T + Send + Sync + 'static) -> Vec<T> {
    *lock(&gh.held) = true;
    let ask = Arc::new(ask);
    let all: Vec<_> = (0..3)
        .map(|_| {
            let ask = ask.clone();
            std::thread::spawn(move || ask())
        })
        .collect();
    // Every reader has had time to ask before gh answers.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(gh.calls(), 1);
    *lock(&gh.held) = false;
    gh.release.notify_all();
    all.into_iter().map(|h| h.join().unwrap()).collect()
}

// ---- main/a-pull-request-link-wears-its-state.test.ts --------------------------------------------------------------

// case: is a pull request, and an issue is named beside it
#[test]
fn is_a_pull_request_and_an_issue_is_named_beside_it() {
    assert_eq!(github_link_of("https://github.com/acme/app/pull/42"), link(GitHubLinkKind::Pull, 42));
    assert_eq!(github_link_of("https://github.com/acme/app/pull/42/"), link(GitHubLinkKind::Pull, 42));
    assert_eq!(github_link_of("https://www.github.com/acme/app/pull/42?w=1#discussion"), link(GitHubLinkKind::Pull, 42));
    assert_eq!(github_link_of("https://github.com/acme/app/issues/3"), link(GitHubLinkKind::Issue, 3));
}

// case: leaves a commit, a file, and anything that is not that address
#[test]
fn leaves_a_commit_a_file_and_anything_that_is_not_that_address() {
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
        assert_eq!(github_link_of(bad), None, "{bad}");
    }
}

// case: finds a bare pull request in a sentence, and the issue beside it as a link of its own kind
#[test]
fn finds_a_bare_pull_request_in_a_sentence_and_the_issue_beside_it_as_a_link_of_its_own_kind() {
    assert_eq!(
        conversation_pieces("Opened https://github.com/acme/app/pull/42 today. See https://github.com/acme/app/issues/3."),
        vec![
            text("Opened "),
            piece("https://github.com/acme/app/pull/42", "https://github.com/acme/app/pull/42", Some(at("acme", "app", 42)), None),
            text(" today. See "),
            piece("https://github.com/acme/app/issues/3", "https://github.com/acme/app/issues/3", None, Some(at("acme", "app", 3))),
            text("."),
        ]
    );
    assert_eq!(
        conversation_pieces("See [the issue](https://github.com/acme/app/issues/3) and [the file](https://github.com/acme/app/blob/main/README.md)."),
        vec![
            text("See "),
            piece("the issue", "https://github.com/acme/app/issues/3", None, Some(at("acme", "app", 3))),
            text(" and "),
            piece("the file", "https://github.com/acme/app/blob/main/README.md", None, None),
            text("."),
        ]
    );
    assert_eq!(
        conversation_pieces("Run `https://github.com/acme/app/pull/42` first."),
        vec![text("Run `https://github.com/acme/app/pull/42` first.")]
    );
}

// case: refuses anything that is not an owner, a repo and a number
#[test]
fn refuses_anything_that_is_not_an_owner_a_repo_and_a_number() {
    assert_eq!(pull_link_request(&address()), Some(at("acme", "app", 42)));
    for bad in [
        json!({ "owner": "acme", "repo": "app", "number": 42, "cwd": "C:/work" }),
        json!({ "owner": "acme", "repo": "app", "number": 42, "args": ["auth", "token"] }),
        json!({ "owner": "acme", "repo": "app", "number": "42" }),
        json!({ "owner": "-acme", "repo": "app", "number": 1 }),
        json!({ "owner": "acme", "repo": "..", "number": 1 }),
        json!({ "owner": "acme", "repo": "app", "number": 0 }),
        json!("gh pr view 42 --repo acme/app"),
        json!(["acme", "app", 42]),
    ] {
        assert_eq!(pull_link_request(&bad), None, "{bad}");
    }
    // The issue's request, the same shape (main/an-issue-link-wears-its-state.test.ts).
    assert_eq!(issue_link_request(&address()), Some(at("acme", "app", 42)));
    for bad in [
        json!({ "owner": "acme", "repo": "app", "number": 42, "cwd": "C:/work" }),
        json!({ "owner": "acme", "repo": "app", "number": 42, "args": ["auth", "token"] }),
        json!({ "owner": "acme", "repo": "app", "number": "42" }),
        json!({ "owner": "-acme", "repo": "app", "number": 1 }),
        json!({ "owner": "acme", "repo": "app", "number": 0 }),
    ] {
        assert_eq!(issue_link_request(&bad), None, "{bad}");
    }
}

// case: reads open, draft, merged and closed
#[test]
fn reads_open_draft_merged_and_closed() {
    use PullLinkChecks as C;
    use PullRequestState as S;
    let of = |over: Value| linked_pull_request_of(&body(over)).unwrap();
    assert_eq!(of(json!({})), linked(S::Open, C::None, 0, 0, 0, 0));
    assert_eq!((of(json!({ "isDraft": true })).state, of(json!({ "isDraft": true })).checks), (S::Draft, C::None));
    let merged = of(json!({ "state": "MERGED", "mergedAt": "2026-10-18T12:00:00Z", "statusCheckRollup": [done("SUCCESS"), done("SUCCESS")] }));
    assert_eq!((merged.state, merged.checks, merged.total), (S::Merged, C::None, 2));
    let closed_merged = of(json!({ "state": "CLOSED", "mergedAt": "2026-10-01T00:00:00Z" }));
    assert_eq!((closed_merged.state, closed_merged.checks), (S::Merged, C::None));
    let closed = of(json!({ "state": "CLOSED", "mergedAt": null, "statusCheckRollup": [done("FAILURE")] }));
    assert_eq!((closed.state, closed.checks, closed.failed), (S::Closed, C::None, 1));
    assert_eq!(of(json!({ "state": "CLOSED", "mergedAt": "  " })).state, S::Closed);
    assert_eq!(linked_pull_request_of("no pull request"), None);
    assert_eq!(linked_pull_request_of("[]"), None);
}

// case: reads checks as passing, failing, running or none, and says the hover
#[test]
fn reads_checks_as_passing_failing_running_or_none_and_says_the_hover() {
    use PullLinkChecks as C;
    use PullRequestState as S;
    let of = |over: Value| linked_pull_request_of(&body(over)).unwrap();
    let running = of(json!({ "statusCheckRollup": [done("SUCCESS"), done("SUCCESS"), done("SUCCESS"), run("", "IN_PROGRESS")] }));
    assert_eq!(running, linked(S::Open, C::Running, 3, 0, 1, 4));
    assert_eq!(pull_link_hover(&running), "Open · 3 of 4 checks passed · 1 running");
    let failing = of(json!({ "statusCheckRollup": [done("FAILURE"), done("SUCCESS")] }));
    assert_eq!(failing, linked(S::Open, C::Failing, 1, 1, 0, 2));
    assert_eq!(pull_link_hover(&failing), "Open · 1 of 2 checks passed · 1 failing");
    let both = of(json!({ "statusCheckRollup": [done("FAILURE"), run("", "QUEUED")] }));
    assert_eq!(both.checks, C::Failing);
    assert_eq!(pull_link_hover(&both), "Open · 0 of 2 checks passed · 1 failing · 1 running");
    let passing = of(json!({ "statusCheckRollup": [done("SUCCESS")] }));
    assert_eq!(passing.checks, C::Passing);
    assert_eq!(pull_link_hover(&passing), "Open · 1 of 1 check passed");
    assert_eq!(pull_link_hover(&of(json!({}))), "Open");
    assert_eq!(pull_link_hover(&of(json!({ "state": "MERGED" }))), "Merged");
    assert_eq!(pull_link_hover(&of(json!({ "isDraft": true, "statusCheckRollup": [done("FAILURE")] }))), "Draft · 0 of 1 check passed · 1 failing");
}

// case: asks gh for that pull request and nothing else
#[test]
fn asks_gh_for_that_pull_request_and_nothing_else() {
    let gh = Gh::new(|_| Ok(body(json!({}))));
    let reader = PullLinkReader::new(gh.clone());
    let got = reader.read(&address()).unwrap();
    assert_eq!((got.state, got.checks), (PullRequestState::Open, PullLinkChecks::None));
    assert_eq!(gh.seen(), vec![vec!["pr", "view", "42", "--repo", "acme/app", "--json", LINK_PULL_FIELDS]]);
}

// case: reads once when many links ask together, and keeps the answer for a minute
#[test]
fn reads_once_when_many_links_ask_together_and_keeps_the_answer_for_a_minute() {
    let gh = Gh::new(|_| Ok(body(json!({ "statusCheckRollup": [done("SUCCESS")] }))));
    let (time, now) = clock(10_000);
    let reader = Arc::new(PullLinkReader::with_options(gh.clone(), now, PULL_LINK_CACHE_MS, PULL_LINK_AT_ONCE));
    let answers = three_together(&gh, {
        let reader = reader.clone();
        move || reader.read(&address())
    });
    assert_eq!(answers[0].as_ref().map(|a| a.checks), Some(PullLinkChecks::Passing));
    assert_eq!(answers[1], answers[0]);
    assert_eq!(reader.read(&address()), answers[0]);
    assert_eq!(gh.calls(), 1);
    time.fetch_add(59_000, Ordering::SeqCst);
    assert_eq!(reader.read(&address()).map(|a| a.checks), Some(PullLinkChecks::Passing));
    assert_eq!(gh.calls(), 1);
    time.fetch_add(2_000, Ordering::SeqCst);
    assert_eq!(reader.read(&address()).map(|a| a.checks), Some(PullLinkChecks::Passing));
    assert_eq!(gh.calls(), 2);
}

// case: does not ask fifty times at once
#[test]
fn does_not_ask_fifty_times_at_once() {
    let mut gh = Gh::new(|_| Ok(body(json!({}))));
    Arc::get_mut(&mut gh).unwrap().delay = Duration::from_millis(40);
    let reader = Arc::new(PullLinkReader::new(gh.clone()));
    let all: Vec<_> = (1..=12)
        .map(|n| {
            let reader = reader.clone();
            std::thread::spawn(move || reader.read(&json!({ "owner": "acme", "repo": "app", "number": n })))
        })
        .collect();
    for one in all {
        assert!(one.join().unwrap().is_some());
    }
    assert_eq!(gh.max.load(Ordering::SeqCst), 2);
}

// case: reads nothing into the chip when gh cannot, and does not ask again for a minute
#[test]
fn reads_nothing_into_the_chip_when_gh_cannot_and_does_not_ask_again_for_a_minute() {
    let gh = Gh::new(|_| Err("spawn gh ENOENT".into()));
    let reader = PullLinkReader::with_options(gh.clone(), Arc::new(|| 5_000), PULL_LINK_CACHE_MS, PULL_LINK_AT_ONCE);
    assert_eq!(reader.read(&address()), None);
    assert_eq!(reader.read(&address()), None);
    assert_eq!(reader.read(&json!({ "owner": "acme", "repo": "app", "number": 42, "token": "secret" })), None);
    assert_eq!(gh.calls(), 1);
    // An issue the same (main/an-issue-link-wears-its-state.test.ts).
    let gh = Gh::new(|_| Err("spawn gh ENOENT".into()));
    let reader = PullLinkReader::with_options(gh.clone(), Arc::new(|| 5_000), PULL_LINK_CACHE_MS, PULL_LINK_AT_ONCE);
    assert_eq!(reader.read_issue(&address()), None);
    assert_eq!(reader.read_issue(&address()), None);
    assert_eq!(reader.read_issue(&json!({ "owner": "acme", "repo": "app", "number": 42, "token": "secret" })), None);
    assert_eq!(gh.calls(), 1);
}

// ---- main/an-issue-link-wears-its-state.test.ts --------------------------------------------------------------------

// case: is an issue, beside the pull request with the same address shape
#[test]
fn is_an_issue_beside_the_pull_request_with_the_same_address_shape() {
    assert_eq!(github_link_of("https://github.com/acme/app/issues/3"), link(GitHubLinkKind::Issue, 3));
    assert_eq!(github_link_of("https://github.com/acme/app/pull/42"), link(GitHubLinkKind::Pull, 42));
}

// case: finds a bare issue in a sentence, and a written one, beside the pull request
#[test]
fn finds_a_bare_issue_in_a_sentence_and_a_written_one_beside_the_pull_request() {
    assert_eq!(
        conversation_pieces("See https://github.com/acme/app/issues/3 today. Opened https://github.com/acme/app/pull/42."),
        vec![
            text("See "),
            piece("https://github.com/acme/app/issues/3", "https://github.com/acme/app/issues/3", None, Some(at("acme", "app", 3))),
            text(" today. Opened "),
            piece("https://github.com/acme/app/pull/42", "https://github.com/acme/app/pull/42", Some(at("acme", "app", 42)), None),
            text("."),
        ]
    );
    assert_eq!(
        conversation_pieces("See [the issue](https://github.com/acme/app/issues/3)."),
        vec![text("See "), piece("the issue", "https://github.com/acme/app/issues/3", None, Some(at("acme", "app", 3))), text(".")]
    );
    assert_eq!(
        conversation_pieces("Run `https://github.com/acme/app/issues/3` first."),
        vec![text("Run `https://github.com/acme/app/issues/3` first.")]
    );
}

// case: reads open, closed completed and closed not planned, and says the words and the hover
#[test]
fn reads_open_closed_completed_and_closed_not_planned_and_says_the_words_and_the_hover() {
    let of = |over: Value| linked_issue_of(&issue(over));
    let titled = |state| Some(LinkedIssue { state, title: "Fix the cart".into() });
    let open = of(json!({})).unwrap();
    assert_eq!(Some(open.clone()), titled(IssueLinkState::Open));
    assert_eq!(issue_link_word(open.state), "Open");
    assert_eq!(issue_link_hover(&open), "Open · Fix the cart");
    let completed = of(json!({ "state": "CLOSED", "stateReason": "COMPLETED" })).unwrap();
    assert_eq!(Some(completed.clone()), titled(IssueLinkState::ClosedCompleted));
    assert_eq!(issue_link_word(completed.state), "Closed");
    assert_eq!(issue_link_hover(&completed), "Closed as completed · Fix the cart");
    let not_planned = of(json!({ "state": "CLOSED", "stateReason": "NOT_PLANNED" })).unwrap();
    assert_eq!(Some(not_planned.clone()), titled(IssueLinkState::ClosedNotPlanned));
    assert_eq!(issue_link_word(not_planned.state), "Not planned");
    assert_eq!(issue_link_hover(&not_planned), "Closed as not planned · Fix the cart");
    assert_eq!(of(json!({ "state": "CLOSED", "stateReason": "" })).map(|i| i.state), Some(IssueLinkState::ClosedCompleted));
    assert_eq!(issue_link_hover(&of(json!({ "title": "  " })).unwrap()), "Open");
    assert_eq!(linked_issue_of("no issue"), None);
    assert_eq!(linked_issue_of("[]"), None);
    assert_eq!(of(json!({ "state": "MERGED" })), None);
}

// case: asks gh for that issue and nothing else
#[test]
fn asks_gh_for_that_issue_and_nothing_else() {
    let gh = Gh::new(|_| Ok(issue(json!({}))));
    let reader = PullLinkReader::new(gh.clone());
    assert_eq!(reader.read_issue(&address()), Some(LinkedIssue { state: IssueLinkState::Open, title: "Fix the cart".into() }));
    assert_eq!(gh.seen(), vec![vec!["issue", "view", "42", "--repo", "acme/app", "--json", LINK_ISSUE_FIELDS]]);
}

// case: keeps a pull request and an issue with one number apart
#[test]
fn keeps_a_pull_request_and_an_issue_with_one_number_apart() {
    let gh = Gh::new(|args| {
        Ok(if args[0] == "pr" {
            json!({ "state": "OPEN", "stateReason": "", "title": "The cart forgets", "statusCheckRollup": [], "isDraft": false, "mergedAt": null }).to_string()
        } else {
            issue(json!({ "state": "CLOSED", "stateReason": "NOT_PLANNED", "title": "The cart forgets" }))
        })
    });
    let reader = PullLinkReader::new(gh.clone());
    let forgets = Some(LinkedIssue { state: IssueLinkState::ClosedNotPlanned, title: "The cart forgets".into() });
    assert_eq!(reader.read(&address()).map(|p| p.state), Some(PullRequestState::Open));
    assert_eq!(reader.read_issue(&address()), forgets);
    // Each is kept on its own: asking again asks gh no more.
    assert_eq!(reader.read(&address()).map(|p| p.state), Some(PullRequestState::Open));
    assert_eq!(reader.read_issue(&address()), forgets);
    assert_eq!(
        gh.seen(),
        vec![
            vec!["pr", "view", "42", "--repo", "acme/app", "--json", "state,isDraft,mergedAt,statusCheckRollup,title"],
            vec!["issue", "view", "42", "--repo", "acme/app", "--json", LINK_ISSUE_FIELDS],
        ]
    );
}

// case: reads once when many links ask together, and keeps the answer for a minute
#[test]
fn an_issue_reads_once_when_many_links_ask_together_and_keeps_the_answer_for_a_minute() {
    let gh = Gh::new(|_| Ok(issue(json!({ "state": "CLOSED", "stateReason": "COMPLETED" }))));
    let (time, now) = clock(10_000);
    let reader = Arc::new(PullLinkReader::with_options(gh.clone(), now, PULL_LINK_CACHE_MS, PULL_LINK_AT_ONCE));
    let answers = three_together(&gh, {
        let reader = reader.clone();
        move || reader.read_issue(&address())
    });
    let completed = Some(LinkedIssue { state: IssueLinkState::ClosedCompleted, title: "Fix the cart".into() });
    assert_eq!(answers[0], completed);
    assert_eq!(answers[1], answers[0]);
    assert_eq!(reader.read_issue(&address()), answers[0]);
    assert_eq!(gh.calls(), 1);
    time.fetch_add(61_000, Ordering::SeqCst);
    assert_eq!(reader.read_issue(&address()), completed);
    assert_eq!(gh.calls(), 2);
}
