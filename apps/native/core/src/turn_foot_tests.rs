//! a-turn-says-what-it-changed-apart-from-the-folder.test.tsx and a-resumed-file-card-matches-the-folder.test.tsx,
//! with Electron's events replayed through `Transcript::apply`; the window's half is in `src/ui/turn_foot_tests.rs`.
use super::*;
use crate::ledger::Transcript;
use serde_json::{Value, json};

struct Events(Vec<Value>);
impl Events {
    fn new() -> Self {
        Events(vec![])
    }
    fn push(&mut self, kind: &str, payload: Value) -> &mut Self {
        let n = self.0.len() + 1;
        self.0.push(json!({"recordType": "mission.event", "event": {"type": kind, "sequence": n, "occurredAt": "2026-09-30T05:00:00.000Z", "payload": payload}}));
        self
    }
    fn tool(&mut self, id: &str, kind: &str, name: &str, path: &str, status: Option<&str>, patch: Option<Value>) -> &mut Self {
        let mut p = json!({"itemId": id, "toolKind": kind, "name": name, "command": path, "phase": "started"});
        if let Some(status) = status { p["status"] = json!(status); }
        self.push("tool.started", p.clone());
        p["phase"] = json!("completed");
        if let Some(patch) = patch { p["patch"] = patch; }
        self.push("tool.completed", p)
    }
    fn items(&self) -> Vec<Item> {
        let mut t = Transcript::default();
        t.apply(self.0.clone());
        t.items
    }
}

fn new_file(path: &str) -> Value {
    json!({"text": format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+x\n"), "truncated": false, "added": 1, "removed": 0})
}

/// The report Claude Code wrote, then the host's look: two files Claude Code itself keeps in the folder.
fn report_and_backups(write: bool) -> Events {
    let mut e = Events::new();
    e.push("run.started", json!({}));
    e.push("message.delta", json!({"itemId": "m1", "operation": "append", "text": "Writing the report.", "final": true}));
    if write {
        e.tool("w1", "file_change", "Write", "report.md", None, Some(new_file("report.md")));
    }
    e.push("message.delta", json!({"itemId": "m2", "operation": "append", "text": "Done.", "final": true}));
    e.push("run.completed", json!({"process": {"exitCode": 0}}));
    for (i, path) in ["file-history/abc@v2", "backups/.claude.json.backup.1"].iter().enumerate() {
        e.tool(&format!("obs{i}"), "observed_edit", "edit", path, Some("observed on disk"), Some(new_file(path)));
    }
    e
}

#[test]
fn counts_what_the_runtime_edited_apart_from_what_only_changed_in_the_folder() {
    let foot = turn_foot(&report_and_backups(true).items(), Some("C:/work")).expect("a foot");
    assert_eq!(foot.heading, "Edited 1 file \u{b7} 2 more changed in the folder");
}

#[test]
fn says_a_turn_that_edited_nothing_itself_only_saw_files_change() {
    let foot = turn_foot(&report_and_backups(false).items(), Some("C:/work")).expect("a foot");
    assert_eq!(foot.heading, "2 files changed in the folder while it ran");
}

#[test]
fn counts_a_file_the_host_could_not_read_once_as_the_step_that_changed_it_0_672() {
    let mut e = Events::new();
    e.push("run.started", json!({}));
    e.push("message.delta", json!({"itemId": "m1", "operation": "append", "text": "Writing the report.", "final": true}));
    e.tool("w1", "file_change", "Write", "report.md", None, Some(new_file("report.md")));
    e.push("message.delta", json!({"itemId": "m2", "operation": "append", "text": "Done.", "final": true}));
    e.push("run.completed", json!({"process": {"exitCode": 0}}));
    e.tool("obs-r", "observed_edit", "edit", "report.md", Some("reported by the runtime, changed on disk"), None);
    let foot = turn_foot(&e.items(), Some("C:/work")).expect("a foot");
    assert_eq!(foot.heading, "Edited 1 file");
}

const WORKSPACE: &str = "C:/Users/me/.locust/compare/cmp_demo-a";
const SCRATCH: &str = "C:/Users/me/scratch/ember";

fn resumed() -> Events {
    let mut e = Events::new();
    e.push("run.started", json!({}));
    e.push("message.delta", json!({"itemId": "m1", "operation": "append", "text": "Continuing.", "final": true}));
    for (i, name) in ["cdp.mjs", "play.mjs", "patch5.mjs", "t3.mjs"].iter().enumerate() {
        e.tool(&format!("w{i}"), "tool_use", "Write", &format!("{SCRATCH}/{name}"), None, None);
    }
    e.tool("same1", "tool_use", "Write", &format!("{SCRATCH}/profile.mjs"), None, None);
    e.tool("same2", "tool_use", "Edit", &format!("{SCRATCH}/profile.mjs"), None, None);
    e.push("message.delta", json!({"itemId": "m2", "operation": "append", "text": "Done.", "final": true}));
    e.push("run.completed", json!({"process": {"exitCode": 0}}));
    let mut patch = new_file("index.html");
    patch["added"] = json!(3456);
    e.tool("obs", "observed_edit", "edit", "index.html", Some("observed on disk"), Some(patch));
    e
}

#[test]
fn counts_the_folders_change_not_every_write_outside_it_and_does_not_blame_claude_code_for_those_writes() {
    let foot = turn_foot(&resumed().items(), Some(WORKSPACE)).expect("a foot");
    assert_eq!(foot.heading, "1 file changed in the folder while it ran");
    assert_eq!(foot.rows.len(), 1, "{:?}", foot.rows);
    assert!(matches!(&foot.rows[0], Row::File { entry, .. } if entry.file.path == "index.html"));
}

#[test]
fn keeps_one_row_when_the_same_file_was_written_twice_without_a_diff() {
    // Outside the folder and nothing inside: all the turn has to show, and the one file is one row.
    let mut e = Events::new();
    e.push("run.started", json!({}));
    e.tool("same1", "tool_use", "Write", &format!("{SCRATCH}/profile.mjs"), None, None);
    e.tool("same2", "tool_use", "Edit", &format!("{SCRATCH}/profile.mjs"), None, None);
    e.push("run.completed", json!({"process": {"exitCode": 0}}));
    let foot = turn_foot(&e.items(), Some(WORKSPACE)).expect("a foot");
    assert_eq!(foot.rows.iter().filter(|r| matches!(r, Row::Unreported { .. })).count(), 1, "{:?}", foot.rows);
    assert_eq!(foot.heading, "Edited 1 file");
}

#[test]
fn knows_a_scratch_path_is_not_the_compare_folder() {
    assert!(!path_in_workspace(&format!("{SCRATCH}/cdp.mjs"), Some(WORKSPACE)));
    assert!(path_in_workspace("index.html", Some(WORKSPACE)));
    assert!(path_in_workspace(&format!("{WORKSPACE}/index.html"), Some(WORKSPACE)));
    assert!(path_in_workspace("C:\\Users\\me\\.locust\\compare\\cmp_demo-a\\src\\a.ts", Some(WORKSPACE)), "either slash, any case");
    assert!(path_in_workspace("/anywhere/at/all", None), "no folder: everything is the turn's");
}

#[test]
fn heads_the_card_as_electrons_files_heading_does() {
    assert_eq!(files_heading(1, 0, false), "Edited 1 file");
    assert_eq!(files_heading(3, 1, false), "Edited 3 files \u{b7} 1 more changed in the folder");
    assert_eq!(files_heading(0, 1, false), "1 file changed in the folder while it ran");
    assert_eq!(files_heading(2, 0, true), "Edited 2 files \u{b7} undone");
}

#[test]
fn adds_undone_to_the_heading_and_only_once_undo_has_put_files_back() {
    assert_eq!(files_heading(1, 0, false), "Edited 1 file");
    assert_eq!(files_heading(1, 0, true), "Edited 1 file \u{b7} undone");
    assert_eq!(files_heading(2, 1, true), "Edited 2 files \u{b7} 1 more changed in the folder \u{b7} undone");
    assert_eq!(files_heading(0, 3, true), "3 files changed in the folder while it ran \u{b7} undone");
}

#[test]
fn has_no_foot_when_nothing_changed() {
    let mut e = Events::new();
    e.push("run.started", json!({}));
    e.push("message.delta", json!({"itemId": "m1", "operation": "append", "text": "Hi.", "final": true}));
    e.push("run.completed", json!({}));
    assert!(turn_foot(&e.items(), Some("C:/work")).is_none());
}
