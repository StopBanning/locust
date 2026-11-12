use super::*;
use serde_json::{Value, json};
#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub memory_tab: bool,
    /// 0 teammates, 1 memory, 2 saved approvals
    pub tab: u8,
    pub entry: Value,
    pub fields: Vec<TextEdit>,
    pub note: TextEdit,
    pub memory: TextEdit,
    pub editing: Option<String>,
    pub team: Value,
    pub memories: Value,
    pub rules: Vec<crate::approval_rules::ApprovalRule>,
    pub confirming_remove_all: bool,
    pub error: String,
    /// The edit form is open (a teammate's pencil, or New teammate); closed, the Team screen is the cards alone.
    pub form_open: bool,
    /// The teammate whose remove button was pressed once, asking before it removes (Electron's armed confirm).
    pub removing: Option<String>,
}
fn field(value: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.replace_text(value);
    edit
}
pub(super) fn open(s: &mut State) {
    s.people_panel.open = true;
    reload(s);
    edit(s, None);
    s.people_panel.form_open = false;
    s.people_panel.removing = None;
}
fn reload(s: &mut State) {
    let result = s
        .data
        .as_ref()
        .ok_or("Native data folder could not be determined.".into())
        .and_then(|store| {
            Ok((
                people::team(store)?,
                people::memories(store)?,
                crate::approval_rule_store::list(store).unwrap_or_default(),
            ))
        });
    match result {
        Ok((team, memories, rules)) => {
            s.people_panel.note = field(team["settings"]["aboutYou"].as_str().unwrap_or(""));
            s.people_panel.team = team;
            s.people_panel.memories = memories;
            s.people_panel.rules = rules;
        }
        Err(e) => s.people_panel.error = e,
    }
}
fn edit(s: &mut State, id: Option<&str>) {
    let entry = id.and_then(|id|s.people_panel.team["teammates"].as_array()?.iter().find(|t|t["teammateId"] == id)).cloned().unwrap_or_else(||json!({"teammateId":"","role":"Custom","route":{"runtime":"codex","model":"account-default","mode":"ask"}}));
    s.people_panel.fields = [
        entry["name"].as_str().unwrap_or(""),
        entry["role"].as_str().unwrap_or("Custom"),
        entry["instructions"].as_str().unwrap_or(""),
        entry["route"]["runtime"].as_str().unwrap_or("codex"),
        entry["route"]["model"]
            .as_str()
            .unwrap_or("account-default"),
        entry["route"]["effort"].as_str().unwrap_or(""),
        entry["route"]["mode"].as_str().unwrap_or("ask"),
        entry["folder"].as_str().unwrap_or(""),
    ]
    .into_iter()
    .map(field)
    .collect();
    s.people_panel.entry = entry;
    s.people_panel.form_open = true;
}
fn done(s: &mut State, result: Result<(), String>) {
    match result {
        Ok(()) => {
            s.people_panel.error.clear();
            reload(s);
            if let Some(store) = &s.data {
                match people::team(store).and_then(|v| native::parse_roster(&v.to_string())) {
                    Ok(team) => {
                        s.engine = Some(Kind::Native);
                        apply_team(s, team);
                    }
                    Err(e) => s.people_panel.error = e,
                }
            }
        }
        Err(e) => s.people_panel.error = e,
    }
}
fn save(s: &mut State) {
    let mut entry = s.people_panel.entry.clone();
    let fields = &s.people_panel.fields;
    if fields.len() != 8 {
        return;
    }
    for (i, key) in [(0, "name"), (1, "role"), (2, "instructions"), (7, "folder")] {
        entry[key] = json!(fields[i].text.trim());
    }
    if fields[7].text.trim().is_empty()
        && let Some(obj) = entry.as_object_mut()
    {
        obj.remove("folder");
    }
    if !entry["route"].is_object() {
        entry["route"] = json!({});
    }
    for (i, key) in [(3, "runtime"), (4, "model"), (6, "mode")] {
        entry["route"][key] = json!(fields[i].text.trim());
    }
    if !fields[5].text.trim().is_empty() {
        entry["route"]["effort"] = json!(fields[5].text.trim());
    }
    let result = s
        .data
        .as_ref()
        .ok_or("Native data folder could not be determined.".into())
        .and_then(|store| people::save_teammate(store, entry));
    done(s, result);
    if s.people_panel.error.is_empty() {
        s.people_panel.form_open = false;
    }
}
fn action(s: &mut State, id: &str, action: &str) {
    let result = s
        .data
        .as_ref()
        .ok_or("Native data folder could not be determined.".into())
        .and_then(|store| {
            people::change_memory(store, id, action, Some(&s.people_panel.memory.text))
        });
    done(s, result);
}
fn button(label: &str, p: &Palette) -> El {
    text(label.to_owned())
        .font(type_::BODY)
        .color(p.secondary)
        .padding(6.0)
}
pub(super) fn view(s: &State, p: &Palette) -> El {
    let panel = &s.people_panel;
    let mut body = col()
        .w_full()
        .padding(22.0)
        .gap(10.0)
        .child(
            row()
                .w_full()
                .child(
                    text("Teammates & Memory")
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Close", p).on_press(|s: &mut State| s.people_panel.open = false)),
        )
        .child(
            text("Native copy · seeded once from Electron; changes stay in native.")
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        );
    if !panel.error.is_empty() {
        body = body.child(text(panel.error.clone()).color(p.red).wrap());
    }
    body = body.child(
        row()
            .child(button("Teammates", p).on_press(|s: &mut State| {
                s.people_panel.tab = 0;
                s.people_panel.memory_tab = false;
            }))
            .child(button("Memory", p).on_press(|s: &mut State| {
                s.people_panel.tab = 1;
                s.people_panel.memory_tab = true;
            }))
            .child(button("Saved approvals", p).on_press(|s: &mut State| {
                s.people_panel.tab = 2;
                s.people_panel.memory_tab = false;
            })),
    );
    if panel.tab == 2 {
        body = body
            .child(
                text(crate::approval_rule_store::SAVED_APPROVALS_HEADING)
                    .font(type_::TITLE)
                    .color(p.text),
            )
            .child(
                text(crate::approval_rule_store::SAVED_APPROVALS_LEDE)
                    .font(type_::LABEL)
                    .color(p.muted)
                    .wrap(),
            )
            .child(
                text(crate::approval_rule_store::SAVED_APPROVALS_NOTE)
                    .font(type_::LABEL)
                    .color(p.muted)
                    .wrap(),
            );
        if panel.rules.is_empty() {
            body = body.child(
                text(crate::approval_rule_store::SAVED_APPROVALS_EMPTY)
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .wrap(),
            );
        } else {
            for rule in &panel.rules {
                let id = rule.rule_id.clone();
                let tag = if rule.effect == crate::approval_rules::RuleEffect::Allow {
                    "ALLOW"
                } else {
                    "NEVER"
                };
                let sentence = crate::approval_rules::rule_sentence(rule, None);
                let uses = rule
                    .uses
                    .filter(|&n| n > 0)
                    .map(|n| format!(" Answered {n} {}.", if n == 1 { "card" } else { "cards" }))
                    .unwrap_or_default();
                body = body.child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(
                            text(tag.to_owned())
                                .font(type_::LABEL)
                                .color(if tag == "ALLOW" { p.green } else { p.red }),
                        )
                        .child(
                            text(format!("{sentence}{uses}"))
                                .font(type_::LABEL)
                                .color(p.text)
                                .grow(1.0)
                                .wrap(),
                        )
                        .child(
                            button(crate::approval_rule_store::SAVED_APPROVALS_REMOVE, p).on_press(
                                move |s: &mut State| {
                                    let result = s
                                        .data
                                        .as_ref()
                                        .ok_or("No native data folder".into())
                                        .and_then(|store| {
                                            crate::approval_rule_store::remove(store, &id)?;
                                            Ok(())
                                        });
                                    done(s, result);
                                },
                            ),
                        ),
                );
            }
            if panel.confirming_remove_all {
                let count = panel.rules.len();
                let confirm = if count == 1 {
                    "Remove the 1 saved rule? Every card asks again afterwards.".to_owned()
                } else {
                    format!("Remove all {count} saved rules? Every card asks again afterwards.")
                };
                body = body.child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(
                            text(confirm)
                                .font(type_::LABEL)
                                .color(p.text)
                                .grow(1.0)
                                .wrap(),
                        )
                        .child(
                            button(crate::approval_rule_store::SAVED_APPROVALS_REMOVE_ALL, p)
                                .on_press(|s: &mut State| {
                                    let result = s
                                        .data
                                        .as_ref()
                                        .ok_or("No native data folder".into())
                                        .and_then(|store| {
                                            crate::approval_rule_store::remove_all(store)?;
                                            Ok(())
                                        });
                                    s.people_panel.confirming_remove_all = false;
                                    done(s, result);
                                }),
                        )
                        .child(
                            button(crate::approval_rule_store::SAVED_APPROVALS_KEEP_THEM, p)
                                .on_press(|s: &mut State| {
                                    s.people_panel.confirming_remove_all = false;
                                }),
                        ),
                );
            } else {
                body = body.child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(
                            text(crate::approval_rule_store::SAVED_APPROVALS_START_OVER)
                                .font(type_::LABEL)
                                .color(p.muted)
                                .grow(1.0)
                                .wrap(),
                        )
                        .child(
                            button(
                                crate::approval_rule_store::SAVED_APPROVALS_REMOVE_ALL_RULES,
                                p,
                            )
                            .on_press(|s: &mut State| {
                                s.people_panel.confirming_remove_all = true;
                            }),
                        ),
                );
            }
        }
    } else if !panel.memory_tab {
        let count = panel.team["teammates"].as_array().map_or(0, Vec::len);
        body = body.child(
            row()
                .w_full()
                .gap(10.0)
                .center_y()
                .pt(6.0)
                .child(text("Team").font(rune::TextStyle::new(20.0, 600.0)).color(p.text))
                .child(text(format!("{count} {}", if count == 1 { "teammate" } else { "teammates" })).font(type_::LABEL).color(p.muted)),
        );
        if panel.form_open {
            body = body.child(edit_form(s, p));
        }
        body = body.child(team_grid(s, p));
    } else {
        body = body
            .child(text("About you").font(type_::TITLE).color(p.text))
            .child(
                text_input(&panel.note, type_::BODY)
                    .key("about-you")
                    .label("About you")
                    .w_full()
                    .padding(8.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.people_panel.note),
            )
            .child(button("Save", p).on_press(|s: &mut State| {
                let result = s
                    .data
                    .as_ref()
                    .ok_or("No native data folder".into())
                    .and_then(|store| people::settings(store, &s.people_panel.note.text, None));
                done(s, result);
            }));
        for suggestion in panel.team["settings"]["aboutYouSuggestions"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let id = suggestion["id"].clone();
            body = body.child(
                text(suggestion["text"].as_str().unwrap_or("").to_owned())
                    .color(p.text)
                    .wrap(),
            );
            for (label, add) in [("Add to About you", true), ("Dismiss", false)] {
                let id = id.clone();
                body = body.child(button(label, p).on_press(move |s: &mut State| {
                    let suggestions: Vec<_> =
                        s.people_panel.team["settings"]["aboutYouSuggestions"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|r| r["id"] != id)
                            .cloned()
                            .collect();
                    let line = s.people_panel.team["settings"]["aboutYouSuggestions"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|r| r["id"] == id)
                        .and_then(|r| r["text"].as_str())
                        .unwrap_or("");
                    let note = if add {
                        format!("{}\n{}", s.people_panel.note.text, line)
                    } else {
                        s.people_panel.note.text.clone()
                    };
                    let result = s
                        .data
                        .as_ref()
                        .ok_or("No native data folder".into())
                        .and_then(|store| people::settings(store, &note, Some(suggestions)));
                    done(s, result);
                }));
            }
        }
        body = body
            .child(text("Memory").font(type_::TITLE).color(p.text))
            .child(
                text_input(&panel.memory, type_::BODY)
                    .label("Memory text")
                    .key("memory-text")
                    .w_full()
                    .padding(8.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.people_panel.memory),
            );
        body = body.child(button("Remember everywhere", p).on_press(|s: &mut State| {
            let result = s
                .data
                .as_ref()
                .ok_or("No native data folder".into())
                .and_then(|store| {
                    people::add_memory(
                        store,
                        &s.people_panel.memory.text,
                        None,
                        "global",
                        "native",
                        json!({"name":"you"}),
                        "kept",
                    )
                });
            done(s, result);
        }));
        for memory in panel.memories["memories"].as_array().into_iter().flatten() {
            let id = memory["memoryId"].as_str().unwrap_or("").to_owned();
            let mut card = col()
                .w_full()
                .padding(12.0)
                .gap(5.0)
                .radius(8.0)
                .fill(p.raised)
                .child(
                    text(memory["text"].as_str().unwrap_or("").to_owned())
                        .color(p.text)
                        .wrap(),
                )
                .child(
                    text(format!(
                        "{} · {} · {}",
                        memory["by"]["name"].as_str().unwrap_or(""),
                        memory["scope"].as_str().unwrap_or(""),
                        memory["status"].as_str().unwrap_or("")
                    ))
                    .font(type_::LABEL)
                    .color(p.muted),
                );
            if let Some(previous) = memory["previousText"].as_str() {
                card = card.child(text(format!("Before: {previous}")).color(p.muted).wrap());
            }
            for (label, act) in [
                ("Keep", "keep"),
                ("Edit", "edit"),
                (
                    if memory["enabled"] == true {
                        "Switch off"
                    } else {
                        "Switch on"
                    },
                    "toggle",
                ),
                ("Remove", "remove"),
            ] {
                if act == "keep" && memory["status"] != "proposed" {
                    continue;
                }
                let id = id.clone();
                card = card.child(button(label, p).on_press(move |s: &mut State| {
                    if act == "edit" && s.people_panel.editing.as_deref() != Some(&id) {
                        if let Some(m) = s.people_panel.memories["memories"]
                            .as_array()
                            .and_then(|r| r.iter().find(|m| m["memoryId"] == id))
                        {
                            s.people_panel.memory = field(m["text"].as_str().unwrap_or(""));
                            s.people_panel.editing = Some(id.clone());
                        }
                    } else {
                        action(s, &id, act);
                        s.people_panel.editing = None;
                    }
                }));
            }
            body = body.child(card);
        }
        body = body.child(button("Suggest rewrite", p).on_press(|s: &mut State| {
            let result = s
                .data
                .as_ref()
                .ok_or("No native data folder".into())
                .and_then(|store| {
                    let id = s
                        .people_panel
                        .editing
                        .clone()
                        .ok_or("Choose Edit on a memory first.")?;
                    let m = s.people_panel.memories["memories"]
                        .as_array()
                        .and_then(|rows| rows.iter().find(|m| m["memoryId"] == id))
                        .ok_or("That memory no longer exists.")?;
                    people::propose_tidy(
                        store,
                        m["workspaceId"].as_str().unwrap_or(""),
                        &[id],
                        &s.people_panel.memory.text,
                        false,
                    )
                });
            done(s, result);
        }));
        body = body.child(text("Recently forgotten").font(type_::TITLE).color(p.text));
        for held in panel.memories["forgotten"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
        {
            let id = held["memory"]["memoryId"].as_str().unwrap_or("").to_owned();
            body = body
                .child(
                    text(held["memory"]["text"].as_str().unwrap_or("").to_owned())
                        .color(p.muted)
                        .wrap(),
                )
                .child(
                    button("Restore", p).on_press(move |s: &mut State| action(s, &id, "restore")),
                );
        }
    }
    col()
        .key("people-scroll")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(body)
}

/// Electron's agoLabel: "just now", "5 minutes ago", "1 week ago".
fn ago_label(iso: &str) -> Option<String> {
    let at = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let seconds = (chrono::Utc::now() - at.with_timezone(&chrono::Utc)).num_seconds();
    if seconds < 60 {
        return Some("just now".into());
    }
    for (size, name) in [(604_800, "week"), (86_400, "day"), (3_600, "hour"), (60, "minute")] {
        let count = seconds / size;
        if count >= 1 {
            return Some(format!("{count} {name}{} ago", if count == 1 { "" } else { "s" }));
        }
    }
    None
}

/// The teammate's edit form (create or change), as a card above the grid while it is open.
fn edit_form(s: &State, p: &Palette) -> El {
    let panel = &s.people_panel;
    let l = crate::look::controls::Look::new(p.dark);
    let creating = panel.entry["teammateId"].as_str().unwrap_or("").is_empty();
    let heading = if creating { "New teammate".to_owned() } else { format!("Edit {}", panel.entry["name"].as_str().unwrap_or("teammate")) };
    let mut form = col()
        .key("team-form")
        .w_full()
        .gap(8.0)
        .padding(16.0)
        .radius(10.0)
        .border(1.0, l.c("border-hairline"))
        .fill(l.c("bg-raised"))
        .child(text(heading).font(rune::TextStyle::new(14.5, 500.0)).color(l.c("text-name")));
    for (i, label) in ["Name", "Role", "Instructions", "Runtime", "Model", "Effort", "Mode", "Folder"].iter().enumerate() {
        if let Some(edit) = panel.fields.get(i) {
            form = form.child(text(*label).font(type_::LABEL).color(p.secondary)).child(
                text_input(edit, type_::BODY)
                    .key(format!("person-field-{i}"))
                    .label(*label)
                    .w_full()
                    .padding(8.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(move |s: &mut State| &mut s.people_panel.fields[i]),
            );
        }
    }
    form.child(
        row()
            .gap(8.0)
            .pt(4.0)
            .child(button(if creating { "Create teammate" } else { "Save changes" }, p).key("team-save").on_press(save))
            .child(button("Cancel", p).on_press(|s: &mut State| s.people_panel.form_open = false)),
    )
}

/// Screens.tsx's roster (`.lc-rostergrid`, `.lc-rostercard`): a card per teammate with its face, name and role, the
/// message, edit and remove buttons, its route and mode, how many conversations and when it last ran, and its two most
/// recent conversations; then the New teammate card.
fn team_grid(s: &State, p: &Palette) -> El {
    let panel = &s.people_panel;
    let l = crate::look::controls::Look::new(p.dark);
    let mono = |size: f32| rune::TextStyle::new(size, 400.0).mono();
    let mut grid = row().key("team-grid").w_full().gap(12.0).flex_wrap();
    for t in panel.team["teammates"].as_array().into_iter().flatten() {
        let id = t["teammateId"].as_str().unwrap_or("").to_owned();
        let name = t["name"].as_str().unwrap_or("").to_owned();
        let role = t["role"].as_str().unwrap_or("").to_owned();
        let runtime = t["route"]["runtime"].as_str().unwrap_or("").to_owned();
        let model = t["route"]["model"].as_str().unwrap_or("").to_owned();
        let mode = t["route"]["mode"].as_str().unwrap_or("ask").to_owned();
        let mut owned: Vec<&ConversationRow> = s.recent.iter().filter(|r| r.teammate == name).collect();
        owned.sort_by(|a, b| b.updated.cmp(&a.updated));
        let last = owned.first().and_then(|r| ago_label(&r.updated));

        let mut head = row()
            .w_full()
            .gap(12.0)
            .child(crate::look::faces::face(s.image_cache.as_deref(), &id, &name, s.looks.get(&id), 40.0, &l))
            .child(
                col()
                    .grow(1.0)
                    .shrink()
                    .gap(2.0)
                    .child(text(name.clone()).font(rune::TextStyle::new(14.5, 500.0)).color(l.c("text-name")).w_full().truncate())
                    .child(text(role).font(rune::TextStyle::new(13.0, 400.0)).color(l.c("text-muted")).w_full().truncate()),
            );
        if panel.removing.as_deref() == Some(id.as_str()) {
            let yes = id.clone();
            head = head.child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(text(format!("Remove {name}?")).font(type_::LABEL).color(l.c("text-secondary")))
                    .child(button("Remove", p).key(format!("team-remove-yes-{id}")).on_press(move |s: &mut State| {
                        let result = s.data.as_ref().ok_or("No native data folder".into()).and_then(|store| people::remove_teammate(store, &yes));
                        s.people_panel.removing = None;
                        done(s, result);
                    }))
                    .child(button("Keep", p).on_press(|s: &mut State| s.people_panel.removing = None)),
            );
        } else {
            let (message, edit_id, remove_id) = (id.clone(), id.clone(), id.clone());
            let small = |icon: &'static rune::Icon, label: String, help: &str| {
                crate::look::controls::head_icon(icon, &label, help).control_size(rune_kit::ControlSize::Small)
            };
            head = head
                .child(small(&crate::look::icons::MESSAGE, format!("Message {name}"), "Message teammate").on_press_tx(move |s: &mut State, tx| {
                    s.people_panel.open = false;
                    select_teammate(s, &message, tx);
                }).build(l.k).key(format!("team-message-{id}")))
                .child(small(&crate::look::icons::PENCIL, format!("Edit {name}"), "Edit teammate").on_press(move |s: &mut State| edit(s, Some(&edit_id))).build(l.k).key(format!("team-edit-{id}")))
                .child(small(&crate::look::icons::CLOSE, format!("Remove {name}"), "Remove teammate").on_press(move |s: &mut State| s.people_panel.removing = Some(remove_id.clone())).build(l.k).key(format!("team-remove-{id}")));
        }

        let mut route = row().w_full().gap(6.0).center_y();
        if !runtime.is_empty() {
            if let Some(mark) = crate::look::controls::agent_mark(&l, &runtime, 12.0, l.c("text-secondary")) {
                route = route.child(mark);
            }
            route = route.child(
                text(format!("{} / {}", locust_core::carry_on::display_name(&runtime), model))
                    .font(mono(11.5))
                    .color(l.c("text-secondary"))
                    .grow(1.0)
                    .shrink()
                    .truncate(),
            );
        }
        let mode_chip = row().child(crate::look::controls::chip(&l, locust_core::mode_facts::fact(&mode).chip, false));

        let stat = |label: &str, value: String, quiet: bool| {
            col()
                .gap(2.0)
                .child(text(label.to_owned()).font(mono(11.5)).color(l.c("text-muted")))
                .child(text(value).font(l.ty.control).color(if quiet { l.c("text-muted") } else { l.c("text-primary") }))
        };
        let stats = row()
            .w_full()
            .gap(24.0)
            .child(stat("Runs", owned.len().to_string(), owned.is_empty()))
            .child(stat("Last run", last.clone().unwrap_or_else(|| "never".into()), last.is_none()));

        let mut card = col()
            .key(format!("team-card-{id}"))
            .w(300.0)
            .grow(1.0)
            .min_w(260.0)
            .gap(14.0)
            .padding(16.0)
            .radius(10.0)
            .border(1.0, l.c("border-hairline"))
            .fill(l.c("bg-raised"))
            .child(head)
            .child(col().w_full().gap(6.0).child(route).child(mode_chip))
            .child(stats);
        if !owned.is_empty() {
            let mut recent = col()
                .w_full()
                .gap(4.0)
                .pt(10.0)
                .border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"))
                .child(text("RECENT").font(rune::TextStyle::new(10.5, 400.0).mono().tracking(1.0)).color(l.c("text-muted")));
            for r in owned.iter().take(2) {
                let mut line = row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(el().size(5.0, 5.0).radius(2.5).fill(l.c("text-faint")))
                    .child(text(r.title.clone()).font(rune::TextStyle::new(13.0, 400.0)).color(l.c("text-secondary")).grow(1.0).shrink().truncate());
                if let Some(age) = ago_label(&r.updated) {
                    line = line.child(text(age).font(mono(11.0)).color(l.c("text-muted")));
                }
                recent = recent.child(line);
            }
            card = card.child(recent);
        }
        grid = grid.child(card);
    }
    grid.child(
        col()
            .key("team-new")
            .w(300.0)
            .min_w(260.0)
            .gap(10.0)
            .padding(16.0)
            .radius(10.0)
            .border(1.0, l.c("border-hairline"))
            .child(crate::look::controls::head_icon(&crate::look::icons::PLUS, "New teammate", "New teammate").on_press(|s: &mut State| edit(s, None)).build(l.k))
            .child(text("New teammate").font(rune::TextStyle::new(14.5, 500.0)).color(l.c("text-name")))
            .child(text("Name, role and avatar. Their conversations group under them.").font(rune::TextStyle::new(13.0, 400.0)).color(l.c("text-muted")).w_full().wrap()),
    )
}
