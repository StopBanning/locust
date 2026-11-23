//! a-design-pass-holds.test.tsx: list tone for settled turns, connector detail lines, and per-window usage meters.
use super::*;
use chrono::{DateTime, Utc};
use locust_core::connector_reader::{Connector, Status, detail as connector_detail};
use rune::{Env, Runtime};

fn parse_utc(iso: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(iso).unwrap().with_timezone(&Utc)
}

fn connector(status: Status, last_connected_iso: Option<&str>) -> Connector {
    Connector {
        name: "claude.ai Gmail".into(),
        location: "https://gmailmcp.googleapis.com/mcp".into(),
        status,
        said: None,
        last_connected_ms: last_connected_iso.map(|s| parse_utc(s).timestamp_millis()),
        checked_in_ms: None,
    }
}

#[test]
// case: a finished turn is muted in a list; the ones that need a look keep their colour
fn a_finished_turn_is_muted_in_a_list_while_ones_that_need_a_look_keep_colour() {
    assert_eq!(list_tone(mission_phase_view("completed", false)), "muted");
    assert_eq!(list_tone(mission_phase_view("running", false)), "live");
    assert_eq!(list_tone(mission_phase_view("interrupted", false)), "red");
    assert_eq!(list_tone(mission_phase_view("failed", false)), "red");
    // A record that did not read to its end is still worth a look.
    assert_eq!(list_tone(mission_phase_view("completed", true)), "amber");
    // A single turn's receipt still says it in blue: there it is the news.
    assert_eq!(mission_phase_view("completed", false).tone, "blue");
    // And the Missions list tag draws completed muted, running primary, failed/interrupted red.
    assert_eq!(tag("completed"), ("Completed", "text-muted"));
    assert_eq!(tag("running"), ("Running", "text-primary"));
    assert_eq!(tag("failed"), ("Failed", "red-text"));
}

#[test]
// case: connected a moment ago: no line under its name
fn connected_a_moment_ago_has_no_line_under_its_name() {
    let now_ms = parse_utc("2026-10-09T22:00:00.000Z").timestamp_millis();
    let c = connector(Status::Connected, Some("2026-10-09T21:59:40.000Z"));
    assert_eq!(connector_detail(&c, now_ms), None);
}

#[test]
// case: one to act on keeps what to do, and an old reading keeps its age
fn one_to_act_on_keeps_what_to_do_and_old_reading_keeps_its_age() {
    let now_ms = parse_utc("2026-10-09T22:00:00.000Z").timestamp_millis();
    let needs_auth = connector(Status::NeedsAuth, None);
    assert_eq!(
        connector_detail(&needs_auth, now_ms).as_deref(),
        Some("Run /mcp in Claude Code and sign in to claude.ai Gmail. Not seen connected since Locust started.")
    );
    let old = connector(Status::Connected, Some("2026-10-09T21:30:00.000Z"));
    assert_eq!(connector_detail(&old, now_ms).as_deref(), Some("Last connected 30 min ago."));
}

#[test]
// case: a meter a window, the whole sentence its tooltip, only the pressing window amber
fn usage_meters_draw_one_per_window_with_tooltip_and_only_pressing_amber() {
    const NOW_ISO: &str = "2026-10-09T22:00:00.000Z";
    const SAID: &str = "Gemini: 5-hour window 100% left · Gemini: weekly window 67% left · resets 2026-10-10T20:49:00.000Z · Claude and GPT: weekly window 20% left · resets 2026-10-10T19:09:00.000Z · as of 2026-10-09T21:53:00.000Z";
    let now = parse_utc(NOW_ISO);
    let mut rt = Runtime::new(
        State::default(),
        |_, _| {
            let l = crate::look::controls::Look::new(true);
            crate::agent_card_ui::usage_meters(&l, SAID, parse_utc(NOW_ISO))
        },
        (1000.0, 200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let l = crate::look::controls::Look::new(true);
    let elements = rt.elements();
    let meter_count = (0..10)
        .filter(|i| elements.iter().any(|e| e.key.as_deref() == Some(format!("usage-meter-{i}").as_str())))
        .count();
    assert_eq!(meter_count, 3, "a meter per window");

    let pressing_count = (0..3)
        .filter(|i| {
            let key = format!("usage-meter-{i}-figure");
            elements
                .iter()
                .find(|e| e.key.as_deref() == Some(key.as_str()))
                .and_then(|e| e.text_color)
                == Some(l.c("amber"))
        })
        .count();
    assert_eq!(pressing_count, 1, "only the pressing window is amber");

    let words: String = elements.iter().filter_map(|e| e.text.as_deref()).collect::<Vec<_>>().join("\n");
    assert!(words.contains("Claude and GPT: weekly"), "{words}");
    assert!(words.contains("20% left"), "{words}");
    assert!(words.contains("as of"), "{words}");

    let container = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("usage-meters"))
        .expect("usage-meters container");
    let expected_tip = locust_core::usage_words::usage_window_sentence(SAID, now);
    assert_eq!(container.help.as_deref(), Some(expected_tip.as_str()), "the whole sentence is the tooltip");
}
