//! the-file-viewer-renders-and-never-runs.test.tsx (13 cases), through the window.
//!
//! The viewer beside the conversation (`viewer_on`, opened by `open_file`) renders Markdown as prose and anything else
//! verbatim in its own scroll box, interprets nothing, and offers no way to hand the file to the system: Show it in
//! the file manager and Save a copy, and the register saying why there is no open button. The thread's file rows
//! (`activity_drawers::activity_item`) offer to open a file a turn touched where a viewer can (a conversation with a
//! folder), and only reveal where nothing can. The versions of a file across turns are the Changes panel's strip
//! (`changes_panel::version_strip`): see `file_versions_strip_tests.rs` for those five cases.
//! Expected words are Electron's (FileViewer.tsx, ActivityCard.tsx and the test).
use super::*;

/// The viewer drawn on macOS with no editor installed (the setting Electron's markup test renders), on `path` holding
/// `text`, in a conversation whose folder is `/work`.
fn viewer(path: &str, text: &str) -> rune::Runtime<State> {
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    s.teammates.push(Teammate { id: "tm".into(), name: "Wren".into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: Some("/work".into()) });
    s.conversations.insert("tm".into(), Default::default());
    s.file_view = Some((FileTarget { path: path.into(), start: 0, end: 0 }, Ok(text.into())));
    let mut rt = rune::Runtime::new(s, |s, _| viewer_on(s, &Palette::new(false), "darwin", &[]).unwrap_or_else(col), (900.0, 1200.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    rt
}

fn keys<T>(rt: &rune::Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.key).collect()
}

/// Every text, label and hover the viewer draws, as one string.
fn said<T>(rt: &rune::Runtime<T>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).flat_map(|e| [e.text, e.label, e.help]).flatten().collect::<Vec<_>>().join("\n")
}

/// The texts drawn under the element keyed `key`.
fn texts_under<T>(rt: &rune::Runtime<T>, key: &str) -> Vec<String> {
    let els = rt.elements();
    let Some(root) = els.iter().find(|e| e.key.as_deref() == Some(key)) else { return Vec::new() };
    let prefix = format!("{}/", root.path);
    els.iter().filter(|e| e.path.starts_with(&prefix) || e.path == root.path).filter_map(|e| e.text.clone()).collect()
}

// the file viewer

#[test]
fn renders_markdown_as_prose_not_as_its_source() {
    let rt = viewer("/work/docs/report.md", "# The rollup\n\nTwo things went wrong.\n");
    assert!(keys(&rt).iter().any(|k| k == "file-reader/prose"), "{:?}", keys(&rt));
    let prose = texts_under(&rt, "file-reader/prose");
    assert!(prose.iter().any(|t| t == "The rollup"), "{prose:?}");
    assert!(rt.elements().iter().any(|e| e.role == "heading" && e.text.as_deref() == Some("The rollup")));
    // The hash is markup: if it survived, the file would be shown, not rendered.
    assert!(!said(&rt).contains("# The rollup"), "{}", said(&rt));
}

#[test]
fn renders_anything_else_in_its_own_scroll_box_verbatim() {
    let rt = viewer("/work/sum.ts", "const total = a + b\n");
    let k = keys(&rt);
    assert!(k.iter().any(|k| k == "file-reader/code"), "{k:?}");
    assert!(!k.iter().any(|k| k == "file-reader/prose"), "{k:?}");
    assert!(texts_under(&rt, "file-reader/code").iter().any(|t| t.contains("const total = a + b")));
}

#[test]
fn does_not_interpret_the_file_whichever_mode_it_is_in() {
    let hostile = r#"<img src=x onerror="alert(1)">"#;
    for path in ["/work/docs/report.md", "/work/sum.ts"] {
        let rt = viewer(path, hostile);
        // The words survive, as the file's own text; no element is made of them, so nothing can fire.
        assert!(said(&rt).contains(hostile), "{path}: {}", said(&rt));
        assert!(!rt.elements().iter().any(|e| matches!(e.role, "image" | "img")), "{path}");
    }
}

#[test]
fn offers_no_way_to_hand_the_file_to_the_operating_system() {
    let rt = viewer("/work/docs/report.md", "notes\n");
    let all = said(&rt);
    // The panel's own controls. Its file menu (file-menu.ts; Electron's right-click on the name, drawn here as the
    // `file-reader/menu/` row) is held by file_menu_drawn_tests and offers only what shows a file, never what runs one.
    let menu: Vec<String> = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("file-reader/menu/"))).map(|e| e.path).collect();
    let panel: String = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting && !menu.iter().any(|m| e.path == *m || e.path.starts_with(&format!("{m}/"))))
        .flat_map(|e| [e.text, e.label, e.help])
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all.contains("Show it in the file manager"), "{all}");
    assert!(all.contains("Save a copy"), "{all}");
    let open_out = regex::Regex::new(r"(?i)open\s+(externally|with|in\b)").unwrap();
    for line in panel.lines() {
        assert!(!open_out.is_match(line) || line.to_lowercase().contains("open in the panel"), "{line:?}");
    }
    // No link out of the panel either.
    assert!(!rt.elements().iter().any(|e| e.role == "link"), "{all}");
}

#[test]
fn says_why_there_is_no_open_button_rather_than_leaving_a_hole() {
    let rt = viewer("/work/docs/report.md", "notes");
    assert_eq!(
        texts_under(&rt, "file-reader/register"),
        ["Shown here only: Locust never runs a file a teammate made, or opens it in another app."]
    );
    assert!(!said(&rt).contains("Locust does not open files"));
}

#[test]
fn names_the_file_by_its_own_name_and_keeps_the_whole_path_reachable() {
    let rt = viewer("C:/work/docs/report.md", "notes\n");
    let name = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("file-reader/name")).expect("the name");
    assert_eq!(name.text.as_deref(), Some("report.md"));
    assert_eq!(name.help.as_deref(), Some("C:/work/docs/report.md"));
}

// the activity fold, which is how a file is usually met

const PATCH: &str = "--- a/docs/report.md\n+++ b/docs/report.md\n@@ -1 +1,2 @@\n first line\n+second line";

thread_local! {
    static FOLD_ITEM: std::cell::RefCell<Option<ledger::Item>> = const { std::cell::RefCell::new(None) };
}

/// An edit of docs/report.md drawn as the thread draws it, in a conversation with `folder` (a viewer beside it) or none.
fn fold(folder: Option<&str>) -> rune::Runtime<State> {
    let item = ledger::Item::Tool {
        name: "edit".into(),
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail {
            id: "c1".into(),
            patch: Some(locust_core::activity_entries::Patch { text: PATCH.into(), added: 1, removed: 0, truncated: false }),
            ..Default::default()
        }),
        status: ledger::Status::Done,
    };
    FOLD_ITEM.with(|drawn| *drawn.borrow_mut() = Some(item));
    let mut s = State::default();
    s.teammates.push(Teammate { id: "tm".into(), name: "Wren".into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: folder.map(str::to_owned) });
    s.conversations.insert("tm".into(), Default::default());
    let mut rt = rune::Runtime::new(
        s,
        |s, _| FOLD_ITEM.with(|item| item.borrow().as_ref().map(|item| crate::activity_drawers::activity_item(item, &Palette::new(true), s, "conv")).unwrap_or_else(col)),
        (900.0, 600.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

#[test]
fn offers_to_open_a_file_it_touched_not_only_to_go_find_it() {
    let mut rt = fold(Some("C:/work"));
    let view = rt.elements().into_iter().find(|e| e.key.as_deref().is_some_and(|k| k.ends_with("/view"))).expect("an open control");
    assert_eq!(view.label.as_deref(), Some("Open docs/report.md"));
    assert!(keys(&rt).iter().any(|k| k.ends_with("/reveal")), "{:?}", keys(&rt));
    // Pressed, it opens the viewer beside the conversation on that file.
    let key = view.key.expect("keyed");
    let (x, y) = rt.find(&key).expect("drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.file_view.as_ref().map(|(t, _)| t.path.as_str()), Some("docs/report.md"));
}

#[test]
fn still_only_reveals_where_nothing_can_open_it() {
    let rt = fold(None);
    let k = keys(&rt);
    assert!(!k.iter().any(|k| k.ends_with("/view")), "{k:?}");
    let reveal = rt.elements().into_iter().find(|e| e.key.as_deref().is_some_and(|k| k.ends_with("/reveal"))).expect("a reveal control");
    assert_eq!(reveal.label.as_deref(), Some("Show docs/report.md in the file manager"));
}

#[test]
fn says_a_refusal_it_gave_for_its_own_file_and_not_another_scopes() {
    let said_with = |scope: &str| {
        let mut rt = viewer("/work/docs/report.md", "Two things went wrong.\n");
        rt.update(|s, _| s.handed_file_refusal = Some((scope.to_owned(), "Locust will not show a file outside the folder.".into())));
        rt.frame(0.016);
        said(&rt)
    };
    assert!(said_with("file-reader").contains("Locust will not show a file outside the folder."));
    assert!(!said_with("conv/t1/file/0").contains("Locust will not show a file outside the folder."), "a refusal on a thread's file row is not the viewer's");
}
