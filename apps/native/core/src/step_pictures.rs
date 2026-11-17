//! Which pictures a step shows (Electron's 0.7xx "Pictures show in the chat": `ThreadImage` under a read row and
//! under a changed image file, from `a-thread-shows-local-pictures.test.tsx`).
//!
//! Electron hung each picture under its own row inside the opened card. Here a step's pictures sit together under the
//! step's line, as rounded cards, so they show while the step is folded -- the way Claude's own app shows a turn's
//! screenshots under "Used 4 tools". Which pictures qualify is Electron's rule exactly: a read of an image file by any
//! runtime's read tool, and an image file a step made or changed, never one it deleted, never a search that merely
//! named one. Whether the file may be drawn at all (inside the conversation's folder, 8 MB, never SVG) is the window's
//! `workspace_image` check, made when the picture is drawn.

use crate::image_files::is_image_path;
use crate::ledger::{Item, Status};

/// The image paths a step's items read or changed, in the order they happened, each once.
pub fn step_picture_paths(items: &[Item], workspace: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |path: &str, out: &mut Vec<String>| {
        let path = path.trim();
        if is_image_path(path) && seen.insert(crate::activity_entries::path_key(path, workspace)) {
            out.push(path.to_owned());
        }
    };
    let patched: Vec<(usize, crate::activity_entries::FileEntry)> = crate::activity_entries::net_entries(items, workspace);
    for (index, item) in items.iter().enumerate() {
        let Item::Tool { name, status, summary, detail } = item else { continue };
        if *status == Status::Failed {
            continue;
        }
        let shown = crate::tool_fold::shown_name(name, summary, &detail.arguments);
        if crate::tool_fold::tool_kind(name) == "read" {
            push(&shown, &mut out);
            continue;
        }
        let edits = name == "file_change" || detail.observed_edit || crate::mission_export::edit_tool_name(name);
        if !edits {
            continue;
        }
        let files: Vec<&crate::activity_entries::FileEntry> = patched.iter().filter(|(at, _)| *at == index).map(|(_, f)| f).collect();
        if detail.patch.is_some() {
            for entry in files {
                if entry.file.status != "DELETED" {
                    push(&entry.file.path, &mut out);
                }
            }
        } else {
            for path in shown.lines() {
                push(path, &mut out);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "step_pictures_tests.rs"]
mod tests;
