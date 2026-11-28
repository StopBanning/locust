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

// case: opens a new web page, so it runs
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

// case: opens the page even when another file came first, and only the page
#[test]
fn opens_the_page_even_when_another_file_came_first_and_only_the_page() {
    let (_t, work) = temp("second");
    let rt = thread(&work, turn(&[("build.ts", SCRIPT), ("index.html", PAGE)]));
    let said = texts(&rt);
    assert!(running(&keys(&rt)), "{said}");
    assert!(said.contains("build.ts"), "its row is there, folded: {said}");
    assert!(!said.contains("export const x") && !a_diff_is_open(&said), "{said}");
}

// case: stays folded in a comparison's cell, whose page runs above it
#[test]
fn stays_folded_in_a_comparisons_cell_whose_page_runs_above_it() {
    use locust_core::compare::ChangesIn;
    let (_t, root) = temp("cmp-cell");
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let folder = root.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let copy_a = locust_core::compare_copies::compare_root(&store).join("cmp_cell1-a");
    std::fs::create_dir_all(&copy_a).unwrap();
    std::fs::write(copy_a.join("index.html"), "<!doctype html>\n<h1>Cozy Corner Coffee</h1>").unwrap();

    let mut s = state(&folder);
    s.data = Some(store);
    s.compare.compares = vec![compare_record("cmp_cell1", Some(ChangesIn::Copy), None)];
    s.compare.active_id = Some("cmp_cell1".into());
    s.compare.loaded = true;
    let mut items = turn(&[("index.html", PAGE)]);
    for item in &mut items {
        if let ledger::Item::Tool { detail, .. } = item {
            detail.arguments = json!({"file_path": "index.html"}).to_string();
        }
    }
    s.compare.snapshots.insert(
        "m_a".into(),
        ledger::Snapshot { tip: "m_a".into(), items, ..Default::default() },
    );
    s.compare.snapshots.insert(
        "m_b".into(),
        ledger::Snapshot { tip: "m_b".into(), items: vec![ledger::Item::Assistant { item_id: "mb".into(), text: "Done.".into(), r#final: true }], ..Default::default() },
    );

    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::compare_view::view(s, &Palette::new(true)),
        (1200.0, 1200.0),
        rune::Env::new(true, 1.0),
    );
    for _ in 0..3 {
        rt.frame(0.05);
    }
    let all_keys = keys(&rt);
    // The comparison cell exposes its single column preview button and never opens a second turn-files page card.
    assert!(all_keys.iter().any(|k| k == "compare-preview-a-index.html"), "{all_keys:?}");
    assert!(!running(&all_keys), "comparison cell must not draw a second page preview card: {all_keys:?}");
    press(&mut rt, "compare-preview-a-index.html");
    crate::browser_panel::poll(&mut rt.state, &mut Tx::default());
    let urls = loaded(&rt.state);
    assert_eq!(urls.len(), 1, "{urls:?} problem: {:?}", rt.state.compare.problem);
    let res = crate::invariants::lock(&rt.state.browser.host).read_preview(&urls[0]);
    assert_eq!(res.status, 200);
    assert_eq!(res.body, b"<!doctype html>\n<h1>Cozy Corner Coffee</h1>");
}

// case: leaves a changed page and any other new file folded, as before
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
    let host = crate::invariants::lock(&rt.state.browser.host);
    assert!(host.open);
    let res = host.read_preview(&urls[0]);
    assert_eq!(res.status, 200, "body: {}", String::from_utf8_lossy(&res.body));
    assert_eq!(res.body, b"<!doctype html>\n<h1>Cozy Corner Coffee</h1>\n");
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

// R92: local page preview and comparison page previews (load-to-read + workspace confinement).

fn compare_record(id: &str, changes_in: Option<locust_core::compare::ChangesIn>, kept: Option<locust_core::compare::CompareSlotId>) -> locust_core::compare::Compare {
    use locust_core::compare::{Compare, CompareKept, CompareRoute, CompareSlot, CompareSlotId};
    Compare {
        compare_id: id.into(),
        teammate_id: None,
        prompt: "Build a page".into(),
        created_at: "2026-11-27T00:00:00Z".into(),
        slots: vec![
            CompareSlot { slot: CompareSlotId::A, route: CompareRoute::new("claude", "sonnet"), mission_ids: vec!["m_a".into()], refused: None, retried: vec![], folder: None },
            CompareSlot { slot: CompareSlotId::B, route: CompareRoute::new("codex", "o3"), mission_ids: vec!["m_b".into()], refused: None, retried: vec![], folder: None },
        ],
        changes: true,
        changes_in,
        blind: false,
        kept: kept.map(|slot| CompareKept { slot, at: "2026-11-27T00:01:00Z".into(), brought: Some(vec!["index.html".into()]) }),
        judge: None,
        base: None,
        rerun_of: None,
        reruns: vec![],
        settled: true,
        point_drafts: vec![],
    }
}

#[test]
fn comparison_copy_page_loads_and_serves_through_host_read_preview() {
    use locust_core::compare::{ChangesIn, CompareSlotId};
    let (_t, root) = temp("cmp-copy");
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let folder = root.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let copy_a = locust_core::compare_copies::compare_root(&store).join("cmp_page1-a");
    std::fs::create_dir_all(&copy_a).unwrap();
    std::fs::write(copy_a.join("index.html"), "<h1>Copy Column A</h1>").unwrap();

    let mut s = state(&folder);
    s.data = Some(store);
    s.compare.compares = vec![compare_record("cmp_page1", Some(ChangesIn::Copy), None)];
    s.compare.active_id = Some("cmp_page1".into());
    s.compare.loaded = true;

    crate::compare_view::preview_column_page(&mut s, CompareSlotId::A, "index.html");
    assert_eq!(s.compare.problem, None);
    // The next browser poll populates data_root & workspace before WKWebView reads the scheme URL.
    crate::browser_panel::poll(&mut s, &mut Tx::default());
    let urls = loaded(&s);
    assert_eq!(urls.len(), 1, "{urls:?}");
    let res = crate::invariants::lock(&s.browser.host).read_preview(&urls[0]);
    assert_eq!(res.status, 200, "body: {}", String::from_utf8_lossy(&res.body));
    assert_eq!(res.body, b"<h1>Copy Column A</h1>");
}

#[test]
fn comparison_worktree_from_home_folder_and_kept_column_serve_while_discarded_column_is_refused() {
    use locust_core::compare::CompareSlotId;
    let (_t, root) = temp("cmp-home");
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let folder = root.join("project");
    let wt_a = folder.join(".locust/compare/cmp_page2-a");
    std::fs::create_dir_all(&wt_a).unwrap();
    std::fs::write(wt_a.join("index.html"), "<h1>Home Worktree A</h1>").unwrap();

    // No teammate selected — comparison launched from Home with only s.home.folder_path set.
    let mut s = State::default();
    s.data = Some(store.clone());
    s.home.folder_path = Some(folder.to_string_lossy().into_owned());
    s.compare.compares = vec![compare_record("cmp_page2", None, None)];
    s.compare.active_id = Some("cmp_page2".into());
    s.compare.loaded = true;

    crate::compare_view::preview_column_page(&mut s, CompareSlotId::A, "index.html");
    assert_eq!(s.compare.problem, None);
    crate::browser_panel::poll(&mut s, &mut Tx::default());
    let urls = loaded(&s);
    assert_eq!(urls.len(), 1, "{urls:?}");
    let res = crate::invariants::lock(&s.browser.host).read_preview(&urls[0]);
    assert_eq!(res.status, 200, "body: {}", String::from_utf8_lossy(&res.body));
    assert_eq!(res.body, b"<h1>Home Worktree A</h1>");

    // After Keep removes the copies, the kept column (A) serves from the folder and non-kept (B) is refused.
    std::fs::remove_dir_all(&wt_a).unwrap();
    std::fs::write(folder.join("index.html"), "<h1>Kept In Folder</h1>").unwrap();
    s.compare.compares = vec![compare_record("cmp_page2", None, Some(CompareSlotId::A))];
    crate::invariants::lock(&s.browser.host).tabs[0].commands.clear();

    crate::compare_view::preview_column_page(&mut s, CompareSlotId::A, "index.html");
    assert_eq!(s.compare.problem, None);
    let kept_urls = loaded(&s);
    assert_eq!(kept_urls.len(), 1);
    let kept_res = crate::invariants::lock(&s.browser.host).read_preview(&kept_urls[0]);
    assert_eq!(kept_res.status, 200);
    assert_eq!(kept_res.body, b"<h1>Kept In Folder</h1>");

    crate::invariants::lock(&s.browser.host).tabs[0].commands.clear();
    crate::compare_view::preview_column_page(&mut s, CompareSlotId::B, "index.html");
    assert_eq!(
        s.compare.problem.as_deref(),
        Some("Its copy was removed when you kept another model, so this page is no longer on this computer.")
    );
    assert!(loaded(&s).is_empty());
}

#[test]
fn file_viewer_page_uses_workspace_json_fallback_refuses_outside_and_stays_confined_when_workspace_changes() {
    let (_t, root) = temp("viewer-confinement");
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let folder_a = root.join("folder-a");
    let folder_b = root.join("folder-b");
    let outside = root.join("outside");
    std::fs::create_dir_all(folder_a.join("site")).unwrap();
    std::fs::create_dir_all(&folder_b).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(folder_a.join("site/index.html"), "<h1>Workspace A</h1>").unwrap();
    std::fs::write(outside.join("secret.html"), "<h1>Secret</h1>").unwrap();
    // Teammate has no folder on its record; workspace.json supplies folder_a.
    std::fs::create_dir_all(store.electron()).unwrap();
    std::fs::write(
        store.electron().join("workspace.json"),
        serde_json::json!({"schemaVersion": 1, "path": folder_a.to_string_lossy()}).to_string(),
    )
    .unwrap();

    let mut s = State::default();
    s.data = Some(store);
    s.teammates.push(crate::Teammate {
        id: "tm".into(),
        name: "Codec".into(),
        role: "Custom".into(),
        model: None,
        runtime: Some("opencode".into()),
        folder: None,
        runs_in: "ask".into(),
        account: None,
        busy: false,
    });
    s.conversations.insert("tm".into(), Default::default());
    s.file_view = Some((
        reply_actions::FileTarget { path: "site/index.html".into(), start: 1, end: 1 },
        Ok("<h1>Workspace A</h1>".into()),
    ));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| reply_actions::viewer(s, &Palette::new(true)).unwrap_or_else(col),
        (800.0, 1200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    press(&mut rt, "file-reader/page");
    let urls = loaded(&rt.state);
    assert_eq!(urls.len(), 1, "status: {}", rt.state.status);
    let url = urls[0].clone();
    let res = crate::invariants::lock(&rt.state.browser.host).read_preview(&url);
    assert_eq!(res.status, 200);
    assert_eq!(res.body, b"<h1>Workspace A</h1>");

    // Switching to another workspace revokes read access to Workspace A's preview address.
    rt.state.teammates[0].folder = Some(folder_b.to_string_lossy().into_owned());
    crate::browser_panel::poll(&mut rt.state, &mut Tx::default());
    assert_eq!(crate::invariants::lock(&rt.state.browser.host).read_preview(&url).status, 403);

    // An outside path via ../ is refused and queues no load.
    let mut rt_outside = viewer(&folder_b, "../outside/secret.html", "<h1>Secret</h1>");
    press(&mut rt_outside, "file-reader/page");
    assert!(loaded(&rt_outside.state).is_empty(), "outside page must never load");
    assert!(rt_outside.state.status.contains("outside"), "status: {}", rt_outside.state.status);
}
