//! Finished-turn totals, derived from the same rows as the activity card.
use crate::ledger::{Item, Status};
use serde_json::Value;
#[derive(Clone, Debug, PartialEq)]
pub struct Segment { pub key: String, pub text: String, pub tone: Option<String> }
#[derive(Clone, Debug, PartialEq)]
pub struct Notice { pub level: String, pub message: String, pub source: String }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace { pub segments: Vec<Segment>, pub notices: Vec<Notice> }
impl Trace { pub fn text(&self) -> String { self.segments.iter().map(|s|s.text.as_str()).collect::<Vec<_>>().join(" · ") } }
pub fn outcome(events: &[Value], running: bool) -> &'static str {
    if events.iter().any(|e|e["type"]=="run.failed") {"failed"} else if events.iter().any(|e|e["type"]=="run.cancelled") {"cancelled"} else if running {"running"} else {"completed"}
}
fn stamp(e:&Value)->Option<i64> { chrono::DateTime::parse_from_rfc3339(e["occurredAt"].as_str()?).ok().map(|t|t.timestamp_millis()) }
pub fn activity_trace(items:&[Item],events:&[Value],outcome:&str)->Trace {
    let mut out=Trace::default();
    let mut push=|key:&str,text:String,amber:bool| out.segments.push(Segment{key:key.into(),text,tone:amber.then(||"amber".into())});
    let times=events.iter().filter_map(stamp).collect::<Vec<_>>();
    let ms=times.iter().max().zip(times.iter().min()).map_or(0,|(a,b)|(a-b).max(0) as u64);
    let stopped=matches!(outcome,"failed"|"cancelled");
    push("duration",format!("{}{}",if stopped{"stopped at "}else{""},crate::ledger::duration_text(ms)),false);
    let edits=items.iter().filter(|i|crate::activity_summary::is_edit(i)).count();
    if outcome=="cancelled" && edits==0 {push("nothing","nothing was changed".into(),false);out.segments.last_mut().unwrap().tone=Some("muted".into());}
    let mut thought=0;let mut open=std::collections::HashMap::new();
    for e in events {let p=&e["payload"];let id=p["itemId"].as_str().unwrap_or("reasoning");
        if p["stepKind"]=="reasoning" {if e["type"]=="step.started" {if let Some(t)=stamp(e){open.insert(id,t);}} else if matches!(e["type"].as_str(),Some("step.completed"|"step.failed")) {if let (Some(a),Some(b))=(open.remove(id),stamp(e)){thought+=(b-a).max(0) as u64;}}}
    }
    // A fresh closure keeps mutations local and leaves the optional muted segment intact.
    let mut push=|key:&str,text:String,amber:bool| out.segments.push(Segment{key:key.into(),text,tone:amber.then(||"amber".into())});
    if thought>=1000 {push("thought",format!("thought {}",crate::ledger::duration_text(thought)),false);}
    if let Some(steps)=items.iter().rev().find_map(|i|if let Item::Plan{steps,..}=i{Some(steps)}else{None}).filter(|s|!s.is_empty()) {push("steps",format!("{} of {} step{}",steps.iter().filter(|s|s.state=="done").count(),steps.len(),if steps.len()==1{""}else{"s"}),false);}
    let helpers=items.iter().filter_map(|i|if let Item::Tool{status,detail,..}=i {detail.helper.then_some((*status,detail))}else{None}).collect::<Vec<_>>();
    if !helpers.is_empty() {let failed=helpers.iter().filter(|(s,_)|*s==Status::Failed).count();let silent=if outcome=="running"{0}else{helpers.iter().filter(|(s,_)|*s==Status::Running).count()};let n=helpers.len();let mut text=format!("asked {n} subagent{}",if n==1{""}else{"s"});if failed>0{text+=&if n==1{" · it failed".into()}else{format!(" · {failed} failed")};}else if silent>0{text+=&if n==1{" · it did not report".into()}else{format!(" · {silent} did not report")};}push("subagents",text,failed+silent>0);}
    let shells=items.iter().filter_map(|i|if let Item::Tool{name,summary,status,detail}=i { (crate::command_outcomes::is_shell(name) && !matches!(detail.runtime_status.as_str(),"refused"|"declined")).then_some((summary,status,detail)) }else{None}).collect::<Vec<_>>();
    let names=shells.iter().map(|(summary,_,d)|if d.arguments.is_empty(){(*summary).clone()}else{d.arguments.clone()}).collect::<Vec<_>>();
    if shells.len()==1 {let command=crate::shell_words::shell_command_text(&names[0]);let command=command.lines().next().unwrap_or("").trim();if !command.is_empty(){let named=if command.chars().count()>60{format!("{}…",command.chars().take(59).collect::<String>())}else{command.into()};let result=crate::command_outcomes::commands_run_text(crate::command_outcomes::commands_run(items,outcome!="running"),&[]);let amber=result.as_ref().is_some_and(|r|r.amber);let suffix=result.filter(|r|r.amber).and_then(|r|r.text.split_once(" · ").map(|(_,s)|format!(" · {s}"))).unwrap_or_default();push("commands",format!("{} {named}{suffix}",if outcome=="running" && *shells[0].1==Status::Running{"running"}else{"ran"}),amber);}}
    else if let Some(result)=crate::command_outcomes::commands_run_text(crate::command_outcomes::commands_run(items,outcome!="running"),&names){push("commands",result.text,result.amber);}
    let calls=items.iter().filter(|i|matches!(i,Item::Tool{name,status,detail,..} if !detail.helper && !crate::command_outcomes::is_shell(name) && (!crate::activity_summary::is_edit(i) || *status==Status::Failed))).count();
    if calls>0 {push("calls",format!("{calls} {}tool call{}",if shells.is_empty(){""}else{"other "},if calls==1{""}else{"s"}),false);}
    if edits>0 {push("files",format!("{edits} file{}",if edits==1{""}else{"s"}),false);}
    for state in ["refused","declined"] {let count=items.iter().filter(|i|matches!(i,Item::Tool{detail,..} if detail.runtime_status==state)).count();if count>0 {push(state,format!("{count} {state}"),true);}}
    out.notices=events.iter().filter(|e|e["type"]=="adapter.diagnostic" && !e["payload"]["code"].as_str().unwrap_or("").starts_with("host.") && !e["payload"]["code"].as_str().unwrap_or("").ends_with(".context_compacted")).filter_map(|e| {let p=&e["payload"];let message=p["message"].as_str()?;(!message.is_empty()).then(||Notice{level:p["level"].as_str().unwrap_or("info").into(),message:crate::ledger::scrub(message),source:e["sourceAdapter"].as_str().unwrap_or("").into()})}).collect();
    out
}
#[cfg(test)] #[path="activity_trace_tests.rs"] mod tests;
