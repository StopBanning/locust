use super::*;
use serde_json::{Value, json};
#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub model_open: bool,
    pub route_changed: bool,
    pub month_job: Latest<Result<Vec<locust_core::usage::UsageTurn>, String>>,
    pub month_turns: Vec<locust_core::usage::UsageTurn>,
    pub month_error: Option<String>,
    pub month_read: Option<std::time::Instant>,
    pub limit: TextEdit,
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
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some(result) = s.people_panel.month_job.take() {
        match result {
            Ok(turns) => { s.people_panel.month_turns = turns; s.people_panel.month_error = None; }
            Err(e) => { s.people_panel.month_turns.clear(); s.people_panel.month_error = Some(e); }
        }
        changed = true;
    }
    if s.people_panel.open && s.people_panel.month_read.is_none_or(|at| at.elapsed() >= Duration::from_secs(30)) {
        request_month(s);
    }
    changed
}
fn request_month(s: &mut State) {
    if s.people_panel.month_job.is_pending() { return; }
    if let Some(store) = s.data.clone() {
        s.people_panel.month_job.request(store, |store| locust_core::usage_ledger::usage_turns(&store));
        s.people_panel.month_read = Some(std::time::Instant::now());
    }
}
fn month_spend(s: &State, id: &str) -> Option<locust_core::spend::Spend> {
    if s.people_panel.month_error.is_some() { return None; }
    let month = locust_core::spend::month_of(&chrono::Local::now().to_rfc3339())?;
    locust_core::teammate_month::spend_for_month(&s.people_panel.month_turns, &month, |conversation| s.people_panel.team["missionOwners"][conversation] == id)
}
fn reload(s: &mut State) {
    request_month(s);
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
pub(super) fn edit(s: &mut State, id: Option<&str>) {
    let mut entry = id.and_then(|id|s.people_panel.team["teammates"].as_array()?.iter().find(|t|t["teammateId"] == id)).cloned().unwrap_or_else(||json!({"teammateId":"","role":"Custom"}));
    if id.is_none() {
        let taken: Vec<&str> = s.people_panel.team["teammates"].as_array().into_iter().flatten().filter_map(|t| t["hue"].as_str()).collect();
        entry["hue"] = json!(people::fresh_hue(&taken));
    }
    // The look is generated, then owned: whatever the preview shows at Create or Save is what is saved.
    entry["avatar"] = crate::look_picker::opening_avatar(&entry, id.is_none());
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
    s.people_panel.limit = field(&entry["monthlyLimitUsd"].as_f64().map(|n| format!("{n:.2}")).unwrap_or_default());
    s.people_panel.model_open = false;
    s.people_panel.route_changed = false;
    s.people_panel.entry = entry;
    s.people_panel.form_open = true;
    if s.engine.is_some() && !s.models.contains_key(LEDGER_TARGET) {
        crate::open_models(s, LEDGER_TARGET.into());
        s.model_menu = None;
    }
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
    entry["route"].as_object_mut()?.remove("effort");
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
    if !s.people_panel.route_changed && s.people_panel.entry.get("route").is_none() {
        entry.as_object_mut()?.remove("route");
    }
    let limit = locust_core::teammate_month::parsed_limit(&s.people_panel.limit.text).ok()?;
    let editing = !entry["teammateId"].as_str().unwrap_or("").is_empty();
    let said = locust_core::teammate_month::limit_said(limit, editing, entry["monthlyLimitUsd"].as_f64());
    if let Some(value) = said.get("monthlyLimitUsd") {
        if value.is_null() { entry.as_object_mut()?.remove("monthlyLimitUsd"); }
        else { entry["monthlyLimitUsd"] = value.clone(); }
    }
    Some(entry)
}

/// The dialog shares the composer's catalog and picker; a pick edits only this form.
fn shown_route(s: &State) -> Option<locust_core::groups::GroupRoute> {
    let f = &s.people_panel.fields;
    if s.people_panel.route_changed || s.people_panel.entry["route"].is_object() {
        Some(locust_core::groups::GroupRoute {
            runtime: f.get(3)?.text.clone(), model: f.get(4)?.text.clone(),
            effort: f.get(5).map(|f| f.text.clone()).filter(|v| !v.is_empty()),
            mode: f.get(6)?.text.clone(), account: s.people_panel.account.clone(),
        })
    } else { crate::group_settings_dialog::current_composer_route(s) }
}
pub(super) fn pick_model(s: &mut State, pick: TurnPick) {
    let was = shown_route(s).unwrap_or(locust_core::groups::GroupRoute {
        runtime: "codex".into(), model: "account-default".into(), mode: "ask".into(), effort: None, account: None,
    });
    let runtime = pick.runtime.unwrap_or_else(|| was.runtime.clone());
    let same = runtime == was.runtime && pick.model == was.model;
    s.people_panel.fields[3] = field(&runtime);
    s.people_panel.fields[4] = field(&pick.model);
    s.people_panel.fields[5] = field(if same { was.effort.as_deref().unwrap_or("") } else { "" });
    s.people_panel.fields[6] = field(if locust_core::mode_facts::runs_on(&was.mode, &runtime) { &was.mode } else { "ask" });
    s.people_panel.account = pick.account.unwrap_or(was.account);
    s.people_panel.route_changed = true;
    s.people_panel.model_open = false;
}
fn set_form_account(s: &mut State, account: Option<String>) {
    let Some(route) = shown_route(s) else { return };
    pick_model(s, TurnPick { model: route.model, runtime: Some(route.runtime), effort: route.effort, account: Some(account) });
}
/// GPT IDs remain readable before discovery has supplied the model's own name.
fn model_fallback_name(id: &str) -> String {
    let Some(rest) = id.strip_prefix("gpt-") else { return id.to_owned() };
    let mut parts = rest.split('-');
    let mut name = format!("GPT-{}", parts.next().unwrap_or_default());
    for part in parts {
        name.push(' ');
        let mut chars = part.chars();
        if let Some(c) = chars.next() { name.extend(c.to_uppercase()); }
        name.extend(chars);
    }
    name
}
/// This choice belongs to the form until Save, never to the open workspace.
fn set_form_folder(s: &mut State, folder: Option<&std::path::Path>) {
    if let Some(edit) = s.people_panel.fields.get_mut(7) {
        *edit = field(&folder.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
    }
}

fn form_mode_summary(mode: &str) -> String {
    let short = match mode {
        "auto" => "may edit anything on this machine",
        "accept-edits" => "may change files in this workspace",
        "approve-each" => "asks before every command or change",
        "plan" => "answers with the steps it would take, changes nothing",
        _ => "reads and explains, every write refused",
    };
    format!("{} · {short}", locust_core::mode_facts::fact(mode).name)
}

fn teammate_folder_row(s: &State, p: &Palette) -> Option<El> {
    let panel = &s.people_panel;
    let id = panel.entry["teammateId"].as_str().filter(|id| !id.is_empty())?;
    s.engine.as_ref()?;
    let folder = panel.fields.get(7)?.text.trim();
    let mut choices = row().w_full().gap(8.0).center_y()
        .child(text(if folder.is_empty() { "The project folder" } else { folder }.to_owned()).grow(1.0).shrink().wrap().color(p.text))
        .child(button(if folder.is_empty() { "Choose folder" } else { "Change" }, p)
            .key("team-folder-choose").on_press_tx({
                let id = id.to_owned();
                move |s: &mut State, tx: &mut Tx| {
                    let id = id.clone();
                    let options = rune::FileDialogOptions { title: "Choose this teammate's folder".into(),
                        directory: s.people_panel.fields.get(7).map(|f| f.text.trim()).filter(|f| !f.is_empty()).map(std::path::PathBuf::from),
                        ..Default::default() };
                    tx.open_folder(options, move |s: &mut State, answer, _tx| {
                        if !s.people_panel.form_open || s.people_panel.entry["teammateId"].as_str() != Some(&id) { return; }
                        if let Ok(Some(path)) = answer { set_form_folder(s, Some(&path)); }
                    });
                }
            }));
    if !folder.is_empty() {
        choices = choices.child(button("Use the project folder", p).key("team-folder-reset")
            .on_press(|s: &mut State| set_form_folder(s, None)));
    }
    Some(col().key("team-folder-row").w_full().gap(6.0)
        .child(text("Works in").font(type_::LABEL).color(p.secondary))
        .child(choices)
        .child(text("Its conversations run here instead of the project folder, which is also how it reaches a connector (an MCP server) set up for that folder. History and memory stay with the project either way.")
            .font(type_::LABEL).color(p.secondary).wrap()))
}

fn model_row(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let route = shown_route(s);
    let catalog = s.models.get(LEDGER_TARGET);
    let mut row_ = col().w_full().gap(6.0).child(text("Model").font(type_::LABEL).color(p.secondary));
    let label = route.as_ref().map(|r| {
        let name = catalog.into_iter().flatten().filter(|a| a.runtime == r.runtime)
            .flat_map(|a| &a.models).find(|m| m.id == r.model).map(|m|m.name.clone())
            .unwrap_or_else(|| model_fallback_name(&r.model));
        format!("{} / {}", engine::runtime_name(&r.runtime), name)
    }).unwrap_or_else(|| "The chat box’s, when they start".into());
    row_ = row_.child(text(label).key("team-model-name").color(p.text));
    if !s.people_panel.route_changed && !s.people_panel.entry["route"].is_object() && route.is_some() {
        row_ = row_.child(text("the chat box's, until you pick one").color(p.muted));
    }
    // Native always has a model provider when its engine is connected.
    if s.engine.is_some() {
        row_ = row_.child(button("Change", p).key("team-model-change").on_press(|s: &mut State| {
            crate::open_models(s, LEDGER_TARGET.into());
            s.model_menu = None;
            s.people_panel.model_open = !s.people_panel.model_open;
        }));
    }
    if route.is_some() || s.people_panel.model_open {
        // A new team can have no composer route yet. The picker still offers every
        // discovered model; this display fallback is never saved unless picked.
        let route = route.unwrap_or(locust_core::groups::GroupRoute {
            runtime: "claude".into(), model: "account-default".into(), mode: "ask".into(), effort: None, account: None,
        });
        let model = catalog.into_iter().flatten().filter(|a| a.runtime == route.runtime)
            .flat_map(|a| &a.models).find(|m| m.id == route.model);
        if s.people_panel.model_open {
            let mate = Teammate { id: LEDGER_TARGET.into(), name: "".into(), role: "".into(), runtime: Some(route.runtime.clone()),
                model: Some(route.model.clone()), account: route.account.clone(), busy: false, runs_in: route.mode.clone(), folder: None };
            row_ = row_.child(crate::thread_view::model_picker(s, &l, LEDGER_TARGET, &mate, None, &route.model, route.effort.as_deref(), true));
        } else if let Some(model) = model.filter(|m| !m.efforts.is_empty()) {
            let levels = &model.efforts;
            let shown = route.effort.as_deref().unwrap_or_else(|| if levels.iter().any(|e| e == "medium") { "medium" } else { &levels[levels.len()/2] });
            let mut stops = row().key("team-effort").gap(6.0).child(text("Effort").color(p.secondary));
            for (i, level) in levels.iter().enumerate() {
                let mut title = level.clone();
                if let Some(first) = title.get_mut(0..1) { first.make_ascii_uppercase(); }
                let choice = level.clone();
                stops = stops.child(crate::look::controls::chip(&l, &title, level == shown).key(format!("team-effort-{i}"))
                    .role("radio").label(format!("{title}: {}", if level == shown { "selected" } else { "not selected" }))
                    .on_press(move |s: &mut State| {
                        let Some(r) = shown_route(s) else { return };
                        pick_model(s, TurnPick { model: r.model, runtime: Some(r.runtime), effort: None, account: Some(r.account) });
                        s.people_panel.fields[5] = field(&choice);
                    }));
            }
            row_ = row_.child(stops);
        }
    }
    row_
}

fn form_name_clash(s: &State) -> Option<String> {
    let panel = &s.people_panel;
    let taken: Vec<&str> = panel.team["teammates"].as_array().into_iter().flatten()
        .filter(|t| t["teammateId"] != panel.entry["teammateId"])
        .filter_map(|t| t["name"].as_str()).collect();
    people::taken_name(panel.fields.first()?.text.as_str(), &taken).map(str::to_owned)
}

fn save(s: &mut State) {
    if form_name_clash(s).is_some() { return; }
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
    if form_name_clash(s).is_some() { return; }
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
    ago_label_at(iso, chrono::Utc::now())
}

fn ago_label_at(iso: &str, now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    let at = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let seconds = (now - at.with_timezone(&chrono::Utc)).num_seconds();
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
pub(super) fn edit_form(s: &State, p: &Palette) -> El {
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
    let taken_names: Vec<&str> = panel.team["teammates"].as_array().into_iter().flatten()
        .filter(|t| t["teammateId"] != panel.entry["teammateId"])
        .filter_map(|t| t["name"].as_str()).collect();
    let clash = people::taken_name(panel.fields.first().map(|f| f.text.as_str()).unwrap_or(""), &taken_names);
    let field = |i: usize, label: &'static str| {
        let edit = &panel.fields[i];
        text_input(edit, type_::BODY)
            .placeholder(if i == 0 { people::suggested_name(&taken_names) } else { "" }, p.muted)
            .key(format!("person-field-{i}"))
            .label(label)
            .w_full()
            .padding(8.0)
            .fill(p.field)
            .color(p.text)
            .on_edit(move |s: &mut State| &mut s.people_panel.fields[i])
    };
    // NewTeammateDialog.tsx `.lc-dialog__identity` (511-559): the preview that moves on the left, and beside it a column
    // of Name, its box, the name-taken line and the colour swatches under them (`.lc-dialog__fields`, gap space-4).
    let mut identity_fields = col().key("team-identity-fields").grow(1.0).shrink().gap(l.t.px("space-4"));
    if !panel.fields.is_empty() {
        identity_fields = identity_fields.child(text("Name").font(type_::LABEL).color(p.secondary)).child(field(0, "Name"));
        if let Some(taken) = clash {
            identity_fields = identity_fields
                .child(text(format!("Another teammate is already called {taken}.")).key("team-name-clash").color(p.secondary));
        }
    }
    identity_fields = identity_fields.child(crate::look_picker::swatches(s, &l));
    form = form.child(
        row()
            .key("team-identity")
            .w_full()
            .gap(l.t.px("space-6"))
            .center_y()
            .child(crate::look_picker::preview(s, &l))
            .child(identity_fields),
    );
    // The look comes before the role, as in Electron's dialog.
    form = form.child(crate::look_picker::section(s, &l));
    for (i, label) in ["Name", "Role", "Instructions", "Runtime", "Model", "Effort", "Mode", "Folder"].into_iter().enumerate().skip(1) {
        if !matches!(i, 3..=5 | 7) && panel.fields.get(i).is_some() {
            form = form.child(text(label).font(type_::LABEL).color(p.secondary)).child(field(i, label));
        }
    }
    form = form.child(model_row(s, p));
    if let Some(folder) = teammate_folder_row(s, p) { form = form.child(folder); }
    form = form.child(text(form_mode_summary(shown_route(s).as_ref().map(|r| r.mode.as_str()).unwrap_or("ask"))).key("team-permissions").font(type_::LABEL).color(p.secondary).wrap());
    let limit_error = locust_core::teammate_month::parsed_limit(&panel.limit.text).err();
    let effective_route = shown_route(s);
    let runtime_now = effective_route.as_ref().map(|r|r.runtime.as_str()).unwrap_or("");
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
                .on_press(|s: &mut State| set_form_account(s, None)),
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
                        .on_press(move |s: &mut State| set_form_account(s, Some(acc_id.clone()))),
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
    let spent = month_spend(s, panel.entry["teammateId"].as_str().unwrap_or(""));
    form = form.child(text("Monthly limit").font(type_::LABEL).color(p.secondary))
        .child(text_input(&panel.limit, type_::BODY).key("team-monthly-limit").label("Monthly limit").w_full().padding(8.0).fill(p.field).color(p.text)
            .on_edit(|s: &mut State| &mut s.people_panel.limit))
        .child(text(locust_core::teammate_month::limit_hint(panel.fields.first().map(|f|f.text.as_str()).unwrap_or("Teammate"), spent.as_ref(), !creating)).font(type_::LABEL).color(p.muted).w_full().wrap());
    if let Some(error) = limit_error { form = form.child(text(error).color(p.red)); }
    if let Some(error) = &panel.month_error { form = form.child(text(format!("This month's spending could not be read: {error}")).color(p.red).w_full().wrap()); }
    // Its keys (KeyCard.tsx TeammateKeysField), for a teammate that exists: a new one has no id to keep them under yet.
    if let Some(id) = panel.editing.as_deref().filter(|_| !creating) {
        form = form.child(crate::key_card::field(s, p, id, if name_now.is_empty() { "this teammate" } else { &name_now }));
    }
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
            .child(button(&interview_label, p).key("team-interview").disabled(clash.is_some() || limit_error.is_some()).on_press_tx(save_and_interview))
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
            .child(button(if creating { "Create teammate" } else { "Save changes" }, p).key("team-save").disabled(clash.is_some() || limit_error.is_some()).on_press(save))
            .child(button("Cancel", p).on_press(|s: &mut State| s.people_panel.form_open = false)),
    )
}

/// Screens.tsx's roster (`.lc-rostergrid`, `.lc-rostercard`): a card per teammate with its face, name and role, the
/// message, edit and remove buttons, its route and mode, how many conversations and when it last ran, and its three most
/// recent conversations; then the New teammate card.
fn team_grid(s: &State, p: &Palette) -> El {
    let panel = &s.people_panel;
    let l = crate::look::controls::Look::new(p.dark);
    let mono = |size: f32| rune::TextStyle::new(size, 400.0).mono();
    let team = panel.team["teammates"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let missions: Vec<_> = s.ledger_summaries.values().cloned().collect();
    let owners = panel.team["missionOwners"].as_object().into_iter().flatten()
        .filter_map(|(mission,owner)| Some((mission.clone(),owner.as_str()?.to_owned()))).collect();
    let groups = crate::team_board::team_board(team, |t| {
        let id = t["teammateId"].as_str().unwrap_or("");
        crate::home_panel::teammate_section(s, id, s.teammates.iter().any(|m| m.id == id && m.busy), t["route"]["runtime"].as_str())
    });
    let grouped = groups.is_some();
    let mut body = col().key("team-grid").w_full().gap(12.0);
    for (section, members) in groups.unwrap_or_else(|| vec![(crate::team_board::Section::Ready, team.iter().collect())]) {
    if grouped { body = body.child(text(section.title()).font(type_::LABEL).color(p.text)); }
    let mut grid = row().w_full().gap(12.0).flex_wrap();
    for t in members {
        let id = t["teammateId"].as_str().unwrap_or("").to_owned();
        let name = t["name"].as_str().unwrap_or("").to_owned();
        let role = t["role"].as_str().unwrap_or("").to_owned();
        let runtime = t["route"]["runtime"].as_str().unwrap_or("").to_owned();
        let model = t["route"]["model"].as_str().unwrap_or("").to_owned();
        let mode = t["route"]["mode"].as_str().unwrap_or("ask").to_owned();
        let work = locust_core::teammate_work::teammate_work(&id, &missions, &owners, |m| m.title.clone());
        let last = work.last_run_at.and_then(ago_label);
        let status = crate::home_panel::teammate_status(s, &id, s.teammates.iter().any(|m| m.id == id && m.busy), t["route"]["runtime"].as_str(), &role);

        let mut head = row()
            .w_full()
            .gap(12.0)
            .child(crate::look::faces::face(s.image_cache.as_deref(), &id, &name, s.looks.get(&id), 40.0, s.settings_panel.settings.terminal_faces, &l))
            .child(
                col()
                    .grow(1.0)
                    .shrink()
                    .gap(2.0)
                    .child(text(name.clone()).font(rune::TextStyle::new(14.5, 500.0)).color(l.c("text-name")).w_full().truncate())
                    .child(row().gap(5.0).center_y().child(crate::home_panel::teammate_presence(&status, p)).child(text(status.label.clone()).font(rune::TextStyle::new(13.0, 400.0)).color(crate::home_panel::teammate_status_color(&status, p)).w_full().truncate())),
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
            .child(stat("Runs", work.mission_count.to_string(), work.mission_count == 0))
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
        if let Some(cost) = locust_core::benchmark::cost_line(work.cost.as_ref()) {
            card = card.child(stat("Usage", cost, false));
        }
        let spent = month_spend(s, &id);
        if let Some(error) = &panel.month_error {
            card = card.child(text(format!("This month's spending could not be read: {error}")).font(type_::LABEL).color(p.red).w_full().wrap());
        } else if panel.month_job.is_pending() && panel.month_turns.is_empty() {
            card = card.child(text("Reading this month's spending…").font(type_::LABEL).color(p.muted));
        } else if let Some(line) = locust_core::teammate_month::month_spend_line(spent.as_ref(), t["monthlyLimitUsd"].as_f64()) {
            card = card.child(col().key(format!("team-month-{id}")).gap(2.0)
                .child(text(if line.reached { "Limit reached" } else { "This month" }).font(type_::LABEL).color(if line.reached { p.amber } else { p.muted }))
                .child(text(line.text).font(mono(11.5)).color(p.text)));
            if let Some(unpriced) = line.unpriced {
                card = card.child(col().gap(2.0).child(text(unpriced).font(type_::LABEL).color(p.muted))
                    .child(text(line.note.unwrap_or("")).font(type_::LABEL).color(p.muted).w_full().wrap()));
            }
        }
        if !work.recent.is_empty() {
            let mut recent = col()
                .w_full()
                .gap(4.0)
                .pt(10.0)
                .border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"))
                .child(text("RECENT").font(rune::TextStyle::new(10.5, 400.0).mono().tracking(1.0)).color(l.c("text-muted")));
            for r in &work.recent {
                let mut line = row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(el().size(5.0, 5.0).radius(2.5).fill(l.c("text-faint")))
                    .child(text(r.title.clone()).font(rune::TextStyle::new(13.0, 400.0)).color(l.c("text-secondary")).grow(1.0).shrink().truncate());
                if let Some(age) = ago_label(&r.mission.updated) {
                    line = line.child(text(age).font(mono(11.0)).color(l.c("text-muted")));
                }
                recent = recent.child(line);
            }
            card = card.child(recent);
        }
        grid = grid.child(card);
    }
    body = body.child(grid);
    }
    body.child(
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
    let id_texts: Vec<(&str, &str)> = memories
        .iter()
        .filter(|m| m["status"] != "proposed")
        .filter_map(|m| Some((m["memoryId"].as_str()?, m["text"].as_str()?)))
        .collect();
    let proposed: Vec<&Value> = memories.iter().filter(|m| m["status"] == "proposed").copied().collect();
    let here: Vec<&Value> = memories
        .iter()
        .filter(|m| m["status"] != "proposed" && m["scope"] != "global")
        .copied()
        .collect();
    let everywhere: Vec<&Value> = memories
        .iter()
        .filter(|m| m["status"] != "proposed" && m["scope"] == "global")
        .copied()
        .collect();
    let kept = here.len() + everywhere.len();
    let about_you_waiting = panel.team["settings"]["aboutYouSuggestions"].as_array().map_or(0, Vec::len);
    let waiting_count = proposed.len() + about_you_waiting;
    let mode = panel.memory_mode.as_str();
    let mut out = col().w_full().gap(14.0).child(
        row()
            .w_full()
            .gap(10.0)
            .center_y()
            .pt(6.0)
            .child(text("Memory").font(rune::TextStyle::new(20.0, 600.0)).color(p.text))
            .child(
                text(format!(
                    "{}{}{}",
                    if kept == 0 {
                        "nothing remembered yet".to_owned()
                    } else {
                        format!("{kept} remembered")
                    },
                    if waiting_count > 0 {
                        format!(" · {waiting_count} waiting for you")
                    } else {
                        String::new()
                    },
                    if mode == "off" { " · off" } else { "" }
                ))
                .font(type_::LABEL)
                .color(p.muted),
            ),
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

    // The memories, in waiting, this folder and everywhere.
    let row_of = |memory: &Value| {
        let id = memory["memoryId"].as_str().unwrap_or("").to_owned();
        let body_text = memory["text"].as_str().unwrap_or("").to_owned();
        let is_proposed = memory["status"] == "proposed";
        let enabled = memory["enabled"] != false;
        let text_color = if enabled { l.c("text-primary") } else { l.c("text-muted") };

        let lead = if is_proposed {
            el()
                .key(format!("memory-suggested-{id}"))
                .pt(2.0)
                .child(crate::look::controls::glyph(&l, &crate::look::icons::SPARK, 14.0, Some(l.c("text-muted"))))
                .a11y_hidden()
        } else {
            let id_toggle = id.clone();
            crate::look::controls::switch(
                &l,
                enabled,
                if enabled { "Switch this memory off" } else { "Switch this memory on" },
                false,
            )
            .key(format!("memory-switch-{id}"))
            .on_press(move |s: &mut State| action(s, &id_toggle, "toggle"))
        };

        let mut body = col().grow(1.0).shrink().gap(4.0);
        if let Some(merges) = memory["merges"].as_array() {
            body = body.child(
                row()
                    .w_full()
                    .wrap()
                    .child(text("Wants to merge these into one: ").font(l.ty.control).color(l.c("text-secondary")))
                    .child(text(body_text.clone()).font(l.ty.control).color(text_color)),
            );
            for mid in merges {
                let old = mid
                    .as_str()
                    .and_then(|target| id_texts.iter().find(|(k, _)| *k == target).map(|(_, t)| *t))
                    .unwrap_or("no longer kept");
                body = body.child(text(format!("Now: {old}")).font(type_::LABEL).color(l.c("text-muted")).w_full().wrap());
            }
        } else if memory["forgets"].as_str().is_some() {
            body = body.child(
                row()
                    .w_full()
                    .wrap()
                    .child(text("Wants to forget this: ").font(l.ty.control).color(l.c("text-secondary")))
                    .child(text(body_text.clone()).font(l.ty.control).color(text_color)),
            );
            if let Some(reason) = memory["reason"].as_str() {
                let readable = locust_core::memory_tidy::readable_reason(reason, &id_texts);
                body = body.child(text(format!("Why: {readable}")).font(type_::LABEL).color(l.c("text-muted")).w_full().wrap());
            }
        } else if let Some(replaces) = memory["replaces"].as_str() {
            let old = id_texts
                .iter()
                .find(|(k, _)| *k == replaces)
                .map(|(_, t)| *t)
                .unwrap_or("no longer kept");
            body = body
                .child(
                    row()
                        .w_full()
                        .wrap()
                        .child(text("Wants to change this to: ").font(l.ty.control).color(l.c("text-secondary")))
                        .child(text(body_text.clone()).font(l.ty.control).color(text_color)),
                )
                .child(text(format!("Now: {old}")).font(type_::LABEL).color(l.c("text-muted")).w_full().wrap());
        } else {
            body = body.child(text(body_text.clone()).font(l.ty.control).color(text_color).w_full().wrap());
        }

        if let Some(previous) = memory["previousText"].as_str()
            && previous != body_text
        {
            let id_back = id.clone();
            let prev_owned = previous.to_owned();
            body = body.child(
                row()
                    .w_full()
                    .gap(6.0)
                    .center_y()
                    .wrap()
                    .child(text(format!("Was: {previous}")).font(type_::LABEL).color(l.c("text-muted")).wrap())
                    .child(
                        button("Put it back", p)
                            .key(format!("memory-put-back-{id}"))
                            .on_press(move |s: &mut State| {
                                let result = s
                                    .data
                                    .as_ref()
                                    .ok_or("No native data folder".into())
                                    .and_then(|store| people::change_memory(store, &id_back, "edit", Some(&prev_owned)));
                                done(s, result);
                            }),
                    ),
            );
        }

        let writer = memory["updatedBy"]["name"]
            .as_str()
            .or_else(|| memory["by"]["name"].as_str())
            .unwrap_or("");
        let ws_name = memory["workspaceName"].as_str().unwrap_or("");
        let scope_part = if memory["scope"] == "global" {
            if ws_name.is_empty() { "everywhere".to_owned() } else { format!("everywhere, from {ws_name}") }
        } else {
            ws_name.to_owned()
        };
        let when_part = if let Some(updated_at) = memory["updatedAt"].as_str() {
            let short = crate::memory_card::when_short(updated_at);
            if short.is_empty() { String::new() } else { format!("changed {short}") }
        } else {
            crate::memory_card::when_short(memory["createdAt"].as_str().unwrap_or(""))
        };
        let meta: Vec<String> = [writer.to_owned(), scope_part, when_part]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect();
        if !meta.is_empty() {
            body = body.child(text(meta.join(" · ")).font(type_::LABEL).color(l.c("text-muted")));
        }

        let mut actions = row().gap(6.0).pt(2.0);
        if is_proposed {
            let (keep_label, drop_label) = if memory.get("merges").is_some_and(|v| !v.is_null()) {
                ("Merge them", "Keep them apart")
            } else if memory.get("forgets").is_some_and(|v| !v.is_null()) {
                ("Forget it", "Keep it")
            } else if memory.get("replaces").is_some_and(|v| !v.is_null()) {
                ("Keep the change", "Keep the old one")
            } else {
                ("Keep", "Forget")
            };
            let id_keep = id.clone();
            let id_drop = id.clone();
            actions = actions
                .child(
                    button(keep_label, p)
                        .key(format!("memory-keep-{id}"))
                        .on_press(move |s: &mut State| action(s, &id_keep, "keep")),
                )
                .child(
                    button(drop_label, p)
                        .key(format!("memory-drop-{id}"))
                        .on_press(move |s: &mut State| action(s, &id_drop, "remove")),
                );
        } else {
            for (label, act) in [("Edit", "edit"), ("Remove", "remove")] {
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
        }

        row()
            .w_full()
            .gap(10.0)
            .py(8.0)
            .border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"))
            .child(lead)
            .child(body.child(actions))
    };

    if !proposed.is_empty() {
        let mut waiting_card = card().key("memory-waiting").child(heading("Waiting for you".into()));
        if proposed.len() >= 2 {
            let proposed_ids: Vec<String> = proposed
                .iter()
                .filter_map(|m| m["memoryId"].as_str().map(str::to_owned))
                .collect();
            waiting_card = waiting_card.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(note("Anything you keep can be put back.").grow(1.0).shrink())
                    .child(
                        button(&format!("Keep all {}", proposed.len()), p)
                            .key("memory-keep-all")
                            .on_press(move |s: &mut State| {
                                let result = match s.data.as_ref() {
                                    None => Err("Native data folder could not be determined.".into()),
                                    Some(store) => {
                                        match crate::memory_card::keep_every(&proposed_ids, |id| {
                                            people::change_memory(store, id, "keep", None).err()
                                        }) {
                                            Some(err) => Err(err),
                                            None => Ok(()),
                                        }
                                    }
                                };
                                done(s, result);
                            }),
                    ),
            );
        }
        for m in proposed {
            waiting_card = waiting_card.child(row_of(m));
        }
        out = out.child(waiting_card);
    }

    let folder_name = s.home.folder_path.as_deref().and_then(|f| f.trim_end_matches('/').rsplit('/').next()).unwrap_or("no folder").to_owned();
    let mut here_card = card().key("memory-here").child(heading(format!("This folder · {folder_name}")));
    if mode != "off" && here.len() >= 2 {
        let nudge = if here.len() > locust_core::memory_tidy::TIDY_NUDGE_AT {
            format!("{} memories here. ", here.len())
        } else {
            String::new()
        };
        here_card = here_card.child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(note(&format!("{nudge}A teammate reads them and suggests merges and retirements; nothing changes until you keep it.")).grow(1.0).shrink())
                .child(button("Tidy up…", p).key("memory-tidy")),
        );
    }
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
    box_el.child(text("You can add the team from it with Add team from image").font(type_::LABEL).color(p.secondary).wrap()).child(
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
#[path = "memory_screen_tests.rs"]
mod memory_screen_tests;

#[cfg(test)]
mod interview_tests {
    #[test]
    fn an_interview_starts_in_ask_so_it_changes_nothing() {
        assert_eq!(super::interview_origin().mode, Some(locust_core::approval::Mode::Ask));
        assert_eq!(super::interview_origin().started_by, None);
    }
}

#[cfg(test)]
mod saved_rules_tests {
    use super::*;
    use locust_core::approval_rules::{ApprovalRule, RuleEffect, RuleKind};
    use rune::{Env, Runtime};

    fn sample_rules(count: usize) -> Vec<ApprovalRule> {
        (0..count)
            .map(|i| ApprovalRule {
                rule_id: format!("rule_{i}"),
                effect: RuleEffect::Allow,
                kind: RuleKind::Command,
                pattern: format!("echo {i}"),
                teammate_id: None,
                folder: None,
                created_at: "2026-09-29T12:00:00.000Z".into(),
                uses: None,
                last_used_at: None,
            })
            .collect()
    }

    fn draw(confirming: bool, count: usize) -> Vec<String> {
        let mut s = State::default();
        s.people_panel.open = true;
        s.people_panel.tab = 2;
        s.people_panel.rules = sample_rules(count);
        s.people_panel.confirming_remove_all = confirming;
        let mut rt = Runtime::new(
            s,
            |s, _| view(s, &Palette::new(true)),
            (800.0, 900.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements().into_iter().filter_map(|e| e.text).collect()
    }

    // every-saved-rule-can-go-at-once.test.tsx
    #[test]
    fn offers_to_start_over_and_nothing_more_until_pressed() {
        let texts = draw(false, 3);
        let joined = texts.join("\n");
        assert!(texts.iter().any(|t| t == "Remove all rules"), "{joined}");
        assert!(!joined.contains("Every card asks again"), "{joined}");
        assert!(!joined.contains("Keep them"), "{joined}");
    }

    // every-saved-rule-can-go-at-once.test.tsx
    #[test]
    fn then_says_how_many_go_and_that_every_card_asks_again_with_a_way_back() {
        let texts = draw(true, 3);
        let joined = texts.join("\n");
        assert!(
            joined.contains("Remove all 3 saved rules? Every card asks again afterwards."),
            "{joined}"
        );
        assert!(texts.iter().any(|t| t == "Remove all"), "{joined}");
        assert!(texts.iter().any(|t| t == "Keep them"), "{joined}");

        let one = draw(true, 1).join("\n");
        assert!(one.contains("Remove the 1 saved rule?"), "{one}");
    }
}

#[cfg(test)]
mod renderer_defaults_tests {
    use super::*;
    fn form(team: Value, initial: Option<&str>) -> State {
        let mut s = State::default();
        s.people_panel.team = json!({"teammates": team});
        edit(&mut s, initial);
        s
    }
    fn wren() -> Value {
        json!({"teammateId":"tm_wren", "name":"Wren", "hue":"lime", "role":"Code & Migrations", "createdAt":"2026-09-23T05:00:00.000Z", "avatar":{"headwear":0,"accessory":0,"mouth":0}})
    }
    fn render(s: State) -> rune::Runtime<State> {
        let mut rt = rune::Runtime::new(s, |s, _| edit_form(s, &Palette::new(true)), (1100.0, 1800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0); rt
    }
    fn texts(rt: &rune::Runtime<State>) -> String {
        rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
    }
    #[test]
    fn is_where_the_dialog_opens_and_an_edit_keeps_the_colour_it_has() {
        let fresh = form(json!([{"hue":"lime"}]), None);
        assert_eq!(fresh.people_panel.entry["hue"], "blue");
        let fresh = render(fresh);
        assert_eq!(crate::look_picker::tests::checked_in(&fresh, "team-hues"), vec!["Blue".to_owned()]);
        let rose = json!({"teammateId":"tm_rose", "name":"Rosa", "hue":"rose", "role":"Custom", "createdAt":"2026-09-23T05:00:00.000Z", "avatar":{"headwear":0,"accessory":0,"mouth":0}});
        let edit = render(form(json!([{"hue":"lime"}, rose]), Some("tm_rose")));
        assert_eq!(crate::look_picker::tests::checked_in(&edit, "team-hues"), vec!["Rose".to_owned()]);
    }
    #[test]
    fn says_when_it_is_taken_and_will_not_save_it() {
        let mut s = form(json!([wren(), {"teammateId":"other", "name":"wren"}]), Some("tm_wren"));
        assert_eq!(form_name_clash(&s).as_deref(), Some("wren"));
        // Guard the action as well as disabling the rendered button.
        save(&mut s);
        assert!(s.people_panel.error.is_empty());
        let rt = render(s);
        assert!(texts(&rt).contains("Another teammate is already called wren."));
        assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("team-save")));
    }
    #[test]
    fn says_nothing_about_a_name_nobody_else_has() {
        let s = form(json!([wren(), {"teammateId":"other", "name":"Booty"}]), Some("tm_wren"));
        assert!(form_name_clash(&s).is_none());
        assert!(!texts(&render(s)).contains("already called"));
    }
    #[test]
    fn is_said_the_same_way_on_home_and_the_team_screen() {
        // These Electron source assertions pin absent user-facing words, rather than React structure.
        for source in [include_str!("people_panel.rs"), include_str!("home_panel.rs")] {
            let production = source.split("#[cfg(test)]").next().unwrap();
            for forbidden in ["runs on the model you pick", "not run yet · runs on", "route set by their first mission"] {
                assert!(!production.contains(forbidden));
            }
        }
    }
    #[test]
    fn names_the_way_in_not_a_drop_that_does_not_exist() {
        let mut s = State::default();
        s.people_panel.team = json!({"teammates":[]});
        let mut rt = rune::Runtime::new(s, |s, _| share_card_view(s, &Palette::new(true)), (1100.0, 780.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        let words = texts(&rt);
        assert!(!words.contains("dropped into Locust"));
        assert!(words.contains("can add the team from it with Add team from image"), "{words}");
    }
}


#[cfg(test)]
mod renderer_work_age_tests {
    use super::ago_label_at;

    fn age(iso: &str) -> Option<String> {
        ago_label_at(iso, chrono::DateTime::parse_from_rfc3339("2026-09-03T12:00:00.000Z").unwrap().with_timezone(&chrono::Utc))
    }

    #[test]
    fn reads_in_the_largest_unit_that_fits() {
        assert_eq!(age("2026-09-03T11:58:00.000Z").as_deref(), Some("2 minutes ago"));
        assert_eq!(age("2026-09-03T09:00:00.000Z").as_deref(), Some("3 hours ago"));
        assert_eq!(age("2026-09-01T12:00:00.000Z").as_deref(), Some("2 days ago"));
        assert_eq!(age("2026-08-20T12:00:00.000Z").as_deref(), Some("2 weeks ago"));
    }
    #[test]
    fn singularises_honestly() {
        assert_eq!(age("2026-09-03T11:00:00.000Z").as_deref(), Some("1 hour ago"));
        assert_eq!(age("2026-09-02T12:00:00.000Z").as_deref(), Some("1 day ago"));
    }
    #[test]
    fn calls_anything_under_a_minute_just_now_including_a_clock_that_ran_ahead() {
        assert_eq!(age("2026-09-03T11:59:30.000Z").as_deref(), Some("just now"));
        assert_eq!(age("2026-09-03T12:00:30.000Z").as_deref(), Some("just now"));
    }
    #[test]
    fn says_nothing_for_a_timestamp_it_cannot_read() {
        assert_eq!(age("not a date"), None);
    }
}

#[cfg(test)]
mod renderer_work_surface_tests {
    use super::*;
    #[test]
    fn team_cards_use_recorded_ids_and_report_only_their_own_usage() {
        let mut s=State::default();
        s.people_panel.team=serde_json::json!({"teammates":[{"teammateId":"tm_atlas","name":"Atlas","role":"Code & Migrations"}],
            "missionOwners":{"m1":"tm_atlas","m2":"tm_other"}});
        for (id,title,input,output) in [("m1","OWN-WORK",100,10),("m2","OTHER-PRIVATE",999,99)] {
            s.recent.push(ConversationRow {id:id.into(),title:title.into(),teammate:"Atlas".into(),started_by:"native".into(),state:"completed".into(),updated:"2026-09-03T10:00:00.000Z".into()});
            s.ledger_summaries.insert(id.into(),ledger::Summary {mission_id:id.into(),title:title.into(),updated:"2026-09-03T10:00:00.000Z".into(),
                cost:Some(locust_core::benchmark::RunCost {input_tokens:Some(input),output_tokens:Some(output),..Default::default()}),..Default::default()});
        }
        let mut rt=rune::Runtime::new(s,|s,_| view(s,&Palette::new(true)),(1200.0,1800.0),rune::Env::new(true,1.0));
        rt.frame(0.0);
        let shown=rt.elements().into_iter().filter_map(|e|e.text).collect::<Vec<_>>().join("\n");
        assert!(shown.contains("OWN-WORK"),"{shown}");
        assert!(!shown.contains("OTHER-PRIVATE"),"{shown}");
        assert!(shown.contains("100 in · 10 out"),"{shown}");
        assert!(!shown.contains("999"),"{shown}");
    }
}

#[cfg(test)]
mod renderer_month_tests {
    use super::*;
    fn state(limit: Option<f64>) -> State {
        let mut s = State::default();
        s.people_panel.open = true;
        s.people_panel.team = json!({"teammates":[{"teammateId":"tm_wren","name":"Wren","hue":"lime","role":"Code & Migrations","createdAt":"2026-09-01T00:00:00.000Z","monthlyLimitUsd":limit}],"missionOwners":{}});
        s
    }
    fn shown(s: State) -> String {
        let mut rt = rune::Runtime::new(s, |s,_| view(s,&Palette::new(true)), (1200.,1800.), rune::Env::new(true,1.));
        rt.frame(0.);
        rt.elements().into_iter().filter_map(|e|e.text).collect::<Vec<_>>().join("\n")
    }
    #[test]
    fn is_in_the_dialog_filled_with_the_limit_already_set() {
        let mut s=state(Some(5.));
        edit(&mut s,Some("tm_wren"));
        assert_eq!(s.people_panel.limit.text,"5.00");
        let at=chrono::Local::now().to_rfc3339();
        s.people_panel.team["missionOwners"]["m"]=json!("tm_wren");
        s.people_panel.month_turns.push(turn(&at,"m",Some(3.2)));
        let text=shown(s);
        assert!(text.contains("Monthly limit"),"{text}");
        assert!(text.contains("$3.20 spent this month so far."),"{text}");
    }
    fn turn(at: &str, conversation: &str, usd: Option<f64>) -> locust_core::usage::UsageTurn {
        locust_core::usage::UsageTurn {at:at.into(),conversation:conversation.into(),runtime:"opencode".into(),model:"own-test/model".into(),usd,premium_requests:None,plan:false,input_tokens:Some(1),output_tokens:None,cache_read_tokens:None,cache_write_tokens:None}
    }
    #[test]
    fn month_cards_use_the_calendar_month_and_recorded_owner_and_show_unpriced_separately() {
        let mut s=state(Some(5.));
        let at=chrono::Local::now().to_rfc3339();
        s.people_panel.team["missionOwners"]=json!({"m":"tm_wren","other":"tm_atlas"});
        s.people_panel.month_turns=vec![turn(&at,"m",Some(5.02)),turn(&at,"m",None),turn(&at,"other",Some(999.)),turn("2000-01-01T00:00:00Z","m",Some(999.))];
        let text=shown(s);
        assert!(text.contains("Limit reached"),"{text}"); assert!(text.contains("$5.02 of $5.00"),"{text}");
        assert!(text.contains("1 run with no price"),"{text}"); assert!(!text.contains("999"),"{text}");
        assert!(shown(state(Some(5.))).contains("$0.00 of $5.00"));
        assert!(!shown(state(None)).contains("This month"));
    }
    #[test]
    fn month_card_names_this_month_when_it_has_money_and_omits_an_empty_month() {
        let mut s=state(None);
        s.people_panel.team["missionOwners"]["m"]=json!("tm_wren");
        s.people_panel.month_turns.push(turn(&chrono::Local::now().to_rfc3339(),"m",Some(3.2)));
        let text=shown(s); assert!(text.contains("This month")); assert!(text.contains("$3.20"));
        assert!(!shown(state(None)).contains("This month"));
    }
    #[test]
    fn invalid_limits_cannot_save_and_clearing_removes_only_the_limit() {
        let mut s=state(Some(5.)); edit(&mut s,Some("tm_wren"));
        s.people_panel.limit=field("five"); assert!(build_entry_from_form(&s).is_none());
        s.people_panel.limit=field(""); let entry=build_entry_from_form(&s).unwrap();
        assert!(entry.get("monthlyLimitUsd").is_none()); assert_eq!(entry["name"],"Wren");
        s.people_panel.limit=field("8"); assert_eq!(build_entry_from_form(&s).unwrap()["monthlyLimitUsd"],8.);
    }
    #[test]
    fn still_says_what_it_cost_from_the_money_the_host_read_without_events() {
        let missions=[ledger::Summary {mission_id:"m".into(),cost:Some(locust_core::benchmark::RunCost{usd:Some(0.75),..Default::default()}),..Default::default()}];
        let owners=std::collections::HashMap::from([("m".into(),"tm_wren".into())]);
        let work=locust_core::teammate_work::teammate_work("tm_wren",&missions,&owners,|m|m.title.clone());
        assert_eq!(work.cost.unwrap().usd,Some(0.75));
        assert!(locust_core::teammate_work::teammate_work("tm_wren",&[],&owners,|m|m.title.clone()).cost.is_none());
    }
    #[test]
    fn unreadable_months_are_reported_instead_of_claiming_zero_spent() {
        let mut s=state(Some(5.)); s.people_panel.month_error=Some("ledger unavailable".into());
        let text=shown(s); assert!(text.contains("This month's spending could not be read: ledger unavailable")); assert!(!text.contains("$0.00 of $5.00"));
    }
}

#[cfg(test)]
mod renderer_model_tests {
    use super::*;
    fn form(own: Option<Value>, picker: bool, levels: &[&str]) -> State {
        let mut s = State::default();
        crate::team_scene(&mut s);
        s.engine = None; // Seed the reported catalog; no CLI discovery in a unit test.
        s.teammates[0].runtime = Some("claude".into());
        s.teammates[0].model = Some("sonnet".into());
        s.teammates[0].runs_in = "accept-edits".into();
        let mut wren = json!({"teammateId":"tm_wren","name":"Wren","hue":"lime","role":"Code & Migrations","createdAt":"2026-09-05T05:00:00.000Z","avatar":{"shape":"classic","eyes":"dots","antenna":"single","mouth":"smile"}});
        if let Some(own) = own { wren["route"] = own; }
        s.people_panel.team = json!({"teammates":[wren]});
        edit(&mut s, Some("tm_wren"));
        if picker { s.engine = Some(Kind::Native); }
        s.models.insert(LEDGER_TARGET.into(), vec![engine::Models { teammate:None,runtime:"codex".into(),model:None,effort:None,
            models:vec![engine::Model {id:"gpt-6-luna".into(),name:"GPT-6 Luna".into(),efforts:levels.iter().map(|s|s.to_string()).collect()}] }]);
        s
    }
    const LEVELS: &[&str] = &["low","medium","high","xhigh"];
    fn own(effort: Option<&str>) -> Value {
        let mut r = json!({"runtime":"codex","model":"gpt-6-luna","mode":"ask"});
        if let Some(e) = effort { r["effort"] = json!(e); } r
    }
    fn render(s: State) -> rune::Runtime<State> {
        let mut rt = rune::Runtime::new(s, |s,_| edit_form(s,&Palette::new(true)), (1100.,2200.),rune::Env::new(true,1.));
        rt.frame(0.); rt
    }
    fn texts(rt: &rune::Runtime<State>) -> String { rt.elements().into_iter().filter_map(|e|e.text).collect::<Vec<_>>().join("\n") }
    #[test]
    fn names_the_model_the_teammate_runs_on_and_offers_the_chats_own_picker() {
        let mut s = form(Some(own(Some("low"))),true,LEVELS);
        s.models.insert(LEDGER_TARGET.into(), vec![]); // Electron's empty picker catalog.
        let rt = render(s);
        assert!(texts(&rt).contains("Codex / GPT-6 Luna"));
        assert!(rt.find("team-model-change").is_some());
        assert!(!texts(&rt).contains("Whichever route is active when a mission starts"));
    }
    #[test]
    fn says_a_teammate_with_none_of_its_own_takes_the_chat_boxs_until_one_is_picked() {
        let mut s = form(None,true,LEVELS);
        edit(&mut s, None); // No initial teammate, as in the Electron creation case.
        s.models.insert(LEDGER_TARGET.into(), vec![]);
        let rt = render(s);
        assert!(texts(&rt).contains("the chat box's, until you pick one"));
        assert!(texts(&rt).contains("Claude Code / sonnet"));
        assert!(build_entry_from_form(&rt.state).unwrap().get("route").is_none());
    }
    #[test]
    fn offers_nothing_to_press_when_there_is_no_picker_to_open() {
        let rt = render(form(None,false,LEVELS));
        assert!(rt.find("team-model-change").is_none());
    }
    #[test]
    fn states_the_teammates_own_level_with_this_models_stops() {
        let rt = render(form(Some(own(Some("high"))),true,LEVELS));
        for i in 0..4 { assert!(rt.find(&format!("team-effort-{i}")).is_some()); }
        assert!(rt.find("team-effort-4").is_none());
        assert!(rt.elements().iter().any(|e|e.label.as_deref()==Some("High: selected")));
    }
    #[test]
    fn says_the_models_default_when_the_teammate_has_no_level_of_its_own() {
        let rt = render(form(Some(own(None)),true,LEVELS));
        assert!(rt.elements().iter().any(|e|e.label.as_deref()==Some("Medium: selected")));
    }
    #[test]
    fn is_not_offered_for_a_model_that_reports_no_levels() {
        let rt = render(form(Some(own(None)),true,&[]));
        assert!(rt.find("team-effort").is_none());
    }
    fn click(rt: &mut rune::Runtime<State>, key: &str) {
        let (x,y)=rt.find(key).unwrap_or_else(||panic!("missing {key}"));
        rt.pointer_down(x,y); rt.pointer_up(x,y); rt.frame(0.1);
    }
    #[test]
    fn picking_a_model_and_effort_updates_only_the_form_and_saves_the_exact_route() {
        let mut s = form(None,true,LEVELS);
        s.people_panel.model_open = true;
        let before = s.picks.clone();
        let mut rt = render(s);
        click(&mut rt,"model-codex-gpt-6-luna");
        assert_eq!(rt.state.picks,before);
        assert_eq!(rt.state.teammates[0].runtime.as_deref(),Some("claude"));
        assert!(!rt.state.people_panel.model_open);
        click(&mut rt,"team-effort-2");
        let saved = build_entry_from_form(&rt.state).unwrap();
        assert_eq!(saved["route"]["runtime"],"codex");
        assert_eq!(saved["route"]["model"],"gpt-6-luna");
        assert_eq!(saved["route"]["effort"],"high");
        assert_eq!(saved["route"]["mode"],"accept-edits");
    }
    #[test]
    fn a_different_model_clears_the_previous_level_and_the_same_model_keeps_it() {
        let mut s = form(Some(own(Some("high"))),true,LEVELS);
        pick_model(&mut s, TurnPick {model:"gpt-6-luna".into(), runtime:Some("codex".into()), effort:None, account:Some(None)});
        assert_eq!(build_entry_from_form(&s).unwrap()["route"]["effort"],"high");
        pick_model(&mut s, TurnPick {model:"sonnet".into(), runtime:Some("claude".into()), effort:Some("high".into()), account:Some(None)});
        let saved = build_entry_from_form(&s).unwrap();
        assert!(saved["route"].get("effort").is_none());
        assert_eq!(saved["route"]["runtime"],"claude");
        assert_eq!(saved["route"]["mode"],"ask");
    }

    #[test]
    fn a_new_team_without_a_composer_route_can_still_pick_its_first_model() {
        let mut s = form(None,true,LEVELS);
        s.teammates.clear();
        s.demo = true; // Change uses the seeded catalog without launching discovery.
        edit(&mut s, None);
        let mut rt = render(s);
        assert!(texts(&rt).contains("The chat box’s, when they start"));
        assert!(build_entry_from_form(&rt.state).unwrap().get("route").is_none());
        click(&mut rt,"team-model-change");
        assert!(rt.state.people_panel.model_open);
        click(&mut rt,"model-codex-gpt-6-luna");
        let saved = build_entry_from_form(&rt.state).unwrap();
        assert_eq!(saved["route"]["runtime"],"codex");
        assert_eq!(saved["route"]["model"],"gpt-6-luna");
        assert!(rt.state.picks.is_empty());
    }

    #[test]
    fn choosing_an_account_on_an_inherited_route_saves_that_route_without_changing_the_chat() {
        let mut s = form(None,true,LEVELS);
        set_form_account(&mut s, Some("account_work".into()));
        let saved = build_entry_from_form(&s).unwrap();
        assert_eq!(saved["route"]["runtime"],"claude");
        assert_eq!(saved["route"]["model"],"sonnet");
        assert_eq!(saved["route"]["account"],"account_work");
        assert!(s.picks.is_empty());
        assert!(s.teammates[0].account.is_none());
    }

}

#[cfg(test)]
mod renderer_folder_tests {
    use super::*;
    fn form(folder: Option<&str>, host: bool, mode: &str) -> State {
        let mut s = State::default();
        crate::team_scene(&mut s);
        s.engine = None;
        s.composer_mode = Some(mode.into());
        let mut wren = json!({"teammateId":"tm_wren", "name":"Wren", "hue":"lime", "role":"Code & Migrations", "createdAt":"2026-09-05T05:00:00.000Z"});
        if let Some(folder) = folder { wren["folder"] = json!(folder); }
        s.people_panel.team = json!({"teammates":[wren]});
        edit(&mut s, Some("tm_wren"));
        s.engine = host.then_some(Kind::Native);
        s.home.folder_path = Some("/home/dev/project".into());
        s
    }
    fn render(s: State) -> rune::Runtime<State> {
        let mut rt = rune::Runtime::new(s, |s,_| edit_form(s,&Palette::new(true)), (1100.,2400.),rune::Env::new(true,1.));
        rt.frame(0.); rt
    }
    fn texts(rt: &rune::Runtime<State>) -> String { rt.elements().into_iter().filter_map(|e|e.text).collect::<Vec<_>>().join("\n") }
    #[test]
    fn is_named_in_the_dialog_and_reads_as_the_project_folder_when_it_has_none() {
        let rt = render(form(None,true,"auto"));
        assert!(rt.find("team-folder-row").is_some());
        assert!(texts(&rt).lines().any(|l|l == "Works in"));
        assert!(texts(&rt).contains("The project folder"));
        assert!(texts(&rt).contains("Choose folder"));
        assert!(rt.find("team-folder-reset").is_none());
    }
    #[test]
    fn shows_the_folder_it_was_pointed_at_and_the_way_back() {
        let rt = render(form(Some("/home/dev/claude"),true,"auto"));
        assert!(texts(&rt).contains("/home/dev/claude"));
        assert!(texts(&rt).contains("Use the project folder"));
        assert!(texts(&rt).contains("Change"));
        assert!(!texts(&rt).lines().any(|l|l == "The project folder"));
    }
    #[test]
    fn says_why_the_folder_is_worth_setting_in_the_persons_terms() {
        let rt = render(form(None,true,"auto"));
        assert!(texts(&rt).contains("MCP server"));
        assert!(texts(&rt).contains("History and memory stay with the project"));
    }
    #[test]
    fn does_not_promise_a_permission_the_next_mission_will_not_have() {
        for (mode, words) in [("auto","may edit anything on this machine"),("ask","every write refused"),("accept-edits","may change files in this workspace"),("approve-each","asks before every command"),("plan","changes nothing")] {
            let rt = render(form(None,true,mode));
            assert!(texts(&rt).contains(words),"{mode}");
            if mode == "auto" { assert!(!texts(&rt).contains("every write refused")); }
        }
    }
    #[test]
    fn is_absent_while_a_teammate_is_being_created_or_without_a_host_chooser() {
        let mut s = form(None,true,"auto"); edit(&mut s,None);
        assert!(render(s).find("team-folder-row").is_none());
        assert!(render(form(None,false,"auto")).find("team-folder-row").is_none());
    }
    #[test]
    fn permissions_use_the_inherited_composer_mode_then_the_teammates_own_mode() {
        let mut s = form(None,true,"auto");
        assert_eq!(s.people_panel.fields[6].text,"ask");
        assert!(texts(&render(s)).contains("may edit anything on this machine"));
        s = form(None,true,"auto");
        s.people_panel.entry["route"] = json!({"runtime":"codex","model":"account-default","mode":"ask"});
        let rt = render(s);
        assert!(texts(&rt).contains("every write refused"));
        assert!(!texts(&rt).contains("may edit anything on this machine"));
    }
    #[test]
    fn resetting_the_folder_changes_only_the_form_until_saved() {
        let mut rt = render(form(Some("/home/dev/claude"),true,"auto"));
        let (x,y) = rt.find("team-folder-reset").unwrap();
        rt.pointer_down(x,y); rt.pointer_up(x,y); rt.frame(0.1);
        assert!(build_entry_from_form(&rt.state).unwrap().get("folder").is_none());
        assert_eq!(rt.state.people_panel.team["teammates"][0]["folder"],"/home/dev/claude");
        assert_eq!(rt.state.home.folder_path.as_deref(),Some("/home/dev/project"));
        assert!(rt.find("team-folder-reset").is_none());
        set_form_folder(&mut rt.state,Some(std::path::Path::new("/home/dev/new")));
        assert_eq!(build_entry_from_form(&rt.state).unwrap()["folder"],"/home/dev/new");
        assert_eq!(rt.state.home.folder_path.as_deref(),Some("/home/dev/project"));
    }
}

#[cfg(test)]
mod memory_proposal_tests {
    use super::*;
    use rune::{Env, Runtime};

    fn kept(id: &str, text: &str) -> Value {
        json!({
            "memoryId": id,
            "text": text,
            "scope": "workspace",
            "workspaceId": "ws_shop",
            "workspaceName": "shop",
            "by": { "name": "Yurt" },
            "createdAt": "2026-09-01T10:00:00.000Z",
            "status": "kept",
            "enabled": true,
            "name": "deploy-day"
        })
    }

    fn render(memories: Vec<Value>, mode: &str) -> Runtime<State> {
        let mut s = State::default();
        s.people_panel.open = true;
        s.people_panel.tab = 1;
        s.people_panel.memory_tab = true;
        s.people_panel.memory_mode = mode.to_owned();
        s.people_panel.memories = json!({ "memories": memories, "forgotten": [] });
        s.home.folder_path = Some("/tmp/shop".into());
        let mut rt = Runtime::new(
            s,
            |s, _| memory_screen(s, &Palette::new(true)),
            (900.0, 2000.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt
    }

    fn texts(rt: &Runtime<State>) -> Vec<String> {
        rt.elements().into_iter().filter_map(|e| e.text).collect()
    }

    // a-proposed-change-says-what-it-changes.test.tsx
    #[test]
    fn a_proposed_rewrite_shows_new_words_old_words_and_keep_the_change_or_old_one() {
        let k = kept("mem_kept", "Deploys go out on Fridays.");
        let mut rewrite = k.clone();
        rewrite["memoryId"] = json!("mem_change");
        rewrite["text"] = json!("Deploys go out on Thursdays.");
        rewrite["status"] = json!("proposed");
        rewrite["replaces"] = json!("mem_kept");
        let rt = render(vec![k, rewrite], "ask");
        let all = texts(&rt);
        let joined = all.join("\n");
        assert!(all.iter().any(|t| t == "Wants to change this to: "), "{joined}");
        assert!(all.iter().any(|t| t == "Deploys go out on Thursdays."), "{joined}");
        assert!(all.iter().any(|t| t == "Now: Deploys go out on Fridays."), "{joined}");
        assert!(all.iter().any(|t| t == "Keep the change"), "{joined}");
        assert!(all.iter().any(|t| t == "Keep the old one"), "{joined}");
    }

    // a-proposed-change-says-what-it-changes.test.tsx
    #[test]
    fn a_proposed_forget_says_so_with_forget_it_and_keep_it() {
        let k = kept("mem_kept", "Deploys go out on Fridays.");
        let mut forget = k.clone();
        forget["memoryId"] = json!("mem_forget");
        forget["status"] = json!("proposed");
        forget["forgets"] = json!("mem_kept");
        let rt = render(vec![k, forget], "ask");
        let all = texts(&rt);
        let joined = all.join("\n");
        assert!(all.iter().any(|t| t == "Wants to forget this: "), "{joined}");
        assert!(all.iter().any(|t| t == "Deploys go out on Fridays."), "{joined}");
        assert!(all.iter().any(|t| t == "Forget it"), "{joined}");
        assert!(all.iter().any(|t| t == "Keep it"), "{joined}");
    }

    // a-proposed-change-says-what-it-changes.test.tsx
    #[test]
    fn a_new_proposed_memory_uses_keep_and_forget() {
        let mut fresh = kept("mem_new", "Tests run with pnpm test.");
        fresh["status"] = json!("proposed");
        let rt = render(vec![fresh], "ask");
        let all = texts(&rt);
        let joined = all.join("\n");
        assert!(all.iter().any(|t| t == "Keep"), "{joined}");
        assert!(all.iter().any(|t| t == "Forget"), "{joined}");
        assert!(!joined.contains("Wants to "), "{joined}");
    }

    // a-tidy-suggestion-says-what-it-does.test.tsx
    #[test]
    fn a_merge_suggestion_lists_every_memory_it_replaces_with_merge_them_and_keep_them_apart() {
        let a = kept("mem_a", "Deploys go out on Thursdays.");
        let b = kept("mem_b", "We deploy every Thursday.");
        let merge = json!({
            "memoryId": "mem_m",
            "text": "Deploys go out every Thursday.",
            "status": "proposed",
            "merges": ["mem_a", "mem_b"],
            "by": { "name": "Wren" },
            "scope": "workspace",
            "workspaceName": "shop",
            "createdAt": "2026-09-01T10:00:00.000Z"
        });
        let rt = render(vec![a, b, merge], "auto");
        let all = texts(&rt);
        let joined = all.join("\n");
        assert!(all.iter().any(|t| t == "Wants to merge these into one: "), "{joined}");
        assert!(all.iter().any(|t| t == "Deploys go out every Thursday."), "{joined}");
        assert!(all.iter().any(|t| t == "Now: Deploys go out on Thursdays."), "{joined}");
        assert!(all.iter().any(|t| t == "Now: We deploy every Thursday."), "{joined}");
        assert!(all.iter().any(|t| t == "Merge them"), "{joined}");
        assert!(all.iter().any(|t| t == "Keep them apart"), "{joined}");
    }

    // a-tidy-suggestion-says-what-it-does.test.tsx
    #[test]
    fn a_retirement_suggestion_says_why_without_citing_model_ids() {
        let a = kept("mem_a", "Deploys go out on Thursdays.");
        let c = kept("mem_c", "The API is on port 3000.");
        let retire = json!({
            "memoryId": "mem_r",
            "text": "The API is on port 3000.",
            "status": "proposed",
            "forgets": "mem_c",
            "reason": "the API moved to 3001 (mem_a)",
            "by": { "name": "Wren" },
            "scope": "workspace",
            "workspaceName": "shop",
            "createdAt": "2026-09-01T10:00:00.000Z"
        });
        let rt = render(vec![a, c, retire], "auto");
        let all = texts(&rt);
        let joined = all.join("\n");
        assert!(all.iter().any(|t| t == "Wants to forget this: "), "{joined}");
        assert!(all.iter().any(|t| t == "Why: the API moved to 3001"), "{joined}");
        assert!(!joined.contains("mem_a"), "{joined}");
    }

    // a-tidy-suggestion-says-what-it-does.test.tsx
    #[test]
    fn offers_a_tidy_pass_when_two_or_more_and_mode_is_on_and_nudges_past_limit() {
        let a = kept("mem_a", "Deploys go out on Thursdays.");
        let b = kept("mem_b", "We deploy every Thursday.");
        let offered = render(vec![a.clone(), b.clone()], "auto");
        let joined = texts(&offered).join("\n");
        assert!(joined.contains("Tidy up…"), "{joined}");
        assert!(joined.contains("suggests merges and retirements; nothing changes until you keep it"), "{joined}");
        assert!(!texts(&render(vec![a.clone()], "auto")).join("\n").contains("Tidy up…"));
        assert!(!texts(&render(vec![a, b], "off")).join("\n").contains("Tidy up…"));

        let at_limit: Vec<Value> = (0..locust_core::memory_tidy::TIDY_NUDGE_AT)
            .map(|i| kept(&format!("mem_{i}"), &format!("Note number {i}.")))
            .collect();
        assert!(!texts(&render(at_limit.clone(), "auto")).join("\n").contains("memories here."));

        let mut past_limit = at_limit;
        past_limit.push(kept("mem_extra", "One more note."));
        let expected = format!("{} memories here.", locust_core::memory_tidy::TIDY_NUDGE_AT + 1);
        assert!(texts(&render(past_limit, "auto")).join("\n").contains(&expected));
    }

    // a-tidy-pass-is-answered-in-one-press.test.tsx
    #[test]
    fn keep_all_is_offered_for_two_or_more_waiting_and_suggestions_wear_mark_not_switch() {
        let a = kept("mem_a", "Deploys go out on Thursdays.");
        let b = kept("mem_b", "We deploy every Thursday.");
        let c = kept("mem_c", "The API is on port 3000.");
        let merge = json!({
            "memoryId": "mem_m",
            "text": "Deploys go out every Thursday.",
            "status": "proposed",
            "merges": ["mem_a", "mem_b"],
            "by": { "name": "Wren" },
            "scope": "workspace",
            "workspaceName": "shop",
            "createdAt": "2026-09-01T10:00:00.000Z"
        });
        let retire = json!({
            "memoryId": "mem_r",
            "text": "The API is on port 3000.",
            "status": "proposed",
            "forgets": "mem_c",
            "reason": "the API moved to 3001",
            "by": { "name": "Wren" },
            "scope": "workspace",
            "workspaceName": "shop",
            "createdAt": "2026-09-01T10:00:00.000Z"
        });

        let one = render(vec![a.clone(), b.clone(), c.clone(), merge.clone()], "auto");
        assert!(!texts(&one).join("\n").contains("Keep all"));

        let two = render(vec![a, b, c, merge, retire], "auto");
        let joined = texts(&two).join("\n");
        assert!(texts(&two).iter().any(|t| t == "Keep all 2"), "{joined}");
        assert!(joined.contains("Anything you keep can be put back."), "{joined}");

        let switches = two.elements().into_iter().filter(|e| e.role == "switch").count();
        assert_eq!(switches, 3, "only kept memories wear switches");
        assert!(two.find("memory-suggested-mem_m").is_some());
        assert!(two.find("memory-suggested-mem_r").is_some());
    }

    // a-memory-shows-what-it-said-before.test.tsx
    #[test]
    fn a_rewritten_memory_shows_what_it_said_before_when_it_changed_and_put_it_back() {
        let mut rewritten = kept("mem_1", "Orb suite: 31/31 passing.");
        rewritten["previousText"] = json!("Orb suite: 29/31, two flaky.");
        rewritten["updatedAt"] = json!("2026-09-24T05:00:00.000Z");
        let rt = render(vec![rewritten], "auto");
        let all = texts(&rt);
        let joined = all.join("\n");
        assert!(all.iter().any(|t| t == "Was: Orb suite: 29/31, two flaky."), "{joined}");
        assert!(all.iter().any(|t| t == "Put it back"), "{joined}");
        let expected_when = format!("changed {}", crate::memory_card::when_short("2026-09-24T05:00:00.000Z"));
        assert!(joined.contains(&expected_when), "expected {expected_when} in {joined}");

        let plain = render(vec![kept("mem_1", "Orb suite: 31/31 passing.")], "auto");
        let plain_joined = texts(&plain).join("\n");
        assert!(!plain_joined.contains("Was: "), "{plain_joined}");
        assert!(!plain_joined.contains("Put it back"), "{plain_joined}");
        assert!(!plain_joined.contains("changed "), "{plain_joined}");
    }

    // MemoryScreen.tsx draws "Was:" only when the earlier words differ from the ones kept now.
    #[test]
    fn a_memory_put_back_to_its_old_words_shows_nothing_extra() {
        let mut same = kept("mem_1", "Orb suite: 31/31 passing.");
        same["previousText"] = json!("Orb suite: 31/31 passing.");
        let joined = texts(&render(vec![same], "auto")).join("\n");
        assert!(!joined.contains("Was: "), "{joined}");
        assert!(!joined.contains("Put it back"), "{joined}");
    }

    // a-tidy-pass-is-answered-in-one-press.test.tsx: the press itself keeps every waiting memory.
    #[test]
    fn keep_all_pressed_keeps_every_waiting_memory() {
        let root = std::env::temp_dir().join(format!(
            "locust-keep-all-{}-{}",
            std::process::id(),
            locust_core::invariants::unique_nanos()
        ));
        let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let waiting = |id: &str, text: &str| {
            let mut m = kept(id, text);
            m["status"] = json!("proposed");
            m["name"] = json!(id);
            m
        };
        let rows = vec![waiting("mem_a", "Deploys go out on Fridays."), waiting("mem_b", "Tests run before a push.")];
        store
            .save_json("memories.json", &json!({ "schemaVersion": 1, "memories": rows.clone() }))
            .unwrap();
        let mut rt = render(rows, "ask");
        rt.update(|s, _| s.data = Some(store.clone()));
        rt.frame(0.0);
        let (x, y) = rt.find("memory-keep-all").expect("Keep all 2 is offered");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.1);
        let file = people::memories(&store).unwrap();
        let statuses: Vec<&str> = file["memories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["status"].as_str().unwrap_or("kept"))
            .collect();
        assert_eq!(statuses.len(), 2, "{file}");
        assert!(statuses.iter().all(|s| *s != "proposed"), "{file}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
