//! A conversation's reference folders in the composer (ReferenceChips.tsx): the folder chip's menu offers "Add a
//! reference…", the picker adds a folder on this computer through the native dialog, and each reference is a chip
//! above the message box with its access note and a remove button. A new conversation holds its chips as a draft
//! until its first turn binds them.

use super::*;
use locust_core::references;

/// The key the chips belong to: the conversation, or a teammate's new conversation.
fn key_now(s: &State) -> Option<String> {
    conversation_draft_key(s)
}

fn conversation_now(s: &State) -> Option<String> {
    selected_conversation(s).and_then(|(_, c)| c.conversation_id.clone())
}

/// When the conversation on screen changes, its references are read again; a new conversation starts with none.
pub(super) fn poll(s: &mut State) -> bool {
    let key = key_now(s).unwrap_or_default();
    if s.references_key == key {
        return false;
    }
    s.references_key = key;
    s.reference_menu = false;
    s.reference_picker = false;
    s.reference_note = None;
    s.reference_items = match (conversation_now(s), s.data.as_ref()) {
        (Some(id), Some(store)) => match references::select(store, Some(&id), None) {
            Ok(items) => items,
            Err(why) => {
                s.reference_note = Some(why);
                Vec::new()
            }
        },
        _ => Vec::new(),
    };
    true
}

/// The ids a send passes: the chips on screen.
pub(super) fn ids(s: &State) -> Vec<String> {
    s.reference_items.iter().map(|i| i.id.clone()).collect()
}

/// A conversation's chips are saved as they change (set); a new conversation's wait for its first turn.
fn save(s: &mut State) {
    let (Some(id), Some(store)) = (conversation_now(s), s.data.clone()) else { return };
    if let Err(why) = references::set(&store, &id, &ids(s)) {
        s.reference_note = Some(why);
    }
}

fn add_folder(s: &mut State, tx: &mut Tx) {
    if s.reference_items.len() >= references::MAX {
        return;
    }
    let options = rune::FileDialogOptions { title: "Add a reference folder".into(), ..Default::default() };
    tx.open_folder(options, |s: &mut State, answer, _tx| {
        let Ok(Some(path)) = answer else { return };
        let Some(store) = s.data.clone() else { return };
        match references::add_folder(&store, &path) {
            Ok(item) => {
                if s.reference_items.len() < references::MAX {
                    s.reference_items.push(item);
                    s.reference_picker = false;
                    s.reference_note = None;
                    save(s);
                }
            }
            Err(why) => s.reference_note = Some(why),
        }
    });
}

fn remove(s: &mut State, id: &str) {
    s.reference_items.retain(|i| i.id != id);
    save(s);
    if let Some(store) = s.data.clone()
        && let Err(why) = references::discard(&store, id)
    {
        s.reference_note = Some(why);
    }
}

/// The folder chip's menu: one row, "Add a reference…", as Electron's folder menu has it.
pub(super) fn folder_menu(s: &State, l: &crate::look::controls::Look) -> Option<El> {
    if !s.reference_menu {
        return None;
    }
    let row = col()
        .gap(2.0)
        .py(7.0)
        .px(10.0)
        .radius(l.t.px("radius-control"))
        .hover(l.c("bg-selected"))
        .child(text("Add a reference…").font(l.ty.control).color(l.c("text-primary")))
        .child(text("Read another folder").font(l.ty.body).color(l.c("text-muted")))
        .role("menuitem")
        .on_press(|s: &mut State| {
            s.reference_menu = false;
            s.reference_picker = true;
        });
    Some(
        col()
            .w(260.0)
            .padding(4.0)
            .radius(11.0)
            .fill(l.c("bg-raised-2"))
            .border(1.0, l.c("border-strong"))
            .box_shadow(l.t.shadows("shadow-menu").first().copied().unwrap_or_default())
            .child(row),
    )
}

/// The picker and the chips, above the message box. None when there is nothing to show.
pub(super) fn view(s: &State, l: &crate::look::controls::Look, teammate: &Teammate) -> Option<El> {
    if !s.reference_picker && s.reference_items.is_empty() && s.reference_note.is_none() {
        return None;
    }
    let mut out = col().w_full().gap(6.0);
    let edge = |e: El| e.radius(l.t.px("radius-control")).border(1.0, l.c("border-control")).fill(l.c("hover-surface"));
    if s.reference_picker {
        let mut picker = col()
            .w_full()
            .gap(8.0)
            .padding(10.0)
            .child(
                row()
                    .w_full()
                    .center_y()
                    .child(text("Add a reference").font(l.ty.control).color(l.c("text-primary")))
                    .child(rune::spacer())
                    .child(
                        crate::look::controls::glyph(l, &crate::look::icons::CLOSE, 12.0, Some(l.c("text-muted")))
                            .padding(4.0)
                            .role("button")
                            .label("Close reference picker")
                            .on_press(|s: &mut State| s.reference_picker = false),
                    ),
            )
            .child(text("Another folder to read alongside this conversation. Your chosen mode stays the same.").font(l.ty.body).color(l.c("text-secondary")).wrap());
        picker = if s.reference_items.len() >= references::MAX {
            picker.child(text("Five references are attached. Remove one to add another.").font(l.ty.body).color(l.c("text-muted")))
        } else {
            picker.child(
                row().child(
                    crate::look::controls::button(l, "Folder on this computer", crate::look::controls::Tone::Neutral, false)
                        .on_press_tx(|s: &mut State, tx: &mut Tx| add_folder(s, tx)),
                ),
            )
        };
        out = out.child(edge(picker));
    }
    if let Some(note) = &s.reference_note {
        out = out.child(text(note.clone()).font(l.ty.mono).color(l.c("text-muted")));
    }
    if !s.reference_items.is_empty() {
        let runtime = teammate.runtime.as_deref().unwrap_or("codex");
        let mut chips = row().w_full().gap(6.0).flex_wrap();
        for item in &s.reference_items {
            let (include, note) = references::access(runtime, &teammate.runs_in, &item.path, teammate.folder.as_deref(), false);
            let id = item.id.clone();
            let mut chip = row()
                .gap(6.0)
                .py(4.0)
                .pl(8.0)
                .pr(2.0)
                .center_y()
                .child(crate::look::controls::glyph(l, &crate::look::icons::FOLDER, 12.0, Some(l.c("text-muted"))))
                .child(
                    col()
                        .gap(2.0)
                        .child(text(item.name.clone()).font(l.ty.mono).color(l.c("text-primary")))
                        .child(text(note).font(l.ty.mono).color(l.c("text-muted"))),
                )
                .child(
                    crate::look::controls::glyph(l, &crate::look::icons::CLOSE, 12.0, Some(l.c("text-muted")))
                        .padding(6.0)
                        .role("button")
                        .label(format!("Remove reference {}", item.name))
                        .on_press(move |s: &mut State| remove(s, &id)),
                )
                .label(item.path.clone());
            chip = edge(chip);
            // `.is-unavailable`: a reference this mode leaves out is drawn dashed in Electron; here, faded.
            if !include {
                chip = chip.opacity(0.6);
            }
            chips = chips.child(chip);
        }
        out = out.child(chips);
    }
    Some(out)
}
