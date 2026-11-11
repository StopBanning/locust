use super::*;
use serde_json::{Value, json};
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
    s.routines.open = true;
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
        .map(str::to_owned)
        .unwrap_or_else(|| r["schedule"]["hours"].as_u64().unwrap_or(4).to_string());
    s.routines.fields = vec![field(r["name"].as_str().unwrap_or("")), field(&parameter)];
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
        )
        .child(
            text("Scheduled execution and automatic continuation are not available in native yet.")
                .font(type_::LABEL)
                .color(p.amber)
                .wrap(),
        );
    let owner = s.teammates.get(s.selected).map(|t| t.id.clone());
    if let Some(owner) = owner {
        body = body.child(button("New routine", p).on_press(move |s: &mut State| {
            edit(s, json!({"name":"","steps":[""],"teammateId":owner}))
        }));
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
            if panel.steps.len() > 1 {
                body = body.child(button("Remove step", p).on_press(move |s: &mut State| {
                    s.routines.steps.remove(i);
                }));
            }
        }
        if panel.steps.len() < 12 {
            body = body.child(
                button("Add a step", p).on_press(|s: &mut State| s.routines.steps.push(field(""))),
            );
        }
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
        ] {
            let selected =
                panel.schedule["kind"] == kind || (kind == "off" && panel.schedule.is_null());
            body=body.child(button(label,p).fill(if selected{p.selected}else{p.raised}).on_press(move|s:&mut State|{
                if s.routines.schedule["kind"]==kind {return;}
                s.routines.schedule=match kind {"every"=>json!({"kind":"every","hours":4}),"daily"=>json!({"kind":"daily","at":"09:00"}),"weekly"=>json!({"kind":"weekly","days":[1,2,3,4,5],"at":"09:00"}),"once"=>json!({"kind":"once","on":(chrono::Local::now()+chrono::Duration::days(1)).format("%Y-%m-%dT09:00").to_string()}),_=>Value::Null};
                let value=if kind=="every"{"4".into()}else if kind=="once"{s.routines.schedule["on"].as_str().unwrap_or("").to_owned()}else{"09:00".into()};s.routines.fields[1]=field(&value);
            }));
        }
        let kind = panel.schedule["kind"].as_str().unwrap_or("");
        if ["every", "daily", "weekly", "once"].contains(&kind) {
            let label = if kind == "every" {
                "Hours between runs"
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
                        _ => {}
                    }
                    entry["schedule"] = schedule;
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
            let remove = id.clone();
            let pause = id.clone();
            let run = id.clone();
            let paused = r["paused"] == true;
            card = card.child(
                row()
                    .gap(10.0)
                    .child(button("Edit", p).on_press(move |s: &mut State| edit(s, entry.clone())))
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
                    .child(button("Run", p).on_press(move |s: &mut State| {
                        let r = s
                            .data
                            .as_ref()
                            .ok_or("No native data folder".into())
                            .and_then(|store| crate::routines::run(store, &run));
                        result(s, r);
                    })),
            );
            if crate::routines::awaits_review(r) {
                card = card.child(
                    text(
                        r["execution"]["reason"]
                            .as_str()
                            .unwrap_or("This routine is waiting for your review.")
                            .to_owned(),
                    )
                    .font(type_::LABEL)
                    .color(p.amber)
                    .wrap(),
                );
                let attempt = r["execution"]["attemptId"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                if let Some(mission)=r["execution"]["missionId"].as_str() {
                    let mission=mission.to_owned();
                    card=card.child(button("Open conversation",p).on_press_tx(move|s:&mut State,tx|{
                        if let Some(row)=s.recent.iter().find(|row|row.id==mission).cloned() { s.routines.open=false;open_recent(s,row,tx); }
                        else {s.routines.error="The conversation is not in Recent yet. Reload the team, then open Automations again.".into();}
                    }));
                }
                let keep_label = if r.get("schedule").is_some_and(|s| !s.is_null()) {
                    "Keep the schedule"
                } else {
                    "Keep the routine"
                };
                let next_keep = {
                    let now = chrono::Local::now();
                    match r.get("schedule").filter(|s| !s.is_null()) {
                        None => "when you press Run".to_owned(),
                        Some(sched) => match crate::routines::schedule::next(sched, now, now) {
                            Some(at) => format!(
                                "at {}, not straight away",
                                at.format("%a %-I:%M %p")
                            ),
                            None if sched["kind"] == "files" => {
                                "when the next new file arrives".into()
                            }
                            None => "when you press Run, since its one scheduled time has passed"
                                .into(),
                        },
                    }
                };
                card = card.child(
                    text(format!(
                        "Keep clears this attempt and leaves the routine as it was; its next run starts from step 1 {next_keep}. Abandon also removes the schedule. Neither stops a runtime or undoes work."
                    ))
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .wrap(),
                );
                let reviewed = panel.reviewed.as_deref() == Some(attempt.as_str());
                let review = attempt.clone();
                card = card.child(
                    button(
                        if reviewed {
                            "✓ I reviewed the saved conversation and external work, and whether the remaining work is still wanted."
                        } else {
                            "□ I reviewed the saved conversation and external work, and whether the remaining work is still wanted."
                        },
                        p,
                    )
                    .wrap()
                    .on_press(move |s: &mut State| {
                        s.routines.reviewed = if reviewed {
                            None
                        } else {
                            Some(review.clone())
                        }
                    }),
                );
                if reviewed {
                    let keep_id = id.clone();
                    let keep_attempt = attempt.clone();
                    card = card.child(button(keep_label, p).on_press(move |s: &mut State| {
                        let at = chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                        let r = s.data.as_ref().ok_or("No native data folder".into()).and_then(
                            |store| crate::routines::keep(store, &keep_id, &keep_attempt, &at),
                        );
                        if r.is_ok() {
                            s.routines.reviewed = None;
                        }
                        result(s, r);
                    }));
                    card = card.child(
                        button("Abandon attempt and remove schedule", p).on_press(
                            move |s: &mut State| {
                                let r = s
                                    .data
                                    .as_ref()
                                    .ok_or("No native data folder".into())
                                    .and_then(|store| {
                                        crate::routines::acknowledge(store, &id, &attempt)
                                    });
                                if r.is_ok() {
                                    s.routines.reviewed = None;
                                }
                                result(s, r);
                            },
                        ),
                    );
                }
            }
            body = body.child(card);
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

pub(super) fn poll(s: &mut State) -> bool {
    if s.routines.initialized {
        return false;
    }
    if let Some(store) = &s.data {
        s.routines.initialized = true;
        if let Err(e) = crate::routines::record_misses(store, s.routines.opened_at) {
            s.routines.error = e;
        }
    }
    false
}
