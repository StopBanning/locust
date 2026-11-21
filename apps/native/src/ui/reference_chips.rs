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
        // Answers are heard only for the chips they were asked for: the key is settled first.
        return heard(s);
    }
    s.references_key = key;
    // A download that was running for the chips that went is kept by nothing: heard discards it, now the key is new.
    heard(s);
    s.reference_menu = false;
    s.reference_picker = false;
    s.reference_note = None;
    s.reference_work.github = false;
    s.reference_work.busy = false;
    s.reference_work.repository = TextEdit::default();
    s.reference_work.cwd = conversation_now(s).zip(s.data.clone()).and_then(|(id, store)| references::cwd(&store, &id));
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
    refresh(s);
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
    if s.reference_work.busy || s.reference_items.len() >= references::MAX {
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
            let busy = s.reference_work.busy;
            let choices = row()
                .gap(6.0)
                .child(
                    crate::look::controls::button(l, "Folder on this computer", crate::look::controls::Tone::Neutral, busy)
                        .key("reference-add-folder")
                        .on_press_tx(|s: &mut State, tx: &mut Tx| add_folder(s, tx)),
                )
                .child(
                    crate::look::controls::button(l, "GitHub repository", crate::look::controls::Tone::Neutral, busy)
                        .key("reference-github")
                        .on_press(|s: &mut State| s.reference_work.github = true),
                );
            picker = picker.child(choices);
            if s.reference_work.github {
                let empty = s.reference_work.repository.text.trim().is_empty();
                picker = picker.child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .child(text("GitHub repository").font(l.ty.body).color(l.c("text-secondary")))
                        .child(
                            row()
                                .w_full()
                                .gap(6.0)
                                .center_y()
                                .child(
                                    text_input(&s.reference_work.repository, l.ty.control)
                                        .key("reference-repository")
                                        .label("GitHub reference repository")
                                        .grow(1.0)
                                        .color(l.c("text-primary"))
                                        .placeholder("owner/repo or https://github.com/owner/repo", l.c("text-faint"))
                                        .fill(l.c("bg-raised"))
                                        .padding(8.0)
                                        .radius(l.t.px("radius-button"))
                                        .on_input_key(|s: &mut State, key: &KeyEvent, _: &mut Tx| {
                                            if key.key == Key::Enter {
                                                add_github(s);
                                                true
                                            } else {
                                                false
                                            }
                                        })
                                        .on_edit(|s: &mut State| &mut s.reference_work.repository),
                                )
                                .child(
                                    crate::look::controls::button(l, "Add repository", crate::look::controls::Tone::Neutral, busy || empty)
                                        .key("reference-add-repository")
                                        .on_press(add_github),
                                ),
                        )
                        .child(
                            text("A shallow copy, up to 500 MB, kept on this computer. Private repositories use your gh sign-in; Locust never signs you in.")
                                .font(l.ty.body)
                                .color(l.c("text-muted"))
                                .wrap(),
                        ),
                );
            }
            picker
        };
        out = out.child(edge(picker));
    }
    if s.reference_work.busy {
        out = out.child(text("Adding reference…").font(l.ty.mono).color(l.c("text-muted")).role("status"));
    }
    if let Some(note) = &s.reference_note {
        out = out.child(text(note.clone()).font(l.ty.mono).color(l.c("text-muted")));
    }
    if !s.reference_items.is_empty() {
        let runtime = teammate.runtime.as_deref().unwrap_or("codex");
        let mut chips = row().w_full().gap(6.0).flex_wrap();
        for item in &s.reference_items {
            // Against the folder this conversation's turns stood in, when it has one (Electron's hostCwd).
            let folder = s.reference_work.cwd.as_deref().or(teammate.folder.as_deref());
            let (include, note) = references::access(runtime, &teammate.runs_in, &item.path, folder, false);
            let note = match &item.problem {
                Some(problem) => format!("{note} {problem}"),
                None => note.to_owned(),
            };
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

/// What a download answers, on its own thread: the chips' key it was asked for, and the copy or why not.
pub(crate) enum Answer {
    Listed(String, Result<Vec<references::Item>, String>),
    Added(String, Result<references::Item, String>),
}

/// The GitHub half of the picker: the box, a download running, and where its answers arrive.
pub(crate) struct Work {
    pub(crate) github: bool,
    pub(crate) repository: TextEdit,
    pub(crate) busy: bool,
    /// The folder this conversation's turns stood in (references.cwd), which the chips' notes are worked out against.
    pub(crate) cwd: Option<String>,
    tx: std::sync::mpsc::Sender<Answer>,
    rx: std::sync::mpsc::Receiver<Answer>,
}

impl Default for Work {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Work { github: false, repository: TextEdit::default(), busy: false, cwd: None, tx, rx }
    }
}

/// A conversation's stale GitHub copies are downloaded again off the window's thread; the chips change when they land.
fn refresh(s: &mut State) {
    let (Some(id), Some(store)) = (conversation_now(s), s.data.clone()) else { return };
    let now = locust_core::pull_requests::now_ms();
    if !s.reference_items.iter().any(|item| references::needs_refresh(item, now)) {
        return;
    }
    let (key, tx) = (s.references_key.clone(), s.reference_work.tx.clone());
    std::thread::spawn(move || {
        let _ = tx.send(Answer::Listed(key, references::list(&store, &references::Copies::default(), Some(&id))));
    });
}

/// Answers from the download thread, for the chips still on screen.
fn heard(s: &mut State) -> bool {
    let mut changed = false;
    while let Ok(answer) = s.reference_work.rx.try_recv() {
        changed = true;
        match answer {
            Answer::Listed(key, listed) if key == s.references_key => match listed {
                Ok(items) => s.reference_items = items,
                Err(why) => s.reference_note = Some(why),
            },
            Answer::Listed(..) => {}
            Answer::Added(key, added) => {
                let here = key == s.references_key;
                if here {
                    s.reference_work.busy = false;
                }
                match added {
                    Ok(item) if here && s.reference_items.len() < references::MAX => {
                        s.reference_items.push(item);
                        s.reference_picker = false;
                        s.reference_work.github = false;
                        s.reference_work.repository = TextEdit::default();
                        s.reference_note = None;
                        save(s);
                    }
                    // Asked for in a conversation no longer on screen: the copy is kept by nothing, so it goes.
                    Ok(item) => {
                        if let Some(store) = s.data.clone() {
                            let _ = references::discard(&store, &item.id);
                        }
                    }
                    Err(why) if here => s.reference_note = Some(why),
                    Err(_) => {}
                }
            }
        }
    }
    changed
}

fn add_github(s: &mut State) {
    if s.reference_work.busy || s.reference_items.len() >= references::MAX || s.reference_work.repository.text.trim().is_empty() {
        return;
    }
    let Some(store) = s.data.clone() else { return };
    // The box takes 240 characters, as Electron's does.
    let value: String = s.reference_work.repository.text.chars().take(240).collect();
    s.reference_work.busy = true;
    s.reference_note = None;
    let (key, tx) = (s.references_key.clone(), s.reference_work.tx.clone());
    std::thread::spawn(move || {
        let _ = tx.send(Answer::Added(key, references::github(&store, &references::Copies::default(), &value)));
    });
}
