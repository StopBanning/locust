//! Turn-scoped peer cards, in Electron missionView's chronological placement.
#[derive(Clone, Debug, Default)]
pub struct Message {
    pub message_id: String,
    pub direction: String,
    pub from_id: String,
    pub from_name: String,
    pub from_mission: Option<String>,
    pub to_id: String,
    pub to_name: String,
    pub text: Option<String>,
    pub posted_at: String,
}
#[derive(Debug)]
pub struct Card {
    pub key: String,
    pub turn_index: usize,
    pub before_work: bool,
    pub messages: Vec<Message>,
}
pub fn thread_peer_cards(turns: &[Vec<Message>]) -> Vec<Card> {
    let mut cards = Vec::new();
    for (turn_index, messages) in turns.iter().enumerate() {
        let mut groups: Vec<(String, String, Vec<Message>)> = Vec::new();
        for message in messages {
            let (id, name) = if message.direction == "received" { (&message.from_id, &message.from_name) } else { (&message.to_id, &message.to_name) };
            let identity = if id.is_empty() {format!("name:{name}")} else {id.clone()};
            if let Some(group) = groups.iter_mut().find(|g| g.0 == identity) { group.2.push(message.clone()); }
            else { groups.push((identity, if id.is_empty() {name.clone()} else {id.clone()}, vec![message.clone()])); }
        }
        for (_, name, mut messages) in groups {
            messages.sort_by(|a,b| a.posted_at.cmp(&b.posted_at));
            cards.push(Card { key: format!("peer_{turn_index}_{name}"), turn_index,
                before_work: messages.iter().any(|m| m.direction == "received"), messages });
        }
    }
    cards
}
#[cfg(test)] #[path="peer_thread_tests.rs"] mod tests;
