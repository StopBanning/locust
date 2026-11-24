//! Electron's renderer `a-conversation-can-be-pinned.test.tsx` through the sidebar as drawn (`sidebar`): the recent
//! rows, a continued conversation keyed by its first turn (its root), and the pins the person kept (`org.pins`).
//! Expected words are Electron's, written out.
use super::*;

fn conversation(s: &mut State, id: &str, title: &str, parent: Option<&str>, minutes_ago: i64) {
    let at = (chrono::DateTime::parse_from_rfc3339("2026-10-10T05:00:00Z").unwrap() - chrono::Duration::minutes(minutes_ago)).to_rfc3339();
    s.recent.push(ConversationRow { id: id.into(), title: title.into(), teammate: String::new(), started_by: "native".into(), state: "completed".into(), updated: at.clone() });
    s.ledger_summaries.insert(id.into(), ledger::Summary { mission_id: id.into(), title: title.into(), parent: parent.map(Into::into), created_at: at.clone(), updated: at, ..Default::default() });
}

/// The native sidebar draws its section headings in capitals (`look::controls::label`), so Electron's words `Pinned` and
/// `Recents` are matched whatever their case.
/// Alpha, Bravo, and Charlie continued from its first turn `m_root_c`, with `pins` kept; the sidebar's words in order.
fn sidebar_words(pins: &[&str]) -> Vec<String> {
    let mut s = State::default();
    s.demo = true;
    conversation(&mut s, "m_a", "Alpha", None, 0);
    conversation(&mut s, "m_b", "Bravo", None, 0);
    conversation(&mut s, "m_root_c", "Charlie", None, 10);
    conversation(&mut s, "m_c", "Charlie", Some("m_root_c"), 0);
    s.org.pins = pins.iter().map(|p| (*p).to_owned()).collect();
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

// case: puts the pinned ones on top, newest pin first, and the rest under Recents, each once
#[test]
fn pinned_conversations_sit_on_top_newest_pin_first_and_the_rest_under_recents() {
    let words = sidebar_words(&["m_root_c", "m_b"]);
    let order: Vec<usize> = ["Pinned", "Charlie", "Bravo", "Recents", "Alpha"]
        .iter()
        .map(|word| words.iter().position(|w| w.eq_ignore_ascii_case(word)).unwrap_or_else(|| panic!("{word}: {words:?}")))
        .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, order, "{words:?}");
    assert_eq!(words.iter().filter(|w| *w == "Bravo").count(), 1, "{words:?}");
    assert_eq!(words.iter().filter(|w| *w == "Charlie").count(), 1, "one row for the continued conversation: {words:?}");
}

// case: looks as it did with nothing pinned
#[test]
fn with_nothing_pinned_the_sidebar_has_no_pinned_or_recents_heading() {
    let words = sidebar_words(&[]);
    assert!(!words.iter().any(|w| w.eq_ignore_ascii_case("Pinned")), "{words:?}");
    assert!(!words.iter().any(|w| w.eq_ignore_ascii_case("Recents")), "{words:?}");
    for title in ["Alpha", "Bravo", "Charlie"] {
        assert!(words.iter().any(|w| w == title), "{title}: {words:?}");
    }
}
