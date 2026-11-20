//! The handed file's pill in the narrow reply beside the browser
//! (`main/the-thread-beside-the-browser-keeps-its-parts-whole.test.ts`, 0.867: Colin's 1205px window, the reply column
//! ~418px, "Blocker eviden" cut mid-word because the pill was wider than the column). Drawn headless at that width.
use super::*;

const COLUMN: f32 = 418.0;

fn files() -> Vec<HandedFile> {
    vec![
        HandedFile { path: "notes/handoff-0501-polish-blocker-evidence.md".into(), note: Some("Blocker evidence for the 0501 polish pass, with the screenshots it cites".into()) },
        HandedFile { path: "report.md".into(), note: None },
    ]
}

/// The pills drawn in a reply `width` wide: each keyed element's rect, by key.
fn drawn(width: f32) -> Vec<(String, [f32; 4])> {
    fn reply(s: &State, width: f32) -> El {
        row().child(col().w(width).child(view(s, &files(), &[], "r1", &Palette::new(true)).unwrap()))
    }
    let draw: fn(&State, &mut rune::Cx<State>) -> El = if width == COLUMN { |s, _| reply(s, COLUMN) } else { |s, _| reply(s, 1100.0) };
    let mut rt = rune::Runtime::new(State::default(), draw, (1200.0, 600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| Some((e.key.or(e.text.map(|t| format!("text:{t}")))?, e.rect))).filter(|(k, _)| k.starts_with("handed-") || k.starts_with("text:")).collect()
}

fn rect(all: &[(String, [f32; 4])], key: &str) -> [f32; 4] {
    all.iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no {key}: {all:?}")).1
}

// case: fits a handed file's pill to the reply, the note giving way before the path
#[test]
fn fits_a_handed_files_pill_to_the_reply_the_note_giving_way_before_the_path() {
    let narrow = drawn(COLUMN);
    let wide = drawn(1100.0);
    // Nothing of a file runs past the reply.
    for (key, [x, _, w, _]) in &narrow {
        assert!(x + w <= COLUMN + 0.5, "{key} ends at {} in a {COLUMN}px reply: {narrow:?}", x + w);
    }
    // The control: in a wide reply the long pill is wider than this column, so it did have to give way.
    let [_, _, whole, _] = rect(&wide, "handed-file-r1-0");
    assert!(whole > COLUMN, "the long pill is {whole}px when nothing presses on it");
    // Copy and Save a copy stay whole: as wide and as tall as when there is room.
    for key in ["handed-file-copy-r1-0", "handed-file-save-r1-0"] {
        let ([_, _, nw, nh], [_, _, ww, wh]) = (rect(&narrow, key), rect(&wide, key));
        assert!((nw - ww).abs() < 0.5 && (nh - wh).abs() < 0.5, "{key}: {nw}x{nh} narrow, {ww}x{wh} wide");
    }
    // The note is cut before the path: the path keeps PATH_KEPT, the note gives up more of itself.
    let path = "text:notes/handoff-0501-polish-blocker-evidence.md";
    let note = "text:Blocker evidence for the 0501 polish pass, with the screenshots it cites";
    let ([_, _, path_narrow, _], [_, _, path_wide, _]) = (rect(&narrow, path), rect(&wide, path));
    let ([_, _, note_narrow, _], [_, _, note_wide, _]) = (rect(&narrow, note), rect(&wide, note));
    assert!(path_narrow >= PATH_KEPT - 0.5, "the path keeps {path_narrow}px");
    assert!(note_wide - note_narrow > path_wide - path_narrow, "note {note_wide}->{note_narrow}, path {path_wide}->{path_narrow}");
    // The second file wraps to a line of its own rather than squeezing the first.
    let ([_, first_y, _, first_h], [_, second_y, _, _]) = (rect(&narrow, "handed-group-r1-0"), rect(&narrow, "handed-group-r1-1"));
    assert!(second_y >= first_y + first_h - 0.5, "the second file sits under the first");
}
