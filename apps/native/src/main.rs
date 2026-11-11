#[path = "ui/context_usage.rs"]
mod context_usage;
mod invariants;
#[path = "ui/keyboard_sheet.rs"]
mod keyboard_sheet;
mod keyboard_shortcuts;
mod keychain_note;
mod own_command;
#[path = "ui/home_panel.rs"]
mod home_panel;
mod people;
#[path = "ui/people_panel.rs"]
mod people_panel;
#[path = "ui/recovery.rs"]
mod recovery;
#[path = "ui/reply_actions.rs"]
mod reply_actions;
mod routines;
#[path = "ui/routines_panel.rs"]
mod routines_panel;
mod runtime_terminal;
#[path = "ui/scenes.rs"]
mod scenes;
mod settings_pages;
#[path = "ui/settings_panel.rs"]
mod settings_panel;
mod spend;
mod terminal;
#[path = "ui/terminal_panel.rs"]
mod terminal_panel;
#[cfg(test)]
mod test_isolation;
mod usage;
mod usage_ledger;
#[path = "ui/usage_panel.rs"]
mod usage_panel;
mod workspace_settings;
use scenes::*;
#[path = "ui/composer_wiring.rs"]
mod composer_wiring;
use composer_wiring::*;
#[path = "ui/approval_card.rs"]
mod approval_card;
use approval_card::*;
#[path = "ui/activity_drawers.rs"]
mod activity_drawers;
use activity_drawers::*;
#[path = "ui/messages.rs"]
mod messages;
use messages::*;
#[path = "ui/conversation_list.rs"]
mod conversation_list;
#[path = "ui/conversation_search.rs"]
mod conversation_search;
#[path = "ui/settled.rs"]
mod settled;
#[path = "ui/thread_find.rs"]
mod thread_find;
#[path = "ui/thread_find_bar.rs"]
mod thread_find_bar;
use thread_find_bar::*;
#[path = "ui/command_palette.rs"]
mod command_palette;
#[path = "ui/org.rs"]
mod org;
use command_palette::*;
#[path = "ui/beside.rs"]
mod beside;
use beside::*;
#[path = "ui/sidebar.rs"]
mod sidebar;
use sidebar::*;
#[path = "ui/thread_view.rs"]
mod thread_view;
use thread_view::*;
#[path = "ui/app_state.rs"]
mod app_state;
use app_state::*;

mod approval;
mod approval_rule_store;
mod approval_rules;
mod away;
mod boot_view;
mod changelog;
mod changes;
mod folder_commit;
#[path = "ui/changes_panel.rs"]
mod changes_panel;
#[path = "ui/commit_panel.rs"]
mod commit_panel;
mod checkpoints;
mod command_guard;
mod command_plain;
mod command_reach;
mod composer;
mod conversation_board;
mod engine;
mod folders;
mod ledger;
mod ledger_write;
mod named_folder;
mod native;
mod quick;
mod run_cli;
mod runtime;
mod secrets;
mod starters;
mod store;

use engine::{ConversationRow, Engine, Reply, Started, Teammate};
use native::Kind;
use rune::tok::type_;
use rune::{
    Align, App, AppSpec, Chrome, Color, Cx, Cycle, El, Key, KeyEvent, Latest, ScrollSpec, TextEdit,
    Tx, Waker, col, el, row, text, text_input,
};
use rune_kit::{Markdown, MarkdownStyle};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LEDGER_TARGET: &str = "@ledger";
const CARRY_TARGET: &str = "@carry-models";

struct RecentBatch {
    rows: Vec<ConversationRow>,
    summaries: HashMap<String, ledger::Summary>,
    local: bool,
}

struct LedgerPane {
    header: Teammate,
    conversation: Conversation,
    items: Vec<ledger::Item>,
    dir: std::path::PathBuf,
    worker: Option<ledger::Session>,
    snapshot: ledger::Snapshot,
    due: f64,
    loading: bool,
    // MCP returns the old root ID on follow-up; wait for its new child.
    after_tip: Option<String>,
    after_tip_deadline: Option<f64>,
    backoff: f64,
}

// Fix 3: return the Session on both Ok and Err so transient errors never rebuild the Session.
type LedgerAnswer = Result<(ledger::Session, ledger::Snapshot), Box<(ledger::Session, String)>>;

// Electron's ledger folder (read-only); native's own folder when there is no Electron one. A Session also reads
// native's folder (`native::session`), so a conversation continues across both.
fn ledger_dir() -> Option<std::path::PathBuf> {
    native::electron_ledger_dir()
        .or_else(|| native::store().and_then(|store| store.ledger_dir().ok()))
}

// Fix 2: check whether a request's (teammate_id, conversation_id) tag still matches the open view.
fn active_matches(s: &State, teammate_id: &str, conversation_id: Option<&str>) -> bool {
    if let Some(pane) = &s.ledger {
        pane.header.id == teammate_id
            && (conversation_id.is_none()
                || pane.conversation.conversation_id.as_deref() == conversation_id)
    } else {
        s.teammates
            .get(s.selected)
            .is_some_and(|t| t.id == teammate_id)
            && (conversation_id.is_none()
                || s.conversations
                    .get(teammate_id)
                    .and_then(|c| c.conversation_id.as_deref())
                    == conversation_id)
    }
}

fn begin_ledger(
    s: &mut State,
    id: String,
    title: String,
    header: Teammate,
    after_tip: Option<String>,
    tx: &mut Tx,
) {
    let Some(dir) = ledger_dir() else {
        s.status = "Locust's data folder could not be found.".into();
        s.status_tone = StatusTone::Red;
        return;
    };
    s.ledger_job.cancel();
    // Fix 3: preserve existing Session and transcript items when continuing the same ledger conversation.
    let (mut items, worker, snapshot) = match s.ledger.take() {
        Some(mut prev) if prev.conversation.conversation_id.as_deref() == Some(id.as_str()) => {
            let worker = prev
                .worker
                .take()
                .unwrap_or_else(|| native::session(dir.clone(), id.clone()));
            (prev.items, worker, prev.snapshot)
        }
        _ => (
            Vec::new(),
            native::session(dir.clone(), id.clone()),
            ledger::Snapshot::default(),
        ),
    };
    if after_tip.is_some() {
        items.retain(|item| !matches!(item, ledger::Item::CarryOn { .. }));
    }
    let loading = items.is_empty();
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some(id),
            turns: vec![("You".into(), title)],
        },
        items,
        worker: Some(worker),
        dir,
        snapshot,
        due: 0.0,
        loading,
        after_tip,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s.status = if loading {
        "Loading conversation…"
    } else {
        "Working…"
    }
    .into();
    s.status_tone = if loading {
        StatusTone::Muted
    } else {
        StatusTone::Green
    };
    tx.wake_after(Duration::ZERO);
}

fn leave_ledger(s: &mut State) {
    s.ledger_job.cancel();
    s.picks.remove(LEDGER_TARGET);
    s.model_menu = None;
    // Fix 2: detach whenever leaving an open ledger so in-flight requests are cancelled and unsent drafts restored.
    if s.ledger.is_some()
        || s.flight
            .as_ref()
            .is_some_and(|f| f.teammate_id == LEDGER_TARGET)
    {
        detach(s);
    }
    s.ledger = None;
}

fn select_teammate(s: &mut State, id: &str, tx: &mut Tx) {
    // Fix 5 (Finding 9): confirm the teammate exists before tearing down an open ledger or active flight.
    let Some(index) = s.teammates.iter().position(|t| t.id == id) else {
        return;
    };
    let previous_key = conversation_draft_key(s);
    leave_ledger(s);
    s.model_menu = None;
    // Fix 2: cancel any in-flight request from the previous conversation when switching teammates.
    if s.selected != index || s.flight.is_some() || s.opening_teammate.is_some() {
        detach(s);
    }
    // Restored unsent text belongs to the conversation we left (Electron drafts Map).
    if let Some(key) = &previous_key {
        s.drafts.put(key, &s.draft.text);
    }
    s.selected = index;
    let fill = s.drafts.switch(conversation_draft_key(s), "");
    s.draft.replace_text(fill);
    s.tagged.clear();
    s.at_dismissed = None;
    if s.ledger_available {
        let header = s.teammates.get(index).cloned();
        let conversation = s
            .conversations
            .get(id)
            .and_then(|c| c.conversation_id.clone());
        if let (Some(header), Some(conversation)) = (header, conversation) {
            begin_ledger(s, conversation, String::new(), header, None, tx);
            let fill = s.drafts.switch(conversation_draft_key(s), &s.draft.text);
            s.draft.replace_text(fill);
            tx.scroll_to_bottom("messages");
            return;
        }
    }
    // Fix 5 (Finding 9): reset status when switching from a ledger to a teammate without a ledger.
    set_status(s, id, "Ready", StatusTone::Muted);
    tx.scroll_to_bottom("messages");
}

fn ledger_tick(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    if let Some(result) = s.ledger_job.take() {
        match result {
            Ok((worker, mut snapshot)) => {
                // The ledger terminal event precedes native checkpoint/memory persistence.
                if s.engine == Some(Kind::Native) && native::owns(worker.anchor()) {
                    snapshot.running = true;
                }
                // Fix 2: ignore a worker result if the open ledger changed while the job was running.
                if let Some(pane) = s.ledger.as_mut()
                    && pane.conversation.conversation_id.as_deref() == Some(worker.anchor())
                {
                    changed = pane.loading || pane.snapshot != snapshot;
                    let finished = pane.snapshot.running && !snapshot.running;
                    let new_tip = pane.snapshot.tip != snapshot.tip;
                    let waiting_for_child = pane.after_tip.is_some();
                    if pane.after_tip.is_some() && pane.after_tip_deadline.is_none() {
                        pane.after_tip_deadline = Some(now + 30.0);
                    }
                    if pane
                        .after_tip
                        .as_ref()
                        .is_some_and(|tip| tip != &snapshot.tip || snapshot.running)
                    {
                        pane.after_tip = None;
                        pane.after_tip_deadline = None;
                    }
                    // Fix 3: bound after_tip waiting to 30 s, then show "No new reply yet" and unlock Send.
                    let mut timed_out = false;
                    if pane.after_tip.is_some() && pane.after_tip_deadline.is_some_and(|d| now >= d)
                    {
                        pane.after_tip = None;
                        pane.after_tip_deadline = None;
                        timed_out = true;
                        changed = true;
                    }
                    let active = snapshot.running || pane.after_tip.is_some();
                    if let Some(own) = s.own_turn.as_mut()
                        && pane.conversation.conversation_id.as_deref()
                            == Some(own.conversation.as_str())
                    {
                        if !active || timed_out {
                            s.own_turn = None;
                        } else if own.tip.is_none() {
                            if waiting_for_child
                                && own.before.is_some()
                                && own.before.as_deref() != Some(snapshot.tip.as_str())
                                && !snapshot.tip.is_empty()
                            {
                                if snapshot.running {
                                    own.tip = Some(snapshot.tip.clone());
                                } else {
                                    s.own_turn = None;
                                }
                            } else if !waiting_for_child {
                                s.own_turn = None;
                            }
                        } else if !snapshot.tip.is_empty()
                            && own.tip.as_deref() != Some(snapshot.tip.as_str())
                            && pane.after_tip.is_none()
                        {
                            s.own_turn = None;
                        }
                    }
                    pane.items = snapshot.items.clone();
                    pane.loading = false;
                    pane.backoff = 0.0;
                    // Fix 3: stop scheduling wakeups once a conversation is finished and no longer waiting on after_tip.
                    pane.due = if active { now + 0.25 } else { f64::INFINITY };
                    pane.worker = Some(worker);
                    pane.snapshot = snapshot;

                    if timed_out {
                        s.status = "No new reply yet".into();
                        s.status_tone = StatusTone::Muted;
                    } else {
                        s.status = if active { "Working…" } else { "Ready" }.into();
                        s.status_tone = if pane.snapshot.failed {
                            StatusTone::Red
                        } else if active {
                            StatusTone::Green
                        } else {
                            StatusTone::Muted
                        };
                    }

                    if active {
                        s.flight = Some(Flight {
                            teammate_id: pane.header.id.clone(),
                            conversation_id: pane.conversation.conversation_id.clone(),
                            header: Some(pane.header.clone()),
                            after_tip: pane.after_tip.clone(),
                            message: None,
                        });
                    } else if s.flight.as_ref().is_some_and(|f| {
                        f.teammate_id == pane.header.id || f.teammate_id == LEDGER_TARGET
                    }) {
                        s.flight = None;
                    }

                    if let Some(id) = &pane.conversation.conversation_id {
                        for row in &mut s.recent {
                            if row.id == *id || row.id == pane.snapshot.tip {
                                row.state = if active {
                                    "running"
                                } else if pane.snapshot.failed {
                                    "failed"
                                } else {
                                    "completed"
                                }
                                .into();
                            }
                        }
                    }
                    if finished || new_tip {
                        refresh_recent(s);
                    }
                    // stick_bottom follows new text while the listener stays at the bottom.
                }
            }
            Err(error) => {
                let (worker, words) = *error;
                if let Some(pane) = s.ledger.as_mut()
                    && pane.conversation.conversation_id.as_deref() == Some(worker.anchor())
                {
                    changed = true;
                    s.status = words;
                    s.status_tone = StatusTone::Red;
                    s.flight = None;
                    pane.loading = false;
                    // Fix 3: keep the Session on transient errors and back off 2 s, 4 s, 8 s up to 30 s.
                    let delay = if pane.backoff <= 0.0 {
                        2.0
                    } else {
                        (pane.backoff * 2.0).min(30.0)
                    };
                    pane.backoff = delay;
                    pane.due = now + delay;
                    pane.worker = Some(worker);
                }
            }
        }
    }

    if !s.sending.is_pending()
        && let Some(pane) = s.ledger.as_mut()
    {
        if pane.after_tip.is_some() && pane.after_tip_deadline.is_none() {
            pane.after_tip_deadline = Some(now + 30.0);
        }
        let pollable =
            pane.loading || pane.snapshot.running || pane.after_tip.is_some() || pane.backoff > 0.0;
        if pollable {
            if now >= pane.due {
                if let Some(mut worker) = pane.worker.take() {
                    s.ledger_job.request((), move |()| match worker.poll() {
                        Ok(snapshot) => Ok((worker, snapshot)),
                        Err(_) => Err(Box::new((
                            worker,
                            "Local conversation could not be read.".to_owned(),
                        ))),
                    });
                }
            } else if pane.worker.is_some() {
                tx.wake_after(Duration::from_secs_f64((pane.due - now).max(0.0)));
            }
        }
    }
    changed
}

// Item 1: pick the engine. `LOCUST_NATIVE_ENGINE=1` forces native; otherwise the Electron door is used while it
// answers, and native takes over only when the door is unreachable (a door that answered and refused is not down).
fn choose_engine() -> Result<(Kind, Vec<Teammate>), String> {
    native::choose(
        native::forced(),
        || Engine::local().and_then(|engine| engine.teammates()),
        native::load_roster,
    )
}

fn load_team(s: &mut State) {
    if s.demo || s.team.is_pending() || s.flight.is_some() {
        return;
    }
    s.load_error = None;
    s.status = "Loading teammates…".into();
    s.status_tone = StatusTone::Muted;
    s.team.request((), |()| choose_engine());
    refresh_recent(s);
}

fn setup(s: &mut State, waker: Waker) {
    s.changes_job.set_waker(waker.clone());
    s.commit_read_job.set_waker(waker.clone());
    s.commit_job.set_waker(waker.clone());
    s.changes_repo_job.set_waker(waker.clone());
    s.team.set_waker(waker.clone());
    s.sending.set_waker(waker.clone());
    s.waiting.set_waker(waker.clone());
    s.recents.set_waker(waker.clone());
    s.opening.set_waker(waker.clone());
    s.model_job.set_waker(waker.clone());
    s.files_job.set_waker(waker.clone());
    s.stopping.set_waker(waker.clone());
    s.ledger_job.set_waker(waker);
    load_composer_store(s);
    load_team(s);
    sync_draft_key(s);
    // No secrets stored in native yet → keeps_key false → note stays down. Never reads the keychain.
    // Scenes may force the note on for snaps; do not clear that.
    if !s.keychain_asking {
        s.keychain_asking =
            keychain_note::should_show_note(keychain_note::may_ask_on_this_os(), false, false);
    }
}

fn selected_conversation(s: &State) -> Option<(&Teammate, &Conversation)> {
    if let Some(pane) = &s.ledger {
        return Some((&pane.header, &pane.conversation));
    }
    let teammate = s.teammates.get(s.selected)?;
    Some((teammate, s.conversations.get(&teammate.id)?))
}

fn can_send(s: &State) -> bool {
    !s.demo
        && (s.load_error.is_none() || s.ledger.is_some())
        && !s.team.is_pending()
        && s.flight.is_none()
        && s.opening_teammate.is_none()
        && !s.ledger.as_ref().is_some_and(|p| p.loading)
        && selected_conversation(s).is_some()
        && !native_blocked(s)
        && s.selected_ready
        && s.route_can_run
        && !s.nothing_installed
        && (!s.draft.text.trim().is_empty() || !s.attached.is_empty())
}

fn can_queue(s: &State) -> bool {
    turn_running(s)
        && selected_conversation(s).is_some()
        && queue_front(s).is_none()
        && !s.draft.text.trim().is_empty()
}

/// On the native engine, a teammate that is not a Codex one runs in Locust for now: nothing is sent.
fn native_blocked(s: &State) -> bool {
    s.engine == Some(Kind::Native)
        && selected_conversation(s).is_some_and(|(teammate, _)| !native::supported(teammate))
}

fn set_status(s: &mut State, id: &str, message: &str, ink: StatusTone) {
    let name = if id == LEDGER_TARGET {
        s.ledger
            .as_ref()
            .map(|p| p.header.name.as_str())
            .unwrap_or("Locust")
    } else {
        s.teammates
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.name.as_str())
            .or_else(|| {
                s.ledger
                    .as_ref()
                    .filter(|p| p.header.id == id)
                    .map(|p| p.header.name.as_str())
            })
            .unwrap_or("Locust")
    };
    s.status = format!("{name}: {message}");
    s.status_tone = ink;
}

fn send(s: &mut State, _: &mut Tx) {
    if !can_send(s) {
        return;
    }
    let Some((teammate, conversation)) = selected_conversation(s) else {
        return;
    };
    let teammate_id = teammate.id.clone();
    let conversation_id = conversation.conversation_id.clone();
    let header = teammate.clone();
    let after_tip = s
        .ledger
        .as_ref()
        .map(|p| p.snapshot.tip.clone())
        .filter(|v| !v.is_empty());
    let message = terminal_panel::carry(s, s.draft.text.clone());
    s.own_turn = None;
    // Fix 2: keep the real teammate ID on ledger sends and capture this conversation's header and tip on the flight.
    s.flight = Some(Flight {
        teammate_id: teammate_id.clone(),
        conversation_id: conversation_id.clone(),
        header: Some(header),
        after_tip,
        message: Some(message.clone()),
    });

    // An idle ledger result must not clear a new send. Reopen after acceptance.
    if s.ledger.is_some() {
        s.ledger_job.cancel();
    }
    s.draft.clear();
    set_status(s, &teammate_id, "Sending…", StatusTone::Muted);

    let pick = take_turn_pick(s, &teammate_id);
    s.model_menu = None;
    // On the native engine the turn runs here, in this process, not through the Electron door.
    let native_job = (s.engine == Some(Kind::Native)).then(|| flight_teammate(s, &teammate_id));
    s.sending.request(
        (teammate_id, conversation_id, message, pick, native_job),
        |(id, conversation, message, pick, native_job)| {
            let model = pick.as_ref().map(|p| p.model.as_str());
            let effort = pick.as_ref().and_then(|p| p.effort.as_deref());
            if let Some(teammate) = native_job {
                let teammate = teammate
                    .ok_or_else(|| "That teammate is no longer in the team.".to_string())?;
                return native::send(native::Send {
                    teammate,
                    conversation,
                    prompt: message,
                    model: model.map(str::to_owned),
                    effort: effort.map(str::to_owned),
                });
            }
            let engine = Engine::local()?;
            match conversation {
                Some(id) => engine.send(&id, &message, model, effort),
                None => engine.start(&id, &message, model, effort),
            }
        },
    );
}

fn flight_teammate(s: &State, id: &str) -> Option<Teammate> {
    s.flight
        .as_ref()
        .and_then(|f| f.header.clone())
        .filter(|h| h.id == id)
        .or_else(|| s.teammates.iter().find(|t| t.id == id).cloned())
}

fn take_turn_pick(s: &mut State, teammate: &str) -> Option<TurnPick> {
    s.picks.remove(teammate)
}

fn open_models(s: &mut State, id: String) {
    if s.demo {
        return;
    }
    if s.engine == Some(Kind::Native) {
        // The model list comes from Locust's door; native runs the teammate's own model for now.
        s.status = "Choosing a model for a turn runs in Locust for now; native uses the teammate's own model.".into();
        s.status_tone = StatusTone::Muted;
        return;
    }
    if s.model_menu.as_ref() == Some(&id) {
        s.model_menu = None;
        return;
    }
    if s.models.get(&id).is_some_and(Vec::is_empty) {
        s.models.remove(&id);
    }
    s.model_menu = Some(id.clone());
    s.model_job.request(id, |id| {
        let result = Engine::local().and_then(|e| {
            e.list_models(if id == LEDGER_TARGET || id == CARRY_TARGET {
                None
            } else {
                Some(&id)
            })
        });
        (id, result)
    });
}

/// Native: the turn this window may stop, from the process's own registry. It survives leaving the view and
/// reopening the turn from Recent, because ownership is the registry and not `own_turn` (cleared on every view change).
fn native_stop_key(s: &State) -> Option<String> {
    if s.engine != Some(Kind::Native) {
        return None;
    }
    match &s.ledger {
        Some(p) if p.snapshot.running => native::owned_key(
            Some(&p.snapshot.tip),
            p.conversation.conversation_id.as_deref(),
        ),
        Some(_) => None,
        None => selected_conversation(s)
            .and_then(|(_, c)| native::owned_key(None, c.conversation_id.as_deref())),
    }
}

fn can_stop(s: &State) -> bool {
    if s.demo || s.sending.is_pending() || s.flight.is_none() {
        return false;
    }
    if s.engine == Some(Kind::Native) {
        return native_stop_key(s).is_some();
    }
    let Some(own) = &s.own_turn else {
        return false;
    };
    let Some((_, c)) = selected_conversation(s) else {
        return false;
    };
    c.conversation_id.as_deref() == Some(own.conversation.as_str())
        && s.ledger.as_ref().is_none_or(|p| {
            p.snapshot.running && own.tip.as_deref() == Some(p.snapshot.tip.as_str())
        })
}

fn stop(s: &mut State) {
    if !can_stop(s) || s.stopping.is_pending() {
        return;
    }
    if let Some(key) = native_stop_key(s) {
        s.stopping.request(key, |id| {
            let result = native::stop(&id);
            (id, result)
        });
        return;
    }
    if let Some(own) = &s.own_turn {
        s.stopping.request(own.conversation.clone(), |id| {
            // A turn this process runs stops through the runtime (turn/interrupt, recorded as cancelled); any other
            // goes through Locust's door.
            let result = if native::owns(&id) {
                native::stop(&id)
            } else {
                Engine::local().and_then(|e| e.stop_turn(&id))
            };
            (id, result)
        });
    }
}

fn carry(s: &mut State, id: String, runtime: String, model: String) {
    if s.flight.is_some() || s.sending.is_pending() {
        return;
    }
    if s.engine == Some(Kind::Native) {
        s.status = "Carrying on with another agent runs in Locust for now.".into();
        s.status_tone = StatusTone::Muted;
        s.model_menu = None;
        return;
    }
    let Some((header, c)) = selected_conversation(s) else {
        return;
    };
    let header_id = header.id.clone();
    let conversation_id = c.conversation_id.clone();
    let header = header.clone();
    let after_tip = s
        .ledger
        .as_ref()
        .map(|p| p.snapshot.tip.clone())
        .filter(|v| !v.is_empty());
    s.own_turn = None;
    s.picks.remove(&header_id);
    s.picks.remove(LEDGER_TARGET);
    if s.ledger.is_some() {
        s.ledger_job.cancel();
    }
    s.flight = Some(Flight {
        teammate_id: header_id.clone(),
        conversation_id,
        header: Some(header),
        after_tip,
        message: None,
    });
    s.model_menu = None;
    set_status(s, &header_id, "Sending…", StatusTone::Muted);
    s.sending
        .request((id, runtime, model), |(id, runtime, model)| {
            Engine::local()?.carry_on(&id, &runtime, &model)
        });
}

fn refresh_recent(s: &mut State) {
    if s.demo {
        return;
    }
    let on_native = s.engine == Some(Kind::Native);
    s.recents.request((), move |()| {
        // Electron's folder (read-only) and native's own, merged newest first; native rows are marked.
        if let Some(listed) = native::list_recent(usize::MAX, on_native) {
            let rows = listed
                .summaries
                .iter()
                .map(|m| ConversationRow {
                    id: m.mission_id.clone(),
                    teammate: engine::runtime_name(&m.runtime).into(),
                    title: m.title.clone(),
                    started_by: if listed.native.contains(&m.mission_id) {
                        "native"
                    } else {
                        "ledger"
                    }
                    .into(),
                    state: if m.running {
                        "running"
                    } else if m.failed {
                        "failed"
                    } else {
                        "completed"
                    }
                    .into(),
                    updated: m.updated.clone(),
                })
                .collect();
            return Ok(RecentBatch {
                rows,
                summaries: listed
                    .summaries
                    .into_iter()
                    .map(|m| (m.mission_id.clone(), m))
                    .collect(),
                local: true,
            });
        }
        if on_native {
            return Err("Local conversations could not be read.".to_string());
        }
        // An unreadable local folder falls back to MCP and its visibility rules.
        Ok(RecentBatch {
            rows: Engine::local()?.conversations()?,
            summaries: HashMap::new(),
            local: false,
        })
    });
}

// Detaching stops local observation, not a turn already running in Locust.
fn detach(s: &mut State) {
    close_find(s);
    s.find_show_earlier = false;
    s.find_pieces.clear();
    s.sending.cancel();
    s.waiting.cancel();
    s.opening.cancel();
    s.ledger_job.cancel();
    // Fix 2: restore unsent draft text if a send is cancelled before landing.
    if let Some(message) = s.flight.take().and_then(|f| f.message) {
        restore_failed_draft(s, message);
    }
    if s.own_turn.as_ref().is_some_and(|o| o.tip.is_none()) || s.ledger.is_none() {
        s.own_turn = None;
    }
    s.opening_teammate = None;
    s.opening_conversation = None;
    s.waiting_target = None;
    if let Some(pane) = s.ledger.as_mut() {
        pane.worker = None;
        pane.loading = false;
        pane.after_tip = None;
        pane.after_tip_deadline = None;
        pane.backoff = 0.0;
    }
}

fn tracking_error(s: &mut State, teammate_id: &str, message: &str) {
    detach(s);
    set_status(s, teammate_id, message, StatusTone::Red);
}

fn target_index(s: &mut State, teammate_id: &str) -> Option<usize> {
    match s.teammates.iter().position(|t| t.id == teammate_id) {
        Some(index) => Some(index),
        None => {
            // Fix: removal cancels all local tracking, including landed stale results.
            tracking_error(
                s,
                teammate_id,
                "Teammate is no longer in the team; stopped watching. The turn goes on in Locust",
            );
            None
        }
    }
}

fn request_reply(s: &mut State, teammate_id: &str, delay: Duration) {
    if target_index(s, teammate_id).is_none() {
        return;
    }
    let conversation_id = s
        .conversations
        .get(teammate_id)
        .and_then(|c| c.conversation_id.clone())
        .filter(|id| !id.is_empty());

    let Some(id) = conversation_id else {
        // Fix: missing IDs end the flight instead of leaving Send locked forever.
        tracking_error(
            s,
            teammate_id,
            "No active conversation ID; stopped watching",
        );
        return;
    };

    // Fix 2: tag the waiting request with the conversation it belongs to.
    s.waiting_target = Some((teammate_id.to_owned(), id.clone()));
    // Fix: retain this handle's waker while choosing each request's debounce.
    s.waiting = std::mem::take(&mut s.waiting).debounce(delay);
    s.waiting.request(id, |id| Engine::local()?.reply(&id, 20));
}

fn apply_team(s: &mut State, team: Vec<Teammate>) {
    let selected = s.teammates.get(s.selected).map(|t| t.id.clone());
    for old in &s.teammates {
        let same_runtime = team
            .iter()
            .find(|t| t.id == old.id)
            .is_some_and(|t| t.runtime == old.runtime);
        if !same_runtime {
            s.picks.remove(&old.id);
            s.models.remove(&old.id);
        }
    }
    for t in &team {
        s.conversations.entry(t.id.clone()).or_default();
    }
    s.teammates = team;
    s.selected = selected
        .as_deref()
        .and_then(|id| s.teammates.iter().position(|t| t.id == id))
        .unwrap_or(0);
    s.load_error = None;

    let active = s
        .flight
        .as_ref()
        .map(|f| f.teammate_id.clone())
        .or_else(|| s.opening_teammate.clone());
    if let Some(id) = active {
        if id != LEDGER_TARGET && (s.ledger.is_none() || s.sending.is_pending()) {
            let _ = target_index(s, &id);
        }
    } else if s.ledger.is_none() {
        s.status = if s.teammates.is_empty() {
            "No teammates yet. Add a teammate in Locust."
        } else {
            "Ready"
        }
        .into();
        s.status_tone = StatusTone::Muted;
    }
}

fn open_recent(s: &mut State, mut row: ConversationRow, tx: &mut Tx) {
    home_panel::opened(s, &row.id);
    if let Some(title) =
        crate::conversation_list::conversation_title_for(&recent_as_missions(s), Some(&row.id))
    {
        row.title = title.to_owned();
    }
    if let Some(summary) = s.ledger_summaries.get(&row.id).cloned() {
        detach(s);
        s.own_turn = None;
        s.picks.remove(LEDGER_TARGET);
        s.model_menu = None;
        // Ledger metadata has no reliable teammate identity; do not guess from a model.
        let header = Teammate {
            id: LEDGER_TARGET.into(),
            name: engine::runtime_name(&summary.runtime).into(),
            role: "Conversation".into(),
            runtime: (!summary.runtime.is_empty()).then_some(summary.runtime),
            model: (!summary.model.is_empty()).then_some(summary.model),
            busy: summary.running,
            runs_in: String::new(),
            folder: None,
        };
        begin_ledger(s, row.id, row.title, header, None, tx);
        return;
    }

    if s.team.is_pending() {
        return;
    }
    let Some(index) = s
        .teammates
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(&row.teammate))
    else {
        s.status = format!("{} is not in the current team.", row.teammate);
        s.status_tone = StatusTone::Muted;
        return;
    };
    let Some(id) = s.teammates.get(index).map(|t| t.id.clone()) else {
        return;
    };
    detach(s);
    s.own_turn = None;
    s.picks.remove(LEDGER_TARGET);
    s.picks.remove(&id);
    s.model_menu = None;
    s.ledger = None;
    s.selected = index;
    s.conversations.insert(
        id.clone(),
        Conversation {
            conversation_id: Some(row.id.clone()),
            turns: vec![("You".into(), row.title)],
        },
    );
    set_status(s, &id, "Loading…", StatusTone::Muted);
    tx.scroll_to_bottom("messages");

    if !s.demo {
        // Fix 2: tag the opening request with both the teammate ID and conversation ID.
        s.opening_teammate = Some(id);
        s.opening_conversation = Some(row.id.clone());
        s.opening
            .request(row.id, |id| Engine::local()?.reply(&id, 0));
    }
}

fn fresh_conversation(s: &mut State) {
    // Fix 5 (Finding 9): close any open ledger on Cmd+N, and do not wipe a teammate's conversation when closing a Recent ledger.
    let ledger_owner = s.ledger.as_ref().map(|p| p.header.id.clone());
    detach(s);
    s.own_turn = None;
    s.picks.remove(LEDGER_TARGET);
    s.model_menu = None;
    s.ledger = None;
    let target_id = match ledger_owner.as_deref() {
        Some(LEDGER_TARGET) => s.teammates.get(s.selected).map(|t| t.id.clone()),
        Some(id) => Some(id.to_owned()),
        None => s.teammates.get(s.selected).map(|t| t.id.clone()),
    };
    if let Some(id) = target_id {
        if ledger_owner.as_deref() != Some(LEDGER_TARGET) {
            s.picks.remove(&id);
            s.conversations.insert(id.clone(), Conversation::default());
        }
        set_status(s, &id, "Ready", StatusTone::Muted);
    } else {
        s.status = "Ready".into();
        s.status_tone = StatusTone::Muted;
    }
    sync_draft_key(s);
    s.attached.clear();
    s.tagged.clear();
}

fn on_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if s.changes.open && event.mods.shift && !event.mods.alt && event.is_command('y') {
        s.changes.files = !s.changes.files;
        return true;
    }
    if event.key == Key::Escape {
        if s.changes.open {
            s.changes.open = false;
            return true;
        }
        if s.home.splash_open {
            s.home.splash_open = false;
            s.home.seen_version = Some(s.home.current_version.clone());
            return true;
        }
        if s.home.boot_open {
            s.home.boot_open = false;
            return true;
        }
        if s.home.board_open {
            s.home.board_open = false;
            return true;
        }
        if s.home.home_open {
            s.home.home_open = false;
            return true;
        }
        if s.settings_panel.open {
            s.settings_panel.open = false;
            return true;
        }
        if s.usage_panel.open {
            s.usage_panel.open = false;
            return true;
        }
        if s.keyboard_sheet.open {
            s.keyboard_sheet.open = false;
            return true;
        }
        if s.people_panel.open {
            s.people_panel.open = false;
            return true;
        }
        if s.palette_open {
            s.palette_open = false;
            return true;
        }
        if s.find_open {
            close_find(s);
            return true;
        }
        if s.org_menu_id.is_some() {
            s.org_menu_id = None;
            return true;
        }
        if s.rename_id.is_some() {
            s.rename_id = None;
            s.rename_draft.clear();
            return true;
        }
        if !s.sidebar_query.text.is_empty() {
            s.sidebar_query.clear();
            return true;
        }
        if s.beside.is_some() {
            s.beside = None;
            return true;
        }
        if s.flight.is_some() || s.opening_teammate.is_some() {
            detach(s);
            s.status = if s.engine == Some(Kind::Native) {
                "Stopped watching; the turn goes on in this window"
            } else {
                "Stopped watching; the turn goes on in Locust"
            }
            .into();
            s.status_tone = StatusTone::Muted;
        } else {
            s.draft.clear();
        }
        return true;
    }
    if s.palette_open {
        let actions = palette_matches(&s.palette_actions, &s.palette_query.text);
        match event.key {
            Key::Down | Key::Up => {
                s.palette_index =
                    thread_find::step_hit(s.palette_index, actions.len(), event.key == Key::Up);
                return true;
            }
            Key::Enter => {
                if let Some(action) = actions.get(s.palette_index) {
                    run_palette_action(s, &action.id, tx);
                }
                return true;
            }
            _ => {}
        }
    }
    if s.find_open && event.key == Key::Enter {
        s.find_pieces = find_pieces_now(s);
        refresh_find_hits(s);
        s.find_index = thread_find::step_hit(s.find_index, s.find_hits.len(), event.mods.shift);
        return true;
    }
    if event.mods.shift || event.mods.alt {
        return false;
    }
    if event.is_command('f') {
        // Seed find pieces from the open conversation's visible text.
        s.find_pieces = find_pieces_now(s);
        open_find(s);
        return true;
    }
    if event.is_command('k') {
        let missions = sidebar_missions_now(s);
        s.palette_actions = default_palette_actions(&missions, can_stop(s));
        s.palette_query.clear();
        s.draft.focused = false;
        s.find_query.focused = false;
        s.palette_query.focused = true;
        s.palette_index = 0;
        s.palette_open = true;
        return true;
    }
    if event.is_command('n') {
        fresh_conversation(s);
        tx.scroll_to_bottom("messages");
        return true;
    }
    for digit in '1'..='9' {
        if event.is_command(digit) {
            if let Some(id) = s
                .teammates
                .get((digit as u8 - b'1') as usize)
                .map(|t| t.id.clone())
            {
                select_teammate(s, &id, tx);
            }
            return true;
        }
    }
    false
}

fn find_pieces_now(s: &State) -> Vec<String> {
    if !s.find_pieces.is_empty() && (s.find_show_earlier || s.demo) {
        return s.find_pieces.clone();
    }
    let mut pieces = Vec::new();
    if let Some((_, conversation)) = selected_conversation(s) {
        for (who, body) in &conversation.turns {
            pieces.push(format!("{who}: {body}"));
        }
    }
    if let Some(pane) = &s.ledger {
        for item in &pane.items {
            match item {
                ledger::Item::User(text) => pieces.push(text.clone()),
                ledger::Item::Assistant { text, .. } => pieces.push(text.clone()),
                _ => {}
            }
        }
    }
    pieces
}

// Locust supplies UTC ISO timestamps. Fractional seconds are irrelevant here.
fn parse_updated(value: &str) -> Option<(i64, usize, u32)> {
    let utc = value
        .strip_suffix('Z')
        .or_else(|| value.strip_suffix("+00:00"))?;
    let (base, fraction) = utc.split_once('.').unwrap_or((utc, ""));
    if !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let b = base.as_bytes();
    if b.len() != 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let number = |start: usize, end: usize| -> Option<i64> {
        let bytes = b.get(start..end)?;
        if !bytes.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(bytes).ok()?.parse().ok()
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);

    if year < 1
        || !(1..=12).contains(&month)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..60).contains(&second)
    {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let lengths = [
        31i64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let month = month as usize;
    if !(1..=lengths[month - 1]).contains(&day) {
        return None;
    }
    let y = year - 1;
    let days = 365 * y + y / 4 - y / 100 + y / 400 + lengths[..month - 1].iter().sum::<i64>() + day
        - 1
        - 719_162;
    Some((
        days * 86_400 + hour * 3_600 + minute * 60 + second,
        month,
        day as u32,
    ))
}

fn when(updated: &str, now: i64) -> String {
    let Some((seconds, month, day)) = parse_updated(updated) else {
        return "unknown time".into();
    };
    let age = now.saturating_sub(seconds).max(0);
    if age < 60 {
        "just now".into()
    } else if age < 3_600 {
        format!("{} min ago", age / 60)
    } else if age < 86_400 {
        format!("{} h ago", age / 3_600)
    } else {
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        format!("{} {day}", months[month - 1])
    }
}

fn clock_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

// Both opened recent threads and sent messages resolve their target when results land.
fn land_reply(s: &mut State, teammate_id: &str, result: Result<Reply, String>, tx: &mut Tx) {
    let Some(index) = target_index(s, teammate_id) else {
        return;
    };
    match result {
        Ok(Reply::Working(message)) => {
            let message = if message.is_empty() {
                "Working…".into()
            } else {
                message
            };
            set_status(s, teammate_id, &message, StatusTone::Green);
            request_reply(s, teammate_id, Duration::from_millis(500));
        }
        Ok(Reply::WaitingForYou(message)) => {
            set_status(s, teammate_id, &message, StatusTone::Amber);
            // Fix: approval cards return immediately; re-ask only after three seconds.
            request_reply(s, teammate_id, Duration::from_secs(3));
        }
        Ok(Reply::Answer(message)) => {
            let Some(who) = s.teammates.get(index).map(|t| t.name.clone()) else {
                return;
            };
            let Some(conversation) = s.conversations.get_mut(teammate_id) else {
                tracking_error(
                    s,
                    teammate_id,
                    "Conversation is no longer available; stopped watching",
                );
                return;
            };
            conversation.turns.push((who, message));
            set_status(s, teammate_id, "Ready", StatusTone::Muted);
            s.own_turn = None;
            s.flight = None;
            refresh_recent(s);
            drain_queue_after_turn(s, tx);
            if s.selected == index {
                tx.scroll_to_bottom("messages");
            }
        }
        Ok(Reply::Failed(error)) | Err(error) => {
            set_status(s, teammate_id, &error, StatusTone::Red);
            s.own_turn = None;
            s.flight = None;
            refresh_recent(s);
        }
    }
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    if s.demo {
        return false;
    }
    let mut changed = routines_panel::poll(s);
    changed |= changes_panel::poll(s);
    changed |= commit_panel::poll(s);
    changed |= terminal_panel::poll(s);
    if let Some(feedback) = &s.copy_feedback {
        let remain = feedback
            .until
            .saturating_duration_since(std::time::Instant::now());
        if remain.is_zero() {
            s.copy_feedback = None;
            changed = true;
        } else {
            tx.wake_after(remain);
        }
    }
    changed |= sync_approvals(s);
    changed |= home_panel::sync_board_asks(s);
    let root = selected_conversation(s)
        .and_then(|(t, _)| t.folder.as_ref())
        .map(std::path::PathBuf::from);
    if root != s.files_root {
        s.files_job.cancel();
        s.project_files.clear();
        s.files_partial = false;
        s.files_asked = None;
        s.files_root = root.clone();
    }
    if composer::at_query(&s.draft.text).is_some()
        && s.files_asked
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(30))
        && let Some(root) = root
    {
        s.files_asked = Some(std::time::Instant::now());
        s.files_job
            .request(root, |root| composer::workspace_files::list(&root));
    }
    if let Some((paths, partial)) = s.files_job.take() {
        s.project_files = paths;
        s.files_partial = partial;
        changed = true;
    }
    if let Some((id, result)) = s.model_job.take() {
        changed = true;
        match result {
            Ok(models) => {
                s.models.insert(id, models);
            }
            Err(error) => {
                s.models.insert(id.clone(), Vec::new());
                if s.model_menu.as_ref() == Some(&id) {
                    s.status = error;
                    s.status_tone = StatusTone::Red;
                    s.model_menu = None;
                }
            }
        }
    }
    if let Some((id, result)) = s.stopping.take() {
        changed = true;
        match result {
            Ok(_) => {
                if s.own_turn.as_ref().is_some_and(|o| o.conversation == id) {
                    if s.ledger.is_none() {
                        s.waiting.cancel();
                        if let Some(f) = &s.flight
                            && let Some(c) = s.conversations.get_mut(&f.teammate_id)
                        {
                            c.turns.push(("Locust".into(), "Stopped".into()));
                        }
                        s.flight = None;
                    }
                    s.own_turn = None;
                    s.status = "Stop requested".into();
                    s.status_tone = StatusTone::Muted;
                } else if s.engine == Some(Kind::Native) {
                    s.status = "Stop requested".into();
                    s.status_tone = StatusTone::Muted;
                }
            }
            Err(error) => {
                s.status = error;
                s.status_tone = StatusTone::Red;
            }
        }
    }

    if !s.model_job.is_pending() && s.load_error.is_none() {
        let id =
            selected_conversation(s).and_then(|(t, _)| t.runtime.as_ref().map(|_| t.id.clone()));
        if let Some(id) = id
            && id != LEDGER_TARGET
            && !s.models.contains_key(&id)
            && s.engine != Some(Kind::Native)
        {
            s.model_job.request(id, |id| {
                let result = Engine::local().and_then(|e| e.list_models(Some(&id)));
                (id, result)
            });
        }
    }

    if let Some(result) = s.team.take() {
        changed = true;
        match result {
            Ok((kind, team)) => {
                let changed_engine = s.engine != Some(kind);
                s.engine = Some(kind);
                apply_team(s, team);
                // The first list was asked before the engine was known; native reads its own folder too.
                if changed_engine {
                    refresh_recent(s);
                }
            }
            Err(error) => {
                s.status = error.clone();
                s.status_tone = StatusTone::Red;
                s.load_error = Some(error);
            }
        }
    }

    if let Some(result) = s.sending.take() {
        changed = true;
        // Fix 2: drop a landed send result if its flight tag no longer matches the active conversation.
        let matches_active = s
            .flight
            .as_ref()
            .is_some_and(|f| active_matches(s, &f.teammate_id, f.conversation_id.as_deref()));
        if matches_active && let Some(id) = s.flight.as_ref().map(|f| f.teammate_id.clone()) {
            let message = s.flight.as_mut().and_then(|f| f.message.take());
            let flight_conv = s.flight.as_ref().and_then(|f| f.conversation_id.clone());
            let flight_header = s.flight.as_ref().and_then(|f| f.header.clone());
            let after = s.flight.as_ref().and_then(|f| f.after_tip.clone());
            match result {
                Ok(started) => {
                    if let Some(from) = queue_key_now(s) {
                        s.queue_rows = composer::queue::requeued_rows(
                            &s.queue_rows,
                            &from,
                            &started.conversation_id,
                        );
                        if s.queue_send_now.as_deref() == Some(from.as_str()) {
                            s.queue_send_now = Some(started.conversation_id.clone());
                        }
                        persist_queue(s);
                    }
                    let same_conv =
                        flight_conv.as_deref() == Some(started.conversation_id.as_str());
                    let use_ledger =
                        s.ledger_available || s.ledger.is_some() || s.engine == Some(Kind::Native);
                    s.own_turn = if !same_conv {
                        Some(OwnTurn {
                            conversation: started.conversation_id.clone(),
                            tip: Some(started.conversation_id.clone()),
                            before: None,
                        })
                    } else if !use_ledger || after.is_some() {
                        Some(OwnTurn {
                            conversation: started.conversation_id.clone(),
                            tip: None,
                            before: after.clone(),
                        })
                    } else {
                        None
                    };
                    if use_ledger {
                        // Fix 2: use the header and tip captured when the send was initiated, never another conversation's pane.
                        let header = flight_header
                            .or_else(|| s.teammates.iter().find(|t| t.id == id).cloned());

                        if let Some(header) = header {
                            if let Some(conversation) = s.conversations.get_mut(&id) {
                                conversation.conversation_id =
                                    Some(started.conversation_id.clone());
                                if let Some(message) = &message {
                                    conversation.turns.push(("You".into(), message.clone()));
                                }
                            }
                            begin_ledger(
                                s,
                                started.conversation_id.clone(),
                                message.unwrap_or_default(),
                                header.clone(),
                                after,
                                tx,
                            );
                            s.flight = Some(Flight {
                                teammate_id: id,
                                conversation_id: Some(started.conversation_id),
                                header: Some(header),
                                after_tip: None,
                                message: None,
                            });
                        } else {
                            tracking_error(s, &id, "Teammate is no longer in the team.");
                        }
                    } else if let Some(index) = target_index(s, &id) {
                        if let Some(conversation) = s.conversations.get_mut(&id) {
                            conversation.conversation_id = Some(started.conversation_id.clone());
                            if let Some(message) = message {
                                conversation.turns.push(("You".into(), message));
                            }
                            if let Some(flight) = s.flight.as_mut() {
                                flight.conversation_id = Some(started.conversation_id);
                            }
                            set_status(s, &id, "Working…", StatusTone::Green);
                            request_reply(s, &id, Duration::ZERO);
                            if s.selected == index {
                                tx.scroll_to_bottom("messages");
                            }
                        } else {
                            tracking_error(s, &id, "Conversation is no longer available.");
                        }
                    }
                }
                Err(error) => {
                    if let Some(message) = message {
                        restore_failed_draft(s, message);
                    }
                    set_status(s, &id, &error, StatusTone::Red);
                    s.flight = None;
                    if let Some(pane) = s.ledger.as_mut() {
                        if pane.worker.is_none()
                            && let Some(id) = &pane.conversation.conversation_id
                        {
                            pane.worker = Some(native::session(pane.dir.clone(), id.clone()));
                        }
                        pane.due = 0.0;
                        tx.wake_after(Duration::ZERO);
                    }
                }
            }
        }
    }

    if let Some(result) = s.recents.take() {
        changed = true;
        match result {
            Ok(batch) => {
                home_panel::record_finishes(s, &batch.rows);
                s.recent = batch.rows;
                s.ledger_summaries = batch.summaries;
                s.ledger_available = batch.local;
                s.recent_error = None;
            }
            Err(words) => {
                s.ledger_available = false;
                s.ledger_summaries.clear();
                s.recent_error = Some(words);
            }
        }
    }

    if let Some(result) = s.opening.take() {
        changed = true;
        let id = s.opening_teammate.take();
        let conv = s.opening_conversation.take();
        // Fix 2: drop an opening result if its conversation tag no longer matches the active view.
        if let Some(id) = id
            && s.ledger.is_none()
            && active_matches(s, &id, conv.as_deref())
        {
            s.flight = Some(Flight {
                teammate_id: id.clone(),
                conversation_id: conv,
                header: None,
                after_tip: None,
                message: None,
            });
            land_reply(s, &id, result, tx);
        }
    }
    if let Some(result) = s.waiting.take() {
        changed = true;
        let target = s.waiting_target.take();
        // Fix 2: drop a waiting result if a ledger opened or its conversation tag no longer matches.
        if s.ledger.is_none()
            && let Some(id) = s.flight.as_ref().map(|f| f.teammate_id.clone())
            && let Some(flight_conv) = s.flight.as_ref().map(|f| f.conversation_id.clone())
            && target.as_ref().is_none_or(|(t_id, t_conv)| {
                t_id == &id
                    && (flight_conv.is_none() || flight_conv.as_deref() == Some(t_conv.as_str()))
            })
            && active_matches(s, &id, flight_conv.as_deref())
        {
            land_reply(s, &id, result, tx);
        }
    }

    changed |= ledger_tick(s, now, tx);
    drain_queue_after_turn(s, tx);
    changed
}

fn on_close(s: &mut State, _: &mut Tx) {
    // Fix: cancelled handles never land; clear their flight and visible status too.
    s.team.cancel();
    s.recents.cancel();
    s.flight = None;
    detach(s);
    s.ledger = None;
    s.status.clear();
    s.status_tone = StatusTone::Muted;
    // Electron terminal-host dispose: end every tab and bang command before the window is gone.
    // Drop ends the PTY/own-command trees; do not wait on the UI thread.
    terminal_panel::dispose(s);
    home_panel::save_attention(s);
    // Native turns live in this process: ask each to stop now and let a background thread watch them write their
    // cancelled records. The UI thread does not wait; `main` waits after the window is gone.
    native::shutdown_in_background();
}

/// The agent line, plus what native cannot yet run: "runs in Locust for now".
fn agent_line(s: &State, teammate: &Teammate) -> String {
    if s.engine == Some(Kind::Native) && !native::supported(teammate) {
        format!("{} · runs in Locust for now", agent(teammate))
    } else {
        agent(teammate)
    }
}

fn agent(teammate: &Teammate) -> String {
    let runtime = teammate
        .runtime
        .as_deref()
        .map(engine::runtime_name)
        .unwrap_or("Agent");
    match teammate.model.as_deref() {
        Some(model) => format!("{runtime} · {}", model.rsplit('/').next().unwrap_or(model)),
        None => runtime.into(),
    }
}

fn mode_word(mode: &str) -> &str {
    match mode {
        "ask" => "Ask",
        "plan" => "Plan",
        "accept-edits" => "Edit",
        "approve-each" => "Approve each action",
        "auto" => "Auto",
        native::IN_LOCUST => "Locust for now",
        other => other,
    }
}

fn markdown_style(p: &Palette) -> MarkdownStyle {
    MarkdownStyle::neutral(p.dark)
        .text(p.text)
        .muted(p.muted)
        .link(p.green)
        .inline_code_bg(p.raised)
        .code_block(p.raised, p.hairline)
        .rule(p.hairline)
        .body_font(type_::BODY)
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Palette::new(s.appearance.unwrap_or_else(|| cx.system_dark()));
    let sidebar = sidebar(s, &p);
    let changes_open = s.changes.open
        && s.changes_repo
            .as_ref()
            .is_none_or(|(path, yes)| path == &s.changes.folder && *yes);
    let mut thread = if changes_open {
        changes_panel::view(s, &p)
    } else if s.routines.open {
        routines_panel::view(s, &p)
    } else if s.home.boot_open {
        home_panel::boot_view_ui(s, &p)
    } else if s.home.board_open {
        home_panel::board_view(s, &p)
    } else if s.home.home_open {
        home_panel::home_view(s, &p)
    } else if s.settings_panel.open {
        settings_panel::view(s, &p)
    } else if s.people_panel.open {
        people_panel::view(s, &p)
    } else {
        thread_view(s, &p)
    };
    if let Some(find) = thread_find_bar(s, &p) {
        thread = col()
            .grow(1.0)
            .h_full()
            .min_w(0.0)
            .child(find)
            .child(thread);
    }
    // The conversation keeps room for its message box; side panels (terminal, beside, viewer) shrink first.
    let thread = col().grow(1.0).h_full().min_w(380.0).child(thread);
    let mut right = row().grow(1.0).h_full().min_w(0.0).child(thread);
    if s.terminal.open {
        right = right.child(terminal_panel::view(s, &p));
    }
    if let Some(beside) = beside_panel(s, &p) {
        right = right.child(beside);
    }
    if let Some(viewer) = reply_actions::viewer(s, &p) {
        right = right.child(viewer);
    }
    let mut app = row()
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(sidebar)
        .child(right);
    if s.keychain_asking {
        app = col()
            .w_full()
            .h_full()
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .padding(10.0)
                    .fill(p.amber.alpha(0.2))
                    .child(
                        text(crate::keychain_note::KEYCHAIN_NOTE.to_owned())
                            .font(type_::LABEL)
                            .color(p.amber)
                            .wrap()
                            .grow(1.0),
                    ),
            )
            .child(app);
    }
    if s.home.splash_open {
        app = app.child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .fill(Color::BLACK.alpha(0.55))
                .center()
                .child(home_panel::splash_view(s, &p).modal(true, |s: &mut State| {
                    s.home.splash_open = false;
                    s.home.seen_version = Some(s.home.current_version.clone());
                })),
        );
    }
    if s.usage_panel.open {
        app = app.child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .fill(Color::BLACK.alpha(0.55))
                .center()
                .child(
                    usage_panel::view(s, &p)
                        .modal(true, |s: &mut State| s.usage_panel.open = false),
                ),
        );
    }
    if s.keyboard_sheet.open {
        app = app.child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .fill(Color::BLACK.alpha(0.55))
                .center()
                .child(
                    keyboard_sheet::view(s, &p)
                        .modal(true, |s: &mut State| s.keyboard_sheet.open = false),
                ),
        );
    }
    if s.palette_open {
        app.child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .fill(Color::BLACK.alpha(0.55))
                .center()
                .child(command_palette(s, &p).modal(true, |s: &mut State| s.palette_open = false)),
        )
    } else {
        app
    }
}

// Only the selected runtime may supply a display name: IDs can overlap across agents.
fn model_chip_name<'a>(id: &'a str, current: Option<&'a engine::Models>) -> &'a str {
    current
        .and_then(|a| a.models.iter().find(|m| m.id == id))
        .map(|m| m.name.as_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(id)
}

fn bench_options(args: &[String]) -> Result<(std::path::PathBuf, Duration), String> {
    let mut out = None;
    let mut seconds = 30_u64;
    let mut args = args.iter();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--bench" => out = Some(args.next().ok_or("--bench requires an output path")?.into()),
            "--bench-seconds" => {
                seconds = args
                    .next()
                    .ok_or("--bench-seconds requires a positive integer")?
                    .parse()
                    .map_err(|_| "--bench-seconds requires a positive integer")?;
                if seconds == 0 {
                    return Err("--bench-seconds must be positive".into());
                }
            }
            _ => return Err(format!("unsupported benchmark argument: {flag}")),
        }
    }
    Ok((
        out.ok_or("--bench <out.json> is required")?,
        Duration::from_secs(seconds),
    ))
}

fn run_bench(args: &[String], process_start: std::time::Instant) {
    let (out, duration) = bench_options(args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    // No setup, poll, key handlers or live scenes: no engine or ledger access.
    let app = App::new(AppSpec::new(
        "Locust — benchmark",
        (1040.0, 720.0),
        || {
            let mut state = State::default();
            team_scene(&mut state);
            state
        },
        view,
    ))
    .args(Vec::<String>::new())
    .accessory()
    .open_inactive()
    .macos_window(rune::MacWindowOptions::default().level(-1))
    .window_options(rune::WindowOptions {
        click_through: true,
        ..Default::default()
    })
    .chrome(Chrome {
        full_size_content: true,
        traffic_lights: Some((20.0, 18.0)),
        drag_regions: true,
    });
    let handle = app.handle();
    let mut recorded = false;
    app.on_frame_drawn(move |_| {
        if recorded {
            return;
        }
        recorded = true;
        // Rune calls this after queue.present and first-frame GPU completion, never
        // for skipped/occluded frames. This is a host observation, not scanout time.
        let ms = process_start.elapsed().as_secs_f64() * 1000.0;
        let result = serde_json::to_vec(&serde_json::json!({"first_frame_ms": ms}))
            .map_err(std::io::Error::other)
            .and_then(|bytes| std::fs::write(&out, bytes));
        if let Err(error) = result {
            eprintln!("cannot write {}: {error}", out.display());
            std::process::exit(1);
        }
        let handle = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            handle.quit();
        });
    })
    .run();
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--permission-bridge") {
        let result =
            runtime::permission_bridge::serve(&std::env::args().nth(2).unwrap_or_default());
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    if std::env::args().nth(1).as_deref() == Some("--run") {
        let code = match run_cli::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("{error}");
                1
            }
        };
        std::process::exit(code);
    }
    let process_start = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|arg| arg == "--bench" || arg == "--bench-seconds")
    {
        run_bench(&args[1..], process_start);
        return;
    }
    if args.get(1).map(String::as_str) == Some("--ask") {
        let (who, message) = (
            args.get(2).cloned().unwrap_or_default(),
            args.get(3).cloned().unwrap_or_default(),
        );
        if let Err(words) = ask_cli(&who, &message) {
            eprintln!("{words}");
            std::process::exit(1);
        }
        return;
    }

    // Quick Ask: the floating panel behind a global chord (round 6).
    if args.get(1).map(String::as_str) == Some("--quick") {
        quick::run();
        return;
    }
    if args.windows(2).any(
        |pair| matches!(pair, [flag, scene] if flag == "--scene" && scene.starts_with("quick-")),
    ) {
        rune::run(quick::scenes());
        return;
    }
    if args.iter().any(|arg| arg == "--list-scenes") {
        for (name, _) in quick::scenes().scenes {
            println!("{name}");
        }
    }

    App::new(AppSpec {
        min_size: Some((720.0, 480.0)),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        scenes: &[
            ("terminal", terminal_panel::scene),
            ("terminal-light", terminal_panel::light_scene),
            ("undo", changes_panel::undo_scene),
            ("undo-light", changes_panel::undo_light_scene),
            ("commit", commit_panel::scene),
            ("commit-light", commit_panel::light_scene),
            ("commit-done", commit_panel::done_scene),
            ("changes", changes_panel::scene),
            ("changes-light", changes_panel::light_scene),
            ("thread-rich", thread_rich_scene),
            ("thread-rich-open", thread_rich_open_scene),
            ("thread-rich-open-light", thread_rich_open_light_scene),
            ("thread-rich-light", thread_rich_light_scene),
            ("people", p6_people_scene),
            ("people-light", p6_people_light_scene),
            ("memory", p6_memory_scene),
            ("memory-light", p6_memory_light_scene),
            ("team", team_scene),
            ("empty", empty_scene),
            ("replay", replay_scene),
            ("team-light", team_light_scene),
            ("empty-light", empty_light_scene),
            ("replay-light", replay_light_scene),
            ("ledger-transcript", ledger_transcript_scene),
            ("ledger-live", ledger_live_scene),
            ("ledger-transcript-light", ledger_transcript_light_scene),
            ("ledger-live-light", ledger_live_light_scene),
            ("native", native_scene_idle),
            ("native-live", native_scene_live),
            ("approval", approval_scene_dark),
            ("approval-light", approval_scene_light),
            ("approval-reach", approval_reach_scene_dark),
            ("approval-reach-light", approval_reach_scene_light),
            ("approval-rules", p8_rules_scene_dark),
            ("approval-rules-light", p8_rules_scene_light),
            ("routines", routines_panel::scene),
            ("routines-light", routines_panel::light_scene),
            ("settings", settings_panel::scene),
            ("settings-light", settings_panel::light_scene),
            ("usage", usage_panel::scene),
            ("usage-light", usage_panel::light_scene),
            ("keyboard", keyboard_sheet::scene),
            ("keyboard-light", keyboard_sheet::light_scene),
            ("keychain-note", keychain_note_scene_dark),
            ("keychain-note-light", keychain_note_scene_light),
            ("composer", composer_scene),
            ("composer-light", composer_light_scene),
            ("composer-slash", composer_slash_scene),
            ("composer-slash-light", composer_slash_light_scene),
            ("composer-mention", composer_mention_scene),
            ("composer-mention-light", composer_mention_light_scene),
            ("composer-queue", composer_queue_scene),
            ("composer-queue-light", composer_queue_light_scene),
            ("composer-blocked", composer_blocked_scene),
            ("composer-blocked-light", composer_blocked_light_scene),
            ("reply-code", p5_code_scene),
            ("reply-code-light", p5_code_light_scene),
            ("file-viewer", p5_file_scene),
            ("file-viewer-light", p5_file_light_scene),
            ("recovery", p5_recovery_scene),
            ("recovery-light", p5_recovery_light_scene),
            ("org-sidebar", org_sidebar_scene),
            ("org-sidebar-light", org_sidebar_light_scene),
            ("org-search", org_search_scene),
            ("org-search-light", org_search_light_scene),
            ("org-find", org_find_scene),
            ("org-find-light", org_find_light_scene),
            ("org-palette", org_palette_scene),
            ("org-palette-light", org_palette_light_scene),
            ("org-beside", org_beside_scene),
            ("org-beside-light", org_beside_light_scene),
            ("home", home_panel::home_scene),
            ("home-light", home_panel::home_light_scene),
            ("board", home_panel::board_scene),
            ("board-light", home_panel::board_light_scene),
            ("boot", home_panel::boot_scene),
            ("boot-light", home_panel::boot_light_scene),
            ("away", home_panel::away_scene),
            ("away-light", home_panel::away_light_scene),
            ("whatsnew", home_panel::whatsnew_scene),
            ("whatsnew-light", home_panel::whatsnew_light_scene),
        ],
        ..AppSpec::new("Locust", (1040.0, 720.0), State::default, view)
    })
    .chrome(Chrome {
        full_size_content: true,
        traffic_lights: Some((20.0, 18.0)),
        drag_regions: true,
    })
    .on_close(on_close)
    .run();
    // The window is gone: let cancelled records land (bounded; an unresponsive server is killed by its watchdog).
    native::wait_for_shutdown();
}

#[cfg(test)]
#[path = "ui/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ui/daily_controls_tests.rs"]
mod daily_controls_tests;

#[cfg(test)]
#[path = "ui/native_window_tests.rs"]
mod native_window_tests;

#[cfg(test)]
#[path = "ui/approval_review_tests.rs"]
mod approval_review_tests;
