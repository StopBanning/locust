//! Electron `main/a-kilo-follow-up-finds-its-session.test.ts` (0.845): every turn of a conversation runs in its first
//! turn's Kilo folder and loads its session; a conversation whose folder is not there starts a new one, told what was
//! said. Driven through `native::send` with the scripted Kilo (`m4-kilo-acp.py sessions`), which keeps its session in its
//! data folder, fails a `session/load` that folder never held as Kilo does, and logs what it was sent there.
use super::*;
use crate::engine::Teammate;
use crate::native::{self, hooks::Guard, Send};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Kilo {
    root: PathBuf,
    store: Store,
    teammate: Teammate,
    guard: Option<Guard>,
}
impl Drop for Kilo {
    fn drop(&mut self) {
        self.guard = None;
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn command() -> Option<(PathBuf, Vec<String>)> {
    let fake = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).join("scripts/fixtures/m4-kilo-acp.py");
    Some(("python3".into(), vec![fake.to_string_lossy().into_owned(), "acp".into(), "sessions".into()]))
}
impl Kilo {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("locust-kilo-follow-up-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let store = Store::new(root.join("native"), root.join("electron")).unwrap();
        std::fs::create_dir_all(store.electron()).unwrap();
        std::fs::create_dir_all(root.join("work")).unwrap();
        let guard = Some(native::hooks::install(store.clone(), command()));
        let teammate = native::parse_roster(
            &json!({"schemaVersion":1,"teammates":[{"teammateId":"k1","name":"Kilo","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-10-31T22:00:00Z","folder":root.join("work"),"route":{"runtime":"kilo","model":"kilo/kilo-auto/free","mode":"ask"}}]}).to_string(),
        )
        .unwrap()
        .remove(0);
        Self { root, store, teammate, guard }
    }
    fn turns(&self) -> Vec<String> {
        let Ok(dir) = std::fs::read_dir(self.store.ledger_dir().unwrap()) else { return Vec::new() };
        dir.flatten().filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned())).collect()
    }
    /// One turn, a follow-up of `after` when given, waited for to the end. Its own id: `send` answers with the
    /// conversation's.
    fn start(&self, after: Option<&str>) -> String {
        let before = self.turns();
        let started = native::send(Send {
            teammate: self.teammate.clone(),
            conversation: after.map(str::to_owned),
            prompt: "What should we build next?".into(),
            model: None,
            effort: None,
        })
        .unwrap();
        crate::load_waits::wait_for("the Kilo turn to finish", || !native::owns(&started.conversation_id));
        let mut new: Vec<String> = self.turns().into_iter().filter(|t| !before.contains(t)).collect();
        assert_eq!(new.len(), 1, "one turn was written: {new:?}");
        new.remove(0)
    }
    /// Locust closed and opened again: the same record and folders, nothing remembered in between.
    fn restart(&mut self) {
        self.guard = None;
        self.guard = Some(native::hooks::install(self.store.clone(), command()));
    }
    fn home(&self, conversation: &str) -> PathBuf {
        let home = self.store.root().join(crate::profile_backup::KILO_CONVERSATIONS_FOLDER).join(conversation);
        std::fs::canonicalize(&home).unwrap_or(home)
    }
    /// What each Kilo process that ran in `conversation`'s folder was sent, one list per process, oldest first.
    fn runs_in(&self, conversation: &str) -> Vec<Vec<Value>> {
        let log = std::fs::read_to_string(self.home(conversation).join("kilo-sent.jsonl")).unwrap_or_default();
        let mut runs: Vec<(u64, Vec<Value>)> = Vec::new();
        for line in log.lines() {
            let sent: Value = serde_json::from_str(line).unwrap();
            let pid = sent["pid"].as_u64().unwrap();
            match runs.iter_mut().find(|(p, _)| *p == pid) {
                Some((_, run)) => run.push(sent),
                None => runs.push((pid, vec![sent])),
            }
        }
        runs.into_iter().map(|(_, run)| run).collect()
    }
    fn header(&self, mission: &str) -> Value {
        let text = std::fs::read_to_string(self.store.ledger_dir().unwrap().join(format!("{mission}.jsonl"))).unwrap();
        let head: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        head["metadata"].clone()
    }
}
fn sent<'a>(run: &'a [Value], method: &str) -> Option<&'a Value> {
    run.iter().find(|m| m["method"] == method)
}

// case: runs every turn of a conversation in its first turn's Kilo folder, and the follow-ups load its session
#[test]
fn runs_every_turn_of_a_conversation_in_its_first_turns_kilo_folder_and_the_follow_ups_load_its_session() {
    let k = Kilo::new();
    let first = k.start(None);
    let second = k.start(Some(&first));
    let third = k.start(Some(&second));
    let runs = k.runs_in(&first);
    assert_eq!(runs.len(), 3, "every turn ran in the first turn's folder: {runs:?}");
    assert!(k.runs_in(&second).is_empty() && k.runs_in(&third).is_empty());
    // Kilo's own config and data are the conversation's folder, never the person's.
    let home = k.home(&first).to_string_lossy().into_owned();
    for run in &runs {
        assert_eq!(run[0]["env"], json!({"KILO_CONFIG_DIR": home, "XDG_CONFIG_HOME": home, "XDG_DATA_HOME": home}), "{run:?}");
    }
    assert!(sent(&runs[0], "session/new").is_some());
    assert!(sent(&runs[1], "session/new").is_none(), "{:?}", runs[1]);
    let load = sent(&runs[1], "session/load").expect("the follow-up loads its session");
    assert_eq!(load["params"]["sessionId"], "s1");
    assert_eq!(load["params"]["cwd"], json!(std::fs::canonicalize(k.root.join("work")).unwrap()));
    assert_eq!(sent(&runs[2], "session/load").map(|m| &m["params"]["sessionId"]), Some(&json!("s1")));
    assert_eq!(k.header(&second)["continuesFrom"]["missionId"], json!(first));
    assert_eq!(k.header(&second)["continuesFrom"]["runtimeThreadId"], "s1");
}

// case: finds the first turn after Locust was reopened, from the record
#[test]
fn finds_the_first_turn_after_locust_was_reopened_from_the_record() {
    let mut k = Kilo::new();
    let first = k.start(None);
    let second = k.start(Some(&first));
    let third = k.start(Some(&second));
    k.restart();
    // three turns back, through the record alone
    let fourth = k.start(Some(&third));
    let runs = k.runs_in(&first);
    assert_eq!(runs.len(), 4, "{runs:?}");
    assert_eq!(sent(&runs[3], "session/load").map(|m| &m["params"]["sessionId"]), Some(&json!("s1")));
    assert!(k.runs_in(&fourth).is_empty());
}

// case: gives two conversations two folders
#[test]
fn gives_two_conversations_two_folders() {
    let k = Kilo::new();
    let one = k.start(None);
    let two = k.start(None);
    assert_ne!(one, two);
    assert_eq!(k.runs_in(&one).len(), 1);
    assert_eq!(k.runs_in(&two).len(), 1);
    assert_ne!(k.home(&one), k.home(&two));
}

// case: starts a conversation whose folder is gone in a new session, told what was said before
#[test]
fn starts_a_conversation_whose_folder_is_gone_in_a_new_session_told_what_was_said_before() {
    let k = Kilo::new();
    let first = k.start(None);
    // as on a restored profile, or a conversation begun before 0.845: its Kilo never held this session
    std::fs::remove_dir_all(k.home(&first)).unwrap();
    let second = k.start(Some(&first));
    let runs = k.runs_in(&first);
    assert_eq!(runs.len(), 1, "the follow-up ran in the first turn's folder, made again: {runs:?}");
    assert!(sent(&runs[0], "session/load").is_none(), "{:?}", runs[0]);
    assert!(sent(&runs[0], "session/new").is_some());
    let prompt = sent(&runs[0], "session/prompt").expect("a prompt").to_string();
    assert!(prompt.contains("Kilo answered."), "{prompt}");
    let ledger = std::fs::read_to_string(k.store.ledger_dir().unwrap().join(format!("{second}.jsonl"))).unwrap();
    assert!(ledger.contains("run.completed"), "{ledger}");
    // the record says this followed that, without claiming a resume that did not happen
    let continued = &k.header(&second)["continuesFrom"];
    assert_eq!(continued["missionId"], json!(first));
    assert!(continued.get("runtimeThreadId").is_none(), "{continued}");
}
