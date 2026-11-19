use super::*;
use crate::ledger::{ToolDetail, Transcript};
use serde_json::json;
const PATCH: &str = "--- a/src/billing.ts\n+++ b/src/billing.ts\n@@ -12,2 +12,3 @@ handle()\n const a = 1;\n-const b = 2;\n+const b = 3;\n+const c = 4;\n";
fn tool(name: &str, target: &str) -> Item { Item::Tool { name: name.into(), summary: target.into(), status: Status::Done, detail: Box::new(ToolDetail { id: "t1".into(), ..Default::default() }) } }
fn edited(text: &str, added: usize, removed: usize, truncated: bool) -> Item {
    let mut item = tool("apply_patch", "apply_patch");
    if let Item::Tool { detail, .. } = &mut item { detail.patch = Some(Patch { text: text.into(), added, removed, truncated }); }
    item
}
#[test] fn turns_one_patch_into_a_row_per_file_it_touched() {
    let entries=files(&edited(&format!("{PATCH}--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n"),3,2,false));
    assert_eq!(entries.iter().map(|e|e.file.path.as_str()).collect::<Vec<_>>(), ["src/billing.ts","README.md"]);
}
#[test] fn sums_the_card_total_from_the_rows_it_will_actually_draw() {
    let item=edited(PATCH,900,900,true);
    assert_eq!(counts(&[item.clone()]),(2,1)); assert_eq!(files(&item)[0].counts,(2,1));
}
#[test] fn keeps_the_runtime_total_beside_a_truncated_single_file_patch_and_withholds_it_across_several() {
    assert_eq!(files(&edited(PATCH,900,900,true))[0].reported,Some((900,900)));
    assert_eq!(files(&edited(&format!("{PATCH}--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n"),900,900,true))[0].reported,None);
}
#[test] fn keeps_an_edit_whose_runtime_reported_no_patch_as_a_row_that_says_so() {
    assert_eq!(unreported(&tool("apply_patch","apply_patch")),Some(("apply_patch",true,false)));
}
#[test] fn says_an_edit_whose_result_was_too_large_to_keep_rather_than_that_nobody_confirmed_it() {
    let mut item=tool("edit","src/app.ts");
    if let Item::Tool {detail,..}=&mut item {detail.runtime_status="result too large to keep".into();}
    assert_eq!(unreported(&item),Some(("src/app.ts",true,true)));
}
#[test] fn names_the_tool_on_a_row_whose_target_is_the_same_path_it_edited() {
    let read=tool("read","src/billing.ts");
    let Item::Tool{name,summary,..}=&read else {panic!()};
    assert_eq!(summary,"src/billing.ts"); assert_eq!(tool_name(name,summary),Some("read"));
    assert_eq!(tool_name("grep","grep"),None);
}
#[test] fn folds_a_run_of_plain_tool_calls_and_never_the_command_beside_them() {
    let rows=crate::tool_fold::fold_plain_tool_runs(&[tool("read","src/billing.ts"),tool("read","src/tax.ts"),tool("shell","pnpm test")]);
    assert!(matches!(&rows[..],[crate::tool_fold::Row::Folded{..},crate::tool_fold::Row::One(2)]));
}
#[test] fn carries_a_command_row_with_its_exit_result() {
    let mut t=Transcript::default();
    t.apply(vec![json!({"recordType":"mission.event","event":{"type":"tool.completed","sequence":1,"payload":{"itemId":"t1","name":"shell","command":"pnpm test","exitCode":1}}})]);
    let Item::Tool{summary,status,detail,..}=&t.items[0] else {panic!()};
    assert_eq!((summary.as_str(),*status,detail.exit_code),("pnpm test",Status::Failed,Some(1)));
}
fn huge(path: &str, page: bool) -> String {
    format!("--- {}\n+++ b/{path}\n@@ -{} +1,400 @@\n{}",if page {"/dev/null".into()}else{format!("a/{path}")},if page{"0,0"}else{"1"},(0..400).map(|i|if page{format!("+<p>line {i}</p>")}else{format!("+line {i}")}).collect::<Vec<_>>().join("\n"))
}
#[test] fn opens_the_first_file_unless_opening_it_would_bury_everything_after_it() {
    let small=files(&edited(PATCH,2,1,false)); assert_eq!(default_open(&small),Some(small[0].key.as_str()));
    let big=files(&edited(&huge("big.ts",false),400,0,false)); assert!(big[0].large); assert_eq!(default_open(&big),None);
}
#[test] fn opens_a_new_web_page_however_long_it_is_because_it_is_shown_running_0_446() {
    let built=files(&edited(&huge("index.html",true),400,0,false));
    assert_eq!((built[0].large,built[0].file.status.as_str()),(true,"ADDED")); assert_eq!(default_open(&built),Some(built[0].key.as_str()));
}
#[test] fn attaches_a_completion_patch_to_the_tool_that_opened_and_names_the_runtime_that_reported_it() {
    let events=[json!({"type":"tool.started","sequence":1,"sourceAdapter":"codex","payload":{"itemId":"t1","toolKind":"file_change","name":"apply_patch","phase":"started"}}),json!({"type":"tool.completed","sequence":2,"sourceAdapter":"codex","payload":{"itemId":"t1","toolKind":"file_change","name":"apply_patch","phase":"completed","patch":{"text":PATCH,"added":2,"removed":1,"truncated":false}}})];
    let mut t=Transcript::default(); t.apply(events.into_iter().map(|event|json!({"recordType":"mission.event","event":event})).collect());
    assert_eq!(counts(&t.items),(2,1));
    let Item::Tool{detail,..}=&t.items[0] else {panic!()}; assert_eq!(detail.reported_by,"codex");
}

// diff.ts fileRowCounts: across a cut patch of several files the tool's total belongs to none, and a cut excerpt's
// lines are not a file's change, so no row and no header says counts; an uncut patch says each file's own.
#[test]
fn a_cut_patch_of_several_files_says_no_counts_and_an_uncut_one_says_each_files() {
    let text = "--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-x\n+y\n--- a/b.ts\n+++ b/b.ts\n@@ -1 +1,2 @@\n-p\n+q\n+r\n";
    let cut = edited(text, 900, 900, true);
    let rows = files(&cut);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|f| f.row_counts().is_none() && f.large), "{rows:?}");
    assert_eq!(header_counts(&[cut], None), None);
    let whole = edited(text, 3, 2, false);
    assert_eq!(files(&whole).iter().map(FileEntry::row_counts).collect::<Vec<_>>(), vec![Some((1, 1)), Some((2, 1))]);
    assert_eq!(header_counts(&[whole], None), Some((3, 2)));
}
