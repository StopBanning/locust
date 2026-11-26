//! The person's own commands in the window (Electron App.tsx's `ownCommandMenu` effect over
//! `own-command-host.ts`): the Markdown files in `<profile>/commands` and in the conversation folder's
//! `.locust/commands`, read in a job whenever the conversation, its teammate or the open folder changes, never while
//! drawing.
use super::*;
use locust_core::own_commands::{self, StoredCommand};
use std::path::PathBuf;

/// Who a command is read for: the profile its own folder is in, the conversation's teammate, and the folder Locust has
/// open (App.tsx's context: the run's mission, the picked teammate, else the workspace).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Context {
    pub profile: PathBuf,
    pub teammate: Option<Teammate>,
    pub open: Option<String>,
}

impl Context {
    /// The conversation's folder, found as its terminal and its `!` commands find it (`terminal_panel::folder`): the
    /// teammate's own, else the folder Locust has open. Reads the disk, so only in a job or on a press.
    pub fn folder(&self) -> Option<PathBuf> {
        self.teammate
            .as_ref()
            .and_then(|teammate| native::folder_for(teammate, &self.profile).ok())
            .or_else(|| self.open.as_ref().map(PathBuf::from))
    }
}

#[derive(Default)]
pub(super) struct Own {
    /// What the `/` menu offers of the person's own, in readTree's order: theirs, then the folder's.
    pub listed: Vec<StoredCommand>,
    /// The context the list was last asked for.
    pub read_for: Option<Context>,
}

/// The context now, or nothing before Locust has a data folder.
pub(super) fn context(s: &State) -> Option<Context> {
    let profile = s
        .data
        .as_ref()
        .map(|store| store.electron().to_path_buf())
        .or_else(|| native::store().map(|store| store.electron().to_path_buf()))?;
    Some(Context {
        profile,
        teammate: selected_conversation(s).map(|(teammate, _)| teammate.clone()),
        open: s.home.folder_path.clone(),
    })
}

/// Take a finished read, and ask again when the context has changed.
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some(listed) = s.own_commands_job.take() {
        s.own_commands.listed = listed;
        changed = true;
    }
    let Some(wanted) = context(s) else {
        return changed;
    };
    if s.own_commands.read_for.as_ref() != Some(&wanted) {
        s.own_commands.read_for = Some(wanted.clone());
        s.own_commands_job.request(wanted, |context| own_commands::commands_in(&context.profile, context.folder().as_deref()));
    }
    changed
}

#[cfg(test)]
#[path = "own_commands_ui_tests.rs"]
mod own_commands_ui_tests;
