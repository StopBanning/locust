//! Row 8: the window's Board (`board_view`) is built from the pieces Electron's Board is made of
//! (`conversation_board::board_header`, `board_columns`, `board_column`, `conv_card`; notes/PAGES-NOTES.md "Row 41",
//! `BoardScreen.tsx`, shell.css `.lc-teamboard`, `.lc-convcard`). Before, the window drew a "Close" button, the meta
//! mid-header, one full-width column of three-line rows, and Scout's own runs in Done. Drawn headless, as the window draws it.
use super::*;

fn wren() -> Teammate {
    Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: Some("claude".into()), model: None,
        account: None, busy: false, runs_in: "ask".into(), folder: None }
}

fn ago(minutes: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::minutes(minutes)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn conversation(id: &str, title: &str, state: &str, minutes_ago: i64) -> ConversationRow {
    ConversationRow { id: id.into(), teammate: "Wren".into(), title: title.into(), started_by: "the person".into(), state: state.into(), updated: ago(minutes_ago) }
}

fn ask(id: &str, command: Option<&str>) -> BoardAsk {
    BoardAsk { approval_id: format!("ap_{id}"), kind: if command.is_some() { "command".into() } else { "question".into() }, command: command.map(str::to_owned), where_: None, shown: std::time::Instant::now() - std::time::Duration::from_secs(5) }
}

fn state(rows: Vec<ConversationRow>, prep: impl FnOnce(&mut State)) -> State {
    let mut s = State::default();
    s.teammates = vec![wren()];
    s.recent = rows;
    s.home.board_open = true;
    prep(&mut s);
    s
}

fn runtime(s: State, width: f32) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| board_view(s, &Palette::new(true)), (width, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn elements(rt: &rune::Runtime<State>) -> Vec<rune::ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn rect(all: &[rune::ElementInfo], key: &str) -> [f32; 4] {
    all.iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect).unwrap_or_else(|| panic!("no {key}: {:?}", all.iter().filter_map(|e| e.key.clone()).filter(|k| k.starts_with("board")).collect::<Vec<_>>()))
}

fn words(all: &[rune::ElementInfo], key: &str) -> Option<String> {
    all.iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone())
}

fn busy_board() -> State {
    state(
        vec![
            conversation("asks", "Rename the invoices", "running", 2),
            conversation("live", "Fix the cart total", "running", 10),
            conversation("ended", "Write the brief", "completed", 30),
            conversation("old", "Yesterday", "completed", 25 * 60),
        ],
        |s| {
            s.home.asks.insert("asks".into(), ask("asks", Some("git mv invoices old-invoices")));
            s.home.to_look_at.insert("ended".into());
        },
    )
}

#[test]
fn the_header_is_board_with_its_meta_right_beside_it_and_no_close() {
    let all = elements(&runtime(busy_board(), 1400.0));
    assert_eq!(words(&all, "board-title").as_deref(), Some("Board"));
    assert_eq!(words(&all, "board-meta").as_deref(), Some("1 waiting on you · 1 working"));
    let (title, meta) = (rect(&all, "board-title"), rect(&all, "board-meta"));
    let gap = meta[0] - (title[0] + title[2]);
    assert!((0.0..=24.0).contains(&gap), "the meta sits beside the title: {gap}");
    assert!(!all.iter().any(|e| e.text.as_deref() == Some("Close")), "Electron's Board has no Close");
}

#[test]
fn the_header_stays_while_the_columns_scroll() {
    let all = elements(&runtime(busy_board(), 1400.0));
    let (header, columns) = (rect(&all, "board-header"), rect(&all, "board-columns"));
    assert!(columns[1] >= header[1] + header[3] - 1.0, "the columns start under the header: {header:?} {columns:?}");
}

#[test]
fn the_columns_stand_side_by_side_tops_aligned() {
    let all = elements(&runtime(busy_board(), 1400.0));
    let cols: Vec<[f32; 4]> = ["needs-you", "working", "to-look-at", "done"].iter().map(|c| rect(&all, &format!("board-column/{c}"))).collect();
    for pair in cols.windows(2) {
        assert!(pair[1][0] > pair[0][0] + pair[0][2] - 1.0, "left to right: {cols:?}");
        assert!((pair[1][1] - pair[0][1]).abs() < 1.0, "tops aligned: {cols:?}");
    }
    assert!((cols[0][2] - 256.0).abs() < 1.0, "a column is 256 wide: {cols:?}");
}

#[test]
fn a_card_is_two_lines_beside_a_face_of_twenty_who_and_state_on_the_second() {
    let all = elements(&runtime(busy_board(), 1400.0));
    let card = rect(&all, "board-card-ended");
    assert_eq!(words(&all, "board-card-title-ended").as_deref(), Some("Write the brief"));
    let meta = words(&all, "board-card-meta-ended").unwrap();
    assert!(meta.starts_with("Wren \u{b7} finished \u{b7} "), "{meta}");
    assert!(card[3] <= 72.0 && card[2] <= 260.0, "a two-line card in a column, not a full-width row: {card:?}");
    let face = all.iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("bot") && k.contains("tm_wren")) && e.rect[1] >= card[1] && e.rect[1] <= card[1] + card[3]).expect("Wren's face on her card");
    assert!((face.rect[2] - 20.0).abs() < 1.0, "a face of 20: {:?}", face.rect);
}

#[test]
fn runs_electron_never_lists_are_not_on_the_board() {
    // Scout's and a judge's runs the sidebar already leaves out; a question on the side and a panel's own run (a commit
    // message, a walk through) it does not, and the Board must (App.tsx:2016 `isHiddenRun`, :8250).
    let kinds = [("scouts", "scout"), ("judged", "judge"), ("aside", "side"), ("commit", "commit-draft"), ("walk", "walkthrough")];
    let mut rows = vec![conversation("mine", "Fix the cart total", "completed", 90)];
    rows.extend(kinds.iter().map(|(id, _)| conversation(id, &format!("Run {id}"), "completed", 80)));
    let s = state(rows, |s| {
        for (id, kind) in kinds {
            s.ledger_summaries.insert(id.into(), locust_core::ledger::Summary { started_by_kind: Some(kind.into()), ..Default::default() });
        }
    });
    let all = elements(&runtime(s, 1400.0));
    assert!(all.iter().any(|e| e.key.as_deref() == Some("board-card-mine")));
    for (id, kind) in kinds {
        assert!(!all.iter().any(|e| e.key.as_deref() == Some(format!("board-card-{id}").as_str())), "a {kind} run is left out");
    }
    assert_eq!(words(&all, "board-done-count").as_deref(), Some("1"), "and not counted");
}

#[test]
fn a_request_keeps_its_words_and_buttons_under_its_card() {
    let all = elements(&runtime(busy_board(), 1400.0));
    let (card, allow) = (rect(&all, "board-card-asks"), rect(&all, "board-allow-asks"));
    assert!(allow[1] >= card[1] + card[3] - 1.0, "the buttons are under the card: {card:?} {allow:?}");
    assert_eq!(words(&all, "board-request-ap_asks").as_deref(), Some("wants to run"));
    // That Allow once answers the request is `request_answered_on_board_tests`, on this same view.
}

#[test]
fn pressing_a_card_opens_its_conversation_and_clears_its_dot() {
    let mut rt = runtime(busy_board(), 1400.0);
    let (x, y) = rt.find("board-card-ended").expect("the card is a press");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.opening_conversation.as_deref(), Some("ended"));
    assert!(!rt.state.home.to_look_at.contains("ended"));
    assert!(!rt.state.home.board_open);
}

#[test]
fn a_narrow_window_wraps_the_columns_onto_a_new_line() {
    let wide = elements(&runtime(busy_board(), 1400.0));
    let narrow = elements(&runtime(busy_board(), 560.0));
    assert!((rect(&wide, "board-column/working")[1] - rect(&wide, "board-column/needs-you")[1]).abs() < 1.0);
    assert!(rect(&narrow, "board-column/working")[1] > rect(&narrow, "board-column/needs-you")[1] + 10.0, "wrapped under, not squeezed");
}
