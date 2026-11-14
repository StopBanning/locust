//! Persistent storage for `scout.json` and `scout-looks.json`, guarding unreadable files
//! against overwrite and keeping all writes native-only when backed by a `Store`.
//!
//! Port of the storage layer in Electron's `main/scout.ts`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use serde_json::{Value, json};

use crate::pull_requests::{FolderPullRequest, PullRequestChecks, PullRequestReview, PullRequestState};
use crate::store::Store;

use super::comments::{CommentTurn, CommentWatch};
use super::log::{ScoutLogEntry, scout_log_from};
use super::types::{
    LastCheck, MAX_SCOUT_SEEN, ScoutCard, ScoutSettings, ScoutState, ScoutToday, ScoutWeek,
    iso_string, notes_from, regex_day, scout_day, scout_settings_from, week_from,
};

pub const SCOUT_FILE: &str = "scout.json";
pub const SCOUT_LOG_FILE: &str = "scout-looks.json";
pub const MAX_LOOK_IDS: usize = 200;
pub const MAX_REPEATED: usize = 500;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SinceBriefRaised {
    pub github: u32,
    pub teammates: u32,
    pub comments: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SinceBrief {
    pub looks: u32,
    pub github_missed: u32,
    pub raised: SinceBriefRaised,
    pub repeated: Vec<String>,
}

impl SinceBrief {
    pub fn to_json(&self) -> Value {
        json!({
            "looks": self.looks,
            "githubMissed": self.github_missed,
            "raised": {
                "github": self.raised.github,
                "teammates": self.raised.teammates,
                "comments": self.raised.comments,
            },
            "repeated": self.repeated,
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let looks = obj.get("looks")?.as_u64().and_then(|n| u32::try_from(n).ok())?;
        let github_missed = obj
            .get("githubMissed")?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())?;
        let raised = obj.get("raised")?.as_object()?;
        let github = raised
            .get("github")?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())?;
        let teammates = raised
            .get("teammates")?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())?;
        let comments = raised
            .get("comments")?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())?;
        let repeated_arr = obj.get("repeated")?.as_array()?;
        let mut repeated: Vec<String> = repeated_arr
            .iter()
            .filter_map(|k| k.as_str().map(str::to_owned))
            .collect();
        if repeated.len() > MAX_REPEATED {
            let start = repeated.len() - MAX_REPEATED;
            repeated.drain(..start);
        }
        Some(Self {
            looks,
            github_missed,
            raised: SinceBriefRaised {
                github,
                teammates,
                comments,
            },
            repeated,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    pub comments: Option<CommentWatch>,
    pub comment_turns: HashMap<String, CommentTurn>,
    pub settings: ScoutSettings,
    pub cards: Vec<ScoutCard>,
    pub seen: Vec<String>,
    pub look_ids: Vec<String>,
    pub folder_mark: Option<String>,
    pub last_check: Option<LastCheck>,
    pub today: Option<ScoutToday>,
    pub last_brief_day: Option<String>,
    pub last_brief_at: Option<String>,
    pub notes: Vec<String>,
    pub week: Option<ScoutWeek>,
    pub prs: Vec<FolderPullRequest>,
    pub last_look_at: Option<String>,
    pub model_said: Option<String>,
    pub since_brief: Option<SinceBrief>,
}

fn pr_to_json(pr: &FolderPullRequest) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("number".into(), json!(pr.number));
    obj.insert("title".into(), json!(pr.title));
    obj.insert("url".into(), json!(pr.url));
    obj.insert("state".into(), json!(pr.state.as_str()));
    obj.insert("checks".into(), json!(pr.checks.as_str()));
    obj.insert("checkCount".into(), json!(pr.check_count));
    obj.insert("notPassed".into(), json!(pr.not_passed));
    if let Some(review) = pr.review {
        let w = match review {
            PullRequestReview::Approved => "approved",
            PullRequestReview::ChangesRequested => "changes-requested",
            PullRequestReview::ReviewRequired => "review-required",
        };
        obj.insert("review".into(), json!(w));
    }
    if pr.conflicts {
        obj.insert("conflicts".into(), json!(true));
    }
    if let Some(updated) = &pr.updated_at {
        obj.insert("updatedAt".into(), json!(updated));
    }
    Value::Object(obj)
}

fn pr_from_json(v: &Value) -> Option<FolderPullRequest> {
    let obj = v.as_object()?;
    let url = obj.get("url")?.as_str()?.to_owned();
    let number = obj.get("number")?.as_u64()?;
    let title = obj
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let state = match obj.get("state").and_then(Value::as_str).unwrap_or("open") {
        "draft" => PullRequestState::Draft,
        "merged" => PullRequestState::Merged,
        "closed" => PullRequestState::Closed,
        _ => PullRequestState::Open,
    };
    let checks = match obj.get("checks").and_then(Value::as_str).unwrap_or("none") {
        "passing" => PullRequestChecks::Passing,
        "failing" => PullRequestChecks::Failing,
        "pending" => PullRequestChecks::Pending,
        _ => PullRequestChecks::None,
    };
    let check_count = obj
        .get("checkCount")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let not_passed = obj
        .get("notPassed")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
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
        .map(str::to_owned);
    Some(FolderPullRequest {
        number,
        title,
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

impl Kept {
    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        if let Some(comments) = &self.comments {
            obj.insert("comments".into(), comments.to_json());
        }
        if !self.comment_turns.is_empty() {
            let mut turns = serde_json::Map::new();
            for (k, v) in &self.comment_turns {
                turns.insert(k.clone(), v.to_json());
            }
            obj.insert("commentTurns".into(), Value::Object(turns));
        }
        obj.insert("settings".into(), self.settings.to_json());
        obj.insert(
            "cards".into(),
            Value::Array(self.cards.iter().map(|c| c.to_json(false)).collect()),
        );
        obj.insert("seen".into(), json!(self.seen));
        obj.insert("lookIds".into(), json!(self.look_ids));
        if let Some(mark) = &self.folder_mark {
            obj.insert("folderMark".into(), json!(mark));
        }
        if let Some(check) = &self.last_check {
            obj.insert("lastCheck".into(), check.to_json());
        }
        if let Some(today) = &self.today {
            obj.insert("today".into(), today.to_json());
        }
        if let Some(day) = &self.last_brief_day {
            obj.insert("lastBriefDay".into(), json!(day));
        }
        if let Some(at) = &self.last_brief_at {
            obj.insert("lastBriefAt".into(), json!(at));
        }
        if !self.notes.is_empty() {
            obj.insert("notes".into(), json!(self.notes));
        }
        if let Some(week) = &self.week {
            obj.insert("week".into(), week.to_json());
        }
        obj.insert("prs".into(), Value::Array(self.prs.iter().map(pr_to_json).collect()));
        if let Some(at) = &self.last_look_at {
            obj.insert("lastLookAt".into(), json!(at));
        }
        if let Some(said) = &self.model_said {
            obj.insert("modelSaid".into(), json!(said));
        }
        if let Some(since) = &self.since_brief {
            obj.insert("sinceBrief".into(), since.to_json());
        }
        Value::Object(obj)
    }

    pub fn from_json(v: &Value) -> Self {
        let Some(parsed) = v.as_object() else {
            return Self::default();
        };
        let settings = parsed
            .get("settings")
            .map(|s| scout_settings_from(s, &ScoutSettings::default()))
            .unwrap_or_default();
        let comments = parsed.get("comments").and_then(CommentWatch::from_json);
        let comment_turns = parsed
            .get("commentTurns")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| CommentTurn::from_json(v).map(|t| (k.clone(), t)))
                    .collect()
            })
            .unwrap_or_default();
        let cards = parsed
            .get("cards")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(ScoutCard::from_json)
                    .map(|mut c| {
                        c.reading = false;
                        c
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut seen: Vec<String> = parsed
            .get("seen")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if seen.len() > MAX_SCOUT_SEEN {
            let start = seen.len() - MAX_SCOUT_SEEN;
            seen.drain(..start);
        }
        let mut look_ids: Vec<String> = parsed
            .get("lookIds")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if look_ids.len() > MAX_LOOK_IDS {
            let start = look_ids.len() - MAX_LOOK_IDS;
            look_ids.drain(..start);
        }
        let prs = parsed
            .get("prs")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(pr_from_json).collect())
            .unwrap_or_default();
        let notes = parsed
            .get("notes")
            .and_then(Value::as_array)
            .map(|arr| {
                let raw: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
                notes_from(&raw.join("\n"))
            })
            .unwrap_or_default();
        let last_look_at = parsed
            .get("lastLookAt")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let folder_mark = parsed
            .get("folderMark")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let last_check = parsed.get("lastCheck").and_then(LastCheck::from_json);
        let today = parsed.get("today").and_then(ScoutToday::from_json);
        let last_brief_day = parsed
            .get("lastBriefDay")
            .and_then(Value::as_str)
            .filter(|s| regex_day().is_match(s))
            .map(str::to_owned);
        let last_brief_at = parsed
            .get("lastBriefAt")
            .and_then(Value::as_str)
            .filter(|s| DateTime::parse_from_rfc3339(s).is_ok())
            .map(str::to_owned);
        let week = parsed.get("week").and_then(week_from);
        let since_brief = parsed.get("sinceBrief").and_then(SinceBrief::from_json);
        Self {
            comments,
            comment_turns,
            settings,
            cards,
            seen,
            look_ids,
            folder_mark,
            last_check,
            today,
            last_brief_day,
            last_brief_at,
            notes,
            week,
            prs,
            last_look_at,
            model_said: None,
            since_brief,
        }
    }

    pub fn state(&self, now: DateTime<FixedOffset>) -> ScoutState {
        let day = scout_day(now);
        ScoutState {
            settings: self.settings.clone(),
            cards: self.cards.clone(),
            look_ids: self.look_ids.clone(),
            last_look_at: self.last_look_at.clone(),
            model_said: self.model_said.clone(),
            today: self.today.as_ref().filter(|t| t.day == day).cloned(),
            notes: self.notes.clone(),
        }
    }
}

/// Target paths for Scout's files, either inside a native `Store` (enforcing native-only writes)
/// or standalone file paths (used in isolated tests).
#[derive(Clone)]
pub enum ScoutFiles {
    Store(Store),
    Paths {
        file: PathBuf,
        log_file: Option<PathBuf>,
    },
}

impl ScoutFiles {
    pub fn scout_path(&self) -> io::Result<PathBuf> {
        match self {
            Self::Store(store) => store.check(&store.root().join(SCOUT_FILE)),
            Self::Paths { file, .. } => Ok(file.clone()),
        }
    }

    pub fn log_path(&self) -> io::Result<Option<PathBuf>> {
        match self {
            Self::Store(store) => Ok(Some(store.check(&store.root().join(SCOUT_LOG_FILE))?)),
            Self::Paths { log_file, .. } => Ok(log_file.clone()),
        }
    }

    pub fn electron_scout_path(&self) -> Option<PathBuf> {
        match self {
            Self::Store(store) => Some(store.electron().join(SCOUT_FILE)),
            Self::Paths { .. } => None,
        }
    }

    pub fn save_json_atomic(&self, path: &Path, name: &str, value: &Value) -> Result<(), String> {
        match self {
            Self::Store(store) => store.save_json(name, value),
            Self::Paths { .. } => write_atomic(path, &format!("{value}\n")).map_err(|e| e.to_string()),
        }
    }
}

fn write_atomic(path: &Path, body: &str) -> io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        crate::invariants::unique_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let res = (|| {
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

pub struct LoadedKept {
    pub kept: Kept,
    pub save_to_disk: bool,
}

pub fn load_kept_with<R, Sleep>(
    files: &ScoutFiles,
    now: DateTime<FixedOffset>,
    mut reader: R,
    mut sleep: Sleep,
) -> LoadedKept
where
    R: FnMut(&Path) -> io::Result<String>,
    Sleep: FnMut(Duration),
{
    let Ok(path) = files.scout_path() else {
        return LoadedKept {
            kept: Kept::default(),
            save_to_disk: false,
        };
    };
    let mut text: Option<String> = None;
    let mut from_native = false;
    let mut save_to_disk = true;
    for attempt in 0..=3 {
        match reader(&path) {
            Ok(s) => {
                text = Some(s);
                from_native = true;
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(_) if attempt >= 3 => {
                save_to_disk = false;
                break;
            }
            Err(_) => {
                sleep(Duration::from_millis(200 * (1u64 << attempt)));
            }
        }
    }
    // Native-first, Electron read-only fallback when native scout.json does not exist yet and read did not fail.
    if text.is_none()
        && save_to_disk
        && let Some(electron_path) = files.electron_scout_path()
        && let Ok(s) = reader(&electron_path)
    {
        text = Some(s);
    }
    let Some(raw) = text else {
        return LoadedKept {
            kept: Kept::default(),
            save_to_disk,
        };
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(val) => LoadedKept {
            kept: Kept::from_json(&val),
            save_to_disk,
        },
        Err(_) => {
            if from_native {
                if let Some(parent) = path.parent()
                    && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                {
                    let stamp = iso_string(now).replace([':', '.'], "-");
                    let backup = parent.join(format!("{file_name}.unreadable-{stamp}"));
                    let rename_ok = match files {
                        ScoutFiles::Store(store) => store
                            .check(&path)
                            .and_then(|src| store.check(&backup).map(|dst| (src, dst)))
                            .and_then(|(src, dst)| std::fs::rename(src, dst))
                            .is_ok(),
                        ScoutFiles::Paths { .. } => std::fs::rename(&path, &backup).is_ok(),
                    };
                    if !rename_ok {
                        save_to_disk = false;
                    }
                } else {
                    save_to_disk = false;
                }
            }
            LoadedKept {
                kept: Kept::default(),
                save_to_disk,
            }
        }
    }
}

pub fn load_log_with<R>(files: &ScoutFiles, mut reader: R) -> (Vec<ScoutLogEntry>, bool)
where
    R: FnMut(&Path) -> io::Result<String>,
{
    let Ok(Some(path)) = files.log_path() else {
        return (Vec::new(), false);
    };
    match reader(&path) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(val) => (scout_log_from(&val), true),
            Err(_) => (Vec::new(), true),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => (Vec::new(), true),
        Err(_) => (Vec::new(), false),
    }
}
