use super::*;

pub(super) fn message_column(items: &[ledger::Item], name: &str, p: &Palette, s: &State) -> El {
    let mut messages = col().w_full().max_w(super::thread_view::THREAD_MAX).gap(24.0).padding(24.0);
    let markers = s
        .ledger
        .as_ref()
        .map(|pane| locust_core::time_marker::thread_markers(&pane.snapshot.timings))
        .unwrap_or_default();
    let (group_joins, group_leavings, active_group_id) = thread_group_lines(s);
    let mut user_index = 0usize;
    let self_teammate_id = selected_conversation(s).map(|(t, _)| t.id.as_str()).unwrap_or("");
    let emit_turn_peers = |messages: El, turn_idx: usize| -> El {
        let Some(mission_id) = s
            .ledger
            .as_ref()
            .and_then(|pane| pane.snapshot.turn_metas.get(turn_idx))
            .map(|m| m.mission_id.as_str())
        else {
            return messages;
        };
        let peer_msgs = peer_thread::messages_for_mission(s, mission_id);
        let mut out = messages;
        if let Some(handed) = peer_thread::handed_out(s, &peer_msgs, p) {
            out = out.child(handed.key(format!("{mission_id}-handed-out")));
        }
        for (pi, card) in peer_thread::cards_for_mission(s, self_teammate_id, mission_id, &peer_msgs, p)
            .into_iter()
            .enumerate()
        {
            out = out.child(card.key(format!("{mission_id}-peer-{pi}")));
        }
        out
    };
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
                if ui > 0 {
                    messages = emit_turn_peers(messages, ui - 1);
                }
                if let Some(pane) = s.ledger.as_ref()
                    && let Some(to_meta) = pane.snapshot.turn_metas.get(ui)
                {
                    let from_meta = ui.checked_sub(1).and_then(|i| pane.snapshot.turn_metas.get(i));
                    if let Some(div) = handoff_divider::view(from_meta, to_meta, p) {
                        messages = messages.child(div.key(format!("{key}-handoff")));
                    }
                }
                // An edit this turn began, or one that touched it, says so above it (0.498).
                if let Some(pane) = s.ledger.as_ref()
                    && let Some(meta) = pane.snapshot.turn_metas.iter().filter(|m| m.check_fix.is_none()).nth(ui)
                    && let Some(note) = crate::edit_message::note_above(s, meta, p)
                {
                    messages = messages.child(note);
                }
                messages = emit_group_lines_at(
                    messages,
                    &group_joins,
                    &group_leavings,
                    active_group_id.as_deref(),
                    |bt| bt == ui,
                    p,
                    s,
                );
                let hit = s.prompt_rail.hit_key.as_deref() == Some(key.as_str());
                // Notes on a diff came as one block after the person's words: the bubble shows the words, the notes go under it.
                let (own_words, sent_notes) = locust_core::diff_notes::split_diff_notes(body);
                let edit = crate::edit_message::edit_control(s, ui, &own_words, !sent_notes.is_empty(), p);
                let turn_edited: Option<&[locust_core::changes::File]> = s.ledger.as_ref().and_then(|pane| {
                    let meta = pane
                        .snapshot
                        .turn_metas
                        .iter()
                        .filter(|m| m.check_fix.is_none())
                        .nth(ui)?;
                    pane.snapshot.turn_edits.get(&meta.mission_id)?.as_deref()
                });
                let layout_track = move |s: &mut State, r: [f32; 4], tx: &mut Tx| {
                    let rel_top = r[1] - s.prompt_rail.column_rect[1];
                    if !s.prompt_rail.set_top(ui, rel_top) {
                        tx.live_only();
                    }
                };
                let bubble_row = if own_words.trim().is_empty() && !sent_notes.is_empty() {
                    None
                } else {
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
                    Some(
                        row()
                            .w_full()
                            .justify_end()
                            .on_layout_tx(layout_track)
                            .children(edit)
                            .child(row().w_frac(0.7).justify_end().child(bubble)),
                    )
                };
                let bubble_row = if sent_notes.is_empty() {
                    bubble_row.expect("empty notes always have a bubble")
                } else {
                    let folder = selected_conversation(s).and_then(|(t, _)| t.folder.clone());
                    let notes_row = row().w_full().justify_end().child(
                        row()
                            .w_frac(0.7)
                            .justify_end()
                            .child(crate::diff_notes_ui::sent_notes(
                                &sent_notes,
                                turn_edited,
                                folder.as_deref(),
                                p,
                            )),
                    );
                    match bubble_row {
                        Some(b) => col().w_full().gap(6.0).child(b).child(notes_row),
                        None => col().w_full().on_layout_tx(layout_track).child(notes_row),
                    }
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
    if user_index > 0 {
        messages = emit_turn_peers(messages, user_index - 1);
    }
    messages = emit_group_lines_at(
        messages,
        &group_joins,
        &group_leavings,
        active_group_id.as_deref(),
        |bt| bt >= user_index,
        p,
        s,
    );
    messages = messages.children(peer_thread::live_relay_rows(s, p));
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
    if let Some(card) = crate::key_card::card(s, p) {
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

fn thread_group_lines(
    s: &State,
) -> (
    Vec<locust_core::groups::GroupBoundary>,
    Vec<locust_core::groups::GroupLeaving>,
    Option<String>,
) {
    let mut conv_keys = Vec::new();
    let mut push_key = |k: &str| {
        if !k.is_empty() && !conv_keys.iter().any(|x| x == k) {
            conv_keys.push(k.to_owned());
        }
    };
    if let Some(pane) = &s.ledger {
        if let Some(id) = pane.conversation.conversation_id.as_deref() {
            push_key(id);
        }
        push_key(&pane.snapshot.tip);
        for meta in &pane.snapshot.turn_metas {
            push_key(&meta.mission_id);
        }
    } else if let Some((_, conv)) = selected_conversation(s)
        && let Some(id) = conv.conversation_id.as_deref()
    {
        push_key(id);
    }
    let matched = recent_as_missions(s).into_iter().find(|m| {
        conversation_list::conversation_keys(m)
            .iter()
            .any(|k| conv_keys.contains(k))
    });
    if let Some(row) = matched {
        for k in conversation_list::conversation_keys(&row) {
            if !k.is_empty() && !conv_keys.iter().any(|x| x == &k) {
                conv_keys.push(k);
            }
        }
    }
    if conv_keys.is_empty() {
        return (Vec::new(), Vec::new(), None);
    }

    let turn_count = s
        .ledger
        .as_ref()
        .map(|pane| {
            pane.snapshot
                .turn_metas
                .len()
                .max(pane.snapshot.timings.len())
        })
        .unwrap_or(0);
    let starts: Vec<Option<&str>> = (0..turn_count)
        .map(|i| {
            let pane = s.ledger.as_ref()?;
            pane.snapshot
                .turn_metas
                .get(i)
                .map(|m| m.created_at.as_str())
                .filter(|c| !c.is_empty())
                .or_else(|| {
                    pane.snapshot
                        .timings
                        .get(i)
                        .and_then(|t| t.first_at.as_deref())
                })
        })
        .collect();

    let membership = conv_keys.iter().find_map(|k| s.org.members.get(k));
    let group = membership.and_then(|m| s.org.groups.iter().find(|g| g.group_id == m.group_id));
    let active_group_id = group.map(|g| g.group_id.clone());
    let current_boundary = group.and_then(|g| {
        locust_core::groups::group_boundary(
            &starts,
            membership.and_then(|m| m.at.as_deref()),
            &g.name,
            &g.instructions,
        )
    });
    let left: &[org::LeftMembership] = conv_keys
        .iter()
        .find_map(|k| s.org.left.get(k))
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut joins = locust_core::groups::group_joins(&starts, left);
    if let Some(b) = current_boundary {
        joins.push(b);
    }
    let leavings = locust_core::groups::group_leavings(&starts, left);
    (joins, leavings, active_group_id)
}

fn emit_group_lines_at(
    mut messages: El,
    joins: &[locust_core::groups::GroupBoundary],
    leavings: &[locust_core::groups::GroupLeaving],
    active_group_id: Option<&str>,
    pred: impl Fn(usize) -> bool,
    p: &Palette,
    s: &State,
) -> El {
    for (idx, leaving) in leavings.iter().enumerate().filter(|(_, l)| pred(l.before_turn)) {
        messages = messages.child(
            text(format!(
                "{}'s instructions no longer apply from here",
                leaving.group_name
            ))
            .key(format!("group-leaving-{idx}"))
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .wrap(),
        );
    }
    for (idx, join) in joins.iter().enumerate().filter(|(_, j)| pred(j.before_turn)) {
        let toggle_key = format!("group-join:{idx}:{}", join.group_name);
        let open = s.activity_open.get(&toggle_key).copied().unwrap_or(false);
        let toggle_id = toggle_key.clone();
        let mut block = col().w_full().gap(6.0).child(
            row()
                .w_full()
                .center_y()
                .gap(4.0)
                .child(
                    text(format!(
                        "{}'s instructions brief every turn from here",
                        join.group_name
                    ))
                    .key(format!("group-join-line-{idx}"))
                    .font(type_::LABEL)
                    .color(p.muted),
                )
                .child(text("·").font(type_::LABEL).color(p.muted))
                .child(
                    text(if open { "hide" } else { "view" })
                        .key(format!("group-join-toggle-{idx}"))
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .role("button")
                        .on_press(move |s: &mut State| {
                            let cur = s.activity_open.get(&toggle_id).copied().unwrap_or(false);
                            s.activity_open.insert(toggle_id.clone(), !cur);
                        }),
                ),
        );
        if open {
            let mut quote = col()
                .w_full()
                .pl(12.0)
                .gap(4.0)
                .border_sides([0.0, 0.0, 0.0, 2.0], p.hairline)
                .child(
                    text(join.instructions.clone())
                        .font(type_::BODY)
                        .color(p.secondary)
                        .wrap(),
                );
            if let Some(gid) = active_group_id {
                let edit_gid = gid.to_owned();
                quote = quote.child(
                    text("edit in Project settings")
                        .key(format!("group-join-edit-{idx}"))
                        .font(type_::LABEL)
                        .color(p.muted)
                        .role("button")
                        .on_press(move |s: &mut State| {
                            crate::group_settings_dialog::open(s, &edit_gid);
                        }),
                );
            }
            block = block.child(quote);
        }
        messages = messages.child(block);
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread_text(items: &[ledger::Item], s: &mut State) -> String {
        let taken = std::mem::take(s);
        let mut rt = rune::Runtime::new(
            (taken, items.to_vec()),
            |(s, items), _| message_column(items, "Codec", &Palette::new(true), s),
            (800.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let out = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n");
        *s = rt.state.0;
        out
    }

    #[test]
    fn thread_renders_group_join_expandable_instructions_and_leaving_line() {
        let mut s = State::default();
        let gid = "grp_trading".to_owned();
        s.org.groups.push(org::Group::new(
            gid.clone(),
            "Trading".into(),
            "2026-09-16T08:00:00.000Z".into(),
            "Ask before executing.".into(),
            None,
        ));
        s.org.left.insert(
            "m_1".into(),
            vec![org::LeftMembership {
                group_id: gid.clone(),
                name: "Trading".into(),
                instructions: "Ask before executing.".into(),
                at: Some("2026-09-16T11:00:00.000Z".into()),
                until: "2026-09-16T13:00:00.000Z".into(),
            }],
        );
        let items = vec![
            ledger::Item::User("First turn".into()),
            ledger::Item::User("Second turn".into()),
            ledger::Item::User("Third turn".into()),
        ];
        let header = Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Engineer".into(),
            runtime: None,
            model: None,
            account: None,
            busy: false,
            runs_in: "local".into(),
            folder: None,
        };
        let pane = LedgerPane {
            header: header.clone(),
            conversation: Conversation {
                conversation_id: Some("m_1".into()),
                turns: Vec::new(),
            },
            items: items.clone(),
            dir: std::path::PathBuf::new(),
            worker: None,
            snapshot: ledger::Snapshot {
                items: items.clone(),
                turn_metas: vec![
                    ledger::Metadata {
                        mission_id: "m_1".into(),
                        created_at: "2026-09-16T10:00:00.000Z".into(),
                        ..Default::default()
                    },
                    ledger::Metadata {
                        mission_id: "m_2".into(),
                        created_at: "2026-09-16T12:00:00.000Z".into(),
                        ..Default::default()
                    },
                    ledger::Metadata {
                        mission_id: "m_3".into(),
                        created_at: "2026-09-16T14:00:00.000Z".into(),
                        ..Default::default()
                    },
                ],
                tip: "m_3".into(),
                ..Default::default()
            },
            due: 0.0,
            loading: false,
            after_tip: None,
            after_tip_deadline: None,
            backoff: 0.0,
        };
        s.ledger = Some(pane);

        let collapsed = thread_text(&items, &mut s);
        let first_at = collapsed.find("First turn").unwrap();
        let join_at = collapsed
            .find("Trading's instructions brief every turn from here")
            .unwrap();
        let second_at = collapsed.find("Second turn").unwrap();
        let leaving_at = collapsed
            .find("Trading's instructions no longer apply from here")
            .unwrap();
        let third_at = collapsed.find("Third turn").unwrap();
        assert!(first_at < join_at && join_at < second_at);
        assert!(second_at < leaving_at && leaving_at < third_at);
        assert!(collapsed.contains("view"));
        assert!(!collapsed.contains("Ask before executing."));

        // Opening the join note reveals the instructions that applied.
        s.activity_open
            .insert("group-join:0:Trading".into(), true);
        let expanded = thread_text(&items, &mut s);
        assert!(expanded.contains("hide"));
        assert!(expanded.contains("Ask before executing."));

        // If still a member, edit in Project settings is also shown.
        s.org.members.insert(
            "m_1".into(),
            org::GroupMembership {
                group_id: gid,
                at: Some("2026-09-16T15:00:00.000Z".into()),
            },
        );
        if let Some(pane) = s.ledger.as_mut() {
            pane.snapshot.turn_metas.push(ledger::Metadata {
                mission_id: "m_4".into(),
                created_at: "2026-09-16T16:00:00.000Z".into(),
                ..Default::default()
            });
        }
        s.activity_open
            .insert("group-join:1:Trading".into(), true);
        let active = thread_text(&items, &mut s);
        assert!(active.contains("edit in Project settings"));
    }
}
