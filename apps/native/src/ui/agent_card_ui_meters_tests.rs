use super::*;
use rune::{Env, Runtime};

const READING: &str = "weekly window 64% used · resets 2099-11-04T06:36:18.000Z · 5-hour window 15% left · as of 2026-11-23T10:00:00.000Z";

// case: opens the agent's card, with a bar for each window, placed for the chip
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

// ---- every-limit-says-what-is-left.test.tsx ---------------------------------------------------------------------

#[test]
fn say_what_is_left_when_the_runtime_reported_what_it_used() {
    // case: say what is left, when the runtime reported what it used
    // Electron's reading, drawn as UsageMeters draws it (Settings > AI agents): Codex reports what it USED.
    let mut rt = Runtime::new(
        State::default(),
        |_, _| {
            let l = crate::look::controls::Look::new(true);
            let now = chrono::DateTime::parse_from_rfc3339("2026-10-10T12:00:00.000Z").unwrap().with_timezone(&chrono::Utc);
            usage_meters(&l, "weekly window 33% used · 5-hour window 0% used", now)
        },
        (900.0, 200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let words: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(words.iter().any(|w| w == "67% left"), "{words:?}");
    assert!(words.iter().any(|w| w == "100% left"), "{words:?}");
    assert!(!words.iter().any(|w| w.contains("% used")), "the same bar in one language: {words:?}");
}

// case: is quiet with room left, and wears its amber dot from 80% -- as before the rings
/// Composer.tsx's model button: no `is-pressing` below 80%, the amber dot from 80% (`.lc-control.is-pressing::after`),
/// and no usage ring at any reading. Drawn through the conversation view the person sees, on Codex's 5-hour window.
#[test]
fn the_model_button_is_quiet_with_room_left_and_wears_its_amber_dot_from_80_and_no_ring() {
    let drawn = |percent: u32| {
        let mut s = State::default();
        s.demo = true;
        s.teammates = vec![Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(),
            runtime: Some("codex".into()), model: Some("gpt-6-astra".into()), account: None, busy: false,
            runs_in: "accept-edits".into(), folder: None }];
        s.selected = 0;
        s.conversations.insert("tm_wren".into(), Conversation::default());
        s.usage_readings.insert("codex".into(), format!("5-hour window {percent}% used · resets 2099-10-31T05:51:36.000Z"));
        let mut rt = Runtime::new(s, |s, _| crate::thread_view::thread_view(s, &Palette::new(true)), (1200.0, 900.0),
            Env::new(true, 1.0));
        rt.frame(0.0);
        let all = rt.elements();
        assert!(all.iter().any(|e| e.key.as_deref() == Some("model-chip")), "the model button");
        let dot = all.iter().any(|e| e.key.as_deref() == Some("model-chip-dot"));
        let ring = all.iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains("ring")) || e.label.as_deref().is_some_and(|l| l.contains("ring")));
        fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
            if node["key"] == key {
                return Some(node);
            }
            node["children"].as_array()?.iter().find_map(|child| find(child, key))
        }
        let said = find(&rt.accessibility_json(), "model-chip").and_then(|n| n["description"].as_str().map(str::to_owned));
        (said, dot, ring)
    };
    let (said, dot, ring) = drawn(40);
    assert!(!dot && !ring, "quiet with room left");
    assert!(said.as_deref().is_some_and(|d| d.starts_with("40% of the 5-hour window used")), "{said:?}");
    let (said, dot, ring) = drawn(85);
    assert!(dot, "the amber dot from 80%");
    assert!(!ring, "a dot, not a ring");
    // Read aloud as Electron's aria-description, with its reset in local time between.
    let said = said.unwrap_or_default();
    assert!(said.starts_with("85% of the 5-hour window used") && said.ends_with(". Long runs may be cut short."), "{said}");
    assert!(drawn(80).1, "80% itself wears it");
    assert!(!drawn(79).1);
}
