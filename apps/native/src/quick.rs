//! Retained Quick Ask panel. All engine I/O stays off the UI thread.
use crate::engine::{Engine, Reply, Started, Teammate};
use crate::{Palette, markdown_style};
use rune::tok::type_;
use rune::{
    Align, App, AppHandle, AppSpec, Chrome, Cx, DisplayTarget, El, Key, KeyEvent,
    Latest, OverlayPlacement, RunOptions, ScreenEdge, ScrollSpec, Shortcut,
    TextEdit, Tx, Waker, WindowOptions, WindowSpec, col, el, row, text, text_input,
};
use rune_kit::Markdown;
use std::time::Duration;

const WIDTH: f32 = 680.0;
const MAX_HEIGHT: f32 = 520.0;
const UNAVAILABLE: &str =
    "Locust isn't running, or 'Let your other AI apps use Locust' is off.";

#[derive(Clone, Copy)]
enum Activity {
    Working,
    WaitingForYou,
}

struct Flight {
    teammate_id: String,
    name: String,
    conversation_id: Option<String>,
}

// Public only because scenes() returns AppSpec<State>; its fields stay private.
pub struct State {
    app: Option<AppHandle<State>>,
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
    activity: Activity,
    flight: Option<Flight>,
    team: Latest<Result<Vec<Teammate>, String>>,
    sending: Latest<Result<Started, String>>,
    waiting: Latest<Result<Reply, String>>,
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
            activity: Activity::Working,
            flight: None,
            team: Latest::default(),
            sending: Latest::default(),
            waiting: Latest::default(),
        }
    }
}

fn setup(s: &mut State, waker: Waker) {
    s.team.set_waker(waker.clone());
    s.sending.set_waker(waker.clone());
    s.waiting.set_waker(waker);
    // The hidden launcher's prepared first frame must not contact the engine.
}

fn begin_show(s: &mut State) {
    s.visible = true;
    s.draft.clear();
    s.draft.focused = true;
    s.error = None;
    // Keep the previous question/answer until the next send.
    if !s.demo {
        s.team.request((), |()| Engine::local()?.teammates());
    }
}

fn stop_watching(s: &mut State) {
    // Latest drops late results; it cannot stop a turn already running in Locust.
    s.sending.cancel();
    s.waiting.cancel();
    s.flight = None;
}

fn hide(s: &mut State) {
    s.visible = false;
    s.draft.focused = false;
    if let Some(app) = &s.app {
        app.hide();
    }
}

fn toggle(s: &mut State, tx: &mut Tx) {
    if s.visible {
        hide(s);
        return;
    }
    begin_show(s);
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
        // Tokens matter on Wayland; open() takes keys without activating a Mac panel.
        if let Some(token) = tx.activation_token() {
            app.activate(token);
        } else {
            app.open();
        }
    }
}

fn cycle(s: &mut State, backwards: bool) {
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

fn engine_error(s: &mut State) {
    stop_watching(s);
    s.error = Some(UNAVAILABLE.into());
}

fn land_team(s: &mut State, result: Result<Vec<Teammate>, String>) {
    match result {
        Ok(team) if !team.is_empty() => {
            let selected_id = s.teammates.get(s.selected).map(|t| t.id.clone());
            s.selected = selected_id.as_deref()
                .and_then(|id| team.iter().position(|t| t.id == id))
                .or_else(|| team.iter().position(|t| t.name == "Codec"))
                .unwrap_or(0);
            if let Some(flight) = s.flight.as_mut()
                && let Some(teammate) =
                    team.iter().find(|t| t.id == flight.teammate_id)
            {
                flight.name = teammate.name.clone();
            }
            s.teammates = team;
            s.error = None;
        }
        _ => {
            s.teammates.clear();
            s.selected = 0;
            engine_error(s);
        }
    }
}

fn send(s: &mut State, _: &mut Tx) {
    if s.demo
        || s.flight.is_some()
        || s.team.is_pending()
        || s.draft.text.trim().is_empty()
    {
        return;
    }
    let Some(teammate) = s.teammates.get(s.selected) else {
        engine_error(s);
        return;
    };
    let id = teammate.id.clone();
    let name = teammate.name.clone();
    let message = s.draft.text.clone();

    s.question = message.clone();
    s.answer = None;
    s.answer_name = name.clone();
    s.error = None;
    s.activity = Activity::Working;
    s.flight = Some(Flight {
        teammate_id: id.clone(),
        name,
        conversation_id: None,
    });
    s.draft.clear();

    s.sending.request((id, message), |(id, message)| {
        Engine::local()?.start(&id, &message)
    });
}

fn request_reply(s: &mut State, delay: Duration) {
    let Some(id) = s.flight.as_ref()
        .and_then(|f| f.conversation_id.clone())
    else {
        engine_error(s);
        return;
    };
    // Moving this handle preserves its installed waker and cancellation generation.
    s.waiting = std::mem::take(&mut s.waiting).debounce(delay);
    s.waiting.request(id, |id| Engine::local()?.reply(&id, 20));
}

fn land_reply(s: &mut State, result: Result<Reply, String>, tx: &mut Tx) {
    if s.flight.is_none() {
        return;
    }
    match result {
        Ok(Reply::Working(_)) => {
            s.activity = Activity::Working;
            request_reply(s, Duration::from_millis(500));
        }
        Ok(Reply::WaitingForYou(_)) => {
            s.activity = Activity::WaitingForYou;
            request_reply(s, Duration::from_secs(3));
        }
        Ok(Reply::Answer(answer)) => {
            // The current engine returns a completed answer, not token chunks.
            s.answer = Some(answer);
            s.flight = None;
            tx.scroll_to_bottom("quick-messages");
        }
        Ok(Reply::Failed(words)) => {
            stop_watching(s);
            s.error = Some(words);
        }
        Err(_) => engine_error(s),
    }
}

fn poll(s: &mut State, _: f64, tx: &mut Tx) -> bool {
    if s.demo {
        return false;
    }
    let mut changed = false;

    if let Some(result) = s.team.take() {
        changed = true;
        land_team(s, result);
    }
    if let Some(result) = s.sending.take() {
        changed = true;
        if s.flight.is_some() {
            match result {
                Ok(started) => {
                    if let Some(flight) = s.flight.as_mut() {
                        flight.conversation_id = Some(started.conversation_id);
                    }
                    request_reply(s, Duration::ZERO);
                }
                Err(_) => engine_error(s),
            }
        }
    }
    if let Some(result) = s.waiting.take() {
        changed = true;
        land_reply(s, result, tx);
    }

    changed
}

fn key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.key == Key::Escape {
        if s.flight.is_some() {
            stop_watching(s);
        }
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
    let name = s.teammates.get(s.selected)
        .map(|t| t.name.as_str()).unwrap_or("Locust");
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

    if let Some(answer) = &s.answer {
        output = output
            .child(text(s.answer_name.clone()).font(type_::LABEL).color(p.muted))
            .child(
                Markdown::new(answer.clone()).selectable(true)
                    .render(&markdown_style(&p))
                    .key("quick-answer").w_full(),
            );
    }

    if !s.question.is_empty()
        || s.error.is_some()
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
    use super::*;

    #[test]
    fn tab_cycles_both_directions_and_wraps() {
        let mut s = State::default();
        sample(&mut s);
        let mut tx = Tx::default();
        let tab = KeyEvent {
            key: Key::Tab,
            mods: rune::Mods::default(),
        };
        assert!(key(&mut s, &tab, &mut tx));
        assert_eq!(s.selected, 1);
        assert!(key(&mut s, &tab, &mut tx));
        assert_eq!(s.selected, 0);

        let back = KeyEvent {
            key: Key::Tab,
            mods: rune::Mods {
                shift: true,
                ..Default::default()
            },
        };
        assert!(key(&mut s, &back, &mut tx));
        assert_eq!(s.selected, 1);

        s.teammates.clear();
        assert!(key(&mut s, &tab, &mut tx));
    }

    #[test]
    fn escape_during_flight_stops_watching_and_hides() {
        let mut s = State::default();
        working_scene(&mut s);
        let escape = KeyEvent {
            key: Key::Escape,
            mods: rune::Mods::default(),
        };
        assert!(key(&mut s, &escape, &mut Tx::default()));
        assert!(s.flight.is_none());
        assert!(!s.visible);
        assert!(!s.sending.is_pending() && !s.waiting.is_pending());
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
    fn engine_errors_and_empty_teams_show_the_unavailable_message() {
        let mut s = State::default();
        sample(&mut s);
        land_team(&mut s, Err("connection refused".into()));
        assert_eq!(s.error.as_deref(), Some(UNAVAILABLE));
        assert!(s.flight.is_none());

        land_team(&mut s, Ok(Vec::new()));
        assert_eq!(s.error.as_deref(), Some(UNAVAILABLE));
        assert!(s.teammates.is_empty());
    }
}
