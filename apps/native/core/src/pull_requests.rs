//! Pull requests in the thread header, on links in replies, and watched for news.
//!
//! Port of Electron's `shared/pull-request.ts`, `main/pull-link.ts`, `main/pull-request-watch.ts`,
//! and `renderer/src/pullLinkWatch.ts`.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::DateTime;
use regex::Regex;
use serde_json::Value;

use crate::{folder_commit::Network, invariants::lock};

/// The fields asked of `gh pr view --json`, in its own names.
pub const PULL_REQUEST_FIELDS: &str = "number,title,url,state,isDraft,statusCheckRollup,reviewDecision,mergeable";

/// What `gh pr view <n> --repo <owner>/<repo> --json` is asked for.
pub const LINK_PULL_FIELDS: &str = "state,isDraft,mergedAt,statusCheckRollup,title";

/// What `gh issue view <n> --repo <owner>/<repo> --json` is asked for.
pub const LINK_ISSUE_FIELDS: &str = "state,stateReason,title";

/// How long a folder's pull request or link read is kept before asking `gh` again (1 minute).
pub const PULL_REQUEST_CACHE_MS: u64 = 60_000;

/// Every three minutes: often enough to hear of a failed check before the next look, rarely enough to be no load.
pub const PULL_REQUEST_WATCH_MS: u64 = 3 * 60_000;

/// How long a link read is kept (1 minute).
pub const PULL_LINK_CACHE_MS: u64 = 60_000;

/// How often a link with running checks is polled (3 minutes).
pub const PULL_LINK_AGAIN_MS: u64 = 3 * 60_000;

/// Max concurrent `gh` processes for reading links in a conversation.
pub const PULL_LINK_AT_ONCE: usize = 2;

const MAX_PULL: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PullRequestState {
    Open,
    Draft,
    Merged,
    Closed,
}

impl PullRequestState {
    pub fn word(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Closed => "Closed",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Draft => "draft",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PullRequestChecks {
    Passing,
    Failing,
    Pending,
    None,
}

impl PullRequestChecks {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passing => "passing",
            Self::Failing => "failing",
            Self::Pending => "pending",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PullRequestReview {
    Approved,
    ChangesRequested,
    ReviewRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: PullRequestState,
    pub checks: PullRequestChecks,
    /// How many checks there are, and how many have not passed (failed, or not yet finished).
    pub check_count: usize,
    pub not_passed: usize,
    pub review: Option<PullRequestReview>,
    /// GitHub says it cannot merge into its base as it stands (`mergeable: CONFLICTING`).
    pub conflicts: bool,
    /// When it last moved, when it was asked for (`updatedAt`).
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestNews {
    pub number: u64,
    pub url: String,
    /// "2 of 9 checks failed", "Changes were requested", joined.
    pub said: String,
    /// Something a teammate could fix: a failed check, requested changes, a conflict.
    pub needs_work: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PullLinkAddress {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitHubLinkKind {
    Pull,
    Issue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitHubConversationLink {
    pub kind: GitHubLinkKind,
    pub address: PullLinkAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PullLinkChecks {
    Passing,
    Failing,
    Running,
    None,
}

impl PullLinkChecks {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passing => "passing",
            Self::Failing => "failing",
            Self::Running => "running",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedPullRequest {
    pub state: PullRequestState,
    pub checks: PullLinkChecks,
    pub passed: usize,
    pub failed: usize,
    pub running: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueLinkState {
    Open,
    ClosedCompleted,
    ClosedNotPlanned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedIssue {
    pub state: IssueLinkState,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationPiece {
    Text(String),
    Link {
        text: String,
        href: String,
        pull: Option<PullLinkAddress>,
        issue: Option<PullLinkAddress>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckStanding {
    Passed,
    Failed,
    Pending,
}

fn is_failed_check(s: &str) -> bool {
    matches!(
        s,
        "FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE" | "ERROR"
    )
}

fn is_passed_check(s: &str) -> bool {
    matches!(s, "SUCCESS" | "NEUTRAL" | "SKIPPED")
}

fn check_of(check: &serde_json::Map<String, Value>) -> CheckStanding {
    if check.get("__typename").and_then(Value::as_str) == Some("StatusContext") {
        let state = check.get("state").and_then(Value::as_str).unwrap_or("");
        return if is_passed_check(state) {
            CheckStanding::Passed
        } else if is_failed_check(state) {
            CheckStanding::Failed
        } else {
            CheckStanding::Pending
        };
    }
    if check.get("status").and_then(Value::as_str) != Some("COMPLETED") {
        return CheckStanding::Pending;
    }
    let conclusion = check.get("conclusion").and_then(Value::as_str).unwrap_or("");
    if is_passed_check(conclusion) {
        CheckStanding::Passed
    } else if is_failed_check(conclusion) {
        CheckStanding::Failed
    } else {
        CheckStanding::Pending
    }
}

fn pr_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        crate::invariants::regex(r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/pull/\d+$")
    })
}

fn clean_title(raw: Option<&str>) -> String {
    let Some(s) = raw else { return String::new() };
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(200).collect()
}

fn pull_request_from(parsed: &Value) -> Option<FolderPullRequest> {
    let pr = parsed.as_object()?;
    let number = pr.get("number").and_then(Value::as_u64).filter(|&n| n > 0)?;
    let url = pr.get("url").and_then(Value::as_str).filter(|u| pr_url_re().is_match(u))?;
    let raw = pr.get("state").and_then(Value::as_str).unwrap_or("").to_uppercase();
    let is_draft = pr.get("isDraft").and_then(Value::as_bool) == Some(true);
    let state = match raw.as_str() {
        "MERGED" => PullRequestState::Merged,
        "CLOSED" => PullRequestState::Closed,
        _ if is_draft => PullRequestState::Draft,
        _ => PullRequestState::Open,
    };
    let mut failed = 0usize;
    let mut pending = 0usize;
    let mut check_count = 0usize;
    if let Some(arr) = pr.get("statusCheckRollup").and_then(Value::as_array) {
        for one in arr {
            let Some(obj) = one.as_object() else { continue };
            check_count += 1;
            match check_of(obj) {
                CheckStanding::Passed => {}
                CheckStanding::Failed => failed += 1,
                CheckStanding::Pending => pending += 1,
            }
        }
    }
    let checks = if check_count == 0 {
        PullRequestChecks::None
    } else if failed > 0 {
        PullRequestChecks::Failing
    } else if pending > 0 {
        PullRequestChecks::Pending
    } else {
        PullRequestChecks::Passing
    };
    let review = match pr.get("reviewDecision").and_then(Value::as_str) {
        Some("APPROVED") => Some(PullRequestReview::Approved),
        Some("CHANGES_REQUESTED") => Some(PullRequestReview::ChangesRequested),
        Some("REVIEW_REQUIRED") => Some(PullRequestReview::ReviewRequired),
        _ => None,
    };
    let conflicts = pr.get("mergeable").and_then(Value::as_str) == Some("CONFLICTING");
    let updated_at = pr
        .get("updatedAt")
        .and_then(Value::as_str)
        .filter(|s| DateTime::parse_from_rfc3339(s).is_ok())
        .map(str::to_owned);
    Some(FolderPullRequest {
        number,
        title: clean_title(pr.get("title").and_then(Value::as_str)),
        url: url.to_owned(),
        state,
        checks,
        check_count,
        not_passed: failed + pending,
        review,
        conflicts,
        updated_at,
    })
}

/// `gh pr view --json <PULL_REQUEST_FIELDS>` -> the pull request, or `None` when it is not one.
pub fn pull_request_of(json: &str) -> Option<FolderPullRequest> {
    let parsed: Value = serde_json::from_str(json).ok()?;
    pull_request_from(&parsed)
}

/// `gh pr list --json <PULL_REQUEST_FIELDS>` -> each pull request in it that reads as one.
pub fn pull_requests_of(json: &str) -> Vec<FolderPullRequest> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    arr.iter().filter_map(pull_request_from).collect()
}

/// `"Open · all 2 checks passed"`, `"Merged"`, `"Draft · 2 of 9 checks not passed"`.
pub fn pull_request_line(pr: &FolderPullRequest) -> String {
    let mut parts = vec![pr.state.word().to_owned()];
    if matches!(pr.state, PullRequestState::Open | PullRequestState::Draft) {
        match pr.checks {
            PullRequestChecks::Passing => {
                if pr.check_count == 1 {
                    parts.push("its check passed".to_owned());
                } else {
                    parts.push(format!("all {} checks passed", pr.check_count));
                }
            }
            PullRequestChecks::Failing => {
                parts.push(format!("{} of {} checks not passed", pr.not_passed, pr.check_count));
            }
            PullRequestChecks::Pending => {
                parts.push("checks running".to_owned());
            }
            PullRequestChecks::None => {}
        }
        match pr.review {
            Some(PullRequestReview::Approved) => parts.push("approved".to_owned()),
            Some(PullRequestReview::ChangesRequested) => parts.push("changes requested".to_owned()),
            Some(PullRequestReview::ReviewRequired) | None => {}
        }
    }
    parts.join(" · ")
}

/// What changed between two looks at one pull request that a person would want to hear; `None` for nothing.
pub fn pull_request_news(before: &FolderPullRequest, now: &FolderPullRequest) -> Option<PullRequestNews> {
    if before.number != now.number {
        return None;
    }
    let mut said: Vec<String> = Vec::new();
    let mut needs_work = false;
    if before.state != now.state && matches!(now.state, PullRequestState::Merged | PullRequestState::Closed) {
        said.push(if now.state == PullRequestState::Merged {
            "it was merged".to_owned()
        } else {
            "it was closed".to_owned()
        });
    } else {
        if now.checks == PullRequestChecks::Failing && before.checks != PullRequestChecks::Failing {
            said.push(format!("{} of {} checks did not pass", now.not_passed, now.check_count));
            needs_work = true;
        } else if now.checks == PullRequestChecks::Passing
            && before.checks != PullRequestChecks::Passing
            && before.checks != PullRequestChecks::None
        {
            if now.check_count == 1 {
                said.push("its check passed".to_owned());
            } else {
                said.push(format!("all {} checks passed", now.check_count));
            }
        }
        if now.review != before.review && now.review == Some(PullRequestReview::ChangesRequested) {
            said.push("changes were requested".to_owned());
            needs_work = true;
        } else if now.review != before.review && now.review == Some(PullRequestReview::Approved) {
            said.push("it was approved".to_owned());
        }
        if now.conflicts && !before.conflicts {
            said.push("it conflicts with its base branch".to_owned());
            needs_work = true;
        }
    }
    if said.is_empty() {
        return None;
    }
    let sentence = said.join(", and ");
    let mut chars = sentence.chars();
    let capitalized = match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => sentence,
    };
    Some(PullRequestNews {
        number: now.number,
        url: now.url.clone(),
        said: capitalized,
        needs_work,
    })
}

/// The words the ask puts in the box, for the person to send or change.
pub fn pull_request_ask(news: &PullRequestNews) -> String {
    format!(
        "Pull request #{} ({}): {}. Look at what is wrong, fix it on this branch, and push the fix.",
        news.number, news.url, news.said
    )
}

fn valid_owner(owner: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        crate::invariants::regex(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$")
    });
    re.is_match(owner)
}

fn valid_repo(repo: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| crate::invariants::regex(r"^[A-Za-z0-9._-]{1,100}$"));
    repo != "." && repo != ".." && re.is_match(repo)
}

fn parse_pull_number(raw: &str) -> Option<u64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| crate::invariants::regex(r"^[1-9]\d{0,9}$"));
    if !re.is_match(raw) {
        return None;
    }
    let number: u64 = raw.parse().ok()?;
    (1..=MAX_PULL).contains(&number).then_some(number)
}

fn address_of(owner: &str, repo: &str, raw: &str) -> Option<PullLinkAddress> {
    if !valid_owner(owner) || !valid_repo(repo) {
        return None;
    }
    let number = parse_pull_number(raw)?;
    Some(PullLinkAddress {
        owner: owner.to_owned(),
        repo: repo.to_owned(),
        number,
    })
}

/// `https://github.com/<owner>/<repo>/pull/<n>`, or the same shape for an issue. Anything else is not one.
pub fn github_link_of(url: &str) -> Option<GitHubConversationLink> {
    if url.len() > 2048 {
        return None;
    }
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host != "github.com" {
        return None;
    }
    let parts: Vec<&str> = parsed
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 4 {
        return None;
    }
    let address = address_of(parts[0], parts[1], parts[3])?;
    match parts[2] {
        "pull" => Some(GitHubConversationLink {
            kind: GitHubLinkKind::Pull,
            address,
        }),
        "issues" => Some(GitHubConversationLink {
            kind: GitHubLinkKind::Issue,
            address,
        }),
        _ => None,
    }
}

/// The only request the window may make: owner, repo, and a positive number. Anything else is refused.
pub fn pull_link_request(value: &Value) -> Option<PullLinkAddress> {
    let record = value.as_object()?;
    for key in record.keys() {
        if !matches!(key.as_str(), "owner" | "repo" | "number") {
            return None;
        }
    }
    let owner = record.get("owner")?.as_str()?;
    let repo = record.get("repo")?.as_str()?;
    let number_val = record.get("number")?;
    // Refuse strings, floats with fractional parts, booleans, etc.
    let number = number_val.as_u64()?;
    if !(1..=MAX_PULL).contains(&number) {
        return None;
    }
    if !valid_owner(owner) || !valid_repo(repo) {
        return None;
    }
    Some(PullLinkAddress {
        owner: owner.to_owned(),
        repo: repo.to_owned(),
        number,
    })
}

/// The only request the window may make for an issue: the same three parts, on its own channel.
pub fn issue_link_request(value: &Value) -> Option<PullLinkAddress> {
    pull_link_request(value)
}

/// `gh pr view --json state,isDraft,mergedAt,statusCheckRollup,title`.
pub fn linked_pull_request_of(json: &str) -> Option<LinkedPullRequest> {
    let parsed: Value = serde_json::from_str(json).ok()?;
    let pr = parsed.as_object()?;
    let raw = pr
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    let merged = pr
        .get("mergedAt")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    let is_draft = pr.get("isDraft").and_then(Value::as_bool) == Some(true);
    let state = match raw.as_str() {
        "MERGED" => PullRequestState::Merged,
        "CLOSED" => {
            if merged {
                PullRequestState::Merged
            } else {
                PullRequestState::Closed
            }
        }
        "OPEN" => {
            if is_draft {
                PullRequestState::Draft
            } else {
                PullRequestState::Open
            }
        }
        _ => return None,
    };
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut running = 0usize;
    if let Some(arr) = pr.get("statusCheckRollup").and_then(Value::as_array) {
        for one in arr {
            let Some(obj) = one.as_object() else { continue };
            match check_of(obj) {
                CheckStanding::Passed => passed += 1,
                CheckStanding::Failed => failed += 1,
                CheckStanding::Pending => running += 1,
            }
        }
    }
    let total = passed + failed + running;
    let settled = matches!(state, PullRequestState::Merged | PullRequestState::Closed);
    let checks = if settled || total == 0 {
        PullLinkChecks::None
    } else if failed > 0 {
        PullLinkChecks::Failing
    } else if running > 0 {
        PullLinkChecks::Running
    } else {
        PullLinkChecks::Passing
    };
    Some(LinkedPullRequest {
        state,
        checks,
        passed,
        failed,
        running,
        total,
    })
}

/// `gh issue view --json state,stateReason,title`.
pub fn linked_issue_of(json: &str) -> Option<LinkedIssue> {
    let parsed: Value = serde_json::from_str(json).ok()?;
    let issue = parsed.as_object()?;
    let raw = issue
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    let reason = issue
        .get("stateReason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    let state = match raw.as_str() {
        "OPEN" => IssueLinkState::Open,
        "CLOSED" => {
            if reason == "NOT_PLANNED" {
                IssueLinkState::ClosedNotPlanned
            } else {
                IssueLinkState::ClosedCompleted
            }
        }
        _ => return None,
    };
    let title = clean_title(issue.get("title").and_then(Value::as_str));
    Some(LinkedIssue { state, title })
}

pub fn issue_link_word(state: IssueLinkState) -> &'static str {
    match state {
        IssueLinkState::Open => "Open",
        IssueLinkState::ClosedCompleted => "Closed",
        IssueLinkState::ClosedNotPlanned => "Not planned",
    }
}

pub fn issue_link_hover(link: &LinkedIssue) -> String {
    let standing = match link.state {
        IssueLinkState::Open => "Open",
        IssueLinkState::ClosedCompleted => "Closed as completed",
        IssueLinkState::ClosedNotPlanned => "Closed as not planned",
    };
    if link.title.is_empty() {
        standing.to_owned()
    } else {
        format!("{standing} · {}", link.title)
    }
}

pub fn pull_link_word(state: PullRequestState) -> &'static str {
    state.word()
}

/// `"Open · 3 of 4 checks passed · 1 running"`. A merged or closed one's checks are history.
pub fn pull_link_hover(link: &LinkedPullRequest) -> String {
    let word = pull_link_word(link.state);
    if matches!(link.state, PullRequestState::Merged | PullRequestState::Closed) || link.total == 0 {
        return word.to_owned();
    }
    let name = if link.total == 1 { "check" } else { "checks" };
    let mut parts = vec![
        word.to_owned(),
        format!("{} of {} {name} passed", link.passed, link.total),
    ];
    if link.failed > 0 {
        parts.push(format!("{} failing", link.failed));
    }
    if link.running > 0 {
        parts.push(format!("{} running", link.running));
    }
    parts.join(" · ")
}

fn web_href(url: &str) -> bool {
    if url.len() > 2048 {
        return false;
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    matches!(parsed.scheme(), "https" | "http")
        && parsed.host_str().is_some_and(|h| !h.is_empty())
        && parsed.username().is_empty()
}

/// Plain text split into words and web links.
/// A bare `https://github.com/.../pull/n` or `.../issues/n` becomes a link and wears its chip.
/// A URL inside `` `code` `` stays code.
pub fn conversation_pieces(text: &str) -> Vec<ConversationPiece> {
    static CODE_RE: OnceLock<Regex> = OnceLock::new();
    static MD_LINK_RE: OnceLock<Regex> = OnceLock::new();
    static BARE_GH_RE: OnceLock<Regex> = OnceLock::new();

    let code_re = CODE_RE.get_or_init(|| crate::invariants::regex(r"`[^`\n]*`"));
    let md_link_re = MD_LINK_RE.get_or_init(|| {
        crate::invariants::regex(r"\[([^\]\n]*)\]\((https?://[^)\s]+)\)")
    });
    let bare_gh_re = BARE_GH_RE.get_or_init(|| {
        crate::invariants::regex(
            r"https://github\.com/[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?/[A-Za-z0-9._-]{1,100}/(?:pull|issues)/[1-9]\d{0,9}/?",
        )
    });

    // Replace backtick spans with spaces of the exact same byte length so all match offsets line up with `text`.
    let mut scan_bytes = text.as_bytes().to_vec();
    for m in code_re.find_iter(text) {
        for b in &mut scan_bytes[m.start()..m.end()] {
            *b = b' ';
        }
    }
    let scan = String::from_utf8(scan_bytes).unwrap_or_else(|_| text.to_owned());

    struct Hit {
        at: usize,
        len: usize,
        piece: ConversationPiece,
    }
    let mut hits: Vec<Hit> = Vec::new();

    for caps in md_link_re.captures_iter(&scan) {
        let Some(full) = caps.get(0) else { continue };
        let label = caps.get(1).map_or("", |m| m.as_str());
        let href = caps.get(2).map_or("", |m| m.as_str());
        if !web_href(href) {
            continue;
        }
        let found = github_link_of(href);
        hits.push(Hit {
            at: full.start(),
            len: full.len(),
            piece: ConversationPiece::Link {
                text: if label.is_empty() {
                    href.to_owned()
                } else {
                    label.to_owned()
                },
                href: href.to_owned(),
                pull: found
                    .as_ref()
                    .filter(|f| f.kind == GitHubLinkKind::Pull)
                    .map(|f| f.address.clone()),
                issue: found
                    .as_ref()
                    .filter(|f| f.kind == GitHubLinkKind::Issue)
                    .map(|f| f.address.clone()),
            },
        });
    }

    for m in bare_gh_re.find_iter(&scan) {
        let next = scan.as_bytes().get(m.end()).copied();
        if matches!(next, Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/')) {
            continue;
        }
        let at = m.start();
        if hits.iter().any(|h| at >= h.at && at < h.at + h.len) {
            continue;
        }
        let href = m.as_str();
        let Some(found) = github_link_of(href) else {
            continue;
        };
        hits.push(Hit {
            at,
            len: href.len(),
            piece: ConversationPiece::Link {
                text: href.to_owned(),
                href: href.to_owned(),
                pull: (found.kind == GitHubLinkKind::Pull).then(|| found.address.clone()),
                issue: (found.kind == GitHubLinkKind::Issue).then(|| found.address.clone()),
            },
        });
    }

    hits.sort_by_key(|h| h.at);
    if hits.is_empty() {
        return vec![ConversationPiece::Text(text.to_owned())];
    }
    let mut pieces = Vec::new();
    let mut cursor = 0usize;
    for hit in hits {
        if hit.at < cursor {
            continue;
        }
        if hit.at > cursor {
            pieces.push(ConversationPiece::Text(text[cursor..hit.at].to_owned()));
        }
        pieces.push(hit.piece);
        cursor = hit.at + hit.len;
    }
    if cursor < text.len() {
        pieces.push(ConversationPiece::Text(text[cursor..].to_owned()));
    }
    pieces
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
enum AnyLinked {
    Pull(Option<LinkedPullRequest>),
    Issue(Option<LinkedIssue>),
}

struct CachedLink {
    at: u64,
    value: AnyLinked,
}

struct InFlight {
    done: bool,
    value: Option<AnyLinked>,
}

struct SemState {
    running: usize,
    at_once: usize,
}

struct Semaphore {
    state: Mutex<SemState>,
    cv: Condvar,
}

impl Semaphore {
    fn new(at_once: usize) -> Self {
        Self {
            state: Mutex::new(SemState {
                running: 0,
                at_once: at_once.max(1),
            }),
            cv: Condvar::new(),
        }
    }

    fn acquire(&self) -> SemGuard<'_> {
        let mut s = lock(&self.state);
        while s.running >= s.at_once {
            s = self.cv.wait(s).expect("semaphore wait");
        }
        s.running += 1;
        SemGuard(self)
    }
}

struct SemGuard<'a>(&'a Semaphore);

impl Drop for SemGuard<'_> {
    fn drop(&mut self) {
        let mut s = lock(&self.0.state);
        s.running = s.running.saturating_sub(1);
        self.0.cv.notify_one();
    }
}

/// Reads a linked pull request or issue with the person's own `gh`.
///
/// Many links to one pull request or issue share one read. A conversation full of different ones
/// runs at most `at_once` (default 2) `gh` processes at a time. A missing `gh` and an error are cached
/// as `None` for `cache_for_ms` (default 60s) and never retried in a tight loop.
pub struct PullLinkReader {
    network: Arc<dyn Network>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
    cache_for_ms: u64,
    cache: Mutex<HashMap<String, CachedLink>>,
    flying: Mutex<HashMap<String, Arc<(Mutex<InFlight>, Condvar)>>>,
    sem: Semaphore,
}

impl PullLinkReader {
    pub fn new(network: Arc<dyn Network>) -> Self {
        Self::with_options(network, Arc::new(now_ms), PULL_LINK_CACHE_MS, PULL_LINK_AT_ONCE)
    }

    pub fn with_options(
        network: Arc<dyn Network>,
        now: Arc<dyn Fn() -> u64 + Send + Sync>,
        cache_for_ms: u64,
        at_once: usize,
    ) -> Self {
        Self {
            network,
            now,
            cache_for_ms,
            cache: Mutex::new(HashMap::new()),
            flying: Mutex::new(HashMap::new()),
            sem: Semaphore::new(at_once),
        }
    }

    fn key_of(kind: GitHubLinkKind, address: &PullLinkAddress) -> String {
        let prefix = match kind {
            GitHubLinkKind::Pull => "pull",
            GitHubLinkKind::Issue => "issue",
        };
        format!("{prefix}:{}/{}#{}", address.owner, address.repo, address.number)
    }

    fn ask(&self, kind: GitHubLinkKind, address: &PullLinkAddress) -> AnyLinked {
        let _guard = self.sem.acquire();
        let num = address.number.to_string();
        let repo = format!("{}/{}", address.owner, address.repo);
        let args: Vec<String> = match kind {
            GitHubLinkKind::Pull => vec![
                "pr".into(),
                "view".into(),
                num,
                "--repo".into(),
                repo,
                "--json".into(),
                LINK_PULL_FIELDS.into(),
            ],
            GitHubLinkKind::Issue => vec![
                "issue".into(),
                "view".into(),
                num,
                "--repo".into(),
                repo,
                "--json".into(),
                LINK_ISSUE_FIELDS.into(),
            ],
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        match self.network.run("gh", &args, &cwd) {
            Ok(json) => match kind {
                GitHubLinkKind::Pull => AnyLinked::Pull(linked_pull_request_of(&json)),
                GitHubLinkKind::Issue => AnyLinked::Issue(linked_issue_of(&json)),
            },
            Err(_) => match kind {
                GitHubLinkKind::Pull => AnyLinked::Pull(None),
                GitHubLinkKind::Issue => AnyLinked::Issue(None),
            },
        }
    }

    fn read_kind(&self, kind: GitHubLinkKind, address: &PullLinkAddress) -> AnyLinked {
        let key = Self::key_of(kind, address);
        let now = (self.now)();
        {
            let cache = lock(&self.cache);
            if let Some(kept) = cache.get(&key)
                && now.saturating_sub(kept.at) < self.cache_for_ms
            {
                return kept.value.clone();
            }
        }
        let (flight, is_owner) = {
            let mut flying = lock(&self.flying);
            if let Some(existing) = flying.get(&key) {
                (existing.clone(), false)
            } else {
                let created = Arc::new((
                    Mutex::new(InFlight {
                        done: false,
                        value: None,
                    }),
                    Condvar::new(),
                ));
                flying.insert(key.clone(), created.clone());
                (created, true)
            }
        };

        if !is_owner {
            let (m, cv) = &*flight;
            let mut st = lock(m);
            while !st.done {
                st = cv.wait(st).expect("flight wait");
            }
            return st.value.clone().unwrap_or(match kind {
                GitHubLinkKind::Pull => AnyLinked::Pull(None),
                GitHubLinkKind::Issue => AnyLinked::Issue(None),
            });
        }

        let value = self.ask(kind, address);
        let finished_at = (self.now)();
        {
            let mut cache = lock(&self.cache);
            cache.insert(
                key.clone(),
                CachedLink {
                    at: finished_at,
                    value: value.clone(),
                },
            );
        }
        {
            let (m, cv) = &*flight;
            let mut st = lock(m);
            st.value = Some(value.clone());
            st.done = true;
            cv.notify_all();
        }
        {
            let mut flying = lock(&self.flying);
            flying.remove(&key);
        }
        value
    }

    pub fn read(&self, request: &Value) -> Option<LinkedPullRequest> {
        let address = pull_link_request(request)?;
        self.read_address(&address)
    }

    pub fn read_address(&self, address: &PullLinkAddress) -> Option<LinkedPullRequest> {
        match self.read_kind(GitHubLinkKind::Pull, address) {
            AnyLinked::Pull(v) => v,
            AnyLinked::Issue(_) => None,
        }
    }

    pub fn read_issue(&self, request: &Value) -> Option<LinkedIssue> {
        let address = issue_link_request(request)?;
        self.read_issue_address(&address)
    }

    pub fn read_issue_address(&self, address: &PullLinkAddress) -> Option<LinkedIssue> {
        match self.read_kind(GitHubLinkKind::Issue, address) {
            AnyLinked::Issue(v) => v,
            AnyLinked::Pull(_) => None,
        }
    }
}

/// Watching the folder's pull request: while the person has asked, it is looked at every few minutes,
/// and what changed is said. One timer while anything is watched, none otherwise; a merged or closed
/// one is no longer watched once that has been said.
pub struct PullRequestWatch {
    read: Box<dyn Fn() -> Option<FolderPullRequest> + Send + Sync>,
    on_news: Box<dyn Fn(PullRequestNews) + Send + Sync>,
    every_ms: u64,
    watched: Mutex<HashSet<String>>,
    last: Mutex<HashMap<String, FolderPullRequest>>,
    looking: Mutex<bool>,
    timer_armed: Mutex<bool>,
    on_timer_change: Option<Box<dyn Fn(bool, u64) + Send + Sync>>,
}

impl PullRequestWatch {
    pub fn new(
        read: impl Fn() -> Option<FolderPullRequest> + Send + Sync + 'static,
        on_news: impl Fn(PullRequestNews) + Send + Sync + 'static,
    ) -> Self {
        Self::with_options(read, on_news, PULL_REQUEST_WATCH_MS, None)
    }

    pub fn with_options(
        read: impl Fn() -> Option<FolderPullRequest> + Send + Sync + 'static,
        on_news: impl Fn(PullRequestNews) + Send + Sync + 'static,
        every_ms: u64,
        on_timer_change: Option<Box<dyn Fn(bool, u64) + Send + Sync>>,
    ) -> Self {
        Self {
            read: Box::new(read),
            on_news: Box::new(on_news),
            every_ms,
            watched: Mutex::new(HashSet::new()),
            last: Mutex::new(HashMap::new()),
            looking: Mutex::new(false),
            timer_armed: Mutex::new(false),
            on_timer_change,
        }
    }

    fn sync_timer(&self) {
        let should_arm = !lock(&self.watched).is_empty();
        let mut armed = lock(&self.timer_armed);
        if *armed != should_arm {
            *armed = should_arm;
            if let Some(cb) = &self.on_timer_change {
                cb(should_arm, self.every_ms);
            }
        }
    }

    pub fn watch(&self, url: &str, on: bool) {
        {
            let mut watched = lock(&self.watched);
            if on {
                watched.insert(url.to_owned());
            } else {
                watched.remove(url);
                lock(&self.last).remove(url);
            }
        }
        self.sync_timer();
        if on {
            self.look();
        }
    }

    pub fn watching(&self, url: &str) -> bool {
        lock(&self.watched).contains(url)
    }

    pub fn timer_armed(&self) -> bool {
        *lock(&self.timer_armed)
    }

    pub fn every_ms(&self) -> u64 {
        self.every_ms
    }

    pub fn stop(&self) {
        lock(&self.watched).clear();
        lock(&self.last).clear();
        self.sync_timer();
    }

    pub fn look(&self) {
        {
            let mut looking = lock(&self.looking);
            if *looking || lock(&self.watched).is_empty() {
                return;
            }
            *looking = true;
        }
        let now = (self.read)();
        if let Some(now) = now {
            let is_watched = lock(&self.watched).contains(&now.url);
            if is_watched {
                let before = {
                    let mut last = lock(&self.last);
                    let prev = last.get(&now.url).cloned();
                    last.insert(now.url.clone(), now.clone());
                    prev
                };
                if let Some(before) = before
                    && let Some(news) = pull_request_news(&before, &now)
                {
                    (self.on_news)(news);
                }
                if matches!(now.state, PullRequestState::Merged | PullRequestState::Closed) {
                    self.watch(&now.url, false);
                }
            }
        }
        *lock(&self.looking) = false;
    }
}

/// Pure state machine for link watching on screen (`renderer/src/pullLinkWatch.ts`):
/// - One pull request or issue is read once, kept for a minute (`minute_ms = 60_000`),
///   and read again when the window comes back after that (`on_focus`).
/// - A pull request is also read every few minutes (`again_ms = 180_000`) while its checks are still running.
/// - A link with no active listeners has no timer and is not read.
pub struct LinkSlot<T> {
    pub address: PullLinkAddress,
    pub value: Option<T>,
    pub at: u64,
    pub listeners: HashSet<u64>,
}

pub struct LinkWatch<T> {
    slots: HashMap<String, LinkSlot<T>>,
    next_id: u64,
    minute_ms: u64,
    again_ms: u64,
    timer_armed: bool,
    focus_armed: bool,
}

impl<T> Default for LinkWatch<T> {
    fn default() -> Self {
        Self::new(PULL_LINK_CACHE_MS, PULL_LINK_AGAIN_MS)
    }
}

impl<T> LinkWatch<T> {
    pub fn new(minute_ms: u64, again_ms: u64) -> Self {
        Self {
            slots: HashMap::new(),
            next_id: 1,
            minute_ms,
            again_ms,
            timer_armed: false,
            focus_armed: false,
        }
    }

    pub fn key_of(address: &PullLinkAddress) -> String {
        format!("{}/{}#{}", address.owner, address.repo, address.number)
    }

    pub fn timer_armed(&self) -> bool {
        self.timer_armed
    }

    pub fn focus_armed(&self) -> bool {
        self.focus_armed
    }

    pub fn again_ms(&self) -> u64 {
        self.again_ms
    }

    pub fn current(&self, address: &PullLinkAddress) -> Option<&T> {
        let slot = self.slots.get(&Self::key_of(address))?;
        if slot.at == 0 {
            return None;
        }
        slot.value.as_ref()
    }

    fn any_listeners(&self) -> bool {
        self.slots.values().any(|s| !s.listeners.is_empty())
    }

    fn prune(&mut self, now: u64) {
        self.slots.retain(|_, slot| {
            !(slot.listeners.is_empty() && slot.at > 0 && now.saturating_sub(slot.at) >= self.minute_ms)
        });
    }

    fn sync_arms(&mut self, is_moving: &impl Fn(&T) -> bool) {
        self.focus_armed = self.any_listeners();
        self.timer_armed = self.slots.values().any(|slot| {
            !slot.listeners.is_empty() && slot.value.as_ref().is_some_and(is_moving)
        });
    }

    /// Registers a listener and returns `(listener_id, should_read_now)`.
    pub fn watch(&mut self, address: &PullLinkAddress, now: u64, is_moving: impl Fn(&T) -> bool) -> (u64, bool) {
        self.prune(now);
        let id = self.next_id;
        self.next_id += 1;
        let key = Self::key_of(address);
        let slot = self.slots.entry(key).or_insert_with(|| LinkSlot {
            address: address.clone(),
            value: None,
            at: 0,
            listeners: HashSet::new(),
        });
        slot.listeners.insert(id);
        let should_read = slot.at == 0 || now.saturating_sub(slot.at) >= self.minute_ms;
        self.sync_arms(&is_moving);
        (id, should_read)
    }

    pub fn finish_read(&mut self, address: &PullLinkAddress, value: Option<T>, now: u64, is_moving: impl Fn(&T) -> bool) {
        if let Some(slot) = self.slots.get_mut(&Self::key_of(address)) {
            slot.value = value;
            slot.at = now;
        }
        self.sync_arms(&is_moving);
    }

    pub fn unwatch(&mut self, address: &PullLinkAddress, id: u64, is_moving: impl Fn(&T) -> bool) {
        if let Some(slot) = self.slots.get_mut(&Self::key_of(address)) {
            slot.listeners.remove(&id);
        }
        self.sync_arms(&is_moving);
    }

    /// Returns addresses that should be re-read on window focus.
    pub fn on_focus(&self, now: u64) -> Vec<PullLinkAddress> {
        if !self.focus_armed {
            return Vec::new();
        }
        self.slots
            .values()
            .filter(|slot| !slot.listeners.is_empty() && (slot.at == 0 || now.saturating_sub(slot.at) >= self.minute_ms))
            .map(|slot| slot.address.clone())
            .collect()
    }

    /// Returns addresses that should be re-read on a periodic tick.
    pub fn on_tick(&self, is_moving: impl Fn(&T) -> bool) -> Vec<PullLinkAddress> {
        if !self.timer_armed {
            return Vec::new();
        }
        self.slots
            .values()
            .filter(|slot| !slot.listeners.is_empty() && slot.value.as_ref().is_some_and(&is_moving))
            .map(|slot| slot.address.clone())
            .collect()
    }
}

pub fn is_pull_link_moving(link: &LinkedPullRequest) -> bool {
    matches!(link.state, PullRequestState::Open | PullRequestState::Draft)
        && link.checks == PullLinkChecks::Running
}

#[cfg(test)]
#[path = "pull_requests_tests.rs"]
mod tests;
