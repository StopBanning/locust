//! A FULL CONTEXT IS COMPACTED FROM THE RING (0.797): renderer/src/a-full-context-is-compacted-from-the-ring.test.tsx,
//! the parts that are decisions (the offer and its words); the ring and its menu are window tests.
use super::*;

fn lists(runtime: &str, names: &[&str]) -> BTreeMap<String, Vec<CommandInfo>> {
    let rows = names.iter().map(|n| CommandInfo { name: (*n).into(), description: String::new(), argument_hint: String::new() }).collect();
    BTreeMap::from([(runtime.to_owned(), rows)])
}

fn ready(commands: &BTreeMap<String, Vec<CommandInfo>>) -> OfferInput<'_> {
    OfferInput { runtime: Some("claude"), next_runtime: "claude", runtime_name: "Claude Code", commands, running: false, queued: false, elsewhere: false }
}

#[test]
fn compact_now_is_offered_for_a_conversation_whose_runtime_lists_compact() {
    let claude = lists("claude", &["compact"]);
    assert_eq!(compact_offer(&ready(&claude)), Some(CompactOffer::Ready));
    let codex = lists("codex", &["compact"]);
    let input = OfferInput { runtime: Some("codex"), next_runtime: "codex", commands: &codex, ..ready(&claude) };
    assert_eq!(compact_offer(&input), Some(CompactOffer::Ready));
    let opencode = lists("opencode", &["compact"]);
    let input = OfferInput { runtime: Some("opencode"), next_runtime: "opencode", commands: &opencode, ..ready(&claude) };
    assert_eq!(compact_offer(&input), Some(CompactOffer::Ready));
}

#[test]
fn compact_now_is_not_offered_where_the_runtime_lists_no_compact_or_the_message_would_go_elsewhere() {
    let init_only = lists("claude", &["init"]);
    assert_eq!(compact_offer(&ready(&init_only)), None);
    let none = BTreeMap::new();
    assert_eq!(compact_offer(&ready(&none)), None);
    let claude = lists("claude", &["compact"]);
    let cursor = OfferInput { runtime: Some("cursor"), next_runtime: "cursor", ..ready(&claude) };
    assert_eq!(compact_offer(&cursor), None);
    assert_eq!(compact_offer(&OfferInput { runtime: None, ..ready(&claude) }), None);
    // A comparison, a cloud or background turn, an earlier message being edited.
    assert_eq!(compact_offer(&OfferInput { elsewhere: true, ..ready(&claude) }), None);
    // Another runtime's list is not this one's.
    let other = lists("codex", &["compact"]);
    assert_eq!(compact_offer(&ready(&other)), None);
}

#[test]
fn compact_now_waits_in_line_behind_a_running_turn_as_a_message_sent_then_does() {
    let claude = lists("claude", &["compact"]);
    let offer = compact_offer(&OfferInput { running: true, ..ready(&claude) }).unwrap();
    assert_eq!(offer, CompactOffer::AfterTurn);
    assert_eq!(item_name(&offer), "Compact when this turn ends");
    assert_eq!(item_detail(&offer, "Claude Code"), after_turn("Claude Code"));
    assert_eq!(after_turn("Claude Code"), "Waits until Claude Code finishes this turn, then summarizes the conversation so far.");
}

#[test]
fn compact_now_says_why_it_cannot_with_a_message_already_waiting_or_another_model_picked() {
    let claude = lists("claude", &["compact"]);
    assert_eq!(
        compact_offer(&OfferInput { running: true, queued: true, ..ready(&claude) }),
        Some(CompactOffer::Refused("A message is already waiting for this turn to end. Compact after it goes.".into()))
    );
    assert_eq!(
        compact_offer(&OfferInput { next_runtime: "codex", ..ready(&claude) }),
        Some(CompactOffer::Refused("Your next message goes to another model. Pick Claude Code again to compact this conversation.".into()))
    );
    // A queued message with no turn running is nothing to wait behind.
    assert_eq!(compact_offer(&OfferInput { queued: true, ..ready(&claude) }), Some(CompactOffer::Ready));
}

#[test]
fn compact_now_says_what_it_does_under_its_name() {
    assert_eq!(item_name(&CompactOffer::Ready), "Compact now");
    assert_eq!(
        consequence("Claude Code"),
        "Summarize the conversation so far, so the turns ahead have room. Claude Code also does this on its own when the context is full."
    );
    assert_eq!(item_detail(&CompactOffer::Ready, "Claude Code"), consequence("Claude Code"));
    let why = "A message is already waiting for this turn to end. Compact after it goes.".to_owned();
    assert_eq!(item_detail(&CompactOffer::Refused(why.clone()), "Claude Code"), why);
    assert_eq!(item_name(&CompactOffer::Refused(why)), "Compact now");
}
