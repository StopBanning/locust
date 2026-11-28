//! Electron quiet.test.ts, checked on the real Room answer card, on a fixed clock.
use super::*;
use rune::{Env, Runtime};

const START: &str = "2026-09-13T17:56:55.000Z";

fn room(started_at: &str) -> Runtime<State> {
    let room = PublicRoom {
        room_id: "room_quiet".into(),
        name: "Quiet".into(),
        teammate_ids: vec!["tm_wren".into()],
        created_at: START.into(),
        posts: vec![RoomPost {
            post_id: "post_1".into(),
            text: "A fruit, please.".into(),
            at: START.into(),
            missions: BTreeMap::from([("tm_wren".into(), "m1".into())]),
            queued: Vec::new(),
            refused: BTreeMap::new(),
            to: Vec::new(),
        }],
        tasks: Vec::new(),
    };
    let mut s = State::default();
    s.rooms.active = RoomView::Room(room.room_id.clone());
    s.rooms.rooms = vec![room];
    s.rooms.transcripts.insert("m1".into(), RoomTranscriptView {
        phase: "running".into(),
        answer_text: String::new(),
        note_error: None,
        started_at: started_at.into(),
        runtime: "opencode".into(),
        model: "free".into(),
        live: None,
    });
    Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1100.0, 820.0), Env::new(true, 1.0))
}

fn tick(rt: &mut Runtime<State>, ms: i64) -> rune::ElementInfo {
    let now = DateTime::parse_from_rfc3339(START).unwrap().timestamp_millis() + ms;
    // Use the real poll's redraw result, not a forced invalidation.
    rt.update_if(|s, tx| poll_at(s, tx, now));
    rt.frame(1.0);
    rt.elements().into_iter().find(|el| {
        !el.exiting && el.key.as_deref() == Some("room-answer-phase-post_1-tm_wren")
    }).expect("the actual Room answer phase")
}

// case: says nothing while the silence is still ordinary
#[test]
fn room_silence_is_unremarkable_during_the_first_nineteen_seconds() {
    let mut rt = room(START);
    for (ms, words) in [(0, "asked · 0s"), (19_000, "asked · 19s")] {
        let drawn = tick(&mut rt, ms);
        assert_eq!(drawn.text.as_deref(), Some(words));
        assert_eq!(drawn.text_color, Some(Palette::new(true).muted));
    }
}

// case: speaks up at the threshold and stays up
#[test]
fn room_silence_redraws_at_the_threshold_and_stays_visible() {
    let mut rt = room(START);
    assert!(tick(&mut rt, 19_000).text.unwrap().ends_with("19s"));
    let drawn = tick(&mut rt, 20_000);
    assert_eq!(drawn.text.as_deref(), Some("asked · 20s · no word back yet"));
    assert_eq!(drawn.text_color, Some(Palette::new(true).amber));
    assert!(tick(&mut rt, 128_700).text.unwrap().ends_with(" · no word back yet"));
}

// case: is the number the room uses, not a second opinion about it
#[test]
fn the_room_uses_the_twenty_second_rule_at_the_actual_boundary() {
    let mut rt = room(START);
    assert!(!tick(&mut rt, 19_999).text.unwrap().contains("no word back yet"));
    assert!(tick(&mut rt, 20_000).text.unwrap().contains("no word back yet"));
}

// case: an unreadable timestamp is never reported as quiet
#[test]
fn the_room_does_not_guess_silence_from_an_unreadable_timestamp() {
    let mut rt = room("not a date");
    let drawn = tick(&mut rt, 600_000);
    assert_eq!(drawn.text.as_deref(), Some("asked"));
    assert_eq!(drawn.text_color, Some(Palette::new(true).muted));
}

#[test]
fn a_late_reply_still_running_removes_the_room_silence_notice() {
    let mut rt = room(START);
    let quiet = tick(&mut rt, 20_000);
    assert_eq!(quiet.text.as_deref(), Some("asked · 20s · no word back yet"));
    assert_eq!(quiet.text_color, Some(Palette::new(true).amber));

    // The running transcript has received its first words; it has not finished.
    rt.update(|s, _| {
        s.rooms.transcripts.get_mut("m1").unwrap().answer_text = "A peach.".into();
    });
    let answered = tick(&mut rt, 21_000);
    assert_eq!(answered.text.as_deref(), Some("running · 21s"));
    assert_eq!(answered.text_color, Some(Palette::new(true).muted));
    assert!(rt.elements().iter().any(|el| !el.exiting && el.text.as_deref() == Some("A peach.")));
}
