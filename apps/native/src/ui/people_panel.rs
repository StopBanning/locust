use super::*;
use serde_json::{Value, json};
#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub memory_tab: bool,
    pub entry: Value,
    pub fields: Vec<TextEdit>,
    pub note: TextEdit,
    pub memory: TextEdit,
    pub editing: Option<String>,
    pub team: Value,
    pub memories: Value,
    pub error: String,
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
}
fn reload(s: &mut State) {
    let result = s
        .data
        .as_ref()
        .ok_or("Native data folder could not be determined.".into())
        .and_then(|store| Ok((people::team(store)?, people::memories(store)?)));
    match result {
        Ok((team, memories)) => {
            s.people_panel.note = field(team["settings"]["aboutYou"].as_str().unwrap_or(""));
            s.people_panel.team = team;
            s.people_panel.memories = memories;
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
            .child(
                button("Teammates", p).on_press(|s: &mut State| s.people_panel.memory_tab = false),
            )
            .child(button("Memory", p).on_press(|s: &mut State| s.people_panel.memory_tab = true)),
    );
    if !panel.memory_tab {
        body = body.child(text("Teammates").font(type_::TITLE).color(p.text));
        for t in panel.team["teammates"].as_array().into_iter().flatten() {
            let id = t["teammateId"].as_str().unwrap_or("").to_owned();
            let delete_id = id.clone();
            body = body.child(
                row()
                    .w_full()
                    .child(
                        button(t["name"].as_str().unwrap_or(""), p)
                            .on_press(move |s: &mut State| edit(s, Some(&id))),
                    )
                    .child(button("Remove teammate", p).on_press(move |s: &mut State| {
                        let result = s
                            .data
                            .as_ref()
                            .ok_or("No native data folder".into())
                            .and_then(|store| people::remove_teammate(store, &delete_id));
                        done(s, result);
                    })),
            );
        }
        body = body.child(button("New teammate", p).on_press(|s: &mut State| edit(s, None)));
        for (i, label) in [
            "Name",
            "Role",
            "Instructions",
            "Runtime",
            "Model",
            "Effort",
            "Mode",
            "Folder",
        ]
        .iter()
        .enumerate()
        {
            if let Some(edit) = panel.fields.get(i) {
                body = body
                    .child(text(*label).font(type_::LABEL).color(p.secondary))
                    .child(
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
        body = body.child(
            button(
                if panel.entry["teammateId"].as_str().unwrap_or("").is_empty() {
                    "Create teammate"
                } else {
                    "Save changes"
                },
                p,
            )
            .on_press(save),
        );
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
