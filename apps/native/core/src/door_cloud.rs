//! CLAUDE'S CLOUD THROUGH THE DOOR (25 Nov): another app starts, lists, follows up and reads a Claude cloud chat the
//! way the window's cloud panel does (`claude_cloud.rs` is the engine both use). Electron's door has none of this.
//!
//! A cloud chat is Locust's own `cc_…` session (the window's `claude-cloud.json`), not a mission ledger, so its
//! conversation id is that id. The door keeps one more small file beside it, `door-cloud.json`: which sessions an app
//! started (an app reads, follows up and is told how to stop only those, unless the person lets apps see their
//! conversations), and how many exchanges a read last saw, so a read straight after a follow-up is never taken for its
//! reply. `Seams` holds what the live door runs against; tests put their own Claude Code and git in its place and never
//! run the real `claude`.

use crate::claude_cloud::{
    ClaudeCloud, ClaudeCloudAnswer, ClaudeCloudReading, ClaudeCloudSession, CloudChoice, CloudReads, CloudSendResult,
    on_this_computer,
};
use crate::cloud_tasks::{FolderOnGitHub, Ran, folder_on_github};
use crate::store::Store;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Sessions the door remembers having started, newest first.
const KEEP: usize = 100;
/// Claude Code's own bound on what one task or follow-up can say (`cloud_task_text`).
pub const MESSAGE_LIMIT: usize = 4_000;

/// What the live door runs Claude's cloud and git on.
#[derive(Clone)]
pub struct Seams {
    /// This computer's Claude Code, made for each call.
    pub claude: Arc<dyn Fn(&Store) -> ClaudeCloud<'static> + Send + Sync>,
    /// Where a folder stands with GitHub.
    pub github: Arc<dyn Fn(&Path) -> FolderOnGitHub + Send + Sync>,
}

/// Reads of one session never overlap, whatever made the `ClaudeCloud` (W4): one ledger for the whole process.
fn reads() -> Arc<CloudReads> {
    static READS: OnceLock<Arc<CloudReads>> = OnceLock::new();
    READS.get_or_init(Arc::default).clone()
}

fn git_in(args: &[&str], cwd: &Path) -> Result<Ran, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .env(crate::git_quiet::NO_OPTIONAL_LOCKS.0, crate::git_quiet::NO_OPTIONAL_LOCKS.1)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    Ok(Ran::new(out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

/// A test's own seams, in place of the live ones while its guard lives.
#[cfg(test)]
static TESTING: Mutex<Option<Seams>> = Mutex::new(None);

/// What the door runs against now: this computer's, or a test's own. The engine asks each time, so no test ever
/// reaches the real `claude`.
pub fn seams() -> Seams {
    #[cfg(test)]
    if let Some(own) = crate::invariants::lock(&TESTING).clone() {
        return own;
    }
    Seams::live()
}

impl Seams {
    /// This computer's Claude Code and git.
    pub fn live() -> Self {
        Self {
            claude: Arc::new(|store| on_this_computer(store, reads())),
            github: Arc::new(|folder| folder_on_github(folder, &git_in)),
        }
    }
}

/// A cloud chat as the door lists it: the session, and whether an app (not the person in the window) started it.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudChat {
    pub session: ClaudeCloudSession,
    pub from_app: bool,
}

/// What the door asks Claude's cloud to start: the folder, the task, the Claude teammate, its model and effort.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudStart {
    pub folder: String,
    pub prompt: String,
    pub teammate_id: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

// ---- what the door remembers --------------------------------------------------------------------------------------

/// One session an app started, with what reads of it have seen: how many exchanges the last read found, and how many
/// follow-ups were sent since, so a read straight after a send is never taken for its reply.
#[derive(Debug, Clone, PartialEq)]
struct Mark {
    id: String,
    started_at: String,
    seen: u64,
    pending: u64,
}

/// Where the door keeps what it started, beside the window's own `claude-cloud.json`.
pub fn marks_path(store: &Store) -> PathBuf {
    store.root().join("door-cloud.json")
}

static WRITES: Mutex<()> = Mutex::new(());

fn read_marks(path: &Path) -> Vec<Mark> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    parsed["started"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            Some(Mark {
                id: m["id"].as_str()?.to_owned(),
                started_at: m["startedAt"].as_str()?.to_owned(),
                seen: m["seen"].as_u64().unwrap_or(0),
                pending: m["pending"].as_u64().unwrap_or(0),
            })
        })
        .collect()
}

fn write_marks(path: &Path, marks: &[Mark]) -> Result<(), String> {
    let body = json!({"started": marks.iter().map(|m| json!({"id": m.id, "startedAt": m.started_at, "seen": m.seen, "pending": m.pending})).collect::<Vec<_>>()});
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".{}.tmp", crate::invariants::unique_nanos()));
    let tmp = PathBuf::from(tmp);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&tmp, body.to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// A started session is recorded under its id with the time it started: the engine counts its ids from 1 in every
/// run, so an id alone could name another session after a restart.
fn remember(path: &Path, session: &ClaudeCloudSession) -> Result<(), String> {
    let _guard = crate::invariants::lock(&WRITES);
    let mut marks: Vec<Mark> = read_marks(path).into_iter().filter(|m| m.id != session.id).collect();
    // The task itself is the first exchange.
    marks.insert(0, Mark { id: session.id.clone(), started_at: session.started_at.clone(), seen: 1, pending: 0 });
    marks.truncate(KEEP);
    write_marks(path, &marks)
}

// ---- start and list -----------------------------------------------------------------------------------------------

/// Where a folder stands with GitHub, for the refusal a task that would start with no project deserves.
pub fn where_with(seams: &Seams, folder: &str) -> FolderOnGitHub {
    (seams.github)(Path::new(folder))
}

/// Start the task in Claude's cloud as the window's panel does. The folder's saved organization environment goes
/// with it: a start with none would erase the one the person saved for that folder.
pub fn start_with(claude: &ClaudeCloud<'_>, marks: &Path, start: &CloudStart) -> ClaudeCloudAnswer {
    let choice = CloudChoice { model: start.model.clone(), effort: start.effort.clone(), environment: claude.environment(&start.folder) };
    match claude.start(&start.folder, &start.prompt, Some(&start.teammate_id), Some(&choice)) {
        ClaudeCloudAnswer::Ok { mut session } => {
            if remember(marks, &session).is_err() {
                // Started, but not recorded as this app's: say so rather than hand back an id it may not read.
                session.note = Some(
                    "Locust could not record that this app started it, so read_reply may refuse it unless the person lets apps see their conversations."
                        .into(),
                );
            }
            ClaudeCloudAnswer::Ok { session }
        }
        err => err,
    }
}

/// Every cloud chat Locust knows, newest first, each with whether an app started it.
pub fn chats_with(claude: &ClaudeCloud<'_>, marks: &Path) -> Vec<CloudChat> {
    let started = read_marks(marks);
    claude
        .all()
        .into_iter()
        .map(|session| {
            let from_app = started.iter().any(|m| m.id == session.id && m.started_at == session.started_at);
            CloudChat { session, from_app }
        })
        .collect()
}

/// One cloud chat by its id.
pub fn chat_with(claude: &ClaudeCloud<'_>, marks: &Path, id: &str) -> Option<CloudChat> {
    chats_with(claude, marks).into_iter().find(|c| c.session.id == id)
}

/// Change what the door remembers of one session, only while the record is of that very session.
fn update(path: &Path, session: &ClaudeCloudSession, change: impl FnOnce(&mut Mark)) {
    let _guard = crate::invariants::lock(&WRITES);
    let mut marks = read_marks(path);
    if let Some(mark) = marks.iter_mut().find(|m| m.id == session.id && m.started_at == session.started_at) {
        change(mark);
        let _ = write_marks(path, &marks);
    }
}

fn mark_of(path: &Path, session: &ClaudeCloudSession) -> Option<Mark> {
    read_marks(path).into_iter().find(|m| m.id == session.id && m.started_at == session.started_at)
}

// ---- follow up and read -------------------------------------------------------------------------------------------

/// A follow-up to the session itself. Sent, it is counted, so a read finds the reply to it and not the answer before.
pub fn send_with(claude: &ClaudeCloud<'_>, marks: &Path, id: &str, message: &str) -> CloudSendResult {
    let sent = claude.send(id, message);
    if sent == CloudSendResult::Ok
        && let Some(chat) = chat_with(claude, marks, id)
    {
        update(marks, &chat.session, |m| m.pending += 1);
    }
    sent
}

/// What reading a cloud chat found.
#[derive(Debug, Clone, PartialEq)]
pub enum CloudRead {
    /// No reply yet: the session has not answered, or not the follow-up just sent.
    Working { note: Option<String> },
    /// The newest answer, with the files the session changed (path, added lines, removed lines).
    Done { answer: String, model: Option<String>, changed: Vec<(String, usize, usize)>, change_too_big: bool, note: Option<String> },
    Failed(String),
}

/// The files a unified diff touches, with the lines added and removed in each.
fn diff_files(diff: &str) -> Vec<(String, usize, usize)> {
    let mut files: Vec<(String, usize, usize)> = Vec::new();
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            files.push((rest.rsplit_once(" b/").map(|(_, b)| b).unwrap_or(rest).to_owned(), 0, 0));
        } else if line.starts_with("+++") || line.starts_with("---") {
            continue;
        } else if let Some(file) = files.last_mut() {
            if line.starts_with('+') {
                file.1 += 1;
            } else if line.starts_with('-') {
                file.2 += 1;
            }
        }
    }
    files
}

/// Read the session the way the window's Check does (the teleport into its own worktree beside the folder, the folder
/// untouched): one look per call. The newest exchange's answer is the reply once the exchanges are as many as were
/// started and sent; before that, it is still working.
pub fn read_with(claude: &ClaudeCloud<'_>, marks: &Path, id: &str) -> CloudRead {
    let (exchanges, diff, change_too_big, note) = match claude.check(id) {
        ClaudeCloudReading::Ok { exchanges, diff, change_too_big, note, .. } => (exchanges, diff, change_too_big, note),
        ClaudeCloudReading::Err { message } => return CloudRead::Failed(message),
    };
    let session = claude.all().into_iter().find(|s| s.id == id);
    let mark = session.as_ref().and_then(|s| mark_of(marks, s));
    let needed = mark.as_ref().map_or(0, |m| m.seen + m.pending);
    // The reading keeps its newest 40 exchanges: a long session can never count past that.
    let arrived = exchanges.len() as u64 >= needed || exchanges.len() >= 40;
    let answered = exchanges.last().and_then(|e| e.answer.as_deref()).filter(|a| !a.trim().is_empty());
    let (true, Some(answer)) = (arrived, answered) else {
        return CloudRead::Working { note };
    };
    if let Some(session) = &session {
        update(marks, session, |m| {
            m.seen = exchanges.len() as u64;
            m.pending = 0;
        });
    }
    CloudRead::Done {
        answer: answer.to_owned(),
        model: exchanges.last().and_then(|e| e.model.clone()),
        changed: diff.as_deref().map(diff_files).unwrap_or_default(),
        change_too_big,
        note,
    }
}

/// The line a reply or a row gives for where to follow it: the session's own page, else the list of sessions.
pub fn link_of(session: &ClaudeCloudSession) -> String {
    session.url.clone().unwrap_or_else(|| "https://claude.ai/code".into())
}

#[cfg(test)]
#[path = "door_cloud_tests.rs"]
mod tests;
