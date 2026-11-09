mod engine;
mod ledger;
mod quick;
mod store;
mod ledger_write;
mod runtime;
mod run_cli;

use engine::{ConversationRow, Engine, Reply, Started, Teammate};
use rune::tok::type_;
use rune::{
    Align, App, AppSpec, Chrome, Color, Cycle, Cx, El, Key, KeyEvent, Latest,
    ScrollSpec, TextEdit, Tx, Waker, col, el, row, text, text_input,
};
use rune_kit::{Markdown, MarkdownStyle};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn color(value: &str) -> Color {
    rune::color::hex(value).expect("valid Locust colour")
}

#[derive(Clone, Copy)]
enum StatusTone {
    Muted,
    Green,
    Amber,
    Red,
}

struct Palette {
    dark: bool,
    bg: Color,
    sidebar: Color,
    raised: Color,
    selected: Color,
    your_bubble: Color,
    field: Color,
    hairline: Color,
    text: Color,
    secondary: Color,
    muted: Color,
    green: Color,
    amber: Color,
    red: Color,
}

impl Palette {
    fn new(dark: bool) -> Self {
        let tokens = if dark {
            [
                "#121415", "#17191a", "#191c1d", "#1e2223", "#23282a",
                "#1a1d1e", "#f2efe9", "#c7c3bc", "#8b9190",
                "#7fc97f", "#e9b949", "#e4685d",
            ]
        } else {
            [
                "#faf9f6", "#f1f0ec", "#ffffff", "#e7e6e1", "#ebeae5",
                "#ffffff", "#1c1e1d", "#3a3e3d", "#5f6563",
                "#2e8540", "#b8790a", "#c9443a",
            ]
        };
        let [
            bg, sidebar, raised, selected, your_bubble, field,
            text, secondary, muted, green, amber, red,
        ] = tokens.map(color);

        Self {
            dark,
            bg,
            sidebar,
            raised,
            selected,
            your_bubble,
            field,
            hairline: if dark {
                Color::WHITE.alpha(0.06)
            } else {
                text.alpha(0.08)
            },
            text,
            secondary,
            muted,
            green,
            amber,
            red,
        }
    }

    fn status(&self, tone: StatusTone) -> Color {
        match tone {
            StatusTone::Muted => self.muted,
            StatusTone::Green => self.green,
            StatusTone::Amber => self.amber,
            StatusTone::Red => self.red,
        }
    }
}

#[derive(Default)]
struct Conversation {
    conversation_id: Option<String>,
    turns: Vec<(String, String)>,
}

// Fix 2: tag each flight with its teammate ID, conversation ID, header, and tip so stale results are dropped.
struct Flight {
    teammate_id: String,
    conversation_id: Option<String>,
    header: Option<Teammate>,
    after_tip: Option<String>,
    // Some only until start/send lands; reply failures must not restore sent text.
    message: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct TurnPick { model: String, effort: Option<String> }

struct OwnTurn { conversation: String, tip: Option<String>, before: Option<String> }

struct State {
    teammates: Vec<Teammate>,
    selected: usize,
    // Fix: histories and conversation IDs survive team reordering and reloads.
    conversations: HashMap<String, Conversation>,
    draft: TextEdit,
    picks: HashMap<String, TurnPick>,
    models: HashMap<String, Vec<engine::Models>>,
    model_menu: Option<String>,
    model_job: Latest<(String, Result<Vec<engine::Models>, String>)>,
    own_turn: Option<OwnTurn>,
    stopping: Latest<(String, Result<String, String>)>,
    status: String,
    status_tone: StatusTone,
    load_error: Option<String>,
    flight: Option<Flight>,
    demo: bool,
    // None follows the system; snapshot scenes explicitly choose an appearance.
    appearance: Option<bool>,
    activity_open: HashMap<String, bool>,
    recent: Vec<ConversationRow>,
    recent_error: Option<String>,
    opening_teammate: Option<String>,
    opening_conversation: Option<String>,
    waiting_target: Option<(String, String)>,
    recents: Latest<Result<RecentBatch, String>>,
    ledger_summaries: HashMap<String, ledger::Summary>,
    ledger_available: bool,
    ledger: Option<LedgerPane>,
    ledger_job: Latest<LedgerAnswer>,
    opening: Latest<Result<Reply, String>>,
    team: Latest<Result<Vec<Teammate>, String>>,
    sending: Latest<Result<Started, String>>,
    waiting: Latest<Result<Reply, String>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            teammates: Vec::new(),
            selected: 0,
            conversations: HashMap::new(),
            draft: TextEdit::new(""),
            picks: HashMap::new(),
            models: HashMap::new(),
            model_menu: None,
            model_job: Latest::default(),
            own_turn: None,
            stopping: Latest::default(),
            status: "Loading teammates…".into(),
            status_tone: StatusTone::Muted,
            load_error: None,
            flight: None,
            demo: false,
            appearance: None,
            activity_open: HashMap::new(),
            recent: Vec::new(),
            recent_error: None,
            opening_teammate: None,
            opening_conversation: None,
            waiting_target: None,
            recents: Latest::default(),
            ledger_summaries: HashMap::new(),
            ledger_available: false,
            ledger: None,
            ledger_job: Latest::default(),
            opening: Latest::default(),
            team: Latest::default(),
            sending: Latest::default(),
            waiting: Latest::default(),
        }
    }
}

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
type LedgerAnswer = Result<
    (ledger::Session, ledger::Snapshot),
    (ledger::Session, String),
>;

fn ledger_dir() -> Option<std::path::PathBuf> {
    engine::default_data_folder().map(|dir| dir.join("mission-ledger"))
}

// Fix 2: check whether a request's (teammate_id, conversation_id) tag still matches the open view.
fn active_matches(
    s: &State,
    teammate_id: &str,
    conversation_id: Option<&str>,
) -> bool {
    if let Some(pane) = &s.ledger {
        pane.header.id == teammate_id
            && (conversation_id.is_none()
                || pane.conversation.conversation_id.as_deref() == conversation_id)
    } else {
        s.teammates.get(s.selected).is_some_and(|t| t.id == teammate_id)
            && (conversation_id.is_none()
                || s.conversations.get(teammate_id)
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
        Some(mut prev)
            if prev.conversation.conversation_id.as_deref() == Some(id.as_str()) =>
        {
            let worker = prev.worker.take().unwrap_or_else(|| {
                ledger::Session::new(dir.clone(), id.clone())
            });
            (prev.items, worker, prev.snapshot)
        }
        _ => (
            Vec::new(),
            ledger::Session::new(dir.clone(), id.clone()),
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
    }.into();
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
        || s.flight.as_ref().is_some_and(|f| f.teammate_id == LEDGER_TARGET)
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
    leave_ledger(s);
    s.model_menu = None;
    // Fix 2: cancel any in-flight request from the previous conversation when switching teammates.
    if s.selected != index || s.flight.is_some() || s.opening_teammate.is_some() {
        detach(s);
    }
    s.selected = index;
    if s.ledger_available {
        let header = s.teammates.get(index).cloned();
        let conversation = s.conversations.get(id)
            .and_then(|c| c.conversation_id.clone());
        if let (Some(header), Some(conversation)) = (header, conversation) {
            begin_ledger(s, conversation, String::new(), header, None, tx);
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
            Ok((worker, snapshot)) => {
                // Fix 2: ignore a worker result if the open ledger changed while the job was running.
                if let Some(pane) = s.ledger.as_mut()
                    && pane.conversation.conversation_id.as_deref()
                        == Some(worker.anchor())
                {
                    changed = pane.loading || pane.snapshot != snapshot;
                    let finished = pane.snapshot.running && !snapshot.running;
                    let new_tip = pane.snapshot.tip != snapshot.tip;
                    let waiting_for_child = pane.after_tip.is_some();
                    if pane.after_tip.is_some() && pane.after_tip_deadline.is_none() {
                        pane.after_tip_deadline = Some(now + 30.0);
                    }
                    if pane.after_tip.as_ref().is_some_and(|tip| {
                        tip != &snapshot.tip || snapshot.running
                    }) {
                        pane.after_tip = None;
                        pane.after_tip_deadline = None;
                    }
                    // Fix 3: bound after_tip waiting to 30 s, then show "No new reply yet" and unlock Send.
                    let mut timed_out = false;
                    if pane.after_tip.is_some()
                        && pane.after_tip_deadline.is_some_and(|d| now >= d)
                    {
                        pane.after_tip = None;
                        pane.after_tip_deadline = None;
                        timed_out = true;
                        changed = true;
                    }
                    let active = snapshot.running || pane.after_tip.is_some();
                    if let Some(own) = s.own_turn.as_mut()
                        && pane.conversation.conversation_id.as_deref() == Some(own.conversation.as_str())
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
                        f.teammate_id == pane.header.id
                            || f.teammate_id == LEDGER_TARGET
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
                                }.into();
                            }
                        }
                    }
                    if finished || new_tip {
                        refresh_recent(s);
                    }
                    // stick_bottom follows new text while the listener stays at the bottom.
                }
            }
            Err((worker, words)) => {
                if let Some(pane) = s.ledger.as_mut()
                    && pane.conversation.conversation_id.as_deref()
                        == Some(worker.anchor())
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
        let pollable = pane.loading
            || pane.snapshot.running
            || pane.after_tip.is_some()
            || pane.backoff > 0.0;
        if pollable {
            if now >= pane.due {
                if let Some(mut worker) = pane.worker.take() {
                    s.ledger_job.request((), move |()| {
                        match worker.poll() {
                            Ok(snapshot) => Ok((worker, snapshot)),
                            Err(_) => Err((
                                worker,
                                "Local conversation could not be read.".to_owned(),
                            )),
                        }
                    });
                }
            } else if pane.worker.is_some() {
                tx.wake_after(Duration::from_secs_f64((pane.due - now).max(0.0)));
            }
        }
    }
    changed
}

fn disclosure(key: String, label: String, open: bool, p: &Palette) -> El {
    el().w_full().padding(8.0).radius(5.0)
        .child(text(format!("{} {label}", if open { "▾" } else { "▸" })).font(type_::LABEL).color(p.muted).w_full().shrink().wrap_anywhere())
        .on_press(move |s: &mut State| { s.activity_open.insert(key.clone(), !open); })
}

fn tool_result(status: ledger::Status, d: &ledger::ToolDetail, finished: bool) -> String {
    let finished=finished || d.finished;
    if status == ledger::Status::Running { return (if finished { "did not report" } else if d.helper { "working on it" } else { "running" }).into(); }
    if matches!(d.runtime_status.as_str(), "declined"|"refused") { return d.runtime_status.clone(); }
    if status == ledger::Status::Failed { return if !d.helper && let Some(code)=d.exit_code { format!("failed · exit {code}") } else { "failed".into() }; }
    if d.background { return match d.background_ended.as_str() {
        "completed" => if d.helper { "reported back" } else { "done" }, "failed"=>"failed", "stopped"=>"stopped",
        "stopped-with-run"=>if d.helper {"stopped when the run ended"} else {"stopped"}, "ended"=>"ended",
        _=>if finished {"did not report"} else if d.helper {"working in the background"} else {"running"},
    }.into(); }
    if d.helper { "reported back".into() } else if let Some(code)=d.exit_code {format!("exit {code}")} else {"done".into()}
}

fn is_shell_tool(name: &str) -> bool {
    matches!(name.to_lowercase().as_str(), "bash"|"shell"|"exec_command"|"commandexecution")
}

fn thought_headline(text: &str) -> Option<&str> {
    text.trim_start().strip_prefix("**").and_then(|rest| rest.split_once("**"))
        .and_then(|(headline, _)| ((2..=80).contains(&headline.len()) && !headline.contains('\n')).then_some(headline))
}

fn without_headline(text: &str) -> &str {
    match thought_headline(text) {
        Some(headline) => {
            let marker = format!("**{headline}**");
            text.find(&marker).map(|at| text[at + marker.len()..].trim_start()).unwrap_or(text)
        }
        None => text,
    }
}

fn tool_row_label(name: &str, status: ledger::Status, summary: &str, d: &ledger::ToolDetail, finished: bool) -> String {
    let result = tool_result(status, d, finished);
    let title = if !summary.is_empty() { summary } else if !d.arguments.is_empty() { &d.arguments } else { name };
    let mut label = if d.helper {
        let mut row = format!("{title} · {}subagent", if d.helper_type.is_empty() { String::new() } else { format!("{} ", d.helper_type) });
        if !d.children.is_empty() {
            row.push_str(&format!(" · {} {}", d.children.len(), if d.children.len() == 1 { "call" } else { "calls" }));
        }
        row.push_str(&format!(" · {result}"));
        row
    } else {
        let tool_word = match name.to_lowercase().as_str() {
            "read"|"read_file"|"readfile"|"view_file"|"cat"=>"Read",
            "write"|"write_file"|"writefile"=>"Write",
            "edit"|"edit_file"|"editfile"|"apply_patch"=>"Edit",
            "grep"|"grep_search"|"ripgrep"=>"Grep",
            "glob"|"list_dir"|"list_directory"|"ls"=>"Glob",
            "bash"|"shell"|"exec_command"|"commandexecution"=>"",
            _=>name,
        };
        if tool_word.is_empty() { format!("{title} · {result}") } else { format!("{title} · {tool_word} · {result}") }
    };
    if d.helper && status == ledger::Status::Done && !d.output.trim().is_empty() && result == "reported back" {
        label.push_str(&format!(" · {}", d.output.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect::<String>()));
    }
    if result == "working in the background" && !d.progress.is_empty() {
        label.push_str(&format!(" · {}", d.progress));
    }
    // BackgroundBadge is shell-only (ActivityCard.tsx:1298); helpers already say it in tool_result.
    if d.background && !d.helper {
        label.push_str(if d.background_ended == "stopped-with-run" { " · when the run ended" } else { " · in the background" });
    }
    if is_shell_tool(name) && d.output_reported && d.output.trim().is_empty() && status != ledger::Status::Running {
        label.push_str(" · no output");
    }
    let refused = matches!(d.runtime_status.as_str(), "declined"|"refused");
    if is_shell_tool(name) && !d.background && !refused && status != ledger::Status::Running
        && let Some(ms) = d.duration_ms.filter(|ms| *ms >= 1000)
    {
        label.push_str(&format!(" · {}", ledger::duration_text(ms)));
    }
    label
}

fn activity_drawer_key(scope: &str, item: &ledger::Item) -> String {
    match item {
        ledger::Item::Plan { id, .. } => format!("{scope}/plan/{id}"),
        ledger::Item::Thought { id, .. } => format!("{scope}/{id}"),
        ledger::Item::Tool { detail, .. } => format!("{scope}/{}", detail.id),
        _ => format!("{scope}/item"),
    }
}

fn activity_item(item: &ledger::Item, p: &Palette, s: &State, scope: &str) -> El {
    let finished = s.ledger.as_ref().is_some_and(|pane| !pane.snapshot.running);
    let label = match item {
        ledger::Item::Tool { name, status, summary, detail: d } => tool_row_label(name, *status, summary, d, finished),
        ledger::Item::Plan { steps, finished: ended, .. } => {
            let finished=finished || *ended;
            let done=steps.iter().filter(|r| r.state=="done").count();
            format!("PLAN · {done} of {} done{}", steps.len(), if finished && done<steps.len() { format!(" · {} not checked off",steps.len()-done) } else { String::new() })
        },
        ledger::Item::Thought { text, duration_ms, .. } => {
            let mut label = duration_ms.filter(|ms| *ms >= 500).map(|ms| format!("Thought for {}",ledger::duration_text(ms))).unwrap_or_else(|| "Thought".into());
            if let Some(headline) = thought_headline(text) { label.push_str(&format!(" · {headline}")); }
            label
        },
        _ => return col(),
    };
    let key = activity_drawer_key(scope, item);
    let open = s.activity_open.get(&key).copied().unwrap_or(matches!(item,ledger::Item::Plan { .. }));
    let expandable = match item {
        ledger::Item::Thought { text, .. } => !text.is_empty(),
        ledger::Item::Tool { detail, .. } => !detail.arguments.is_empty() || !detail.output.is_empty() || !detail.children.is_empty(),
        ledger::Item::Plan { steps, .. } => !steps.is_empty(), _=>false,
    };
    let heading = if expandable { disclosure(key.clone(),label,open,p) }
        else { el().w_full().padding(8.0).child(text(label).font(type_::LABEL).color(p.muted).w_full().shrink().wrap_anywhere()) };
    let mut card = col().w_full().gap(4.0);
    if matches!(item,ledger::Item::Plan { .. }) { card=card.radius(7.0).border(1.0,p.hairline); }
    card=card.child(heading);
    if open {
        let mut body = col().w_full().padding(10.0).gap(8.0);
        match item {
            ledger::Item::Tool { detail: d, .. } => {
                if !d.arguments.is_empty() { body = body.child(text(d.arguments.clone()).font(type_::LABEL).color(p.text).wrap_anywhere().selectable(true)); }
                if !d.output.is_empty() {
                    let all_key = format!("{key}/output");
                    let all = s.activity_open.get(&all_key).copied().unwrap_or(false);
                    let output=ledger::bounded_output(&d.output,all);
                    let lines: Vec<_>=d.output.trim_end().split('\n').collect();
                    if !all && lines.len()>16 {
                        body=body.child(text(lines[..8].join("\n")).font(type_::LABEL).color(p.muted).w_full().shrink().wrap_anywhere().selectable(true));
                        body=body.child(disclosure(all_key,format!("{} more lines",lines.len()-16),false,p));
                        body=body.child(text(lines[lines.len()-8..].join("\n")).font(type_::LABEL).color(p.muted).w_full().shrink().wrap_anywhere().selectable(true));
                    } else { body=body.child(text(output).font(type_::LABEL).color(p.muted).w_full().shrink().wrap_anywhere().selectable(true)); }
                }
                for child in &d.children { body=body.child(activity_item(child,p,s,&key)); }
            }
            ledger::Item::Plan { steps, finished: ended, .. } => {
                let finished=finished || *ended;
                for (n, step) in steps.iter().enumerate() {
                    let mark = if step.state=="done" {"✓"} else if step.state=="running" && !finished {"●"} else {"○"};
                    body=body.child(text(format!("{mark} {}. {}", n+1, step.text)).font(type_::LABEL).color(if step.state=="running" && !finished {p.text} else {p.muted}).wrap_anywhere());
                }
            }
            ledger::Item::Thought { text: words, .. } => {
                // Multi-item thought rows keep the full text under the headline button (ActivityCard.tsx:866).
                if !words.is_empty() {
                    body=body.child(Markdown::new(words.clone()).selectable(true).render(&markdown_style(p)).w_full());
                }
            }
            _=>{}
        }
        card=card.child(body);
    }
    card
}

fn is_activity(item: &ledger::Item) -> bool {
    matches!(item, ledger::Item::Tool { .. } | ledger::Item::Thought { .. })
}
fn activity_id(item: &ledger::Item) -> &str {
    match item { ledger::Item::Tool { detail, .. } => &detail.id, ledger::Item::Thought { id, .. } | ledger::Item::Plan { id, .. } => id, _ => "" }
}
fn activity_group(items: &[ledger::Item], p: &Palette, s: &State, scope: &str) -> El {
    let key = format!("{scope}/group/{}",activity_id(&items[0]));
    let open = s.activity_open.get(&key).copied().unwrap_or(false);
    let mut commands=0; let mut helpers=0; let mut tools=0; let mut thought_ms=0; let mut headline=None;
    for item in items {
        match item {
            ledger::Item::Tool { name, detail, .. } => {
                if detail.helper { helpers+=1; }
                else if matches!(name.to_lowercase().as_str(), "bash"|"shell"|"exec_command"|"commandexecution") { commands+=1; } else { tools+=1; }
            }
            ledger::Item::Thought {text,duration_ms,..} => {
                thought_ms+=duration_ms.unwrap_or(0);
                if let Some(h)=thought_headline(text) { headline=Some(h.to_owned()); }
            }
            _=>{}
        }
    }
    let mut words=Vec::new();
    if commands>0 { words.push(format!("ran {commands} {}",if commands==1 {"command"} else {"commands"})); }
    if helpers>0 { words.push(format!("asked {helpers} {}",if helpers==1 {"subagent"} else {"subagents"})); }
    if tools>0 { words.push(format!("{tools} other tool {}",if tools==1 {"call"} else {"calls"})); }
    let mut label=headline.unwrap_or_else(|| if words.is_empty() { if thought_ms>=1000 {format!("Thought for {}",ledger::duration_text(thought_ms))} else {"Thought".into()} } else { words.join(", ") });
    if let Some(first)=label.chars().next() { label=format!("{}{}",first.to_uppercase(),&label[first.len_utf8()..]); }
    let heading = if matches!(items,[ledger::Item::Thought {text,..}] if text.is_empty()) {
        el().w_full().padding(8.0).child(text(label).font(type_::LABEL).color(p.muted))
    } else { disclosure(key,label,open,p) };
    let mut group=col().w_full().gap(4.0).child(heading);
    if open {
        if let [ledger::Item::Thought {text: words,..}]=items {
            // Single-thought groups already named the headline on the button (ActivityCard.tsx:535).
            let body = without_headline(words);
            if !body.is_empty() { group=group.child(Markdown::new(body.to_owned()).selectable(true).render(&markdown_style(p)).w_full()); }
        } else { for item in items { group=group.child(activity_item(item,p,s,scope)); } }
    }
    group
}

fn message_column(items: &[ledger::Item], name: &str, p: &Palette, s: &State) -> El {
    let mut messages = col().w_full().gap(24.0).padding(24.0);
    let style = markdown_style(p);
    for (index, item) in items.iter().enumerate() {
        let conversation = s.ledger.as_ref().and_then(|p|p.conversation.conversation_id.as_deref()).unwrap_or("preview");
        let key = format!("{conversation}/ledger-item-{index}");
        if matches!(item,ledger::Item::Plan {steps,..} if steps.is_empty()) {continue;}
        if is_activity(item) {
            if index>0 && is_activity(&items[index-1]) { continue; }
            let end=items[index..].iter().position(|i| !is_activity(i)).map(|n|index+n).unwrap_or(items.len());
            messages=messages.child(activity_group(&items[index..end],p,s,conversation));
            continue;
        }
        let content = match item {
            ledger::Item::CarryOn { conversation_id } => {
                if s.flight.is_some()
                    || s.sending.is_pending()
                    || s.ledger.as_ref().is_some_and(|pane| pane.snapshot.running || pane.after_tip.is_some())
                {
                    continue;
                }
                let id = conversation_id.clone();
                let mut card = col().w_full().gap(8.0).padding(12.0).radius(9.0).fill(p.raised)
                    .child(text("Carry on with").font(type_::LABEL).color(p.text))
                    .child(el().padding(8.0).child(text("Choose AI agent and model").color(p.text))
                        .on_press(|s: &mut State| open_models(s, CARRY_TARGET.into())));
                if s.model_menu.as_deref() == Some(CARRY_TARGET) {
                    if let Some(agents) = s.models.get(CARRY_TARGET).filter(|a| !a.is_empty()) {
                        let mut list = col().key(format!("{key}-carry-menu"))
                            .w_full().max_h(180.0)
                            .scroll(ScrollSpec { stick_bottom: false })
                            .gap(4.0);
                        for agent in agents { for model in &agent.models {
                            let (id, runtime, model_id) = (id.clone(), agent.runtime.clone(), model.id.clone());
                            list = list.child(el().padding(8.0).disabled(s.flight.is_some())
                                .child(text(format!("{} · {}", engine::runtime_name(&runtime), model.name)).color(p.text))
                                .on_press(move |s: &mut State| carry(s, id.clone(), runtime.clone(), model_id.clone())));
                        }}
                        card = card.child(list);
                    } else { card = card.child(text("Loading models…").color(p.muted)); }
                }
                card
            }
            ledger::Item::User(body) => row().w_full().justify_end()
                .child(
                    row().w_frac(0.7).justify_end().child(
                        text(body.clone()).font(type_::BODY).color(p.text)
                            .shrink().wrap_anywhere().selectable(true)
                            .padding(12.0).radius(10.0).fill(p.your_bubble)
                            .border(1.0, p.hairline),
                    ),
                ),
            ledger::Item::Assistant { text: body, r#final, .. } => {
                col().w_full().gap(8.0)
                    .child(text(name).font(type_::LABEL).color(p.muted))
                    .child(
                        Markdown::new(body.clone()).streaming(!*r#final)
                            .selectable(true).render(&style)
                            .key(format!("{key}-markdown")).w_full(),
                    )
            }
            ledger::Item::Tool { .. } | ledger::Item::Plan { .. } | ledger::Item::Thought { .. } => {
                activity_item(item, p, s, conversation)
            }
            ledger::Item::Note { level, text: body } => {
                text(if level.is_empty() { body.clone() } else { format!("{level}: {body}") })
                    .font(type_::LABEL).color(p.muted)
                    .w_full().wrap_anywhere().selectable(true)
            }
        };
        messages = messages.child(content.key(key));
    }
    messages
}

fn load_team(s: &mut State) {
    if s.demo || s.team.is_pending() || s.flight.is_some() {
        return;
    }
    s.load_error = None;
    s.status = "Loading teammates…".into();
    s.status_tone = StatusTone::Muted;
    s.team.request((), |()| Engine::local()?.teammates());
    refresh_recent(s);
}

fn setup(s: &mut State, waker: Waker) {
    s.team.set_waker(waker.clone());
    s.sending.set_waker(waker.clone());
    s.waiting.set_waker(waker.clone());
    s.recents.set_waker(waker.clone());
    s.opening.set_waker(waker.clone());
    s.model_job.set_waker(waker.clone());
    s.stopping.set_waker(waker.clone());
    s.ledger_job.set_waker(waker);
    load_team(s);
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
        && !s.draft.text.trim().is_empty()
}

fn set_status(s: &mut State, id: &str, message: &str, ink: StatusTone) {
    let name = if id == LEDGER_TARGET {
        s.ledger.as_ref().map(|p| p.header.name.as_str()).unwrap_or("Locust")
    } else {
        s.teammates.iter().find(|t| t.id == id)
            .map(|t| t.name.as_str())
            .or_else(|| {
                s.ledger.as_ref()
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
    let after_tip = s.ledger.as_ref()
        .map(|p| p.snapshot.tip.clone())
        .filter(|v| !v.is_empty());
    let message = s.draft.text.clone();
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
    s.sending.request(
        (teammate_id, conversation_id, message, pick),
        |(id, conversation, message, pick)| {
            let model = pick.as_ref().map(|p| p.model.as_str());
            let effort = pick.as_ref().and_then(|p| p.effort.as_deref());
            let engine = Engine::local()?;
            match conversation {
                Some(id) => engine.send(&id, &message, model, effort),
                None => engine.start(&id, &message, model, effort),
            }
        },
    );
}

fn take_turn_pick(s: &mut State, teammate: &str) -> Option<TurnPick> {
    s.picks.remove(teammate)
}

fn open_models(s: &mut State, id: String) {
    if s.demo { return; }
    if s.model_menu.as_ref() == Some(&id) { s.model_menu = None; return; }
    if s.models.get(&id).is_some_and(Vec::is_empty) {
        s.models.remove(&id);
    }
    s.model_menu = Some(id.clone());
    s.model_job.request(id, |id| {
        let result = Engine::local().and_then(|e| e.list_models(if id == LEDGER_TARGET || id == CARRY_TARGET { None } else { Some(&id) }));
        (id, result)
    });
}

fn can_stop(s: &State) -> bool {
    if s.demo || s.sending.is_pending() || s.flight.is_none() { return false; }
    let Some(own) = &s.own_turn else { return false; };
    let Some((_, c)) = selected_conversation(s) else { return false; };
    c.conversation_id.as_deref() == Some(own.conversation.as_str())
        && s.ledger.as_ref().is_none_or(|p| p.snapshot.running && own.tip.as_deref() == Some(p.snapshot.tip.as_str()))
}

fn stop(s: &mut State) {
    if !can_stop(s) || s.stopping.is_pending() { return; }
    if let Some(own) = &s.own_turn {
        s.stopping.request(own.conversation.clone(), |id| {
            let result = Engine::local().and_then(|e| e.stop_turn(&id));
            (id, result)
        });
    }
}

fn carry(s: &mut State, id: String, runtime: String, model: String) {
    if s.flight.is_some() || s.sending.is_pending() { return; }
    let Some((header, c)) = selected_conversation(s) else { return; };
    let header_id = header.id.clone();
    let conversation_id = c.conversation_id.clone();
    let header = header.clone();
    let after_tip = s.ledger.as_ref()
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
    s.sending.request((id, runtime, model), |(id, runtime, model)| Engine::local()?.carry_on(&id, &runtime, &model));
}

fn refresh_recent(s: &mut State) {
    if s.demo {
        return;
    }
    s.recents.request((), |()| {
        if let Some(dir) = ledger_dir() {
            if let Ok(summaries) = ledger::try_list_recent(&dir, 12) {
                let rows = summaries.iter().map(|m| ConversationRow {
                    id: m.mission_id.clone(),
                    teammate: engine::runtime_name(&m.runtime).into(),
                    title: m.title.clone(),
                    started_by: "ledger".into(),
                    state: if m.running {
                        "running"
                    } else if m.failed {
                        "failed"
                    } else {
                        "completed"
                    }.into(),
                    updated: m.updated.clone(),
                }).collect();
                return Ok(RecentBatch {
                    rows,
                    summaries: summaries.into_iter()
                        .map(|m| (m.mission_id.clone(), m)).collect(),
                    local: true,
                });
            }
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
    let conversation_id = s.conversations.get(teammate_id)
        .and_then(|c| c.conversation_id.clone())
        .filter(|id| !id.is_empty());

    let Some(id) = conversation_id else {
        // Fix: missing IDs end the flight instead of leaving Send locked forever.
        tracking_error(s, teammate_id, "No active conversation ID; stopped watching");
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
        let same_runtime = team.iter()
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
    s.selected = selected.as_deref()
        .and_then(|id| s.teammates.iter().position(|t| t.id == id))
        .unwrap_or(0);
    s.load_error = None;

    let active = s.flight.as_ref().map(|f| f.teammate_id.clone())
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
        }.into();
        s.status_tone = StatusTone::Muted;
    }
}

fn open_recent(s: &mut State, row: ConversationRow, tx: &mut Tx) {
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
    let Some(index) = s.teammates.iter()
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
    s.conversations.insert(id.clone(), Conversation {
        conversation_id: Some(row.id.clone()),
        turns: vec![("You".into(), row.title)],
    });
    set_status(s, &id, "Loading…", StatusTone::Muted);
    tx.scroll_to_bottom("messages");

    if !s.demo {
        // Fix 2: tag the opening request with both the teammate ID and conversation ID.
        s.opening_teammate = Some(id);
        s.opening_conversation = Some(row.id.clone());
        s.opening.request(row.id, |id| Engine::local()?.reply(&id, 0));
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
}

fn on_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.key == Key::Escape {
        if s.flight.is_some() || s.opening_teammate.is_some() {
            detach(s);
            s.status = "Stopped watching; the turn goes on in Locust".into();
            s.status_tone = StatusTone::Muted;
        } else {
            s.draft.clear();
        }
        return true;
    }
    if event.mods.shift || event.mods.alt {
        return false;
    }
    if event.is_command('n') {
        fresh_conversation(s);
        tx.scroll_to_bottom("messages");
        return true;
    }
    for digit in '1'..='9' {
        if event.is_command(digit) {
            if let Some(id) = s.teammates.get((digit as u8 - b'1') as usize)
                .map(|t| t.id.clone())
            {
                select_teammate(s, &id, tx);
            }
            return true;
        }
    }
    false
}

// Locust supplies UTC ISO timestamps. Fractional seconds are irrelevant here.
fn parse_updated(value: &str) -> Option<(i64, usize, u32)> {
    let utc = value.strip_suffix('Z')
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
    let (hour, minute, second) =
        (number(11, 13)?, number(14, 16)?, number(17, 19)?);

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
        31i64, if leap { 29 } else { 28 }, 31, 30, 31, 30,
        31, 31, 30, 31, 30, 31,
    ];
    let month = month as usize;
    if !(1..=lengths[month - 1]).contains(&day) {
        return None;
    }
    let y = year - 1;
    let days = 365 * y + y / 4 - y / 100 + y / 400
        + lengths[..month - 1].iter().sum::<i64>()
        + day - 1 - 719_162;
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
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        format!("{} {day}", months[month - 1])
    }
}

fn clock_seconds() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

// Both opened recent threads and sent messages resolve their target when results land.
fn land_reply(
    s: &mut State,
    teammate_id: &str,
    result: Result<Reply, String>,
    tx: &mut Tx,
) {
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
                    s, teammate_id,
                    "Conversation is no longer available; stopped watching",
                );
                return;
            };
            conversation.turns.push((who, message));
            set_status(s, teammate_id, "Ready", StatusTone::Muted);
            s.own_turn = None;
            s.flight = None;
            refresh_recent(s);
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

fn restore_failed_draft(s: &mut State, message: String) {
    // Fix: restore only failed sends; never overwrite text typed after dispatch.
    let text = if s.draft.text.is_empty() {
        message
    } else {
        format!("{message}\n\n{}", s.draft.text)
    };
    s.draft.replace_text(text);
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    if s.demo {
        return false;
    }
    let mut changed = false;
    if let Some((id, result)) = s.model_job.take() {
        changed = true;
        match result {
            Ok(models) => { s.models.insert(id, models); }
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
                        if let Some(f) = &s.flight && let Some(c) = s.conversations.get_mut(&f.teammate_id) {
                            c.turns.push(("Locust".into(), "Stopped".into()));
                        }
                        s.flight = None;
                    }
                    s.own_turn = None;
                    s.status = "Stop requested".into();
                    s.status_tone = StatusTone::Muted;
                }
            }
            Err(error) => { s.status = error; s.status_tone = StatusTone::Red; }
        }
    }

    if !s.model_job.is_pending() && s.load_error.is_none() {
        let id = selected_conversation(s)
            .and_then(|(t, _)| t.runtime.as_ref().map(|_| t.id.clone()));
        if let Some(id) = id && id != LEDGER_TARGET && !s.models.contains_key(&id) {
            s.model_job.request(id, |id| {
                let result = Engine::local().and_then(|e| e.list_models(Some(&id)));
                (id, result)
            });
        }
    }

    if let Some(result) = s.team.take() {
        changed = true;
        match result {
            Ok(team) => apply_team(s, team),
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
        let matches_active = s.flight.as_ref().is_some_and(|f| {
            active_matches(s, &f.teammate_id, f.conversation_id.as_deref())
        });
        if matches_active
            && let Some(id) = s.flight.as_ref().map(|f| f.teammate_id.clone())
        {
            let message = s.flight.as_mut().and_then(|f| f.message.take());
            let flight_conv = s.flight.as_ref().and_then(|f| f.conversation_id.clone());
            let flight_header = s.flight.as_ref().and_then(|f| f.header.clone());
            let after = s.flight.as_ref().and_then(|f| f.after_tip.clone());
            match result {
                Ok(started) => {
                    let same_conv = flight_conv.as_deref() == Some(started.conversation_id.as_str());
                    let use_ledger = s.ledger_available || s.ledger.is_some();
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
                        let header = flight_header.or_else(|| {
                            s.teammates.iter().find(|t| t.id == id).cloned()
                        });

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
                            pane.worker = Some(ledger::Session::new(
                                pane.dir.clone(), id.clone(),
                            ));
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
                    && (flight_conv.is_none()
                        || flight_conv.as_deref() == Some(t_conv.as_str()))
            })
            && active_matches(s, &id, flight_conv.as_deref())
        {
            land_reply(s, &id, result, tx);
        }
    }

    changed | ledger_tick(s, now, tx)
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
}

fn agent(teammate: &Teammate) -> String {
    let runtime = teammate.runtime.as_deref()
        .map(engine::runtime_name).unwrap_or("Agent");
    match teammate.model.as_deref() {
        Some(model) => format!(
            "{runtime} · {}",
            model.rsplit('/').next().unwrap_or(model)
        ),
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
    let mut rows = col().w_full().gap(6.0).padding(10.0);

    for (index, teammate) in s.teammates.iter().enumerate() {
        let initial = teammate.name.trim_start().chars().next()
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_else(|| "?".into());
        let selected = s.ledger.as_ref()
            .map(|pane| pane.header.id == teammate.id)
            .unwrap_or(s.selected == index);
        let busy = teammate.busy
            || s.flight.as_ref().is_some_and(|f| f.teammate_id == teammate.id)
            || s.ledger.as_ref().is_some_and(|pane| {
                pane.header.id == teammate.id && pane.snapshot.running
            });

        let mut title = row().w_full().center_y().gap(8.0)
            .child(
                el().size(22.0, 22.0).radius(11.0).center().fill(p.raised)
                    .child(text(initial).font(type_::LABEL).color(p.secondary)),
            )
            .child(
                text(teammate.name.clone()).font(type_::TITLE)
                    .color(p.text).grow(1.0).shrink(),
            );
        if busy {
            title = title.child(
                el().size(6.0, 6.0).radius(3.0).fill(p.green),
            );
        }
        let id = teammate.id.clone();
        rows = rows.child(
            col().key(format!("teammate-{id}")).w_full()
                .padding(12.0).gap(5.0).radius(8.0)
                .fill(if selected { p.selected } else { p.sidebar })
                .child(title)
                .child(
                    text(teammate.role.clone()).font(type_::BODY)
                        .color(p.secondary).w_full().pl(30.0).wrap_anywhere(),
                )
                .child(
                    text(agent(teammate)).font(type_::LABEL)
                        .color(p.muted).w_full().pl(30.0).wrap_anywhere(),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    select_teammate(s, &id, tx);
                }),
        );
    }

    let mut recent = col().w_full().px(10.0).pb(16.0).gap(4.0)
        .child(
            row().w_full().center_y().px(12.0).py(10.0)
                .child(text("Recent").font(type_::LABEL)
                    .color(p.secondary).grow(1.0))
                .child(text("Refresh").key("refresh-recent").font(type_::LABEL)
                    .color(p.muted).on_press(refresh_recent)),
        );
    if let Some(words) = &s.recent_error {
        recent = recent.child(
            text(words.clone()).font(type_::LABEL).color(p.muted)
                .w_full().px(12.0).wrap_anywhere(),
        );
    } else if s.recent.is_empty() {
        recent = recent.child(
            text(if s.recents.is_pending() {
                "Loading…"
            } else {
                "No recent conversations."
            })
            .font(type_::LABEL).color(p.muted).w_full().px(12.0).wrap(),
        );
    } else {
        for item in s.recent.iter().take(12) {
            // Fix 5 (Finding 9): highlight Recent row when either the root conversation ID or active continuation tip matches.
            let picked = selected_conversation(s)
                .and_then(|(_, c)| c.conversation_id.as_deref())
                == Some(item.id.as_str())
                || s.ledger.as_ref().is_some_and(|p| {
                    !p.snapshot.tip.is_empty() && p.snapshot.tip == item.id
                });
            let mut title = row().w_full().center_y().gap(6.0)
                .child(
                    text(item.title.clone()).font(type_::BODY)
                        .color(p.text).grow(1.0).shrink().truncate(),
                );
            let dot = match item.state.as_str() {
                "running" => Some(p.green),
                "failed" => Some(p.red),
                _ => None,
            };
            if let Some(ink) = dot {
                title = title.child(
                    el().size(5.0, 5.0).radius(2.5).fill(ink),
                );
            }
            let item = item.clone();
            recent = recent.child(
                col().key(format!("recent-{}", item.id)).w_full()
                    .padding(12.0).gap(5.0).radius(8.0)
                    .fill(if picked { p.selected } else { p.sidebar })
                    .child(title)
                    .child(
                        text(format!(
                            "{} · {}",
                            item.teammate,
                            when(&item.updated, clock_seconds()),
                        ))
                        .font(type_::LABEL).color(p.muted).w_full().truncate(),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        open_recent(s, item.clone(), tx);
                    }),
            );
        }
    }

    let sidebar = col().w(248.0).h_full().fill(p.sidebar)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.hairline)
        .child(
            col().w_full().h(88.0).pt(48.0).pl(52.0).pr(20.0).window_drag()
                .child(text("LOCUST").font(type_::LABEL).color(p.secondary)),
        )
        .child(
            col().key("team-list").w_full().grow(1.0).min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(col().w_full().child(rows).child(recent)),
        );

    // Local history remains readable even when the MCP engine is unavailable.
    let error = s.load_error.as_ref().filter(|_| s.ledger.is_none());
    let right = if let Some(error) = error {
        col().grow(1.0).shrink().h_full().center().padding(32.0).gap(18.0)
            .child(
                text(error.clone()).font(type_::BODY).color(p.red)
                    .align(Align::Center).w_full().wrap_anywhere().selectable(true),
            )
            .child(
                el().key("retry").h(40.0).px(18.0).center().radius(8.0)
                    .fill(p.raised).border(1.0, p.hairline)
                    .child(text("Try again").font(type_::LABEL).color(p.text))
                    .on_press(load_team),
            )
    } else if let Some((teammate, conversation)) = selected_conversation(s) {
        let fallback;
        let items = if let Some(pane) = &s.ledger {
            &pane.items
        } else {
            fallback = conversation.turns.iter().enumerate()
                .map(|(i, (who, body))| {
                    if who == "You" {
                        ledger::Item::User(body.clone())
                    } else {
                        ledger::Item::Assistant {
                            item_id: format!("legacy-{i}"),
                            text: body.clone(),
                            r#final: true,
                        }
                    }
                }).collect::<Vec<_>>();
            &fallback
        };

        let transcript = if items.is_empty() {
            let title = if let Some(pane) = &s.ledger {
                if pane.loading {
                    "Loading conversation…".into()
                } else {
                    "No transcript items.".into()
                }
            } else {
                format!("Ask {} anything", teammate.name)
            };
            let mut empty = col().key("messages").w_full().grow(1.0).min_h(0.0)
                .center().padding(24.0).gap(10.0)
                .child(
                    text(title).font(type_::TITLE).color(p.text)
                        .w_full().align(Align::Center).wrap(),
                );
            if s.ledger.is_none() {
                empty = empty.child(
                    text(format!(
                        "{} · runs in {}",
                        agent(teammate),
                        mode_word(&teammate.runs_in),
                    ))
                    .font(type_::LABEL).color(p.muted)
                    .w_full().align(Align::Center).wrap_anywhere(),
                );
            }
            empty
        } else {
            col().key("messages").w_full().grow(1.0).min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .child(message_column(items, &teammate.name, &p, s))
        };

        let mut status = row().w_full().center_y().gap(8.0).px(24.0).py(10.0);
        if s.flight.is_some() || s.opening_teammate.is_some() {
            status = status.child(
                el().key("activity-dot").size(6.0, 6.0).radius(3.0)
                    .fill(p.status(s.status_tone))
                    .cycle(Cycle {
                        period: 1.6,
                        phase: 0.0,
                        stops: &[(0.0, 0.35), (0.5, 1.0), (1.0, 0.35)],
                        still: 1.0,
                        fps: 30.0,
                    }),
            );
        }
        status = status.child(
            text(s.status.clone()).font(type_::LABEL)
                .color(p.status(s.status_tone))
                .grow(1.0).shrink().wrap_anywhere().selectable(true),
        );

        let composer = text_input(&s.draft, type_::BODY)
            .key("composer").single_line()
            .placeholder(format!("Message {}", teammate.name), p.muted)
            .color(p.text).grow(1.0).shrink().h(48.0).padding(12.0)
            .radius(9.0).fill(p.field).border(1.0, p.hairline)
            .on_edit(|s: &mut State| &mut s.draft)
            .on_submit(send).on_input_key(on_key);
        let stopping = can_stop(s);
        let enabled = (can_send(s) || stopping) && !s.stopping.is_pending();
        let mut button = el().key("send").h(48.0).px(18.0).center().radius(9.0)
            .fill(p.raised).border(1.0, p.hairline).disabled(!enabled)
            .child(
                text(if stopping { "Stop" } else { "Send" }).font(type_::LABEL)
                    .color(if enabled { p.text } else { p.muted }),
            );
        if enabled {
            button = if stopping { button.on_press(stop) } else { button.on_press_tx(send) };
        }

        let id = teammate.id.clone();
        let current = s.models.get(&id).and_then(|agents| {
            agents.iter()
                .find(|a| teammate.runtime.as_deref() == Some(a.runtime.as_str()))
                .or_else(|| (id != LEDGER_TARGET && teammate.runtime.is_none() && agents.len() == 1).then(|| &agents[0]))
        });
        let picked = s.picks.get(&id);
        let model = picked.map(|p| p.model.as_str())
            .or_else(|| if id == LEDGER_TARGET { teammate.model.as_deref() } else { current.and_then(|a| a.model.as_deref()) })
            .or(teammate.model.as_deref())
            .filter(|m| !m.is_empty())
            .unwrap_or("Choose model");
        let effort = picked.and_then(|p| p.effort.as_deref()).or_else(|| if picked.is_none() { current.and_then(|a| a.effort.as_deref()) } else { None });
        let model_name = model_chip_name(model, current);
        let label = format!("{}{}{}", model_name, effort.map(|e| format!(" · {e}")).unwrap_or_default(), if picked.is_some() { ", this turn" } else { "" });
        let chip_id = id.clone();
        let chip = el().max_w(180.0).h(48.0).padding(6.0).center().radius(6.0).fill(p.raised)
            .child(text(label).font(type_::LABEL).color(p.text).wrap_anywhere())
            .on_press(move |s: &mut State| open_models(s, chip_id.clone()));
        let mut picker = col().w_full().px(24.0);
        if s.model_menu.as_ref() == Some(&id) {
            let mut menu = col().key("model-menu").w_full().max_h(180.0).scroll(ScrollSpec { stick_bottom: false }).gap(4.0);
            if let Some(agents) = s.models.get(&id).filter(|a| !a.is_empty()) {
                let reset_id = id.clone();
                menu = menu.child(el().padding(6.0).child(text("Teammate’s own").color(p.text))
                    .on_press(move |s: &mut State| { s.picks.remove(&reset_id); s.model_menu = None; }));
                for agent in agents.iter().filter(|a| id != LEDGER_TARGET || teammate.runtime.as_deref() == Some(a.runtime.as_str())) { for model in &agent.models {
                    let efforts = if model.efforts.is_empty() { vec![None] } else { model.efforts.iter().cloned().map(Some).collect() };
                    for effort in efforts {
                        let (id, pick) = (id.clone(), TurnPick { model: model.id.clone(), effort });
                        let label = format!("{}{}", model.name, pick.effort.as_ref().map(|e| format!(" · {e}")).unwrap_or_default());
                        menu = menu.child(el().padding(6.0).child(text(label).color(p.text))
                            .on_press(move |s: &mut State| { s.picks.insert(id.clone(), pick.clone()); s.model_menu = None; }));
                    }
                }}
            } else { menu = menu.child(text("Loading models…").color(p.muted)); }
            picker = picker.child(menu);
        }

        col().grow(1.0).shrink().h_full()
            .child(
                col().w_full().h(88.0).px(24.0).pt(30.0).gap(5.0).window_drag()
                    .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
                    .child(
                        text(teammate.name.clone()).font(type_::TITLE).color(p.text),
                    )
                    .child(text(agent(teammate)).font(type_::LABEL).color(p.muted)),
            )
            .child(transcript)
            .child(status)
            .child(picker)
            .child(
                row().w_full().gap(10.0).px(24.0).pb(24.0)
                    .child(chip).child(composer).child(button),
            )
    } else {
        col().grow(1.0).shrink().h_full().center().padding(32.0)
            .child(
                text(s.status.clone()).font(type_::BODY)
                    .color(p.status(s.status_tone)).w_full().wrap(),
            )
    };

    row().w_full().h_full().fill(p.bg).child(sidebar).child(right)
}

fn team_scene(s: &mut State) {
    s.demo = true;
    s.appearance = Some(true);
    s.teammates = [
        (
            "codec", "Codec", "Code & Migrations",
            "codex", "gpt-6.1-sol", false,
        ),
        (
            "argon", "Argon", "Code & Migrations",
            "antigravity", "Gemini 3.8 Flash", true,
        ),
        (
            "scout", "Scout", "Custom",
            "opencode", "deepseek-v4.1-flash-free", false,
        ),
    ]
    .into_iter()
    .map(|(id, name, role, runtime, model, busy)| Teammate {
        id: id.into(),
        name: name.into(),
        role: role.into(),
        runtime: Some(runtime.into()),
        model: Some(model.into()),
        busy,
        runs_in: "ask".into(),
        folder: None,
    })
    .collect();

    s.conversations = s.teammates.iter()
        .map(|t| (t.id.clone(), Conversation::default())).collect();
    if let Some(conversation) = s.conversations.get_mut("codec") {
        conversation.turns = vec![
            (
                "You".into(),
                "How is the native preview coming along?".into(),
            ),
            (
                "Codec".into(),
                "The window is taking shape. Teammates live on the left, and your conversation stays here.".into(),
            ),
            (
                "You".into(),
                "Let's keep it simple and make it feel like Locust.".into(),
            ),
            (
                "Codec".into(),
                "Agreed. A clear conversation, a quiet status line, and room to write.".into(),
            ),
        ];
    }
    s.recent = [
        (
            "sample-1", "Scout", "A real Markdown answer",
            "completed", "2026-11-08T21:23:37.380Z",
        ),
        (
            "sample-2", "Argon", "Review the native preview",
            "running", "2026-11-08T21:18:00Z",
        ),
        (
            "sample-3", "Codec", "Check the conversation sidebar",
            "failed", "2026-11-08T19:00:00Z",
        ),
        (
            "sample-4", "Scout",
            "A longer conversation title that should end in an ellipsis",
            "completed", "2026-11-07T12:00:00Z",
        ),
    ]
    .into_iter()
    .map(|(id, teammate, title, state, updated)| ConversationRow {
        id: id.into(),
        teammate: teammate.into(),
        title: title.into(),
        started_by: "the person".into(),
        state: state.into(),
        updated: updated.into(),
    })
    .collect();
    s.selected = 0;
    s.status = "Ready".into();
    s.status_tone = StatusTone::Muted;
}

fn empty_scene(s: &mut State) {
    team_scene(s);
    for conversation in s.conversations.values_mut() {
        *conversation = Conversation::default();
    }
}

/*
 * A REAL EXCHANGE, CHECKED WITHOUT A WINDOW. `--ask <teammate> <message>` runs the window's own engine calls (start,
 * then read_reply until the turn ends) against the running Locust and saves what came back; the "replay" scene draws
 * that file headless (`--snap shot.png --scene replay`), so a real answer is seen without opening a window that would
 * take the screen from whoever is working.
 */
const REPLAY_FILE: &str = "locust-native-replay.json";

fn replay_path() -> std::path::PathBuf {
    std::env::var_os("LOCUST_NATIVE_REPLAY").map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join(REPLAY_FILE))
}

fn ask_cli(who: &str, message: &str) -> Result<(), String> {
    let engine = Engine::local()?;
    let team = engine.teammates()?;
    let teammate = team.iter()
        .find(|t| t.name.eq_ignore_ascii_case(who) || t.id == who)
        .ok_or_else(|| format!("No teammate called {who}."))?;
    let started = engine.start(&teammate.id, message, None, None)?;
    eprintln!("started {} in {}", started.conversation_id, started.mode);

    let answer = loop {
        match engine.reply(&started.conversation_id, engine::WAIT_LIMIT_S)? {
            Reply::Working(doing) => eprintln!("working: {doing}"),
            Reply::WaitingForYou(words) => {
                eprintln!("waiting for you in Locust: {words}");
                std::thread::sleep(Duration::from_secs(3));
            }
            Reply::Answer(text) => break text,
            Reply::Failed(words) => return Err(words),
        }
    };
    println!("{answer}");
    let saved = serde_json::json!({
        "team": team.iter().map(|t| serde_json::json!({
            "id": t.id,
            "name": t.name,
            "role": t.role,
            "runtime": t.runtime,
            "model": t.model,
            "busy": t.busy,
            "runs_in": t.runs_in,
        })).collect::<Vec<_>>(),
        "teammate": teammate.id,
        "message": message,
        "answer": answer,
    });
    std::fs::write(replay_path(), saved.to_string()).map_err(|e| e.to_string())
}

fn replay_scene(s: &mut State) {
    s.demo = true;
    s.appearance = Some(true);
    let Some(saved) = std::fs::read_to_string(replay_path()).ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        s.status = "No saved exchange: run --ask first.".into();
        return;
    };

    s.teammates = saved.get("team")
        .and_then(|v| engine::parse_teammates(&v.to_string()).ok())
        .unwrap_or_default();
    s.conversations = s.teammates.iter()
        .map(|t| (t.id.clone(), Conversation::default())).collect();
    let picked = saved.get("teammate").and_then(serde_json::Value::as_str)
        .and_then(|id| s.teammates.iter().position(|t| t.id == id))
        .unwrap_or(0);

    if let Some(teammate) = s.teammates.get(picked)
        && let Some(conversation) = s.conversations.get_mut(&teammate.id)
    {
        conversation.turns = vec![
            (
                "You".into(),
                saved.get("message").and_then(serde_json::Value::as_str).unwrap_or_default().into(),
            ),
            (
                teammate.name.clone(),
                saved.get("answer").and_then(serde_json::Value::as_str).unwrap_or_default().into(),
            ),
        ];
    }
    s.selected = picked;
    s.status = "Ready".into();
    s.status_tone = StatusTone::Muted;
}

fn team_light_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(false);
}

fn empty_light_scene(s: &mut State) {
    empty_scene(s);
    s.appearance = Some(false);
}

fn replay_light_scene(s: &mut State) {
    replay_scene(s);
    s.appearance = Some(false);
}

fn ledger_scene(s: &mut State, live: bool, light: bool) {
    team_scene(s);
    s.appearance = Some(!light);
    let Some(header) = s.teammates.first().cloned() else {
        return;
    };
    let items = vec![
        ledger::Item::User("Explain this synthetic preview.".into()),
        ledger::Item::Plan { id: "preview-plan".into(), steps: ledger::read_plan(&serde_json::json!([
            {"step":"Read the input","status":"completed"}, {"step":"Summarize the result","status":"in_progress"}
        ])), finished: !live },
        ledger::Item::Tool {
            name: "inspect".into(),
            status: ledger::Status::Done,
            summary: "Read a synthetic fixture".into(),
            detail: ledger::ToolDetail { id: "preview-tool".into(), ..Default::default() },
        },
        ledger::Item::Assistant {
            item_id: "synthetic-answer".into(),
            text: if live {
                "### Live answer\n\nThis text is still arriving".into()
            } else {
                "### Complete transcript\n\nThe preview shows **messages**, tool steps, plans, and notes.".into()
            },
            r#final: !live,
        },
        ledger::Item::Note {
            level: "warning".into(),
            text: "Synthetic demonstration note.".into(),
        },
    ];
    s.ledger = Some(LedgerPane {
        header: header.clone(),
        conversation: Conversation {
            conversation_id: Some("synthetic-ledger".into()),
            turns: Vec::new(),
        },
        items: items.clone(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot {
            items,
            tip: "synthetic-ledger".into(),
            running: live,
            failed: false,
        },
        due: if live { 0.0 } else { f64::INFINITY },
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    if live {
        s.flight = Some(Flight {
            teammate_id: header.id.clone(),
            conversation_id: Some("synthetic-ledger".into()),
            header: Some(header),
            after_tip: None,
            message: None,
        });
    }
    s.status = if live { "Working…" } else { "Ready" }.into();
    s.status_tone = if live { StatusTone::Green } else { StatusTone::Muted };
}

fn thread_rich_scene(s: &mut State) {
    ledger_scene(s,true,false);
    let mut transcript = ledger::Transcript::default();
    let rows = [
        ("run.started", serde_json::json!({})),
        ("plan.updated", serde_json::json!({"itemId":"plan","plan":[{"step":"Read the fixture","status":"completed"},{"step":"Check the result","status":"in_progress"},{"step":"Write the report","status":"pending"}]})),
        ("step.started", serde_json::json!({"itemId":"thought","stepKind":"reasoning"})),
        ("step.completed", serde_json::json!({"itemId":"thought","stepKind":"reasoning","message":"**Checking the fixture**\nCompare the synthetic input with the expected result.","durationMs":12000})),
        ("tool.started", serde_json::json!({"itemId":"command","name":"Bash","title":"Check the fixture","command":"printf 'synthetic result\\n'"})),
        ("tool.completed", serde_json::json!({"itemId":"command","name":"Bash","output":"synthetic result\n1 check passed","exitCode":0,"durationMs":1500})),
        ("tool.started", serde_json::json!({"itemId":"helper","name":"Agent","title":"Review the fixture","status":"Explore","command":"Review the synthetic fixture and report what it contains."})),
        ("tool.started", serde_json::json!({"itemId":"child","parentItemId":"helper","name":"Read","command":"fixture.txt"})),
        ("tool.completed", serde_json::json!({"itemId":"child","parentItemId":"helper","name":"Read","output":"Synthetic fixture."})),
        ("tool.completed", serde_json::json!({"itemId":"helper","name":"Agent","output":"The synthetic fixture contains one passing check."})),
    ];
    let records = rows.into_iter().enumerate().map(|(n,(kind,payload))| serde_json::json!({"event":{"sequence":n,"type":kind,"occurredAt":"2026-01-01T00:00:00Z","payload":payload}})).collect();
    transcript.apply(records);
    if let Some(pane)=s.ledger.as_mut() {
        pane.items=vec![ledger::Item::User("Inspect this synthetic fixture.".into())];
        pane.items.extend(transcript.items);
        pane.snapshot.items=pane.items.clone();
    }
}
fn thread_rich_open_scene(s: &mut State) {
    thread_rich_scene(s);
    for key in ["synthetic-ledger/group/thought:3", "synthetic-ledger/thought:3", "synthetic-ledger/command", "synthetic-ledger/helper"] {
        s.activity_open.insert(key.into(),true);
    }
}
fn thread_rich_open_light_scene(s: &mut State) { thread_rich_open_scene(s); s.appearance=Some(false); }
fn thread_rich_light_scene(s: &mut State) { thread_rich_scene(s); s.appearance=Some(false); }

fn ledger_transcript_scene(s: &mut State) {
    ledger_scene(s, false, false);
}

fn ledger_live_scene(s: &mut State) {
    ledger_scene(s, true, false);
}

fn ledger_transcript_light_scene(s: &mut State) {
    ledger_scene(s, false, true);
}

fn ledger_live_light_scene(s: &mut State) {
    ledger_scene(s, true, true);
}

// Only the selected runtime may supply a display name: IDs can overlap across agents.
fn model_chip_name<'a>(id: &'a str, current: Option<&'a engine::Models>) -> &'a str {
    current.and_then(|a| a.models.iter().find(|m| m.id == id))
        .map(|m| m.name.as_str()).filter(|name| !name.trim().is_empty()).unwrap_or(id)
}

fn bench_options(args: &[String]) -> Result<(std::path::PathBuf, Duration), String> {
    let mut out = None;
    let mut seconds = 30_u64;
    let mut args = args.iter();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--bench" => out = Some(args.next().ok_or("--bench requires an output path")?.into()),
            "--bench-seconds" => {
                seconds = args.next().ok_or("--bench-seconds requires a positive integer")?
                    .parse().map_err(|_| "--bench-seconds requires a positive integer")?;
                if seconds == 0 { return Err("--bench-seconds must be positive".into()); }
            }
            _ => return Err(format!("unsupported benchmark argument: {flag}")),
        }
    }
    Ok((out.ok_or("--bench <out.json> is required")?, Duration::from_secs(seconds)))
}

fn run_bench(args: &[String], process_start: std::time::Instant) {
    let (out, duration) = bench_options(args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    // No setup, poll, key handlers or live scenes: no engine or ledger access.
    let app = App::new(AppSpec::new("Locust — benchmark", (1040.0, 720.0), || {
        let mut state = State::default();
        team_scene(&mut state);
        state
    }, view))
        .args(Vec::<String>::new())
        .accessory().open_inactive()
        .macos_window(rune::MacWindowOptions::default().level(-1))
        .window_options(rune::WindowOptions { click_through: true, ..Default::default() })
        .chrome(Chrome { full_size_content: true, traffic_lights: Some((20.0, 18.0)), drag_regions: true });
    let handle = app.handle();
    let mut recorded = false;
    app.on_frame_drawn(move |_| {
        if recorded { return; }
        recorded = true;
        // Rune calls this after queue.present and first-frame GPU completion, never
        // for skipped/occluded frames. This is a host observation, not scanout time.
        let ms = process_start.elapsed().as_secs_f64() * 1000.0;
        let result = serde_json::to_vec(&serde_json::json!({"first_frame_ms": ms}))
            .map_err(std::io::Error::other).and_then(|bytes| std::fs::write(&out, bytes));
        if let Err(error) = result {
            eprintln!("cannot write {}: {error}", out.display());
            std::process::exit(1);
        }
        let handle = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            handle.quit();
        });
    }).run();
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--run") {
        let code = match run_cli::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(code) => code, Err(error) => { eprintln!("{error}"); 1 }
        };
        std::process::exit(code);
    }
    let process_start = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--bench" || arg == "--bench-seconds") {
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
    if args.windows(2).any(|pair| {
        matches!(pair, [flag, scene] if flag == "--scene" && scene.starts_with("quick-"))
    }) {
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
            ("thread-rich", thread_rich_scene),
            ("thread-rich-open", thread_rich_open_scene),
            ("thread-rich-open-light", thread_rich_open_light_scene),
            ("thread-rich-light", thread_rich_light_scene),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn activity_status_words_follow_finished_refused_and_background_work() {
        use ledger::{Status, ToolDetail};
        let mut d=ToolDetail::default();
        assert_eq!(tool_result(Status::Running,&d,false),"running");
        assert_eq!(tool_result(Status::Running,&d,true),"did not report");
        d.helper=true; assert_eq!(tool_result(Status::Running,&d,false),"working on it");
        assert_eq!(tool_result(Status::Done,&d,true),"reported back");
        d.background=true; assert_eq!(tool_result(Status::Done,&d,false),"working in the background");
        assert_eq!(tool_result(Status::Done,&d,true),"did not report");
        d.background_ended="stopped-with-run".into(); assert_eq!(tool_result(Status::Done,&d,true),"stopped when the run ended");
        d.helper=false; assert_eq!(tool_result(Status::Done,&d,true),"stopped");
        d.runtime_status="refused".into(); assert_eq!(tool_result(Status::Failed,&d,true),"refused");
        d.runtime_status.clear(); d.exit_code=Some(2); assert_eq!(tool_result(Status::Failed,&d,true),"failed · exit 2");
    }

    #[test]
    fn tool_row_label_omits_zero_calls_background_dupes_and_non_shell_no_output() {
        use ledger::{Status, ToolDetail};
        let mut helper = ToolDetail { helper: true, background: true, background_ended: "stopped-with-run".into(), ..ToolDetail::default() };
        let label = tool_row_label("Agent", Status::Done, "Review", &helper, true);
        assert!(label.contains("stopped when the run ended"));
        assert!(!label.contains(" · when the run ended"));
        assert!(!label.contains("0 calls"));
        helper.children.push(ledger::Item::Note { level: String::new(), text: "x".into() });
        assert!(tool_row_label("Agent", Status::Done, "Review", &helper, true).contains("1 call"));

        let mut shell = ToolDetail { output_reported: true, output: "   ".into(), duration_ms: Some(3000), ..ToolDetail::default() };
        let shell_label = tool_row_label("Bash", Status::Done, "Check", &shell, true);
        assert!(shell_label.contains("no output"));
        assert!(shell_label.contains("3s"));
        let read_label = tool_row_label("Read", Status::Done, "file", &shell, true);
        assert!(!read_label.contains("no output"));
        assert!(!read_label.contains("3s"));
        shell.runtime_status = "declined".into();
        assert!(!tool_row_label("Bash", Status::Done, "Check", &shell, true).contains("3s"));
    }

    #[test]
    fn thought_headline_helpers_match_activity_card() {
        let text = "**Checking the fixture**\nCompare the synthetic input.";
        assert_eq!(thought_headline(text), Some("Checking the fixture"));
        assert_eq!(without_headline(text), "Compare the synthetic input.");
        assert_eq!(thought_headline("no headline"), None);
        assert_eq!(without_headline("no headline"), "no headline");
        let thought = ledger::Item::Thought { id: "t".into(), text: text.into(), duration_ms: Some(3000) };
        let label = match &thought {
            ledger::Item::Thought { text, duration_ms, .. } => {
                let mut label = format!("Thought for {}", ledger::duration_text(duration_ms.unwrap()));
                if let Some(h) = thought_headline(text) { label.push_str(&format!(" · {h}")); }
                label
            }
            _ => unreachable!(),
        };
        assert_eq!(label, "Thought for 3s · Checking the fixture");
    }

    #[test]
    fn plan_drawer_keys_do_not_collide_with_tool_item_ids() {
        let plan = ledger::Item::Plan { id: "plan".into(), steps: vec![], finished: false };
        let tool = ledger::Item::Tool {
            name: "Bash".into(), status: ledger::Status::Done, summary: "x".into(),
            detail: ledger::ToolDetail { id: "plan".into(), ..Default::default() },
        };
        assert_eq!(activity_drawer_key("scope", &plan), "scope/plan/plan");
        assert_eq!(activity_drawer_key("scope", &tool), "scope/plan");
        assert_ne!(activity_drawer_key("scope", &plan), activity_drawer_key("scope", &tool));
    }

    #[test]
    fn rich_scenes_start_folded_and_open_variant_has_independent_drawers() {
        let mut s=State::default(); thread_rich_scene(&mut s);
        assert!(s.activity_open.is_empty());
        let pane=s.ledger.as_ref().unwrap();
        assert!(matches!(pane.items[1],ledger::Item::Plan {..}));
        assert_eq!(pane.items.iter().filter(|i|is_activity(i)).count(),3);
        thread_rich_open_scene(&mut s);
        assert_eq!(s.activity_open.get("synthetic-ledger/command"),Some(&true));
        assert!(!s.activity_open.contains_key("synthetic-ledger/helper/child"));
        s.activity_open.insert("synthetic-ledger/command".into(),false);
        assert_eq!(s.activity_open.get("synthetic-ledger/helper"),Some(&true));
        let plan_key = s.ledger.as_ref().unwrap().items.iter()
            .find(|i| matches!(i, ledger::Item::Plan { .. }))
            .map(|plan| activity_drawer_key("synthetic-ledger", plan))
            .unwrap();
        assert!(plan_key.starts_with("synthetic-ledger/plan/"));
        thread_rich_light_scene(&mut s); assert_eq!(s.appearance,Some(false));
    }

    #[test]
    fn chip_name_uses_known_nonempty_name_and_falls_back_to_id() {
        let mut agent = engine::Models { teammate: None, runtime: "sample".into(),
            model: None, effort: None, models: vec![engine::Model {
                id: "raw/id".into(), name: "Friendly name".into(), efforts: vec![],
            }] };
        assert_eq!(model_chip_name("raw/id", Some(&agent)), "Friendly name");
        assert_eq!(model_chip_name("unknown", Some(&agent)), "unknown");
        assert_eq!(model_chip_name("raw/id", None), "raw/id");
        agent.models[0].name = " ".into();
        assert_eq!(model_chip_name("raw/id", Some(&agent)), "raw/id");
    }

    #[test]
    fn synthetic_home_never_requests_models() {
        let mut state = State::default();
        team_scene(&mut state);
        open_models(&mut state, "codec".into());
        assert!(!state.model_job.is_pending());
        assert!(state.model_menu.is_none());
    }

    #[test]
    fn bench_arguments_are_isolated_and_bounded() {
        let parse = |args: &[&str]| bench_options(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(parse(&["--bench", "out.json"]).unwrap().1, Duration::from_secs(30));
        assert_eq!(parse(&["--bench-seconds", "2", "--bench", "out.json"]).unwrap().1, Duration::from_secs(2));
        for args in [vec!["--bench"], vec!["--bench-seconds", "2"], vec!["--bench", "x", "--bench-seconds", "0"], vec!["--bench", "x", "--scene", "replay"]] {
            assert!(parse(&args).is_err());
        }
    }

    static NEXT: AtomicU64 = AtomicU64::new(0);

    pub(super) struct TempDir(pub(super) PathBuf);
    impl TempDir {
        pub(super) fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "locust-main-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        pub(super) fn write(&self, id: &str, records: &[serde_json::Value]) {
            let text = records.iter().map(|r| format!("{r}\n")).collect::<String>();
            std::fs::write(self.0.join(format!("{id}.jsonl")), text).unwrap();
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> State {
        let mut s = State::default();
        team_scene(&mut s);
        s
    }

    fn wait_job(s: &mut State) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while s.ledger_job.is_pending() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!s.ledger_job.is_pending(), "ledger_job did not finish");
    }

    #[test]
    fn reload_preserves_history_recent_selection_and_reply_target() {
        let mut s = sample();
        let conversation = s.conversations.get_mut("codec").unwrap();
        conversation.conversation_id = Some("sample-3".into());
        let turns = conversation.turns.clone();
        s.flight = Some(Flight {
            teammate_id: "codec".into(),
            conversation_id: Some("sample-3".into()),
            header: None,
            after_tip: None,
            message: None,
        });
        let mut team = s.teammates.clone();
        team.reverse();
        apply_team(&mut s, team);

        assert_eq!(s.teammates.get(s.selected).unwrap().id, "codec");
        let conversation = s.conversations.get("codec").unwrap();
        assert_eq!(conversation.conversation_id.as_deref(), Some("sample-3"));
        assert_eq!(conversation.turns, turns);
        assert!(s.recent.iter().any(|r| r.id == "sample-3"));

        land_reply(
            &mut s, "codec", Ok(Reply::Answer("landed".into())),
            &mut Tx::default(),
        );
        assert_eq!(
            s.conversations.get("codec").unwrap().turns.last().unwrap(),
            &("Codec".into(), "landed".into()),
        );
        assert!(s.flight.is_none());
    }

    #[test]
    fn removed_teammate_clears_active_and_opening_requests() {
        let mut s = sample();
        s.flight = Some(Flight {
            teammate_id: "codec".into(),
            conversation_id: None,
            header: None,
            after_tip: None,
            message: None,
        });
        s.opening_teammate = Some("codec".into());
        let team = s.teammates.iter()
            .filter(|t| t.id != "codec").cloned().collect();
        apply_team(&mut s, team);

        assert!(s.flight.is_none() && s.opening_teammate.is_none());
        assert!(s.status.contains("no longer in the team"));
    }

    #[test]
    fn missing_conversation_id_ends_the_flight() {
        let mut s = sample();
        s.flight = Some(Flight {
            teammate_id: "codec".into(),
            conversation_id: None,
            header: None,
            after_tip: None,
            message: None,
        });
        request_reply(&mut s, "codec", Duration::ZERO);

        assert!(s.flight.is_none());
        assert!(!s.waiting.is_pending());
        assert!(s.status.contains("No active conversation ID"));
    }

    #[test]
    fn escape_stops_watching_then_clears_draft_and_close_clears_state() {
        let mut s = sample();
        s.draft.replace_text("new draft");
        s.flight = Some(Flight {
            teammate_id: "codec".into(),
            conversation_id: None,
            header: None,
            after_tip: None,
            message: None,
        });
        let escape = KeyEvent {
            key: Key::Escape,
            mods: rune::Mods::default(),
        };
        let mut tx = Tx::default();

        assert!(on_key(&mut s, &escape, &mut tx));
        assert!(s.flight.is_none());
        assert_eq!(s.status, "Stopped watching; the turn goes on in Locust");
        assert_eq!(s.draft.text, "new draft");

        assert!(on_key(&mut s, &escape, &mut tx));
        assert!(s.draft.text.is_empty());

        s.flight = Some(Flight {
            teammate_id: "codec".into(),
            conversation_id: None,
            header: None,
            after_tip: None,
            message: None,
        });
        s.opening_teammate = Some("codec".into());
        on_close(&mut s, &mut tx);

        assert!(s.flight.is_none() && s.opening_teammate.is_none());
        assert!(s.status.is_empty());
    }

    #[test]
    fn failed_send_restores_unsent_text_without_losing_new_typing() {
        let mut s = sample();
        s.demo = false;
        s.draft.replace_text("new typing");
        s.flight = Some(Flight {
            teammate_id: "codec".into(),
            conversation_id: None,
            header: None,
            after_tip: None,
            message: Some("unsent text".into()),
        });
        // A local result exercises poll without contacting the engine.
        s.sending.request((), |()| Err("send failed".into()));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while s.sending.is_pending() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!s.sending.is_pending(), "fake result did not land");
        assert!(poll(&mut s, 0.0, &mut Tx::default()));
        assert_eq!(s.draft.text, "unsent text\n\nnew typing");
        assert!(s.flight.is_none());
    }

    #[test]
    fn switching_conversations_restores_cancelled_ledger_send_and_drops_stale_results() {
        let mut s = sample();
        s.demo = false;
        ledger_transcript_scene(&mut s);
        s.demo = false;
        let codec_header = s.teammates[0].clone();
        s.flight = Some(Flight {
            teammate_id: codec_header.id.clone(),
            conversation_id: Some("synthetic-ledger".into()),
            header: Some(codec_header),
            after_tip: Some("synthetic-ledger".into()),
            message: Some("follow-up question".into()),
        });

        // Fix 2 & 9: switching teammates cancels the ledger send, restores the unsent draft, and resets status.
        select_teammate(&mut s, "argon", &mut Tx::default());
        assert!(s.ledger.is_none());
        assert!(s.flight.is_none());
        assert_eq!(s.draft.text, "follow-up question");
        assert_eq!(s.status, "Argon: Ready");

        // Fix 2: a stale send result tagged for Codec must not swap in Argon's state or hijack Argon's view.
        s.flight = Some(Flight {
            teammate_id: "codec".into(),
            conversation_id: Some("old-conv".into()),
            header: Some(s.teammates[0].clone()),
            after_tip: Some("old-tip".into()),
            message: None,
        });
        s.sending.request((), |()| Ok(Started {
            conversation_id: "codec-new".into(),
            mode: "ask".into(),
        }));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while s.sending.is_pending() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        poll(&mut s, 1.0, &mut Tx::default());
        assert!(s.ledger.is_none());
        assert_eq!(s.teammates[s.selected].id, "argon");
    }

    #[test]
    fn idle_ledger_stops_polling_errors_back_off_without_rebuilding_and_after_tip_times_out() {
        let temp = TempDir::new();
        temp.write("done", &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"done",
                    "prompt":"Synthetic prompt",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:00Z"
                }
            }),
            json!({
                "schemaVersion":27,
                "recordType":"mission.event",
                "event":{
                    "sequence":1,
                    "occurredAt":"2026-01-01T00:00:01Z",
                    "type":"run.completed",
                    "payload":{}
                }
            }),
        ]);
        temp.write("other", &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"other",
                    "prompt":"Other prompt",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:00Z"
                }
            }),
        ]);

        let mut s = sample();
        s.demo = false;
        let header = s.teammates[0].clone();
        s.ledger = Some(LedgerPane {
            header,
            conversation: Conversation {
                conversation_id: Some("done".into()),
                turns: Vec::new(),
            },
            items: Vec::new(),
            dir: temp.0.clone(),
            worker: Some(ledger::Session::new(temp.0.clone(), "done".into())),
            snapshot: ledger::Snapshot::default(),
            due: 0.0,
            loading: true,
            after_tip: None,
            after_tip_deadline: None,
            backoff: 0.0,
        });

        // Fix 3: finished conversation stops scheduling wakeups after its initial load.
        let mut tx = Tx::default();
        ledger_tick(&mut s, 0.0, &mut tx);
        wait_job(&mut s);
        ledger_tick(&mut s, 0.1, &mut tx);
        let pane = s.ledger.as_ref().unwrap();
        assert!(!pane.loading && !pane.snapshot.running);
        assert!(pane.due.is_infinite());
        assert!(!s.ledger_job.is_pending());
        assert_eq!(pane.worker.as_ref().unwrap().cached_headers(), 2);

        // Fix 3: bound after_tip waiting to 30 s, then show "No new reply yet" and unlock Send.
        s.ledger.as_mut().unwrap().after_tip = Some("done".into());
        s.ledger.as_mut().unwrap().due = 10.0;
        ledger_tick(&mut s, 10.0, &mut tx);
        wait_job(&mut s);
        ledger_tick(&mut s, 10.1, &mut tx);
        assert_eq!(s.status, "Working…");
        assert!(s.flight.is_some());

        s.ledger.as_mut().unwrap().due = 40.0;
        ledger_tick(&mut s, 40.0, &mut tx);
        wait_job(&mut s);
        ledger_tick(&mut s, 40.0, &mut tx);
        assert_eq!(s.status, "No new reply yet");
        assert!(s.flight.is_none());
        assert!(s.ledger.as_ref().unwrap().after_tip.is_none());

        // Fix 3: transient errors back off 2 s, 4 s, 8 s, 16 s, 30 s without rebuilding the Session.
        std::fs::remove_file(temp.0.join("done.jsonl")).unwrap();
        std::fs::remove_file(temp.0.join("other.jsonl")).unwrap();
        let mut now = 100.0;
        s.ledger.as_mut().unwrap().loading = true;
        s.ledger.as_mut().unwrap().due = now;
        for expected in [2.0, 4.0, 8.0, 16.0, 30.0, 30.0] {
            ledger_tick(&mut s, now, &mut tx);
            wait_job(&mut s);
            ledger_tick(&mut s, now, &mut tx);
            let pane = s.ledger.as_ref().unwrap();
            assert_eq!(pane.backoff, expected);
            assert_eq!(pane.due, now + expected);
            // If Session had been rebuilt after `other.jsonl` was removed, cached_headers() would be 0.
            assert_eq!(pane.worker.as_ref().unwrap().cached_headers(), 2);
            now += expected;
        }
    }
}

#[cfg(test)]
mod daily_controls_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_pick_is_consumed_for_one_send_only_and_does_not_leak() {
        let mut s = State::default();
        team_scene(&mut s);
        s.demo = false;

        let pick = TurnPick { model: "sample-model".into(), effort: Some("high".into()) };
        s.picks.insert("codec".into(), pick.clone());
        assert_eq!(s.picks.get("argon"), None, "pick on one teammate never leaks to another");
        assert_eq!(take_turn_pick(&mut s, "codec"), Some(pick.clone()));
        assert_eq!(take_turn_pick(&mut s, "codec"), None);

        // A pick made on a Recent ledger (@ledger) is cleared when leaving or switching Recent ledgers.
        s.ledger_summaries.insert("recent-1".into(), ledger::Summary {
            mission_id: "recent-1".into(),
            title: "First".into(),
            runtime: "codex".into(),
            model: "gpt-6.1-sol".into(),
            created_at: "2026-11-08T00:00:00Z".into(),
            running: false,
            failed: false,
            updated: "2026-11-08T00:00:00Z".into(),
        });
        s.ledger_summaries.insert("recent-2".into(), ledger::Summary {
            mission_id: "recent-2".into(),
            title: "Second".into(),
            runtime: "claude".into(),
            model: "sonnet".into(),
            created_at: "2026-11-08T00:00:00Z".into(),
            running: false,
            failed: false,
            updated: "2026-11-08T00:00:00Z".into(),
        });
        let mut tx = Tx::default();
        open_recent(&mut s, ConversationRow {
            id: "recent-1".into(),
            teammate: "Codex".into(),
            title: "First".into(),
            started_by: "ledger".into(),
            state: "completed".into(),
            updated: "2026-11-08T00:00:00Z".into(),
        }, &mut tx);
        s.picks.insert(LEDGER_TARGET.into(), pick.clone());
        open_recent(&mut s, ConversationRow {
            id: "recent-2".into(),
            teammate: "Claude Code".into(),
            title: "Second".into(),
            started_by: "ledger".into(),
            state: "completed".into(),
            updated: "2026-11-08T00:00:00Z".into(),
        }, &mut tx);
        assert!(s.picks.get(LEDGER_TARGET).is_none(), "Recent ledger pick must not leak to another Recent conversation");

        // Starting a fresh conversation (Cmd+N) or changing a teammate's runtime clears its unconsumed pick.
        select_teammate(&mut s, "codec", &mut tx);
        s.picks.insert("codec".into(), pick.clone());
        fresh_conversation(&mut s);
        assert!(s.picks.get("codec").is_none());

        s.picks.insert("codec".into(), pick);
        let mut updated_team = s.teammates.clone();
        updated_team[0].runtime = Some("claude".into());
        apply_team(&mut s, updated_team);
        assert!(s.picks.get("codec").is_none(), "runtime change clears stale model pick");
    }

    #[test]
    fn stop_requires_our_accepted_turn_and_exact_ledger_tip() {
        let mut s = State::default();
        ledger_live_scene(&mut s);
        s.demo = false;
        assert!(!can_stop(&s), "watching someone else's working turn is never ownership");
        s.own_turn = Some(OwnTurn { conversation: "synthetic-ledger".into(), tip: Some("synthetic-ledger".into()), before: None });
        assert!(can_stop(&s));
        s.ledger.as_mut().expect("scene").snapshot.tip = "another-turn".into();
        assert!(!can_stop(&s));
        s.ledger.as_mut().expect("scene").snapshot.tip = "synthetic-ledger".into();
        s.ledger.as_mut().expect("scene").snapshot.running = false;
        assert!(!can_stop(&s));
    }

    #[test]
    fn stop_never_latches_onto_external_turn_after_fast_completion_or_detach() {
        let temp = tests::TempDir::new();
        temp.write("root", &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"root",
                    "prompt":"Initial",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:00Z"
                }
            }),
            json!({
                "schemaVersion":27,
                "recordType":"mission.event",
                "event":{
                    "sequence":1,
                    "occurredAt":"2026-01-01T00:00:01Z",
                    "type":"run.completed",
                    "payload":{}
                }
            }),
        ]);
        // Our follow-up child_1 already finished before the first ledger poll saw it running.
        temp.write("child_1", &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"child_1",
                    "prompt":"Our follow-up",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:02Z",
                    "continuesFrom":{"missionId":"root"}
                }
            }),
            json!({
                "schemaVersion":27,
                "recordType":"mission.event",
                "event":{
                    "sequence":1,
                    "occurredAt":"2026-01-01T00:00:03Z",
                    "type":"run.completed",
                    "payload":{}
                }
            }),
        ]);

        let mut s = State::default();
        team_scene(&mut s);
        s.demo = false;
        let header = s.teammates[0].clone();
        s.own_turn = Some(OwnTurn {
            conversation: "root".into(),
            tip: None,
            before: Some("root".into()),
        });
        s.ledger = Some(LedgerPane {
            header,
            conversation: Conversation {
                conversation_id: Some("root".into()),
                turns: Vec::new(),
            },
            items: Vec::new(),
            dir: temp.0.clone(),
            worker: Some(ledger::Session::new(temp.0.clone(), "root".into())),
            snapshot: ledger::Snapshot::default(),
            due: 0.0,
            loading: true,
            after_tip: Some("root".into()),
            after_tip_deadline: None,
            backoff: 0.0,
        });

        let mut tx = Tx::default();
        ledger_tick(&mut s, 0.0, &mut tx);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while s.ledger_job.is_pending() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        ledger_tick(&mut s, 0.1, &mut tx);
        assert!(s.own_turn.is_none(), "completed turn must clear ownership");
        assert!(!can_stop(&s));

        // Now an external turn child_2 starts in Locust; polling it must never grant Stop.
        temp.write("child_2", &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"child_2",
                    "prompt":"External turn",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:04Z",
                    "continuesFrom":{"missionId":"child_1"}
                }
            }),
        ]);
        s.ledger.as_mut().unwrap().loading = true;
        s.ledger.as_mut().unwrap().due = 1.0;
        ledger_tick(&mut s, 1.0, &mut tx);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while s.ledger_job.is_pending() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        ledger_tick(&mut s, 1.1, &mut tx);
        assert!(s.flight.is_some(), "window follows the external turn");
        assert!(!can_stop(&s), "external turn must not expose Stop");
    }
}
