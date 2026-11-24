//! exchange.test.ts through the window: the turns the window has read (`ledger_summaries`, their owners and the
//! message ids each posted or received), the conversation open with its own turns, and the exchange strip the thread
//! draws over it (`compute`, then `view`). Electron's missions, names and costs, written out. Electron gives a live run
//! its own run id; the window stops a live turn by its mission id, so that is the id it lists.
use super::*;
use locust_core::ledger::PeerLink;

struct Turn {
    id: &'static str,
    owner: Option<(&'static str, &'static str)>,
    runtime: &'static str,
    model: &'static str,
    hop: Option<usize>,
    messages: Vec<&'static str>,
    usd: Option<f64>,
    live: bool,
}

fn turn(id: &'static str, owner: Option<(&'static str, &'static str)>, messages: &[&'static str]) -> Turn {
    Turn { id, owner, runtime: "codex", model: "account-default", hop: None, messages: messages.to_vec(), usd: None, live: false }
}

const WREN: Option<(&str, &str)> = Some(("tm_wren", "Wren"));
const BOOTY: Option<(&str, &str)> = Some(("tm_booty", "Booty"));
const ATLAS: Option<(&str, &str)> = Some(("tm_atlas", "Atlas"));

/// The window holding `turns`, with the conversation whose turns are `open` (newest last) on screen.
fn window(turns: &[Turn], open: &[&str]) -> State {
    let mut s = State::default();
    for t in turns {
        if let Some((id, name)) = t.owner {
            s.mission_owners.insert(t.id.into(), id.into());
            if !s.teammates.iter().any(|m| m.id == id) {
                s.teammates.push(Teammate { id: id.into(), name: name.into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
            }
        }
        s.ledger_summaries.insert(t.id.into(), ledger::Summary {
            mission_id: t.id.into(),
            runtime: t.runtime.into(),
            model: t.model.into(),
            started_by_hop: t.hop,
            running: t.live,
            cost: t.usd.map(|usd| locust_core::benchmark::RunCost { usd: Some(usd), ..Default::default() }),
            peer_links: t.messages.iter().map(|m| PeerLink { message_id: (*m).into(), ..Default::default() }).collect(),
            ..Default::default()
        });
    }
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None },
        conversation: Conversation { conversation_id: open.first().map(|id| (*id).to_owned()), turns: vec![] },
        items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            tip: open.last().map(|id| (*id).to_owned()).unwrap_or_default(),
            turn_metas: open.iter().map(|id| ledger::Metadata { mission_id: (*id).into(), runtime: "codex".into(), ..Default::default() }).collect(),
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    s
}

fn strip_words(s: State) -> Option<Vec<String>> {
    let mut rt = rune::Runtime::new(s, |s, _| col().w(1000.0).child(view(s, &Palette::new(true)).unwrap_or_else(|| col().key("no-strip"))), (1000.0, 200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    if rt.find("no-strip").is_some() {
        return None;
    }
    Some(rt.elements().into_iter().filter_map(|e| e.text).collect())
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

// case: is nothing for a conversation that talked to nobody
#[test]
fn is_nothing_for_a_conversation_that_talked_to_nobody() {
    let s = window(&[turn("m1", WREN, &[])], &["m1"]);
    assert!(compute(&s).is_none());
    assert!(strip_words(s).is_none(), "no strip over the thread");
}

// case: links two missions by the message one posted and the other received
#[test]
fn links_two_missions_by_the_message_one_posted_and_the_other_received() {
    let wren = Turn { runtime: "cursor", model: "auto", ..turn("m_wren", WREN, &["wm_1"]) };
    let booty = Turn { runtime: "claude", model: "claude/sonnet", hop: Some(1), live: true, ..turn("m_booty", BOOTY, &["wm_1"]) };
    let s = window(&[wren, booty], &["m_wren"]);
    let (overview, _) = compute(&s).expect("an exchange");
    let parts: Vec<_> = overview.participants.iter().map(|p| (p.name.as_str(), p.runtime.as_str(), p.model.as_str(), p.live)).collect();
    assert_eq!(sorted(parts.iter().map(|p| format!("{p:?}")).collect()), sorted(vec![
        format!("{:?}", ("Wren", "cursor", "auto", false)),
        format!("{:?}", ("Booty", "claude", "claude/sonnet", true)),
    ]));
    assert_eq!(overview.hops, 1);
    assert_eq!(overview.live_run_ids, ["m_booty"]);
    assert_eq!(sorted(overview.mission_ids.clone()), ["m_booty", "m_wren"]);
}

// case: reaches a third teammate through the second, and takes the deepest hop
#[test]
fn reaches_a_third_teammate_through_the_second_and_takes_the_deepest_hop() {
    let turns = [
        turn("m_wren", WREN, &["wm_1"]),
        Turn { hop: Some(1), ..turn("m_booty", BOOTY, &["wm_1", "wm_2"]) },
        Turn { hop: Some(2), ..turn("m_atlas", ATLAS, &["wm_2"]) },
        // Unrelated: a mission that shares no message id stays out.
        turn("m_other", WREN, &["wm_9"]),
    ];
    let s = window(&turns, &["m_atlas"]);
    let (overview, _) = compute(&s).expect("an exchange");
    assert_eq!(sorted(overview.participants.iter().map(|p| p.name.clone()).collect()), ["Atlas", "Booty", "Wren"]);
    assert_eq!(overview.hops, 2);
    assert!(!overview.mission_ids.iter().any(|m| m == "m_other"));
    let words = strip_words(s).expect("the strip");
    assert!(words.iter().any(|w| w.contains("2 of ") && w.contains("automatic replies")), "{words:?}");
}

// case: adds up what every run in the exchange cost
#[test]
fn adds_up_what_every_run_in_the_exchange_cost() {
    let s = window(&[Turn { usd: Some(0.2), ..turn("m_wren", WREN, &["wm_1"]) }, Turn { usd: Some(0.25), ..turn("m_booty", BOOTY, &["wm_1"]) }], &["m_wren"]);
    let usd = compute(&s).and_then(|(o, _)| o.cost).and_then(|c| c.usd).expect("a cost");
    assert!((usd - 0.45).abs() < 1e-5, "{usd}");
    let words = strip_words(s).expect("the strip");
    assert!(words.iter().any(|w| w.ends_with("· $0.45")), "{words:?}");
}

// case: folds one teammate’s several turns into one participant, on their latest route
#[test]
fn folds_one_teammates_several_turns_into_one_participant_on_their_latest_route() {
    let turns = [
        Turn { runtime: "cursor", model: "auto", ..turn("m_w1", WREN, &["wm_1"]) },
        turn("m_b", BOOTY, &["wm_1", "wm_2"]),
        Turn { runtime: "claude", model: "claude/fable", ..turn("m_w2", WREN, &["wm_2"]) },
    ];
    let s = window(&turns, &["m_w1"]);
    let (overview, _) = compute(&s).expect("an exchange");
    let wren = overview.participants.iter().find(|p| p.name == "Wren").expect("Wren");
    assert_eq!(sorted(wren.mission_ids.clone()), ["m_w1", "m_w2"]);
    assert_eq!((wren.runtime.as_str(), wren.model.as_str()), ("claude", "claude/fable"));
}

// case: names a conversation of nobody’s as such, rather than dropping it
#[test]
fn names_a_conversation_of_nobodys_as_such_rather_than_dropping_it() {
    let s = window(&[turn("m_n", None, &["wm_1"]), turn("m_b", BOOTY, &["wm_1"])], &["m_b"]);
    let (overview, _) = compute(&s).expect("an exchange");
    assert_eq!(sorted(overview.participants.iter().map(|p| p.name.clone()).collect()), ["Booty", "Nobody"]);
    let words = strip_words(s).expect("the strip");
    assert!(words.iter().any(|w| w.contains("Nobody")), "{words:?}");
}

/// A room post's two halves: each teammate's opening, and the other's reply to it.
fn halves() -> Vec<Turn> {
    vec![
        turn("m_wren1", WREN, &["wm_a"]),
        Turn { hop: Some(1), ..turn("m_booty2", BOOTY, &["wm_a"]) },
        turn("m_booty1", BOOTY, &["wm_b"]),
        Turn { hop: Some(1), ..turn("m_wren2", WREN, &["wm_b"]) },
    ]
}

// case: walking from one opening sees only that half
#[test]
fn walking_from_one_opening_sees_only_that_half() {
    let s = window(&halves(), &["m_wren1"]);
    assert_eq!(sorted(compute(&s).expect("an exchange").0.mission_ids), ["m_booty2", "m_wren1"]);
}

// case: walking from both openings is the whole argument
#[test]
fn walking_from_both_openings_is_the_whole_argument() {
    let s = window(&halves(), &["m_wren1", "m_booty1"]);
    let (whole, _) = compute(&s).expect("an exchange");
    assert_eq!(sorted(whole.mission_ids.clone()), ["m_booty1", "m_booty2", "m_wren1", "m_wren2"]);
    // Two people, four turns -- not four people.
    assert_eq!(sorted(whole.participants.iter().map(|p| p.name.clone()).collect()), ["Booty", "Wren"]);
}

// case: takes the deepest hop of any half, so the budget is the argument’s
#[test]
fn takes_the_deepest_hop_of_any_half_so_the_budget_is_the_arguments() {
    let mut turns = halves();
    turns.push(Turn { hop: Some(1), ..turn("m_booty3", BOOTY, &["wm_c"]) });
    let s = window(&turns, &["m_wren1", "m_booty1"]);
    assert_eq!(compute(&s).expect("an exchange").0.hops, 1);
}

// case: is nothing when none of the openings exists
#[test]
fn is_nothing_when_none_of_the_openings_exists() {
    let s = window(&halves(), &["m_nope", "m_nope_too"]);
    assert!(compute(&s).is_none());
    assert!(strip_words(s).is_none());
    // A conversation with no turn read yet has no opening at all.
    let s = window(&halves(), &[]);
    assert!(compute(&s).is_none());
}

// case: is nothing when the openings talked to nobody
#[test]
fn is_nothing_when_the_openings_talked_to_nobody() {
    let s = window(&[turn("m1", WREN, &[]), turn("m2", BOOTY, &[])], &["m1", "m2"]);
    assert!(compute(&s).is_none());
    assert!(strip_words(s).is_none());
}
