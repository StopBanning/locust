//! Shared hover tooltip primitive: resting the pointer on an element for `HOVER_DELAY` shows its full label
//! using Rune's `hover_delays` and `rune_kit::Tooltip`.
//!
//! Electron gives these as a native `title` (a sidebar conversation row's is `title={hover}`, Sidebar.tsx), which the
//! system opens only once the pointer has rested, never on a pass. The tip sits ABOVE what it describes, so in a
//! list of rows (the sidebar's conversations) it never covers the next row's face and title, as it did when it hung
//! below (the 26 Nov audit).

use super::*;
use rune_kit::{Placement, Tooltip};
use std::time::Duration;

/// How long the pointer rests before a tip shows. Electron's `title` leaves it to the system; this is the system
/// tooltip delay Rune's own `.help` stands in with (`Runtime::help_delay`, 0.5 s).
pub(crate) const HOVER_DELAY: Duration = Duration::from_millis(500);

/// Where the tip sits against its trigger: above it, clear of the row below.
const TIP_PLACEMENT: Placement = Placement::Top;

#[derive(Default)]
pub(crate) struct HoverState {
    active: Option<String>,
}

impl HoverState {
    pub(crate) fn set(&mut self, key: &str, over: bool) {
        if over {
            self.active = Some(key.to_owned());
        } else if self.active.as_deref() == Some(key) {
            self.active = None;
        }
    }

    pub(crate) fn is_open(&self, key: &str) -> bool {
        self.active.as_deref() == Some(key)
    }
}

/// Wraps `trigger` so resting the pointer on it for `HOVER_DELAY` opens a `rune_kit::Tooltip` with `label`.
pub(crate) fn tip(s: &State, l: &crate::look::controls::Look, key: &str, label: &str, trigger: El) -> El {
    let open = !label.is_empty() && s.hover.is_open(key);
    let key_owned = key.to_owned();
    let trigger = trigger
        .hover_delays(HOVER_DELAY, Duration::ZERO)
        .on_hover_change(move |s: &mut State, over, _| {
            s.hover.set(&key_owned, over);
        });
    let mut layer = Vec::new();
    let built = Tooltip::new(key)
        .trigger(trigger)
        .text(&l.k, label)
        .placement(TIP_PLACEMENT)
        .open(open)
        .build(&l.k, &mut layer);
    // One shape open or closed: wrapping the trigger only while the tip shows moved it under a new parent, which
    // Rune reads as the pointer leaving it, so the tip closed the frame after it opened and flickered every delay.
    // The anchor row inside gives way with the wrapper, so a caller's `.shrink()` still reaches the trigger
    // (handed_files.rs: a narrow reply cuts the pill's note).
    row().child(built.shrink().min_w(0.0)).children(layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime, col, text};

    fn render() -> Runtime<State> {
        render_labelled("Full hover label")
    }

    fn view(s: &State, label: &str) -> El {
        let l = crate::look::controls::Look::new(true);
        col()
            .w_full()
            .h_full()
            .pt(100.0)
            .items_center()
            .child(tip(s, &l, "sample", label, text("Chip").key("sample-chip").padding(8.0)))
    }

    fn render_labelled(label: &'static str) -> Runtime<State> {
        let s = State::default();
        let mut rt = if label.is_empty() {
            Runtime::new(s, |s, _| view(s, ""), (600.0, 400.0), Env::new(true, 1.0))
        } else {
            Runtime::new(s, |s, _| view(s, "Full hover label"), (600.0, 400.0), Env::new(true, 1.0))
        };
        rt.frame(0.0);
        rt
    }

    #[test]
    fn resting_the_pointer_shows_the_tooltip_only_after_the_delay_and_hides_when_leaving() {
        let mut rt = render();
        let (x, y) = rt.find("sample-chip").unwrap();
        rt.pointer_move(x, y);
        rt.frame(0.1);
        assert!(rt.find("sample-tooltip").is_none(), "not shown before HOVER_DELAY");

        rt.frame(0.5);
        let tip_el = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("sample-tooltip"));
        assert_eq!(tip_el.and_then(|e| e.label).as_deref(), Some("Full hover label"));

        rt.pointer_move(5.0, 5.0);
        rt.frame(0.016);
        assert!(rt.find("sample-tooltip").is_none(), "closes when pointer leaves");
    }

    #[test]
    fn moving_away_before_the_delay_cancels_the_tooltip() {
        let mut rt = render();
        let (x, y) = rt.find("sample-chip").unwrap();
        rt.pointer_move(x, y);
        rt.frame(0.2);
        assert!(rt.find("sample-tooltip").is_none());

        rt.pointer_move(5.0, 5.0);
        rt.frame(0.6);
        assert!(rt.find("sample-tooltip").is_none(), "leaving before delay cancels it");
    }

    /// Two sidebar conversation rows, 32 high and touching, the first with a tipped plug in its title line
    /// (sidebar.rs `conversation-app-`), as the 26 Nov audit saw them.
    fn two_rows(s: &State) -> El {
        let l = crate::look::controls::Look::new(true);
        let plug = text("P").key("row-one-plug").padding(2.0);
        col()
            .w_full()
            .h_full()
            .pt(120.0)
            .px(12.0)
            .child(
                row().key("row-one").w(240.0).h(32.0).center_y().gap(6.0).px(8.0)
                    .child(text("Face"))
                    .child(tip(s, &l, "row-one-plug", "Started by another app", plug))
                    .child(text("Fix the login page")),
            )
            .child(row().key("row-two").w(240.0).h(32.0).center_y().gap(6.0).px(8.0).child(text("Face")).child(text("Next title")))
    }

    /// The element's box as x0 y0 x1 y1 (`ElementInfo::rect` is x y w h).
    fn rect_of(rt: &Runtime<State>, key: &str) -> Option<[f32; 4]> {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
            .map(|e| [e.rect[0], e.rect[1], e.rect[0] + e.rect[2], e.rect[1] + e.rect[3]])
    }

    #[test]
    fn a_row_tip_waits_for_the_rest_then_sits_clear_of_the_next_row() {
        let mut rt = Runtime::new(State::default(), |s, _| two_rows(s), (600.0, 400.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let (x, y) = rt.find("row-one-plug").unwrap();
        rt.pointer_move(x, y);
        // `frame` takes the time since the last frame, in seconds.
        rt.frame(0.1);
        rt.frame(0.3);
        assert!(rect_of(&rt, "row-one-plug-tooltip").is_none(), "no tip before the pointer has rested HOVER_DELAY");

        rt.frame(0.2);
        assert!(rect_of(&rt, "row-one-plug-tooltip").is_some(), "the tip shows once the pointer has rested");
        // And stays while the pointer rests, rather than closing and waiting again.
        for _ in 0..40 {
            rt.frame(0.016);
            assert!(rect_of(&rt, "row-one-plug-tooltip").is_some(), "the tip stays open while the pointer rests");
        }
        let tip = rect_of(&rt, "row-one-plug-tooltip").expect("the tip shows once the pointer has rested");
        let next = rect_of(&rt, "row-two").unwrap();
        let overlaps = tip[0] < next[2] && next[0] < tip[2] && tip[1] < next[3] && next[1] < tip[3];
        assert!(!overlaps, "the tip {tip:?} covers the next row {next:?}");
        let trigger = rect_of(&rt, "row-one-plug").unwrap();
        assert!(tip[3] <= trigger[1], "the tip {tip:?} sits above its trigger {trigger:?}");
    }

    #[test]
    fn an_empty_label_never_opens_a_tooltip() {
        let mut rt = render_labelled("");
        let (x, y) = rt.find("sample-chip").unwrap();
        rt.pointer_move(x, y);
        rt.frame(0.1);
        rt.frame(0.6);
        assert!(rt.find("sample-tooltip").is_none());
    }
}
