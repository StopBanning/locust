//! The two moments a run touches the workroom (Electron `main/peer-exchange.ts`). Before: what waits for the teammate
//! is quoted into its brief (`people::turn_brief`), and marked delivered only once the run is live, so a start that
//! fails never consumes a message. After a run that completed on its own terms: its reply is read for share blocks,
//! each checked against the roster and bounded, and each becomes one attributed workroom message. A cancelled or failed
//! run shares nothing.

use crate::briefing::PeerEntry;
use crate::peer_share::{MAX_SHARES_PER_MISSION, bounded_share_text, parse_share_blocks};
use crate::workroom::{Message, Party, Workroom};

#[derive(Debug, PartialEq, Eq)]
pub enum Recipient<'a> {
    Teammate(&'a PeerEntry),
    /// Addressed home, by its own name or role: nobody lost a message.
    Itself,
    Nobody,
}

/// Who a block's `to=` names: a name first, then a role, never a guess between two of the same name.
pub fn recipient_of<'a>(to: &str, me: &PeerEntry, others: &'a [PeerEntry]) -> Recipient<'a> {
    let wanted = to.trim().to_lowercase();
    if wanted.is_empty() {
        return Recipient::Nobody;
    }
    let by_name: Vec<&PeerEntry> = others.iter().filter(|e| e.name.to_lowercase() == wanted).collect();
    if by_name.len() == 1 {
        return Recipient::Teammate(by_name[0]);
    }
    if me.name.to_lowercase() == wanted {
        return Recipient::Itself;
    }
    if by_name.len() > 1 {
        return Recipient::Nobody;
    }
    let by_role: Vec<&PeerEntry> = others.iter().filter(|e| e.role.to_lowercase() == wanted).collect();
    if by_role.len() == 1 {
        return Recipient::Teammate(by_role[0]);
    }
    if by_role.is_empty() && me.role.to_lowercase() == wanted {
        return Recipient::Itself;
    }
    Recipient::Nobody
}

/// Post each share block of a finished reply. Returns what was posted and what the person should be told: a block
/// for someone not on the roster, more blocks than a run may send, or one the workroom would not take.
pub fn share_reply(workroom: &Workroom, me: &PeerEntry, others: &[PeerEntry], mission_id: &str, reply: &str, observed: Option<&[String]>) -> (Vec<Message>, Vec<String>) {
    let blocks = parse_share_blocks(reply);
    let mut said = Vec::new();
    let mut posted: Vec<Message> = Vec::new();
    if blocks.len() > MAX_SHARES_PER_MISSION {
        said.push(format!("{} tried to share {} messages; only the first {MAX_SHARES_PER_MISSION} were sent.", me.name, blocks.len()));
    }
    let from = Party { teammate_id: me.teammate_id.clone(), name: me.name.clone() };
    for block in blocks.iter().take(MAX_SHARES_PER_MISSION) {
        let target = match recipient_of(&block.to, me, others) {
            Recipient::Teammate(t) => t,
            Recipient::Itself => continue,
            Recipient::Nobody => {
                said.push(format!("{} addressed a message to \"{}\", who is not on the roster. Nothing was sent.", me.name, block.to));
                continue;
            }
        };
        let text = bounded_share_text(&block.text);
        if text.is_empty() {
            continue;
        }
        let to = Party { teammate_id: target.teammate_id.clone(), name: target.name.clone() };
        match workroom.post(&from, mission_id, &to, &text, observed) {
            // The same block twice in one reply is one message (A2.2).
            Ok(message) if posted.iter().any(|p| p.message_id == message.message_id) => {}
            Ok(message) => posted.push(message),
            Err(_) => said.push(format!("A message from {} to {} could not be written to the workroom. Nothing was sent.", me.name, target.name)),
        }
    }
    (posted, said)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, role: &str) -> PeerEntry {
        PeerEntry { teammate_id: id.into(), name: name.into(), role: role.into() }
    }

    #[test]
    fn a_share_reaches_a_teammate_by_name_or_role_and_never_one_of_two_with_the_same_name() {
        let me = entry("tm_j", "Jimothy", "Finance Bro");
        let others = vec![entry("tm_w", "Wembley", "Docs"), entry("tm_g1", "Gem", "Code"), entry("tm_g2", "Gem", "Review")];
        assert_eq!(recipient_of("wembley", &me, &others), Recipient::Teammate(&others[0]));
        assert_eq!(recipient_of("Docs", &me, &others), Recipient::Teammate(&others[0]));
        assert_eq!(recipient_of("Gem", &me, &others), Recipient::Nobody);
        assert_eq!(recipient_of("Finance Bro", &me, &others), Recipient::Itself);
        assert_eq!(recipient_of("Jimothy", &me, &others), Recipient::Itself);
        assert_eq!(recipient_of("Nobody", &me, &others), Recipient::Nobody);
    }

    #[test]
    fn a_finished_reply_posts_each_share_once_and_says_what_it_could_not_send() {
        let root = std::env::temp_dir().join(format!("locust-exchange-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        let workroom = Workroom::new(&root);
        let me = entry("tm_atlas", "Atlas", "Research");
        let others = vec![entry("tm_wren", "Wren", "Code")];
        let reply = "Done.\n<locust-share to=\"Wren (Code)\">The build runs through pnpm check.</locust-share>\n<locust-share to=\"Wren\">The build runs through pnpm check.</locust-share>\n<locust-share to=\"Pip\">hello</locust-share>\n<locust-share to=\"Atlas\">note to self</locust-share>";
        let (posted, said) = share_reply(&workroom, &me, &others, "mission_a1", reply, Some(&["src/app.ts".to_owned()]));
        assert_eq!(posted.len(), 1);
        assert_eq!((posted[0].to.name.as_str(), posted[0].text.as_str()), ("Wren", "The build runs through pnpm check."));
        assert_eq!(posted[0].observed.as_deref(), Some(&["src/app.ts".to_owned()][..]));
        assert_eq!(said, vec!["Atlas addressed a message to \"Pip\", who is not on the roster. Nothing was sent.".to_owned()]);
        assert_eq!(workroom.unread("tm_wren", 5, &[]).unwrap().messages.len(), 1);
        let many: String = (0..6).map(|i| format!("<locust-share to=\"Wren\">note {i}</locust-share>\n")).collect();
        let (posted, said) = share_reply(&workroom, &me, &others, "mission_a2", &many, None);
        assert_eq!(posted.len(), MAX_SHARES_PER_MISSION);
        assert_eq!(said, vec!["Atlas tried to share 6 messages; only the first 4 were sent.".to_owned()]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
