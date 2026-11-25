//! `a-beta-can-report-itself.test.ts`, the window's half: Settings > Report a problem, Show the log, the panic hook
//! `main` starts before the window opens, and the missions this run deleted, checked on every history read. Electron's
//! cases read `index.ts` and `Screens.tsx`; these draw the section, press it, delete through the one delete the window
//! takes, read the history as the window does, and read the log left in a temporary data folder.
use super::*;
use crate::store::Store;
use locust_core::diagnostic_log::{self, DeletedThisSession, LOG_FILE};
use rune::{Env, Runtime};
use crate::{native, poll, refresh_recent, Kind};
use std::path::{Path, PathBuf};

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static HOOK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn data() -> (Dirs, Store, native::hooks::Guard) {
    let root = std::env::temp_dir().join(format!(
        "locust-report-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    let hook = native::hooks::install(store.clone(), None);
    (Dirs(root), store, hook)
}

fn log(root: &Path) -> String {
    std::fs::read_to_string(root.join(LOG_FILE)).unwrap_or_default()
}

fn section(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| settings_section(s, &Palette::new(true)), (900.0, 600.0), Env::new(true, 1.0));
    for _ in 0..10 {
        rt.frame(0.05);
    }
    rt
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n")
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

/// A finished record on disk, as the ledger writes one, with a prompt that must never reach the log.
fn record(store: &Store, id: &str, prompt: &str) {
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let created = serde_json::json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": at,
        "metadata": {"missionId": id, "runId": format!("run_{id}"), "prompt": prompt, "runtime": "codex",
            "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
            "cliVersion": "test", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": at}});
    let ended = serde_json::json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": at,
        "event": {"id": "e1", "runId": format!("run_{id}"), "missionId": id, "sequence": 1, "occurredAt": at,
            "sourceAdapter": "codex", "type": "run.completed", "payload": {}}});
    std::fs::write(store.ledger_dir().unwrap().join(format!("{id}.jsonl")), format!("{created}\n{ended}\n")).unwrap();
}

/// Read the history as the window does: `refresh_recent`, then `poll` until the read lists `id` (or a while passes).
fn read_history(store: &Store, id: &str) -> Runtime<State> {
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    let mut rt = Runtime::new(s, |_, _| rune::col(), (400.0, 300.0), Env::new(true, 1.0));
    rt.update(|s, _| refresh_recent(s));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut now = 0.0;
    while !rt.state.recent.iter().any(|r| r.id == id) && std::time::Instant::now() < deadline {
        if !rt.state.recents.is_pending() && !rt.state.team.is_pending() {
            rt.update(|s, _| refresh_recent(s));
        }
        rt.update(move |s, tx| {
            poll(s, now, tx);
        });
        now += 0.1;
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    rt
}

/// Delete one conversation through the one delete the window takes, then forge its record back onto disk, as
/// Electron's note says the finding was proven.
fn deleted_then_back(id: &str, prompt: &str) -> (Dirs, Store, native::hooks::Guard) {
    let (dirs, store, hook) = data();
    record(&store, id, prompt);
    let mut s = State::default();
    s.data = Some(store.clone());
    crate::sidebar::delete_conversation(&mut s, &store, &[id.to_owned()]).unwrap();
    record(&store, id, prompt);
    (dirs, store, hook)
}

// ---- nothing is uploaded, which the app says on screen ----

// case: starts the crash reporter with uploading off
#[test]
fn panics_are_kept_on_this_machine_as_settings_says() {
    let (_dirs, store, _hook) = data();
    {
        let _held = HOOK.lock().unwrap_or_else(|e| e.into_inner());
        diagnostic_log::install_panic_hook(store.root().to_path_buf());
        let _ = std::thread::Builder::new().name("main".into()).spawn(|| std::panic::panic_any("window gone")).unwrap().join();
        drop(std::panic::take_hook());
    }
    assert!(log(store.root()).contains("panic: the window stopped: thread 'main' panicked at "), "kept in the data folder's own log");
    let mut s = State::default();
    s.data = Some(store.clone());
    assert!(words(&section(s)).contains("Nothing is uploaded."));
}

// case: starts it before the app is ready, since a startup crash is one worth catching
#[test]
fn main_begins_the_log_before_the_window_opens() {
    let main = include_str!("../main.rs");
    let body = &main[main.find("\nfn main() {").expect("fn main")..];
    let begun = body.find("locust_core::diagnostic_log::begin(").expect("main begins the log");
    let opened = body.find("App::new(AppSpec").expect("main opens the window");
    assert!(begun < opened, "the hook and the first line come before the window");
}

// ---- a person can find the log before they need it ----

// case: has a place in Settings that says what to send
#[test]
fn settings_has_report_a_problem_and_show_the_log() {
    let said = words(&section(State::default()));
    assert!(said.contains("Report a problem") && said.contains("Show the log"), "{said}");
}

// case: says what is in it, because sending a file is a decision
#[test]
fn settings_says_what_the_log_holds() {
    assert!(words(&section(State::default())).contains("what happened, never what was said"));
}

// case: reveals rather than opens
#[test]
fn show_the_log_reveals_the_file() {
    let (_dirs, store, _hook) = data();
    let mut s = State::default();
    s.data = Some(store.clone());
    s.feedback_dialog.suppress_os_open = true;
    let mut rt = section(s);
    press(&mut rt, "settings-show-log");
    assert_eq!(rt.state.feedback_dialog.revealed.as_deref(), Some(store.root().join(LOG_FILE).as_path()));
    assert!(rt.state.settings_panel.error.is_empty(), "revealed, not said as a path to go and open");
}

// case: takes no path from the renderer at all
#[test]
fn show_the_log_takes_no_path() {
    let (_dirs, store, _hook) = data();
    let mut s = State::default();
    s.data = Some(store.clone());
    s.feedback_dialog.suppress_os_open = true;
    s.feedback_dialog.saved_path = Some("/somewhere/else.txt".into());
    let mut rt = section(s);
    press(&mut rt, "settings-show-log");
    // The press carries nothing; the only answer is the data folder's own log.
    assert_eq!(rt.state.feedback_dialog.revealed, Some(store.root().join("locust-errors.log")));
    let mut none = State::default();
    none.feedback_dialog.suppress_os_open = true;
    let mut rt = section(none);
    press(&mut rt, "settings-show-log");
    assert_eq!(rt.state.feedback_dialog.revealed, None, "no data folder, nothing revealed");
}

// ---- a mission that comes back says so ----

// case: remembers what it deleted
#[test]
fn a_delete_is_remembered_and_written() {
    let (_dirs, store, _hook) = data();
    record(&store, "m_gone_1", "Plan the launch");
    let mut s = State::default();
    s.data = Some(store.clone());
    crate::sidebar::delete_conversation(&mut s, &store, &["m_gone_1".to_owned()]).unwrap();
    assert!(diagnostic_log::this_session().knows("m_gone_1"));
    assert!(log(store.root()).contains("mission-deleted: m_gone_1\n"));
}

// case: checks every history read against that list
#[test]
fn a_history_read_that_brings_one_back_is_written() {
    let (_dirs, store, _hook) = deleted_then_back("m_back_2", "Plan the launch");
    let rt = read_history(&store, "m_back_2");
    assert!(rt.state.recent.iter().any(|r| r.id == "m_back_2"), "the record is listed again");
    assert!(log(store.root()).contains("deleted-mission-returned: m_back_2\n"), "{}", log(store.root()));
    assert!(!diagnostic_log::this_session().knows("m_back_2"), "said once");
}

// case: records ids and never a prompt, because this file is made to be sent
#[test]
fn the_log_names_ids_never_prompts() {
    let (_dirs, store, _hook) = deleted_then_back("m_back_3", "Draft the confidential memo");
    let _rt = read_history(&store, "m_back_3");
    let text = log(store.root());
    assert!(text.contains("mission-deleted: m_back_3") && text.contains("deleted-mission-returned: m_back_3"), "{text}");
    assert!(!text.contains("confidential") && !text.contains("Draft the"), "{text}");
}

// case: forgets across restarts, since a durable list of deletions is the opposite of the point
#[test]
fn the_deleted_list_is_in_memory_only() {
    let (_dirs, store, _hook) = data();
    record(&store, "m_gone_4", "Plan the launch");
    let mut s = State::default();
    s.data = Some(store.clone());
    crate::sidebar::delete_conversation(&mut s, &store, &["m_gone_4".to_owned()]).unwrap();
    assert!(diagnostic_log::this_session().knows("m_gone_4"));
    // A new run starts with a new, empty list.
    assert!(!DeletedThisSession::default().knows("m_gone_4"));
    // Nothing in the data folder keeps the list: the id is only in the log line and the trashed record's own name.
    for entry in std::fs::read_dir(store.root()).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() && path.file_name().is_some_and(|n| n != LOG_FILE) {
            assert!(!std::fs::read_to_string(&path).unwrap_or_default().contains("m_gone_4"), "{path:?}");
        }
    }
}

