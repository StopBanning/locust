//! a-handed-file-is-a-control.test.tsx (10 cases): a file a teammate handed over is a pill that reveals it in the file
//! manager (never opens it), with Save a copy beside it, its note in the hover where the pill cuts it, and a whole path
//! drawn by its file's name. Drawn headless through `view`.
use super::*;

fn report() -> HandedFile {
    HandedFile { path: "docs/report.md".into(), note: Some("the rollup you asked for".into()) }
}

/// Every element's text and label, with the pill's hover open when `hovered`.
fn drawn(files: Vec<HandedFile>, hovered: bool) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    if hovered {
        s.hover.set("handed-tip-r1-0", true);
    }
    let mut rt = rune::Runtime::new((s, files), |(s, files), _| col().w(900.0).child(view(s, files, &[], "r1", &Palette::new(true)).unwrap()), (1000.0, 400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn words(els: &[rune::ElementInfo]) -> Vec<String> {
    els.iter().flat_map(|e| [e.text.clone(), e.label.clone()]).flatten().collect()
}

fn pill(els: &[rune::ElementInfo]) -> &rune::ElementInfo {
    els.iter().find(|e| e.key.as_deref() == Some("handed-file-r1-0")).expect("the pill")
}

// case: names the file and says what it is
#[test]
fn names_the_file_and_says_what_it_is() {
    let w = words(&drawn(vec![report()], false));
    assert!(w.iter().any(|t| t == "docs/report.md") && w.iter().any(|t| t == "the rollup you asked for"), "{w:?}");
}

// case: is a control, not a sentence
#[test]
fn is_a_control_not_a_sentence() {
    let els = drawn(vec![report()], false);
    assert_eq!(pill(&els).role, "button");
}

// case: offers to show the file, and never to open it
// case: still never offers to open it
#[test]
fn offers_to_show_the_file_and_never_to_open_it() {
    let els = drawn(vec![report()], false);
    assert_eq!(pill(&els).label.as_deref(), Some("Show docs/report.md in the file manager"));
    assert!(!words(&els).iter().any(|t| t == "Open" || t.starts_with("Open ")), "{:?}", words(&els));
}

// case: draws a file with no note without an empty caption beside it
#[test]
fn a_file_with_no_note_has_no_empty_caption() {
    let els = drawn(vec![HandedFile { path: "notes.md".into(), note: None }], false);
    let texts: Vec<&str> = els.iter().filter_map(|e| e.text.as_deref()).collect();
    assert!(texts.contains(&"notes.md"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.trim().is_empty()), "no empty caption: {texts:?}");
}

// case: offers it, beside the reveal rather than instead of it
#[test]
fn save_a_copy_is_offered_beside_the_reveal() {
    let els = drawn(vec![report()], false);
    let save = els.iter().find(|e| e.key.as_deref() == Some("handed-file-save-r1-0")).expect("Save a copy");
    assert_eq!(save.label.as_deref(), Some("Save a copy of docs/report.md"));
    assert_eq!(save.role, "button");
    assert!(pill(&els).role == "button", "the reveal is still there");
}

// case: keeps the whole note reachable when the pill truncates it
#[test]
fn keeps_the_whole_note_reachable_when_the_pill_truncates_it() {
    let w = words(&drawn(vec![report()], true));
    assert!(w.iter().any(|t| t == "the rollup you asked for\n\nShow docs/report.md in the file manager"), "{w:?}");
}

// case: is drawn as a control, by its file's name, with all of its path on hover
#[test]
fn a_whole_path_is_drawn_by_its_files_name() {
    let path = "D:/work/scratch/toaster-review2/Codex.md";
    let els = drawn(vec![HandedFile { path: path.into(), note: Some("the review".into()) }], false);
    let texts: Vec<&str> = els.iter().filter_map(|e| e.text.as_deref()).collect();
    assert!(texts.contains(&"Codex.md") && !texts.contains(&path), "{texts:?}");
    assert_eq!(pill(&els).role, "button");
    assert_eq!(pill(&els).label.as_deref(), Some(format!("Show {path} in the file manager").as_str()));
    let save = els.iter().find(|e| e.key.as_deref() == Some("handed-file-save-r1-0")).unwrap();
    assert_eq!(save.label.as_deref(), Some(format!("Save a copy of {path}").as_str()));
}

// case: leaves a path in the folder exactly as it was drawn before
#[test]
fn a_path_in_the_folder_is_drawn_as_it_was() {
    let texts: Vec<String> = drawn(vec![report()], false).into_iter().filter_map(|e| e.text).collect();
    assert!(texts.iter().any(|t| t == "docs/report.md"), "{texts:?}");
}
