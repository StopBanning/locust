//! Electron's `a-board-of-conversations.test.tsx` through the window's Board (`board_view`): the recent rows the
//! sidebar holds, the asks the approval cards made, and the conversations still to look at. Expected words are
//! Electron's, written out.
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

/// The Board drawn over `rows`: (every word top to bottom, every element).
fn board(rows: Vec<ConversationRow>, prep: impl FnOnce(&mut State)) -> (Vec<String>, Vec<rune::ElementInfo>) {
    let mut s = State::default();
    s.teammates = vec![wren()];
    s.recent = rows;
    s.home.board_open = true;
    prep(&mut s);
    let mut rt = rune::Runtime::new(s, |s, _| board_view(s, &Palette::new(true)), (1200.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let els = rt.elements();
    (els.iter().filter_map(|e| e.text.clone()).collect(), els)
}

// case: draws each column with its count, and each card with its face, title and state
#[test]
fn the_board_draws_columns_with_counts_and_cards_with_face_title_and_state() {
    let rows = vec![
        conversation("asks", "Rename the invoices", "running", 2),
        conversation("live", "Fix the cart total", "running", 10),
        conversation("ended", "Write the brief", "completed", 30),
        conversation("old", "Yesterday", "completed", 25 * 60),
    ];
    let (words, els) = board(rows, |s| {
        s.home.asks.insert("asks".into(), BoardAsk { approval_id: "ask_1".into(), kind: "question".into(), command: None, where_: None, shown: std::time::Instant::now() });
        s.home.waiting_for.insert("asks".into(), "Run a command: git mv invoices old-invoices".into());
        s.home.to_look_at.insert("ended".into());
    });
    // Row 8: a column is titled in the section-label voice, as Electron's `.lc-boardsection` title is (notes/PAGES-NOTES.md, Row 41).
    for title in ["NEEDS YOU", "WORKING", "READY TO LOOK AT", "DONE"] {
        assert!(words.iter().any(|w| w == title), "{title}: {words:?}");
    }
    assert!(words.iter().any(|w| w == "1 waiting on you · 1 working"), "{words:?}");
    assert!(els.iter().any(|e| e.key.as_deref() == Some("board-column/needs-you")), "the Needs you column");
    // Each column with its count.
    let count = |key: &str| els.iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone());
    assert_eq!(count("board-needs-you-count").as_deref(), Some("1"));
    assert_eq!(count("board-working-count").as_deref(), Some("1"));
    assert_eq!(count("board-to-look-at-count").as_deref(), Some("1"));
    assert_eq!(count("board-done-count").as_deref(), Some("1"));
    assert!(words.iter().any(|w| w == "Wren \u{b7} Run a command: git mv invoices old-invoices"), "{words:?}");
    assert!(words.iter().any(|w| w == "Rename the invoices"), "{words:?}");
    assert!(words.iter().any(|w| w == "Wren \u{b7} working"), "{words:?}");
    assert!(words.iter().any(|w| w.starts_with("Wren \u{b7} finished \u{b7} ")), "{words:?}");
    // The owner's face and name on its cards.
    assert!(els.iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("bot") && k.contains("tm_wren"))), "Wren's face");
    // A card the column order puts first: the one waiting.
    let at = |title: &str| words.iter().position(|w| w == title).unwrap_or(usize::MAX);
    assert!(at("Rename the invoices") < at("Fix the cart total"), "{words:?}");
}

// case: says it is quiet, and lists what is done, when nothing needs the person (control)
#[test]
fn a_quiet_board_says_so_and_still_lists_what_is_done() {
    let (words, _) = board(vec![conversation("old", "Yesterday", "completed", 25 * 60)], |_| {});
    assert!(words.iter().any(|w| w == "all quiet"), "{words:?}");
    assert!(words.iter().any(|w| w.starts_with("Nothing is waiting on you and nothing is running.")), "{words:?}");
    assert!(!words.iter().any(|w| w == "NEEDS YOU"), "{words:?}");
    assert!(!words.iter().any(|w| w == "WORKING"), "{words:?}");
    assert!(words.iter().any(|w| w == "Yesterday"), "{words:?}");
}
