//! Handed-over files under a reply (`Thread.tsx` `HandedFiles` + `missionView.ts` refused lines).
//! Reveal in the file manager only — never opens or runs the file.

use super::*;
use locust_core::protocol_tags::{self, HandedFile, RefusedFileLine};
use std::path::PathBuf;

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
        let mut pills = row().w_full().gap(8.0);
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
                        .color(l.c("text-primary")),
                );
            if let Some(note) = &file.note {
                inner = inner.child(
                    text(note.clone())
                        .font(l.ty.label)
                        .color(l.c("text-muted")),
                );
            }
            let path = file.path.clone();
            let scope_owned = scope.to_owned();
            let pill = inner
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
            pills = pills.child(pill);
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
