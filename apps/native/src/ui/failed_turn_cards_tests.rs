//! Electron `Thread.tsx` (the `limit` item, `DiagnosticLine`, `ReceiptCard`), seen in live hour 1 on a turn that ended on
//! Claude's 5-hour limit: the red "Usage limit reached" card with its tag and a reset in a person's words, the limit's
//! sentence in red with no "error:" in front, and the durable receipt under them. Drawn headless through the message
//! column, the way the window draws the newest turn.
use super::*;
use crate::messages::message_column;
use locust_core::ledger::{Item, Snapshot};
use std::collections::BTreeMap;

const LIMIT: &str = "5-hour window 100% used \u{b7} resets 2026-11-26T01:30:00.000Z";
const SAID: &str = "You've hit your 5-hour limit \u{b7} resets 8:30pm";

fn meta() -> ledger::Metadata {
    ledger::Metadata {
        mission_id: "mission_8f3a91c2d4".into(),
        prompt: "Do the looks pass".into(),
        runtime: "claude".into(),
        model: "opus".into(),
        cli_version: Some("2.1.322".into()),
        ..Default::default()
    }
}

fn failed_items() -> Vec<Item> {
    vec![
        Item::User("Do the looks pass".into()),
        Item::Limit { kind: "quota-exhausted".into(), text: LIMIT.into() },
        Item::Note { level: "error".into(), text: SAID.into() },
    ]
}

/// The newest turn drawn after `tweak` has changed its snapshot; every element, parents before children.
fn drawn(items: Vec<Item>, tweak: impl FnOnce(&mut Snapshot)) -> Vec<rune::ElementInfo> {
    let mut snapshot = Snapshot { turn_metas: vec![meta()], tip: "mission_8f3a91c2d4".into(), failed: true, ..Default::default() };
    tweak(&mut snapshot);
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("mission_8f3a91c2d4".into()), turns: Vec::new() },
        items: Vec::new(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Wren", &Palette::new(true), s), (800.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn words(drawn: &[rune::ElementInfo]) -> Vec<String> {
    let mut with_words: Vec<_> = drawn.iter().filter(|e| e.text.is_some()).collect();
    with_words.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    with_words.iter().filter_map(|e| e.text.clone()).collect()
}

fn find<'a>(drawn: &'a [rune::ElementInfo], key: &str) -> &'a rune::ElementInfo {
    drawn.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("nothing drawn under {key}: {:?}", words(drawn)))
}

#[test]
fn a_turn_that_ended_on_a_usage_limit_draws_the_red_card_with_its_tag() {
    let d = drawn(failed_items(), |_| {});
    assert_eq!(words(&d).iter().filter(|w| *w == "Usage limit reached").count(), 1, "{:?}", words(&d));
    assert_eq!(find(&d, "limit-tag").text.as_deref(), Some("QUOTA-EXHAUSTED"));
    let look = crate::look::controls::Look::new(true);
    assert_eq!(find(&d, "limit-tag").text_color, Some(look.c("red-text")));
}

#[test]
fn the_reset_is_in_a_persons_words_never_the_raw_instant() {
    let d = drawn(failed_items(), |_| {});
    let said = find(&d, "limit-said").text.clone().unwrap();
    assert!(!said.contains("2026-11-26T"), "a raw ISO time: {said}");
    let shape = locust_core::invariants::regex(r"^5-hour window 100% used · resets ([A-Z][a-z]{2} )?\d{1,2}:\d{2} (AM|PM)$");
    assert!(shape.is_match(&said), "{said}");
}

#[test]
fn a_warning_that_is_only_a_temporary_limit_is_a_line_not_the_red_card() {
    let items = vec![Item::User("hi".into()), Item::Limit { kind: "temporary-rate-limit".into(), text: LIMIT.into() }];
    let d = drawn(items, |snapshot| snapshot.failed = false);
    assert!(!words(&d).iter().any(|w| w == "Usage limit reached"), "{:?}", words(&d));
    let line = words(&d).into_iter().find(|w| w.starts_with("5-hour window")).expect("the warning is said");
    assert!(!line.contains("2026-11-26T"), "{line}");
}

#[test]
fn the_error_is_its_sentence_in_red_with_no_error_label() {
    let d = drawn(failed_items(), |_| {});
    let said = d.iter().find(|e| e.text.as_deref() == Some(SAID)).unwrap_or_else(|| panic!("the sentence alone: {:?}", words(&d)));
    let look = crate::look::controls::Look::new(true);
    assert_eq!(said.text_color, Some(look.c("red-text")));
    assert!(!words(&d).iter().any(|w| w.starts_with("error:")), "{:?}", words(&d));
}

#[test]
fn a_warning_is_its_sentence_in_amber_with_no_warning_label() {
    let items = vec![Item::User("hi".into()), Item::Note { level: "warning".into(), text: "Codex retried the connection".into() }];
    let d = drawn(items, |snapshot| snapshot.failed = false);
    let said = d.iter().find(|e| e.text.as_deref() == Some("Codex retried the connection")).unwrap_or_else(|| panic!("{:?}", words(&d)));
    assert_eq!(said.text_color, Some(crate::look::controls::Look::new(true).c("amber-text")));
}

#[test]
fn a_failed_turn_ends_on_the_durable_receipt() {
    let d = drawn(failed_items(), |_| {});
    assert_eq!(find(&d, "receipt-title").text.as_deref(), Some("DURABLE RECEIPT"));
    assert_eq!(find(&d, "receipt-tag").text.as_deref(), Some("FAILED"));
    assert_eq!(find(&d, "receipt-line").text.as_deref(), Some("claude 2.1.322 / opus \u{b7} no checkpoints \u{b7} record readable"));
    assert!(words(&d).iter().any(|w| w == "restored from local ledger"));
    // Under the limit card and the error, not above them.
    let at = |key: &str| find(&d, key).rect[1];
    assert!(at("durable-receipt") > at("limit-tag"));
}

#[test]
fn the_receipts_line_counts_checkpoints_and_names_what_never_reported_back() {
    let d = drawn(failed_items(), |snapshot| {
        snapshot.turn_metas[0].checkpoint_unsettled = BTreeMap::from([(1, 0), (2, 2)]);
    });
    assert_eq!(find(&d, "receipt-line").text.as_deref(), Some("claude 2.1.322 / opus \u{b7} 2 checkpoints \u{b7} record readable \u{b7} 2 unverified"));
}

#[test]
fn a_record_that_could_not_be_read_says_incomplete_and_a_stopped_turn_has_no_second_receipt() {
    let d = drawn(failed_items(), |snapshot| snapshot.damaged = true);
    assert_eq!(find(&d, "receipt-line").text.as_deref(), Some("claude 2.1.322 / opus \u{b7} no checkpoints \u{b7} record incomplete"));
    // A routine, finished turn carries no news: no card (Electron draws its quiet "Details" link instead).
    let ok = drawn(vec![Item::User("hi".into()), Item::Assistant { item_id: "a".into(), text: "Done.".into(), r#final: true }], |snapshot| snapshot.failed = false);
    assert!(ok.iter().all(|e| e.key.as_deref() != Some("durable-receipt")), "{:?}", words(&ok));
}

#[test]
fn a_turn_still_running_has_no_receipt_yet() {
    let d = drawn(failed_items(), |snapshot| snapshot.running = true);
    assert!(d.iter().all(|e| e.key.as_deref() != Some("durable-receipt")));
}

#[test]
fn the_receipt_line_opens_the_rest_of_the_record() {
    let items = failed_items();
    let mut snapshot = Snapshot { turn_metas: vec![meta()], tip: "mission_8f3a91c2d4".into(), failed: true, ..Default::default() };
    snapshot.turn_metas[0].cwd = Some("/Users/c/work/locust".into());
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("mission_8f3a91c2d4".into()), turns: Vec::new() },
        items: Vec::new(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    // A press needs the app's state to be the State alone, and a view function that holds no items of its own.
    thread_local! { static THREAD: std::cell::RefCell<Vec<Item>> = const { std::cell::RefCell::new(Vec::new()) }; }
    THREAD.with(|held| *held.borrow_mut() = items);
    let mut rt = rune::Runtime::new(s, |s, _| THREAD.with(|held| message_column(&held.borrow(), "Wren", &Palette::new(true), s)), (800.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(!words(&rt.elements()).iter().any(|w| w == "Checkpoints"), "closed at rest");
    let (x, y) = rt.find("receipt-summary").expect("the line is a press");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    let opened = words(&rt.elements());
    for label in ["Runtime", "Ran on", "Run", "Checkpoints", "Ledger"] {
        assert!(opened.iter().any(|w| w == label), "{label} in {opened:?}");
    }
    assert!(opened.iter().any(|w| w == "8f3a91c2"), "{opened:?}");
    assert!(opened.iter().any(|w| w.ends_with("locust") && w.contains('\u{b7}')), "the folder is named: {opened:?}");
}
