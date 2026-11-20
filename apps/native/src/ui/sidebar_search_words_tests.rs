//! what-running-it-again-does.test.ts, its sidebar half: the search names what the list holds (conversations), and a
//! search that matches nothing says so in the same noun, instead of the empty fold's "Nothing in here yet."

use super::*;

fn drawn(recents: usize, query: &str) -> Vec<String> {
    let mut s = State::default();
    for i in 0..recents {
        s.recent.push(ConversationRow { id: format!("mission_{i}"), title: format!("Tidy the release notes {i}"),
            teammate: String::new(), started_by: String::new(), state: "completed".into(), updated: "2026-11-20T05:00:00.000Z".into() });
    }
    s.sidebar_query.replace_text(query);
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().flat_map(|e| [e.text, e.label]).flatten().collect()
}

// case: names what this list actually holds
// case: does not promise missions, which live on their own screen and do not filter
#[test]
fn the_search_names_conversations_not_missions() {
    let src = include_str!("sidebar.rs");
    assert!(src.contains(".placeholder(\"Search conversations\""));
    assert!(!src.contains("Search missions"));
}

// case: agrees with itself when nothing matches
#[test]
fn a_search_that_matches_nothing_says_so_in_the_same_noun() {
    let words = drawn(2, "zebra crossing");
    assert!(words.iter().any(|w| w == "No conversations match that."), "{words:?}");
    assert!(!words.iter().any(|w| w == "Nothing in here yet."), "{words:?}");
    assert!(!words.iter().any(|w| w.contains("No missions match")), "{words:?}");
    // With nothing to search at all, it still says the search found nothing.
    assert!(drawn(0, "zebra").iter().any(|w| w == "No conversations match that."));
    // A search that finds one, or no search, never says it.
    assert!(!drawn(2, "release").iter().any(|w| w == "No conversations match that."));
    assert!(!drawn(2, "").iter().any(|w| w == "No conversations match that."));
}
