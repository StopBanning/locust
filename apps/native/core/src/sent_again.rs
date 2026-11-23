//! A run stopped before it said anything, and the way on from it (`an-ended-run-offers-the-way-on`, 0.496).
//!
//! Thread.tsx: a run stopped before any tool ran offers Send again; a turn stopped before it said, planned or ran
//! anything and then sent again with the same words is one message, not two (`sentAgainBy`); a kept one says it was
//! "Stopped before it replied" (`stoppedBeforeSaying`); and a fresh session is said only when a turn before it is still
//! on screen. The ledger's items stand in for the events: a turn is its person's words and what follows up to the next.
use crate::ledger::Item;

/// Under a kept turn stopped before it said anything (Claude Code's "Interrupted").
pub const STOPPED_BEFORE_IT_REPLIED: &str = "Stopped before it replied";
/// Beside Send again, when no tool had run (not on OpenCode, whose card already says to check the folder, 0.696).
pub const NOTHING_TO_REPEAT: &str = "No tool had run, so sending it again cannot repeat anything.";

/// The person's turns: each `User` item's index, its words, and where its turn ends (the next `User`, or the end).
pub fn turns(items: &[Item]) -> Vec<(usize, &str, usize)> {
    let starts: Vec<usize> = items.iter().enumerate().filter(|(_, i)| matches!(i, Item::User(_))).map(|(at, _)| at).collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &at)| {
            let Item::User(words) = &items[at] else { unreachable!() };
            (at, words.as_str(), starts.get(n + 1).copied().unwrap_or(items.len()))
        })
        .collect()
}

/// stoppedBeforeSaying: the turn was stopped, and before that it said, planned or ran nothing. A message that came
/// (even an empty one), a step begun or a plan counts as having started; a thought alone is the model thinking.
pub fn stopped_before_saying(turn: &[Item]) -> bool {
    let mut stopped = false;
    for item in turn {
        match item {
            Item::Note { level, text } if level.is_empty() && text == "Stopped" => stopped = true,
            Item::Assistant { .. } | Item::Tool { .. } | Item::Plan { .. } => return false,
            _ => {}
        }
    }
    stopped
}

/// sentAgainBy: a turn stopped before it said anything, followed by the same words (trimmed, never empty), is one
/// message: the stopped copy gives way to the one sent again.
pub fn sent_again_by(words: &str, turn: &[Item], next_words: Option<&str>) -> bool {
    let Some(next) = next_words else { return false };
    !words.trim().is_empty() && words.trim() == next.trim() && stopped_before_saying(turn)
}

/// The turns that gave way to the one sent again after them, by their `User` item's index.
pub fn given_way(items: &[Item]) -> Vec<(usize, usize)> {
    let turns = turns(items);
    turns
        .iter()
        .enumerate()
        .filter(|(n, (at, words, end))| sent_again_by(words, &items[at + 1..*end], turns.get(n + 1).map(|t| t.1)))
        .map(|(_, (at, _, end))| (*at, *end))
        .collect()
}

/// Whether a turn before the newest is still on screen (not given way to the one after it): only then is a fresh
/// session said, since only then is anything before it lost to the runtime.
pub fn a_turn_before_is_shown(items: &[Item]) -> bool {
    let turns = turns(items);
    let given = given_way(items);
    turns.len() > 1 && turns[..turns.len() - 1].iter().any(|(at, _, _)| !given.iter().any(|(g, _)| g == at))
}
