//! Settings › Connectors › "Add a connector" (Electron `AddConnector.tsx`, 0.716; core/src/connector_add.rs says why): a
//! name, how it runs (a command on this computer or a web address) and which agents get it. Locust runs each agent's
//! own `mcp add` and says what each answered: added, had one by that name already (kept as it was), or why not. Undo,
//! right after, runs each one's own `mcp remove`. Only agents installed here are offered, and all of them are chosen
//! until one is turned off.

use super::*;
use locust_core::connector_add::{self, AgentResult, Kind, Launch, Outcome};

/// What came back from the agents, off the window's thread.
pub(super) enum Answer {
    Added(String, Result<Vec<AgentResult>, String>),
    Removed(Vec<AgentResult>),
}

/// A row of what came back; `undo` marks one that Undo's answer replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub result: AgentResult,
    pub undo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Busy {
    Adding,
    Undoing,
}

pub(super) struct Form {
    pub open: bool,
    pub name: TextEdit,
    pub kind: Kind,
    pub value: TextEdit,
    /// Turned OFF, not on: an agent whose check lands after this opened is chosen too.
    pub left: Vec<String>,
    pub busy: Option<Busy>,
    pub problem: Option<String>,
    pub results: Option<(String, Vec<Row>)>,
    /// The agents installed here that can take one, once looked for.
    pub installed: Option<Vec<String>>,
    pub job: Latest<Answer>,
    pub installed_job: Latest<Vec<String>>,
}

impl Default for Form {
    fn default() -> Self {
        Form {
            open: false,
            name: TextEdit::new(""),
            kind: Kind::Command,
            value: TextEdit::new(""),
            left: Vec::new(),
            busy: None,
            problem: None,
            results: None,
            installed: None,
            job: Latest::default(),
            installed_job: Latest::default(),
        }
    }
}

impl Form {
    pub(super) fn set_waker(&self, waker: &rune::Waker) {
        self.job.set_waker(waker.clone());
        self.installed_job.set_waker(waker.clone());
    }
}

/// The agents and the runner the form uses: the real CLIs, or a test's own.
pub(super) trait Agents: Send + Sync {
    fn launch(&self, agent: &str) -> Option<Launch>;
    fn runner(&self) -> &dyn connector_add::Runner;
    fn installed(&self) -> Vec<String> {
        connector_add::AGENTS.iter().filter(|agent| self.launch(agent).is_some()).map(|a| (*a).to_owned()).collect()
    }
}

#[cfg_attr(test, allow(dead_code))]
struct Real;

impl Agents for Real {
    fn launch(&self, agent: &str) -> Option<Launch> {
        connector_add::real_launch(agent)
    }
    fn runner(&self) -> &dyn connector_add::Runner {
        &connector_add::RealRunner
    }
}

static AGENTS_IN_USE: std::sync::Mutex<Option<std::sync::Arc<dyn Agents>>> = std::sync::Mutex::new(None);

/// Tests answer for the agents themselves; without that, a test never runs a real CLI (it finds none installed).
#[cfg(test)]
pub(super) fn use_agents(agents: std::sync::Arc<dyn Agents>) {
    *AGENTS_IN_USE.lock().unwrap_or_else(|e| e.into_inner()) = Some(agents);
}

fn agents() -> Option<std::sync::Arc<dyn Agents>> {
    let held = AGENTS_IN_USE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    #[cfg(test)]
    return held;
    #[cfg(not(test))]
    Some(held.unwrap_or_else(|| std::sync::Arc::new(Real)))
}

/// Look for the installed agents off the window's thread, once.
pub(super) fn request_installed(s: &mut State) {
    let form = &mut s.connectors.add;
    if form.installed.is_some() || form.installed_job.is_pending() {
        return;
    }
    let Some(agents) = agents() else { return };
    form.installed_job.request((), move |()| agents.installed());
}

pub(super) fn poll(s: &mut State) -> bool {
    let form = &mut s.connectors.add;
    let mut changed = false;
    if let Some(found) = form.installed_job.take() {
        form.installed = Some(found);
        changed = true;
    }
    if let Some(answer) = form.job.take() {
        changed = true;
        form.busy = None;
        match answer {
            Answer::Added(_, Err(message)) => form.problem = Some(message),
            Answer::Added(name, Ok(results)) => {
                // All of it done: the form goes, and what each said stays. Anything not done keeps the form as
                // typed, to change and try again.
                if results.iter().all(|one| matches!(one.outcome, Outcome::Added | Outcome::Had)) {
                    form.open = false;
                    form.name = TextEdit::new("");
                    form.value = TextEdit::new("");
                }
                let added = results.iter().any(|one| one.outcome == Outcome::Added);
                form.results = Some((name, results.into_iter().map(|result| Row { result, undo: false }).collect()));
                if added {
                    crate::connectors_section::request(s, true);
                }
            }
            Answer::Removed(back) => {
                // What came back replaces the added rows; the rest stay as they were said.
                if let Some((_, rows)) = form.results.as_mut() {
                    for row in rows.iter_mut() {
                        if let Some(now) = back.iter().find(|one| one.agent == row.result.agent) {
                            *row = Row { result: now.clone(), undo: true };
                        }
                    }
                }
                crate::connectors_section::request(s, true);
            }
        }
    }
    changed
}

fn offered(form: &Form) -> Vec<String> {
    let installed = form.installed.clone().unwrap_or_default();
    connector_add::AGENTS.iter().filter(|agent| installed.iter().any(|one| one == *agent)).map(|a| (*a).to_owned()).collect()
}

fn picked(form: &Form) -> Vec<String> {
    offered(form).into_iter().filter(|agent| !form.left.contains(agent)).collect()
}

fn add(s: &mut State) {
    let form = &mut s.connectors.add;
    if form.busy.is_some() {
        return;
    }
    let (name, kind, value) = (form.name.text.clone(), form.kind, form.value.text.clone());
    let connector = match connector_add::connector_from(&name, kind, &value) {
        Ok(connector) => connector,
        Err(problem) => {
            form.problem = Some(problem);
            return;
        }
    };
    let chosen = picked(form);
    if chosen.is_empty() {
        form.problem = Some("Choose at least one agent.".into());
        return;
    }
    let Some(agents) = agents() else { return };
    form.problem = None;
    form.busy = Some(Busy::Adding);
    form.job.request((), move |()| {
        let launch = |agent: &str| agents.launch(agent);
        Answer::Added(connector.name.clone(), connector_add::add(&name, kind, &value, &chosen, &launch, agents.runner()))
    });
}

fn undo(s: &mut State) {
    let form = &mut s.connectors.add;
    let Some((name, rows)) = form.results.clone() else { return };
    let given: Vec<String> = rows.iter().filter(|row| row.result.outcome == Outcome::Added).map(|row| row.result.agent.clone()).collect();
    if given.is_empty() || form.busy.is_some() {
        return;
    }
    let Some(agents) = agents() else { return };
    form.problem = None;
    form.busy = Some(Busy::Undoing);
    form.job.request((), move |()| {
        let launch = |agent: &str| agents.launch(agent);
        Answer::Removed(connector_add::remove(&name, &given, &launch, agents.runner()))
    });
}

/// The words a result row draws: its state, its tone and what to say under it.
pub(super) fn row_words(row: &Row) -> (&'static str, &'static str, Option<String>) {
    let (state, tone, detail) = match row.result.outcome {
        Outcome::Added => ("Added", "good", None),
        Outcome::Had => ("Had one", "muted", Some("It already has a connector by this name, and keeps its own.")),
        Outcome::Failed => ("Not added", "amber", None),
        Outcome::Removed => ("Taken back", "muted", None),
        Outcome::Kept => ("Still has it", "amber", None),
    };
    let state = if row.result.outcome == Outcome::Failed && row.undo { "Not taken back" } else { state };
    (state, tone, row.result.said.clone().or(detail.map(str::to_owned)))
}

/// `.lc-fieldlabel`: the plain label over a field, not a section's spaced capitals.
fn field_label(l: &crate::look::controls::Look, words: &str) -> El {
    text(words.to_owned()).font(type_::LABEL).color(l.c("text-secondary"))
}

fn hint(l: &crate::look::controls::Look, words: &str) -> El {
    text(words.to_owned()).font(l.ty.body).color(l.c("text-muted")).wrap()
}

fn input(l: &crate::look::controls::Look, edit: &TextEdit, key: &str, label: &str, placeholder: &str) -> El {
    text_input(edit, l.ty.control.mono())
        .key(key.to_owned())
        .label(label.to_owned())
        .w_full()
        .color(l.c("text-primary"))
        .placeholder(placeholder.to_owned(), l.c("text-faint"))
        .fill(l.c("bg-raised"))
        .padding(8.0)
        .radius(l.t.px("radius-button"))
}

fn tone_color(p: &Palette, tone: &str) -> Color {
    match tone {
        "good" => p.green,
        "amber" => p.amber,
        _ => p.muted,
    }
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    use crate::look::controls::{Tone, button, primary_button};
    let l = crate::look::controls::Look::new(p.dark);
    let form = &s.connectors.add;
    let offered = offered(form);
    let mut body = col().w_full().gap(10.0).key("add-connector");
    if !form.open {
        let none = form.installed.as_ref().is_some_and(|_| offered.is_empty());
        let label = if form.results.is_none() { "Add a connector…" } else { "Add another…" };
        let disabled = offered.is_empty() || form.busy.is_some();
        let mut start = row().gap(10.0).center_y().child(
            button(&l, label, Tone::Neutral, disabled).key("add-connector-start").on_press(move |s: &mut State| {
                if disabled {
                    return;
                }
                s.connectors.add.problem = None;
                s.connectors.add.open = true;
            }),
        );
        if none {
            start = start.child(hint(&l, "None of the agents that can take one is installed here.").key("add-connector-none"));
        }
        body = body.child(start);
    } else {
        let picked = picked(form);
        let busy = form.busy.is_some();
        let segment = |label: &str, kind: Kind, key: &str| {
            button(&l, label, if form.kind == kind { Tone::Active } else { Tone::Neutral }, false)
                .key(key.to_owned())
                .role("radio")
                .on_press(move |s: &mut State| s.connectors.add.kind = kind)
        };
        let mut agents_row = row().gap(6.0).flex_wrap();
        for agent in &offered {
            let on = !form.left.contains(agent);
            let name = locust_core::inspector::runtime_display_name(agent).to_owned();
            let mark = crate::look::controls::agent_mark(&l, agent, 13.0, if on { l.c("text-primary") } else { l.c("text-faint") });
            let toggle = agent.clone();
            agents_row = agents_row.child(
                row()
                    .gap(6.0)
                    .center_y()
                    .h(28.0)
                    .px(10.0)
                    .radius(999.0)
                    .border(1.0, if on { l.c("border-control") } else { l.c("border-hairline") })
                    .fill(if on { l.c("bg-control") } else { Color::TRANSPARENT })
                    .children(mark)
                    .child(text(name.clone()).font(l.ty.body).color(if on { l.c("text-primary") } else { l.c("text-muted") }))
                    .opacity(if on { 1.0 } else { 0.55 })
                    .key(format!("add-connector-agent-{agent}"))
                    .role("button")
                    .label(format!("{name}{}", if on { ", chosen" } else { "" }))
                    .on_press(move |s: &mut State| {
                        let left = &mut s.connectors.add.left;
                        if let Some(at) = left.iter().position(|one| one == &toggle) {
                            left.remove(at);
                        } else {
                            left.push(toggle.clone());
                        }
                    }),
            );
        }
        let count = picked.len();
        let add_label = if form.busy == Some(Busy::Adding) {
            "Adding…".to_owned()
        } else {
            format!("Add to {count} {}", if count == 1 { "agent" } else { "agents" })
        };
        let command = form.kind == Kind::Command;
        body = body.child(
            col()
                .w_full()
                .gap(12.0)
                .key("add-connector-form")
                .label("Add a connector")
                .child(col().gap(6.0).child(field_label(&l, "Name")).child(
                    input(&l, &form.name, "add-connector-name", "Name", "rea").on_edit(|s: &mut State| &mut s.connectors.add.name),
                ))
                .child(
                    col().gap(6.0).child(field_label(&l, "It runs as")).child(
                        row()
                            .gap(6.0)
                            .child(segment("A command", Kind::Command, "add-connector-command"))
                            .child(segment("A web address", Kind::Url, "add-connector-url")),
                    ),
                )
                .child(
                    col()
                        .gap(6.0)
                        .child(field_label(&l, if command { "Command" } else { "Address" }))
                        .child(
                            input(
                                &l,
                                &form.value,
                                "add-connector-value",
                                if command { "Command" } else { "Address" },
                                if command { "npx -y rea-agents@6.3.0 mcp" } else { "https://example.com/mcp" },
                            )
                            .on_edit(|s: &mut State| &mut s.connectors.add.value),
                        )
                        .child(hint(
                            &l,
                            if command {
                                "The command that starts it, as its instructions give it. Each agent starts it itself, on this computer."
                            } else {
                                "Where it answers, starting https://. One that needs a key or a sign-in is added in the agent itself."
                            },
                        )),
                )
                .child(
                    col()
                        .gap(6.0)
                        .child(field_label(&l, "Give it to"))
                        .child(agents_row)
                        .child(hint(&l, "Cursor Agent has no command for this yet: add it in Cursor.")),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(button(&l, "Cancel", Tone::Neutral, busy).key("add-connector-cancel").on_press(|s: &mut State| {
                            if s.connectors.add.busy.is_none() {
                                s.connectors.add.problem = None;
                                s.connectors.add.open = false;
                            }
                        }))
                        .child(primary_button(&l, &add_label, busy || count == 0).key("add-connector-add").on_press(add)),
                ),
        );
    }
    if let Some(problem) = &form.problem {
        body = body.child(text(problem.clone()).font(l.ty.body).color(p.amber).wrap().key("add-connector-problem").role("alert"));
    }
    if let Some((name, rows)) = &form.results {
        let mut list = col().w_full().gap(0.0).radius(8.0).border(1.0, p.hairline);
        for one in rows {
            let (state, tone, detail) = row_words(one);
            let agent = &one.result.agent;
            let who = locust_core::inspector::runtime_display_name(agent).to_owned();
            let mut words = col().gap(2.0).grow(1.0).shrink().child(
                row()
                    .gap(6.0)
                    .center_y()
                    .children(crate::look::controls::agent_mark(&l, agent, 13.0, p.text))
                    .child(text(who).font(type_::LABEL).color(p.text)),
            );
            if let Some(detail) = detail {
                words = words.child(text(detail).font(type_::BODY).color(p.secondary).wrap());
            }
            list = list.child(
                row()
                    .w_full()
                    .gap(10.0)
                    .padding(10.0)
                    .key(format!("add-connector-result-{agent}"))
                    .label(state)
                    .child(text(state.to_uppercase()).font(type_::LABEL).color(tone_color(p, tone)).px(8.0).py(2.0).radius(999.0).border(1.0, p.hairline).self_start())
                    .child(words),
            );
        }
        let mut results = col()
            .w_full()
            .gap(8.0)
            .key("add-connector-results")
            .role("status")
            .label(format!("What each agent said about {name}"))
            .child(field_label(&l, name))
            .child(list);
        let added = rows.iter().filter(|row| row.result.outcome == Outcome::Added).count();
        if added > 0 {
            let undoing = form.busy == Some(Busy::Undoing);
            results = results.child(
                row()
                    .gap(10.0)
                    .center_y()
                    .child(hint(&l, if added == 1 { "That agent has it from their next turn." } else { "Those agents have it from their next turn." }))
                    .child(
                        button(&l, if undoing { "Taking it back…" } else { "Undo" }, Tone::Neutral, form.busy.is_some())
                            .key("add-connector-undo")
                            .on_press(undo),
                    ),
            );
        }
        body = body.child(results);
    }
    body
}

/// Settings › Connectors with the form open and filled in, three agents installed and one turned off.
pub(super) fn scene(s: &mut State) {
    crate::settings_panel::scene(s);
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Connectors);
    s.connectors.connectors = Some(Vec::new());
    let form = &mut s.connectors.add;
    form.installed = Some(vec!["claude".into(), "codex".into(), "gemini".into()]);
    form.open = true;
    form.name = TextEdit::new("rea");
    form.value = TextEdit::new("npx -y rea-agents@6.3.0 mcp");
    form.left = vec!["gemini".into()];
}

/// What each agent said after an add, with Undo.
pub(super) fn added_scene(s: &mut State) {
    scene(s);
    let form = &mut s.connectors.add;
    form.open = false;
    let one = |agent: &str, outcome, said: Option<&str>| Row { result: AgentResult { agent: agent.into(), outcome, said: said.map(str::to_owned) }, undo: false };
    form.results = Some((
        "rea".into(),
        vec![
            one("claude", Outcome::Added, None),
            one("codex", Outcome::Had, None),
            one("gemini", Outcome::Failed, Some("It did not say whether it already has one named rea, so nothing was changed.")),
        ],
    ));
}

#[cfg(test)]
#[path = "add_connector_tests.rs"]
mod add_connector_tests;
