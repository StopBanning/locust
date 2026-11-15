use crate::{ledger::{Transcript, Item}, thread_view::live_line};
use serde_json::{json, Value};
const AT:&str="2026-09-01T15:00:00.000Z";
fn event(ty:&str, payload:Value)->Value {json!({"type":ty,"occurredAt":AT,"sourceAdapter":"codex","payload":payload})}
fn notice()->Value {event("adapter.diagnostic",json!({"level":"warning","code":"codex.item_error","message":"Skill descriptions were shortened.","terminal":false}))}
fn step()->Value {event("step.started",json!({"stepKind":"turn"}))}
fn fold(events:Vec<Value>,running:bool)->Transcript {
    let mut all=vec![event("run.started",json!({}))];all.extend(events);
    if !running {all.push(event("run.completed",json!({})));}
    let mut t=Transcript::default();t.apply(all.into_iter().enumerate().map(|(i,mut e)|{e["sequence"]=json!(i+1);json!({"recordType":"mission.event","event":e})}).collect());t
}
fn said(t:&Transcript, words:&str)->bool {t.items.iter().any(|i|matches!(i,Item::Note{text,..} if text.contains(words)))}
#[test]
fn says_a_notice_raised_before_any_work_once_the_turn_ends_with_no_fold_to_put_it_under() {
    assert!(said(&fold(vec![notice(),step()],false),"Skill descriptions were shortened."));
    assert!(said(&fold(vec![step(),notice()],false),"Skill descriptions were shortened."));
    assert!(!said(&fold(vec![step(),notice()],true),"Skill descriptions were shortened."));
}
#[test]
fn shows_a_run_in_trouble_even_before_any_tool_has_run() {
    let reconnect=event("adapter.diagnostic",json!({"level":"error","code":"codex.runtime_error","message":"Reconnecting... 2/5","terminal":false}));
    assert!(said(&fold(vec![step(),reconnect],true),"Reconnecting"));
}
#[test]
fn shows_a_rate_limited_opencode_run_retrying_before_anything_has_run() {
    let retry=event("adapter.diagnostic",json!({"level":"warning","code":"opencode.runtime_error","message":"The model's provider answered \"Rate limit exceeded\", and OpenCode is trying again on its own.","terminal":false}));
    assert!(said(&fold(vec![retry.clone()],true),"trying again on its own"));
    let line=live_line(&[event("run.started",json!({})),retry],true,AT,false).unwrap();
    assert_eq!((line.label.as_str(),line.register.as_str()),("Trying the model again","starting"));
}
#[test]
fn shows_a_notice_raised_while_the_work_was_under_way() {
    assert!(said(&fold(vec![step(),event("tool.started",json!({"itemId":"t1","toolKind":"shell","name":"shell","command":"pnpm test"})),notice()],false),"Skill descriptions were shortened."));
}
