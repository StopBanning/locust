//! R50 / Electron `an-account-shows-how-full-it-is.test.tsx`: Home's folded accounts line draws
//! ready agent marks that name themselves and their usage, and open a status + usage card on hover
//! or press (without an empty meter when no usage was reported).

use super::*;
use locust_core::agents::{Auth, Found, Probe};

const RESETS: &str = "2099-09-27T22:10:00.000Z";

fn ready(id: &str, name: &str, version: Option<&str>) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: true,
        version: version.map(str::to_owned),
        auth: Auth::Authenticated,
        ready: true,
        status: Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

fn home_with_agents(agents: Vec<Found>, readings: &[(&str, &str)]) -> rune::Runtime<State> {
    let mut s = State::default();
    crate::team_scene(&mut s);
    s.settings_panel.agents = Some(agents);
    for (k, v) in readings {
        s.usage_readings.insert((*k).into(), (*v).into());
    }
    let mut rt = rune::Runtime::new(
        s,
        |s, _| home_view(s, &Palette::new(true)),
        (1215.0, 1200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn texts_in(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

fn texts_inside_card(rt: &rune::Runtime<State>, card_key: &str) -> Vec<String> {
    let card = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(card_key))
        .expect("agent card drawn");
    let [x0, y0, w, h] = card.rect;
    rt.elements()
        .into_iter()
        .filter(|e| {
            let [ex, ey, ew, eh] = e.rect;
            ex >= x0 - 1.0 && ey >= y0 - 1.0 && ex + ew <= x0 + w + 1.0 && ey + eh <= y0 + h + 1.0
        })
        .filter_map(|e| e.text)
        .collect()
}

// case: is a thing to point at, named with everything its card says
#[test]
fn home_agent_mark_is_focusable_and_named_with_everything_its_card_says() {
    let rt = home_with_agents(
        vec![ready("claude", "Claude Code", Some("2.1.283"))],
        &[("claude", &format!("5-hour window 35% used · resets {RESETS}"))],
    );
    let mark = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("home-agents-mark-claude"))
        .expect("claude mark");
    assert!(mark.focusable, "agent mark is keyboard-reachable (tabindex=0)");
    assert_eq!(mark.role, "img");
    let label = mark.label.as_deref().unwrap_or("");
    assert!(
        label.starts_with("Claude Code. Ready · 2.1.283. 35% of the 5-hour window used, resets "),
        "unexpected mark label: {label:?}"
    );
}

// case: opens a card with its name, its state, and a bar for each window
#[test]
fn hovering_or_pressing_home_agent_mark_opens_its_card_with_name_state_and_bars() {
    let reading = format!("5-hour window 35% used · resets {RESETS} · 7-day window 12% used");
    let mut rt = home_with_agents(
        vec![ready("claude", "Claude Code", Some("2.1.283"))],
        &[("claude", &reading)],
    );
    assert!(rt.find("home-agents-card-claude").is_none(), "closed at rest");

    // Hover opens the card above the mark.
    let (mx, my) = rt.find("home-agents-mark-claude").expect("mark coords");
    rt.pointer_move(mx, my);
    rt.frame(0.0);
    assert!(rt.find("home-agents-card-claude").is_some(), "hover opens agent card");
    let lines = texts_inside_card(&rt, "home-agents-card-claude");
    let joined = lines.join("\n");
    assert!(joined.contains("Claude Code"), "{joined}");
    assert!(joined.contains("Ready · 2.1.283"), "{joined}");
    assert!(joined.contains("35%"), "{joined}");
    assert!(joined.contains("12%"), "{joined}");
    assert!(joined.contains("resets "), "{joined}");
    assert!(rt.find("usage-meter-0").is_some());
    assert!(rt.find("usage-meter-1").is_some());

    // Moving away closes the hovered card.
    rt.pointer_move(5.0, 5.0);
    rt.frame(0.0);
    assert!(rt.find("home-agents-card-claude").is_none(), "hover leave closes card");

    // Pressing toggles the card pinned open, and pressing again closes it.
    rt.pointer_down(mx, my);
    rt.pointer_up(mx, my);
    rt.pointer_move(5.0, 5.0);
    rt.frame(0.0);
    assert!(rt.find("home-agents-card-claude").is_some(), "press pins card open");
    rt.pointer_down(mx, my);
    rt.pointer_up(mx, my);
    rt.pointer_move(5.0, 5.0);
    rt.frame(0.0);
    assert!(rt.find("home-agents-card-claude").is_none(), "second press closes pinned card");
}

// case: draws a bar amber from 80% and red when spent
#[test]
fn home_agent_card_bar_is_quiet_under_80_amber_from_80_and_red_when_spent() {
    use locust_core::agent_card::{Tone, agent_card, tone_of};
    let l = crate::look::controls::Look::new(true);
    let now = chrono::Utc::now();

    assert_eq!(tone_of(79), Tone::Calm);
    assert_eq!(tone_of(80), Tone::Pressing);
    assert_eq!(tone_of(100), Tone::Spent);
    assert_eq!(crate::agent_card_ui::bar_colour(&l, Tone::Calm), l.c("text-secondary"));
    assert_eq!(crate::agent_card_ui::bar_colour(&l, Tone::Pressing), l.c("amber"));
    assert_eq!(crate::agent_card_ui::bar_colour(&l, Tone::Spent), l.c("red"));

    for (usage, expected_pct) in [
        ("5-hour window 79% used", "79%"),
        ("5-hour window 80% used", "80%"),
        ("weekly window 100% used", "100%"),
    ] {
        let card_val = agent_card("Claude Code", "Ready · 2.1.283", Some(usage), now);
        assert_eq!(card_val.windows.len(), 1);
        let mut rt = home_with_agents(
            vec![ready("claude", "Claude Code", Some("2.1.283"))],
            &[("claude", usage)],
        );
        let (mx, my) = rt.find("home-agents-mark-claude").expect("mark");
        rt.pointer_move(mx, my);
        rt.frame(0.0);
        let fig = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("usage-meter-0-figure"))
            .expect("meter figure");
        assert_eq!(fig.text.as_deref(), Some(expected_pct));
    }
}

// case: draws no meter for an agent whose runs reported none: its name and state alone
#[test]
fn home_agent_mark_draws_no_meter_when_runs_reported_none() {
    let mut rt = home_with_agents(vec![ready("claude", "Claude Code", Some("2.1.283"))], &[]);
    let mark = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("home-agents-mark-claude"))
        .expect("mark");
    assert_eq!(mark.label.as_deref(), Some("Claude Code. Ready · 2.1.283"));

    let (mx, my) = rt.find("home-agents-mark-claude").expect("mark coords");
    rt.pointer_move(mx, my);
    rt.frame(0.0);
    assert!(rt.find("home-agents-card-claude").is_some(), "card opens with name and state alone");
    assert!(rt.find("usage-meters").is_none(), "no empty meter drawn");
    let lines = texts_inside_card(&rt, "home-agents-card-claude").join("\n");
    assert!(lines.contains("Claude Code") && lines.contains("Ready · 2.1.283"), "{lines}");
}

// case: draws a window reset since the reading as reset, without its old bar, and says when the reading was taken
#[test]
fn home_agent_card_draws_reset_window_without_old_bar_and_says_when_taken() {
    let mut rt = rune::Runtime::new(
        State::default(),
        |_, _| {
            let said = "7-day window 66% used · resets 2026-09-28T07:00:00.000Z · 5-hour window 20% used · resets 2026-09-27T05:30:00.000Z · from a run at 2026-09-27T01:23:19.233Z";
            let now = chrono::DateTime::parse_from_rfc3339("2026-09-27T15:30:00.000Z")
                .unwrap()
                .with_timezone(&chrono::Utc);
            let l = crate::look::controls::Look::new(true);
            home_agent_card(&l, "claude", "Claude Code", "Ready", Some(said), now)
        },
        (400.0, 300.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let joined = texts_in(&rt).join("\n");
    assert!(joined.contains("66%"), "active window keeps its bar: {joined}");
    assert!(!joined.contains("20%"), "reset window drops its stale percentage: {joined}");
    assert!(joined.contains("reset since ("), "reset window says reset since: {joined}");
    assert!(
        joined.contains("From Locust's last run on it,") && joined.contains("Use outside Locust since then isn't counted."),
        "source line says when reading was taken: {joined}"
    );
}

// case: is the marks alone, each carrying its card; no rings
#[test]
fn home_agents_line_has_a_mark_per_ready_agent_with_each_carrying_its_own_card() {
    let reading = format!("5-hour window 35% used · resets {RESETS}");
    let rt = home_with_agents(
        vec![
            ready("codex", "Codex CLI", Some("2.1.283")),
            ready("claude", "Claude Code", Some("2.1.283")),
        ],
        &[("claude", &reading)],
    );
    let marks: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("home-agents-mark-")))
        .collect();
    assert_eq!(marks.len(), 2);
    let claude = marks.iter().find(|e| e.key.as_deref() == Some("home-agents-mark-claude")).unwrap();
    let codex = marks.iter().find(|e| e.key.as_deref() == Some("home-agents-mark-codex")).unwrap();
    assert!(
        claude.label.as_deref().unwrap_or("").starts_with("Claude Code. Ready · 2.1.283. 35% of the 5-hour window used"),
        "{:?}",
        claude.label
    );
    assert_eq!(codex.label.as_deref(), Some("Codex CLI. Ready · 2.1.283"));
}
