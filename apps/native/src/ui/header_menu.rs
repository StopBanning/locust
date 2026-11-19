//! The conversation header's `⋯` (App.tsx `headerActions`): what the open conversation offers, each entry present
//! only where it is true, opened as the window's one menu (context_menu.rs). Verbose or Normal first; Send feedback
//! about this conversation, carrying what was said in it; Delete conversation, asked in place, once nothing runs.
use super::*;
use crate::context_menu::{Item, MenuState};

/// The latest turn's id, which the whole conversation is found by (Electron's `liveRun.data.missionId`).
fn latest_turn(s: &State) -> Option<String> {
    s.ledger.as_ref().and_then(|pane| pane.snapshot.turn_metas.last()).map(|m| m.mission_id.clone()).filter(|id| !id.is_empty())
}

/// The entries, in Electron's order, for the conversation on screen.
pub(super) fn items(s: &State) -> Vec<Item> {
    let mut items = vec![Item::new(if verbose(s) { "Normal" } else { "Verbose" }, "header:verbose")];
    if s.ledger.is_some() {
        items.push(Item::new("Send feedback", "header:feedback"));
    }
    if latest_turn(s).is_some() && !turn_running(s) {
        items.push(Item::new("Delete conversation", "delete").danger("Delete for good?"));
    }
    items
}

/// The turns on screen as a report reads them (feedback.ts `conversationText`): what was typed, what was said, and a
/// failure's words where the turn ended on one.
pub(super) fn conversation_for_report(s: &State) -> Option<String> {
    let pane = s.ledger.as_ref()?;
    let mut turns: Vec<locust_core::report_problem::ReportTurn> = Vec::new();
    for item in &pane.items {
        match item {
            ledger::Item::User(prompt) => turns.push(locust_core::report_problem::ReportTurn { prompt: prompt.clone(), reply: String::new(), failed_message: None }),
            ledger::Item::Assistant { text, .. } => {
                if let Some(turn) = turns.last_mut() {
                    let said = locust_core::protocol_tags::shown_text(text);
                    if !said.trim().is_empty() {
                        if !turn.reply.is_empty() {
                            turn.reply.push_str("\n\n");
                        }
                        turn.reply.push_str(said.trim());
                    }
                }
            }
            ledger::Item::Note { level, text } if level == "error" => {
                if let Some(turn) = turns.last_mut() {
                    turn.failed_message = Some(text.clone());
                }
            }
            _ => {}
        }
    }
    if turns.is_empty() {
        return None;
    }
    let speaker = selected_conversation(s).map(|(t, _)| t.name.clone()).unwrap_or_else(|| "The teammate".into());
    Some(locust_core::report_problem::conversation_text(&turns, &speaker))
}

/// What the header's own entries do; the rest are the conversation menu's (`context_menu::run`).
pub(super) fn run(s: &mut State, action: &str) -> bool {
    match action {
        "header:verbose" => settings_panel::toggle_transcript_view(s),
        "header:feedback" => {
            let conversation = conversation_for_report(s);
            crate::feedback_dialog::open(s, conversation);
        }
        _ => return false,
    }
    true
}

/// The `⋯` button, at the end of the header's actions; it opens the menu under itself. No entries, no button.
pub(super) fn control(s: &State, p: &Palette) -> Option<El> {
    if items(s).is_empty() {
        return None;
    }
    let button = crate::look::controls::head_icon(&crate::look::icons::DOTS, "More", "More actions for this conversation").on_press(|s: &mut State| {
        let r = s.header_menu_at;
        let target = latest_turn(s).unwrap_or_default();
        s.row_menu = Some(MenuState {
            x: r[0] + r[2] - crate::context_menu::MENU_WIDTH,
            y: r[1] + r[3] + 4.0,
            title: "Conversation actions".into(),
            target,
            items: items(s),
            armed: None,
            open_sub: None,
        });
    });
    let button = button.build(crate::look::controls::Look::new(p.dark).k);
    Some(el().key("thread-header-more").child(button).on_layout(|s: &mut State, r| {
        if s.header_menu_at != r {
            s.header_menu_at = r;
        }
    }))
}

#[cfg(test)]
#[path = "header_menu_tests.rs"]
mod tests;
