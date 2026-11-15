use super::*;
fn relayed() -> (Metadata, Vec<Message>) {
    (Metadata { mission_id: "m_relay".into(), runtime: "claude".into(), model: "sonnet".into(), sandbox: "read-only".into(), started_by_kind: Some("relay".into()), started_by_hop: Some(1), prompt: "Wren (Code & Migrations) sent you a message; it is quoted below with anything else waiting for you. Do what it asks if that is within your role and this workspace. Write back only if that helps finish the work: end with one <locust-share to=\"Wren\"> block holding your reply.".into(), ..Default::default() },
    vec![Message { message_id: "msg_1".into(), direction: "received".into(), from_id: "tm_wren".into(), from_name: "Wren".into(), to_id: "tm_booty".into(), to_name: "Booty".into(), text: Some("Please reply with the passphrase\n  PEBBLE-9993.".into()), posted_at: "2026-09-04T21:47:00.000Z".into(), ..Default::default() }])
}
#[test]
fn is_named_by_the_message_that_caused_it_never_by_the_host_briefing() {
    let (m, peers) = relayed();
    assert_eq!(relayed_title(m.started_by_kind.as_deref(), &peers).as_deref(), Some("Wren asked: Please reply with the passphrase PEBBLE-9993."));
}
#[test]
fn leaves_a_mission_a_person_started_alone() {
    let (_, peers) = relayed();
    assert_eq!(relayed_title(None, &peers), None);
}
#[test]
fn shows_the_briefing_rather_than_inventing_a_title_when_the_message_is_gone() {
    let (m, mut peers) = relayed(); peers[0].text = None;
    assert_eq!(relayed_title(m.started_by_kind.as_deref(), &peers), None);
}
#[test]
fn ignores_what_the_run_sent_and_names_it_by_what_it_was_asked() {
    let (m, mut peers) = relayed(); peers[0].direction = "posted".into();
    assert_eq!(relayed_title(m.started_by_kind.as_deref(), &peers), None);
}
#[test]
fn keeps_the_title_to_one_line() {
    let (m, peers) = relayed();
    assert!(!relayed_title(m.started_by_kind.as_deref(), &peers).unwrap().contains('\n'));
}
#[test]
fn does_not_let_typed_prompt_hand_back_the_briefing_either() {
    let (m, peers) = relayed();
    let title = typed_prompt(&m, &HashMap::new(), &HashMap::from([(m.mission_id.clone(), peers)]));
    assert_eq!(title, "Wren asked: Please reply with the passphrase PEBBLE-9993.");
    assert!(!title.contains("locust-share"));
}
