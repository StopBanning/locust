//! a-chain-template-opens-in-the-editor.test.tsx, the window's half: the editor opened on a chain (4 cases), drawn
//! headless, and "Use template" on a chain opening it there instead of the one-teammate preview (fixed 20 Nov: every
//! template went to the preview; the editor had no Role line and dropped the choices for one teammate on a chain).
//! The draft, card and list cases are in core/src/chain_draft_tests.rs.
use super::*;
use locust_core::routine_io::RoutineImportPreview;

fn mate(id: &str, name: &str, role: &str) -> Teammate {
    Teammate { id: id.into(), name: name.into(), role: role.into(), runtime: Some("opencode".into()), model: Some("opencode/free".into()), account: None, busy: false, runs_in: "accept-edits".into(), folder: None }
}

fn fix(with_roles: bool) -> RoutineImportPreview {
    RoutineImportPreview {
        token: String::new(),
        name: "Fix a bug, then check the fix".into(),
        steps: vec!["Find the cause.".into(), "Fix it.".into(), "Check the fix.".into()],
        inputs: vec![],
        hand_off_roles: if with_roles { vec![Some("Research & Briefs".into()), Some("Code & Migrations".into()), Some("Docs & QA".into())] } else { vec![None, None, None] },
        hand_off_checks: vec![false, false, true],
        hand_off_asks: None,
        changes_files: true,
        hand_off_on_another_agent: None,
        runtime: None,
        connectors: vec![],
    }
}

fn editor(team: Vec<Teammate>, preview: &RoutineImportPreview) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.teammates = team;
    s.routines.open = true;
    open_chain_draft(&mut s, preview);
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1100.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn three() -> Vec<Teammate> {
    vec![mate("tm_wren", "Wren", "Code & Migrations"), mate("tm_atlas", "Atlas", "Research & Briefs"), mate("tm_sable", "Sable", "Docs & QA")]
}

fn words(all: &[rune::ElementInfo]) -> String {
    all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn who_buttons(all: &[rune::ElementInfo]) -> usize {
    all.iter().filter(|e| e.label.as_deref().is_some_and(|l| l.starts_with("Who takes step "))).count()
}

// case: shows who takes each step, the checker marked on the last, and the run set to change files in a copy
#[test]
fn the_editor_shows_who_takes_each_step_the_checker_and_the_copy() {
    let all = editor(three(), &fix(true));
    assert_eq!(who_buttons(&all), 3);
    let said = words(&all);
    assert!(said.contains("Takes step 1: Atlas (runs it)"), "{said}");
    assert!(said.contains("Takes step 2: Wren"));
    assert!(said.contains("Takes step 3: Sable"));
    assert_eq!(said.matches("✓ Checker: must approve").count(), 1, "the last step alone checks");
    assert!(all.iter().any(|e| e.label.as_deref() == Some("In a copy, you keep, chosen")), "set to change files in a copy");
}

// case: says beside each step the role it was proposed for, and nothing when the routine has no roles
#[test]
fn each_step_says_its_proposed_role() {
    let said = words(&editor(three(), &fix(true)));
    for role in ["Role: Research & Briefs", "Role: Code & Migrations", "Role: Docs & QA"] {
        assert!(said.contains(role), "{role}");
    }
    assert!(!words(&editor(three(), &fix(false))).contains("Role: "));
}

// case: keeps the checker for a teammate on their own: every step is theirs and the last still has to approve
#[test]
fn one_teammate_keeps_the_checker() {
    let all = editor(vec![mate("tm_wren", "Wren", "Code & Migrations")], &fix(true));
    assert_eq!(who_buttons(&all), 3);
    let said = words(&all);
    assert!(said.contains("Wren (runs it)"));
    assert_eq!(said.matches("✓ Checker: must approve").count(), 1);
}

// case: still draws no choice for one teammate on a plain routine
#[test]
fn one_teammate_on_a_plain_routine_has_no_choice() {
    let mut s = State::default();
    s.teammates = vec![mate("tm_wren", "Wren", "Code & Migrations")];
    s.routines.open = true;
    edit(&mut s, json!({"name": "n", "steps": ["a", "b"], "teammateId": "tm_wren", "handOffs": [{}, {}]}));
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1100.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    assert_eq!(who_buttons(&rt.elements()), 0);
}

#[test]
fn use_template_opens_a_chain_in_the_editor_and_a_plain_one_in_the_preview() {
    let mut s = State::default();
    s.teammates = three();
    s.routines.open = true;
    s.routines.templates_open = true;
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1100.0, 4000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let press = |rt: &mut rune::Runtime<State>, key: &str| {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
    };
    press(&mut rt, "routine-use-template-fix-a-bug");
    assert!(rt.state.routines.import_preview.is_none(), "a chain is not given to one teammate");
    assert_eq!(rt.state.routines.entry["teammateId"], "tm_atlas", "{}", rt.state.routines.entry);
    assert!(rt.state.routines.entry["handOffs"].as_array().unwrap().iter().any(|h| h["check"] == true));
    assert_eq!(rt.state.routines.entry["mode"], "accept-edits");
}
