//! Window tests for Row 1 (`a-document-change-reads-as-words.test.ts`): opening the Changes panel on a changed `.md`
//! file draws its passages with old words struck and new words marked by whole word, and lets the reader toggle to the
//! line diff and back.
use super::*;

const POLICY_MD_DIFF: &str = "\
diff --git a/workspace-policy.md b/workspace-policy.md
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

fn panel_of(diff: &str) -> rune::Runtime<State> {
    let mut s = State::default();
    s.changes = Panel {
        open: true,
        diff: Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "feature".into(),
            showing: "all".into(),
            files: changes::parse(diff),
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

#[test]
fn changes_panel_draws_markdown_file_as_struck_and_marked_words_and_toggles_to_lines() {
    let mut rt = panel_of(POLICY_MD_DIFF);
    let els = rt.elements();
    let said = texts(&rt);

    // Drawn as passages, not raw diff lines or Markdown pipes.
    assert!(said.contains(&"Changed".to_owned()), "{said:?}");
    assert!(said.contains(&"Added".to_owned()), "{said:?}");
    assert!(said.contains(&"Removed".to_owned()), "{said:?}");
    assert!(said.contains(&"Requests close at ".to_owned()), "{said:?}");
    assert!(said.contains(&"Cutoff · ".to_owned()), "{said:?}");
    assert!(said.contains(&"Review · Monday".to_owned()), "{said:?}");
    assert!(said.contains(&"• Old rule nobody follows".to_owned()), "{said:?}");
    assert!(
        !said.iter().any(|t| t.starts_with("@@ -")),
        "Markdown file should open in word mode, not line diff: {said:?}"
    );

    // Old words carry the deletion mark fill and new words carry the addition mark fill.
    let del_bg = color("#4a2429");
    let add_bg = color("#1f4029");
    let del_four = els
        .iter()
        .find(|e| e.text.as_deref() == Some("4") && e.fill == del_bg);
    let add_three = els
        .iter()
        .find(|e| e.text.as_deref() == Some("3") && e.fill == add_bg);
    assert!(del_four.is_some(), "struck old word '4' missing: {said:?}");
    assert!(add_three.is_some(), "marked new word '3' missing: {said:?}");

    // Whole-word check: Monday -> Tuesday never splits inside "day".
    let rt_day = panel_of(
        "diff --git a/plan.md b/plan.md\n--- a/plan.md\n+++ b/plan.md\n@@ -1 +1 @@\n-Ship on Monday\n+Ship on Tuesday\n",
    );
    let day_els = rt_day.elements();
    assert!(
        day_els
            .iter()
            .any(|e| e.text.as_deref() == Some("Monday") && e.fill == del_bg),
        "Monday should be struck whole: {:?}",
        texts(&rt_day)
    );
    assert!(
        day_els
            .iter()
            .any(|e| e.text.as_deref() == Some("Tuesday") && e.fill == add_bg),
        "Tuesday should be marked whole: {:?}",
        texts(&rt_day)
    );

    // Clicking "Show the lines that changed" switches to the line diff, and "Show it as words" switches back.
    let (x, y) = rt
        .find("changes-doc-lines-0")
        .expect("Show the lines that changed button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    let line_said = texts(&rt);
    assert!(
        line_said.iter().any(|t| t.starts_with("@@ -")),
        "expected line diff after toggling: {line_said:?}"
    );
    let (wx, wy) = rt
        .find("changes-doc-words-0")
        .expect("Show it as words button");
    rt.pointer_down(wx, wy);
    rt.pointer_up(wx, wy);
    rt.frame(0.016);
    let back_said = texts(&rt);
    assert!(
        back_said.contains(&"Review · Monday".to_owned()),
        "expected word mode back: {back_said:?}"
    );
}
