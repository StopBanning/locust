//! Handed-over files under a reply (`Thread.tsx` `HandedFiles` + `missionView.ts` refused lines).
//! Reveal in the file manager only — never opens or runs the file.

use super::*;
use locust_core::protocol_tags::{self, HandedFile, RefusedFileLine};
use std::path::{Path, PathBuf};

/// What a press on a handed file asks for: each says its refusal in Electron's own words (index.ts, the reveal, the save
/// and the read).
#[derive(Clone, Copy)]
enum Asked {
    Show,
    Copy,
    Save,
}

const OUTSIDE: &str = "That file is outside the folders your teammates work in, and no teammate wrote it or handed it over, so Locust will";

const LINKED_OUT: &str = "That file is a link to somewhere outside the folder your teammates work in, so Locust will not open it.";

/// The file on disk, inside this conversation's folders or named by a teammate's own record (`decideTeammateFile`, 0.787),
/// or why not (said beside the files).
fn resolved(s: &mut State, scope: &str, path: &str, asked: Asked) -> Option<PathBuf> {
    resolved_with_roots(s, scope, path, asked).map(|(file, _)| file)
}

/// The file, as `resolved`, with the folders a read of it asks its link question of (`readRoots`).
fn resolved_with_roots(s: &mut State, scope: &str, path: &str, asked: Asked) -> Option<(PathBuf, Vec<PathBuf>)> {
    let refuse = |s: &mut State, why: String| {
        s.handed_file_refusal = Some((scope.to_owned(), why));
        None
    };
    let folder = selected_conversation(s).and_then(|(t, _)| t.folder.clone()).filter(|f| !f.is_empty());
    // A whole path needs no folder to be found; a relative one is the conversation's folder's.
    let on_disk = match &folder {
        Some(folder) => protocol_tags::handed_file_on_disk(folder, path),
        None if protocol_tags::is_whole_path(path) => path.to_owned(),
        None => return refuse(s, "Locust could not reach that file. Nothing was changed.".into()),
    };
    let mut roots: Vec<PathBuf> = folder.iter().map(PathBuf::from).collect();
    roots.extend(s.reference_items.iter().map(|r| PathBuf::from(&r.path)));
    // The folders a relative handover may be resolved against: every teammate's, and this conversation's.
    let mut folders: Vec<String> = s.teammates.iter().filter_map(|t| t.folder.clone()).filter(|f| !f.is_empty()).collect();
    folders.extend(folder);
    let named = |file: &Path| locust_core::named_files::named_by_a_teammate(&locust_core::named_files::ledger_dirs(), &file.to_string_lossy(), &folders);
    match crate::file_menu::decide_teammate_file(&on_disk, &roots, named) {
        crate::file_menu::RevealDecision::Ok { path, named } => {
            let read_roots = crate::file_menu::read_roots(&path, named, &roots);
            Some((path, read_roots))
        }
        refused => refuse(
            s,
            match (asked, refused) {
                (Asked::Show, crate::file_menu::RevealDecision::NoPath) => "There is no file to show.".into(),
                (Asked::Save, crate::file_menu::RevealDecision::NoPath) => "There is no file to save.".into(),
                (Asked::Save, _) => format!("{OUTSIDE} not copy it."),
                _ => format!("{OUTSIDE} not open it."),
            },
        ),
    }
}

/// Copy a handed text file's words (Electron's CopyButton on the file): a spreadsheet or a Word file has none to copy.
fn copy_handed_file(s: &mut State, tx: &mut Tx, scope: &str, path: &str) {
    let Some((file, read_roots)) = resolved_with_roots(s, scope, path, Asked::Copy) else { return };
    // A link in the folder that leads out of it is not the folder's to read (Electron's text channel, SEC-01).
    if !crate::file_menu::inside_on_disk(&file, &read_roots) {
        s.handed_file_refusal = Some((scope.to_owned(), LINKED_OUT.into()));
        return;
    }
    // The same reading as the viewer's (Electron's CopyButton asks `readTextFile`): a spreadsheet or a Word file has no
    // text to copy as it is; a kind of file a teammate does not write to be read, a folder, a file not there and a file
    // past a quarter of a megabyte are each refused in the viewer's own words.
    match locust_core::text_files::read_for_copy(path, &file) {
        Ok(text) => {
            s.handed_file_refusal = None;
            crate::reply_actions::request_copy(tx, text);
        }
        Err(why) => s.handed_file_refusal = Some((scope.to_owned(), why)),
    }
}

/// Save a copy where the person picks, in the system's save dialog; the file itself is never opened or run.
fn save_handed_copy(s: &mut State, tx: &mut Tx, scope: &str, path: &str) {
    let Some(file) = resolved(s, scope, path, Asked::Save) else { return };
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned());
    let scope = scope.to_owned();
    let options = rune::FileDialogOptions { title: "Save a copy".into(), file_name: name, ..Default::default() };
    tx.save_file(options, move |s: &mut State, answer, _tx| match answer {
        Ok(None) => {}
        Ok(Some(dest)) => {
            s.handed_file_refusal = match std::fs::copy(&file, &dest) {
                Ok(_) => None,
                Err(_) => Some((scope.clone(), "The copy could not be saved. Nothing was changed.".into())),
            };
        }
        Err(_) => s.handed_file_refusal = Some((scope.clone(), "The copy could not be saved. Nothing was changed.".into())),
    });
}

fn reveal_handed_file(s: &mut State, scope: &str, path: &str) {
    if selected_conversation(s).is_none() {
        return;
    }
    let Some(resolved) = resolved(s, scope, path, Asked::Show) else { return };
    s.handed_file_refusal = crate::file_menu::reveal_path(&resolved).err().map(|err| (scope.to_owned(), err));
}

/// How much of a handed file's path its pill keeps when the reply is narrow; the note gives way first.
const PATH_KEPT: f32 = 96.0;

pub(super) fn view(
    s: &State,
    files: &[HandedFile],
    refused: &[RefusedFileLine],
    scope: &str,
    p: &Palette,
) -> Option<El> {
    if files.is_empty() && refused.is_empty() {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let mut container = col().w_full().gap(6.0);

    if !files.is_empty() {
        // Beside the browser the reply is ~418px wide (the-thread-beside-the-browser-keeps-its-parts-whole, 0.867): a
        // file and its buttons wrap to a line of their own, the pill gives way inside the reply, its note before its
        // path, and Copy and Save a copy stay whole.
        let mut pills = row().w_full().gap(8.0).flex_wrap();
        for (idx, file) in files.iter().enumerate() {
            let display_path = if protocol_tags::is_whole_path(&file.path) {
                file.path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&file.path)
                    .to_owned()
            } else {
                file.path.clone()
            };
            let mut inner = row()
                .gap(6.0)
                .center_y()
                .child(rune::icon(&crate::look::icons::FILE, 12.0).color(l.c("text-secondary")))
                .child(
                    text(display_path)
                        .font(l.ty.label)
                        .color(l.c("text-primary"))
                        .shrink()
                        .truncate()
                        .min_w(PATH_KEPT),
                );
            if let Some(note) = &file.note {
                // Electron's note is `flex: 0 1000 auto`: it goes before the path does. Rune has no shrink weight, so
                // the path keeps PATH_KEPT and the note takes the rest of the cut.
                inner = inner.child(
                    text(note.clone())
                        .font(l.ty.label)
                        .color(l.c("text-muted"))
                        .shrink()
                        .truncate(),
                );
            }
            let path = file.path.clone();
            let scope_owned = scope.to_owned();
            let pill = inner
                .shrink()
                .key(format!("handed-file-{scope}-{idx}"))
                .py(5.0)
                .px(10.0)
                .radius(6.0)
                .fill(l.c("bg-control"))
                .border(1.0, l.c("border-control"))
                .hover(l.c("hover-surface"))
                .focusable(true)
                .focus_ring(2.0, 2.0, l.c("focus-ring"))
                .role("button")
                .label(format!("Show {} in the file manager", file.path))
                .on_press(move |s: &mut State| {
                    reveal_handed_file(s, &scope_owned, &path);
                });
            // THE NOTE IS IN THE HOVER (Thread.tsx HandedFiles), because the note is what truncates: the teammate's
            // own words about the file, then what a press does, so the cut-off half is never lost.
            let what = format!("Show {} in the file manager", file.path);
            let hover = match &file.note { Some(note) => format!("{note}\n\n{what}"), None => what };
            // The tooltip's anchor row must give way as the pill does, or a narrow reply cannot cut the note.
            let pill = super::hover::tip(s, &l, &format!("handed-tip-{scope}-{idx}"), &hover, pill).shrink().min_w(0.0);
            let (copy_path, copy_scope, save_path, save_scope) = (file.path.clone(), scope.to_owned(), file.path.clone(), scope.to_owned());
            let small = |label: &str, key: String, words: String| {
                text(label.to_owned()).font(l.ty.label).color(l.c("text-secondary")).py(5.0).px(8.0).radius(6.0).hover(l.c("hover-surface")).key(key).role("button").label(words)
            };
            pills = pills.child(
                row()
                    .gap(8.0)
                    .center_y()
                    .shrink()
                    .key(format!("handed-group-{scope}-{idx}"))
                    .child(pill)
                    .child(small("Copy", format!("handed-file-copy-{scope}-{idx}"), format!("Copy {}", file.path)).on_press_tx(move |s: &mut State, tx: &mut Tx| copy_handed_file(s, tx, &copy_scope, &copy_path)))
                    .child(small("Save a copy\u{2026}", format!("handed-file-save-{scope}-{idx}"), format!("Save a copy of {}", file.path)).on_press_tx(move |s: &mut State, tx: &mut Tx| save_handed_copy(s, tx, &save_scope, &save_path))),
            );
        }
        container = container.child(pills);
    }

    if let Some((err_scope, err)) = &s.handed_file_refusal
        && err_scope == scope
    {
        container = container.child(
            text(err.clone())
                .font(l.ty.label)
                .color(p.amber)
                .wrap_anywhere(),
        );
    }

    if !refused.is_empty() {
        let paths: Vec<&str> = refused.iter().map(|r| r.path.as_str()).collect();
        let word = if refused.len() == 1 {
            "this is not one"
        } else {
            "these are not"
        };
        let msg = format!(
            "Not shown as a file: {} -- a file is handed over by its path on this computer, and {word}.",
            paths.join(", ")
        );
        container = container.child(
            text(msg)
                .font(l.ty.label)
                .color(p.amber)
                .w_full()
                .wrap_anywhere(),
        );
    }

    Some(container)
}

#[cfg(test)]
#[path = "handed_files_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "handed_control_tests.rs"]
mod handed_control_tests;

#[cfg(test)]
#[path = "reveal_file_tests.rs"]
mod reveal_file_tests;
