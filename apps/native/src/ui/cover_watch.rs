//! The window's side of `locust_core::cover_activity` (Electron's `useCoverActivity`): it tells the rule what the
//! window did since the last poll (focus, reduced motion, the platform's last input), and answers whether motion that
//! follows the rule rests. Settings' face legend follows it; Home's cover, when the Rust app has one, will too.
//!
//! Not carried: a hidden window (`visibilitychange`). Rune tells the app about focus, not about being minimised or
//! covered; a hidden window has lost focus first, so it already rests.

use super::*;
use locust_core::cover_activity::{is_stoppable, work_signature, CoverActivity};

/// What the window hands the cover (`workSignature`, 0.623): each stoppable run in `s.recent` (or `s.seen_live`),
/// by its id, phase, and item count (when open in `s.ledger`), never the runs map itself.
pub(crate) fn work_of(s: &State) -> String {
    let active_len = |id: &str| {
        s.ledger
            .as_ref()
            .filter(|pane| pane.snapshot.tip == id)
            .map_or(0, |pane| pane.items.len())
    };
    let mut entries: Vec<(&str, &str, usize)> = s
        .recent
        .iter()
        .map(|row| (row.id.as_str(), row.state.as_str(), active_len(&row.id)))
        .collect();
    for id in &s.seen_live {
        if !entries.iter().any(|(k, _, _)| *k == id.as_str()) {
            entries.push((id.as_str(), "running", active_len(id)));
        }
    }
    work_signature(entries, is_stoppable)
}

/// In `poll`, before anything asks `away`. `last_input` is `rune::last_input()`; tests pass their own.
pub(crate) fn observe(s: &mut State, now: f64, last_input: Option<rune::time::Instant>) {
    let reduced = s.reduced_motion.get();
    let work = work_of(s);
    let cover = s.cover.get_or_insert_with(|| CoverActivity::new(now, s.window_focused, false, reduced));
    if cover.focused() != s.window_focused {
        cover.set_focused(s.window_focused, now);
    }
    if cover.reduced() != reduced {
        cover.set_reduced(reduced, now);
    }
    if last_input.is_some() && last_input != s.cover_input_seen {
        s.cover_input_seen = last_input;
        cover.touch(now);
    }
    cover.work(&work, now);
}

/// Whether motion that follows the cover's rule rests now (`presence.away()`).
pub(crate) fn away(s: &State, now: f64) -> bool {
    s.cover.as_ref().map_or(!s.window_focused || s.reduced_motion.get(), |cover| cover.away(now))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, state: &str) -> ConversationRow {
        ConversationRow {
            id: id.into(),
            teammate: String::new(),
            title: String::new(),
            started_by: "the person".into(),
            state: state.into(),
            updated: String::new(),
        }
    }

    // case: is what the app hands the cover, never the map itself
    #[test]
    fn is_what_the_app_hands_the_cover_never_the_map_itself() {
        let mut s = State::default();
        s.window_focused = true;
        s.recent = vec![row("run_a", "running"), row("run_b", "completed")];
        observe(&mut s, 0.0, None);
        assert!(!away(&s, 0.0));
        // Routine rebuild of `s.recent` with the same runs does not reset the rest clock.
        s.recent = vec![row("run_b", "completed"), row("run_a", "running")];
        observe(&mut s, 30.0, None);
        assert!(away(&s, 45.0), "rebuilt list with same runs still rests at 45 s");
        // Real work arriving (a new running run) wakes the cover via work_signature.
        s.recent.push(row("run_c", "starting"));
        observe(&mut s, 50.0, None);
        assert!(!away(&s, 50.0), "new running run wakes the cover");
        assert!(away(&s, 95.0), "rests again 45 s later");
    }
}
