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
/// The title projection can stop on a relay with a retained received message.
/// Keep ancestry, cycle protection and handoff parsing in one walk.
pub(crate) fn prompt_source<'a>(m: &'a Metadata, by_id: &'a HashMap<String, Metadata>, stop: impl Fn(&Metadata) -> bool) -> (&'a Metadata, &'a str) {
    let mut current = m;
    let mut seen = HashSet::from([m.mission_id.as_str()]);
    loop {
        if stop(current) || current.continues_reason.as_deref() != Some("route-switch") { return (current, &current.prompt); }
        if let Some(asked) = handoff_instruction(&current.prompt) { return (current, asked); }
        let Some(prior) = current.parent.as_ref().and_then(|id| by_id.get(id)) else { return (current, &current.prompt); };
        if !seen.insert(prior.mission_id.as_str()) { return (current, &current.prompt); }
        current = prior;
    }
}
#[derive(Clone,Debug)]pub struct Turn{pub mission_id:String,pub prompt:String,pub switched_from:Option<Switch>}
pub fn conversation_turns(m:&Metadata,by_id:&HashMap<String,Metadata>)->Vec<Turn>{let mut seen=HashSet::from([m.mission_id.as_str()]);let mut turns=vec![];let mut latest=m;loop{let mut head=latest;while head.continues_reason.as_deref()==Some("route-switch") && !is_reply_switch(head){let Some(prior)=head.parent.as_ref().and_then(|id|by_id.get(id))else{break};if !seen.insert(prior.mission_id.as_str()){break;}head=prior;}
turns.push(Turn{mission_id:latest.mission_id.clone(),prompt:latest.prompt.clone(),switched_from:switch_of(head,by_id)});
if head.continues_reason.as_deref()!=Some("follow-up") && !is_reply_switch(head){break;}let Some(prior)=head.parent.as_ref().and_then(|id|by_id.get(id))else{break};if !seen.insert(prior.mission_id.as_str()){break;}latest=prior;}
turns.reverse();turns}
#[cfg(test)]#[path="conversation_turns_tests.rs"]mod tests;
