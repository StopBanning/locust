//! Electron's `a-working-row-shimmers-like-claude-codes.test.ts` (3 cases). Electron reads its stylesheet; Rust draws
//! the band itself, so these read what each character is painted and the rows the sidebar draws.
use super::*;
use crate::*;

fn ink() -> Color {
    Color { r: 0.9, g: 0.88, b: 0.86, a: 1.0 }
}

fn alphas(spans: &[Span]) -> Vec<f32> {
    spans.iter().map(|span| span.color.expect("each character is painted").a).collect()
}

// case: is the grey title with one white band passing through it, from the theme’s own tokens
#[test]
fn is_the_grey_title_with_one_white_band_passing_through_it_from_the_themes_own_tokens() {
    let words = "Fix the login page and tidy the tests";
    // Half way through a pass the band stands in the middle of the title.
    let spans = spans(words, PASS_SECONDS * 0.5, ink());
    assert_eq!(spans.len(), words.chars().count());
    let a = alphas(&spans);
    // The grey is the theme's text at 34% (`color-mix(... 34%, transparent)`), at both ends.
    assert!((a[0] - 0.34).abs() < 1e-6 && (a[a.len() - 1] - 0.34).abs() < 1e-6, "{a:?}");
    // One band: brightening to a single peak near full white, then fading back, never a second rise.
    let peak = a.iter().cloned().fold(0.0, f32::max);
    assert!(peak > 0.95, "{a:?}");
    let top = a.iter().position(|&v| v == peak).unwrap();
    assert!(a[..=top].windows(2).all(|w| w[0] <= w[1]) && a[top..].windows(2).all(|w| w[0] >= w[1]), "{a:?}");
    // No colour of its own: every character is the theme's text, only its strength changes.
    for span in &spans {
        let c = span.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (ink().r, ink().g, ink().b));
    }
    // The gradient's own stops: grey to 32%, the theme's text at 50%, grey again from 68%.
    assert_eq!(gradient(0.0), DIM);
    assert_eq!(gradient(0.32), DIM);
    assert_eq!(gradient(0.5), 1.0);
    assert_eq!(gradient(0.68), DIM);
    assert!(gradient(0.41) > DIM && gradient(0.41) < 1.0);
}

// case: passes slowly, left to right, and stands still for a person who asked for less motion
#[test]
fn passes_slowly_left_to_right_and_stands_still_for_a_person_who_asked_for_less_motion() {
    // Where the band is brightest along the title, at a moment of the pass.
    let crest = |seconds: f64| {
        (0..=200).map(|i| i as f64 / 200.0).max_by(|a, b| brightness_at(*a, seconds).total_cmp(&brightness_at(*b, seconds))).unwrap()
    };
    // It enters on the left before the middle of the pass and leaves on the right after it.
    assert!(crest(0.6) < crest(1.0) && crest(1.0) < crest(1.4), "{} {} {}", crest(0.6), crest(1.0), crest(1.4));
    // Two seconds a pass: the band is where it was one pass later.
    for x in [0.1, 0.4, 0.7] {
        assert!((brightness_at(x, 0.9) - brightness_at(x, 0.9 + PASS_SECONDS)).abs() < 1e-5);
    }
    // As a pass begins the band is entering on the left, the right still grey; as it ends, the other way round.
    assert!(brightness_at(0.05, 0.0) > DIM && brightness_at(0.9, 0.0) == DIM);
    assert!(brightness_at(0.95, 1.99) > DIM && brightness_at(0.1, 1.99) == DIM);
    // Less motion: no band, the title in the theme's text.
    assert!(title_spans("Fix the login page", ink(), true, 1.0).is_empty());
    assert_eq!(element_ink(ink(), true), ink());
    // While it moves, what the spans leave (a long title's "…") is the grey, never full white.
    assert!((element_ink(ink(), false).a - 0.34).abs() < 1e-6);
    assert!(!title_spans("Fix the login page", ink(), false, 1.0).is_empty());
}

fn sidebar_with(rows: &[(&str, &str)]) -> rune::Runtime<State> {
    let mut s = State::default();
    for (id, state) in rows {
        s.recent.push(ConversationRow {
            id: (*id).into(),
            title: format!("Conversation {id}"),
            teammate: String::new(),
            started_by: String::new(),
            state: (*state).into(),
            updated: "2026-11-21T09:00:00.000Z".into(),
        });
    }
    let mut rt = rune::Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn room(missions: &[&str]) -> locust_core::room_store::PublicRoom {
    locust_core::room_store::PublicRoom {
        room_id: "room_1".into(),
        name: "Launch".into(),
        teammate_ids: vec!["tm_pip".into()],
        created_at: "2026-11-21T09:00:00.000Z".into(),
        posts: vec![locust_core::room_store::RoomPost {
            post_id: "post_1".into(),
            text: "Go".into(),
            at: "2026-11-21T09:00:00.000Z".into(),
            missions: missions.iter().enumerate().map(|(i, m)| (format!("tm_{i}"), (*m).to_string())).collect(),
            queued: Vec::new(),
            refused: Default::default(),
            to: Vec::new(),
        }],
        tasks: Vec::new(),
    }
}

// case: is worn by a running conversation and a running room only, and keeps the dot’s footprint empty
#[test]
fn is_worn_by_a_running_conversation_and_a_running_room_only_and_keeps_the_dots_footprint_empty() {
    let rt = sidebar_with(&[("mission_run", "running"), ("mission_done", "completed"), ("mission_bad", "failed")]);
    // The running row keeps an empty 6 by 6 place where the dot was; the others have none.
    let [_, _, w, h] = rt.presented("conversation-working-mission_run").expect("the footprint").0;
    assert_eq!((w, h), (6.0, 6.0));
    assert!(rt.find("conversation-working-mission_done").is_none());
    assert!(rt.find("conversation-working-mission_bad").is_none());
    // The window keeps the band moving while a drawn row works, and asks for no frames once none does.
    assert!(crate::sidebar::shimmering(&rt.state));
    let rt = sidebar_with(&[("mission_done", "completed"), ("mission_bad", "failed")]);
    assert!(!crate::sidebar::shimmering(&rt.state));
    // A room works while a conversation one of its posts started is running, and not otherwise.
    let running = vec!["mission_a".to_string()];
    assert!(crate::sidebar::room_working(&room(&["mission_z", "mission_a"]), &running));
    assert!(!crate::sidebar::room_working(&room(&["mission_z"]), &running));
    assert!(!crate::sidebar::room_working(&room(&[]), &running));
}
