//! The window's half of Electron's `main/attention.test.ts` room cases: a room's changes reach the desk as one toast per
//! two minutes, and only while the window is elsewhere; the room screen says each at once either way.
use super::*;
use locust_core::attention::ROOM_TOAST_WINDOW_MS;
use locust_core::room_tasks::RoomNotice;
use std::time::{Duration, Instant};

fn notice(room: &str, name: &str, message: &str) -> RoomNotice {
    RoomNotice { room_id: room.into(), room_name: name.into(), message: message.into() }
}
fn later(start: Instant) -> Instant {
    start + Duration::from_millis(ROOM_TOAST_WINDOW_MS)
}

#[test]
fn a_room_change_is_said_on_the_screen_at_once_and_to_the_desk_after_two_minutes() {
    let start = Instant::now();
    let mut s = State::default();
    s.window_focused = false;
    heard_room_notices(&mut s, vec![notice("room_r", "Release", "Wren finished \"Write the notes\"."), notice("room_r", "Release", "  ")], start);
    assert_eq!(s.rooms.notice.as_deref(), Some("Wren finished \"Write the notes\"."));
    assert!(room_toasts_due(&mut s, start, true).is_empty(), "not before the window");
    heard_room_notices(&mut s, vec![notice("room_r", "Release", "Booty took on \"Ship it\".")], start + Duration::from_secs(1));
    assert_eq!(
        room_toasts_due(&mut s, later(start), true),
        vec![("Release · 2 changes".to_owned(), "Wren finished \"Write the notes\".\nBooty took on \"Ship it\".".to_owned())]
    );
    // A notice with no words (a room to read again) is never a change to tell.
    heard_room_notices(&mut s, vec![notice("room_r", "Release", "")], start);
    assert!(s.room_toasts.pending().is_empty());
}

// case: when a room changes while the person is elsewhere > says nothing while the window has attention -- the room screen already does
#[test]
fn says_nothing_while_the_window_has_attention_the_room_screen_already_does() {
    let start = Instant::now();
    let mut s = State::default();
    s.window_focused = true;
    heard_room_notices(&mut s, vec![notice("room_r", "Release", "Wren finished \"Write the notes\".")], start);
    assert_eq!(s.rooms.notice.as_deref(), Some("Wren finished \"Write the notes\"."));
    assert!(room_toasts_due(&mut s, later(start), true).is_empty());
    assert!(s.room_toasts.pending().is_empty(), "let go, not held for later");
    // And nothing where the system cannot show one.
    s.window_focused = false;
    heard_room_notices(&mut s, vec![notice("room_r", "Release", "x")], start);
    assert!(room_toasts_due(&mut s, later(start), false).is_empty());
    assert!(s.room_toasts.pending().is_empty());
}
