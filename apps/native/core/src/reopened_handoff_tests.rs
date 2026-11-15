use super::*;
fn first() -> Value { let mut m=created("mission_1", "Inspect the workspace."); m["metadata"]["runtime"]=json!("codex"); m["metadata"]["createdAt"]=json!("2026-09-01T15:00:00.000Z"); m }
fn second() -> Value { let mut m=created("mission_2", "You are continuing work that another agent (Codex) started..."); m["metadata"]["runtime"]=json!("claude"); m["metadata"]["createdAt"]=json!("2026-09-01T15:02:00.000Z"); m["metadata"]["continuesFrom"]=json!({"missionId":"mission_1","checkpointEpoch":1,"reason":"route-switch"}); m }
fn checkpoint() -> Value { json!({"schemaVersion":27,"recordType":"mission.checkpoint","checkpoint":{"epoch":1,"reason":"route-switch","resumeSafety":"approval-required","safetyReason":"one action never reported back","createdAt":"2026-09-01T15:01:00.000Z","unsettledActions":[{"itemId":"tool_1","name":"shell"}]}}) }
fn pair(second: Value) -> (Temp, Snapshot) {
    let temp=Temp::new(); write(&temp.path("mission_1"), &[first(),checkpoint()]); write(&temp.path("mission_2"), &[second]);
    let snap=Session::new(temp.0.clone(),"mission_2".into()).poll().unwrap(); (temp,snap)
}
#[test]
fn shows_the_words_the_person_typed_not_the_briefing_the_host_wrote() {
    let (_temp,snap)=pair(second());
    assert_eq!(snap.items.iter().filter_map(|i|if let Item::User(s)=i{Some(s.as_str())}else{None}).collect::<Vec<_>>(), ["Inspect the workspace."]);
    let t=synthetic_transcript(vec![first()]); assert!(matches!(&t.items[0],Item::User(s) if s=="Inspect the workspace."));
}
#[test]
fn rebuilds_the_divider_from_the_route_switch_checkpoint_it_resumed_from() {
    let (_temp,snap)=pair(second());
    let (from,to)=snap.items.iter().find_map(|i|if let Item::Handoff{from,to}=i{Some((from,to))}else{None}).expect("reopened seam");
    let by=HashMap::from([(from.mission_id.clone(), *from.clone())]);
    let seam=crate::conversation_turns::stitched_handoff(to,&by).unwrap();
    assert_eq!((seam.from.as_str(),seam.to.as_str(),seam.unsettled_count,seam.omitted_briefing), ("codex","claude",1,vec![]));
    assert_eq!(crate::conversation_turns::stitched_handoff(from,&by),None);
}
#[test]
fn says_what_the_brief_left_out_as_recorded_with_the_switch() {
    let mut m=second();m["metadata"]["continuesFrom"]["leftOut"]=json!(["summary","earlier"]);
    let (_temp,snap)=pair(m);
    let (from,to)=snap.items.iter().find_map(|i|if let Item::Handoff{from,to}=i{Some((from,to))}else{None}).unwrap();
    assert_eq!(crate::conversation_turns::stitched_handoff(to,&HashMap::from([(from.mission_id.clone(),*from.clone())])).unwrap().omitted_briefing,["summary","earlier"]);
}
#[test]
fn stops_walking_a_chain_whose_earlier_mission_is_missing_and_a_cyclic_one() {
    let temp=Temp::new();let mut orphan=second();orphan["metadata"]["missionId"]=json!("mission_3");orphan["metadata"]["continuesFrom"]["missionId"]=json!("mission_gone");write(&temp.path("mission_3"),&[orphan]);
    let snap=Session::new(temp.0.clone(),"mission_3".into()).poll().unwrap();assert_eq!(snap.turn_metas[0].mission_id,"mission_3");assert!(!snap.items.iter().any(|i|matches!(i,Item::Handoff{..})));
    let mut a=first();a["metadata"]["missionId"]=json!("a");a["metadata"]["continuesFrom"]=json!({"missionId":"b","reason":"route-switch","checkpointEpoch":1});
    let mut b=second();b["metadata"]["missionId"]=json!("b");b["metadata"]["continuesFrom"]["missionId"]=json!("a");write(&temp.path("a"),&[a]);write(&temp.path("b"),&[b]);
    assert!(Session::new(temp.0.clone(),"a".into()).poll().is_ok());
}
