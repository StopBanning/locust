//! speed-2 row 4: whether a drawn row shimmers is asked on every poll, and every 50 ms while one does. With no listed
//! turn running the answer needs no drawn list, so a resting window never builds it; with one running the answer is
//! still the drawn list's own (a row a filter hides never shimmers).
use super::*;

const AT: &str = "2026-11-27T05:00:00.000Z";

fn listed(s: &mut State, id: &str, state: &str) {
    s.recent.push(ConversationRow { id: id.into(), title: format!("Conversation {id}"), teammate: "Wren".into(),
        started_by: "native".into(), state: state.into(), updated: AT.into() });
}

fn window(rows: usize) -> State {
    let mut s = State::default();
    s.demo = true;
    s.teammates.push(Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code".into(), runtime: None,
        model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
    for n in 0..rows {
        listed(&mut s, &format!("m_rest_{n}"), if n % 7 == 0 { "failed" } else { "completed" });
    }
    s
}

fn builds() -> usize {
    DRAWN_BUILDS.with(|n| n.get())
}

#[test]
fn a_resting_window_s_poll_never_builds_the_drawn_list() {
    let s = window(90);
    let before = builds();
    for _ in 0..200 {
        assert!(!shimmering(&s), "nothing listed is running");
    }
    assert_eq!(builds() - before, 0, "200 polls at rest built the drawn list {} times", builds() - before);
}

#[test]
fn a_running_row_shimmers_and_a_running_row_the_list_hides_does_not() {
    let mut s = window(90);
    listed(&mut s, "m_live", "running");
    let before = builds();
    assert!(shimmering(&s), "a drawn running row shimmers");
    assert!(builds() > before, "with a turn running the answer is the drawn list's");
    // A face filter for someone else hides Wren's running row: nothing drawn is running, so nothing shimmers.
    s.sidebar_face_filter = Some("tm_other".into());
    assert!(!shimmering(&s), "a running row the list does not draw never shimmers");
}
