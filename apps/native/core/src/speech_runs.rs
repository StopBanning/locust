//! One face per run of speech, and Copy under the turn's last reply (Electron's `Thread.tsx` `continuesSpeech` and
//! `lastSpeech`, design pass 2026-09-22 and 0.491/0.545).
//!
//! A finished turn drew the teammate's face over every thing it said, so a turn that spoke three times between its
//! steps wore the face three times. Consecutive things the teammate said share the face of the first; its steps (tool
//! calls and thoughts) sit between them without starting a new run, as Claude's apps draw a turn; anything else -- the
//! person's words, a note, a limit -- does. Copy sits once, under the turn's last reply, once nothing is still going.

use crate::ledger::Item;

/// Whether the reply at `index` continues the teammate's speech before it, so draws no face of its own. `said(i)`
/// answers whether item `i` is a reply that shows something (an empty reply is not drawn and is not speech).
pub fn continues_speech(items: &[Item], index: usize, said: impl Fn(usize) -> bool) -> bool {
    for at in (0..index.min(items.len())).rev() {
        match &items[at] {
            Item::Assistant { .. } if said(at) => return true,
            Item::Assistant { .. } => {}
            Item::Plan { .. } => return true,
            Item::Tool { .. } | Item::Thought { .. } => {}
            _ => return false,
        }
    }
    false
}

/// The reply that carries Copy: the last one shown, once the turn is not running and that reply is whole.
pub fn last_speech(items: &[Item], running: bool, said: impl Fn(usize) -> bool) -> Option<usize> {
    if running {
        return None;
    }
    let at = (0..items.len()).rev().find(|&i| matches!(items[i], Item::Assistant { .. }) && said(i))?;
    matches!(&items[at], Item::Assistant { r#final: true, .. }).then_some(at)
}

#[cfg(test)]
#[path = "speech_runs_tests.rs"]
mod tests;
