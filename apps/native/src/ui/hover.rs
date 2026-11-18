//! Shared hover tooltip primitive: resting the pointer on an element for `HOVER_DELAY` shows its full label
//! using Rune's `hover_delays` and `rune_kit::Tooltip`.

use super::*;
use rune_kit::Tooltip;
use std::time::Duration;

pub(crate) const HOVER_DELAY: Duration = Duration::from_millis(500);

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
        .open(open)
        .build(&l.k, &mut layer);
    if layer.is_empty() {
        built
    } else {
        row().child(built).children(layer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime, col, text};

    fn render() -> Runtime<State> {
        let s = State::default();
        let mut rt = Runtime::new(
            s,
            |s, _| {
                let l = crate::look::controls::Look::new(true);
                col()
                    .w_full()
                    .h_full()
                    .pt(100.0)
                    .items_center()
                    .child(tip(s, &l, "sample", "Full hover label", text("Chip").key("sample-chip").padding(8.0)))
            },
            (600.0, 400.0),
            Env::new(true, 1.0),
        );
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
}
