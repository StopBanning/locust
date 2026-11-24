//! Electron's `a-routine-waiting-for-keep-is-on-the-board.test.tsx`, case by case, on the Board the window draws
//! (home_panel.rs `board_view`) and the Routines screen it opens.
use super::*;
use crate::scout_cards::tests::{board_test_state, board_wait_card_now, click_key, collect_rt_text, render_state};
use crate::org::RoutineName;

/// The Board's state with a routine named Morning digest, Cedar's, whose copy holds `staged` files.
fn with_digest(scout: bool, staged: Option<usize>) -> State {
    let mut s = board_test_state(if scout { vec![board_wait_card_now(false)] } else { vec![] }, true);
    s.teammates.push(Teammate { id: "tm_cedar".into(), name: "Cedar".into(), role: "Custom".into(), runtime: Some("opencode".into()),
        model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
    s.org.routines.push(RoutineName { id: "rt_inbox".into(), name: "Morning digest".into(), steps: 1, teammate: Some("tm_cedar".into()), staged_files: staged });
    s
}

fn board(s: &State, _cx: &mut rune::Cx<State>) -> El {
    crate::home_panel::board_view(s, &Palette::new(true))
}

#[test]
fn a_waiting_routine_sits_above_the_conversation_with_its_face_and_three_acts() {
    // case: sits above the conversation, with Cedar’s face, and Keep, Discard and Open the copy
    let rt = render_state(with_digest(true, Some(2)), board);
    let y = |key: &str| rt.find(key).unwrap_or_else(|| panic!("no {key}")).1;
    assert!(y("board-scout-card_now") < y("board-routine-rt_inbox"), "the Scout card first, then the routine");
    let all = collect_rt_text(&rt);
    assert!(all.find("Morning digest").unwrap() < all.find("Rename the invoices").unwrap());
    for words in ["Waiting for you", "Cedar", "Keep", "Discard", "Discard the changes from Morning digest", "Open the copy", "3 waiting on you"] {
        assert!(all.contains(words), "{words} in {all}");
    }
    assert!(!all.contains("Discard for good?"), "Discard is asked first, not armed");
    // Cedar's face is on the card.
    let (card, _) = rt.presented("board-routine-rt_inbox").unwrap();
    assert!(rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains("tm_cedar"))
        && e.rect[0] >= card[0] && e.rect[1] >= card[1] && e.rect[1] < card[1] + card[3]), "Cedar's face in the card");
}

#[test]
fn a_board_with_no_routine_waiting_is_as_it_was() {
    // case: leaves the board as it was when no routine is waiting (control)
    let rt = render_state(with_digest(false, None), board);
    assert!(!rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("board-routine"))));
    let all = collect_rt_text(&rt);
    assert!(!all.lines().any(|line| line == "Keep"));
    assert!(all.contains("Rename the invoices"));
}

#[test]
fn opening_the_card_marks_that_routine_on_the_routines_screen() {
    // case: marks that row so Routines can bring it into view
    let mut rt = render_state(with_digest(false, Some(1)), board);
    rt.state.home.board_open = true;
    click_key(&mut rt, "board-routine-name-rt_inbox");
    assert!(rt.state.routines.open && !rt.state.home.board_open);
    assert_eq!(rt.state.routines.focus.as_deref(), Some("rt_inbox"));
    let mut s = rt.state;
    s.routines.file = serde_json::json!({"schemaVersion": 1, "routines": [
        {"routineId": "rt_inbox", "name": "Morning digest", "teammateId": "tm_cedar", "steps": ["Write the digest."], "runs": 1},
        {"routineId": "rt_other", "name": "Other", "teammateId": "tm_cedar", "steps": ["Write the digest."], "runs": 1}]});
    let rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 1800.0), rune::Env::new(true, 1.0));
    let mut rt = rt;
    rt.frame(0.0);
    let marks: Vec<_> = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("routine-bring-into-view")).collect();
    assert_eq!(marks.len(), 1, "one row is marked");
    let (row, _) = rt.presented("routine-rt_inbox").expect("the routine's row");
    assert!(marks[0].rect[1] >= row[1] && marks[0].rect[1] <= row[1] + row[3], "the mark is on rt_inbox's row");
    assert!(rt.find("routine-rt_other").is_some());
}

#[test]
fn the_boards_keep_is_the_routines_screens_keep() {
    // case: is the same Keep the Routines screen already uses
    let panel = include_str!("routines_panel.rs");
    let card = &panel[panel.find("pub(super) fn board_wait_card").unwrap()..];
    let card = &card[..card.find("\n}\n").unwrap()];
    // The Board settles through the screen's own settle, and opens the screen on that routine.
    assert!(card.contains("settle(s, &keep, locust_core::routine_recovery::Settle::Keep)"));
    assert!(card.contains("settle(s, &discard, locust_core::routine_recovery::Settle::Discard)"));
    assert!(card.contains("show_routine(s, &open, tx)"));
    assert!(panel.contains("on_press(move |s: &mut State| settle(s, &keep, locust_core::routine_recovery::Settle::Keep))"), "the screen's Keep");
    // Waiting for you's routine row opens the same place.
    assert!(include_str!("rail.rs").contains("crate::routines_panel::show_routine(s, routine_id, tx)"));
}
