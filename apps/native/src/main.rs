mod engine;

use engine::{ConversationRow, Engine, Reply, Started, Teammate};
use rune::tok::type_;
use rune::{
    Align, App, AppSpec, Chrome, Color, Cycle, Cx, El, Key, KeyEvent, Latest,
    ScrollSpec, TextEdit, Tx, Waker, col, el, row, text, text_input,
};
use rune_kit::{Markdown, MarkdownStyle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BG: &str = "#121415";
const SIDEBAR: &str = "#17191a";
const CARD: &str = "#191c1d";
const SELECTED: &str = "#1e2223";
const YOUR_BUBBLE: &str = "#23282a";
const FIELD: &str = "#1a1d1e";
const TEXT: &str = "#f2efe9";
const SECONDARY: &str = "#c7c3bc";
const MUTED: &str = "#8b9190";
const GREEN: &str = "#7fc97f";
const AMBER: &str = "#e9b949";
const RED: &str = "#e4685d";

fn color(value: &str) -> Color {
    rune::color::hex(value).expect("valid Locust colour")
}

fn hairline() -> Color {
    Color::WHITE.alpha(0.06)
}

#[derive(Default)]
struct Conversation {
    conversation_id: Option<String>,
    turns: Vec<(String, String)>,
}

// A single flight protects side-effecting sends from Latest's replacement semantics.
// Selection can change while it runs; results still belong to this index.
struct Flight {
    index: usize,
    message: String,
}

struct State {
    teammates: Vec<Teammate>,
    selected: usize,
    conversations: Vec<Conversation>,
    draft: TextEdit,
    status: String,
    status_color: Color,
    load_error: Option<String>,
    flight: Option<Flight>,
    demo: bool,
    recent: Vec<ConversationRow>,
    recent_error: Option<String>,
    opening_index: Option<usize>,
    recents: Latest<Result<Vec<ConversationRow>, String>>,
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
            conversations: Vec::new(),
            draft: TextEdit::new(""),
            status: "Loading teammates…".into(),
            status_color: color(MUTED),
            load_error: None,
            flight: None,
            demo: false,
            recent: Vec::new(),
            recent_error: None,
            opening_index: None,
            recents: Latest::default(),
            opening: Latest::default(),
            team: Latest::default(),
            sending: Latest::default(),
            // WaitingForYou can return immediately. Pace repeat requests without
            // sleeping in poll or spinning the UI thread.
            waiting: Latest::default().debounce(Duration::from_millis(500)),
        }
    }
}

fn load_team(s: &mut State) {
    if s.demo || s.team.is_pending() || s.flight.is_some() {
        return;
    }
    s.load_error = None;
    s.status = "Loading teammates…".into();
    s.status_color = color(MUTED);
    s.team.request((), |()| Engine::local()?.teammates());
    refresh_recent(s);
}

fn setup(s: &mut State, waker: Waker) {
    s.team.set_waker(waker.clone());
    s.sending.set_waker(waker.clone());
    s.waiting.set_waker(waker.clone());
    s.recents.set_waker(waker.clone());
    s.opening.set_waker(waker);
    load_team(s);
}

fn can_send(s: &State) -> bool {
    !s.demo
        && s.load_error.is_none()
        && !s.team.is_pending()
        && s.flight.is_none()
        && s.opening_index.is_none()
        && s.selected < s.teammates.len()
        && !s.draft.text.trim().is_empty()
}

fn set_status(s: &mut State, index: usize, message: &str, ink: &str) {
    s.status = format!("{}: {message}", s.teammates[index].name);
    s.status_color = color(ink);
}

fn send(s: &mut State, _: &mut Tx) {
    if !can_send(s) {
        return;
    }
    let index = s.selected;
    let message = s.draft.text.clone();
    let teammate_id = s.teammates[index].id.clone();
    let conversation_id = s.conversations[index].conversation_id.clone();

    s.flight = Some(Flight {
        index,
        message: message.clone(),
    });
    set_status(s, index, "Sending…", MUTED);

    // Keep the draft until the engine confirms acceptance.
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

fn request_reply(s: &State, index: usize) {
    if let Some(id) = s.conversations[index].conversation_id.clone() {
        s.waiting.request(id, |id| Engine::local()?.reply(&id, 20));
    }
}

fn refresh_recent(s: &mut State) {
    if s.demo {
        return;
    }
    s.recents.request((), |()| Engine::local()?.conversations());
}

// Detaching never cancels an engine turn. Latest suppresses detached results,
// including a start/send that was already running when navigation occurred.
fn detach(s: &mut State) {
    s.sending.cancel();
    s.waiting.cancel();
    s.opening.cancel();
    s.flight = None;
    s.opening_index = None;
}

fn open_recent(s: &mut State, row: ConversationRow, tx: &mut Tx) {
    let Some(index) = s.teammates.iter()
        .position(|t| t.name.eq_ignore_ascii_case(&row.teammate))
    else {
        s.status = format!("{} is not in the current team.", row.teammate);
        s.status_color = color(MUTED);
        return;
    };
    if s.team.is_pending() {
        return;
    }

    detach(s);
    s.selected = index;
    s.conversations[index] = Conversation {
        conversation_id: Some(row.id.clone()),
        turns: vec![("You".into(), row.title)],
    };
    set_status(s, index, "Loading…", MUTED);
    tx.scroll_to_bottom("messages");

    if !s.demo {
        s.opening_index = Some(index);
        s.opening.request(row.id, |id| Engine::local()?.reply(&id, 0));
    }
}

fn fresh_conversation(s: &mut State) {
    if s.selected >= s.teammates.len() {
        return;
    }
    detach(s);
    s.conversations[s.selected] = Conversation::default();
    set_status(s, s.selected, "Ready", MUTED);
}

fn on_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.key == Key::Escape {
        s.draft.clear();
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
            let index = (digit as u8 - b'1') as usize;
            if index < s.teammates.len() {
                s.selected = index;
                tx.scroll_to_bottom("messages");
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

// Both an opened recent thread and a newly sent message share the same reply path.
fn land_reply(
    s: &mut State,
    index: usize,
    result: Result<Reply, String>,
    tx: &mut Tx,
) {
    match result {
        Ok(Reply::Working(message)) => {
            let message = if message.is_empty() {
                "Working…".into()
            } else {
                message
            };
            set_status(s, index, &message, GREEN);
            request_reply(s, index);
        }
        Ok(Reply::WaitingForYou(message)) => {
            set_status(s, index, &message, AMBER);
            request_reply(s, index);
        }
        Ok(Reply::Answer(message)) => {
            s.conversations[index].turns
                .push((s.teammates[index].name.clone(), message));
            set_status(s, index, "Ready", MUTED);
            s.flight = None;
            refresh_recent(s);
            if s.selected == index {
                tx.scroll_to_bottom("messages");
            }
        }
        Ok(Reply::Failed(error)) | Err(error) => {
            set_status(s, index, &error, RED);
            s.flight = None;
            refresh_recent(s);
        }
    }
}

fn poll(s: &mut State, _: f64, tx: &mut Tx) -> bool {
    if s.demo {
        return false;
    }
    let mut changed = false;

    if let Some(result) = s.team.take() {
        changed = true;
        match result {
            Ok(team) => {
                s.conversations = (0..team.len())
                    .map(|_| Conversation::default()).collect();
                s.teammates = team;
                s.selected = 0;
                s.load_error = None;
                s.status = if s.teammates.is_empty() {
                    "No teammates yet. Add a teammate in Locust.".into()
                } else {
                    "Ready".into()
                };
                s.status_color = color(MUTED);
            }
            Err(error) => {
                s.status = error.clone();
                s.status_color = color(RED);
                s.load_error = Some(error);
            }
        }
    }

    if let Some(result) = s.sending.take() {
        changed = true;
        if let Some(flight) = s.flight.as_ref() {
            let index = flight.index;
            let message = flight.message.clone();
            match result {
                Ok(started) => {
                    s.conversations[index].conversation_id =
                        Some(started.conversation_id);
                    s.conversations[index].turns
                        .push(("You".into(), message.clone()));
                    if s.draft.text == message {
                        s.draft.clear();
                    }
                    set_status(s, index, "Working…", GREEN);
                    request_reply(s, index);
                    if s.selected == index {
                        tx.scroll_to_bottom("messages");
                    }
                }
                Err(error) => {
                    set_status(s, index, &error, RED);
                    s.flight = None;
                }
            }
        }
    }

    if let Some(result) = s.recents.take() {
        changed = true;
        match result {
            Ok(rows) => {
                s.recent = rows.into_iter().take(12).collect();
                s.recent_error = None;
            }
            Err(words) => {
                s.recent.clear();
                s.recent_error = Some(words);
            }
        }
    }

    if let Some(result) = s.opening.take() {
        changed = true;
        if let Some(index) = s.opening_index.take() {
            s.flight = Some(Flight {
                index,
                message: String::new(),
            });
            land_reply(s, index, result, tx);
        }
    }

    if let Some(result) = s.waiting.take() {
        changed = true;
        if let Some(index) = s.flight.as_ref().map(|f| f.index) {
            land_reply(s, index, result, tx);
        }
    }

    changed
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

fn markdown_style() -> MarkdownStyle {
    MarkdownStyle::neutral(true)
        .text(color(TEXT))
        .muted(color(MUTED))
        .link(color(GREEN))
        .inline_code_bg(color(CARD))
        .code_block(color(CARD), hairline())
        .rule(hairline())
        .body_font(type_::BODY)
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let mut sidebar = col()
        .w(248.0).h_full().fill(color(SIDEBAR))
        .border_sides([0.0, 1.0, 0.0, 0.0], hairline())
        .child(
            col().w_full().h(88.0).pt(48.0).px(20.0).window_drag()
                .child(text("LOCUST").font(type_::LABEL).color(color(SECONDARY))),
        );

    let mut team_rows = col().w_full().gap(6.0).padding(10.0);
    for (index, teammate) in s.teammates.iter().enumerate() {
        let mut title = row().w_full().center_y().gap(8.0)
            .child(
                text(teammate.name.clone()).font(type_::TITLE)
                    .color(color(TEXT)).grow(1.0).shrink(),
            );

        if teammate.busy || s.flight.as_ref().is_some_and(|f| f.index == index) {
            title = title.child(
                el().size(6.0, 6.0).radius(3.0).fill(color(GREEN)),
            );
        }

        team_rows = team_rows.child(
            col().key(format!("teammate-{}", teammate.id))
                .w_full().padding(12.0).gap(5.0).radius(8.0)
                .fill(color(if s.selected == index { SELECTED } else { SIDEBAR }))
                .child(title)
                .child(
                    text(teammate.role.clone()).font(type_::BODY)
                        .color(color(SECONDARY)).w_full().wrap_anywhere(),
                )
                .child(
                    text(agent(teammate)).font(type_::LABEL)
                        .color(color(MUTED)).w_full().wrap_anywhere(),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    s.selected = index;
                    tx.scroll_to_bottom("messages");
                }),
        );
    }

    let mut recent = col().w_full().px(10.0).pb(16.0).gap(4.0);
    recent = recent.child(
        row().w_full().center_y().px(12.0).py(10.0)
            .child(
                text("Recent").font(type_::LABEL)
                    .color(color(SECONDARY)).grow(1.0),
            )
            .child(
                text("Refresh").key("refresh-recent").font(type_::LABEL)
                    .color(color(MUTED)).on_press(refresh_recent),
            ),
    );

    let now = clock_seconds();
    if let Some(words) = &s.recent_error {
        recent = recent.child(
            text(words.clone()).font(type_::LABEL).color(color(MUTED))
                .w_full().px(12.0).wrap_anywhere(),
        );
    } else if s.recent.is_empty() {
        recent = recent.child(
            text(if s.recents.is_pending() {
                "Loading…"
            } else {
                "No recent conversations."
            })
            .font(type_::LABEL).color(color(MUTED))
            .w_full().px(12.0).wrap(),
        );
    } else {
        for item in s.recent.iter().take(12) {
            let picked = s.teammates.get(s.selected)
                .and_then(|_| s.conversations.get(s.selected))
                .and_then(|c| c.conversation_id.as_deref())
                == Some(item.id.as_str());

            let mut title = row().w_full().center_y().gap(6.0)
                .child(
                    text(item.title.clone()).font(type_::BODY)
                        .color(color(TEXT)).grow(1.0).shrink().truncate(),
                );
            let dot = match item.state.as_str() {
                "running" => Some(GREEN),
                "failed" => Some(RED),
                _ => None,
            };
            if let Some(ink) = dot {
                title = title.child(
                    el().size(5.0, 5.0).radius(2.5).fill(color(ink)),
                );
            }

            let item = item.clone();
            recent = recent.child(
                col().key(format!("recent-{}", item.id)).w_full()
                    .padding(12.0).gap(5.0).radius(8.0)
                    .fill(color(if picked { SELECTED } else { SIDEBAR }))
                    .child(title)
                    .child(
                        text(format!(
                            "{} · {}",
                            item.teammate,
                            when(&item.updated, now)
                        ))
                        .font(type_::LABEL).color(color(MUTED))
                        .w_full().truncate(),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        open_recent(s, item.clone(), tx);
                    }),
            );
        }
    }

    sidebar = sidebar.child(
        col().key("team-list").w_full().grow(1.0).min_h(0.0)
            .scroll(ScrollSpec { stick_bottom: false })
            .child(col().w_full().child(team_rows).child(recent)),
    );

    let right = if let Some(error) = &s.load_error {
        col().grow(1.0).shrink().h_full().center().padding(32.0).gap(18.0)
            .child(
                text(error.clone()).font(type_::BODY).color(color(RED))
                    .align(Align::Center).w_full().wrap_anywhere().selectable(true),
            )
            .child(
                el().key("retry").h(40.0).px(18.0).center().radius(8.0)
                    .fill(color(CARD)).border(1.0, hairline())
                    .child(text("Try again").font(type_::LABEL).color(color(TEXT)))
                    .on_press(load_team),
            )
    } else if let Some(teammate) = s.teammates.get(s.selected) {
        let style = markdown_style();
        let mut messages = col().w_full().gap(24.0).padding(24.0);

        for (index, (who, body)) in
            s.conversations[s.selected].turns.iter().enumerate()
        {
            let key = format!("turn-{}-{index}", teammate.id);
            if who == "You" {
                // The right-aligned wrapper bounds the bubble to 70%; the text
                // keeps its intrinsic width until it needs to wrap.
                messages = messages.child(
                    row().key(key).w_full().justify_end()
                        .child(
                            row().w_frac(0.7).justify_end()
                                .child(
                                    text(body.clone()).font(type_::BODY)
                                        .color(color(TEXT)).shrink()
                                        .wrap_anywhere().selectable(true)
                                        .padding(12.0).radius(10.0)
                                        .fill(color(YOUR_BUBBLE))
                                        .border(1.0, hairline()),
                                ),
                        ),
                );
            } else {
                messages = messages.child(
                    col().key(key.clone()).w_full().gap(8.0)
                        .child(text(who.clone()).font(type_::LABEL).color(color(MUTED)))
                        .child(
                            Markdown::new(body.clone()).selectable(true)
                                .render(&style)
                                .key(format!("{key}-markdown")).w_full(),
                        ),
                );
            }
        }

        let transcript = if s.conversations[s.selected].turns.is_empty() {
            col().key("messages").w_full().grow(1.0).min_h(0.0)
                .center().padding(24.0).gap(10.0)
                .child(
                    text(format!("Ask {} anything", teammate.name))
                        .font(type_::TITLE).color(color(TEXT))
                        .w_full().align(Align::Center).wrap(),
                )
                .child(
                    text(format!(
                        "{} · runs in {}",
                        agent(teammate),
                        mode_word(&teammate.runs_in)
                    ))
                    .font(type_::LABEL).color(color(MUTED))
                    .w_full().align(Align::Center).wrap_anywhere(),
                )
        } else {
            col().key("messages").w_full().grow(1.0).min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: true }).child(messages)
        };

        let mut status = row().w_full().center_y().gap(8.0).px(24.0).py(10.0);
        if s.flight.is_some() || s.opening_index.is_some() {
            status = status.child(
                el().key("activity-dot").size(6.0, 6.0).radius(3.0)
                    .fill(s.status_color)
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
            text(s.status.clone()).font(type_::LABEL).color(s.status_color)
                .grow(1.0).shrink().wrap_anywhere().selectable(true),
        );

        let composer = text_input(&s.draft, type_::BODY)
            .key("composer").single_line()
            .placeholder(format!("Message {}", teammate.name), color(MUTED))
            .color(color(TEXT)).grow(1.0).shrink().h(48.0).padding(12.0)
            .radius(9.0).fill(color(FIELD)).border(1.0, hairline())
            .on_edit(|s: &mut State| &mut s.draft)
            .on_submit(send)
            .on_input_key(on_key);

        let enabled = can_send(s);
        let mut button = el().key("send").h(48.0).px(18.0).center().radius(9.0)
            .fill(color(CARD)).border(1.0, hairline()).disabled(!enabled)
            .child(
                text("Send").font(type_::LABEL)
                    .color(color(if enabled { TEXT } else { MUTED })),
            );
        if enabled {
            button = button.on_press_tx(send);
        }

        col().grow(1.0).shrink().h_full()
            .child(
                col().w_full().h(88.0).px(24.0).pt(30.0).gap(5.0).window_drag()
                    .border_sides([0.0, 0.0, 1.0, 0.0], hairline())
                    .child(
                        text(teammate.name.clone()).font(type_::TITLE).color(color(TEXT)),
                    )
                    .child(text(agent(teammate)).font(type_::LABEL).color(color(MUTED))),
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
                text(s.status.clone()).font(type_::BODY).color(s.status_color)
                    .w_full().wrap(),
            )
    };

    row().w_full().h_full().fill(color(BG)).child(sidebar).child(right)
}

fn team_scene(s: &mut State) {
    s.demo = true;
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

    s.conversations = (0..s.teammates.len())
        .map(|_| Conversation::default()).collect();
    s.conversations[0].turns = vec![
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
    s.status_color = color(MUTED);
}

fn empty_scene(s: &mut State) {
    team_scene(s);
    for conversation in &mut s.conversations {
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
    std::env::var_os("LOCUST_NATIVE_REPLAY").map(Into::into).unwrap_or_else(|| std::env::temp_dir().join(REPLAY_FILE))
}

fn ask_cli(who: &str, message: &str) -> Result<(), String> {
    let engine = Engine::local()?;
    let team = engine.teammates()?;
    let teammate = team.iter().find(|t| t.name.eq_ignore_ascii_case(who) || t.id == who).ok_or_else(|| format!("No teammate called {who}."))?;
    let started = engine.start(&teammate.id, message)?;
    eprintln!("started {} in {}", started.conversation_id, started.mode);
    let answer = loop {
        match engine.reply(&started.conversation_id, engine::WAIT_LIMIT_S)? {
            Reply::Working(doing) => eprintln!("working: {doing}"),
            Reply::WaitingForYou(words) => { eprintln!("waiting for you in Locust: {words}"); std::thread::sleep(Duration::from_secs(2)); }
            Reply::Answer(text) => break text,
            Reply::Failed(words) => return Err(words),
        }
    };
    println!("{answer}");
    let saved = serde_json::json!({ "team": team.iter().map(|t| serde_json::json!({ "id": t.id, "name": t.name, "role": t.role, "runtime": t.runtime, "model": t.model, "busy": t.busy, "runs_in": t.runs_in })).collect::<Vec<_>>(), "teammate": teammate.id, "message": message, "answer": answer });
    std::fs::write(replay_path(), saved.to_string()).map_err(|e| e.to_string())
}

fn replay_scene(s: &mut State) {
    s.demo = true;
    let Some(saved) = std::fs::read_to_string(replay_path()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
        s.status = "No saved exchange: run --ask first.".into();
        return;
    };
    s.teammates = engine::parse_teammates(&saved["team"].to_string()).unwrap_or_default();
    s.conversations = (0..s.teammates.len()).map(|_| Conversation::default()).collect();
    let picked = s.teammates.iter().position(|t| saved["teammate"] == t.id.as_str()).unwrap_or(0);
    if let Some(conversation) = s.conversations.get_mut(picked) {
        conversation.turns = vec![
            ("You".into(), saved["message"].as_str().unwrap_or_default().into()),
            (s.teammates[picked].name.clone(), saved["answer"].as_str().unwrap_or_default().into()),
        ];
    }
    s.selected = picked;
    s.status = "Ready".into();
    s.status_color = color(MUTED);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--ask") {
        let (who, message) = (args.get(2).cloned().unwrap_or_default(), args.get(3).cloned().unwrap_or_default());
        if let Err(words) = ask_cli(&who, &message) { eprintln!("{words}"); std::process::exit(1); }
        return;
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
        ],
        ..AppSpec::new("Locust", (1040.0, 720.0), State::default, view)
    })
    .chrome(Chrome {
        full_size_content: true,
        traffic_lights: Some((20.0, 18.0)),
        drag_regions: true,
    })
    .on_close(|s, _| {
        s.team.cancel();
        s.sending.cancel();
        s.waiting.cancel();
        s.recents.cancel();
        s.opening.cancel();
    })
    .run();
}
