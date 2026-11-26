//! the-file-viewer-renders-and-never-runs.test.tsx, "the versions of a file across turns" (5 cases): the window's
//! version strip is the Changes panel's (`version_strip`), over the turns this conversation's ledger shows changed the
//! file. It counts them, offers each and Now, is absent for a file no turn here changed, opens on the file as it is,
//! and never claims to restore it. Expected words are Electron's (FileViewer.tsx and the test).
use super::*;
use locust_core::{activity_entries::Patch, ledger};

const MADE: &str = "diff --git a/plan.md b/plan.md\nnew file mode 100644\n--- /dev/null\n+++ b/plan.md\n@@ -0,0 +1,2 @@\n+# Plan\n+Ship on Monday\n";
const CHANGED: &str = "diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,2 +1,2 @@\n # Plan\n-Ship on Monday\n+Ship on Tuesday\n";

fn edit(id: &str, diff: &str) -> ledger::Item {
    ledger::Item::Tool {
        name: "Edit".into(),
        status: ledger::Status::Done,
        summary: "plan.md".into(),
        detail: Box::new(ledger::ToolDetail { id: id.into(), patch: Some(Patch { text: diff.into(), added: 1, removed: 0, truncated: false }), ..ledger::ToolDetail::default() }),
    }
}

/// The Changes panel on plan.md, in a conversation whose turns made `diffs` (one turn each).
fn panel(diffs: &[&str]) -> rune::Runtime<State> {
    let folder = std::env::temp_dir().join(format!("locust-versions-strip-{}-{}", std::process::id(), diffs.len()));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("plan.md"), "# Plan\nShip on Tuesday\n").unwrap();
    let mut items = Vec::new();
    for (i, diff) in diffs.iter().enumerate() {
        items.push(ledger::Item::User(format!("ask {}", i + 1)));
        items.push(edit(&format!("t{i}"), diff));
    }
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        header: Teammate { id: "tm_doc".into(), name: "Doc".into(), role: "Writer".into(), runtime: Some("claude".into()), model: None, account: None, busy: false, runs_in: "local".into(), folder: Some(folder.to_string_lossy().into_owned()) },
        conversation: Conversation::default(),
        items,
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
        diff: Some(Ok(changes::Folder { base: "main".into(), branch: "feature".into(), showing: "all".into(), files: changes::parse(CHANGED), ..changes::Folder::default() })),
        ..Panel::default()
    };
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn texts<T>(rt: &rune::Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

fn all_said<T>(rt: &rune::Runtime<T>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).flat_map(|e| [e.text, e.label]).flatten().collect::<Vec<_>>().join("\n")
}

#[test]
fn counts_the_turns_that_changed_it_and_offers_each_one() {
    let rt = panel(&[MADE, CHANGED]);
    let t = texts(&rt);
    assert!(t.contains(&"CHANGED IN 2 TURNS".to_owned()), "{t:?}");
    for key in ["changes-version-0-0", "changes-version-0-1"] {
        assert!(rt.find(key).is_some(), "{key}");
    }
    // And a way back to the file itself, which is where it opens.
    assert!(rt.find("changes-version-0-now").is_some());
    assert!(t.contains(&"Now".to_owned()), "{t:?}");
}

#[test]
fn says_one_turn_in_the_singular() {
    assert!(texts(&panel(&[CHANGED])).contains(&"CHANGED IN 1 TURN".to_owned()));
}

#[test]
fn shows_nothing_at_all_for_a_file_this_conversation_never_changed() {
    let rt = panel(&[]);
    assert!(!rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("changes-versions-"))));
    assert!(!all_said(&rt).contains("CHANGED IN"));
}

#[test]
fn opens_on_the_file_not_on_a_turn() {
    let rt = panel(&[MADE]);
    assert!(rt.state.changes.file_turn.is_empty(), "no turn is picked");
    let said = all_said(&rt);
    assert!(!said.contains("The file as it was after") && !said.contains("What turn 1 changed"), "{said}");
}

#[test]
fn never_offers_to_show_the_file_as_it_stood() {
    let said = all_said(&panel(&[MADE, CHANGED]));
    assert!(!regex::Regex::new(r"(?i)as it (looked|stood|was)").unwrap().is_match(&said), "{said}");
    assert!(!regex::Regex::new(r"(?i)\b(restore|revert|roll ?back)\b").unwrap().is_match(&said), "{said}");
}
