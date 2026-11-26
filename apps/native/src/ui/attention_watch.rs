//! What the native engine tells you while Locust's window is elsewhere (main/attention.ts): a card waiting for your
//! answer, a turn that hit its plan's limit, and a long turn you started that ended, finishes close together said once.
//! The window's own engine is the native one here; with Electron's engine, Electron tells you itself.

use super::*;
use locust_core::attention;

/// Checks the native engine once per poll. Returns whether anything changed on screen (nothing does: a notification
/// is the system's).
pub(super) fn poll(s: &mut State) -> bool {
    if s.engine != Some(Kind::Native) {
        return false;
    }
    let tell = attention::should_notify(s.window_focused, crate::notify::supported());
    let mut show = |title: &str, body: &str, opens: Option<&str>| crate::notify::show(title, body, opens);
    // A card raised since the last look.
    for card in locust_core::engine_api::api().waiting() {
        say_card(s, &card, tell, &mut show);
    }
    // A turn this process ran that has ended since the last look.
    let now: std::collections::HashSet<String> = locust_core::engine_api::api().running().into_iter().collect();
    poll_ended(s, now, tell, &mut show, native::turn_records);
    say_finishes(s, std::time::Instant::now(), tell, &mut show);
    false
}

/// The turns that were running at the last look and are not now, read from their records: a limit or a question is
/// said at once, and a finish is gathered for `say_finishes`. Only a turn this process ran is ever looked at (it was in
/// `live_turns`), so a run restored from the records is not told as a finish.
pub(super) fn poll_ended(
    s: &mut State,
    now: std::collections::HashSet<String>,
    tell: bool,
    show: &mut impl FnMut(&str, &str, Option<&str>),
    records: impl Fn(&str) -> Option<native::TurnRecords>,
) {
    let ended: Vec<String> = s.live_turns.difference(&now).cloned().collect();
    s.live_turns = now;
    for id in ended {
        let Some(turn) = records(&id) else { continue };
        if turn.metadata["missionId"].as_str() != Some(id.as_str()) {
            continue;
        }
        // attention-reader.ts: a run that ended waiting for the person -- at its account limit, or with a card asking
        // them something -- says so. A limit is said instead of the finish.
        if say_ended(s, &id, &turn, tell, show) {
            continue;
        }
        if let Some(finish) = finish_of(s, &id, &turn) {
            if s.finishes.is_empty() {
                s.finishes_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(attention::FINISH_WINDOW_MS));
            }
            s.finishes.push(finish);
            s.finish_ids.push(id.clone());
        }
    }
}

/// finishedToast, read from a turn's record: who, the first line of the last thing said, how long it took, said for a
/// turn that completed or failed and that the person started; nothing for a stop, a room's or a relay's turn, or one
/// with no start time (it reads as 0 ms, under the minute worth telling).
pub(super) fn finish_of(s: &State, id: &str, turn: &native::TurnRecords) -> Option<(String, String)> {
    let end = turn.events.iter().rev().find(|e| matches!(e["type"].as_str(), Some("run.completed" | "run.failed" | "run.cancelled")));
    let kind = end.and_then(|e| e["type"].as_str()).unwrap_or("");
    let started = turn.metadata["createdAt"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
    let took = started.map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_milliseconds().max(0) as u64).unwrap_or(0);
    attention::finished_toast(
        kind == "run.completed",
        kind == "run.failed",
        turn.metadata["startedBy"].is_null(),
        took,
        owner_name(s, id).as_deref(),
        locust_core::carry_on::latest_final(&turn.events).as_deref(),
        end.and_then(|e| e["payload"]["message"].as_str()),
    )
}

/// approvalArrived: a card raised since the last look is said once, and only while the window is elsewhere; a click
/// brings Locust forward (the system does) and opens the card's conversation.
pub(super) fn say_card(s: &mut State, card: &locust_core::approval::Request, tell: bool, show: &mut impl FnMut(&str, &str, Option<&str>)) {
    if !s.told_cards.insert(card.approval_id.clone()) || !tell {
        return;
    }
    let notice = card_notice(s, card);
    show(&notice.title, &notice.body, notice.opens.as_deref());
}

/// attention-reader.ts: a turn that ended waiting for the person, with a card asking them something or at its account
/// limit, is said at once while the window is elsewhere, and not at all while it is in front. True for a limit, which
/// is said instead of the finish.
pub(super) fn say_ended(s: &State, id: &str, turn: &native::TurnRecords, tell: bool, show: &mut impl FnMut(&str, &str, Option<&str>)) -> bool {
    let Some(notice) = ended_turn_notice(s, id, turn) else { return false };
    if tell {
        show(&notice.title, &notice.body, notice.opens.as_deref());
    }
    notice.limit
}

/// Finishes gathered for four seconds, said as one, and only if the window is still elsewhere when they are due.
pub(super) fn say_finishes(s: &mut State, now: std::time::Instant, tell: bool, show: &mut impl FnMut(&str, &str, Option<&str>)) {
    if !s.finishes_due.is_some_and(|due| now >= due) {
        return;
    }
    let said = std::mem::take(&mut s.finishes);
    let ids = std::mem::take(&mut s.finish_ids);
    s.finishes_due = None;
    if tell && !said.is_empty() {
        let (title, body) = attention::finished_text(&said);
        // Several finishes said as one open the newest of them.
        show(&title, &body, ids.last().map(String::as_str));
    }
}

/// The teammate a turn is the work of, by its name in the window's roster; None is said as "A teammate".
fn owner_name(s: &State, mission: &str) -> Option<String> {
    let id = locust_core::people::owner_of(s.data.as_ref()?, mission)?;
    s.teammates.iter().find(|t| t.id == id).map(|t| t.name.clone())
}

/// conversationAttentionTitle: the conversation a notification names, from the sidebar's titles and the records.
fn title_of(s: &State, mission: &str) -> String {
    let titles = s.data.as_ref().and_then(|store| locust_core::people::team(store).ok()).map(|file| file["missionTitles"].clone()).unwrap_or_default();
    attention::conversation_title(mission, &titles, |id| native::turn_records(id).map(|turn| turn.metadata))
}

/// approvalArrived: a card waiting for the person, its teammate and conversation named; a click opens its turn.
pub(super) fn card_notice(s: &State, card: &locust_core::approval::Request) -> attention::Notice {
    let question = card.kind == locust_core::approval::Kind::Question;
    attention::approval_notice(&card.mission_id, question, &card.summary, owner_name(s, &card.mission_id).as_deref(), Some(&title_of(s, &card.mission_id)))
}

/// attention-reader.ts onRunEnded: what a turn that ended waiting for the person raises, if anything.
pub(super) fn ended_turn_notice(s: &State, id: &str, turn: &native::TurnRecords) -> Option<attention::Notice> {
    attention::ended_notice(id, &turn.events, turn.metadata["runtime"].as_str().unwrap_or(""), owner_name(s, id).as_deref(), || title_of(s, id))
}

#[cfg(test)]
#[path = "attention_watch_tests.rs"]
mod attention_watch_tests;
