use super::*;
fn turn(id:&str,prompt:&str,parent:Option<(&str,&str)>)->Metadata{Metadata{mission_id:id.into(),prompt:prompt.into(),runtime:"claude".into(),model:"sonnet".into(),created_at:"2026-09-02T10:00:00.000Z".into(),parent:parent.map(|p|p.0.into()),continues_reason:parent.map(|p|p.1.into()),..Default::default()}}
fn ordinary()->HashMap<String,Metadata>{[turn("m1","check the google stock price",None),turn("m2","cant you look it up for me?",Some(("m1","follow-up"))),turn("m3","what about yesterday?",Some(("m2","follow-up")))].into_iter().map(|m|(m.mission_id.clone(),m)).collect()}
fn ids(m:&Metadata,by:&HashMap<String,Metadata>)->Vec<String>{conversation_turns(m,by).into_iter().map(|t|t.mission_id).collect()}
#[test]fn walks_a_reply_back_to_every_earlier_turn_oldest_first(){let by=ordinary();assert_eq!(conversation_turns(&by["m3"],&by).iter().map(|t|t.prompt.as_str()).collect::<Vec<_>>(),["check the google stock price","cant you look it up for me?","what about yesterday?"]);}
#[test]fn is_just_itself_for_a_first_turn(){let by=ordinary();assert_eq!(ids(&by["m1"],&by),["m1"]);}
#[test]fn does_not_walk_a_route_switch_which_is_a_handoff_and_keeps_its_divider(){let mut by=ordinary();let handed=turn("m4","briefing text",Some(("m1","route-switch")));by.insert("m4".into(),handed.clone());assert_eq!(ids(&handed,&by),["m4"]);}
#[test]fn stops_at_a_missing_or_cyclic_link_rather_than_spinning(){let orphan=turn("m9","reply",Some(("gone","follow-up")));assert_eq!(ids(&orphan,&HashMap::from([("m9".into(),orphan.clone())])),["m9"]);let a=turn("a","a",Some(("b","follow-up")));let b=turn("b","b",Some(("a","follow-up")));assert!(ids(&a,&HashMap::from([("a".into(),a.clone()),("b".into(),b)])).len()<=2);}
#[test]fn draws_no_handoff_divider_across_an_ordinary_reply(){let by=ordinary();assert_eq!(stitched_handoff(&by["m2"],&by),None);}
fn switched()->HashMap<String,Metadata>{let mut a=turn("s1","Remember the word marigold. Reply with just OK.",None);a.runtime="codex".into();let mut b=turn("s2","You are continuing a conversation another agent (Codex) started.\n\nThe person now asks:\n\nWhat was the word?",Some(("s1","route-switch")));b.runtime="opencode".into();b.created_at="2026-09-24T04:13:00.000Z".into();let mut c=turn("s3","And spell it backwards.",Some(("s2","follow-up")));c.runtime="opencode".into();[a,b,c].into_iter().map(|m|(m.mission_id.clone(),m)).collect()}
#[test]fn walks_a_reply_sent_to_another_runtime_back_to_the_turns_before_it_and_marks_the_seam(){let by=switched();let turns=conversation_turns(&by["s2"],&by);assert_eq!(ids(&by["s2"],&by),["s1","s2"]);assert_eq!(turns[0].switched_from,None);let seam=Switch{from:"codex".into(),to:"opencode".into(),unsettled_count:0,omitted_briefing:vec![]};assert_eq!(turns[1].switched_from,Some(seam.clone()));assert_eq!(switch_of(&by["s2"],&by),Some(seam));assert_eq!(typed_prompt(&by["s2"],&by),"What was the word?");assert_eq!(stitched_handoff(&by["s2"],&by),None);}
#[test]fn keeps_the_seam_on_the_switched_turn_once_the_conversation_moves_on(){let by=switched();let turns=conversation_turns(&by["s3"],&by);assert_eq!(ids(&by["s3"],&by),["s1","s2","s3"]);assert_eq!(turns.iter().map(|t|t.switched_from.as_ref().map(|s|s.to.as_str())).collect::<Vec<_>>(),[None,Some("opencode"),None]);}
#[test]fn keeps_the_turns_before_a_running_mission_that_was_handed_over(){let mut by=ordinary();let handed=turn("m5","briefing text",Some(("m2","route-switch")));by.insert("m5".into(),handed.clone());assert_eq!(ids(&handed,&by),["m1","m5"]);assert!(stitched_handoff(&handed,&by).is_some());assert_eq!(switch_of(&handed,&by),None);}

// a-tagged-conversation-shows-the-message.test.ts
#[test]
fn a_tagged_conversation_is_titled_by_the_message() {
    let m = Metadata { prompt: "Can you double-check the tax rate?\n\n(You were tagged in Wren's conversation. Wren's latest answer there, for context:)\n\nI changed TAX_RATE.".into(), ..Default::default() };
    assert_eq!(typed_prompt(&m, &HashMap::new()), "Can you double-check the tax rate?");
    assert_eq!(crate::relay_title::typed_prompt(&m, &HashMap::new(), &HashMap::new()), "Can you double-check the tax rate?");
}

#[test]
fn is_shown_as_the_steps_own_words_not_the_answer_and_rule_it_was_given() {
    let prompt = crate::hand_off::hand_off_prompt("Check the plan.", Some(crate::hand_off::From {name:"Atlas", answer:Some("Plan: remove the second sum.")}), true, None);
    let m=Metadata {mission_id:"mission_3".into(),prompt,started_by_kind:Some("routine".into()),..Default::default()};
    assert_eq!(typed_prompt(&m,&HashMap::new()),"Check the plan.");
}
#[test]
fn are_the_step_not_the_answer_quoted_above_it() {
    let prompt=crate::hand_off::hand_off_prompt("Write a plan to fix it.",Some(crate::hand_off::From{name:"Wren",answer:Some("The total is summed twice.")}),false,None);
    let mut m=Metadata{prompt:prompt.clone(),started_by_kind:Some("routine".into()),..Default::default()};
    assert_eq!(shown_prompt(&m),"Write a plan to fix it.");
    assert_eq!(crate::relay_title::typed_prompt(&m,&HashMap::new(),&HashMap::new()),"Write a plan to fix it.");
    let mut transcript=crate::ledger::Transcript::default();
    transcript.apply(vec![serde_json::json!({"schemaVersion":27,"recordType":"mission.created","metadata":{
        "missionId":"mission_2","prompt":prompt,"startedBy":{"kind":"routine","routineId":"rt_1","step":2}
    }})]);
    assert!(matches!(&transcript.items[0],crate::ledger::Item::User(words) if words == "Write a plan to fix it."));
    m.started_by_kind=None; assert_eq!(shown_prompt(&m),prompt);
}
