//! a-chain-template-opens-in-the-editor.test.tsx, the core half: the draft the editor opens on (5 cases), the saved
//! card's chain (3 cases) and the template list (1 case). The editor's own cases are drawn in
//! src/ui/chain_editor_tests.rs.
use super::*;
use crate::engine::Teammate;
use crate::routine_io::RoutineImportPreview;

fn who(id: &str, role: &str, runtime: &str) -> ProposableTeammate {
    ProposableTeammate { teammate_id: id.into(), role: role.into(), role_title: None, runtime: Some(runtime.into()) }
}

fn draft_mate(id: &str, role: &str, mode: Option<&str>) -> DraftTeammate {
    DraftTeammate { who: who(id, role, "opencode"), route: mode.map(|m| json!({"runtime": "opencode", "model": "opencode/free", "mode": m})) }
}

fn team(mode: &str) -> Vec<DraftTeammate> {
    vec![draft_mate("tm_wren", "Code & Migrations", Some(mode)), draft_mate("tm_atlas", "Research & Briefs", Some(mode)), draft_mate("tm_sable", "Docs & QA", Some(mode))]
}

fn preview(changes: bool) -> RoutineImportPreview {
    RoutineImportPreview {
        token: String::new(),
        name: "Fix a bug, then check the fix".into(),
        steps: vec!["Find the cause.".into(), "Fix it.".into(), "Check the fix.".into()],
        inputs: vec![],
        hand_off_roles: vec![Some("Research & Briefs".into()), Some("Code & Migrations".into()), Some("Docs & QA".into())],
        hand_off_checks: vec![false, false, true],
        hand_off_asks: None,
        changes_files: changes,
        hand_off_on_another_agent: None,
        runtime: None,
        connectors: vec![],
    }
}

fn fallback() -> Value {
    json!({"runtime": "codex", "model": "account-default", "mode": "ask"})
}

// case: proposes the roles, marks the checker, and opens a chain that changes files set to change them, in a copy
#[test]
fn proposes_the_roles_marks_the_checker_and_opens_in_a_copy() {
    let d = chain_draft_from(&preview(true), &team("accept-edits"), &fallback());
    assert_eq!(d.teammate_id.as_deref(), Some("tm_atlas"));
    assert_eq!(d.name, "Fix a bug, then check the fix");
    assert!(d.in_copy);
    assert_eq!(d.force_mode, Some("accept-edits"));
    assert_eq!(d.route.as_ref().unwrap()["runtime"], "opencode");
    assert_eq!(d.route.as_ref().unwrap()["mode"], "accept-edits");
    assert_eq!(d.hand_offs, vec![json!({}), json!({"teammateId": "tm_wren"}), json!({"teammateId": "tm_sable", "check": true})]);
    assert_eq!(d.step_roles, preview(true).hand_off_roles);
}

// case: opens a chain that only reads as it reads: no copy, no forced mode, the own mode of the teammate
#[test]
fn a_chain_that_only_reads_opens_as_it_reads() {
    let d = chain_draft_from(&preview(false), &team("ask"), &fallback());
    assert!(!d.in_copy);
    assert_eq!(d.force_mode, None);
    assert_eq!(d.route.unwrap()["mode"], "ask");
}

// case: replays a teammate with no route of their own on the one the box shows, changing files when the chain does
#[test]
fn a_teammate_with_no_route_takes_the_boxs_route() {
    let bare = vec![draft_mate("tm_wren", "Code & Migrations", None)];
    assert_eq!(chain_draft_from(&preview(true), &bare, &fallback()).route, Some(json!({"runtime": "codex", "model": "account-default", "mode": "accept-edits"})));
}

// case: has nobody to propose and no route when the roster is empty (the editor then says to add a teammate first)
#[test]
fn an_empty_roster_has_nobody_and_no_route() {
    let d = chain_draft_from(&preview(true), &[], &fallback());
    assert_eq!(d.teammate_id, None);
    assert_eq!(d.route, None);
    assert_eq!(d.hand_offs, vec![json!({}), json!({}), json!({"check": true})]);
}

// case: gives a Council review step to a teammate on another agent, as its template says (0.837)
#[test]
fn a_council_step_goes_to_a_teammate_on_another_agent() {
    let council = RoutineImportPreview {
        name: "Council review: three AIs check a change".into(),
        steps: vec!["Review it.".into(), "Review it again.".into(), "Settle it.".into()],
        hand_off_roles: vec![None, None, None],
        hand_off_on_another_agent: Some(vec![false, true, true]),
        ..preview(false)
    };
    let team: Vec<DraftTeammate> = [("tm_wren", "claude"), ("tm_atlas", "claude"), ("tm_sable", "codex")]
        .iter()
        .zip(["Code & Migrations", "Research & Briefs", "Docs & QA"])
        .map(|((id, rt), role)| DraftTeammate { who: who(id, role, rt), route: Some(json!({"runtime": rt, "model": "m", "mode": "ask"})) })
        .collect();
    assert_eq!(chain_draft_from(&council, &team, &fallback()).hand_offs, vec![json!({}), json!({"teammateId": "tm_sable"}), json!({"teammateId": "tm_atlas", "check": true})]);
}

fn mate(id: &str, name: &str, role: &str) -> Teammate {
    Teammate { id: id.into(), name: name.into(), role: role.into(), runtime: Some("opencode".into()), model: Some("opencode/free".into()), account: None, busy: false, runs_in: "accept-edits".into(), folder: None }
}

fn fix() -> Vec<ChainStep> {
    vec![
        ChainStep { role: Some("Research & Briefs".into()), check: false, another: false },
        ChainStep { role: Some("Code & Migrations".into()), check: false, another: false },
        ChainStep { role: Some("Docs & QA".into()), check: true, another: false },
    ]
}

fn roster() -> (Vec<Teammate>, Vec<ProposableTeammate>) {
    let t = vec![mate("tm_wren", "Wren", "Code & Migrations"), mate("tm_atlas", "Atlas", "Research & Briefs"), mate("tm_sable", "Sable", "Docs & QA")];
    let p = t.iter().map(|m| who(&m.id, &m.role, "opencode")).collect();
    (t, p)
}

// case: names the chain Atlas → Wren → Sable (checks), as any hand-off chain reads
#[test]
fn the_saved_card_names_the_chain() {
    let (t, p) = roster();
    let (_, hand_offs) = proposed_hand_offs(&fix(), &p);
    let routine = json!({"teammateId": "tm_atlas", "steps": ["a", "b", "c"], "handOffs": hand_offs});
    assert_eq!(crate::routines::routine_chain(&routine, &t).as_deref(), Some("Atlas → Wren → Sable (checks)"));
}

// case: says the checker is the same teammate when there is only one
#[test]
fn one_teammate_checks_their_own_chain() {
    let (t, p) = roster();
    let (_, hand_offs) = proposed_hand_offs(&fix(), &p[..1]);
    let routine = json!({"teammateId": "tm_wren", "steps": ["a", "b", "c"], "handOffs": hand_offs});
    assert_eq!(crate::routines::routine_chain(&routine, &t[..1]).as_deref(), Some("Wren → Wren (checks)"));
}

// case: names a model once when the whole chain runs on it
#[test]
fn a_model_is_named_once_for_the_whole_chain() {
    let (t, p) = roster();
    let (_, hand_offs) = proposed_hand_offs(&fix(), &p);
    let routine = json!({"teammateId": "tm_atlas", "steps": ["a", "b", "c"], "handOffs": hand_offs});
    let said = crate::routines::routine_chain_with_models(&routine, &t);
    assert!(said.starts_with("Atlas → Wren → Sable (checks), all on "), "{said}");
}

// case: says a chain hands work on, and says nothing new about the others
#[test]
fn the_template_list_says_a_chain_hands_work_on() {
    let info = |chain| RoutineTemplateInfo { id: "fix-a-bug".into(), name: "Fix a bug, then check the fix".into(), summary: "s".into(), steps: 3, asks: vec!["What is going wrong?".into()], chain };
    assert_eq!(template_meta(&info(true)), "3 steps · hand-offs · 1 question");
    assert_eq!(template_meta(&info(false)), "3 steps · 1 question");
}
