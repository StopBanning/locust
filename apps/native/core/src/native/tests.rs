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
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
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
        vec![
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../scripts/fixtures/m1-app-server.py"
            )
            .into(),
            scenario.into(),
        ],
    ))
}

fn codex(t: &Temp) -> Teammate {
    Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
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
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn kinds(path: &Path) -> Vec<String> {
    records(path)
        .iter()
        .filter_map(|r| r["event"]["type"].as_str().map(str::to_owned))
        .collect()
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
                out.push((
                    e.path(),
                    m.len(),
                    m.modified().unwrap(),
                    fs::read(e.path()).unwrap(),
                ));
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
        teammate_json(
            "tm_1",
            "Codec",
            "Code & Migrations",
            json!({"runtime":"codex","model":"gpt-6.1-sol","mode":"accept-edits"}),
        ),
        teammate_json(
            "tm_2",
            "Argon",
            "Code & Migrations",
            json!({"runtime":"antigravity","model":"Gemini 3.8 Flash","mode":"auto"}),
        ),
        teammate_json(
            "tm_3",
            "Wren",
            "Custom",
            json!({"runtime":"codex","model":"account-default","mode":"ask","account":"account_0123456789abcdef0123456789abcdef"}),
        ),
        teammate_json("tm_4", "Nova", "Custom", Value::Null),
        teammate_json(
            "tm_5",
            "Gemini",
            "Custom",
            json!({"runtime":"gemini","model":"account-default","mode":"ask"}),
        ),
    ]);
    let team = parse_roster(&text).unwrap();
    assert_eq!(team.len(), 5);
    assert_eq!(
        (
            team[0].runtime.as_deref(),
            team[0].model.as_deref(),
            team[0].runs_in.as_str()
        ),
        (Some("codex"), Some("gpt-6.1-sol"), "accept-edits")
    );
    assert!(supported(&team[0]));
    assert_eq!(
        (team[1].runtime.as_deref(), team[1].runs_in.as_str()),
        (Some("antigravity"), "auto")
    );
    assert!(supported(&team[1]), "Antigravity is native in M4C");
    assert!(
        supported(&team[2]),
        "a Codex named account now runs natively with its own login home"
    );
    assert_eq!(
        team[2].account.as_deref(),
        Some("account_0123456789abcdef0123456789abcdef")
    );
    assert!(
        !supported(&team[3]),
        "no route yet: nothing to run natively"
    );
    assert_eq!(team[3].role, "Custom");
    assert_eq!(team[4].runs_in, IN_LOCUST);
    assert!(!supported(&team[4]), "Gemini still runs in Locust for now");
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
        teammate_json(
            "tm_route",
            "Route",
            "Custom",
            json!({"runtime":"codex","model":"","mode":"ask"}),
        ),
        json!({"teammateId":"tm_nodate","name":"NoDate","hue":"blue","role":"Custom"}),
        json!("not an object"),
    ]);
    let team = parse_roster(&text).unwrap();
    let names: Vec<_> = team.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        ["Padded", "Route"],
        "bad records are dropped, the rest stays usable"
    );
    assert!(
        team[1].runtime.is_none(),
        "a malformed route reads as no route"
    );
    // The file itself: not JSON, or another schema, is unreadable, never "empty".
    assert!(parse_roster("{").is_err());
    assert!(parse_roster(&json!({"schemaVersion":2,"teammates":[]}).to_string()).is_err());
    assert!(parse_roster("[]").is_err());
}

#[test]
fn roster_is_read_only_and_a_missing_file_is_an_empty_team() {
    let t = Temp::new();
    assert_eq!(roster(&t.electron()).unwrap().len(), 0);
    let text = roster_file(vec![teammate_json(
        "tm_1",
        "Codec",
        "Custom",
        json!({"runtime":"codex","model":"m","mode":"ask"}),
    )]);
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
    vec![Teammate {
        id: "a".into(),
        name: "A".into(),
        role: String::new(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }]
}
#[test]
fn engine_choice_forced_door_down_door_up_and_door_refusal() {
    let dies = || -> Result<Vec<Teammate>, String> { panic!("must not be asked") };
    // LOCUST_NATIVE_ENGINE=1: native, and the door is never asked.
    assert_eq!(choose(true, dies, || Ok(team())).unwrap().0, Kind::Native);
    // The door answers: door, and native is never asked.
    assert_eq!(choose(false, || Ok(team()), dies).unwrap().0, Kind::Door);
    // Unreachable: native, for each of the three "cannot reach it" wordings.
    for words in [
        engine::DOWN,
        engine::NOT_ON,
        "Locust's data folder could not be found on this computer.",
    ] {
        assert_eq!(
            choose(false, || Err(words.to_string()), || Ok(team()))
                .unwrap()
                .0,
            Kind::Native,
            "{words}"
        );
    }
    // Reachable but refused (wrong token, an error status): that is not "unreachable", so no silent switch.
    for words in [
        "Locust refused the request (500).",
        "Locust refused the request: its connection has changed. Try again; if it keeps happening, restart Locust.",
    ] {
        assert_eq!(
            choose(false, || Err(words.to_string()), dies).unwrap_err(),
            words
        );
    }
    // Door down and nothing for native to read either: the door's words are what the person sees.
    assert_eq!(
        choose(
            false,
            || Err(engine::DOWN.into()),
            || Err("native words".into())
        )
        .unwrap_err(),
        engine::DOWN
    );
    assert_eq!(Kind::Native.label(), "Engine: native");
    assert_eq!(Kind::Door.label(), "Engine: Locust door");
}

// ---- continue a conversation ----------------------------------------------------------------------------------
fn assert_no_reasoning(path: &Path) {
    assert!(
        !fs::read_to_string(path)
            .unwrap()
            .contains("RAW_REASONING_MUST_NOT_SURVIVE")
    );
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
    assert_eq!(
        kinds(&one).last().map(String::as_str),
        Some("run.completed")
    );
    assert_no_reasoning(&one);

    // Continue: thread/resume with the stored runtimeThreadId (the fake fails the turn on thread/start or a wrong id).
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("resume"));
    let second = send(job(&t, Some(&first.conversation_id), "Synthetic follow-up")).unwrap();
    assert_eq!(
        second.conversation_id, first.conversation_id,
        "the conversation keeps its id, as through the door"
    );
    finished(&second.conversation_id);
    let files = ledgers(&t);
    assert_eq!(files.len(), 2);
    let child = files.iter().find(|p| **p != one).unwrap();
    let recs = records(child);
    assert_eq!(
        kinds(child).last().map(String::as_str),
        Some("run.completed"),
        "{:?}",
        kinds(child)
    );
    assert_no_reasoning(child);
    let link = &recs[0]["metadata"]["continuesFrom"];
    assert_eq!(link["missionId"], first.conversation_id.as_str());
    assert_eq!(
        (link["checkpointEpoch"].clone(), link["reason"].clone()),
        (json!(1), json!("follow-up"))
    );
    assert_eq!(link["runtimeThreadId"], "thread_fake");

    // The reader the window uses follows the chain and shows both prompts and both answers.
    let mut s = session(
        t.electron().join("mission-ledger"),
        first.conversation_id.clone(),
    );
    let snap = s.poll().unwrap();
    assert_eq!(snap.tip, recs[0]["metadata"]["missionId"].as_str().unwrap());
    let users: Vec<_> = snap
        .items
        .iter()
        .filter_map(|i| {
            if let Item::User(u) = i {
                Some(u.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(users, ["Synthetic prompt", "Synthetic follow-up"]);
    assert_eq!(
        snap.items
            .iter()
            .filter(|i| matches!(i, Item::Assistant { text, .. } if text == "Hello world"))
            .count(),
        2
    );
    assert!(!snap.running && !snap.failed);

    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let child_id = recs[0]["metadata"]["missionId"].as_str().unwrap();
        for id in [first.conversation_id.as_str(), child_id] {
            let status = Command::new("node")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../scripts/check-m1-ledger.mjs"
                ))
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
    let path = t
        .electron()
        .join("mission-ledger")
        .join(format!("{id}.jsonl"));
    fs::write(&path, text).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old)
        .unwrap();
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
    assert_eq!(
        fingerprint(&t.electron()),
        before,
        "Electron's data folder is byte-for-byte as it was"
    );
    let files = ledgers(&t);
    assert_eq!(files.len(), 1);
    let recs = records(&files[0]);
    assert_eq!(
        recs[0]["metadata"]["continuesFrom"]["missionId"],
        "root_cont"
    );
    assert_eq!(
        kinds(&files[0]).last().map(String::as_str),
        Some("run.completed")
    );
    // The window's reader sees Electron's turn then native's, as one conversation.
    let snap = session(t.electron().join("mission-ledger"), "root_cont".into())
        .poll()
        .unwrap();
    assert_eq!(
        snap.items
            .iter()
            .filter(|i| matches!(i, Item::User(_)))
            .count(),
        2
    );
}

#[test]
fn a_continuation_follows_the_newest_turn_not_the_anchor() {
    // Two turns already exist (root_newest, then a native child); a third continues the CHILD's thread, not the root's.
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "root_newest", "thread_old");
    let _g = hooks::install(t.store(), fake("resume"));
    let w = LedgerWriter::create_continuing(
        &t.store(),
        "child_n",
        "Second",
        "synthetic-model",
        "x",
        Some(&Continuation {
            mission_id: "root_newest".into(),
            thread: Some("thread_old".into()),
        }),
    )
    .unwrap();
    drop(w);
    let mut w = LedgerWriter::create_continuing(
        &t.store(),
        "child_m",
        "Third",
        "synthetic-model",
        "x",
        Some(&Continuation {
            mission_id: "child_n".into(),
            thread: Some("thread_fake".into()),
        }),
    )
    .unwrap();
    w.append(
        "run.started",
        json!({"runtimeThreadId":"thread_fake"}),
        Some("thread_fake"),
    )
    .unwrap();
    w.append(
        "run.completed",
        json!({"runtimeThreadId":"thread_fake"}),
        Some("thread_fake"),
    )
    .unwrap();
    drop(w);
    // Deliberately tie mtimes and reverse lexical chronology: graph links, not file order, choose the tip.
    let tied = std::time::SystemTime::now();
    for id in ["child_n", "child_m"] {
        fs::File::options()
            .write(true)
            .open(path_of(&t, id))
            .unwrap()
            .set_modified(tied)
            .unwrap();
    }
    assert_eq!(real_tip("root_newest").unwrap(), "child_m");
    let prior = prior_of("child_m").unwrap();
    assert_eq!(prior.thread.as_deref(), Some("thread_fake"));
    assert!(!prior.running);
    send(job(&t, Some("root_newest"), "Synthetic follow-up")).unwrap();
    finished("root_newest");
    let newest = ledgers(&t)
        .into_iter()
        .find(|p| {
            let r = records(p);
            r[0]["metadata"]["continuesFrom"]["missionId"] == "child_m"
        })
        .expect("a child of the newest turn");
    assert_eq!(
        kinds(&newest).last().map(String::as_str),
        Some("run.completed")
    );
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
    write(
        "held",
        vec![
            ("run.started", json!({"runtimeThreadId":"t1"})),
            ("run.completed", json!({"runtimeThreadId":"t2"})),
        ],
        None,
    );
    assert_eq!(
        prior_of("held").unwrap().thread.as_deref(),
        Some("t2"),
        "the LAST thread id wins"
    );
    write(
        "ended",
        vec![
            ("run.started", json!({"runtimeThreadId":"t1"})),
            ("run.failed", json!({"sessionEnded":true,"message":"gone"})),
        ],
        None,
    );
    assert_eq!(
        prior_of("ended").unwrap().thread,
        None,
        "a failure that ended the session leaves nothing to resume"
    );
    write(
        "moved",
        vec![
            ("run.started", json!({"runtimeThreadId":"t1"})),
            (
                "adapter.diagnostic",
                json!({"code":"host.folder_moved","level":"warning","message":"m"}),
            ),
        ],
        None,
    );
    assert_eq!(prior_of("moved").unwrap().thread, None);
    write("never", vec![], None);
    assert_eq!(prior_of("never").unwrap().thread, None);
    assert!(
        prior_of("never").unwrap().running,
        "a fresh file with no terminal record is a running turn"
    );
    let mut w = LedgerWriter::create_continuing(
        &t.store(),
        "carried",
        "p",
        "m",
        "x",
        Some(&Continuation {
            mission_id: "held".into(),
            thread: Some("t9".into()),
        }),
    )
    .unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    assert_eq!(
        prior_of("carried").unwrap().thread.as_deref(),
        Some("t9"),
        "falls back to the thread its continuesFrom carried"
    );
    assert!(prior_of("missing").is_err());
    assert!(prior_of("../x").is_err());
}

#[test]
fn a_conversation_with_nothing_to_resume_or_still_running_is_refused_without_starting_anything() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("resume"));
    let mut w =
        LedgerWriter::create_continuing(&t.store(), "nothread", "p", "m", "x", None).unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    let before = ledgers(&t).len();
    assert!(
        send(job(&t, Some("nothread"), "Synthetic follow-up"))
            .unwrap_err()
            .contains("no Codex session")
    );
    // A turn still running in the conversation (fresh file, no terminal record).
    let _running =
        LedgerWriter::create_continuing(&t.store(), "busy", "p", "m", "x", None).unwrap();
    assert!(
        send(job(&t, Some("busy"), "Synthetic follow-up"))
            .unwrap_err()
            .contains("still running")
    );
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
    // A conversation that ran on another agent is continued on this one through a route switch (codex-mission.ts):
    // the new turn starts cold with a brief, and the stopped turn's ledger gets its checkpoint.
    let mut w =
        LedgerWriter::create_continuing(&t.store(), "claude_c", "p", "m", "x", None).unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    let path = path_of(&t, "claude_c");
    let text = fs::read_to_string(&path).unwrap().replacen(
        "\"runtime\":\"codex\"",
        "\"runtime\":\"claude\"",
        1,
    );
    fs::write(&path, text).unwrap();
    let started = send(job(&t, Some("claude_c"), "Synthetic follow-up")).unwrap();
    assert_eq!(started.conversation_id, "claude_c");
    let records: Vec<Value> = fs::read_to_string(&path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let checkpoint = records.iter().find(|r| r["recordType"] == "mission.checkpoint").expect("the stopped turn is reconciled");
    assert_eq!(checkpoint["checkpoint"]["reason"], "route-switch");
    assert_eq!(checkpoint["checkpoint"]["resumeSafety"], "safe");
    let newest = ledgers(&t).into_iter().find(|p| p != &path).expect("the switched turn's ledger");
    let first: Value = serde_json::from_str(fs::read_to_string(&newest).unwrap().lines().next().unwrap()).unwrap();
    let meta = &first["metadata"];
    assert_eq!(meta["continuesFrom"], json!({"missionId": "claude_c", "checkpointEpoch": 1, "reason": "route-switch"}));
    let brief = meta["prompt"].as_str().unwrap();
    assert!(brief.starts_with("You are taking over this conversation from another agent (Claude Code), which finished its last turn."), "{brief}");
    assert!(brief.ends_with("The person now asks:\n\nSynthetic follow-up"), "{brief}");
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
    assert_eq!(
        kinds.last().map(String::as_str),
        Some("run.cancelled"),
        "{kinds:?}"
    );
    assert!(
        !kinds
            .iter()
            .any(|k| k == "run.failed" || k == "run.completed")
    );
    assert!(
        !owns(&s.conversation_id),
        "after the turn ends Stop is gone"
    );
    assert!(stop(&s.conversation_id).is_err());
    let transcript = ledger::read_transcript(&path);
    assert!(!transcript.running && !transcript.failed);
    assert!(
        transcript
            .items
            .iter()
            .any(|i| matches!(i, Item::Note { text, .. } if text == "Stopped"))
    );
}

#[test]
fn a_second_send_while_a_turn_runs_is_refused_and_stop_works_on_a_continuation() {
    let t = Temp::new();
    synthetic_electron_codex_ledger(&t, "root_second", "thread_fake");
    let _g = hooks::install(t.store(), fake("resume-interrupt"));
    send(job(&t, Some("root_second"), "Synthetic follow-up")).unwrap();
    let child = wait_for_child(&t);
    wait("the continuation to start", || started(&child));
    assert!(
        send(job(&t, Some("root_second"), "Synthetic follow-up"))
            .unwrap_err()
            .contains("already running")
    );
    assert_eq!(ledgers(&t).len(), 1, "the refused send created nothing");
    stop("root_second").unwrap();
    finished("root_second");
    assert_eq!(
        kinds(&child).last().map(String::as_str),
        Some("run.cancelled")
    );
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
    assert!(
        !k.iter().any(|k| k == "run.started"),
        "the turn never started"
    );
    let failed = records(&files[0])
        .into_iter()
        .rev()
        .find(|r| r["event"]["type"] == "run.failed")
        .unwrap();
    assert!(
        failed["event"]["payload"]["message"]
            .as_str()
            .unwrap()
            .contains("no rollout found")
    );
    let t2 = ledger::read_transcript(&files[0]);
    assert!(t2.failed && !t2.running);
}

#[test]
fn a_missing_codex_is_recorded_in_the_ledger_not_lost() {
    let t = Temp::new();
    let _g = hooks::install(
        t.store(),
        Some((PathBuf::from("/nonexistent/locust-no-such-codex"), vec![])),
    );
    let s = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&s.conversation_id);
    assert_eq!(
        kinds(&path_of(&t, &s.conversation_id))
            .last()
            .map(String::as_str),
        Some("run.failed")
    );
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
    let mut w = LedgerWriter::create_continuing(
        &t.store(),
        "nat_1",
        "Native question",
        "m",
        "x",
        Some(&Continuation {
            mission_id: "root_e".into(),
            thread: None,
        }),
    )
    .unwrap();
    w.append("run.completed", json!({}), None).unwrap();
    let both = list_recent(12, false).unwrap();
    let ids: Vec<_> = both
        .summaries
        .iter()
        .map(|m| m.mission_id.as_str())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"root_e") && ids.contains(&"nat_1"));
    assert!(
        both.native.contains("nat_1") && !both.native.contains("root_e"),
        "only native rows are marked"
    );
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
    assert!(
        store
            .write_path("ok")
            .unwrap()
            .starts_with(t.0.canonicalize().unwrap().join("native"))
    );
    assert_eq!(fingerprint(&t.electron()), before);
    assert!(
        LedgerWriter::create_continuing(
            &store,
            "bad",
            "p",
            "m",
            "x",
            Some(&Continuation {
                mission_id: "../x".into(),
                thread: None
            })
        )
        .is_err()
    );
    assert!(
        !store.write_path("bad").unwrap().exists(),
        "a refused continuation creates no file"
    );
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
    let path = t
        .electron()
        .join("mission-ledger")
        .join(format!("{id}.jsonl"));
    fs::write(
        &path,
        lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .unwrap();
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
    assert_eq!(
        real_tip("stale_a").unwrap(),
        "stale_b",
        "the chain is rescanned, not remembered"
    );
    let refused = send(job(&t, Some("stale_a"), "Synthetic follow-up")).unwrap_err();
    assert!(refused.contains("still running"), "{refused}");
    assert!(ledgers(&t).is_empty(), "a refused send creates no ledger");
    // B finishes with ITS thread. The follow-up resumes B's thread (the fake rejects anything but thread_fake, so a
    // resume of A's thread_old would end as a failure) and links to B, not to A.
    electron_child(&t, "stale_b", "stale_a", "thread_fake", true);
    fs::File::options()
        .write(true)
        .open(&b)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
    send(job(&t, Some("stale_a"), "Synthetic follow-up")).unwrap();
    finished("stale_a");
    let files = ledgers(&t);
    assert_eq!(files.len(), 1);
    let recs = records(&files[0]);
    assert_eq!(
        recs[0]["metadata"]["continuesFrom"]["missionId"], "stale_b",
        "continues the real tip, not a sibling of it"
    );
    assert_eq!(
        recs[0]["metadata"]["continuesFrom"]["runtimeThreadId"],
        "thread_fake"
    );
    assert_eq!(
        kinds(&files[0]).last().map(String::as_str),
        Some("run.completed")
    );
    assert_eq!(
        real_tip("stale_a").unwrap(),
        recs[0]["metadata"]["missionId"].as_str().unwrap()
    );
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
    assert!(
        owns(&tip),
        "the turn is registered under its own id as well as the conversation's"
    );
    assert_eq!(
        owned_key(Some(&tip), Some("own_a")).as_deref(),
        Some(tip.as_str())
    );
    // A stale view that still thinks the anchor is the tip: the real tip is running, so it is refused.
    assert!(send(job(&t, Some("own_a"), "Synthetic follow-up")).is_err());
    stop("own_a").unwrap();
    finished("own_a");
    assert_eq!(
        owned_key(Some(&tip), Some("own_a")),
        None,
        "ownership ends with the turn"
    );
}

#[test]
fn shutdown_returns_at_once_and_cancels_in_the_background() {
    // An unresponsive server (never answers the handshake) cannot make closing the window wait.
    let t = Temp::new();
    let hang = t.0.join("hang.py");
    fs::write(
        &hang,
        "import sys, time\nfor line in sys.stdin:\n    time.sleep(60)\n",
    )
    .unwrap();
    let _g = hooks::install(
        t.store(),
        Some((
            PathBuf::from("python3"),
            vec![hang.to_string_lossy().into()],
        )),
    );
    let s = send(job(&t, None, "Synthetic prompt")).unwrap();
    assert!(owns(&s.conversation_id));
    let begun = Instant::now();
    shutdown_in_background_for(Duration::from_secs(8));
    assert!(
        begun.elapsed() < Duration::from_millis(200),
        "the UI thread must not wait: {:?}",
        begun.elapsed()
    );
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
    assert_eq!(
        std::env::var("LOCUST_REAL_CODEX").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_CODEX=1 to run this real test"
    );
    real_continue("codex");
}

/// The same with real Claude Code: the second turn passes `--resume` with the session the first one recorded.
#[test]
#[ignore = "real Claude Code; signed-in CLI, no Electron, isolated temp project"]
fn real_claude_continue() {
    assert_eq!(
        std::env::var("LOCUST_REAL_CLAUDE").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_CLAUDE=1 to run this real test"
    );
    real_continue("claude");
}

fn real_continue(runtime: &str) {
    let t = Temp::new();
    fs::write(t.0.join("work/note.txt"), "alpha beta gamma\n").unwrap();
    let _g = hooks::install(t.store(), None);
    let mate = Teammate {
        id: "tm_real".into(),
        name: "Real".into(),
        role: String::new(),
        runtime: Some(runtime.into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    };
    let dir = t.store().ledger_dir().unwrap();
    let wait_done = |before: usize| -> PathBuf {
        let end = Instant::now() + Duration::from_secs(240);
        loop {
            assert!(Instant::now() < end, "turn did not finish");
            let mut files: Vec<_> = fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .collect();
            files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).unwrap());
            if files.len() > before {
                let last = files.last().unwrap().clone();
                let text = fs::read_to_string(&last).unwrap();
                if ["run.completed", "run.failed", "run.cancelled"]
                    .iter()
                    .any(|k| text.contains(&format!("\"type\":\"{k}\"")))
                {
                    return last;
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    };
    let first = send(Send {
        teammate: mate.clone(),
        conversation: None,
        prompt: "Remember the word PELICAN. Reply with just OK.".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    let a = wait_done(0);
    let b_started = send(Send {
        teammate: mate,
        conversation: Some(first.conversation_id.clone()),
        prompt: "What word did I ask you to remember? One word.".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    assert_eq!(b_started.conversation_id, first.conversation_id);
    let b = wait_done(1);
    let text_b = fs::read_to_string(&b).unwrap();
    let thread_a = prior_of(a.file_stem().unwrap().to_str().unwrap())
        .unwrap()
        .thread
        .unwrap();
    println!(
        "A: {}\nB: {}\nthread A: {thread_a}",
        a.display(),
        b.display()
    );
    for line in text_b.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let e = &v["event"];
        println!(
            "B {} {}",
            e["type"],
            e["payload"]["message"]
                .as_str()
                .or(e["payload"]["text"].as_str())
                .unwrap_or("")
                .chars()
                .take(200)
                .collect::<String>()
        );
    }
    assert!(
        text_b.contains("\"type\":\"run.completed\""),
        "second turn did not complete"
    );
    assert!(
        text_b.contains(&thread_a),
        "second turn did not resume the same {runtime} session"
    );
    assert!(
        text_b.contains("continuesFrom"),
        "second turn is not chained"
    );
    assert!(
        text_b.to_uppercase().contains("PELICAN"),
        "the resumed thread lost its context"
    );
}

/// Real agents in Auto, where nothing asks first: the command guard runs as a hook inside the agent and refuses a push
/// from a Scout fix copy. A push is the test because it is harmless if the guard failed (the folder has no remote); a
/// test that asked to end a browser would end one.
#[test]
#[ignore = "real Claude Code; signed-in CLI, no Electron, isolated temp project"]
fn real_claude_guard_refuses_a_push_from_a_fix_copy() {
    assert_eq!(std::env::var("LOCUST_REAL_CLAUDE").ok().as_deref(), Some("1"), "set LOCUST_REAL_CLAUDE=1 to run this real test");
    guard_refuses_a_push_from_a_fix_copy("claude");
}

#[test]
#[ignore = "real Codex; signed-in CLI, no Electron, isolated temp project"]
fn real_codex_guard_refuses_a_push_from_a_fix_copy() {
    assert_eq!(std::env::var("LOCUST_REAL_CODEX").ok().as_deref(), Some("1"), "set LOCUST_REAL_CODEX=1 to run this real test");
    guard_refuses_a_push_from_a_fix_copy("codex");
}

#[test]
#[ignore = "real Copilot CLI; signed-in CLI, no Electron, isolated temp project"]
fn real_copilot_guard_refuses_a_push_from_a_fix_copy() {
    assert_eq!(std::env::var("LOCUST_REAL_COPILOT").ok().as_deref(), Some("1"), "set LOCUST_REAL_COPILOT=1 to run this real test");
    guard_refuses_a_push_from_a_fix_copy("copilot");
}

fn guard_refuses_a_push_from_a_fix_copy(runtime: &str) {
    let t = Temp::new();
    let folder = t.0.join("rt_scoutfix0123456789ab");
    fs::create_dir_all(&folder).unwrap();
    for args in [&["init", "-q"][..], &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "init"]] {
        assert!(std::process::Command::new("git").args(args).current_dir(&folder).status().unwrap().success());
    }
    let _g = hooks::install(t.store(), None);
    // Auto runs only where Settings allow it, as in Electron.
    let mut settings = crate::workspace_settings::load(&t.store()).unwrap();
    settings.auto_mode = true;
    crate::workspace_settings::save(&t.store(), &settings).unwrap();
    let mate = Teammate {
        id: "tm_guard".into(),
        name: "Guard".into(),
        role: String::new(),
        runtime: Some(runtime.into()),
        model: (runtime == "claude").then(|| "haiku".into()),
        account: None,
        busy: false,
        runs_in: "auto".into(),
        folder: Some(folder.to_string_lossy().into()),
    };
    let started = send(Send {
        teammate: mate,
        conversation: None,
        prompt: "Run exactly this shell command once: git push origin main. Do not retry or try anything else. Then report in one line what happened, quoting any refusal.".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    let path = path_of(&t, &started.conversation_id);
    let end = Instant::now() + Duration::from_secs(240);
    let text = loop {
        let text = fs::read_to_string(&path).unwrap_or_default();
        if ["run.completed", "run.failed", "run.cancelled"].iter().any(|k| text.contains(&format!("\"type\":\"{k}\""))) {
            break text;
        }
        assert!(Instant::now() < end, "turn did not finish");
        std::thread::sleep(Duration::from_millis(300));
    };
    println!("{text}");
    assert!(text.contains("\"type\":\"run.completed\""), "the turn did not complete");
    assert!(text.contains("Scout made to prepare a fix"), "the guard's refusal is not in the ledger");
}

/// Real Codex in Accept edits writes in its folder without asking (workspace-write, approval never), as Electron runs it.
#[test]
#[ignore = "real Codex; signed-in CLI, no Electron, isolated temp project"]
fn real_codex_accept_edits_writes_in_its_folder() {
    assert_eq!(std::env::var("LOCUST_REAL_CODEX").ok().as_deref(), Some("1"), "set LOCUST_REAL_CODEX=1 to run this real test");
    let t = Temp::new();
    let folder = t.0.join("work");
    let _g = hooks::install(t.store(), None);
    let mate = Teammate {
        id: "tm_edits".into(),
        name: "Edits".into(),
        role: String::new(),
        runtime: Some("codex".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "accept-edits".into(),
        folder: Some(folder.to_string_lossy().into()),
    };
    let started = send(Send {
        teammate: mate,
        conversation: None,
        prompt: "Create a file named hello.txt in the current folder containing exactly the word hello and a newline. Then reply DONE.".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    let path = path_of(&t, &started.conversation_id);
    let end = Instant::now() + Duration::from_secs(240);
    let text = loop {
        let text = fs::read_to_string(&path).unwrap_or_default();
        if ["run.completed", "run.failed", "run.cancelled"].iter().any(|k| text.contains(&format!("\"type\":\"{k}\""))) {
            break text;
        }
        assert!(Instant::now() < end, "turn did not finish");
        std::thread::sleep(Duration::from_millis(300));
    };
    assert!(text.contains("\"type\":\"run.completed\""), "the turn did not complete: {text}");
    assert_eq!(fs::read_to_string(folder.join("hello.txt")).unwrap_or_default().trim(), "hello", "Codex did not write the file");
}

/// Both live tests deliberately use the public window API and isolated stores.
/// No retry after denial: another pending request in that turn fails the test.
fn approval_deny_then_allow(runtime: &str, synthetic: bool) {
    let t = Temp::new();
    let command = synthetic.then(|| {
        (
            PathBuf::from("python3"),
            vec![
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../scripts/fixtures/native-approvals.py"
                )
                .into(),
                runtime.into(),
            ],
        )
    });
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
            while owns(&self.0) && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    for (decision, expected) in [
        (Decision::Deny, "denied"),
        (Decision::ApproveOnce, "allowed"),
    ] {
        let started = send(Send {
            teammate: mate.clone(), conversation: None,
            prompt: "Use a tool to create hello.txt in the current folder containing exactly hello\n. Ask for permission for the write. If denied, stop immediately and report the denial; do not try another tool or retry. After an allowed write, reply DONE.".into(),
            model: None, effort: None,
        }).unwrap();
        let _cleanup = StopOnDrop(started.conversation_id.clone());
        let path = path_of(&t, &started.conversation_id);
        let deadline = Instant::now() + Duration::from_secs(240);
        let request = loop {
            if let Some(request) = waiting_approvals()
                .into_iter()
                .find(|r| r.mission_id == started.conversation_id)
            {
                break request;
            }
            assert!(
                owns(&started.conversation_id),
                "{runtime} ended without asking: {}",
                fs::read_to_string(&path).unwrap()
            );
            assert!(
                Instant::now() < deadline,
                "{runtime} did not ask for permission"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(
            !t.0.join("work/hello.txt").exists(),
            "write happened before approval"
        );
        assert!(
            request.detail.contains("hello.txt"),
            "unrelated approval: {}",
            request.detail
        );
        decide_approval(
            &request.approval_id,
            decision.clone(),
            Some("Live approval test".into()),
        )
        .unwrap();
        while owns(&started.conversation_id) {
            assert!(
                waiting_approvals()
                    .iter()
                    .all(|r| r.mission_id != started.conversation_id),
                "{runtime} retried after its one decision"
            );
            assert!(Instant::now() < deadline, "{runtime} did not finish");
            std::thread::sleep(Duration::from_millis(50));
        }
        let rows = records(&path);
        let approvals: Vec<_> = rows
            .iter()
            .filter(|r| r["recordType"] == "mission.approval")
            .collect();
        assert_eq!(approvals.len(), 1, "{rows:?}");
        assert_eq!(approvals[0]["approval"]["answer"], expected);
        assert_eq!(approvals[0]["approval"]["approvalId"], request.approval_id);
        assert_eq!(
            kinds(&path).last().map(String::as_str),
            Some("run.completed"),
            "{}",
            fs::read_to_string(&path).unwrap()
        );
        assert!(decide_approval(&request.approval_id, Decision::ApproveOnce, None).is_err());
        if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
            assert!(
                Command::new("node")
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../scripts/check-m1-ledger.mjs"
                    ))
                    .arg(reader)
                    .arg(t.store().ledger_dir().unwrap())
                    .arg(&started.conversation_id)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        if decision == Decision::Deny {
            assert!(!t.0.join("work/hello.txt").exists());
        } else {
            assert_eq!(
                fs::read_to_string(t.0.join("work/hello.txt")).unwrap(),
                "hello\n"
            );
        }
    }
    assert_eq!(fingerprint(&t.electron()), electron_before);
}

#[test]
#[ignore = "real Claude Code; signed-in CLI, no Electron, isolated temp project"]
fn real_claude_approval_deny_then_allow() {
    assert_eq!(
        std::env::var("LOCUST_REAL_CLAUDE").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_CLAUDE=1 to run this real test"
    );
    approval_deny_then_allow("claude", false);
}

#[test]
#[ignore = "real Codex; signed-in CLI, no Electron, isolated temp project"]
fn real_codex_approval_deny_then_allow() {
    assert_eq!(
        std::env::var("LOCUST_REAL_CODEX").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_CODEX=1 to run this real test"
    );
    approval_deny_then_allow("codex", false);
}

#[test]
fn native_claude_denies_then_allows_one_file_write() {
    approval_deny_then_allow("claude", true);
}
#[test]
fn native_codex_denies_then_allows_one_file_write() {
    approval_deny_then_allow("codex", true);
}

#[test]
fn a_copilot_session_id_is_a_version_4_uuid() {
    let a = uuid_v4().unwrap();
    let b = uuid_v4().unwrap();
    assert_ne!(a, b);
    let parts: Vec<&str> = a.split('-').collect();
    assert_eq!(
        parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
        [8, 4, 4, 4, 12]
    );
    assert!(a.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    assert_eq!(&parts[2][..1], "4");
    assert!(matches!(&parts[3][..1], "8" | "9" | "a" | "b"));
}

/// Carry-on, live (B2): a Codex turn that stopped when its plan ran out, carried on to OpenCode's free model. The
/// stopped turn chose a code word and said it, then ran out before writing it down, so the carried turn can only
/// write the right word from the brief. `LOCUST_REAL_OPENCODE=1 cargo test --release real_carry_on -- --ignored --nocapture`.
#[test]
#[ignore = "real OpenCode on its free route; isolated temp folders"]
fn real_carry_on_from_a_plan_that_ran_out() {
    assert_eq!(std::env::var("LOCUST_REAL_OPENCODE").ok().as_deref(), Some("1"), "set LOCUST_REAL_OPENCODE=1");
    let t = Temp::new();
    let _g = hooks::install(t.store(), None);
    let store = t.store();
    let work = t.0.join("work");
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_carry", "name": "Carry", "role": "Custom", "hue": "lime",
                "createdAt": "2026-11-12T00:00:00.000Z", "folder": work.to_string_lossy(), "route": {"runtime": "codex", "model": "gpt-5.5", "mode": "auto"}}],
                "missionOwners": {}, "settings": {"autoMode": true}}),
        )
        .unwrap();
    let prompt = "Choose a code word, tell me what it is, then write exactly that code word into answer.txt.";
    let mut w = LedgerWriter::create_runtime_with(&store, "mission_stopped", prompt, "gpt-5.5", "codex", "codex", &[], None, &json!({"mode": "auto", "sandbox": "full-access"})).unwrap();
    // Each event as a real Codex run writes it: with its evidence, and the run's end with its process.
    let evidence = json!({"runtimeEventType": "codex", "redacted": true});
    let process = json!({"exitCode": 1, "signal": null, "stderr": "", "stderrTruncated": false, "recordCount": 5, "inputDeliveryFailed": false,
        "outputLimitExceeded": false, "oversizedRecordsDropped": 0, "forcedTerminationAttempted": false, "terminationUnconfirmed": false,
        "startedAt": crate::ledger_write::now(), "finishedAt": crate::ledger_write::now()});
    for (kind, payload) in [
        ("tool.started", json!({"itemId": "c1", "toolKind": "command", "name": "shell", "command": "ls", "phase": "started"})),
        ("tool.completed", json!({"itemId": "c1", "toolKind": "command", "name": "shell", "command": "ls", "phase": "completed", "output": ""})),
        ("message.delta", json!({"itemId": "m1", "operation": "replace", "text": "My code word is AMBER-42. Next I will write it into answer.txt.", "final": true})),
        ("route.limit_detected", json!({"kind": "quota-exhausted", "message": "You've hit your usage limit."})),
        ("run.failed", json!({"kind": "quota-exhausted", "message": "You've hit your usage limit.", "runtimeTerminal": "failed", "process": process})),
    ] {
        let mut payload = payload;
        payload["evidence"] = evidence.clone();
        w.append(kind, payload, None).unwrap();
    }
    drop(w);
    crate::people::record_owner(&store, "mission_stopped", "tm_carry").unwrap();

    let (carried, _, _) = carry("mission_stopped", "opencode", Some("opencode/deepseek-v4.1-flash-free")).unwrap();
    assert_eq!(carry("mission_stopped", "opencode", None).unwrap_err(), "This turn was already carried on.");
    let path = path_of(&t, &carried);
    let end = Instant::now() + Duration::from_secs(300);
    let text = loop {
        let text = fs::read_to_string(&path).unwrap_or_default();
        if ["run.completed", "run.failed", "run.cancelled"].iter().any(|k| text.contains(&format!("\"type\":\"{k}\""))) {
            break text;
        }
        assert!(Instant::now() < end, "the carried turn did not finish");
        std::thread::sleep(Duration::from_millis(500));
    };
    let records: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let meta = &records[0]["metadata"];
    println!("brief:\n{}\n", meta["prompt"].as_str().unwrap());
    assert_eq!(meta["runtime"], "opencode");
    assert_eq!(meta["mode"], "auto");
    assert_eq!(meta["continuesFrom"]["reason"], "route-switch");
    assert_eq!(meta["startedBy"], json!({"kind": "carry-on", "from": "codex"}));
    let reply = crate::carry_on::latest_final(&records.iter().filter_map(|r| r.get("event").cloned()).collect::<Vec<_>>()).unwrap_or_default();
    println!("reply: {reply}");
    let answer = fs::read_to_string(work.join("answer.txt")).unwrap_or_default();
    println!("answer.txt: {answer}");
    assert!(text.contains("\"type\":\"run.completed\""), "the carried turn did not complete");
    // The word is only in the brief: the carried turn wrote it, rather than choosing its own.
    assert_eq!(answer.trim(), "AMBER-42", "the carried turn did not finish from the brief");
    let stopped = fs::read_to_string(path_of(&t, "mission_stopped")).unwrap();
    assert!(stopped.contains("\"recordType\":\"mission.checkpoint\""));
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        for (id, phase) in [("mission_stopped", "failed"), (carried.as_str(), "completed")] {
            let status = Command::new("node")
                .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/check-m1-ledger.mjs"))
                .args([reader.as_str(), store.ledger_dir().unwrap().to_str().unwrap(), id, phase])
                .status()
                .unwrap();
            assert!(status.success(), "TypeScript reader rejected {id}");
        }
    }
    wait_for_shutdown();
}

/// main/carry-on.ts onRunEnded: without the person's "Without asking" nothing is carried on; with it, a pick that is the
/// agent which stopped is said in the stopped turn's own thread, and that ledger still reads in Electron.
#[test]
fn carrying_on_without_asking_needs_the_persons_pick_and_never_hands_back_to_the_agent_that_stopped() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), None);
    let store = t.store();
    let stopped = |id: &str| {
        let mut w = LedgerWriter::create_runtime_with(&store, id, "Fix it", "gpt-5.5", "codex", "codex", &[], None, &json!({"mode": "ask", "sandbox": "read-only"})).unwrap();
        let process = json!({"exitCode": 1, "signal": null, "stderr": "", "stderrTruncated": false, "recordCount": 1, "inputDeliveryFailed": false,
            "outputLimitExceeded": false, "oversizedRecordsDropped": 0, "forcedTerminationAttempted": false, "terminationUnconfirmed": false,
            "startedAt": crate::ledger_write::now(), "finishedAt": crate::ledger_write::now()});
        w.append("run.failed", json!({"kind": "quota-exhausted", "message": "Usage limit reached.", "runtimeTerminal": "failed", "process": process, "evidence": {"runtimeEventType": "codex", "redacted": true}}), None).unwrap();
    };
    let team = |plan: Value| {
        store.save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [], "missionOwners": {}, "settings": {"planRunsOut": plan}})).unwrap();
    };
    stopped("mission_quiet");
    team(json!({"runtime": "codex", "automatic": false}));
    carry_on_without_asking(&store, "mission_quiet");
    assert!(!fs::read_to_string(path_of(&t, "mission_quiet")).unwrap().contains("host.carry_on"));

    stopped("mission_same");
    team(json!({"runtime": "codex", "automatic": true}));
    carry_on_without_asking(&store, "mission_same");
    let text = fs::read_to_string(path_of(&t, "mission_same")).unwrap();
    assert!(
        text.contains("Codex CLI is the agent picked to carry on when a plan runs out, and its own plan ran out, so nothing was carried on."),
        "{text}"
    );
    // The note comes after the run's end, as Electron appends it: its reader finds no issue and the turn still failed.
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let script = "const [r, d, id] = process.argv.slice(1); const { createFileMissionLedger } = await import('file://' + r); \
            const got = await createFileMissionLedger({ rootDirectory: d }).readMission(id); \
            if (!got.mission || got.issues.length || got.mission.issues.length || got.mission.phase !== 'failed') { console.error(JSON.stringify(got.issues)); process.exit(1) }";
        let status = Command::new("node")
            .args(["--input-type=module", "-e", script, reader.as_str(), store.ledger_dir().unwrap().to_str().unwrap(), "mission_same"])
            .status()
            .unwrap();
        assert!(status.success(), "Electron's reader did not accept the noted ledger");
    }
}

/// A turn sent with reference ids keeps them on its conversation (references.rs bind), so the next turn has them too.
#[test]
fn a_turn_keeps_the_references_it_was_sent_with_for_the_conversation() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let store = t.store();
    let docs = crate::references::add_folder(&store, &t.0.join("work")).unwrap();
    let started = send_from(job(&t, None, "Synthetic prompt"), Origin { reference_ids: Some(vec![docs.id.clone()]), ..Default::default() }).unwrap();
    finished(&started.conversation_id);
    assert_eq!(crate::references::select(&store, Some(&started.conversation_id), None).unwrap(), vec![docs]);
}

/// The native app's own open folder wins over Electron's, and a turn works there when its teammate has none.
#[test]
fn the_folder_the_native_app_opened_is_where_turns_work() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), None);
    let mine = t.0.join("mine");
    fs::create_dir_all(&mine).unwrap();
    fs::write(t.electron().join("workspace.json"), json!({"schemaVersion": 1, "path": t.0.join("work")}).to_string()).unwrap();
    assert_eq!(open_folder().unwrap(), t.0.join("work"));
    set_open_folder(&t.store(), &mine).unwrap();
    assert_eq!(open_folder().unwrap(), mine);
    let mut mate = job(&t, None, "x").teammate;
    mate.folder = None;
    assert_eq!(folder_for(&mate, &t.electron()).unwrap(), mine.canonicalize().unwrap());
}

#[test]
fn a_ninth_turn_at_once_is_refused_in_electrons_words() {
    let flags: Vec<Arc<AtomicBool>> = (0..8).map(|_| Arc::new(AtomicBool::new(false))).collect();
    let mut m = HashMap::new();
    for (i, f) in flags.iter().enumerate() {
        // Each turn registers twice, under its conversation and its own id: it still counts once.
        m.insert(format!("conversation-{i}"), f.clone());
        m.insert(format!("mission-{i}"), f.clone());
    }
    assert_eq!(live_turns(&m), MAX_LIVE_TURNS);
    m.remove("conversation-0");
    m.remove("mission-0");
    assert_eq!(live_turns(&m), 7);
    assert_eq!(too_many_live(), "Up to 8 missions can run at once. Wait for one to finish or stop it first.");
}

#[test]
fn a_teammate_on_an_own_api_route_runs_natively_through_its_opencode_provider() {
    // Since own-models.rs: a model of your own runs as an OpenCode provider declared in the run's environment.
    let t = Temp::new();
    let mut sage = codex(&t);
    sage.runtime = Some("opencode".into());
    sage.model = Some("own-bf9f2516/claude-sonnet-5-5".into());
    assert!(supported(&sage));
    sage.model = Some("opencode/deepseek-v4.1-flash-free".into());
    assert!(supported(&sage));
    // On another agent, own- is not a route of your own and changes nothing.
    sage.runtime = Some("codex".into());
    assert!(supported(&sage));
}

#[test]
fn a_turn_whose_process_is_gone_is_not_running() {
    // This process, not running it: ended without its last record.
    assert!(orphaned(&format!("mission_native_{}_1", std::process::id())));
    // A process that cannot exist.
    assert!(orphaned("mission_native_4194999_1"));
    // A live process (launchd).
    assert!(!orphaned("mission_native_1_1"));
    // Electron's ids and odd shapes are left to the file's own state.
    assert!(!orphaned("mission_87e6bbdd-b609-4690-bf13-24c3c9fc03d7"));
    assert!(!orphaned("mission_native_x_1"));
}

#[test]
fn a_teammate_on_its_own_branch_works_in_its_copy_and_each_turn_is_committed_there_with_a_receipt() {
    // Electron 0.439 (a-turn-on-its-own-branch-leaves-a-receipt): the turn runs in .locust/worktrees/<id> on the
    // teammate's branch, is committed there when it ends, and its stream says so. The person's checkout is untouched.
    let t = Temp::new();
    let store = t.store();
    let work = t.0.join("work");
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(&work).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "person@locust.test"]); // made-up person; public-guard: allow
    git(&["config", "user.name", "The person"]);
    fs::write(work.join("README.md"), "the project\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "first"]);
    store
        .save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "worktree": true}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let _hook = hooks::install(store.clone(), fake("cards-edit-check"));
    let mate = Teammate { runs_in: "accept-edits".into(), ..codex(&t) };
    let started = send(Send { teammate: mate, conversation: None, prompt: "Write the notes. Then stop.".into(), model: None, effort: None }).unwrap();
    let path = path_of(&t, &started.conversation_id);
    let receipt = |path: &Path| {
        records(path).iter().find_map(|r| (r["event"]["payload"]["code"] == crate::worktrees::CHECKPOINT_CODE).then(|| r["event"]["payload"].clone()))
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    while receipt(&path).is_none() {
        assert!(Instant::now() < deadline, "no receipt: {:?}", kinds(&path));
        std::thread::sleep(Duration::from_millis(25));
    }
    let tree = work.join(".locust/worktrees/tm_codec");
    assert_eq!(fs::read_to_string(tree.join("notes.txt")).unwrap(), "BROKEN\n", "the turn ran in the teammate's copy");
    assert!(!work.join("notes.txt").exists(), "the person's checkout is untouched");
    assert_eq!(git(&["log", "-1", "--format=%s", "locust/codec"]).trim(), "Write the notes.");
    let trailers = git(&["log", "-1", "--format=%B", "locust/codec"]);
    assert!(trailers.contains("Locust-Teammate: Codec") && trailers.contains("Locust-Turn: completed"), "{trailers}");
    assert_eq!(git(&["rev-list", "--count", "main"]).trim(), "1", "main is where the person left it");
    let note = receipt(&path).unwrap();
    assert_eq!(note["level"], "info");
    assert!(note["message"].as_str().unwrap().starts_with("Saved this turn on locust/codec as "), "{note}");
    // The thread shows the receipt.
    assert!(ledger::read_transcript(path.clone()).items.iter().any(|i| matches!(i, ledger::Item::Note { text, .. } if text.starts_with("Saved this turn on locust/codec"))));
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let out = Command::new("node")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/check-m1-ledger.mjs"))
            .args([reader.as_str(), path.parent().unwrap().to_str().unwrap(), started.conversation_id.as_str()])
            .output()
            .unwrap();
        assert!(out.status.success(), "Electron's reader refused the ledger: {}", String::from_utf8_lossy(&out.stderr));
    }
}

#[test]
fn a_teammate_on_its_own_branch_in_a_folder_that_is_no_repository_is_refused_saying_why() {
    // Electron's worktreeRefused: never run in the person's checkout instead.
    let t = Temp::new();
    let store = t.store();
    store
        .save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "worktree": true}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let _hook = hooks::install(store.clone(), fake("success"));
    let refused = send(job(&t, None, "Synthetic prompt")).unwrap_err();
    assert_eq!(refused, "Codec is set to work on its own branch, but the project folder is not a git repository.");
    assert!(ledgers(&t).is_empty(), "nothing was started");
}

#[test]
fn slot_busy_check_refuses_second_turn_in_same_slot() {
    let t = Temp::new();
    let store = t.store();
    let _hook = hooks::install(store.clone(), fake("slow"));
    let first = send_from(
        job(&t, None, "Synthetic prompt"),
        Origin {
            slot: Some(SlotOrigin {
                key: "slot:cmp_1:a".into(),
                cwd: None,
                instructions: None,
                skill: None,
                sealed: false,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    let err = send_from(
        job(&t, None, "Synthetic prompt"),
        Origin {
            slot: Some(SlotOrigin {
                key: "slot:cmp_1:a".into(),
                cwd: None,
                instructions: None,
                skill: None,
                sealed: false,
            }),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(err, "A turn is already running in this conversation.");
    let _ = stop(&first.conversation_id);
}

#[test]
fn slot_cwd_bypasses_own_branch_folder() {
    let t = Temp::new();
    let store = t.store();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "worktree": true}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    let alt_dir = t.0.join("compare-copy");
    std::fs::create_dir_all(&alt_dir).unwrap();
    let _hook = hooks::install(store.clone(), fake("success"));
    let started = send_from(
        job(&t, None, "Synthetic prompt"),
        Origin {
            slot: Some(SlotOrigin {
                key: "slot:cmp_2:a".into(),
                cwd: Some(alt_dir),
                instructions: None,
                skill: None,
                sealed: false,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    wait("slot_cwd turn", || !owns(&started.conversation_id));
}

#[test]
fn sealed_slot_omits_teammate_brief_and_skills() {
    let t = Temp::new();
    let store = t.store();
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [{"teammateId": "tm_codec", "name": "Codec", "role": "SECRET_ROLE_MARKER"}],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();
    let _hook = hooks::install(store.clone(), fake("success"));
    let started = send_from(
        job(&t, None, "Synthetic prompt"),
        Origin {
            slot: Some(SlotOrigin {
                key: "slot:cmp_3:a".into(),
                cwd: None,
                instructions: None,
                skill: None,
                sealed: true,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    wait("sealed_slot turn", || !owns(&started.conversation_id));
    let path = ledgers(&t).into_iter().next().unwrap();
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(!raw.contains("SECRET_ROLE_MARKER"), "sealed slot leaked teammate brief: {raw}");
}

/// a-managed-turn-is-refused-before-an-agent-starts: an organization's policy refuses an agent, a mode or a broken file
/// before anything is started or recorded, in its own words.
#[test]
fn a_managed_turn_is_refused_before_an_agent_starts() {
    let t = Temp::new();
    let store = t.store();
    let _hooks = hooks::install(store.clone(), fake("success"));
    // Whatever happens below, the next test starts with an open policy.
    struct Reopen;
    impl Drop for Reopen {
        fn drop(&mut self) {
            crate::org_policy::use_policy(None);
        }
    }
    let _reopen = Reopen;
    let restrict = |policy: Value| {
        let mut skipped = Vec::new();
        crate::org_policy::use_policy(Some(crate::org_policy::PolicyState {
            path: "/admin/policy.json".into(),
            present: true,
            policy: crate::org_policy::parse_policy(&policy, &mut |k| skipped.push(k.to_owned())).unwrap(),
            error: None,
        }));
    };
    let ledgers = || std::fs::read_dir(store.ledger_dir().unwrap()).map(|d| d.count()).unwrap_or(0);
    restrict(json!({"allowedAgents": ["claude"]}));
    let refused = send(job(&t, None, "Work")).err().expect("refused");
    assert!(refused.contains("is not an allowed AI agent.") && refused.contains("(allowedAgents in /admin/policy.json)"), "{refused}");
    restrict(json!({"highestMode": "ask"}));
    let mut auto = job(&t, None, "Work");
    auto.teammate.runs_in = "accept-edits".into();
    let refused = send(auto).err().expect("refused");
    assert!(refused.contains("Accept edits exceeds the highest allowed mode, Ask."), "{refused}");
    crate::org_policy::use_policy(Some(crate::org_policy::PolicyState {
        path: "/admin/policy.json".into(),
        present: true,
        policy: serde_json::Map::new(),
        error: Some("Locust cannot use the organization policy at /admin/policy.json. The file is not valid JSON.".into()),
    }));
    assert!(send(job(&t, None, "Work")).err().expect("refused").contains("cannot use the organization policy"));
    assert_eq!(ledgers(), 0, "a refused turn recorded something");
    // An open policy lets the same turn start.
    crate::org_policy::use_policy(None);
    let started = send(job(&t, None, "Work")).expect("starts");
    wait("the turn to end", || !owns(&started.conversation_id));
}

#[test]
fn slot_instructions_override_brief_and_sealed_omits_candidate_skill() {
    let t = Temp::new();
    let store = t.store();
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [{"teammateId": "tm_codec", "name": "Codec", "instructions": "SAVED_INST_MARKER"}],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();
    let _hook = hooks::install(store.clone(), fake("success"));
    // 1) Brief respects slot instructions override (saved replaced vs removed).
    let overridden = crate::people::brief_with_instructions(
        &store,
        "tm_codec",
        &t.0.join("work"),
        "Synthetic prompt",
        Some("COLUMN_INST_MARKER"),
    )
    .unwrap();
    assert!(overridden.contains("COLUMN_INST_MARKER"));
    assert!(!overridden.contains("SAVED_INST_MARKER"));

    // 2) Sealed turn never emits candidate or saved skill roots (m1-app-server succeeds without skills/extraRoots/set).
    let started = send_from(
        job(&t, None, "Synthetic prompt"),
        Origin {
            slot: Some(SlotOrigin {
                key: "slot:cmp_tune:b".into(),
                cwd: None,
                instructions: Some("COLUMN_INST_MARKER".into()),
                skill: Some(crate::tune::TuneSkill {
                    name: "col-skill".into(),
                    description: "When checking".into(),
                    body: "Check candidate".into(),
                }),
                sealed: true,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    wait("sealed tune slot turn", || !owns(&started.conversation_id));
    assert!(crate::skills::roots(&started.conversation_id).is_empty());
    let ledger = ledgers(&t).into_iter().next().unwrap();
    let raw = std::fs::read_to_string(ledger).unwrap();
    assert!(!raw.contains("SAVED_INST_MARKER"));
    assert!(!raw.contains("COLUMN_INST_MARKER"));
}


/// peer-exchange.ts, end to end: a teammate's finished reply shares with another through the workroom, the other's
/// next brief quotes it as a claim beside the team and the block formats, and once that run is live it is delivered.
#[test]
fn a_share_waits_in_the_workroom_and_reaches_the_teammates_next_brief() {
    let t = Temp::new();
    let store = t.store();
    let mate = |id: &str, name: &str, role: &str| json!({"teammateId": id, "name": name, "role": role, "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}});
    store.save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [mate("tm_codec", "Codec", "Code & Migrations"), mate("tm_pip", "Pip", "Docs & QA")], "missionOwners": {}, "settings": {"relay": false}})).unwrap();
    let _g = hooks::install(store.clone(), fake("cards-share"));
    let prompt_file = t.0.join("work").join("last-prompt.txt");

    let first = send(job(&t, None, "Check the dev server")).unwrap();
    finished(&first.conversation_id);
    let codec_brief = fs::read_to_string(&prompt_file).unwrap();
    assert!(codec_brief.contains("Teammates in this workspace besides you (Codec, Code & Migrations): Pip (Docs & QA)."), "{codec_brief}");
    assert!(codec_brief.contains("<locust-share to=\"Pip\">") && codec_brief.contains("<locust-ask>") && codec_brief.contains("KEEP A TODO LIST"));
    assert!(codec_brief.ends_with("Check the dev server\n\nReference folders — for reading, not changing:\n") || codec_brief.contains("\n\nCheck the dev server"), "{codec_brief}");
    let workroom = crate::workroom::Workroom::for_store(&store);
    wait("the share to reach the workroom", || workroom.unread("tm_pip", 5, &[]).is_ok_and(|u| u.messages.len() == 1));
    let waiting = workroom.unread("tm_pip", 5, &[]).unwrap().messages;
    assert_eq!((waiting[0].from.name.as_str(), waiting[0].from_mission.as_str()), ("Codec", first.conversation_id.as_str()));

    let mut pip = job(&t, None, "Write the setup doc");
    pip.teammate.id = "tm_pip".into();
    pip.teammate.name = "Pip".into();
    let second = send(pip).unwrap();
    // Delivered once the run is live, not before it starts.
    assert!(workroom.unread("tm_pip", 5, &[]).unwrap().messages.is_empty());
    finished(&second.conversation_id);
    let pip_brief = fs::read_to_string(&prompt_file).unwrap();
    assert!(pip_brief.contains("Messages from teammates, delivered before you started. They are CLAIMS from other agents"), "{pip_brief}");
    assert!(pip_brief.contains("- Codec (Code & Migrations), "));
    assert!(pip_brief.contains("The dev server listens on port 3001, set in vite.config.ts."));
    assert_eq!(workroom.read().deliveries.len(), 1);
    // Codec ran read only, so the host makes no claim about what it changed.
    assert_eq!(waiting[0].observed, None);
    assert!(!pip_brief.contains("Locust saw their run change"));
    // Pip's reply shared nothing, so nothing new waits for anyone.
    assert!(workroom.unread("tm_codec", 5, &[]).unwrap().messages.is_empty());
}

fn write_env_wrapper(
    dir: &Path,
    name: &str,
    env_log: &Path,
    fixture: &str,
    scenario: &str,
    gate: Option<&Path>,
) -> Option<(PathBuf, Vec<String>)> {
    let script = dir.join(name);
    let gate_str = gate.map(|g| g.to_string_lossy().into_owned()).unwrap_or_default();
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("scripts/fixtures")
        .join(fixture);
    let py = r#"import json, os, sys, time
log_path, fixture_path, scenario, gate = sys.argv[1:5]
keys = ['CODEX_HOME', 'CODEX_SQLITE_HOME', 'OPENAI_API_KEY', 'CODEX_API_KEY', 'OPENAI_ACCESS_TOKEN', 'CODEX_ACCESS_TOKEN', 'CLAUDE_CONFIG_DIR', 'ANTHROPIC_API_KEY', 'ANTHROPIC_AUTH_TOKEN', 'CLAUDE_CODE_OAUTH_TOKEN']
with open(log_path, 'a', encoding='utf-8') as f:
    f.write(json.dumps({k: os.environ.get(k) for k in keys}) + '\n')
if gate:
    while not os.path.exists(gate):
        time.sleep(0.02)
os.execv(sys.executable, [sys.executable, fixture_path, scenario] + sys.argv[5:])
"#;
    fs::write(&script, py).unwrap();
    Some((
        PathBuf::from("python3"),
        vec![
            script.to_string_lossy().into_owned(),
            env_log.to_string_lossy().into_owned(),
            fixture_path.to_string_lossy().into_owned(),
            scenario.to_owned(),
            gate_str,
        ],
    ))
}

#[test]
fn named_codex_and_claude_accounts_launch_with_own_home_and_fail_cleanly() {
    let t = Temp::new();
    let store = t.store();
    let env_log = t.0.join("env.jsonl");
    let _hook = hooks::install(
        store.clone(),
        write_env_wrapper(&t.0, "wrap_codex.py", &env_log, "m1-app-server.py", "success", None),
    );
    struct ClearPolicy;
    impl Drop for ClearPolicy {
        fn drop(&mut self) {
            crate::org_policy::use_policy(None);
        }
    }
    let _clear = ClearPolicy;
    crate::org_policy::use_policy(None);

    let accounts = crate::agent_accounts::Accounts::new(
        store.root(),
        Arc::new(|_, _, _, _| Ok(Box::new(|| {}))),
        Box::new(|| crate::org_policy::feature_refusal(Some(&crate::org_policy::current()), "accounts")),
        Box::new(account_in_use),
    );
    let codex_work = accounts.add("codex", "Work").unwrap();
    let codex_personal = accounts.add("codex", "Personal").unwrap();
    let claude_work = accounts.add("claude", "Claude Work").unwrap();
    let _ = crate::agent_accounts::register(Arc::new(accounts));
    let accounts = accounts_for(&store);

    {
        // 1) Codex teammate with saved Work account runs with its own CODEX_HOME and blank keys.
        let mut mate = codex(&t);
        mate.account = Some(codex_work.id.clone());
        let first = send(Send { teammate: mate.clone(), ..job(&t, None, "First") }).unwrap();
        wait("codex Work turn", || !owns(&first.conversation_id));

        // 2) Explicit machine default override runs without CODEX_HOME override & triggers route_switch.
        let second = send_from(
            Send { teammate: mate.clone(), ..job(&t, Some(&first.conversation_id), "On machine") },
            Origin { account: Some(None), ..Default::default() },
        )
        .unwrap();
        wait("codex default switch", || !owns(&second.conversation_id));
        let m2_id = super::real_tip(&first.conversation_id).unwrap();
        assert_ne!(m2_id, first.conversation_id);

        // 3) Switching to Personal account starts cold without resuming Work's thread.
        let third = send_from(
            Send { teammate: mate.clone(), ..job(&t, Some(&first.conversation_id), "Switch account") },
            Origin { account: Some(Some(codex_personal.id.clone())), ..Default::default() },
        )
        .unwrap();
        wait("codex Personal switch", || !owns(&third.conversation_id));
        let m3_id = super::real_tip(&first.conversation_id).unwrap();
        assert_ne!(m3_id, m2_id);

        let logged: Vec<Value> = fs::read_to_string(&env_log)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(logged.len(), 3);
        let work_home = accounts.home_for(&codex_work.id).to_string_lossy().into_owned();
        assert_eq!(logged[0]["CODEX_HOME"].as_str(), Some(work_home.as_str()));
        assert_eq!(logged[0]["CODEX_SQLITE_HOME"].as_str(), Some(work_home.as_str()));
        assert_eq!(logged[0]["OPENAI_API_KEY"].as_str(), Some(""));
        assert_eq!(logged[0]["CODEX_API_KEY"].as_str(), Some(""));
        assert_ne!(logged[1]["CODEX_HOME"].as_str(), Some(work_home.as_str()));
        assert_eq!(logged[2]["CODEX_HOME"].as_str(), Some(accounts.home_for(&codex_personal.id).to_string_lossy().as_ref()));

        // Check resolvedRouteId & continuesFrom on ledgers.
        let m1_created: Value = serde_json::from_str(fs::read_to_string(path_of(&t, &first.conversation_id)).unwrap().lines().next().unwrap()).unwrap();
        assert_eq!(m1_created["metadata"]["resolvedRouteId"], format!("codex:synthetic-model-account:{}", codex_work.id));
        let m2_raw = fs::read_to_string(path_of(&t, &m2_id)).unwrap();
        let m2_created: Value = serde_json::from_str(m2_raw.lines().next().unwrap()).unwrap();
        assert_eq!(m2_created["metadata"]["resolvedRouteId"], "codex:synthetic-model-account:default");
        assert!(m2_created["metadata"]["continuesFrom"].is_object());
        assert!(!m2_created["metadata"]["continuesFrom"]["runtimeThreadId"].is_string());

        // Refusals: wrong agent, unsupported agent, removed account, and policy off.
        let wrong = send_from(job(&t, None, "Wrong agent"), Origin { account: Some(Some(claude_work.id.clone())), ..Default::default() });
        assert_eq!(wrong.unwrap_err(), crate::agent_accounts::OTHER_AGENT);

        let unsup = send_from(job(&t, None, "Cursor"), Origin { runtime: Some("cursor".into()), account: Some(Some(codex_work.id.clone())), ..Default::default() });
        assert_eq!(unsup.unwrap_err(), crate::agent_accounts::NOT_SUPPORTED);

        accounts.remove(&codex_personal.id).unwrap();
        let gone = send_from(job(&t, None, "Removed"), Origin { account: Some(Some(codex_personal.id.clone())), ..Default::default() });
        assert_eq!(gone.unwrap_err(), crate::agent_accounts::REMOVED);

        let mut policy = crate::org_policy::PolicyState {
            path: String::new(),
            present: true,
            policy: Default::default(),
            error: None,
        };
        policy.policy.insert("accounts".into(), json!(false));
        crate::org_policy::use_policy(Some(policy));
        let blocked = send_from(job(&t, None, "Policy off"), Origin { account: Some(Some(codex_work.id.clone())), ..Default::default() });
        assert!(blocked.unwrap_err().contains("organization"), "accounts:false must refuse named account");
        crate::org_policy::use_policy(None);
    }

    // 4) Live account_in_use refuses removal while turn runs, then releases cleanly; and Claude uses CLAUDE_CONFIG_DIR.
    let claude_log = t.0.join("claude_env.jsonl");
    let gate = t.0.join("claude.gate");
    *crate::invariants::lock(&hooks::STATE) = (
        Some(store.clone()),
        write_env_wrapper(&t.0, "wrap_claude.py", &claude_log, "m3-claude.py", "success", Some(&gate)),
    );
    let claude_mate = Teammate {
        id: "tm_claude".into(),
        name: "Claude".into(),
        role: "Code & Migrations".into(),
        runtime: Some("claude".into()),
        model: Some("sonnet".into()),
        account: Some(claude_work.id.clone()),
        busy: false,
        runs_in: "ask".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    };
    let claude_run = send(Send { teammate: claude_mate, ..job(&t, None, "Claude turn") }).unwrap();
    wait("claude account busy", || account_in_use(crate::agent_accounts::Agent::Claude, &claude_work.id));
    assert_eq!(accounts.remove(&claude_work.id).unwrap_err(), crate::agent_accounts::BUSY_REMOVE);
    fs::write(&gate, "go").unwrap();
    wait("claude turn finish", || !owns(&claude_run.conversation_id));
    assert!(!account_in_use(crate::agent_accounts::Agent::Claude, &claude_work.id));
    accounts.remove(&claude_work.id).unwrap();

    let claude_logged: Value = serde_json::from_str(fs::read_to_string(&claude_log).unwrap().lines().next().unwrap()).unwrap();
    let claude_home = accounts.home_for(&claude_work.id).to_string_lossy().into_owned();
    assert_eq!(claude_logged["CLAUDE_CONFIG_DIR"].as_str(), Some(claude_home.as_str()));
    assert_eq!(claude_logged["ANTHROPIC_API_KEY"].as_str(), Some(""));
    assert_eq!(claude_logged["ANTHROPIC_AUTH_TOKEN"].as_str(), Some(""));
    assert_eq!(claude_logged["CLAUDE_CODE_OAUTH_TOKEN"].as_str(), Some(""));
}

#[test]
fn relay_roundtrip_starts_recipient_records_peer_links_remembers_hub_and_returns_to_origin_thread() {
    let t = Temp::new();
    let store = t.store();
    let mate = |id: &str, name: &str, role: &str| {
        json!({
            "teammateId": id,
            "name": name,
            "hue": "blue",
            "role": "Custom",
            "roleTitle": role,
            "createdAt": "2026-11-01T00:00:00.000Z",
            "avatar": {},
            "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}
        })
    };
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [
                    mate("tm_codec", "Codec", "Code & Migrations"),
                    mate("tm_pip", "Pip", "Docs & QA")
                ],
                "missionOwners": {},
                "settings": {"relay": true, "relayHopCap": 6}
            }),
        )
        .unwrap();
    let _g = hooks::install(store.clone(), fake("relay-roundtrip"));

    let first = send(job(&t, None, "Check Vite config")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        if !owns(&first.conversation_id)
            && crate::invariants::lock(&ACTIVE_TEAMMATES).is_empty()
            && crate::people::hub_of(&store, "tm_pip").is_some()
            && real_tip(&first.conversation_id).ok().as_deref() != Some(first.conversation_id.as_str())
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !( !owns(&first.conversation_id)
        && crate::invariants::lock(&ACTIVE_TEAMMATES).is_empty()
        && crate::people::hub_of(&store, "tm_pip").is_some()
        && real_tip(&first.conversation_id).ok().as_deref() != Some(first.conversation_id.as_str()) )
    {
        panic!(
            "roundtrip stuck: owns={} active={:?} hub={:?} tip={:?} notices={:?} prompts={:?}",
            owns(&first.conversation_id),
            *crate::invariants::lock(&ACTIVE_TEAMMATES),
            crate::people::hub_of(&store, "tm_pip"),
            real_tip(&first.conversation_id),
            *crate::invariants::lock(&RELAY_NOTICES),
            fs::read_to_string(t.0.join("work/relay-prompts.jsonl")),
        );
    }

    // Pip's hub mission exists, was started by relay hop 1, is titled "<Name>'s replies", and has links.
    let pip_hub = crate::people::hub_of(&store, "tm_pip").expect("Pip hub mission recorded");
    wait("Pip hub run to release", || !owns(&pip_hub));
    let pip_snap = crate::ledger::read_transcript(store.write_path(&pip_hub).unwrap());
    let pip_meta = pip_snap.metadata.as_ref().unwrap();
    assert_eq!(pip_meta.started_by_kind.as_deref(), Some("relay"));
    assert_eq!(pip_meta.started_by_hop, Some(1));
    assert!(pip_meta.relay.is_some());
    assert!(pip_snap.peer_links.iter().any(|l| l.direction == "received" && l.peer_teammate_id == "tm_codec"));
    assert!(pip_snap.peer_links.iter().any(|l| l.direction == "posted" && l.peer_teammate_id == "tm_codec"));

    let team_file = crate::people::team(&store).unwrap();
    assert_eq!(team_file["missionTitles"][&pip_hub].as_str(), Some("Pip's replies"));

    // Codec's conversation received Pip's reply as a hop-2 relay continuation.
    let codec_tip = real_tip(&first.conversation_id).unwrap();
    assert_ne!(codec_tip, first.conversation_id);
    wait("Codec continuation to finish", || !owns(&codec_tip));
    let codec_first_snap = crate::ledger::read_transcript(store.write_path(&first.conversation_id).unwrap());
    assert!(codec_first_snap.peer_links.iter().any(|l| l.direction == "posted" && l.peer_teammate_id == "tm_pip"));

    let codec_tip_snap = crate::ledger::read_transcript(store.write_path(&codec_tip).unwrap());
    let codec_tip_meta = codec_tip_snap.metadata.as_ref().unwrap();
    assert_eq!(codec_tip_meta.started_by_kind.as_deref(), Some("relay"));
    assert_eq!(codec_tip_meta.started_by_hop, Some(2));
    assert!(codec_tip_snap.peer_links.iter().any(|l| l.direction == "received" && l.peer_teammate_id == "tm_pip"));

    // Both Codec (root) and Pip (nested under Codec) nest cleanly at 1 level deep.
    let rows = vec![
        crate::relay::NestedRow {
            mission_id: codec_tip.clone(),
            member_ids: vec![first.conversation_id.clone(), codec_tip.clone()],
        },
        crate::relay::NestedRow {
            mission_id: pip_hub.clone(),
            member_ids: vec![pip_hub.clone()],
        },
    ];
    let nested = crate::relay::nested_under(&rows, |row| {
        (row.mission_id == pip_hub).then(|| first.conversation_id.clone())
    });
    assert_eq!(nested.get(&pip_hub).map(String::as_str), Some(codec_tip.as_str()));
}

#[test]
fn relay_prose_return_posts_unanswered_prose_back_to_asker() {
    let t = Temp::new();
    let store = t.store();
    let mate = |id: &str, name: &str, role: &str| {
        json!({
            "teammateId": id,
            "name": name,
            "hue": "blue",
            "role": "Custom",
            "roleTitle": role,
            "createdAt": "2026-11-01T00:00:00.000Z",
            "avatar": {},
            "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}
        })
    };
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [
                    mate("tm_codec", "Codec", "Code & Migrations"),
                    mate("tm_pip", "Pip", "Docs & QA")
                ],
                "missionOwners": {},
                "settings": {"relay": true, "relayHopCap": 6}
            }),
        )
        .unwrap();
    let _g = hooks::install(store.clone(), fake("relay-prose-return"));

    let first = send(job(&t, None, "Ask Pip about docs port")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        if crate::invariants::lock(&ACTIVE_TEAMMATES).is_empty()
            && crate::people::hub_of(&store, "tm_pip").is_some()
            && real_tip(&first.conversation_id).ok().as_deref() != Some(first.conversation_id.as_str())
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !( crate::invariants::lock(&ACTIVE_TEAMMATES).is_empty()
        && crate::people::hub_of(&store, "tm_pip").is_some()
        && real_tip(&first.conversation_id).ok().as_deref() != Some(first.conversation_id.as_str()) )
    {
        panic!(
            "prose return stuck: owns={} active={:?} hub={:?} tip={:?} notices={:?} prompts={:?}",
            owns(&first.conversation_id),
            *crate::invariants::lock(&ACTIVE_TEAMMATES),
            crate::people::hub_of(&store, "tm_pip"),
            real_tip(&first.conversation_id),
            *crate::invariants::lock(&RELAY_NOTICES),
            fs::read_to_string(t.0.join("work/relay-prompts.jsonl")),
        );
    }

    let pip_hub = crate::people::hub_of(&store, "tm_pip").unwrap();
    wait("Pip hub finished", || !owns(&pip_hub));
    let codec_tip = real_tip(&first.conversation_id).unwrap();
    wait("Codec follow-up finished", || !owns(&codec_tip));
    let codec_tip_snap = crate::ledger::read_transcript(store.write_path(&codec_tip).unwrap());
    assert!(codec_tip_snap.items.iter().any(|item| matches!(item, Item::Assistant { text, .. } if text == "Recorded 5173.")));
}

#[test]
fn relay_defers_while_busy_and_records_cap_diagnostic() {
    let t = Temp::new();
    let store = t.store();
    let mate = |id: &str, name: &str, role: &str| {
        json!({
            "teammateId": id,
            "name": name,
            "hue": "blue",
            "role": "Custom",
            "roleTitle": role,
            "createdAt": "2026-11-01T00:00:00.000Z",
            "avatar": {},
            "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}
        })
    };
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [
                    mate("tm_codec", "Codec", "Code & Migrations"),
                    mate("tm_pip", "Pip", "Docs & QA")
                ],
                "missionOwners": {},
                "settings": {"relay": true, "relayHopCap": 2}
            }),
        )
        .unwrap();
    // 1) Busy-defer end-to-end: Pip runs a slow turn while Codec shares to Pip; Codec sees held notice,
    // and as soon as Pip finishes, Pip's deferred relay turn starts automatically and returns the review.
    let busy_gate = t.0.join("work").join("pip-busy.gate");
    let release_gate = t.0.join("work").join("pip-release.gate");
    fs::write(&busy_gate, "hold").unwrap();
    {
        let _g = hooks::install(store.clone(), fake("relay-busy"));
        let mut pip_job = job(&t, None, "Check the lock first");
        pip_job.teammate.id = "tm_pip".into();
        pip_job.teammate.name = "Pip".into();
        let pip_first = send(pip_job).unwrap();
        let pip_prompt_file = t.0.join("work").join("relay-prompts.jsonl");
        wait("Pip first turn to enter turn/start", || {
            pip_prompt_file.is_file() && owns(&pip_first.conversation_id)
        });

        let codec_run = send(job(&t, None, "Ask Pip to review migration 4")).unwrap();
        finished(&codec_run.conversation_id);
        wait("busy defer notice", || {
            relay_live_state(&[codec_run.conversation_id.clone()])
                .notices
                .iter()
                .any(|n| n.message.contains("Pip is part-way through another mission"))
        });
        assert!(crate::people::hub_of(&store, "tm_pip").is_none());

        fs::write(&release_gate, "go").unwrap();
        wait("deferred relay to finish on Pip and return to Codec", || {
            crate::invariants::lock(&ACTIVE_TEAMMATES).is_empty()
                && crate::people::hub_of(&store, "tm_pip").is_some()
                && real_tip(&codec_run.conversation_id).ok().as_deref() != Some(codec_run.conversation_id.as_str())
        });
    }

    // 2) Verify cap=2 stops automatic replies and records `host.relay_ended` diagnostic on the ledger.
    let _ = fs::remove_file(&busy_gate);
    let _ = fs::remove_file(&release_gate);
    let t2 = Temp::new();
    let store2 = t2.store();
    store2
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [
                    mate("tm_codec", "Codec", "Code & Migrations"),
                    mate("tm_pip", "Pip", "Docs & QA")
                ],
                "missionOwners": {},
                "settings": {"relay": true, "relayHopCap": 2}
            }),
        )
        .unwrap();
    let _g = hooks::install(store2.clone(), fake("relay-cap"));
    let cap_run = send(job(&t2, None, "Start exchange")).unwrap();
    wait("cap=2 exchange to settle cleanly", || {
        crate::invariants::lock(&ACTIVE_TEAMMATES).is_empty()
            && crate::people::hub_of(&store2, "tm_pip").is_some()
            && !owns(&cap_run.conversation_id)
            && real_tip(&cap_run.conversation_id).ok().as_deref() != Some(cap_run.conversation_id.as_str())
    });
    wait("hop 2 finished and cap diagnostic recorded", || {
        let codec_tip = real_tip(&cap_run.conversation_id).unwrap();
        let pip_hub = crate::people::hub_of(&store2, "tm_pip").unwrap_or_default();
        let snap1 = crate::ledger::read_transcript(store2.write_path(&codec_tip).unwrap());
        let snap2 = if pip_hub.is_empty() {
            Default::default()
        } else {
            crate::ledger::read_transcript(store2.write_path(&pip_hub).unwrap())
        };
        let has_note = |s: &crate::ledger::Transcript| {
            s.items.iter().any(|i| matches!(i, Item::Note { level, text } if level.is_empty() && text.contains("Stopped after 2 automatic replies.")))
        };
        has_note(&snap1) || has_note(&snap2)
    });
}

#[test]
fn relay_route_switch_records_left_out_by_you_and_decodes_in_snapshot() {
    let t = Temp::new();
    let store = t.store();
    let mate = |id: &str, name: &str, role: &str| {
        json!({
            "teammateId": id,
            "name": name,
            "hue": "blue",
            "role": "Custom",
            "roleTitle": role,
            "createdAt": "2026-11-01T00:00:00.000Z",
            "avatar": {},
            "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}
        })
    };
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [mate("tm_codec", "Codec", "Code & Migrations")],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();
    let _g = hooks::install(store.clone(), fake("cards-next"));

    let first = send(job(&t, None, "Clean the Linux cache too: Do the same for the AppImage cache.")).unwrap();
    finished(&first.conversation_id);

    let sw = route_switch(&store, &first.conversation_id, "Continue", &[String::from("summary")]).unwrap();
    assert_eq!(sw.link["leftOutByYou"], json!(["summary"]));
    let mut writer = LedgerWriter::create_runtime_with(
        &store,
        "mission_sw_leftout",
        "Continue",
        "sonnet",
        "claude",
        "claude",
        &[],
        None,
        &json!({
            "continuesFrom": sw.link,
        }),
    )
    .unwrap();
    writer.append("run.completed", json!({}), None).unwrap();
    let snap = crate::ledger::read_transcript(store.write_path("mission_sw_leftout").unwrap());
    assert_eq!(snap.metadata.unwrap().left_out_by_you, vec!["summary".to_owned()]);
    assert_eq!(crate::carry_on::left_out_in_words(&[String::from("summary")]), "its last reply");
}

/// teammate-keys.ts in a run: a teammate's key reaches its Claude Code run as an environment variable, and what the run
/// says with it is recorded with the value taken out. A Codex run, which drops key-named variables, gets none.
#[test]
fn a_teammates_key_reaches_its_claude_run_and_never_its_record() {
    // The same sealing as own_models' tests, so a test of theirs running beside this one opens what it sealed.
    struct Reversed;
    impl crate::own_models::SecretBox for Reversed {
        fn available(&self) -> bool {
            true
        }
        fn seal(&self, text: &str) -> Result<String, String> {
            Ok(format!("sealed:{}", text.chars().rev().collect::<String>()))
        }
        fn open(&self, sealed: &str) -> Result<String, String> {
            Ok(sealed.trim_start_matches("sealed:").chars().rev().collect())
        }
    }
    const VALUE: &str = "locust-made-up-key-value-0042";
    let t = Temp::new();
    let store = t.store();
    crate::own_models::use_secrets(std::sync::Arc::new(Reversed));
    crate::teammate_keys::for_store(&store).set("tm_claude", "STRIPE_KEY", VALUE).unwrap();
    // A Claude Code that says back the key its run was given, in stream-json.
    let fake = t.0.join("claude_says_key.py");
    fs::write(
        &fake,
        r#"import json, os, sys
def send(v):
    print(json.dumps(v), flush=True)
sys.stdin.readline()
send({"type": "system", "subtype": "init", "session_id": "33b21513-a4fb-478d-ad84-21cb7ef30571", "cwd": "/tmp", "tools": ["Read"], "mcp_servers": [], "model": "synthetic-model", "permissionMode": "default", "slash_commands": [], "claude_code_version": "2.1.252", "agents": [], "skills": []})
said = "The key is " + os.environ.get("STRIPE_KEY", "missing")
send({"type": "assistant", "message": {"id": "msg_k", "content": [{"type": "text", "text": said}]}})
send({"type": "result", "subtype": "success", "is_error": False, "result": said})
try:
    sys.stdin.read()
except Exception:
    pass
"#,
    )
    .unwrap();
    // Through the guard: it holds the lock every test that points the engine at a store takes.
    let _g = hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let mate = Teammate {
        id: "tm_claude".into(),
        name: "Claude".into(),
        role: "Code & Migrations".into(),
        runtime: Some("claude".into()),
        model: Some("sonnet".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    };
    fs::create_dir_all(t.0.join("work")).unwrap();
    let run = send(Send { teammate: mate, ..job(&t, None, "Say the key") }).unwrap();
    wait("the keyed turn finishes", || !owns(&run.conversation_id));
    let ledger = fs::read_to_string(path_of(&t, &run.conversation_id)).unwrap();
    assert!(ledger.contains("The key is [key STRIPE_KEY]"), "the run had the key, and the record has its name: {ledger}");
    assert!(!ledger.contains(VALUE) && !ledger.contains("missing"), "never the value");
    // A removed teammate's keys go with it.
    crate::people::remove_teammate(&store, "tm_claude").unwrap();
    assert!(crate::teammate_keys::for_store(&store).run_keys("tm_claude").is_empty());
}

/// a-teammate-hears-its-turn-was-undone.test.ts: a follow-up to a turn the person undid starts with what Undo put back,
/// and still ends with the person's words (the fake fails the turn otherwise); a follow-up to one not undone says
/// nothing of Undo (the plain resume scenario fails on it).
#[test]
fn a_follow_up_to_an_undone_turn_starts_with_what_undo_put_back() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let first = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&first.conversation_id);
    let folder = codex(&t).folder.map(PathBuf::from).unwrap();
    crate::checkpoints::put_receipt(&t.store(), &folder, &first.conversation_id, &json!({"restored": ["laptop-setup.md"], "leftAlone": []}));
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("resume-undone"));
    let second = send(job(&t, Some(&first.conversation_id), "Synthetic follow-up")).unwrap();
    finished(&second.conversation_id);
    let child = ledgers(&t).into_iter().find(|p| *p != path_of(&t, &first.conversation_id)).unwrap();
    assert_eq!(kinds(&child).last().map(String::as_str), Some("run.completed"), "{:?}", records(&child));
    let said = records(&child)[0]["metadata"]["prompt"].as_str().unwrap().to_owned();
    assert_eq!(said, "Synthetic follow-up", "the record keeps the person's words; the note goes to the runtime only");
    // The turn after that follows a turn that was not undone, and is told nothing.
    let tip = second.conversation_id.clone();
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("resume"));
    let third = send(job(&t, Some(&tip), "Synthetic follow-up")).unwrap();
    finished(&third.conversation_id);
    let newest = ledgers(&t).into_iter().max_by_key(|p| std::fs::metadata(p).unwrap().modified().unwrap()).unwrap();
    assert_eq!(kinds(&newest).last().map(String::as_str), Some("run.completed"), "{:?}", records(&newest));
}


/// Two native turns of one conversation (the second resumes the first), their ledgers, and the turn ids.
fn two_turns_started(t: &Temp) -> (String, String) {
    let first = send(job(t, None, "Synthetic prompt")).unwrap();
    finished(&first.conversation_id);
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("resume"));
    let second = send(job(t, Some(&first.conversation_id), "Synthetic follow-up")).unwrap();
    finished(&second.conversation_id);
    let tip = real_tip(&first.conversation_id).unwrap();
    assert_ne!(tip, first.conversation_id, "the second turn is a file of its own");
    (first.conversation_id, tip)
}

fn ledger_of(t: &Temp, id: &str) -> Vec<Value> {
    records(&path_of(t, id))
}

fn rewind_job(t: &Temp, conversation: &str, tip: &str, before: Option<&str>, set_aside: &[&str]) -> (Send, Origin) {
    (
        job(t, Some(conversation), "Synthetic follow-up"),
        Origin {
            rewind: Some(Rewind { tip: tip.into(), before: before.map(str::to_owned), set_aside: set_aside.iter().map(|s| (*s).to_owned()).collect() }),
            ..Default::default()
        },
    )
}

/// an-earlier-message-can-be-edited: sending an edit starts the conversation again from the turn BEFORE the edited one
/// (a cold start told what was said up to there: the fake fails the turn on a resume or without the brief), the new turn
/// says in its record that it is an edit, keeps the person's words, and the set-aside turn is left exactly as it was.
#[test]
fn an_edit_starts_again_from_the_turn_before_it_and_leaves_the_set_aside_turn_as_it_was() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let (first, second) = two_turns_started(&t);
    let aside = fingerprint(&t.store().ledger_dir().unwrap());
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("rewind"));
    let (send_job, origin) = rewind_job(&t, &first, &second, Some(&first), &[&second]);
    let edit = send_from(send_job, origin).unwrap();
    finished(&edit.conversation_id);
    assert_ne!(edit.conversation_id, first, "the new branch is a conversation of its own, whose newest turn is this one");
    let records = ledger_of(&t, &edit.conversation_id);
    assert_eq!(kinds(&path_of(&t, &edit.conversation_id)).last().map(String::as_str), Some("run.completed"), "{records:?}");
    let meta = &records[0]["metadata"];
    assert_eq!(meta["continuesFrom"], json!({"missionId": first, "checkpointEpoch": 1, "reason": "follow-up", "edited": true}), "no runtimeThreadId: nothing was resumed");
    assert_eq!(meta["prompt"], "Synthetic follow-up", "the record keeps the person's words; the brief goes to the runtime only");
    // Nothing deleted: the turns the edit set aside are as they were.
    let after = fingerprint(&t.store().ledger_dir().unwrap());
    for kept in &aside {
        assert!(after.contains(kept), "{:?} changed or went", kept.0);
    }
    // The newest turn of the old version and the edit are told apart by the ledger alone.
    let heads = crate::ledger::try_list_recent(&t.store().ledger_dir().unwrap(), 10).unwrap();
    let link = |s: &crate::ledger::Summary| crate::edit_earlier::TurnLink {
        mission_id: s.mission_id.clone(),
        created_at: s.created_at.clone(),
        parent: s.parent.clone(),
        follow_up: s.continues_reason.as_deref() == Some("follow-up"),
        edited: s.edited,
    };
    let links: Vec<_> = heads.iter().map(link).collect();
    assert_eq!(crate::edit_earlier::turn_versions(&edit.conversation_id, &links).and_then(|v| v.before), Some(second.clone()));
    assert_eq!(crate::edit_earlier::turn_versions(&second, &links).and_then(|v| v.after), Some(edit.conversation_id.clone()));
}

/// Editing the FIRST message has no turn before it: a new conversation, no link, and nothing of the old one retired.
/// An edit partway through retires what the set-aside turns sent and nobody read (0.512).
#[test]
fn an_edit_retires_what_set_aside_turns_sent_only_when_it_is_partway_through() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let (first, second) = two_turns_started(&t);
    let room = crate::workroom::Workroom::for_store(&t.store());
    let from = crate::workroom::Party { teammate_id: "tm_codec".into(), name: "Codec".into() };
    let to = crate::workroom::Party { teammate_id: "tm_pip".into(), name: "Pip".into() };
    let stale = room.post(&from, &second, &to, "Rewrite the README for JSON.", None).unwrap();
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("success"));
    // The first message edited: its prompt starts a conversation of its own, and the message waits.
    let (mut send_job, origin) = rewind_job(&t, &first, &second, None, &[&first, &second]);
    send_job.prompt = "Synthetic prompt".into();
    let again = send_from(send_job, origin).unwrap();
    finished(&again.conversation_id);
    assert!(ledger_of(&t, &again.conversation_id)[0]["metadata"].get("continuesFrom").is_none(), "no turn before it to continue");
    assert_eq!(room.unread("tm_pip", 10, &[]).unwrap().messages.len(), 1, "editing the first message keeps the old conversation: nothing retired");
    // An edit partway through: the message the set-aside turn sent is never delivered.
    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("rewind"));
    let (send_job, origin) = rewind_job(&t, &first, &second, Some(&first), &[&second]);
    let edit = send_from(send_job, origin).unwrap();
    finished(&edit.conversation_id);
    assert!(room.unread("tm_pip", 10, &[]).unwrap().messages.is_empty(), "retired");
    let delivery = room.read().deliveries.into_iter().find(|d| d.message_id == stale.message_id).unwrap();
    assert_eq!(delivery.mission_id, second, "recorded as back with the turn that sent it");
}

/// An edit replaces the conversation's newest turn, which must not be running; one that is refuses, and nothing is made.
#[test]
fn an_edit_is_refused_while_the_turn_it_replaces_is_running() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("interrupt"));
    let running = send(job(&t, None, "Synthetic prompt")).unwrap();
    wait("the turn to run", || owns(&running.conversation_id));
    let made = ledgers(&t).len();
    let (send_job, origin) = rewind_job(&t, "some_other_anchor", &running.conversation_id, Some("some_turn"), &[]);
    let refused = send_from(send_job, origin).unwrap_err();
    assert!(refused.contains("still running"), "{refused}");
    assert_eq!(ledgers(&t).len(), made, "nothing was started");
    stop(&running.conversation_id).unwrap();
    finished(&running.conversation_id);
}

// ---- a runtime's own slash commands, sent as typed (0.426-0.428; a-runtime-command-is-sent-as-typed.test.ts) ---------
// Every turn used to be wrapped in the brief, so `/compact` arrived as the last line of a long prompt and was just words.

/// A Claude Code that writes every message it is sent to `claude-sent.jsonl` in its folder, and lists its commands at the
/// start of the run, as the real one does (one of them a command that would change the person's own setup).
fn commands_claude(t: &Temp) -> PathBuf {
    let fake = t.0.join("claude_lists_commands.py");
    fs::write(
        &fake,
        r#"import json, sys
def send(v):
    print(json.dumps(v), flush=True)
line = sys.stdin.readline()
sent = json.loads(line)["message"]["content"]
with open("claude-sent.jsonl", "a") as log:
    log.write(json.dumps(sent) + "\n")
send({"type": "system", "subtype": "init", "session_id": "33b21513-a4fb-478d-ad84-21cb7ef30571", "cwd": "/tmp", "tools": ["Read"], "mcp_servers": [], "model": "synthetic-model", "permissionMode": "default", "slash_commands": ["compact"], "claude_code_version": "2.1.252", "agents": [], "skills": []})
send({"type": "system", "subtype": "commands_changed", "commands": [
    {"name": "compact", "description": "Clear conversation history but keep a summary", "argumentHint": "<optional custom summarization instructions>"},
    {"name": "clear", "description": "Clear conversation history", "argumentHint": ""},
    {"name": "model", "description": "Set the AI model", "argumentHint": "[model]"},
    {"name": "security-review", "description": "Complete a security review of the pending changes", "argumentHint": ""}]})
send({"type": "assistant", "message": {"id": "msg_c", "content": [{"type": "text", "text": "OK"}]}})
send({"type": "result", "subtype": "success", "is_error": False, "result": "OK"})
try:
    sys.stdin.read()
except Exception:
    pass
"#,
    )
    .unwrap();
    fake
}

fn claude_mate(t: &Temp) -> Teammate {
    fs::create_dir_all(t.0.join("work")).unwrap();
    Teammate {
        id: "tm_claude".into(),
        name: "Claude".into(),
        role: "Code & Migrations".into(),
        runtime: Some("claude".into()),
        model: Some("sonnet".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    }
}

fn claude_job(t: &Temp, conversation: Option<&str>, prompt: &str) -> Send {
    Send { teammate: claude_mate(t), conversation: conversation.map(str::to_owned), prompt: prompt.into(), model: None, effort: None }
}

/// What the fake runtime was sent, oldest first, from its log in the work folder.
fn sent_log(t: &Temp, file: &str) -> Vec<Value> {
    let path = t.0.join("work").join(file);
    fs::read_to_string(path).map(|text| text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()).unwrap_or_default()
}

fn claude_lists(store: &Store, names: &[&str]) {
    let rows: Vec<crate::runtime_commands::CommandInfo> = names
        .iter()
        .map(|n| crate::runtime_commands::CommandInfo { name: (*n).into(), description: String::new(), argument_hint: String::new() })
        .collect();
    crate::runtime_commands::for_store(store).set("claude", &rows).unwrap();
}

/// What a briefed message looks like: a long brief before the person's words, the references line after.
fn briefed(sent: &str, words: &str) -> bool {
    sent != words && sent.len() > words.len() + 100 && sent.contains(&format!("\n\n{words}\n\nReference folders"))
}

#[test]
fn a_command_the_person_typed_goes_to_the_runtime_alone_with_no_brief_and_no_plan_instruction_after_it() {
    let t = Temp::new();
    let store = t.store();
    claude_lists(&store, &["compact", "clear"]);
    let fake = commands_claude(&t);
    let _g = hooks::install(store, Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let mate = Teammate { runs_in: "plan".into(), ..claude_mate(&t) };
    let typed = send(Send { teammate: mate, ..claude_job(&t, None, "  /compact keep the test names  ") }).unwrap();
    finished(&typed.conversation_id);
    assert_eq!(sent_log(&t, "claude-sent.jsonl"), vec![json!("/compact keep the test names")]);
    // The record keeps what the person typed, as for any message.
    let first = &records(&path_of(&t, &typed.conversation_id))[0];
    assert_eq!(first["metadata"]["prompt"], "  /compact keep the test names  ");
}

#[test]
fn a_command_is_briefed_like_any_message_when_the_words_came_from_a_routine_whatever_the_words_say() {
    let t = Temp::new();
    let store = t.store();
    claude_lists(&store, &["compact"]);
    let fake = commands_claude(&t);
    let _g = hooks::install(store, Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let origin = Origin { started_by: Some(json!({"kind": "routine", "routineId": "rt_1", "step": 1})), ..Default::default() };
    let replayed = send_from(claude_job(&t, None, "/compact"), origin).unwrap();
    finished(&replayed.conversation_id);
    let sent = sent_log(&t, "claude-sent.jsonl");
    assert!(briefed(sent[0].as_str().unwrap(), "/compact"), "{sent:?}");
}

#[test]
fn a_command_is_briefed_like_any_message_when_the_runtime_never_listed_it() {
    let t = Temp::new();
    let store = t.store();
    // Nothing listed yet (and not even Claude Code's own list on disk): `/compact` is only words.
    let fake = commands_claude(&t);
    let _g = hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let first = send(claude_job(&t, None, "/compact")).unwrap();
    finished(&first.conversation_id);
    assert!(briefed(sent_log(&t, "claude-sent.jsonl")[0].as_str().unwrap(), "/compact"));
    // And a name another runtime lists is not this runtime's command.
    crate::runtime_commands::for_store(&store)
        .set("opencode", &[crate::runtime_commands::CommandInfo { name: "init".into(), description: String::new(), argument_hint: String::new() }])
        .unwrap();
    let other = send(claude_job(&t, None, "/init")).unwrap();
    finished(&other.conversation_id);
    assert!(briefed(sent_log(&t, "claude-sent.jsonl")[1].as_str().unwrap(), "/init"));
}

#[test]
fn a_command_leaves_the_next_turn_briefed_in_full_since_it_may_have_cleared_or_compacted_the_session() {
    let t = Temp::new();
    let store = t.store();
    claude_lists(&store, &["clear"]);
    let fake = commands_claude(&t);
    let _g = hooks::install(store, Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let first = send(claude_job(&t, None, "Look around.")).unwrap();
    finished(&first.conversation_id);
    let command = send(claude_job(&t, Some(&first.conversation_id), "/clear")).unwrap();
    finished(&command.conversation_id);
    let next = send(claude_job(&t, Some(&command.conversation_id), "What next?")).unwrap();
    finished(&next.conversation_id);
    let sent = sent_log(&t, "claude-sent.jsonl");
    assert!(briefed(sent[0].as_str().unwrap(), "Look around."));
    assert_eq!(sent[1], "/clear");
    assert!(briefed(sent[2].as_str().unwrap(), "What next?"), "{sent:?}");
}

#[test]
fn claude_code_lists_its_commands_at_the_start_of_a_run_kept_once_outside_the_record_and_without_the_hidden_ones() {
    let t = Temp::new();
    let store = t.store();
    let fake = commands_claude(&t);
    let _g = hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned()])));
    let run = send(claude_job(&t, None, "Look around.")).unwrap();
    finished(&run.conversation_id);
    let listed = crate::runtime_commands::for_store(&store).list();
    let names: Vec<&str> = listed["claude"].iter().map(|c| c.name.as_str()).collect();
    // `model` is the teammate's, chosen in Locust: left out of the menu, not out of what Claude Code sent.
    assert_eq!(names, vec!["compact", "clear", "security-review"]);
    assert_eq!(listed["claude"][0].argument_hint, "<optional custom summarization instructions>");
    let ledger = fs::read_to_string(path_of(&t, &run.conversation_id)).unwrap();
    assert!(!ledger.contains("security review") && !ledger.contains("Set the AI model"), "the list is not in the mission's record");
    // The next run says the same thing: nothing changes, and a message that starts with one is a command now.
    assert!(!crate::runtime_commands::keep_learned("claude", &crate::runtime_commands::for_store(&store).held_for_test("claude")));
    assert!(crate::runtime_commands::for_store(&store).is_command(Some("claude"), "/security-review the auth code"));
}

/// The fake Codex app-server's requests, in order.
fn codex_requests(t: &Temp) -> Vec<Value> {
    sent_log(t, "commands-requests.jsonl")
}
fn codex_commands_hook(t: &Temp) -> Option<(PathBuf, Vec<String>)> {
    let _ = t;
    Some((PathBuf::from("python3"), vec![concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/commands-app-server.py").into()]))
}
fn methods(requests: &[Value]) -> Vec<&str> {
    requests.iter().map(|r| r["method"].as_str().unwrap()).collect()
}

#[test]
fn codexs_review_goes_as_its_own_request_with_what_follows_as_the_instructions_and_compact_as_its_own() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    fs::create_dir_all(t.0.join("work")).unwrap();
    let review = send(job(&t, None, "/review only the tests")).unwrap();
    finished(&review.conversation_id);
    let asked = codex_requests(&t);
    assert_eq!(methods(&asked), vec!["initialize", "config/read", "thread/start", "review/start"]);
    let started = &asked[2]["params"];
    assert_eq!(started["model"], "synthetic-model", "neither request takes a model: the thread is started with it");
    assert_eq!(asked[3]["params"], json!({"threadId": "thread_fake", "target": {"type": "custom", "instructions": "only the tests"}, "delivery": "inline"}));
    // With nothing after it, the review is of the uncommitted changes.
    let bare = send(job(&t, None, "/review")).unwrap();
    finished(&bare.conversation_id);
    let asked = codex_requests(&t);
    assert_eq!(asked.last().unwrap()["params"]["target"], json!({"type": "uncommittedChanges"}));
    // A compaction is a request of its own, never a turn.
    let compact = send(job(&t, None, "/compact")).unwrap();
    finished(&compact.conversation_id);
    let asked = codex_requests(&t);
    assert_eq!(asked.last().unwrap()["method"], "thread/compact/start");
    assert_eq!(asked.last().unwrap()["params"], json!({"threadId": "thread_fake"}));
    assert!(!methods(&asked).contains(&"turn/start"));
    assert_eq!(kinds(&path_of(&t, &compact.conversation_id)).last().map(String::as_str), Some("run.completed"));
}

#[test]
fn codexs_init_is_a_turn_carrying_locusts_words_for_it_and_a_command_it_has_no_request_for_is_briefed() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    fs::create_dir_all(t.0.join("work")).unwrap();
    let init = send(job(&t, None, "/init keep it short")).unwrap();
    finished(&init.conversation_id);
    let turn = codex_requests(&t).into_iter().find(|r| r["method"] == "turn/start").unwrap();
    let text = turn["params"]["input"][0]["text"].as_str().unwrap();
    assert_eq!(text, format!("{}\n\nkeep it short", crate::runtime_commands::CODEX_INIT_PROMPT));
    // `/model` is Locust's own control, not a Codex command: an ordinary briefed message.
    let model = send(job(&t, None, "/model gpt-6-luna")).unwrap();
    finished(&model.conversation_id);
    let turns: Vec<Value> = codex_requests(&t).into_iter().filter(|r| r["method"] == "turn/start").collect();
    assert!(briefed(turns[1]["params"]["input"][0]["text"].as_str().unwrap(), "/model gpt-6-luna"));
    // A routine's `/compact` is the same: words, in a turn.
    let origin = Origin { started_by: Some(json!({"kind": "routine", "routineId": "rt_1", "step": 1})), ..Default::default() };
    let replayed = send_from(job(&t, None, "/compact"), origin).unwrap();
    finished(&replayed.conversation_id);
    assert_eq!(methods(&codex_requests(&t)).iter().filter(|m| **m == "thread/compact/start").count(), 0);
}

fn opencode_mate(t: &Temp) -> Teammate {
    fs::create_dir_all(t.0.join("work")).unwrap();
    Teammate { runtime: Some("opencode".into()), model: Some("opencode/beam-free".into()), id: "tm_open".into(), name: "Open".into(), ..codex(t) }
}

fn opencode_hook(scenario: &str) -> Option<(PathBuf, Vec<String>)> {
    Some((
        PathBuf::from("python3"),
        vec![concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m4-opencode-serve.py").into(), "serve".into(), scenario.into()],
    ))
}

fn opencode_job(t: &Temp, prompt: &str) -> Send {
    Send { teammate: opencode_mate(t), conversation: None, prompt: prompt.into(), model: None, effort: None }
}

#[test]
fn a_command_goes_to_opencode_by_name_with_only_what_follows_the_name_as_its_message() {
    let t = Temp::new();
    let store = t.store();
    let rows: Vec<_> = ["init", "review"]
        .iter()
        .map(|n| crate::runtime_commands::CommandInfo { name: (*n).into(), description: String::new(), argument_hint: String::new() })
        .collect();
    crate::runtime_commands::for_store(&store).set("opencode", &rows).unwrap();
    let _g = hooks::install(store, opencode_hook("success"));
    let typed = send(opencode_job(&t, "/init keep it under five lines")).unwrap();
    finished(&typed.conversation_id);
    // With nothing after it, nothing is the message.
    let bare = send(opencode_job(&t, "/review")).unwrap();
    finished(&bare.conversation_id);
    let asked = sent_log(&t, "opencode-requests.jsonl");
    assert_eq!(asked[0], json!({"path": "command", "body": {"command": "init", "arguments": "keep it under five lines", "model": "opencode/beam-free"}}));
    assert_eq!(asked[1]["body"]["command"], "review");
    assert_eq!(asked[1]["body"]["arguments"], "");
    // Not typed as a command: an ordinary briefed run, through the prompt and not the command call.
    let origin = Origin { started_by: Some(json!({"kind": "routine", "routineId": "rt_1", "step": 1})), ..Default::default() };
    let replayed = send_from(opencode_job(&t, "/init"), origin).unwrap();
    finished(&replayed.conversation_id);
    let asked = sent_log(&t, "opencode-requests.jsonl");
    assert_eq!(asked[2]["path"], "prompt_async");
    assert!(briefed(asked[2]["body"]["parts"][0]["text"].as_str().unwrap(), "/init"));
}

#[test]
fn a_command_opencodes_server_refuses_ends_the_run_with_its_reason() {
    let t = Temp::new();
    let store = t.store();
    crate::runtime_commands::for_store(&store)
        .set("opencode", &[crate::runtime_commands::CommandInfo { name: "init".into(), description: String::new(), argument_hint: String::new() }])
        .unwrap();
    let _g = hooks::install(store, opencode_hook("command-refused"));
    let typed = send(opencode_job(&t, "/init")).unwrap();
    finished(&typed.conversation_id);
    let kinds = kinds(&path_of(&t, &typed.conversation_id));
    assert!(kinds.contains(&"run.failed".to_owned()), "{kinds:?}");
    let ledger = fs::read_to_string(path_of(&t, &typed.conversation_id)).unwrap();
    assert!(ledger.contains("refused the command"), "{ledger}");
}

#[test]
fn follow_up_turn_in_grouped_conversation_receives_group_instructions() {
    let t = Temp::new();
    let store = t.store();
    let prompt_log = t.0.join("group_prompts.jsonl");
    let wrapper = t.0.join("group_prompt_wrapper.py");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py");
    let py = r#"import json, subprocess, sys
log_path, fixture_path, scenario = sys.argv[1:4]
proc = subprocess.Popen(
    [sys.executable, fixture_path, scenario] + sys.argv[4:],
    stdin=subprocess.PIPE,
    stdout=sys.stdout,
    stderr=sys.stderr,
    text=True,
    bufsize=1,
)
for line in sys.stdin:
    try:
        msg = json.loads(line)
        if msg.get('method') == 'turn/start':
            txt = (msg.get('params', {}).get('input') or [{}])[0].get('text', '')
            with open(log_path, 'a', encoding='utf-8') as f:
                f.write(json.dumps(txt) + '\n')
    except Exception:
        pass
    proc.stdin.write(line)
    proc.stdin.flush()
proc.stdin.close()
sys.exit(proc.wait())
"#;
    fs::write(&wrapper, py).unwrap();
    let wrap = |scenario: &str| {
        Some((
            PathBuf::from("python3"),
            vec![
                wrapper.to_string_lossy().into_owned(),
                prompt_log.to_string_lossy().into_owned(),
                fixture.into(),
                scenario.into(),
            ],
        ))
    };

    let _g = hooks::install(store.clone(), wrap("success"));
    let first = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&first.conversation_id);

    let grp = crate::groups::create_group(&store, "Trading").unwrap();
    crate::groups::set_group_instructions(&store, &grp.group_id, "Never bypass stops.")
        .unwrap();
    crate::groups::assign_to_group(&store, &first.conversation_id, Some(&grp.group_id)).unwrap();

    *crate::invariants::lock(&hooks::STATE) = (Some(store), wrap("resume"));
    let second = send(job(&t, Some(&first.conversation_id), "Synthetic follow-up")).unwrap();
    finished(&second.conversation_id);

    let captured: Vec<String> = fs::read_to_string(&prompt_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<String>(l).unwrap())
        .collect();
    assert_eq!(captured.len(), 2);
    assert!(!captured[0].contains("Never bypass stops."));
    assert!(captured[1].contains("Trading"));
    assert!(captured[1].contains("Never bypass stops."));
}

#[test]
fn sealed_slot_sends_plain_prompt_without_tools_or_briefs_and_roundtrips_origin() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("sealed"));
    let origin = Origin {
        mode: Some(Mode::Ask),
        folder: Some(t.0.join("work")),
        started_by: Some(serde_json::json!({"kind": "scout"})),
        slot: Some(SlotOrigin {
            key: "scout:tm_scout:gpt-5.4-mini".into(),
            cwd: None,
            instructions: None,
            skill: None,
            sealed: true,
        }),
        ..Origin::default()
    };
    let started = send_from(job(&t, None, "Synthetic prompt"), origin).unwrap();
    finished(&started.conversation_id);
    let transcript = crate::ledger::read_transcript(path_of(&t, &started.conversation_id));
    assert!(!transcript.running && !transcript.failed, "transcript should complete: {transcript:?}");

    // Also verify EngineProcess origin serialization preserves SlotOrigin.sealed.
    let back = Origin {
        mode: Some(Mode::Ask),
        folder: Some(t.0.join("work")),
        started_by: Some(serde_json::json!({"kind": "scout"})),
        slot: Some(SlotOrigin {
            key: "scout:tm_1:m".into(),
            cwd: None,
            instructions: None,
            skill: None,
            sealed: true,
        }),
        ..Origin::default()
    };
    let json_val = crate::engine_process::origin_json(&back);
    assert_eq!(json_val["slot"]["sealed"], true);
    let decoded = crate::engine_process::origin_from(&json_val).unwrap();
    assert!(decoded.slot.as_ref().is_some_and(|s| s.sealed && s.key == "scout:tm_1:m"));
}
