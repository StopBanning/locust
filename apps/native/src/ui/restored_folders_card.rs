//! "Find your folders on this computer" (App.tsx, the `restoreFolders` card): after a backup from another computer is
//! restored, and when the person goes to choose a folder, the folders the conversations were in that are not on this
//! computer are listed with the one found for each. Nothing moves until Apply folders.

use super::*;
use locust_core::restored_folders::{self, FolderMove, RestoredFolderMatch};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(super) const TITLE: &str = "Find your folders on this computer";
pub(super) const LATER_NOTE: &str = "Unmatched folders can be found later from the folder button.";
pub(super) const NOT_APPLIED: &str = "The folders could not be applied. Open the list again to try.";
const PICK_TITLE: &str = "Find this folder on this computer";

#[derive(Clone, Debug, Default)]
pub(crate) struct Finding {
    pub(super) matches: Vec<RestoredFolderMatch>,
    /// The folder chosen for each row: the one found, until the person turns it off or picks another.
    pub(super) choices: HashMap<String, String>,
    /// What may be applied for each row: the one found and any the person picked (index.ts `restoreOffers`).
    pub(super) offers: HashMap<String, HashSet<String>>,
    pub(super) message: String,
    /// The home folder the scan looked under, which no row may be moved to (index.ts `folderCaution`).
    pub(super) home: Option<String>,
}

/// The person's home folder. None in a test, which never scans a real home: the tests give one of their own.
fn home() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Scan, and fill the card when something is missing. True when the card has rows.
pub(super) fn show(s: &mut State) -> bool {
    home().is_some_and(|home| show_in(s, &home))
}

pub(super) fn show_in(s: &mut State, home: &Path) -> bool {
    let Some(store) = s.data.clone() else { return false };
    let matches = restored_folders::check(&store, home);
    s.restore_folders = Finding {
        choices: matches.iter().filter_map(|m| Some((m.id.clone(), m.candidate.clone()?))).collect(),
        offers: matches.iter().map(|m| (m.id.clone(), m.candidate.iter().cloned().collect())).collect(),
        matches,
        message: String::new(),
        home: Some(home.to_string_lossy().into_owned()),
    };
    !s.restore_folders.matches.is_empty()
}

/// Choose folder (App.tsx `chooseWorkspace`): the restored folders first when any are missing, the picker otherwise.
pub(super) fn choose_workspace(s: &mut State, tx: &mut Tx) {
    choose_workspace_in(s, tx, home().as_deref());
}

pub(super) fn choose_workspace_in(s: &mut State, tx: &mut Tx, home: Option<&Path>) {
    if !home.is_some_and(|home| show_in(s, home)) {
        crate::home_panel::choose_folder(tx);
    }
}

fn toggle(s: &mut State, id: &str) {
    let finding = &mut s.restore_folders;
    let candidate = finding.matches.iter().find(|m| m.id == id).and_then(|m| m.candidate.clone());
    match (finding.choices.contains_key(id), candidate) {
        (false, Some(candidate)) => {
            finding.choices.insert(id.to_owned(), candidate);
        }
        _ => {
            finding.choices.remove(id);
        }
    }
}

/// A folder the person picked for a row. Never one a teammate should not work in (index.ts `folderCaution`).
pub(super) fn picked(s: &mut State, id: &str, path: &Path) {
    let path = path.to_string_lossy().into_owned();
    if locust_core::thread_view::folder_caution(&path, s.restore_folders.home.as_deref(), cfg!(windows), None).is_some() {
        return;
    }
    let Some(offered) = s.restore_folders.offers.get_mut(id) else { return };
    offered.insert(path.clone());
    s.restore_folders.choices.insert(id.to_owned(), path);
}

fn pick(id: String, tx: &mut Tx) {
    let options = rune::FileDialogOptions { title: PICK_TITLE.into(), ..Default::default() };
    tx.open_folder(options, move |s: &mut State, answer, _tx| {
        if let Ok(Some(path)) = answer {
            picked(s, &id, &path);
        }
    });
}

/// Apply folders: every choice checked first, nothing moved while a conversation runs, then each folder moved and the
/// open folder followed when it was one of them.
pub(super) fn apply(s: &mut State, running: bool) {
    let Some(store) = s.data.clone() else { return };
    let finding = &s.restore_folders;
    let chosen: Vec<(String, String)> =
        finding.matches.iter().filter_map(|m| Some((m.id.clone(), finding.choices.get(&m.id)?.clone()))).collect();
    let moves: Vec<FolderMove> = match restored_folders::moves_for(&store, &chosen, &finding.offers, finding.home.as_deref()) {
        Ok(moves) => moves,
        Err(said) => {
            s.restore_folders.message = said.unwrap_or(NOT_APPLIED).to_owned();
            return;
        }
    };
    if running {
        s.restore_folders.message = restored_folders::WAIT_FOR_RUNS.to_owned();
        return;
    }
    // Locust's own records only: Electron's ledger folder is only ever read (native.rs `electron_ledger_dir`).
    let ledgers: Vec<PathBuf> = store.ledger_dir().ok().into_iter().collect();
    for one in &moves {
        if restored_folders::apply_move(&store, &ledgers, one).is_err() {
            s.restore_folders.message = restored_folders::NOT_SAVED.to_owned();
            return;
        }
        if s.home.folder_path.is_none() || s.home.folder_path.as_deref() == Some(one.old_path.as_str()) {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            if let Ok(entry) = locust_core::folders::use_folder(&store, Path::new(&one.path), &now) {
                let _ = native::set_open_folder(&store, Path::new(&entry.path));
                s.home.folder_path = Some(entry.path);
                s.home.folder_invented = false;
            }
        }
    }
    s.restore_folders = Finding::default();
    // App.tsx `refreshForFolder`: the team and the conversations read again for the folders they are in now. Not in a
    // test, where it would ask the real engines.
    if !cfg!(test) {
        load_team(s);
    }
}

pub(super) fn card(s: &State, p: &Palette) -> Option<El> {
    let finding = &s.restore_folders;
    if finding.matches.is_empty() {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let mut body = col()
        .key("restore-folders")
        .role("region")
        .label("Find restored folders")
        .w_full()
        .gap(8.0)
        .px(16.0)
        .py(10.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(text(TITLE).font(l.ty.control).color(l.c("text-primary")));
    for (index, m) in finding.matches.iter().enumerate() {
        let mut line = row()
            .key(format!("restore-folders-row-{index}"))
            .w_full()
            .gap(8.0)
            .center_y()
            .child(text(m.path.clone()).font(l.ty.mono).color(l.c("amber-text")).shrink().wrap_anywhere());
        let chosen = finding.choices.get(&m.id);
        if let Some(shown) = chosen.or(m.candidate.as_ref()) {
            let id = m.id.clone();
            let label = format!("Use {shown}{}", if chosen.is_some() { " ✓" } else { "" });
            line = line.child(
                crate::look::controls::ghost_button(&l, &label)
                    .key(format!("restore-folders-use-{index}"))
                    .on_press(move |s: &mut State| toggle(s, &id)),
            );
        }
        let id = m.id.clone();
        line = line.child(
            crate::look::controls::ghost_button(&l, "Choose…")
                .key(format!("restore-folders-choose-{index}"))
                .on_press_tx(move |_s: &mut State, tx: &mut Tx| pick(id.clone(), tx)),
        );
        body = body.child(line);
    }
    let message = if finding.message.is_empty() { LATER_NOTE } else { finding.message.as_str() };
    let none_chosen = finding.choices.is_empty();
    Some(body.child(text(message.to_owned()).key("restore-folders-said").font(l.ty.body).color(l.c("text-secondary")).wrap())
        .child(
            row()
                .gap(8.0)
                .child(
                    crate::look::controls::button(&l, "Apply folders", crate::look::controls::Tone::Neutral, none_chosen)
                        .key("restore-folders-apply")
                        .on_press(move |s: &mut State| {
                            if !none_chosen {
                                apply(s, !native::running_ids().is_empty());
                            }
                        }),
                )
                .child(
                    crate::look::controls::ghost_button(&l, "Later")
                        .key("restore-folders-later")
                        .on_press(|s: &mut State| s.restore_folders = Finding::default()),
                ),
        ))
}

#[cfg(test)]
#[path = "restored_folders_card_tests.rs"]
mod restored_folders_card_tests;
