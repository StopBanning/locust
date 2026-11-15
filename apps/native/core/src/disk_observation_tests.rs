use crate::{activity_entries, activity_summary, ledger::{Item, Transcript}};
use serde_json::json;
const FIRST: &str = "--- a/README.md\n+++ b/README.md\n@@ -3 +3 @@\n-Colour: blue\n+Colour: green\n";
const SECOND: &str = "--- a/README.md\n+++ b/README.md\n@@ -4 +4 @@\n-Status: draft\n+Status: ready\n";
fn fold(rows: &[(&str, &str, &str, Option<&str>)]) -> Vec<Item> {
    let mut events = vec![];
    for (id, kind, path, patch) in rows {
        for phase in ["started", "completed"] {
            let mut p = json!({"itemId":id,"toolKind":kind,"name":if *kind == "file_change" {"file_change"} else {"edit"},"command":path,"phase":phase});
            if *kind == "observed_edit" { p["status"] = json!("reported by the runtime, read from disk"); }
            if phase == "completed" && let Some(text) = patch {
                let counts = crate::changes::parse(text).iter().fold((0,0), |(a,d),f| {let c=f.counts();(a+c.0,d+c.1)});
                p["patch"] = json!({"text":text,"added":counts.0,"removed":counts.1,"truncated":false});
            }
            events.push(json!({"recordType":"mission.event","event":{"type":format!("tool.{phase}"),"sequence":events.len()+1,"payload":p}}));
        }
    }
    let mut t=Transcript::default();t.apply(events);t.items
}
#[test]
fn attaches_the_patch_to_the_row_the_runtime_drew_and_draws_a_row_only_for_a_path_it_never_named() {
    let patch="--- /dev/null\n+++ b/NOTES.md\n@@ -0,0 +1,2 @@\n+# Notes\n+First entry.\n";
    let items=fold(&[("fc","file_change","NOTES.md",None),("disk-observed-1","observed_edit","NOTES.md",Some(patch)),("disk-observed-2","observed_edit","other.txt",None)]);
    let notes=items.iter().filter(|i|matches!(i,Item::Tool{summary,..} if summary.contains("NOTES"))).collect::<Vec<_>>();
    assert_eq!(notes.len(),1);
    let Item::Tool{detail,..}=notes[0] else {panic!()};assert_eq!(detail.patch.as_ref().unwrap().added,2);
    assert!(items.iter().any(|i|matches!(i,Item::Tool{summary,..} if summary=="other.txt")));
    assert_eq!(activity_summary::activity_summary(&items),"Edited 2 files");
}
#[test]
fn still_attaches_when_the_runtime_already_sent_its_own_diff_for_that_path() {
    let patch="--- /dev/null\n+++ b/report.md\n@@ -0,0 +1 @@\n+hello\n";
    let items=fold(&[("w","write","report.md",Some(patch)),("obs","observed_edit","report.md",Some(patch))]);
    assert_eq!(activity_entries::net_entries(&items,None).len(),1);
    assert_eq!(activity_summary::activity_summary(&items),"Edited 1 file");
}
#[test]
fn supersedes_every_other_edit_row_for_that_file_so_the_card_counts_the_file_once() {
    let net="--- a/README.md\n+++ b/README.md\n@@ -3,2 +3,2 @@\n-Colour: blue\n-Status: draft\n+Colour: green\n+Status: ready\n";
    let items=fold(&[("e1","edit","C:\\work\\README.md",Some(FIRST)),("e2","edit","C:\\work\\README.md",Some(SECOND)),("obs","observed_edit","README.md",Some(net))]);
    let card=activity_entries::net_entries(&items,Some("C:\\work"));assert_eq!(card.len(),1);assert_eq!(card[0].1.counts,(2,2));
    assert_eq!(activity_summary::activity_summary(&items),"Edited 1 file");
}
#[test]
fn leaves_two_edits_to_one_file_alone_when_no_observation_arrives() {
    let items=fold(&[("e1","edit","README.md",Some(FIRST)),("e2","edit","README.md",Some(SECOND))]);
    assert_eq!(activity_entries::counts(&items),(2,2));
}
