//! Electron `renderer/src/diff.test.ts`, the cases the window draws: the fold of unchanged lines (`foldContext`), the
//! statement a file ends with as a button or as words (`completenessOf`'s `canExpand`), and the marks on what changed
//! within a line (`pairedSpans`). The reading itself is tested in core (`core/src/changes_diff_tests.rs`).
use super::*;

const TWO_HUNKS: &str = "diff --git a/src/billing.ts b/src/billing.ts
--- a/src/billing.ts
+++ b/src/billing.ts
@@ -12,4 +12,6 @@ registerHandlers()
 export function registerHandlers(app: App) {
   app.post(\"/hooks/billing/v2\", handleV2);
-  app.post(\"/hooks/billing/v3\", handleV3);
+  if (flags.billingV3) {
+    app.post(\"/hooks/billing/v3\", handleV3);
+  }
 }
@@ -41,4 +43,6 @@ handleV3()
 async function handleV3(req: Request) {
-  return old(req);
+  const body = parse(req);
+  return next(body);
+  // logged
 }
";

/// Fourteen rows, one replacement at the eighth: four lines fold before it, three stay either side, two fold after.
fn fourteen() -> String {
    let mut text = String::from("--- a/x\n+++ b/x\n@@ -1,13 +1,13 @@\n");
    for i in 0..14 {
        text.push_str(&match i {
            7 => "-seven".to_owned(),
            8 => "+SEVEN".to_owned(),
            _ => format!(" line {i}"),
        });
        text.push('\n');
    }
    text
}

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
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}
fn texts<T>(rt: &rune::Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

// ---- folding unchanged lines ----

// case: keeps three lines either side of a change and folds the rest with an exact count
#[test]
fn keeps_three_lines_either_side_of_a_change_and_folds_the_rest_with_an_exact_count() {
    let rt = panel_of(&fourteen());
    let said = texts(&rt);
    let folds: Vec<&String> = said.iter().filter(|t| t.ends_with("unchanged lines") || t.ends_with("unchanged line")).collect();
    assert_eq!(folds, ["4 unchanged lines", "2 unchanged lines"], "{said:?}");
    // Folded before: lines 0 to 3; kept: 4 to 6, the change, 9 to 11; folded after: 12 and 13.
    for i in [0, 1, 2, 3, 12, 13] {
        assert!(!said.contains(&format!("line {i}")), "line {i} should fold: {said:?}");
    }
    for i in [4, 5, 6, 9, 10, 11] {
        assert!(said.contains(&format!("line {i}")), "line {i} should show: {said:?}");
    }
    // And one fold says one line.
    let mut rows = vec![changes::Row { kind: ' ', ..changes::Row::default() }; 5];
    rows[0].kind = '+';
    assert_eq!(changes::kept_context(&rows), [true, true, true, true, false]);
}

// case: never folds a change, only context
#[test]
fn never_folds_a_change_only_context() {
    // Changes far apart, with long runs of context between: every change is drawn, and only context is folded.
    let mut text = String::from("--- a/x\n+++ b/x\n@@ -1,30 +1,30 @@\n");
    for i in 0..30 {
        text.push_str(&if i % 10 == 0 { format!("-old {i}\n+new {i}\n") } else { format!(" line {i}\n") });
    }
    let rt = panel_of(&text);
    let said = texts(&rt);
    // Each changed line is drawn: its sign, and the marked part of what it says.
    assert_eq!(said.iter().filter(|t| *t == "old").count(), 3, "{said:?}");
    assert_eq!(said.iter().filter(|t| *t == "new").count(), 3, "{said:?}");
    for i in [0, 10, 20] {
        assert!(said.contains(&format!(" {i}")), "change {i} was folded: {said:?}");
    }
    let f = changes::parse(&text).remove(0);
    let keep = changes::kept_context(&f.hunks[0].rows);
    assert!(f.hunks[0].rows.iter().zip(&keep).all(|(r, k)| r.kind == ' ' || *k));
    assert!(said.iter().any(|t| t == "3 unchanged lines"), "{said:?}");
}

// ---- no open file ends in silence: the statement, as a button when it can expand ----

#[test]
fn a_file_with_hunks_still_hidden_ends_with_a_button_naming_them_and_one_shown_whole_with_words() {
    // Three hunks: the panel shows two first, like Electron's HUNKS_SHOWN_FIRST, and offers the third.
    let three = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n@@ -10 +10 @@\n-c\n+d\n@@ -20,2 +20,2 @@\n ctx\n-e\n+f\n";
    let mut rt = panel_of(three);
    let said = texts(&rt);
    let expand = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Expand full file · 1 hunk more, 2 lines"));
    let [x, y, w, h] = expand.unwrap_or_else(|| panic!("no expand statement: {said:?}")).rect;
    assert!(!said.iter().any(|t| t.starts_with("@@ -20")), "the third hunk is offered, not shown: {said:?}");
    // Pressing it shows the rest, and the file then says all of it is shown.
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    for _ in 0..5 {
        rt.frame(0.1);
    }
    let said = texts(&rt);
    assert!(said.iter().any(|t| t.starts_with("@@ -20,2 +20,2 @@")), "{said:?}");
    assert!(said.iter().any(|t| t == "All 3 sections shown"), "{said:?}");
    assert!(!said.iter().any(|t| t.starts_with("Expand full file")), "{said:?}");

    // Nothing left to expand: the file ends in words, and pressing them changes nothing.
    let mut rt = panel_of(TWO_HUNKS);
    let all = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("All 2 sections shown"));
    let [x, y, w, h] = all.unwrap_or_else(|| panic!("no closing statement: {:?}", texts(&rt))).rect;
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    rt.frame(0.016);
    assert!(rt.state.changes.expanded.is_empty());
}

#[test]
fn a_change_drawn_in_a_drawer_ends_with_its_statement_too() {
    let draw = |diff: &str, truncated: bool| {
        let f = changes::parse(diff).remove(0);
        let mut rt = rune::Runtime::new(
            (f, truncated),
            |(f, truncated), _| crate::review_changes::file_diff(&State::default(), None, false, f, *truncated, &Palette::new(true)),
            (1200.0, 1600.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        texts(&rt)
    };
    assert!(draw(TWO_HUNKS, false).iter().any(|t| t == "All 2 sections shown"));
    assert!(draw("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n", false).iter().any(|t| t == "The whole change is shown"));
    let cut = draw(TWO_HUNKS, true);
    assert!(cut.iter().any(|t| t.ends_with("the rest was not recorded inline")), "{cut:?}");
}

// ---- marking only what changed on a line ----

#[test]
fn a_replaced_line_marks_only_what_changed_on_its_word_background() {
    let rt = panel_of("--- a/x\n+++ b/x\n@@ -1 +1,2 @@\n-  app.post(\"/v2\", handleV2);\n+  app.post(\"/v3\", handleV3);\n+  extra();\n");
    let els = rt.elements();
    let old = els.iter().find(|e| e.text.as_deref() == Some("2\", handleV2")).unwrap_or_else(|| panic!("{:?}", texts(&rt)));
    let new = els.iter().find(|e| e.text.as_deref() == Some("3\", handleV3")).unwrap_or_else(|| panic!("{:?}", texts(&rt)));
    assert_eq!(old.fill, color("#4a2429"));
    assert_eq!(new.fill, color("#1f4029"));
    // The plain prefix is drawn apart from the mark, unfilled; an unpaired addition has no mark.
    let plain = els.iter().find(|e| e.text.as_deref() == Some("  app.post(\"/v")).expect("plain prefix");
    assert_ne!(plain.fill, new.fill);
    assert!(els.iter().filter(|e| e.fill == new.fill || e.fill == old.fill).count() == 2, "only the two marks are filled");
}

// ---- the same DiffView everywhere Electron draws one: the approval card, a review, an activity drawer ----

const THREE_HUNKS: &str = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n@@ -10 +10 @@\n-c\n+d\n@@ -20,2 +20,2 @@\n ctx\n-e\n+f\n";

thread_local! {
    /// What the drawer test draws: a view is a plain `fn`, so what it draws is kept here, one process per test.
    static DRAWN_FILE: std::cell::RefCell<Option<changes::File>> = const { std::cell::RefCell::new(None) };
    static DRAWN_ITEMS: std::cell::RefCell<Vec<locust_core::ledger::Item>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn drawn_with(view: fn(&State, &mut rune::Cx<State>) -> El) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(State::default(), view, (1200.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}
fn press_words(rt: &mut rune::Runtime<State>, words: &str) {
    let found = rt.elements().into_iter().find(|e| e.text.as_deref() == Some(words));
    let [x, y, w, h] = found.unwrap_or_else(|| panic!("no {words:?} in {:?}", texts(rt))).rect;
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    // The window keeps what the press replaced for a frame or two; let it go.
    for _ in 0..5 {
        rt.frame(0.1);
    }
}
fn approval_with(patch: &str, truncated: bool, added: usize, removed: usize) -> rune::Runtime<State> {
    let request = locust_core::approval::Request {
        approval_id: "ap_1".into(),
        mission_id: "mission_1".into(),
        runtime: "codex".into(),
        kind: locust_core::approval::Kind::FileChange,
        summary: "Edit x".into(),
        detail: "x".into(),
        cwd: None,
        requested_at: "2026-10-10T22:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: Some(locust_core::approval_patch::ToolPatch { text: patch.into(), added, removed, truncated }),
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: Vec::new(),
    };
    let mut rt = drawn_with(|s, _| crate::approval_card::approval_card(s, &Palette::new(true)));
    rt.update(|s, _| s.pending_approval = Some(request));
    for _ in 0..20 {
        rt.frame(0.1);
    }
    rt
}

#[test]
fn the_approval_card_draws_its_change_as_diffview_does_with_two_hunks_first_and_the_rest_behind_a_button() {
    let mut rt = approval_with(THREE_HUNKS, false, 3, 3);
    let said = texts(&rt);
    assert!(said.iter().any(|t| t.starts_with("@@ -10")), "{said:?}");
    assert!(!said.iter().any(|t| t.starts_with("@@ -20")), "the third hunk is offered, not shown: {said:?}");
    press_words(&mut rt, "Expand full file · 1 hunk more, 2 lines");
    let said = texts(&rt);
    assert!(said.iter().any(|t| t.starts_with("@@ -20,2 +20,2 @@")), "{said:?}");
    assert!(said.iter().any(|t| t == "All 3 sections shown"), "{said:?}");
}

#[test]
fn the_approval_card_folds_unchanged_lines_and_marks_only_what_changed() {
    let mut rt = approval_with(&fourteen(), false, 1, 1);
    let said = texts(&rt);
    assert!(said.iter().any(|t| t == "4 unchanged lines") && said.iter().any(|t| t == "2 unchanged lines"), "{said:?}");
    assert!(!said.contains(&"line 0".to_owned()), "{said:?}");
    press_words(&mut rt, "4 unchanged lines");
    assert!(texts(&rt).contains(&"line 0".to_owned()), "the fold opens on its own press");
    let marked = approval_with("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-  call(\"/v2\", h2);\n+  call(\"/v3\", h3);\n", false, 1, 1);
    let els = marked.elements();
    let new = els.iter().find(|e| e.text.as_deref() == Some("3\", h3")).unwrap_or_else(|| panic!("{:?}", texts(&marked)));
    assert_eq!(new.fill, color("#1f4029"));
}

#[test]
fn a_cut_approval_patch_says_what_the_runtime_counted_in_all() {
    let rt = approval_with("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n", true, 120, 7);
    let said = texts(&rt);
    assert!(
        said.iter().any(|t| t == "The runtime reported more than fits here · 1 hunk rendered · the rest was not recorded inline · the runtime counted +120 −7 in all"),
        "{said:?}"
    );
    // A patch that was not cut says only that it is whole.
    let whole = texts(&approval_with("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n", false, 1, 1));
    assert!(whole.iter().any(|t| t == "The whole change is shown"), "{whole:?}");
    assert!(!whole.iter().any(|t| t.contains("the runtime counted")), "{whole:?}");
}

#[test]
fn a_drawer_holds_back_hunks_past_two_and_folds_unchanged_lines() {
    let draw = |diff: String| {
        DRAWN_FILE.with(|file| *file.borrow_mut() = Some(changes::parse(&diff).remove(0)));
        drawn_with(|s, _| {
            DRAWN_FILE.with(|file| match file.borrow().as_ref() {
                Some(f) => crate::review_changes::file_diff(s, None, false, f, false, &Palette::new(true)),
                None => col(),
            })
        })
    };
    let mut rt = draw(THREE_HUNKS.to_owned());
    assert!(!texts(&rt).iter().any(|t| t.starts_with("@@ -20")), "{:?}", texts(&rt));
    press_words(&mut rt, "Expand full file · 1 hunk more, 2 lines");
    assert!(texts(&rt).iter().any(|t| t == "All 3 sections shown"), "{:?}", texts(&rt));
    let mut rt = draw(fourteen());
    assert!(texts(&rt).iter().any(|t| t == "4 unchanged lines"), "{:?}", texts(&rt));
    press_words(&mut rt, "2 unchanged lines");
    assert!(texts(&rt).contains(&"line 13".to_owned()));
}

#[test]
fn an_activity_drawers_cut_change_says_what_the_runtime_counted_in_all() {
    let item = locust_core::ledger::Item::Tool {
        name: "edit".into(),
        summary: "x".into(),
        detail: Box::new(locust_core::ledger::ToolDetail {
            id: "t1".into(),
            finished: true,
            observed_edit: true,
            patch: Some(locust_core::activity_entries::Patch {
                text: "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n".into(),
                added: 300,
                removed: 40,
                truncated: true,
            }),
            ..Default::default()
        }),
        status: locust_core::ledger::Status::Done,
    };
    DRAWN_ITEMS.with(|items| *items.borrow_mut() = vec![item]);
    let mut rt = drawn_with(|s, _| DRAWN_ITEMS.with(|items| crate::activity_drawers::activity_group(&items.borrow(), &Palette::new(true), s, "conv")));
    rt.update(|s, _| {
        s.activity_open.insert("conv/group/t1".into(), true);
        s.activity_open.insert("conv/t1/file/0".into(), true);
    });
    rt.frame(0.016);
    let said = texts(&rt);
    assert!(said.iter().any(|t| t.ends_with("the runtime counted +300 −40 in all")), "{said:?}");
}

// ---- the other places DiffView is drawn (DiffView.tsx's foot: Copy, and what the runtime counted in all) ----

fn copy_buttons(rt: &rune::Runtime<State>) -> usize {
    rt.elements().iter().filter(|e| e.label.as_deref().is_some_and(|l| l.starts_with("Copy"))).count()
}

#[test]
fn the_approval_card_offers_copy_of_the_new_code_at_the_foot_of_a_change_and_none_for_a_deleted_file() {
    // DiffView.tsx: `after !== undefined && <CopyButton ...>`; afterText has nothing for a file that is gone.
    let changed = approval_with("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n", false, 1, 1);
    assert_eq!(copy_buttons(&changed), 1, "{:?}", texts(&changed));
    let deleted = approval_with("--- a/x\n+++ /dev/null\n@@ -1 +0,0 @@\n-a\n", false, 0, 1);
    assert_eq!(copy_buttons(&deleted), 0, "{:?}", texts(&deleted));
}

/// A new file's activity row, opened and set to "Show the change" (`as_change`), its patch cut with the runtime
/// having counted +300 −40 across the whole change.
fn new_file_shown_as_its_change(path: &str) -> Vec<String> {
    let item = locust_core::ledger::Item::Tool {
        name: "write".into(),
        summary: "x".into(),
        detail: Box::new(locust_core::ledger::ToolDetail {
            id: "t1".into(),
            finished: true,
            observed_edit: true,
            patch: Some(locust_core::activity_entries::Patch {
                text: format!("--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+hello\n"),
                added: 300,
                removed: 40,
                truncated: true,
            }),
            ..Default::default()
        }),
        status: locust_core::ledger::Status::Done,
    };
    DRAWN_ITEMS.with(|items| *items.borrow_mut() = vec![item]);
    let mut rt = drawn_with(|s, _| DRAWN_ITEMS.with(|items| crate::activity_drawers::activity_group(&items.borrow(), &Palette::new(true), s, "conv")));
    rt.update(|s, _| {
        s.activity_open.insert("conv/group/t1".into(), true);
        s.activity_open.insert("conv/t1/file/0".into(), true);
        s.activity_open.insert("conv/t1/file/0/as-change".into(), true);
    });
    rt.frame(0.016);
    texts(&rt)
}

#[test]
fn a_new_documents_change_in_the_activity_drawer_says_what_the_runtime_counted_in_all() {
    let said = new_file_shown_as_its_change("notes.md");
    assert!(said.iter().any(|t| t.ends_with("the runtime counted +300 −40 in all")), "{said:?}");
}

#[test]
fn a_new_pages_change_in_the_activity_drawer_says_what_the_runtime_counted_in_all() {
    let said = new_file_shown_as_its_change("index.html");
    assert!(said.iter().any(|t| t.ends_with("the runtime counted +300 −40 in all")), "{said:?}");
}
