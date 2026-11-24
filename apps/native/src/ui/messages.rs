use super::*;

pub(super) fn conversation_history_key(s: &State) -> String {
    s.ledger.as_ref().and_then(|pane| pane.conversation.conversation_id.clone())
        .or_else(|| selected_conversation(s).map(|(teammate, conversation)| conversation.conversation_id.clone().unwrap_or_else(|| format!("teammate:{}", teammate.id))))
        .unwrap_or_else(|| "preview".into())
}

/// A transcript's still owner face; nobody's conversation wears the pearl ghost.
fn thread_face(s: &State, key: &str, size: f32, p: &Palette) -> El {
    let owner = selected_conversation(s).map(|(t, _)| t).filter(|t| t.id != LEDGER_TARGET);
    let plain = locust_core::native::FaceLook {
        hue: "pearl".into(), parts: None, bot: Some(("ghost".into(), "eyes".into())),
        bot_screen: None, pet: None,
    };
    let id = owner.map_or(LEDGER_TARGET, |t| t.id.as_str());
    let look = owner.and_then(|t| s.looks.get(&t.id)).or_else(|| owner.is_none().then_some(&plain));
    let (shape, _, hue) = crate::look::faces::bot_of(id, look);
    crate::look::faces::face(s.image_cache.as_deref(), id, "", look, size,
        s.settings_panel.settings.terminal_faces, &crate::look::controls::Look::new(p.dark))
        .key(format!("thread-face-{key}-{shape}-{hue}"))
}

/// Where the ui-th bubble's turn is among the conversation's turns: every turn but the fixes Locust sent back.
fn person_turn_at(metas: &[ledger::Metadata], ui: usize) -> Option<usize> {
    metas.iter().enumerate().filter(|(_, m)| locust_core::conversation_turns::fix_of_turn(m).is_none()).nth(ui).map(|(i, _)| i)
}

/// Thread.tsx, 0.495: said as what happens. "Started without the earlier messages" read as the history gone.
pub(super) const COLD_START_NOTE: &str =
    "A fresh session: the turn before could not be picked up again, so the teammate was given the conversation so far";

/// The conversation's newest turn, when it is a reply that started cold (the host's say, `followsUp.runtimeThreadId`
/// absent) and a turn before it is in the thread: the note under its opening. An edit says its own note instead.
pub(super) fn cold_start_note(s: &State, items: &[ledger::Item], meta: &ledger::Metadata, at: Option<usize>, p: &Palette) -> Option<El> {
    let pane = s.ledger.as_ref()?;
    let newest = pane.snapshot.turn_metas.len().checked_sub(1)?;
    let cold = pane.cold_tip.as_ref().is_some_and(|(tip, cold)| *cold && *tip == meta.mission_id);
    // Only when a turn before it is still on screen: one that gave way to this one, sent again, loses nothing (0.496).
    if !cold || meta.edited || at != Some(newest) || newest == 0 || pane.snapshot.tip != meta.mission_id || !locust_core::sent_again::a_turn_before_is_shown(items) {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    Some(
        row()
            .w_full()
            .wrap()
            .child(text(COLD_START_NOTE).font(l.ty.label).color(l.c("text-muted")).wrap())
            .key(format!("cold-start-note-{}", meta.mission_id)),
    )
}

pub(super) fn message_column(items: &[ledger::Item], name: &str, p: &Palette, s: &State) -> El {
    let mut messages = col().w_full().max_w(super::thread_view::THREAD_MAX).gap(24.0).padding(24.0);
    if let Some(label) = s.ledger.as_ref().and_then(|pane| pane.snapshot.started_label.as_ref()) {
        messages = messages.child(text(format!("Started {label}")).key("mission-started").font(type_::LABEL).color(p.muted));
    }
    if let Some(pane) = &s.ledger {
        let mut ids: Vec<_> = pane.snapshot.turn_metas.iter().map(|meta| meta.mission_id.clone()).collect();
        ids.extend(pane.conversation.conversation_id.iter().cloned());
        ids.push(pane.snapshot.tip.clone());
        if let Some(compare) = sidebar::comparison_of(s, &ids) {
            let id = compare.compare_id.clone();
            let l = crate::look::controls::Look::new(p.dark);
            messages = messages.child(crate::look::controls::button(
                &l, "Open the comparison", crate::look::controls::Tone::Neutral, false,
            ).key("open-thread-comparison").on_press(move |s: &mut State| sidebar::open_comparison(s, &id)));
        }
    }
    let markers = s
        .ledger
        .as_ref()
        .map(|pane| locust_core::time_marker::thread_markers(&pane.snapshot.timings))
        .unwrap_or_default();
    let (group_joins, group_leavings, active_group_id) = thread_group_lines(s);
    let conversation_key = conversation_history_key(s);
    let budget = s.earlier_turn_budgets.get(&conversation_key).copied().unwrap_or(locust_core::thread_view::EARLIER_ROWS_SHOWN);
    let (first_item, hidden_turns) = locust_core::thread_view::newest_turn_window(items, budget);
    if hidden_turns > 0 {
        let press_key = conversation_key.clone();
        let anchor = format!("{}/ledger-item-{first_item}", s.ledger.as_ref().and_then(|pane| pane.conversation.conversation_id.as_deref()).unwrap_or("preview"));
        messages = messages.child(text(format!("Show {hidden_turns} earlier turn{}", if hidden_turns == 1 { "" } else { "s" }))
            .key("show-earlier-turns").font(type_::LABEL).color(p.secondary)
            .role("button").on_press(move |s: &mut State| {
                s.earlier_turn_budgets.insert(press_key.clone(), budget.saturating_add(locust_core::thread_view::EARLIER_ROWS_SHOWN));
            }).on_layout_tx(move |s: &mut State, r, tx| {
                let viewport = s.prompt_rail.viewport_rect;
                // As in Thread's observer, grow the window when its older edge
                // comes within 600 points of the viewport after scrolling up.
                if s.thread_away && viewport[3] > 0.0
                    && r[1] >= viewport[1] - 600.0 && r[1] <= viewport[1] + 600.0
                    && s.earlier_turn_budgets.get(&conversation_key).copied().unwrap_or(locust_core::thread_view::EARLIER_ROWS_SHOWN) == budget
                {
                    s.earlier_turn_budgets.insert(conversation_key.clone(), budget.saturating_add(locust_core::thread_view::EARLIER_ROWS_SHOWN));
                    tx.scroll_key_to(anchor.clone(), 0.0);
                }
            }));
    }
    let mut user_index = hidden_turns;
    let self_teammate_id = selected_conversation(s).map(|(t, _)| t.id.as_str()).unwrap_or("");
    let peer_turns = s.ledger.as_ref().map(|pane| pane.snapshot.turn_metas.iter()
        .map(|meta| peer_thread::messages_for_mission(s, &meta.mission_id)).collect::<Vec<_>>()).unwrap_or_default();
    let peer_cards = locust_core::peer_thread::thread_peer_cards(&peer_turns);
    let emit_turn_peers = |mut messages: El, turn_idx: usize, before_work: bool| -> El {
        let Some(mission_id) = s.ledger.as_ref().and_then(|pane| pane.snapshot.turn_metas.get(turn_idx)).map(|m| m.mission_id.as_str()) else { return messages; };
        // What the turn handed out, under it and above the exchanges after its work (Thread.tsx `HandedOut`).
        if !before_work
            && let Some(card) = peer_turns.get(turn_idx).and_then(|msgs| crate::handed_out::card(s, mission_id, msgs, p))
        {
            messages = messages.child(card);
        }
        for card in peer_cards.iter().filter(|c| c.turn_index == turn_idx && c.before_work == before_work) {
            for view in peer_thread::cards_for_mission(s, self_teammate_id, mission_id, &card.messages, p) {
                messages = messages.child(view.key(card.key.clone()));
            }
        }
        messages
    };
    messages = emit_turn_peers(messages, hidden_turns, true);
    // One face per run of speech, Copy once under the turn's last reply (Thread.tsx continuesSpeech, lastSpeech).
    let said = |i: usize| matches!(&items[i], ledger::Item::Assistant { text, .. } if !locust_core::protocol_tags::shown_text(text).is_empty());
    let last_speech = locust_core::speech_runs::last_speech(items, turn_running(s), said);
    let pieces = locust_core::steps_fold::thread_pieces(items, turn_running(s));
    let drawn_alone: std::collections::HashSet<usize> =
        pieces.iter().filter_map(|piece| if let locust_core::steps_fold::Piece::Item(i) = piece { Some(*i) } else { None }).collect();
    let mut steps_at: std::collections::HashMap<usize, Vec<locust_core::steps_fold::StepsGroup>> = std::collections::HashMap::new();
    for piece in &pieces {
        if let locust_core::steps_fold::Piece::Steps { run, group } = piece {
            steps_at.entry(*run).or_default().push(group.clone());
        }
    }
    // A turn stopped before it said anything and then sent again with the same words gives way to the one sent again:
    // one message, not two (Thread.tsx `sentAgainBy`, 0.496).
    let given_way = locust_core::sent_again::given_way(items);
    for (index, item) in items.iter().enumerate().skip(first_item) {
        for marker in markers.iter().filter(|m| m.before_item == index) {
            messages = messages.child(time_marker::view(marker, p));
        }
        if let Some(&(at, _)) = given_way.iter().find(|(at, end)| (*at..*end).contains(&index)) {
            if at == index {
                user_index += 1;
            }
            continue;
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
        // The turn in the order it happened: the steps lines of a run drawn where the run began (a message with nothing
        // on screen does not split it), and an item no piece holds -- that message, a row the host added after the run
        // ended (in the foot's files) -- not drawn on its own.
        if let Some(lines) = steps_at.get(&index) {
            for line in lines {
                let held: Vec<ledger::Item> = line.items.iter().map(|&i| items[i].clone()).collect();
                messages = messages.child(activity_group(&held, p, s, conversation));
            }
            continue;
        }
        if !drawn_alone.contains(&index) {
            continue;
        }
        let content = match item {
            ledger::Item::Handoff { from, to } => handoff_divider::view(Some(from), to, p).unwrap_or_else(col),
            ledger::Item::Trace(trace) => {
                // Thread.tsx TurnFoot: a finished turn closes with its files card above the totals, and the line need
                // not say how many files again. A turn still going has no foot.
                let start = items[..index].iter().rposition(|i| matches!(i, ledger::Item::User(_))).map_or(0, |at| at + 1);
                let finished = !(turn_running(s) && items[index + 1..].iter().all(|i| !matches!(i, ledger::Item::User(_))));
                // Thread.tsx `if (!item.finished) return null`: the foot is what the turn came to; a turn still going
                // has not come to it, so nothing of it is drawn and nothing moves in when it ends.
                if !finished {
                    continue;
                }
                let mission = s.ledger.as_ref().and_then(|pane| pane.snapshot.turn_metas.get(user_index.saturating_sub(1))).map(|m| m.mission_id.as_str());
                let foot = turn_foot_card(&items[start..index], p, s, &format!("{conversation}/foot/{index}"), mission);
                // Beside the browser (~418px, the-thread-beside-the-browser-keeps-its-parts-whole, 0.867) the line wraps
                // by whole parts, each with the "·" before it, and a part longer than the line gives way to its own
                // ellipsis rather than being cut mid-word at the column's edge.
                let mut line = row().w_full().flex_wrap().key(format!("{conversation}/trace/{index}"));
                let file_count = locust_core::activity_entries::file_count(&trace.changed_paths, selected_conversation(s).and_then(|(t,_)|t.folder.as_deref()));
                let mut first = true;
                for segment in &trace.segments {
                    if segment.key == "files" && foot.is_some() { continue; }
                    let shown = if segment.key == "files" && file_count > 0 { format!("{file_count} file{}", if file_count==1 {""} else {"s"}) } else {segment.text.clone()};
                    let mut part = row().shrink().key(format!("{conversation}/trace/{index}/{}", segment.key)).pr(5.0);
                    if !first { part = part.child(text("· ").font(type_::LABEL).color(p.muted)); }
                    first = false;
                    part = part.child(text(shown).font(type_::LABEL).color(if segment.tone.as_deref() == Some("amber") {p.amber} else {p.muted}).shrink().truncate());
                    line = line.child(part);
                }
                let mut body = col().w_full().gap(4.0).children(foot).child(line);
                if !trace.notices.is_empty() {
                    let key = format!("{conversation}/trace/{index}/notices");
                    let open = s.activity_open.get(&key).copied().unwrap_or(false);
                    body = body.child(disclosure(key, "Runtime notices".into(), open, p));
                    if open { for n in &trace.notices { body = body.child(text(format!("{} · {}", n.source, n.message)).font(type_::LABEL).color(p.muted).w_full().wrap()); } }
                }
                body
            }
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
                    messages = emit_turn_peers(messages, ui - 1, false);
                    messages = messages.children(crate::memory_card::view(s, s.ledger.as_ref().and_then(|pane| person_turn_at(&pane.snapshot.turn_metas, ui - 1).and_then(|i| pane.snapshot.turn_metas.get(i))).map(|m| m.mission_id.as_str()), p));
                    messages = emit_turn_peers(messages, ui, true);
                }
                // The ui-th bubble's own turn: a fix turn Locust sent draws a line, not a bubble, so it is passed over.
                let at = s.ledger.as_ref().and_then(|pane| person_turn_at(&pane.snapshot.turn_metas, ui));
                let this_meta = at.and_then(|i| s.ledger.as_ref()?.snapshot.turn_metas.get(i));
                let before_meta = at.and_then(|i| i.checked_sub(1)).and_then(|i| s.ledger.as_ref()?.snapshot.turn_metas.get(i));
                if let Some(to_meta) = this_meta
                    && let Some(div) = handoff_divider::view(before_meta, to_meta, p)
                {
                    messages = messages.child(div.key(format!("{key}-handoff")));
                }
                // Into the runtime's own terminal before its first turn there; back in Locust before the next of its own.
                if let Some(seam) = locust_core::conversation_turns::terminal_seam_before(
                    before_meta.and_then(locust_core::conversation_turns::in_terminal),
                    this_meta.and_then(locust_core::conversation_turns::in_terminal),
                ) {
                    messages = messages.child(crate::terminal_divider::view(&seam, p).key(format!("{key}-terminal-seam")));
                }
                // An edit this turn began, or one that touched it, says so above it (0.498).
                if let Some(pane) = s.ledger.as_ref()
                    && let Some(meta) = person_turn_at(&pane.snapshot.turn_metas, ui).and_then(|i| pane.snapshot.turn_metas.get(i))
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
                // Where this turn began, said where it begins (Thread.tsx): beside the time marker that opens it, above
                // the work it explains, in the note register -- never at the end, where live and pending things are.
                if let Some(note) = this_meta.and_then(|meta| cold_start_note(s, items, meta, at, p)) {
                    messages = messages.child(note);
                }
                if this_meta.is_some_and(|m| locust_core::conversation_turns::turn_line(m).is_none()) { continue; }
                let hit = s.prompt_rail.hit_key.as_deref() == Some(key.as_str());
                // Notes on a diff came as one block after the person's words: the bubble shows the words, the notes go under it.
                let kind = this_meta.and_then(|m| m.started_by_kind.as_deref());
                let body = if kind.is_none() { locust_core::conversation_turns::tagged_words(body) } else { body };
                let (own_words, sent_notes) = locust_core::diff_notes::split_diff_notes(body);
                let edit = crate::edit_message::edit_control(s, ui, &own_words, !sent_notes.is_empty(), p);
                let turn_edited: Option<&[locust_core::changes::File]> = s.ledger.as_ref().and_then(|pane| {
                    let meta = pane.snapshot.turn_metas.get(person_turn_at(&pane.snapshot.turn_metas, ui)?)?;
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
                    // `.lc-bubble` is set in `--lc-text-said`, which grows with the reply size.
                    let said = locust_core::workspace_settings::ask_sizes(&s.settings_panel.settings.reply_size).said;
                    let mut bubble = text(own_words)
                        .font(rune::TextStyle::new(said, 400.0).lh(said * 21.0 / 14.5))
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
                let mut msg = col().w_full().gap(8.0);
                if !locust_core::speech_runs::continues_speech(items, index, said) {
                    msg = msg.child(row().gap(8.0).center_y()
                        .child(thread_face(s, &key, 24.0, p))
                        .child(text(name).font(type_::LABEL).color(p.muted)));
                }
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
                        .child(reply_actions::reply_in_thread(s, &shown, locust_core::thread_view::streaming(*r#final, running), last_speech == Some(index), p));
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
            // A turn that only planned (`touchedNothing`): its plan is the answer, a numbered list; with any step in the
            // turn the plan is a record of work and keeps its outcomes card (a-plan-shows-what-happened-to-it).
            ledger::Item::Plan { steps, .. } if {
                let from = items[..index].iter().rposition(|i| matches!(i, ledger::Item::User(_))).map_or(0, |at| at + 1);
                let to = items[index..].iter().position(|i| matches!(i, ledger::Item::User(_))).map_or(items.len(), |n| index + n);
                locust_core::steps_fold::touched_nothing(&items[from..to])
            } => plan_answer(steps, &key, p).unwrap_or_else(col),
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
                let newest = !items[index + 1..].iter().any(|i| matches!(i, ledger::Item::User(_)));
                if !newest && locust_core::sent_again::stopped_before_saying(&items[from..=index]) {
                    // A kept turn stopped before it said anything had nothing under it, as if still waiting (Claude
                    // Code: "Interrupted"); the receipt is the newest turn's.
                    text(locust_core::sent_again::STOPPED_BEFORE_IT_REPLIED).key(format!("{key}-stopped-before-it-replied")).font(type_::LABEL).color(p.muted).w_full()
                } else {
                    // The time of the turn's last event, as Electron's stoppedAt reads it from the record.
                    let stopped_at = s.ledger.as_ref().and_then(|pane| pane.snapshot.timings.iter().rev().find(|t| t.before_item <= index))
                        .and_then(|t| t.last_at.as_deref()).map(locust_core::time_marker::clock_time).filter(|at| at != "--:--");
                    let card = crate::cancellation_card::card(&summary, by_person, stopped_at.as_deref(), p);
                    match (newest && !turn_running(s) && locust_core::cancellation::stopped_before_any_tool(&summary))
                        .then(|| items[..from].iter().rev().find_map(|i| if let ledger::Item::User(words) = i { Some(words.clone()) } else { None }))
                        .flatten()
                    {
                        Some(words) => col().w_full().gap(12.0).child(card).child(send_again_row(words, summary.tools_reported_when_done, p)),
                        None => card,
                    }
                }
            }
            ledger::Item::Limit { text: body, .. } => text(body).font(type_::LABEL).color(p.muted).w_full().wrap_anywhere(),
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
    messages = emit_turn_peers(messages, user_index.saturating_sub(1), false);
    messages = messages.children(crate::memory_card::view(s, s.ledger.as_ref().and_then(|pane| pane.snapshot.turn_metas.last()).map(|m| m.mission_id.as_str()), p));
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
    if live_sweeping(s)
        && let Some(live) = s.ledger.as_ref().and_then(|pane| pane.snapshot.live.as_ref())
    {
        let elapsed = chrono::DateTime::parse_from_rfc3339(&live.started_at).ok()
            .map(|at| (chrono::Utc::now().timestamp_millis() - at.timestamp_millis()).max(0) as u64);
        let words = match (&live.action, live.register.as_str()) {
            // What the step under way is doing, in words: "Running npm test", the plan's "Step 2" (live-step `action`).
            (Some(action), _) => action.clone(),
            (None, "connector") => format!("Using a connector · {}", live.label),
            (None, "tool") => format!("Using a tool · {}", live.label),
            // The live word, with its one ellipsis (sweepText): "Working…".
            (None, _) => locust_core::thread_view::sweep_text(&live.label),
        };
        let headline = live.detail.as_ref().map_or_else(|| words.clone(), |d| format!("{words} · {d}"));
        // Beside the browser the column is ~418px (the-thread-beside-the-browser-keeps-its-parts-whole, 0.867): the
        // words give way to their ellipsis, and the plan's "step 2 of 3" and the time stay whole after them
        // (lc-livestep__label `min-width: 0; overflow: hidden`, lc-livestep__peek `flex: none`).
        let mut line = row().w_full().center_y()
            .child(thread_face(s, "live", 34.0, p))
            .child(el().w(8.0))
            // The same live projection supplies the register, words and orb.
            .children(crate::look::orb::view(
                "live-step-orb".into(), live.orb, crate::look::orb::Surface::Live, s, p.dark,
            ).map(|orb| orb.mr(6.0)))
            // The headline is the label's hover title too, for a row that ellipsizes (lc-livestep__label's `title`).
            // The word sweeps (`.lc-sweep`): a painted word with a highlight passing over it, so it is never stale text.
            .child(crate::working_shimmer::live_word(&words, p.muted, crate::look::controls::Look::new(p.dark).c("text-name"), s.reduced_motion.get())
                .key("live-step-label").shrink().truncate().help(headline));
        if let Some(step) = &live.detail {
            line = line.child(text(format!(" · {step}")).key("live-step-peek").font(type_::LABEL).color(p.muted));
        }
        if let Some(ms) = elapsed {
            line = line.child(text(format!(" · {}", ledger::duration_text(ms))).key("live-step-time").font(type_::LABEL).color(p.muted));
        }
        messages = messages.child(line);
    }
    messages = failed_turn_rows(messages, items, s, p);
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

/// Under a run stopped before any tool ran: Send again, the same words sent again (Thread.tsx `onSendAgain`). On OpenCode
/// the card above already says to check the folder, so the line beside the press says nothing (0.696).
fn send_again_row(words: String, tools_reported_when_done: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut line = row().key("send-again").w_full().gap(10.0).center_y();
    if !tools_reported_when_done {
        line = line.child(text(locust_core::sent_again::NOTHING_TO_REPEAT).font(type_::LABEL).color(p.muted).grow(1.0).shrink().wrap());
    }
    line.child(
        crate::look::controls::button(&l, "Send again", crate::look::controls::Tone::Neutral, false)
            .key("send-again-button")
            .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                // The same words again; what is in the box stays.
                let kept = s.draft.text.clone();
                s.draft.replace_text(words.clone());
                submit_composer(s, tx, false);
                s.draft.replace_text(kept);
            }),
    )
}

/// Run again with edits allowed: the mode switched to Accept edits on the message box, and the same words sent in it
/// (App.tsx `onRunWithEdits`: `setMode('accept-edits')`, `startMission(prompt, 'accept-edits')`). What is in the box stays.
fn run_with_edits_row(prompt: String, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    row().key("run-with-edits").w_full().gap(10.0).center_y()
        .child(text(locust_core::thread_view::MODE_REFUSED_SENTENCE).font(type_::LABEL).color(p.muted).grow(1.0).shrink().wrap())
        .child(
            crate::look::controls::button(&l, "Run again with edits allowed", crate::look::controls::Tone::Neutral, false)
                .key("run-with-edits-button")
                .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                    s.composer_mode = Some("accept-edits".to_owned());
                    let kept = s.draft.text.clone();
                    s.draft.replace_text(prompt.clone());
                    submit_composer(s, tx, false);
                    s.draft.replace_text(kept);
                }),
        )
}

/// Whether the live line is on screen. The window refreshes its orb and sweeping word while it is.
pub(super) fn live_sweeping(s: &State) -> bool {
    locust_core::thread_view::live_visible(turn_running(s), s.pending_approval.is_some())
        && s.ledger.as_ref().is_some_and(|pane| pane.snapshot.live.is_some())
}

/// Thread.tsx under the current turn: a never-started run of the person's own words offers Run it again
/// (`runAgainOffer(liveRun, running, ...)`), a host-started one says why it does not, and a damaged record says so
/// where it is read.
fn failed_turn_rows(mut messages: El, items: &[ledger::Item], s: &State, p: &Palette) -> El {
    use locust_core::thread_view::{self as tv, RunAgainOffer};
    let Some(pane) = s.ledger.as_ref() else { return messages };
    let snap = &pane.snapshot;
    if let Some(meta) = snap.turn_metas.last() {
        let from = items.iter().rposition(|i| matches!(i, ledger::Item::User(_))).unwrap_or(0);
        let turn = tv::RunAgainTurn {
            prompt: &meta.prompt,
            started_by_kind: meta.started_by_kind.as_deref(),
            handoff: items[from..].iter().any(|i| matches!(i, ledger::Item::Handoff { .. })),
            continues_reason: meta.continues_reason.as_deref(),
        };
        let phase = if snap.failed { "failed" } else if snap.running { "running" } else { "completed" };
        // A run its mode refused did what the mode allows: Run it again would refuse again for ever, so the offer is the
        // mode that works (Thread.tsx `refusedByMode`, Sol's beta finding 3).
        let refused = snap.failed
            && !(snap.running || turn_running(s))
            && tv::mode_refused_a_tool(items[from..].iter().rev().find_map(|i| match i {
                ledger::Item::Note { level, text } if level == "error" => Some(text.as_str()),
                _ => None,
            }));
        if refused && !meta.prompt.trim().is_empty() {
            messages = messages.child(run_with_edits_row(meta.prompt.clone(), p));
        }
        match tv::run_again_offer(&turn, phase, snap.never_started, snap.running || turn_running(s)).filter(|o| !(refused && matches!(o, RunAgainOffer::RunAgain))) {
            Some(RunAgainOffer::Note(note)) => {
                messages = messages.child(text(note).key("run-again-note").font(type_::LABEL).color(p.muted).w_full().wrap());
            }
            Some(RunAgainOffer::RunAgain) => {
                let l = crate::look::controls::Look::new(p.dark);
                let prompt = meta.prompt.clone();
                messages = messages.child(
                    row().key("run-again").w_full().gap(10.0).center_y()
                        .child(text(tv::RUN_AGAIN_NOTHING_STARTED).font(type_::LABEL).color(p.muted).grow(1.0).shrink().wrap())
                        .child(
                            crate::look::controls::button(&l, "Run it again", crate::look::controls::Tone::Neutral, false)
                                .key("run-again-button")
                                .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                                    // The same words again, as Electron's startMission(liveRun.prompt); what is in the box stays.
                                    let kept = s.draft.text.clone();
                                    s.draft.replace_text(prompt.clone());
                                    submit_composer(s, tx, false);
                                    s.draft.replace_text(kept);
                                }),
                        ),
                );
            }
            None => {}
        }
    }
    if snap.damaged {
        messages = messages.child(text(tv::DAMAGED_RECORD_NOTE).key("damaged-record-note").font(type_::LABEL).color(p.muted).w_full().wrap());
    }
    messages
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
#[path = "speech_run_tests.rs"]
mod speech_run_tests;
#[cfg(test)]
#[path = "run_again_tests.rs"]
mod run_again_tests;
#[cfg(test)]
#[path = "thread_order_tests.rs"]
mod thread_order_tests;
#[cfg(test)]
#[path = "sent_again_tests.rs"]
mod sent_again_tests;
#[cfg(test)]
#[path = "mode_refusal_tests.rs"]
mod mode_refusal_tests;

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

    /// a-turn-sent-back-to-fix-is-said-as-locusts.test.tsx, the whole thread in one frame: the person's turn, an
    /// earlier fix turn and the one on screen. Each fix is a line, never a bubble, and what was sent waits behind a click.
    #[test]
    fn says_what_failed_and_which_fix_it_is_for_an_earlier_turn_and_the_one_on_screen_with_what_was_sent_a_click_away() {
        let sent = |line: &str, fix: u32| locust_core::edit_check::check_fix_prompt("npm test", &[line.to_owned()], false, fix, 2);
        let said = |id: &str, body: &str| ledger::Item::Assistant { item_id: id.into(), text: body.into(), r#final: true };
        let fix = |n: u32, line: &str| ledger::Item::CheckFix { command: "npm test".into(), fix: n, of: 2, prompt: sent(line, n) };
        let items = vec![
            ledger::Item::User("Add a sum function.".into()),
            said("m_person_m", "Added sum."),
            fix(1, "FAIL adds two numbers"),
            said("m_fix1_m", "Fixed the off-by-one."),
            fix(2, "FAIL adds three numbers"),
            said("m_fix2_m", "Fixed the three-number case."),
        ];
        // Electron's fixture has no teammate: "Locust" is how a thread with none is drawn (`check_fix_line` names nobody).
        let draw = |s: State| {
            let mut rt = rune::Runtime::new((s, items.clone()), |(s, items), _| message_column(items, "Locust", &Palette::new(true), s), (800.0, 1600.0), rune::Env::new(true, 1.0));
            rt.frame(0.0);
            rt.elements()
        };
        let all = draw(composition_state(None, false));
        let shown = all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
        assert!(shown.contains("Add a sum function."), "{shown}");
        assert!(shown.contains("npm test failed, so Locust sent it back"), "{shown}");
        assert!(shown.contains("· fix 1 of 2"), "{shown}");
        assert!(shown.contains("npm test still failed, so Locust sent it back again"), "{shown}");
        assert!(shown.contains("· fix 2 of 2"), "{shown}");
        assert_eq!(shown.matches("What Locust sent").count(), 2, "{shown}");
        // One bubble in the whole thread, the person's: what was sent is the fold-out's, never a bubble.
        let bubble = Palette::new(true).your_bubble;
        let bubbles: Vec<_> = all.iter().filter(|e| e.fill == bubble && e.text.is_some()).filter_map(|e| e.text.clone()).collect();
        assert_eq!(bubbles, vec!["Add a sum function.".to_owned()]);
        assert!(!shown.contains("FAIL adds three numbers"), "what was sent waits behind the click: {shown}");
        // A click away: the second fix's fold-out holds the failing line it sent.
        let mut s = composition_state(None, false);
        s.open_check_fix_prompts.insert("m_1/ledger-item-4".into());
        let opened = draw(s).iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
        assert!(opened.contains("FAIL adds three numbers"), "{opened}");
    }

    #[test]
    fn draws_the_tagged_message_as_the_bubble_without_the_context_after_it() {
        let prompt = "Can you double-check the tax rate?\n\n(You were tagged in Wren's conversation. Wren's latest answer there, for context:)\n\nI changed TAX_RATE.";
        let shown = thread_text(&[ledger::Item::User(prompt.into())], &mut composition_state(None, false));
        assert!(shown.contains("Can you double-check the tax rate?"), "{shown}");
        assert!(!shown.contains("You were tagged") && !shown.contains("TAX_RATE"), "{shown}");
        let shown = thread_text(&[ledger::Item::User("Can you double-check the tax rate?".into())], &mut composition_state(Some("tag"), false));
        assert!(shown.contains("Can you double-check the tax rate?"), "{shown}");
    }

    fn runtime_reply(body: &str) -> (String, usize) {
        let mut rt = rune::Runtime::new(
            body.to_owned(),
            |body, _| reply_actions::reply(&State::default(), body, false, &Palette::new(true)),
            (800.0, 800.0), rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let elements = rt.elements();
        let notes = elements.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("runtime-gap-"))).count();
        (elements.into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n"), notes)
    }

    // a-runtime-gap-is-not-the-teammate.test.tsx: exact Electron inputs.
    #[test]
    fn draws_the_marker_as_a_note() {
        let (shown, notes) = runtime_reply("before the gap\n\n<truncated 259 bytes>\n\nafter the gap");
        assert_eq!(notes, 1);
        assert!(shown.contains("259 bytes the runtime did not keep"), "{shown}");
        assert!(!shown.contains("<truncated 259 bytes>"), "{shown}");
    }

    #[test]
    fn keeps_every_other_word_of_the_reply() {
        let (shown, _) = runtime_reply("before the gap\n\n<truncated 259 bytes>\n\nafter the gap");
        assert!(shown.contains("before the gap") && shown.contains("after the gap"), "{shown}");
    }

    #[test]
    fn leaves_a_sentence_that_merely_mentions_truncation_alone() {
        let (shown, notes) = runtime_reply("The log said <truncated 259 bytes> which is why the tail is missing.");
        assert_eq!(notes, 0);
        assert!(shown.contains("which is why the tail is missing"), "{shown}");
    }

    #[test]
    fn runtime_gaps_preserve_fenced_code_and_split_single_newline_prose() {
        let (shown, notes) = runtime_reply("```text\n<truncated 259 bytes>\n```\nbefore\n<truncated 12 bytes>\nafter");
        assert_eq!(notes, 1);
        assert!(shown.contains("<truncated 259 bytes>"), "{shown}");
        assert!(shown.contains("12 bytes the runtime did not keep"), "{shown}");
        assert!(shown.contains("before") && shown.contains("after"), "{shown}");
    }

    // a-teammate-can-hand-you-a-file.test.ts: one native test per Electron case.
    const HANDED_REPLY: &str = "Here is the rollup.\n\n<locust-file>\ndocs/report.md :: the rollup you asked for\n</locust-file>";
    fn handed_reply(text: &str, final_: bool, running: bool) -> (String, usize) {
        let event = serde_json::json!({"id":"e-msg1", "runId":"r", "missionId":"m", "sequence":1,
            "occurredAt":"2026-09-19T10:00:00.000Z", "sourceAdapter":"opencode", "type":"message.delta",
            "payload":{"itemId":"msg1", "operation":"replace", "text":text, "final":final_}});
        let mut transcript = ledger::Transcript::default();
        transcript.apply(vec![serde_json::json!({"recordType":"mission.event", "event":event})]);
        let mut rt = rune::Runtime::new(
            (composition_state(None, running), transcript.items),
            |(s, items), _| message_column(items, "Codec", &Palette::new(true), s),
            (800.0, 800.0), rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let elements = rt.elements();
        let files = elements.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("handed-file-") && !k.starts_with("handed-file-copy-") && !k.starts_with("handed-file-save-"))).count();
        let text = elements.into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
        (text, files)
    }
    #[test]
    fn draws_the_file_as_its_own_item_with_the_path_and_the_note() {
        let (shown, files) = handed_reply(HANDED_REPLY, true, false);
        assert_eq!(files, 1);
        assert!(shown.contains("docs/report.md"), "{shown}");
        assert!(shown.contains("the rollup you asked for"), "{shown}");
    }
    #[test]
    fn shows_the_person_the_answer_and_not_the_protocol() {
        let (shown, files) = handed_reply(HANDED_REPLY, true, false);
        assert_eq!(files, 1);
        assert!(shown.contains("Here is the rollup."), "{shown}");
        assert!(!shown.contains("locust-file"), "{shown}");
    }
    #[test]
    fn still_draws_the_file_when_the_block_was_the_whole_reply() {
        let (shown, files) = handed_reply("<locust-file>\ndocs/report.md\n</locust-file>", true, false);
        assert_eq!(files, 1);
        assert!(shown.contains("docs/report.md"), "{shown}");
        assert!(!shown.contains("locust-file"), "{shown}");
        assert!(!shown.contains("Here is the rollup."));
    }
    #[test]
    fn waits_for_the_message_to_finish_before_drawing_a_button() {
        let (shown, files) = handed_reply(HANDED_REPLY, false, true);
        assert_eq!(files, 0);
        assert!(!shown.contains("docs/report.md"), "{shown}");
        assert!(!shown.contains("the rollup you asked for"), "{shown}");
    }
    #[test]
    fn leaves_an_ordinary_reply_with_no_file_item() {
        let (shown, files) = handed_reply("I wrote docs/report.md for you.", true, false);
        assert_eq!(files, 0);
        assert!(shown.contains("I wrote docs/report.md for you."), "{shown}");
        assert!(!shown.contains("Save a copy"), "{shown}");
    }

    // a-trimmed-live-turn-says-so-on-screen.test.tsx: one test per case, drawn in the window.
    fn shown_after_steps(count: u64) -> String {
        let mut t = ledger::Transcript::default();
        for from in (1..=count).step_by(500) {
            let batch = (from..=count.min(from + 499)).map(|sequence| serde_json::json!({"recordType":"mission.event","event":{
                "id": format!("e{sequence}"), "runId":"run", "missionId":"mission", "sequence": sequence,
                "occurredAt":"2026-10-05T12:00:00.000Z", "sourceAdapter":"claude", "type":"step.completed",
                "payload":{"stepKind":"turn","evidence":{"redacted":true}}}})).collect();
            t.apply(batch);
        }
        thread_text(&t.items, &mut composition_state(None, true))
    }
    #[test]
    fn shows_the_omission_notice_once_when_a_running_turn_exceeds_3000_events() {
        assert_eq!(shown_after_steps(3001).matches(ledger::TRIMMED_TURN_LINE).count(), 1);
    }
    #[test]
    fn shows_no_omission_notice_below_or_exactly_at_3000_events() {
        assert!(!shown_after_steps(739).contains(ledger::TRIMMED_TURN_LINE));
        assert!(!shown_after_steps(3000).contains(ledger::TRIMMED_TURN_LINE));
    }

    #[test]
    fn the_trace_and_its_folded_notices_reach_the_headless_thread() {
        let events = vec![serde_json::json!({"sequence":1,"type":"run.started","occurredAt":"2026-11-01T00:00:00Z","payload":{}}),
            serde_json::json!({"sequence":2,"sourceAdapter":"claude","type":"adapter.diagnostic","occurredAt":"2026-11-01T00:00:01Z","payload":{"code":"codex.setup","level":"info","message":"Skill descriptions were shortened."}}),
            serde_json::json!({"sequence":3,"type":"tool.completed","occurredAt":"2026-11-01T00:00:02Z","payload":{"itemId":"a","name":"shell","command":"ls","exitCode":0}}),
            serde_json::json!({"sequence":4,"type":"run.completed","occurredAt":"2026-11-01T00:00:04Z","payload":{}})];
        let mut t = ledger::Transcript::default();
        t.apply(events.into_iter().map(|event| serde_json::json!({"recordType":"mission.event","event":event})).collect());
        let index = t.items.iter().position(|i| matches!(i, ledger::Item::Trace(_))).expect("ledger trace");
        let mut s = State::default();
        let folded = thread_text(&t.items, &mut s);
        assert!(folded.contains("4s") && folded.contains("ran ls"), "{folded}");
        assert!(!folded.contains("Skill descriptions were shortened."), "{folded}");
        s.activity_open.insert(format!("preview/trace/{index}/notices"), true);
        let opened = thread_text(&t.items, &mut s);
        assert!(opened.contains("claude · Skill descriptions were shortened."), "{opened}");
    }

    // case: wraps a turn's closing line by whole parts instead of cutting it at the Copy button
    #[test]
    fn wraps_a_turns_closing_line_by_whole_parts_instead_of_cutting_it_at_the_copy_button() {
        // the-thread-beside-the-browser-keeps-its-parts-whole.test.ts (0.867): "3 exited non-zero · 4 o" at ~418px.
        let seg = |key: &str, text: &str| locust_core::activity_trace::Segment { key: key.into(), text: text.into(), tone: None };
        let trace = locust_core::activity_trace::Trace {
            segments: vec![seg("took", "12m 40s"), seg("ran", "ran 23 commands"), seg("failed", "3 exited non-zero"), seg("other", "4 other tool calls"), seg("read", "read 18 files"), seg("search", "searched 6 times")],
            notices: vec![],
            changed_paths: vec![],
        };
        fn drawn(items: &[ledger::Item], width: f32) -> Vec<(String, [f32; 4])> {
            let draw: fn(&(State, Vec<ledger::Item>), &mut rune::Cx<(State, Vec<ledger::Item>)>) -> El = if width < 500.0 {
                |(s, items), _| row().child(col().w(418.0).child(message_column(items, "Codec", &Palette::new(true), s)))
            } else {
                |(s, items), _| row().child(col().w(1400.0).child(message_column(items, "Codec", &Palette::new(true), s)))
            };
            let mut rt = rune::Runtime::new((State::default(), items.to_vec()), draw, (1600.0, 900.0), rune::Env::new(true, 1.0));
            rt.frame(0.0);
            rt.elements().into_iter().filter_map(|e| Some((e.key?, e.rect))).filter(|(k, _)| k.contains("/trace/")).collect()
        }
        let items = [ledger::Item::User("go".into()), ledger::Item::Trace(trace)];
        let (narrow, wide) = (drawn(&items, 418.0), drawn(&items, 1400.0));
        let parts: Vec<_> = narrow.iter().filter(|(k, _)| k.matches('/').count() >= 3).collect();
        assert_eq!(parts.len(), 6, "{narrow:?}");
        for (key, [x, _, w, _]) in &parts {
            assert!(x + w <= 418.5, "{key} ends at {} in a 418px column", x + w);
            // Whole: as wide as with room to spare.
            let whole = wide.iter().find(|(k, _)| k == key).unwrap().1[2];
            assert!((w - whole).abs() < 0.5, "{key} is {w}px, {whole}px whole");
        }
        let lines: std::collections::BTreeSet<i32> = parts.iter().map(|(_, r)| r[1].round() as i32).collect();
        assert!(lines.len() > 1, "the line wrapped: {parts:?}");
    }

    // case: starts a new line each time the model thinks, led by what it thought about (drawn in the window)
    #[test]
    fn the_thread_draws_one_steps_line_per_thought_that_says_something() {
        let thought = |id: &str, words: &str| ledger::Item::Thought { id: id.into(), text: words.into(), duration_ms: Some(1_000) };
        let call = |id: &str, command: &str| ledger::Item::Tool {
            name: "shell".into(), status: ledger::Status::Done, summary: command.into(),
            detail: Box::new(ledger::ToolDetail { id: id.into(), arguments: command.into(), finished: true, ..Default::default() }),
        };
        let items = vec![
            thought("r1", "**Inspecting the config**\n\nWhere the port is set."), call("c1", "cat config.json"),
            thought("r2", "**Running the tests**\n\nThen the suite."), call("c2", "npm test"),
        ];
        let shown = thread_text(&items, &mut State::default());
        assert!(shown.contains("Inspecting the config") && shown.contains("Running the tests"), "{shown}");
    }

    // case: a wordless thought folds into its line, which reads as its moves (drawn in the window)
    #[test]
    fn the_thread_draws_a_wordless_stretch_as_one_line_of_its_moves() {
        let thought = |id: &str| ledger::Item::Thought { id: id.into(), text: String::new(), duration_ms: Some(3_800) };
        let call = |id: &str, command: &str| ledger::Item::Tool {
            name: "run_command".into(), status: ledger::Status::Done, summary: command.into(),
            detail: Box::new(ledger::ToolDetail { id: id.into(), arguments: command.into(), finished: true, ..Default::default() }),
        };
        let items = vec![thought("t1"), call("c1", "Get-ChildItem"), thought("t2"), call("c2", "Get-Content a.md"), thought("t3"), call("c3", "git status")];
        let shown = thread_text(&items, &mut State::default());
        assert_eq!(shown.matches("Listed a folder, read a file, ran a command").count(), 1, "{shown}");
    }

    fn plan_of(count: usize, state: &str) -> ledger::Item {
        let steps = (0..count).map(|i| ledger::PlanStep { text: format!("step {}", i + 1), state: state.into() }).collect();
        ledger::Item::Plan { id: "plan".into(), steps, finished: true, stopped: false }
    }
    /// The thread's texts and keys for these items.
    fn thread_drawn(items: &[ledger::Item]) -> (Vec<String>, Vec<String>) {
        let mut rt = rune::Runtime::new(
            (State::default(), items.to_vec()),
            |(s, items), _| message_column(items, "Codec", &Palette::new(true), s),
            (800.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let elements = rt.elements();
        (elements.iter().filter_map(|e| e.text.clone()).collect(), elements.iter().filter_map(|e| e.key.clone()).collect())
    }

    // case: is an ordered list, not a card
    // case: numbers the steps instead of giving them states
    // case: has no header at all — not the label, not the counter
    // case: is marked as having touched nothing, so it draws as an answer (drawn)
    #[test]
    fn a_turn_that_only_planned_draws_its_plan_as_a_numbered_answer_with_no_header() {
        let (texts, keys) = thread_drawn(&[ledger::Item::User("plan it".into()), plan_of(5, "pending")]);
        assert!(keys.iter().any(|k| k.ends_with("/plan-answer")), "{keys:?}");
        let ordinals: Vec<&String> = keys.iter().filter(|k| k.contains("/plan-ordinal/")).collect();
        assert_eq!(ordinals.len(), 5);
        for n in 1..=5 {
            assert!(texts.contains(&n.to_string()) && texts.contains(&format!("step {n}")), "{texts:?}");
        }
        assert!(!texts.iter().any(|t| t == "PLAN" || t.contains("of 5 done")), "{texts:?}");
    }

    // case: widens the gutter past nine steps so the text column stays flush
    #[test]
    fn a_planned_answer_widens_its_gutter_past_nine_steps() {
        let (_, nine) = thread_drawn(&[ledger::Item::User("plan it".into()), plan_of(9, "pending")]);
        let (_, ten) = thread_drawn(&[ledger::Item::User("plan it".into()), plan_of(10, "pending")]);
        assert!(!nine.iter().any(|k| k.ends_with("/is-wide")));
        assert!(ten.iter().any(|k| k.ends_with("/plan-answer/is-wide")), "{ten:?}");
    }

    // case: renders nothing at all when the turn produced no steps
    // case: also renders nothing when there are no steps
    #[test]
    fn a_plan_with_no_steps_draws_nothing_answer_or_card() {
        let call = ledger::Item::Tool { name: "shell".into(), status: ledger::Status::Done, summary: "ls".into(), detail: Box::new(ledger::ToolDetail { id: "c".into(), arguments: "ls".into(), finished: true, ..Default::default() }) };
        for items in [vec![ledger::Item::User("plan it".into()), plan_of(0, "pending")], vec![ledger::Item::User("do it".into()), plan_of(0, "pending"), call]] {
            let (texts, keys) = thread_drawn(&items);
            assert!(!keys.iter().any(|k| k.contains("plan-answer")) && !texts.iter().any(|t| t == "PLAN"), "{texts:?}");
        }
    }

    // case: keeps its markers, its counter and its label
    // case: is NOT marked as having touched nothing, so its outcomes are drawn (drawn)
    #[test]
    fn a_plan_in_a_turn_that_did_work_keeps_its_label_counter_and_markers() {
        let call = ledger::Item::Tool { name: "read".into(), status: ledger::Status::Done, summary: "read".into(), detail: Box::new(ledger::ToolDetail { id: "t1".into(), arguments: "read".into(), finished: true, ..Default::default() }) };
        let mut plan = plan_of(5, "pending");
        if let ledger::Item::Plan { steps, .. } = &mut plan { steps[0].state = "done".into(); }
        let (texts, keys) = thread_drawn(&[ledger::Item::User("do it".into()), plan, call]);
        assert!(texts.iter().any(|t| t == "PLAN") && texts.iter().any(|t| t == "1 of 5 done"), "{texts:?}");
        assert!(!keys.iter().any(|k| k.contains("plan-answer") || k.contains("plan-ordinal")));
    }

    fn composition_state(kind: Option<&str>, running: bool) -> State {
        let mut s = State::default();
        s.ledger = Some(LedgerPane {
            cold_tip: None,
            header: Teammate { id: "tm_codec".into(), name: "Codec".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
            conversation: Conversation { conversation_id: Some("m_1".into()), turns: Vec::new() },
            items: Vec::new(), dir: std::path::PathBuf::new(), worker: None,
            snapshot: ledger::Snapshot {
                running,
                live: locust_core::thread_view::live_line(&[], running, "2026-09-04T21:47:00.000Z", false),
                turn_metas: vec![ledger::Metadata { mission_id: "m_1".into(), started_by_kind: kind.map(str::to_owned), ..Default::default() }],
                ..Default::default()
            },
            due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
        });
        s
    }
    fn face_keys(owned: bool, running: bool) -> Vec<String> {
        let mut s = composition_state(None, running);
        if let Some(live) = s.ledger.as_mut().unwrap().snapshot.live.as_mut() {
            live.label = "Thinking".into();
            live.register = "thinking".into();
            live.started_at = "1970-01-01T00:00:00.000Z".into();
        }
        s.ledger.as_mut().unwrap().header.id = if owned { "tm_atlas" } else { LEDGER_TARGET }.into();
        s.looks.insert("tm_atlas".into(), locust_core::native::FaceLook {
            hue: "blue".into(), parts: Some(crate::look::faces::seed_parts("tm_atlas")),
            bot: Some(("droid".into(), "eyes".into())), bot_screen: None, pet: None,
        });
        let items = vec![ledger::Item::Assistant { item_id: "reply".into(), text: "Hello.".into(), r#final: true }];
        let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Atlas", &Palette::new(true), s),
            (800.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.contains("thread-face-")).collect()
    }
    #[test]
    fn beside_a_reply_the_ghost_when_the_conversation_is_nobodys_the_teammates_own_bot_when_it_has_one() {
        let nobody = face_keys(false, false);
        assert!(nobody.iter().any(|k| k.ends_with("ghost-pearl")), "{nobody:?}");
        assert!(!nobody.iter().any(|k| k.contains("swarm")));
        let owned = face_keys(true, false);
        assert!(owned.iter().any(|k| k.ends_with("droid-blue")), "{owned:?}");
        assert!(!owned.iter().any(|k| k.contains("ghost")));
    }
    #[test]
    fn wears_the_ghosts_own_white_as_the_covers_ghost_does_on_the_reply_and_the_live_row_alike() {
        let keys = face_keys(false, true);
        assert_eq!(keys.len(), 2, "{keys:?}");
        assert!(keys.iter().all(|k| k.ends_with("ghost-pearl")), "{keys:?}");
    }
    #[test]
    fn on_the_live_row_the_same_ghost_while_nobodys_conversation_thinks() {
        let keys = face_keys(false, true);
        assert!(keys.iter().any(|k| k.ends_with("live-ghost-pearl")), "{keys:?}");
        assert!(!keys.iter().any(|k| k.contains("swarm")));
    }

    #[test]
    fn an_ended_threads_start_date_refreshes_without_reopening_its_ledger() {
        let mut s = composition_state(None, false);
        let pane = s.ledger.as_mut().unwrap();
        pane.snapshot.started_at = "2000-01-01T09:15:00Z".into();
        pane.snapshot.started_label = Some("9:15 AM".into());
        pane.snapshot.started_label_minute = 0;
        pane.due = f64::INFINITY;
        let mut tx = Tx::default();
        assert!(ledger_tick(&mut s, 0.0, &mut tx));
        let pane = s.ledger.as_ref().unwrap();
        assert!(pane.snapshot.started_label.as_ref().unwrap().contains("2000"));
        assert!(pane.due.is_infinite());
        assert!(!s.ledger_job.is_pending(), "refreshing the date never rereads the ledger");
    }

    #[test]
    fn the_cached_mission_start_label_reaches_the_headless_thread() {
        let items = [ledger::Item::User("read status.ts".into())];
        let mut s = composition_state(None, false);
        s.ledger.as_mut().unwrap().snapshot.started_label = Some("Sep 2, 11:25 PM".into());
        assert!(thread_text(&items, &mut s).contains("Started Sep 2, 11:25 PM"));
        s.ledger.as_mut().unwrap().snapshot.started_label = None;
        assert!(!thread_text(&items, &mut s).contains("Started "));
    }

    /// the-live-line-says-which-plan-step.test.ts, drawn: the open call in words and the plan's step, on the line a
    /// person reads; and with nothing to say but the word, the word with its ellipsis.
    #[test]
    fn the_live_line_draws_what_the_call_is_doing_and_which_plan_step() {
        let items = [ledger::Item::User("run the tests".into())];
        let mut s = composition_state(None, true);
        assert!(thread_text(&items, &mut s).contains("Starting\u{2026}"));
        let events = [
            serde_json::json!({"type": "run.started", "occurredAt": "2026-09-04T21:47:00.000Z", "payload": {}}),
            serde_json::json!({"type": "plan.updated", "payload": {"plan": {"plan": [{"step": "Step 1", "status": "completed"}, {"step": "Step 2", "status": "in_progress"}, {"step": "Step 3", "status": "pending"}]}}}),
            serde_json::json!({"type": "tool.started", "payload": {"itemId": "t", "toolKind": "command_execution", "name": "shell", "command": "npm test"}}),
        ];
        s.ledger.as_mut().unwrap().snapshot.live = locust_core::thread_view::live_line(&events, true, "2026-09-04T21:47:00.000Z", false);
        let drawn = thread_text(&items, &mut s);
        // The words, then the plan's step as its own part (kept whole in a narrow column).
        assert!(drawn.contains("Running npm test\n · step 2 of 3"), "{drawn}");
    }

    // case: keeps the plan pill of the running line whole, so the thread never scrolls sideways under it
    #[test]
    fn keeps_the_plan_pill_of_the_running_line_whole_so_the_thread_never_scrolls_sideways_under_it() {
        // the-thread-beside-the-browser-keeps-its-parts-whole.test.ts (0.867): a long turn beside the browser, the
        // conversation ~418px wide; "step 2 of" was cut at the edge.
        fn drawn(narrow: bool) -> Vec<(String, [f32; 4])> {
            let items = vec![ledger::Item::User("run the tests".into())];
            let mut s = composition_state(None, true);
            let events = [
                serde_json::json!({"type": "run.started", "occurredAt": "2026-09-04T21:47:00.000Z", "payload": {}}),
                serde_json::json!({"type": "plan.updated", "payload": {"plan": {"plan": [{"step": "Step 1", "status": "completed"}, {"step": "Step 2", "status": "in_progress"}, {"step": "Step 3", "status": "pending"}]}}}),
                serde_json::json!({"type": "tool.started", "payload": {"itemId": "t", "toolKind": "command_execution", "name": "shell", "command": "pnpm --filter @locust/desktop exec vitest run src/renderer/src/a-very-long-test-file-name.test.tsx --reporter verbose"}}),
            ];
            s.ledger.as_mut().unwrap().snapshot.live = locust_core::thread_view::live_line(&events, true, "2026-09-04T21:47:00.000Z", false);
            let draw: fn(&(State, Vec<ledger::Item>), &mut rune::Cx<(State, Vec<ledger::Item>)>) -> El = if narrow {
                |(s, items), _| row().child(col().w(418.0).child(message_column(items, "Codec", &Palette::new(true), s)))
            } else {
                |(s, items), _| row().child(col().w(1400.0).child(message_column(items, "Codec", &Palette::new(true), s)))
            };
            let mut rt = rune::Runtime::new((s, items), draw, (1600.0, 800.0), rune::Env::new(true, 1.0));
            rt.frame(0.0);
            rt.elements().into_iter().filter_map(|e| Some((e.key?, e.rect))).filter(|(k, _)| k.starts_with("live-step")).collect()
        }
        let at = |all: &[(String, [f32; 4])], key: &str| all.iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no {key}: {all:?}")).1;
        let (narrow, wide) = (drawn(true), drawn(false));
        for (key, [x, _, w, _]) in &narrow {
            assert!(x + w <= 418.5, "{key} ends at {} in a 418px column: {narrow:?}", x + w);
        }
        // The pill and the time are as wide as when there is room; the words gave way.
        for key in ["live-step-peek", "live-step-time"] {
            assert!((at(&narrow, key)[2] - at(&wide, key)[2]).abs() < 0.5, "{key}: {narrow:?} / {wide:?}");
        }
        assert!(at(&narrow, "live-step-label")[2] < at(&wide, "live-step-label")[2], "the words gave way");
    }

    // case: leads with the phase, swept, in the place of the word "starting"
    #[test]
    fn the_live_line_leads_with_the_starts_phase_and_holds_it_as_its_hover_title() {
        let items = [ledger::Item::User("Reply with one word".into())];
        let mut s = composition_state(None, true);
        s.ledger.as_mut().unwrap().snapshot.live =
            locust_core::thread_view::live_line_starting(&[], true, "2026-10-04T20:00:00.000Z", false, Some("Reading the folder"));
        let taken = std::mem::take(&mut s);
        let mut rt = rune::Runtime::new(
            (taken, items.to_vec()),
            |(s, items), _| message_column(items, "Codec", &Palette::new(true), s),
            (800.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let elements = rt.elements();
        let line = elements.iter().find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("Reading the folder\u{2026}"))).expect("the phase leads the line");
        assert_eq!(line.help.as_deref(), Some("Reading the folder\u{2026}"));
        assert!(!elements.iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains("Starting\u{2026}"))));
    }

    #[test]
    fn the_cached_live_line_reaches_the_headless_thread() {
        let items = [ledger::Item::User("read status.ts".into())];
        let mut s = composition_state(None, true);
        assert!(thread_text(&items, &mut s).contains("Starting"));
        s.ledger.as_mut().unwrap().snapshot.running = false;
        assert!(!thread_text(&items, &mut s).contains("Starting"));
    }
    #[test]
    fn the_headless_thread_never_draws_the_hosts_relay_briefing_as_a_user_bubble() {
        let items = [ledger::Item::User("end with one <locust-share to=\"Wren\"> block holding your reply.".into())];
        let mut s = composition_state(Some("relay"), false);
        assert!(!thread_text(&items, &mut s).contains("locust-share"));
        s.ledger.as_mut().unwrap().snapshot.turn_metas[0].started_by_kind = None;
        assert!(thread_text(&items, &mut s).contains("locust-share"));
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
            cold_tip: None,
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


#[cfg(test)]
mod renderer_newest_tests {
    use super::*;
    fn items(turns: usize, rows: usize) -> Vec<ledger::Item> {
        let mut items = Vec::new();
        for n in 0..turns {
            items.push(ledger::Item::User(format!("Question number {n}.")));
            for at in 0..rows {
                items.push(ledger::Item::Assistant { item_id: format!("m_{n}_{at}"), text: format!("Answer {n}.{at}"), r#final: true });
            }
        }
        items.push(ledger::Item::User("The question now.".into()));
        items.push(ledger::Item::Assistant { item_id: "now".into(), text: "The answer now.".into(), r#final: true });
        items
    }
    fn draw(items: Vec<ledger::Item>) -> String {
        let mut rt = rune::Runtime::new((State::default(), items), |(s, items), _| message_column(items, "", &Palette::new(true), s), (1200.0, 1800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
    }
    #[test]
    fn a_long_one_opens_on_its_newest_turns_with_the_older_ones_a_press_or_a_scroll_away() {
        let shown = draw(items(20, 60));
        assert!(shown.contains("Show 14 earlier turns"));
        assert!(!shown.contains("Question number 13."));
        assert!(!shown.contains("Answer 0.0"));
        assert!(shown.contains("Question number 14."));
        assert!(shown.contains("Question number 19."));
        assert!(shown.contains("The answer now."));
    }
    #[test]
    fn a_short_one_is_whole_with_nothing_to_press() {
        let shown = draw(items(3, 5));
        assert!(!shown.contains("earlier turn"));
        assert!(shown.contains("Question number 0."));
    }
}

#[cfg(test)]
mod renderer_newest_interaction_tests {
    use super::*;
    #[test]
    fn show_earlier_reveals_another_budget_without_switching_conversations() {
        let mut rt = rune::Runtime::new(State::default(), |s, _| {
            let items = (0..21).flat_map(|n| std::iter::once(ledger::Item::User(format!("Question number {n}.")))
                .chain((0..60).map(move |at| ledger::Item::Assistant { item_id: format!("m_{n}_{at}"), text: format!("Answer {n}.{at}"), r#final: true }))).collect::<Vec<_>>();
            message_column(&items, "", &Palette::new(true), s)
        }, (1200.0, 1800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        let (x, y) = rt.find("show-earlier-turns").expect("earlier turns button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.05);
        assert_eq!(rt.state.earlier_turn_budgets.get("preview"), Some(&800));
        let shown = rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
        assert!(shown.contains("Show 7 earlier turns"));
        assert!(shown.contains("Question number 7."));
        assert!(!shown.contains("Question number 6."));
        assert!(!rt.state.earlier_turn_budgets.contains_key("another-conversation"));
    }
}

#[cfg(test)]
mod renderer_newest_scroll_tests {
    use super::*;
    #[test]
    fn scrolling_to_the_older_edge_reveals_the_next_budget() {
        let mut rt = rune::Runtime::new(State::default(), |s, _| {
            let items = (0..21).flat_map(|n| std::iter::once(ledger::Item::User(format!("Question number {n}.")))
                .chain((0..60).map(move |at| ledger::Item::Assistant { item_id: format!("m_{n}_{at}"), text: format!("Answer {n}.{at}"), r#final: true }))).collect::<Vec<_>>();
            col().key("messages").w_full().h_full().scroll(ScrollSpec { stick_bottom: true })
                .on_layout_tx(|s: &mut State, r, _| s.prompt_rail.viewport_rect = r)
                .child(message_column(&items, "", &Palette::new(true), s))
        }, (1200.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        assert!(!rt.state.earlier_turn_budgets.contains_key("preview"));
        rt.update(|s, tx| { s.thread_away = true; tx.scroll_key_to("show-earlier-turns", 0.0); });
        for _ in 0..4 { rt.frame(0.05); }
        assert_eq!(rt.state.earlier_turn_budgets.get("preview"), Some(&800));
    }
}

#[cfg(test)]
#[path = "turn_foot_tests.rs"]
mod turn_foot_tests;

#[cfg(test)]
#[path = "long_fold_tests.rs"]
mod long_fold_tests;
#[cfg(test)]
#[path = "a_page_runs_tests.rs"]
mod a_page_runs_tests;

#[cfg(test)]
#[path = "terminal_thread_tests.rs"]
mod terminal_thread_tests;

#[cfg(test)]
#[path = "cold_start_note_tests.rs"]
mod cold_start_note_tests;

/// A running turn's thread with its live line, for the sweep's tests (working_shimmer_tests.rs).
#[cfg(test)]
pub(crate) fn tests_live_state() -> State {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_codec".into(), name: "Codec".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("m_1".into()), turns: Vec::new() },
        items: Vec::new(), dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            running: true,
            live: locust_core::thread_view::live_line(&[], true, "2026-09-04T21:47:00.000Z", false),
            turn_metas: vec![ledger::Metadata { mission_id: "m_1".into(), ..Default::default() }],
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    s
}

/// The live line drawn, and the element whose key starts with `prefix`.
#[cfg(test)]
fn tests_live_element(s: &mut State, prefix: &str) -> rune::ElementInfo {
    let taken = std::mem::take(s);
    let items = vec![ledger::Item::User("run the tests".into())];
    let mut rt = rune::Runtime::new((taken, items), |(s, items), _| message_column(items, "Codec", &Palette::new(true), s), (800.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let found = rt.elements().into_iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with(prefix)))
        .unwrap_or_else(|| panic!("no {prefix}: {:?}", rt.elements().into_iter().filter_map(|e| e.key).collect::<Vec<_>>()));
    *s = std::mem::take(&mut rt.state.0);
    found
}

#[cfg(test)]
pub(crate) fn tests_live_label(s: &mut State) -> rune::ElementInfo {
    tests_live_element(s, "live-step-label")
}

#[cfg(test)]
pub(crate) fn tests_live_face(s: &mut State) -> rune::ElementInfo {
    tests_live_element(s, "thread-face-live-")
}

#[cfg(test)]
#[path = "messages_orb_tests.rs"]
mod messages_orb_tests;
