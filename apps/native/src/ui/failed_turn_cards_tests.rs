//! A turn that did not go through, drawn headless through the message column the way the window draws the newest
//! turn. Electron's `Thread.tsx` (`DiagnosticLine`, `ReceiptCard`) and, for a usage limit, Big Brother's review of
//! 26 Nov (`docs/REVIEW-ui-changes-2026-11-26.md`, change 2, approved by Colin as better than Electron's
//! `Thread.tsx:557-564`): ONE amber card, "Claude Code reached its 5-hour limit · resets Wed 8:30 PM", the carry-on
//! button inside it, no raw tag, no second line repeating the limit, and the receipt folded to "Details".
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

/// A turn that ended on Claude's limit: the limit, the run's own sentence about it, and the offer to carry on.
fn limited_items() -> Vec<Item> {
    vec![
        Item::User("Do the looks pass".into()),
        Item::Limit { kind: "quota-exhausted".into(), text: LIMIT.into() },
        Item::Note { level: "error".into(), text: SAID.into() },
        Item::CarryOn { conversation_id: "mission_8f3a91c2d4".into(), from: "claude".into(), why: "plan".into() },
    ]
}

/// A turn that failed for a real reason: the error alone.
fn failed_items() -> Vec<Item> {
    vec![
        Item::User("Do the looks pass".into()),
        Item::Note { level: "error".into(), text: "Claude Code could not start: spawn ENOENT".into() },
    ]
}

thread_local! { static THREAD: std::cell::RefCell<Vec<Item>> = const { std::cell::RefCell::new(Vec::new()) }; }

/// The newest turn in a window of its own (a press needs the app's state to be the State alone).
fn window(items: Vec<Item>, tweak: impl FnOnce(&mut Snapshot, &mut State)) -> rune::Runtime<State> {
    let mut snapshot = Snapshot { turn_metas: vec![meta()], tip: "mission_8f3a91c2d4".into(), failed: true, ..Default::default() };
    let mut s = State::default();
    tweak(&mut snapshot, &mut s);
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
    THREAD.with(|held| *held.borrow_mut() = items);
    let mut rt = rune::Runtime::new(s, |s, _| THREAD.with(|held| message_column(&held.borrow(), "Wren", &Palette::new(true), s)), (800.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn drawn(items: Vec<Item>, tweak: impl FnOnce(&mut Snapshot)) -> Vec<rune::ElementInfo> {
    window(items, |snapshot, _| tweak(snapshot)).elements()
}


fn words(drawn: &[rune::ElementInfo]) -> Vec<String> {
    let mut with_words: Vec<_> = drawn.iter().filter(|e| e.text.is_some()).collect();
    with_words.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    with_words.iter().filter_map(|e| e.text.clone()).collect()
}

fn find<'a>(drawn: &'a [rune::ElementInfo], key: &str) -> &'a rune::ElementInfo {
    drawn.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("nothing drawn under {key}: {:?}", words(drawn)))
}

fn with_offer(s: &mut State) {
    s.carry_offer = Some(("codex".into(), None));
}

fn card_of(drawn: &[rune::ElementInfo]) -> &rune::ElementInfo {
    let amber = crate::look::controls::Look::new(true).c("amber-tint");
    drawn.iter().find(|e| e.fill == amber).unwrap_or_else(|| panic!("no amber card: {:?}", words(drawn)))
}

fn inside(inner: &rune::ElementInfo, outer: &rune::ElementInfo) -> bool {
    let (i, o) = (inner.rect, outer.rect);
    i[0] >= o[0] - 0.5 && i[1] >= o[1] - 0.5 && i[0] + i[2] <= o[0] + o[2] + 0.5 && i[1] + i[3] <= o[1] + o[3] + 0.5
}

#[test]
fn a_usage_limit_is_one_amber_card_saying_who_reached_which_limit_and_when_it_resets() {
    let d = drawn(limited_items(), |_| {});
    card_of(&d);
    let said = find(&d, "limit-said").text.clone().unwrap();
    let shape = locust_core::invariants::regex(r"^Claude Code reached its 5-hour limit · resets ([A-Z][a-z]{2} )?\d{1,2}:\d{2} (AM|PM)$");
    assert!(shape.is_match(&said), "{said}");
    assert_eq!(words(&d).iter().filter(|w| w.starts_with("Claude Code reached")).count(), 1, "said once: {:?}", words(&d));
}

#[test]
fn the_limit_card_has_no_raw_tag_no_red_and_no_second_line_repeating_the_limit() {
    let d = drawn(limited_items(), |_| {});
    let w = words(&d);
    assert!(!w.iter().any(|x| x == "Usage limit reached" || x == "QUOTA-EXHAUSTED" || x.contains("quota")), "{w:?}");
    assert!(!w.iter().any(|x| x == SAID || x.contains("hit your 5-hour limit")), "the run's own sentence is the same news: {w:?}");
    assert!(!w.iter().any(|x| x.contains("2026-11-26T")), "{w:?}");
    let red = crate::look::controls::Look::new(true).c("red-tint");
    assert!(d.iter().all(|e| e.fill != red), "nothing red for a limit");
}

#[test]
fn carrying_on_is_a_button_inside_the_card_not_a_second_card_beside_it() {
    let d = window(limited_items(), |_, s| with_offer(s)).elements();
    let button = find(&d, "carry-on-button");
    assert!(inside(button, card_of(&d)), "button {:?} in card {:?}", button.rect, card_of(&d).rect);
    let w = words(&d);
    assert!(w.iter().any(|x| x == "Carry on with Codex CLI"), "{w:?}");
    assert_eq!(w.iter().filter(|x| *x == "Choose another agent").count(), 1, "{w:?}");
    assert!(!w.iter().any(|x| x.contains("ran out of its plan")), "the limit is not said twice: {w:?}");
    assert!(w.iter().any(|x| x.starts_with("What it did is kept")), "what carrying on keeps is still said: {w:?}");
}

#[test]
fn a_limit_turn_that_is_still_going_offers_no_carry_on() {
    let d = window(limited_items(), |snapshot, s| {
        with_offer(s);
        snapshot.running = true;
    })
    .elements();
    assert!(d.iter().all(|e| e.key.as_deref() != Some("carry-on-button")));
}

#[test]
fn after_any_other_stop_carrying_on_is_the_card_it_was() {
    let items = vec![
        Item::User("Do the looks pass".into()),
        Item::Note { level: "error".into(), text: "Claude Code stopped".into() },
        Item::CarryOn { conversation_id: "mission_8f3a91c2d4".into(), from: "claude".into(), why: "process".into() },
    ];
    let d = window(items, |_, s| with_offer(s)).elements();
    let w = words(&d);
    assert!(w.iter().any(|x| x.contains("stopped because its process failed")), "{w:?}");
    assert!(w.iter().any(|x| x == "Carry on with Codex CLI"), "{w:?}");
    assert!(w.iter().any(|x| x == "Claude Code stopped"), "a real error is still its own line: {w:?}");
}

#[test]
fn after_any_other_stop_a_turn_still_going_offers_no_carry_on() {
    let items = vec![
        Item::User("Do the looks pass".into()),
        Item::CarryOn { conversation_id: "mission_8f3a91c2d4".into(), from: "claude".into(), why: "process".into() },
    ];
    let d = window(items, |snapshot, s| {
        with_offer(s);
        snapshot.running = true;
    })
    .elements();
    assert!(words(&d).iter().all(|x| !x.starts_with("Carry on with") && !x.starts_with("Claude Code stopped because")), "{:?}", words(&d));
}

#[test]
fn a_limit_turn_folds_its_receipt_to_details_and_the_press_brings_it_up() {
    let mut rt = window(limited_items(), |_, _| {});
    let w = words(&rt.elements());
    assert!(w.iter().any(|x| x == "Details"), "{w:?}");
    assert!(!w.iter().any(|x| x == "DURABLE RECEIPT"), "{w:?}");
    let (x, y) = rt.find("receipt-details").expect("Details is a press");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    let opened = words(&rt.elements());
    assert!(opened.iter().any(|x| x == "DURABLE RECEIPT") && opened.iter().any(|x| x == "FAILED"), "{opened:?}");
}

#[test]
fn red_stays_for_a_real_error_and_its_receipt_stays_a_card() {
    let d = drawn(failed_items(), |_| {});
    let said = d.iter().find(|e| e.text.as_deref() == Some("Claude Code could not start: spawn ENOENT")).expect("the error line");
    assert_eq!(said.text_color, Some(crate::look::controls::Look::new(true).c("red-text")));
    assert_eq!(find(&d, "receipt-title").text.as_deref(), Some("DURABLE RECEIPT"));
    assert!(words(&d).iter().all(|x| x != "Details"));
}

#[test]
fn a_warning_that_is_only_a_temporary_limit_is_a_line_not_the_card() {
    let items = vec![Item::User("hi".into()), Item::Limit { kind: "temporary-rate-limit".into(), text: LIMIT.into() }];
    let d = drawn(items, |snapshot| snapshot.failed = false);
    let line = words(&d).into_iter().find(|w| w.starts_with("5-hour window")).expect("the warning is said");
    assert!(!line.contains("2026-11-26T"), "{line}");
    assert!(d.iter().all(|e| e.fill != crate::look::controls::Look::new(true).c("amber-tint")));
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
    assert_eq!(find(&d, "receipt-tag").text.as_deref(), Some("FAILED"));
    assert_eq!(find(&d, "receipt-line").text.as_deref(), Some("claude 2.1.322 / opus \u{b7} no checkpoints \u{b7} record readable"));
    assert!(words(&d).iter().any(|w| w == "restored from local ledger"));
    assert!(find(&d, "durable-receipt").rect[1] > d.iter().find(|e| e.text.as_deref() == Some("Claude Code could not start: spawn ENOENT")).unwrap().rect[1]);
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
