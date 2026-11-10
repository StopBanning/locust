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
                .child(message_column(items, &teammate.name, p, s))
        };

        let mut status = row().w_full().center_y().gap(8.0).px(24.0).py(10.0);
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

        let placeholder = composer_placeholder(s);
        let field = text_input(&s.draft, type_::BODY)
            .key("composer")
            .disabled(turn_running(s) && queue_front(s).is_some())
            .placeholder(placeholder, p.muted)
            .color(p.text)
            .w_full()
            .padding(12.0)
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
        let composer = rune::auto_grow(
            field,
            composer::COMPOSER_MIN_LINES,
            composer::COMPOSER_MAX_LINES,
        )
        .max_h(composer::COMPOSER_MAX_HEIGHT_PX)
        .radius(9.0)
        .fill(p.field)
        .border(1.0, p.hairline)
        .grow(1.0)
        .shrink()
        .on_drop_files(|s: &mut State, files, _| attach_dropped_paths(s, files));

        let queueing = can_queue(s);
        let stopping = can_stop(s) && !queueing;
        let blocked = send_blocked_now(s);
        let send_enabled = (can_send(s) || queueing || stopping) && !s.stopping.is_pending();
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
        let mut button = el()
            .key("send")
            .h(48.0)
            .px(18.0)
            .center()
            .radius(9.0)
            .fill(p.raised)
            .border(1.0, p.hairline)
            .disabled(!send_enabled)
            .child(
                text(button_label)
                    .font(type_::LABEL)
                    .color(if send_enabled { p.text } else { p.muted }),
            );
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
        let chip = el()
            .max_w(180.0)
            .h(48.0)
            .padding(6.0)
            .center()
            .radius(6.0)
            .fill(p.raised)
            .child(text(label).font(type_::LABEL).color(p.text).wrap_anywhere())
            .on_press(move |s: &mut State| open_models(s, chip_id.clone()));
        let mut picker = col().w_full().px(24.0);
        if s.model_menu.as_ref() == Some(&id) {
            let mut menu = col()
                .key("model-menu")
                .w_full()
                .max_h(180.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .gap(4.0);
            if let Some(agents) = s.models.get(&id).filter(|a| !a.is_empty()) {
                let reset_id = id.clone();
                menu = menu.child(
                    el().padding(6.0)
                        .child(text("Teammate’s own").color(p.text))
                        .on_press(move |s: &mut State| {
                            s.picks.remove(&reset_id);
                            s.model_menu = None;
                        }),
                );
                for agent in agents.iter().filter(|a| {
                    id != LEDGER_TARGET || teammate.runtime.as_deref() == Some(a.runtime.as_str())
                }) {
                    for model in &agent.models {
                        let efforts = if model.efforts.is_empty() {
                            vec![None]
                        } else {
                            model.efforts.iter().cloned().map(Some).collect()
                        };
                        for effort in efforts {
                            let (id, pick) = (
                                id.clone(),
                                TurnPick {
                                    model: model.id.clone(),
                                    effort,
                                },
                            );
                            let label = format!(
                                "{}{}",
                                model.name,
                                pick.effort
                                    .as_ref()
                                    .map(|e| format!(" · {e}"))
                                    .unwrap_or_default()
                            );
                            menu = menu.child(
                                el().padding(6.0).child(text(label).color(p.text)).on_press(
                                    move |s: &mut State| {
                                        s.picks.insert(id.clone(), pick.clone());
                                        s.model_menu = None;
                                    },
                                ),
                            );
                        }
                    }
                }
            } else {
                menu = menu.child(text("Loading models…").color(p.muted));
            }
            picker = picker.child(menu);
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

        composer_stack = composer_stack.child(
            row()
                .w_full()
                .gap(10.0)
                .child(chip)
                .child(composer)
                .child(button),
        );

        col()
            .grow(1.0)
            .shrink()
            .h_full()
            .child(
                col()
                    .w_full()
                    .h(88.0)
                    .px(24.0)
                    .pt(30.0)
                    .gap(5.0)
                    .window_drag()
                    .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
                    .child(text(teammate.name.clone()).font(type_::TITLE).color(p.text))
                    .child(
                        text(agent_line(s, teammate))
                            .font(type_::LABEL)
                            .color(p.muted),
                    ),
            )
            .child(transcript)
            .child(status)
            .child(picker)
            .child(composer_stack)
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
