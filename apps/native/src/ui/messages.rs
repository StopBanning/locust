use super::*;

pub(super) fn message_column(items: &[ledger::Item], name: &str, p: &Palette, s: &State) -> El {
    let mut messages = col().w_full().max_w(super::thread_view::THREAD_MAX).gap(24.0).padding(24.0);
    let markers = s
        .ledger
        .as_ref()
        .map(|pane| locust_core::time_marker::thread_markers(&pane.snapshot.timings))
        .unwrap_or_default();
    let mut user_index = 0usize;
    for (index, item) in items.iter().enumerate() {
        for marker in markers.iter().filter(|m| m.before_item == index) {
            messages = messages.child(time_marker::view(marker, p));
        }
        let conversation = s
            .ledger
            .as_ref()
            .and_then(|p| p.conversation.conversation_id.as_deref())
            .unwrap_or("preview");
        let key = format!("{conversation}/ledger-item-{index}");
        if matches!(item,ledger::Item::Plan {steps,..} if steps.is_empty()) {
            continue;
        }
        if is_activity(item) {
            if index > 0 && is_activity(&items[index - 1]) {
                continue;
            }
            let end = items[index..]
                .iter()
                .position(|i| !is_activity(i))
                .map(|n| index + n)
                .unwrap_or(items.len());
            messages = messages.child(activity_group(&items[index..end], p, s, conversation));
            continue;
        }
        let content = match item {
            ledger::Item::CarryOn { conversation_id, from, why } => {
                if s.flight.is_some()
                    || s.sending.is_pending()
                    || s.ledger
                        .as_ref()
                        .is_some_and(|pane| pane.snapshot.running || pane.after_tip.is_some())
                {
                    continue;
                }
                let id = conversation_id.clone();
                let l = crate::look::controls::Look::new(p.dark);
                // `.lc-rerun`: the reason in muted words beside one press for the agent Locust suggests, on a green
                // rule; every other agent is behind "Choose another agent".
                let mut line = row()
                    .w_full()
                    .gap(12.0)
                    .center_y()
                    .child(
                        col()
                            .grow(1.0)
                            .child(text(locust_core::carry_on::stopped_sentence(from, why)).font(l.ty.body).color(l.c("text-muted")).wrap()),
                    );
                if let Some((runtime, model)) = &s.carry_offer {
                    let label = format!(
                        "Carry on with {}",
                        model.clone().unwrap_or_else(|| locust_core::carry_on::display_name(runtime).to_owned())
                    );
                    let (id, runtime, model) = (id.clone(), runtime.clone(), model.clone().unwrap_or_default());
                    line = line.child(
                        crate::look::controls::button(&l, &label, crate::look::controls::Tone::Neutral, false)
                            .on_press(move |s: &mut State| carry(s, id.clone(), runtime.clone(), model.clone())),
                    );
                }
                // Indented 36 inside the thread's width: a row whose first cell is the indent, so nothing runs past.
                let mut card = col()
                    .grow(1.0)
                    .gap(6.0)
                    .py(8.0)
                    .pl(12.0)
                    .border_sides([0.0, 0.0, 0.0, 2.0], l.c("green-border"))
                    .child(line)
                    .child(
                        text("Choose another agent")
                            .font(l.ty.body)
                            .color(l.c("text-muted"))
                            .hover(l.c("hover-surface"))
                            .py(2.0)
                            .role("button")
                            .on_press(|s: &mut State| open_models(s, CARRY_TARGET.into())),
                    );
                if s.model_menu.as_deref() == Some(CARRY_TARGET) {
                    if let Some(agents) = s.models.get(CARRY_TARGET).filter(|a| !a.is_empty()) {
                        let mut list = col()
                            .key(format!("{key}-carry-menu"))
                            .w_full()
                            .max_h(180.0)
                            .scroll(ScrollSpec {
                                stick_bottom: false,
                            })
                            .gap(4.0);
                        for agent in agents.iter().filter(|a| a.runtime != *from) {
                            for model in &agent.models {
                                let (id, runtime, model_id) =
                                    (id.clone(), agent.runtime.clone(), model.id.clone());
                                list = list.child(
                                    el().padding(8.0)
                                        .disabled(s.flight.is_some())
                                        .child(
                                            text(format!(
                                                "{} · {}",
                                                engine::runtime_name(&runtime),
                                                model.name
                                            ))
                                            .color(p.text),
                                        )
                                        .on_press(move |s: &mut State| {
                                            carry(s, id.clone(), runtime.clone(), model_id.clone())
                                        }),
                                );
                            }
                        }
                        card = card.child(list);
                    } else {
                        card = card.child(text("Loading models…").color(p.muted));
                    }
                }
                row().w_full().child(el().w(36.0)).child(card)
            }
            ledger::Item::User(body) => {
                let ui = user_index;
                user_index += 1;
                let hit = s.prompt_rail.hit_key.as_deref() == Some(key.as_str());
                // Notes on a diff came as one block after the person's words: the bubble shows the words, the notes go under it.
                let (own_words, sent_notes) = locust_core::diff_notes::split_diff_notes(body);
                let mut bubble = text(own_words)
                    .font(rune::TextStyle::new(14.5, 400.0).lh(21.0))
                    .color(p.text)
                    .shrink()
                    .wrap_anywhere()
                    .selectable(true)
                    .py(12.0)
                    .px(16.0)
                    .radius(10.0)
                    .fill(p.your_bubble);
                if hit {
                    bubble = bubble.border(1.5, p.amber);
                }
                let bubble_row = row()
                    .w_full()
                    .justify_end()
                    .on_layout_tx(move |s: &mut State, r, tx| {
                        let rel_top = r[1] - s.prompt_rail.column_rect[1];
                        if !s.prompt_rail.set_top(ui, rel_top) {
                            tx.live_only();
                        }
                    })
                    .child(row().w_frac(0.7).justify_end().child(bubble));
                let bubble_row = if sent_notes.is_empty() {
                    bubble_row
                } else {
                    let folder = selected_conversation(s).and_then(|(t, _)| t.folder.clone());
                    col()
                        .w_full()
                        .gap(6.0)
                        .child(bubble_row)
                        .child(row().w_full().justify_end().child(row().w_frac(0.7).justify_end().child(crate::diff_notes_ui::sent_notes(&sent_notes, folder.as_deref(), p))))
                };
                if let Some(chips) = pull_request_chip::link_chips_for_text(s, body, p) {
                    col()
                        .w_full()
                        .gap(4.0)
                        .child(bubble_row)
                        .child(row().w_full().justify_end().child(chips))
                } else {
                    bubble_row
                }
            }
            ledger::Item::Assistant {
                item_id,
                text: body,
                r#final,
            } => {
                let running = turn_running(s);
                let finished = *r#final || !running;
                let shown = locust_core::protocol_tags::shown_text(body);
                let files = if finished {
                    locust_core::protocol_tags::parse_file_blocks(body)
                } else {
                    Vec::new()
                };
                let refused = if finished {
                    locust_core::protocol_tags::refused_file_lines(body)
                } else {
                    Vec::new()
                };
                let steps = if finished {
                    locust_core::protocol_tags::parse_next_steps(body)
                } else {
                    Vec::new()
                };
                if shown.is_empty()
                    && files.is_empty()
                    && refused.is_empty()
                    && steps.is_empty()
                {
                    continue;
                }
                let scope = format!("{conversation}-{index}-{item_id}");
                let mut msg = col()
                    .w_full()
                    .gap(8.0)
                    .child(text(name).font(type_::LABEL).color(p.muted));
                if !shown.is_empty() {
                    let ask_scope = scope.clone();
                    let reply_box = col()
                        .w_full()
                        .selection_group(true)
                        .on_selection_change(move |s: &mut State, change, _tx| {
                            if change.cleared() || change.dragging() {
                                if s.selection_ask.as_ref().is_some_and(|a| a.scope == ask_scope) {
                                    s.selection_ask = None;
                                }
                            } else if let Some(sel) = change.selection() {
                                if !sel.text().trim().is_empty() {
                                    s.selection_ask = Some(crate::side_chat::SelectionAskState {
                                        scope: ask_scope.clone(),
                                        text: sel.text().to_owned(),
                                    });
                                }
                            }
                        })
                        .child(reply_actions::reply(s, &shown, !*r#final, p));
                    msg = msg.child(reply_box);
                    if let Some(chips) = pull_request_chip::link_chips_for_text(s, &shown, p) {
                        msg = msg.child(chips);
                    }
                }
                if let Some(files_el) = handed_files::view(s, &files, &refused, &scope, p) {
                    msg = msg.child(files_el);
                }
                let can_here = !running
                    && s.flight.is_none()
                    && !s.sending.is_pending()
                    && s.pending_approval.is_none();
                if let Some(steps_el) = next_steps::view(s, &steps, &scope, can_here, p) {
                    msg = msg.child(steps_el);
                }
                msg
            }
            ledger::Item::Tool { .. }
            | ledger::Item::Plan { .. }
            | ledger::Item::Thought { .. } => activity_item(item, p, s, conversation),
            // A stopped run's receipt: what this turn's record says finished, was cut off, or never started.
            ledger::Item::Note { level, text: body } if level.is_empty() && body == "Stopped" => {
                let from = items[..index].iter().rposition(|i| matches!(i, ledger::Item::User(_))).map_or(0, |at| at + 1);
                let teammate = selected_conversation(s).map(|(t, _)| t);
                let runtime = teammate.and_then(|t| t.runtime.clone()).unwrap_or_default();
                let folder = teammate.and_then(|t| t.folder.clone());
                let summary = locust_core::cancellation::summary(&items[from..index], &runtime, folder.as_deref());
                let last_stop = !items[index + 1..].iter().any(|i| matches!(i, ledger::Item::Note { text, .. } if text == "Stopped"));
                let by_person = last_stop && s.stopped_by_you.as_deref().is_some_and(|c| Some(c) == s.ledger.as_ref().and_then(|p| p.conversation.conversation_id.as_deref()));
                crate::cancellation_card::card(&summary, by_person, p)
            }
            ledger::Item::Note { level, text: body } => text(if level.is_empty() {
                body.clone()
            } else {
                format!("{level}: {body}")
            })
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .wrap_anywhere()
            .selectable(true),
            ledger::Item::CheckFix {
                command,
                fix,
                of,
                prompt,
            } => {
                let teammate_name = (!name.trim().is_empty() && name != "Locust").then_some(name);
                edit_check_card::check_fix_line(
                    command,
                    *fix,
                    *of,
                    prompt,
                    teammate_name,
                    p,
                    s,
                    &key,
                )
            }
            ledger::Item::EditCheck {
                mission_id,
                command,
                outcome,
                new_lines,
                unchanged,
                first,
                fix,
            } => {
                let is_tip = s
                    .ledger
                    .as_ref()
                    .is_some_and(|pane| pane.snapshot.tip == *mission_id);
                let teammate_name = (!name.trim().is_empty() && name != "Locust").then_some(name);
                edit_check_card::card(
                    mission_id,
                    command,
                    *outcome,
                    new_lines,
                    *unchanged,
                    *first,
                    *fix,
                    is_tip,
                    teammate_name,
                    p,
                    s,
                )
            }
        };
        messages = messages.child(content.key(key));
    }
    if let Some(pane) = &s.ledger
        && !pane.snapshot.tip.is_empty()
        && let Some(command) = s.running_checks.get(&pane.snapshot.tip)
    {
        messages = messages.child(edit_check_card::running_card(command, p));
    }
    if let Some(card) = recovery_card::card(s, p) {
        messages = messages.child(card);
    }
    if let Some(card) = decision_card::card(s, p) {
        messages = messages.child(card);
    }
    if let Some(card) = crate::instructions_card::card(s, p) {
        messages = messages.child(card);
    }
    messages
        .child(approval_card(s, p))
        .on_layout_tx(|s: &mut State, r, tx| {
            let resized = (s.prompt_rail.column_rect[2] - r[2]).abs() > 0.5
                || (s.prompt_rail.column_rect[3] - r[3]).abs() > 0.5
                || (s.prompt_rail.column_rect[0] - r[0]).abs() > 0.5;
            s.prompt_rail.column_rect = r;
            let view_top = (s.prompt_rail.viewport_rect[1] - r[1]).max(0.0);
            let next_current = prompt_rail::current_prompt_mark(&s.prompt_rail.tops, view_top);
            let current_changed = next_current != s.prompt_rail.current;
            s.prompt_rail.current = next_current;
            if !resized && !current_changed {
                tx.live_only();
            }
        })
}
