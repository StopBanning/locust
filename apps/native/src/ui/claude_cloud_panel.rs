//! Claude's cloud panel (`CloudTasks.tsx`): beside the conversation, what was sent to the cloud from this folder.
//! CLAUDE'S CLOUD SAYS WHAT IT WORKS ON (0.545): a folder that is not on GitHub is said to start with no project, before
//! a send, with the person's folders that are on GitHub one click away. Codex's environment is not Claude's concern.
//!
//! Built so far: the head and the not-on-GitHub rule. The session rows (`ClaudeCloudSessions`: home, forget, check,
//! apply, follow-ups, the watch), Codex's task rows and its missing-environment block wait.

use super::*;
use locust_core::claude_cloud::ClaudeCloudSession;

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
                .child(text(panel.title()).key("cloud-tasks-where").font(l.ty.mono).color(l.c("text-secondary"))),
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

#[cfg(test)]
#[path = "claude_cloud_panel_tests.rs"]
mod claude_cloud_panel_tests;
