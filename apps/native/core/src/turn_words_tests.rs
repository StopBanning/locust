//! The four which-words-a-turn-shows cases reuse the existing handoff walk.
use crate::{conversation_turns::typed_prompt, ledger::Metadata};
use std::collections::HashMap;
fn mission(id:&str,prompt:&str,parent:Option<(&str,&str)>)->Metadata {
    Metadata { mission_id:id.into(),prompt:prompt.into(),runtime:"codex".into(),model:"account-default".into(),parent:parent.map(|(id,_)|id.into()),continues_reason:parent.map(|(_,reason)|reason.into()),..Default::default() }
}
fn index(missions:Vec<Metadata>)->HashMap<String,Metadata>{missions.into_iter().map(|m|(m.mission_id.clone(),m)).collect()}
#[test]
fn shows_a_reply_the_words_that_were_typed_for_it_not_the_opening_line(){
    let first=mission("m1","Audit the config",None);
    let reply=mission("m2","Now fix the two you found",Some(("m1","follow-up")));
    assert_eq!(typed_prompt(&reply,&index(vec![first,reply.clone()])),"Now fix the two you found");
}
#[test]
fn shows_a_handed_over_mission_the_words_a_person_typed_not_the_briefing_written_for_it(){
    let first=mission("m1","Audit the config",None);
    let handed=mission("m2","You are continuing a mission…",Some(("m1","route-switch")));
    assert_eq!(typed_prompt(&handed,&index(vec![first,handed.clone()])),"Audit the config");
}
#[test]
fn reaches_back_through_a_handoff_but_stops_at_the_reply_above_it(){
    let a=mission("m1","Audit the config",None);
    let b=mission("m2","You are continuing a mission…",Some(("m1","route-switch")));
    let c=mission("m3","Now fix the two you found",Some(("m2","follow-up")));
    let by=index(vec![a,b.clone(),c.clone()]);
    assert_eq!(typed_prompt(&c,&by),"Now fix the two you found");
    assert_eq!(typed_prompt(&b,&by),"Audit the config");
}
#[test]
fn keeps_the_mission_own_words_when_the_one_it_continues_is_gone(){
    let orphan=mission("m2","You are continuing a mission…",Some(("m_missing","route-switch")));
    assert_eq!(typed_prompt(&orphan,&index(vec![orphan.clone()])),"You are continuing a mission…");
}
