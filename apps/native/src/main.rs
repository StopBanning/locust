mod engine;
mod ledger;
mod quick;

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

// Fix: requests keep stable teammate IDs, never positions in a reloadable list.
struct Flight {
    teammate_id: String,
    // Some only until start/send lands; reply failures must not restore sent text.
    message: Option<String>,
}

struct State {
    teammates: Vec<Teammate>,
    selected: usize,
    // Fix: histories and conversation IDs survive team reordering and reloads.
    conversations: HashMap<String, Conversation>,
    draft: TextEdit,
    status: String,
    status_tone: StatusTone,
    load_error: Option<String>,
    flight: Option<Flight>,
    demo: bool,
    // None follows the system; snapshot scenes explicitly choose an appearance.
    appearance: Option<bool>,
    recent: Vec<ConversationRow>,
    recent_error: Option<String>,
    opening_teammate: Option<String>,
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
            status: "Loading teammates…".into(),
            status_tone: StatusTone::Muted,
            load_error: None,
            flight: None,
            demo: false,
            appearance: None,
            recent: Vec::new(),
            recent_error: None,
            opening_teammate: None,
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
}

type LedgerAnswer = Result<(ledger::Session, ledger::Snapshot), String>;

fn ledger_dir() -> Option<std::path::PathBuf> {
    engine::default_data_folder().map(|dir| dir.join("mission-ledger"))
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
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some(id.clone()),
            turns: vec![("You".into(), title)],
        },
        items: Vec::new(),
        worker: Some(ledger::Session::new(dir.clone(), id)),
        dir,
        snapshot: ledger::Snapshot::default(),
        due: 0.0,
        loading: true,
        after_tip,
    });
    s.status = "Loading conversation…".into();
    s.status_tone = StatusTone::Muted;
    tx.wake_after(Duration::ZERO);
}

fn leave_ledger(s: &mut State) {
    s.ledger_job.cancel();
    if s.flight.as_ref().is_some_and(|f| f.teammate_id == LEDGER_TARGET) {
        detach(s);
    }
    s.ledger = None;
}

fn select_teammate(s: &mut State, id: &str, tx: &mut Tx) {
    leave_ledger(s);
    let Some(index) = s.teammates.iter().position(|t| t.id == id) else {
        return;
    };
    s.selected = index;
    if s.ledger_available {
        let header = s.teammates.get(index).cloned();
        let conversation = s.conversations.get(id)
            .and_then(|c| c.conversation_id.clone());
        if let (Some(header), Some(conversation)) = (header, conversation) {
            begin_ledger(s, conversation, String::new(), header, None, tx);
        }
    }
    tx.scroll_to_bottom("messages");
}

fn ledger_tick(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    if let Some(result) = s.ledger_job.take() {
        match result {
            Ok((worker, snapshot)) => {
                if let Some(pane) = s.ledger.as_mut() {
                    changed = pane.loading || pane.snapshot != snapshot;
                    let finished = pane.snapshot.running && !snapshot.running;
                    let new_tip = pane.snapshot.tip != snapshot.tip;
                    if pane.after_tip.as_ref()
                        .is_some_and(|tip| tip != &snapshot.tip)
                    {
                        pane.after_tip = None;
                    }
                    pane.items = snapshot.items.clone();
                    pane.loading = false;
                    pane.due = now + if snapshot.running || pane.after_tip.is_some() {
                        0.25
                    } else {
                        2.0
                    };
                    pane.worker = Some(worker);
                    pane.snapshot = snapshot;
                    let active = pane.snapshot.running || pane.after_tip.is_some();
                    s.status = if active { "Working…" } else { "Ready" }.into();
                    s.status_tone = if pane.snapshot.failed {
                        StatusTone::Red
                    } else if active {
                        StatusTone::Green
                    } else {
                        StatusTone::Muted
                    };

                    if active {
                        s.flight = Some(Flight {
                            teammate_id: LEDGER_TARGET.into(),
                            message: None,
                        });
                    } else if s.flight.as_ref()
                        .is_some_and(|f| f.teammate_id == LEDGER_TARGET)
                    {
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
            Err(words) => {
                changed = true;
                s.status = words;
                s.status_tone = StatusTone::Red;
                s.flight = None;
                if let Some(pane) = s.ledger.as_mut() {
                    pane.loading = false;
                    pane.due = now + 2.0;
                    if let Some(id) = &pane.conversation.conversation_id {
                        pane.worker = Some(ledger::Session::new(
                            pane.dir.clone(), id.clone(),
                        ));
                    }
                }
            }
        }
    }

    if let Some(pane) = s.ledger.as_mut() {
        if now >= pane.due {
            if let Some(mut worker) = pane.worker.take() {
                s.ledger_job.request((), move |()| {
                    let snapshot = worker.poll()
                        .map_err(|_| "Local conversation could not be read.".to_owned())?;
                    Ok((worker, snapshot))
                });
            }
        } else if pane.worker.is_some() {
            tx.wake_after(Duration::from_secs_f64((pane.due - now).max(0.0)));
        }
    }
    changed
}

fn message_column(items: &[ledger::Item], name: &str, p: &Palette) -> El {
    let mut messages = col().w_full().gap(24.0).padding(24.0);
    let style = markdown_style(p);
    for (index, item) in items.iter().enumerate() {
        let key = format!("ledger-item-{index}");
        let content = match item {
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
            ledger::Item::Tool { name, status, summary } => {
                let verb = match status {
                    ledger::Status::Running => "Running",
                    ledger::Status::Done => "Ran",
                    ledger::Status::Failed => "Failed",
                };
                text(format!(
                    "{verb}: {name}{}",
                    if summary.is_empty() {
                        String::new()
                    } else {
                        format!(" · {summary}")
                    },
                ))
                .font(type_::LABEL).color(p.muted)
                .w_full().wrap_anywhere().selectable(true)
            }
            ledger::Item::Plan(body) => text(format!("Plan\n{body}"))
                .font(type_::LABEL).color(p.muted)
                .w_full().wrap_anywhere().selectable(true),
            ledger::Item::Note { level, text: body } => {
                text(format!("{level}: {body}"))
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
            .map(|t| t.name.as_str()).unwrap_or("Locust")
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
    let message = s.draft.text.clone();
    let target = if s.ledger.is_some() {
        LEDGER_TARGET.into()
    } else {
        teammate_id.clone()
    };
    s.flight = Some(Flight {
        teammate_id: target.clone(),
        message: Some(message.clone()),
    });

    // An idle ledger result must not clear a new send. Reopen after acceptance.
    if s.ledger.is_some() {
        s.ledger_job.cancel();
        if let Some(pane) = s.ledger.as_mut() {
            pane.worker = None;
        }
    }
    s.draft.clear();
    set_status(s, &target, "Sending…", StatusTone::Muted);

    s.sending.request(
        (teammate_id, conversation_id, message),
        |(id, conversation, message)| {
            let engine = Engine::local()?;
            match conversation {
                Some(id) => engine.send(&id, &message),
                None => engine.start(&id, &message),
            }
        },
    );
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
    s.flight = None;
    s.opening_teammate = None;
    if let Some(pane) = s.ledger.as_mut() {
        pane.worker = None;
        pane.loading = false;
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

    // Fix: retain this handle's waker while choosing each request's debounce.
    s.waiting = std::mem::take(&mut s.waiting).debounce(delay);
    s.waiting.request(id, |id| Engine::local()?.reply(&id, 20));
}

fn apply_team(s: &mut State, team: Vec<Teammate>) {
    let selected = s.teammates.get(s.selected).map(|t| t.id.clone());
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
        if id != LEDGER_TARGET {
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
        // Ledger metadata has no reliable teammate identity; do not guess from a model.
        let header = Teammate {
            id: LEDGER_TARGET.into(),
            name: engine::runtime_name(&summary.runtime).into(),
            role: "Conversation".into(),
            runtime: Some(summary.runtime),
            model: Some(summary.model),
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
    s.ledger = None;
    s.selected = index;
    s.conversations.insert(id.clone(), Conversation {
        conversation_id: Some(row.id.clone()),
        turns: vec![("You".into(), row.title)],
    });
    set_status(s, &id, "Loading…", StatusTone::Muted);
    tx.scroll_to_bottom("messages");

    if !s.demo {
        s.opening_teammate = Some(id);
        s.opening.request(row.id, |id| Engine::local()?.reply(&id, 0));
    }
}

fn fresh_conversation(s: &mut State) {
    let Some(id) = s.teammates.get(s.selected).map(|t| t.id.clone()) else {
        return;
    };
    detach(s);
    s.ledger = None;
    s.conversations.insert(id.clone(), Conversation::default());
    set_status(s, &id, "Ready", StatusTone::Muted);
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
            s.flight = None;
            refresh_recent(s);
            if s.selected == index {
                tx.scroll_to_bottom("messages");
            }
        }
        Ok(Reply::Failed(error)) | Err(error) => {
            set_status(s, teammate_id, &error, StatusTone::Red);
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
        if let Some(id) = s.flight.as_ref().map(|f| f.teammate_id.clone()) {
            let message = s.flight.as_mut().and_then(|f| f.message.take());
            match result {
                Ok(started) => {
                    if s.ledger_available || s.ledger.is_some() {
                        let header = s.ledger.as_ref().map(|p| p.header.clone())
                            .or_else(|| s.teammates.iter()
                                .find(|t| t.id == id).cloned());
                        let after = s.ledger.as_ref()
                            .map(|p| p.snapshot.tip.clone())
                            .filter(|v| !v.is_empty());

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
                                started.conversation_id,
                                message.unwrap_or_default(),
                                header,
                                after,
                                tx,
                            );
                            s.flight = Some(Flight {
                                teammate_id: LEDGER_TARGET.into(),
                                message: None,
                            });
                        } else {
                            tracking_error(s, &id, "Teammate is no longer in the team.");
                        }
                    } else if let Some(index) = target_index(s, &id) {
                        if let Some(conversation) = s.conversations.get_mut(&id) {
                            conversation.conversation_id = Some(started.conversation_id);
                            if let Some(message) = message {
                                conversation.turns.push(("You".into(), message));
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
                        if let Some(id) = &pane.conversation.conversation_id {
                            pane.worker = Some(ledger::Session::new(
                                pane.dir.clone(), id.clone(),
                            ));
                            pane.due = 0.0;
                            tx.wake_after(Duration::ZERO);
                        }
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
        if let Some(id) = s.opening_teammate.take() {
            s.flight = Some(Flight {
                teammate_id: id.clone(),
                message: None,
            });
            land_reply(s, &id, result, tx);
        }
    }
    if let Some(result) = s.waiting.take() {
        changed = true;
        if let Some(id) = s.flight.as_ref().map(|f| f.teammate_id.clone()) {
            land_reply(s, &id, result, tx);
        }
    }

    changed | ledger_tick(s, now, tx)
}

fn on_close(s: &mut State, _: &mut Tx) {
    // Fix: cancelled handles never land; clear their flight and visible status too.
    s.team.cancel();
    s.recents.cancel();
    detach(s);
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
            let picked = selected_conversation(s)
                .and_then(|(_, c)| c.conversation_id.as_deref())
                == Some(item.id.as_str());
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
                .child(message_column(items, &teammate.name, &p))
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
        let enabled = can_send(s);
        let mut button = el().key("send").h(48.0).px(18.0).center().radius(9.0)
            .fill(p.raised).border(1.0, p.hairline).disabled(!enabled)
            .child(
                text("Send").font(type_::LABEL)
                    .color(if enabled { p.text } else { p.muted }),
            );
        if enabled {
            button = button.on_press_tx(send);
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
            .child(
                row().w_full().gap(10.0).px(24.0).pb(24.0)
                    .child(composer).child(button),
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
    let started = engine.start(&teammate.id, message)?;
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

    s.teammates = engine::parse_teammates(&saved["team"].to_string())
        .unwrap_or_default();
    s.conversations = s.teammates.iter()
        .map(|t| (t.id.clone(), Conversation::default())).collect();
    let picked = s.teammates.iter()
        .position(|t| saved["teammate"] == t.id.as_str()).unwrap_or(0);

    if let Some(teammate) = s.teammates.get(picked)
        && let Some(conversation) = s.conversations.get_mut(&teammate.id)
    {
        conversation.turns = vec![
            (
                "You".into(),
                saved["message"].as_str().unwrap_or_default().into(),
            ),
            (
                teammate.name.clone(),
                saved["answer"].as_str().unwrap_or_default().into(),
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
        ledger::Item::Plan(
            "completed: Read the input\nin_progress: Summarize the result".into(),
        ),
        ledger::Item::Tool {
            name: "inspect".into(),
            status: ledger::Status::Done,
            summary: "Read a synthetic fixture".into(),
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
        header,
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
        due: 0.0,
        loading: false,
        after_tip: None,
    });
    if live {
        s.flight = Some(Flight {
            teammate_id: LEDGER_TARGET.into(),
            message: None,
        });
    }
    s.status = if live { "Working…" } else { "Ready" }.into();
    s.status_tone = if live { StatusTone::Green } else { StatusTone::Muted };
}

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
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

    fn sample() -> State {
        let mut s = State::default();
        team_scene(&mut s);
        s
    }

    #[test]
    fn reload_preserves_history_recent_selection_and_reply_target() {
        let mut s = sample();
        let conversation = s.conversations.get_mut("codec").unwrap();
        conversation.conversation_id = Some("sample-3".into());
        let turns = conversation.turns.clone();
        s.flight = Some(Flight {
            teammate_id: "codec".into(),
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
}
