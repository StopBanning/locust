//! Conversation links distinguish a new reply on another runtime from a running turn handed over.
use crate::ledger::Metadata;
use std::collections::{HashMap,HashSet};
pub const INSTRUCTION_SECTION:&str="\n\nThe person now asks:\n\n";
pub fn handoff_instruction(prompt:&str)->Option<&str>{prompt.rsplit_once(INSTRUCTION_SECTION).map(|(_,s)|s.trim()).filter(|s|!s.is_empty())}
pub fn is_reply_switch(m:&Metadata)->bool{m.continues_reason.as_deref()==Some("route-switch") && handoff_instruction(&m.prompt).is_some()}
#[derive(Clone,Debug,PartialEq)] pub struct Switch {pub from:String,pub to:String,pub unsettled_count:usize,pub omitted_briefing:Vec<String>}
pub fn switch_of(m:&Metadata,by_id:&HashMap<String,Metadata>)->Option<Switch>{if !is_reply_switch(m){return None;}seam(m,by_id)}
fn seam(m:&Metadata,by_id:&HashMap<String,Metadata>)->Option<Switch>{let prior=by_id.get(m.parent.as_ref()?)?;Some(Switch{from:prior.runtime.clone(),to:m.runtime.clone(),unsettled_count:m.checkpoint_epoch.and_then(|epoch|prior.checkpoint_unsettled.get(&epoch).copied()).unwrap_or(0),omitted_briefing:m.omitted_briefing.clone()})}
pub fn stitched_handoff(m:&Metadata,by_id:&HashMap<String,Metadata>)->Option<Switch>{if m.continues_reason.as_deref()!=Some("route-switch") || is_reply_switch(m){return None;}seam(m,by_id)}
pub fn typed_prompt<'a>(m: &'a Metadata, by_id: &'a HashMap<String, Metadata>) -> &'a str {
    prompt_source(m, by_id, |_| false).1
}
/// The tag context is host briefing, never words the person typed.
pub fn tagged_words(prompt: &str) -> &str {
    prompt.split_once(crate::tagging::CONTEXT_MARK).map_or(prompt, |(words, _)| words)
}
pub fn shown_prompt(m: &Metadata) -> &str {
    if m.started_by_kind.as_deref() == Some("routine") {
        return crate::hand_off::step_words_of(&m.prompt);
    }
    if m.started_by_kind.is_none() || m.started_by_kind.as_deref() == Some("tag") {
        tagged_words(&m.prompt)
    } else { &m.prompt }
}
/// A routine's own fix turn (`routine_runner::goal_fix_prompt`): Locust's words, quoting its check, sent as the turn's
/// whole prompt. The record names only the routine and its step, so the fix is read back from those words.
static ROUTINE_FIX: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^The check `([^`\n]{1,500})` failed after the work above \(fix (\d{1,2}) of (\d{1,2})\)\. The end of its output:").unwrap()
});

/// Which fix of a failing check a turn is, when Locust sent it: a check-fix turn's own starter, or a routine's fix turn
/// read from its words. None for every other turn -- the same words typed by the person stay the person's.
pub fn fix_of_turn(m: &Metadata) -> Option<crate::edit_check::CheckFixStarter> {
    if let Some(cf) = &m.check_fix {
        return Some(cf.clone());
    }
    if m.started_by_kind.as_deref() != Some("routine") {
        return None;
    }
    let found = ROUTINE_FIX.captures(&m.prompt)?;
    Some(crate::edit_check::CheckFixStarter { command: found[1].to_owned(), fix: found[2].parse().ok()?, of: found[3].parse().ok()? })
}

/// Electron's `turnPromptLine`: what the turn's bubble says, in the person's voice, or None when nobody typed it.
/// Display only -- `typed_prompt` (what a routine replays) still reads `shown_prompt`, so a routine saved from a tidy
/// pass sends the whole brief again, not the one line drawn for it.
pub fn turn_line(m: &Metadata) -> Option<std::borrow::Cow<'_, str>> {
    use std::borrow::Cow;
    // A button's brief (a tidy pass, a review): what was asked for, not the page of instructions the host wrote.
    if let Some(asked) = crate::thread_view::brief_line(&m.prompt) {
        return Some(Cow::Owned(asked));
    }
    // Carried on after a plan ran out, or sent back to fix the check: Locust's turns, with lines of their own.
    if m.started_by_kind.as_deref() == Some("carry-on") || fix_of_turn(m).is_some() {
        return None;
    }
    crate::thread_view::person_prompt(m.started_by_kind.as_deref()).then(|| Cow::Borrowed(shown_prompt(m)))
}

/// The files attached to a turn, drawn as rows under its bubble: the person's own and a routine's or a room's turn.
/// A turn typed in the runtime's terminal brought nothing in through Locust.
pub fn turn_attachments(m: &Metadata) -> Vec<String> {
    if !matches!(m.started_by_kind.as_deref(), None | Some("routine" | "room")) {
        return Vec::new();
    }
    crate::attachments::split_attachments(&m.prompt).1
}

/// The runtime whose own terminal a turn was typed in, read from its record in this one place.
pub fn in_terminal(m: &Metadata) -> Option<&str> {
    (m.started_by_kind.as_deref() == Some("terminal")).then_some(m.runtime.as_str())
}

/// The seam drawn above a turn: into a runtime's terminal before its first turn there, back in Locust before the next
/// turn of Locust's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalSeam {
    Into(String),
    Back,
}

pub fn terminal_seam_before(previous: Option<&str>, current: Option<&str>) -> Option<TerminalSeam> {
    match current {
        Some(now) => (Some(now) != previous).then(|| TerminalSeam::Into(now.to_owned())),
        None => previous.map(|_| TerminalSeam::Back),
    }
}

/// The title projection can stop on a relay with a retained received message.
/// Keep ancestry, cycle protection and handoff parsing in one walk.
pub(crate) fn prompt_source<'a>(m: &'a Metadata, by_id: &'a HashMap<String, Metadata>, stop: impl Fn(&Metadata) -> bool) -> (&'a Metadata, &'a str) {
    let mut current = m;
    let mut seen = HashSet::from([m.mission_id.as_str()]);
    loop {
        if stop(current) || current.continues_reason.as_deref() != Some("route-switch") { return (current, shown_prompt(current)); }
        if let Some(asked) = handoff_instruction(&current.prompt) { return (current, asked); }
        let Some(prior) = current.parent.as_ref().and_then(|id| by_id.get(id)) else { return (current, shown_prompt(current)); };
        if !seen.insert(prior.mission_id.as_str()) { return (current, shown_prompt(current)); }
        current = prior;
    }
}
#[derive(Clone,Debug)]pub struct Turn{pub mission_id:String,pub prompt:String,pub switched_from:Option<Switch>,pub in_terminal:Option<String>}
pub fn conversation_turns(m:&Metadata,by_id:&HashMap<String,Metadata>)->Vec<Turn>{let mut seen=HashSet::from([m.mission_id.as_str()]);let mut turns=vec![];let mut latest=m;loop{let mut head=latest;while head.continues_reason.as_deref()==Some("route-switch") && !is_reply_switch(head){let Some(prior)=head.parent.as_ref().and_then(|id|by_id.get(id))else{break};if !seen.insert(prior.mission_id.as_str()){break;}head=prior;}
turns.push(Turn{mission_id:latest.mission_id.clone(),prompt:latest.prompt.clone(),switched_from:switch_of(head,by_id),in_terminal:in_terminal(latest).map(str::to_owned)});
if head.continues_reason.as_deref()!=Some("follow-up") && !is_reply_switch(head){break;}let Some(prior)=head.parent.as_ref().and_then(|id|by_id.get(id))else{break};if !seen.insert(prior.mission_id.as_str()){break;}latest=prior;}
turns.reverse();turns}
#[cfg(test)]#[path="conversation_turns_tests.rs"]mod tests;
