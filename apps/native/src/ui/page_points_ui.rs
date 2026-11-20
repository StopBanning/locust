//! Point at a part of the browser page (Electron `BrowserPanel.tsx` BrowserPointButton and `PagePointChips.tsx`,
//! 0.660): the point button in the browser panel (and Ctrl+Shift+C, ⌘⇧C here), the parts waiting in the chat box with
//! a note each and a way to remove them, and Send, which carries each as a block a teammate reads as page content and
//! its picture as an attachment. What is read, and whether a picture may be taken, is `locust_core::browser_point`'s;
//! the page itself is run by the WKWebView host (`browser_mac::point`).

use super::*;
use locust_core::browser_point::{
    BROWSER_POINT_KEYS, PagePointChip, PointAnswer, PointControl, add_page_point, begin_browser_point, browser_point_control, is_browser_point_shortcut,
    message_of_page_points, note_on_page_point, style_summary, without_page_point,
};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::time::{Duration, Instant};

/// How long a pick may wait on the person before it is let go (Electron `PICK_WAIT_MS`).
const PICK_WAIT: Duration = Duration::from_secs(5 * 60);
pub(super) const PAGE_PARTS_FOLDER: &str = "page-parts";

#[derive(Default)]
pub(super) struct PagePoints {
    pub chips: Vec<PagePointChip>,
    /// Each chip's note as it is typed, by chip id.
    pub notes: HashMap<String, TextEdit>,
    /// A pick running in a tab: its answer, when it began, and the tab.
    pub waiting: Option<(Receiver<PointAnswer>, Instant, u64)>,
    /// The address of a page that would not run the picker.
    pub blocked: Option<String>,
    pub next: u64,
}

fn active_tab(s: &State) -> Option<(u64, String)> {
    active_of(&crate::invariants::lock(&s.browser.host))
}

fn active_of(host: &crate::browser::Host) -> Option<(u64, String)> {
    host.tabs.iter().find(|t| Some(t.id) == host.active).map(|t| (t.id, t.url.clone()))
}

pub(super) fn control(s: &State) -> PointControl {
    control_of(&crate::invariants::lock(&s.browser.host), s)
}

/// The button's state from a host already held (the panel draws with it locked).
pub(super) fn control_of(host: &crate::browser::Host, s: &State) -> PointControl {
    let page = active_of(host).filter(|(_, url)| !url.is_empty());
    let blocked = page.as_ref().is_some_and(|(_, url)| s.page_points.blocked.as_deref() == Some(url.as_str()));
    browser_point_control(host.refusal.as_deref(), blocked, s.page_points.waiting.is_some(), page.is_some())
}

/// The browser panel's point button: its name is what it would do, or why it cannot.
pub(super) fn point_button(c: PointControl, p: &Palette) -> El {
    let mut button = el()
        .key("browser-point")
        .role("button")
        .label(c.label.clone())
        .shortcut_hint(BROWSER_POINT_KEYS)
        .disabled(c.disabled)
        .child(text(if c.sentence { c.label.clone() } else if c.pressed { "Stop pointing".into() } else { "Point".into() }).font(type_::LABEL).color(if c.disabled { p.muted } else if c.pressed { p.green } else { p.secondary }));
    if c.pressed {
        button = button.checked(true);
    }
    if !c.disabled {
        button = button.on_press(toggle);
    }
    button
}

/// Starts a pick in the open tab, or lets the running one go.
pub(super) fn toggle(s: &mut State) {
    if let Some((_, _, tab)) = &s.page_points.waiting {
        push(s, *tab, crate::browser::Command::CancelPoint);
        return;
    }
    let refusal = crate::invariants::lock(&s.browser.host).refusal.clone();
    if let Err(why) = begin_browser_point(refusal.as_deref()) {
        s.status = why;
        s.status_tone = StatusTone::Muted;
        return;
    }
    if control(s).disabled {
        return;
    }
    let Some((tab, _)) = active_tab(s) else { return };
    let (reply, answer) = sync_channel(1);
    push(s, tab, crate::browser::Command::Point { reply });
    s.page_points.waiting = Some((answer, Instant::now(), tab));
}

fn push(s: &State, tab: u64, command: crate::browser::Command) {
    let mut host = crate::invariants::lock(&s.browser.host);
    if let Some(t) = host.tabs.iter_mut().find(|t| t.id == tab) {
        t.commands.push_back(command);
    }
}

/// Ctrl+Shift+C (⌘⇧C) points while the browser is open.
pub(super) fn key(s: &mut State, event: &KeyEvent) -> bool {
    let c = match &event.key {
        Key::Command(k) => *k,
        Key::Text(t) if event.mods.ctrl => t.chars().next().unwrap_or(' '),
        _ => return false,
    };
    if !is_browser_point_shortcut(event.mods.ctrl, event.mods.cmd, event.mods.shift, event.mods.alt, c) || !crate::invariants::lock(&s.browser.host).open {
        return false;
    }
    toggle(s);
    true
}

/// The pick's answer, when it has come: a part goes into the box with its picture; a page that will not run the
/// picker is remembered so its button says so; a pick left too long is let go.
pub(super) fn poll(s: &mut State) -> bool {
    let Some((answer, began, tab)) = &s.page_points.waiting else { return false };
    let (began, tab) = (*began, *tab);
    let answer = match answer.try_recv() {
        Ok(answer) => answer,
        Err(TryRecvError::Empty) => {
            if began.elapsed() > PICK_WAIT {
                push(s, tab, crate::browser::Command::CancelPoint);
                s.page_points.waiting = None;
                return true;
            }
            return false;
        }
        Err(TryRecvError::Disconnected) => PointAnswer::Nothing,
    };
    s.page_points.waiting = None;
    take_answer(s, answer);
    true
}

pub(super) fn take_answer(s: &mut State, answer: PointAnswer) {
    match answer {
        PointAnswer::Nothing => {}
        PointAnswer::Refused { message, blocked } => {
            if blocked {
                s.page_points.blocked = active_tab(s).map(|(_, url)| url);
            }
            s.status = message;
            s.status_tone = StatusTone::Muted;
        }
        PointAnswer::Picked { point, png } => {
            s.page_points.next += 1;
            let id = format!("point-{}", s.page_points.next);
            let attachment = png.and_then(|png| keep_picture(s, &id, &png));
            let chip = PagePointChip { id: id.clone(), point, note: String::new(), attachment };
            match add_page_point(&mut s.page_points.chips, chip) {
                Ok(()) => {
                    s.page_points.notes.insert(id, TextEdit::default());
                }
                Err(why) => {
                    s.status = why.into();
                    s.status_tone = StatusTone::Muted;
                }
            }
        }
    }
}

/// The picture of a part, kept in the app's own data folder until it is sent with the message.
fn keep_picture(s: &State, id: &str, png: &[u8]) -> Option<String> {
    let folder = s.data.as_ref()?.root().join(PAGE_PARTS_FOLDER);
    std::fs::create_dir_all(&folder).ok()?;
    let path = folder.join(format!("{id}-{}.png", locust_core::native::uuid_v4().ok()?.replace('-', "")));
    std::fs::write(&path, png).ok()?;
    Some(path.to_string_lossy().into_owned())
}

fn remove(s: &mut State, id: &str) {
    if let Some(path) = s.page_points.chips.iter().find(|c| c.id == id).and_then(|c| c.attachment.clone()) {
        let _ = std::fs::remove_file(path);
    }
    without_page_point(&mut s.page_points.chips, id);
    s.page_points.notes.remove(id);
}

/// The parts waiting in the chat box. Nothing here is sent until Send; Tab reaches each note and remove button.
pub(super) fn chips(s: &State, p: &Palette) -> Option<El> {
    if s.page_points.chips.is_empty() {
        return None;
    }
    let mut list = col().w_full().gap(6.0).key("page-points").role("group").label("Pointed parts of the page");
    for chip in &s.page_points.chips {
        let id = chip.id.clone();
        let point = &chip.point;
        let mut body = col().grow(1.0).shrink().gap(2.0).child(text(point.selector().to_owned()).font(type_::LABEL.mono()).color(p.text).truncate().help(point.url().to_owned()));
        if let Some(source) = point.source() {
            body = body.child(text(source.words()).key(format!("{id}/source")).font(type_::LABEL.mono()).color(p.secondary).truncate());
        }
        if !point.styles().is_empty() {
            body = body.child(text(style_summary(point.styles(), 4)).key(format!("{id}/styles")).font(type_::LABEL).color(p.muted).wrap_anywhere());
        }
        if let Some(note) = point.picture_note() {
            body = body.child(text(note.to_owned()).key(format!("{id}/picture-note")).font(type_::LABEL).color(p.amber).wrap_anywhere());
        }
        let edit = s.page_points.notes.get(&id).cloned().unwrap_or_default();
        let at = id.clone();
        body = body.child(
            text_input(&edit, type_::LABEL)
                .key(format!("{id}/note"))
                .label(format!("Note on {}", point.selector()))
                .placeholder("Add a note", p.muted)
                .color(p.text)
                .padding(4.0)
                .fill(p.raised)
                .w_full()
                .on_edit(move |s: &mut State| s.page_points.notes.entry(at.clone()).or_default()),
        );
        let mut row_el = row().w_full().gap(8.0).padding(6.0).radius(8.0).border(1.0, p.hairline).key(format!("{id}/chip"));
        if let Some(picture) = &chip.attachment {
            row_el = row_el.child(el().key(format!("{id}/picture")).role("img").label(format!("Picture of {}", point.selector())).help(picture.clone()).padding(4.0).radius(4.0).fill(p.raised).child(text("Picture").font(type_::LABEL).color(p.secondary)));
        }
        let gone = id.clone();
        row_el = row_el.child(body).child(
            text("×")
                .key(format!("{id}/remove"))
                .role("button")
                .label(format!("Remove {}", point.selector()))
                .help("Remove this part and its picture")
                .font(type_::LABEL)
                .color(p.muted)
                .focusable(true)
                .on_press(move |s: &mut State| remove(s, &gone)),
        );
        list = list.child(row_el);
    }
    Some(list)
}

/// The message with the waiting parts after the person's words, and the attachments with their pictures; the box is
/// left as it is, so a message that is not taken (a queue already full) keeps them.
pub(super) fn with_points(s: &State, typed: &str, attached: &[String]) -> (String, Vec<String>) {
    let mut attached = attached.to_vec();
    if s.page_points.chips.is_empty() {
        return (typed.to_owned(), attached);
    }
    let mut chips = s.page_points.chips.clone();
    for (id, edit) in &s.page_points.notes {
        note_on_page_point(&mut chips, id, &edit.text);
    }
    let (blocks, pictures) = message_of_page_points(&chips);
    for picture in pictures {
        if !attached.contains(&picture) {
            attached.push(picture);
        }
    }
    (if typed.trim().is_empty() { blocks } else { format!("{typed}\n\n{blocks}") }, attached)
}

/// The box emptied, once its parts went with a message; their pictures stay on disk for it.
pub(super) fn sent(s: &mut State) {
    s.page_points.chips.clear();
    s.page_points.notes.clear();
}

/// The message with the waiting parts after the person's words, and their pictures attached; the box is emptied.
pub(super) fn take_into(s: &mut State, typed: &str) -> String {
    let (message, attached) = with_points(s, typed, &s.attached);
    s.attached = attached;
    sent(s);
    message
}

#[cfg(test)]
#[path = "page_points_tests.rs"]
mod tests;
