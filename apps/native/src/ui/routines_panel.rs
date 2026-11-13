use super::*;
use serde_json::{Value, json};
pub(super) struct InputDraft {
    pub key: TextEdit,
    pub label: TextEdit,
    pub kind: String,
    pub required: bool,
    pub options: TextEdit,
    pub default_val: TextEdit,
}

pub(super) struct RunDialogState {
    pub routine_id: String,
    pub routine_name: String,
    pub routine: Value,
    pub inputs: Vec<locust_core::routine_inputs::RoutineInput>,
    pub values: Vec<TextEdit>,
    pub error: String,
}

pub(super) struct ImportPreviewState {
    pub preview: locust_core::routine_io::RoutineImportPreview,
    pub teammate_id: String,
    pub error: String,
}

pub(super) struct ExportDialogState {
    pub routine: Value,
    pub paths: Vec<String>,
}

pub(super) struct Panel {
    pub open: bool,
    pub initialized: bool,
    /// Process open time for closed-miss accounting (Electron `appOpenedAt`).
    pub opened_at: chrono::DateTime<chrono::Local>,
    pub reviewed: Option<String>,
    pub file: Value,
    pub entry: Value,
    pub fields: Vec<TextEdit>,
    pub steps: Vec<TextEdit>,
    pub schedule: Value,
    pub error: String,
    /// The runner and its host, made once a data folder is known (`createRoutineRunner` in Electron's main).
    pub engine: Option<Engine>,
    /// The last thing each routine's run said (its relay notices), and whether it needs the person.
    pub notices: std::collections::HashMap<String, (String, bool)>,
    /// Which routine each started turn belongs to, so a notice finds its card.
    pub missions: std::collections::HashMap<String, String>,
    /// What the last Keep, Discard or Run came to, when it was not a refusal.
    pub said: String,
    /// The routine whose Discard was pressed once and now asks "Discard for good?".
    pub armed: Option<String>,
    pub cadence: Cadence,
    pub io: locust_core::routine_io::RoutineIO,
    pub input_rows: Vec<InputDraft>,
    pub run_dialog: Option<RunDialogState>,
    pub import_preview: Option<ImportPreviewState>,
    pub export_dialog: Option<ExportDialogState>,
    pub templates_open: bool,
}
/// How often the window looks: the schedule (Electron: once a minute), the watched folders (`POLL_MS`), and the
/// ends of the steps it follows. Tests set them to zero rather than waiting.
#[derive(Clone, Copy)]
pub(super) struct Cadence {
    pub tick: Duration,
    pub watch: Duration,
    pub ended: Duration,
}
impl Default for Cadence {
    fn default() -> Self {
        Self { tick: Duration::from_secs(60), watch: Duration::from_millis(locust_core::routine_files::POLL_MS as u64), ended: Duration::from_secs(1) }
    }
}
/// What a background run, decision or settle came to.
enum Answer {
    Said(String),
    Refused(String),
}
pub(super) struct Engine {
    pub runner: std::sync::Arc<locust_core::routine_runner::RoutineRunner>,
    pub host: std::sync::Arc<locust_core::routine_host::LiveHost>,
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    answers: std::sync::Arc<std::sync::Mutex<Vec<Answer>>>,
    last_tick: Option<std::time::Instant>,
    last_watch: Option<std::time::Instant>,
    last_ended: Option<std::time::Instant>,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            open: false,
            initialized: false,
            opened_at: chrono::Local::now(),
            reviewed: None,
            file: Value::Null,
            entry: Value::Null,
            fields: Vec::new(),
            steps: Vec::new(),
            schedule: Value::Null,
            error: String::new(),
            engine: None,
            notices: Default::default(),
            missions: Default::default(),
            said: String::new(),
            armed: None,
            cadence: Cadence::default(),
            io: locust_core::routine_io::RoutineIO::new(),
            input_rows: Vec::new(),
            run_dialog: None,
            import_preview: None,
            export_dialog: None,
            templates_open: false,
        }
    }
}
fn field(t: &str) -> TextEdit {
    let mut e = TextEdit::default();
    e.replace_text(t);
    e
}
fn reload(s: &mut State) {
    match s
        .data
        .as_ref()
        .ok_or("No native data folder".into())
        .and_then(crate::routines::list)
    {
        Ok(v) => s.routines.file = v,
        Err(e) => s.routines.error = e,
    }
}
pub(super) fn open(s: &mut State) {
    crate::room_screen::close(s);
    s.routines.open = true;
    let _ = engine(s);
    reload(s);
}
fn edit(s: &mut State, r: Value) {
    s.routines.steps = r["steps"]
        .as_array()
        .map(|ss| ss.iter().map(|s| field(s.as_str().unwrap_or(""))).collect())
        .unwrap_or_default();
    s.routines.schedule = r["schedule"].clone();
    let parameter = r["schedule"]["at"]
        .as_str()
        .or(r["schedule"]["on"].as_str())
        .or(r["schedule"]["folder"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| r["schedule"]["hours"].as_u64().unwrap_or(4).to_string());
    s.routines.fields = vec![field(r["name"].as_str().unwrap_or("")), field(&parameter)];
    s.routines.input_rows = locust_core::routine_inputs::parse_inputs_array(&r["inputs"])
        .unwrap_or_default()
        .into_iter()
        .map(|inp| InputDraft {
            key: field(&inp.key),
            label: field(&inp.label),
            kind: inp.kind.as_str().to_owned(),
            required: inp.required,
            options: field(&inp.choices.as_deref().unwrap_or(&[]).join(", ")),
            default_val: field(inp.default.as_deref().unwrap_or("")),
        })
        .collect();
    s.routines.entry = r;
}
fn result(s: &mut State, r: Result<(), String>) {
    match r {
        Ok(()) => {
            s.routines.error.clear();
            reload(s);
        }
        Err(e) => s.routines.error = e,
    }
}
fn button(label: &str, p: &Palette) -> El {
    text(label.to_owned())
        .font(type_::BODY)
        .color(p.secondary)
        .padding(6.0)
}
pub(super) fn view(s: &State, p: &Palette) -> El {
    let panel = &s.routines;
    let mut body = col()
        .w_full()
        .padding(22.0)
        .gap(12.0)
        .child(
            row()
                .gap(12.0)
                .child(
                    text("Automations")
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Close", p).on_press(|s: &mut State| s.routines.open = false)),
        );
    let owner = s.teammates.get(s.selected).map(|t| t.id.clone());
    let mut top_actions = row().gap(8.0);
    if let Some(owner) = owner {
        top_actions = top_actions.child(
            button("New routine", p)
                .key("routine-new")
                .on_press(move |s: &mut State| {
                    edit(s, json!({"name":"","steps":[""],"teammateId":owner}))
                }),
        );
    }
    top_actions = top_actions
        .child(
            button("Start from a template", p)
                .key("routine-templates-open")
                .on_press(|s: &mut State| {
                    s.routines.templates_open = !s.routines.templates_open;
                }),
        )
        .child(
            button("Open routine", p)
                .key("routine-import-file")
                .on_press_tx(|_s: &mut State, tx: &mut Tx| {
                    open_routine_file(tx);
                }),
        );
    body = body.child(top_actions);
    if panel.templates_open {
        body = body.child(templates_view(s, p));
    }
    if let Some(preview) = &panel.import_preview {
        body = body.child(import_preview_view(s, preview, p));
    }
    if let Some(exp) = &panel.export_dialog {
        body = body.child(export_dialog_view(exp, p));
    }
    if let Some(run_dlg) = &panel.run_dialog {
        body = body.child(run_dialog_view(run_dlg, p));
    }
    if !panel.said.is_empty() {
        body = body.child(text(panel.said.clone()).font(type_::LABEL).color(p.secondary).wrap().key("routines-said"));
    }
    if !panel.error.is_empty() {
        body = body.child(
            text(panel.error.clone())
                .font(type_::LABEL)
                .color(p.red)
                .wrap(),
        );
    }
    if !panel.fields.is_empty() {
        body = body
            .child(text("Name").font(type_::LABEL).color(p.secondary))
            .child(
                text_input(&panel.fields[0], type_::BODY)
                    .label("Name")
                    .w_full()
                    .padding(8.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.routines.fields[0]),
            );
        for (i, step) in panel.steps.iter().enumerate() {
            body = body
                .child(
                    text(format!("Step {}", i + 1))
                        .font(type_::LABEL)
                        .color(p.secondary),
                )
                .child(
                    text_input(step, type_::BODY)
                        .key(format!("routine-step-{i}"))
                        .label(format!("Step {}", i + 1))
                        .w_full()
                        .padding(8.0)
                        .fill(p.field)
                        .color(p.text)
                        .on_edit(move |s: &mut State| &mut s.routines.steps[i]),
                );
            body = body.child(step_choices(s, i, p));
            if panel.steps.len() > 1 {
                body = body.child(button("Remove step", p).on_press(move |s: &mut State| {
                    s.routines.steps.remove(i);
                    if let Some(h) = s.routines.entry["handOffs"].as_array_mut()
                        && i < h.len()
                    {
                        h.remove(i);
                    }
                }));
            }
        }
        if panel.steps.len() < 12 {
            body = body.child(
                button("Add a step", p).on_press(|s: &mut State| s.routines.steps.push(field(""))),
            );
        }
        body = body.child(inputs_editor(s, p));
        body = body.child(where_and_goal(s, p));
        body = body.child(
            text("Runs on its own")
                .font(type_::LABEL)
                .color(p.secondary),
        );
        for (kind, label) in [
            ("off", "When I press Run"),
            ("every", "Every few hours"),
            ("daily", "Daily"),
            ("weekly", "On set days"),
            ("once", "Once"),
            ("files", "On a new file"),
        ] {
            let selected =
                panel.schedule["kind"] == kind || (kind == "off" && panel.schedule.is_null());
            body=body.child(button(label,p).fill(if selected{p.selected}else{p.raised}).on_press(move|s:&mut State|{
                if s.routines.schedule["kind"]==kind {return;}
                s.routines.schedule=match kind {"every"=>json!({"kind":"every","hours":4}),"daily"=>json!({"kind":"daily","at":"09:00"}),"weekly"=>json!({"kind":"weekly","days":[1,2,3,4,5],"at":"09:00"}),"once"=>json!({"kind":"once","on":(chrono::Local::now()+chrono::Duration::days(1)).format("%Y-%m-%dT09:00").to_string()}),"files"=>json!({"kind":"files","folder":"inbox"}),_=>Value::Null};
                let value=if kind=="every"{"4".into()}else if kind=="once"{s.routines.schedule["on"].as_str().unwrap_or("").to_owned()}else if kind=="files"{"inbox".into()}else{"09:00".into()};s.routines.fields[1]=field(&value);
            }));
        }
        let kind = panel.schedule["kind"].as_str().unwrap_or("");
        if ["every", "daily", "weekly", "once", "files"].contains(&kind) {
            let label = if kind == "every" {
                "Hours between runs"
            } else if kind == "files" {
                "in the folder"
            } else if kind == "once" {
                "Date and time (YYYY-MM-DDTHH:MM)"
            } else {
                "Time of day"
            };
            body = body
                .child(text(label).font(type_::LABEL).color(p.secondary))
                .child(
                    text_input(&panel.fields[1], type_::BODY)
                        .label(label)
                        .w_full()
                        .padding(8.0)
                        .fill(p.field)
                        .color(p.text)
                        .on_edit(|s: &mut State| &mut s.routines.fields[1]),
                );
        }
        if kind == "weekly" {
            let mut days = row().gap(6.0);
            for (day, label) in [
                (1, "Mon"),
                (2, "Tue"),
                (3, "Wed"),
                (4, "Thu"),
                (5, "Fri"),
                (6, "Sat"),
                (0, "Sun"),
            ] {
                let selected = panel.schedule["days"]
                    .as_array()
                    .is_some_and(|ds| ds.contains(&json!(day)));
                days = days.child(
                    button(label, p)
                        .fill(if selected { p.selected } else { p.raised })
                        .on_press(move |s: &mut State| {
                            let mut ds = s.routines.schedule["days"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default();
                            if selected {
                                ds.retain(|d| d != day);
                            } else {
                                ds.push(json!(day));
                            }
                            ds.sort_by_key(|d| d.as_u64());
                            s.routines.schedule["days"] = json!(ds);
                        }),
                );
            }
            body = body.child(days);
        }
        body = body.child(
            row()
                .gap(12.0)
                .child(button("Save changes", p).on_press(|s: &mut State| {
                    let mut entry = s.routines.entry.clone();
                    entry["name"] = json!(s.routines.fields[0].text);
                    entry["steps"] = json!(
                        s.routines
                            .steps
                            .iter()
                            .map(|e| e.text.clone())
                            .collect::<Vec<_>>()
                    );
                    let mut schedule = s.routines.schedule.clone();
                    let value = s.routines.fields[1].text.trim();
                    match schedule["kind"].as_str().unwrap_or("") {
                        "every" => schedule["hours"] = json!(value.parse::<u64>().unwrap_or(0)),
                        "daily" | "weekly" => schedule["at"] = json!(value),
                        "once" => schedule["on"] = json!(value),
                        "files" => schedule["folder"] = json!(value),
                        _ => {}
                    }
                    entry["schedule"] = schedule;
                    // Who takes each step, lined up with the steps as they now are.
                    let count = entry["steps"].as_array().map_or(0, Vec::len);
                    let mut hand_offs = entry["handOffs"].as_array().cloned().unwrap_or_default();
                    hand_offs.resize(count, json!({}));
                    entry["handOffs"] = json!(hand_offs);
                    entry["inCopy"] = json!(entry["inCopy"] == true);
                    if entry.get("untilCheck").is_none() {
                        entry["untilCheck"] = Value::Null;
                    }
                    let inputs_val = serialize_input_rows(&s.routines.input_rows);
                    if let Some(obj) = entry.as_object_mut() {
                        if inputs_val.is_empty() {
                            obj.remove("inputs");
                        } else {
                            obj.insert("inputs".into(), Value::Array(inputs_val));
                        }
                    }
                    let r = s
                        .data
                        .as_ref()
                        .ok_or("No native data folder".into())
                        .and_then(|store| crate::routines::save(store, entry));
                    let ok = r.is_ok();
                    result(s, r);
                    if ok {
                        s.routines.fields.clear();
                    }
                }))
                .child(button("Cancel", p).on_press(|s: &mut State| s.routines.fields.clear())),
        );
    }
    if let Some(rs) = panel.file["routines"].as_array() {
        if rs.is_empty() {
            body = body.child(text("No routines yet").font(type_::BODY).color(p.muted));
            if !panel.templates_open {
                body = body.child(templates_view(s, p));
            }
        }
        for r in rs {
            let id = r["routineId"].as_str().unwrap_or("").to_owned();
            let mut card = col()
                .w_full()
                .padding(12.0)
                .gap(8.0)
                .fill(p.raised)
                .child(
                    text(r["name"].as_str().unwrap_or("Routine").to_owned())
                        .font(type_::BODY)
                        .color(p.text),
                )
                .child(
                    text(format!(
                        "{} · {} · {} runs",
                        r["teammateId"].as_str().unwrap_or(""),
                        crate::routines::schedule::label(&r["schedule"]),
                        r["runs"].as_u64().unwrap_or(0)
                    ))
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .wrap(),
                );
            if r["paused"] == true {
                card = card.child(text("Paused").font(type_::LABEL).color(p.amber));
            }
            if let Some(history) = r["history"].as_array() {
                for h in history.iter().rev().take(3) {
                    card = card.child(
                        text(format!("Missed · {}", h["dueAt"].as_str().unwrap_or("")))
                            .font(type_::LABEL)
                            .color(p.muted),
                    );
                }
            }
            let entry = r.clone();
            let export_entry = r.clone();
            let remove = id.clone();
            let pause = id.clone();
            let run = id.clone();
            let paused = r["paused"] == true;
            card = card.child(
                row()
                    .gap(10.0)
                    .child(button("Edit", p).on_press(move |s: &mut State| edit(s, entry.clone())))
                    .child(button("Save as file", p).key(format!("routine-export-{id}")).on_press_tx(move |s: &mut State, tx: &mut Tx| {
                        begin_export_routine(s, export_entry.clone(), tx);
                    }))
                    .child(button(if paused { "Resume" } else { "Pause" }, p).on_press(
                        move |s: &mut State| {
                            let at = chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                            let r = s
                                .data
                                .as_ref()
                                .ok_or("No native data folder".into())
                                .and_then(|store| {
                                    crate::routines::pause(store, &pause, !paused, &at)
                                });
                            result(s, r);
                        },
                    ))
                    .child(button("Remove", p).on_press(move |s: &mut State| {
                        let r = s
                            .data
                            .as_ref()
                            .ok_or("No native data folder".into())
                            .and_then(|store| crate::routines::remove(store, &remove));
                        result(s, r);
                    }))
                    .child(button("Run", p).key(format!("routine-run-{id}")).on_press(move |s: &mut State| start_run(s, &run))),
            );
            if let Some((said, warn)) = panel.notices.get(&id) {
                card = card.child(text(said.clone()).font(type_::LABEL).color(if *warn { p.amber } else { p.secondary }).wrap());
            }
            if let Some(failed) = r["lastFailed"].as_str() {
                card = card.child(text(format!("Last run: {failed}")).font(type_::LABEL).color(p.muted).wrap());
            }
            card = recovery_card(s, r, card, p);
            card = changes_card(s, r, card, p);
            body = body.child(card.key(format!("routine-{id}")));
        }
    }
    body.scroll(ScrollSpec { stick_bottom: false }).h_full()
}
pub(super) fn scene(s: &mut State) {
    crate::scenes::team_scene(s);
    s.routines.open = true;
    s.routines.file = json!({"schemaVersion":1,"routines":[{"routineId":"rt_review","name":"Review the weekly changes","teammateId":"codec","steps":["Review changes"],"schedule":{"kind":"weekly","days":[1,2,3,4,5],"at":"09:00"},"runs":3,"paused":true,"history":[{"kind":"missed","dueAt":"2026-11-09T14:00:00Z"}]}]});
}
pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}


/// The step's entry in who-takes-each-step, the list lined up with the steps first.
fn hand_off(s: &mut State, i: usize) -> &mut Value {
    let count = s.routines.steps.len();
    let entry = &mut s.routines.entry;
    let mut list = entry["handOffs"].as_array().cloned().unwrap_or_default();
    list.resize(count.max(i + 1), json!({}));
    entry["handOffs"] = json!(list);
    &mut entry["handOffs"][i]
}

fn tick_box(on: bool, label: &str) -> String {
    format!("{} {label}", if on { "✓" } else { "□" })
}

/// Who takes the step and whether it checks (RoutineDialog's step row), and, from step 2 on, Wait for my OK first.
fn step_choices(s: &State, i: usize, p: &Palette) -> El {
    let panel = &s.routines;
    let entry = &panel.entry["handOffs"][i];
    let owner = panel.entry["teammateId"].as_str().unwrap_or("").to_owned();
    let name_of = |id: &str| s.teammates.iter().find(|t| t.id == id).map(|t| t.name.clone());
    let mut choices = row().gap(10.0);
    if s.teammates.len() > 1 {
        let who = match entry["teammateId"].as_str() {
            Some(id) => name_of(id).unwrap_or_else(|| id.to_owned()),
            None => name_of(&owner).map_or("The teammate who runs it".into(), |n| format!("{n} (runs it)")),
        };
        let ids: Vec<String> = s.teammates.iter().map(|t| t.id.clone()).filter(|id| *id != owner).collect();
        choices = choices
            .child(button(&format!("Takes step {}: {who}", i + 1), p).key(format!("routine-step-who-{i}")).on_press(move |s: &mut State| {
                let at = hand_off(s, i);
                let now = at["teammateId"].as_str().map(str::to_owned);
                let next = match now.and_then(|n| ids.iter().position(|id| *id == n)) {
                    None => ids.first().cloned(),
                    Some(k) => ids.get(k + 1).cloned(),
                };
                match (next, at.as_object_mut()) {
                    (Some(id), Some(o)) => { o.insert("teammateId".into(), json!(id)); }
                    (None, Some(o)) => { o.remove("teammateId"); }
                    _ => {}
                }
            }))
            .child(button(&tick_box(entry["check"] == true, "Checker: must approve"), p).key(format!("routine-step-check-{i}")).on_press(move |s: &mut State| {
                let at = hand_off(s, i);
                let on = at["check"] == true;
                if let Some(o) = at.as_object_mut() {
                    if on { o.remove("check"); } else { o.insert("check".into(), json!(true)); }
                }
            }));
    }
    if i > 0 {
        choices = choices.child(button(&tick_box(entry["askFirst"] == true, "Wait for my OK first"), p).key(format!("routine-step-ok-{i}")).on_press(move |s: &mut State| {
            let at = hand_off(s, i);
            let on = at["askFirst"] == true;
            if let Some(o) = at.as_object_mut() {
                if on { o.remove("askFirst"); } else { o.insert("askFirst".into(), json!(true)); }
            }
        }));
    }
    choices
}

/// Where it works (0.533) and when its steps are done (0.534), in RoutineDialog's words.
fn where_and_goal(s: &State, p: &Palette) -> El {
    let entry = &s.routines.entry;
    let in_copy = entry["inCopy"] == true;
    let tries = entry["untilCheck"]["tries"].as_u64();
    let mut places = row().gap(6.0).child(text("Where it works").font(type_::LABEL).color(p.secondary));
    for (copy, label) in [(false, "In the folder"), (true, "In a copy, you keep")] {
        places = places.child(button(label, p).fill(if in_copy == copy { p.selected } else { p.raised }).key(format!("routine-in-copy-{copy}")).on_press(move |s: &mut State| {
            s.routines.entry["inCopy"] = json!(copy);
        }));
    }
    let mut goal = row().gap(6.0).child(text("When its steps are done").font(type_::LABEL).color(p.secondary));
    goal = goal.child(button("Stop", p).fill(if tries.is_none() { p.selected } else { p.raised }).on_press(|s: &mut State| {
        s.routines.entry["untilCheck"] = Value::Null;
    }));
    goal = goal.child(button("Keep going until the check passes", p).fill(if tries.is_some() { p.selected } else { p.raised }).key("routine-goal").on_press(move |s: &mut State| {
        // at most 1 to 5 fixes, round again; three first, as RoutineDialog starts.
        let next = match tries { None => 3, Some(5) => 1, Some(n) => n + 1 };
        s.routines.entry["untilCheck"] = json!({"tries": next});
    }));
    if let Some(n) = tries {
        goal = goal.child(text(format!("at most {n} {}", if n == 1 { "fix" } else { "fixes" })).font(type_::LABEL).color(p.secondary));
    }
    let note = if in_copy {
        "It changes files in a copy of the folder. When a run finishes, its changes wait under Routines: Keep writes them into the folder, Discard throws them away."
    } else {
        "What a run changes lands in the folder straight away, with nothing to keep or undo first."
    };
    col().gap(6.0).child(places).child(text(note).font(type_::LABEL).color(p.muted).wrap()).child(goal)
}

/// When a run started, as `toLocaleString` says it here.
fn local_time(at: &Value) -> String {
    at.as_str()
        .and_then(|a| chrono::DateTime::parse_from_rfc3339(a).ok())
        .map(|d| d.with_timezone(&chrono::Local).format("%-m/%-d/%Y, %-I:%M:%S %p").to_string())
        .unwrap_or_default()
}

/// When Keep (or Stop here) runs it next, said before the click (0.404).
fn next_run_words(r: &Value) -> String {
    let now = chrono::Local::now();
    match r.get("schedule").filter(|s| !s.is_null()) {
        None => "when you press Run".to_owned(),
        Some(sched) if sched["kind"] == "files" => "when the next new file arrives".into(),
        Some(sched) => match crate::routines::schedule::next(sched, now, now) {
            Some(at) => format!("at {}, not straight away", at.format("%a %-I:%M %p")),
            None => "when you press Run, since its one scheduled time has passed".into(),
        },
    }
}

fn open_button(label: &str, mission: String, p: &Palette) -> El {
    button(label, p).on_press_tx(move |s: &mut State, tx| {
        if let Some(row) = s.recent.iter().find(|row| row.id == mission).cloned() {
            s.routines.open = false;
            open_recent(s, row, tx);
        } else {
            s.routines.error = "The conversation is not in Recent yet. Reload the team, then open Automations again.".into();
        }
    })
}

/// RoutineRecovery: a held attempt's card -- a question when the routine asked to wait for an OK, a review otherwise.
fn recovery_card(s: &State, r: &Value, mut card: El, p: &Palette) -> El {
    let Some(e) = r.get("execution").filter(|e| e.is_object()) else { return card };
    let id = r["routineId"].as_str().unwrap_or("").to_owned();
    let (step, of) = (e["step"].as_u64().unwrap_or(1), e["of"].as_u64().unwrap_or(1));
    if e["status"] == "running" || e["status"] == "dispatching" {
        return card.child(text(format!("Running · step {step} of {of}")).font(type_::LABEL).color(p.secondary));
    }
    let attempt = e["attemptId"].as_str().unwrap_or("").to_owned();
    let next_run = next_run_words(r);
    let mission = e["missionId"].as_str().map(str::to_owned);
    // WAITING FOR YOUR OK (0.840): a question, not a recovery.
    if e["status"] == "held" && e["waitsForOk"] == true {
        let next = step + 1;
        card = card
            .child(text(format!("Waiting for your OK · Step {next} of {of} · Run started {}", local_time(&e["startedAt"]))).font(type_::LABEL).color(p.amber).wrap())
            .child(text(e["reason"].as_str().map_or_else(|| format!("Step {next} waits for your OK."), str::to_owned)).font(type_::LABEL).color(p.text).wrap());
        if let Some(words) = e["steps"][step as usize].as_str() {
            let shown = if words.chars().count() > 280 { format!("{}…", words.chars().take(279).collect::<String>().trim_end()) } else { words.to_owned() };
            card = card.child(text(format!("Step {next}: {shown}")).font(type_::LABEL).color(p.secondary).wrap());
        }
        if let Some(m) = mission {
            card = card.child(open_button("Open the conversation", m, p));
        }
        let (go, stop) = ((id.clone(), attempt.clone()), (id.clone(), attempt));
        return card
            .child(text(format!("Stop here ends this run after step {step}; the routine runs again from step 1 {next_run}.")).font(type_::LABEL).color(p.secondary).wrap())
            .child(
                row()
                    .gap(10.0)
                    .child(button(&format!("Go ahead with step {next}"), p).key(format!("routine-go-ahead-{id}")).on_press(move |s: &mut State| decide(s, &go.0, &go.1, step, "continue")))
                    .child(button("Stop here", p).key(format!("routine-stop-{id}")).on_press(move |s: &mut State| decide(s, &stop.0, &stop.1, step, "keep"))),
            );
    }
    let abandoned = e["status"] == "abandoned";
    let reason = e["reason"].as_str().unwrap_or("Dispatch outcome is uncertain. Nothing will be replayed automatically.");
    let reason = match reason.strip_prefix("Review required: ") {
        Some(rest) => {
            let mut c = rest.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
        None => reason.to_owned(),
    };
    card = card
        .child(
            text(format!(
                "{} · Step {step} of {of} · Attempt started {}",
                if abandoned { "Attempt abandoned" } else { "Waiting for your review" },
                local_time(&e["startedAt"])
            ))
            .font(type_::LABEL)
            .color(p.amber)
            .wrap(),
        )
        .child(text(reason).font(type_::LABEL).color(p.text).wrap());
    if let Some(m) = mission {
        card = card.child(open_button("Open the saved conversation", m, p));
    }
    if abandoned {
        return card;
    }
    let route = &e["route"];
    card = card.child(
        text(format!("{} · {} · {}", route["runtime"].as_str().unwrap_or(""), route["model"].as_str().unwrap_or(""), route["mode"].as_str().unwrap_or("")))
            .font(type_::LABEL)
            .color(p.muted),
    );
    let last = step >= of;
    let can_continue = e["canContinue"] == true;
    if last {
        card = card.child(text("This was the last step, so there is nothing left to continue.").font(type_::LABEL).color(p.secondary).wrap());
    } else if !can_continue {
        let why = if e["settledAtDispatch"] == true {
            "Continue would be refused the same way, so it is off until that changes.".to_owned()
        } else {
            format!("Continue waits for step {step} to be confirmed finished. It was not, so step {} would build on work that may not be there.", step + 1)
        };
        card = card.child(text(why).font(type_::LABEL).color(p.secondary).wrap());
    }
    card = card.child(
        text(format!("Keep clears this attempt and leaves the routine as it was; its next run starts from step 1 {next_run}. Abandon also removes the schedule. Neither stops a runtime or undoes work."))
            .font(type_::LABEL)
            .color(p.secondary)
            .wrap(),
    );
    let reviewed = s.routines.reviewed.as_deref() == Some(attempt.as_str());
    let review = attempt.clone();
    card = card.child(
        button(&tick_box(reviewed, "I reviewed the saved conversation and external work, and whether the remaining work is still wanted."), p)
            .wrap()
            .key(format!("routine-reviewed-{id}"))
            .on_press(move |s: &mut State| s.routines.reviewed = if reviewed { None } else { Some(review.clone()) }),
    );
    if reviewed {
        let keep_label = if r.get("schedule").is_some_and(|s| !s.is_null()) { "Keep the schedule" } else { "Keep the routine" };
        let mut actions = row().gap(10.0);
        if !last && can_continue {
            let (i, a) = (id.clone(), attempt.clone());
            actions = actions.child(button("Continue remaining steps", p).key(format!("routine-continue-{id}")).on_press(move |s: &mut State| decide(s, &i, &a, step, "continue")));
        }
        let (i, a) = (id.clone(), attempt.clone());
        actions = actions.child(button(keep_label, p).key(format!("routine-keep-schedule-{id}")).on_press(move |s: &mut State| decide(s, &i, &a, step, "keep")));
        let (i, a) = (id.clone(), attempt);
        actions = actions.child(button("Abandon attempt and remove schedule", p).key(format!("routine-abandon-{id}")).on_press(move |s: &mut State| decide(s, &i, &a, step, "abandon")));
        card = card.child(actions);
    }
    card
}

/// RoutineChanges: a copy run's last changes, waiting for Keep or Discard.
fn changes_card(s: &State, r: &Value, card: El, p: &Palette) -> El {
    let Some(staged) = r.get("staged").filter(|s| s.is_object()) else { return card };
    let id = r["routineId"].as_str().unwrap_or("").to_owned();
    let mut files: Vec<String> = staged["changed"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_owned)).collect();
    files.extend(staged["deleted"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(|f| format!("{f} (deleted)"))));
    let n = files.len();
    let named = files.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
    let more = if n > 4 { format!(" and {} more", n - 4) } else { String::new() };
    let line = if n == 1 {
        "Nothing has reached the folder. Keep writes it into the folder; Discard throws it away. It runs again once you choose."
    } else {
        "Nothing has reached the folder. Keep writes them into the folder; Discard throws them away. It runs again once you choose."
    };
    let armed = s.routines.armed.as_deref() == Some(id.as_str());
    let (keep, discard, open) = (id.clone(), id.clone(), id.clone());
    card.child(
        col()
            .gap(6.0)
            .key(format!("routine-changes-{id}"))
            .child(text(format!("Waiting for you · its last run changed {n} file{} in its copy", if n == 1 { "" } else { "s" })).font(type_::LABEL).color(p.amber).wrap())
            .child(text(format!("{named}{more}")).font(type_::LABEL).color(p.text).wrap())
            .child(text(line).font(type_::LABEL).color(p.secondary).wrap())
            .child(
                row()
                    .gap(10.0)
                    .child(button("Keep", p).key(format!("routine-keep-{id}")).on_press(move |s: &mut State| settle(s, &keep, locust_core::routine_recovery::Settle::Keep)))
                    .child(button(if armed { "Discard for good?" } else { "Discard" }, p).key(format!("routine-discard-{id}")).on_press(move |s: &mut State| {
                        if s.routines.armed.as_deref() == Some(discard.as_str()) {
                            s.routines.armed = None;
                            settle(s, &discard, locust_core::routine_recovery::Settle::Discard);
                        } else {
                            s.routines.armed = Some(discard.clone());
                        }
                    }))
                    .child(button("Open the copy", p).key(format!("routine-open-copy-{id}")).on_press(move |s: &mut State| {
                        let Some(engine) = &s.routines.engine else { return };
                        match engine.host.routine_copies().path(&open) {
                            Ok(path) => {
                                if let Err(e) = std::process::Command::new("open").arg(path).spawn() {
                                    s.routines.error = format!("The copy could not be opened: {e}");
                                }
                            }
                            Err(e) => s.routines.error = e,
                        }
                    })),
            ),
    )
}

/// The runner and its host over this window's data folder, made the first time they are needed.
fn engine(s: &mut State) -> Option<&mut Engine> {
    let store = s.data.clone()?;
    let opened_at = s.routines.opened_at.with_timezone(&chrono::Utc);
    Some(s.routines.engine.get_or_insert_with(|| {
        let host = std::sync::Arc::new(locust_core::routine_host::LiveHost::new(store.clone(), opened_at));
        let routines = std::sync::Arc::new(locust_core::routines::progress::StoreRoutines(store));
        Engine {
            runner: std::sync::Arc::new(locust_core::routine_runner::RoutineRunner::new(routines, host.clone())),
            host,
            busy: Default::default(),
            answers: Default::default(),
            last_tick: None,
            last_watch: None,
            last_ended: None,
        }
    }))
}

/// Work for the runner, off the window's thread: a run, a decision or a settle can start a turn or copy a folder.
fn background(s: &mut State, work: impl FnOnce(&Engine) -> Answer + std::marker::Send + 'static) {
    if engine(s).is_none() {
        s.routines.error = "Open a project folder before running routines.".into();
        return;
    }
    s.routines.error.clear();
    s.routines.said.clear();
    let Some(engine) = &s.routines.engine else { return };
    let worker = Engine {
        runner: engine.runner.clone(),
        host: engine.host.clone(),
        busy: engine.busy.clone(),
        answers: engine.answers.clone(),
        last_tick: None,
        last_watch: None,
        last_ended: None,
    };
    std::thread::spawn(move || {
        let answer = work(&worker);
        worker.answers.lock().unwrap_or_else(|e| e.into_inner()).push(answer);
    });
}

fn start_run(s: &mut State, id: &str) {
    if let Some(routine) = s
        .routines
        .file
        .get("routines")
        .and_then(Value::as_array)
        .and_then(|rs| rs.iter().find(|r| r["routineId"] == id))
        .cloned()
    {
        let inputs = locust_core::routine_inputs::parse_inputs_array(&routine["inputs"]).unwrap_or_default();
        if !inputs.is_empty() {
            let values = inputs
                .iter()
                .map(|inp| {
                    let initial = if inp.kind == locust_core::routine_inputs::RoutineInputKind::Folder {
                        ""
                    } else {
                        inp.default.as_deref().unwrap_or("")
                    };
                    field(initial)
                })
                .collect();
            s.routines.run_dialog = Some(RunDialogState {
                routine_id: id.to_owned(),
                routine_name: routine["name"].as_str().unwrap_or("Routine").to_owned(),
                routine,
                inputs,
                values,
                error: String::new(),
            });
            return;
        }
    }
    let id = id.to_owned();
    background(s, move |e| match e.runner.run(&id, None) {
        Ok(Ok(_)) => Answer::Said(String::new()),
        Ok(Err(refused)) | Err(refused) => Answer::Refused(refused),
    });
}

fn decide(s: &mut State, id: &str, attempt: &str, step: u64, decision: &str) {
    let request = json!({"routineId": id, "attemptId": attempt, "step": step, "decision": decision});
    s.routines.reviewed = None;
    background(s, move |e| match locust_core::routine_recovery::decide(&request, true, Some(&e.runner)) {
        Ok(()) => Answer::Said(String::new()),
        Err(refused) => Answer::Refused(refused),
    });
}

fn settle(s: &mut State, id: &str, decision: locust_core::routine_recovery::Settle) {
    let Some(store) = s.data.clone() else { return };
    let id = id.to_owned();
    background(s, move |e| {
        let answer = locust_core::routine_recovery::settle(&store, e.host.routine_copies(), &id, decision);
        locust_core::routine_runner::Host::notify(&*e.host, locust_core::routine_runner::Update::RecoveryChanged);
        match answer {
            Ok(said) => Answer::Said(said),
            Err(refused) => Answer::Refused(refused),
        }
    });
}

pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if !s.routines.initialized
        && let Some(store) = &s.data
    {
        s.routines.initialized = true;
        if let Err(e) = crate::routines::record_misses(store, s.routines.opened_at) {
            s.routines.error = e;
        }
    }
    let cadence = s.routines.cadence;
    let Some(engine) = engine(s) else { return changed };
    // The schedule, the watched folders and the ends of the steps it follows: one piece of work at a time.
    let now = std::time::Instant::now();
    let due = |last: Option<std::time::Instant>, every: Duration| last.is_none_or(|l| now.duration_since(l) >= every);
    if !engine.busy.load(std::sync::atomic::Ordering::SeqCst) {
        let tick = due(engine.last_tick, cadence.tick);
        let watch = due(engine.last_watch, cadence.watch);
        let ended = engine.runner.following() && due(engine.last_ended, cadence.ended);
        if tick || watch || ended {
            if tick {
                engine.last_tick = Some(now);
            }
            if watch {
                engine.last_watch = Some(now);
            }
            if ended {
                engine.last_ended = Some(now);
            }
            engine.busy.store(true, std::sync::atomic::Ordering::SeqCst);
            let (runner, host, busy, answers) = (engine.runner.clone(), engine.host.clone(), engine.busy.clone(), engine.answers.clone());
            std::thread::spawn(move || {
                let mut tick = tick;
                if watch
                    && let Ok(list) = locust_core::routines::progress::all(host.store())
                    && host.watch(&list)
                {
                    tick = true;
                }
                let result = if tick { runner.tick(locust_core::routine_runner::Host::now(&*host)).map(|_| ()) } else if ended { runner.ended() } else { Ok(()) };
                if let Err(e) = result {
                    answers.lock().unwrap_or_else(|e| e.into_inner()).push(Answer::Refused(format!("Routines could not be brought up to date: {e}")));
                }
                busy.store(false, std::sync::atomic::Ordering::SeqCst);
            });
        }
    }
    let updates = engine.host.take_updates();
    let answers = std::mem::take(&mut *engine.answers.lock().unwrap_or_else(|e| e.into_inner()));
    let mut reload_now = !answers.is_empty();
    let mut listed = false;
    for update in updates {
        changed = true;
        use locust_core::routine_runner::{Level, Update};
        match update {
            Update::MissionStarted { mission_id, routine_id, .. } => {
                s.routines.missions.insert(mission_id, routine_id);
                reload_now = true;
                listed = true;
            }
            Update::Notice { mission_id, message, level, .. } => {
                if let Some(routine) = s.routines.missions.get(&mission_id).cloned() {
                    s.routines.notices.insert(routine, (message.clone(), level == Level::Warning));
                }
                if level == Level::Warning {
                    s.status = message;
                }
            }
            Update::RecoveryChanged => reload_now = true,
            Update::Blocked { name, message, retry_at, .. } => {
                let at = local_time(&json!(retry_at));
                s.routines.error = format!("\"{name}\" could not start on its own: {message} It tries again at {at}.");
            }
        }
    }
    for answer in answers {
        changed = true;
        match answer {
            Answer::Said(said) if !said.is_empty() => s.routines.said = said,
            Answer::Said(_) => {}
            Answer::Refused(refused) => s.routines.error = refused,
        }
    }
    if reload_now {
        reload(s);
    }
    if listed {
        refresh_recent(s);
    }
    changed
}

fn serialize_input_rows(rows: &[InputDraft]) -> Vec<Value> {
    rows.iter()
        .filter(|r| !r.key.text.trim().is_empty() || !r.label.text.trim().is_empty())
        .map(|r| {
            let mut obj = serde_json::Map::new();
            obj.insert("key".into(), json!(r.key.text.trim()));
            obj.insert("label".into(), json!(r.label.text.trim()));
            obj.insert("kind".into(), json!(r.kind));
            obj.insert("required".into(), json!(r.required));
            if r.kind == "choice" {
                let opts: Vec<String> = r
                    .options
                    .text
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                obj.insert("choices".into(), json!(opts));
            }
            let def = r.default_val.text.trim();
            if !def.is_empty() {
                obj.insert("default".into(), json!(def));
            }
            Value::Object(obj)
        })
        .collect()
}

fn inputs_editor(s: &State, p: &Palette) -> El {
    let mut box_el = col().w_full().gap(6.0);
    box_el = box_el.child(text("Inputs").font(type_::LABEL).color(p.secondary));
    for (i, row_draft) in s.routines.input_rows.iter().enumerate() {
        let next_kind = match row_draft.kind.as_str() {
            "text" => "long-text",
            "long-text" => "choice",
            "choice" => "folder",
            _ => "text",
        };
        let mut item = col()
            .w_full()
            .padding(8.0)
            .gap(6.0)
            .fill(p.raised)
            .radius(6.0)
            .child(
                row()
                    .gap(6.0)
                    .child(
                        text_input(&row_draft.key, type_::LABEL)
                            .key(format!("routine-input-key-{i}"))
                            .label("Key")
                            .grow(1.0)
                            .padding(6.0)
                            .fill(p.field)
                            .color(p.text)
                            .on_edit(move |s: &mut State| &mut s.routines.input_rows[i].key),
                    )
                    .child(
                        text_input(&row_draft.label, type_::LABEL)
                            .key(format!("routine-input-label-{i}"))
                            .label("Label")
                            .grow(1.0)
                            .padding(6.0)
                            .fill(p.field)
                            .color(p.text)
                            .on_edit(move |s: &mut State| &mut s.routines.input_rows[i].label),
                    )
                    .child(
                        button(&format!("Kind: {}", row_draft.kind), p)
                            .key(format!("routine-input-kind-{i}"))
                            .on_press(move |s: &mut State| {
                                s.routines.input_rows[i].kind = next_kind.to_owned();
                            }),
                    )
                    .child(
                        button(
                            &tick_box(row_draft.required, "Required"),
                            p,
                        )
                        .key(format!("routine-input-req-{i}"))
                        .on_press(move |s: &mut State| {
                            s.routines.input_rows[i].required = !s.routines.input_rows[i].required;
                        }),
                    )
                    .child(
                        button("Remove", p).on_press(move |s: &mut State| {
                            s.routines.input_rows.remove(i);
                        }),
                    ),
            )
            .child(
                text_input(&row_draft.default_val, type_::LABEL)
                    .key(format!("routine-input-default-{i}"))
                    .label("Default (optional)")
                    .w_full()
                    .padding(6.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(move |s: &mut State| &mut s.routines.input_rows[i].default_val),
            );
        if row_draft.kind == "choice" {
            item = item.child(
                text_input(&row_draft.options, type_::LABEL)
                    .key(format!("routine-input-options-{i}"))
                    .label("Choices (comma-separated)")
                    .w_full()
                    .padding(6.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(move |s: &mut State| &mut s.routines.input_rows[i].options),
            );
        }
        box_el = box_el.child(item);
    }
    if s.routines.input_rows.len() < locust_core::routine_inputs::MAX_INPUTS {
        box_el = box_el.child(
            button("Add input", p)
                .key("routine-add-input")
                .on_press(|s: &mut State| {
                    s.routines.input_rows.push(InputDraft {
                        key: field(""),
                        label: field(""),
                        kind: "text".into(),
                        required: true,
                        options: field(""),
                        default_val: field(""),
                    });
                }),
        );
    }
    box_el
}

fn run_dialog_view(dlg: &RunDialogState, p: &Palette) -> El {
    let mut box_el = col()
        .w_full()
        .padding(14.0)
        .gap(8.0)
        .fill(p.field)
        .radius(10.0)
        .child(
            row()
                .w_full()
                .child(
                    text(format!("Run \"{}\"", dlg.routine_name))
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Cancel", p).key("routine-run-cancel").on_press(|s: &mut State| {
                    s.routines.run_dialog = None;
                })),
        );
    if !dlg.error.is_empty() {
        box_el = box_el.child(text(dlg.error.clone()).font(type_::LABEL).color(p.red).wrap());
    }
    for (i, inp) in dlg.inputs.iter().enumerate() {
        let label = if inp.required {
            format!("{} (required)", inp.label)
        } else {
            inp.label.clone()
        };
        box_el = box_el.child(text(label).font(type_::LABEL).color(p.secondary));
        if inp.kind == locust_core::routine_inputs::RoutineInputKind::Folder {
            let current = dlg.values.get(i).map(|v| v.text.as_str()).unwrap_or("");
            let display = if current.is_empty() { "No folder chosen" } else { current };
            box_el = box_el.child(
                row()
                    .gap(8.0)
                    .child(text(display.to_owned()).font(type_::LABEL).color(p.text).grow(1.0))
                    .child(button("Choose folder", p).key(format!("routine-run-folder-{i}")).on_press_tx(move |_s: &mut State, tx: &mut Tx| {
                        let options = rune::FileDialogOptions { title: "Choose folder".into(), ..Default::default() };
                        tx.open_folder(options, move |s: &mut State, answer, _tx| {
                            let Ok(Some(path)) = answer else { return };
                            let path_str = path.to_string_lossy().into_owned();
                            s.routines.io.register_folder(&path_str);
                            if let Some(d) = &mut s.routines.run_dialog
                                && let Some(v) = d.values.get_mut(i)
                            {
                                *v = field(&path_str);
                            }
                        });
                    })),
            );
        } else if inp.kind == locust_core::routine_inputs::RoutineInputKind::Choice {
            let current = dlg.values.get(i).map(|v| v.text.as_str()).unwrap_or("");
            let mut choices = row().gap(6.0);
            for (j, opt) in inp.choices.as_deref().unwrap_or(&[]).iter().enumerate() {
                let opt_val = opt.clone();
                choices = choices.child(
                    button(opt, p)
                        .key(format!("routine-run-choice-{i}-{j}"))
                        .fill(if current == opt { p.selected } else { p.raised })
                        .on_press(move |s: &mut State| {
                            if let Some(d) = &mut s.routines.run_dialog
                                && let Some(v) = d.values.get_mut(i)
                            {
                                *v = field(&opt_val);
                            }
                        }),
                );
            }
            box_el = box_el.child(choices);
        } else if let Some(val_edit) = dlg.values.get(i) {
            box_el = box_el.child(
                text_input(val_edit, type_::BODY)
                    .key(format!("routine-run-input-{i}"))
                    .label(&inp.label)
                    .w_full()
                    .padding(8.0)
                    .fill(p.raised)
                    .color(p.text)
                    .on_edit(move |s: &mut State| {
                        &mut s.routines.run_dialog.as_mut().unwrap().values[i]
                    }),
            );
        }
    }
    box_el.child(
        row().gap(8.0).child(
            button("Run now", p)
                .key("routine-run-submit")
                .on_press(|s: &mut State| {
                    let Some(dlg) = s.routines.run_dialog.as_ref() else { return };
                    let mut given = serde_json::Map::new();
                    for (i, inp) in dlg.inputs.iter().enumerate() {
                        if let Some(v) = dlg.values.get(i) {
                            given.insert(inp.key.clone(), json!(v.text));
                        }
                    }
                    let routine = dlg.routine.clone();
                    let id = dlg.routine_id.clone();
                    let given_val = Value::Object(given);
                    match s
                        .routines
                        .io
                        .validate_run_inputs(&routine, Some(&given_val))
                    {
                        Ok(_) => {
                            let chosen = s.routines.io.chosen_folders();
                            s.routines.run_dialog = None;
                            background(s, move |e| {
                                let check = |p: &str| chosen.contains(p);
                                match e.runner.run_with_inputs(&id, None, Some(&given_val), Some(&check)) {
                                    Ok(Ok(_)) => Answer::Said(String::new()),
                                    Ok(Err(refused)) | Err(refused) => Answer::Refused(refused),
                                }
                            });
                        }
                        Err(err) => {
                            if let Some(d) = &mut s.routines.run_dialog {
                                d.error = err;
                            }
                        }
                    }
                }),
        ),
    )
}

fn templates_view(_s: &State, p: &Palette) -> El {
    let list = locust_core::routine_templates::list_shipped_templates();
    let mut box_el = col()
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .fill(p.field)
        .radius(10.0)
        .child(
            row()
                .w_full()
                .child(text("Starter templates").font(type_::TITLE).color(p.text).grow(1.0))
                .child(
                    text(locust_core::routine_templates::templates_line(&list))
                        .font(type_::LABEL)
                        .color(p.muted),
                )
                .child(button("Close", p).on_press(|s: &mut State| {
                    s.routines.templates_open = false;
                })),
        );
    for info in list {
        let tid = info.id.clone();
        let meta = locust_core::routine_templates::template_meta(&info);
        box_el = box_el.child(
            row()
                .w_full()
                .padding(8.0)
                .gap(8.0)
                .fill(p.raised)
                .radius(6.0)
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(text(info.name.clone()).font(type_::BODY).color(p.text))
                        .child(text(meta).font(type_::LABEL).color(p.muted)),
                )
                .child(
                    button("Use template", p)
                        .key(format!("routine-use-template-{}", info.id))
                        .on_press(move |s: &mut State| {
                            let ws = s
                                .home
                                .folder_path
                                .as_deref()
                                .map(std::path::Path::new)
                                .map(locust_core::people::workspace_id);
                            let teammate_id = s
                                .teammates
                                .get(s.selected)
                                .or_else(|| s.teammates.first())
                                .map(|t| t.id.clone())
                                .unwrap_or_default();
                            match s.routines.io.preview_template(&tid, ws.as_deref(), &[]) {
                                Ok(preview) => {
                                    s.routines.templates_open = false;
                                    s.routines.import_preview = Some(ImportPreviewState {
                                        preview,
                                        teammate_id,
                                        error: String::new(),
                                    });
                                }
                                Err(e) => s.routines.error = e,
                            }
                        }),
                ),
        );
    }
    box_el
}

fn import_preview_view(s: &State, imp: &ImportPreviewState, p: &Palette) -> El {
    let mut box_el = col()
        .w_full()
        .padding(14.0)
        .gap(8.0)
        .fill(p.field)
        .radius(10.0)
        .child(
            row()
                .w_full()
                .child(
                    text(format!("Import \"{}\"", imp.preview.name))
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Cancel", p).on_press(|s: &mut State| {
                    s.routines.import_preview = None;
                })),
        )
        .child(
            text(format!(
                "{} {} · arrives in Ask mode without a schedule",
                imp.preview.steps.len(),
                if imp.preview.steps.len() == 1 { "step" } else { "steps" }
            ))
            .font(type_::LABEL)
            .color(p.muted),
        );
    let missing_connectors: Vec<&str> = imp
        .preview
        .connectors
        .iter()
        .filter(|c| !c.present)
        .map(|c| c.name.as_str())
        .collect();
    if !missing_connectors.is_empty() {
        box_el = box_el.child(
            text(format!(
                "Needs connectors on this computer: {}",
                missing_connectors.join(", ")
            ))
            .font(type_::LABEL)
            .color(p.amber)
            .wrap(),
        );
    }
    let roles: Vec<&str> = imp
        .preview
        .hand_off_roles
        .iter()
        .filter_map(|r| r.as_deref())
        .collect();
    if !roles.is_empty() {
        box_el = box_el.child(
            text(format!("Step roles: {}", roles.join(", ")))
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        );
    }
    if !imp.error.is_empty() {
        box_el = box_el.child(text(imp.error.clone()).font(type_::LABEL).color(p.red).wrap());
    }
    let owner_name = s
        .teammates
        .iter()
        .find(|t| t.id == imp.teammate_id)
        .map(|t| t.name.as_str())
        .unwrap_or("Selected teammate");
    box_el.child(
        row()
            .gap(8.0)
            .child(text(format!("Owner: {owner_name}")).font(type_::LABEL).color(p.secondary))
            .child(
                button("Import routine", p)
                    .key("routine-import-confirm")
                    .on_press(|s: &mut State| {
                        let Some(store) = s.data.clone() else { return };
                        let Some(imp) = s.routines.import_preview.as_ref() else { return };
                        let token = imp.preview.token.clone();
                        let teammate_id = imp.teammate_id.clone();
                        let ws = s
                            .home
                            .folder_path
                            .as_deref()
                            .map(std::path::Path::new)
                            .map(locust_core::people::workspace_id);
                        match s
                            .routines
                            .io
                            .import_preview(&store, &token, &teammate_id, ws.as_deref())
                        {
                            Ok(saved) => {
                                s.routines.import_preview = None;
                                s.routines.error.clear();
                                s.routines.said = format!(
                                    "Imported \"{}\".",
                                    saved["name"].as_str().unwrap_or("Routine")
                                );
                                reload(s);
                            }
                            Err(e) => {
                                if let Some(cur) = &mut s.routines.import_preview {
                                    cur.error = e;
                                }
                            }
                        }
                    }),
            ),
    )
}

fn export_dialog_view(exp: &ExportDialogState, p: &Palette) -> El {
    col()
        .w_full()
        .padding(14.0)
        .gap(8.0)
        .fill(p.field)
        .radius(10.0)
        .child(
            row()
                .w_full()
                .child(text("This routine names folders from this computer").font(type_::TITLE).color(p.text).grow(1.0))
                .child(button("Cancel", p).on_press(|s: &mut State| {
                    s.routines.export_dialog = None;
                })),
        )
        .child(
            text(format!("Found: {}", exp.paths.join(", ")))
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        )
        .child(
            row()
                .gap(8.0)
                .child(
                    button("Turn paths into inputs", p)
                        .key("routine-export-as-inputs")
                        .on_press_tx(|s: &mut State, tx: &mut Tx| {
                            if let Some(exp) = s.routines.export_dialog.take() {
                                export_with_choice(s, &exp.routine, Some(locust_core::routine_io::ExportPaths::Input), tx);
                            }
                        }),
                )
                .child(
                    button("Keep paths as written", p)
                        .key("routine-export-keep-paths")
                        .on_press_tx(|s: &mut State, tx: &mut Tx| {
                            if let Some(exp) = s.routines.export_dialog.take() {
                                export_with_choice(s, &exp.routine, Some(locust_core::routine_io::ExportPaths::Keep), tx);
                            }
                        }),
                ),
        )
}

fn open_routine_file(tx: &mut Tx) {
    let options = rune::FileDialogOptions {
        title: "Open routine file".into(),
        ..Default::default()
    }
    .filter("Routine files", &["md", "markdown", "json"]);
    tx.open_file(options, |s: &mut State, answer, _tx| {
        let Ok(Some(path)) = answer else { return };
        let ws = s
            .home
            .folder_path
            .as_deref()
            .map(std::path::Path::new)
            .map(locust_core::people::workspace_id);
        let teammate_id = s
            .teammates
            .get(s.selected)
            .or_else(|| s.teammates.first())
            .map(|t| t.id.clone())
            .unwrap_or_default();
        match s.routines.io.preview_file_path(&path, ws.as_deref(), &[]) {
            Ok(preview) => {
                s.routines.error.clear();
                s.routines.import_preview = Some(ImportPreviewState {
                    preview,
                    teammate_id,
                    error: String::new(),
                });
            }
            Err(e) => s.routines.error = e,
        }
    });
}

fn begin_export_routine(s: &mut State, routine: Value, tx: &mut Tx) {
    export_with_choice(s, &routine, None, tx);
}

fn export_with_choice(
    s: &mut State,
    routine: &Value,
    paths: Option<locust_core::routine_io::ExportPaths>,
    tx: &mut Tx,
) {
    let team_vals: Vec<Value> = s
        .data
        .as_ref()
        .and_then(|store| locust_core::people::team(store).ok())
        .and_then(|t| t["teammates"].as_array().cloned())
        .unwrap_or_default();
    match s.routines.io.prepare_export(routine, paths, &team_vals, &[]) {
        Ok(locust_core::routine_io::ExportOutcome::AbsolutePaths(flagged)) => {
            let path_strs: Vec<String> = flagged.into_iter().map(|f| f.path).collect();
            s.routines.export_dialog = Some(ExportDialogState {
                routine: routine.clone(),
                paths: path_strs,
            });
        }
        Ok(locust_core::routine_io::ExportOutcome::Ready {
            text,
            markdown: _,
            file_name,
        }) => {
            save_routine_file_dialog(text, file_name, tx);
        }
        Err(e) => s.routines.error = e,
    }
}

fn save_routine_file_dialog(content: String, suggested: String, tx: &mut Tx) {
    let options = rune::FileDialogOptions {
        title: "Save routine as file".into(),
        file_name: Some(suggested),
        ..Default::default()
    }
    .filter("Markdown or JSON", &["md", "json"]);
    tx.save_file(options, move |s: &mut State, answer, _tx| {
        let Ok(Some(path)) = answer else { return };
        match std::fs::write(&path, &content) {
            Ok(()) => {
                s.routines.error.clear();
                s.routines.said = format!("Saved routine to {}.", path.display());
            }
            Err(e) => s.routines.error = e.to_string(),
        }
    });
}
