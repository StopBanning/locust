//! Electron's `a-comparison-that-edits-says-what-each-changed.test.tsx`, on the compare screen the window draws: the
//! bar says where each model's changes go (its own copy, Auto's reach past it, or the folder itself when it is too
//! big to copy), Keep says what keeping brings in, and each column's foot says what it changed before its time.
use super::*;
use locust_core::compare::{Compare, CompareSlot, CompareSlotId};
use rune::{Env, Runtime};

fn slot(id: CompareSlotId, model: &str, label: &str, mode: Option<&str>) -> CompareSlot {
    CompareSlot {
        slot: id,
        route: CompareRoute { runtime: "opencode".into(), model: model.into(), account: None, effort: None, label: Some(label.into()),
            mode: mode.map(Into::into), instructions: None, why: None, skill: None },
        mission_ids: vec![format!("m_{}", id.as_str())],
        refused: None,
        retried: vec![],
        folder: None,
    }
}

/// A finished comparison that edits, in copies unless `folder`, the first model on `mode`.
fn screen(mode: Option<&str>, folder: bool, lines: &[(CompareSlotId, &str)]) -> Runtime<State> {
    let mut s = State::default();
    for id in ["m_a", "m_b"] {
        let items = vec![ledger::Item::Assistant { item_id: format!("{id}_1"), text: "Done.".into(), r#final: true }];
        let timings = vec![locust_core::time_marker::TurnTiming { before_item: 0, first_at: Some("2026-10-12T18:00:00Z".into()), last_at: Some("2026-10-12T18:00:12Z".into()) }];
        s.compare.snapshots.insert(id.into(), ledger::Snapshot { tip: id.into(), items, timings, ..Default::default() });
    }
    s.compare.compares = vec![Compare {
        compare_id: "cmp_edit".into(),
        teammate_id: None,
        prompt: "Split total into parseRow and summarize.".into(),
        created_at: "2026-10-12T18:00:00.000Z".into(),
        slots: vec![
            slot(CompareSlotId::A, "opencode/nemotron-3-ultra-free", "Nemotron 3 Ultra Free", mode),
            slot(CompareSlotId::B, "opencode/mimo-v2.6-flash-free", "Mimo V2.6 Flash Free", None),
        ],
        changes: true,
        changes_in: Some(if folder { ChangesIn::Folder } else { ChangesIn::Copy }),
        blind: false,
        kept: None,
        judge: None,
        base: None,
        rerun_of: None,
        reruns: vec![],
        settled: false,
        point_drafts: vec![],
    }];
    for (id, line) in lines {
        s.compare.changes_lines.insert(("cmp_edit".into(), *id), (*line).into());
    }
    s.compare.active_id = Some("cmp_edit".into());
    s.compare.loaded = true;
    let mut rt = Runtime::new(s, |s: &State, _: &mut Cx<State>| view(s, &Palette::new(true)), (1400.0, 2400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn help(rt: &Runtime<State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key)).and_then(|e| e.help).unwrap_or_else(|| panic!("{key} has no help"))
}

#[test]
fn says_each_model_changes_its_own_copy_and_only_the_kept_one_comes_into_the_folder() {
    // case: says each model changes its own copy, and only the kept one comes into the folder
    let rt = screen(None, false, &[]);
    let said = words(&rt);
    assert!(said.contains("Each changes its own copy of your project; only the one you keep comes into your folder."), "{said}");
    assert!(!said.contains("They answer without changing files."), "{said}");
    let keeps = [help(&rt, "compare-keep-a"), help(&rt, "compare-keep-b")];
    assert!(keeps[0].contains("Its changes come into your folder, not committed"), "{keeps:?}");
    let everything = format!("{said}\n{}", keeps.join("\n")).to_lowercase();
    assert!(!everything.contains("worktree") && !everything.contains("branch"), "{everything}");
}

#[test]
fn on_auto_says_a_model_can_still_change_files_outside_its_copy() {
    // case: on Auto, says a model can still change files outside its copy
    assert!(words(&screen(Some("auto"), false, &[])).contains("On Auto, a model can still change files outside its copy if it is asked to."));
    assert!(!words(&screen(None, false, &[])).contains("On Auto"));
}

#[test]
fn in_a_folder_too_big_to_copy_says_they_all_work_in_the_folder_itself_and_keep_brings_nothing_in() {
    // case: in a folder too big to copy, says they all work in the folder itself, and Keep brings nothing in
    let rt = screen(None, true, &[]);
    let said = words(&rt);
    assert!(said.contains("This folder is too big to give each its own copy, so they all work in your folder itself."), "{said}");
    assert!(!said.contains("Each changes its own copy"), "{said}");
    assert!(help(&rt, "compare-keep-a").contains("its changes are there already"));
}

#[test]
fn puts_what_each_changed_at_its_foot_before_its_time() {
    // case: puts what each changed at its foot, before its time
    let rt = screen(None, false, &[(CompareSlotId::A, "+12 −3 in 2 files"), (CompareSlotId::B, "no changes")]);
    let at = |key: &str| rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default();
    assert_eq!(at("compare-number-a-0"), "+12 −3 in 2 files\u{a0}·");
    assert_eq!(at("compare-number-a-1"), "12s");
    assert_eq!(at("compare-number-b-0"), "no changes\u{a0}·");
    assert_eq!(at("compare-number-b-1"), "12s");
}
