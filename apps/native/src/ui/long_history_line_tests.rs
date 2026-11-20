//! The sidebar's half of a-long-history-says-what-it-lists.test.ts: a history longer than the list says how much of
//! it is listed, under the conversations, and says nothing when all of it is (Screens.tsx's meta words).
use super::*;

fn drawn(total: Option<usize>) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.recent = (1..=3)
        .map(|i| ConversationRow {
            id: format!("m_{i}"),
            teammate: "Wren".into(),
            title: format!("Task {i}"),
            started_by: "native".into(),
            state: "completed".into(),
            updated: format!("2026-11-18T10:0{i}:00Z"),
        })
        .collect();
    s.recent_total = total;
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

#[test]
fn a_longer_history_says_how_much_is_listed() {
    let all = drawn(Some(2_005));
    let line = all.iter().find(|e| e.key.as_deref() == Some("sidebar-listed-of")).expect("the line is drawn");
    assert_eq!(line.text.as_deref(), Some("Only the newest 3 of 2,005 turns are listed"));
}

#[test]
fn a_history_listed_whole_says_nothing_more() {
    assert!(!drawn(None).iter().any(|e| e.key.as_deref() == Some("sidebar-listed-of")));
    assert!(!drawn(Some(3)).iter().any(|e| e.key.as_deref() == Some("sidebar-listed-of")), "a total no larger is not said");
}
