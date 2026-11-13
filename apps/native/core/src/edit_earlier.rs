//! Edit an earlier message, optionally putting files back (Electron 0.498, 0.502 and 0.512).
//!
//! Sending an edit starts the conversation again from the turn BEFORE the edited one: a new branch, the turns after
//! the edit set aside rather than deleted, so the earlier version stays reachable. What lives here is what the
//! window and the engine share and can be tested without either: which turn is the other version of an edited one
//! (`turnVersions`), what an edit sets aside that a teammate was told (`set-aside-messages.ts`), and the words the
//! message box and the thread use about it.

use crate::workroom::Workroom;
use std::collections::{HashMap, HashSet};

/// Above the new turn: it began again from an edit (Thread.tsx).
pub const STARTED_AGAIN: &str = "Started again from an edited message.";

/// A turn as the version finder reads it: its place in the chain and when it was made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnLink {
    pub mission_id: String,
    pub created_at: String,
    /// The turn it continues, when it continues one.
    pub parent: Option<String>,
    /// Its link is a follow-up (a route switch is not an edit's neighbour).
    pub follow_up: bool,
    /// It is an edit: an earlier version followed the same turn.
    pub edited: bool,
}

/// The other version of a turn an edit touched: `before` for the edit (the newest turn of the version it replaced),
/// `after` for the replaced turn (the newest turn of the edit).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Versions {
    pub before: Option<String>,
    pub after: Option<String>,
}

fn at_millis(text: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(text).map(|t| t.timestamp_millis()).unwrap_or(i64::MIN)
}

/// missionView.ts `newestTurnFrom`: the newest turn reached from `start` through the turns that continue it.
fn newest_from<'a>(start: &'a TurnLink, children: &HashMap<&str, Vec<&'a TurnLink>>) -> &'a TurnLink {
    let mut newest = start;
    let mut seen = HashSet::new();
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(at) = queue.pop_front() {
        if !seen.insert(at.mission_id.as_str()) {
            continue;
        }
        if at_millis(&at.created_at) > at_millis(&newest.created_at) {
            newest = at;
        }
        queue.extend(children.get(at.mission_id.as_str()).into_iter().flatten().copied());
    }
    newest
}

/// missionView.ts `turnVersions`, for several turns at once (the chain is read once).
pub fn turn_versions_of(ids: &[&str], turns: &[TurnLink]) -> HashMap<String, Versions> {
    let by_id: HashMap<&str, &TurnLink> = turns.iter().map(|t| (t.mission_id.as_str(), t)).collect();
    let mut children: HashMap<&str, Vec<&TurnLink>> = HashMap::new();
    for turn in turns {
        if let Some(from) = turn.parent.as_deref() {
            children.entry(from).or_default().push(turn);
        }
    }
    let mut found = HashMap::new();
    for id in ids {
        let Some(turn) = by_id.get(id) else { continue };
        let Some(parent) = turn.parent.as_deref() else { continue };
        if !turn.follow_up {
            continue;
        }
        let mut siblings: Vec<&TurnLink> = children.get(parent).into_iter().flatten().copied().filter(|t| t.follow_up).collect();
        siblings.sort_by_key(|t| at_millis(&t.created_at));
        let Some(at) = siblings.iter().position(|t| t.mission_id == *id) else { continue };
        let older = if turn.edited && at > 0 { siblings.get(at - 1).copied() } else { None };
        let newer = siblings[at + 1..].iter().copied().find(|t| t.edited);
        if older.is_none() && newer.is_none() {
            continue;
        }
        found.insert(
            (*id).to_owned(),
            Versions {
                before: older.map(|t| newest_from(t, &children).mission_id.clone()),
                after: newer.map(|t| newest_from(t, &children).mission_id.clone()),
            },
        );
    }
    found
}

/// missionView.ts `turnVersions` for one turn.
pub fn turn_versions(id: &str, turns: &[TurnLink]) -> Option<Versions> {
    turn_versions_of(&[id], turns).remove(id)
}

const MAX_SET_ASIDE: usize = 500;

fn is_turn_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// set-aside-messages.ts `retireSetAsideMessages`: each message the set-aside turns sent that nobody has read goes back
/// to the turn that sent it, recorded as delivered to that turn, which is the record's way of saying it reached no one
/// else (a new kind of record would make every older reader drop the rest of the workroom file). A message already
/// delivered is left as it was. Returns the messages retired; a list that is not a list of turn ids retires none.
pub fn retire_set_aside_messages(workroom: &Workroom, listed: &[String]) -> Result<Vec<String>, String> {
    if listed.is_empty() || listed.len() > MAX_SET_ASIDE {
        return Ok(Vec::new());
    }
    let turns: HashSet<&str> = listed.iter().map(String::as_str).filter(|id| is_turn_id(id)).collect();
    if turns.is_empty() {
        return Ok(Vec::new());
    }
    let snapshot = workroom.read();
    let delivered: HashSet<&str> = snapshot.deliveries.iter().map(|d| d.message_id.as_str()).collect();
    let mut by_sender: Vec<(&str, Vec<String>)> = Vec::new();
    for message in &snapshot.messages {
        if delivered.contains(message.message_id.as_str()) || !turns.contains(message.from_mission.as_str()) {
            continue;
        }
        match by_sender.iter_mut().find(|(sender, _)| *sender == message.from_mission) {
            Some((_, ids)) => ids.push(message.message_id.clone()),
            None => by_sender.push((message.from_mission.as_str(), vec![message.message_id.clone()])),
        }
    }
    let mut retired = Vec::new();
    for (sender, ids) in by_sender {
        workroom.mark_delivered(&ids, sender)?;
        retired.extend(ids);
    }
    Ok(retired)
}

/// Composer.tsx, over the box while an earlier message is edited: what sending will do. `total` is how many files the
/// replies after it changed and `cannot` how many of them cannot be put back exactly (all of them or none go back, so
/// one that cannot holds the rest).
pub fn editing_line(total: usize, cannot: usize) -> String {
    const HEAD: &str = "Editing an earlier message \u{2014} sending starts again from there.";
    if total == 0 {
        format!("{HEAD} Files stay as they are.")
    } else if cannot > 0 {
        format!("{HEAD} Files stay as they are: {cannot} of the {total} files the replies changed cannot be put back exactly, and putting back only the others could leave the project half changed.")
    } else {
        HEAD.to_owned()
    }
}

/// The tick beside it, when files can all go back.
pub fn put_back_label(count: usize) -> String {
    format!("Also put back the {count} {} the replies after it changed", if count == 1 { "file" } else { "files" })
}

/// What putting the files back came to, as the thread says it above the new turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PutBackNote {
    pub put_back: Vec<String>,
    pub left_alone: Vec<(String, String)>,
    /// Files that could have gone back and were held because others could not.
    pub held_back: usize,
}

fn left_words(left: &[(String, String)]) -> String {
    left.iter().map(|(path, why)| format!("{path} ({why})")).collect::<Vec<_>>().join("; ")
}

/// Thread.tsx: the note above a turn that started again from an edit. `None` for the put-back is "nothing asked".
pub fn started_again_note(put_back: Option<&PutBackNote>) -> String {
    const SET_ASIDE: &str = "The replies after it were set aside";
    let Some(note) = put_back else {
        return format!("{STARTED_AGAIN} {SET_ASIDE}; files they changed are as they left them. ");
    };
    let mut said = if note.held_back > 0 {
        format!(
            "{STARTED_AGAIN} {SET_ASIDE}; files they changed are as they left them. Nothing was put back: {} could not be, and putting back only the other {} could leave the project half changed. ",
            left_words(&note.left_alone),
            note.held_back
        )
    } else {
        let n = note.put_back.len();
        let put = match n {
            0 => String::new(),
            1 => ", and 1 file they changed was put back".to_owned(),
            n => format!(", and {n} files they changed were put back"),
        };
        format!("{STARTED_AGAIN} {SET_ASIDE}{put}. ")
    };
    if !note.left_alone.is_empty() && note.held_back == 0 {
        said.push_str(&format!("Left as {}: {}. ", if note.left_alone.len() == 1 { "it is" } else { "they are" }, left_words(&note.left_alone)));
    }
    said
}

/// Thread.tsx's note over a turn an edit touched, when this window did not start it: it says so, and where the other
/// version is. `None` when there is no other version.
pub fn edited_note(versions: &Versions) -> Option<&'static str> {
    if versions.before.is_some() {
        Some("Edited.")
    } else if versions.after.is_some() {
        Some("This message was edited later.")
    } else {
        None
    }
}

#[cfg(test)]
#[path = "edit_earlier_tests.rs"]
mod tests;
