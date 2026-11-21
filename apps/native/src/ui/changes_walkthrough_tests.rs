//! The window's half of Electron's walkthrough (ChangesPanel.tsx, 0.771): the walk said while it is asked or why not,
//! and once shown the summary over the files, the files in the order to read them, each with its place and note. The
//! reading itself is tested in core (`core/src/walkthrough_tests.rs`).
use super::*;
use locust_core::walkthrough::{Walkthrough, WalkthroughFile};

const TWO: &str = "diff --git a/src/total.js b/src/total.js
--- a/src/total.js
+++ b/src/total.js
@@ -1 +1 @@
-a
+b
diff --git a/test/total.test.js b/test/total.test.js
--- a/test/total.test.js
+++ b/test/total.test.js
@@ -1 +1 @@
-c
+d
";

fn panel_with(walk: Option<Walk>) -> rune::Runtime<State> {
    let mut s = State::default();
    s.changes = Panel {
        open: true,
        diff: Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "feature".into(),
            showing: "all".into(),
            files: changes::parse(TWO),
            ..changes::Folder::default()
        })),
        walk,
        ..Panel::default()
    };
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn texts<T>(rt: &rune::Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

fn at(texts: &[String], words: &str) -> usize {
    texts.iter().position(|t| t == words).unwrap_or_else(|| panic!("{words:?} not drawn in {texts:?}"))
}

#[test]
fn shows_the_summary_over_the_files_in_the_order_to_read_them_each_with_its_note() {
    let walk = Walkthrough {
        summary: "The total now adds each line.".into(),
        files: vec![
            WalkthroughFile { path: "test/total.test.js".into(), why: "A new test.".into(), check: None },
            WalkthroughFile { path: "src/total.js".into(), why: "It adds now.".into(), check: Some("An invoice with no lines.".into()) },
        ],
    };
    let rt = panel_with(Some(Walk::Shown(walk)));
    let t = texts(&rt);
    let summary = at(&t, "The total now adds each line.");
    assert!(at(&t, "Your teammate\u{2019}s walkthrough") < summary);
    let test_file = t.iter().rposition(|x| x == "test/total.test.js").unwrap();
    let src_file = t.iter().rposition(|x| x == "src/total.js").unwrap();
    assert!(summary < test_file && test_file < src_file, "{t:?}");
    assert!(test_file < at(&t, "A new test.") && at(&t, "A new test.") < src_file);
    assert!(src_file < at(&t, "It adds now.") && at(&t, "It adds now.") < at(&t, "An invoice with no lines."));
    assert!(t.contains(&"Check".to_owned()));
    assert!(rt.elements().iter().any(|e| e.label.as_deref() == Some("Read it 2 of 2")));
    assert!(rt.elements().iter().any(|e| e.label.as_deref() == Some("Close the walkthrough")));
}

#[test]
fn says_it_is_walking_through_and_says_why_not_and_without_one_shows_gits_order() {
    let asking = texts(&panel_with(Some(Walk::Asking)));
    assert!(asking.contains(&"Your teammate is walking through the change\u{2026}".to_owned()));
    let failed = texts(&panel_with(Some(Walk::Failed("It did not answer with a walkthrough. The changes are as they were.".into()))));
    assert!(failed.contains(&"It did not answer with a walkthrough. The changes are as they were.".to_owned()));
    let none = texts(&panel_with(None));
    assert!(!none.iter().any(|t| t.contains("walk")));
    assert!(none.iter().rposition(|x| x == "src/total.js").unwrap() < none.iter().rposition(|x| x == "test/total.test.js").unwrap());
}

#[test]
fn a_change_read_again_ends_the_walk() {
    let mut s = State::default();
    s.changes.walk = Some(Walk::Failed("x".into()));
    refresh(&mut s, None);
    assert!(s.changes.walk.is_none());
}

#[test]
fn close_the_walkthrough_puts_the_files_back_in_gits_order() {
    let walk = Walkthrough {
        summary: "Summary here.".into(),
        files: vec![WalkthroughFile { path: "test/total.test.js".into(), why: "First.".into(), check: None }],
    };
    let mut rt = panel_with(Some(Walk::Shown(walk)));
    for _ in 0..20 {
        rt.frame(0.05);
    }
    let (x, y) = rt.find("changes-walk-close").expect("the close button is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..40 {
        rt.frame(0.05);
    }
    assert!(rt.state.changes.walk.is_none(), "the press reached Close the walkthrough");
    let t = texts(&rt);
    assert!(!t.contains(&"Summary here.".to_owned()), "{t:?}");
    assert!(t.iter().rposition(|x| x == "src/total.js").unwrap() < t.iter().rposition(|x| x == "test/total.test.js").unwrap());
}
