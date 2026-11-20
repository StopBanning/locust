//! A teammate tagged in the message box is sent the message too, in a conversation of their own (Electron 0.438,
//! `TEAMMATES_TAG_CHANNEL`): the teammate on screen still gets it as before; each one tagged gets
//! `tagging::tagged_prompt` -- the message, whose conversation it came from, and that conversation's latest answer.

use super::*;

/// Start the tagged teammates' runs off the window's thread. The composer clears its tags after this, as it did.
pub(super) fn send_tags(s: &mut State, message: &str) {
    if s.tagged.is_empty() || message.trim().is_empty() || s.engine != Some(Kind::Native) {
        return;
    }
    let on_screen = selected_conversation(s).map(|(t, _)| t.clone()).filter(|t| !locust_core::native::is_nobody(t));
    let tagged: Vec<Teammate> = s
        .tagged
        .iter()
        .filter(|id| on_screen.as_ref().is_none_or(|t| &t.id != *id))
        .filter_map(|id| s.teammates.iter().find(|t| &t.id == id).cloned())
        .collect();
    if tagged.is_empty() {
        return;
    }
    let from_name = on_screen.map(|t| t.name);
    // That conversation's latest answer, for context: the newest final reply on screen.
    let answer = from_name.as_ref().and_then(|_| {
        s.ledger.as_ref()?.snapshot.items.iter().rev().find_map(|item| match item {
            ledger::Item::Assistant { text, r#final: true, .. } => Some(text.clone()),
            _ => None,
        })
    });
    let message = message.to_owned();
    s.tag_job.request((tagged, message, from_name, answer), |(tagged, message, from, answer)| {
        locust_core::tagging::send_tagged(tagged, &message, from.as_deref(), answer.as_deref(), |job, origin| {
            locust_core::engine_api::api().send(job, origin)
        })
    });
}

/// Who it reached, and who it could not, in the composer's note.
pub(super) fn poll(s: &mut State) -> bool {
    let Some(out) = s.tag_job.take() else { return false };
    if let Some(said) = out.said() {
        s.composer_note = Some(said);
    }
    true
}
