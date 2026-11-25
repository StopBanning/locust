//! Codex's cloud task rows in the cloud panel (`apps/desktop/src/renderer/src/components/CloudTasks.tsx`):
//! drawn under Claude's sessions in `claude_cloud_panel::beside`, wired to `locust_core::cloud_task_service`
//! (list, refresh, where/environment), with Electron's own words for every state, the empty state,
//! Codex not signed in, and a missing Codex Cloud environment.

use super::*;
use crate::claude_cloud_panel::{CloudFolder, CloudWhere};
use locust_core::changes;
use locust_core::cloud_task_service::{self, CloudApplyResult, CloudStartInput, CloudStartResult, CloudTaskService};
use locust_core::cloud_tasks::{CloudEnvironment, CloudState, CloudTask};
use locust_core::store::Store;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

const CODEX_NOT_READY: &str =
    "Cloud tasks run through Codex CLI, which is not installed or not signed in here. Settings > AI agents shows how.";
const CODEX_MENU_REFUSAL: &str = "Codex CLI is not installed or not signed in here.";
const CLOUD_START_FAILED: &str = "The cloud task could not be started. Nothing was sent.";
const EMPTY_STATE: &str = "Pick Cloud in the chat-type menu and describe a task. It runs in Codex Cloud on this repository, as GitHub has it; its change stays there until you apply it here.";
const CHECKING_ENV_FOLDERS: &str = "Checking your other folders for one that has an environment…";
const USE_FOLDER_WITH_ENV: &str = "Or use a folder that has one:";
const FAILED_STATUS: &str = "The cloud could not finish it. Open it in Codex to see why.";
const UNKNOWN_STATUS: &str = "Codex has not said how it is going yet.";
const READING_CHANGE: &str = "Reading the change…";
const NO_CHANGE_RETURNED: &str = "Codex did not return a change for it.";
/// Pending tasks are polled every 15 s while the panel is open (`App.tsx:2937`).
const REFRESH_INTERVAL_MS: u64 = 15_000;

/// What makes the window's `CloudTaskService`; None is this computer's Codex CLI (`cloud_task_service::on_this_computer`).
pub(crate) type MakeCodex = Arc<dyn Fn(&Store) -> CloudTaskService<'static> + Send + Sync>;

/// The one task whose change is currently open in the panel (`CloudTasks.tsx`'s `open` state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenChange {
    pub(crate) task_id: String,
    pub(crate) diff: Option<String>,
    pub(crate) loading: bool,
}

enum Answer {
    Loaded {
        folder: String,
        tasks: Vec<CloudTask>,
        place: CloudWhere,
        codex_ready: bool,
        environment: Option<CloudEnvironment>,
        folders: Vec<CloudFolder>,
    },
    Refreshed {
        task_id: String,
        task: Option<CloudTask>,
    },
    Diff {
        task_id: String,
        diff: Option<String>,
    },
    Applied {
        result: CloudApplyResult,
    },
}

/// Codex Cloud state held inside `claude_cloud_panel::Cloud`.
#[derive(Default)]
pub(crate) struct CodexCloud {
    /// Cloud is the chat type with a Codex model picked: the next send starts a Codex Cloud task.
    /// One message's choice, not a state the box stays in (0.505): another conversation, model change, or send clears it.
    pub(crate) on: bool,
    /// The teammate `on` was chosen for.
    pub(crate) on_for: Option<String>,
    pub(crate) tasks: Vec<CloudTask>,
    pub(crate) place: Option<CloudWhere>,
    pub(crate) codex_ready: Option<bool>,
    pub(crate) environment: Option<CloudEnvironment>,
    /// None while checking other folders for an environment; Some once checked.
    pub(crate) folders: Option<Vec<CloudFolder>>,
    /// What the cloud will not see, said when the newest task started (`CloudTasks.tsx`'s `notes`).
    pub(crate) notes: Vec<String>,
    pub(crate) problem: Option<String>,
    /// The task whose diff is currently open, if any.
    pub(crate) open: Option<OpenChange>,
    /// The task currently being applied to this folder, if any.
    pub(crate) applying: Option<String>,
    pub(crate) make: Option<MakeCodex>,
    loaded_folder: Option<String>,
    was_open: bool,
    next_refresh_ms: u64,
    refreshing: HashSet<String>,
    inbox: Arc<Mutex<Vec<Answer>>>,
}

fn teammate(s: &State) -> Option<&Teammate> {
    match &s.ledger {
        Some(pane) => Some(&pane.header),
        None => s.teammates.get(s.selected),
    }
}

fn folder(s: &State) -> Option<String> {
    teammate(s)
        .and_then(|mate| mate.folder.clone())
        .or_else(|| s.home.folder_path.clone())
}

/// Whether the composer's picked model is a Codex route (Composer.tsx `cloud.where === 'codex'`, 0.503).
pub(crate) fn codex_picked(s: &State) -> bool {
    let Some(mate) = teammate(s) else { return false };
    let runtime = s
        .picks
        .get(&mate.id)
        .and_then(|p| p.runtime.as_deref())
        .or(mate.runtime.as_deref());
    runtime == Some("codex")
}

/// Why Cloud cannot be picked for Codex here, when Codex CLI is known to be unready (`App.tsx:11032`).
pub(crate) fn menu_refusal(s: &State) -> Option<&'static str> {
    (s.cloud.codex.codex_ready == Some(false)).then_some(CODEX_MENU_REFUSAL)
}

/// The chat-type menu's Cloud choice for a Codex model (`Composer.tsx` `cloud.onMode`):
/// turning it on opens the cloud panel and reads this folder's Codex tasks and environment.
pub(crate) fn choose(s: &mut State, on: bool) {
    s.cloud.codex.on = on;
    if !on {
        return;
    }
    s.cloud.codex.on_for = teammate(s).map(|mate| mate.id.clone());
    s.cloud.open = true;
    refresh(s);
}

/// A send with Cloud on and a Codex model picked starts a Codex Cloud task instead of a turn (`App.tsx` `startCloudTask`).
/// True when this branch handled the send (started or refused); a refusal is shown over the box and keeps the draft.
pub(crate) fn send(s: &mut State, typed: &str) -> bool {
    if !s.cloud.codex.on || !codex_picked(s) {
        return false;
    }
    let (Some(store), Some(here)) = (s.data.as_ref(), folder(s)) else {
        s.composer_note = Some(CLOUD_START_FAILED.into());
        return true;
    };
    let svc = build_service(store, &s.cloud.codex.make);
    let teammate_id = teammate(s).map(|mate| mate.id.clone());
    match svc.start(CloudStartInput {
        folder: here,
        prompt: typed.to_owned(),
        teammate_id,
    }) {
        CloudStartResult::Ok { task, notes } => {
            s.cloud.codex.tasks.retain(|one| one.task_id != task.task_id);
            s.cloud.codex.tasks.push(task);
            s.cloud.panel.codex_tasks = s.cloud.codex.tasks.len();
            s.cloud.codex.notes = notes;
            s.cloud.codex.problem = None;
            s.cloud.open = true;
            s.cloud.codex.on = false;
            s.draft.clear();
            s.attached.clear();
            s.composer_note = None;
        }
        CloudStartResult::Err { message } => {
            s.composer_note = Some(message);
        }
    }
    true
}

fn codex_side(s: &State) -> bool {
    !crate::claude_cloud_panel::claude_picked(s) || !s.cloud.codex.tasks.is_empty()
}

fn build_service(store: &Store, make: &Option<MakeCodex>) -> CloudTaskService<'static> {
    match make {
        Some(make) => make(store),
        None => cloud_task_service::on_this_computer(store),
    }
}

/// Reads this folder's Codex cloud tasks, GitHub place, Codex readiness, and environment off the window's thread.
pub(crate) fn refresh(s: &mut State) {
    let (Some(store), Some(here)) = (s.data.clone(), folder(s)) else {
        return;
    };
    s.cloud.codex.was_open = s.cloud.open;
    s.cloud.codex.loaded_folder = Some(here.clone());
    s.cloud.codex.next_refresh_ms = s.cloud.now_ms.saturating_add(REFRESH_INTERVAL_MS);
    s.cloud.codex.folders = None;
    let claude_picked = crate::claude_cloud_panel::claude_picked(s);
    let make = s.cloud.codex.make.clone();
    let inbox = s.cloud.codex.inbox.clone();
    std::thread::spawn(move || {
        let svc = build_service(&store, &make);
        let tasks = svc.list(Some(&here));
        let codex_ready = svc.codex_ready();
        let on = svc.where_folder(Path::new(&here));
        let place = CloudWhere {
            folder_name: Some(locust_core::folders::folder_name(&here)),
            repo: on.repo.clone(),
            branch: on.branch.clone(),
        };
        let on_codex_side = !claude_picked || !tasks.is_empty();
        let environment = if on_codex_side && codex_ready && let Some(repo) = on.repo.as_deref() {
            Some(svc.environment(repo, Path::new(&here)))
        } else {
            None
        };
        let mut folders: Vec<(u8, CloudFolder)> = if on_codex_side
            && (on.repo.is_none() || environment == Some(CloudEnvironment::Missing))
        {
            locust_core::folders::list(&store)
                .unwrap_or_default()
                .into_iter()
                .filter(|known| known.path != here && Path::new(&known.path).is_dir())
                .take(40)
                .filter_map(|known| {
                    let other = svc.where_folder(Path::new(&known.path));
                    let repo = other.repo?;
                    let env = if codex_ready {
                        svc.environment(&repo, Path::new(&known.path))
                    } else {
                        CloudEnvironment::Unknown
                    };
                    let rank = match env {
                        CloudEnvironment::Ready => 0,
                        CloudEnvironment::Unknown => 1,
                        CloudEnvironment::Missing => 2,
                    };
                    Some((
                        rank,
                        CloudFolder {
                            id: known.id,
                            name: known.name,
                            repo,
                            path: known.path,
                            environment_ready: env == CloudEnvironment::Ready,
                        },
                    ))
                })
                .take(12)
                .collect()
        } else {
            Vec::new()
        };
        folders.sort_by_key(|(rank, _)| *rank);
        let folders = folders.into_iter().map(|(_, f)| f).collect();
        inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Answer::Loaded {
                folder: here,
                tasks,
                place,
                codex_ready,
                environment,
                folders,
            });
    });
}

fn refresh_pending(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    let pending_ids: Vec<String> = s
        .cloud
        .codex
        .tasks
        .iter()
        .filter(|task| {
            task.status.state == CloudState::Pending
                && !s.cloud.codex.refreshing.contains(&task.task_id)
        })
        .map(|task| task.task_id.clone())
        .collect();
    if pending_ids.is_empty() {
        return;
    }
    for task_id in pending_ids {
        s.cloud.codex.refreshing.insert(task_id.clone());
        let store = store.clone();
        let make = s.cloud.codex.make.clone();
        let inbox = s.cloud.codex.inbox.clone();
        std::thread::spawn(move || {
            let svc = build_service(&store, &make);
            let task = svc.refresh(&task_id);
            inbox
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(Answer::Refreshed { task_id, task });
        });
    }
}

/// Toggle showing a task's diff (`CloudTasks.tsx`'s `show`). Never applies anything.
fn show_change(s: &mut State, task_id: &str) {
    if s.cloud
        .codex
        .open
        .as_ref()
        .is_some_and(|open| open.task_id == task_id)
    {
        s.cloud.codex.open = None;
        return;
    }
    let Some(store) = s.data.clone() else { return };
    s.cloud.codex.open = Some(OpenChange {
        task_id: task_id.to_owned(),
        diff: None,
        loading: true,
    });
    let id = task_id.to_owned();
    let make = s.cloud.codex.make.clone();
    let inbox = s.cloud.codex.inbox.clone();
    std::thread::spawn(move || {
        let svc = build_service(&store, &make);
        let diff = svc.diff(&id);
        inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Answer::Diff { task_id: id, diff });
    });
}

/// Apply a ready task's change into this folder (`App.tsx:11238-11248`). Called only on the person's click.
fn apply_task(s: &mut State, task_id: &str) {
    if s.cloud.codex.applying.is_some() {
        return;
    }
    let Some(store) = s.data.clone() else { return };
    s.cloud.codex.applying = Some(task_id.to_owned());
    let id = task_id.to_owned();
    let make = s.cloud.codex.make.clone();
    let inbox = s.cloud.codex.inbox.clone();
    std::thread::spawn(move || {
        let svc = build_service(&store, &make);
        let result = svc.apply(&id);
        inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Answer::Applied { result });
    });
}

/// Applies off-thread Codex task reads and polls pending tasks every 15 s while the panel is open.
pub(crate) fn poll(s: &mut State, now: f64) -> bool {
    let mut changed = false;
    let now_ms = (now.max(0.0) * 1000.0) as u64;
    if s.cloud.codex.on
        && (teammate(s).map(|mate| mate.id.clone()) != s.cloud.codex.on_for || !codex_picked(s))
    {
        s.cloud.codex.on = false;
        changed = true;
    }
    let current_folder = folder(s);
    if !s.cloud.open {
        if s.cloud.codex.was_open || s.cloud.codex.open.is_some() {
            s.cloud.codex.was_open = false;
            s.cloud.codex.open = None;
            s.cloud.codex.refreshing.clear();
            changed = true;
        }
    } else {
        if !s.cloud.codex.was_open || s.cloud.codex.loaded_folder != current_folder {
            if s.cloud.codex.loaded_folder.is_some() && s.cloud.codex.loaded_folder != current_folder {
                s.cloud.codex.notes.clear();
                s.cloud.codex.problem = None;
                s.cloud.codex.open = None;
            }
            s.cloud.codex.next_refresh_ms = now_ms.saturating_add(REFRESH_INTERVAL_MS);
            refresh(s);
            changed = true;
        }
        let has_pending = s
            .cloud
            .codex
            .tasks
            .iter()
            .any(|task| task.status.state == CloudState::Pending);
        if has_pending && now_ms >= s.cloud.codex.next_refresh_ms {
            s.cloud.codex.next_refresh_ms = now_ms.saturating_add(REFRESH_INTERVAL_MS);
            refresh_pending(s);
            changed = true;
        }
    }

    let landed: Vec<Answer> = std::mem::take(
        &mut *s
            .cloud
            .codex
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner()),
    );
    for answer in landed {
        match answer {
            Answer::Loaded {
                folder: there,
                tasks,
                place,
                codex_ready,
                environment,
                folders,
            } => {
                if folder(s).as_deref() != Some(there.as_str()) {
                    continue;
                }
                s.cloud.codex.tasks = tasks;
                s.cloud.codex.place = Some(place);
                s.cloud.codex.codex_ready = Some(codex_ready);
                s.cloud.codex.environment = environment;
                s.cloud.codex.folders = Some(folders);
                s.cloud.panel.codex_tasks = s.cloud.codex.tasks.len();
                changed = true;
            }
            Answer::Refreshed { task_id, task } => {
                s.cloud.codex.refreshing.remove(&task_id);
                if let Some(updated) = task
                    && let Some(entry) = s
                        .cloud
                        .codex
                        .tasks
                        .iter_mut()
                        .find(|t| t.task_id == updated.task_id)
                {
                    *entry = updated;
                    changed = true;
                }
            }
            Answer::Diff { task_id, diff } => {
                if let Some(open) = s.cloud.codex.open.as_mut()
                    && open.task_id == task_id
                {
                    open.diff = diff;
                    open.loading = false;
                    changed = true;
                }
            }
            Answer::Applied { result } => {
                s.cloud.codex.applying = None;
                match result {
                    CloudApplyResult::Ok { task } => {
                        if let Some(entry) = s
                            .cloud
                            .codex
                            .tasks
                            .iter_mut()
                            .find(|t| t.task_id == task.task_id)
                        {
                            *entry = task;
                        }
                        s.cloud.codex.problem = None;
                    }
                    CloudApplyResult::Err { message } => {
                        s.cloud.codex.problem = Some(message);
                    }
                }
                changed = true;
            }
        }
    }
    changed
}

fn created_key(iso: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

fn newest_first(tasks: &[CloudTask]) -> Vec<&CloudTask> {
    let mut ordered: Vec<&CloudTask> = tasks.iter().collect();
    ordered.sort_by(|left, right| {
        created_key(&right.created_at)
            .cmp(&created_key(&left.created_at))
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
    ordered
}

fn change_counts(task: &CloudTask) -> Option<String> {
    let added = task.status.added?;
    let removed = task.status.removed.unwrap_or(0);
    let files = task.status.files.unwrap_or(0);
    let noun = if files == 1 { "file" } else { "files" };
    Some(format!("+{added} \u{2212}{removed} in {files} {noun}"))
}

fn status_line(task: &CloudTask, now: chrono::DateTime<chrono::Utc>) -> String {
    let counts = change_counts(task);
    match task.status.state {
        CloudState::Pending => {
            let ago = locust_core::home_team_state::ago_label(&task.created_at, now)
                .unwrap_or_else(|| "just now".into());
            format!("Working in the cloud · started {ago}")
        }
        CloudState::Ready => match counts {
            Some(c) => format!("Ready · {c}"),
            None => "Ready".into(),
        },
        CloudState::Applied => {
            let when = task
                .applied_at
                .as_deref()
                .and_then(|at| locust_core::home_team_state::ago_label(at, now));
            let mut out = match when {
                Some(w) if !w.is_empty() => format!("Applied to this folder {w}"),
                _ => "Applied to this folder".into(),
            };
            if let Some(c) = counts {
                out.push_str(&format!(" · {c}"));
            }
            out
        }
        CloudState::Failed => FAILED_STATUS.into(),
        CloudState::Unknown => UNKNOWN_STATUS.into(),
    }
}

fn open_folder(s: &mut State, path: &str) {
    let Some(store) = s.data.clone() else { return };
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    match locust_core::folders::use_folder(&store, Path::new(path), &now)
        .and_then(|entry| native::set_open_folder(&store, Path::new(&entry.path)).map(|_| entry))
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

/// Codex's section of the cloud panel, drawn under Claude's sessions.
pub(crate) fn view(s: &State, p: &Palette) -> Option<El> {
    if !codex_side(s) {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let note = |words: String| {
        text(words)
            .font(l.ty.body)
            .color(l.c("text-secondary"))
            .w_full()
            .wrap()
    };
    let codex = &s.cloud.codex;
    let ordered = newest_first(&codex.tasks);
    let now = chrono::Utc::now();
    let repo = codex
        .place
        .as_ref()
        .and_then(|place| place.repo.as_deref())
        .or_else(|| s.cloud.panel.place.as_ref().and_then(|p| p.repo.as_deref()));

    let mut body = col()
        .key("codex-cloud-tasks")
        .w_full()
        .gap(10.0)
        .px(16.0)
        .py(6.0);

    if let Some(problem) = &codex.problem {
        body = body.child(
            text(problem.clone())
                .key("codex-cloud-problem")
                .role("alert")
                .font(l.ty.body)
                .color(l.c("red-text"))
                .w_full()
                .wrap(),
        );
    }

    if codex.codex_ready == Some(false) {
        body = body.child(note(CODEX_NOT_READY.into()).key("codex-cloud-not-ready"));
    }

    for (idx, line) in codex.notes.iter().enumerate() {
        body = body.child(note(line.clone()).key(format!("codex-cloud-note-{idx}")));
    }

    if let Some(repo) = repo
        && codex.codex_ready != Some(false)
        && codex.environment == Some(CloudEnvironment::Missing)
    {
        let mut missing = col()
            .key("codex-cloud-no-env")
            .w_full()
            .gap(6.0)
            .child(note(format!(
                "{repo} has no Codex Cloud environment yet, so a task sent from here would be refused. \
                 Make one in the Codex app: Settings > Legacy Codex Cloud, pick this repository, then Save and publish."
            )));
        match &codex.folders {
            None => {
                missing = missing.child(note(CHECKING_ENV_FOLDERS.into()));
            }
            Some(folders) => {
                let ready: Vec<&CloudFolder> =
                    folders.iter().filter(|f| f.environment_ready).collect();
                if !ready.is_empty() {
                    missing = missing.child(note(USE_FOLDER_WITH_ENV.into()));
                    for other in ready {
                        let path = other.path.clone();
                        missing = missing.child(
                            row()
                                .key(format!("codex-cloud-env-folder-{}", other.id))
                                .w_full()
                                .gap(8.0)
                                .center_y()
                                .child(
                                    text(other.name.clone())
                                        .font(l.ty.body)
                                        .color(l.c("text-primary")),
                                )
                                .child(
                                    text(other.repo.clone())
                                        .font(l.ty.mono)
                                        .color(l.c("text-secondary"))
                                        .shrink(),
                                )
                                .child(
                                    crate::look::controls::ghost_button(&l, "Open")
                                        .key(format!("codex-cloud-env-open-{}", other.id))
                                        .on_press(move |s: &mut State| open_folder(s, &path)),
                                ),
                        );
                    }
                }
            }
        }
        missing = missing.child(
            crate::look::controls::ghost_button(&l, "Choose a folder…")
                .key("codex-cloud-env-choose")
                .on_press_tx(|_s: &mut State, tx: &mut Tx| crate::home_panel::choose_folder(tx)),
        );
        body = body.child(missing);
    }

    if ordered.is_empty()
        && repo.is_some()
        && codex.codex_ready != Some(false)
        && codex.environment != Some(CloudEnvironment::Missing)
    {
        body = body.child(note(EMPTY_STATE.into()).key("codex-cloud-empty"));
    }

    for task in ordered {
        body = body.child(task_row(codex, task, now, &l, p));
    }

    Some(body)
}

fn diff_section(
    task_id: &str,
    open: &OpenChange,
    l: &crate::look::controls::Look,
    p: &Palette,
) -> El {
    let mut box_el = col()
        .key(format!("codex-task-diff-view-{task_id}"))
        .w_full()
        .gap(6.0);
    if open.loading {
        return box_el.child(
            text(READING_CHANGE)
                .key(format!("codex-task-diff-loading-{task_id}"))
                .font(l.ty.body)
                .color(l.c("text-secondary"))
                .w_full()
                .wrap(),
        );
    }
    let Some(raw) = open.diff.as_deref() else {
        return box_el.child(
            text(NO_CHANGE_RETURNED)
                .key(format!("codex-task-diff-none-{task_id}"))
                .font(l.ty.body)
                .color(l.c("text-secondary"))
                .w_full()
                .wrap(),
        );
    };
    let files = changes::parse(raw);
    for file in &files {
        let (added, removed) = file.counts();
        let mut head = row()
            .w_full()
            .gap(6.0)
            .center_y()
            .child(
                text(file.path.clone())
                    .font(l.ty.mono)
                    .color(l.c("text-primary"))
                    .shrink(),
            );
        if file.status != "MODIFIED" {
            head = head.child(
                text(file.status.to_ascii_lowercase())
                    .font(l.ty.mono)
                    .color(l.c("text-secondary")),
            );
        }
        if added > 0 || removed > 0 {
            head = head.child(
                text(changes::line_counts(added, removed))
                    .font(l.ty.mono)
                    .color(l.c("text-secondary")),
            );
        }
        let mut file_col = col()
            .key(format!("codex-task-file-{task_id}-{}", file.path))
            .role("region")
            .label(format!("Changes to {}", file.path))
            .w_full()
            .gap(2.0)
            .px(8.0)
            .py(6.0)
            .radius(l.t.px("radius-control"))
            .border(1.0, p.hairline)
            .child(head);
        for hunk in &file.hunks {
            let range = if hunk.heading.is_empty() {
                hunk.range()
            } else {
                format!("{} {}", hunk.range(), hunk.heading)
            };
            file_col = file_col.child(
                text(range)
                    .font(l.ty.mono)
                    .color(l.c("text-muted"))
                    .w_full()
                    .wrap(),
            );
            for diff_row in &hunk.rows {
                let ink = match diff_row.kind {
                    '+' => p.green,
                    '-' => p.red,
                    _ => l.c("text-secondary"),
                };
                file_col = file_col.child(
                    text(format!("{}{}", diff_row.kind, diff_row.text))
                        .font(l.ty.mono)
                        .color(ink)
                        .w_full()
                        .wrap(),
                );
            }
        }
        if !file.hunks.is_empty() {
            let footer = if file.hunks.len() == 1 {
                "The whole change is shown".into()
            } else {
                format!("All {} sections shown", file.hunks.len())
            };
            file_col = file_col.child(
                text(footer)
                    .font(l.ty.body)
                    .color(l.c("text-muted"))
                    .w_full()
                    .wrap(),
            );
        }
        box_el = box_el.child(file_col);
    }
    box_el
}

fn task_row(
    codex: &CodexCloud,
    task: &CloudTask,
    now: chrono::DateTime<chrono::Utc>,
    l: &crate::look::controls::Look,
    p: &Palette,
) -> El {
    use crate::look::controls::{Tone, button, primary_button};
    let title = task
        .status
        .title
        .as_deref()
        .unwrap_or(&task.prompt)
        .to_owned();
    let line = status_line(task, now);
    let state = task.status.state;
    let showing = codex
        .open
        .as_ref()
        .is_some_and(|open| open.task_id == task.task_id);
    let mut el = col()
        .key(format!("codex-task-{}", task.task_id))
        .role("group")
        .label(title.clone())
        .w_full()
        .gap(6.0)
        .px(10.0)
        .py(10.0)
        .radius(l.t.px("radius-control"))
        .border(1.0, p.hairline)
        .child(
            text(title)
                .key(format!("codex-task-title-{}", task.task_id))
                .font(l.ty.control)
                .color(l.c("text-primary"))
                .w_full()
                .wrap(),
        )
        .child(
            text(line)
                .key(format!("codex-task-state-{}", task.task_id))
                .font(l.ty.body)
                .color(l.c("text-secondary"))
                .w_full()
                .wrap(),
        );

    let mut actions = row().w_full().gap(6.0).flex_wrap();
    if state == CloudState::Ready || state == CloudState::Applied {
        let id = task.task_id.clone();
        let label = if showing {
            "Hide the change"
        } else {
            "Show the change"
        };
        actions = actions.child(
            button(l, label, Tone::Neutral, false)
                .key(format!("codex-task-diff-{}", task.task_id))
                .on_press(move |s: &mut State| show_change(s, &id)),
        );
    }
    if state == CloudState::Ready {
        let id = task.task_id.clone();
        let busy = codex.applying.is_some();
        let label = if codex.applying.as_deref() == Some(task.task_id.as_str()) {
            "Applying…"
        } else {
            "Apply to this folder"
        };
        actions = actions.child(
            primary_button(l, label, busy)
                .key(format!("codex-task-apply-{}", task.task_id))
                .on_press(move |s: &mut State| apply_task(s, &id)),
        );
    }
    let url = task.url.clone();
    actions = actions.child(
        button(l, "Open in Codex", Tone::Neutral, false)
            .key(format!("codex-task-open-{}", task.task_id))
            .on_press(move |s: &mut State| crate::published_cards::open_web_link(s, &url)),
    );
    el = el.child(actions);
    if showing && let Some(open) = &codex.open {
        el = el.child(diff_section(&task.task_id, open, l, p));
    }
    el
}

#[cfg(test)]
#[path = "cloud_tasks_tests.rs"]
mod cloud_tasks_tests;

#[cfg(test)]
#[path = "cloud_tasks_apply_tests.rs"]
mod cloud_tasks_apply_tests;

#[cfg(test)]
#[path = "cloud_tasks_more_tests.rs"]
mod cloud_tasks_more_tests;

#[cfg(test)]
#[path = "cloud_tasks_send_tests.rs"]
mod cloud_tasks_send_tests;
