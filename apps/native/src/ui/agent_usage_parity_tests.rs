use super::*;
use rune::{Env, Runtime};

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-10-10T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn card(reading: Option<&'static str>) -> Vec<rune::ElementInfo> {
    let mut rt = Runtime::new(
        reading,
        |reading, _| {
            let l = crate::look::controls::Look::new(true);
            let model = agent_card("Claude Code", "Ready · 2.1.283", *reading, now());
            draw(&l, "claude", &model, reading.unwrap_or(""), now())
        },
        (500.0, 500.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
}

// case: reads as windows, fullest first, each with its reset
#[test]
fn usage_card_orders_the_fuller_window_first_and_prints_its_reset() {
    let el = card(Some(
        "5-hour window 35% used · resets 2026-10-10T15:00:00Z · 7-day window 12% used",
    ));
    let by_key = |key| el.iter().find(|el| el.key.as_deref() == Some(key)).unwrap();
    assert_eq!(by_key("usage-meter-0-name").text.as_deref(), Some("5-hour"));
    assert_eq!(by_key("usage-meter-0-figure").text.as_deref(), Some("35%"));
    assert_eq!(by_key("usage-meter-1-name").text.as_deref(), Some("7-day"));
    assert_eq!(by_key("usage-meter-1-figure").text.as_deref(), Some("12%"));
    assert_eq!(
        el.iter()
            .filter(|el| el.text.as_deref().is_some_and(|t| t.starts_with("resets ")))
            .count(),
        1
    );
}

// case: opens a card with its name, its state, and a bar for each window
// Existing real pointer test resting_on_the_model_chip_opens_the_agents_card_above_it opens the card.
#[test]
fn an_account_card_has_its_name_state_and_both_figures() {
    let words = card(Some("5-hour window 35% used · 7-day window 12% used"))
        .into_iter()
        .filter_map(|el| el.text)
        .collect::<Vec<_>>();
    for word in ["Claude Code", "Ready · 2.1.283", "35%", "12%"] {
        assert!(words.iter().any(|t| t == word), "{word}: {words:?}");
    }
}

// case: draws a bar amber from 80% and red when spent
#[test]
fn an_account_card_paints_79_80_and_100_in_three_different_tones() {
    let l = crate::look::controls::Look::new(true);
    for (reading, colour) in [
        ("5-hour window 79% used", "text-secondary"),
        ("5-hour window 80% used", "amber"),
        ("5-hour window 100% used", "red"),
    ] {
        let el = card(Some(reading));
        let figure = el
            .iter()
            .find(|el| el.key.as_deref() == Some("usage-meter-0-figure"))
            .unwrap();
        assert_eq!(figure.text_color, Some(l.c(colour)), "{reading}");
    }
}

// case: draws no meter for an agent whose runs reported none: its name and state alone
#[test]
fn a_card_without_a_report_draws_no_meter_or_read_line() {
    let el = card(None);
    assert!(
        el.iter()
            .any(|el| el.text.as_deref() == Some("Claude Code"))
    );
    assert!(
        el.iter()
            .any(|el| el.text.as_deref() == Some("Ready · 2.1.283"))
    );
    assert!(!el.iter().any(|el| {
        el.key
            .as_deref()
            .is_some_and(|k| k.starts_with("usage-meter"))
    }));
}

// case: draws a window reset since the reading as reset, without its old bar, and says when the reading was taken
#[test]
fn a_reset_window_drops_its_old_bar_but_keeps_the_other_window_and_reading_time() {
    let el = card(Some(
        "5-hour window 20% used · resets 2026-10-10T11:00:00Z · 7-day window 66% used · from a run at 2026-10-10T09:00:00Z",
    ));
    let words = el
        .iter()
        .filter_map(|el| el.text.as_deref())
        .collect::<Vec<_>>();
    assert!(
        words.iter().any(|t| t.starts_with("reset since")),
        "{words:?}"
    );
    assert!(
        words.contains(&"66%") && !words.contains(&"20%"),
        "{words:?}"
    );
    assert!(
        el.iter()
            .any(|el| el.key.as_deref() == Some("usage-meters-read"))
    );
}
