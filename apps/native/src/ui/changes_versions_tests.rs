//! Window tests for Row 2 (`an-earlier-version-reads-as-it-was.test.ts`): picking an earlier turn in the Changes
//! panel reads the file as it was after that turn, and refuses with a reason when a later change was cut short or the
//! file was edited since.
use super::*;
use locust_core::{activity_entries::Patch, ledger};

fn tool_patch(id: &str, diff: &str, truncated: bool) -> ledger::Item {
    ledger::Item::Tool {
        name: "Edit".into(),
        status: ledger::Status::Done,
        summary: "plan.md".into(),
        detail: Box::new(ledger::ToolDetail {
            id: id.into(),
            patch: Some(Patch {
                text: diff.into(),
                added: 1,
                removed: 0,
                truncated,
            }),
            ..ledger::ToolDetail::default()
        }),
    }
}

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "locust-changes-ver-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const MADE: &str = "diff --git a/plan.md b/plan.md\nnew file mode 100644\n--- /dev/null\n+++ b/plan.md\n@@ -0,0 +1,2 @@\n+# Plan\n+Ship on Monday\n";
const CHANGED: &str = "diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,2 +1,2 @@\n # Plan\n-Ship on Monday\n+Ship on Tuesday\n";
const ADDED: &str = "diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,2 +1,3 @@\n # Plan\n Ship on Tuesday\n+Tell the team\n";

fn panel_with_turns(folder: PathBuf, disk_text: &str, truncated_last: bool) -> rune::Runtime<State> {
    std::fs::write(folder.join("plan.md"), disk_text).unwrap();
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        header: Teammate {
            id: "tm_doc".into(),
            name: "Doc".into(),
            role: "Writer".into(),
            runtime: Some("claude".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "local".into(),
            folder: Some(folder.to_string_lossy().into_owned()),
        },
        conversation: Conversation::default(),
        items: vec![
            ledger::Item::User("Write the plan".into()),
            tool_patch("t1", MADE, false),
            ledger::Item::User("Move to Tuesday".into()),
            tool_patch("t2", CHANGED, false),
            ledger::Item::User("Add announcement".into()),
            tool_patch("t3", ADDED, truncated_last),
        ],
        dir: folder.clone(),
        worker: None,
        snapshot: ledger::Snapshot::default(),
        cold_tip: None,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s.changes = Panel {
        open: true,
        folder,
        diff: Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "feature".into(),
            showing: "all".into(),
            files: changes::parse(ADDED),
            ..changes::Folder::default()
        })),
        ..Panel::default()
    };
    let mut rt = rune::Runtime::new(
        s,
        |s, _| view(s, &Palette::new(true)),
        (1200.0, 2400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn texts<T>(rt: &rune::Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

fn click(rt: &mut rune::Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing element {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..30 {
        rt.frame(0.05);
    }
}

#[test]
fn picking_an_earlier_turn_in_changes_panel_reads_the_file_as_it_was() {
    let tmp = TempDir::new("ok");
    let now = "# Plan\nShip on Tuesday\nTell the team\n";
    let mut rt = panel_with_turns(tmp.0.clone(), now, false);
    let initial = texts(&rt);
    assert!(initial.contains(&"CHANGED IN 3 TURNS".to_owned()), "{initial:?}");

    // Pick turn 1: reads the file as it was after turn 1 (Monday, before Tuesday and Tell the team).
    click(&mut rt, "changes-version-0-0");
    let after_t1 = texts(&rt);
    assert!(
        after_t1.contains(&"The file as it was after turn 1.".to_owned()),
        "{after_t1:?}"
    );
    assert!(
        after_t1.contains(&"# Plan\nShip on Monday\n".to_owned()),
        "expected rebuilt turn 1 file: {after_t1:?}"
    );

    // Pick turn 2: reads the file as it was after turn 2 (Tuesday, without Tell the team).
    click(&mut rt, "changes-version-0-1");
    let after_t2 = texts(&rt);
    assert!(
        after_t2.contains(&"The file as it was after turn 2.".to_owned()),
        "{after_t2:?}"
    );
    assert!(
        after_t2.contains(&"# Plan\nShip on Tuesday\n".to_owned()),
        "expected rebuilt turn 2 file: {after_t2:?}"
    );

    // Clicking "What turn 2 changed" switches to that turn's change, and "Now" returns to the current diff.
    click(&mut rt, "changes-version-as-change-0");
    let as_change = texts(&rt);
    assert!(
        as_change.contains(&"What turn 2 changed.".to_owned()),
        "{as_change:?}"
    );
    assert!(as_change.contains(&"Monday".to_owned()), "{as_change:?}");
    assert!(as_change.contains(&"Tuesday".to_owned()), "{as_change:?}");

    click(&mut rt, "changes-version-0-now");
    let back_now = texts(&rt);
    assert!(
        !back_now.iter().any(|t| t.starts_with("The file as it was")),
        "{back_now:?}"
    );
    assert!(back_now.contains(&"Tell the team".to_owned()), "{back_now:?}");
}

#[test]
fn picking_an_earlier_turn_refuses_when_a_later_change_was_cut_short() {
    let tmp = TempDir::new("cut");
    let now = "# Plan\nShip on Tuesday\nTell the team\n";
    let mut rt = panel_with_turns(tmp.0.clone(), now, true);
    click(&mut rt, "changes-version-0-0");
    let said = texts(&rt);
    assert!(
        said.iter()
            .any(|t| t.contains("a later change to it was recorded cut short")),
        "expected cut-short refusal: {said:?}"
    );
}
