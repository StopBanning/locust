//! A card for each thing a reply says a teammate published (Electron `PublishedCards.tsx`, 0.727): what it is, its
//! name, where it lives, and Open, which goes through the same check as every link in a reply. A refusal is said
//! beside the card that was pressed. The detection is `locust_core::published_links`, kept per reply text so drawing
//! does not read it again.

use super::*;
use locust_core::published_links::{NO_BROWSER, NOT_OPENED, Published, is_web_link, published_in};
use std::collections::HashMap;
use std::sync::Mutex;

static FOUND: Mutex<Option<HashMap<u64, Vec<Published>>>> = Mutex::new(None);

fn found_in(body: &str) -> Vec<Published> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    body.hash(&mut hasher);
    let key = hasher.finish();
    let mut held = FOUND.lock().unwrap_or_else(|e| e.into_inner());
    let map = held.get_or_insert_with(HashMap::new);
    if map.len() > 256 {
        map.clear();
    }
    map.entry(key).or_insert_with(|| published_in(body)).clone()
}

/// Open a reply's web address in the person's browser, or say why not (Electron's openLink).
pub(super) fn open_web_link(s: &mut State, url: &str) {
    s.link_refusal = None;
    if !is_web_link(url) {
        s.link_refusal = Some((url.to_owned(), NOT_OPENED.into()));
        return;
    }
    if cfg!(test) {
        s.opened_links.push(url.to_owned());
        return;
    }
    #[cfg(target_os = "macos")]
    let opened = std::process::Command::new("/usr/bin/open").arg(url).spawn().is_ok();
    #[cfg(target_os = "linux")]
    let opened = std::process::Command::new("xdg-open").arg(url).spawn().is_ok();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let opened = false;
    if !opened {
        s.link_refusal = Some((url.to_owned(), NO_BROWSER.into()));
    }
}

pub(super) fn cards(s: &State, body: &str, p: &Palette) -> Option<El> {
    let items = found_in(body);
    if items.is_empty() {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let mut out = col().w_full().gap(6.0).key("published").label("What this reply links to");
    for (index, item) in items.into_iter().enumerate() {
        let url = item.url.clone();
        let card = row()
            .w_full()
            .gap(10.0)
            .padding(10.0)
            .center_y()
            .radius(8.0)
            .border(1.0, l.c("border-card"))
            .key(format!("published-{index}"))
            .role("button")
            .label(item.url.clone())
            .child(crate::look::controls::glyph(&l, &crate::look::icons::FILE, 16.0, Some(l.c("text-muted"))))
            .child(
                col()
                    .grow(1.0)
                    .gap(2.0)
                    .child(text(item.title.clone()).font(l.ty.control).color(l.c("text-primary")).truncate())
                    .child(text(format!("{} \u{b7} {}", item.kind, item.host)).font(l.ty.body).color(l.c("text-secondary"))),
            )
            .child(text("Open").font(l.ty.body).color(l.c("text-primary")))
            .on_press(move |s: &mut State| open_web_link(s, &url));
        out = out.child(card);
        if let Some((refused_url, why)) = &s.link_refusal
            && *refused_url == item.url
        {
            out = out.child(text(why.clone()).font(l.ty.body).color(l.c("amber-text")).role("status"));
        }
    }
    Some(out)
}
