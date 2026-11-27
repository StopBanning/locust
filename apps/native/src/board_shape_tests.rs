//! The Board's pieces held to Electron's (`BoardScreen.tsx` 0.882 and shell.css), drawn headless. The owner's live
//! hour (26 Nov, row 41) found one full-width column of tall rows (title, teammate and state on three lines), "Done 74"
//! counting Scout's own runs, and "all quiet" floating mid-header. The window's Board view (home_panel.rs `board_view`,
//! the Home lane's file) is to be built from these pieces; notes/PAGES-NOTES.md says how.
use super::*;
use crate::look::controls::Look;
use rune::{ElementInfo, Env, Runtime};

fn row_of(id: &str, phase: &str, at: &str) -> SidebarMission {
    SidebarMission {
        mission_id: id.into(),
        title: format!("Title of {id}"),
        phase: phase.into(),
        last_at: Some(at.into()),
        root_id: None,
        member_ids: Vec::new(),
        owner_id: None,
        said: String::new(),
        nested_under: None,
        integrity_issue_count: 0,
        needs_you: false,
        app_started: false,
    }
}

fn draw(view: fn(&(), &mut rune::Cx<()>) -> El, width: f32) -> Vec<ElementInfo> {
    let mut rt = Runtime::new((), view, (width, 1400.0), Env::new(true, 1.0));
    for _ in 0..4 {
        rt.frame(0.016);
    }
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn rect(all: &[ElementInfo], key: &str) -> [f32; 4] {
    all.iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect).unwrap_or_else(|| panic!("no {key}"))
}

fn words(all: &[ElementInfo], key: &str) -> Option<String> {
    all.iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone())
}

fn three_columns(_: &(), _: &mut rune::Cx<()>) -> El {
    let l = &Look::new(true);
    let card = |column, id: &str, meta: &str| conv_card(l, column, id, None, &format!("Title of {id}"), meta);
    col().w_full().child(board_header(l, "1 waiting on you · 1 working")).child(board_columns(vec![
        board_column(l, ConversationBoardKey::NeedsYou, 1, vec![card(ConversationBoardKey::NeedsYou, "ask", "Codec · waiting on you")]),
        board_column(l, ConversationBoardKey::Working, 1, vec![card(ConversationBoardKey::Working, "busy", "Codec · working")]),
        board_column(l, ConversationBoardKey::ToLookAt, 1, vec![card(ConversationBoardKey::ToLookAt, "done1", "Codec · finished · 2 weeks ago")]),
        board_column(l, ConversationBoardKey::Done, 2, vec![card(ConversationBoardKey::Done, "old1", "Codec · done · 3 weeks ago"), card(ConversationBoardKey::Done, "old2", "Argon · done · 3 weeks ago")]),
    ]))
}

#[test]
fn the_header_says_board_and_its_meta_right_beside_it() {
    // BoardScreen.tsx:305-308: `<span className="lc-screen__title">Board</span>` then `<span className="lc-screen__meta">`.
    let all = draw(three_columns, 1400.0);
    assert_eq!(words(&all, "board-title").as_deref(), Some("Board"));
    assert_eq!(words(&all, "board-meta").as_deref(), Some("1 waiting on you · 1 working"));
    let (header, title, meta) = (rect(&all, "board-header"), rect(&all, "board-title"), rect(&all, "board-meta"));
    assert_eq!(header[3], 60.0);
    let gap = meta[0] - (title[0] + title[2]);
    assert!((0.0..=24.0).contains(&gap), "the meta sits beside the title, not mid-header: {gap}");
    assert!(meta[0] < header[0] + header[2] / 3.0, "{meta:?} in {header:?}");
}

#[test]
fn the_columns_stand_side_by_side_tops_aligned() {
    // `.lc-teamboard` (shell.css:11863): flex, wrapping, aligned to the top; `.lc-boardsection` `flex: 0 1 256px`.
    let all = draw(three_columns, 1400.0);
    let cols: Vec<[f32; 4]> = ["needs-you", "working", "to-look-at", "done"].iter().map(|c| rect(&all, &format!("board-column/{c}"))).collect();
    for pair in cols.windows(2) {
        assert!(pair[1][0] > pair[0][0] + pair[0][2] - 1.0, "left to right: {cols:?}");
        assert!((pair[1][1] - pair[0][1]).abs() < 1.0, "tops aligned: {cols:?}");
    }
    assert!((cols[0][2] - 256.0).abs() < 1.0, "a column is 256 wide: {cols:?}");
    // Done takes the rest of the line (`.lc-convboard .lc-boardsection--done { flex: 1 1 256px }`).
    assert!(cols[3][2] > 256.0, "{cols:?}");
}

#[test]
fn a_column_is_titled_with_its_name_and_count() {
    // BoardScreen.tsx:286-290: the mark, the title, the count.
    let all = draw(three_columns, 1400.0);
    assert_eq!(words(&all, "board-done-count").as_deref(), Some("2"));
    assert_eq!(words(&all, "board-needs-you-count").as_deref(), Some("1"));
    let title = rect(&all, "board-column-title/to-look-at");
    assert!(all.iter().any(|e| e.text.as_deref() == Some("READY TO LOOK AT") && e.rect[1] >= title[1] - 1.0 && e.rect[1] <= title[1] + title[3]));
}

#[test]
fn a_card_is_two_lines_its_title_then_who_and_state_on_one_line() {
    // `.lc-convcard` (shell.css:11983): the face spans two rows; the title, then `.lc-convcard__meta` — who and state
    // in one line, joined by a dot (BoardScreen.tsx:196-202).
    let all = draw(three_columns, 1400.0);
    let (card, title, meta) = (rect(&all, "board-card-done1"), rect(&all, "board-card-title-done1"), rect(&all, "board-card-meta-done1"));
    assert_eq!(words(&all, "board-card-meta-done1").as_deref(), Some("Codec · finished · 2 weeks ago"));
    assert!(meta[1] >= title[1] + title[3] - 1.0, "the meta is under the title");
    assert!(card[3] <= 72.0, "two lines, not three: {card:?}");
    assert!(card[2] <= 260.0, "a card is as wide as its column, not the window: {card:?}");
}

#[test]
fn the_meta_line_is_who_under_what_and_the_state() {
    // BoardScreen.tsx:198-201: who, then `under …`, then the state, each its own span joined by " · ".
    assert_eq!(card_meta_line(Some("Codec"), None, "done · 1 day ago"), "Codec · done · 1 day ago");
    assert_eq!(card_meta_line(Some("Codec"), Some("under Plan the release"), "working"), "Codec · under Plan the release · working");
    assert_eq!(card_meta_line(None, None, "waiting on you"), "waiting on you");
}

#[test]
fn scouts_own_runs_judges_and_side_questions_are_not_on_the_board() {
    // App.tsx:2016-2018 `isHiddenRun`: judge, scout, a panel's run; App.tsx:8250: a question on the side.
    for kind in ["scout", "judge", "side", "commit-draft", "walkthrough"] {
        assert!(is_hidden_run(Some(kind)), "{kind}");
    }
    for kind in [None, Some("carry-on"), Some("relay"), Some("routine"), Some("resume")] {
        assert!(!is_hidden_run(kind), "{kind:?}");
    }
    let rows = vec![
        row_of("yours", "completed", "2026-11-20T10:00:00.000Z"),
        row_of("scout_look_1", "completed", "2026-11-20T11:00:00.000Z"),
        row_of("scout_look_2", "completed", "2026-11-20T12:00:00.000Z"),
    ];
    let kind_of = |id: &str| id.starts_with("scout_").then(|| "scout".to_owned());
    let shown = without_hidden_runs(&rows, kind_of);
    assert_eq!(shown.iter().map(|r| r.mission_id.as_str()).collect::<Vec<_>>(), ["yours"]);
    // So Done counts the person's conversation only: one, not three.
    let facts = ConversationBoardFacts { needs_you: HashSet::new(), to_look_at: HashSet::new() };
    let done = shown.iter().filter(|m| conversation_column(m, &facts) == ConversationBoardKey::Done).count();
    assert_eq!(done, 1);
}

#[test]
fn a_conversation_with_one_of_its_turns_a_persons_stays() {
    // Only the hidden runs drop out; a conversation with a turn of the person's still has that turn.
    let mut mixed = row_of("root", "completed", "2026-11-20T10:00:00.000Z");
    mixed.member_ids = vec!["root".into(), "followup".into()];
    let shown = without_hidden_runs(&[mixed], |id| (id == "root").then(|| "scout".to_owned()));
    assert_eq!(shown.len(), 1);
}
