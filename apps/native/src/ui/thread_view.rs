use super::*;

pub(super) fn thread_view(s: &State, p: &Palette) -> El {
    // Local history remains readable even when the MCP engine is unavailable.
    let error = s.load_error.as_ref().filter(|_| s.ledger.is_none());

    if let Some(error) = error {
        col()
            .grow(1.0)
            .shrink()
            .h_full()
            .center()
            .padding(32.0)
            .gap(18.0)
            .child(
                text(error.clone())
                    .font(type_::BODY)
                    .color(p.red)
                    .align(Align::Center)
                    .w_full()
                    .wrap_anywhere()
                    .selectable(true),
            )
            .child(
                el().key("retry")
                    .h(40.0)
                    .px(18.0)
                    .center()
                    .radius(8.0)
                    .fill(p.raised)
                    .border(1.0, p.hairline)
                    .child(text("Try again").font(type_::LABEL).color(p.text))
                    .on_press(load_team),
            )
    } else if let Some((teammate, conversation)) = selected_conversation(s) {
        let fallback;
        let items = if let Some(pane) = &s.ledger {
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

        let transcript = if items.is_empty() && s.ledger.is_none() {
            crate::idle_teammate::view(s, teammate, p).child(approval_card(s, p))
        } else if items.is_empty() {
            let title = if let Some(pane) = &s.ledger {
                if pane.loading {
                    "Loading conversation…".into()
                } else {
                    "No transcript items.".into()
                }
            } else {
                format!("Ask {} anything", teammate.name)
            };
            let mut empty = col()
                .key("messages")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .center()
                .padding(24.0)
                .gap(10.0)
                .child(
                    text(title)
                        .font(type_::TITLE)
                        .color(p.text)
                        .w_full()
                        .align(Align::Center)
                        .wrap(),
                );
            if s.ledger.is_none() {
                empty = empty.child(
                    text(format!(
                        "{} · runs in {}",
                        agent(teammate),
                        mode_word(&teammate.runs_in),
                    ))
                    .font(type_::LABEL)
                    .color(p.muted)
                    .w_full()
                    .align(Align::Center)
                    .wrap_anywhere(),
                );
            }
            empty.child(approval_card(s, p))
        } else {
            col()
                .key("messages")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .on_at_bottom(|s: &mut State, at| {
                    s.thread_away = !at;
                })
                .on_layout_tx(|s: &mut State, r, tx| {
                    let changed = (0..4).any(|i| (s.prompt_rail.viewport_rect[i] - r[i]).abs() > 0.5);
                    s.prompt_rail.viewport_rect = r;
                    if !changed {
                        tx.live_only();
                    }
                })
                .child(
                    row().w_full().justify_center().child(
                        col()
                            .w_full()
                            .items_center()
                            .child(message_column(items, &teammate.name, p, s))
                            .children(crate::free_offer::view(s, p))
                            .children(crate::busy_continue::view(s, p))
                            .children(crate::failure_offers::view(s, p)),
                    ),
                )
        };
        let mut transcript_box = col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .items_center()
            .child(transcript);
        if !items.is_empty() {
            if let Some(rail) = prompt_rail::view(s, items, p) {
                transcript_box = transcript_box.child(rail);
            }
            if let Some(jump) = jump_to_bottom::view(s.thread_away, turn_running(s), s.jump_hover, p) {
                transcript_box = transcript_box.child(jump);
            }
        }
        if let Some(ask_chip) = side_chat::selection_ask_chip(s, p) {
            transcript_box = transcript_box.child(ask_chip);
        }

        // The conversation's actions sit in its header, on the right, as Electron's thread header draws them.
        let mut actions = row()
            .gap(4.0)
            .center_y()
            .child(pull_request_chip::control(s, p))
            .child(commit_panel::control(s, p))
            .children(inspector_panel::control(s, p))
            .child(changes_panel::controls(s, p))
            .children(review_changes::control(s, p))
            .child(terminal_panel::runtime_controls(s, p))
            .child(terminal_panel::toggle(s, p))
            .child(browser_panel::toggle(s, p));
        if let Some(btn) = side_chat::header_button(s, p) {
            actions = actions.child(btn);
        }
        actions = actions.children(crate::header_menu::control(s, p));
        let mut status = row().w_full().center_y().gap(8.0).px(28.0).py(8.0);
        if s.flight.is_some() || s.opening_teammate.is_some() {
            status = status.child(
                el().key("activity-dot")
                    .size(6.0, 6.0)
                    .radius(3.0)
                    .fill(p.status(s.status_tone))
                    .cycle(Cycle {
                        period: 1.6,
                        phase: 0.0,
                        stops: &[(0.0, 0.35), (0.5, 1.0), (1.0, 0.35)],
                        still: 1.0,
                        fps: 30.0,
                    }),
            );
        }
        status = status.child(
            text(s.status.clone())
                .font(type_::LABEL)
                .color(p.status(s.status_tone))
                .grow(1.0)
                .shrink()
                .wrap_anywhere()
                .selectable(true),
        );
        // Electron shows no engine builder's label on conversation screens (LIVE2 #6); Kind::label() stays off-screen.

        if let Some(context) = context_ring::control(s, p) {
            status = status.child(context);
        }
        let placeholder = if own_command::bang(&s.draft.text).is_some() {
            "Type a command".to_owned()
        } else {
            composer_placeholder(s)
        };
        let field = text_input(&s.draft, type_::BODY)
            .key("composer")
            .disabled(turn_running(s) && queue_front(s).is_some())
            .placeholder(placeholder, p.muted)
            .color(p.text)
            .w_full()
            // One line is as tall as the send circle (Electron: a 16-point line and 12 above and below, the send's 40).
            .py((crate::look::controls::SEND_SIZE - composer_line_height()) / 2.0)
            .on_edit(|s: &mut State| &mut s.draft)
            .on_submit(|s: &mut State, tx: &mut Tx| submit_composer(s, tx, false))
            .on_input_key(composer_input_key)
            .on_input_paste(|s: &mut State, content, _| {
                let mut added = Vec::new();
                for path in content.files() {
                    added.push(path.to_string_lossy().replace('\\', "/"));
                }
                if let Some(image) = content.image()
                    && let Some(path) = save_pasted_image(s, image.png())
                {
                    added.push(path);
                }
                if added.is_empty() {
                    return false;
                }
                attach_files(s, &added);
                true
            });
        let l = crate::look::controls::Look::new(p.dark);
        let composer = rune::auto_grow(
            field,
            composer::COMPOSER_MIN_LINES,
            composer::COMPOSER_MAX_LINES,
        )
        .max_h(composer::COMPOSER_MAX_HEIGHT_PX)
        .grow(1.0)
        .shrink()
        .on_drop_files(|s: &mut State, files, _| attach_dropped_paths(s, files));

        let queueing = can_queue(s);
        let stopping = stop_press_ui::offered(s) && !queueing && own_command::bang(&s.draft.text).is_none();
        let blocked = send_blocked_now(s);
        let send_enabled =
            (own_command::bang(&s.draft.text).is_some() || can_send(s) || queueing || stopping)
                && !s.stopping.is_pending();
        let send_title = if stopping {
            "Stop the running reply".to_owned()
        } else if queueing {
            composer::QUEUE_SEND_TITLE.to_owned()
        } else {
            blocked.unwrap_or(composer::SEND_LIVE_TITLE).to_owned()
        };
        let button_label = if stopping {
            "Stop"
        } else if queueing {
            "Queue"
        } else {
            "Send"
        };
        // The round send button (Electron's metal composer): ivory with an arrow when there is something to send or
        // queue, a square while a reply runs and nothing waits.
        let send_state = if stopping {
            crate::look::controls::SendState::Stop
        } else if send_enabled {
            crate::look::controls::SendState::Ready
        } else {
            crate::look::controls::SendState::Idle
        };
        let mut button = crate::look::controls::send_circle(&l, send_state, button_label).key("send").disabled(!send_enabled);
        // Rune has no title tooltip API on El in this pin; the label/status carry the why.
        let _ = send_title;
        if send_enabled {
            button = if stopping {
                button.on_press(stop)
            } else {
                button.on_press_tx(|s: &mut State, tx: &mut Tx| submit_composer(s, tx, false))
            };
        }
        if stopping {
            let mode = crate::look::stop_beam::mode(s.stopping.is_pending(), s.reduced_motion.get());
            button = crate::look::stop_beam::wrap(button, mode, p.dark);
        }

        let id = teammate.id.clone();
        let current = s.models.get(&id).and_then(|agents| {
            agents
                .iter()
                .find(|a| teammate.runtime.as_deref() == Some(a.runtime.as_str()))
                .or_else(|| {
                    (id != LEDGER_TARGET && teammate.runtime.is_none() && agents.len() == 1)
                        .then(|| &agents[0])
                })
        });
        let picked = s.picks.get(&id);
        let model = picked
            .map(|p| p.model.as_str())
            .or_else(|| {
                if id == LEDGER_TARGET {
                    teammate.model.as_deref()
                } else {
                    current.and_then(|a| a.model.as_deref())
                }
            })
            .or(teammate.model.as_deref())
            .filter(|m| !m.is_empty())
            .unwrap_or("Choose model");
        let effort = picked.and_then(|p| p.effort.as_deref()).or_else(|| {
            if picked.is_none() {
                current.and_then(|a| a.effort.as_deref())
            } else {
                None
            }
        });
        let rt = picked.and_then(|p| p.runtime.as_deref()).or(teammate.runtime.as_deref()).or(current.map(|a| a.runtime.as_str())).unwrap_or("codex");
        let resolved_id = s.resolved_model_names.get(&format!("{rt}:{model}")).map(String::as_str);
        let default_model_name = (model == locust_core::models::ACCOUNT_DEFAULT)
            .then_some(resolved_id)
            .flatten()
            .map(|id| locust_core::teammate_tooltip::route_model_name(rt, id));
        let model_name_owned: String;
        let model_name = if model == locust_core::models::ACCOUNT_DEFAULT {
            if let Some(ref resolved) = default_model_name {
                resolved.as_str()
            } else {
                locust_core::teammate_tooltip::short_runtime_name(rt)
            }
        } else if let Some(resolved) = resolved_id {
            resolved
        } else {
            model_name_owned = model_chip_name(model, current).to_owned();
            model_name_owned.as_str()
        };
        let (chip_model_name, chip_is_free) = locust_core::route_rows::free_tag_of(model_name);
        let active_account = match picked.and_then(|p| p.account.as_ref()) {
            Some(explicit) => explicit.as_deref(),
            None => teammate.account.as_deref(),
        };
        let account_name = active_account
            .and_then(|acc| s.agent_accounts.accounts.iter().find(|a| a.id == acc))
            .map(|a| a.name.as_str());
        let base_name = if let Some(n) = account_name {
            format!("{chip_model_name} \u{b7} {n}")
        } else {
            chip_model_name.to_owned()
        };
        let effort_text = effort.map(locust_core::effort_levels::effort_phrase);
        let chip_reading = locust_core::effort_levels::route_chip_reading(&base_name, effort_text.as_deref());
        let label = format!("{chip_reading}{}", if picked.is_some() { ", this turn" } else { "" });
        let chip_id = id.clone();
        let runtime = picked
            .and_then(|p| p.runtime.as_deref())
            .or(teammate.runtime.as_deref())
            .or(current.map(|a| a.runtime.as_str()))
            .unwrap_or("codex");
        // The runtime's mark where the ready dot was, greyed when it cannot take work; a dot for a model of the
        // person's own; "No AI agent" when nothing can take work (chip_mark.rs).
        let found = crate::chip_mark::found_agents(s);
        let chip_mark = crate::chip_mark::chip_mark(runtime, model, found.as_deref());
        let mark = crate::chip_mark::chip_mark_el(&l, runtime, &chip_mark);
        let no_agent = chip_mark == crate::chip_mark::ChipMark::NoAgent;
        let label = if no_agent { "No AI agent".to_owned() } else { label };
        let mut chip = crate::look::controls::composer_route_chip(&l, mark, &label, chip_is_free && !no_agent, true)
            .key("model-chip")
            .on_hover_change(|s: &mut State, over, _| {
                s.chip_hover = over;
                if over {
                    crate::agent_card_ui::request(s);
                }
            })
            .on_press(move |s: &mut State| open_models(s, chip_id.clone()));
        // Its usage on hover (Composer.tsx, 2026-10-30): the agent's card above the chip, aligned to its right.
        let chip_account = match picked.and_then(|p| p.account.as_ref()) {
            Some(explicit) => explicit.as_deref(),
            None => teammate.account.as_deref(),
        };
        // From 80% the chip wears an amber dot after its chevron, and its usage is read aloud as its description
        // (`.lc-control.is-pressing::after`, `aria-description`).
        let chip_reading = s.usage_readings.get(&locust_core::agent_accounts::usage_key(runtime, chip_account)).map(String::as_str);
        let now = chrono::Utc::now();
        if locust_core::agent_card::chip_is_pressing(chip_reading, now) {
            chip = chip.child(el().key("model-chip-dot").size(5.0, 5.0).radius(2.5).fill(l.c("amber")).a11y_hidden());
        }
        if let Some(said) = locust_core::agent_card::chip_usage_sentence(chip_reading, now) {
            chip = chip.description(said);
        }
        if let Some(card) = crate::agent_card_ui::chip_card(s, &l, runtime, chip_account) {
            chip = chip.child(card);
        }
        if let Some(asking) = crate::compare_view::asking_label(s) {
            chip = crate::hover::tip(s, &l, "compare-route", &asking, chip);
        } else if model == locust_core::models::ACCOUNT_DEFAULT {
            let exact = format!(
                "{} / its own default model{}",
                locust_core::failure_words::runtime_display_name(runtime),
                default_model_name
                    .as_deref()
                    .map(|n| format!(", {n} on its last run"))
                    .unwrap_or_default()
            );
            chip = crate::hover::tip(s, &l, "model-chip-tip", &exact, el().child(chip));
        }
        let mut picker = col().w_full().px(24.0);
        if s.model_menu.as_ref() == Some(&id) {
            picker = picker.child(row().w_full().justify_end().child(model_picker(s, &l, &id, teammate, current, model, effort, false)));
        }

        let mut composer_stack = col().w_full().gap(8.0).px(24.0).pb(24.0);

        composer_stack = composer_stack.children(crate::voice_panel::notices(s, p));
        if let Some(question) = crate::folder_confirmation::view(s, p) {
            composer_stack = composer_stack.child(question);
        }

        // Slash / @ popups
        let slash = slash_choices_now(s);
        let at = at_choices_now(s);
        if !slash.is_empty() {
            let mut menu = col()
                .key("slash-menu")
                .w_full()
                .max_h(220.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .gap(2.0)
                .padding(8.0)
                .radius(9.0)
                .fill(p.raised)
                .border(1.0, p.hairline);
            for (index, command) in slash.iter().enumerate() {
                if command.group.as_ref().is_some_and(|g| {
                    slash
                        .get(index.wrapping_sub(1))
                        .and_then(|c| c.group.as_ref())
                        != Some(g)
                }) && let Some(group) = &command.group
                {
                    menu = menu.child(text(group.clone()).font(type_::LABEL).color(p.muted));
                }
                let active = index == s.slash_at.min(slash.len().saturating_sub(1));
                let name = if let Some(hint) = &command.hint {
                    format!("/{} {hint}", command.name)
                } else {
                    format!("/{}", command.name)
                };
                let detail = command.detail.clone();
                let cmd = command.clone();
                menu = menu.child(
                    el().w_full()
                        .padding(6.0)
                        .radius(6.0)
                        .fill(if active { p.selected } else { p.raised })
                        .child(
                            col()
                                .w_full()
                                .gap(2.0)
                                .child(text(name).font(type_::LABEL).color(if command.weighted {
                                    p.amber
                                } else {
                                    p.text
                                }))
                                .child(
                                    text(detail).font(type_::LABEL).color(if command.weighted {
                                        p.amber
                                    } else {
                                        p.muted
                                    }),
                                ),
                        )
                        .on_press(move |s: &mut State| run_slash_command(s, &cmd)),
                );
            }
            composer_stack = composer_stack.child(menu);
        } else if !at.is_empty()
            || (composer::at_query(&s.draft.text).is_some_and(|q| !q.is_empty())
                && s.files_partial
                && s.at_dismissed.as_deref() != Some(s.draft.text.as_str()))
        {
            let mut menu = col()
                .key("at-menu")
                .w_full()
                .max_h(220.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .gap(2.0)
                .padding(8.0)
                .radius(9.0)
                .fill(p.raised)
                .border(1.0, p.hairline);
            let mut saw_file = false;
            let mut saw_mate = false;
            if at.is_empty() && s.files_partial {
                let heading = if s.files_partial {
                    format!(
                        "Files in this folder, the first {} found",
                        s.project_files.len()
                    )
                } else {
                    composer::FILES_HEADING.into()
                };
                menu = menu
                    .child(text(heading).font(type_::LABEL).color(p.muted))
                    .child(
                        text(composer::NO_MATCH_PARTIAL)
                            .font(type_::LABEL)
                            .color(p.muted),
                    );
            }
            for (index, choice) in at.iter().enumerate() {
                let active = index == s.at_at.min(at.len().saturating_sub(1));
                match choice {
                    AtChoice::Teammate { name, role, .. } => {
                        if !saw_mate {
                            saw_mate = true;
                            menu = menu.child(
                                text(composer::TEAMMATES_HEADING)
                                    .font(type_::LABEL)
                                    .color(p.muted),
                            );
                        }
                        let title = format!("@{name}");
                        let detail = composer::teammate_detail(role);
                        let picked = choice.clone();
                        menu = menu.child(
                            el().w_full()
                                .padding(6.0)
                                .radius(6.0)
                                .fill(if active { p.selected } else { p.raised })
                                .child(
                                    col()
                                        .gap(2.0)
                                        .child(text(title).font(type_::LABEL).color(p.text))
                                        .child(text(detail).font(type_::LABEL).color(p.muted)),
                                )
                                .on_press(move |s: &mut State| pick_at_choice(s, picked.clone())),
                        );
                    }
                    AtChoice::File { path } => {
                        if !saw_file {
                            saw_file = true;
                            menu = menu.child(
                                text(if s.files_partial {
                                    format!(
                                        "Files in this folder, the first {} found",
                                        s.project_files.len()
                                    )
                                } else {
                                    composer::FILES_HEADING.into()
                                })
                                .font(type_::LABEL)
                                .color(p.muted),
                            );
                        }
                        let cut = path.rfind('/').map(|i| i as isize).unwrap_or(-1);
                        let base = if cut < 0 {
                            path.clone()
                        } else {
                            path[cut as usize + 1..].to_owned()
                        };
                        let detail = if cut < 0 {
                            "the folder itself".into()
                        } else {
                            path[..cut as usize].to_owned()
                        };
                        let picked = choice.clone();
                        menu = menu.child(
                            el().w_full()
                                .padding(6.0)
                                .radius(6.0)
                                .fill(if active { p.selected } else { p.raised })
                                .child(
                                    col()
                                        .gap(2.0)
                                        .child(text(base).font(type_::LABEL).color(p.text))
                                        .child(text(detail).font(type_::LABEL).color(p.muted)),
                                )
                                .on_press(move |s: &mut State| pick_at_choice(s, picked.clone())),
                        );
                    }
                }
            }
            composer_stack = composer_stack.child(menu);
        }

        // Queued follow-up strip
        if let Some(front) = queue_front(s) {
            let key = front.key.clone();
            let count = composer::queued_in(&s.queue_rows, Some(&key)).len();
            let verdict = composer::queued_verdict(composer::VerdictInput {
                running: turn_running(s),
                phase: Some(queue_phase(s)),
                on_screen: true,
                restored: front.restored,
                held: front.held.as_deref(),
            });
            let note = composer::queue_note(&verdict, &working_name(s), count);
            let mut strip = col()
                .key("queued")
                .w_full()
                .gap(6.0)
                .padding(10.0)
                .radius(9.0)
                .fill(p.raised)
                .border(1.0, p.hairline)
                .child(
                    text(front.text.clone())
                        .font(type_::BODY)
                        .color(p.text)
                        .w_full()
                        .wrap(),
                )
                .child(text(note).font(type_::LABEL).color(p.muted));
            let mut actions = row().gap(8.0);
            if turn_running(s) {
                actions = actions.child(
                    el().padding(6.0)
                        .child(
                            text(composer::STOP_AND_SEND_NOW_LABEL)
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .on_press_tx(|s: &mut State, tx: &mut Tx| {
                            s.queue_send_now = queue_key_now(s);
                            stop(s);
                            // Drain once the stop lands.
                            let _ = tx;
                        }),
                );
            } else {
                let label = composer::SEND_NOW_LABEL;
                actions = actions.child(
                    el().padding(6.0)
                        .child(text(label).font(type_::LABEL).color(p.text))
                        .on_press_tx(|s: &mut State, tx: &mut Tx| {
                            s.queue_send_now = queue_key_now(s);
                            drain_queue_after_turn(s, tx);
                        }),
                );
            }
            actions = actions
                .child(
                    el().padding(6.0)
                        .child(text(composer::EDIT_LABEL).font(type_::LABEL).color(p.text))
                        .on_press(|s: &mut State| {
                            unqueue(s, true);
                        }),
                )
                .child(
                    el().padding(6.0)
                        .child(
                            text(composer::DISCARD_LABEL)
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .on_press(|s: &mut State| {
                            unqueue(s, false);
                        }),
                );
            strip = strip.child(actions);
            composer_stack = composer_stack.child(strip);
        } else if s.off_the_queue {
            let note = if turn_running(s) {
                format!(
                    "Off the queue while you edit — Enter queues it again for when {} finishes",
                    working_name(s)
                )
            } else {
                "Off the queue — Enter sends it".into()
            };
            composer_stack = composer_stack.child(
                el().w_full()
                    .padding(8.0)
                    .radius(9.0)
                    .fill(p.raised)
                    .child(text(note).font(type_::LABEL).color(p.muted)),
            );
        }
        if let Some(error) = &s.queue_error {
            composer_stack = composer_stack.child(
                text(error.clone())
                    .font(type_::LABEL)
                    .color(p.amber)
                    .w_full()
                    .wrap(),
            );
        }
        if let Some(note) = &s.composer_note {
            composer_stack = composer_stack.child(
                text(note.clone())
                    .font(type_::LABEL)
                    .color(p.amber)
                    .w_full()
                    .wrap(),
            );
        }
        let effective_runtime = picked
            .and_then(|p| p.runtime.as_deref())
            .unwrap_or(runtime);
        let effective_model = picked
            .map(|p| p.model.as_str())
            .or(teammate.model.as_deref())
            .or_else(|| current.and_then(|a| a.model.as_deref()));
        if effective_runtime == "claude"
            && id != LEDGER_TARGET
            && s.teammate_connectors.get(&id).is_none_or(Vec::is_empty)
            && !s.connector_names.is_empty()
            && !s.draft.text.trim().is_empty()
            && let Some(named) =
                locust_core::connectors_off::connector_mentioned(&s.draft.text, &s.connector_names)
        {
            composer_stack = composer_stack.child(
                text(locust_core::connectors_off::connectors_off_hint(
                    &teammate.name,
                    &named,
                ))
                .key("connectors-off-hint")
                .font(type_::LABEL)
                .color(p.secondary)
                .w_full()
                .wrap(),
            );
        }
        let oc_models = s
            .models
            .get(&id)
            .or_else(|| s.models.get(CARRY_TARGET))
            .and_then(|agents| agents.iter().find(|a| a.runtime == "opencode"))
            .map(|a| a.models.as_slice())
            .unwrap_or(&[]);
        if let Some(dropped_id) = effective_model
            && locust_core::connectors_off::dropped_free_model(
                effective_runtime,
                Some(dropped_id),
                oc_models,
            )
            && crate::free_offer::limit_replacement(s, chrono::Utc::now().timestamp_millis()).is_none()
            && let Some(next_id) = locust_core::connectors_off::free_start_model(
                effective_runtime,
                oc_models,
                &free_models_answered_here(s),
                &std::collections::HashSet::new(),
            )
        {
            let on_teammate = id != LEDGER_TARGET
                && teammate.runtime.as_deref() == Some(effective_runtime)
                && teammate.model.as_deref() == Some(dropped_id);
            let next_display = locust_core::connectors_off::opencode_model_display_name(&next_id);
            let banner_text = s
                .dropped_problem
                .as_ref()
                .filter(|(m, _)| m == dropped_id)
                .map(|(_, t)| t.clone())
                .unwrap_or_else(|| {
                    format!(
                        "OpenCode no longer offers {}.",
                        locust_core::connectors_off::opencode_model_display_name(dropped_id)
                    )
                });
            let button_label = if on_teammate {
                format!("Switch {} to {next_display}", teammate.name)
            } else {
                format!("Switch to {next_display}")
            };
            let switch_id = id.clone();
            let dropped_key = dropped_id.to_owned();
            let next_model = next_id;
            composer_stack = composer_stack.child(
                row()
                    .key("model-dropped-offer")
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(
                        text(banner_text)
                            .font(type_::LABEL)
                            .color(p.amber)
                            .grow(1.0)
                            .wrap(),
                    )
                    .child(
                        text(button_label)
                            .font(type_::LABEL)
                            .color(p.text)
                            .padding(6.0)
                            .radius(6.0)
                            .fill(p.raised)
                            .border(1.0, p.hairline)
                            .key("model-dropped-switch")
                            .on_press(move |s: &mut State| {
                                s.dropped_problem = None;
                                if !on_teammate {
                                    s.picks.insert(
                                        switch_id.clone(),
                                        TurnPick {
                                            runtime: Some("opencode".into()),
                                            model: next_model.clone(),
                                            effort: None,
                                            account: None,
                                        },
                                    );
                                    return;
                                }
                                let saved = s
                                    .data
                                    .as_ref()
                                    .ok_or_else(|| {
                                        "Native data folder could not be determined.".to_owned()
                                    })
                                    .and_then(|store| {
                                        let team = people::team(store)?;
                                        let mut entry = team["teammates"]
                                            .as_array()
                                            .and_then(|arr| {
                                                arr.iter()
                                                    .find(|t| t["teammateId"] == switch_id)
                                                    .cloned()
                                            })
                                            .ok_or_else(|| {
                                                "That teammate is not in Locust any more."
                                                    .to_owned()
                                            })?;
                                        entry["route"]["model"] = serde_json::json!(next_model);
                                        people::save_teammate(store, entry)?;
                                        Ok(())
                                    });
                                match saved {
                                    Ok(()) => {
                                        s.picks.remove(&switch_id);
                                        if let Some(mate) =
                                            s.teammates.iter_mut().find(|t| t.id == switch_id)
                                        {
                                            mate.model = Some(next_model.clone());
                                        }
                                    }
                                    Err(problem) => {
                                        s.dropped_problem = Some((dropped_key.clone(), problem));
                                    }
                                }
                            }),
                    ),
            );
        }
        if blocked.is_some() && s.scene_popup == ComposerScenePopup::Blocked {
            composer_stack = composer_stack.child(
                text(blocked.unwrap_or("").to_owned())
                    .font(type_::LABEL)
                    .color(p.amber)
                    .w_full()
                    .wrap(),
            );
        }

        if let Some(banner) = pull_request_chip::news_banner(s, p) {
            composer_stack = composer_stack.child(banner);
        }

        // Attachment chips: a file's name, a PDF's card.
        if !s.attached.is_empty() {
            let mut chips = row().gap(6.0).w_full();
            chips = chips.child(
                text(composer::attachment_label(s.attached.len()))
                    .font(type_::LABEL)
                    .color(p.muted),
            );
            for (i, path) in s.attached.iter().enumerate() {
                let name = path.rsplit('/').next().unwrap_or(path).to_owned();
                // A PDF is a card: its name and how many pages, never its path (pdf_pages_ui, Electron PdfTile).
                let face = if crate::pdf::is_pdf_path(path) { crate::pdf_pages_ui::card(s, path, p) } else { text(name).font(type_::LABEL).color(p.text) };
                chips = chips.child(
                    el().padding(4.0)
                        .radius(4.0)
                        .fill(p.raised)
                        .border(1.0, p.hairline)
                        .child(face)
                        .on_press(move |s: &mut State| {
                            if i < s.attached.len() {
                                s.attached.remove(i);
                            }
                        }),
                );
            }
            composer_stack = composer_stack.child(chips);
        }
        if !s.tagged.is_empty() {
            let mut tags = row().gap(6.0);
            for id in &s.tagged {
                let name = s
                    .teammates
                    .iter()
                    .find(|t| t.id == *id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| id.clone());
                let tid = id.clone();
                tags = tags.child(
                    el().padding(4.0)
                        .radius(4.0)
                        .fill(p.raised)
                        .child(text(format!("@{name}")).font(type_::LABEL).color(p.text))
                        .on_press(move |s: &mut State| {
                            s.tagged.retain(|t| t != &tid);
                        }),
                );
            }
            composer_stack = composer_stack.child(tags);
        }

        // The box, then the chips under it: the mode and the folder on the left, the model on the right (Electron's
        // `.lc-composer__box` and its footer row).
        // The mode these turns run in: the one chosen here, else the teammate's own (Electron's composer mode).
        let mode_now = s.composer_mode.clone().unwrap_or_else(|| teammate.runs_in.clone());
        let mode_word = locust_core::mode_facts::fact(&mode_now).chip;
        let mode_runtime = s
            .picks
            .get(&teammate.id)
            .and_then(|pk| pk.runtime.as_deref())
            .or(teammate.runtime.as_deref())
            .unwrap_or("codex");
        let has_connectors = s.connectors.connectors.as_ref().is_some_and(|c| !c.is_empty());
        let mode_tip = locust_core::mode_facts::connectors_note(mode_runtime, &mode_now, has_connectors)
            .unwrap_or_else(|| "Permission mode".to_owned());
        // Where the turn will run, as native::folder_for chooses: the teammate's own folder, else the one Locust has open.
        let folder = teammate.folder.as_deref().or(s.home.folder_path.as_deref());
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok();
        let system = std::env::var("SystemRoot").ok();
        let mode_btn = crate::look::controls::composer_chip(&l, None, mode_word, false)
            .key("mode-chip")
            .role("button")
            .label(format!("Mode: {}", locust_core::mode_facts::fact(&mode_now).name))
            .on_press(|s: &mut State| {
                s.mode_menu = !s.mode_menu;
                s.reference_menu = false;
            });
        let footer = row()
            .w_full()
            .center_y()
            .gap(10.0)
            .px(6.0)
            .child(crate::hover::tip(s, &l, "mode-chip-tip", &mode_tip, el().child(mode_btn)))
            .child(
                folder_chip(&l, folder, home.as_deref(), cfg!(windows), system.as_deref())
                    .on_press(|s: &mut State| s.reference_menu = !s.reference_menu),
            )
            .child(crate::compare_view::composer_mode_chip(s, &l))
            .child(rune::spacer())
            .child(chip);
        if let Some(chips) = crate::reference_chips::view(s, &l, teammate) {
            composer_stack = composer_stack.child(chips);
        }
        if let Some(note) = crate::link_open::note(s, &l) {
            composer_stack = composer_stack.child(note);
        }
        if let Some(list) = crate::issue_list::view(s, p) {
            composer_stack = composer_stack.child(list);
        }
        if let Some(points) = crate::page_points_ui::chips(s, p) {
            composer_stack = composer_stack.child(points);
        }
        if let Some(tile) = crate::diff_notes_ui::tile(s, p) {
            composer_stack = composer_stack.child(row().w_full().child(tile));
        }
        if let Some(picks) = crate::compare_view::composer_picks_row(s, &l) {
            composer_stack = composer_stack.child(picks);
        }
        if let Some(form) = crate::claude_cloud_panel::start_form(s, p) {
            composer_stack = composer_stack.child(form);
        }
        composer_stack = composer_stack
            .children(crate::update_banner::view(s, p))
            .children(crate::handoff_divider::composer_omissions(s, teammate, p))
            .children(crate::edit_message::line(s, p))
            .child(
                crate::look::controls::composer_box(&l)
                    // The + in a slot as tall as the send, so on the last line both share the line's centre.
                    .child(row().h(crate::look::controls::SEND_SIZE).center_y().child(plus_button(s, &l)))
                    .child(composer)
                    .children(crate::voice_panel::controls(s, p))
                    .child(button),
            )
            .child(footer);
        // The footer's menus open above the box, where the model picker does, taking their room from the thread: the
        // box never moves (Electron's menus open upward from their chip).
        if let Some(menu) = crate::reference_chips::folder_menu(s, &l) {
            picker = picker.child(row().w_full().px(6.0).child(menu));
        }
        if s.mode_menu {
            picker = picker.child(row().w_full().px(6.0).child(mode_menu(s, &l, teammate, &mode_now)));
        }
        if let Some(cmenu) = crate::compare_view::composer_mode_menu(s, &l) {
            picker = picker.child(row().w_full().px(6.0).child(cmenu));
        }
        if s.plus_menu {
            picker = picker.child(row().w_full().px(6.0).child(plus_menu(s, &l, teammate)));
        }

        col()
            .grow(1.0)
            .shrink()
            .h_full()
            .child(
                // The thread's head: the teammate's initial (faces come in C4), name and role, the agent's mark and
                // model under them, and the conversation's actions at the right.
                row()
                    .w_full()
                    .h(88.0)
                    .px(24.0)
                    .pt(28.0)
                    .gap(12.0)
                    .center_y()
                    .window_drag()
                    .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
                    .child(crate::look::faces::face(s.image_cache.as_deref(), &teammate.id, &teammate.name, s.looks.get(&teammate.id), 36.0, s.settings_panel.settings.terminal_faces, &l))
                    .child(
                        col()
                            .gap(3.0)
                            .grow(1.0)
                            .shrink()
                            .child(
                                row()
                                    .gap(8.0)
                                    .center_y()
                                    .child(text(header_name(s, teammate)).font(rune::TextStyle::new(15.0, 600.0)).color(l.c("text-name")).key("thread-header-name"))
                                    .child(text(teammate.role.clone()).font(rune::TextStyle::new(12.5, 400.0)).color(l.c("text-muted")).truncate().shrink()),
                            )
                            .child({
                                let mut line = row().gap(5.0).center_y();
                                if let Some(mark) = crate::look::controls::agent_mark(&l, teammate.runtime.as_deref().unwrap_or("codex"), 11.0, l.c("text-muted")) {
                                    line = line.child(mark);
                                }
                                let header_cost = locust_core::benchmark::header_cost_tail(
                                    s.ledger.as_ref().and_then(|ledger| ledger.snapshot.cost.as_ref()),
                                    turn_running(s),
                                );
                                // The runtime, then the mission strip: the model by the chip's name and the phase only when it
                                // is news; no id, no `Mission ·`, no permission prose (mission_strip.rs, THREE-PLACEMENTS).
                                // A routine's step says which one it is on, in place of "running" (App.tsx
                                // `routineStepPhrase(liveRun.startedBy, routines) ?? 'running'`).
                                let step_phrase = s.ledger.as_ref().and_then(|ledger| ledger.snapshot.turn_metas.last()).and_then(|run| {
                                    crate::conversation_list::routine_step_phrase(
                                        run.started_by_kind.as_deref(),
                                        run.started_by_routine.as_deref(),
                                        run.started_by_step,
                                        &s.org.routines,
                                    )
                                });
                                let phase = if turn_running(s) {
                                    step_phrase.as_deref().unwrap_or("running")
                                } else if s.ledger.as_ref().is_some_and(|ledger| ledger.snapshot.failed) {
                                    "failed"
                                } else {
                                    "completed"
                                };
                                let strip = locust_core::mission_strip::mission_strip(
                                    teammate.runtime.as_deref().unwrap_or(""),
                                    teammate.model.as_deref(),
                                    phase,
                                    &header_cost,
                                );
                                let runtime_word = teammate.runtime.as_deref().map(engine::runtime_name).unwrap_or("Agent");
                                let in_locust = (s.engine == Some(Kind::Native) && !native::supported(teammate)).then_some("runs in Locust for now");
                                let route_words = [Some(runtime_word), (!strip.is_empty()).then_some(strip.as_str()), in_locust]
                                    .into_iter()
                                    .flatten()
                                    .collect::<Vec<_>>()
                                    .join(" · ");
                                line.child(
                                    text(route_words)
                                        .key("thread-header-route")
                                        .font(rune::TextStyle::new(12.0, 400.0))
                                        .color(l.c("text-muted"))
                                        .truncate()
                                        .shrink(),
                                )
                            }),
                    )
                    .child(actions),
            )
            .children(crate::exchange_strip::view(s, p))
            .child(transcript_box)
            .child(terminal_panel::commands(s, p))
            .child(column(status))
            .child(commit_panel::view(s, p).map_or_else(row, |panel| row().w_full().px(24.0).pb(8.0).child(panel)))
            .child(column(picker))
            .child(column(composer_stack))
    } else {
        col()
            .grow(1.0)
            .shrink()
            .h_full()
            .center()
            .padding(32.0)
            .child(
                text(s.status.clone())
                    .font(type_::BODY)
                    .color(p.status(s.status_tone))
                    .w_full()
                    .wrap(),
            )
    }
}

/// RoutePicker.tsx (`.lc-picker`): 420 wide on the raised fill with a strong edge and a menu shadow. Each agent's models
/// under its mark and name, one row per model (a dot, its name, the levels it takes, ACTIVE on the one in use), and the
/// level for the turn chosen at the foot, apart from the model, as Electron chose it ("effort being separate").
pub(super) fn model_picker(
    s: &State,
    l: &crate::look::controls::Look,
    id: &str,
    teammate: &Teammate,
    current: Option<&engine::Models>,
    model: &str,
    effort: Option<&str>,
    for_team: bool,
) -> El {
    let mut list = col().key("model-menu").w_full().max_h(330.0).scroll(ScrollSpec { stick_bottom: false }).padding(6.0);
    let Some(agents) = s.models.get(id).filter(|a| !a.is_empty()) else {
        return picker_card(l, list.child(text("Reading the models…").font(l.ty.body).color(l.c("text-muted")).padding(10.0)), None);
    };
    let row_of = |label: String, detail: String, active: bool| -> El {
        let dot = row().w(7.0).h(7.0).radius(4.0).fill(l.c(if active { "lime-text" } else { "green-text" }));
        let mut r = row()
            .w_full()
            .gap(10.0)
            .py(7.0)
            .px(9.0)
            .center_y()
            .radius(l.t.px("radius-control"))
            .hover(l.c("bg-selected"))
            .child(dot)
            .child(text(label).font(l.ty.control).color(l.c("text-primary")))
            .child(col().grow(1.0).child(text(detail).font(l.ty.body).color(l.c("text-muted"))));
        if active {
            r = r
                .fill(l.c("lime-tint"))
                .border(1.0, l.c("lime-border"))
                .child(text("ACTIVE").font(l.ty.mono).color(l.c("lime-text")));
        }
        r.role("button")
    };
    let reset_id = id.to_owned();
    let picked = if for_team { None } else { s.picks.get(id) };
    let needle = route_search_text(&s.model_query.text);
    let searching = !needle.is_empty();
    if !searching && !for_team {
        list = list.child(row_of("Teammate’s own".into(), "its saved model and level".into(), picked.is_none()).on_press(move |s: &mut State| {
            s.picks.remove(&reset_id);
            s.model_menu = None;
        }));
    }
    let policy = locust_core::org_policy::current();
    let active_runtime = picked
        .and_then(|p| p.runtime.as_deref())
        .or(teammate.runtime.as_deref())
        .unwrap_or("codex");
    let active_account: Option<&str> = match picked.and_then(|p| p.account.as_ref()) {
        Some(explicit) => explicit.as_deref(),
        None => teammate.account.as_deref(),
    };
    let mut groups: Vec<(&engine::Models, Option<&locust_core::agent_accounts::Account>)> = agents
        .iter()
        .filter(|a| for_team || id != LEDGER_TARGET || teammate.runtime.as_deref() == Some(a.runtime.as_str()))
        .map(|a| (a, None))
        .collect();
    if locust_core::org_policy::feature_refusal(Some(&policy), "accounts").is_none() {
        for acc in &s.agent_accounts.accounts {
            if let Some(agent) = agents
                .iter()
                .filter(|a| for_team || id != LEDGER_TARGET || teammate.runtime.as_deref() == Some(a.runtime.as_str()))
                .find(|a| a.runtime == acc.agent.as_str())
            {
                groups.push((agent, Some(acc)));
            }
        }
    }
    let offered: Vec<String> = agents.iter()
        .filter(|a| for_team || id != LEDGER_TARGET || teammate.runtime.as_deref() == Some(a.runtime.as_str()))
        .flat_map(|a| a.models.iter().filter(|m| locust_core::org_policy::model_allowed(Some(&policy), &a.runtime, Some(&m.id))).map(|m| format!("{}:{}", a.runtime, m.id)))
        .collect();
    let shortcuts = locust_core::recent_routes::shortcut_keys(&offered, &s.recent_route_keys);
    let mut recent_header = false;
    for key in shortcuts {
        let Some((agent, m)) = agents.iter().find_map(|a| a.models.iter().find(|m| format!("{}:{}",a.runtime,m.id)==key).map(|m|(a,m))) else { continue };
        let label = format!("{} · {}", m.name, locust_core::carry_on::display_name(&agent.runtime));
        if searching && !route_search_text(&label).contains(&needle) { continue; }
        if !recent_header {
            list = list.child(text("Recent").font(l.ty.body).color(l.c("text-muted")).px(9.0).pt(12.0).pb(4.0));
            recent_header = true;
        }
        let detail = if s.limited_runtimes.contains_key(&agent.runtime) { "AT LIMIT".to_owned() } else { s.resolved_model_names.get(&key).cloned().unwrap_or_default() };
        let active = active_runtime == agent.runtime && active_account.is_none() && picked.is_some() && model == m.id;
        let (target, choice, levels) = (id.to_owned(),m.id.clone(),m.efforts.clone());
        let runtime = (teammate.runtime.as_deref()!=Some(agent.runtime.as_str())).then(||agent.runtime.clone());
        let keep = effort.map(str::to_owned);
        list = list.child(row_of(label,detail,active).key(format!("recent-model-{}-{}",agent.runtime,m.id)).on_press(move |s:&mut State| {
            let effort = keep.clone().filter(|e|levels.contains(e));
            apply_model_pick(s, &target, TurnPick {model:choice.clone(),effort,runtime:runtime.clone(),account:Some(None)}, for_team);
            s.model_menu=None;
        }));
    }
    // orderRouteRows: groups by what can run, alphabetical inside a band; inside a group the route in use, then what was
    // used, the flagships, the rest.
    let group_name_of = |agent: &engine::Models, acc: Option<&locust_core::agent_accounts::Account>| {
        let base = locust_core::carry_on::display_name(&agent.runtime);
        match acc {
            Some(a) => format!("{base} · {}", a.name),
            None => base.to_owned(),
        }
    };
    let mut route_rows: Vec<locust_core::route_rows::RouteRow> = Vec::new();
    for (agent, acc) in &groups {
        let acc_id = acc.map(|a| a.id.as_str());
        let tag = if acc.is_some_and(|a| a.state != locust_core::agent_accounts::AccountState::Ready) {
            "SIGN IN"
        } else if s.limited_runtimes.contains_key(&locust_core::agent_accounts::usage_key(&agent.runtime, acc_id)) {
            "AT LIMIT"
        } else {
            "READY"
        };
        let is_active_group = active_runtime == agent.runtime && active_account == acc_id;
        for m in &agent.models {
            let active = is_active_group && (for_team || picked.is_some() || active_account.is_some()) && m.id == model;
            route_rows.push(locust_core::route_rows::RouteRow {
                key: format!("{}:{}", agent.runtime, m.id),
                group: group_name_of(agent, *acc),
                model: Some(m.id.clone()),
                tag: Some(if active { "ACTIVE".to_owned() } else { tag.to_owned() }),
            });
        }
    }
    let ordered = locust_core::route_rows::order_route_rows(&route_rows, &s.recent_route_keys);
    // Each row's place and each group's first place, read once.
    let mut places: HashMap<(&str, &str), usize> = HashMap::new();
    let mut group_first: HashMap<&str, usize> = HashMap::new();
    for (place, &i) in ordered.iter().enumerate() {
        places.insert((route_rows[i].group.as_str(), route_rows[i].key.as_str()), place);
        group_first.entry(route_rows[i].group.as_str()).or_insert(place);
    }
    let place_of = |group: &str, key: &str| places.get(&(group, key)).copied().unwrap_or(usize::MAX);
    groups.sort_by_cached_key(|(agent, acc)| group_first.get(group_name_of(agent, *acc).as_str()).copied().unwrap_or(usize::MAX));
    let mut any = false;
    for (agent, acc) in groups {
        let base_name = locust_core::carry_on::display_name(&agent.runtime);
        let group_name = match acc {
            Some(a) => format!("{base_name} · {}", a.name),
            None => base_name.to_owned(),
        };
        let mut matched: Vec<&engine::Model> = agent
            .models
            .iter()
            .filter(|m| locust_core::org_policy::model_allowed(Some(&policy), &agent.runtime, Some(&m.id)))
            .filter(|m| !searching || route_search_text(&format!("{group_name} {}", m.name)).contains(&needle))
            .collect();
        matched.sort_by_key(|m| place_of(&group_name, &format!("{}:{}", agent.runtime, m.id)));
        // Claude Code's older fixed versions, folded under its group (RoutePicker's fold): searched, they are rows.
        let older: Vec<(String, String, Vec<String>)> = if agent.runtime == "claude" && !agent.models.is_empty() {
            let now: Vec<String> = agent.models.iter().map(|m| m.name.clone()).collect();
            locust_core::models::claude_older_offered(&now)
                .into_iter()
                .filter(|(id, _, _)| locust_core::org_policy::model_allowed(Some(&policy), &agent.runtime, Some(id)))
                .collect()
        } else {
            Vec::new()
        };
        let older_models: Vec<engine::Model> = older.iter().map(|(id, name, efforts)| engine::Model { id: id.clone(), name: name.clone(), efforts: efforts.clone() }).collect();
        let (older_shown, older_folded): (Vec<&engine::Model>, Vec<&engine::Model>) = older_models
            .iter()
            .filter(|m| !searching || route_search_text(&format!("{group_name} {}", m.name)).contains(&needle))
            .partition(|m| searching || (active_runtime == agent.runtime && active_account == acc.map(|a| a.id.as_str()) && m.id == model));
        matched.extend(older_shown);
        if matched.is_empty() && older_folded.is_empty() {
            continue;
        }
        any = true;
        let acc_id = acc.map(|a| a.id.as_str());
        let is_active_group = active_runtime == agent.runtime && active_account == acc_id;
        let active_at = matched.iter().position(|m| is_active_group && (for_team || picked.is_some() || active_account.is_some()) && m.id == model);
        let (shown, hidden) = cap_rows(matched.len(), active_at, if searching { ROUTE_GROUP_LIMIT * 2 } else { ROUTE_GROUP_LIMIT });
        // A row says the model's name, spelled out when the catalogue's name is only its id (RoutePicker).
        let runtime_now = agent.runtime.clone();
        let earned_names = s.resolved_model_names.clone();
        let label_of = move |m: &locust_core::engine::Model| {
            let earned = earned_names.get(&format!("{runtime_now}:{}", m.id)).map(String::as_str);
            locust_core::route_rows::picker_row_label(&runtime_now, &m.id, &m.name, earned)
        };
        let mut group = row().gap(6.0).px(9.0).pt(12.0).pb(4.0).center_y();
        if let Some(mark) = crate::look::controls::agent_mark(l, &agent.runtime, 12.0, l.c("text-muted")) {
            group = group.child(mark.key(format!("picker-group-mark-{}", agent.runtime)));
        }
        list = list.child(group.child(text(group_name.clone()).font(l.ty.body).color(l.c("text-muted"))));
        let usage_key = locust_core::agent_accounts::usage_key(&agent.runtime, acc_id);
        let at_limit = s.limited_runtimes.contains_key(&usage_key);
        let needs_sign_in = acc.is_some_and(|a| a.state != locust_core::agent_accounts::AccountState::Ready);
        for m in shown.into_iter().map(|i| matched[i]) {
            let active = is_active_group && (for_team || picked.is_some() || active_account.is_some()) && m.id == model;
            let detail = if needs_sign_in {
                "SIGN IN".to_owned()
            } else if at_limit {
                "AT LIMIT".to_owned()
            } else {
                String::new()
            };
            // One line: what the catalogue says and what the route resolved to, else its levels in words; everything
            // known, levels included, on the hover (RoutePicker, a-model-is-one-line).
            let description = locust_core::models::catalogue_description(&agent.runtime, &m.id);
            let resolved = s.resolved_model_names.get(&format!("{}:{}", agent.runtime, m.id)).map(String::as_str);
            let detail = if detail.is_empty() {
                let raw = locust_core::route_rows::row_detail(description.as_deref(), resolved, &m.efforts);
                if locust_core::route_rows::row_detail_repeats_heading(&raw, &group_name) {
                    String::new()
                } else {
                    raw
                }
            } else {
                detail
            };
            let title = locust_core::route_rows::row_title(&m.name, description.as_deref(), resolved, &m.efforts);
            let (key, choice, levels) = (id.to_owned(), m.id.clone(), m.efforts.clone());
            let keep = effort.map(str::to_owned);
            // A model of another agent than the teammate's own moves this reply there.
            let runtime = (teammate.runtime.as_deref() != Some(agent.runtime.as_str())).then(|| agent.runtime.clone());
            let account_pick = Some(acc.map(|a| a.id.clone()));
            let (row_label, row_key) = match acc {
                Some(a) => (format!("{} · {}", label_of(m), a.name), format!("model-{}-{}:{}", agent.runtime, m.id, a.id)),
                None => (label_of(m), format!("model-{}-{}", agent.runtime, m.id)),
            };
            let tip_key = format!("{row_key}-tip");
            let drawn = row_of(row_label, detail, active).key(row_key).on_press(move |s: &mut State| {
                // The level stays with the turn when the new model takes it.
                let effort = keep.clone().filter(|e| levels.contains(e));
                apply_model_pick(s, &key, TurnPick { model: choice.clone(), effort, runtime: runtime.clone(), account: account_pick.clone() }, for_team);
                s.model_menu = None;
            });
            list = list.child(crate::hover::tip(s, l, &tip_key, &title, drawn));
        }
        // The fold: how many older versions it holds, and them when it is open.
        if !older_folded.is_empty() {
            let open = s.older_models_open;
            // Keyed as the group's rows are: a Claude account's group has its own fold.
            let suffix = acc.map(|a| format!(":{}", a.id)).unwrap_or_default();
            list = list.child(
                row()
                    .key(format!("picker-fold-{}{suffix}", agent.runtime))
                    .gap(6.0)
                    .px(9.0)
                    .py(6.0)
                    .center_y()
                    .child(text(if open { "▾" } else { "▸" }).font(l.ty.mono).color(l.c("text-muted")).a11y_hidden())
                    .child(text("Older versions").font(l.ty.body).color(l.c("text-muted")))
                    .child(text(older_folded.len().to_string()).key(format!("picker-fold-count-{}{suffix}", agent.runtime)).font(l.ty.mono).color(l.c("text-muted")))
                    .role("button")
                    .label("Older versions")
                    .expanded(open)
                    .on_press(|s: &mut State| s.older_models_open = !s.older_models_open),
            );
            if open {
                for m in older_folded {
                    let (key, choice, runtime) = (id.to_owned(), m.id.clone(), (teammate.runtime.as_deref() != Some(agent.runtime.as_str())).then(|| agent.runtime.clone()));
                    let account_pick = Some(acc.map(|a| a.id.clone()));
                    let (levels, keep) = (m.efforts.clone(), effort.map(str::to_owned));
                    let description = locust_core::models::catalogue_description(&agent.runtime, &m.id);
                    let detail = locust_core::route_rows::row_detail(description.as_deref(), None, &m.efforts);
                    list = list.child(row_of(label_of(m), detail, false).key(format!("model-{}-{}{suffix}", agent.runtime, m.id)).on_press(move |s: &mut State| {
                        let effort = keep.clone().filter(|e| levels.contains(e));
                        apply_model_pick(s, &key, TurnPick { model: choice.clone(), effort, runtime: runtime.clone(), account: account_pick.clone() }, for_team);
                        s.model_menu = None;
                    }));
                }
            }
        }
        // Nothing dropped silently: a capped group says how many it holds back.
        if hidden > 0 {
            let said = format!(
                "{hidden} more {} {}",
                if hidden == 1 { "model" } else { "models" },
                if searching { "match · keep typing" } else { "· type to search them" }
            );
            list = list.child(text(said).font(l.ty.mono).color(l.c("text-muted")).pl(30.0).py(2.0));
        }
    }
    if !any && searching {
        list = list.child(text("Nothing matches that.").font(l.ty.body).color(l.c("text-muted")).padding(10.0));
    }
    // The levels the chosen model takes, at the foot.
    let levels: Vec<String> = agents
        .iter()
        .flat_map(|a| a.models.iter())
        .find(|m| m.id == model)
        .map(|m| m.efforts.clone())
        .or_else(|| current.and_then(|a| a.models.iter().find(|m| Some(m.id.as_str()) == a.model.as_deref())).map(|m| m.efforts.clone()))
        .unwrap_or_default();
    // Claude Code's effort control (EffortSlider in the RoutePicker's effort slot), on the level shown or its default.
    let mut foot = row().w_full().gap(6.0).center_y();
    if !levels.is_empty() && !for_team {
        let (key, pick_model) = (id.to_owned(), model.to_owned());
        let pick_runtime = s.picks.get(id).and_then(|p| p.runtime.clone());
        let pick_account = s.picks.get(id).and_then(|p| p.account.clone());
        let pick: crate::effort_slider::PickLevel = std::rc::Rc::new(move |s: &mut State, level: String| {
            apply_model_pick(s, &key, TurnPick { model: pick_model.clone(), effort: Some(level), runtime: pick_runtime.clone(), account: pick_account.clone() }, for_team);
        });
        let shown_runtime = s.picks.get(id).and_then(|p| p.runtime.as_deref()).or(teammate.runtime.as_deref()).unwrap_or(active_runtime);
        let runtime_default = agents.iter().find(|a| a.runtime == shown_runtime).and_then(|a| a.effort.as_deref());
        let shown_effort = effort.map(str::to_owned).or_else(|| locust_core::effort_levels::default_effort(&levels, runtime_default));
        foot = foot.child(crate::effort_slider::effort_panel(s, l, &levels, shown_effort.as_deref(), shown_runtime, pick));
    }
    // The note on a line of its own: beside four levels it ran past the card's edge.
    let foot = col()
        .w_full()
        .gap(8.0)
        .child(foot)
        .child(text("Privacy and permissions live in Settings.").font(l.ty.body).color(l.c("text-muted")));
    // `.lc-picker__head`: a mono slash, then the search.
    let head = row()
        .w_full()
        .gap(9.0)
        .py(10.0)
        .px(14.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-card"))
        .child(text("/").font(l.ty.mono).color(l.c("text-muted")))
        .child(
            text_input(&s.model_query, l.ty.control)
                .key("model-search")
                .grow(1.0)
                .color(l.c("text-primary"))
                .placeholder("Search runtimes and models", l.c("text-muted"))
                .on_edit(|s: &mut State| &mut s.model_query),
        );
    picker_card_with(l, Some(head), list, Some(foot))
}

fn apply_model_pick(s: &mut State, target: &str, pick: TurnPick, for_team: bool) {
    if for_team {
        crate::people_panel::pick_model(s, pick);
    } else {
        s.picks.insert(target.to_owned(), pick);
    }
}

/// status.ts ROUTE_GROUP_LIMIT: rows a group shows until the person searches (twice that while searching).
const ROUTE_GROUP_LIMIT: usize = locust_core::recent_routes::ROUTE_GROUP_LIMIT;

/// status.ts routeSearchText: lower case, separators as spaces.
fn route_search_text(text: &str) -> String {
    let spaced: String = text.to_lowercase().chars().map(|c| if matches!(c, '-' | '_' | '/' | '.' | ':') { ' ' } else { c }).collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// capRouteRows for one group: the first `limit` rows, the active one kept by taking the last kept place, and how
/// many are held back.
fn cap_rows(len: usize, active: Option<usize>, limit: usize) -> (Vec<usize>, usize) {
    let mut kept: Vec<usize> = (0..len.min(limit)).collect();
    if let Some(a) = active
        && a >= limit
        && let Some(last) = kept.last_mut()
    {
        *last = a;
    }
    (kept, len.saturating_sub(limit))
}

fn picker_card(l: &crate::look::controls::Look, list: El, foot: Option<El>) -> El {
    picker_card_with(l, None, list, foot)
}

fn picker_card_with(l: &crate::look::controls::Look, head: Option<El>, list: El, foot: Option<El>) -> El {
    let mut card = col()
        .w(420.0)
        .radius(11.0)
        .fill(l.c("bg-raised-2"))
        .border(1.0, l.c("border-strong"))
        .box_shadow(l.t.shadows("shadow-menu").first().copied().unwrap_or_default());
    if let Some(head) = head {
        card = card.child(head);
    }
    card = card.child(list);
    if let Some(foot) = foot {
        card = card.child(row().w_full().py(10.0).px(14.0).border_sides([0.5, 0.0, 0.0, 0.0], l.c("border-hairline")).child(foot));
    }
    card
}

#[cfg(test)]
#[path = "picker_empty_tests.rs"]
mod picker_empty_tests;

#[cfg(test)]
mod picker_tests {
    use super::*;

    #[test]
    fn a_group_shows_six_says_how_many_more_and_never_hides_the_one_in_use() {
        assert_eq!(cap_rows(4, None, 6), (vec![0, 1, 2, 3], 0));
        assert_eq!(cap_rows(56, None, 6), (vec![0, 1, 2, 3, 4, 5], 50));
        assert_eq!(cap_rows(56, Some(30), 6), (vec![0, 1, 2, 3, 4, 30], 50));
        assert_eq!(route_search_text("Claude-Opus 5.5"), "claude opus 5 5");
        assert!(route_search_text("Cursor Agent GPT-5.5 Extra High").contains(&route_search_text("gpt 5.5")));
    }

    /// the-runtimes-wear-their-marks.test.tsx: "the picker's runtime groups carry their marks".
    #[test]
    fn the_pickers_runtime_groups_carry_their_marks() {
        let agent = |runtime: &str| engine::Models {
            teammate: None,
            runtime: runtime.into(),
            model: None,
            effort: None,
            models: vec![engine::Model { id: format!("{runtime}-model"), name: format!("{runtime} model"), efforts: vec![] }],
        };
        let mut s = State::default();
        s.models.insert("tm_pick".into(), vec![agent("claude"), agent("cursor")]);
        let mut rt = rune::Runtime::new(
            s,
            |s, _| {
                let teammate = Teammate {
                    id: "tm_pick".into(),
                    name: "Pick".into(),
                    role: "Custom".into(),
                    runtime: Some("claude".into()),
                    model: None,
                    account: None,
                    busy: false,
                    runs_in: String::new(),
                    folder: None,
                };
                model_picker(s, &crate::look::controls::Look::new(true), "tm_pick", &teammate, None, "claude-model", None, true)
            },
            (500.0, 900.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).collect();
        for runtime in ["claude", "cursor"] {
            assert!(keys.iter().any(|k| k == &format!("picker-group-mark-{runtime}")), "{runtime}: {keys:?}");
        }
    }
}

/// Electron's mode menu ("Permission mode"): the five modes, each with what it does; one the agent cannot run is
/// greyed with why. Choosing Auto is what turns Auto on, as in Electron, rather than a trip to Settings first.
fn mode_menu(s: &State, l: &crate::look::controls::Look, teammate: &Teammate, current: &str) -> El {
    let runtime = mode_menu_runtime(s, teammate);
    let mut menu = col()
        .key("mode-menu")
        .w(320.0)
        .padding(4.0)
        .radius(11.0)
        .fill(l.c("bg-raised-2"))
        .border(1.0, l.c("border-strong"))
        .box_shadow(l.t.shadows("shadow-menu").first().copied().unwrap_or_default())
        .role("menu")
        .label("Permission mode");
    let policy = locust_core::org_policy::current();
    for fact in locust_core::mode_facts::MODE_FACTS {
        if !locust_core::org_policy::mode_allowed(Some(&policy), fact.mode) {
            continue;
        }
        let unavailable = locust_core::mode_facts::unavailable_reason(fact.mode, &runtime);
        let chosen = fact.mode == current;
        let said = unavailable.clone().unwrap_or_else(|| fact.consequence.to_owned());
        let tone = if unavailable.is_none() && fact.mode == "auto" { l.c("amber-text") } else { l.c("text-muted") };
        let mut name = row().w_full().gap(8.0).center_y().child(
            text(fact.name).font(l.ty.control).color(if unavailable.is_some() { l.c("text-faint") } else { l.c("text-primary") }).grow(1.0),
        );
        if chosen {
            name = name.child(crate::look::controls::glyph(l, &crate::look::icons::CHECK, 13.0, Some(l.c("text-secondary"))));
        }
        let mut item = col()
            .key(format!("mode-{}", fact.mode))
            .w_full()
            .gap(2.0)
            .py(7.0)
            .px(10.0)
            .radius(l.t.px("radius-control"))
            .child(name)
            .child(text(said).font(l.ty.body).color(tone).w_full().wrap())
            .role("menuitemradio")
            .label(fact.name);
        if unavailable.is_none() {
            let mode = fact.mode;
            item = item.hover(l.c("bg-selected")).on_press(move |s: &mut State| choose_mode(s, mode));
        }
        menu = menu.child(item);
    }
    menu
}

/// The agent the mode menu reads its modes for: the one picked for this turn, else the teammate's.
fn mode_menu_runtime(s: &State, teammate: &Teammate) -> String {
    s.picks
        .get(&teammate.id)
        .and_then(|p| p.runtime.clone())
        .or_else(|| teammate.runtime.clone())
        .unwrap_or_else(|| "codex".into())
}

/// A mode chosen in the menu, by a press or by its digit.
fn choose_mode(s: &mut State, mode: &str) {
    if mode == "auto" {
        enable_auto_mode(s);
    }
    s.composer_mode = Some(mode.to_owned());
    s.mode_menu = false;
}

/// 1-9 in the open mode menu (Composer.tsx over `menuRowFromDigit`): the nth row that can be chosen, counting from the
/// top, as the menu draws them. The window only hears a digit no text field took, so the message box keeps its own.
pub(super) fn mode_menu_digit(s: &mut State, event: &KeyEvent) -> bool {
    use crate::look::select_keys::{Held, menu_row_from_digit};
    if !s.mode_menu {
        return false;
    }
    let Key::Text(typed) = &event.key else { return false };
    let held = Held { ctrl: event.mods.ctrl, meta: event.mods.cmd, alt: event.mods.alt, shift: event.mods.shift };
    let Some(nth) = menu_row_from_digit(typed, held, false) else { return false };
    let Some(teammate) = selected_conversation(s).map(|(t, _)| t.clone()) else { return false };
    let runtime = mode_menu_runtime(s, &teammate);
    let policy = locust_core::org_policy::current();
    let Some(mode) = locust_core::mode_facts::MODE_FACTS
        .iter()
        .filter(|fact| locust_core::org_policy::mode_allowed(Some(&policy), fact.mode))
        .filter(|fact| locust_core::mode_facts::unavailable_reason(fact.mode, &runtime).is_none())
        .nth(nth)
        .map(|fact| fact.mode)
    else {
        return false;
    };
    choose_mode(s, mode);
    true
}

/// Settings' Auto switch, turned on where the person chose Auto (Electron's onEnableAutoMode).
fn enable_auto_mode(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    if let Ok(mut settings) = crate::workspace_settings::load(&store)
        && !settings.auto_mode
    {
        settings.auto_mode = true;
        if crate::workspace_settings::save(&store, &settings).is_ok() {
            s.settings_panel.settings.auto_mode = true;
        }
    }
}

/// `--lc-thread-max-width`: the thread and its message box keep to one centred column, 840 points of content at any
/// ordinary window width (`clamp(760px, 68vw, 840px)`), with their own 24-point sides.
pub(super) const THREAD_MAX: f32 = 840.0 + 48.0;

fn column(inner: El) -> El {
    row().w_full().justify_center().child(inner.max_w(THREAD_MAX))
}

/// What the conversation's header is called (App.tsx `missionOwner?.name ?? shownConversationTitle ?? missionTitle(...)`):
/// its teammate's name; a conversation of no teammate is called what its sidebar row says, found by any of its turns
/// (conversation_list.rs `conversation_title_for`), else the words it was opened with, else the agent's name.
pub(super) fn header_name(s: &State, teammate: &Teammate) -> String {
    if teammate.id != LEDGER_TARGET {
        return teammate.name.clone();
    }
    let pane = s.ledger.as_ref();
    let tip = pane.map(|p| p.snapshot.tip.as_str()).filter(|id| !id.is_empty())
        .or_else(|| s.conversations.get(LEDGER_TARGET).and_then(|c| c.conversation_id.as_deref()));
    crate::conversation_list::conversation_title_for(&crate::sidebar::recent_as_missions(s), tip)
        .map(str::to_owned)
        .or_else(|| pane.and_then(|p| p.conversation.turns.first()).map(|(_, words)| words.clone()).filter(|w| !w.trim().is_empty()))
        .unwrap_or_else(|| teammate.name.clone())
}

/// PlusMenu.tsx's +: at the box's left, closed while a turn runs (Electron's `disabled={running}`).
fn plus_button(s: &State, l: &crate::look::controls::Look) -> El {
    let running = s.flight.is_some() || s.ledger.as_ref().is_some_and(|p| p.snapshot.running);
    crate::look::controls::head_icon(&crate::look::icons::PLUS, "Attach files or choose a folder", "Attach files, or choose the folder teammates work in")
        .disabled(running)
        .on_press(|s: &mut State| {
            s.plus_menu = !s.plus_menu;
            s.mode_menu = false;
            s.reference_menu = false;
        })
        .build(l.k)
        .key("plus")
}

/// The + menu's two choices, in Electron's words: Attach files (the system's picker, opening in the conversation's
/// folder; a file outside it is attached as the drop path attaches one) and Choose a folder (Home's folder dialog).
fn plus_menu(s: &State, l: &crate::look::controls::Look, teammate: &Teammate) -> El {
    let folder = teammate.folder.clone().or_else(|| s.home.folder_path.clone());
    let item = |key: &str, icon: &'static rune::Icon, words: &str, said: &str| {
        row()
            .key(key.to_owned())
            .w_full()
            .gap(10.0)
            .py(7.0)
            .px(10.0)
            .center_y()
            .radius(l.t.px("radius-control"))
            .hover(l.c("bg-selected"))
            .child(crate::look::controls::glyph(l, icon, 14.0, Some(l.c("text-secondary"))))
            .child(
                col()
                    .grow(1.0)
                    .gap(2.0)
                    .child(text(words.to_owned()).font(l.ty.control).color(l.c("text-primary")))
                    .child(text(said.to_owned()).font(l.ty.body).color(l.c("text-muted")).w_full().wrap()),
            )
            .role("menuitem")
            .label(words.to_owned())
    };
    let folder_said = if folder.is_some() { "The folder teammates work in" } else { "None is chosen yet" };
    col()
        .key("plus-menu")
        .w(280.0)
        .padding(4.0)
        .radius(11.0)
        .fill(l.c("bg-raised-2"))
        .border(1.0, l.c("border-strong"))
        .box_shadow(l.t.shadows("shadow-menu").first().copied().unwrap_or_default())
        .role("menu")
        .child(item("plus-attach", &crate::look::icons::ATTACHMENT, "Attach files", "Their paths go with your message").on_press_tx(move |s: &mut State, tx| {
            s.plus_menu = false;
            let options = rune::FileDialogOptions {
                title: "Attach a file".into(),
                directory: folder.clone().map(std::path::PathBuf::from),
                ..Default::default()
            };
            tx.open_file(options, |s: &mut State, answer, _tx| {
                if let Ok(Some(path)) = answer {
                    crate::composer_wiring::attach_dropped_paths(s, &[path]);
                }
            });
        }))
        .child(item("plus-folder", &crate::look::icons::FOLDER, "Choose a folder", folder_said).on_press_tx(|s: &mut State, tx| {
            s.plus_menu = false;
            crate::restored_folders_card::choose_workspace(s, tx);
        }))
}

/// Settings folders retain the ordinary chip treatment; the hover explains why.
fn folder_chip(
    l: &crate::look::controls::Look,
    folder: Option<&str>,
    home: Option<&str>,
    windows: bool,
    system: Option<&str>,
) -> El {
    let word = folder
        .map(locust_core::folders::folder_name)
        .unwrap_or_else(|| "No folder".into());
    crate::look::controls::composer_chip(
        l,
        Some(crate::look::controls::glyph(
            l,
            &crate::look::icons::FOLDER,
            13.0,
            Some(l.c("text-muted")),
        )),
        &word,
        false,
    )
    .key("folder-chip")
    .max_w(260.0)
    .role("button")
    .help(locust_core::thread_view::folder_help(
        folder, home, windows, system,
    ))
}

#[cfg(test)]
mod renderer_folder_chip_tests {
    use super::*;
    fn hover(path: &str) -> rune::Runtime<String> {
        let path = path.to_owned();
        let mut rt = rune::Runtime::new(
            path,
            |path, _| {
                col().padding(24.0).child(folder_chip(
                    &crate::look::controls::Look::new(true),
                    Some(path),
                    Some(r"C:\Users\person"),
                    true,
                    None,
                ))
            },
            (1000., 500.),
            rune::Env::new(true, 1.),
        );
        rt.frame(0.);
        let (x, y) = rt.find("folder-chip").unwrap();
        rt.pointer_move(x, y);
        for _ in 0..100 {
            rt.frame(0.016);
        }
        rt
    }
    fn help_words(tree: &serde_json::Value) -> String {
        match tree {
            serde_json::Value::String(word) => word.clone(),
            serde_json::Value::Array(items) => items.iter().map(help_words).collect::<Vec<_>>().join("\n"),
            serde_json::Value::Object(items) => items.values().map(help_words).collect::<Vec<_>>().join("\n"),
            _ => String::new(),
        }
    }
    fn words(rt: &rune::Runtime<String>) -> String {
        help_words(&rt.accessibility_json())
    }
    #[test]
    fn says_why_in_its_hover_for_a_folder_like_claude_in_the_chips_own_colour() {
        let rt = hover(r"C:\Users\person\.claude");
        assert!(words(&rt).contains("This is .claude, where the agents keep their own settings: a teammate here reads it as its project, sign-in files included."), "{}", words(&rt));
        assert!(!words(&rt).contains("Start this conversation"));
    }
    #[test]
    fn the_thread_attaches_the_warning_to_its_effective_folder_chip() {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap();
        let path = format!("{home}/.claude");
        let mut s = State::default();
        crate::team_scene(&mut s);
        for mate in &mut s.teammates {
            mate.folder = None;
        }
        s.home.folder_path = Some(path.clone());
        let mut rt = rune::Runtime::new(
            s,
            |s, _| thread_view(s, &Palette::new(true)),
            (1200., 900.),
            rune::Env::new(true, 1.),
        );
        rt.frame(0.);
        let (x, y) = rt.find("folder-chip").unwrap();
        rt.pointer_move(x, y);
        for _ in 0..100 {
            rt.frame(0.016);
        }
        let words = help_words(&rt.accessibility_json());
        assert!(words.contains(&format!("New conversations work in {path}. This is .claude,")), "{words}");
    }
    #[test]
    fn stays_as_it_was_for_a_project_folder_control() {
        let rt = hover(r"C:\Users\person\Documents\shop");
        assert!(
            words(&rt).contains(r"New conversations work in C:\Users\person\Documents\shop"),
            "{}",
            words(&rt)
        );
        assert!(!words(&rt).contains("sign-in files"));
    }
}

/// `freeModelsThatAnswered` over the runs this window has listed: a free model that finished a run here is offered
/// before the catalogue's own order (a-free-model-that-answered-comes-first).
pub(super) fn free_models_answered_here(s: &State) -> Vec<String> {
    let runs: Vec<locust_core::connectors_off::RunSeen<'_>> = s
        .ledger_summaries
        .values()
        .map(|m| locust_core::connectors_off::RunSeen {
            runtime: &m.runtime,
            model: &m.model,
            phase: if m.running { "running" } else if m.failed { "failed" } else if m.cancelled { "cancelled" } else { "completed" },
            at: &m.updated,
        })
        .collect();
    locust_core::connectors_off::free_models_that_answered(&runs)
}

#[cfg(test)]
mod free_answered_tests {
    use super::*;

    fn summary(id: &str, model: &str, failed: bool, cancelled: bool, updated: &str) -> locust_core::ledger::Summary {
        locust_core::ledger::Summary { mission_id: id.into(), runtime: "opencode".into(), model: model.into(), failed, cancelled, updated: updated.into(), ..Default::default() }
    }

    /// The window feeds what it has listed into the rule: the newest finished free model first, a stopped one not at all.
    #[test]
    fn the_window_offers_the_free_model_that_finished_a_run_here_and_never_one_that_was_stopped() {
        let mut s = State::default();
        for m in [
            summary("m1", "opencode/muse-spark-1.3-contributor-free", false, false, "2026-09-29T10:00:00Z"),
            summary("m2", "opencode/mimo-v2.6-flash-free", false, false, "2026-09-30T10:00:00Z"),
            summary("m3", "opencode/ling-3.0-flash-fin-free", false, true, "2026-09-30T12:00:00Z"),
        ] {
            s.ledger_summaries.insert(m.mission_id.clone(), m);
        }
        assert_eq!(free_models_answered_here(&s), vec!["opencode/mimo-v2.6-flash-free".to_owned(), "opencode/muse-spark-1.3-contributor-free".to_owned()]);
    }
}

#[cfg(test)]
#[path = "plus_menu_words_tests.rs"]
mod plus_menu_words_tests;

#[cfg(test)]
#[path = "live2_engine_label_tests.rs"]
mod live2_engine_label_tests;

#[cfg(test)]
#[path = "stop_beam_ui_tests.rs"]
mod stop_beam_ui_tests;

/// The message field's line: its own line height, or 1.25 times its size when it names none (Rune's rule).
fn composer_line_height() -> f32 {
    if type_::BODY.line_height > 0.0 { type_::BODY.line_height } else { type_::BODY.size * 1.25 }
}
