//! Load-to-read tests for runnable HTML reply pages (R153): opening a reply's whole page writes it under
//! `<data_root>/reply-pages/<hash>/index.html`, loads its `locust-page://` address, and `Host::read_preview`
//! (the WKURLSchemeHandler path) serves 200 OK while refusing sibling pages and the rest of `data_root`.
use super::*;
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(name: &str) -> Temp {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "locust-reply-page-ui-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("workspace")).unwrap();
    Temp(root)
}

fn loaded_url(s: &State) -> String {
    let host = crate::invariants::lock(&s.browser.host);
    host.tabs
        .iter()
        .flat_map(|t| t.commands.iter())
        .find_map(|c| match c {
            crate::browser::Command::Load(url) => Some(url.clone()),
            _ => None,
        })
        .expect("a locust-page URL was queued to load")
}

#[test]
fn opening_a_reply_page_loads_and_serves_from_data_root_even_without_a_conversation_folder() {
    let t = temp("no-workspace");
    let store = crate::store::Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    let html = "<!DOCTYPE html>\n<html><body><h1>Reply Game</h1></body></html>";
    open_page(&mut s, html);
    assert!(crate::invariants::lock(&s.browser.host).open, "status: {}", s.status);
    let url = loaded_url(&s);
    assert!(url.starts_with("locust-page://") && url.ends_with("/index.html"), "{url}");
    let res = crate::invariants::lock(&s.browser.host).read_preview(&url);
    assert_eq!(res.status, 200, "body: {}", String::from_utf8_lossy(&res.body));
    assert!(res.mime.contains("text/html"), "{}", res.mime);
    assert_eq!(res.body, html.as_bytes());
}

#[test]
fn reply_page_serves_while_confined_to_its_own_folder_and_never_exposes_siblings_or_data_root() {
    let t = temp("confined");
    let store = crate::store::Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::write(store.root().join("teammates.json"), "{\"secret\":true}").unwrap();
    std::fs::write(store.root().join("leak.html"), "<h1>Leak</h1>").unwrap();
    let other_page = write_reply_page(
        "<!DOCTYPE html>\n<html><body><h1>Other</h1></body></html>",
        &store.root().join(REPLY_PAGES_FOLDER),
    )
    .unwrap();
    let other_dir = other_page.parent().unwrap().file_name().unwrap().to_string_lossy().into_owned();

    let mut s = State::default();
    s.data = Some(store.clone());
    // An unrelated conversation workspace is open; the reply page still serves from data_root/reply-pages.
    crate::invariants::lock(&s.browser.host).workspace = Some(t.0.join("workspace"));
    let html = "<!DOCTYPE html>\n<html><body><h1>Mine</h1></body></html>";
    open_page(&mut s, html);
    let url = loaded_url(&s);
    let host = crate::invariants::lock(&s.browser.host);
    assert_eq!(host.read_preview(&url).status, 200);
    assert_eq!(host.read_preview(&url).body, html.as_bytes());
    // Sibling reply page and files above reply-pages in data_root are refused.
    assert_ne!(host.read_preview(&url.replace("index.html", &format!("../{other_dir}/index.html"))).status, 200);
    assert_ne!(host.read_preview(&url.replace("index.html", "../../teammates.json")).status, 200);
    drop(host);

    // Even if an HTML page sat directly in data_root (outside reply-pages/ and compare/), read_preview refuses it.
    let mut host = crate::invariants::lock(&s.browser.host);
    let tab = host.active.unwrap();
    host.open_page(tab, &store.root().join("leak.html"), store.root()).unwrap();
    let leak_url = host
        .tabs
        .iter()
        .flat_map(|t| t.commands.iter())
        .filter_map(|c| match c {
            crate::browser::Command::Load(u) => Some(u.clone()),
            _ => None,
        })
        .last()
        .unwrap();
    assert_eq!(host.read_preview(&leak_url).status, 403);
}

#[test]
fn mac_scheme_handler_reads_previews_through_host_read_preview() {
    let mac = include_str!("../browser_mac.rs");
    assert!(mac.contains("h.read_preview("), "browser_mac.rs must route WKURLSchemeHandler through Host::read_preview");
    assert!(!mac.contains("h.preview.read("), "browser_mac.rs must not bypass Host::read_preview");
}

// My break: without the early check in `page_control`, a file that is not a web page still reached `run_beside`,
// which made a browser tab before refusing it. Nothing noticed (28 Nov 2026).
#[test]
fn pressing_page_on_a_file_that_is_not_a_web_page_says_so_and_makes_no_browser_tab() {
    let t = temp("not-a-page");
    let store = crate::store::Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    let workspace = t.0.join("workspace");
    std::fs::write(workspace.join("notes.txt"), "plain words").unwrap();
    let mut s = State::default();
    s.data = Some(store);
    s.home.folder_path = Some(workspace.to_string_lossy().into_owned());
    let mut rt = rune::Runtime::new(
        s,
        |_, _| rune::col().w_full().h_full().child(page_control("page-run".into(), "notes.txt".into(), &Palette::new(true))),
        (400.0, 200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let (x, y) = rt.find("page-run").expect("the Page control is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(rt.state.status, "Only a web page (.html) opens as a page.");
    let host = crate::invariants::lock(&rt.state.browser.host);
    assert!(host.tabs.is_empty(), "no tab is made for a file that cannot run as a page");
    assert!(!host.open, "and the browser stays closed");
}
