//! Claude's cloud panel (`CloudTasks.tsx`): beside the conversation, what was sent to the cloud from this folder.
//! CLAUDE'S CLOUD SAYS WHAT IT WORKS ON (0.545): a folder that is not on GitHub is said to start with no project, before
//! a send, with the person's folders that are on GitHub one click away. Codex's environment is not Claude's concern.
//!
//! Reached from the composer's chat type menu (Cloud, offered while a Claude model is picked), as Electron's App.tsx
//! does: choosing it opens this panel beside the conversation and reads where a send would go, and the start form
//! gains the organization environment field. A send with Cloud on starts a Claude cloud session instead of a turn.
//! The session rows (`ClaudeCloudSessions`) and Codex's task rows are built below them.

use super::*;
use locust_core::claude_cloud::{ClaudeCloud, ClaudeCloudAnswer, ClaudeCloudSession, CloudChoice, CloudReads};
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
pub(super) fn poll(s: &mut State, _now: f64) -> bool {
    let mut changed = false;
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
        }
        changed = true;
    }
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
    if let Some(side) = panel.claude.as_mut() {
        side.picked = claude_picked(s);
    }
    Some(col().key("cloud-tasks-beside").w(380.0).h_full().fill(p.raised).border(1.0, p.hairline).child(view(&panel, p)))
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
