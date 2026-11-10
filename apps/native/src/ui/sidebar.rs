use super::*;
use crate::conversation_list::{
    SidebarMission, conversation_keys, held_for, narrowing_line, pinned_and_recent,
};
use crate::conversation_search::missions_matching;
use crate::settled::is_put_away;

pub(super) fn recent_as_missions(s: &State) -> Vec<SidebarMission> {
    let mut rows = Vec::new();
    let by_id: HashMap<_, _> = s.recent.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut groups: Vec<(String, Vec<&ConversationRow>)> = Vec::new();
    for item in &s.recent {
        let mut root = item.id.as_str();
        let mut seen = std::collections::HashSet::from([root]);
        while let Some(parent) = s
            .ledger_summaries
            .get(root)
            .and_then(|m| m.parent.as_deref())
        {
            if !by_id.contains_key(parent) || !seen.insert(parent) {
                break;
            }
            root = parent;
        }
        if let Some((_, members)) = groups.iter_mut().find(|(key, _)| key == root) {
            members.push(item);
        } else {
            groups.push((root.to_owned(), vec![item]));
        }
    }
    for (root, mut members) in groups {
        members.sort_by_key(|m| {
            s.ledger_summaries
                .get(&m.id)
                .map(|m| m.created_at.as_str())
                .unwrap_or("")
        });
        let continued: std::collections::HashSet<_> = members
            .iter()
            .filter_map(|m| s.ledger_summaries.get(&m.id)?.parent.as_deref())
            .collect();
        let Some(leaf) = members
            .iter()
            .rev()
            .find(|m| !continued.contains(m.id.as_str()))
            .or_else(|| members.last())
        else {
            continue;
        };
        let mut mission = SidebarMission {
            mission_id: leaf.id.clone(),
            title: by_id
                .get(root.as_str())
                .map(|m| m.title.clone())
                .unwrap_or_else(|| leaf.title.clone()),
            phase: leaf.state.clone(),
            last_at: Some(leaf.updated.clone()),
            root_id: Some(root),
            member_ids: members.iter().map(|m| m.id.clone()).collect(),
            owner_id: None,
            said: members
                .iter()
                .filter_map(|m| s.ledger_summaries.get(&m.id).map(|m| m.words.as_str()))
                .collect::<Vec<_>>()
                .join(" · "),
        };
        if let Some(title) = held_for(&mission, &s.org.titles) {
            mission.title = title.clone();
        }
        rows.push(mission);
    }
    crate::conversation_list::conversation_rows(&rows)
}

pub(super) fn sidebar_missions_now(s: &State) -> Vec<SidebarMission> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let selected = selected_conversation(s)
        .and_then(|(_, c)| c.conversation_id.clone())
        .or_else(|| s.ledger.as_ref().map(|p| p.snapshot.tip.clone()));
    let query = s.sidebar_query.text.clone();
    let searching = !query.trim().is_empty();
    let missions = recent_as_missions(s);
    let visible: Vec<SidebarMission> = missions
        .into_iter()
        .filter(|m| {
            let entry = held_for(m, &s.org.settled);
            let put_away = is_put_away(m.last_at.as_deref(), entry, now_ms);
            if !put_away {
                return true;
            }
            // Settled rows hide unless selected or searching (Electron Sidebar).
            searching
                || selected
                    .as_deref()
                    .is_some_and(|id| conversation_keys(m).iter().any(|k| k == id))
        })
        .collect();
    let matched: Vec<SidebarMission> = missions_matching(
        &visible,
        &query,
        Some(|row: &SidebarMission| {
            s.recent
                .iter()
                .find(|r| r.id == row.mission_id)
                .map(|r| r.teammate.clone())
        }),
    )
    .into_iter()
    .cloned()
    .collect();
    matched
}

fn mission_row(s: &State, p: &Palette, item: &SidebarMission, picked: bool) -> El {
    let recent = s.recent.iter().find(|r| r.id == item.mission_id);
    let mut title = row().w_full().center_y().gap(6.0).child(
        text(item.title.clone())
            .font(type_::BODY)
            .color(p.text)
            .grow(1.0)
            .shrink()
            .truncate(),
    );
    let dot = match item.phase.as_str() {
        "running" => Some(p.green),
        "failed" => Some(p.red),
        _ => None,
    };
    if let Some(ink) = dot {
        title = title.child(el().size(5.0, 5.0).radius(2.5).fill(ink));
    }
    let id = item.mission_id.clone();
    let subtitle = recent
        .map(|item| {
            format!(
                "{}{} · {}",
                item.teammate,
                if item.started_by == "native" {
                    " · native"
                } else {
                    ""
                },
                when(&item.updated, clock_seconds()),
            )
        })
        .unwrap_or_default();
    if picked || s.recent_hover.as_deref() == Some(&item.mission_id) {
        let menu_id = item.mission_id.clone();
        title = title.child(
            text("⋯")
                .label(format!("Actions for {}", item.title))
                .key(format!("org-menu-open-{menu_id}"))
                .font(type_::BODY)
                .color(p.secondary)
                .padding(4.0)
                .on_press(move |s: &mut State| s.org_menu_id = Some(menu_id.clone())),
        );
    } else {
        title = title.child(el().size(22.0, 22.0));
    }
    let hover_id = id.clone();
    let mut row_el = col()
        .on_hover_change(move |s: &mut State, over, _| {
            if over {
                s.recent_hover = Some(hover_id.clone());
            } else if s.recent_hover.as_deref() == Some(&hover_id) {
                s.recent_hover = None;
            }
        })
        .key(format!("recent-{id}"))
        .w_full()
        .padding(12.0)
        .gap(5.0)
        .radius(8.0)
        .fill(if picked { p.selected } else { p.sidebar })
        .child(title)
        .child(
            text(subtitle)
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .truncate(),
        )
        .on_press_tx(move |s: &mut State, tx| {
            if let Some(item) = s.recent.iter().find(|r| r.id == id).cloned() {
                open_recent(s, item, tx);
            }
        });
    if s.org_menu_id.as_deref() == Some(item.mission_id.as_str()) {
        row_el = row_el.child(org_menu(s, p, &item.mission_id));
    }
    row_el
}

fn org_menu(s: &State, p: &Palette, mission_id: &str) -> El {
    let pinned = s.org.pins.iter().any(|k| {
        let missions = recent_as_missions(s);
        missions
            .iter()
            .any(|m| m.mission_id == mission_id && conversation_keys(m).iter().any(|id| id == k))
            || k == mission_id
    });
    let settled = recent_as_missions(s)
        .iter()
        .find(|m| m.mission_id == mission_id)
        .is_some_and(|m| held_for(m, &s.org.settled).is_some());
    let id = mission_id.to_owned();
    let mut menu = col().w_full().gap(4.0).pt(6.0);
    let actions: [(&str, &str); 4] = [
        ("Open beside", "beside"),
        ("Rename", "rename"),
        (if pinned { "Unpin" } else { "Pin to top" }, "pin"),
        (if settled { "Bring back" } else { "Settle" }, "settle"),
    ];
    for (label, action) in actions {
        let id = id.clone();
        let action = action.to_owned();
        menu = menu.child(
            text(label.to_owned())
                .key(format!("org-{action}-{id}"))
                .font(type_::LABEL)
                .color(p.secondary)
                .on_press(move |s: &mut State| {
                    apply_org_action(s, &id, &action);
                }),
        );
    }
    menu = menu.child(text("Snooze").font(type_::LABEL).color(p.muted));
    for choice in settled::snooze_choices(chrono::Local::now()) {
        let id = id.clone();
        menu = menu.child(
            text(choice.label)
                .font(type_::LABEL)
                .color(p.secondary)
                .on_press(move |s: &mut State| {
                    apply_org_action(s, &id, &format!("snooze:{}", choice.until));
                }),
        );
    }
    menu = menu.child(text("Move to project").font(type_::LABEL).color(p.muted));
    for (gid, label) in std::iter::once((String::new(), "Ungrouped".to_owned())).chain(
        s.org
            .groups
            .iter()
            .map(|g| (g.group_id.clone(), g.name.clone())),
    ) {
        let id = id.clone();
        menu = menu.child(text(label).font(type_::LABEL).color(p.secondary).on_press(
            move |s: &mut State| {
                apply_org_action(s, &id, &format!("group:{gid}"));
            },
        ));
    }
    let id = id.clone();
    menu.child(
        text("New project…")
            .font(type_::LABEL)
            .color(p.secondary)
            .on_press(move |s: &mut State| {
                apply_org_action(s, &id, "new-project");
            }),
    )
}

pub(super) fn apply_org_action(s: &mut State, mission_id: &str, action: &str) {
    let Some(store) = s.data.clone() else {
        s.status = "No native data folder for conversation organization.".into();
        s.status_tone = StatusTone::Amber;
        s.org_menu_id = None;
        return;
    };
    let row = recent_as_missions(s)
        .into_iter()
        .find(|m| conversation_keys(m).iter().any(|k| k == mission_id));
    let stable_id = row
        .as_ref()
        .and_then(|m| m.root_id.clone())
        .unwrap_or_else(|| mission_id.to_owned());
    let mission_id = if action == "beside" {
        mission_id
    } else {
        stable_id.as_str()
    };
    let result = match action {
        "beside" => {
            let title = row
                .as_ref()
                .map(|r| r.title.clone())
                .unwrap_or_else(|| "Conversation".into());
            let owner_name = s
                .recent
                .iter()
                .find(|r| r.id == mission_id)
                .map(|r| r.teammate.clone());
            let middle = selected_conversation(s)
                .and_then(|(_, c)| c.conversation_id.clone())
                .or_else(|| s.ledger.as_ref().map(|p| p.snapshot.tip.clone()));
            if beside::beside_allowed(middle.as_deref(), mission_id) {
                let snapshot = ledger_dir()
                    .map(|dir| native::session(dir, mission_id.into()).poll())
                    .transpose()
                    .map_err(|e| e.to_string());
                let snapshot = match snapshot {
                    Ok(value) => value,
                    Err(error) => {
                        s.status = error;
                        s.status_tone = StatusTone::Amber;
                        return;
                    }
                };
                let lines = snapshot
                    .as_ref()
                    .map(|snap| {
                        snap.items
                            .iter()
                            .filter_map(|item| match item {
                                ledger::Item::User(text) | ledger::Item::Assistant { text, .. } => {
                                    Some(text.clone())
                                }
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                s.beside = Some(beside::BesideState {
                    mission_id: mission_id.into(),
                    title,
                    owner_name,
                    prompt: String::new(),
                    reply_lines: lines,
                    phase: "completed".into(),
                });
            }
            Ok(())
        }
        "new-project" => {
            s.rename_id = Some(format!("@new-project:{mission_id}"));
            s.rename_draft.clear();
            s.rename_draft.focused = true;
            Ok(())
        }
        "rename" => {
            s.rename_id = Some(mission_id.into());
            let current = s.org.titles.get(mission_id).cloned().unwrap_or_default();
            s.rename_draft.replace_text(&current);
            Ok(())
        }
        "pin" => {
            let keys = row
                .as_ref()
                .map(conversation_keys)
                .unwrap_or_else(|| vec![mission_id.into()]);
            org::toggle_conversation_pin(&store, &mut s.org, mission_id, &keys)
        }
        "settle" => {
            let keys = row
                .as_ref()
                .map(conversation_keys)
                .unwrap_or_else(|| vec![mission_id.into()]);
            org::toggle_conversation_settled(&store, &mut s.org, mission_id, &keys)
        }
        action if action.starts_with("snooze:") => org::settle_mission(
            &store,
            &mut s.org,
            mission_id,
            Some(Some(action[7..].into())),
        ),
        action if action.starts_with("group:") => org::assign_conversation(
            &store,
            &mut s.org,
            mission_id,
            &row.as_ref()
                .map(conversation_keys)
                .unwrap_or_else(|| vec![mission_id.into()]),
            if action.len() == 6 {
                None
            } else {
                Some(&action[6..])
            },
        ),
        _ => Ok(()),
    };
    if let Err(words) = result {
        s.status = words;
        s.status_tone = StatusTone::Amber;
    }
    s.org_menu_id = None;
}

fn section_label(p: &Palette, words: &str) -> El {
    text(words.to_owned())
        .font(type_::LABEL)
        .color(p.secondary)
        .w_full()
        .px(12.0)
        .py(8.0)
}

pub(super) fn sidebar(s: &State, p: &Palette) -> El {
    let mut rows = col().w_full().gap(6.0).padding(10.0);

    for (index, teammate) in s.teammates.iter().enumerate() {
        let initial = teammate
            .name
            .trim_start()
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_else(|| "?".into());
        let selected = s
            .ledger
            .as_ref()
            .map(|pane| pane.header.id == teammate.id)
            .unwrap_or(s.selected == index);
        let busy = teammate.busy
            || s.flight
                .as_ref()
                .is_some_and(|f| f.teammate_id == teammate.id)
            || s.ledger
                .as_ref()
                .is_some_and(|pane| pane.header.id == teammate.id && pane.snapshot.running);

        let mut title = row()
            .w_full()
            .center_y()
            .gap(8.0)
            .child(
                el().size(22.0, 22.0)
                    .radius(11.0)
                    .center()
                    .fill(p.raised)
                    .child(text(initial).font(type_::LABEL).color(p.secondary)),
            )
            .child(
                text(teammate.name.clone())
                    .font(type_::TITLE)
                    .color(p.text)
                    .grow(1.0)
                    .shrink(),
            );
        if busy {
            title = title.child(el().size(6.0, 6.0).radius(3.0).fill(p.green));
        }
        let id = teammate.id.clone();
        rows = rows.child(
            col()
                .key(format!("teammate-{id}"))
                .w_full()
                .padding(12.0)
                .gap(5.0)
                .radius(8.0)
                .fill(if selected { p.selected } else { p.sidebar })
                .child(title)
                .child(
                    text(teammate.role.clone())
                        .font(type_::BODY)
                        .color(p.secondary)
                        .w_full()
                        .pl(30.0)
                        .wrap_anywhere(),
                )
                .child(
                    text(agent_line(s, teammate))
                        .font(type_::LABEL)
                        .color(p.muted)
                        .w_full()
                        .pl(30.0)
                        .wrap_anywhere(),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    select_teammate(s, &id, tx);
                }),
        );
    }

    rows = rows.child(
        text("Teammates & Memory")
            .font(type_::BODY)
            .color(p.secondary)
            .padding(12.0)
            .on_press(crate::people_panel::open),
    );
    let missions = sidebar_missions_now(s);
    let all_count = recent_as_missions(s).len();
    let (pinned, recent_rows) = pinned_and_recent(&missions, &s.org.pins);
    let show_sections = !s.org.pins.is_empty();
    let groups_shown = !s.org.groups.is_empty() && !s.org.groups_unreadable;

    let mut recent = col().w_full().px(10.0).pb(16.0).gap(4.0).child(
        row()
            .w_full()
            .center_y()
            .px(12.0)
            .py(10.0)
            .child(
                text("Recent")
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .grow(1.0),
            )
            .child(
                text("Refresh")
                    .key("refresh-recent")
                    .font(type_::LABEL)
                    .color(p.muted)
                    .on_press(refresh_recent),
            ),
    );

    recent = recent.child(
        text_input(&s.sidebar_query, type_::BODY)
            .key("sidebar-search")
            .w_full()
            .color(p.text)
            .placeholder("Search conversations", p.muted)
            .padding(10.0)
            .radius(6.0)
            .fill(p.field)
            .on_edit(|s: &mut State| &mut s.sidebar_query),
    );

    if let Some(line) = narrowing_line(
        None,
        &s.sidebar_query.text,
        missions.len(),
        missions.len(),
        all_count,
    ) {
        recent = recent.child(
            text(line)
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .px(12.0),
        );
    }

    let unreadable: Vec<_> = [
        (s.org.groups_unreadable, "groups"),
        (s.org.teammates_org_unreadable, "teammates"),
    ]
    .into_iter()
    .filter_map(|(bad, name)| bad.then_some(name))
    .collect();
    if let Some((happened, safe)) = crate::conversation_list::unreadable_sentence(&unreadable) {
        recent = recent
            .child(
                text(happened)
                    .font(type_::LABEL)
                    .color(p.amber)
                    .w_full()
                    .px(12.0)
                    .wrap(),
            )
            .child(
                text(safe)
                    .font(type_::LABEL)
                    .color(p.muted)
                    .w_full()
                    .px(12.0)
                    .wrap(),
            );
    }

    if let Some(words) = &s.recent_error {
        recent = recent.child(
            text(words.clone())
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .px(12.0)
                .wrap_anywhere(),
        );
    } else if missions.is_empty() && s.recent.is_empty() {
        recent = recent.child(
            text(if s.recents.is_pending() {
                "Loading…"
            } else if !s.sidebar_query.text.trim().is_empty() {
                "Nothing in here yet."
            } else {
                "No recent conversations."
            })
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .px(12.0)
            .wrap(),
        );
    } else if missions.is_empty() {
        recent = recent.child(
            text("Nothing in here yet.")
                .font(type_::LABEL)
                .color(p.muted)
                .w_full()
                .px(12.0)
                .wrap(),
        );
    } else {
        if show_sections && !pinned.is_empty() {
            recent = recent.child(section_label(p, "Pinned"));
            for item in &pinned {
                let picked = selected_conversation(s)
                    .and_then(|(_, c)| c.conversation_id.as_deref())
                    == Some(item.mission_id.as_str())
                    || s.ledger.as_ref().is_some_and(|p| {
                        !p.snapshot.tip.is_empty() && p.snapshot.tip == item.mission_id
                    });
                recent = recent.child(mission_row(s, p, item, picked));
            }
        }
        // Project groups
        if groups_shown {
            for group in &s.org.groups {
                recent = recent.child(section_label(p, &group.name));
                let in_group: Vec<_> = recent_rows
                    .iter()
                    .filter(|m| {
                        held_for(m, &s.org.members).is_some_and(|gid| gid == &group.group_id)
                    })
                    .collect();
                for item in in_group {
                    let picked = selected_conversation(s)
                        .and_then(|(_, c)| c.conversation_id.as_deref())
                        == Some(item.mission_id.as_str());
                    recent = recent.child(mission_row(s, p, item, picked));
                }
            }
        }
        let rest = if groups_shown {
            recent_rows
                .iter()
                .filter(|m| held_for(m, &s.org.members).is_none())
                .cloned()
                .collect::<Vec<_>>()
        } else {
            recent_rows.clone()
        };
        if !rest.is_empty() && (show_sections || groups_shown) {
            recent = recent.child(section_label(
                p,
                if groups_shown { "Ungrouped" } else { "Recents" },
            ));
        }
        for item in &rest {
            let picked = selected_conversation(s).and_then(|(_, c)| c.conversation_id.as_deref())
                == Some(item.mission_id.as_str())
                || s.ledger.as_ref().is_some_and(|p| {
                    !p.snapshot.tip.is_empty() && p.snapshot.tip == item.mission_id
                });
            recent = recent.child(mission_row(s, p, item, picked));
        }
    }

    if let Some(id) = &s.rename_id {
        let id = id.clone();
        recent = recent.child(
            col()
                .w_full()
                .px(12.0)
                .gap(6.0)
                .child(
                    text(if id.starts_with("@new-project:") {
                        "Name this project"
                    } else {
                        "Name this conversation"
                    })
                    .font(type_::LABEL)
                    .color(p.secondary),
                )
                .child(
                    text_input(&s.rename_draft, type_::BODY)
                        .key("rename-draft")
                        .w_full()
                        .color(p.text)
                        .placeholder(
                            if id.starts_with("@new-project:") {
                                "Name this project"
                            } else {
                                "Name this conversation"
                            },
                            p.muted,
                        )
                        .padding(8.0)
                        .fill(p.field)
                        .on_edit(|s: &mut State| &mut s.rename_draft),
                )
                .child(
                    text("Save")
                        .key("rename-save")
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .on_press(move |s: &mut State| {
                            if let Some(store) = s.data.clone() {
                                let title = s.rename_draft.text.clone();
                                if let Err(words) = if let Some(mission) =
                                    id.strip_prefix("@new-project:")
                                {
                                    org::create_group(&store, &mut s.org, &title).and_then(|gid| {
                                        org::assign_to_group(
                                            &store,
                                            &mut s.org,
                                            mission,
                                            Some(&gid),
                                        )
                                    })
                                } else {
                                    {
                                        let keys = recent_as_missions(s)
                                            .iter()
                                            .find(|m| conversation_keys(m).contains(&id))
                                            .map(conversation_keys)
                                            .unwrap_or_else(|| vec![id.clone()]);
                                        org::rename_conversation(
                                            &store, &mut s.org, &id, &keys, &title,
                                        )
                                    }
                                } {
                                    s.status = words;
                                    s.status_tone = StatusTone::Amber;
                                }
                            }
                            s.rename_id = None;
                            s.rename_draft.clear();
                        }),
                ),
        );
    }

    col()
        .w(248.0)
        .h_full()
        .fill(p.sidebar)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.hairline)
        .child(
            col()
                .w_full()
                .h(88.0)
                .pt(48.0)
                .pl(52.0)
                .pr(20.0)
                .window_drag()
                .child(text("LOCUST").font(type_::LABEL).color(p.secondary))
                .child(
                    text(if s.engine == Some(Kind::Native) {
                        "Native team copy"
                    } else {
                        ""
                    })
                    .font(type_::LABEL)
                    .color(p.muted),
                ),
        )
        .child(
            col()
                .key("team-list")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(col().w_full().child(rows).child(recent)),
        )
}

#[cfg(test)]
mod review_tests {
    use super::*;
    fn chain() -> State {
        let mut s = State::default();
        s.demo = true;
        for (id, parent, words, time) in [
            ("ship", Some("signup"), "Good ship it", "03"),
            ("signup", Some("root"), "Check signup", "02"),
            ("root", None, "Login redirect", "01"),
            ("other", None, "Other", "00"),
        ] {
            let at = format!("2026-10-01T10:00:{time}Z");
            s.recent.push(ConversationRow {
                id: id.into(),
                title: words.into(),
                teammate: "Atlas".into(),
                started_by: "native".into(),
                state: "completed".into(),
                updated: at.clone(),
            });
            s.ledger_summaries.insert(
                id.into(),
                ledger::Summary {
                    mission_id: id.into(),
                    title: words.into(),
                    parent: parent.map(str::to_owned),
                    words: words.into(),
                    runtime: "codex".into(),
                    model: String::new(),
                    created_at: at.clone(),
                    updated: at,
                    running: false,
                    failed: false,
                },
            );
        }
        s
    }
    #[test]
    fn real_sidebar_rows_collapse_turns_keep_names_groups_and_search_every_prompt() {
        let mut s = chain();
        s.org.titles.insert("signup".into(), "Chosen name".into());
        s.org.members.insert("root".into(), "project".into());
        let rows = recent_as_missions(&s);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].mission_id, "ship");
        assert_eq!(rows[0].root_id.as_deref(), Some("root"));
        assert_eq!(rows[0].member_ids, ["root", "signup", "ship"]);
        assert_eq!(rows[0].title, "Chosen name");
        assert_eq!(
            held_for(&rows[0], &s.org.members).map(String::as_str),
            Some("project")
        );
        s.sidebar_query.replace_text("signup");
        assert_eq!(sidebar_missions_now(&s).len(), 1);
        s.sidebar_query.replace_text("atlas");
        assert_eq!(sidebar_missions_now(&s).len(), 2);
        s.ledger_summaries.get_mut("root").unwrap().parent = Some("ship".into());
        assert!(!recent_as_missions(&s).is_empty(), "cycles terminate");
    }

    #[test]
    fn palette_and_find_keys_step_wrap_close_and_execute() {
        let mut s = chain();
        let mut tx = Tx::default();
        let event = |key, shift| KeyEvent {
            key,
            mods: rune::Mods {
                shift,
                ..Default::default()
            },
        };
        assert!(on_key(&mut s, &event(Key::Command('k'), false), &mut tx));
        assert!(s.palette_open);
        assert!(on_key(&mut s, &event(Key::Up, false), &mut tx));
        assert_eq!(s.palette_index, s.palette_actions.len() - 1);
        assert!(on_key(&mut s, &event(Key::Down, false), &mut tx));
        assert_eq!(s.palette_index, 0);
        assert!(on_key(&mut s, &event(Key::Enter, false), &mut tx));
        assert!(!s.palette_open);
        s.find_open = true;
        s.find_pieces = vec!["word word".into()];
        s.find_show_earlier = true;
        s.find_query.replace_text("word");
        on_key(&mut s, &event(Key::Enter, false), &mut tx);
        assert_eq!(s.find_index, 1);
        on_key(&mut s, &event(Key::Enter, true), &mut tx);
        assert_eq!(s.find_index, 0);
        on_key(&mut s, &event(Key::Escape, false), &mut tx);
        assert!(!s.find_open);
        assert!(s.find_query.text.is_empty());
    }
}
