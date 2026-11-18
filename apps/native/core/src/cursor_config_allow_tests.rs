//! One Rust test per Electron case, with its exact values and sentences:
//! - `main/cursor-connector-allow.test.ts`
//! - `main/a-cursor-run-takes-back-the-rules-it-added.test.ts`
//! - `main/a-cursor-mission-gives-its-connector-rules-back-when-it-ends.test.ts`
//! - `main/a-cursor-turn-does-not-wait-for-its-connector-list.test.ts`

use super::*;
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;

const FILE: &str = "C:/work/.cursor/cli.json";
const DIR: &str = "C:/work/.cursor";

fn norm(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

struct MemIo {
    held: Mutex<HashMap<String, String>>,
    made: Mutex<BTreeSet<String>>,
    writes: Mutex<Vec<(String, String)>>,
    dirs: Mutex<Vec<String>>,
    fail_read: bool,
}

impl MemIo {
    fn new(files: &[(&str, &str)], dirs: &[&str]) -> Arc<Self> {
        let mut held = HashMap::new();
        for (k, v) in files {
            held.insert((*k).to_owned(), (*v).to_owned());
        }
        let mut made = BTreeSet::new();
        for d in dirs {
            made.insert((*d).to_owned());
        }
        Arc::new(Self {
            held: Mutex::new(held),
            made: Mutex::new(made),
            writes: Mutex::new(Vec::new()),
            dirs: Mutex::new(Vec::new()),
            fail_read: false,
        })
    }

    fn read_json(&self, path: &str) -> Option<Value> {
        let held = self.held.lock().unwrap();
        let text = held.get(path)?;
        serde_json::from_str(text).ok()
    }

    fn write_json(&self, path: &str, v: &Value) {
        let mut text = serde_json::to_string_pretty(v).unwrap();
        text.push('\n');
        self.held.lock().unwrap().insert(path.to_owned(), text);
    }
}

impl AllowIo for MemIo {
    fn read_file(&self, path: &Path) -> io::Result<String> {
        if self.fail_read {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "EACCES"));
        }
        self.held
            .lock()
            .unwrap()
            .get(&norm(path))
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "ENOENT"))
    }

    fn write_file(&self, path: &Path, text: &str) -> io::Result<()> {
        let p = norm(path);
        self.writes.lock().unwrap().push((p.clone(), text.to_owned()));
        self.held.lock().unwrap().insert(p, text.to_owned());
        Ok(())
    }

    fn mkdir(&self, path: &Path) -> io::Result<()> {
        let p = norm(path);
        self.dirs.lock().unwrap().push(p.clone());
        self.made.lock().unwrap().insert(p);
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        let p = norm(path);
        self.made.lock().unwrap().contains(&p) || self.held.lock().unwrap().contains_key(&p)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.held.lock().unwrap().remove(&norm(path));
        Ok(())
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        let prefix = format!("{}/", norm(path));
        if self.held.lock().unwrap().keys().any(|k| k.starts_with(&prefix)) {
            return Err(io::Error::other("ENOTEMPTY"));
        }
        self.made.lock().unwrap().remove(&norm(path));
        Ok(())
    }
}

struct MemHoldStore {
    text: Mutex<Option<String>>,
}
impl MemHoldStore {
    fn new(initial: Option<&str>) -> Arc<Self> {
        Arc::new(Self { text: Mutex::new(initial.map(str::to_owned)) })
    }
}
impl HoldStore for MemHoldStore {
    fn read(&self) -> Option<String> {
        self.text.lock().unwrap().clone()
    }
    fn write(&self, text: &str) -> io::Result<()> {
        *self.text.lock().unwrap() = Some(text.to_owned());
        Ok(())
    }
}

// --- cursor-connector-allow.test.ts ---

#[test]
fn allows_every_tool_of_that_server_and_nothing_else() {
    assert_eq!(cursor_allow_rule("robinhood-local"), "Mcp(robinhood-local:*)");
}

#[test]
fn creates_the_file_when_there_is_none() {
    let merged = merged_cursor_config(None, &["robinhood-local"]).unwrap();
    assert_eq!(merged.added, vec!["Mcp(robinhood-local:*)"]);
    let parsed: Value = serde_json::from_str(merged.text.as_deref().unwrap()).unwrap();
    assert_eq!(parsed, json!({"permissions": {"allow": ["Mcp(robinhood-local:*)"], "deny": []}}));
}

#[test]
fn keeps_everything_already_in_the_file_and_never_touches_deny() {
    let before = json!({
        "version": 1,
        "editor": {"vimMode": true},
        "permissions": {"allow": ["Shell(ls)", "Mcp(robinhood-trading:get_accounts)"], "deny": ["Shell(rm)"]}
    });
    let merged = merged_cursor_config(Some(&before.to_string()), &["robinhood-trading", "robinhood-local"]).unwrap();
    let after: Value = serde_json::from_str(merged.text.as_deref().unwrap()).unwrap();
    assert_eq!(after["version"], 1);
    assert_eq!(after["editor"], json!({"vimMode": true}));
    assert_eq!(after["permissions"]["deny"], json!(["Shell(rm)"]));
    assert_eq!(
        after["permissions"]["allow"],
        json!(["Shell(ls)", "Mcp(robinhood-trading:get_accounts)", "Mcp(robinhood-trading:*)", "Mcp(robinhood-local:*)"])
    );
}

#[test]
fn writes_nothing_when_every_rule_is_already_there() {
    let text = json!({"permissions": {"allow": ["Mcp(a:*)"], "deny": []}}).to_string();
    assert_eq!(merged_cursor_config(Some(&text), &["a"]), Some(MergedCursorConfig { text: None, added: Vec::new() }));
    let wildcard = json!({"permissions": {"allow": ["Mcp(*:*)"]}}).to_string();
    assert_eq!(merged_cursor_config(Some(&wildcard), &["a"]).unwrap().added, Vec::<String>::new());
}

#[test]
fn leaves_a_file_it_cannot_read_as_the_config_alone_rather_than_writing_over_it() {
    assert_eq!(merged_cursor_config(Some("{ not json"), &["a"]), None);
    assert_eq!(merged_cursor_config(Some("[]"), &["a"]), None);
    assert_eq!(merged_cursor_config(Some(&json!({"permissions": "yes"}).to_string()), &["a"]), None);
    assert_eq!(merged_cursor_config(Some(&json!({"permissions": {"allow": "all"}}).to_string()), &["a"]), None);
}

#[test]
fn adds_the_rules_under_the_workspace_and_says_which_were_added() {
    let fs = MemIo::new(&[], &[]);
    let grant = allow_cursor_connectors(Path::new("C:/work"), &["robinhood-local"], fs.clone());
    assert_eq!(grant.added, vec!["Mcp(robinhood-local:*)"]);
    let writes = fs.writes.lock().unwrap();
    let (path, text) = &writes[0];
    assert_eq!(path, FILE);
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["permissions"]["allow"], json!(["Mcp(robinhood-local:*)"]));
    assert_eq!(fs.dirs.lock().unwrap()[0], DIR);
}

#[test]
fn does_nothing_with_no_servers_and_does_not_write_when_nothing_is_new() {
    let fs = MemIo::new(&[], &[]);
    assert_eq!(allow_cursor_connectors(Path::new("C:/work"), &[], fs.clone()).added, Vec::<String>::new());
    assert!(fs.writes.lock().unwrap().is_empty());
}

#[test]
fn does_not_write_over_a_file_it_could_not_read_and_does_not_throw() {
    let fs = MemIo::new(&[(FILE, "{ broken")], &[]);
    assert_eq!(allow_cursor_connectors(Path::new("C:/work"), &["a"], fs.clone()).added, Vec::<String>::new());
    assert!(fs.writes.lock().unwrap().is_empty());
    let failing = Arc::new(MemIo {
        held: Mutex::new(HashMap::new()),
        made: Mutex::new(BTreeSet::new()),
        writes: Mutex::new(Vec::new()),
        dirs: Mutex::new(Vec::new()),
        fail_read: true,
    });
    assert_eq!(allow_cursor_connectors(Path::new("C:/work"), &["a"], failing).added, Vec::<String>::new());
}

// --- a-cursor-run-takes-back-the-rules-it-added.test.ts ---

#[test]
fn leaves_the_rules_that_were_already_in_the_file_and_its_own_other_settings() {
    let before = json!({
        "version": 1,
        "permissions": {"allow": ["Shell(ls)", "Mcp(robinhood-trading:get_accounts)", "Mcp(a:*)"], "deny": ["Shell(rm)"]}
    });
    let mut text = serde_json::to_string_pretty(&before).unwrap();
    text.push('\n');
    let fs = MemIo::new(&[(FILE, &text)], &[DIR]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["a", "robinhood-trading"]);
    assert_eq!(grant.added, vec!["Mcp(robinhood-trading:*)"]);
    assert!(fs.read_json(FILE).unwrap()["permissions"]["allow"]
        .as_array()
        .unwrap()
        .contains(&json!("Mcp(robinhood-trading:*)")));
    grant.release();
    assert_eq!(fs.read_json(FILE).unwrap(), before);
}

#[test]
fn leaves_a_rule_the_person_added_while_the_run_was_going() {
    let fs = MemIo::new(&[], &[]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["a"]);
    let mut during = fs.read_json(FILE).unwrap();
    let mut allow = during["permissions"]["allow"].as_array().unwrap().clone();
    allow.push(json!("Shell(git status)"));
    during["permissions"] = json!({"allow": allow, "deny": ["Shell(rm)"]});
    fs.write_json(FILE, &during);
    grant.release();
    assert_eq!(fs.read_json(FILE).unwrap(), json!({"permissions": {"allow": ["Shell(git status)"], "deny": ["Shell(rm)"]}}));
}

#[test]
fn never_touches_deny() {
    let init = json!({"permissions": {"allow": [], "deny": ["Mcp(a:*)"]}}).to_string();
    let fs = MemIo::new(&[(FILE, &init)], &[DIR]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["b"]);
    grant.release();
    assert_eq!(fs.read_json(FILE).unwrap()["permissions"]["deny"], json!(["Mcp(a:*)"]));
}

#[test]
fn keeps_a_rule_until_the_last_of_two_overlapping_runs_ends() {
    let fs = MemIo::new(&[], &[]);
    let both = CursorConnectorKeeper::new(fs.clone(), None);
    let first = both.allow(Path::new("C:/work"), &["a"]);
    let second = both.allow(Path::new("C:/work"), &["a", "b"]);
    assert_eq!(second.added, vec!["Mcp(b:*)"]);
    first.release();
    assert_eq!(fs.read_json(FILE).unwrap()["permissions"]["allow"], json!(["Mcp(a:*)", "Mcp(b:*)"]));
    second.release();
    assert!(fs.read_json(FILE).is_none());
}

#[test]
fn says_whether_a_live_run_holds_rules_in_a_folder_until_the_last_one_ends() {
    let fs = MemIo::new(&[], &[]);
    let both = CursorConnectorKeeper::new(fs.clone(), None);
    assert!(!both.holds(Path::new("C:/work")));
    let first = both.allow(Path::new("C:/work"), &["a"]);
    let second = both.allow(Path::new("C:/work"), &["a"]);
    assert!(both.holds(Path::new("C:/work")));
    assert!(!both.holds(Path::new("C:/elsewhere")));
    first.release();
    assert!(both.holds(Path::new("C:/work")));
    second.release();
    assert!(!both.holds(Path::new("C:/work")));
    fs.write_json(FILE, &json!({"permissions": {"allow": ["Mcp(a:*)"]}}));
    let theirs = both.allow(Path::new("C:/work"), &["a"]);
    assert!(theirs.added.is_empty());
    assert!(!both.holds(Path::new("C:/work")));
}

#[test]
fn keeps_the_rule_when_the_run_that_added_it_ends_first_and_the_other_is_still_going() {
    let fs = MemIo::new(&[], &[]);
    let both = CursorConnectorKeeper::new(fs.clone(), None);
    let first = both.allow(Path::new("C:/work"), &["a"]);
    let second = both.allow(Path::new("C:/work"), &["a"]);
    first.release();
    assert_eq!(fs.read_json(FILE).unwrap()["permissions"]["allow"], json!(["Mcp(a:*)"]));
    second.release();
    assert!(fs.read_json(FILE).is_none());
}

#[test]
fn leaves_a_file_that_has_stopped_parsing_exactly_as_it_is() {
    let fs = MemIo::new(&[], &[]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["a"]);
    fs.held.lock().unwrap().insert(FILE.to_owned(), "{ the person is half way through typing".into());
    grant.release();
    assert_eq!(fs.held.lock().unwrap().get(FILE).map(String::as_str), Some("{ the person is half way through typing"));
}

#[test]
fn leaves_no_file_and_no_folder_when_the_run_created_them() {
    let fs = MemIo::new(&[], &[]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["a", "b"]);
    assert!(fs.read_json(FILE).is_some());
    assert!(fs.made.lock().unwrap().contains(DIR));
    grant.release();
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
    assert!(!fs.made.lock().unwrap().contains(DIR));
}

#[test]
fn keeps_the_folder_but_not_the_file_when_the_folder_was_already_there() {
    let fs = MemIo::new(&[], &[DIR]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["a"]);
    grant.release();
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
    assert!(fs.made.lock().unwrap().contains(DIR));
}

#[test]
fn keeps_a_file_the_run_created_if_the_person_put_something_else_in_it_meanwhile() {
    let fs = MemIo::new(&[], &[]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["a"]);
    fs.write_json(FILE, &json!({"editor": {"vimMode": true}, "permissions": {"allow": ["Mcp(a:*)"], "deny": []}}));
    grant.release();
    assert_eq!(fs.read_json(FILE).unwrap(), json!({"editor": {"vimMode": true}}));
}

#[test]
fn does_not_take_back_a_connector_rule_the_person_had_already_allowed_though_the_run_asked_for_it_too() {
    let init = json!({"permissions": {"allow": ["Mcp(a:*)"], "deny": []}}).to_string();
    let fs = MemIo::new(&[(FILE, &init)], &[DIR]);
    let k = CursorConnectorKeeper::new(fs.clone(), None);
    let grant = k.allow(Path::new("C:/work"), &["a", "b"]);
    assert_eq!(grant.added, vec!["Mcp(b:*)"]);
    grant.release();
    assert_eq!(fs.read_json(FILE).unwrap()["permissions"]["allow"], json!(["Mcp(a:*)"]));
}

#[test]
fn does_nothing_twice_when_the_run_is_released_twice() {
    let fs = MemIo::new(&[], &[]);
    let both = CursorConnectorKeeper::new(fs.clone(), None);
    let first = both.allow(Path::new("C:/work"), &["a"]);
    let second = both.allow(Path::new("C:/work"), &["a"]);
    first.release();
    first.release();
    assert!(fs.read_json(FILE).is_some());
    second.release();
    assert!(fs.read_json(FILE).is_none());
}

#[test]
fn does_not_mix_up_two_folders() {
    let fs = MemIo::new(&[], &[]);
    let both = CursorConnectorKeeper::new(fs.clone(), None);
    let here = both.allow(Path::new("C:/work"), &["a"]);
    let there = both.allow(Path::new("C:/other"), &["a"]);
    here.release();
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
    assert!(fs.held.lock().unwrap().contains_key("C:/other/.cursor/cli.json"));
    there.release();
    assert!(!fs.held.lock().unwrap().contains_key("C:/other/.cursor/cli.json"));
}

#[test]
fn no_connector_rule_after_an_accept_edits_run_in_the_same_folder_has_ended() {
    let init = json!({"permissions": {"allow": ["Shell(ls)"], "deny": []}}).to_string();
    let fs = MemIo::new(&[(FILE, &init)], &[DIR]);
    let grant = CursorConnectorKeeper::new(fs.clone(), None).allow(Path::new("C:/work"), &["robinhood-trading", "robinhood-local"]);
    assert!(fs.held.lock().unwrap().get(FILE).unwrap().contains("Mcp("));
    grant.release();
    assert!(!fs.held.lock().unwrap().get(FILE).unwrap().contains("Mcp("));
}

#[test]
fn recovers_connector_rules_left_behind_when_an_earlier_locust_quit() {
    let fs = MemIo::new(&[], &[]);
    let store = MemHoldStore::new(None);
    // Earlier Locust process acquired a grant and exited without calling release().
    let earlier = CursorConnectorKeeper::new(fs.clone(), Some(store.clone()));
    let grant = earlier.allow(Path::new("C:/work"), &["robinhood-local"]);
    assert_eq!(grant.added, vec!["Mcp(robinhood-local:*)"]);
    std::mem::forget(grant);
    // Next Locust start recovers and removes the file and directory it created.
    let next = CursorConnectorKeeper::new(fs.clone(), Some(store));
    let changed = next.recover();
    assert_eq!(changed, vec![PathBuf::from(FILE)]);
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
    assert!(!fs.made.lock().unwrap().contains(DIR));
}

// --- a-cursor-mission-gives-its-connector-rules-back-when-it-ends.test.ts ---

fn run_mission_harness(
    mode: crate::approval::Mode,
    outcome: Result<(), &'static str>,
    events: &mut Vec<&'static str>,
    fs: Arc<MemIo>,
) -> Result<(), &'static str> {
    let keeper = CursorConnectorKeeper::new(fs, None);
    let grant = if crate::runtime::cursor::sandbox_for(mode) == "workspace-write" {
        events.push("rules-added");
        Some(keeper.allow(Path::new("C:/work"), &["robinhood-local"]))
    } else {
        None
    };
    events.push("process-started");
    let res = outcome;
    if let Some(g) = grant {
        g.release();
        events.push("rules-taken-back");
    }
    res
}

#[test]
fn gives_them_back_when_its_process_completes() {
    let fs = MemIo::new(&[], &[]);
    let mut events = Vec::new();
    run_mission_harness(crate::approval::Mode::AcceptEdits, Ok(()), &mut events, fs.clone()).unwrap();
    assert_eq!(events, vec!["rules-added", "process-started", "rules-taken-back"]);
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
}

#[test]
fn gives_them_back_when_it_is_cancelled() {
    let fs = MemIo::new(&[], &[]);
    let mut events = Vec::new();
    let _ = run_mission_harness(crate::approval::Mode::AcceptEdits, Err("cancelled"), &mut events, fs.clone());
    assert_eq!(events.last().copied(), Some("rules-taken-back"));
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
}

#[test]
fn gives_them_back_when_its_process_fails() {
    let fs = MemIo::new(&[], &[]);
    let mut events = Vec::new();
    let _ = run_mission_harness(crate::approval::Mode::AcceptEdits, Err("the process died"), &mut events, fs.clone());
    assert_eq!(events.last().copied(), Some("rules-taken-back"));
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
}

#[test]
fn gives_them_back_when_the_process_cannot_be_started_at_all() {
    let fs = MemIo::new(&[], &[]);
    let mut events = Vec::new();
    let res = run_mission_harness(crate::approval::Mode::AcceptEdits, Err("spawn failed"), &mut events, fs.clone());
    assert!(res.is_err());
    assert_eq!(events, vec!["rules-added", "process-started", "rules-taken-back"]);
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
}

#[test]
fn is_given_none_and_so_has_none_to_give_back_when_it_only_reads() {
    let fs = MemIo::new(&[], &[]);
    let mut events = Vec::new();
    run_mission_harness(crate::approval::Mode::Ask, Ok(()), &mut events, fs.clone()).unwrap();
    assert_eq!(events, vec!["process-started"]);
    assert!(!fs.held.lock().unwrap().contains_key(FILE));
}

// --- a-cursor-turn-does-not-wait-for-its-connector-list.test.ts ---

struct SlowLister {
    calls: AtomicU64,
    gates: Mutex<Vec<mpsc::Sender<String>>>,
}
impl SlowLister {
    fn new() -> Arc<Self> {
        Arc::new(Self { calls: AtomicU64::new(0), gates: Mutex::new(Vec::new()) })
    }
    fn lister(self: &Arc<Self>) -> McpLister {
        let me = Arc::clone(self);
        Arc::new(move || {
            me.calls.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel();
            me.gates.lock().unwrap().push(tx);
            rx.recv().ok()
        })
    }
    fn answer(&self, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if !self.gates.lock().unwrap().is_empty() {
                let tx = self.gates.lock().unwrap().remove(0);
                let _ = tx.send(format!("{name}: ready\n"));
                return;
            }
            if Instant::now() >= deadline {
                panic!("slow_lister had no pending call to answer");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

#[test]
fn answers_from_an_old_reading_at_once_and_reads_a_new_one_behind_it() {
    let slow = SlowLister::new();
    let clock = Arc::new(AtomicU64::new(0));
    let clock_fn: Clock = {
        let c = Arc::clone(&clock);
        Arc::new(move || c.load(Ordering::SeqCst))
    };
    let cache = Arc::new(CursorConnectorCache::new(slow.lister(), clock_fn));
    let c1 = Arc::clone(&cache);
    let h1 = std::thread::spawn(move || c1.ready_connectors());
    slow.answer("notion");
    assert!(h1.join().unwrap().unwrap().contains("notion"));

    // Ten minutes later the reading is old: the turn still gets an answer without waiting.
    clock.store(10 * 60_000, Ordering::SeqCst);
    let second = cache.ready_connectors().unwrap();
    assert!(second.contains("notion"));
    // Wait briefly for background refresh to register its call, then answer it.
    slow.answer("notion-and-linear");
    // Wait for background refresh to store the new value.
    let deadline = Instant::now() + Duration::from_secs(2);
    while !cache.ready_connectors().unwrap().contains("notion-and-linear") {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(slow.calls(), 2);
}

#[test]
fn reads_once_however_many_turns_ask_while_it_is_reading() {
    let slow = SlowLister::new();
    let cache = Arc::new(CursorConnectorCache::new(slow.lister(), Arc::new(|| 0)));
    let c1 = Arc::clone(&cache);
    let c2 = Arc::clone(&cache);
    let h1 = std::thread::spawn(move || c1.ready_connectors());
    let h2 = std::thread::spawn(move || c2.ready_connectors());
    slow.answer("notion");
    assert!(h1.join().unwrap().unwrap().contains("notion"));
    assert!(h2.join().unwrap().unwrap().contains("notion"));
    assert_eq!(slow.calls(), 1);
}
#[test]
fn is_read_when_cursor_is_found_so_the_first_turn_finds_it_read() {
    let slow = SlowLister::new();
    let cache = Arc::new(CursorConnectorCache::new(slow.lister(), Arc::new(|| 1)));
    cache.warm();
    cache.warm();
    slow.answer("notion");
    // Wait for warm() thread to finish storing.
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(line) = cache.ready_connectors()
            && line.contains("notion")
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(slow.calls(), 1);
}

// --- a-cursor-teammate-gets-its-skills.test.ts ---

#[test]
fn gives_cursor_a_disposable_plugin_manifest_pointing_at_the_copied_skills() {
    let root = std::env::temp_dir().join(format!("cursor-skills-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let cand = crate::tune::TuneSkill {
        name: "canary".into(),
        description: "A canary.".into(),
        body: "QUARTZFINCH".into(),
    };
    let sources = crate::skills::Sources { candidate: Some(cand), ..Default::default() };
    let prepared = crate::skills::prepare_run(&root, &sources).unwrap();
    let manifest_path = prepared.dir.join(".cursor-plugin").join("plugin.json");
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["name"], "locust-run-skills");
    assert_eq!(manifest["skills"], "skills");
    let skill_md = std::fs::read_to_string(prepared.dir.join(manifest["skills"].as_str().unwrap()).join("canary").join("SKILL.md")).unwrap();
    assert!(skill_md.contains("QUARTZFINCH"));
    let dir_str = prepared.dir.to_string_lossy().into_owned();
    for mode in [crate::approval::Mode::Ask, crate::approval::Mode::AcceptEdits, crate::approval::Mode::Auto] {
        let args = crate::runtime::cursor::print_args(&root, mode, None, None, Some(&dir_str)).unwrap();
        let at = args.iter().position(|a| a == "--plugin-dir").unwrap();
        assert_eq!(args[at + 1], dir_str);
        assert!(!args.iter().any(|a| a == "--add-dir"));
        let without = crate::runtime::cursor::print_args(&root, mode, None, None, None).unwrap();
        assert!(!without.iter().any(|a| a == "--plugin-dir"));
    }
    assert!(crate::skill_runtimes::takes_skills(Some("cursor")));
    assert!(crate::skill_runtimes::SKILL_RUNTIME_NAMES.contains("Cursor"));
    let _ = std::fs::remove_dir_all(&prepared.dir);
    assert!(!manifest_path.exists());
    let _ = std::fs::remove_dir_all(&root);
}

// --- a-cursor-level-becomes-its-id-at-the-start.test.ts ---

fn argon_models() -> Vec<Value> {
    vec![
        json!({
            "id": "gemini-4-argon-medium",
            "displayName": "Gemini 4 Argon",
            "runtime": "cursor",
            "variants": {"low": "gemini-4-argon-low", "medium": "gemini-4-argon-medium", "high": "gemini-4-argon-high"}
        }),
        json!({"id": "composer-2.5", "displayName": "Composer 2.5", "runtime": "cursor"}),
    ]
}

#[test]
fn argon_low_at_high_starts_as_gemini_4_argon_high_with_no_level_beside_it() {
    assert_eq!(
        crate::models::cursor_choice_for(&argon_models(), Some("gemini-4-argon-low"), Some("high")),
        (Some("gemini-4-argon-high".into()), None)
    );
}

#[test]
fn an_id_already_on_its_level_is_left_as_it_is() {
    assert_eq!(
        crate::models::cursor_choice_for(&argon_models(), Some("gemini-4-argon-high"), None),
        (Some("gemini-4-argon-high".into()), None)
    );
}

#[test]
fn a_level_the_family_does_not_list_or_a_model_not_listed_is_left_as_it_came_for_the_builder_to_refuse() {
    assert_eq!(
        crate::models::cursor_choice_for(&argon_models(), Some("gemini-4-argon-low"), Some("xhigh")),
        (Some("gemini-4-argon-low".into()), Some("xhigh".into()))
    );
    assert_eq!(
        crate::models::cursor_choice_for(&[], Some("gemini-4-argon-low"), Some("high")),
        (Some("gemini-4-argon-low".into()), Some("high".into()))
    );
}

#[test]
fn no_model_the_account_default_with_nothing_to_turn() {
    assert_eq!(
        crate::models::cursor_choice_for(&argon_models(), None, None),
        (None, None)
    );
}
