//! Electron `main/a-file-written-elsewhere-opens.test.ts`: the host, asked to open a file outside every folder it knows
//! (`decide_teammate_file` with the ledger lookup `locust_core::named_files::named_by_a_teammate`). What a turn names on
//! its own is in core's `named_files_tests.rs`.
use super::*;
use locust_core::named_files::named_by_a_teammate;
use serde_json::{Value, json};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

const REPORT: &str = "/work/scratch/toaster-review2/Codex.md";
const NEVER_NAMED: &str = "/home/someone/.ssh/id_rsa";
const WORKSPACE: &str = "/home/someone/.claude";

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Ledger(PathBuf);
impl Drop for Ledger {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Ledger {
    fn new(workspace_id: &str, events: &[Value]) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-named-host-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        let head = json!({"schemaVersion": 1, "recordType": "mission.created", "metadata": {
            "missionId": "mission_review", "runId": "run_review", "prompt": "Review the Toaster changes and write a report",
            "runtime": "codex", "model": "gpt-6.1-sol", "requestedRouteId": "codex:gpt-6.1-sol", "resolvedRouteId": "codex:gpt-6.1-sol",
            "cliVersion": "0.160.0", "workspaceId": workspace_id, "createdAt": "2026-10-18T20:00:00.000Z",
            "sandbox": "workspace-write", "executionPolicyVersion": 1}});
        let mut f = std::fs::File::create(dir.join("mission_review.jsonl")).unwrap();
        writeln!(f, "{head}").unwrap();
        for e in events {
            writeln!(f, "{}", json!({"schemaVersion": 1, "recordType": "mission.event", "event": e})).unwrap();
        }
        Self(dir)
    }
    fn named(&self, path: &Path, folders: &[&str]) -> bool {
        let folders: Vec<String> = folders.iter().map(|f| (*f).to_owned()).collect();
        named_by_a_teammate(std::slice::from_ref(&self.0), &path.to_string_lossy(), &folders)
    }
}

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({"id": format!("run_review:{sequence}"), "runId": "run_review", "missionId": "mission_review", "sequence": sequence,
        "type": kind, "occurredAt": "2026-10-18T20:00:00.000Z", "sourceAdapter": "codex", "payload": payload})
}
fn the_turn() -> Vec<Value> {
    let step = |sequence, kind: &str, status: Option<&str>| {
        let mut p = json!({"itemId": "item_patch", "toolKind": "fileChange", "name": "apply_patch", "command": REPORT});
        if let Some(s) = status {
            p["status"] = json!(s);
        }
        event(sequence, kind, p)
    };
    vec![
        step(1, "tool.started", None),
        step(2, "tool.completed", Some("completed")),
        event(3, "message.delta", json!({"itemId": "msg_1", "operation": "append", "text": "The review found four issues.\n\n<locust-", "final": false})),
        event(4, "message.delta", json!({"itemId": "msg_1", "operation": "append", "text": format!("file>\n{REPORT} :: Review findings, evidence, fixes and verification limits\n</locust-file>"), "final": true})),
    ]
}
fn roots(of: &[&str]) -> Vec<PathBuf> {
    of.iter().map(PathBuf::from).collect()
}

// ---- the host, asked to open a file outside every folder it knows ------------------------------------------------

// case: opens the one the teammate wrote and handed over, in any spelling of it
#[test]
fn opens_the_one_the_teammate_wrote_and_handed_over_in_any_spelling_of_it() {
    let ledger = Ledger::new("ws_claude", &the_turn());
    let decision = decide_teammate_file(REPORT, &roots(&[WORKSPACE]), |p| ledger.named(p, &[]));
    assert_eq!(decision, RevealDecision::Ok { path: PathBuf::from(REPORT), named: true });
    // Another spelling of the same file is the same file.
    let spelled = "/work/scratch/./toaster-review2/../toaster-review2/Codex.md";
    assert_eq!(decide_teammate_file(spelled, &roots(&[WORKSPACE]), |p| ledger.named(p, &[])), decision);
    // A read asks its link question of the file's own folder: it is in no root.
    assert_eq!(read_roots(Path::new(REPORT), true, &roots(&[WORKSPACE])), vec![PathBuf::from("/work/scratch/toaster-review2")]);
    assert_eq!(read_roots(Path::new("/home/someone/.claude/a.md"), false, &roots(&[WORKSPACE])), roots(&[WORKSPACE]));
}

// case: refuses a file no teammate named, the way it always has
#[test]
fn refuses_a_file_no_teammate_named_the_way_it_always_has() {
    let ledger = Ledger::new("ws_claude", &the_turn());
    assert_eq!(
        decide_teammate_file(NEVER_NAMED, &roots(&[WORKSPACE]), |p| ledger.named(p, &[])),
        RevealDecision::Outside { path: NEVER_NAMED.into() }
    );
}

// case: never looks up a network path, a relative one, or one inside the folder
#[test]
fn never_looks_up_a_network_path_a_relative_one_or_one_inside_the_folder() {
    let asked = std::cell::RefCell::new(Vec::<PathBuf>::new());
    let named = |p: &Path| {
        asked.borrow_mut().push(p.to_owned());
        true
    };
    let r = roots(&[WORKSPACE]);
    assert!(!matches!(decide_teammate_file("\\\\server\\share\\report.md", &r, named), RevealDecision::Ok { .. }));
    assert!(!matches!(decide_teammate_file("//server/share/report.md", &r, named), RevealDecision::Ok { .. }));
    assert!(!matches!(decide_teammate_file("/\\server/share/report.md", &r, named), RevealDecision::Ok { .. }));
    assert!(!matches!(decide_teammate_file("docs/report.md", &r, named), RevealDecision::Ok { .. }));
    assert!(matches!(decide_teammate_file("   ", &r, named), RevealDecision::NoPath));
    let in_folder = format!("{WORKSPACE}/notes.md");
    assert_eq!(decide_teammate_file(&in_folder, &r, named), RevealDecision::Ok { path: PathBuf::from(&in_folder), named: false });
    assert!(asked.borrow().is_empty(), "looked up {:?}", asked.borrow());
    // The control: a whole path outside every root is the one that is looked up.
    assert!(matches!(decide_teammate_file(REPORT, &r, named), RevealDecision::Ok { named: true, .. }));
    assert_eq!(*asked.borrow(), vec![PathBuf::from(REPORT)]);
}

// ---- a file handed over by a path from the folder its turn ran in (0.868) ----------------------------------------

const FOLDER: &str = "/work/now-coding";
const ELSEWHERE_FOLDER: &str = "/work/other";
const WORKTREE_REPORT: &str = "/work/locust-wt/spotify-artist/REPORT.md";

// case: copies it when the folder its turn ran in is one Locust knows
#[test]
fn copies_it_when_the_folder_its_turn_ran_in_is_one_locust_knows() {
    let handed = event(1, "message.delta", json!({"itemId": "msg_1", "operation": "append", "final": true,
        "text": "Committed on `exec/spotify-artist`.\n\n<locust-file>\n../locust-wt/spotify-artist/REPORT.md :: Implementation and verification limits\n</locust-file>"}));
    let ledger = Ledger::new(&locust_core::workspace_settings::workspace_id_for(Path::new(FOLDER)), &[handed]);
    // As the thread draws it: the conversation's folder, then the path as written.
    let requested = locust_core::protocol_tags::handed_file_on_disk(FOLDER, "../locust-wt/spotify-artist/REPORT.md");
    assert_eq!(
        decide_teammate_file(&requested, &roots(&[FOLDER]), |p| ledger.named(p, &[ELSEWHERE_FOLDER, FOLDER])),
        RevealDecision::Ok { path: PathBuf::from(WORKTREE_REPORT), named: true }
    );
}
