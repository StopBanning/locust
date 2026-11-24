//! Electron `renderer/src/a-document-change-reads-as-words.test.ts`: a document's change as the passages a reader sees.
use super::*;
use crate::changes;

const POLICY_DIFF: &str = "\
--- a/workspace-policy.md
+++ b/workspace-policy.md
@@ -1,8 +1,9 @@
 # Workspace policy
 
-Requests close at 4 pm on Friday.
+Requests close at 3 pm on Friday.
 
 | Task | Timing |
 | --- | --- |
-| Cutoff | 4 pm |
+| Cutoff | 3 pm |
+| Review | Monday |
-- Old rule nobody follows
";

// case: names each changed passage with its old words and new words, Markdown marks taken off
#[test]
fn names_each_changed_passage_with_its_old_words_and_new_words_markdown_marks_taken_off() {
    let policy = changes::parse(POLICY_DIFF).remove(0);
    let passages = document_changes(&policy);
    assert_eq!(
        passages,
        vec![
            ChangedPassage::Changed {
                before: "Requests close at ".into(),
                removed: "4".into(),
                added: "3".into(),
                after: " pm on Friday.".into(),
            },
            ChangedPassage::Changed {
                before: "Cutoff · ".into(),
                removed: "4".into(),
                added: "3".into(),
                after: " pm".into(),
            },
            ChangedPassage::Added { text: "Review · Monday".into() },
            ChangedPassage::Removed { text: "• Old rule nobody follows".into() },
        ]
    );
}

// case: strikes and marks whole words, never the inside of one (drive: "~~Mon~~Tuesday")
#[test]
fn strikes_and_marks_whole_words_never_the_inside_of_one() {
    let day = changes::parse("--- a/p.md\n+++ b/p.md\n@@ -1 +1 @@\n-Ship on Monday\n+Ship on Tuesday\n").remove(0);
    assert_eq!(
        document_changes(&day),
        vec![ChangedPassage::Changed {
            before: "Ship on ".into(),
            removed: "Monday".into(),
            added: "Tuesday".into(),
            after: String::new(),
        }]
    );
    let middle = changes::parse("--- a/p.md\n+++ b/p.md\n@@ -1 +1 @@\n-We ship slowly today.\n+We ship quickly today.\n").remove(0);
    assert_eq!(
        document_changes(&middle),
        vec![ChangedPassage::Changed {
            before: "We ship ".into(),
            removed: "slowly".into(),
            added: "quickly".into(),
            after: " today.".into(),
        }]
    );
}

// case: reads a heading, a bullet, a quote and a table row as a reader sees them
#[test]
fn reads_a_heading_a_bullet_a_quote_and_a_table_row_as_a_reader_sees_them() {
    assert_eq!(as_read("## Hours"), "Hours");
    assert_eq!(as_read("- Bring a badge"), "• Bring a badge");
    assert_eq!(as_read("> Note **well**"), "Note well");
    assert_eq!(as_read(r"| a | b \| c |"), "a · b | c");
    assert_eq!(as_read("| --- | :---: |"), "");
}
