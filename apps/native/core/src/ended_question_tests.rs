use super::*;
use crate::ledger::Transcript;
use serde_json::json;
const ASK:&str="Here is what I found.\n\n<locust-ask>\nKeep the two callers on v2, or migrate them now?\n- Keep them on v2 :: Smaller change\n- Migrate all callers now :: Touches 4 more files\n</locust-ask>";
fn finished(texts:&[&str])->Vec<Item>{let mut t=Transcript::default();t.apply(texts.iter().enumerate().map(|(i,text)|json!({"recordType":"mission.event","event":{"type":"message.delta","sequence":i+1,"payload":{"itemId":if i==0 {"a"} else {"b"},"text":text,"operation":"append","final":true}}})).collect());t.items}
#[test] fn becomes_a_card_on_the_turn_a_person_can_answer(){assert_eq!(ended_question(&finished(&[ASK]),false,true).unwrap().options.len(),2);}
#[test] fn is_not_offered_on_an_earlier_turn_where_the_answer_already_exists(){assert!(ended_question(&finished(&[ASK]),false,false).is_none());}
#[test] fn is_not_offered_while_the_run_is_still_going(){assert!(ended_question(&finished(&[ASK]),true,true).is_none());}
#[test] fn is_taken_out_of_the_reply_bubble_so_it_is_never_asked_twice(){let items=finished(&[ASK]);let text=items.iter().find_map(|i|if let Item::Assistant{text,..}=i{Some(text.as_str())}else{None}).unwrap();assert_eq!(crate::protocol_tags::shown_text(text),"Here is what I found.");}
#[test] fn leaves_an_ordinary_finished_run_with_no_card(){assert!(ended_question(&finished(&["Done, two files changed."]),false,true).is_none());}
#[test] fn reads_the_last_final_message_not_an_earlier_one(){assert!(ended_question(&finished(&["First pass done.",ASK]),false,true).is_some());}

// Review (14 Nov): only a provider-final message can end on a question; a streamed draft is not the answer.
#[test] fn a_draft_still_streaming_is_not_read_as_the_turns_question(){
    let mut t=Transcript::default();
    t.apply(vec![json!({"recordType":"mission.event","event":{"type":"message.delta","sequence":1,"payload":{"itemId":"a","text":ASK,"operation":"append","final":false}}})]);
    assert!(ended_question(&t.items,false,true).is_none());
}
