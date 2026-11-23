use super::*;
use rune::{Env, Runtime};

const READING: &str = "weekly window 64% used · resets 2099-11-04T06:36:18.000Z · 5-hour window 15% left · as of 2026-11-23T10:00:00.000Z";

#[test]
fn hovering_the_model_chip_draws_the_usage_meters_with_their_own_tones_and_reading() {
    let mut s = State::default();
    s.chip_hover = true;
    s.usage_readings.insert("codex".into(), READING.into());
    let mut rt = Runtime::new(
        s,
        |s, _| {
            let l = crate::look::controls::Look::new(true);
            col()
                .child(
                    el().key("model-chip")
                        .absolute(300.0, 260.0)
                        .size(40.0, 20.0),
                )
                .children(chip_card(s, &l, "codex", None))
        },
        (400.0, 300.0),
        Env::new(true, 1.0),
    );
    for _ in 0..8 {
        rt.frame(0.05);
    }
    assert!(rt.find("model-chip-card").is_some());
    let elements = rt.elements();
    let by_key = |key: &str| {
        elements
            .iter()
            .find(|el| el.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("missing {key}"))
    };
    let tooltip = by_key("usage-meters").help.as_deref().unwrap();
    assert!(
        tooltip.contains("64% of the weekly window used"),
        "{tooltip}"
    );
    assert!(
        tooltip.contains("15% of the 5-hour window left"),
        "{tooltip}"
    );

    let l = crate::look::controls::Look::new(true);
    let weekly = by_key("usage-meter-0-figure");
    let pressing = by_key("usage-meter-1-figure");
    assert_eq!(weekly.text.as_deref(), Some("64%"));
    assert_eq!(weekly.text_color, Some(l.c("text-secondary")));
    assert_eq!(pressing.text.as_deref(), Some("15% left"));
    assert_eq!(pressing.text_color, Some(l.c("amber")));
    assert!(
        by_key("usage-meters-read")
            .text
            .as_deref()
            .unwrap()
            .starts_with("Read from your account at ")
    );
}

// an-account-shows-how-full-it-is, the-model-chip-shows-usage-on-hover, antigravity-shows-remaining-not-used (Electron
// b7bb4e53): the card names each window without a trailing " window", first letter capital.
#[test]
fn the_chip_card_names_each_window_without_a_trailing_window() {
    let mut s = State::default();
    s.chip_hover = true;
    s.usage_readings.insert("codex".into(), READING.into());
    let mut rt = Runtime::new(
        s,
        |s, _| {
            let l = crate::look::controls::Look::new(true);
            col()
                .child(el().key("model-chip").absolute(300.0, 260.0).size(40.0, 20.0))
                .children(chip_card(s, &l, "codex", None))
        },
        (400.0, 300.0),
        Env::new(true, 1.0),
    );
    for _ in 0..8 {
        rt.frame(0.05);
    }
    let elements = rt.elements();
    let name = |key: &str| {
        elements
            .iter()
            .find(|el| el.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("missing {key}"))
            .text
            .clone()
    };
    assert_eq!(name("usage-meter-0-name").as_deref(), Some("Weekly"));
    assert_eq!(name("usage-meter-1-name").as_deref(), Some("5-hour"));

    assert_eq!(card_label("weekly window"), "Weekly");
    assert_eq!(card_label("5-hour window"), "5-hour");
    assert_eq!(card_label("Gemini: weekly window"), "Gemini: weekly");
    assert_eq!(card_label("window"), "Window", "only a trailing \" window\" goes");
}
