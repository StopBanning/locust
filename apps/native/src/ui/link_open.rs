//! A `locust://new?prompt=...` link opens a new conversation with its message in the box (Electron App.tsx's link
//! effect and Composer's `fromLink`, 0.770). Never sent: Send is the person's. The box says the words came from a link
//! until they are sent or cleared. Rune hands the link over (`App::on_open_url`), the one that launched the app too;
//! what it holds is `locust_core::locust_link`'s to read.

use super::*;
use locust_core::locust_link::{FROM_A_LINK, parse_locust_link};

/// The hook Rune calls with each link the system opens with this app.
pub(super) fn opened(s: &mut State, url: &str, _tx: &mut Tx) {
    open(s, url);
}

pub(super) fn open(s: &mut State, url: &str) {
    let Some(link) = parse_locust_link(url) else { return };
    s.compare.active_id = None;
    crate::rail::new_conversation(s);
    s.draft.replace_text(&link.prompt);
    s.draft.focused = true;
    s.from_link = true;
}

/// Sent or emptied: the note goes once the box is empty, and does not come back with what is typed next.
pub(super) fn poll(s: &mut State) -> bool {
    if s.from_link && s.draft.text.trim().is_empty() {
        s.from_link = false;
        return true;
    }
    false
}

/// The note over the box, with its Clear.
pub(super) fn note(s: &State, l: &crate::look::controls::Look) -> Option<El> {
    if !s.from_link || s.draft.text.trim().is_empty() {
        return None;
    }
    Some(
        row()
            .w_full()
            .gap(6.0)
            .px(10.0)
            .py(6.0)
            .center_y()
            .radius(l.t.px("radius-control"))
            .border(1.0, l.c("border-card"))
            .key("from-a-link")
            .role("status")
            .child(crate::look::controls::glyph(l, &crate::look::icons::LINK, 12.0, Some(l.c("text-muted"))))
            .child(text(FROM_A_LINK).font(l.ty.body).color(l.c("text-secondary")).grow(1.0))
            .child(
                text("Clear")
                    .font(l.ty.body)
                    .color(l.c("text-primary"))
                    .key("from-a-link-clear")
                    .role("button")
                    .on_press(|s: &mut State| {
                        s.draft.replace_text("");
                        s.from_link = false;
                    }),
            ),
    )
}
