//! Electron's `a-change-can-be-copied.test.tsx` through the thread's file rows (`activity_item` over an edit's patch):
//! the opened change's Copy (`review_changes::file_diff`, which reads `changes::File::after`) and the changed file's
//! row's Copy, each pressed and read off the clipboard request. Diffs and expected words are Electron's, written out.
use super::*;

const EDIT: &str = "--- a/src/cart.js\n+++ b/src/cart.js\n@@ -1,3 +1,3 @@\n function total(items) {\n-  return sum(items) * 2\n+  return sum(items)\n }\n@@ -10,2 +10,3 @@\n export { total }\n+export { sum }\n // end";
const CREATED: &str = "--- /dev/null\n+++ b/notes.md\n@@ -0,0 +1,2 @@\n+# Notes\n+First line";
const DELETED: &str = "--- a/old.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n";

thread_local! {
    /// What this test's thread draws: an edit's item, or one file's change.
    static DRAWN_ITEM: std::cell::RefCell<Option<ledger::Item>> = const { std::cell::RefCell::new(None) };
    static DRAWN_FILE: std::cell::RefCell<Option<locust_core::changes::File>> = const { std::cell::RefCell::new(None) };
}

fn draw_item(s: &State, _: &mut rune::Cx<State>) -> El {
    DRAWN_ITEM.with(|item| item.borrow().as_ref().map(|item| activity_item(item, &Palette::new(true), s, "conv")).unwrap_or_else(col))
}

fn draw_file(s: &State, _: &mut rune::Cx<State>) -> El {
    DRAWN_FILE.with(|file| file.borrow().as_ref().map(|file| crate::review_changes::file_diff(s, None, false, file, false, &Palette::new(true))).unwrap_or_else(col))
}

/// An edit's file rows drawn, the first opened, in a conversation whose folder is `folder`.
fn edited(patch: &str, folder: Option<&str>) -> rune::Runtime<State> {
    let item = ledger::Item::Tool {
        name: "edit".into(),
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail {
            id: "c1".into(),
            patch: Some(locust_core::activity_entries::Patch { text: patch.into(), added: 0, removed: 0, truncated: false }),
            ..Default::default()
        }),
        status: ledger::Status::Done,
    };
    let mut s = State::default();
    s.teammates.push(Teammate { id: "tm".into(), name: "Wren".into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: folder.map(str::to_owned) });
    s.conversations.insert("tm".into(), Default::default());
    s.activity_open.insert("conv/c1/file/0".into(), true);
    DRAWN_ITEM.with(|drawn| *drawn.borrow_mut() = Some(item));
    let mut rt = rune::Runtime::new(s, draw_item, (900.0, 1200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn offered<T>(rt: &rune::Runtime<T>, label: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| !e.exiting && e.label.as_deref() == Some(label)).and_then(|e| e.key)
}

/// The control labelled `label` pressed: what it asked the clipboard to hold.
fn pressed_copy<T>(rt: &mut rune::Runtime<T>, label: &str) -> Option<String> {
    let key = offered(rt, label).unwrap_or_else(|| panic!("{label} is offered"));
    let (x, y) = rt.find(&key).expect("drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    rt.copy_request.take().as_deref().map(str::to_owned)
}

// case: gives the code as it reads after an edit: no signs, no removed lines, parts apart
#[test]
fn copying_an_edit_gives_the_code_after_it_with_parts_apart() {
    let mut rt = edited(EDIT, None);
    assert_eq!(
        pressed_copy(&mut rt, "Copy the new code in src/cart.js").as_deref(),
        Some(["function total(items) {", "  return sum(items)", "}", "", "export { total }", "export { sum }", "// end"].join("\n").as_str())
    );
}

// case: gives a new file whole, and nothing for a deleted one
#[test]
fn copying_a_new_file_gives_it_whole_and_a_deleted_one_offers_nothing() {
    // A new Markdown file's row opens as its document; its change, drawn, is where the new code's Copy is.
    let file = locust_core::changes::parse(CREATED).remove(0);
    DRAWN_FILE.with(|drawn| *drawn.borrow_mut() = Some(file));
    let mut rt = rune::Runtime::new(State::default(), draw_file, (900.0, 600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    assert_eq!(pressed_copy(&mut rt, "Copy the new code in notes.md").as_deref(), Some("# Notes\nFirst line"));
    let rt = edited(DELETED, None);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("old.txt")), "the deleted file's change is open");
    assert_eq!(offered(&rt, "Copy the new code in old.txt"), None);
    assert_eq!(offered(&rt, "Copy old.txt"), None, "no file to copy either");
}

// case: offers Copy at the foot of an opened change
#[test]
fn an_opened_change_offers_copy_at_its_foot() {
    let rt = edited(EDIT, None);
    let els = rt.elements();
    let copy = els.iter().find(|e| e.label.as_deref() == Some("Copy the new code in src/cart.js")).expect("Copy at the foot");
    let last_row = els.iter().filter(|e| e.text.as_deref().is_some_and(|t| t == "// end")).map(|e| e.rect[1]).fold(f32::MIN, f32::max);
    assert!(copy.rect[1] > last_row, "below the change's last line");
}

// case: offers Copy on a changed file's row, beside Open and Show
#[test]
fn a_changed_files_row_offers_copy_of_the_whole_file() {
    let dir = std::env::temp_dir().join(format!("locust-change-copied-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/cart.js"), "function total(items) {\n  return sum(items)\n}\n").unwrap();
    let folder = dir.canonicalize().unwrap().to_string_lossy().into_owned();
    let mut rt = edited(EDIT, Some(&folder));
    // The whole file as it is now, read when asked, not the change's lines.
    assert_eq!(pressed_copy(&mut rt, "Copy src/cart.js").as_deref(), Some("function total(items) {\n  return sum(items)\n}\n"));
    let _ = std::fs::remove_dir_all(&dir);
}
