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

        let transcript = if items.is_empty() {
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
                .child(row().w_full().justify_center().child(message_column(items, &teammate.name, p, s)))
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
            .child(changes_panel::controls(s, p))
            .children(review_changes::control(s, p))
            .child(terminal_panel::runtime_controls(s, p))
            .child(terminal_panel::toggle(p))
            .child(browser_panel::toggle(p));
        if let Some(btn) = side_chat::header_button(s, p) {
            actions = actions.child(btn);
        }
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
        // Which engine this window is on, always visible (AGENTS.md: a switch is never silent).
        if let Some(kind) = s.engine {
            status = status.child(
                text(kind.label())
                    .key("engine-label")
                    .font(type_::LABEL)
                    .color(p.muted),
            );
        }

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
            .py(7.0)
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
                let (next, note) = composer::attach_paths(&s.attached, &added);
                s.attached = next;
                if let Some(note) = note {
                    s.composer_note = Some(note);
                }
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
        let stopping = can_stop(s) && !queueing && own_command::bang(&s.draft.text).is_none();
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
        let model_name = model_chip_name(model, current);
        let label = format!(
            "{}{}{}",
            model_name,
            effort.map(|e| format!(" · {e}")).unwrap_or_default(),
            if picked.is_some() { ", this turn" } else { "" }
        );
        let chip_id = id.clone();
        let runtime = teammate.runtime.as_deref().or(current.map(|a| a.runtime.as_str())).unwrap_or("codex");
        let mark = crate::look::controls::agent_mark(&l, runtime, 13.0, l.c("text-secondary"));
        let chip = crate::look::controls::composer_chip(&l, mark, &label, true)
            .key("model-chip")
            .on_press(move |s: &mut State| open_models(s, chip_id.clone()));
        let mut picker = col().w_full().px(24.0);
        if s.model_menu.as_ref() == Some(&id) {
            picker = picker.child(row().w_full().justify_end().child(model_picker(s, &l, &id, teammate, current, model, effort)));
        }

        let mut composer_stack = col().w_full().gap(8.0).px(24.0).pb(24.0);

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

        // Attachment chips (path labels; PDF page tiles are not drawn — see M4B-NOTES).
        if !s.attached.is_empty() {
            let mut chips = row().gap(6.0).w_full();
            chips = chips.child(
                text(composer::attachment_label(s.attached.len()))
                    .font(type_::LABEL)
                    .color(p.muted),
            );
            for (i, path) in s.attached.iter().enumerate() {
                let name = path.rsplit('/').next().unwrap_or(path).to_owned();
                chips = chips.child(
                    el().padding(4.0)
                        .radius(4.0)
                        .fill(p.raised)
                        .border(1.0, p.hairline)
                        .child(text(name).font(type_::LABEL).color(p.text))
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
        // Where the turn will run, as native::folder_for chooses: the teammate's own folder, else the one Locust has open.
        let folder_word = teammate
            .folder
            .as_deref()
            .or(s.home.folder_path.as_deref())
            .and_then(|f| f.trim_end_matches('/').rsplit('/').next())
            .filter(|f| !f.is_empty())
            .unwrap_or("No folder");
        let footer = row()
            .w_full()
            .center_y()
            .gap(10.0)
            .px(6.0)
            .child(
                crate::look::controls::composer_chip(&l, None, mode_word, false)
                    .key("mode-chip")
                    .role("button")
                    .label(format!("Mode: {}", locust_core::mode_facts::fact(&mode_now).name))
                    .on_press(|s: &mut State| {
                        s.mode_menu = !s.mode_menu;
                        s.reference_menu = false;
                    }),
            )
            .child(
                crate::look::controls::composer_chip(
                    &l,
                    Some(crate::look::controls::glyph(&l, &crate::look::icons::FOLDER, 13.0, Some(l.c("text-muted")))),
                    folder_word,
                    false,
                )
                .key("folder-chip")
                .max_w(260.0)
                .role("button")
                .on_press(|s: &mut State| s.reference_menu = !s.reference_menu),
            )
            .child(rune::spacer())
            .child(chip);
        if let Some(chips) = crate::reference_chips::view(s, &l, teammate) {
            composer_stack = composer_stack.child(chips);
        }
        composer_stack = composer_stack
            .child(crate::look::controls::composer_box(&l).child(plus_button(s, &l)).child(composer).child(button))
            .child(footer);
        // The footer's menus open above the box, where the model picker does, taking their room from the thread: the
        // box never moves (Electron's menus open upward from their chip).
        if let Some(menu) = crate::reference_chips::folder_menu(s, &l) {
            picker = picker.child(row().w_full().px(6.0).child(menu));
        }
        if s.mode_menu {
            picker = picker.child(row().w_full().px(6.0).child(mode_menu(s, &l, teammate, &mode_now)));
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
                    .child(crate::look::faces::face(s.image_cache.as_deref(), &teammate.id, &teammate.name, s.looks.get(&teammate.id), 36.0, &l))
                    .child(
                        col()
                            .gap(3.0)
                            .grow(1.0)
                            .shrink()
                            .child(
                                row()
                                    .gap(8.0)
                                    .center_y()
                                    .child(text(teammate.name.clone()).font(rune::TextStyle::new(15.0, 600.0)).color(l.c("text-name")))
                                    .child(text(teammate.role.clone()).font(rune::TextStyle::new(12.5, 400.0)).color(l.c("text-muted")).truncate().shrink()),
                            )
                            .child({
                                let mut line = row().gap(5.0).center_y();
                                if let Some(mark) = crate::look::controls::agent_mark(&l, teammate.runtime.as_deref().unwrap_or("codex"), 11.0, l.c("text-muted")) {
                                    line = line.child(mark);
                                }
                                line.child(text(agent_line(s, teammate)).font(rune::TextStyle::new(12.0, 400.0)).color(l.c("text-muted")).truncate().shrink())
                            }),
                    )
                    .child(actions),
            )
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
fn model_picker(
    s: &State,
    l: &crate::look::controls::Look,
    id: &str,
    teammate: &Teammate,
    current: Option<&engine::Models>,
    model: &str,
    effort: Option<&str>,
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
    let picked = s.picks.get(id);
    let needle = route_search_text(&s.model_query.text);
    let searching = !needle.is_empty();
    if !searching {
        list = list.child(row_of("Teammate’s own".into(), "its saved model and level".into(), picked.is_none()).on_press(move |s: &mut State| {
            s.picks.remove(&reset_id);
            s.model_menu = None;
        }));
    }
    let mut any = false;
    for agent in agents.iter().filter(|a| id != LEDGER_TARGET || teammate.runtime.as_deref() == Some(a.runtime.as_str())) {
        let group_name = locust_core::carry_on::display_name(&agent.runtime).to_owned();
        let matched: Vec<&engine::Model> =
            agent.models.iter().filter(|m| !searching || route_search_text(&format!("{group_name} {}", m.name)).contains(&needle)).collect();
        if matched.is_empty() {
            continue;
        }
        any = true;
        let active_at = matched.iter().position(|m| picked.is_some() && m.id == model);
        let (shown, hidden) = cap_rows(matched.len(), active_at, if searching { ROUTE_GROUP_LIMIT * 2 } else { ROUTE_GROUP_LIMIT });
        let mut group = row().gap(6.0).px(9.0).pt(12.0).pb(4.0).center_y();
        if let Some(mark) = crate::look::controls::agent_mark(l, &agent.runtime, 12.0, l.c("text-muted")) {
            group = group.child(mark);
        }
        list = list.child(group.child(text(group_name).font(l.ty.body).color(l.c("text-muted"))));
        for m in shown.into_iter().map(|i| matched[i]) {
            let active = picked.is_some() && m.id == model;
            let detail = if m.efforts.is_empty() { String::new() } else { m.efforts.join(" · ") };
            let (key, choice, levels) = (id.to_owned(), m.id.clone(), m.efforts.clone());
            let keep = effort.map(str::to_owned);
            // A model of another agent than the teammate's own moves this reply there.
            let runtime = (teammate.runtime.as_deref() != Some(agent.runtime.as_str())).then(|| agent.runtime.clone());
            list = list.child(row_of(m.name.clone(), detail, active).key(format!("model-{}-{}", agent.runtime, m.id)).on_press(move |s: &mut State| {
                // The level stays with the turn when the new model takes it.
                let effort = keep.clone().filter(|e| levels.contains(e));
                s.picks.insert(key.clone(), TurnPick { model: choice.clone(), effort, runtime: runtime.clone() });
                s.model_menu = None;
            }));
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
    let mut foot = row().w_full().gap(6.0).center_y();
    if !levels.is_empty() {
        foot = foot.child(text("Level").font(l.ty.body).color(l.c("text-muted")).pr(4.0));
        for level in levels {
            let on = effort == Some(level.as_str());
            let (key, choice, pick_model) = (id.to_owned(), level.clone(), model.to_owned());
            let pick_runtime = s.picks.get(id).and_then(|p| p.runtime.clone());
            foot = foot.child(crate::look::controls::chip(l, &level, on).on_press(move |s: &mut State| {
                s.picks.insert(key.clone(), TurnPick { model: pick_model.clone(), effort: Some(choice.clone()), runtime: pick_runtime.clone() });
            }));
        }
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

/// status.ts ROUTE_GROUP_LIMIT: rows a group shows until the person searches (twice that while searching).
const ROUTE_GROUP_LIMIT: usize = 6;

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
}

/// Electron's mode menu ("Permission mode"): the five modes, each with what it does; one the agent cannot run is
/// greyed with why. Choosing Auto is what turns Auto on, as in Electron, rather than a trip to Settings first.
fn mode_menu(s: &State, l: &crate::look::controls::Look, teammate: &Teammate, current: &str) -> El {
    let runtime = s
        .picks
        .get(&teammate.id)
        .and_then(|p| p.runtime.clone())
        .or_else(|| teammate.runtime.clone())
        .unwrap_or_else(|| "codex".into());
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
    for fact in locust_core::mode_facts::MODE_FACTS {
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
            item = item.hover(l.c("bg-selected")).on_press(move |s: &mut State| {
                if mode == "auto" {
                    enable_auto_mode(s);
                }
                s.composer_mode = Some(mode.to_owned());
                s.mode_menu = false;
            });
        }
        menu = menu.child(item);
    }
    menu
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
            crate::home_panel::choose_folder(tx);
        }))
}
