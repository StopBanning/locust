use super::*;
use rune::{Env, Runtime};
use serde_json::json;

struct Disk(std::path::PathBuf, store::Store);

impl Disk {
    fn new(saved: serde_json::Value) -> Self {
        let root = std::env::temp_dir().join(format!("locust-b25-{}", crate::invariants::unique_nanos()));
        std::fs::create_dir_all(root.join("electron")).unwrap();
        let store = store::Store::new(root.join("native"), root.join("electron")).unwrap();
        // The Electron file contract, not Rust's writer: a fresh window must restore these exact rows.
        let path = composer::queue_store::queue_path(&store).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, json!({"schemaVersion":1, "rows":saved}).to_string()).unwrap();
        Self(root, store)
    }

    fn window(&self) -> Runtime<State> {
        let mut s = State::default();
        s.engine = Some(Kind::Native);
        s.data = Some(self.1.clone());
        s.queue_rows = composer::read_queue(&self.1).unwrap();
        s.teammates.push(Teammate {
            id: "tm_wren".into(), name: "Wren".into(), role: "Code".into(),
            runtime: Some("codex".into()), model: Some("gpt-5.6-luna".into()),
            account: None, busy: false, runs_in: "ask".into(), folder: Some(self.0.to_string_lossy().into_owned()),
        });
        s.conversations.insert("tm_wren".into(), Conversation {
            conversation_id: Some("run_one".into()), ..Default::default()
        });
        fn draw(s: &State, _: &mut Cx<State>) -> El {
            crate::thread_view(s, &Palette::new(true))
        }
        let mut rt = Runtime::new(s, draw, (1000.0, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    }

    fn read(&self) -> Vec<composer::QueuedRow> {
        composer::read_queue(&self.1).unwrap()
    }
}

impl Drop for Disk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn two_rows() -> serde_json::Value {
    json!([
        {"id":"q_one","key":"run_one","text":"Check it.","attachments":["notes.md"]},
        {"id":"q_other","key":"run_other","text":"Keep me."}
    ])
}

fn press_send(rt: &mut Runtime<State>) {
    let button = rt.elements().into_iter()
        .find(|e| e.text.as_deref() == Some("Send now") && e.path.contains("/queued"))
        .unwrap_or_else(|| panic!("restored queue's Send now is missing"));
    let [x, y, w, h] = button.rect;
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    rt.frame(0.016);
}

fn pump_refusal(rt: &mut Runtime<State>) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut now = 0.1;
    while rt.state.flight.is_some() {
        assert!(std::time::Instant::now() < until, "no refusal: {}", rt.state.status);
        rt.update(|s, tx| { poll(s, now, tx); });
        rt.frame(0.016);
        now += 0.1;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

// case: Send durably removes its row before dispatch and leaves every other conversation held.
#[test]
fn b25_send_removes_only_the_selected_restored_row_before_dispatch() {
    let disk = Disk::new(two_rows());
    let _host = native::hooks::install(disk.1.clone(), None);
    let mut rt = disk.window();
    press_send(&mut rt);
    assert!(rt.state.flight.is_some(), "Send was pressed in the rendered strip");
    assert_eq!(disk.read().iter().map(|r| (r.key.as_str(), r.restored)).collect::<Vec<_>>(), [("run_other", true)]);
    assert_eq!(rt.state.queue_in_flight.as_ref().unwrap().attachments, ["notes.md"]);
    pump_refusal(&mut rt);
}

// case: A refused Send offers the message back held rather than dropping or retrying it.
#[test]
fn b25_a_refused_restored_send_returns_to_its_queue_with_its_files() {
    let disk = Disk::new(two_rows());
    let _host = native::hooks::install(disk.1.clone(), None);
    let mut rt = disk.window();
    // No run_one ledger exists. The real in-process starter refuses before it can start a runtime.
    press_send(&mut rt);
    assert!(rt.state.flight.is_some(), "Send dispatched");
    pump_refusal(&mut rt);
    assert_eq!(disk.read().iter().map(|r| (r.key.as_str(), r.text.as_str())).collect::<Vec<_>>(),
        [("run_one", "Check it."), ("run_other", "Keep me.")]);
    assert_eq!(disk.read()[0].attachments, ["notes.md"]);
    assert_eq!(rt.state.queue_rows[0].held.as_deref(), Some("not sent — press Send to try again"));
    assert!(rt.state.draft.text.is_empty());
    rt.update(|s, tx| { poll(s, 5.0, tx); });
    assert!(rt.state.flight.is_none(), "no automatic replay");
}

// case: A failed removal write sends nothing and keeps the saved words.
#[test]
fn b25_a_failed_removal_never_dispatches_or_replays_after_storage_recovers() {
    let disk = Disk::new(two_rows());
    let _host = native::hooks::install(disk.1.clone(), None);
    let mut rt = disk.window();
    let path = composer::queue_store::queue_path(&disk.1).unwrap();
    let spared = path.with_extension("backup");
    std::fs::rename(&path, &spared).unwrap();
    std::fs::create_dir(&path).unwrap(); // Fail the real write without discarding the valid file.
    press_send(&mut rt);
    assert!(rt.state.flight.is_none());
    assert_eq!(rt.state.queue_rows[0].text, "Check it.");
    assert!(rt.state.queue_error.as_deref().is_some_and(|e| e.contains("not saved")));
    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(spared, path).unwrap();
    rt.update(|s, tx| { poll(s, 1.0, tx); });
    assert!(rt.state.flight.is_none(), "fixing storage does not press Send again");
    assert_eq!(disk.read().iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["Check it.", "Keep me."]);
}

// Electron requeues the original rows after a refused fold. Native can only keep one row in
// its flight slot. Don't offer an oversized fold to the engine and lose the two saved rows.
#[test]
fn b25_an_oversized_fold_leaves_its_original_saved_rows_untouched() {
    let disk = Disk::new(json!([
        {"id":"q_one","key":"run_one","text":"a".repeat(8_000)},
        {"id":"q_second","key":"run_one","text":"b".repeat(8_000)}
    ]));
    let _host = native::hooks::install(disk.1.clone(), None);
    let mut rt = disk.window();
    press_send(&mut rt);
    assert!(rt.state.flight.is_none());
    assert_eq!(disk.read().iter().map(|r| r.text.clone()).collect::<Vec<_>>(),
        ["a".repeat(8_000), "b".repeat(8_000)]);
    assert!(rt.state.queue_error.as_deref().is_some_and(|e| e.contains("Nothing was sent")));
}
