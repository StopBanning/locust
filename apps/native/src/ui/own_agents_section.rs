//! Settings › AI agents › Your own agents (Electron `Screens.tsx` `data-setting="acp-agents"` and
//! `AcpAgentsSettings.tsx`, 0.777): any program that speaks the Agent Client Protocol, added by the command that starts
//! it. Each saved agent is listed with its command, the name and version it answered with and how many models it
//! offers, and a Remove that asks on the first press. Test opens a session with it and closes it again -- no prompt, so
//! nothing is spent -- and says what it answered; Add agent tests first and keeps only an agent that answered.
//!
//! The engine is `locust_core::acp_agents::AcpAgentsStore` (the window's data folder, reading Electron's list until it
//! has its own). Test and Add run off the window's thread: a first start can take half a minute.

use super::*;
use locust_core::acp_agents::{AcpAgent, AcpAgentAnswer, AcpAgentsStore};

pub(crate) const HEADING: &str = "Your own agents";
const LEDE: &str = "Any agent that speaks the Agent Client Protocol, added by the command that starts it. It joins every teammate\u{2019}s model list, in a group of its own.";
/// `ACP_AGENT_TESTING`.
pub(crate) const TESTING: &str = "Testing\u{2026} An agent that is starting for the first time can take half a minute.";
/// ArmedButton's `SETTLE_MS`: the second click of a double-click is not a second decision.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(400);

/// What a Test or an Add came back with, off the window's thread.
pub(crate) enum Done {
    Tested(Result<AcpAgentAnswer, String>),
    Added { name: String, saved: Result<Vec<AcpAgent>, String> },
}

#[derive(Default)]
pub(crate) struct Card {
    /// The saved agents, read when the page is shown; None until then.
    agents: Option<Vec<AcpAgent>>,
    name: TextEdit,
    command_line: TextEdit,
    busy: bool,
    /// What the last Test or Add said, and whether it went well.
    said: Option<(String, bool)>,
    /// The agent whose Remove was pressed once, and when.
    armed: Option<(String, std::time::Instant)>,
    job: Latest<Done>,
}

fn store(s: &State) -> Option<AcpAgentsStore> {
    s.data.as_ref().map(AcpAgentsStore::for_store)
}

/// `acpAnswerSentence`: what a Test found, in one sentence.
pub(crate) fn answer_sentence(answer: &AcpAgentAnswer) -> String {
    let who = [answer.agent_name.as_deref(), answer.version.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let models = answer.model_option.as_ref().map_or(0, |o| o.models.len());
    let offers = if models == 0 {
        "It offers no models to pick from, so it runs on its own default.".to_owned()
    } else {
        format!("It offers {models} model{} to pick from.", if models == 1 { "" } else { "s" })
    };
    format!("{} {offers}", if who.is_empty() { "It answered.".to_owned() } else { format!("{who} answered.") })
}

/// The row's second line: the command, who answered, and how many models (`lc-ownmodel__meta`).
fn meta(agent: &AcpAgent) -> String {
    let who = agent.answer.as_ref().and_then(|a| {
        a.agent_name.as_ref().map(|name| match &a.version {
            Some(version) => format!(" \u{b7} {name} {version}"),
            None => format!(" \u{b7} {name}"),
        })
    });
    let models = agent.answer.as_ref().and_then(|a| a.model_option.as_ref()).map_or(0, |o| o.models.len());
    format!("{}{} \u{b7} {models} models", agent.command_line, who.unwrap_or_default())
}

/// Reads the list once the page is up, takes a Test's or an Add's answer, and forgets the list when Settings closes so
/// it is read again next time (the component's `useEffect(load)`).
pub(crate) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some(done) = s.own_agents.job.take() {
        s.own_agents.busy = false;
        match done {
            Done::Tested(Ok(answer)) => s.own_agents.said = Some((answer_sentence(&answer), true)),
            Done::Tested(Err(why)) => s.own_agents.said = Some((why, false)),
            Done::Added { saved: Err(why), .. } => s.own_agents.said = Some((why, false)),
            Done::Added { name, saved: Ok(agents) } => {
                s.own_agents.said = Some((format!("{name} is in every teammate's model list, in a group of its own. It runs in Approve each."), true));
                s.own_agents.name = TextEdit::default();
                s.own_agents.command_line = TextEdit::default();
                s.own_agents.agents = Some(agents);
            }
        }
        changed = true;
    }
    let shown = s.settings_panel.open && s.settings_panel.page == Some(crate::settings_pages::SettingsPageId::Runtimes);
    if shown && s.own_agents.agents.is_none() {
        if let Some(agents) = store(s).and_then(|store| store.list().ok()) {
            s.own_agents.agents = Some(agents);
            changed = true;
        }
    } else if !s.settings_panel.open && s.own_agents.agents.is_some() {
        s.own_agents.agents = None;
    }
    changed
}

fn run(s: &mut State, work: impl FnOnce(AcpAgentsStore) -> Done + Send + 'static) {
    let Some(store) = store(s) else { return };
    s.own_agents.busy = true;
    s.own_agents.said = Some((TESTING.to_owned(), true));
    if let Some(waker) = s.pet_faces.waker.borrow().clone() {
        s.own_agents.job.set_waker(waker);
    }
    s.own_agents.job.request(store, work);
}

fn test(s: &mut State) {
    let command_line = s.own_agents.command_line.text.trim().to_owned();
    if command_line.is_empty() || s.own_agents.busy {
        return;
    }
    run(s, move |store| Done::Tested(store.test(&command_line)));
}

fn add(s: &mut State) {
    let (name, command_line) = (s.own_agents.name.text.trim().to_owned(), s.own_agents.command_line.text.trim().to_owned());
    if name.is_empty() || command_line.is_empty() || s.own_agents.busy {
        return;
    }
    run(s, move |store| {
        let saved = store.save(&name, &command_line);
        Done::Added { name, saved }
    });
}

/// ArmedButton: the first press arms it and says what the second will do; the second, a settle later, removes.
fn remove(s: &mut State, id: &str) {
    match &s.own_agents.armed {
        Some((armed, at)) if armed == id => {
            if at.elapsed() < SETTLE {
                return;
            }
            s.own_agents.armed = None;
            if let Some(store) = store(s) {
                match store.remove(id) {
                    Ok(agents) => s.own_agents.agents = Some(agents),
                    Err(why) => s.own_agents.said = Some((why, false)),
                }
            }
        }
        _ => s.own_agents.armed = Some((id.to_owned(), std::time::Instant::now())),
    }
}

fn field(l: &crate::look::controls::Look, edit: &TextEdit, key: &str, label: &str, placeholder: &str, mono: bool, pick: fn(&mut State) -> &mut TextEdit) -> El {
    col()
        .gap(4.0)
        .grow(1.0)
        .child(text(label.to_owned()).font(type_::LABEL).color(l.c("text-muted")))
        .child(
            text_input(edit, if mono { l.ty.mono } else { l.ty.body })
                .key(key.to_owned())
                .single_line()
                .label(label.to_owned())
                .placeholder(placeholder, l.c("text-muted"))
                .w_full()
                .h(32.0)
                .padding(6.0)
                .radius(6.0)
                .fill(l.c("bg-control"))
                .color(l.c("text-primary"))
                .on_edit(pick),
        )
}

/// The section: heading, lede and How it works, the saved agents, and the form that tests and adds one.
pub(crate) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let card = &s.own_agents;
    let out = col()
        .w_full()
        .key("acp-agents")
        .child(section_heading(HEADING, p))
        .child(text(LEDE).font(type_::LABEL).color(p.secondary).wrap().padding(10.0))
        .child(more(s, p, "acp-agents"));
    let mut body = col().w_full().gap(10.0).padding(10.0);
    for agent in card.agents.iter().flatten() {
        let armed = card.armed.as_ref().is_some_and(|(id, _)| *id == agent.id);
        let (label, shown) = if armed { (format!("Remove {}?", agent.name), format!("Remove {}?", agent.name)) } else { (format!("Remove {}", agent.name), "Remove".to_owned()) };
        let id = agent.id.clone();
        body = body.child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .key(format!("acp-agent-{}", agent.id))
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(text(agent.name.clone()).font(l.ty.control).color(l.c("text-primary")))
                        .child(text(meta(agent)).font(type_::LABEL.mono()).color(l.c("text-secondary")).wrap_anywhere()),
                )
                .child(
                    crate::look::controls::button(&l, &shown, crate::look::controls::Tone::Neutral, false)
                        .key(format!("acp-agent-remove-{}", agent.id))
                        .label(label)
                        .on_press(move |s: &mut State| remove(s, &id)),
                ),
        );
    }
    let filled = !card.name.text.trim().is_empty() && !card.command_line.text.trim().is_empty();
    let can_test = !card.command_line.text.trim().is_empty() && !card.busy && s.data.is_some();
    body = body
        .child(
            row()
                .w_full()
                .gap(10.0)
                .child(field(&l, &card.name, "acp-agent-name", "Name", "Goose", false, |s| &mut s.own_agents.name))
                .child(field(&l, &card.command_line, "acp-agent-command", "Command", "goose acp", true, |s| &mut s.own_agents.command_line)),
        )
        .child(
            row()
                .gap(8.0)
                .child(crate::look::controls::button(&l, "Test", crate::look::controls::Tone::Neutral, !can_test).key("acp-agent-test").on_press(test))
                .child(crate::look::controls::primary_button(&l, "Add agent", !filled || card.busy).key("acp-agent-add").on_press(add)),
        );
    if let Some((words, good)) = &card.said {
        body = body.child(text(words.clone()).font(l.ty.body).color(if *good { l.c("text-secondary") } else { l.c("amber-text") }).wrap().role("status").key("acp-agent-said"));
    }
    out.child(body)
}

#[cfg(test)]
#[path = "own_agents_section_tests.rs"]
mod tests;
