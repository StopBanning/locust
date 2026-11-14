use super::*;
fn message(direction:&str, who:&str, text:&str, at:&str)->Message {Message {
    message_id:format!("m_{}_{}",text.chars().take(6).collect::<String>(),direction), direction:direction.into(),
    from_id:if direction=="posted" {"booty"}else{"wren"}.into(), from_name:if direction=="posted" {"Booty"}else{"Wren"}.into(),
    to_id:if direction=="posted" {"wren"}else{"booty"}.into(), to_name:if direction=="posted" {who}else{"Booty"}.into(),
    text:Some(text.into()), posted_at:at.into(), ..Default::default()}}
fn turns()->Vec<Vec<Message>> {vec![vec![message("posted","Wren","Write me a soliloquy.","2026-09-04T21:47:00.000Z")],vec![message("received","Booty","O silent hall of half-built code...","2026-09-04T21:48:00.000Z")]]}
#[test] fn draws_the_message_an_earlier_turn_sent_not_only_the_reply(){let cards=thread_peer_cards(&turns());assert_eq!(cards.iter().flat_map(|c| &c.messages).filter(|m|m.direction=="posted").map(|m|m.text.as_deref().unwrap()).collect::<Vec<_>>(),["Write me a soliloquy."]);}
#[test] fn keeps_every_turn_of_a_long_conversation_not_just_the_last_two(){let many=(0..6).map(|i|vec![message("posted","Wren",&format!("ask {i}"),&format!("2026-09-04T21:{}:00.000Z",40+i))]).collect::<Vec<_>>();assert_eq!(thread_peer_cards(&many).iter().flat_map(|c| &c.messages).map(|m|m.text.clone().unwrap()).collect::<Vec<_>>(),["ask 0","ask 1","ask 2","ask 3","ask 4","ask 5"]);}
#[test] fn files_each_card_against_the_turn_it_happened_on(){assert_eq!(thread_peer_cards(&turns()).iter().map(|c|c.turn_index).collect::<Vec<_>>(),[0,1]);}
#[test] fn puts_what_a_turn_sent_after_its_work_and_what_it_was_handed_before_it(){let cards=thread_peer_cards(&turns());assert!(!cards[0].before_work);assert!(cards[1].before_work);}
#[test] fn gives_the_same_peer_a_distinct_key_on_each_turn(){let cards=thread_peer_cards(&turns());assert_eq!(cards.iter().map(|c|&c.key).collect::<std::collections::HashSet<_>>().len(),cards.len());}
#[test] fn opens_a_short_exchange_in_place(){assert!(exchange_starts_open(1));assert!(exchange_starts_open(2));}
#[test] fn leaves_a_long_back_and_forth_collapsed(){assert!(!exchange_starts_open(3));assert!(!exchange_starts_open(9));}
#[test] fn has_nothing_to_open_when_there_are_no_messages(){assert!(!exchange_starts_open(0));}
#[test] fn previews_what_a_collapsed_exchange_said(){assert_eq!(snippet(Some("Write me a soliloquy."),72).as_deref(),Some("Write me a soliloquy."));}
#[test] fn flattens_a_multi_line_message_to_one_line(){assert_eq!(snippet(Some("first line\n\n  second line"),72).as_deref(),Some("first line second line"));}
#[test] fn cuts_a_long_message_rather_than_letting_the_card_wrap(){let s=snippet(Some(&"x".repeat(200)),20).unwrap();assert_eq!(s.chars().count(),20);assert!(s.ends_with('…'));}
#[test] fn previews_nothing_for_a_message_the_workroom_no_longer_holds(){assert_eq!(snippet(None,72),None);assert_eq!(snippet(Some("   "),72),None);}

// Review (14 Nov).
#[test] fn an_exchange_reads_oldest_first_however_its_messages_arrived(){
    let turn=vec![message("posted","Wren","second","2026-09-04T21:49:00.000Z"),message("posted","Wren","first","2026-09-04T21:47:00.000Z")];
    assert_eq!(thread_peer_cards(&[turn])[0].messages.iter().map(|m|m.text.clone().unwrap()).collect::<Vec<_>>(),["first","second"]);
}
#[test] fn a_card_with_anything_it_was_handed_goes_before_the_work_even_when_it_also_sent(){
    let turn=vec![message("posted","Wren","ask","2026-09-04T21:47:00.000Z"),message("received","Booty","answer","2026-09-04T21:48:00.000Z")];
    let cards=thread_peer_cards(&[turn]);
    assert_eq!(cards.len(),1);
    assert!(cards[0].before_work);
}
#[test] fn two_peers_who_share_a_name_get_their_own_cards_by_id(){
    let mut a=message("posted","Wren","to one","2026-09-04T21:47:00.000Z");
    let mut b=message("posted","Wren","to the other","2026-09-04T21:48:00.000Z");
    a.to_id="wren_1".into(); b.to_id="wren_2".into();
    assert_eq!(thread_peer_cards(&[vec![a,b]]).len(),2);
}
