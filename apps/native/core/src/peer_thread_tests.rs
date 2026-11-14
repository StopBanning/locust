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
