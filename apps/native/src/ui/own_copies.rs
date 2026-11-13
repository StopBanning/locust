//! Settings › Project folder › "OWN COPIES" (Electron `Screens.tsx` and `WorktreeRow.tsx`): each teammate's own copy of
//! the folder, its branch, and Remove. Removing keeps the branch; a copy with uncommitted work names it first, and is
//! removed only for exactly those changes (`worktrees::Manager::remove`).

use super::*;
use locust_core::worktrees::{Manager, WorktreeInfo};

/// Electron's WorktreeRow names at most this many changes, then "and N more".
const NAMED: usize = 6;

/// A Remove that came back with something to say: why it failed, or the changes it would delete.
#[derive(Clone, Debug, Default)]
pub(super) struct Answer {
    pub message: String,
    pub changes: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct OwnCopies {
    pub folder: String,
    /// Why own copies cannot be made here, when they cannot.
    pub reason: Option<String>,
    pub list: Vec<WorktreeInfo>,
    pub answers: std::collections::HashMap<String, Answer>,
}

/// Read the folder's copies again (git's own list; a few bounded git calls).
pub(super) fn reload(s: &mut State, folder: Option<String>) {
    s.settings_panel.own_copies = folder.map(|folder| {
        let manager = Manager::new(std::path::Path::new(&folder));
        let probe = manager.probe();
        OwnCopies { reason: probe.reason, list: manager.list(), answers: Default::default(), folder }
    });
}

fn remove(s: &mut State, teammate_id: &str, discard: Option<Vec<String>>) {
    let Some(copies) = &s.settings_panel.own_copies else { return };
    let folder = copies.folder.clone();
    let manager = Manager::new(std::path::Path::new(&folder));
    match manager.remove(teammate_id, discard.as_deref()) {
        Ok(()) => reload(s, Some(folder)),
        Err(message) => {
            // Uncommitted work: named, so the person can keep it or delete exactly that.
            let changes = manager.uncommitted(teammate_id).ok().filter(|c| !c.is_empty());
            if let Some(copies) = &mut s.settings_panel.own_copies {
                copies.answers.insert(teammate_id.to_owned(), Answer { message, changes });
            }
        }
    }
}

fn name_of(s: &State, teammate_id: &str) -> String {
    s.teammates.iter().find(|t| t.id == teammate_id).map(|t| t.name.clone()).unwrap_or_else(|| teammate_id.to_owned())
}

/// A run is live in this copy: its teammate owns a running turn.
fn busy(s: &State, teammate_id: &str) -> bool {
    let Some(store) = &s.data else { return false };
    locust_core::engine_api::api()
        .running()
        .iter()
        .any(|mission| locust_core::people::owner_of(store, mission).as_deref() == Some(teammate_id))
}

/// The note under a row whose Remove found uncommitted work (Electron's WorktreeRow, word for word).
pub(super) fn changes_note(who: &str, branch: &str, changes: &[String]) -> String {
    let named: Vec<&str> = changes.iter().take(NAMED).map(String::as_str).collect();
    let more = changes.len().saturating_sub(named.len());
    let one = changes.len() == 1;
    format!(
        "{who}'s copy has {} uncommitted {} not on {branch}: {}{}. Removing it deletes {} for good.",
        changes.len(),
        if one { "change that is" } else { "changes that are" },
        named.join(", "),
        if more > 0 { format!(", and {more} more") } else { String::new() },
        if one { "it" } else { "them" }
    )
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let Some(copies) = &s.settings_panel.own_copies else { return el() };
    let note = match (&copies.reason, copies.list.is_empty()) {
        (Some(reason), _) => format!("{reason} A teammate with Own copy on cannot start until this is fixed."),
        (None, true) => "None yet. Turn Own copy on in a teammate's card and its next run makes one.".to_owned(),
        (None, false) => {
            "Removing one keeps its branch, with what was committed on it; uncommitted changes are named before anything is deleted.".to_owned()
        }
    };
    let mut column = col()
        .key("own-copies")
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .child(text("OWN COPIES").font(type_::TITLE).color(p.text))
        .child(text(note).font(type_::LABEL).color(p.secondary).wrap());
    for tree in &copies.list {
        let id = tree.teammate_id.clone();
        let who = name_of(s, &id);
        let answer = copies.answers.get(&id);
        let in_use = busy(s, &id);
        let mut line = row()
            .key(format!("own-copy-{id}"))
            .gap(10.0)
            .center_y()
            .child(text(who.clone()).font(l.ty.control).color(l.c("text-primary")))
            .child(text(tree.branch.clone()).font(l.ty.mono).color(l.c("text-muted")).label(tree.path.clone()));
        match answer.and_then(|a| a.changes.clone()) {
            None => {
                let remove_id = id.clone();
                let mut button = text(if in_use { "In use" } else { "Remove" })
                    .font(l.ty.label)
                    .color(l.c("text-secondary"))
                    .px(6.0)
                    .py(2.0)
                    .radius(4.0)
                    .key(format!("own-copy-remove-{id}"))
                    .role("button")
                    .label(if in_use { "A run is live in this worktree".to_owned() } else { format!("Remove the worktree; the branch {} stays", tree.branch) });
                if !in_use {
                    button = button.hover(l.c("hover-surface")).on_press(move |s: &mut State| remove(s, &remove_id, None));
                }
                line = line.child(button);
            }
            Some(changes) => {
                let keep_id = id.clone();
                let drop_id = id.clone();
                line = line
                    .child(
                        text("Keep it")
                            .font(l.ty.label)
                            .color(l.c("text-secondary"))
                            .px(6.0)
                            .py(2.0)
                            .radius(4.0)
                            .hover(l.c("hover-surface"))
                            .key(format!("own-copy-keep-{id}"))
                            .role("button")
                            .on_press(move |s: &mut State| {
                                if let Some(c) = &mut s.settings_panel.own_copies {
                                    c.answers.remove(&keep_id);
                                }
                            }),
                    )
                    .child(
                        text("Delete the changes and remove")
                            .font(l.ty.label)
                            .color(l.c("red-text"))
                            .px(6.0)
                            .py(2.0)
                            .radius(4.0)
                            .hover(l.c("hover-surface"))
                            .key(format!("own-copy-delete-{id}"))
                            .role("button")
                            .on_press(move |s: &mut State| remove(s, &drop_id, Some(changes.clone()))),
                    );
            }
        }
        column = column.child(line);
        if let Some(a) = answer {
            let said = match &a.changes {
                Some(changes) => changes_note(&who, &tree.branch, changes),
                None => a.message.clone(),
            };
            column = column.child(text(said).font(type_::LABEL).color(p.secondary).wrap());
        }
    }
    column
}
