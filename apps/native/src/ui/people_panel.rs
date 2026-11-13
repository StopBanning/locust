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
    /// How memory is kept (`memoryMode`: auto, ask, off), read with the team.
    pub memory_mode: String,
    /// The saved account id for Claude Code / Codex teammates (`None` = this computer's sign-in).
    pub account: Option<String>,
    /// Whether the Share your team card modal is open.
    pub share_card_open: bool,
    /// Routines snapshot for the Share your team card preview.
    pub share_card_routines: Value,
}
fn field(value: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.replace_text(value);
    edit
}
pub(super) fn open(s: &mut State) {
    crate::room_screen::close(s);
    s.people_panel.open = true;
    reload(s);
    edit(s, None);
    s.people_panel.form_open = false;
    s.people_panel.removing = None;
}
pub(super) fn open_edit(s: &mut State, id: &str) {
    crate::room_screen::close(s);
    s.home.home_open = false;
    s.home.board_open = false;
    s.settings_panel.open = false;
    s.routines.open = false;
    s.people_panel.open = true;
    reload(s);
    edit(s, Some(id));
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
            s.people_panel.memory_mode = team["settings"]["memoryMode"].as_str().unwrap_or(crate::workspace_settings::DEFAULT_MEMORY_MODE).to_owned();
            s.people_panel.note = field(team["settings"]["aboutYou"].as_str().unwrap_or(""));
            s.people_panel.team = team;
            s.people_panel.memories = memories;
            s.people_panel.rules = rules;
            agent_accounts_card::reload(s);
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
    s.people_panel.account = entry["route"]["account"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
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
/// Own copy on or off. Only "on" is written, as Electron writes it.
fn toggle_own_copy(s: &mut State) {
    if s.people_panel.entry["worktree"] == true {
        if let Some(entry) = s.people_panel.entry.as_object_mut() {
            entry.remove("worktree");
        }
    } else {
        s.people_panel.entry["worktree"] = json!(true);
    }
}

fn build_entry_from_form(s: &State) -> Option<Value> {
    let mut entry = s.people_panel.entry.clone();
    let fields = &s.people_panel.fields;
    if fields.len() != 8 {
        return None;
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
    if matches!(fields[3].text.trim(), "claude" | "codex") {
        entry["route"]["account"] = match &s.people_panel.account {
            Some(acc) => json!(acc),
            None => Value::Null,
        };
    } else {
        entry["route"]["account"] = Value::Null;
    }
    Some(entry)
}

fn save(s: &mut State) {
    let Some(entry) = build_entry_from_form(s) else {
        return;
    };
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

fn save_and_interview(s: &mut State, tx: &mut Tx) {
    let Some(entry) = build_entry_from_form(s) else {
        return;
    };
    let saved = s
        .data
        .as_ref()
        .ok_or("Native data folder could not be determined.".into())
        .and_then(|store| people::save_teammate_id(store, entry));
    let teammate_id = match saved {
        Ok(id) => {
            done(s, Ok(()));
            id
        }
        Err(e) => {
            done(s, Err(e));
            return;
        }
    };
    if !s.people_panel.error.is_empty() {
        return;
    }
    s.people_panel.form_open = false;
    s.people_panel.open = false;
    s.conversations.remove(&teammate_id);
    select_teammate(s, &teammate_id, tx);
    let Some(mate) = s.teammates.iter().find(|t| t.id == teammate_id).cloned() else {
        return;
    };
    let prompt = locust_core::interview::interview_prompt(&mate.name, &mate.role);
    let native_job = flight_teammate(s, &teammate_id);
    s.own_turn = None;
    s.flight = Some(Flight {
        teammate_id: teammate_id.clone(),
        conversation_id: None,
        header: Some(mate),
        after_tip: None,
        message: Some(prompt.clone()),
    });
    set_status(s, &teammate_id, "Sending…", StatusTone::Muted);
    s.sending
        .request((teammate_id, prompt, native_job), move |(_id, message, native_job)| {
            let teammate = native_job
                .ok_or_else(|| "That teammate is no longer in the team.".to_string())?;
            locust_core::engine_api::api().send(
                native::Send {
                    teammate,
                    conversation: None,
                    prompt: message,
                    model: None,
                    effort: None,
                },
                interview_origin(),
            )
        });
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
                .child(text(format!("{count} {}", if count == 1 { "teammate" } else { "teammates" })).font(type_::LABEL).color(p.muted).grow(1.0))
                .child(button("Add team from image", p).key("team-import-image").on_press_tx(|_s: &mut State, tx: &mut Tx| {
                    import_team_from_image(tx);
                }))
                .child(button("Share your team", p).key("team-share").on_press(|s: &mut State| {
                    s.people_panel.share_card_routines = s
                        .data
                        .as_ref()
                        .and_then(|store| locust_core::routines::list(store).ok())
                        .unwrap_or_else(|| json!({"routines": []}));
                    s.people_panel.share_card_open = true;
                })),
        );
        if panel.share_card_open {
            body = body.child(share_card_view(s, p));
        }
        if panel.form_open {
            body = body.child(edit_form(s, p));
        }
        body = body.child(team_grid(s, p));
    } else {
        body = body.child(memory_screen(s, p));
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
pub(super) fn ago_label(iso: &str) -> Option<String> {
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
    let runtime_now = panel.fields.get(3).map(|f| f.text.trim()).unwrap_or("");
    if let Some(agent) = locust_core::agent_accounts::Agent::parse(runtime_now) {
        let policy = locust_core::org_policy::current();
        let allowed = locust_core::org_policy::feature_refusal(Some(&policy), "accounts").is_none();
        let matches: Vec<&locust_core::agent_accounts::Account> = if allowed {
            s.agent_accounts.accounts.iter().filter(|a| a.agent == agent).collect()
        } else {
            Vec::new()
        };
        if !matches.is_empty() || panel.account.is_some() {
            let mut choices = row().w_full().gap(6.0).center_y();
            let pick_btn = |title: String, key: String, selected: bool, p: &Palette| {
                row()
                    .child(
                        text(title)
                            .font(type_::LABEL)
                            .color(if selected { p.text } else { p.secondary }),
                    )
                    .padding(6.0)
                    .fill(if selected { p.selected } else { p.field })
                    .radius(6.0)
                    .key(key)
            };
            choices = choices.child(
                pick_btn(
                    "This computer's sign-in".to_owned(),
                    "team-account-default".to_owned(),
                    panel.account.is_none(),
                    p,
                )
                .on_press(|s: &mut State| s.people_panel.account = None),
            );
            for acc in matches {
                let acc_id = acc.id.clone();
                let selected = panel.account.as_deref() == Some(acc.id.as_str());
                let label = if acc.state == locust_core::agent_accounts::AccountState::Ready {
                    acc.name.clone()
                } else {
                    format!("{} (sign in)", acc.name)
                };
                choices = choices.child(
                    pick_btn(label, format!("team-account-{}", acc.id), selected, p)
                        .on_press(move |s: &mut State| s.people_panel.account = Some(acc_id.clone())),
                );
            }
            form = form
                .child(text("Account").font(type_::LABEL).color(p.secondary))
                .child(choices);
        }
    }
    // Own copy (Electron NewTeammateDialog): the teammate's turns run in its own worktree of the folder, on its branch.
    let own_copy = panel.entry["worktree"] == true;
    let name_now = panel.fields.first().map(|f| f.text.trim().to_owned()).unwrap_or_default();
    let branch = locust_core::worktrees::branch_name_for(if name_now.is_empty() { "teammate" } else { &name_now }, None);
    form = form.child(
        row()
            .gap(10.0)
            .center_y()
            .child(crate::look::controls::switch(&l, own_copy, "Own copy", false).key("team-own-copy").on_press(toggle_own_copy))
            .child(
                col()
                    .gap(2.0)
                    .grow(1.0)
                    .shrink()
                    .child(text("Own copy").font(type_::LABEL).color(p.text))
                    .child(
                        text(format!(
                            "Works in its own copy of the folder, on branch {branch}. Needs the folder to be a git repository. Merging back is yours to do."
                        ))
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .wrap(),
                    ),
            ),
    );
    let who = if name_now.is_empty() { "this teammate" } else { &name_now };
    let who_pronoun = if name_now.is_empty() { "it" } else { &name_now };
    let interview_label = if creating {
        format!("Let {who} interview you instead")
    } else {
        format!("Let {who} interview you again")
    };
    let interview_note = format!(
        "{} {who}, then opens a conversation where {who_pronoun} asks you a few short questions and writes its instructions for you to keep. The interview only asks: nothing is changed in the folder.",
        if creating { "Creates" } else { "Saves" }
    );
    form = form.child(
        col()
            .gap(4.0)
            .pt(4.0)
            .child(button(&interview_label, p).key("team-interview").on_press_tx(save_and_interview))
            .child(text(interview_note).font(type_::LABEL).color(p.secondary).wrap()),
    );
    if !creating
        && let Some(tm_id) = panel.entry["teammateId"].as_str().filter(|s| !s.is_empty())
        && let Some(store) = s.data.as_ref()
    {
        let lessons = locust_core::tune_lesson_store::list(store, Some(tm_id));
        if !lessons.is_empty() {
            let mut lcol = col()
                .key("team-tune-lessons")
                .w_full()
                .gap(4.0)
                .pt(4.0)
                .child(text("Tune lessons").font(type_::LABEL).color(p.text));
            for lesson in lessons {
                let lid = lesson.lesson_id.clone();
                lcol = lcol.child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(
                            text(format!("[{}] {}", lesson.verdict, lesson.lesson))
                                .font(type_::LABEL)
                                .color(p.secondary)
                                .grow(1.0)
                                .wrap(),
                        )
                        .child(
                            button("Remove", p)
                                .key(format!("team-lesson-remove-{}", lesson.lesson_id))
                                .on_press(move |s: &mut State| {
                                    if let Some(st) = s.data.as_ref() {
                                        let _ = locust_core::tune_lesson_store::remove(st, &lid);
                                    }
                                    reload(s);
                                }),
                        ),
                );
            }
            form = form.child(lcol);
        }
        let skills = locust_core::teammate_skills::list(store, Some(tm_id));
        if !skills.is_empty() {
            let rt = panel.fields.get(3).map(|f| f.text.trim().to_owned()).or_else(|| {
                panel.entry["route"]["runtime"].as_str().map(str::to_owned)
            });
            let skill_note = if locust_core::skill_runtimes::takes_skills(rt.as_deref()) {
                "Kept from tunes. A run loads one only when the work matches when to use it.".to_owned()
            } else {
                format!(
                    "Only {} run is handed these",
                    locust_core::skill_runtimes::SKILL_RUNTIME_ANY
                )
            };
            let mut scol = col()
                .key("team-tune-skills")
                .w_full()
                .gap(4.0)
                .pt(4.0)
                .child(text("Skills").font(type_::LABEL).color(p.text))
                .child(text(skill_note).font(type_::LABEL).color(p.muted).wrap());
            for sk in skills {
                let sid = sk.skill_id.clone();
                scol = scol.child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(
                            text(format!("{} — {}", sk.name, sk.description))
                                .font(type_::LABEL)
                                .color(p.secondary)
                                .grow(1.0)
                                .wrap(),
                        )
                        .child(
                            button("Remove", p)
                                .key(format!("team-skill-remove-{}", sk.skill_id))
                                .on_press(move |s: &mut State| {
                                    if let Some(st) = s.data.as_ref() {
                                        let _ = locust_core::teammate_skills::remove(st, &sid);
                                    }
                                    reload(s);
                                }),
                        ),
                );
            }
            form = form.child(scol);
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
            let (message, setup_id, edit_id, remove_id) = (id.clone(), id.clone(), id.clone(), id.clone());
            let small = |icon: &'static rune::Icon, label: String, help: &str| {
                crate::look::controls::head_icon(icon, &label, help).control_size(rune_kit::ControlSize::Small)
            };
            head = head
                .child(small(&crate::look::icons::MESSAGE, format!("Message {name}"), "Message teammate").on_press_tx(move |s: &mut State, tx| {
                    s.people_panel.open = false;
                    select_teammate(s, &message, tx);
                }).build(l.k).key(format!("team-message-{id}")))
                .child(button("Setup", p).key(format!("team-setup-{id}")).on_press(move |s: &mut State| {
                    crate::setup_card::open_for_teammate(s, &setup_id);
                }))
                .child(small(&crate::look::icons::PENCIL, format!("Edit {name}"), "Edit teammate").on_press(move |s: &mut State| edit(s, Some(&edit_id))).build(l.k).key(format!("team-edit-{id}")))
                .child(small(&crate::look::icons::CLOSE, format!("Remove {name}"), "Remove teammate").on_press(move |s: &mut State| s.people_panel.removing = Some(remove_id.clone())).build(l.k).key(format!("team-remove-{id}")));
        }

        let oc_models = s
            .models
            .get(&id)
            .or_else(|| s.models.get(CARRY_TARGET))
            .and_then(|agents| agents.iter().find(|a| a.runtime == "opencode"))
            .map(|a| a.models.as_slice())
            .unwrap_or(&[]);
        let dropped = locust_core::connectors_off::dropped_free_model(
            &runtime,
            (!model.is_empty()).then_some(model.as_str()),
            oc_models,
        );
        let mut route = row().w_full().gap(6.0).center_y();
        if !runtime.is_empty() {
            if let Some(mark) = crate::look::controls::agent_mark(&l, &runtime, 12.0, l.c("text-secondary")) {
                route = route.child(mark);
            }
            route = route.child(
                text(format!("{} / {}", locust_core::carry_on::display_name(&runtime), model))
                    .font(mono(11.5))
                    .color(if dropped { p.amber } else { l.c("text-secondary") })
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

fn set_memory_mode(s: &mut State, mode: &str) {
    let result = s.data.clone().ok_or("No native data folder".to_owned()).and_then(|store| {
        let mut settings = crate::workspace_settings::load(&store)?;
        settings.memory_mode = mode.to_owned();
        crate::workspace_settings::save(&store, &settings)
    });
    done(s, result);
}

/// MemoryScreen.tsx: About you, How memory is kept, This folder, Everywhere and Recently forgotten, each a card in
/// Electron's words, with every action the memory store has (keep, edit, switch off or on, remove, suggest a rewrite,
/// remember, restore).
fn memory_screen(s: &State, p: &Palette) -> El {
    let panel = &s.people_panel;
    let l = crate::look::controls::Look::new(p.dark);
    let card = || col().w_full().max_w(680.0).gap(10.0).padding(16.0).radius(10.0).border(1.0, l.c("border-hairline")).fill(l.c("bg-raised"));
    let heading = |words: String| text(words).font(rune::TextStyle::new(14.5, 500.0)).color(l.c("text-name"));
    let lede = |words: &str| text(words.to_owned()).font(l.ty.body).color(l.c("text-secondary")).w_full().wrap();
    let note = |words: &str| text(words.to_owned()).font(l.ty.body).color(l.c("text-muted")).w_full().wrap();
    let memories: Vec<&Value> = panel.memories["memories"].as_array().into_iter().flatten().collect();
    let kept = memories.iter().filter(|m| m["status"] != "proposed").count();
    let mode = panel.memory_mode.as_str();
    let mode_word = match mode {
        "ask" => crate::workspace_settings::MEMORY_ASK,
        "off" => crate::workspace_settings::MEMORY_OFF,
        _ => crate::workspace_settings::MEMORY_KEEP,
    }
    .to_lowercase();
    let mut out = col().w_full().gap(14.0).child(
        row()
            .w_full()
            .gap(10.0)
            .center_y()
            .pt(6.0)
            .child(text("Memory").font(rune::TextStyle::new(20.0, 600.0)).color(p.text))
            .child(text(format!("{} · {mode_word}", if kept == 0 { "nothing remembered yet".to_owned() } else { format!("{kept} remembered") })).font(type_::LABEL).color(p.muted)),
    );

    // About you, with the suggestions waiting for it.
    let mut about = card()
        .key("memory-about")
        .child(heading("About you".into()))
        .child(lede("Every teammate reads this before each run, on every runtime — how you like to work, what to always or never do. Only you change it."))
        .child(
            text_input(&panel.note, type_::BODY)
                .key("about-you")
                .label("About you")
                .w_full()
                .min_h(72.0)
                .padding(10.0)
                .radius(8.0)
                .fill(p.field)
                .border(1.0, l.c("border-control"))
                .color(p.text)
                .placeholder("For example: I read diffs, not long explanations. Keep answers short. Ask before deleting anything.", p.muted)
                .on_edit(|s: &mut State| &mut s.people_panel.note),
        )
        .child(row().w_full().justify_end().child(button("Save", p).on_press(|s: &mut State| {
            let result = s.data.as_ref().ok_or("No native data folder".into()).and_then(|store| people::settings(store, &s.people_panel.note.text, None));
            done(s, result);
        })));
    for suggestion in panel.team["settings"]["aboutYouSuggestions"].as_array().into_iter().flatten() {
        let id = suggestion["id"].clone();
        let mut actions = row().gap(6.0);
        for (label, add) in [("Add to About you", true), ("Dismiss", false)] {
            let id = id.clone();
            actions = actions.child(button(label, p).on_press(move |s: &mut State| {
                let all = s.people_panel.team["settings"]["aboutYouSuggestions"].as_array().cloned().unwrap_or_default();
                let line = all.iter().find(|r| r["id"] == id).and_then(|r| r["text"].as_str()).unwrap_or("").to_owned();
                let rest: Vec<_> = all.into_iter().filter(|r| r["id"] != id).collect();
                let text_now = if add { format!("{}\n{}", s.people_panel.note.text, line) } else { s.people_panel.note.text.clone() };
                let result = s.data.as_ref().ok_or("No native data folder".into()).and_then(|store| people::settings(store, &text_now, Some(rest)));
                done(s, result);
            }));
        }
        about = about.child(
            col()
                .w_full()
                .gap(6.0)
                .padding(10.0)
                .radius(8.0)
                .fill(l.c("hover-surface"))
                .child(text(suggestion["text"].as_str().unwrap_or("").to_owned()).font(l.ty.control).color(l.c("text-primary")).w_full().wrap())
                .child(actions),
        );
    }
    out = out.child(about);

    // How memory is kept.
    let mut modes = row().gap(6.0);
    for (value, label) in [("auto", crate::workspace_settings::MEMORY_KEEP), ("ask", crate::workspace_settings::MEMORY_ASK), ("off", crate::workspace_settings::MEMORY_OFF)] {
        let on = mode == value || (value == "auto" && !matches!(mode, "ask" | "off"));
        modes = modes.child(crate::look::controls::chip(&l, label, on).key(format!("memory-mode-{value}")).on_press(move |s: &mut State| set_memory_mode(s, value)));
    }
    out = out.child(
        card()
            .key("memory-mode")
            .child(heading("How memory is kept".into()))
            .child(lede("Every teammate in a folder reads what is remembered for that folder and everything marked everywhere. A teammate writes a memory by ending a reply with one; you can also write one below. Nothing here is uploaded: memory lives in Locust's own data on this machine."))
            .child(modes)
            .child(note(match mode {
                "ask" => "A memory a teammate writes, changes or forgets waits below until you answer it.",
                "off" => "Teammates are not told what is remembered and cannot write memory. What is kept stays here.",
                _ => "A memory a teammate writes is kept at once and said in the conversation; undo it here.",
            })),
    );

    // The memories, in this folder and everywhere.
    let row_of = |memory: &Value| {
        let id = memory["memoryId"].as_str().unwrap_or("").to_owned();
        let mut item = col()
            .w_full()
            .gap(4.0)
            .py(8.0)
            .border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"))
            .child(text(memory["text"].as_str().unwrap_or("").to_owned()).font(l.ty.control).color(if memory["enabled"] == false { l.c("text-muted") } else { l.c("text-primary") }).w_full().wrap())
            .child(text(format!("{} · {}", memory["by"]["name"].as_str().unwrap_or(""), memory["status"].as_str().unwrap_or(""))).font(type_::LABEL).color(l.c("text-muted")));
        if let Some(previous) = memory["previousText"].as_str() {
            item = item.child(text(format!("Before: {previous}")).font(type_::LABEL).color(l.c("text-muted")).w_full().wrap());
        }
        let mut actions = row().gap(6.0).pt(2.0);
        for (label, act) in [("Keep", "keep"), ("Edit", "edit"), (if memory["enabled"] == true { "Switch off" } else { "Switch on" }, "toggle"), ("Remove", "remove")] {
            if act == "keep" && memory["status"] != "proposed" {
                continue;
            }
            let id = id.clone();
            actions = actions.child(button(label, p).on_press(move |s: &mut State| {
                if act == "edit" && s.people_panel.editing.as_deref() != Some(&id) {
                    if let Some(m) = s.people_panel.memories["memories"].as_array().and_then(|r| r.iter().find(|m| m["memoryId"] == id)) {
                        s.people_panel.memory = field(m["text"].as_str().unwrap_or(""));
                        s.people_panel.editing = Some(id.clone());
                    }
                } else {
                    action(s, &id, act);
                    s.people_panel.editing = None;
                }
            }));
        }
        item.child(actions)
    };
    let folder_name = s.home.folder_path.as_deref().and_then(|f| f.trim_end_matches('/').rsplit('/').next()).unwrap_or("no folder").to_owned();
    let here: Vec<&&Value> = memories.iter().filter(|m| m["scope"] != "global").collect();
    let everywhere: Vec<&&Value> = memories.iter().filter(|m| m["scope"] == "global").collect();
    let mut here_card = card().key("memory-here").child(heading(format!("This folder · {folder_name}")));
    if here.is_empty() {
        here_card = here_card.child(note("Nothing remembered for this folder yet."));
    }
    for m in here {
        here_card = here_card.child(row_of(m));
    }
    out = out.child(here_card);
    let mut everywhere_card = card().key("memory-everywhere").child(heading("Everywhere".into()));
    if everywhere.is_empty() {
        everywhere_card = everywhere_card.child(note("Nothing remembered for every folder yet."));
    }
    for m in everywhere {
        everywhere_card = everywhere_card.child(row_of(m));
    }
    // Writing one, and a rewrite of the one being edited.
    everywhere_card = everywhere_card
        .child(
            text_input(&panel.memory, type_::BODY)
                .label("Memory text")
                .key("memory-text")
                .w_full()
                .padding(10.0)
                .radius(8.0)
                .fill(p.field)
                .border(1.0, l.c("border-control"))
                .color(p.text)
                .placeholder("Something every teammate should remember", p.muted)
                .on_edit(|s: &mut State| &mut s.people_panel.memory),
        )
        .child(
            row()
                .gap(6.0)
                .child(button("Remember everywhere", p).on_press(|s: &mut State| {
                    let result = s.data.as_ref().ok_or("No native data folder".into()).and_then(|store| {
                        people::add_memory(store, &s.people_panel.memory.text, None, "global", "native", json!({"name":"you"}), "kept")
                    });
                    done(s, result);
                }))
                .child(button("Suggest rewrite", p).on_press(|s: &mut State| {
                    let result = s.data.as_ref().ok_or("No native data folder".into()).and_then(|store| {
                        let id = s.people_panel.editing.clone().ok_or("Choose Edit on a memory first.")?;
                        let m = s.people_panel.memories["memories"]
                            .as_array()
                            .and_then(|rows| rows.iter().find(|m| m["memoryId"] == id))
                            .ok_or("That memory no longer exists.")?;
                        people::propose_tidy(store, m["workspaceId"].as_str().unwrap_or(""), &[id], &s.people_panel.memory.text, false)
                    });
                    done(s, result);
                })),
        );
    out = out.child(everywhere_card);

    // Recently forgotten, newest first, with the way back.
    let forgotten: Vec<&Value> = panel.memories["forgotten"].as_array().into_iter().flatten().rev().collect();
    if !forgotten.is_empty() {
        let mut held_card = card().key("memory-forgotten").child(heading("Recently forgotten".into()));
        for held in forgotten {
            let id = held["memory"]["memoryId"].as_str().unwrap_or("").to_owned();
            held_card = held_card.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .py(4.0)
                    .border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"))
                    .child(text(held["memory"]["text"].as_str().unwrap_or("").to_owned()).font(l.ty.control).color(l.c("text-muted")).grow(1.0).shrink().wrap())
                    .child(button("Restore", p).on_press(move |s: &mut State| action(s, &id, "restore"))),
            );
        }
        out = out.child(held_card);
    }
    out
}

fn current_workspace_id(s: &State) -> Option<String> {
    s.home
        .folder_path
        .as_deref()
        .map(std::path::Path::new)
        .map(people::workspace_id)
}

fn share_card_view(s: &State, p: &Palette) -> El {
    let ws = current_workspace_id(s);
    let team_arr = s.people_panel.team["teammates"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let routines_arr = s.people_panel.share_card_routines["routines"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let card = locust_core::team_card::team_card_of(team_arr);
    let card_mates = card["teammates"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let routines = locust_core::team_card::team_card_routines(
        team_arr,
        routines_arr,
        ws.as_deref(),
        &[],
    );
    let count = card_mates.len();
    let routine_count = routines.len();
    let mut box_el = col()
        .w_full()
        .padding(14.0)
        .gap(8.0)
        .fill(p.field)
        .radius(10.0)
        .child(
            row()
                .w_full()
                .child(text("Share your team").font(type_::TITLE).color(p.text).grow(1.0))
                .child(button("Close", p).key("team-card-close").on_press(|s: &mut State| {
                    s.people_panel.share_card_open = false;
                })),
        )
        .child(
            text(format!(
                "{count} {} · team card PNG (secrets and credentials are never included)",
                if count == 1 { "teammate" } else { "teammates" }
            ))
            .font(type_::LABEL)
            .color(p.muted)
            .wrap(),
        );
    for mate in card_mates {
        let name = mate["name"].as_str().unwrap_or("Teammate");
        let role = mate["role"].as_str().unwrap_or("");
        let mode = mate["route"]["mode"].as_str().unwrap_or("ask");
        box_el = box_el.child(
            text(format!("{name} — {role} ({mode})"))
                .font(type_::LABEL)
                .color(p.text),
        );
    }
    if routine_count > 0 {
        box_el = box_el.child(
            text(format!(
                "Also carries {routine_count} {}",
                if routine_count == 1 { "routine" } else { "routines" }
            ))
            .font(type_::LABEL)
            .color(p.secondary),
        );
    }
    box_el.child(
        row()
            .gap(8.0)
            .child(button("Save image", p).key("team-card-save").on_press_tx(|_s: &mut State, tx: &mut Tx| {
                export_team_to_image(tx);
            })),
    )
}

pub(super) fn export_team_to_image(tx: &mut Tx) {
    let options = rune::FileDialogOptions {
        title: "Save team card".into(),
        file_name: Some("locust-team.png".into()),
        ..Default::default()
    }
    .filter("PNG image", &["png"]);
    tx.save_file(options, |s: &mut State, answer, _tx| {
        let Ok(Some(path)) = answer else { return };
        let Some(store) = s.data.clone() else { return };
        let ws = current_workspace_id(s);
        match locust_core::team_card::export_team_card(&store, Some(&store), ws.as_deref(), &[])
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                s.people_panel.share_card_open = false;
                s.people_panel.error.clear();
                s.status = format!("Saved team card to {}.", path.display());
                s.status_tone = StatusTone::Green;
            }
            Err(e) => {
                s.people_panel.error = e.clone();
                s.status = e;
                s.status_tone = StatusTone::Amber;
            }
        }
    });
}

pub(super) fn import_team_from_image(tx: &mut Tx) {
    let options = rune::FileDialogOptions {
        title: "Add team from image".into(),
        ..Default::default()
    }
    .filter("PNG image", &["png"]);
    tx.open_file(options, |s: &mut State, answer, _tx| {
        let Ok(Some(path)) = answer else { return };
        let Some(store) = s.data.clone() else { return };
        let ws = current_workspace_id(s);
        match locust_core::team_card::import_team_card_file(&store, &store, ws.as_deref(), &path) {
            Ok(res) => {
                reload(s);
                s.team = Latest::default();
                s.home.first_launch_done = true;
                s.people_panel.error.clear();
                if s.routines.open {
                    if let Ok(list) = crate::routines::list(&store) {
                        s.routines.file = list;
                    }
                }
                let routines_note = if res.routines.is_empty() {
                    String::new()
                } else {
                    format!(" ({} routines)", res.routines.len())
                };
                s.status = if res.added.is_empty() {
                    format!("Team card imported{routines_note}.")
                } else {
                    format!("Added {}{routines_note}.", res.added.join(", "))
                };
                s.status_tone = StatusTone::Green;
            }
            Err(e) => {
                s.people_panel.error = e.clone();
                s.status = e;
                s.status_tone = StatusTone::Amber;
            }
        }
    });
}

/// An interview's first turn runs in Ask, as a review starts, so it changes nothing (Electron: `startMission(
/// interviewPrompt(...), { as: asReviewer(...) })`, pinned by a-new-teammate-interviews-you.test.tsx).
pub(super) fn interview_origin() -> native::Origin {
    native::Origin { mode: Some(locust_core::approval::Mode::Ask), ..Default::default() }
}

#[cfg(test)]
mod interview_tests {
    #[test]
    fn an_interview_starts_in_ask_so_it_changes_nothing() {
        assert_eq!(super::interview_origin().mode, Some(locust_core::approval::Mode::Ask));
        assert_eq!(super::interview_origin().started_by, None);
    }
}
