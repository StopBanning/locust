use super::*;
use locust_core::{ledger_write::LedgerWriter, native, store::Store};
use rune::{Env, Runtime};
use serde_json::json;
use std::fs;
use std::path::Path;

struct Fixture {
    dir: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "locust-export-card-{}-{}",
            std::process::id(),
            locust_core::invariants::unique_nanos()
        ));
        fs::create_dir_all(dir.join("chosen")).unwrap();
        let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
        Self { dir, store }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn window(store: &Store) -> Runtime<State> {
    let mut state = State::default();
    state.data = Some(store.clone());
    open(&mut state, "mission_one");
    let mut window = Runtime::new(state, crate::view, (1100., 780.), Env::new(true, 1.));
    settle(&mut window);
    window
}

fn settle(window: &mut Runtime<State>) {
    for _ in 0..20 {
        window.frame(0.05);
    }
}

fn press(window: &mut Runtime<State>, key: &str) {
    let (x, y) = window.find(key).unwrap_or_else(|| panic!("missing {key}"));
    window.pointer_down(x, y);
    window.pointer_up(x, y);
    settle(window);
}

fn recorded_turn(store: &Store) -> PathBuf {
    let stamp = |min: u8, sec: u8| format!("2026-10-02T09:{min:02}:{sec:02}.000Z");
    let mut turn = LedgerWriter::create_runtime(
        store,
        "mission_one",
        "Tidy the build folder.",
        "account-default",
        "codex",
        "codex",
        &[],
        None,
    )
    .unwrap();
    let mut append = |id, command, answer, by, words: Option<&str>, asked_at, answered_at| {
        let mut approval = json!({
            "approvalId": id, "kind": "command", "asked": format!("Run a command\n{command}"),
            "answer": answer, "by": by, "askedAt": asked_at, "occurredAt": answered_at,
        });
        if let Some(words) = words {
            approval["words"] = json!(words);
        }
        turn.append_approval(&approval).unwrap();
    };
    append(
        "ap_1",
        "npm test",
        "allowed",
        "card",
        None,
        stamp(0, 59),
        stamp(1, 0),
    );
    append(
        "ap_2",
        "rm -rf build",
        "denied",
        "card",
        Some("use the build script instead"),
        stamp(1, 10),
        stamp(1, 12),
    );
    append(
        "ap_3",
        "git push origin main",
        "denied",
        "saved-rule",
        Some("never run git push for Wren in this folder."),
        stamp(1, 20),
        stamp(1, 21),
    );
    append(
        "ap_4",
        "npm run lint",
        "allowed",
        "earlier-always",
        Some("The Always given earlier in this run allows every command it runs."),
        stamp(1, 30),
        stamp(1, 30),
    );
    turn.path.clone()
}

fn save(window: &mut Runtime<State>, target: &Path) {
    press(window, "save-record-submit");
    let (kind, options) = window.file_dialog().expect("opens the native save dialog");
    assert_eq!(kind, rune::FileDialogKind::Save);
    assert_eq!(options.title, "Save the record");
    assert!(
        !target.exists(),
        "no file is written until the chooser returns a path"
    );
    window
        .resolve_file_dialog(Some(target.to_path_buf()))
        .unwrap();
    settle(window);
    let status = format!("Saved to {}.", target.canonicalize().unwrap().display());
    assert!(
        window
            .elements()
            .iter()
            .any(|e| e.text.as_deref() == Some(&status)),
        "{status}"
    );
}

// case: lists each card answered: what it asked, the answer, and who gave it
#[test]
fn save_record_exports_retained_cards_from_the_disk_to_the_chosen_file() {
    let fixture = Fixture::new();
    let ledger = recorded_turn(&fixture.store);
    let before = fs::read(&ledger).unwrap();
    let _hooks = native::hooks::install(fixture.store.clone(), None);
    let mut window = window(&fixture.store);
    let target = fixture.dir.join("chosen/receipt.md");
    save(&mut window, &target);
    let report = fs::read_to_string(&target).unwrap();
    let mut after = 0;
    for receipt in [
        "- **Allowed** by the person, on the card · command · asked 2026-10-02T09:00:59.000Z · answered 2026-10-02T09:01:00.000Z",
        "> The card asked: Run a command\n> npm test",
        "- **Denied** by the person, on the card · command",
        "> Said with the answer: use the build script instead",
        "- **Denied** by a rule the person saved, before the card reached them · command",
        "> Said with the answer: never run git push for Wren in this folder.",
        "- **Allowed** by the person’s Always on an earlier card of this run, with no card of its own · command",
        "> Said with the answer: The Always given earlier in this run allows every command it runs.",
    ] {
        after += report[after..]
            .find(receipt)
            .unwrap_or_else(|| panic!("missing or out of order: {receipt}\n{report}"))
            + receipt.len();
    }
    assert_eq!(
        fs::read(&ledger).unwrap(),
        before,
        "saving is read-only on the mission"
    );
    assert_eq!(fs::read_dir(target.parent().unwrap()).unwrap().count(), 1);
}

// case: has a dialog that says what the file holds once, and offers the raw record as a tick that starts unticked
#[test]
fn save_record_dialog_names_raw_json_but_does_not_save_it_by_default() {
    let fixture = Fixture::new();
    recorded_turn(&fixture.store);
    let _hooks = native::hooks::install(fixture.store.clone(), None);
    let mut window = window(&fixture.store);
    let all: Vec<_> = window
        .elements()
        .into_iter()
        .filter_map(|el| el.text)
        .collect();
    let warnings = all
        .iter()
        .filter(|t| {
            t.as_str() == "This file contains the conversation. Read it before you send it."
        })
        .count();
    assert_eq!(warnings, 1);
    assert!(
        all.iter()
            .any(|t| t == "Include the raw record (JSON, not scrubbed)")
    );
    assert!(all.iter().any(|t| t == "Save the record…"));
    let target = fixture.dir.join("chosen/only-markdown.md");
    save(&mut window, &target);
    assert!(
        !target.with_extension("json").exists(),
        "raw JSON requires a tick"
    );
}

// case: says "record incomplete" and lists each problem when a ledger file was torn
#[test]
fn save_record_reports_the_torn_tail_but_keeps_the_readable_card() {
    use std::io::Write as _;
    let fixture = Fixture::new();
    let ledger = recorded_turn(&fixture.store);
    fs::OpenOptions::new()
        .append(true)
        .open(&ledger)
        .unwrap()
        .write_all(br#"{"recordType":"mission.event","event":"#)
        .unwrap();
    let before = fs::read(&ledger).unwrap();
    let _hooks = native::hooks::install(fixture.store.clone(), None);
    let mut window = window(&fixture.store);
    let target = fixture.dir.join("chosen/torn.md");
    save(&mut window, &target);
    let report = fs::read_to_string(&target).unwrap();
    assert!(
        report.contains(
            "**record incomplete.** 1 problem was found reading this conversation's ledger"
        ),
        "{report}"
    );
    assert!(report.contains(
        "`truncated-tail`: An incomplete final ledger record was ignored after recovery. (mission_one)"
    ));
    assert!(!report.contains("**record readable.**"));
    assert!(report.contains("> The card asked: Run a command\n> npm test"));
    assert_eq!(fs::read(&ledger).unwrap(), before);
}
