//! Handed-over files under a reply (`Thread.tsx` `HandedFiles` + `missionView.ts` refused lines).
//! Reveal in the file manager only — never opens or runs the file.

use super::*;
use locust_core::protocol_tags::{self, HandedFile, RefusedFileLine};
use std::path::PathBuf;

/// The file on disk, inside this conversation's folders, or why not (said beside the files).
fn resolved(s: &mut State, scope: &str, path: &str) -> Option<PathBuf> {
    let refuse = |s: &mut State, why: String| {
        s.handed_file_refusal = Some((scope.to_owned(), why));
        None
    };
    let Some(folder) = selected_conversation(s).and_then(|(t, _)| t.folder.clone()).filter(|f| !f.is_empty()) else {
        return refuse(s, "Locust could not reach that file. Nothing was changed.".into());
    };
    let on_disk = protocol_tags::handed_file_on_disk(&folder, path);
    let mut roots = vec![PathBuf::from(&folder)];
    roots.extend(s.reference_items.iter().map(|r| PathBuf::from(&r.path)));
    match crate::file_menu::decide_reveal(&on_disk, &roots) {
        crate::file_menu::RevealDecision::Ok { path, .. } => Some(path),
        crate::file_menu::RevealDecision::NoPath => refuse(s, "Locust could not reach that file. Nothing was changed.".into()),
        crate::file_menu::RevealDecision::Outside { path: bad } => refuse(s, format!("Not shown as a file: {bad} -- outside this conversation's folders.")),
    }
}

/// Copy a handed text file's words (Electron's CopyButton on the file): a spreadsheet or a Word file has none to copy.
fn copy_handed_file(s: &mut State, tx: &mut Tx, scope: &str, path: &str) {
    let Some(file) = resolved(s, scope, path) else { return };
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
    let Some(file) = resolved(s, scope, path) else { return };
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
    let Some((teammate, _)) = selected_conversation(s) else {
        return;
    };
    let Some(folder) = teammate.folder.as_deref().filter(|f| !f.is_empty()) else {
        s.handed_file_refusal = Some((
            scope.to_owned(),
            "Locust could not reach that file. Nothing was changed.".into(),
        ));
        return;
    };
    let on_disk = protocol_tags::handed_file_on_disk(folder, path);
    let mut roots = vec![PathBuf::from(folder)];
    for r in &s.reference_items {
        roots.push(PathBuf::from(&r.path));
    }
    match crate::file_menu::decide_reveal(&on_disk, &roots) {
        crate::file_menu::RevealDecision::Ok { path: resolved, .. } => {
            if let Err(err) = crate::file_menu::reveal_path(&resolved) {
                s.handed_file_refusal = Some((scope.to_owned(), err));
            } else {
                s.handed_file_refusal = None;
            }
        }
        crate::file_menu::RevealDecision::NoPath => {
            s.handed_file_refusal = Some((
                scope.to_owned(),
                "Locust could not reach that file. Nothing was changed.".into(),
            ));
        }
        crate::file_menu::RevealDecision::Outside { path: bad } => {
            s.handed_file_refusal = Some((
                scope.to_owned(),
                format!("Not shown as a file: {bad} -- outside this conversation's folders."),
            ));
        }
    }
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
                .label(file.path.clone())
                .on_press(move |s: &mut State| {
                    reveal_handed_file(s, &scope_owned, &path);
                });
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
