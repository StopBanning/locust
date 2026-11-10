//! Synthetic data and the fake app-server only: no Codex, no credentials, no network, no window.
use super::*;
use crate::ledger::Item;
use std::{
    fs,
    time::{Duration, Instant},
};

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-m2-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(p.join("electron/mission-ledger")).unwrap();
        fs::create_dir_all(p.join("work")).unwrap();
        Self(p)
    }
    fn electron(&self) -> PathBuf {
        self.0.join("electron")
    }
    fn store(&self) -> Store {
        Store::new(self.0.join("native"), self.electron()).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fake(scenario: &str) -> Option<(PathBuf, Vec<String>)> {
    Some((
        PathBuf::from("python3"),
        vec![concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/fixtures/m1-app-server.py").into(), scenario.into()],
    ))
}

fn codex(t: &Temp) -> Teammate {
    Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        busy: false,
        runs_in: "ask".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    }
}

fn job(t: &Temp, conversation: Option<&str>, prompt: &str) -> Send {
    Send {
        teammate: codex(t),
        conversation: conversation.map(str::to_owned),
        prompt: prompt.into(),
        model: Some("synthetic-model".into()),
        effort: None,
    }
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn finished(id: &str) {
    wait("the turn to release its slot", || !owns(id));
}
fn records(path: &Path) -> Vec<Value> {
    fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}
fn kinds(path: &Path) -> Vec<String> {
    records(path).iter().filter_map(|r| r["event"]["type"].as_str().map(str::to_owned)).collect()
}
fn ledgers(t: &Temp) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(t.store().ledger_dir().unwrap())
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}
fn path_of(t: &Temp, id: &str) -> PathBuf {
    t.store().write_path(id).unwrap()
}
/// Names, sizes, modification times and bytes of everything under a folder.
fn fingerprint(dir: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64, std::time::SystemTime, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            let m = e.metadata().unwrap();
            if m.is_dir() {
                walk(&e.path(), out);
            } else {
                out.push((e.path(), m.len(), m.modified().unwrap(), fs::read(e.path()).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

// ---- the roster -------------------------------------------------------------------------------------------------
fn teammate_json(id: &str, name: &str, role: &str, route: Value) -> Value {
    let mut t = json!({"teammateId":id,"name":name,"hue":"blue","role":role,"createdAt":"2026-11-01T00:00:00.000Z","avatar":{}});
    if !route.is_null() {
        t["route"] = route;
    }
    t
}
fn roster_file(teammates: Vec<Value>) -> String {
    json!({"schemaVersion":1,"teammates":teammates,"missionOwners":{},"missionTitles":{},"missionPins":[],"missionSettled":{},"settings":{}}).to_string()
}

#[test]
fn roster_reads_electrons_shape_and_marks_what_native_cannot_run() {
    let text = roster_file(vec![
        teammate_json("tm_1", "Codec", "Code & Migrations", json!({"runtime":"codex","model":"gpt-6.1-sol","mode":"accept-edits"})),
        teammate_json("tm_2", "Argon", "Code & Migrations", json!({"runtime":"antigravity","model":"Gemini 3.8 Flash","mode":"auto"})),
        teammate_json("tm_3", "Wren", "Custom", json!({"runtime":"codex","model":"account-default","mode":"ask","account":"account_0123456789abcdef0123456789abcdef"})),
        teammate_json("tm_4", "Nova", "Custom", Value::Null),
    ]);
    let team = parse_roster(&text).unwrap();
    assert_eq!(team.len(), 4);
    assert_eq!((team[0].runtime.as_deref(), team[0].model.as_deref(), team[0].runs_in.as_str()), (Some("codex"), Some("gpt-6.1-sol"), "accept-edits"));
    assert!(supported(&team[0]));
    assert_eq!(team[1].runs_in, IN_LOCUST);
    assert!(!supported(&team[1]), "another agent runs in Locust for now");
    assert!(!supported(&team[2]), "a Codex account of its own needs its own login home: not touched");
    assert!(!supported(&team[3]), "no route yet: nothing to run natively");
    assert_eq!(team[3].role, "Custom");
}

#[test]
fn roster_treats_the_file_as_untrusted_like_teammate_store() {
    let mut bad_role = teammate_json("tm_x", "Bad", "Wizard", Value::Null);
    bad_role["role"] = json!("Wizard");
    let mut long = teammate_json("tm_long", &"n".repeat(41), "Custom", Value::Null);
    long["name"] = json!("n".repeat(41));
    let text = roster_file(vec![
        teammate_json("tm_ok", " Padded ", "Docs & QA", Value::Null),
        bad_role,
        long,
        teammate_json("../escape", "Traversal", "Custom", Value::Null),
        teammate_json("tm_ok", "Duplicate", "Custom", Value::Null),
        teammate_json("tm_route", "Route", "Custom", json!({"runtime":"codex","model":"","mode":"ask"})),
        json!({"teammateId":"tm_nodate","name":"NoDate","hue":"blue","role":"Custom"}),
        json!("not an object"),
    ]);
    let team = parse_roster(&text).unwrap();
    let names: Vec<_> = team.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Padded", "Route"], "bad records are dropped, the rest stays usable");
    assert!(team[1].runtime.is_none(), "a malformed route reads as no route");
    // The file itself: not JSON, or another schema, is unreadable, never "empty".
    assert!(parse_roster("{").is_err());
    assert!(parse_roster(&json!({"schemaVersion":2,"teammates":[]}).to_string()).is_err());
    assert!(parse_roster("[]").is_err());
}

#[test]
fn roster_is_read_only_and_a_missing_file_is_an_empty_team() {
    let t = Temp::new();
    assert_eq!(roster(&t.electron()).unwrap().len(), 0);
    let text = roster_file(vec![teammate_json("tm_1", "Codec", "Custom", json!({"runtime":"codex","model":"m","mode":"ask"}))]);
    let file = t.electron().join("teammates.json");
    fs::write(&file, &text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // A read-only file proves nothing was opened for writing: any write attempt would fail loudly.
        fs::set_permissions(&file, fs::Permissions::from_mode(0o400)).unwrap();
    }
    let before = fingerprint(&t.electron());
    assert_eq!(roster(&t.electron()).unwrap().len(), 1);
    assert_eq!(fingerprint(&t.electron()), before);
    // A file past the size cliff is unreadable, never empty (so no caller could mistake it for a roster).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fs::write(&file, vec![b' '; 1_000_100]).unwrap();
    assert!(roster(&t.electron()).is_err());
}

// ---- the engine choice -----------------------------------------------------------------------------------------
fn team() -> Vec<Teammate> {
    vec![Teammate { id: "a".into(), name: "A".into(), role: String::new(), runtime: None, model: None, busy: false, runs_in: "ask".into(), folder: None }]
}
#[test]
fn engine_choice_forced_door_down_door_up_and_door_refusal() {
    let dies = || -> Result<Vec<Teammate>, String> { panic!("must not be asked") };
    // LOCUST_NATIVE_ENGINE=1: native, and the door is never asked.
    assert_eq!(choose(true, dies, || Ok(team())).unwrap().0, Kind::Native);
    // The door answers: door, and native is never asked.
    assert_eq!(choose(false, || Ok(team()), dies).unwrap().0, Kind::Door);
    // Unreachable: native, for each of the three "cannot reach it" wordings.
    for words in [engine::DOWN, engine::NOT_ON, "Locust's data folder could not be found on this computer."] {
        assert_eq!(choose(false, || Err(words.to_string()), || Ok(team())).unwrap().0, Kind::Native, "{words}");
    }
    // Reachable but refused (wrong token, an error status): that is not "unreachable", so no silent switch.
    for words in ["Locust refused the request (500).", "Locust refused the request: its connection has changed. Try again; if it keeps happening, restart Locust."] {
        assert_eq!(choose(false, || Err(words.to_string()), dies).unwrap_err(), words);
    }
    // Door down and nothing for native to read either: the door's words are what the person sees.
    assert_eq!(choose(false, || Err(engine::DOWN.into()), || Err("native words".into())).unwrap_err(), engine::DOWN);
    assert_eq!(Kind::Native.label(), "Engine: native");
    assert_eq!(Kind::Door.label(), "Engine: Locust door");
}

// ---- continue a conversation ----------------------------------------------------------------------------------
fn assert_no_reasoning(path: &Path) {
    assert!(!fs::read_to_string(path).unwrap().contains("RAW_REASONING_MUST_NOT_SURVIVE"));
}

#[test]
fn start_then_continue_resumes_the_stored_thread_and_chains_the_ledger() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let first = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(first.conversation_id.starts_with("mission_native_"));
    assert!(owns(&first.conversation_id), "this window started it");
    finished(&first.conversation_id);
    let one = path_of(&t, &first.conversation_id);
    assert_eq!(kinds(&one).last().map(String::as_str), Some("run.completed"));
    assert_no_reasoning(&one);

    // Continue: thread/resume with the stored runtimeThreadId (the fake fails the turn on thread/start or a wrong id).
    *hooks::STATE.lock().unwrap() = (Some(t.store()), fake("resume"));
    let second = send(job(&t, Some(&first.conversation_id), "Synthetic follow-up")).unwrap();
    assert_eq!(second.conversation_id, first.conversation_id, "the conversation keeps its id, as through the door");
    finished(&second.conversation_id);
    let files = ledgers(&t);
    assert_eq!(files.len(), 2);
    let child = files.iter().find(|p| **p != one).unwrap();
    let recs = records(child);
    assert_eq!(kinds(child).last().map(String::as_str), Some("run.completed"), "{:?}", kinds(child));
    assert_no_reasoning(child);
    let link = &recs[0]["metadata"]["continuesFrom"];
    assert_eq!(link["missionId"], first.conversation_id.as_str());
    assert_eq!((link["checkpointEpoch"].clone(), link["reason"].clone()), (json!(1), json!("follow-up")));
    assert_eq!(link["runtimeThreadId"], "thread_fake");

    // The reader the window uses follows the chain and shows both prompts and both answers.
    let mut s = session(t.electron().join("mission-ledger"), first.conversation_id.clone());
    let snap = s.poll().unwrap();
    assert_eq!(snap.tip, recs[0]["metadata"]["missionId"].as_str().unwrap());
    let users: Vec<_> = snap.items.iter().filter_map(|i| if let Item::User(u) = i { Some(u.as_str()) } else { None }).collect();
    assert_eq!(users, ["Synthetic prompt", "Synthetic follow-up"]);
    assert_eq!(snap.items.iter().filter(|i| matches!(i, Item::Assistant { text, .. } if text == "Hello world")).count(), 2);
    assert!(!snap.running && !snap.failed);

    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let child_id = recs[0]["metadata"]["missionId"].as_str().unwrap();
        for id in [first.conversation_id.as_str(), child_id] {
            let status = Command::new("node")
                .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/check-m1-ledger.mjs"))
                .arg(reader.clone())
                .arg(t.store().ledger_dir().unwrap())
                .arg(id)
                .status()
                .unwrap();
            assert!(status.success(), "TypeScript reader rejected {id}");
        }
    }
}

fn synthetic_electron_codex_ledger(t: &Temp, id: &str, thread: &str) {
    let lines = [
        json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":"2026-11-01T00:00:00.000Z","metadata":{"missionId":id,"prompt":"Earlier question","runtime":"codex","model":"synthetic-model","createdAt":"2026-11-01T00:00:00.000Z"}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":2,"occurredAt":"2026-11-01T00:00:01.000Z","event":{"id":"e1","sequence":1,"occurredAt":"2026-11-01T00:00:01.000Z","type":"run.started","payload":{"runtimeThreadId":thread}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":3,"occurredAt":"2026-11-01T00:00:02.000Z","event":{"id":"e2","sequence":2,"occurredAt":"2026-11-01T00:00:02.000Z","type":"run.completed","payload":{"runtimeThreadId":thread}}}),
    ];
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let path = t.electron().join("mission-ledger").join(format!("{id}.jsonl"));
    fs::write(&path, text).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
}

#[test]
fn continuing_an_electron_conversation_writes_only_native_and_leaves_electron_untouched() {
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "root_cont", "thread_fake");
    fs::write(t.electron().join("teammates.json"), roster_file(vec![])).unwrap();
    let before = fingerprint(&t.electron());
    let _g = hooks::install(t.store(), fake("resume"));
    let started = send(job(&t, Some("root_cont"), "Synthetic follow-up")).unwrap();
    assert_eq!(started.conversation_id, "root_cont");
    finished("root_cont");
    assert_eq!(fingerprint(&t.electron()), before, "Electron's data folder is byte-for-byte as it was");
    let files = ledgers(&t);
    assert_eq!(files.len(), 1);
    let recs = records(&files[0]);
    assert_eq!(recs[0]["metadata"]["continuesFrom"]["missionId"], "root_cont");
    assert_eq!(kinds(&files[0]).last().map(String::as_str), Some("run.completed"));
    // The window's reader sees Electron's turn then native's, as one conversation.
    let snap = session(t.electron().join("mission-ledger"), "root_cont".into()).poll().unwrap();
    assert_eq!(snap.items.iter().filter(|i| matches!(i, Item::User(_))).count(), 2);
}

#[test]
fn a_continuation_follows_the_newest_turn_not_the_anchor() {
    // Two turns already exist (root_newest, then a native child); a third continues the CHILD's thread, not the root's.
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "root_newest", "thread_old");
    let _g = hooks::install(t.store(), fake("resume"));
    let w = LedgerWriter::create_continuing(&t.store(), "child_n", "Second", "synthetic-model", "x", Some(&Continuation { mission_id: "root_newest".into(), thread: Some("thread_old".into()) })).unwrap();
    drop(w);
    let mut w = LedgerWriter::create_continuing(&t.store(), "child_m", "Third", "synthetic-model", "x", Some(&Continuation { mission_id: "child_n".into(), thread: Some("thread_fake".into()) })).unwrap();
    w.append("run.started", json!({"runtimeThreadId":"thread_fake"}), Some("thread_fake")).unwrap();
    w.append("run.completed", json!({"runtimeThreadId":"thread_fake"}), Some("thread_fake")).unwrap();
    drop(w);
    // Deliberately tie mtimes and reverse lexical chronology: graph links, not file order, choose the tip.
    let tied = std::time::SystemTime::now();
    for id in ["child_n", "child_m"] {
        fs::File::options().write(true).open(path_of(&t, id)).unwrap().set_modified(tied).unwrap();
    }
    assert_eq!(real_tip("root_newest").unwrap(), "child_m");
    let prior = prior_of("child_m").unwrap();
    assert_eq!(prior.thread.as_deref(), Some("thread_fake"));
    assert!(!prior.running);
    send(job(&t, Some("root_newest"), "Synthetic follow-up")).unwrap();
    finished("root_newest");
    let newest = ledgers(&t).into_iter().find(|p| {
        let r = records(p);
        r[0]["metadata"]["continuesFrom"]["missionId"] == "child_m"
    }).expect("a child of the newest turn");
    assert_eq!(kinds(&newest).last().map(String::as_str), Some("run.completed"));
}

#[test]
fn resume_point_ports_runtimethreadidof_and_resumablethreadof() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), None);
    let write = |id: &str, events: Vec<(&str, Value)>, link: Option<Value>| {
        let mut w = LedgerWriter::create_continuing(&t.store(), id, "p", "m", "x", None).unwrap();
        for (kind, payload) in events {
            w.append(kind, payload, None).unwrap();
        }
        let _ = link;
    };
    write("held", vec![("run.started", json!({"runtimeThreadId":"t1"})), ("run.completed", json!({"runtimeThreadId":"t2"}))], None);
    assert_eq!(prior_of("held").unwrap().thread.as_deref(), Some("t2"), "the LAST thread id wins");
    write("ended", vec![("run.started", json!({"runtimeThreadId":"t1"})), ("run.failed", json!({"sessionEnded":true,"message":"gone"}))], None);
    assert_eq!(prior_of("ended").unwrap().thread, None, "a failure that ended the session leaves nothing to resume");
    write("moved", vec![("run.started", json!({"runtimeThreadId":"t1"})), ("adapter.diagnostic", json!({"code":"host.folder_moved","level":"warning","message":"m"}))], None);
    assert_eq!(prior_of("moved").unwrap().thread, None);
    write("never", vec![], None);
    assert_eq!(prior_of("never").unwrap().thread, None);
    assert!(prior_of("never").unwrap().running, "a fresh file with no terminal record is a running turn");
    let mut w = LedgerWriter::create_continuing(&t.store(), "carried", "p", "m", "x", Some(&Continuation { mission_id: "held".into(), thread: Some("t9".into()) })).unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    assert_eq!(prior_of("carried").unwrap().thread.as_deref(), Some("t9"), "falls back to the thread its continuesFrom carried");
    assert!(prior_of("missing").is_err());
    assert!(prior_of("../x").is_err());
}

#[test]
fn a_conversation_with_nothing_to_resume_or_still_running_is_refused_without_starting_anything() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("resume"));
    let mut w = LedgerWriter::create_continuing(&t.store(), "nothread", "p", "m", "x", None).unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    let before = ledgers(&t).len();
    assert!(send(job(&t, Some("nothread"), "Synthetic follow-up")).unwrap_err().contains("no Codex session"));
    // A turn still running in the conversation (fresh file, no terminal record).
    let _running = LedgerWriter::create_continuing(&t.store(), "busy", "p", "m", "x", None).unwrap();
    assert!(send(job(&t, Some("busy"), "Synthetic follow-up")).unwrap_err().contains("still running"));
    assert_eq!(ledgers(&t).len(), before + 1, "refusals create no ledger");
}

#[test]
fn only_codex_runs_natively_and_nothing_else_is_started() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let mut other = job(&t, None, "Synthetic prompt");
    other.teammate.runtime = Some("claude".into());
    other.teammate.runs_in = IN_LOCUST.into();
    assert!(send(other).unwrap_err().contains("runs in Locust for now"));
    assert!(ledgers(&t).is_empty());
    // A conversation that ran on another agent cannot be continued by the Codex runtime.
    let mut w = LedgerWriter::create_continuing(&t.store(), "claude_c", "p", "m", "x", None).unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    let path = path_of(&t, "claude_c");
    let text = fs::read_to_string(&path).unwrap().replacen("\"runtime\":\"codex\"", "\"runtime\":\"claude\"", 1);
    fs::write(&path, text).unwrap();
    assert!(send(job(&t, Some("claude_c"), "Synthetic follow-up")).unwrap_err().contains("another agent"));
}

// ---- stop -----------------------------------------------------------------------------------------------------
fn started(path: &Path) -> bool {
    kinds(path).iter().any(|k| k == "run.started")
}

#[test]
fn stop_interrupts_the_runtime_records_cancelled_and_only_stops_what_this_window_started() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("interrupt"));
    // Not ours: nothing registered, nothing stopped.
    assert!(stop("someone_elses_turn").is_err());
    let s = send(job(&t, None, "Synthetic prompt")).unwrap();
    let path = path_of(&t, &s.conversation_id);
    wait("the turn to start", || path.exists() && started(&path));
    // A second send in a running conversation is refused (it would race the first on one thread).
    assert!(owns(&s.conversation_id));
    assert!(stop(&s.conversation_id).is_ok());
    finished(&s.conversation_id);
    let kinds = kinds(&path);
    assert_eq!(kinds.last().map(String::as_str), Some("run.cancelled"), "{kinds:?}");
    assert!(!kinds.iter().any(|k| k == "run.failed" || k == "run.completed"));
    assert!(!owns(&s.conversation_id), "after the turn ends Stop is gone");
    assert!(stop(&s.conversation_id).is_err());
    let transcript = ledger::read_transcript(&path);
    assert!(!transcript.running && !transcript.failed);
    assert!(transcript.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text == "Stopped")));
}

#[test]
fn a_second_send_while_a_turn_runs_is_refused_and_stop_works_on_a_continuation() {
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "root_second", "thread_fake");
    let _g = hooks::install(t.store(), fake("resume-interrupt"));
    send(job(&t, Some("root_second"), "Synthetic follow-up")).unwrap();
    let child = wait_for_child(&t);
    wait("the continuation to start", || started(&child));
    assert!(send(job(&t, Some("root_second"), "Synthetic follow-up")).unwrap_err().contains("already running"));
    assert_eq!(ledgers(&t).len(), 1, "the refused send created nothing");
    stop("root_second").unwrap();
    finished("root_second");
    assert_eq!(kinds(&child).last().map(String::as_str), Some("run.cancelled"));
}
fn wait_for_child(t: &Temp) -> PathBuf {
    wait("the ledger", || !ledgers(t).is_empty());
    ledgers(t).remove(0)
}

#[test]
fn a_refused_resume_is_recorded_as_failed_once_and_never_retried() {
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "root_refused", "thread_fake");
    let _g = hooks::install(t.store(), fake("resume-refused"));
    send(job(&t, Some("root_refused"), "Synthetic follow-up")).unwrap();
    finished("root_refused");
    let files = ledgers(&t);
    assert_eq!(files.len(), 1, "no second attempt, no fresh thread");
    let k = kinds(&files[0]);
    assert_eq!(k.last().map(String::as_str), Some("run.failed"));
    assert!(!k.iter().any(|k| k == "run.started"), "the turn never started");
    let failed = records(&files[0]).into_iter().rev().find(|r| r["event"]["type"] == "run.failed").unwrap();
    assert!(failed["event"]["payload"]["message"].as_str().unwrap().contains("no rollout found"));
    let t2 = ledger::read_transcript(&files[0]);
    assert!(t2.failed && !t2.running);
}

#[test]
fn a_missing_codex_is_recorded_in_the_ledger_not_lost() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), Some((PathBuf::from("/nonexistent/locust-no-such-codex"), vec![])));
    let s = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&s.conversation_id);
    assert_eq!(kinds(&path_of(&t, &s.conversation_id)).last().map(String::as_str), Some("run.failed"));
}

// ---- recents: the merge ----------------------------------------------------------------------------------------
#[test]
fn recents_merge_electrons_read_only_list_with_native_marked() {
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "root_e", "thread_fake");
    let _g = hooks::install(t.store(), None);
    // Nothing native yet: native-engine reads that as an empty folder, door-engine falls back to Electron alone.
    let only = list_recent(12, true).unwrap();
    assert_eq!(only.summaries.len(), 1);
    assert!(only.native.is_empty());
    let mut w = LedgerWriter::create_continuing(&t.store(), "nat_1", "Native question", "m", "x", Some(&Continuation { mission_id: "root_e".into(), thread: None })).unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    let both = list_recent(12, false).unwrap();
    let ids: Vec<_> = both.summaries.iter().map(|m| m.mission_id.as_str()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"root_e") && ids.contains(&"nat_1"));
    assert!(both.native.contains("nat_1") && !both.native.contains("root_e"), "only native rows are marked");
    assert_eq!(list_recent(1, false).unwrap().summaries.len(), 1);
}

// ---- the guard ------------------------------------------------------------------------------------------------
#[test]
fn nothing_can_be_written_inside_electrons_folder() {
    let t = Temp::new();
    // A native root inside Electron's folder (also through a symlink) cannot be built at all.
    assert!(Store::new(t.electron().join("native"), t.electron()).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(t.electron(), t.0.join("alias")).unwrap();
        assert!(Store::new(t.0.join("alias/native"), t.electron()).is_err());
    }
    // And a writer on a good store only ever creates files under the native folder.
    let before = fingerprint(&t.electron());
    let store = t.store();
    LedgerWriter::create_continuing(&store, "ok", "p", "m", "x", None).unwrap();
    assert!(store.write_path("ok").unwrap().starts_with(t.0.canonicalize().unwrap().join("native")));
    assert_eq!(fingerprint(&t.electron()), before);
    assert!(LedgerWriter::create_continuing(&store, "bad", "p", "m", "x", Some(&Continuation { mission_id: "../x".into(), thread: None })).is_err());
    assert!(!store.write_path("bad").unwrap().exists(), "a refused continuation creates no file");
}

// ---- fixes after review ----------------------------------------------------------------------------------------
fn electron_child(t: &Temp, id: &str, parent: &str, thread: &str, finished: bool) -> PathBuf {
    let mut lines = vec![
        json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":"2026-11-01T01:00:00.000Z","metadata":{"missionId":id,"prompt":"Electron follow-up","runtime":"codex","model":"synthetic-model","createdAt":"2026-11-01T01:00:00.000Z","continuesFrom":{"missionId":parent,"checkpointEpoch":1,"reason":"follow-up"}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":2,"occurredAt":"2026-11-01T01:00:01.000Z","event":{"id":format!("{id}:1"),"sequence":1,"occurredAt":"2026-11-01T01:00:01.000Z","type":"run.started","payload":{"runtimeThreadId":thread}}}),
    ];
    if finished {
        lines.push(json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":3,"occurredAt":"2026-11-01T01:00:02.000Z","event":{"id":format!("{id}:2"),"sequence":2,"occurredAt":"2026-11-01T01:00:02.000Z","type":"run.completed","payload":{"runtimeThreadId":thread}}}));
    }
    let path = t.electron().join("mission-ledger").join(format!("{id}.jsonl"));
    fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    path
}

#[test]
fn a_stale_view_never_resumes_an_obsolete_thread_or_starts_beside_a_running_turn() {
    // The window shows completed A (thread_old). Electron then wrote child B. The send names only the conversation:
    // the real newest turn is read from disk, in both folders, at that moment.
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "stale_a", "thread_old");
    let _g = hooks::install(t.store(), fake("resume"));
    assert_eq!(real_tip("stale_a").unwrap(), "stale_a");
    // B is running (fresh file, no terminal record): refused with a visible message, nothing created.
    let b = electron_child(&t, "stale_b", "stale_a", "thread_fake", false);
    assert_eq!(real_tip("stale_a").unwrap(), "stale_b", "the chain is rescanned, not remembered");
    let refused = send(job(&t, Some("stale_a"), "Synthetic follow-up")).unwrap_err();
    assert!(refused.contains("still running"), "{refused}");
    assert!(ledgers(&t).is_empty(), "a refused send creates no ledger");
    // B finishes with ITS thread. The follow-up resumes B's thread (the fake rejects anything but thread_fake, so a
    // resume of A's thread_old would end as a failure) and links to B, not to A.
    electron_child(&t, "stale_b", "stale_a", "thread_fake", true);
    fs::File::options().write(true).open(&b).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(3600)).unwrap();
    send(job(&t, Some("stale_a"), "Synthetic follow-up")).unwrap();
    finished("stale_a");
    let files = ledgers(&t);
    assert_eq!(files.len(), 1);
    let recs = records(&files[0]);
    assert_eq!(recs[0]["metadata"]["continuesFrom"]["missionId"], "stale_b", "continues the real tip, not a sibling of it");
    assert_eq!(recs[0]["metadata"]["continuesFrom"]["runtimeThreadId"], "thread_fake");
    assert_eq!(kinds(&files[0]).last().map(String::as_str), Some("run.completed"));
    assert_eq!(real_tip("stale_a").unwrap(), recs[0]["metadata"]["missionId"].as_str().unwrap());
}

#[test]
fn a_turn_running_in_native_blocks_a_send_from_a_stale_anchor_too() {
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "own_a", "thread_fake");
    let _g = hooks::install(t.store(), fake("resume-interrupt"));
    send(job(&t, Some("own_a"), "Synthetic follow-up")).unwrap();
    let child = wait_for_child(&t);
    wait("the turn to start", || started(&child));
    let tip = real_tip("own_a").unwrap();
    assert_ne!(tip, "own_a");
    assert!(owns(&tip), "the turn is registered under its own id as well as the conversation's");
    assert_eq!(owned_key(Some(&tip), Some("own_a")).as_deref(), Some(tip.as_str()));
    // A stale view that still thinks the anchor is the tip: the real tip is running, so it is refused.
    assert!(send(job(&t, Some("own_a"), "Synthetic follow-up")).is_err());
    stop("own_a").unwrap();
    finished("own_a");
    assert_eq!(owned_key(Some(&tip), Some("own_a")), None, "ownership ends with the turn");
}

#[test]
fn shutdown_returns_at_once_and_cancels_in_the_background() {
    // An unresponsive server (never answers the handshake) cannot make closing the window wait.
    let t = Temp::new();
    let hang = t.0.join("hang.py");
    fs::write(&hang, "import sys, time\nfor line in sys.stdin:\n    time.sleep(60)\n").unwrap();
    let _g = hooks::install(t.store(), Some((PathBuf::from("python3"), vec![hang.to_string_lossy().into()])));
    let s = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(owns(&s.conversation_id));
    let begun = Instant::now();
    shutdown_in_background_for(Duration::from_secs(8));
    assert!(begun.elapsed() < Duration::from_millis(200), "the UI thread must not wait: {:?}", begun.elapsed());
    // The turn still ends, recorded, and the background watcher finishes once it has.
    wait("the cancelled record", || !owns(&s.conversation_id));
    let k = kinds(&path_of(&t, &s.conversation_id));
    assert_eq!(k.last().map(String::as_str), Some("run.cancelled"), "{k:?}");
    wait_for_shutdown();
}

/// Real Codex, real login, throwaway folders: a turn, then a continuation that resumes the same Codex thread.
/// `cargo test --release real_codex_continue -- --ignored --nocapture`. Uses a fake Electron folder, never the real one.
#[test]
#[ignore]
fn real_codex_continue() {
    let t = Temp::new();
    fs::write(t.0.join("work/note.txt"), "alpha beta gamma\n").unwrap();
    let _g = hooks::install(t.store(), None);
    let mate = Teammate {
        id: "tm_real".into(), name: "Real".into(), role: String::new(), runtime: Some("codex".into()),
        model: None, busy: false, runs_in: "ask".into(), folder: Some(t.0.join("work").to_string_lossy().into()),
    };
    let dir = t.store().ledger_dir().unwrap();
    let wait_done = |before: usize| -> PathBuf {
        let end = Instant::now() + Duration::from_secs(240);
        loop {
            assert!(Instant::now() < end, "turn did not finish");
            let mut files: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
            files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).unwrap());
            if files.len() > before {
                let last = files.last().unwrap().clone();
                let text = fs::read_to_string(&last).unwrap();
                if ["run.completed", "run.failed", "run.cancelled"].iter().any(|k| text.contains(&format!("\"type\":\"{k}\""))) {
                    return last;
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    };
    let first = send(Send { teammate: mate.clone(), conversation: None, prompt: "Remember the word PELICAN. Reply with just OK.".into(), model: None, effort: None }).unwrap();
    let a = wait_done(0);
    let b_started = send(Send { teammate: mate, conversation: Some(first.conversation_id.clone()), prompt: "What word did I ask you to remember? One word.".into(), model: None, effort: None }).unwrap();
    assert_eq!(b_started.conversation_id, first.conversation_id);
    let b = wait_done(1);
    let text_b = fs::read_to_string(&b).unwrap();
    let thread_a = prior_of(a.file_stem().unwrap().to_str().unwrap()).unwrap().thread.unwrap();
    println!("A: {}\nB: {}\nthread A: {thread_a}", a.display(), b.display());
    for line in text_b.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let e = &v["event"];
        println!("B {} {}", e["type"], e["payload"]["message"].as_str().or(e["payload"]["text"].as_str()).unwrap_or("").chars().take(200).collect::<String>());
    }
    assert!(text_b.contains("\"type\":\"run.completed\""), "second turn did not complete");
    assert!(text_b.contains(&thread_a), "second turn did not resume the same Codex thread");
    assert!(text_b.contains("continuesFrom"), "second turn is not chained");
    assert!(text_b.to_uppercase().contains("PELICAN"), "the resumed thread lost its context");
}

/// Both live tests deliberately use the public window API and isolated stores.
/// No retry after denial: another pending request in that turn fails the test.
fn approval_deny_then_allow(runtime: &str, synthetic: bool) {
    let t = Temp::new();
    let command = synthetic.then(|| (PathBuf::from("python3"), vec![concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/fixtures/native-approvals.py").into(), runtime.into()]));
    let _g = hooks::install(t.store(), command);
    let electron_before = fingerprint(&t.electron());
    let mut mate = codex(&t);
    mate.runtime = Some(runtime.into());
    mate.model = None;
    mate.runs_in = "approve-each".into();
    struct StopOnDrop(String);
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            let _ = stop(&self.0);
            let end = Instant::now() + Duration::from_secs(8);
            while owns(&self.0) && Instant::now() < end { std::thread::sleep(Duration::from_millis(20)); }
        }
    }
    for (decision, expected) in [(Decision::Deny, "denied"), (Decision::ApproveOnce, "allowed")] {
        let started = send(Send {
            teammate: mate.clone(), conversation: None,
            prompt: "Use a tool to create hello.txt in the current folder containing exactly hello\n. Ask for permission for the write. If denied, stop immediately and report the denial; do not try another tool or retry. After an allowed write, reply DONE.".into(),
            model: None, effort: None,
        }).unwrap();
        let _cleanup = StopOnDrop(started.conversation_id.clone());
        let path = path_of(&t, &started.conversation_id);
        let deadline = Instant::now() + Duration::from_secs(240);
        let request = loop {
            if let Some(request) = waiting_approvals().into_iter().find(|r| r.mission_id == started.conversation_id) { break request; }
            assert!(owns(&started.conversation_id), "{runtime} ended without asking: {}", fs::read_to_string(&path).unwrap());
            assert!(Instant::now() < deadline, "{runtime} did not ask for permission");
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(!t.0.join("work/hello.txt").exists(), "write happened before approval");
        assert!(request.detail.contains("hello.txt"), "unrelated approval: {}", request.detail);
        decide_approval(&request.approval_id, decision.clone(), Some("Live approval test".into())).unwrap();
        while owns(&started.conversation_id) {
            assert!(waiting_approvals().iter().all(|r| r.mission_id != started.conversation_id), "{runtime} retried after its one decision");
            assert!(Instant::now() < deadline, "{runtime} did not finish");
            std::thread::sleep(Duration::from_millis(50));
        }
        let rows = records(&path);
        let approvals: Vec<_> = rows.iter().filter(|r| r["recordType"] == "mission.approval").collect();
        assert_eq!(approvals.len(), 1, "{rows:?}");
        assert_eq!(approvals[0]["approval"]["answer"], expected);
        assert_eq!(approvals[0]["approval"]["approvalId"], request.approval_id);
        assert_eq!(kinds(&path).last().map(String::as_str), Some("run.completed"), "{}", fs::read_to_string(&path).unwrap());
        assert!(decide_approval(&request.approval_id, Decision::ApproveOnce, None).is_err());
        if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
            assert!(Command::new("node").arg(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/check-m1-ledger.mjs"))
                .arg(reader).arg(t.store().ledger_dir().unwrap()).arg(&started.conversation_id).status().unwrap().success());
        }
        if decision == Decision::Deny {
            assert!(!t.0.join("work/hello.txt").exists());
        } else {
            assert_eq!(fs::read_to_string(t.0.join("work/hello.txt")).unwrap(), "hello\n");
        }
    }
    assert_eq!(fingerprint(&t.electron()), electron_before);
}

#[test]
#[ignore = "real Claude Code; signed-in CLI, no Electron, isolated temp project"]
fn real_claude_approval_deny_then_allow() { approval_deny_then_allow("claude", false); }

#[test]
#[ignore = "real Codex; signed-in CLI, no Electron, isolated temp project"]
fn real_codex_approval_deny_then_allow() { approval_deny_then_allow("codex", false); }

#[test]
fn native_claude_denies_then_allows_one_file_write() { approval_deny_then_allow("claude", true); }
#[test]
fn native_codex_denies_then_allows_one_file_write() { approval_deny_then_allow("codex", true); }
