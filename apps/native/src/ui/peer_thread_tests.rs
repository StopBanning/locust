//! three-placements.test.ts, part 3: what arrived from a teammate is attributed, not explained. Drawn headless through
//! `cards_for_mission`, the card the thread shows under the turn a message reached.
use super::*;
use locust_core::peer_thread::Message;
use rune::{Env, Runtime};

fn message(id: &str, direction: &str, from: (&str, &str), to: (&str, &str), text: &str) -> Message {
    Message {
        message_id: id.into(),
        direction: direction.into(),
        from_id: from.0.into(),
        from_name: from.1.into(),
        from_mission: (direction == "received").then(|| "m_yurt_own".to_owned()),
        to_id: to.0.into(),
        to_name: to.1.into(),
        text: Some(text.into()),
        posted_at: String::new(),
    }
}

const ME: (&str, &str) = ("tm_codec", "You");
const YURT: (&str, &str) = ("tm_yurt", "Yurt");

/// A back-and-forth: two from Yurt, one answered.
fn exchange() -> Vec<Message> {
    vec![
        message("w1", "received", YURT, ME, "Can you look at the parser?"),
        message("w2", "sent", ME, YURT, "Looking now."),
        message("w3", "received", YURT, ME, "Thanks, the failing case is in tests/parse.rs."),
    ]
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().children(cards_for_mission(s, "tm_codec", "m_mine", &exchange(), &Palette::new(true)))
}

fn drawn() -> Runtime<State> {
    let mut s = State::default();
    s.teammates.push(Teammate { id: "tm_yurt".into(), name: "Yurt".into(), role: "Reviewer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
    s.recent.push(ConversationRow { id: "m_yurt_own".into(), teammate: "Yurt".into(), title: "Parser review".into(), started_by: "native".into(), state: "completed".into(), updated: "2026-11-18T10:00:00Z".into() });
    s.peer_thread_open.insert("m_mine:tm_yurt".into(), true);
    let mut rt = Runtime::new(s, view, (900.0, 700.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text.or(e.label)).collect()
}

#[test]
fn says_the_teammate_sent_it_rather_than_naming_them_and_stopping() {
    let rt = drawn();
    let words = texts(&rt);
    assert!(words.iter().any(|w| w == "Yurt sent"), "got {words:?}");
    assert!(!words.iter().any(|w| w == "Yurt"), "never the bare name over a message");
}

#[test]
fn offers_the_senders_own_conversation_once_in_the_header() {
    let rt = drawn();
    let links: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("peer-open-sender-")))
        .collect();
    assert_eq!(links.len(), 1, "once per card, however many messages it holds");
    let header = rt.presented("peer-toggle-m_mine:tm_yurt").expect("the card's header").0;
    assert!((links[0].rect[1] - header[1]).abs() < header[3], "in the header, not under a message");
}

#[test]
fn never_claims_there_is_more_which_is_the_comparison_it_cannot_make() {
    let rt = drawn();
    let words = texts(&rt).join(" ").to_lowercase();
    for claim in ["full reply", "said more", "longer than", "only part"] {
        assert!(!words.contains(claim), "{claim:?} in {words}");
    }
}

#[test]
fn reaches_the_sender_by_their_own_mission_which_the_store_always_had() {
    let mut rt = drawn();
    let (x, y) = rt.find("peer-open-sender-m_mine-tm_yurt").expect("the link to Yurt's conversation");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert_eq!(
        rt.state.conversations.get("tm_yurt").and_then(|c| c.conversation_id.as_deref()),
        Some("m_yurt_own"),
        "the sender's own conversation, by the mission the message came from"
    );
}
