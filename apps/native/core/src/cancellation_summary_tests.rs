use super::*;
use crate::ledger::Transcript;
use serde_json::{json,Value};
fn start(id:&str,name:&str,command:&str)->Value{json!({"type":"tool.started","payload":{"itemId":id,"name":name,"toolKind":name,"command":command,"phase":"started"}})}
fn done(id:&str)->Value{json!({"type":"tool.completed","payload":{"itemId":id,"phase":"completed"}})}
fn summarize(events:Vec<Value>,planned:usize,workspace:Option<&str>)->Summary{let mut t=Transcript::default();t.apply(events.into_iter().enumerate().map(|(i,mut event)|{event["sequence"]=json!(i+1);json!({"recordType":"mission.event","event":event})}).collect());summary_with_planned(&t.items,"codex",workspace,planned)}
#[test]fn separates_tool_outcomes_and_never_substitutes_them_for_unreported_plan_states(){let s=summarize(vec![start("t1","shell","pnpm build"),done("t1"),start("t2","shell","pnpm test")],4,None);assert_eq!(s.settled,["pnpm build"]);assert_eq!(s.interrupted,["pnpm test"]);assert_eq!(s.never_started,4);}
#[test]fn counts_one_file_once_however_each_tool_spelled_its_path(){let s=summarize(vec![start("t1","read","C:\\work\\note-1.txt"),done("t1"),start("t2","write","note-1.txt"),done("t2")],0,Some("C:\\work"));assert_eq!(s.settled,["note-1.txt"]);}
#[test]fn still_counts_two_genuinely_different_files_as_two(){let s=summarize(vec![start("t1","write","note-1.txt"),done("t1"),start("t2","write","note-2.txt"),done("t2")],0,Some("C:\\work"));assert_eq!(s.settled,["note-1.txt","note-2.txt"]);}
#[test]fn does_not_repeat_a_finished_item_in_the_interrupted_list(){let s=summarize(vec![start("t1","write","note-1.txt"),done("t1"),start("t2","write","note-1.txt")],0,None);assert_eq!(s.settled,["note-1.txt"]);assert_eq!(s.interrupted,Vec::<String>::new());}
#[test]fn never_reports_a_negative_count_when_more_ran_than_were_planned(){let s=summarize(vec![start("t1","shell","a"),done("t1")],0,None);assert_eq!(s.never_started,0);}
