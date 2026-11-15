use crate::{ledger::{Transcript, Item}, peer_thread::{Message, thread_peer_cards}};
use serde_json::json;
#[test]
fn hides_a_share_block_from_the_agent_bubble_keeping_the_prose() {
    let mut t = Transcript::default();
    t.apply(vec![json!({"recordType":"mission.event","event":{
        "id":"e1","runId":"run_1","missionId":"mission_1","sequence":1,
        "type":"message.delta","occurredAt":"2026-09-01T15:00:00.000Z","sourceAdapter":"codex",
        "payload":{"itemId":"answer","operation":"replace","final":true,
        "text":"The gate is pnpm check.\n\n<locust-share to=\"Atlas\">\npnpm check runs everything.\n</locust-share>"}
    }})]);
    let replies = t.items.iter().filter_map(|item| match item {
        Item::Assistant { item_id, text, r#final } => Some((item_id.as_str(), crate::protocol_tags::shown_text(text), *r#final)),
        _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(replies, [("answer", "The gate is pnpm check.".into(), true)]);
}
#[test]
fn groups_an_exchange_by_the_other_party_and_marks_whether_anything_was_received() {
    let messages = vec![
        Message { message_id:"wm_2".into(), direction:"posted".into(), from_id:"tm_wren".into(), from_name:"Wren".into(), to_id:"tm_atlas".into(), to_name:"Atlas".into(), text:Some("Noted as a claim.".into()), posted_at:"2026-09-01T15:05:00.000Z".into(), ..Default::default() },
        Message { message_id:"wm_1".into(), direction:"received".into(), from_id:"tm_atlas".into(), from_name:"Atlas".into(), to_id:"tm_wren".into(), to_name:"Wren".into(), text:Some("Dispute events changed shape.".into()), posted_at:"2026-09-01T15:00:00.000Z".into(), ..Default::default() },
        Message { message_id:"wm_3".into(), direction:"posted".into(), from_id:"tm_wren".into(), from_name:"Wren".into(), to_id:"tm_nova".into(), to_name:"Nova".into(), text:Some("Docs are stale.".into()), posted_at:"2026-09-01T15:06:00.000Z".into(), ..Default::default() },
    ];
    let groups = thread_peer_cards(&[messages]);
    let names = groups.iter().map(|g| {let m=&g.messages[0]; if m.direction=="received" {m.from_name.as_str()} else {m.to_name.as_str()}}).collect::<Vec<_>>();
    assert_eq!(names, ["Atlas", "Nova"]);
    assert_eq!(groups[0].messages.iter().map(|m|m.message_id.as_str()).collect::<Vec<_>>(), ["wm_1", "wm_2"]);
    assert!(groups[0].before_work);
    assert!(!groups[1].before_work);
}
