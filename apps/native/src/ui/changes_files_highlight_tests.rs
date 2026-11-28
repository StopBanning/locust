use super::*;
use rune::{Env, Runtime};

#[test]
fn changes_files_button_shows_selected_fill_only_when_file_tree_is_open() {
    let mut s = State::default();
    s.changes = Panel {
        open: true,
        files: true,
        diff: Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "feature".into(),
            showing: "all".into(),
            files: changes::parse("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n"),
            ..changes::Folder::default()
        })),
        ..Panel::default()
    };

    let mut rt = Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200., 800.), Env::new(true, 1.));
    for _ in 0..4 {
        rt.frame(0.05);
    }

    let l = crate::look::controls::Look::new(true);
    let button_fill = |rt: &Runtime<State>| {
        rt.elements()
            .into_iter()
            .find(|el| !el.exiting && el.key.as_deref() == Some("changes-files"))
            .expect("changes-files button")
            .fill
    };
    assert_eq!(
        button_fill(&rt),
        l.c("bg-selected"),
        "the Show files toggle shows its selected background while the file tree is open"
    );

    let (x, y) = rt.find("changes-files").expect("changes-files button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..4 {
        rt.frame(0.05);
    }
    assert!(!rt.state.changes.files);
    assert_ne!(
        button_fill(&rt),
        l.c("bg-selected"),
        "hiding the file tree clears the selected background"
    );
}
