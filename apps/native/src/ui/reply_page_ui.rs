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
    let opened = {
        let mut host = crate::invariants::lock(&s.browser.host);
        host.preview.url_for(&path, Some(&folder)).and_then(|url| {
            let tab = match host.active {
                Some(id) => id,
                None => host.add(crate::browser::Profile(None))?,
            };
            host.navigate(tab, &url, true)?;
            host.open = true;
            Ok(())
        })
    };
    if let Err(why) = opened {
        refused(s, why);
    }
}
