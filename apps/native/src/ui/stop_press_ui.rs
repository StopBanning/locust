//! Stop on a run that is still starting (Electron's `stopPress`, 0.806): the send has not answered with the run's name
//! yet, but the runtime is already going, so the composer offers Stop and a press is remembered, not dropped. When the
//! start lands with the name, the remembered press is honoured at once.
use super::*;
use locust_core::stop_press::{StopPress, StoppableRun, stop_press};

/// The send is out and its answer has not been taken yet: Electron's `starting` phase, for the conversation on screen.
/// A send keeps its message on the flight until the answer is taken, which covers the moment between the answer
/// landing and the window reading it.
pub(super) fn starting(s: &State) -> bool {
    !s.demo
        && s.flight.as_ref().is_some_and(|f| {
            (s.sending.is_pending() || f.message.is_some()) && active_matches(s, &f.teammate_id, f.conversation_id.as_deref())
        })
}

/// Whether the composer offers Stop: for a running turn this window may stop, and for one still starting. (`can_stop`
/// stays the running-only test, which the turn's other controls and tests read.)
pub(super) fn offered(s: &State) -> bool {
    can_stop(s) || starting(s)
}

/// The key a starting run is filed under: its conversation once it has one, else the teammate it was sent to.
fn shown_key(s: &State) -> Option<String> {
    let f = s.flight.as_ref()?;
    Some(f.conversation_id.clone().unwrap_or_else(|| format!("pending:{}", f.teammate_id)))
}

/// Called by Stop before it cancels: true when the press was answered here (remembered for a starting run).
pub(super) fn remembered(s: &mut State) -> bool {
    if !starting(s) {
        return false;
    }
    let key = shown_key(s);
    match stop_press(Some(StoppableRun { phase: "starting", run_id: None }), key.as_deref()) {
        StopPress::CancelWhenNamed { key } => {
            s.stop_when_named = Some(key);
            s.status = "Stopping…".into();
            s.status_tone = StatusTone::Muted;
        }
        StopPress::Cancel { .. } | StopPress::Nothing => {}
    }
    true
}

/// After a send lands: a press remembered while it was starting stops the turn it started. Returns whether it acted.
pub(super) fn named(s: &mut State) -> bool {
    if s.sending.is_pending() || s.stop_when_named.is_none() {
        return false;
    }
    s.stop_when_named = None;
    // A send that failed started nothing (its flight is gone); there is nothing to stop.
    let Some(conversation) = s.flight.as_ref().and_then(|f| f.conversation_id.clone()) else {
        return true;
    };
    s.stopped_by_you = Some(conversation.clone());
    if s.engine != Some(Kind::Native) {
        // Through the door: the start set the window's own turn, so Stop takes its ordinary path.
        stop(s);
        return true;
    }
    // Native: by the engine's registry, before the window's ledger has seen the turn running.
    s.stopping.request(conversation, |id| {
        let api = locust_core::engine_api::api();
        let result = match api.owned_key(None, Some(&id)) {
            Some(key) => api.stop(&key),
            None => Err("That turn had already ended.".to_owned()),
        };
        (id, result)
    });
    true
}

#[cfg(test)]
#[path = "stop_press_ui_tests.rs"]
mod tests;
