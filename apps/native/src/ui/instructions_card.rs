//! WHAT AN INTERVIEW WROTE (`InstructionsCard.tsx`, `Thread.tsx`).
//!
//! Under the teammate's last answer of an interview, the instructions it wrote from the person's answers,
//! in a box they can change, and Save. Nothing is kept until they press it; a key in them is refused here,
//! since instructions go to the teammate's model on every turn.

use super::*;
use locust_core::interview::{instructions_block, instructions_refusal, is_interview};
use serde_json::json;

#[derive(Default)]
pub(super) struct InstructionsCardState {
    pub(super) key: String,
    pub(super) draft: TextEdit,
    pub(super) problem: Option<String>,
    pub(super) saved: bool,
}

fn current_teammate_instructions(s: &State, teammate_id: &str) -> Option<String> {
    let store = s.data.as_ref()?;
    let team = people::team(store).ok()?;
    team["teammates"]
        .as_array()?
        .iter()
        .find(|t| t["teammateId"] == teammate_id)
        .and_then(|t| t["instructions"].as_str())
        .map(str::to_owned)
        .filter(|inst| !inst.trim().is_empty())
}

/// The active interview instructions block on the latest finished turn, if any.
pub(super) fn active_interview_instructions(s: &State) -> Option<(String, String, String)> {
    if turn_running(s) || s.flight.is_some() || s.sending.is_pending() {
        return None;
    }
    let (teammate, conversation) = selected_conversation(s)?;
    if teammate.id == LEDGER_TARGET || teammate.name.trim().is_empty() {
        return None;
    }
    let fallback;
    let items: &[ledger::Item] = if let Some(pane) = &s.ledger {
        if pane.loading || pane.snapshot.running {
            return None;
        }
        &pane.items
    } else {
        fallback = conversation
            .turns
            .iter()
            .enumerate()
            .map(|(i, (who, body))| {
                if who == "You" {
                    ledger::Item::User(body.clone())
                } else {
                    ledger::Item::Assistant {
                        item_id: format!("legacy-{i}"),
                        text: body.clone(),
                        r#final: true,
                    }
                }
            })
            .collect::<Vec<_>>();
        &fallback
    };

    let first_prompt = items.iter().find_map(|item| match item {
        ledger::Item::User(prompt) => Some(prompt.as_str()),
        _ => None,
    });
    if !is_interview(first_prompt) {
        return None;
    }

    let turn_start = items
        .iter()
        .rposition(|item| matches!(item, ledger::Item::User(_)))
        .map(|i| i + 1)
        .unwrap_or(0);
    let last_final = items[turn_start..].iter().rev().find_map(|item| match item {
        ledger::Item::Assistant {
            text, r#final: true, ..
        } => Some(text.as_str()),
        _ => None,
    })?;
    let written = instructions_block(last_final)?;
    Some((teammate.id.clone(), teammate.name.clone(), written))
}

pub(super) fn sync_state(s: &mut State) {
    let Some((teammate_id, _, written)) = active_interview_instructions(s) else {
        return;
    };
    let key = format!("{teammate_id}:{written}");
    if s.instructions_card.key != key {
        let current = current_teammate_instructions(s, &teammate_id);
        s.instructions_card.key = key;
        s.instructions_card.draft.replace_text(written.clone());
        s.instructions_card.problem = None;
        s.instructions_card.saved = current
            .as_deref()
            .is_some_and(|c| c.trim() == written.trim());
    }
}

pub(super) fn card(s: &State, p: &Palette) -> Option<El> {
    let (teammate_id, name, written) = active_interview_instructions(s)?;
    let key = format!("{teammate_id}:{written}");
    let l = crate::look::controls::Look::new(p.dark);
    let current = current_teammate_instructions(s, &teammate_id);
    let state_matches = s.instructions_card.key == key;
    let saved = if state_matches {
        s.instructions_card.saved
    } else {
        current
            .as_deref()
            .is_some_and(|c| c.trim() == written.trim())
    };
    if saved {
        return Some(
            row()
                .key("instructions-card-saved")
                .w_full()
                .gap(8.0)
                .padding(12.0)
                .radius(8.0)
                .fill(l.c("bg-raised"))
                .border(1.0, l.c("border-hairline"))
                .child(
                    text(format!(
                        "Saved as {name}’s instructions. {name} follows them from its next turn; change them any time in {name}’s settings."
                    ))
                    .font(type_::LABEL)
                    .color(l.c("text-secondary"))
                    .wrap(),
                ),
        );
    }

    let draft_ref = if state_matches {
        &s.instructions_card.draft
    } else {
        // Fallback read-only view if sync_state hasn't run yet.
        &s.instructions_card.draft
    };
    let mut body = col()
        .key("instructions-card")
        .w_full()
        .gap(8.0)
        .padding(14.0)
        .radius(10.0)
        .fill(l.c("bg-raised"))
        .border(1.0, l.c("border-hairline"))
        .child(
            text(format!(
                "{name} wrote its own instructions from your answers. Change anything, then save them: every turn {name} takes is given these."
            ))
            .font(type_::LABEL)
            .color(l.c("text-secondary"))
            .wrap(),
        )
        .child(
            text_input(draft_ref, type_::BODY)
                .key("instructions-card-input")
                .label(format!("{name}’s instructions"))
                .w_full()
                .padding(8.0)
                .fill(p.field)
                .color(p.text)
                .on_edit(move |s: &mut State| {
                    sync_state(s);
                    s.instructions_card.problem = None;
                    &mut s.instructions_card.draft
                }),
        );
    if current.is_some() {
        body = body.child(
            text(format!(
                "They take the place of the instructions {name} has now."
            ))
            .font(type_::LABEL)
            .color(p.muted)
            .wrap(),
        );
    }
    if state_matches && let Some(problem) = &s.instructions_card.problem {
        body = body.child(
            text(problem.clone())
                .font(type_::LABEL)
                .color(p.red)
                .wrap(),
        );
    }
    let save_id = teammate_id;
    let save_name = name.clone();
    let save_written = written;
    body = body.child(
        row().child(
            text(format!("Save as {name}’s instructions"))
                .font(type_::LABEL)
                .color(p.text)
                .padding(8.0)
                .radius(6.0)
                .fill(p.selected)
                .key("instructions-card-save")
                .on_press(move |s: &mut State| {
                    sync_state(s);
                    let trimmed = s.instructions_card.draft.text.trim().to_owned();
                    let text_to_save = if trimmed.is_empty() {
                        save_written.trim().to_owned()
                    } else {
                        trimmed
                    };
                    if text_to_save.is_empty() {
                        return;
                    }
                    if let Some(refused) = instructions_refusal(&save_name, &text_to_save) {
                        s.instructions_card.problem = Some(refused);
                        return;
                    }
                    let result = s
                        .data
                        .as_ref()
                        .ok_or_else(|| {
                            format!(
                                "They could not be saved. {save_name}’s instructions are unchanged."
                            )
                        })
                        .and_then(|store| {
                            let team = people::team(store)?;
                            let mut entry = team["teammates"]
                                .as_array()
                                .and_then(|arr| {
                                    arr.iter()
                                        .find(|t| t["teammateId"] == save_id)
                                        .cloned()
                                })
                                .ok_or_else(|| {
                                    "That teammate is not in Locust any more. Nothing was saved."
                                        .to_owned()
                                })?;
                            entry["instructions"] = json!(text_to_save);
                            people::save_teammate(store, entry)?;
                            Ok(())
                        });
                    match result {
                        Ok(()) => {
                            s.instructions_card.saved = true;
                            s.instructions_card.problem = None;
                            if let Some(store) = &s.data
                                && let Ok(team) = people::team(store)
                            {
                                s.people_panel.team = team;
                            }
                        }
                        Err(e) => s.instructions_card.problem = Some(e),
                    }
                }),
        ),
    );
    Some(body)
}
