//! SCOUT'S AWAY LOOK & /LOCUST ASKS WHILE YOUR COMPUTER IS OFF
//! (Electron `shared/away-letter.ts`, `main/scout-away-look.ts`, `main/scout-away.ts`, 0.829 / 0.838).

use crate::pull_requests::{FolderPullRequest, PullRequestChecks, PullRequestReview, PullRequestState};
use crate::relay_crypto::{
    aes_gcm_decrypt, aes_gcm_encrypt, fill_random, from_base64url, random_text, sha256_b64url,
    to_base64url,
};
use crate::scout::types::{ScoutChip, ScoutFact, ScoutOpen, ScoutQuietHours, ScoutTone, ScoutUrgency};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

pub const AWAY_LETTER_VERSION: u32 = 1;
pub const MAX_TOLD: usize = 500;
pub const AWAY_ASK_KEY: &str = "away-ask:";
pub const MAX_FOUND: usize = 40;
const MAX_SAID: usize = 400;

pub const SCOUT_AWAY_FILE: &str = "scout-away.json";
pub const AWAY_SECRET: &str = "LOCUST_SCOUT";
pub const AWAY_BRANCH: &str = "locust/scout-away";
pub const AWAY_WORKFLOW_NAME: &str = "Locust Scout";
pub const AWAY_WORKFLOW_PATH: &str = ".github/workflows/locust-scout.yml";
pub const AWAY_SCRIPT_PATH: &str = ".github/locust/scout-away.mjs";
/// The generated scheduled entry installed by the native app and run by GitHub Actions.
pub const PACKAGED_AWAY_SCRIPT: &str = include_str!("../../../desktop/resources/scout-away.mjs");

// ---- Away letters (shared/away-letter.ts) ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayFinding {
    pub key: String,
    pub urgency: ScoutUrgency,
    pub said: String,
    pub detail: Option<String>,
    pub lines: Option<Vec<String>>,
    pub chips: Option<Vec<ScoutChip>>,
    pub url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwayLetterKind {
    Look,
    Told,
}

impl AwayLetterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Look => "look",
            Self::Told => "told",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwayLetterFrom {
    Away,
    Pc,
}

impl AwayLetterFrom {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Away => "away",
            Self::Pc => "pc",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayLetter {
    pub v: u32,
    pub kind: AwayLetterKind,
    pub at: String,
    pub repo: String,
    pub read: bool,
    pub found: Vec<AwayFinding>,
    pub prs: Vec<FolderPullRequest>,
    pub told: Vec<String>,
    pub brief_day: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedAwayBox {
    pub n: String,
    pub c: String,
}

pub fn away_aad(room: &str, from: AwayLetterFrom) -> Vec<u8> {
    format!("locust-away/v1|{room}|{}", from.as_str()).into_bytes()
}

pub fn new_away_key() -> String {
    random_text(32)
}

fn clip(text: &str, most: usize) -> String {
    if text.chars().count() > most {
        let mut s: String = text.chars().take(most - 1).collect();
        s.push('…');
        s
    } else {
        text.to_owned()
    }
}

fn repo_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[\w.-]{1,100}/[\w.-]{1,100}$").unwrap())
}

fn github_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^https://github\.com/[\w./#?=&%-]+$").unwrap())
}

fn day_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap())
}

pub fn repo_link(repo: &str, url: Option<&str>) -> Option<String> {
    let url = url?;
    if url.len() > 300 {
        return None;
    }
    let prefix = format!("https://github.com/{repo}/").to_ascii_lowercase();
    if url.to_ascii_lowercase().starts_with(&prefix) && github_url_re().is_match(url) {
        Some(url.to_owned())
    } else {
        None
    }
}

impl AwayFinding {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("key".into(), json!(self.key));
        obj.insert("urgency".into(), json!(self.urgency.as_str()));
        obj.insert("said".into(), json!(self.said));
        if let Some(detail) = &self.detail {
            obj.insert("detail".into(), json!(detail));
        }
        if let Some(lines) = &self.lines
            && !lines.is_empty()
        {
            obj.insert("lines".into(), json!(lines));
        }
        if let Some(chips) = &self.chips
            && !chips.is_empty()
        {
            obj.insert(
                "chips".into(),
                Value::Array(chips.iter().map(ScoutChip::to_json).collect()),
            );
        }
        if let Some(url) = &self.url {
            obj.insert("url".into(), json!(url));
        }
        Value::Object(obj)
    }

    pub fn from_json(v: &Value, repo: &str) -> Option<Self> {
        let obj = v.as_object()?;
        let key = obj.get("key").and_then(Value::as_str)?;
        if key.is_empty() || key.len() > 300 {
            return None;
        }
        let urgency = ScoutUrgency::from_str(obj.get("urgency").and_then(Value::as_str)?)?;
        let said = obj.get("said").and_then(Value::as_str)?;
        if said.trim().is_empty() {
            return None;
        }
        let detail = obj
            .get("detail")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| clip(s, MAX_SAID));
        let lines: Vec<String> = obj
            .get("lines")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .take(8)
                    .map(|l| clip(l, 200))
                    .collect()
            })
            .unwrap_or_default();
        let chips: Vec<ScoutChip> = obj
            .get("chips")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let o = c.as_object()?;
                        let text = o.get("text").and_then(Value::as_str)?;
                        if text.is_empty() {
                            return None;
                        }
                        let tone = match o.get("tone").and_then(Value::as_str)? {
                            "red" => ScoutTone::Red,
                            "amber" => ScoutTone::Amber,
                            "green" => ScoutTone::Green,
                            "blue" => ScoutTone::Blue,
                            "quiet" => ScoutTone::Quiet,
                            _ => return None,
                        };
                        Some(ScoutChip::new(clip(text, 40), tone))
                    })
                    .take(4)
                    .collect()
            })
            .unwrap_or_default();
        let url = repo_link(repo, obj.get("url").and_then(Value::as_str));
        Some(Self {
            key: key.to_owned(),
            urgency,
            said: clip(said, MAX_SAID),
            detail,
            lines: if lines.is_empty() { None } else { Some(lines) },
            chips: if chips.is_empty() { None } else { Some(chips) },
            url,
        })
    }
}

fn pr_to_json(pr: &FolderPullRequest) -> Value {
    let mut obj = Map::new();
    obj.insert("number".into(), json!(pr.number));
    obj.insert("title".into(), json!(pr.title));
    obj.insert("url".into(), json!(pr.url));
    obj.insert("state".into(), json!(pr.state.as_str()));
    obj.insert("checks".into(), json!(pr.checks.as_str()));
    obj.insert("checkCount".into(), json!(pr.check_count));
    obj.insert("notPassed".into(), json!(pr.not_passed));
    if let Some(review) = pr.review {
        let r = match review {
            PullRequestReview::Approved => "approved",
            PullRequestReview::ChangesRequested => "changes-requested",
            PullRequestReview::ReviewRequired => "review-required",
        };
        obj.insert("review".into(), json!(r));
    }
    if pr.conflicts {
        obj.insert("conflicts".into(), json!(true));
    }
    if let Some(updated) = &pr.updated_at {
        obj.insert("updatedAt".into(), json!(updated));
    }
    Value::Object(obj)
}

fn pr_from_json(v: &Value, repo: &str) -> Option<FolderPullRequest> {
    let obj = v.as_object()?;
    let url = repo_link(repo, obj.get("url").and_then(Value::as_str))?;
    let number = obj.get("number").and_then(Value::as_u64)?;
    if number == 0 {
        return None;
    }
    let title = obj.get("title").and_then(Value::as_str)?;
    let state = match obj.get("state").and_then(Value::as_str)? {
        "open" => PullRequestState::Open,
        "draft" => PullRequestState::Draft,
        "merged" => PullRequestState::Merged,
        "closed" => PullRequestState::Closed,
        _ => return None,
    };
    let checks = match obj.get("checks").and_then(Value::as_str)? {
        "passing" => PullRequestChecks::Passing,
        "failing" => PullRequestChecks::Failing,
        "pending" => PullRequestChecks::Pending,
        "none" => PullRequestChecks::None,
        _ => return None,
    };
    let check_count = obj.get("checkCount").and_then(Value::as_u64).unwrap_or(0) as usize;
    let not_passed = obj.get("notPassed").and_then(Value::as_u64).unwrap_or(0) as usize;
    let review = match obj.get("review").and_then(Value::as_str) {
        Some("approved") => Some(PullRequestReview::Approved),
        Some("changes-requested") => Some(PullRequestReview::ChangesRequested),
        Some("review-required") => Some(PullRequestReview::ReviewRequired),
        _ => None,
    };
    let conflicts = obj.get("conflicts").and_then(Value::as_bool) == Some(true);
    let updated_at = obj
        .get("updatedAt")
        .and_then(Value::as_str)
        .filter(|s| DateTime::parse_from_rfc3339(s).is_ok())
        .map(str::to_owned);
    Some(FolderPullRequest {
        number,
        title: clip(title, 300),
        url,
        state,
        checks,
        check_count,
        not_passed,
        review,
        conflicts,
        updated_at,
    })
}

impl AwayLetter {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("v".into(), json!(self.v));
        obj.insert("kind".into(), json!(self.kind.as_str()));
        obj.insert("at".into(), json!(self.at));
        obj.insert("repo".into(), json!(self.repo));
        obj.insert("read".into(), json!(self.read));
        obj.insert(
            "found".into(),
            Value::Array(self.found.iter().map(AwayFinding::to_json).collect()),
        );
        obj.insert(
            "prs".into(),
            Value::Array(self.prs.iter().map(pr_to_json).collect()),
        );
        obj.insert("told".into(), json!(self.told));
        if let Some(day) = &self.brief_day {
            obj.insert("briefDay".into(), json!(day));
        }
        Value::Object(obj)
    }

    pub fn from_json(v: &Value, from: AwayLetterFrom) -> Option<Self> {
        let obj = v.as_object()?;
        if obj.get("v").and_then(Value::as_u64) != Some(u64::from(AWAY_LETTER_VERSION)) {
            return None;
        }
        let kind = match obj.get("kind").and_then(Value::as_str)? {
            "look" => AwayLetterKind::Look,
            "told" => AwayLetterKind::Told,
            _ => return None,
        };
        if (kind == AwayLetterKind::Told) != (from == AwayLetterFrom::Pc) {
            return None;
        }
        let at = obj.get("at").and_then(Value::as_str)?;
        if DateTime::parse_from_rfc3339(at).is_err() {
            return None;
        }
        let repo = obj.get("repo").and_then(Value::as_str)?;
        if !repo_re().is_match(repo) {
            return None;
        }
        let found: Vec<AwayFinding> = obj
            .get("found")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .take(MAX_FOUND)
                    .filter_map(|one| AwayFinding::from_json(one, repo))
                    .collect()
            })
            .unwrap_or_default();
        let prs: Vec<FolderPullRequest> = obj
            .get("prs")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .take(50)
                    .filter_map(|one| pr_from_json(one, repo))
                    .collect()
            })
            .unwrap_or_default();
        let mut told: Vec<String> = obj
            .get("told")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .filter(|k| !k.is_empty() && k.len() <= 300)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if told.len() > MAX_TOLD {
            let start = told.len() - MAX_TOLD;
            told.drain(..start);
        }
        let brief_day = obj
            .get("briefDay")
            .and_then(Value::as_str)
            .filter(|d| day_re().is_match(d))
            .map(str::to_owned);
        Some(Self {
            v: AWAY_LETTER_VERSION,
            kind,
            at: at.to_owned(),
            repo: repo.to_owned(),
            read: obj.get("read").and_then(Value::as_bool) == Some(true),
            found,
            prs,
            told,
            brief_day,
        })
    }
}

pub fn seal_away_letter(
    key: &str,
    letter: &AwayLetter,
    room: &str,
    from: AwayLetterFrom,
) -> Result<SealedAwayBox, String> {
    let key_bytes = from_base64url(key)?;
    let mut nonce = [0u8; 12];
    fill_random(&mut nonce);
    let plain = serde_json::to_vec(&letter.to_json()).map_err(|e| e.to_string())?;
    let aad = away_aad(room, from);
    let sealed = aes_gcm_encrypt(&key_bytes, &nonce, &aad, &plain)?;
    Ok(SealedAwayBox {
        n: to_base64url(&nonce),
        c: to_base64url(&sealed),
    })
}

pub fn open_away_letter(
    key: &str,
    boxed: &SealedAwayBox,
    room: &str,
    from: AwayLetterFrom,
) -> Option<AwayLetter> {
    let key_bytes = from_base64url(key).ok()?;
    let nonce = from_base64url(&boxed.n).ok()?;
    let ct = from_base64url(&boxed.c).ok()?;
    let aad = away_aad(room, from);
    let plain = aes_gcm_decrypt(&key_bytes, &nonce, &aad, &ct).ok()?;
    let val: Value = serde_json::from_slice(&plain).ok()?;
    AwayLetter::from_json(&val, from)
}

pub fn finding_of(fact: &ScoutFact) -> AwayFinding {
    let url = match &fact.open {
        Some(ScoutOpen::Url { url, .. }) => Some(url.clone()),
        _ => None,
    };
    AwayFinding {
        key: fact.key.clone(),
        urgency: fact.urgency,
        said: fact.said.clone(),
        detail: fact.detail.clone(),
        lines: fact.lines.clone(),
        chips: fact.chips.clone(),
        url,
    }
}

pub fn fact_of(finding: &AwayFinding) -> ScoutFact {
    ScoutFact {
        key: finding.key.clone(),
        urgency: finding.urgency,
        said: finding.said.clone(),
        detail: finding.detail.clone(),
        quote: None,
        lines: finding.lines.clone(),
        chips: finding.chips.clone(),
        open: finding.url.as_ref().map(|url| ScoutOpen::Url {
            url: url.clone(),
            label: "Open on GitHub".to_owned(),
        }),
        teammate_id: None,
        draft: None,
        tune: None,
        comment_ask: None,
        post: None,
    }
}

// ---- Timezone helper (no env mutation, thread-safe) -----------------------------------------------------------------

fn nth_sunday_of(year: i32, month: u32, n: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let offset = (7 - first.weekday().num_days_from_sunday()) % 7;
    first + ChronoDuration::days(i64::from(offset + 7 * (n - 1)))
}

fn is_us_dst(at: DateTime<Utc>, std_offset_hours: i64) -> bool {
    let year = at.year();
    let start_day = nth_sunday_of(year, 3, 2);
    let end_day = nth_sunday_of(year, 11, 1);
    let start_utc = Utc.from_utc_datetime(
        &(start_day.and_time(NaiveTime::from_hms_opt(2, 0, 0).unwrap())
            - ChronoDuration::hours(std_offset_hours)),
    );
    let end_utc = Utc.from_utc_datetime(
        &(end_day.and_time(NaiveTime::from_hms_opt(2, 0, 0).unwrap())
            - ChronoDuration::hours(std_offset_hours + 1)),
    );
    at >= start_utc && at < end_utc
}

pub fn local_time(at: DateTime<Utc>, tz: &str) -> (String, u32) {
    let offset_hours: i64 = match tz {
        "America/New_York" => if is_us_dst(at, -5) { -4 } else { -5 },
        "America/Chicago" => if is_us_dst(at, -6) { -5 } else { -6 },
        "America/Denver" => if is_us_dst(at, -7) { -6 } else { -7 },
        "America/Los_Angeles" => if is_us_dst(at, -8) { -7 } else { -8 },
        "UTC" | "Etc/UTC" | "GMT" => 0,
        _ => 0,
    };
    let shifted = at + ChronoDuration::hours(offset_hours);
    (shifted.format("%Y-%m-%d").to_string(), shifted.hour())
}

fn quiet_at(quiet: Option<ScoutQuietHours>, hour: u32) -> bool {
    let Some(q) = quiet else {
        return false;
    };
    if q.from == q.to {
        return false;
    }
    if q.from < q.to {
        hour >= q.from && hour < q.to
    } else {
        hour >= q.from || hour < q.to
    }
}

// ---- /locust ask while computer is off (main/scout-away-look.ts) ----------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayConfig {
    pub v: u32,
    pub relay: String,
    pub room: String,
    pub token: String,
    pub key: String,
    pub login: String,
    pub tz: String,
    pub quiet: Option<ScoutQuietHours>,
    pub brief_at: Option<u32>,
}

impl AwayConfig {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("v".into(), json!(self.v));
        obj.insert("relay".into(), json!(self.relay));
        obj.insert("room".into(), json!(self.room));
        obj.insert("token".into(), json!(self.token));
        obj.insert("key".into(), json!(self.key));
        obj.insert("login".into(), json!(self.login));
        obj.insert("tz".into(), json!(self.tz));
        if let Some(q) = self.quiet {
            obj.insert("quiet".into(), q.to_json());
        }
        if let Some(b) = self.brief_at {
            obj.insert("briefAt".into(), json!(b));
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayComment {
    pub id: u64,
    pub login: String,
    pub body: String,
    pub url: String,
    pub number: u64,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayLookResult {
    pub said: String,
    pub ok: bool,
}

#[derive(Clone, Debug)]
pub struct AwayHttpResponse {
    pub status: u16,
    pub body: String,
}

pub trait AwayFetch: Send + Sync {
    fn fetch(&self, url: &str, method: &str, body: Option<&str>) -> Result<AwayHttpResponse, String>;
}

pub fn away_comment_of(event: &Value) -> Option<AwayComment> {
    let obj = event.as_object()?;
    if obj.get("action").and_then(Value::as_str) != Some("created") {
        return None;
    }
    let comment = obj.get("comment").and_then(Value::as_object)?;
    let issue = obj.get("issue").and_then(Value::as_object)?;
    let login = comment
        .get("user")
        .and_then(Value::as_object)
        .and_then(|u| u.get("login"))
        .and_then(Value::as_str)?;
    let id = comment.get("id").and_then(Value::as_u64)?;
    let body = comment.get("body").and_then(Value::as_str)?;
    let url = comment.get("html_url").and_then(Value::as_str)?;
    let number = issue.get("number").and_then(Value::as_u64)?;
    if number < 1 {
        return None;
    }
    let title = issue
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    Some(AwayComment {
        id,
        login: login.to_owned(),
        body: body.to_owned(),
        url: url.to_owned(),
        number,
        title,
    })
}

pub fn ask_of(body: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?m)^/locust(?:[ \t]+|$)").unwrap());
    let m = re.find(body)?;
    let task = body[m.end()..].trim();
    if task.is_empty() {
        None
    } else {
        Some(task.to_owned())
    }
}

pub fn away_ask(
    config: &AwayConfig,
    repo: &str,
    fetch: &dyn AwayFetch,
    now: DateTime<Utc>,
    comment: &AwayComment,
) -> AwayLookResult {
    if !comment.login.eq_ignore_ascii_case(&config.login) {
        return AwayLookResult {
            said: "This comment is not yours, so Scout leaves it.".into(),
            ok: true,
        };
    }
    let Some(task) = ask_of(&comment.body) else {
        return AwayLookResult {
            said: "This comment asks Locust nothing.".into(),
            ok: true,
        };
    };
    let expected_url_re = format!(
        r"^https://github\.com/{}/(?:issues|pull)/{}#issuecomment-{}$",
        regex::escape(repo),
        comment.number,
        comment.id
    );
    if !Regex::new(&expected_url_re)
        .map(|re| re.is_match(&comment.url))
        .unwrap_or(false)
    {
        return AwayLookResult {
            said: "This comment’s link is not this repository’s, so Scout leaves it.".into(),
            ok: true,
        };
    }

    let base = format!("{}/v1/room/{}/away", config.relay, config.room);
    let asked = match fetch.fetch(&base, "GET", None) {
        Ok(r) => r,
        Err(_) => {
            return AwayLookResult {
                said: "Locust’s relay could not be reached. The next look tries again.".into(),
                ok: false,
            };
        }
    };
    if asked.status == 403 {
        return AwayLookResult {
            said: "Scout’s away look is off in Locust. Delete .github/workflows/locust-scout.yml and the LOCUST_SCOUT secret to stop these runs.".into(),
            ok: true,
        };
    }
    if asked.status == 401 || asked.status == 404 {
        return AwayLookResult {
            said: "Locust’s relay did not know this look’s token. Turn “Look while my computer is off” off and on again in Locust.".into(),
            ok: false,
        };
    }
    if !(200..300).contains(&asked.status) {
        return AwayLookResult {
            said: format!("Locust’s relay answered {}. The next look tries again.", asked.status),
            ok: false,
        };
    }
    let answer: Value = serde_json::from_str(&asked.body).unwrap_or_else(|_| json!({}));
    if answer.get("pc").and_then(Value::as_bool) == Some(true) {
        return AwayLookResult {
            said: "Locust is open on your computer, so it reads your ask from there.".into(),
            ok: true,
        };
    }
    let before = answer.get("latest").and_then(|latest| {
        let obj = latest.as_object()?;
        let n = obj.get("n")?.as_str()?.to_owned();
        let c = obj.get("c")?.as_str()?.to_owned();
        let from = if obj.get("from").and_then(Value::as_str) == Some("pc") {
            AwayLetterFrom::Pc
        } else {
            AwayLetterFrom::Away
        };
        open_away_letter(&config.key, &SealedAwayBox { n, c }, &config.room, from)
    });

    let key = format!("{AWAY_ASK_KEY}{repo}:{}", comment.id);
    let told = before.as_ref().map(|b| b.told.clone()).unwrap_or_default();
    if told.iter().any(|t| t == &key) {
        return AwayLookResult {
            said: "Your phone has this ask already.".into(),
            ok: true,
        };
    }
    let first_line = task.lines().next().unwrap_or("").trim();
    let short_line = if first_line.chars().count() > 140 {
        let mut s: String = first_line.chars().take(139).collect();
        s.push('…');
        s
    } else {
        first_line.to_owned()
    };
    let title_part = if comment.title.is_empty() {
        String::new()
    } else {
        format!(" {}", comment.title)
    };
    let fact = ScoutFact {
        key: key.clone(),
        urgency: ScoutUrgency::Now,
        said: format!("You asked on #{}{}: {}", comment.number, title_part, short_line),
        detail: Some(task),
        quote: None,
        lines: Some(vec![
            "Your computer is off: your ask waits on GitHub, and becomes a card for a teammate when Locust is open again.".into(),
        ]),
        chips: Some(vec![ScoutChip::new("while your PC was off", ScoutTone::Quiet)]),
        open: Some(ScoutOpen::Url {
            url: comment.url.clone(),
            label: "Open your comment".into(),
        }),
        teammate_id: None,
        draft: None,
        tune: None,
        comment_ask: None,
        post: None,
    };
    let mut next_told = told;
    next_told.push(key);
    if next_told.len() > MAX_TOLD {
        let drop_n = next_told.len() - MAX_TOLD;
        next_told.drain(..drop_n);
    }
    let letter = AwayLetter {
        v: AWAY_LETTER_VERSION,
        kind: AwayLetterKind::Look,
        at: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        repo: repo.to_owned(),
        read: true,
        found: vec![finding_of(&fact)],
        prs: before.as_ref().map(|b| b.prs.clone()).unwrap_or_default(),
        told: next_told,
        brief_day: before.and_then(|b| b.brief_day),
    };
    let (_, hour) = local_time(now, &config.tz);
    let quiet = quiet_at(config.quiet, hour);
    let Ok(boxed) = seal_away_letter(&config.key, &letter, &config.room, AwayLetterFrom::Away) else {
        return AwayLookResult {
            said: "Locust could not seal your ask.".into(),
            ok: false,
        };
    };
    let mut payload = json!({ "n": boxed.n, "c": boxed.c });
    if !quiet {
        payload["alert"] = json!("found");
    }
    let posted = match fetch.fetch(&base, "POST", Some(&payload.to_string())) {
        Ok(r) => r,
        Err(_) => {
            return AwayLookResult {
                said: "Locust’s relay could not be reached to pass your ask on.".into(),
                ok: false,
            };
        }
    };
    if !(200..300).contains(&posted.status) {
        return AwayLookResult {
            said: format!(
                "Locust’s relay answered {}, so your ask was not passed on.",
                posted.status
            ),
            ok: false,
        };
    }
    let rung = serde_json::from_str::<Value>(&posted.body)
        .ok()
        .and_then(|v| v.get("rung").and_then(Value::as_u64))
        .unwrap_or(0);
    let tail = if rung > 0 {
        format!(" It rang {rung} phone{}.", if rung == 1 { "" } else { "s" })
    } else if quiet {
        " Quiet hours: no phone was rung.".into()
    } else {
        String::new()
    };
    AwayLookResult {
        said: format!("Scout passed your ask on.{tail}"),
        ok: true,
    }
}

// ---- Workflow & ScoutAway (main/scout-away.ts) ----------------------------------------------------------------------

pub fn away_workflow(every: u8) -> String {
    let cron_hour = if every == 1 { "*" } else { "*/2" };
    format!(
        "# Scout's away look, from Locust (https://github.com/StopBanning/locust).
# While your computer is off, Scout looks at this repository's pull requests, reviews, checks and
# issues for you, and leaves what it found at Locust's relay, sealed with a key only your devices
# have, so your phone can show it. With Locust open on your computer, it does nothing more.
# A comment of yours that starts a line with /locust goes to your phone as you write it; anyone
# else's comment ends the run at once. It reads with this workflow's own read-only token. To stop
# it, turn it off in Locust, or delete this file, .github/locust/scout-away.mjs and the LOCUST_SCOUT secret.
name: {AWAY_WORKFLOW_NAME}
on:
  schedule:
    - cron: '23 {cron_hour} * * *'
  issue_comment:
    types: [created]
  workflow_dispatch:
permissions:
  contents: read
  pull-requests: read
  issues: read
  checks: read
  statuses: read
  actions: read
concurrency:
  group: locust-scout${{{{ github.event_name == 'issue_comment' && format('-ask-{{0}}', github.event.comment.id) || '' }}}}
  cancel-in-progress: true
jobs:
  look:
    if: github.event_name != 'issue_comment' || contains(github.event.comment.body, '/locust')
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - name: Look for you
        env:
          LOCUST_SCOUT: ${{{{ secrets.LOCUST_SCOUT }}}}
          GH_TOKEN: ${{{{ github.token }}}}
          GH_REPO: ${{{{ github.repository }}}}
        run: |
          gh api \"repos/$GH_REPO/contents/{AWAY_SCRIPT_PATH}?ref=$GITHUB_SHA\" -H 'Accept: application/vnd.github.raw' > \"$RUNNER_TEMP/scout-away.mjs\"
          node \"$RUNNER_TEMP/scout-away.mjs\"
"
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScoutAwayStatus {
    Off {
        blocked: Option<String>,
        message: Option<String>,
    },
    Cleaning { message: String },
    Waiting {
        repo: String,
        pr: String,
        message: Option<String>,
    },
    On {
        repo: String,
        every: u8,
        private: bool,
        last_look: Option<String>,
        outdated: bool,
        update: Option<String>,
        message: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScoutAwaySet {
    On,
    Off,
    Update,
}

pub trait ScoutAwayRelay: Send + Sync {
    fn room(&self) -> Option<(String, String)>;
    fn connected(&self) -> bool;
    fn away_open(&self, hash: &str) -> bool;
    fn away_close(&self) -> bool;
    fn keep_away_alerts(&self) -> bool;
    fn post(&self, n: &str, c: &str) -> bool;
    fn letters(&self, _after: Option<&str>) -> Option<crate::phone_anywhere::LettersPage> { None }
}

pub trait ScoutAwayGh: Send + Sync {
    fn run(&self, args: &[&str], cwd: &str, input: Option<&str>) -> Result<String, String>;
}

pub struct ScoutAwayDeps {
    pub file: PathBuf,
    pub folder: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    pub gh: Arc<dyn ScoutAwayGh>,
    pub relay: Arc<dyn Fn() -> Option<Arc<dyn ScoutAwayRelay>> + Send + Sync>,
    pub script: Arc<dyn Fn() -> String + Send + Sync>,
    pub tz: Arc<dyn Fn() -> String + Send + Sync>,
    pub now: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

#[derive(Clone, Debug)]
struct SavedPr {
    url: String,
    number: u64,
}

#[derive(Clone, Debug)]
struct SavedAway {
    repo: String,
    folder: String,
    login: String,
    key: String,
    token: String,
    room: String,
    every: u8,
    private: bool,
    branch: String,
    pr: Option<SavedPr>,
    merged: bool,
    last_look: Option<String>,
    cursor: Option<String>,
    told: Option<String>,
    cleanup: Option<AwayCleanup>,
    script: String,
    workflow: Option<String>,
    update: Option<SavedPr>,
}

#[derive(Clone, Debug, Default)]
struct AwayCleanup {
    relay: bool,
    secret: bool,
    workflow: bool,
}

impl SavedAway {
    fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("v".into(), json!(1));
        obj.insert("repo".into(), json!(self.repo));
        obj.insert("folder".into(), json!(self.folder));
        obj.insert("login".into(), json!(self.login));
        obj.insert("key".into(), json!(self.key));
        obj.insert("token".into(), json!(self.token));
        obj.insert("room".into(), json!(self.room));
        obj.insert("every".into(), json!(self.every));
        obj.insert("private".into(), json!(self.private));
        obj.insert("branch".into(), json!(self.branch));
        if let Some(pr) = &self.pr {
            obj.insert("pr".into(), json!({ "url": pr.url, "number": pr.number }));
        }
        obj.insert("merged".into(), json!(self.merged));
        if let Some(ll) = &self.last_look {
            obj.insert("lastLook".into(), json!(ll));
        }
        if let Some(cursor) = &self.cursor {
            obj.insert("cursor".into(), json!(cursor));
        }
        if let Some(told) = &self.told {
            obj.insert("told".into(), json!(told));
        }
        if let Some(cleanup) = &self.cleanup {
            obj.insert("cleanup".into(), json!({
                "relay": cleanup.relay, "secret": cleanup.secret, "workflow": cleanup.workflow,
            }));
        }
        obj.insert("script".into(), json!(self.script));
        if let Some(wf) = &self.workflow {
            obj.insert("workflow".into(), json!(wf));
        }
        if let Some(up) = &self.update {
            obj.insert("update".into(), json!({ "url": up.url, "number": up.number }));
        }
        Value::Object(obj)
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        if obj.get("v").and_then(Value::as_u64) != Some(1) {
            return None;
        }
        let parse_pr = |k: &str| {
            obj.get(k).and_then(Value::as_object).and_then(|p| {
                Some(SavedPr {
                    url: p.get("url")?.as_str()?.to_owned(),
                    number: p.get("number")?.as_u64()?,
                })
            })
        };
        let repo = obj.get("repo")?.as_str()?;
        if !repo_re().is_match(repo) { return None; }
        Some(Self {
            repo: repo.to_owned(),
            folder: obj.get("folder").and_then(Value::as_str).unwrap_or("").to_owned(),
            login: obj.get("login").and_then(Value::as_str).unwrap_or("").to_owned(),
            key: obj.get("key")?.as_str()?.to_owned(),
            token: obj.get("token")?.as_str()?.to_owned(),
            room: obj.get("room")?.as_str()?.to_owned(),
            every: obj.get("every").and_then(Value::as_u64).unwrap_or(1) as u8,
            private: obj.get("private").and_then(Value::as_bool).unwrap_or(false),
            branch: obj.get("branch").and_then(Value::as_str).unwrap_or("main").to_owned(),
            pr: parse_pr("pr"),
            merged: obj.get("merged").and_then(Value::as_bool).unwrap_or(false),
            last_look: obj.get("lastLook").and_then(Value::as_str).map(str::to_owned),
            cursor: obj.get("cursor").and_then(Value::as_str).map(str::to_owned),
            told: obj.get("told").and_then(Value::as_str).map(str::to_owned),
            cleanup: obj.get("cleanup").map(|value| AwayCleanup {
                relay: value["relay"] == true,
                secret: value["secret"] == true,
                workflow: value["workflow"] == true,
            }),
            script: obj.get("script").and_then(Value::as_str).unwrap_or("").to_owned(),
            workflow: obj.get("workflow").and_then(Value::as_str).map(str::to_owned),
            update: parse_pr("update"),
        })
    }
}

pub struct ScoutAway {
    deps: ScoutAwayDeps,
    state: Mutex<ScoutAwayState>,
    scout: Option<std::sync::Weak<crate::scout::Scout>>,
    // Kept apart from the state mutex: phone request handlers must not wait on a worker
    // that is itself waiting for the receiver to deliver the relay's answer.
    phone: Mutex<Option<(String, crate::phone_anywhere::AwayForPhone)>>,
}

struct ScoutAwayState {
    loaded: bool,
    saved: Option<SavedAway>,
    message: Option<String>,
    carried: Option<String>,
}

impl ScoutAway {
    pub fn new(deps: ScoutAwayDeps) -> Self {
        Self {
            deps,
            state: Mutex::new(ScoutAwayState {
                loaded: false,
                saved: None,
                message: None,
                carried: None,
            }),
            scout: None,
            phone: Mutex::new(None),
        }
    }

    pub fn with_scout(mut self, scout: &Arc<crate::scout::Scout>) -> Self {
        self.scout = Some(Arc::downgrade(scout));
        self
    }

    pub fn for_phone(&self) -> Option<crate::phone_anywhere::AwayForPhone> {
        self.phone.lock().unwrap().as_ref().map(|(_, info)| info.clone())
    }

    pub fn for_phone_in_room(&self, room: &str) -> Option<crate::phone_anywhere::AwayForPhone> {
        self.phone.lock().unwrap().as_ref()
            .filter(|(saved, _)| saved == room).map(|(_, info)| info.clone())
    }

    fn phone_snapshot(&self, saved: Option<&SavedAway>) {
        *self.phone.lock().unwrap() = saved.filter(|held| held.cleanup.is_none()).map(|held| (held.room.clone(), crate::phone_anywhere::AwayForPhone {
            key: held.key.clone(),
            repo: held.repo.clone(),
        }));
    }

    fn load_inner(&self, st: &mut ScoutAwayState) {
        if st.loaded { return; }
        st.loaded = true;
        st.saved = fs::read_to_string(&self.deps.file).ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| SavedAway::from_json(&value));
        self.phone_snapshot(st.saved.as_ref());
    }

    fn persist_inner(&self, st: &mut ScoutAwayState, next: Option<SavedAway>) -> bool {
        let saved = match &next {
            None => fs::remove_file(&self.deps.file).or_else(|error|
                if error.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(error) }),
            Some(held) => (|| {
                use std::io::Write;
                if let Some(parent) = self.deps.file.parent() { fs::create_dir_all(parent)?; }
                let tmp = self.deps.file.with_extension(format!("tmp-{}", random_text(8)));
                let mut options = fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let saved = (|| {
                    let mut file = options.open(&tmp)?;
                    file.write_all(held.to_json().to_string().as_bytes())?;
                    file.sync_all()?;
                    drop(file);
                    fs::rename(&tmp, &self.deps.file)
                })();
                if saved.is_err() { let _ = fs::remove_file(&tmp); }
                saved
            })(),
        };
        if saved.is_err() {
            st.message = Some("Scout could not save the away look’s settings.".into());
            return false;
        }
        self.phone_snapshot(next.as_ref());
        st.saved = next;
        true
    }

    /// Read on a worker, not in a phone receive callback. Persist only after delivering the page.
    pub fn catch_up(&self) {
        let mut st = self.state.lock().unwrap();
        self.load_inner(&mut st);
        let Some(mut held) = st.saved.clone().filter(|held| held.cleanup.is_none()) else { return };
        let Some(scout) = self.scout.as_ref().and_then(std::sync::Weak::upgrade) else { return };
        let Some(relay) = (self.deps.relay)() else { return };
        if !relay.connected() || relay.room().map(|(_, room)| room).as_deref() != Some(held.room.as_str()) {
            return;
        }
        let mut cursor = held.cursor.clone();
        let mut looks = Vec::new();
        for _ in 0..10 {
            let Some(page) = relay.letters(cursor.as_deref()) else { break };
            let before = cursor.clone();
            for item in page.items {
                cursor = Some(item.id);
                if item.from != "away" { continue; }
                let boxed = SealedAwayBox { n: item.n, c: item.c };
                if let Some(letter) = open_away_letter(&held.key, &boxed, &held.room, AwayLetterFrom::Away)
                    && letter.kind == AwayLetterKind::Look
                    && letter.repo.eq_ignore_ascii_case(&held.repo)
                {
                    looks.push(letter);
                }
            }
            if !page.more || cursor == before { break; }
        }
        if !looks.is_empty() {
            if scout.absorb_saved(&looks).is_err() {
                st.message = Some("Scout could not save the away look. It will read it again.".into());
                return;
            }
            held.last_look = looks.last().map(|letter| letter.at.clone());
        }
        if cursor != held.cursor {
            held.cursor = cursor;
            self.persist_inner(&mut st, Some(held));
        }
    }

    /// Put the desktop's current receipt on the encrypted relay, only after a change.
    pub fn leave_told(&self) {
        let mut st = self.state.lock().unwrap();
        self.load_inner(&mut st);
        let Some(mut held) = st.saved.clone().filter(|held| held.cleanup.is_none()) else { return };
        let Some(scout) = self.scout.as_ref().and_then(std::sync::Weak::upgrade) else { return };
        let Some(relay) = (self.deps.relay)() else { return };
        if !relay.connected() || relay.room().map(|(_, room)| room).as_deref() != Some(held.room.as_str()) {
            return;
        }
        let mut letter = scout.away_receipt(&held.repo);
        let mark = sha256_b64url(&letter.to_json().to_string());
        if held.told.as_deref() == Some(mark.as_str()) { return; }
        letter.at = (self.deps.now)().to_rfc3339();
        if let Ok(boxed) = seal_away_letter(&held.key, &letter, &held.room, AwayLetterFrom::Pc)
            && relay.post(&boxed.n, &boxed.c)
        {
            held.told = Some(mark);
            self.persist_inner(&mut st, Some(held));
        }
    }

    pub fn refresh(&self) {
        self.catch_up();
        self.leave_told();
        if let Some(relay) = (self.deps.relay)().filter(|relay| relay.connected())
            && self.for_phone().is_some()
        {
            let _ = relay.keep_away_alerts();
        }
    }

    fn remove_remote(&self, held: &SavedAway) -> bool {
        if let Some(relay) = (self.deps.relay)()
            && relay.room().map(|(_, room)| room).as_deref() == Some(held.room.as_str())
        {
            let _ = relay.away_close();
        }
        let folder = (self.deps.folder)().unwrap_or_else(|| held.folder.clone());
        let secret = self.deps.gh.run(&["secret", "delete", AWAY_SECRET, "--repo", &held.repo], &folder, None);
        let end = if held.merged {
            self.deps.gh.run(&["workflow", "disable", AWAY_WORKFLOW_NAME, "--repo", &held.repo], &folder, None)
        } else if let Some(pr) = &held.pr {
            self.deps.gh.run(&["pr", "close", &pr.number.to_string(), "--repo", &held.repo, "--delete-branch"], &folder, None)
        } else {
            Ok(String::new())
        };
        secret.is_ok() && end.is_ok()
    }

    fn turn_off(&self, st: &mut ScoutAwayState, mut held: SavedAway, room_gone: bool) -> bool {
        let mut progress = held.cleanup.clone().unwrap_or_default();
        progress.relay |= room_gone;
        held.cleanup = Some(progress.clone());
        // Keep enough state to finish cleanup after restart, and stop exposing the
        // key before attempting any external operation.
        if !self.persist_inner(st, Some(held.clone())) { return false; }
        if !progress.relay
            && let Some(relay) = (self.deps.relay)()
        {
            match relay.room() {
                Some((_, room)) if room == held.room => progress.relay = relay.away_close(),
                _ => {}
            }
        }
        let folder = (self.deps.folder)().unwrap_or_else(|| held.folder.clone());
        if !progress.secret {
            progress.secret = self.deps.gh.run(&["secret", "delete", AWAY_SECRET, "--repo", &held.repo], &folder, None).is_ok();
        }
        if !progress.workflow {
            progress.workflow = if held.merged {
                self.deps.gh.run(&["workflow", "disable", AWAY_WORKFLOW_NAME, "--repo", &held.repo], &folder, None).is_ok()
            } else if let Some(pr) = &held.pr {
                self.deps.gh.run(&["pr", "close", &pr.number.to_string(), "--repo", &held.repo, "--delete-branch"], &folder, None).is_ok()
            } else { true };
        }
        if progress.relay && progress.secret && progress.workflow {
            return self.persist_inner(st, None);
        }
        held.cleanup = Some(progress);
        self.persist_inner(st, Some(held));
        st.message = Some("Scout stopped using the away look here. Try turning it off again to finish removing it from the relay and GitHub.".into());
        false
    }

    /// Called when the user turns the phone off, never on a transient disconnect.
    /// If the phone still has a closing record, retain the Scout cleanup until its
    /// relay reset is confirmed instead of losing the room reference.
    pub fn phone_gone(&self, room_gone: bool) -> ScoutAwayStatus {
        let mut st = self.state.lock().unwrap();
        self.load_inner(&mut st);
        if let Some(held) = st.saved.clone()
            && self.turn_off(&mut st, held, room_gone)
        {
            st.message = Some("Your phones were turned off, so Scout stopped looking while your computer is off. Turn it on again here.".into());
        }
        self.status_from(&st)
    }

    fn know_carried(&self, st: &mut ScoutAwayState) {
        if st.carried.is_none() {
            st.carried = Some(sha256_b64url(&(self.deps.script)()));
        }
    }

    fn is_outdated(&self, st: &ScoutAwayState, held: &SavedAway) -> bool {
        let Some(carried) = &st.carried else {
            return false;
        };
        if carried == &sha256_b64url("") {
            return false;
        }
        held.script != *carried
            || held
                .workflow
                .as_ref()
                .is_some_and(|w| w != &sha256_b64url(&away_workflow(held.every)))
    }

    fn status_from(&self, st: &ScoutAwayState) -> ScoutAwayStatus {
        let Some(held) = &st.saved else {
            let blocked = if (self.deps.relay)().and_then(|r| r.room()).is_none() {
                Some("Turn on “Your phone, from anywhere” first: Scout leaves what it finds there.".into())
            } else if (self.deps.folder)().is_none() {
                Some("Open a project folder that is on GitHub first.".into())
            } else {
                None
            };
            return ScoutAwayStatus::Off {
                blocked,
                message: st.message.clone(),
            };
        };
        if held.cleanup.is_some() {
            return ScoutAwayStatus::Cleaning { message: st.message.clone().unwrap_or_else(||
                "Scout stopped using the away look here. Finish turning it off to remove what remains.".into()) };
        }
        if !held.merged {
            return ScoutAwayStatus::Waiting {
                repo: held.repo.clone(),
                pr: held
                    .pr
                    .as_ref()
                    .map(|p| p.url.clone())
                    .unwrap_or_else(|| format!("https://github.com/{}/pulls", held.repo)),
                message: st.message.clone(),
            };
        }
        let outdated = held.update.is_none() && self.is_outdated(st, held);
        ScoutAwayStatus::On {
            repo: held.repo.clone(),
            every: held.every,
            private: held.private,
            last_look: held.last_look.clone(),
            outdated,
            update: held.update.as_ref().map(|u| u.url.clone()),
            message: st.message.clone(),
        }
    }

    fn on_default(&self, held: &SavedAway, workflow: &str, script: &str) -> bool {
        let folder = (self.deps.folder)().unwrap_or_else(|| held.folder.clone());
        let read = |path: &str| {
            let api = format!("repos/{}/contents/{path}?ref={}", held.repo, held.branch);
            self.deps
                .gh
                .run(&["api", &api, "-H", "Accept: application/vnd.github.raw"], &folder, None)
                .ok()
        };
        let had_wf = read(AWAY_WORKFLOW_PATH).map(|s| s.replace("\r\n", "\n"));
        let had_sc = read(AWAY_SCRIPT_PATH).map(|s| s.replace("\r\n", "\n"));
        had_wf.as_deref() == Some(workflow) && had_sc.as_deref() == Some(script)
    }

    fn merged_yet(&self, held: &SavedAway) -> Option<bool> {
        let folder = (self.deps.folder)().unwrap_or_else(|| held.folder.clone());
        let api = format!(
            "repos/{}/contents/{AWAY_WORKFLOW_PATH}?ref={}",
            held.repo, held.branch
        );
        if self.deps.gh.run(&["api", &api, "--jq", ".sha"], &folder, None).is_ok() {
            return Some(true);
        }
        let pr = held.pr.as_ref()?;
        let pr_api = format!("repos/{}/pulls/{}", held.repo, pr.number);
        if let Ok(state) = self.deps.gh.run(&["api", &pr_api, "--jq", ".state"], &folder, None)
            && state.trim() == "closed"
        {
            return Some(false); // closed without merging
        }
        None
    }

    fn open_pr(
        &self,
        repo: &str,
        branch: &str,
        folder: &str,
        workflow: &str,
        script: &str,
        title: &str,
        message: &str,
        body: &str,
    ) -> Result<SavedPr, String> {
        let ref_api = format!("repos/{repo}/git/ref/heads/{branch}");
        let base_sha = self
            .deps
            .gh
            .run(&["api", &ref_api, "--jq", ".object.sha"], folder, None)?
            .trim()
            .to_owned();
        let commit_api = format!("repos/{repo}/git/commits/{base_sha}");
        let base_json: Value =
            serde_json::from_str(&self.deps.gh.run(&["api", &commit_api], folder, None)?)
                .map_err(|e| e.to_string())?;
        let base_tree = base_json["tree"]["sha"].as_str().unwrap_or("").to_owned();
        let trees_api = format!("repos/{repo}/git/trees");
        let tree_body = json!({
            "base_tree": base_tree,
            "tree": [
                { "path": AWAY_WORKFLOW_PATH, "mode": "100644", "type": "blob", "content": workflow },
                { "path": AWAY_SCRIPT_PATH, "mode": "100644", "type": "blob", "content": script },
            ]
        })
        .to_string();
        let tree_json: Value = serde_json::from_str(&self.deps.gh.run(
            &["api", "-X", "POST", &trees_api, "--input", "-"],
            folder,
            Some(&tree_body),
        )?)
        .map_err(|e| e.to_string())?;
        let tree_sha = tree_json["sha"].as_str().unwrap_or("").to_owned();
        let commits_api = format!("repos/{repo}/git/commits");
        let commit_body = json!({ "message": message, "tree": tree_sha, "parents": [base_sha] }).to_string();
        let new_commit: Value = serde_json::from_str(&self.deps.gh.run(
            &["api", "-X", "POST", &commits_api, "--input", "-"],
            folder,
            Some(&commit_body),
        )?)
        .map_err(|e| e.to_string())?;
        let new_sha = new_commit["sha"].as_str().unwrap_or("").to_owned();
        let refs_api = format!("repos/{repo}/git/refs");
        let ref_name = format!("refs/heads/{AWAY_BRANCH}");
        let ref_body = json!({ "ref": ref_name, "sha": new_sha }).to_string();
        if self
            .deps
            .gh
            .run(&["api", "-X", "POST", &refs_api, "--input", "-"], folder, Some(&ref_body))
            .is_err()
        {
            let patch_api = format!("repos/{repo}/git/{ref_name}");
            let patch_body = json!({ "sha": new_sha, "force": true }).to_string();
            let _ = self.deps.gh.run(
                &["api", "-X", "PATCH", &patch_api, "--input", "-"],
                folder,
                Some(&patch_body),
            );
        }
        let pulls_api = format!("repos/{repo}/pulls");
        let pr_body = json!({ "title": title, "head": AWAY_BRANCH, "base": branch, "body": body }).to_string();
        let pr_json: Value = serde_json::from_str(&self.deps.gh.run(
            &["api", "-X", "POST", &pulls_api, "--input", "-", "--jq", "{url: .html_url, number: .number}"],
            folder,
            Some(&pr_body),
        )?)
        .map_err(|e| e.to_string())?;
        let number = pr_json["number"].as_u64().filter(|number| *number > 0)
            .ok_or_else(|| "GitHub did not return a pull request number.".to_owned())?;
        let url = format!("https://github.com/{repo}/pull/{number}");
        if !pr_json["url"].as_str().is_some_and(|reported| reported.eq_ignore_ascii_case(&url)) {
            return Err("GitHub did not return this repository’s pull request.".into());
        }
        Ok(SavedPr { url, number })
    }

    pub fn status(&self, check: bool) -> ScoutAwayStatus {
        let mut st = self.state.lock().unwrap();
        self.load_inner(&mut st);
        self.know_carried(&mut st);
        let Some(held) = st.saved.clone() else {
            return self.status_from(&st);
        };
        if held.cleanup.is_some() { return self.status_from(&st); }
        if check && let Some(up) = &held.update {
            let script = (self.deps.script)();
            let workflow = away_workflow(held.every);
            if self.on_default(&held, &workflow, &script) {
                let mut next = held;
                next.update = None;
                next.script = sha256_b64url(&script);
                next.workflow = Some(sha256_b64url(&workflow));
                st.message = None;
                self.persist_inner(&mut st, Some(next));
            } else {
                let folder = (self.deps.folder)().unwrap_or_else(|| held.folder.clone());
                let pr_api = format!("repos/{}/pulls/{}", held.repo, up.number);
                if let Ok(state) = self.deps.gh.run(&["api", &pr_api, "--jq", ".state"], &folder, None)
                    && state.trim() == "closed"
                {
                    let mut next = held;
                    next.update = None;
                    st.message = Some(
                        "The update’s pull request was closed without merging, so the away look goes on as it was.".into(),
                    );
                    self.persist_inner(&mut st, Some(next));
                }
            }
            return self.status_from(&st);
        }
        if check && !held.merged {
            match self.merged_yet(&held) {
                Some(true) => {
                    let mut next = held;
                    next.merged = true;
                    st.message = None;
                    self.persist_inner(&mut st, Some(next));
                }
                Some(false) => {
                    if self.turn_off(&mut st, SavedAway { pr: None, ..held }, false) {
                        st.message = Some("The pull request was closed without merging, so Scout does not look while your computer is off.".into());
                    }
                }
                None => {}
            }
        }
        self.status_from(&st)
    }

    pub fn set(&self, action: ScoutAwaySet) -> ScoutAwayStatus {
        let mut st = self.state.lock().unwrap();
        self.load_inner(&mut st);
        self.know_carried(&mut st);
        match action {
            ScoutAwaySet::Update => {
                if let Some(held) = st.saved.clone()
                    && held.merged
                    && held.cleanup.is_none()
                    && held.update.is_none()
                    && self.is_outdated(&st, &held)
                {
                    let folder = (self.deps.folder)().unwrap_or_else(|| held.folder.clone());
                    let script = (self.deps.script)();
                    let workflow = away_workflow(held.every);
                    if self.on_default(&held, &workflow, &script) {
                        let mut next = held;
                        next.script = sha256_b64url(&script);
                        next.workflow = Some(sha256_b64url(&workflow));
                        st.message = None;
                        self.persist_inner(&mut st, Some(next));
                    } else if let Ok(pr) = self.open_pr(
                        &held.repo,
                        &held.branch,
                        &folder,
                        &workflow,
                        &script,
                        "Scout’s away look, updated from Locust",
                        "Update Scout's away look from Locust",
                        "Locust has a newer away look for this repository.",
                    ) {
                        let mut next = held;
                        next.update = Some(pr);
                        st.message = None;
                        self.persist_inner(&mut st, Some(next));
                    }
                }
            }
            ScoutAwaySet::On => {
                if st.saved.is_some() {
                    return self.status_from(&st);
                }
                let Some(relay) = (self.deps.relay)() else {
                    return self.status_from(&st);
                };
                let Some((relay_url, room)) = relay.room() else {
                    return self.status_from(&st);
                };
                let Some(folder) = (self.deps.folder)() else {
                    return self.status_from(&st);
                };
                if !relay.connected() {
                    st.message = Some("Locust’s relay cannot be reached right now. Try again in a minute.".into());
                    return self.status_from(&st);
                }
                let Ok(repo_raw) = self.deps.gh.run(
                    &["repo", "view", "--json", "nameWithOwner,visibility,defaultBranchRef,viewerPermission"],
                    &folder,
                    None,
                ) else {
                    st.message = Some("This folder is not on GitHub, or GitHub’s CLI is not signed in here (Settings > Accounts).".into());
                    return self.status_from(&st);
                };
                let Ok(repo_json) = serde_json::from_str::<Value>(&repo_raw) else {
                    return self.status_from(&st);
                };
                let repo_name = repo_json["nameWithOwner"].as_str().unwrap_or("").to_owned();
                if !repo_re().is_match(&repo_name) {
                    st.message = Some("GitHub did not name a usable repository.".into());
                    return self.status_from(&st);
                }
                if repo_json["viewerPermission"].as_str() != Some("ADMIN") {
                    st.message = Some(format!("Only an owner or admin of {repo_name} can add the secret the away look needs."));
                    return self.status_from(&st);
                }
                let is_private = repo_json["visibility"].as_str() != Some("PUBLIC");
                let branch = repo_json["defaultBranchRef"]["name"].as_str().unwrap_or("main").to_owned();
                let every: u8 = if is_private { 2 } else { 1 };
                let login = self
                    .deps
                    .gh
                    .run(&["api", "user", "--jq", ".login"], &folder, None)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let token = random_text(32);
                let key = new_away_key();
                let script = (self.deps.script)();
                if script.trim().is_empty() || !(1..=39).contains(&login.len())
                    || !login.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                {
                    st.message = Some("Scout’s away program or GitHub login is unavailable.".into());
                    return self.status_from(&st);
                }
                let workflow = away_workflow(every);
                if !relay.away_open(&sha256_b64url(&token)) {
                    st.message = Some("The relay did not take the away look. Try again in a minute.".into());
                    return self.status_from(&st);
                }
                let (quiet, brief_at) = self.scout.as_ref().and_then(std::sync::Weak::upgrade)
                    .map_or((None, None), |scout| {
                        let settings = scout.state().settings;
                        (settings.quiet, settings.brief_at)
                    });
                let cfg = AwayConfig {
                    v: 1,
                    relay: relay_url,
                    room: room.clone(),
                    token: token.clone(),
                    key: key.clone(),
                    login: login.clone(),
                    tz: (self.deps.tz)(),
                    quiet,
                    brief_at,
                };
                if self.deps.gh.run(
                    &["secret", "set", AWAY_SECRET, "--repo", &repo_name],
                    &folder,
                    Some(&cfg.to_json().to_string()),
                ).is_err() {
                    let _ = relay.away_close();
                    st.message = Some("GitHub did not save the away look’s secret.".into());
                    return self.status_from(&st);
                }
                let draft = SavedAway {
                    repo: repo_name.clone(),
                    folder: folder.clone(),
                    login,
                    key,
                    token,
                    room,
                    every,
                    private: is_private,
                    branch: branch.clone(),
                    pr: None,
                    merged: false,
                    last_look: None,
                    cursor: None,
                    told: None,
                    cleanup: None,
                    script: sha256_b64url(&script),
                    workflow: Some(sha256_b64url(&workflow)),
                    update: None,
                };
                let cadence = if every == 1 { "every hour" } else { "every two hours" };
                let cost = if is_private {
                    "about 360 of the 2,000 free Actions minutes a private repository gets each month"
                } else { "nothing: Actions are free on public repositories" };
                let body = format!(
                    "Scout looks at this repository while your computer is off.\n\n- `{AWAY_WORKFLOW_PATH}` runs `{AWAY_SCRIPT_PATH}` {cadence}, with a read-only GitHub token.\n- Your own /locust comments can reach your phone. Results are sealed for your devices.\n- Cost: {cost}.\n- Turn it off in Locust to remove the secret and disable the workflow."
                );
                if self.on_default(&draft, &workflow, &script) {
                    let mut merged = draft;
                    merged.merged = true;
                    if self.deps.gh.run(&["workflow", "enable", AWAY_WORKFLOW_NAME, "--repo", &repo_name], &folder, None).is_err() {
                        let _ = self.remove_remote(&SavedAway { merged: false, ..merged });
                        st.message = Some("GitHub did not enable the away workflow.".into());
                        return self.status_from(&st);
                    }
                    if !self.persist_inner(&mut st, Some(merged.clone())) {
                        let _ = self.remove_remote(&merged);
                    }
                } else if let Ok(pr) = self.open_pr(
                    &repo_name,
                    &branch,
                    &folder,
                    &workflow,
                    &script,
                    "Scout’s away look, from Locust",
                    "Add Scout's away look from Locust",
                    &body,
                ) {
                    let mut waiting = draft;
                    waiting.pr = Some(pr);
                    if !self.persist_inner(&mut st, Some(waiting.clone())) {
                        let _ = self.remove_remote(&waiting);
                    }
                } else {
                    let _ = self.remove_remote(&draft);
                    st.message = Some("GitHub did not open the pull request for the away look.".into());
                }
                let _ = relay.keep_away_alerts();
            }
            ScoutAwaySet::Off => {
                if let Some(held) = st.saved.clone() {
                    if self.turn_off(&mut st, held, false) {
                        st.message = None;
                    }
                }
            }
        }
        self.status_from(&st)
    }
}

#[cfg(test)]
mod sealed_result_tests {
    use super::*;

    // case: takes a letter’s links only to its own repository on GitHub, and a look only from the away look
    #[test]
    fn sealed_look_opens_only_for_its_room_and_sender_and_keeps_repo_links() {
        let key = new_away_key();
        let room = "RRRRRRRRRRRRRRRRRRRRRR";
        let repo = "acme/app";
        let good = "https://github.com/acme/app/issues/1";
        let look = AwayLetter {
            v: AWAY_LETTER_VERSION,
            kind: AwayLetterKind::Look,
            at: "2026-10-23T14:00:00Z".into(),
            repo: repo.into(),
            read: true,
            found: [
                good,
                "https://github.com/other/repo/issues/1",
                "https://evil.example/acme/app",
                "https://github.com/acme/app.evil/x",
                "javascript:alert(1)//https://github.com/acme/app/",
            ].into_iter().enumerate().map(|(idx, url)| AwayFinding {
                key: format!("finding:{idx}"),
                urgency: ScoutUrgency::Now,
                said: "Synthetic finding".into(),
                detail: None,
                lines: None,
                chips: None,
                url: Some(url.into()),
            }).collect(),
            prs: vec![],
            told: vec![],
            brief_day: None,
        };
        let sealed = seal_away_letter(&key, &look, room, AwayLetterFrom::Away).unwrap();
        let opened = open_away_letter(&key, &sealed, room, AwayLetterFrom::Away).unwrap();
        let links: Vec<_> = opened.found.iter().map(|finding| finding.url.as_deref()).collect();
        assert_eq!(links, [Some(good), None, None, None, None]);
        assert!(open_away_letter(&key, &sealed, "different-room", AwayLetterFrom::Away).is_none());
        assert!(open_away_letter(&key, &sealed, room, AwayLetterFrom::Pc).is_none());

        // A correctly encrypted packet still cannot mislabel its letter kind.
        let as_pc = seal_away_letter(&key, &look, room, AwayLetterFrom::Pc).unwrap();
        assert!(open_away_letter(&key, &as_pc, room, AwayLetterFrom::Pc).is_none());
        let told = AwayLetter { kind: AwayLetterKind::Told, ..look };
        let as_away = seal_away_letter(&key, &told, room, AwayLetterFrom::Away).unwrap();
        assert!(open_away_letter(&key, &as_away, room, AwayLetterFrom::Away).is_none());
    }
}

#[cfg(test)]
#[path = "scout_away_workflow_tests.rs"]
mod scout_away_workflow_tests;
