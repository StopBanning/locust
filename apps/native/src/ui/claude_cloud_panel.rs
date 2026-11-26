//! Claude's cloud panel (`CloudTasks.tsx`): beside the conversation, what was sent to the cloud from this folder.
//! CLAUDE'S CLOUD SAYS WHAT IT WORKS ON (0.545): a folder that is not on GitHub is said to start with no project, before
//! a send, with the person's folders that are on GitHub one click away. Codex's environment is not Claude's concern.
//!
//! Reached from the composer's chat type menu (Cloud, offered while a Claude model is picked), as Electron's App.tsx
//! does: choosing it opens this panel beside the conversation and reads where a send would go, and the start form
//! gains the organization environment field. A send with Cloud on starts a Claude cloud session instead of a turn.
//! Under them, Claude's session rows (`ClaudeCloudSessions`, one `CloudWatch` each); Codex's task rows wait.

use super::*;
use locust_core::claude_cloud::{
    ClaudeCloud, ClaudeCloudAnswer, ClaudeCloudReading, ClaudeCloudSession, CloudChoice, CloudReads, CloudSendResult,
};
use locust_core::cloud_watch::{cloud_updated_label, CloudWatch, WatchStep, WatchStop};
use std::collections::HashMap;
use locust_core::store::Store;
use std::sync::Mutex;

pub(super) const TITLE: &str = "Cloud tasks";
const CHECKING: &str = "Checking your other folders…";
const FOLDERS_THAT_ARE: &str = "Your folders that are:";
const NONE_ON_GITHUB: &str =
    "None of the folders Locust knows is on GitHub yet. Push this project to GitHub, or open one that is.";

/// Where a send from here would go (`PublicCloudWhere`).
#[derive(Clone, Debug, Default)]
pub(crate) struct CloudWhere {
    pub folder_name: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
}

/// One of the person's folders that is on GitHub (`PublicCloudFolder`).
#[derive(Clone, Debug)]
pub(crate) struct CloudFolder {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub path: String,
    /// Whether Codex Cloud has an environment for it.
    pub environment_ready: bool,
}

/// Claude's cloud, when a Claude model is picked or a session was sent from this folder.
#[derive(Clone, Debug, Default)]
pub(crate) struct ClaudeSide {
    pub picked: bool,
    pub sessions: Vec<ClaudeCloudSession>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Panel {
    pub place: Option<CloudWhere>,
    /// None while Locust is still asking which folders are on GitHub.
    pub folders: Option<Vec<CloudFolder>>,
    pub claude: Option<ClaudeSide>,
    /// Codex Cloud tasks sent from this folder.
    pub codex_tasks: usize,
}

impl Panel {
    fn claude_picked(&self) -> bool {
        self.claude.as_ref().is_some_and(|c| c.picked)
    }

    /// Codex's warnings are about Codex Cloud: not said while Claude's is the one picked, unless Codex tasks are here.
    fn codex_side(&self) -> bool {
        !self.claude_picked() || self.codex_tasks > 0
    }

    pub(super) fn title(&self) -> String {
        if self.claude_picked() {
            return "Claude’s cloud".into();
        }
        match &self.place {
            Some(CloudWhere { repo: Some(repo), branch, .. }) => match branch {
                Some(branch) => format!("{repo} · {branch}"),
                None => repo.clone(),
            },
            _ => "Codex Cloud".into(),
        }
    }

    /// What is said when this folder is not on GitHub; None when it is, or when nothing here is about to be sent.
    pub(super) fn not_on_github(&self) -> Option<String> {
        let place = self.place.as_ref()?;
        if place.repo.is_some() || !(self.codex_side() || self.claude_picked()) {
            return None;
        }
        let name = place.folder_name.as_deref().unwrap_or("This folder");
        Some(if self.claude_picked() && !self.codex_side() {
            format!(
                "{name} is not on GitHub. Claude’s cloud works on a copy of a GitHub repository, so a task sent from here \
                 starts with no project. Open a folder that is on GitHub to work on it there."
            )
        } else {
            format!(
                "{name} is not on GitHub. A cloud task runs on a copy of a GitHub repository in Codex Cloud, not on this \
                 computer, so it needs a folder that is on GitHub."
            )
        })
    }

    /// A folder's repository, with Codex's missing environment said only on Codex's side.
    pub(super) fn repo_line(&self, folder: &CloudFolder) -> String {
        if folder.environment_ready || !self.codex_side() {
            folder.repo.clone()
        } else {
            format!("{} · no cloud environment", folder.repo)
        }
    }
}

/// What makes the window's `ClaudeCloud`; None is this computer's Claude Code (`claude_cloud::on_this_computer`).
pub(crate) type MakeClaude = Arc<dyn Fn(&Store, Arc<CloudReads>) -> ClaudeCloud<'static> + Send + Sync>;

/// What a read off the window's thread brought.
enum Answer {
    Where { folder: String, place: CloudWhere, folders: Vec<CloudFolder>, sessions: Vec<ClaudeCloudSession>, environment: Option<String> },
    /// A session read in its own worktree (Show what it did, Check again, or the watch).
    Read { id: String, reading: ClaudeCloudReading },
    Applied { id: String, result: CloudSendResult },
    Sent { id: String, result: CloudSendResult },
    Home { result: CloudSendResult, known: bool },
}

/// One session row's own state (`ClaudeCloudRow`'s hooks).
#[derive(Default)]
pub(crate) struct Row {
    /// Tell it more.
    pub(crate) draft: TextEdit,
    pub(crate) sending: bool,
    /// The line said under the row.
    pub(crate) said: Option<String>,
    pub(crate) reading: Option<ClaudeCloudReading>,
    pub(crate) checking: bool,
    pub(crate) applying: bool,
    pub(crate) applied: bool,
    /// What it did is shown.
    pub(crate) expanded: bool,
    /// The row's watch while Watch is on; None once it stops.
    pub(crate) watch: Option<CloudWatch>,
    /// The read under way was asked for by the watch, which takes what it brings.
    watch_waiting: bool,
}

/// Claude's cloud in the window (App.tsx's `cloudOn`, `cloudPanel`, `claudeCloud` and `cloudEnvironment`).
#[derive(Default)]
pub(crate) struct Cloud {
    /// Cloud is the chat type: the next send goes to Claude's cloud. One message's choice, not a state the box stays
    /// in (0.505): another conversation, or a send, sets it back.
    pub(crate) on: bool,
    /// The teammate it was chosen for.
    pub(crate) on_for: Option<String>,
    /// The panel beside the conversation.
    pub(crate) open: bool,
    pub(crate) panel: Panel,
    /// The environment field's words, for `environment_folder` only.
    pub(crate) environment: TextEdit,
    pub(crate) environment_folder: Option<String>,
    /// The words as last read or set back: anything else was typed, and a later read leaves it alone.
    pub(crate) environment_read: String,
    /// The one ledger of reads under way, shared by every `ClaudeCloud` the window makes.
    pub(crate) reads: Arc<CloudReads>,
    inbox: Arc<Mutex<Vec<Answer>>>,
    pub(crate) make: Option<MakeClaude>,
    /// Each session row's state, by the session's id; gone with the panel, as Electron's rows unmount.
    pub(crate) rows: HashMap<String, Row>,
    /// The line over the rows (Bring it home's answer).
    pub(crate) note: Option<String>,
    /// The window's clock at its last poll, in milliseconds: what a watch starts from.
    pub(crate) now_ms: u64,
    /// Codex Cloud tasks in this panel (`src/ui/cloud_tasks.rs`).
    pub(crate) codex: crate::cloud_tasks::CodexCloud,
}

/// The conversation's teammate.
fn teammate(s: &State) -> Option<&Teammate> {
    match &s.ledger {
        Some(pane) => Some(&pane.header),
        None => s.teammates.get(s.selected),
    }
}

/// The agent and model the box would send on: the one picked for this turn, else the teammate's own.
fn route(s: &State) -> Option<(String, String, Option<String>)> {
    let mate = teammate(s)?;
    let pick = s.picks.get(&mate.id);
    let runtime = pick.and_then(|p| p.runtime.clone()).or(mate.runtime.clone())?;
    let model = pick.map(|p| p.model.clone()).or(mate.model.clone()).unwrap_or_else(|| "account-default".into());
    Some((runtime, model, pick.and_then(|p| p.effort.clone())))
}

/// Claude's cloud is what Cloud means while a Claude model is picked (0.538).
pub(super) fn claude_picked(s: &State) -> bool {
    route(s).is_some_and(|(runtime, _, _)| runtime == "claude")
}

/// The folder a send from here starts in: the teammate's own, else the one Locust has open.
fn folder(s: &State) -> Option<String> {
    teammate(s).and_then(|mate| mate.folder.clone()).or_else(|| s.home.folder_path.clone())
}

fn service(s: &State) -> Option<ClaudeCloud<'static>> {
    let store = s.data.as_ref()?;
    Some(match &s.cloud.make {
        Some(make) => make(store, s.cloud.reads.clone()),
        None => locust_core::claude_cloud::on_this_computer(store, s.cloud.reads.clone()),
    })
}

fn git_in(args: &[&str], cwd: &std::path::Path) -> Result<locust_core::cloud_tasks::Ran, String> {
    let out = std::process::Command::new("git").args(args).current_dir(cwd).output().map_err(|e| e.to_string())?;
    Ok(locust_core::cloud_tasks::Ran::new(
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    ))
}

/// Where a send would go, the person's folders that are on GitHub, this folder's sessions and its saved environment,
/// read off the window's thread (App.tsx's `cloudWhere`, `cloudFolders`, `listClaudeCloud`, `getClaudeCloudEnvironment`).
pub(super) fn refresh(s: &mut State) {
    crate::cloud_tasks::refresh(s);
    let (Some(store), Some(here)) = (s.data.clone(), folder(s)) else { return };
    let make = s.cloud.make.clone();
    let reads = s.cloud.reads.clone();
    let inbox = s.cloud.inbox.clone();
    std::thread::spawn(move || {
        let claude = match &make {
            Some(make) => make(&store, reads),
            None => locust_core::claude_cloud::on_this_computer(&store, reads),
        };
        let on = locust_core::cloud_tasks::folder_on_github(std::path::Path::new(&here), &git_in);
        let place = CloudWhere { folder_name: Some(locust_core::folders::folder_name(&here)), repo: on.repo, branch: on.branch };
        let folders = locust_core::folders::list(&store)
            .unwrap_or_default()
            .into_iter()
            .filter(|known| known.path != here && std::path::Path::new(&known.path).is_dir())
            .filter_map(|known| {
                let ran = git_in(&["remote", "get-url", "origin"], std::path::Path::new(&known.path)).ok()?;
                let repo = (ran.code == 0).then(|| locust_core::cloud_tasks::github_repo_of(ran.stdout.trim())).flatten()?;
                // Whether Codex Cloud has an environment is Codex's question, asked on its side.
                Some(CloudFolder { id: known.id, name: known.name, repo, path: known.path, environment_ready: true })
            })
            .collect();
        let answer = Answer::Where { sessions: claude.list(&here), environment: claude.environment(&here), folder: here, place, folders };
        inbox.lock().unwrap_or_else(|e| e.into_inner()).push(answer);
    });
}

/// The chat type menu's Cloud (Composer.tsx's `cloud.onMode`): on, it opens the panel and reads where a send goes.
pub(super) fn choose(s: &mut State, on: bool) {
    s.cloud.on = on;
    if !on {
        return;
    }
    s.cloud.on_for = teammate(s).map(|mate| mate.id.clone());
    s.cloud.open = true;
    refresh(s);
}

/// What came back from off the window's thread, and Cloud set back once the conversation or the agent changes.
pub(super) fn poll(s: &mut State, now: f64) -> bool {
    let mut changed = false;
    let now_ms = (now.max(0.0) * 1000.0) as u64;
    s.cloud.now_ms = now_ms;
    // A closed panel's rows are gone, their watches with them (Electron unmounts them).
    if !s.cloud.open && !s.cloud.rows.is_empty() {
        s.cloud.rows.clear();
        changed = true;
    }
    changed |= tick_watches(s, now_ms);
    if s.cloud.on && (teammate(s).map(|mate| mate.id.clone()) != s.cloud.on_for || !claude_picked(s)) {
        s.cloud.on = false;
        changed = true;
    }
    let landed: Vec<Answer> = std::mem::take(&mut *s.cloud.inbox.lock().unwrap_or_else(|e| e.into_inner()));
    for answer in landed {
        match answer {
            Answer::Where { folder: there, place, folders, sessions, environment } => {
                if folder(s).as_deref() != Some(there.as_str()) {
                    continue;
                }
                s.cloud.panel.place = Some(place);
                s.cloud.panel.folders = Some(folders);
                s.cloud.panel.claude = Some(ClaudeSide { picked: claude_picked(s), sessions });
                let typed = s.cloud.environment_folder.as_deref() == Some(there.as_str())
                    && s.cloud.environment.text != s.cloud.environment_read;
                if !typed {
                    let saved = environment.unwrap_or_default();
                    s.cloud.environment.replace_text(saved.clone());
                    s.cloud.environment_read = saved;
                    s.cloud.environment_folder = Some(there);
                }
            }
            Answer::Read { id, reading } => {
                if let Some(one) = s.cloud.panel.claude.iter_mut().flat_map(|side| side.sessions.iter_mut()).find(|one| one.id == id) {
                    one.take_reading(&reading);
                }
                if s.cloud.rows.get(&id).is_some_and(|row| row.expanded) {
                    opened(s, &id);
                }
                let Some(row) = s.cloud.rows.get_mut(&id) else { continue };
                row.checking = false;
                row.applied = false;
                if std::mem::take(&mut row.watch_waiting) {
                    let stopped = row.watch.as_mut().and_then(|watch| watch.read(now_ms, Some(reading.clone()))).and_then(|(_, stop)| stop);
                    if let Some(stop) = stopped {
                        row.watch = None;
                        row.said = Some(watch_stopped(stop).into());
                    }
                }
                row.reading = Some(reading);
            }
            Answer::Applied { id, result } => {
                let Some(row) = s.cloud.rows.get_mut(&id) else { continue };
                row.applying = false;
                match result {
                    CloudSendResult::Ok => {
                        row.applied = true;
                        row.said = Some(APPLIED.into());
                    }
                    CloudSendResult::Err { message } => row.said = Some(message),
                }
            }
            Answer::Sent { id, result } => {
                let Some(row) = s.cloud.rows.get_mut(&id) else { continue };
                row.sending = false;
                match result {
                    CloudSendResult::Ok => {
                        row.draft.clear();
                        row.said = Some(SENT.into());
                    }
                    CloudSendResult::Err { message } => row.said = Some(message),
                }
            }
            Answer::Home { result, known } => {
                s.cloud.note = Some(match result {
                    CloudSendResult::Ok if known => HOME_KNOWN.into(),
                    CloudSendResult::Ok => HOME_UNKNOWN.into(),
                    CloudSendResult::Err { message } => message,
                });
            }
        }
        changed = true;
    }
    changed |= crate::cloud_tasks::poll(s, now);
    changed
}

/// The environment typed for this folder; empty for another folder's.
fn environment_here(s: &State) -> String {
    if s.cloud.environment_folder.is_some() && s.cloud.environment_folder == folder(s) {
        s.cloud.environment.text.trim().to_owned()
    } else {
        String::new()
    }
}

/// The Claude start form's cloud part: the organization environment field, only while Cloud is on with a Claude
/// model picked (Composer.tsx: `cloud.on && cloud.where === 'claude'`).
pub(super) fn start_form(s: &State, p: &Palette) -> Option<El> {
    if !s.cloud.on || !claude_picked(s) {
        return None;
    }
    let shown = if s.cloud.environment_folder.is_some() && s.cloud.environment_folder == folder(s) {
        s.cloud.environment.clone()
    } else {
        TextEdit::default()
    };
    Some(environment_field(
        &shown,
        |s: &mut State| {
            if s.cloud.environment_folder != folder(s) {
                s.cloud.environment.clear();
                s.cloud.environment_read.clear();
                s.cloud.environment_folder = folder(s);
            }
            &mut s.cloud.environment
        },
        p,
    ))
}

/// A send with Cloud on and a Claude model picked starts a Claude cloud session instead of a turn (App.tsx's
/// `startClaudeCloud` branch): on the model and effort in the box and the environment typed for this folder. True
/// when the send was this one's, sent or refused; a refusal is said over the box and the words stay in it.
pub(super) fn send(s: &mut State, typed: &str) -> bool {
    if !s.cloud.on || !claude_picked(s) {
        return false;
    }
    let (Some((_, model, effort)), Some(here), Some(claude)) = (route(s), folder(s), service(s)) else {
        s.composer_note = Some("That could not be sent to the cloud.".into());
        return true;
    };
    // The effort only where the model offers it: without that, every session ran on the account's default (0.557).
    let effort = effort.filter(|effort| {
        s.models.values().flatten().filter(|m| m.runtime == "claude").flat_map(|m| &m.models).any(|m| m.id == model && m.efforts.contains(effort))
    });
    let environment = environment_here(s);
    let choice = CloudChoice { model: Some(model), effort, environment: (!environment.is_empty()).then_some(environment) };
    let teammate_id = teammate(s).map(|mate| mate.id.clone());
    match claude.start(&here, typed, teammate_id.as_deref(), Some(&choice)) {
        ClaudeCloudAnswer::Ok { session } => {
            let side = s.cloud.panel.claude.get_or_insert_with(|| ClaudeSide { picked: true, sessions: Vec::new() });
            side.sessions.retain(|one| one.id != session.id);
            side.sessions.insert(0, session);
            s.cloud.open = true;
            s.cloud.on = false;
            s.draft.clear();
            s.attached.clear();
            s.composer_note = None;
        }
        ClaudeCloudAnswer::Err { message } => s.composer_note = Some(message),
    }
    true
}

/// The panel beside the conversation (App.tsx: after the background panel, `cloudPanel && screen === 'workroom'`).
pub(super) fn beside(s: &State, p: &Palette) -> Option<El> {
    if !s.cloud.open || s.background_runs.open {
        return None;
    }
    let mut panel = s.cloud.panel.clone();
    panel.codex_tasks = s.cloud.codex.tasks.len();
    if (!claude_picked(s) || panel.codex_tasks > 0)
        && let Some(folders) = &s.cloud.codex.folders
    {
        panel.folders = Some(folders.clone());
    }
    if (!claude_picked(s) || panel.place.is_none())
        && let Some(place) = &s.cloud.codex.place
    {
        panel.place = Some(place.clone());
    }
    if let Some(side) = panel.claude.as_mut() {
        side.picked = claude_picked(s);
    }
    Some(
        col()
            .key("cloud-tasks-beside")
            .w(380.0)
            .h_full()
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(view(&panel, p))
            .children(sessions_view(s, p))
            .children(crate::cloud_tasks::view(s, p)),
    )
}

const INTRO: &str = "Describe a task and send it. Claude does the work on Anthropic\u{2019}s machines, not this computer. Show what it \
     did reads its conversation and its change here; Apply brings the change into this folder. If Claude Code needs to ask \
     you something first, such as whether you trust this folder, it opens a window for that.";
const APPLIED: &str = "Its change is in this folder, not committed. Look it over before you keep it.";
const SENT: &str = "Sent. Check again in a moment to read its reply.";
const HOME_KNOWN: &str = "Claude Code opened in this folder and is bringing this session\u{2019}s work into it.";
const HOME_UNKNOWN: &str =
    "Claude Code opened in this folder with its list of cloud sessions. Pick this one, and its work comes into this folder.";
const CLAUDE_CODE_SESSIONS: &str = "https://claude.ai/code";
/// A closed row that learned something since the person last opened it.
const NEW: &str = "New";

fn watch_stopped(stop: WatchStop) -> &'static str {
    match stop {
        WatchStop::Time => "Watch stopped after 30 minutes.",
        WatchStop::Unchanged => "Watch stopped after two reads with nothing new.",
        WatchStop::Failed => "Watch stopped because the session could not be read. Check again; the last reading stays here.",
    }
}

/// One job for the window's `ClaudeCloud`, run off the window's thread; what it answers lands in `poll`.
fn off_thread(s: &State, job: impl FnOnce(&ClaudeCloud<'static>) -> Option<Answer> + Send + 'static) {
    let Some(store) = s.data.clone() else { return };
    let (make, reads, inbox) = (s.cloud.make.clone(), s.cloud.reads.clone(), s.cloud.inbox.clone());
    std::thread::spawn(move || {
        let claude = match &make {
            Some(make) => make(&store, reads),
            None => locust_core::claude_cloud::on_this_computer(&store, reads),
        };
        if let Some(answer) = job(&claude) {
            inbox.lock().unwrap_or_else(|e| e.into_inner()).push(answer);
        }
    });
}

/// Read the session once; a read already under way is the one answer (`readOnce`).
fn read_once(s: &mut State, id: &str) {
    let row = s.cloud.rows.entry(id.to_owned()).or_default();
    if row.checking {
        return;
    }
    row.checking = true;
    row.said = None;
    let id = id.to_owned();
    off_thread(s, move |claude| Some(Answer::Read { reading: claude.check(&id), id }));
}

/// Show what it did / Check again.
pub(super) fn check(s: &mut State, id: &str) {
    s.cloud.rows.entry(id.to_owned()).or_default().expanded = true;
    opened(s, id);
    read_once(s, id);
}

/// The person has the session's row open: what Locust learned of it so far is no longer new, here and in its record.
fn opened(s: &mut State, id: &str) {
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    if let Some(one) = s.cloud.panel.claude.iter_mut().flat_map(|side| side.sessions.iter_mut()).find(|one| one.id == id) {
        one.opened_at = Some(at.clone());
    }
    let id = id.to_owned();
    off_thread(s, move |claude| {
        claude.mark_opened(&id, &at);
        None
    });
}

/// The time of the newest reply, as a clock reads it: today's alone, another day's with its weekday.
fn reply_clock(at: &str, now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    let at = chrono::DateTime::parse_from_rfc3339(at).ok()?.with_timezone(&chrono::Utc);
    let clock = locust_core::usage_words::clock_time(at);
    let local = at.with_timezone(&chrono::Local);
    Some(if local.date_naive() == now.with_timezone(&chrono::Local).date_naive() {
        clock
    } else {
        format!("{} {clock}", local.format("%a"))
    })
}

/// A closed row's glance (Cloud view 1): the newest reply's first line and its time, from what the last read
/// brought; None before any read brought a reply.
pub(super) fn glance(session: &ClaudeCloudSession, now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    let line = session.last_reply.as_deref()?;
    Some(match session.last_reply_at.as_deref().and_then(|at| reply_clock(at, now)) {
        Some(when) => format!("As of {when}: {line}"),
        None => line.to_owned(),
    })
}

/// Watch / Stop watching: a watch from the window's clock, its first read at once unless a reading is shown.
pub(super) fn toggle_watch(s: &mut State, id: &str) {
    let now = s.cloud.now_ms;
    let row = s.cloud.rows.entry(id.to_owned()).or_default();
    if row.applying {
        return;
    }
    row.expanded = true;
    row.watch = match row.watch.take() {
        Some(_) => None,
        None => Some(CloudWatch::start(now, row.reading.as_ref())),
    };
    row.watch_waiting = false;
}

/// Hide what it did: the watch stops with it.
fn hide(s: &mut State, id: &str) {
    let row = s.cloud.rows.entry(id.to_owned()).or_default();
    row.expanded = false;
    row.watch = None;
}

/// Every open row's watch, from the window's poll: a read when one is due, the line said when it stops.
fn tick_watches(s: &mut State, now_ms: u64) -> bool {
    let mut changed = false;
    let ids: Vec<String> = s.cloud.rows.iter().filter(|(_, row)| row.watch.is_some()).map(|(id, _)| id.clone()).collect();
    for id in ids {
        let Some(row) = s.cloud.rows.get_mut(&id) else { continue };
        let Some(watch) = row.watch.as_mut() else { continue };
        match watch.poll(now_ms) {
            WatchStep::Wait => {}
            WatchStep::Read => {
                row.watch_waiting = true;
                read_once(s, &id);
                changed = true;
            }
            WatchStep::Stopped(stop) => {
                row.watch = None;
                row.said = Some(watch_stopped(stop).into());
                changed = true;
            }
        }
    }
    changed
}

/// Apply to this folder: its change comes in, not committed.
pub(super) fn apply(s: &mut State, id: &str) {
    let row = s.cloud.rows.entry(id.to_owned()).or_default();
    if row.applying {
        return;
    }
    row.watch = None;
    row.applying = true;
    let id = id.to_owned();
    off_thread(s, move |claude| Some(Answer::Applied { result: claude.apply(&id), id }));
}

/// Tell it more: a follow-up to the session by its id.
pub(super) fn tell_more(s: &mut State, id: &str) {
    let row = s.cloud.rows.entry(id.to_owned()).or_default();
    let words = row.draft.text.trim().to_owned();
    if words.is_empty() || row.sending {
        return;
    }
    row.sending = true;
    row.said = None;
    let id = id.to_owned();
    off_thread(s, move |claude| Some(Answer::Sent { result: claude.send(&id, &words), id }));
}

/// Bring it home: Claude Code opens in this folder and brings the session's work into it.
pub(super) fn home(s: &mut State, id: &str) {
    let known = s.cloud.panel.claude.iter().flat_map(|side| &side.sessions).any(|one| one.id == id && one.session_id.is_some());
    let id = id.to_owned();
    off_thread(s, move |claude| Some(Answer::Home { result: claude.home(&id), known }));
}

/// Remove from this list; the session itself stays on claude.ai.
pub(super) fn forget(s: &mut State, id: &str) {
    if let Some(side) = s.cloud.panel.claude.as_mut() {
        side.sessions.retain(|one| one.id != id);
    }
    s.cloud.rows.remove(id);
    let id = id.to_owned();
    off_thread(s, move |claude| {
        claude.forget(&id);
        None
    });
}

/// A unified diff's files with their added and removed line counts, in order.
fn diff_files(diff: &str) -> Vec<(String, usize, usize)> {
    let mut files: Vec<(String, usize, usize)> = Vec::new();
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let path = rest.rsplit_once(" b/").map(|(_, b)| b).unwrap_or(rest);
            files.push((path.to_owned(), 0, 0));
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

/// Claude's cloud sessions from this folder (`ClaudeCloudSessions`), while Claude is picked or a session is here.
pub(super) fn sessions_view(s: &State, p: &Palette) -> Option<El> {
    let side = s.cloud.panel.claude.as_ref()?;
    let picked = claude_picked(s);
    if !picked && side.sessions.is_empty() {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let mut list = col().key("cloud-sessions").w_full().gap(10.0).px(16.0).py(6.0);
    if picked || side.sessions.is_empty() {
        list = list.child(text(INTRO).key("cloud-sessions-intro").font(l.ty.body).color(l.c("text-secondary")).wrap());
    }
    if let Some(note) = &s.cloud.note {
        list = list.child(text(note.clone()).key("cloud-sessions-note").role("status").font(l.ty.body).color(l.c("text-primary")).wrap());
    }
    for session in &side.sessions {
        list = list.child(session_row(s, session, &l, p));
    }
    Some(list)
}

fn session_row(s: &State, session: &ClaudeCloudSession, l: &crate::look::controls::Look, p: &Palette) -> El {
    use crate::look::controls::{button, primary_button, Tone};
    let blank = Row::default();
    let state = s.cloud.rows.get(&session.id).unwrap_or(&blank);
    let id = session.id.clone();
    let key = |what: &str| format!("cloud-session-{what}-{id}");
    let note = |words: String| text(words).font(l.ty.body).color(l.c("text-secondary")).w_full().wrap();
    let known = session.session_id.is_some();
    let ago = locust_core::home_team_state::ago_label(&session.started_at, chrono::Utc::now()).unwrap_or_else(|| "just now".into());
    let read = match &state.reading {
        Some(ClaudeCloudReading::Ok { exchanges, checked_at, diff, change_too_big, note }) => {
            Some((exchanges, checked_at, diff, *change_too_big, note))
        }
        _ => None,
    };
    let mut el = col()
        .key(format!("cloud-session-{id}"))
        .role("group")
        .label(session.title.clone().unwrap_or_else(|| session.prompt.clone()))
        .w_full()
        .gap(6.0)
        .px(10.0).py(10.0)
        .radius(l.t.px("radius-control"))
        .border(1.0, p.hairline);
    let news = !state.expanded && session.has_news();
    let mark = news.then(|| text(NEW).key(key("new")).role("status").font(l.ty.control).color(l.c("blue-text")));
    if let Some(title) = &session.title {
        el = el.child(row().w_full().gap(6.0).center_y().child(text(title.clone()).font(l.ty.control).color(l.c("text-primary")).grow(1.0).wrap()).children(mark));
        el = el.child(text(session.prompt.clone()).font(l.ty.body).color(l.c("text-primary")).w_full().wrap());
    } else {
        el = el.child(row().w_full().gap(6.0).center_y().child(text(session.prompt.clone()).font(l.ty.body).color(l.c("text-primary")).grow(1.0).wrap()).children(mark));
    }
    el = el.child(note(if known {
        format!("In Claude\u{2019}s cloud since {ago}.")
    } else {
        format!("Sent to Claude\u{2019}s cloud {ago}. Follow it on claude.ai or in the Claude app.")
    }));
    if !state.expanded
        && let Some(line) = glance(session, chrono::Utc::now())
    {
        el = el.child(note(line).key(key("glance")));
    }
    if let Some(said) = &session.note {
        el = el.child(note(said.clone()));
    }
    let mut actions = row().w_full().gap(6.0).flex_wrap();
    if known {
        let label = if state.checking {
            "Reading it…"
        } else if read.is_none() {
            "Show what it did"
        } else {
            "Check again"
        };
        let one = id.clone();
        actions = actions.child(primary_button(l, label, state.checking).key(key("check")).on_press(move |s: &mut State| {
            if !s.cloud.rows.get(&one).is_some_and(|row| row.checking) {
                check(s, &one)
            }
        }));
        let one = id.clone();
        actions = actions.child(
            button(l, if state.watch.is_some() { "Stop watching" } else { "Watch" }, Tone::Neutral, state.applying)
                .key(key("watch"))
                .on_press(move |s: &mut State| toggle_watch(s, &one)),
        );
        if state.expanded {
            let one = id.clone();
            actions = actions.child(button(l, "Hide what it did", Tone::Neutral, false).key(key("hide")).on_press(move |s: &mut State| hide(s, &one)));
        }
    }
    let url = session.url.clone().unwrap_or_else(|| CLAUDE_CODE_SESSIONS.into());
    let web = if known { button(l, "See it on claude.ai", Tone::Neutral, false) } else { primary_button(l, "See it on claude.ai", false) };
    actions = actions.child(web.key(key("web")).on_press(move |s: &mut State| crate::published_cards::open_web_link(s, &url)));
    let one = id.clone();
    actions = actions.child(button(l, "Bring it home", Tone::Neutral, false).key(key("home")).on_press(move |s: &mut State| home(s, &one)));
    let one = id.clone();
    actions = actions.child(
        crate::look::controls::ghost_button(l, "Remove")
            .key(key("forget"))
            .label(format!("Remove \"{}\" from this list", session.prompt))
            .on_press(move |s: &mut State| forget(s, &one)),
    );
    el = el.child(actions);
    if state.expanded {
        if let Some(ClaudeCloudReading::Err { message }) = &state.reading {
            el = el.child(note(message.clone()).key(key("problem")).role("alert"));
        }
    }
    if let (true, Some((exchanges, checked_at, diff, too_big, read_note))) = (state.expanded, read) {
        let mut shown = col().key(key("read")).w_full().gap(6.0);
        let now_ms = chrono::Utc::now().timestamp_millis();
        shown = shown.child(note(cloud_updated_label(checked_at, now_ms)).role("status"));
        if !exchanges.is_empty() || read_note.is_none() {
            shown = shown.child(note(if exchanges.is_empty() {
                "Claude Code brought it in, but it has no finished reply yet.".into()
            } else {
                let at = locust_core::home_team_state::ago_label(checked_at, chrono::Utc::now()).unwrap_or_else(|| "just now".into());
                format!("What it said, as of {at}:")
            }));
        }
        for exchange in exchanges {
            let mut one = col().w_full().gap(3.0).child(text(exchange.prompt.clone()).font(l.ty.body).color(l.c("text-secondary")).w_full().wrap());
            if let Some(answer) = &exchange.answer {
                one = one.child(text(answer.clone()).font(l.ty.body).color(l.c("text-primary")).w_full().wrap());
            }
            shown = shown.child(one);
        }
        if let Some(said) = read_note {
            shown = shown.child(note(said.clone()));
        }
        if too_big {
            shown = shown.child(note("Its change is too big to show here. Apply still brings it in whole.".into()));
        }
        let files = diff.as_deref().map(diff_files).unwrap_or_default();
        for (path, added, removed) in &files {
            shown = shown.child(text(format!("{path}  +{added} \u{2212}{removed}")).font(l.ty.mono).color(l.c("text-secondary")));
        }
        if !files.is_empty() || too_big {
            let label = if state.applied {
                "Applied"
            } else if state.applying {
                "Applying…"
            } else {
                "Apply to this folder"
            };
            let one = id.clone();
            shown = shown.child(primary_button(l, label, state.applying || state.applied).key(key("apply")).on_press(move |s: &mut State| {
                if !s.cloud.rows.get(&one).is_some_and(|row| row.applying || row.applied) {
                    apply(s, &one)
                }
            }));
        }
        if diff.is_none() && !too_big && read_note.is_none() {
            shown = shown.child(note("It has changed nothing yet.".into()));
        }
        el = el.child(shown);
    }
    if known {
        let one = id.clone();
        let sent = id.clone();
        let more = text_input(&state.draft, l.ty.body)
            .key(key("more"))
            .single_line()
            .label(format!("Tell \"{}\" more", session.title.as_deref().unwrap_or(&session.prompt)))
            .placeholder("Tell it more…", p.muted)
            .grow(1.0)
            .h(30.0)
            .border(1.0, p.hairline)
            .on_edit(move |s: &mut State| &mut s.cloud.rows.entry(one.clone()).or_default().draft)
            .on_submit(move |s: &mut State, _: &mut Tx| tell_more(s, &sent));
        let one = id.clone();
        let busy = state.sending || state.draft.text.trim().is_empty();
        el = el.child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(more)
                .child(button(l, if state.sending { "Sending…" } else { "Send" }, Tone::Neutral, busy).key(key("send")).on_press(move |s: &mut State| tell_more(s, &one))),
        );
    }
    if let Some(said) = &state.said {
        el = el.child(note(said.clone()).key(key("said")).role("status"));
    }
    el
}

/// Open one of the person's folders, as Choose folder does with the one chosen.
fn open_folder(s: &mut State, path: &str) {
    let Some(store) = s.data.clone() else { return };
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    match locust_core::folders::use_folder(&store, std::path::Path::new(path), &now)
        .and_then(|entry| native::set_open_folder(&store, std::path::Path::new(&entry.path)).map(|_| entry))
    {
        Ok(entry) => {
            s.home.folder_path = Some(entry.path);
            s.home.folder_invented = false;
        }
        Err(e) => {
            s.status = e;
            s.status_tone = StatusTone::Amber;
        }
    }
}

pub(super) fn view(panel: &Panel, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let note = |words: String| text(words).font(l.ty.body).color(l.c("text-secondary")).wrap();
    let mut body = col()
        .key("cloud-tasks")
        .role("complementary")
        .label("Cloud tasks")
        .w_full()
        .gap(8.0)
        .px(16.0)
        .py(10.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text(TITLE).font(l.ty.control).color(l.c("text-primary")))
                .child(text(panel.title()).key("cloud-tasks-where").font(l.ty.mono).color(l.c("text-secondary")))
                .child(rune::spacer())
                .child(
                    crate::look::controls::ghost_button(&l, "Close")
                        .key("cloud-tasks-close")
                        .on_press(|s: &mut State| s.cloud.open = false),
                ),
        );
    if let Some(said) = panel.not_on_github() {
        let mut elsewhere = col().key("cloud-tasks-elsewhere").w_full().gap(6.0).child(note(said));
        match &panel.folders {
            None => elsewhere = elsewhere.child(note(CHECKING.into())),
            Some(folders) if folders.is_empty() => elsewhere = elsewhere.child(note(NONE_ON_GITHUB.into())),
            Some(folders) => {
                elsewhere = elsewhere.child(note(FOLDERS_THAT_ARE.into()));
                for folder in folders {
                    let path = folder.path.clone();
                    elsewhere = elsewhere.child(
                        row()
                            .key(format!("cloud-tasks-folder-{}", folder.id))
                            .w_full()
                            .gap(8.0)
                            .center_y()
                            .child(text(folder.name.clone()).font(l.ty.body).color(l.c("text-primary")))
                            .child(text(panel.repo_line(folder)).font(l.ty.mono).color(l.c("text-secondary")).shrink())
                            .child(
                                crate::look::controls::ghost_button(&l, "Open")
                                    .key(format!("cloud-tasks-open-{}", folder.id))
                                    .on_press(move |s: &mut State| open_folder(s, &path)),
                            ),
                    );
                }
            }
        }
        body = body.child(elsewhere.child(
            crate::look::controls::ghost_button(&l, "Choose a folder…")
                .key("cloud-tasks-choose")
                .on_press_tx(|_s: &mut State, tx: &mut Tx| crate::home_panel::choose_folder(tx)),
        ));
    }
    body
}

pub(super) const ENVIRONMENT_LABEL: &str = "Run on your organization's environment (ID)";
const ENVIRONMENT_HELP: &str =
    "Optional. Use an environment ID supplied by your organization. Saved for this folder when you send a task.";
const ENVIRONMENT_INVALID: &str =
    "Use ccpool_ followed by 1 to 80 letters, digits, underscores or hyphens, or leave this empty.";

/// W6's optional environment field for a Claude cloud start (`CloudEnvironmentField.tsx`): empty is fine, anything else
/// must be an environment ID, and the line under it says which. Claude Code documents no list command, so it is typed.
/// `edit` finds the field's words in the state that holds them.
pub(super) fn environment_field(
    value: &TextEdit,
    edit: impl Fn(&mut State) -> &mut TextEdit + Clone + 'static,
    p: &Palette,
) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let typed = value.text.trim_end_matches(['\r', '\n']);
    let valid = typed.is_empty() || locust_core::claude_cloud::is_cloud_environment_id(typed);
    let field = text_input(value, l.ty.mono)
        .key("cloud-environment")
        .single_line()
        .label(ENVIRONMENT_LABEL)
        .placeholder("Optional · ccpool_…", p.muted)
        .w_full()
        .h(34.0)
        .border(1.0, if valid { p.hairline } else { l.c("amber-text") })
        .on_edit(move |s: &mut State| edit(s));
    col()
        .key("cloud-environment-field")
        .w_full()
        .gap(4.0)
        .child(text(ENVIRONMENT_LABEL).font(l.ty.body).color(l.c("text-primary")))
        .child(field)
        .child(
            text(if valid { ENVIRONMENT_HELP } else { ENVIRONMENT_INVALID })
                .key(if valid { "cloud-environment-help" } else { "cloud-environment-invalid" })
                .font(l.ty.body)
                .color(l.c(if valid { "text-secondary" } else { "amber-text" }))
                .wrap(),
        )
}

#[cfg(test)]
#[path = "claude_cloud_panel_tests.rs"]
mod claude_cloud_panel_tests;

#[cfg(test)]
#[path = "claude_cloud_panel_window_tests.rs"]
mod claude_cloud_panel_window_tests;

#[cfg(test)]
#[path = "claude_cloud_panel_rows_tests.rs"]
mod claude_cloud_panel_rows_tests;
