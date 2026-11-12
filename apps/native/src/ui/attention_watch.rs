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
    // A card raised since the last look.
    for card in locust_core::engine_api::api().waiting() {
        if !s.told_cards.insert(card.approval_id.clone()) || !tell {
            continue;
        }
        let who = s.data.as_ref().and_then(|store| locust_core::people::owner_of(store, &card.mission_id)).and_then(|id| s.teammates.iter().find(|t| t.id == id).map(|t| t.name.clone()));
        let question = card.kind == locust_core::approval::Kind::Question;
        let (title, body) = attention::approval_text(question, &card.summary, who.as_deref(), None);
        crate::notify::show(&title, &body, Some(&card.mission_id));
    }
    // A turn this process ran that has ended since the last look.
    let now: std::collections::HashSet<String> = locust_core::engine_api::api().running().into_iter().collect();
    let ended: Vec<String> = s.live_turns.difference(&now).cloned().collect();
    s.live_turns = now;
    for id in ended {
        let Some(turn) = native::turn_records(&id) else { continue };
        if turn.metadata["missionId"].as_str() != Some(id.as_str()) {
            continue;
        }
        let who = s.data.as_ref().and_then(|store| locust_core::people::owner_of(store, &id)).and_then(|t| s.teammates.iter().find(|m| m.id == t).map(|m| m.name.clone()));
        let end = turn.events.iter().rev().find(|e| matches!(e["type"].as_str(), Some("run.completed" | "run.failed" | "run.cancelled")));
        let kind = end.and_then(|e| e["type"].as_str()).unwrap_or("");
        if kind == "run.failed" && end.is_some_and(|e| e["payload"]["kind"] == "quota-exhausted") {
            if tell {
                let runtime = locust_core::carry_on::display_name(turn.metadata["runtime"].as_str().unwrap_or(""));
                let (title, body) = attention::limit_text(who.as_deref(), runtime, end.and_then(|e| e["payload"]["message"].as_str()));
                crate::notify::show(&title, &body, Some(&id));
            }
            continue;
        }
        let started = turn.metadata["createdAt"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
        let took = started.map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_milliseconds().max(0) as u64).unwrap_or(0);
        let toast = attention::finished_toast(
            kind == "run.completed",
            kind == "run.failed",
            turn.metadata["startedBy"].is_null(),
            took,
            who.as_deref(),
            locust_core::carry_on::latest_final(&turn.events).as_deref(),
            end.and_then(|e| e["payload"]["message"].as_str()),
        );
        if let Some(finish) = toast {
            if s.finishes.is_empty() {
                s.finishes_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(attention::FINISH_WINDOW_MS));
            }
            s.finishes.push(finish);
            s.finish_ids.push(id.clone());
        }
    }
    // Finishes gathered for four seconds, said as one, and only if the window is still elsewhere.
    if s.finishes_due.is_some_and(|due| std::time::Instant::now() >= due) {
        let said = std::mem::take(&mut s.finishes);
        let ids = std::mem::take(&mut s.finish_ids);
        s.finishes_due = None;
        if tell && !said.is_empty() {
            let (title, body) = attention::finished_text(&said);
            // Several finishes said as one open the newest of them.
            crate::notify::show(&title, &body, ids.last().map(String::as_str));
        }
    }
    false
}
