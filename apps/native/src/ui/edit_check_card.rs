//! The post-edit check card (`EditCheckCard.tsx`) and the automatic fix turn line (`CheckFixLine.tsx`).

use super::*;
use locust_core::edit_check::{EditCheckOutcome, edit_check_follow_up};

pub(super) fn running_card(command: &str, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    row()
        .key("edit-check-running")
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .radius(9.0)
        .fill(p.raised)
        .center_y()
        .child(rune::icon(&crate::look::icons::SPARK, 12.0).color(l.c("text-muted")))
        .child(
            text(format!("Running the check {command} after this turn…"))
                .font(l.ty.label)
                .color(l.c("text-muted"))
                .wrap_anywhere(),
        )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn card(
    mission_id: &str,
    command: &str,
    outcome: EditCheckOutcome,
    new_lines: &[String],
    unchanged: bool,
    first: bool,
    fix: Option<u32>,
    is_tip: bool,
    teammate_name: Option<&str>,
    p: &Palette,
    s: &State,
) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let key = format!("edit-check-card-{mission_id}");
    match outcome {
        EditCheckOutcome::Passed => {
            let after = match fix {
                Some(n) => format!("fix {n}"),
                None => "this turn".into(),
            };
            row()
                .key(key)
                .w_full()
                .gap(8.0)
                .padding(10.0)
                .radius(9.0)
                .fill(p.raised)
                .center_y()
                .child(rune::icon(&crate::look::icons::SHIELD, 12.0).color(l.c("text-muted")))
                .child(
                    text(format!("The check {command} passed after {after}."))
                        .font(l.ty.label)
                        .color(l.c("text-muted"))
                        .wrap_anywhere(),
                )
        }
        EditCheckOutcome::TimedOut => row()
            .key(key)
            .w_full()
            .gap(8.0)
            .padding(10.0)
            .radius(9.0)
            .fill(p.raised)
            .border(1.0, p.amber.alpha(0.35))
            .center_y()
            .child(rune::icon(&crate::look::icons::SHIELD, 12.0).color(p.amber))
            .child(
                text(format!(
                    "The check {command} ran past three minutes and was stopped, so it said nothing about this turn."
                ))
                .font(l.ty.label)
                .color(p.amber)
                .wrap_anywhere(),
            ),
        EditCheckOutcome::CouldNotRun => {
            let detail = new_lines
                .first()
                .map(|line| format!(": {line}"))
                .unwrap_or_default();
            row()
                .key(key)
                .w_full()
                .gap(8.0)
                .padding(10.0)
                .radius(9.0)
                .fill(p.raised)
                .border(1.0, p.amber.alpha(0.35))
                .center_y()
                .child(rune::icon(&crate::look::icons::SHIELD, 12.0).color(p.amber))
                .child(
                    text(format!(
                        "The check {command} could not be run{detail}. Change it in Settings."
                    ))
                    .font(l.ty.label)
                    .color(p.amber)
                    .wrap_anywhere(),
                )
        }
        EditCheckOutcome::Failed if unchanged || new_lines.is_empty() => row()
            .key(key)
            .w_full()
            .gap(8.0)
            .padding(10.0)
            .radius(9.0)
            .fill(p.raised)
            .border(1.0, p.amber.alpha(0.35))
            .center_y()
            .child(rune::icon(&crate::look::icons::SHIELD, 12.0).color(p.amber))
            .child(
                text(format!(
                    "The check {command} still fails after this turn, with nothing new in it."
                ))
                .font(l.ty.label)
                .color(p.amber)
                .wrap_anywhere(),
            ),
        EditCheckOutcome::Failed => {
            let suffix = if first {
                String::new()
            } else {
                format!(
                    " with {} line{} that were not there before",
                    new_lines.len(),
                    if new_lines.len() == 1 { "" } else { "s" }
                )
            };
            let mut body = col()
                .grow(1.0)
                .gap(8.0)
                .child(
                    text(format!("The check {command} failed after this turn{suffix}:"))
                        .font(l.ty.label)
                        .color(p.amber)
                        .wrap_anywhere(),
                )
                .child(
                    col()
                        .w_full()
                        .padding(8.0)
                        .radius(6.0)
                        .fill(p.field)
                        .child(
                            text(new_lines.join("\n"))
                                .font(l.ty.mono)
                                .color(p.text)
                                .wrap_anywhere()
                                .selectable(true),
                        ),
                );
            let can_send = is_tip
                && !turn_running(s)
                && s.flight.is_none()
                && !s.sending.is_pending()
                && s.running_checks.is_empty();
            if can_send {
                let send_label = format!(
                    "Send to {}",
                    teammate_name
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or("the teammate")
                );
                let prompt = edit_check_follow_up(command, new_lines, first);
                body = body.child(
                    row()
                        .key(format!("edit-check-send-{mission_id}"))
                        .padding(8.0)
                        .radius(6.0)
                        .fill(p.selected)
                        .on_press_tx(move |s: &mut State, tx| {
                            if turn_running(s)
                                || s.flight.is_some()
                                || s.sending.is_pending()
                                || !s.running_checks.is_empty()
                            {
                                return;
                            }
                            s.draft.replace_text(prompt.clone());
                            submit_composer(s, tx, false);
                        })
                        .child(text(send_label).font(l.ty.label).color(p.text)),
                );
            }
            row()
                .key(key)
                .w_full()
                .gap(8.0)
                .padding(10.0)
                .radius(9.0)
                .fill(p.raised)
                .border(1.0, p.amber.alpha(0.35))
                .child(rune::icon(&crate::look::icons::SHIELD, 12.0).color(p.amber))
                .child(body)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_fix_line(
    command: &str,
    fix: u32,
    of: u32,
    prompt: &str,
    teammate_name: Option<&str>,
    p: &Palette,
    s: &State,
    key: &str,
) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let to = teammate_name
        .filter(|n| !n.trim().is_empty())
        .map(|n| format!(" to {n}"))
        .unwrap_or_default();
    let said = if fix == 1 {
        format!("{command} failed, so Locust sent it back{to}")
    } else {
        format!("{command} still failed, so Locust sent it back{to} again")
    };
    let open = s.open_check_fix_prompts.contains(key);
    let toggle_key = key.to_owned();
    let mut box_ = col()
        .w_full()
        .gap(4.0)
        .child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(rune::icon(&crate::look::icons::REFRESH, 12.0).color(l.c("text-muted")))
                .child(text(said).font(l.ty.label).color(l.c("text-muted")).wrap())
                .child(
                    text(format!("· fix {fix} of {of}"))
                        .font(l.ty.mono)
                        .color(l.c("text-muted")),
                ),
        )
        .child(
            row()
                .key(format!("check-fix-toggle-{key}"))
                .on_press(move |s: &mut State| {
                    if s.open_check_fix_prompts.contains(&toggle_key) {
                        s.open_check_fix_prompts.remove(&toggle_key);
                    } else {
                        s.open_check_fix_prompts.insert(toggle_key.clone());
                    }
                })
                .child(
                    text(if open {
                        "▼ What Locust sent"
                    } else {
                        "▶ What Locust sent"
                    })
                    .font(l.ty.label)
                    .color(p.secondary),
                ),
        );
    if open {
        box_ = box_.child(
            col()
                .w_full()
                .padding(8.0)
                .radius(6.0)
                .fill(p.field)
                .child(
                    text(prompt.to_owned())
                        .font(l.ty.mono)
                        .color(p.secondary)
                        .wrap_anywhere()
                        .selectable(true),
                ),
        );
    }
    box_
}
