//! Retained Quick Ask panel. All engine and ledger I/O stays off the UI thread.
//!
//! A send goes out through Engine::start. Once that returns a conversation_id, the
//! panel follows the conversation through ledger::Session, the way main.rs's
//! ledger_tick does, so the answer streams in as it is written. Engine::reply is
//! only the fallback, used when the ledger folder cannot be read.
use crate::engine::{self, Engine, Reply, Started, Teammate};
use crate::ledger::{self, Item};
use crate::{Palette, markdown_style};
use rune::tok::type_;
use rune::{
    Align, App, AppHandle, AppSpec, Chrome, Cx, DisplayTarget, El, Key, KeyEvent,
    Latest, OverlayPlacement, RunOptions, ScreenEdge, ScrollSpec, Shortcut,
    TextEdit, Tx, Waker, WindowOptions, WindowSpec, col, el, row, text, text_input,
};
use rune_kit::Markdown;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const WIDTH: f32 = 680.0;
const MAX_HEIGHT: f32 = 520.0;
const UNAVAILABLE: &str =
    "Locust isn't running, or 'Let your other AI apps use Locust' is off.";
const MAX_FLIGHT_AGE: Duration = Duration::from_secs(30 * 60);
const STOPPED: &str =
    "Stopped watching after 30 minutes; the turn goes on in Locust";

// Same cadence as main.rs's ledger_tick (now + 0.25) while the panel is drawn.
const LEDGER_VISIBLE: Duration = Duration::from_millis(250);
// Hidden, nobody sees the text grow; only the end matters. 2 s also matches how
// often Session rescans the directory, so polling faster than this gains nothing.
const LEDGER_HIDDEN: Duration = Duration::from_secs(2);
// After a failed ledger read, retry at this pace whether shown or hidden.
const LEDGER_ERROR_RETRY: Duration = Duration::from_secs(1);
// A readable folder that still cannot give us this conversation (no file, bad
// read) for this long counts as "ledger unavailable" and we fall back to reply.
const LEDGER_GRACE: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
enum Activity {
    Working,
    WaitingForYou,
}

// How the answer of a flight is being followed.
enum Follow {
    // The send has not returned a conversation_id yet.
    Starting,
    // Streaming from the ledger (the normal path).
    Ledger(LedgerFollow),
    // Fallback: Engine::reply polling, with the Retry machinery below.
    Reply,
}

struct LedgerFollow {
    dir: PathBuf,
    // None while a background job owns the Session; its landing hands it back
    // (main.rs does the same, so the Session's byte offsets and header cache survive).
    session: Option<ledger::Session>,
    // Last time a read succeeded (or the follow began). Drives the grace period.
    healthy: Instant,
    // When the last poll landed; None until the first one.
    last: Option<Instant>,
    // True after a failed read, to retry at LEDGER_ERROR_RETRY.
    erred: bool,
}

struct Flight {
    teammate_id: String,
    name: String,
    conversation_id: Option<String>,
    started_at: Instant,
    unsent: Option<String>,
    follow: Follow,
}

#[derive(Clone, Copy)]
enum RetryKind {
    Immediate,
    Working,
    Approval,
}

#[derive(Clone, Copy)]
struct Retry {
    since: Instant,
    kind: RetryKind,
}

// Both outcomes return the Session so a transient error never rebuilds it
// (as main.rs's LedgerAnswer). The bool on Err says whether the folder itself
// could still be listed, which is decided on the worker thread, not the UI thread.
type StreamAnswer = Result<
    (ledger::Session, ledger::Snapshot),
    (ledger::Session, bool),
>;

// Public only because scenes() returns AppSpec<State>; its fields stay private.
pub struct State {
    app: Option<AppHandle<State>>,
    // Last visibility requested by this module; native visibility is read for toggles.
    visible: bool,
    demo: bool,
    appearance: Option<bool>,
    top: f32,
    teammates: Vec<Teammate>,
    selected: usize,
    draft: TextEdit,
    question: String,
    answer: Option<String>,
    answer_name: String,
    error: Option<String>,
    notice: Option<String>,
    retry: Option<Retry>,
    activity: Activity,
    flight: Option<Flight>,
    team: Latest<Result<Vec<Teammate>, String>>,
    sending: Latest<Result<Started, String>>,
    waiting: Latest<Result<Reply, String>>,
    stream: Latest<StreamAnswer>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            app: None,
            visible: false,
            demo: false,
            appearance: None,
            top: 160.0,
            teammates: Vec::new(),
            selected: 0,
            draft: TextEdit::new(""),
            question: String::new(),
            answer: None,
            answer_name: String::new(),
            error: None,
            notice: None,
            retry: None,
            activity: Activity::Working,
            flight: None,
            team: Latest::default(),
            sending: Latest::default(),
            waiting: Latest::default(),
            stream: Latest::default(),
        }
    }
}

fn setup(s: &mut State, waker: Waker) {
    s.team.set_waker(waker.clone());
    s.sending.set_waker(waker.clone());
    s.waiting.set_waker(waker.clone());
    s.stream.set_waker(waker);
    // The hidden launcher's prepared first frame must not contact the engine.
}

fn begin_show(s: &mut State) {
    begin_show_at(s, Instant::now());
}

fn begin_show_at(s: &mut State, now: Instant) {
    s.visible = true;
    s.draft.clear();
    s.draft.focused = true;
    expire(s, now);

    // Fix: a re-show never restarts a pending load or reloads an active flight.
    if s.flight.is_none() && !s.team.is_pending() {
        s.error = None;
        if !s.demo {
            s.team.request((), |()| Engine::local()?.teammates());
        }
    }
    // Keep the previous exchange and any timeout notice until a new send.
}

fn stop_watching(s: &mut State) {
    s.sending.cancel();
    s.waiting.cancel();
    // A cancelled Latest never lands, so a late ledger result cannot revive a stopped flight.
    s.stream.cancel();
    s.flight = None;
    s.retry = None;
}

fn expire(s: &mut State, now: Instant) -> bool {
    if s.flight.as_ref().is_some_and(|f| {
        now.saturating_duration_since(f.started_at) >= MAX_FLIGHT_AGE
    }) {
        // Fix: a watchdog also ends requests that never return, without stopping Locust.
        stop_watching(s);
        s.notice = Some(STOPPED.into());
        return true;
    }
    false
}

fn hide(s: &mut State) {
    // Fix: chord and blur share this path; both keep observing the active exchange.
    s.visible = false;
    s.draft.focused = false;
    if let Some(app) = &s.app {
        app.hide();
    }
}

fn toggle(s: &mut State, tx: &mut Tx) {
    // Fix: query Rune's published native visibility instead of trusting the requested flag.
    let shown = s.app.as_ref()
        .map(|app| app.is_visible()).unwrap_or(s.visible);
    if shown {
        hide(s);
        return;
    }

    begin_show(s);
    // Showing shortens the poll interval (2 s hidden -> 250 ms), so re-arm the timer
    // now instead of waiting out a wake that was scheduled while hidden.
    arm_deadline(s, Instant::now(), tx);
    if let Some(app) = &s.app {
        app.place(
            DisplayTarget::Primary,
            OverlayPlacement::Edge {
                edge: ScreenEdge::Top,
                margin: s.top,
                align: Align::Center,
                inset: 0.0,
            },
        );
        // Fix: queue a show in either path. Keep activate last: Rune coalesces requests,
        // and activate itself is Open(token), so a later tokenless open would lose the token.
        app.show();
        if let Some(token) = tx.activation_token() {
            app.activate(token);
        } else {
            app.open();
        }
    }
}

fn cycle(s: &mut State, backwards: bool) {
    // Fix: the header and composer stay attached to the teammate still answering.
    if s.flight.is_some() {
        return;
    }
    let count = s.teammates.len();
    if count == 0 {
        return;
    }
    let current = s.selected.min(count - 1);
    s.selected = if backwards {
        if current == 0 { count - 1 } else { current - 1 }
    } else if current + 1 == count {
        0
    } else {
        current + 1
    };
    s.draft.focused = true;
}

fn error_words(words: String) -> String {
    // Fix: Engine returns String errors. Match only its exact local/transport failures;
    // HTTP refusals, malformed responses, and tool refusals retain Locust's own words.
    if words == engine::NOT_ON
        || words == "Locust's data folder could not be found on this computer."
        || words == "Locust isn't running, or isn't taking requests from other apps right now. Open Locust and try again."
    {
        UNAVAILABLE.into()
    } else {
        words
    }
}

fn show_failure(s: &mut State, words: String) {
    stop_watching(s);
    s.notice = None;
    s.error = Some(error_words(words));
}

fn land_team(s: &mut State, result: Result<Vec<Teammate>, String>) {
    // Fix: a load started before sending may land afterward; ignore it during the flight.
    // In particular, a failed refresh cannot replace the team or end observation.
    if s.flight.is_some() {
        return;
    }
    match result {
        Ok(team) if !team.is_empty() => {
            let selected_id = s.teammates.get(s.selected).map(|t| t.id.clone());
            s.selected = selected_id.as_deref()
                .and_then(|id| team.iter().position(|t| t.id == id))
                .or_else(|| team.iter().position(|t| t.name == "Codec"))
                .unwrap_or(0);
            s.teammates = team;
            s.error = None;
        }
        Ok(_) => {
            s.teammates.clear();
            s.selected = 0;
            // A reachable engine with no teammates is not a connection failure.
            show_failure(s, "No teammates are available in Locust.".into());
        }
        Err(words) => show_failure(s, words),
    }
}

fn send(s: &mut State, tx: &mut Tx) {
    send_at(s, tx, Instant::now());
}

fn send_at(s: &mut State, tx: &mut Tx, now: Instant) {
    expire(s, now);
    if let Some(flight) = &s.flight {
        // Fix: Enter explains why it cannot send and leaves all new typing intact.
        s.notice = Some(format!(
            "{} is still answering. Esc stops watching.",
            flight.name,
        ));
        tx.scroll_to_bottom("quick-messages");
        return;
    }
    if s.demo || s.draft.text.trim().is_empty() {
        return;
    }
    let Some(teammate) = s.teammates.get(s.selected) else {
        if s.team.is_pending() {
            s.notice = Some("Loading teammates… Press Enter again when ready.".into());
        } else {
            show_failure(s, "No teammates are available in Locust.".into());
        }
        return;
    };

    // Existing loaded teammates remain usable while a refresh is pending.
    let id = teammate.id.clone();
    let name = teammate.name.clone();
    let message = s.draft.text.clone();

    s.question = message.clone();
    s.answer = None;
    s.answer_name.clear();
    s.error = None;
    s.notice = None;
    s.activity = Activity::Working;
    s.flight = Some(Flight {
        teammate_id: id.clone(),
        name,
        conversation_id: None,
        started_at: now,
        unsent: Some(message.clone()),
        follow: Follow::Starting,
    });
    s.retry = None;
    s.draft.clear();

    s.sending.request((id, message), |(id, message)| {
        Engine::local()?.start(&id, &message)
    });
    arm_deadline(s, now, tx);
}

fn restore_failed_draft(s: &mut State, message: String) {
    // Fix: prepend failed text without discarding anything typed after dispatch.
    let draft = if s.draft.text.is_empty() {
        message
    } else {
        format!("{message}\n\n{}", s.draft.text)
    };
    s.draft.replace_text(draft);
}

fn land_started(s: &mut State, result: Result<Started, String>, now: Instant) {
    if expire(s, now) || s.flight.is_none() {
        return;
    }
    match result {
        Ok(started) => {
            if let Some(flight) = s.flight.as_mut() {
                flight.conversation_id = Some(started.conversation_id);
                flight.unsent = None;
            }
            // crate::ledger_dir is private to main.rs, but a child module may use it.
            begin_follow(s, crate::ledger_dir(), now);
        }
        Err(words) => {
            let unsent = s.flight.as_mut().and_then(|f| f.unsent.take());
            if let Some(message) = unsent {
                restore_failed_draft(s, message);
            }
            show_failure(s, words);
        }
    }
}

// Choose how to follow the conversation. `dir` is injected so tests can use a temp
// folder; production passes crate::ledger_dir(). Readability is NOT probed here: that
// would touch the disk on the UI thread. The first job reports it instead.
fn begin_follow(s: &mut State, dir: Option<PathBuf>, now: Instant) {
    let Some(flight) = s.flight.as_mut() else { return };
    let id = flight.conversation_id.clone().unwrap_or_default();
    match dir {
        // Session::poll would refuse an unsafe id with InvalidInput; skip the pointless poll.
        Some(dir) if ledger::safe_id(&id) => {
            let session = ledger::Session::new(dir.clone(), id);
            flight.follow = Follow::Ledger(LedgerFollow {
                dir,
                session: Some(session),
                healthy: now,
                last: None,
                erred: false,
            });
            s.retry = None;
        }
        _ => fall_back_to_reply(s, now),
    }
}

// Engine::reply is kept only for this: the ledger folder (or id) cannot be used.
fn fall_back_to_reply(s: &mut State, now: Instant) {
    if let Some(flight) = s.flight.as_mut() {
        flight.follow = Follow::Reply;
    }
    s.retry = Some(Retry {
        since: now,
        kind: RetryKind::Immediate,
    });
}

fn retry_due(s: &State) -> Option<Instant> {
    let retry = s.retry?;
    let delay = match retry.kind {
        RetryKind::Immediate => Duration::ZERO,
        RetryKind::Working => Duration::from_millis(500),
        // Fix: visibility changes adjust an already scheduled approval retry too.
        RetryKind::Approval => Duration::from_secs(if s.visible { 3 } else { 15 }),
    };
    Some(retry.since + delay)
}

fn ledger_interval(visible: bool) -> Duration {
    if visible { LEDGER_VISIBLE } else { LEDGER_HIDDEN }
}

// When the next ledger read is due. Computed from the last landing and the CURRENT
// visibility, so showing or hiding moves an already scheduled poll (like Approval above).
// None while a job owns the Session: its landing wakes us through the Latest waker.
fn ledger_due(s: &State) -> Option<Instant> {
    let Follow::Ledger(f) = &s.flight.as_ref()?.follow else {
        return None;
    };
    f.session.as_ref()?;
    Some(match f.last {
        None => f.healthy,
        Some(last) => last + if f.erred {
            LEDGER_ERROR_RETRY
        } else {
            ledger_interval(s.visible)
        },
    })
}

fn request_reply(s: &mut State) {
    let Some(id) = s.flight.as_ref()
        .and_then(|f| f.conversation_id.clone())
        .filter(|id| !id.is_empty())
    else {
        show_failure(s, "No active conversation ID; stopped watching".into());
        return;
    };
    s.retry = None;
    s.waiting.request(id, |id| Engine::local()?.reply(&id, 20));
}

// Move the Session onto a worker for one poll (as main.rs does with ledger_job).
fn request_stream(s: &mut State) {
    let taken = match s.flight.as_mut().map(|f| &mut f.follow) {
        Some(Follow::Ledger(f)) => {
            f.session.take().map(|session| (session, f.dir.clone()))
        }
        _ => None,
    };
    let Some((mut session, dir)) = taken else { return };
    s.stream.request((), move |()| match session.poll() {
        Ok(snapshot) => Ok((session, snapshot)),
        Err(_) => {
            // Decided here, off the UI thread: can the folder still be listed at all?
            let readable = std::fs::read_dir(&dir).is_ok();
            Err((session, readable))
        }
    });
}

// The assistant text of THIS turn: everything after the last user message, joined.
// Session also yields earlier turns of a continued chain; Quick Ask never continues one,
// but the cut keeps that safe. Tool, plan and note items are left out of the answer.
fn streamed_text(items: &[Item]) -> String {
    let from = items.iter()
        .rposition(|i| matches!(i, Item::User(_)))
        .map_or(0, |i| i + 1);
    items.iter().skip(from)
        .filter_map(|i| match i {
            Item::Assistant { text, .. } if !text.trim().is_empty() => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

// Locust's own words for a failed run: the ledger's last error note.
fn failure_words(items: &[Item], name: &str) -> String {
    items.iter().rev()
        .find_map(|i| match i {
            Item::Note { level, text }
                if level == "error" && !text.trim().is_empty() =>
            {
                Some(text.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| format!("{name} stopped before finishing."))
}

// The conversation ended in the ledger: show the final text (or the failure) and stop.
fn finish_stream(s: &mut State, snapshot: &ledger::Snapshot, tx: &mut Tx) {
    let name = s.flight.as_ref().map(|f| f.name.clone()).unwrap_or_default();
    let body = streamed_text(&snapshot.items);
    stop_watching(s);
    s.answer_name = name.clone();
    s.notice = None;
    s.answer = (!body.is_empty()).then_some(body);
    s.error = if snapshot.failed {
        // A failed run keeps whatever text streamed before it failed.
        Some(failure_words(&snapshot.items, &name))
    } else if s.answer.is_none() {
        Some(format!(
            "{name} finished without an answer. Open Locust to see the conversation.",
        ))
    } else {
        None
    };
    tx.scroll_to_bottom("quick-messages");
}

// Returns whether anything drawn changed, so an unchanged poll does not redraw.
fn land_stream(
    s: &mut State,
    result: StreamAnswer,
    tx: &mut Tx,
    now: Instant,
) -> bool {
    if expire(s, now) {
        return true;
    }
    // Same guard as main.rs: a result for another conversation, or for a flight that has
    // since fallen back or ended, is dropped.
    let anchor = match &result {
        Ok((session, _)) | Err((session, _)) => session.anchor().to_owned(),
    };
    let current = s.flight.as_ref().and_then(|f| match &f.follow {
        Follow::Ledger(_) => f.conversation_id.clone(),
        _ => None,
    });
    if current.as_deref() != Some(anchor.as_str()) {
        return false;
    }

    match result {
        Ok((_, snapshot)) if !snapshot.running => {
            finish_stream(s, &snapshot, tx);
            true
        }
        Ok((session, snapshot)) => {
            let body = streamed_text(&snapshot.items);
            if let Some(flight) = s.flight.as_mut() {
                s.answer_name = flight.name.clone();
                if let Follow::Ledger(f) = &mut flight.follow {
                    f.session = Some(session);
                    f.last = Some(now);
                    f.healthy = now;
                    f.erred = false;
                }
            }
            s.activity = Activity::Working;
            let answer = (!body.is_empty()).then_some(body);
            // stick_bottom on quick-messages follows the growing text; no explicit scroll.
            let changed = s.answer != answer;
            s.answer = answer;
            changed
        }
        Err((session, readable)) => {
            let stale = s.flight.as_ref().is_some_and(|f| {
                matches!(&f.follow, Follow::Ledger(l)
                    if now.saturating_duration_since(l.healthy) >= LEDGER_GRACE)
            });
            if !readable || stale {
                // The ledger cannot be read: this is the one case that uses Engine::reply.
                fall_back_to_reply(s, now);
            } else if let Some(Follow::Ledger(f)) =
                s.flight.as_mut().map(|f| &mut f.follow)
            {
                // Keep the Session (as main.rs does) and retry soon; the file may
                // simply not exist yet right after start.
                f.session = Some(session);
                f.last = Some(now);
                f.erred = true;
            }
            false
        }
    }
}

fn land_reply(
    s: &mut State,
    result: Result<Reply, String>,
    tx: &mut Tx,
    now: Instant,
) {
    if expire(s, now) || s.flight.is_none() {
        return;
    }
    match result {
        Ok(Reply::Working(_)) => {
            s.activity = Activity::Working;
            s.retry = Some(Retry {
                since: now,
                kind: RetryKind::Working,
            });
        }
        Ok(Reply::WaitingForYou(_)) => {
            s.activity = Activity::WaitingForYou;
            s.retry = Some(Retry {
                since: now,
                kind: RetryKind::Approval,
            });
        }
        Ok(Reply::Answer(answer)) => {
            // Fix: use the answering flight's identity at landing, not a dispatch-time label.
            if let Some(flight) = s.flight.take() {
                s.answer_name = flight.name;
            }
            s.answer = Some(answer);
            s.retry = None;
            s.notice = None;
            s.error = None;
            tx.scroll_to_bottom("quick-messages");
        }
        Ok(Reply::Failed(words)) => {
            stop_watching(s);
            s.notice = None;
            s.error = Some(words);
        }
        Err(words) => show_failure(s, words),
    }
}

fn arm_deadline(s: &State, now: Instant, tx: &mut Tx) {
    if let Some(flight) = &s.flight {
        let expires = flight.started_at + MAX_FLIGHT_AGE;
        // Whichever follow mode is active has at most one of these set.
        let due = [retry_due(s), ledger_due(s)].into_iter().flatten().min();
        let next = due.map(|due| due.min(expires)).unwrap_or(expires);
        // Rune runs due poll timers while hidden; no sleeping worker or periodic UI timer.
        tx.wake_after(next.saturating_duration_since(now));
    }
}

fn poll(s: &mut State, _: f64, tx: &mut Tx) -> bool {
    poll_at(s, tx, Instant::now())
}

fn poll_at(s: &mut State, tx: &mut Tx, now: Instant) -> bool {
    if s.demo {
        return false;
    }
    let mut changed = expire(s, now);

    if let Some(result) = s.team.take() {
        changed = true;
        land_team(s, result);
    }
    if let Some(result) = s.sending.take() {
        changed = true;
        land_started(s, result, now);
    }
    if let Some(result) = s.waiting.take() {
        changed = true;
        land_reply(s, result, tx, now);
    }
    if let Some(result) = s.stream.take() {
        changed |= land_stream(s, result, tx, now);
    }
    if s.flight.is_some() && retry_due(s).is_some_and(|due| due <= now) {
        request_reply(s);
        changed = true;
    }
    // One ledger job at a time; landing hands the Session back before the next is due.
    if !s.stream.is_pending() && ledger_due(s).is_some_and(|due| due <= now) {
        request_stream(s);
    }
    arm_deadline(s, now, tx);
    changed
}

fn key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.key == Key::Escape {
        if s.flight.is_some() {
            stop_watching(s);
        }
        s.notice = None;
        hide(s);
        return true;
    }
    if event.key == Key::Tab
        && !event.mods.cmd && !event.mods.ctrl && !event.mods.alt
    {
        cycle(s, event.mods.shift);
        return true;
    }
    if event.key == Key::Enter && event.mods.cmd {
        // TODO: offer opening this answered exchange in the main app.
        return true;
    }
    if event.key == Key::Enter
        && !event.mods.shift && !event.mods.alt
        && !event.mods.ctrl && !event.mods.cmd
    {
        send(s, tx);
        return true;
    }
    // Shift+Enter reaches Rune's multiline editor. IME composition runs before this hook.
    false
}

fn focus(s: &mut State, focused: bool, _: &mut Tx) {
    if !focused && s.visible {
        // Blur hides without cancelling observation, so the answer can finish offscreen.
        hide(s);
    }
}

fn close(s: &mut State, _: &mut Tx) {
    s.visible = false;
    s.team.cancel();
    stop_watching(s);
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Palette::new(s.appearance.unwrap_or_else(|| cx.system_dark()));
    let teammate = if let Some(flight) = &s.flight {
        s.teammates.iter().find(|t| t.id == flight.teammate_id)
    } else {
        s.teammates.get(s.selected)
    };
    let name = s.flight.as_ref().map(|f| f.name.as_str())
        .or_else(|| teammate.map(|t| t.name.as_str()))
        .unwrap_or("Locust");
    let initial = name.chars().next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".into());

    let header = row().key("quick-header").w_full().center_y().gap(10.0)
        .child(
            el().size(22.0, 22.0).radius(11.0).center().fill(p.raised)
                .child(text(initial).font(type_::LABEL).color(p.secondary)),
        )
        .child(
            text(name).font(type_::TITLE).color(p.text)
                .max_w(100.0).truncate(),
        )
        .child(
            text_input(&s.draft, type_::BODY).key("quick-composer")
                .placeholder(format!("Ask {name}…"), p.muted)
                .color(p.text).grow(1.0).shrink()
                .min_h(48.0).max_h(112.0)
                .padding(12.0).radius(10.0).fill(p.field)
                .border(1.0, p.hairline)
                .on_edit(|s: &mut State| &mut s.draft)
                .on_input_key(key).on_submit(send),
        );

    let mut panel = col().key("quick-panel").w_full().max_h(MAX_HEIGHT)
        .padding(16.0).gap(12.0).radius(16.0)
        .fill(p.bg).border(1.0, p.hairline)
        .child(header);
    let mut output = col().key("quick-output").w_full().gap(12.0);

    if !s.question.is_empty() {
        output = output.child(
            text(s.question.clone()).font(type_::BODY)
                .color(p.secondary).w_full()
                .wrap_anywhere().selectable(true),
        );
    }
    if let Some(error) = &s.error {
        output = output.child(
            text(error.clone()).font(type_::BODY)
                .color(p.red).w_full().wrap_anywhere().selectable(true),
        );
    } else if let Some(flight) = &s.flight {
        let status = match s.activity {
            Activity::Working => format!("{} is working…", flight.name),
            Activity::WaitingForYou => {
                format!("{} is waiting for you in Locust", flight.name)
            }
        };
        output = output.child(
            text(status).font(type_::LABEL).color(p.muted).w_full().wrap(),
        );
    } else if s.team.is_pending() {
        output = output.child(
            text("Loading teammates…").font(type_::LABEL).color(p.muted),
        );
    }

    if let Some(notice) = &s.notice {
        output = output.child(
            text(notice.clone()).font(type_::LABEL)
                .color(p.muted).w_full().wrap_anywhere().selectable(true),
        );
    }

    if let Some(answer) = &s.answer {
        output = output
            .child(text(s.answer_name.clone()).font(type_::LABEL).color(p.muted))
            .child(
                // An answer shown during a flight is still growing; main.rs passes
                // !final the same way. After the flight ends it is complete.
                Markdown::new(answer.clone()).streaming(s.flight.is_some())
                    .selectable(true)
                    .render(&markdown_style(&p))
                    .key("quick-answer").w_full(),
            );
    }

    if !s.question.is_empty()
        || s.error.is_some()
        || s.notice.is_some()
        || s.flight.is_some()
        || s.answer.is_some()
        || s.team.is_pending()
    {
        // Measure natural output, then cap only its viewport; the editor remains visible.
        let header_h = cx.layout_size("quick-header")
            .map(|r| r[1]).unwrap_or(48.0);
        let output_h = cx.layout_size("quick-output")
            .map(|r| r[1]).unwrap_or(80.0);
        let available = (MAX_HEIGHT - 32.0 - 12.0 - header_h).max(1.0);

        panel = panel.child(
            col().key("quick-messages").w_full()
                .h(output_h.min(available)).min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .child(output),
        );
    } else {
        // An empty panel says how to drive it; Tab only matters with more than one teammate.
        let hint = if s.teammates.len() > 1 {
            "Tab switches teammate · Enter sends · Esc hides"
        } else {
            "Enter sends · Esc hides"
        };
        panel = panel.child(text(hint).font(type_::LABEL).color(p.muted));
    }

    panel
}

fn sample(s: &mut State) {
    s.demo = true;
    s.visible = true;
    s.appearance = Some(true);
    s.teammates = [
        ("codec", "Codec", "codex", "gpt-6.1-sol"),
        ("scout", "Scout", "opencode", "deepseek-v4.1-flash-free"),
    ]
    .into_iter()
    .map(|(id, name, runtime, model)| Teammate {
        id: id.into(),
        name: name.into(),
        role: "Quick Ask".into(),
        runtime: Some(runtime.into()),
        model: Some(model.into()),
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    })
    .collect();
}

fn empty_scene(s: &mut State) {
    sample(s);
}

fn working_scene(s: &mut State) {
    sample(s);
    s.question = "What should I check before shipping?".into();
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        name: "Codec".into(),
        conversation_id: Some("sample".into()),
        started_at: Instant::now(),
        unsent: None,
        // Demo scenes never poll (poll_at returns early); Reply keeps the old test shape.
        follow: Follow::Reply,
    });
}

fn answer_scene(s: &mut State) {
    sample(s);
    s.question = "What should I check before shipping?".into();
    s.answer_name = "Codec".into();
    s.answer = Some(
        "### Three checks\n\n- Run the focused tests.\n- Check the empty and error states.\n- Try one real exchange.\n\nKeep the change **small** and record what remains unverified.".into(),
    );
}

fn answer_light_scene(s: &mut State) {
    answer_scene(s);
    s.appearance = Some(false);
}

/// A separate typed spec lets main route quick snapshots without changing its own state.
pub fn scenes() -> AppSpec<State> {
    AppSpec {
        min_size: Some((WIDTH, 0.0)),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(key),
        scenes: &[
            ("quick-empty", empty_scene),
            ("quick-working", working_scene),
            ("quick-answer", answer_scene),
            ("quick-answer-light", answer_light_scene),
        ],
        ..AppSpec::new("Locust", (WIDTH, 420.0), State::default, view)
    }
}

pub fn run() {
    if rune::is_headless() {
        rune::run(scenes());
        return;
    }
    if !cfg!(target_os = "macos") {
        eprintln!("Quick Ask currently requires macOS.");
        std::process::exit(1);
    }

    let chord = std::env::var("LOCUST_QUICK_CHORD")
        .unwrap_or_else(|_| "alt+space".into());
    let shortcut: Shortcut = match chord.parse() {
        Ok(shortcut) => shortcut,
        Err(error) => {
            eprintln!("Quick Ask shortcut {chord}: {error}; holder: unknown");
            std::process::exit(1);
        }
    };

    rune::run_windows_with(RunOptions { accessory: true }, move |displays| {
        let top = displays.iter().find(|d| d.primary)
            .or_else(|| displays.first())
            .map(|d| (d.size.1 as f64 / d.scale / 6.0) as f32)
            .unwrap_or(160.0);

        let app = App::new(scenes())
            .windowless().keep_running().fit_content_height()
            .configured_options(
                WindowOptions::launcher_panel()
                    .macos(|native| native.corner_radius(16.0).shadow(true)),
            )
            // This replaces only portable limits, retaining the preset's native launcher.
            .window_options(WindowOptions {
                visible: false,
                transparent: true,
                borderless: true,
                resizable: false,
                always_on_top: true,
                min_size: Some((WIDTH, 0.0)),
                max_size: Some((WIDTH, MAX_HEIGHT)),
                ..WindowOptions::default()
            })
            .chrome(Chrome {
                full_size_content: false,
                traffic_lights: None,
                drag_regions: false,
            })
            .on_focus(focus)
            .on_close(close);

        let handle = app.handle();
        let state_handle = handle.clone();
        let mut registered = false;

        let app = app.init(move || State {
            app: Some(state_handle),
            top,
            ..State::default()
        })
        .on_ready(move |_| {
            // A launcher prepares a hidden native frame before this callback.
            // Runtime registration avoids the builder's additional error logging.
            if registered {
                return;
            }
            registered = true;
            if let Err(error) = handle.register_shortcut(shortcut.clone(), toggle) {
                let holder = handle.shortcut_owner(&shortcut)
                    .unwrap_or_else(|| "unknown".into());
                eprintln!("Quick Ask shortcut {chord}: {error}; holder: {holder}");
                std::process::exit(1);
            }
        });

        vec![WindowSpec::from_app("quick", app)]
    });
}

#[cfg(test)]
mod tests {
    // Carried over from quick.r7.rs, none dropped. Only `escape_during_flight_...`
    // changed (it also checks the new `stream` handle). The rest compile as before
    // because working_scene() now sets Follow::Reply, which keeps the old retry timing.
    // New: the three `streams_...`, `hiding_...` and `missing_...` tests at the end.
    use super::*;
    use serde_json::{Value, json};
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn working(now: Instant) -> State {
        let mut s = State::default();
        working_scene(&mut s);
        if let Some(flight) = s.flight.as_mut() {
            flight.started_at = now;
        }
        s
    }

    // ---- synthetic ledger helpers: JSONL in a temp dir only ----
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir()
                .join(format!("locust-quick-test-{}-{nanos}-{n}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn created(id: &str) -> Value {
        json!({"schemaVersion":27,"recordType":"mission.created","metadata":{
            "missionId":id,"prompt":"Synthetic question","runtime":"codex",
            "model":"sample","createdAt":"2026-01-01T00:00:00Z"}})
    }
    fn ev(seq: u64, kind: &str, payload: Value) -> Value {
        json!({"schemaVersion":27,"recordType":"mission.event","event":{
            "sequence":seq,"occurredAt":"2026-01-01T00:00:01Z",
            "type":kind,"payload":payload}})
    }
    fn delta(seq: u64, piece: &str, last: bool) -> Value {
        ev(seq, "message.delta", json!({
            "itemId":"answer","operation":"append","text":piece,"final":last}))
    }
    fn append(dir: &Path, id: &str, records: &[Value]) {
        let mut f = OpenOptions::new().create(true).append(true)
            .open(dir.join(format!("{id}.jsonl"))).unwrap();
        for r in records {
            writeln!(f, "{r}").unwrap();
        }
    }

    // A flight that has just returned its conversation_id "conv1", following `dir`.
    // demo is turned off because poll_at returns early for demo scenes.
    fn streaming(dir: &Path, now: Instant) -> State {
        let mut s = State::default();
        working_scene(&mut s);
        s.demo = false;
        if let Some(f) = s.flight.as_mut() {
            f.started_at = now;
            f.conversation_id = Some("conv1".into());
            f.follow = Follow::Starting;
        }
        begin_follow(&mut s, Some(dir.to_path_buf()), now);
        s
    }

    fn mode(s: &State) -> &'static str {
        match s.flight.as_ref().map(|f| &f.follow) {
            Some(Follow::Starting) => "starting",
            Some(Follow::Ledger(_)) => "ledger",
            Some(Follow::Reply) => "reply",
            None => "none",
        }
    }

    // Same wait-for-the-worker pattern as main.rs's tests.
    fn wait(s: &State) {
        let end = Instant::now() + Duration::from_secs(5);
        while s.stream.is_pending() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!s.stream.is_pending(), "ledger job did not land");
    }

    // One full cycle at the synthetic time `now`: dispatch if due, wait, land.
    // Only used while following the ledger, so it never reaches Engine::reply.
    fn pump(s: &mut State, now: Instant) -> bool {
        let mut tx = Tx::default();
        let first = poll_at(s, &mut tx, now);
        wait(s);
        poll_at(s, &mut tx, now) | first
    }

    #[test]
    fn tab_cycles_both_directions_and_wraps_when_idle() {
        let mut s = State::default();
        sample(&mut s);
        let mut tx = Tx::default();
        let tab = KeyEvent { key: Key::Tab, mods: rune::Mods::default() };
        assert!(key(&mut s, &tab, &mut tx));
        assert_eq!(s.selected, 1);
        assert!(key(&mut s, &tab, &mut tx));
        assert_eq!(s.selected, 0);

        let back = KeyEvent {
            key: Key::Tab,
            mods: rune::Mods { shift: true, ..Default::default() },
        };
        assert!(key(&mut s, &back, &mut tx));
        assert_eq!(s.selected, 1);

        s.teammates.clear();
        assert!(key(&mut s, &tab, &mut tx));
    }

    #[test]
    fn escape_during_flight_stops_watching_and_hides() {
        let mut s = working(Instant::now());
        let escape = KeyEvent { key: Key::Escape, mods: rune::Mods::default() };
        assert!(key(&mut s, &escape, &mut Tx::default()));
        assert!(s.flight.is_none() && s.retry.is_none());
        assert!(!s.visible);
        assert!(
            !s.sending.is_pending() && !s.waiting.is_pending() && !s.stream.is_pending()
        );
    }

    #[test]
    fn showing_resets_field_but_keeps_the_last_exchange() {
        let mut s = State::default();
        answer_scene(&mut s);
        let question = s.question.clone();
        let answer = s.answer.clone();
        s.draft.replace_text("old draft");

        hide(&mut s);
        begin_show(&mut s);

        assert!(s.visible && s.draft.focused);
        assert!(s.draft.text.is_empty());
        assert_eq!(s.question, question);
        assert_eq!(s.answer, answer);
    }

    #[test]
    fn hidden_approval_waits_fifteen_seconds_and_hidden_answers_survive_show() {
        let now = Instant::now();
        let mut s = working(now);
        let mut tx = Tx::default();
        land_reply(&mut s, Ok(Reply::WaitingForYou("approve".into())), &mut tx, now);
        assert_eq!(retry_due(&s), Some(now + Duration::from_secs(3)));

        toggle(&mut s, &mut tx);
        assert!(!s.visible && s.flight.is_some());
        assert_eq!(retry_due(&s), Some(now + Duration::from_secs(15)));

        begin_show_at(&mut s, now + Duration::from_secs(1));
        assert_eq!(retry_due(&s), Some(now + Duration::from_secs(3)));

        focus(&mut s, false, &mut tx);
        assert!(!s.visible && s.flight.is_some());
        assert_eq!(retry_due(&s), Some(now + Duration::from_secs(15)));

        land_reply(
            &mut s,
            Ok(Reply::Answer("finished while hidden".into())),
            &mut tx,
            now + Duration::from_secs(2),
        );
        assert!(!s.visible);

        begin_show_at(&mut s, now + Duration::from_secs(3));
        assert_eq!(s.answer.as_deref(), Some("finished while hidden"));
    }

    #[test]
    fn watchdog_stops_at_thirty_minutes_and_discards_late_results() {
        let now = Instant::now();
        let mut s = working(now);
        hide(&mut s);

        assert!(!expire(&mut s, now + MAX_FLIGHT_AGE - Duration::from_secs(1)));
        assert!(expire(&mut s, now + MAX_FLIGHT_AGE));
        assert!(s.flight.is_none() && s.retry.is_none());
        assert_eq!(s.notice.as_deref(), Some(STOPPED));

        land_reply(
            &mut s,
            Ok(Reply::Answer("late".into())),
            &mut Tx::default(),
            now + MAX_FLIGHT_AGE + Duration::from_secs(1),
        );
        assert!(s.answer.is_none());
        assert_eq!(s.notice.as_deref(), Some(STOPPED));
    }

    #[test]
    fn busy_enter_preserves_draft_and_failed_reload_does_not_end_flight() {
        let now = Instant::now();
        let mut s = working(now);
        let team = s.teammates.clone();

        begin_show_at(&mut s, now);
        assert!(!s.team.is_pending());
        s.draft.replace_text("my next question");
        send_at(&mut s, &mut Tx::default(), now);

        assert_eq!(s.draft.text, "my next question");
        assert_eq!(
            s.notice.as_deref(),
            Some("Codec is still answering. Esc stops watching."),
        );
        land_team(&mut s, Err(engine::NOT_ON.into()));
        assert_eq!(s.teammates, team);
        assert!(s.flight.is_some());
        assert!(s.error.is_none());
    }

    #[test]
    fn tab_is_locked_during_flight_and_answer_label_comes_from_flight() {
        let now = Instant::now();
        let mut s = working(now);
        let tab = KeyEvent { key: Key::Tab, mods: rune::Mods::default() };
        assert!(key(&mut s, &tab, &mut Tx::default()));
        assert_eq!(s.selected, 0);

        if let Some(flight) = s.flight.as_mut() {
            flight.name = "Codec renamed".into();
        }
        s.answer_name = "stale".into();
        land_reply(&mut s, Ok(Reply::Answer("answer".into())), &mut Tx::default(), now);
        assert_eq!(s.answer_name, "Codec renamed");
    }

    #[test]
    fn failed_send_restores_text_and_keeps_locust_refusal_words() {
        let now = Instant::now();
        let mut s = working(now);
        if let Some(flight) = s.flight.as_mut() {
            flight.unsent = Some("unsent question".into());
        }
        s.draft.replace_text("new typing");
        let refusal = "Choose a teammate with an AI agent. Nothing was started.";
        land_started(&mut s, Err(refusal.into()), now);

        assert_eq!(s.draft.text, "unsent question\n\nnew typing");
        assert_eq!(s.error.as_deref(), Some(refusal));
        assert!(s.flight.is_none());
    }

    #[test]
    fn only_local_connection_errors_become_unavailable() {
        assert_eq!(error_words(engine::NOT_ON.into()), UNAVAILABLE);
        assert_eq!(
            error_words("Locust's data folder could not be found on this computer.".into()),
            UNAVAILABLE,
        );
        assert_eq!(
            error_words(
                "Locust isn't running, or isn't taking requests from other apps right now. Open Locust and try again.".into(),
            ),
            UNAVAILABLE,
        );

        let refusal = "Locust refused the request (403).";
        assert_eq!(error_words(refusal.into()), refusal);

        let now = Instant::now();
        let mut s = working(now);
        land_reply(&mut s, Err(refusal.into()), &mut Tx::default(), now);
        assert_eq!(s.error.as_deref(), Some(refusal));

        land_team(&mut s, Ok(Vec::new()));
        assert_eq!(s.error.as_deref(), Some("No teammates are available in Locust."));
    }

    // ---- new: live streaming from the ledger ----

    #[test]
    fn streams_growing_text_across_two_polls_then_finishes() {
        let temp = TempDir::new();
        let t0 = Instant::now();
        append(&temp.0, "conv1", &[created("conv1"), delta(1, "Hello", false)]);
        let mut s = streaming(&temp.0, t0);
        assert_eq!(mode(&s), "ledger");

        // First poll: partial text shows while the flight is still running.
        assert!(pump(&mut s, t0));
        assert_eq!(s.answer.as_deref(), Some("Hello"));
        assert_eq!(s.answer_name, "Codec");
        assert!(s.flight.is_some() && s.error.is_none());
        assert_eq!(ledger_due(&s), Some(t0 + LEDGER_VISIBLE));

        // Second poll: the same text grew; it is replaced, not duplicated.
        append(&temp.0, "conv1", &[delta(2, " world", false)]);
        assert!(pump(&mut s, t0 + Duration::from_millis(300)));
        assert_eq!(s.answer.as_deref(), Some("Hello world"));

        // A poll that finds nothing new reports no change (no pointless redraw).
        assert!(!pump(&mut s, t0 + Duration::from_millis(600)));
        assert_eq!(s.answer.as_deref(), Some("Hello world"));

        // The run ends: final text stays, the flight is released, nothing used Engine::reply.
        append(&temp.0, "conv1", &[delta(3, "!", true), ev(4, "run.completed", json!({}))]);
        assert!(pump(&mut s, t0 + Duration::from_secs(1)));
        assert_eq!(s.answer.as_deref(), Some("Hello world!"));
        assert!(s.flight.is_none() && s.retry.is_none() && s.error.is_none());
        assert!(!s.waiting.is_pending() && !s.stream.is_pending());
    }

    #[test]
    fn hiding_mid_stream_keeps_the_turn_going_but_polls_slowly() {
        let temp = TempDir::new();
        let t0 = Instant::now();
        append(&temp.0, "conv1", &[created("conv1"), delta(1, "Hello", false)]);
        let mut s = streaming(&temp.0, t0);
        assert!(s.visible);
        pump(&mut s, t0);
        assert_eq!(s.answer.as_deref(), Some("Hello"));
        assert_eq!(ledger_due(&s), Some(t0 + LEDGER_VISIBLE));

        // Hiding keeps the flight and moves the already scheduled poll out to 2 s.
        hide(&mut s);
        assert!(!s.visible && s.flight.is_some());
        assert_eq!(ledger_due(&s), Some(t0 + LEDGER_HIDDEN));

        // New text on disk is NOT read at +300 ms while hidden...
        append(&temp.0, "conv1", &[delta(2, " world", false)]);
        pump(&mut s, t0 + Duration::from_millis(300));
        assert_eq!(s.answer.as_deref(), Some("Hello"));
        // ...but is read once the slow interval has passed.
        pump(&mut s, t0 + LEDGER_HIDDEN);
        assert_eq!(s.answer.as_deref(), Some("Hello world"));

        // Showing again (flight active, so no team reload) brings the fast pace back.
        begin_show_at(&mut s, t0 + Duration::from_millis(2100));
        assert_eq!(ledger_due(&s), Some(t0 + LEDGER_HIDDEN + LEDGER_VISIBLE));

        // Finishing while hidden still lands the final answer; the panel stays hidden.
        hide(&mut s);
        append(&temp.0, "conv1", &[delta(3, "!", true), ev(4, "run.completed", json!({}))]);
        pump(&mut s, t0 + Duration::from_secs(5));
        assert!(!s.visible && s.flight.is_none());
        assert_eq!(s.answer.as_deref(), Some("Hello world!"));
    }

    #[test]
    fn missing_ledger_folder_falls_back_to_engine_reply() {
        let t0 = Instant::now();

        // No data folder at all: straight to the Engine::reply path.
        let mut s = streaming(Path::new("unused"), t0);
        if let Some(f) = s.flight.as_mut() {
            f.follow = Follow::Starting;
        }
        begin_follow(&mut s, None, t0);
        assert_eq!(mode(&s), "reply");
        assert_eq!(retry_due(&s), Some(t0));
        assert!(ledger_due(&s).is_none());

        // A folder that does not exist: the first job reports it unreadable and we fall back.
        // land_stream is called directly so the test never dispatches a real Engine::reply.
        let temp = TempDir::new();
        let mut s = streaming(&temp.0.join("nope"), t0);
        request_stream(&mut s);
        wait(&s);
        let result = s.stream.take().expect("job result");
        assert!(!land_stream(&mut s, result, &mut Tx::default(), t0));
        assert_eq!(mode(&s), "reply");
        assert_eq!(retry_due(&s), Some(t0));
        assert!(ledger_due(&s).is_none());
        assert!(s.flight.is_some() && s.error.is_none());
        assert!(!s.waiting.is_pending());
    }

    #[test]
    fn a_late_ledger_file_is_waited_for_before_falling_back() {
        let temp = TempDir::new(); // readable folder, but conv1.jsonl is not there yet
        let t0 = Instant::now();
        let mut s = streaming(&temp.0, t0);
        let mut tx = Tx::default();

        request_stream(&mut s);
        wait(&s);
        let result = s.stream.take().expect("job result");
        land_stream(&mut s, result, &mut tx, t0);
        assert_eq!(mode(&s), "ledger");
        assert_eq!(ledger_due(&s), Some(t0 + LEDGER_ERROR_RETRY));

        // Still nothing once the grace period is over: now the fallback applies.
        request_stream(&mut s);
        wait(&s);
        let result = s.stream.take().expect("job result");
        land_stream(&mut s, result, &mut tx, t0 + LEDGER_GRACE);
        assert_eq!(mode(&s), "reply");
    }
}
