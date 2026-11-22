//! A page a teammate made runs (Electron `renderer/src/a-page-opens-running.test.tsx`, 0.425, and
//! `a-page-a-teammate-made-runs-in-its-turn.test.tsx`, 0.580), drawn headless.
//!
//! Electron runs the page in a sandboxed frame inside the viewer and inside the turn's card. This window has no frame
//! inside the thread: a page's Page runs it at its own `locust-page` address in the browser panel beside the
//! conversation (page_preview; WKWebView gives a preview page a store of its own and blocks the network), and the
//! viewer and the card's "Show the change" are its source. Which page opens, and what stays folded, is Electron's.
use super::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(name: &str) -> (Temp, PathBuf) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!("locust-page-runs-{name}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&root).unwrap();
    let real = root.canonicalize().unwrap();
    (Temp(root), real)
}

const PAGE: &str = "--- /dev/null\n+++ b/index.html\n@@ -0,0 +1,2 @@\n+<!doctype html>\n+<h1>Cozy Corner Coffee</h1>\n";
const CHANGED_PAGE: &str = "--- a/index.html\n+++ b/index.html\n@@ -1,2 +1,2 @@\n <!doctype html>\n-<h1>Old</h1>\n+<h1>New</h1>\n";
const SCRIPT: &str = "--- /dev/null\n+++ b/build.ts\n@@ -0,0 +1,1 @@\n+export const x = 1\n";

/// A finished turn that wrote each `(path, patch)`, through Electron's events.
fn turn(writes: &[(&str, &str)]) -> Vec<ledger::Item> {
    let mut rows: Vec<(&str, Value)> = vec![("run.started", json!({}))];
    for (i, (path, patch)) in writes.iter().enumerate() {
        let id = format!("w{i}");
        let patch = json!({"text": patch, "truncated": false, "added": 1, "removed": 0});
        rows.push(("tool.started", json!({"itemId": id, "toolKind": "file_change", "name": "Write", "command": path, "phase": "started"})));
        rows.push(("tool.completed", json!({"itemId": id, "toolKind": "file_change", "name": "Write", "command": path, "phase": "completed", "patch": patch})));
    }
    rows.push(("message.delta", json!({"itemId": "m", "operation": "append", "text": "Done.", "final": true})));
    rows.push(("run.completed", json!({"process": {"exitCode": 0}})));
    let mut t = locust_core::ledger::Transcript::default();
    t.apply(
        rows.iter()
            .enumerate()
            .map(|(i, (kind, payload))| json!({"recordType": "mission.event", "event": {"type": kind, "sequence": i + 1, "occurredAt": "2026-11-19T05:00:00.000Z", "payload": payload}}))
            .collect(),
    );
    t.items
}

fn state(folder: &Path) -> State {
    let mut s = State::default();
    s.teammates.push(crate::Teammate { id: "tm".into(), name: "Codec".into(), role: "Custom".into(), model: None, runtime: Some("opencode".into()), folder: Some(folder.to_string_lossy().into_owned()), runs_in: "ask".into(), account: None, busy: false });
    s.conversations.insert("tm".into(), Default::default());
    s
}

/// The thread of a conversation in `folder` with `items`, drawn as the window draws an open conversation.
fn thread(folder: &Path, items: Vec<ledger::Item>) -> rune::Runtime<State> {
    let mut s = state(folder);
    let header = s.teammates[0].clone();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some("mission_page".into()), turns: Vec::new() },
        items,
        dir: PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { turn_metas: vec![ledger::Metadata { mission_id: "mission_page".into(), ..Default::default() }], ..Default::default() },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = rune::Runtime::new(s, |s, _| message_column(&s.ledger.as_ref().unwrap().items, "Codec", &Palette::new(true), s), (800.0, 2400.0), rune::Env::new(true, 1.0));
    for _ in 0..4 {
        rt.frame(0.05);
    }
    rt
}

fn texts<T>(rt: &rune::Runtime<T>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n")
}

fn keys<T>(rt: &rune::Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.key).collect()
}

/// A row's change, drawn open: the diff heads itself with the file's state in lower case (review_changes::file_diff).
/// Its code lines are drawn token by token, so a line's words are never one text; the state is what can be read.
fn a_diff_is_open(said: &str) -> bool {
    said.lines().any(|l| l == "added" || l == "modified")
}

/// The turn's files card (its rows are keyed under the turn's foot) runs a page.
fn running(keys: &[String]) -> bool {
    keys.iter().any(|k| k.contains("/foot/") && k.ends_with("/page"))
}

// a-page-a-teammate-made-runs-in-its-turn.test.tsx

#[test]
fn opens_a_new_web_page_so_it_runs() {
    let (_t, work) = temp("new");
    let rt = thread(&work, turn(&[("index.html", PAGE)]));
    let said = texts(&rt);
    assert!(running(&keys(&rt)), "{said}");
    assert!(said.contains("index.html, running"), "{said}");
    assert!(said.contains(crate::reply_page_ui::RUNS_APART), "{said}");
    assert!(!a_diff_is_open(&said), "the page, not its change: {said}");
    // Its source a press away, which is also the control for `a_diff_is_open`.
    let mut rt = rt;
    let switch = rt.elements().into_iter().find(|e| e.label.as_deref() == Some("Show the change") || e.text.as_deref() == Some("Show the change")).expect("Show the change");
    let [x, y, w, h] = switch.rect;
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    for _ in 0..3 {
        rt.frame(0.05);
    }
    let said = texts(&rt);
    assert!(a_diff_is_open(&said), "{said}");
}

#[test]
fn opens_the_page_even_when_another_file_came_first_and_only_the_page() {
    let (_t, work) = temp("second");
    let rt = thread(&work, turn(&[("build.ts", SCRIPT), ("index.html", PAGE)]));
    let said = texts(&rt);
    assert!(running(&keys(&rt)), "{said}");
    assert!(said.contains("build.ts"), "its row is there, folded: {said}");
    assert!(!said.contains("export const x") && !a_diff_is_open(&said), "{said}");
}

#[test]
fn leaves_a_changed_page_and_any_other_new_file_folded_as_before() {
    let (_t, work) = temp("folded");
    let rt = thread(&work, turn(&[("index.html", CHANGED_PAGE)]));
    let said = texts(&rt);
    assert!(!running(&keys(&rt)), "{said}");
    assert!(!said.contains("<h1>New</h1>") && !a_diff_is_open(&said), "{said}");
    let rt = thread(&work, turn(&[("build.ts", SCRIPT)]));
    let said = texts(&rt);
    assert!(said.contains("build.ts"), "{said}");
    assert!(!said.contains("export const x") && !a_diff_is_open(&said), "{said}");
}

/// The page's Page, pressed: the page is loaded in the browser panel at its own address, and the panel opens.
fn press<T>(rt: &mut rune::Runtime<T>, ending: &str) {
    let key = keys(rt).into_iter().find(|k| k.ends_with(ending)).unwrap_or_else(|| panic!("no {ending}"));
    let (x, y) = rt.find(&key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

fn loaded(s: &State) -> Vec<String> {
    let host = crate::invariants::lock(&s.browser.host);
    host.tabs.iter().flat_map(|t| t.commands.iter()).filter_map(|c| if let crate::browser::Command::Load(url) = c { Some(url.clone()) } else { None }).collect()
}

#[test]
fn runs_the_new_page_beside_the_conversation_at_its_own_address_when_its_page_is_pressed() {
    let (_t, work) = temp("press");
    std::fs::write(work.join("index.html"), "<!doctype html>\n<h1>Cozy Corner Coffee</h1>\n").unwrap();
    let mut rt = thread(&work, turn(&[("index.html", PAGE)]));
    assert!(loaded(&rt.state).is_empty(), "nothing runs until the person asks");
    // A tab already there, the panel shut: the page runs in that tab, and the panel opens on it.
    {
        let mut host = crate::invariants::lock(&rt.state.browser.host);
        host.add(crate::browser::Profile(None)).unwrap();
        host.open = false;
    }
    // The files card's Page (its keys are under the turn's foot), not the step card's.
    let page = keys(&rt).into_iter().find(|k| k.contains("/foot/") && k.ends_with("/run")).expect("the files card's Page");
    let (x, y) = rt.find(&page).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    let urls = loaded(&rt.state);
    assert_eq!(urls.len(), 1, "{urls:?} status: {}", rt.state.status);
    assert!(urls[0].starts_with("locust-page://") && urls[0].ends_with("/index.html"), "{urls:?}");
    assert!(crate::invariants::lock(&rt.state.browser.host).open);
}

// a-page-opens-running.test.tsx: the viewer.

fn viewer(folder: &Path, path: &str, body: &str) -> rune::Runtime<State> {
    let mut s = state(folder);
    s.file_view = Some((reply_actions::FileTarget { path: path.into(), start: 1, end: 1 }, Ok(body.into())));
    let mut rt = rune::Runtime::new(s, |s, _| reply_actions::viewer(s, &Palette::new(true)).unwrap_or_else(col), (800.0, 1200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

#[test]
fn runs_a_page_in_a_sandboxed_frame_at_its_own_address_with_page_and_source() {
    let (_t, work) = temp("viewer");
    std::fs::create_dir_all(work.join("site")).unwrap();
    std::fs::write(work.join("site/index.html"), "<h1>Corner Shop</h1>").unwrap();
    let mut rt = viewer(&work, "site/index.html", "<h1>Corner Shop</h1>");
    let said = texts(&rt);
    assert!(said.lines().any(|l| l == "Page"), "{said}");
    assert!(said.lines().any(|l| l == "Source"), "{said}");
    assert!(said.contains("cannot reach Locust, your files or your accounts"), "{said}");
    press(&mut rt, "file-reader/page");
    let urls = loaded(&rt.state);
    assert_eq!(urls.len(), 1, "{urls:?} status: {}", rt.state.status);
    // Its own address: the page's folder under a token, never the file path or the project above it.
    assert!(urls[0].starts_with("locust-page://") && urls[0].ends_with("/index.html") && !urls[0].contains("site/"), "{urls:?}");
}

#[test]
fn runs_nothing_without_a_page_address_the_same_file_shows_as_its_source() {
    let (_t, work) = temp("source");
    let rt = viewer(&work, "site/index.html", "<h1>Corner Shop</h1>");
    assert!(texts(&rt).contains("<h1>Corner Shop</h1>"), "the page's own words, as text");
    assert!(loaded(&rt.state).is_empty(), "opening the viewer runs nothing");
    // Any other file has no Page at all.
    let rt = viewer(&work, "src/cart.ts", "export const x = 1");
    assert!(!keys(&rt).iter().any(|k| k == "file-reader/page"), "{:?}", keys(&rt));
}
