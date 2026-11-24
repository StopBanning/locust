//! Electron `renderer/src/an-earlier-version-reads-as-it-was.test.ts`: rebuilding an earlier version of a file by
//! undoing each later turn's recorded change, exactly or not at all.
use super::*;
use crate::changes;

fn turn(diff: &str, truncated: bool) -> FileTurn {
    let file = changes::parse(diff).into_iter().next().expect("no file in the diff");
    FileTurn {
        mission_id: "m".into(),
        prompt: "p".into(),
        counts: file.counts(),
        file,
        truncated,
        reported: None,
    }
}

fn fixture() -> (FileTurn, FileTurn, FileTurn, &'static str) {
    let made = turn(
        "diff --git a/plan.md b/plan.md\nnew file mode 100644\n--- /dev/null\n+++ b/plan.md\n@@ -0,0 +1,2 @@\n+# Plan\n+Ship on Monday\n",
        false,
    );
    let changed = turn(
        "diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,2 +1,2 @@\n # Plan\n-Ship on Monday\n+Ship on Tuesday\n",
        false,
    );
    let added = turn(
        "diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,2 +1,3 @@\n # Plan\n Ship on Tuesday\n+Tell the team\n",
        false,
    );
    let now = "# Plan\nShip on Tuesday\nTell the team\n";
    (made, changed, added, now)
}

// case: after each turn, from the file as it is now
#[test]
fn after_each_turn_from_the_file_as_it_is_now() {
    let (made, changed, added, now) = fixture();
    let turns = [made, changed, added];
    assert_eq!(
        file_as_it_was(now, &turns, 0),
        Reversed::Ok(Some("# Plan\nShip on Monday\n".into()))
    );
    assert_eq!(
        file_as_it_was(now, &turns, 1),
        Reversed::Ok(Some("# Plan\nShip on Tuesday\n".into()))
    );
    assert_eq!(file_as_it_was(now, &turns, 2), Reversed::Ok(Some(now.into())));
}

// case: is not rebuilt when a later change was recorded cut short, and says so
#[test]
fn is_not_rebuilt_when_a_later_change_was_recorded_cut_short_and_says_so() {
    let (made, changed, _, now) = fixture();
    let cut = turn(
        "diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1,2 +1,3 @@\n # Plan\n Ship on Tuesday\n+Tell the team\n",
        true,
    );
    assert_eq!(
        file_as_it_was(now, &[made, changed, cut], 0),
        Reversed::Err("a later change to it was recorded cut short".into())
    );
}

// case: is not rebuilt when the file was changed since, rather than rebuilt wrong
#[test]
fn is_not_rebuilt_when_the_file_was_changed_since_rather_than_rebuilt_wrong() {
    let (made, changed, added, _) = fixture();
    let edited_since = "# Plan\nShip on Wednesday\nTell the team\n";
    assert!(matches!(
        file_as_it_was(edited_since, &[made, changed, added], 0),
        Reversed::Err(_)
    ));
}

// case: keeps Windows line endings
#[test]
fn keeps_windows_line_endings() {
    let (made, changed, added, now) = fixture();
    let crlf = now.replace('\n', "\r\n");
    assert_eq!(
        file_as_it_was(&crlf, &[made, changed, added], 1),
        Reversed::Ok(Some("# Plan\r\nShip on Tuesday\r\n".into()))
    );
}

fn edit(id: &str, diff: &str) -> Item {
    Item::Tool {
        name: "Edit".into(),
        status: crate::ledger::Status::Done,
        summary: "plan.md".into(),
        detail: Box::new(crate::ledger::ToolDetail {
            id: id.into(),
            patch: Some(crate::activity_entries::Patch { text: diff.into(), added: 2, removed: 0, truncated: false }),
            ..crate::ledger::ToolDetail::default()
        }),
    }
}

/// Electron keeps one turn per message sent (`missionView.ts::fileTurns` walks every turn), so a message that got no
/// answer still counts: the edit after it belongs to the second turn, not the first.
#[test]
fn a_message_with_no_answer_still_counts_as_a_turn() {
    let made = "diff --git a/plan.md b/plan.md\nnew file mode 100644\n--- /dev/null\n+++ b/plan.md\n@@ -0,0 +1,2 @@\n+# Plan\n+Ship on Monday\n";
    let items = vec![
        Item::User("Write the plan".into()),
        Item::User("Actually, write it now".into()),
        edit("t1", made),
    ];
    let turns = file_turns_from_items(&items, "plan.md", None);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].mission_id, "turn-2");
    assert_eq!(turns[0].prompt, "Actually, write it now");
}
