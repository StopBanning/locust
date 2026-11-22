use crate::*;
use crate::memory_card::{memories_of_conversation, memories_of_turn, turns_of_conversation};
use serde_json::{Value, json};
fn memories() -> Vec<Value> {
    vec![json!({"missionId":"turn1","by":{"name":"Wren"},"text":"learned first","status":"kept"}),
        json!({"missionId":"turn3","by":{"name":"Wren"},"text":"learned later","status":"kept"})]
}
fn rows() -> Vec<crate::conversation_list::SidebarMission> {
    vec![crate::conversation_list::SidebarMission { mission_id:"turn3".into(), title:"Thread".into(), phase:"completed".into(), last_at:None,
        root_id:Some("turn1".into()), member_ids:vec!["turn1".into(),"turn2".into(),"turn3".into()], owner_id:None, said:String::new(), nested_under:None, integrity_issue_count:0, needs_you:false, app_started:false }]
}
fn lines(shown: Option<&str>) -> Vec<crate::memory_card::MemoryCardLine> { memories_of_conversation(&memories(), &turns_of_conversation(shown, &rows())) }
#[test]
fn are_every_turn_of_it_whichever_turn_is_on_screen() {
    // case: are every turn of it, whichever turn is on screen
    for shown in ["turn1","turn2","turn3"] { assert_eq!(lines(Some(shown)).iter().map(|l|l.text.as_str()).collect::<Vec<_>>(), ["learned first","learned later"]); }
}
#[test]
fn never_leak_in_from_another_conversation() {
    // case: never leak in from another conversation
    let mut all=memories(); all.push(json!({"missionId":"other","text":"theirs"}));
    assert_eq!(memories_of_conversation(&all,&turns_of_conversation(Some("turn2"),&rows())).len(),2);
}
#[test]
fn are_the_shown_turn_alone_when_the_sidebar_has_not_caught_up_yet() {
    // case: are the shown turn alone when the sidebar has not caught up yet
    let turns=turns_of_conversation(Some("fresh"),&[]);
    assert_eq!(turns, ["fresh".to_owned()].into_iter().collect());
    assert_eq!(memories_of_conversation(&[json!({"missionId":"fresh","text":"just learned"})],&turns)[0].text,"just learned");
}
#[test]
fn are_none_at_all_when_nothing_is_shown() {
    // case: are none at all when nothing is shown
    assert!(turns_of_conversation(None,&rows()).is_empty()); assert!(lines(None).is_empty());
}
#[test]
fn ignore_a_memory_that_names_no_conversation() {
    // case: ignore a memory that names no conversation
    let mut all=memories(); all.push(json!({"text":"from nowhere"}));
    assert_eq!(memories_of_conversation(&all,&turns_of_conversation(Some("turn1"),&rows())).len(),2);
}
#[test]
fn keep_the_order_they_were_learned_in() {
    // case: keep the order they were learned in
    let mut all=memories();all.reverse();assert_eq!(memories_of_conversation(&all,&turns_of_conversation(Some("turn2"),&rows()))[0].text,"learned later");
}
#[test]
fn keeps_the_turn_on_the_line_so_the_thread_can_place_it() {
    // case: keeps the turn on the line, so the thread can place it
    assert_eq!(lines(Some("turn2")).iter().map(|l|l.mission_id.as_str()).collect::<Vec<_>>(),["turn1","turn3"]);
}
#[test]
fn gives_each_turn_only_its_own() {
    // case: gives each turn only its own
    let all=lines(Some("turn2"));assert_eq!(memories_of_turn(&all,Some("turn1"))[0].text,"learned first");assert_eq!(memories_of_turn(&all,Some("turn3"))[0].text,"learned later");
}
#[test]
fn gives_a_turn_that_taught_nothing_an_empty_card_rather_than_a_neighbour_list() {
    // case: gives a turn that taught nothing an empty card rather than a neighbour list
    assert!(memories_of_turn(&lines(Some("turn2")),Some("turn2")).is_empty());
}
#[test]
fn draws_nothing_at_all_when_there_is_no_turn_to_draw_under() {
    // case: draws nothing at all when there is no turn to draw under
    assert!(memories_of_turn(&lines(Some("turn1")),None).is_empty());
    assert!(crate::memory_card::view(&State::default(),None,&Palette::new(true)).is_none());
}
#[test]
fn the_thread_keeps_cards_under_their_turns_and_opens_their_saved_words() {
    let mut s=State::default();s.demo=true;
    s.people_panel.memories=json!({"memories":memories()});
    let header=Teammate {id:"tm_wren".into(),name:"Wren".into(),role:"Code".into(),runtime:None,model:None,account:None,busy:false,runs_in:"ask".into(),folder:None};
    let items=vec![ledger::Item::User("First prompt".into()),ledger::Item::Assistant{item_id:"a".into(),text:"First answer".into(),r#final:true},ledger::Item::User("Next prompt".into()),ledger::Item::Assistant{item_id:"b".into(),text:"Next answer".into(),r#final:true},ledger::Item::User("Last prompt".into())];
    s.ledger=Some(LedgerPane {cold_tip:None,header,conversation:Conversation{conversation_id:Some("turn1".into()),turns:vec![]},items:items.clone(),dir:Default::default(),worker:None,snapshot:ledger::Snapshot{turn_metas:["turn1","turn2","turn3"].into_iter().map(|id|ledger::Metadata{mission_id:id.into(),..Default::default()}).collect(),tip:"turn3".into(),..Default::default()},due:0.0,loading:false,after_tip:None,after_tip_deadline:None,backoff:0.0});
    let mut rt=rune::Runtime::new(s,|s,_|message_column(&s.ledger.as_ref().unwrap().items,"Wren",&Palette::new(true),s),(800.0,3000.0),rune::Env::new(true,1.0));rt.frame(0.0);rt.frame(0.016);
    let (_, first)=rt.find("turn-memory-turn1").unwrap();let (_,last)=rt.find("turn-memory-turn3").unwrap();assert!(first<last);assert!(rt.find("turn-memory-turn2").is_none());
    let next=rt.elements().into_iter().find(|e|e.text.as_deref()==Some("Next prompt")).unwrap();assert!(first<next.rect[1]);
    assert!(!rt.elements().iter().any(|e|e.text.as_deref()==Some("learned first")));
    let button=rt.elements().into_iter().find(|e|e.role=="button" && e.label.as_deref()==Some("Wren remembered 1 thing") && e.rect[1]<next.rect[1]).unwrap();
    rt.pointer_down(button.rect[0]+10.0,button.rect[1]+10.0);rt.pointer_up(button.rect[0]+10.0,button.rect[1]+10.0);rt.frame(0.032);rt.frame(1.0);
    assert!(rt.elements().iter().any(|e|e.text.as_deref()==Some("learned first")));
    assert!(!rt.elements().iter().any(|e|e.text.as_deref()==Some("learned later")));
}
