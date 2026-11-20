//! Electron `main/a-file-written-elsewhere-opens.test.ts`: what a turn names as its own, and the host's lookup in its own
//! ledger. The events are that turn's, from Colin's ledger (2026-10-18): the patch's target as a whole path, the handover
//! in fragments. The cases that ask the host's whole decision are in the app's `src/named_file_tests.rs`.
use super::*;
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

const REPORT: &str = "/work/scratch/toaster-review2/Codex.md";
const NEVER_NAMED: &str = "/home/someone/.ssh/id_rsa";

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    let mut payload = payload;
    payload["evidence"] = json!({"redacted": false});
    json!({"id": format!("run_review:{sequence}"), "runId": "run_review", "missionId": "mission_review", "sequence": sequence,
        "type": kind, "occurredAt": "2026-10-18T20:00:00.000Z", "sourceAdapter": "codex", "payload": payload})
}
fn said(sequence: u64, text: &str, last: bool) -> Value {
    event(sequence, "message.delta", json!({"itemId": "msg_1", "operation": "append", "text": text, "final": last}))
}
fn step(sequence: u64, kind: &str, command: &str, status: Option<&str>) -> Value {
    let mut p = json!({"itemId": "item_patch", "toolKind": "fileChange", "name": "apply_patch", "command": command,
        "phase": if kind == "tool.started" { "started" } else { "completed" }});
    if let Some(s) = status {
        p["status"] = json!(s);
    }
    event(sequence, kind, p)
}
fn wrote() -> Vec<Value> {
    vec![step(1, "tool.started", REPORT, None), step(2, "tool.completed", REPORT, Some("completed"))]
}
fn handed_over() -> Vec<Value> {
    vec![
        said(3, "The review found four issues.\n\n<locust-", false),
        said(4, &format!("file>\n{REPORT} :: Review findings, evidence, fixes and verification limits\n</locust-file>"), true),
    ]
}
fn the_turn() -> Vec<Value> {
    [wrote(), handed_over()].concat()
}
fn key(path: &str) -> Vec<String> {
    vec![named_file_key(path).unwrap()]
}

// ---- what a turn names as its own --------------------------------------------------------------------------------

// case: names the file its finished step wrote and the file it handed over, as one file
#[test]
fn names_the_file_its_finished_step_wrote_and_the_file_it_handed_over_as_one_file() {
    assert_eq!(files_named_in(&the_turn()), key(REPORT));
    // The handover alone, in its own spelling, is the same file.
    assert_eq!(files_named_in(&handed_over()), key(REPORT));
    assert_eq!(files_named_in(&wrote()), key(REPORT));
    // A message rewritten whole names what it says at the end, not what it said before.
    let replaced = vec![
        said(1, "<locust-file>\n/work/scratch/draft.md\n</locust-file>", false),
        event(2, "message.delta", json!({"itemId": "msg_1", "operation": "replace", "text": format!("<locust-file>\n{REPORT}\n</locust-file>"), "final": true})),
    ];
    assert_eq!(files_named_in(&replaced), key(REPORT));
    // One spelling per file: resolved.
    assert_eq!(named_file_key("/work/scratch/./x/../toaster-review2/Codex.md").as_deref(), Some(REPORT));
}

// case: names nothing for a step that was declined, failed, or never finished
#[test]
fn names_nothing_for_a_step_that_was_declined_failed_or_never_finished() {
    assert!(files_named_in(&[step(1, "tool.started", REPORT, None)]).is_empty());
    assert!(files_named_in(&[step(1, "tool.failed", REPORT, None)]).is_empty());
    assert!(files_named_in(&[step(1, "tool.completed", REPORT, Some("declined"))]).is_empty());
    for status in ["Refused", "denied", "rejected", "failed", "error", "cancelled"] {
        assert!(files_named_in(&[step(1, "tool.completed", REPORT, Some(status))]).is_empty(), "{status}");
    }
    // A finished step with no status at all names its target.
    assert_eq!(files_named_in(&[step(1, "tool.completed", REPORT, None)]), key(REPORT));
}

// case: does not read a shell command for paths, nor take a network path or a relative one
#[test]
fn does_not_read_a_shell_command_for_paths_nor_take_a_network_path_or_a_relative_one() {
    let shell = format!("powershell -Command \"Get-Content '{REPORT}'\"");
    assert!(files_named_in(&[step(1, "tool.completed", &shell, Some("completed"))]).is_empty());
    assert!(files_named_in(&[step(1, "tool.completed", "\\\\server\\share\\report.md", Some("completed"))]).is_empty());
    assert!(files_named_in(&[said(1, "<locust-file>\n//server/share/report.md\ndocs/report.md\n</locust-file>", true)]).is_empty());
    assert_eq!(named_file_key("\\\\?\\C:\\x.md"), None);
    // A step's target that is a path line by line names each line that is one; a line in a sentence names nothing.
    let lines = format!("{REPORT}\nsee {NEVER_NAMED}\n");
    assert_eq!(files_named_in(&[step(1, "tool.completed", &lines, Some("completed"))]), key(REPORT));
    // Nothing past 400 characters, and nothing with a control character in it.
    let long = format!("/{}", "a".repeat(399));
    assert!(named_file_key(&long).is_some());
    assert_eq!(named_file_key(&format!("{long}a")), None);
    assert_eq!(named_file_key("/work/a\u{7}b.md"), None);
    assert_eq!(named_file_key("   "), None);
}

// case: reads a handover only from what the teammate said, never a file block quoted in code
#[test]
fn reads_a_handover_only_from_what_the_teammate_said_never_a_file_block_quoted_in_code() {
    let quoted = format!("Write it like this:\n\n```\n<locust-file>\n{REPORT}\n</locust-file>\n```");
    assert!(files_named_in(&[said(1, &quoted, true)]).is_empty());
}

// ---- the host's lookup in its own ledger -------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Ledger(PathBuf);
impl Drop for Ledger {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Ledger {
    /// A ledger folder holding `mission_review`, created as the turn was, in `workspace_id`'s folder.
    fn new(workspace_id: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-named-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        let head = json!({"schemaVersion": 1, "recordType": "mission.created", "metadata": {
            "missionId": "mission_review", "runId": "run_review",
            "prompt": "Review the Toaster changes and write the report to D:\\work\\scratch\\toaster-review2\\Codex.md",
            "runtime": "codex", "model": "gpt-6.1-sol", "requestedRouteId": "codex:gpt-6.1-sol", "resolvedRouteId": "codex:gpt-6.1-sol",
            "cliVersion": "0.160.0", "workspaceId": workspace_id, "createdAt": "2026-10-18T20:00:00.000Z",
            "sandbox": "workspace-write", "executionPolicyVersion": 1}});
        std::fs::write(dir.join("mission_review.jsonl"), format!("{head}\n")).unwrap();
        Self(dir)
    }
    fn append(&self, events: &[Value]) {
        let mut f = std::fs::OpenOptions::new().append(true).open(self.0.join("mission_review.jsonl")).unwrap();
        for e in events {
            writeln!(f, "{}", json!({"schemaVersion": 1, "recordType": "mission.event", "event": e})).unwrap();
        }
    }
    fn dirs(&self) -> Vec<PathBuf> {
        vec![self.0.clone()]
    }
}

// case: finds a file handed over while the turn is still running
#[test]
fn finds_a_file_handed_over_while_the_turn_is_still_running() {
    let ledger = Ledger::new("ws_claude");
    ledger.append(&[said(1, "Working on it. ", false)]);
    assert!(!named_by_a_teammate(&ledger.dirs(), REPORT, &[]));
    // The same record, grown: the cache reads a changed file again.
    ledger.append(&[
        said(2, "The review found four issues.\n\n<locust-", false),
        said(3, &format!("file>\n{REPORT} :: Review findings\n</locust-file>"), true),
    ]);
    assert!(named_by_a_teammate(&ledger.dirs(), REPORT, &[]));
}

// case: stops opening it once the conversation is deleted
#[test]
fn stops_opening_it_once_the_conversation_is_deleted() {
    let ledger = Ledger::new("ws_claude");
    ledger.append(&the_turn());
    assert!(named_by_a_teammate(&ledger.dirs(), REPORT, &[]));
    assert!(!named_by_a_teammate(&ledger.dirs(), NEVER_NAMED, &[]));
    std::fs::remove_file(ledger.0.join("mission_review.jsonl")).unwrap();
    assert!(!named_by_a_teammate(&ledger.dirs(), REPORT, &[]));
}

// ---- a file handed over by a path from the folder its turn ran in (0.868) ----------------------------------------

const FOLDER: &str = "/work/now-coding";
const ELSEWHERE_FOLDER: &str = "/work/other";
const WORKTREE_REPORT: &str = "/work/locust-wt/spotify-artist/REPORT.md";
fn handed_relative() -> Vec<Value> {
    vec![said(
        1,
        "Committed on `exec/spotify-artist`.\n\n<locust-file>\n../locust-wt/spotify-artist/REPORT.md :: Implementation and verification limits\n</locust-file>",
        true,
    )]
}
fn relative_ledger() -> Ledger {
    let ledger = Ledger::new(&crate::workspace_settings::workspace_id_for(Path::new(FOLDER)));
    ledger.append(&handed_relative());
    ledger
}

// case: keeps the relative path a file block hands over, and no whole path for it
#[test]
fn keeps_the_relative_path_a_file_block_hands_over_and_no_whole_path_for_it() {
    assert_eq!(relative_files_named_in(&handed_relative()), vec!["../locust-wt/spotify-artist/REPORT.md".to_owned()]);
    assert!(files_named_in(&handed_relative()).is_empty());
    assert!(relative_files_named_in(&[said(1, "<locust-file>\n//server/share/report.md\n</locust-file>", true)]).is_empty());
    // A whole path is not a relative one; a drive spelling is never one.
    assert!(relative_files_named_in(&[said(1, &format!("<locust-file>\n{REPORT}\nC:/x.md\n</locust-file>"), true)]).is_empty());
    assert_eq!(relative_named_key(FOLDER, "../locust-wt/spotify-artist/REPORT.md").as_deref(), Some(WORKTREE_REPORT));
}

// case: resolves it against that folder only, never another one it knows
#[test]
fn resolves_it_against_that_folder_only_never_another_one_it_knows() {
    let ledger = relative_ledger();
    // From the other folder the same words would name /work/locust-wt/... too, so ask for a file only another folder
    // resolves to.
    let from_other = "/locust-wt/spotify-artist/REPORT.md";
    assert!(!named_by_a_teammate(&ledger.dirs(), from_other, &[FOLDER.into(), "/work/deep/other".into()]));
    // And without the folder it ran in among the known ones, nothing counts.
    assert!(!named_by_a_teammate(&ledger.dirs(), WORKTREE_REPORT, &[ELSEWHERE_FOLDER.into()]));
    assert!(!named_by_a_teammate(&ledger.dirs(), WORKTREE_REPORT, &[]));
    // The control: with it, the file counts.
    assert!(named_by_a_teammate(&ledger.dirs(), WORKTREE_REPORT, &[ELSEWHERE_FOLDER.into(), FOLDER.into()]));
}
