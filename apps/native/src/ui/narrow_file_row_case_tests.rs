use super::*;
use locust_core::activity_entries::FileEntry;

// case: lets a file row give way to its open and copy buttons, never push them past the card
#[test]
fn long_file_label_yields_to_open_and_copy_at_narrow_width() {
    let mut state = State::default();
    state.teammates.push(crate::Teammate {
        id: "tm".into(),
        name: "Codec".into(),
        role: "Developer".into(),
        model: None,
        runtime: Some("opencode".into()),
        folder: Some("/fixture/project".into()),
        runs_in: "ask".into(),
        account: None,
        busy: false,
    });
    state.conversations.insert("tm".into(), Default::default());

    let path = format!("src/{}.rs", "very-long-filename-".repeat(12));
    let patch = format!("--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+hello\n");
    let file = FileEntry {
        key: "file".into(),
        file: locust_core::changes::parse(&patch).remove(0),
        counts: (1, 0),
        reported: None,
        large: false,
        truncated: false,
    };
    let mut rt = rune::Runtime::new(
        (state, file),
        |(s, file), _| {
            col().w_full().key("card").child(file_rows(
                std::slice::from_ref(file), None, &Palette::new(true), s, "turn",
            ))
        },
        (220.0, 240.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let elements = rt.elements();
    let rect = |key| {
        elements.iter().find(|e| e.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("missing {key}")).rect
    };
    let card = rect("card");
    let filename = rect("fold/turn/file");
    let open = rect("turn/file/view");
    let copy = rect("turn/file/copy");
    assert!(filename[2] > 0.0 && filename[0] + filename[2] <= open[0] + 1.0, "{filename:?} before {open:?}");
    assert!(open[2] > 0.0 && open[0] + open[2] <= copy[0] + 1.0, "{open:?} before {copy:?}");
    assert!(copy[2] > 0.0 && copy[0] + copy[2] <= card[0] + card[2] + 1.0, "{copy:?} inside {card:?}");
}
