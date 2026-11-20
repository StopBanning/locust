//! A whole web page in a reply, running (Electron `ReplyPage.tsx`, 0.553): a code block that is a whole page gets
//! "Open the page", which writes it to its own folder and runs it in the browser panel at its own address, as a page a
//! teammate wrote to disk opens. Electron runs it on a framed stage with its code a tab away; this window has no frame
//! inside a reply, so the page opens beside the thread and its code stays where it is.

use super::*;
use locust_core::reply_page::{REPLY_PAGES_FOLDER, is_whole_page, write_reply_page};

pub(super) const OPEN_THE_PAGE: &str = "Open the page";
const NOT_OPENED: &str = "This page could not be opened. Its code is in the reply.";

/// The control over a code block that is a whole page, once the block has fully arrived.
pub(super) fn control(source: &str, language: Option<&str>, open: bool, index: usize, p: &Palette) -> Option<El> {
    if open || !is_whole_page(source, language) {
        return None;
    }
    let html = source.to_owned();
    Some(
        el().key(format!("reply-page-{index}"))
            .role("button")
            .label(OPEN_THE_PAGE)
            .child(text(OPEN_THE_PAGE).font(type_::LABEL).color(p.green))
            .on_press(move |s: &mut State| open_page(s, &html)),
    )
}

pub(super) fn open_page(s: &mut State, html: &str) {
    let refused = |s: &mut State, why: String| {
        s.status = why;
        s.status_tone = StatusTone::Muted;
    };
    let Some(store) = s.data.clone() else { return refused(s, NOT_OPENED.into()) };
    let Some(path) = write_reply_page(html, &store.root().join(REPLY_PAGES_FOLDER)) else { return refused(s, NOT_OPENED.into()) };
    let Some(folder) = path.parent().map(std::path::Path::to_path_buf) else { return refused(s, NOT_OPENED.into()) };
    let opened = run_beside(s, &path, &folder);
    if let Err(why) = opened {
        refused(s, why);
    }
}

/// Runs the page at `path` in the browser panel beside the conversation, at its own `locust-page` address under
/// `folder` (page_preview): the same way a reply's page, a teammate's new page and the viewer's Page open.
pub(super) fn run_beside(s: &mut State, path: &std::path::Path, folder: &std::path::Path) -> Result<(), String> {
    let mut host = crate::invariants::lock(&s.browser.host);
    let tab = match host.active {
        Some(id) => id,
        None => host.add(crate::browser::Profile(None))?,
    };
    host.open_page(tab, path, folder)?;
    host.open = true;
    Ok(())
}

/// The Page of a web page in the file viewer and on its turn's card: what it is and where it runs.
pub(super) const RUNS_APART: &str = "It runs at its own address in the browser beside the conversation, and cannot reach Locust, your files or your accounts.";
pub(super) const RUN_THE_PAGE: &str = "Page";

/// The Page control for a web page at `path` (relative to the conversation's folder, or absolute): runs it beside the
/// conversation, or says why not where the person is looking.
pub(super) fn page_control(key: String, path: String, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    crate::look::controls::button(&l, RUN_THE_PAGE, crate::look::controls::Tone::Neutral, false)
        .key(key)
        .on_press(move |s: &mut State| {
            let Some(folder) = selected_conversation(s).and_then(|(t, _)| t.folder.clone()) else {
                s.status = "This conversation has no folder, so the page has nowhere to run.".into();
                s.status_tone = StatusTone::Muted;
                return;
            };
            let absolute = std::path::PathBuf::from(crate::file_menu::rooted_path(&path, Some(&folder)));
            if let Err(why) = run_beside(s, &absolute, std::path::Path::new(&folder)) {
                s.status = why;
                s.status_tone = StatusTone::Muted;
            }
        })
}
