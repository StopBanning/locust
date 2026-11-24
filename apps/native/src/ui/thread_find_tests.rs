//! a-conversation-can-be-searched.test.ts through the window: the find bar (`thread_find_bar`) over a conversation's
//! pieces of text, typed into, counted, and stepped with its buttons and with Enter / Shift+Enter (`on_key`). The
//! expected hits, words and steps are Electron's own (threadFind.ts's test).
use super::*;
use crate::thread_find::{FindHit, MAX_FIND_HITS};
use rune::{Env, Key, Runtime};

/// The find bar open over these pieces of the conversation (the whole of it, earlier turns included).
fn bar(pieces: &[&str]) -> Runtime<State> {
    let mut s = State::default();
    s.find_pieces = pieces.iter().map(|p| (*p).to_owned()).collect();
    s.find_show_earlier = true;
    open_find(&mut s);
    let mut rt = Runtime::new(
        s,
        |s, _| thread_find_bar(s, &Palette::new(true)).unwrap_or_else(el),
        (900.0, 120.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

/// Types the query into the bar's own field, as a person does.
fn type_query(rt: &mut Runtime<State>, query: &str) {
    press(rt, "find-query");
    rt.paste(query);
    rt.frame(0.016);
    rt.update(|s, _| refresh_find_hits(s));
    rt.frame(0.016);
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..6 {
        rt.frame(0.016);
    }
}

fn press(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} on the bar"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn hits(rt: &Runtime<State>) -> Vec<FindHit> {
    rt.state.find_hits.clone()
}

fn hit(from: (usize, usize), to: (usize, usize)) -> FindHit {
    FindHit { from, to }
}

/// What the bar says beside the field: "3 of 12", "No matches", or nothing.
fn count_words(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().filter_map(|e| e.text).find(|t| {
        t == "No matches" || t.split_once(" of ").is_some_and(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

#[test]
fn finds_every_place_the_words_are_whatever_their_case() {
    // case: finds every place the words are, whatever their case
    let mut rt = bar(&["The deploy failed. Deploy again?"]);
    type_query(&mut rt, "deploy");
    assert_eq!(rt.state.find_query.text, "deploy", "typed into the bar's field");
    assert_eq!(hits(&rt), vec![hit((0, 4), (0, 10)), hit((0, 19), (0, 25))]);
    assert_eq!(count_words(&rt).as_deref(), Some("1 of 2"));
}

#[test]
fn finds_words_that_cross_a_bold_word_or_a_code_span_and_ends_inside_the_piece_that_holds_the_last_letter() {
    // case: finds words that cross a bold word or a code span, and ends inside the piece that holds the last letter
    let mut rt = bar(&["Run ", "npm test", " again"]);
    type_query(&mut rt, "run npm test a");
    assert_eq!(hits(&rt), vec![hit((0, 0), (2, 2))]);

    let mut rt = bar(&["Run ", "npm test", " again"]);
    type_query(&mut rt, "npm test");
    assert_eq!(hits(&rt), vec![hit((1, 0), (1, 8))], "a match that ends where its piece ends stays in it");

    let mut rt = bar(&["ab", "", "cd"]);
    type_query(&mut rt, "bc");
    assert_eq!(hits(&rt), vec![hit((0, 1), (2, 1))], "empty pieces in between change nothing");
}

#[test]
fn takes_what_is_typed_as_words_never_as_a_pattern() {
    // case: takes what is typed as words, never as a pattern
    let mut rt = bar(&["cost (USD): $4.20 vs $4x20"]);
    type_query(&mut rt, "$4.20");
    assert_eq!(hits(&rt), vec![hit((0, 12), (0, 17))]);

    let mut rt = bar(&["a+b and a*b"]);
    type_query(&mut rt, "a*b");
    assert_eq!(hits(&rt), vec![hit((0, 8), (0, 11))]);
}

#[test]
fn finds_nothing_for_an_empty_or_blank_query_and_stops_at_the_most_it_looks_for() {
    // case: finds nothing for an empty or blank query, and stops at the most it looks for
    let mut rt = bar(&["anything"]);
    rt.update(|s, _| refresh_find_hits(s));
    assert!(hits(&rt).is_empty(), "nothing typed, nothing found");

    let mut rt = bar(&["anything"]);
    type_query(&mut rt, "   ");
    assert!(hits(&rt).is_empty(), "a blank query finds nothing");

    let mut rt = bar(&[]);
    type_query(&mut rt, "a");
    assert!(hits(&rt).is_empty(), "nothing to look in");

    let long = "a".repeat(2000 + 50);
    let mut rt = bar(&[long.as_str()]);
    type_query(&mut rt, "a");
    assert_eq!(hits(&rt).len(), 2000);
    assert_eq!(MAX_FIND_HITS, 2000);
}

#[test]
fn counts_the_hit_it_is_on_says_when_there_are_none_and_says_nothing_before_anything_is_typed() {
    // case: counts the hit it is on, says when there are none, and says nothing before anything is typed
    let twelve = "deploy ".repeat(12);
    let mut rt = bar(&[twelve.as_str()]);
    assert_eq!(count_words(&rt), None, "nothing before anything is typed");
    type_query(&mut rt, "deploy");
    press(&mut rt, "find-next");
    press(&mut rt, "find-next");
    assert_eq!(count_words(&rt).as_deref(), Some("3 of 12"));

    let mut rt = bar(&["nothing here"]);
    type_query(&mut rt, "deploy");
    assert_eq!(count_words(&rt).as_deref(), Some("No matches"));

    let long = "a".repeat(2000 + 50);
    let mut rt = bar(&[long.as_str()]);
    type_query(&mut rt, "a");
    assert_eq!(count_words(&rt).as_deref(), Some("1 of 2000+"));
}

#[test]
fn steps_to_the_next_hit_and_the_one_before_round_the_end_and_back_to_the_start() {
    // case: steps to the next hit and the one before, round the end and back to the start
    let mut rt = bar(&["deploy deploy deploy"]);
    type_query(&mut rt, "deploy");
    assert_eq!(rt.state.find_index, 0);
    press(&mut rt, "find-next");
    assert_eq!(rt.state.find_index, 1);
    press(&mut rt, "find-next");
    press(&mut rt, "find-next");
    assert_eq!(rt.state.find_index, 0, "round the end and back to the start");
    press(&mut rt, "find-prev");
    assert_eq!(rt.state.find_index, 2, "the one before the first is the last");

    // Enter in the field steps on, Shift+Enter back, as the bar's own keys.
    press(&mut rt, "find-query");
    assert_eq!(rt.state.find_index, 2, "going back to the field keeps the hit");
    rt.key(Key::Enter.into());
    rt.frame(0.016);
    assert_eq!(rt.state.find_index, 0);
    rt.key(rune::KeyEvent { key: Key::Enter, mods: rune::Mods { shift: true, ..Default::default() } });
    rt.frame(0.016);
    assert_eq!(rt.state.find_index, 2);

    // With nothing found, a press goes nowhere.
    let mut rt = bar(&["nothing"]);
    type_query(&mut rt, "deploy");
    press(&mut rt, "find-next");
    assert_eq!(rt.state.find_index, 0);
}
