//! KEEP CARRIES THE WHOLE CHOICE (`renderer/src/keep-carries-the-whole-choice.test.tsx`, 0.622): keeping a column
//! carries its model AND the effort it ran at into the chat box; the mode stays the chat box's own, and the bar names
//! it. The rules (`route_after_keep`, `kept_sentence`) case for case, then Keep pressed in the whole window.
use super::*;
use crate::compare_view::{KeptRoute, kept_sentence, route_after_keep};
use crate::store::Store;
use locust_core::compare::{ChangesIn, CompareRoute, CompareSlotId, CompareStore, CreateCompareInput};
use rune::{Env, Runtime};
use serde_json::json;
use std::path::PathBuf;

fn model(id: &str, efforts: &[&str]) -> engine::Model {
    engine::Model { id: id.into(), name: id.into(), efforts: efforts.iter().map(|e| (*e).to_owned()).collect() }
}

/// Sonnet offers low, medium and high (medium its default); Haiku offers no levels.
fn claude() -> engine::Models {
    engine::Models {
        teammate: None,
        runtime: "claude".into(),
        model: None,
        effort: Some("medium".into()),
        models: vec![model("sonnet", &["low", "medium", "high"]), model("haiku", &[])],
    }
}

fn kept(model: &str, effort: Option<&str>) -> KeptRoute {
    let agents = claude();
    route_after_keep("claude", model, effort, &[&agents])
}

#[test]
fn sets_the_effort_the_kept_column_ran_at() {
    // case: sets the effort the kept column ran at
    assert_eq!(kept("sonnet", Some("high")), KeptRoute { runtime: "claude".into(), model: "sonnet".into(), effort: Some("high".into()) });
}

#[test]
fn lands_on_the_models_own_default_when_the_column_ran_at_a_level_it_does_not_offer() {
    // case: lands on the model's own default when the column ran at a level it does not offer
    assert_eq!(kept("sonnet", Some("ultra")).effort.as_deref(), Some("medium"));
}

#[test]
fn leaves_the_effort_unset_for_a_model_with_no_levels() {
    // case: leaves the effort unset for a model with no levels
    assert_eq!(kept("haiku", Some("high")).effort, None);
    assert_eq!(kept("haiku", None).effort, None);
}

#[test]
fn has_no_mode_in_what_it_carries() {
    // case: has no mode in what it carries
    // KeptRoute is runtime, model and effort, and nothing else: built whole here, a mode field would not compile.
    let KeptRoute { runtime, model, effort } = kept("sonnet", Some("high"));
    assert_eq!((runtime.as_str(), model.as_str(), effort.as_deref()), ("claude", "sonnet", Some("high")));
}

#[test]
fn names_the_mode_the_conversation_is_in() {
    // case: names the mode the conversation is in
    assert_eq!(kept_sentence(Some("Sonnet 5.5 · High"), Some("accept-edits"), None), "You kept Sonnet 5.5 · High; the conversation carries on with it in Edit.");
    assert_eq!(kept_sentence(Some("Sonnet 5.5 · High"), Some("ask"), None), "You kept Sonnet 5.5 · High; the conversation carries on with it in Ask.");
}

#[test]
fn says_so_when_the_comparison_ran_in_auto_and_the_conversation_does_not() {
    // case: says so when the comparison ran in Auto and the conversation does not
    assert_eq!(
        kept_sentence(Some("Sonnet 5.5 · High"), Some("accept-edits"), Some("auto")),
        "You kept Sonnet 5.5 · High; the conversation carries on with it in Edit, not in Auto as the comparison did, in copies of your folder."
    );
    assert!(kept_sentence(Some("X"), Some("ask"), Some("auto")).contains("in Ask, not in Auto"));
}

#[test]
fn does_not_apologise_when_the_modes_agree_and_promises_nothing_when_the_mode_is_not_known() {
    // case: does not apologise when the modes agree, and promises nothing when the mode is not known
    assert_eq!(kept_sentence(Some("X"), Some("auto"), Some("auto")), "You kept X; the conversation carries on with it in Auto.");
    assert_eq!(kept_sentence(Some("X"), None, None), "You kept X; the conversation carries on with it.");
}

// ---- in the window ----------------------------------------------------------------------------------------------

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A finished two-column comparison of Haiku at Low and at High, both run in Auto, for Codec, on screen.
fn window(keep_b_already: bool) -> (Dirs, Store, String, Runtime<State>) {
    let root = std::env::temp_dir().join(format!("locust-keep-carries-{}", locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    let route = |effort: &str, label: &str| CompareRoute {
        runtime: "claude".into(),
        model: "sonnet".into(),
        account: None,
        effort: Some(effort.into()),
        label: Some(label.into()),
        mode: Some("auto".into()),
        instructions: None,
        why: None,
        skill: None,
    };
    let compares = CompareStore::from_store(&store).unwrap();
    let cmp = compares
        .create(CreateCompareInput {
            teammate_id: Some("tm_codec".into()),
            prompt: "name three prime numbers".into(),
            routes: vec![route("low", "Sonnet · Low"), route("high", "Sonnet · High")],
            // Auto is kept on a column only where the comparison changes files (compare-store.ts:313).
            changes: true,
            changes_in: Some(ChangesIn::Folder),
            blind: false,
            base: None,
            rerun_of: None,
        })
        .unwrap();
    compares.add_turn(&cmp.compare_id, CompareSlotId::A, "m_a").unwrap();
    compares.add_turn(&cmp.compare_id, CompareSlotId::B, "m_b").unwrap();
    if keep_b_already {
        compares.keep(&cmp.compare_id, CompareSlotId::B, None).unwrap();
    }
    let mut s = State::default();
    s.data = Some(store.clone());
    apply_team(
        &mut s,
        vec![Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Code".into(),
            runtime: Some("claude".into()),
            model: Some("sonnet".into()),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: Some(root.join("work").to_string_lossy().into()),
        }],
    );
    s.models.insert("tm_codec".into(), vec![claude()]);
    // The person's chat box is in Edit when Keep is pressed.
    s.composer_mode = Some("accept-edits".into());
    for mission in ["m_a", "m_b"] {
        let said = ledger::Item::Assistant { item_id: "a1".into(), text: "2, 3, 5.".into(), r#final: true };
        s.compare.snapshots.insert(mission.into(), ledger::Snapshot { tip: mission.into(), items: vec![said], ..Default::default() });
    }
    s.compare.active_id = Some(cmp.compare_id.clone());
    crate::compare_view::reload_compares(&mut s);
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    settle(&mut rt);
    (Dirs(root), store, cmp.compare_id, rt)
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..4 {
        rt.frame(0.05);
    }
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn puts_the_mode_the_person_had_back_after_opening_the_kept_conversation() {
    // case: puts the mode the person had back after opening the kept conversation
    let (_dirs, store, id, mut rt) = window(false);
    let (x, y) = rt.find("compare-keep-b").expect("Keep this one, under the High column");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    let saved = CompareStore::from_store(&store).unwrap().get(&id).unwrap().unwrap();
    assert_eq!(saved.kept.map(|k| k.slot), Some(CompareSlotId::B), "the High column is kept");
    // The chat box carries on with the kept column's model at the effort it ran at...
    let pick = rt.state.picks.get("tm_codec").expect("Keep sets the chat box's route");
    assert_eq!((pick.model.as_str(), pick.effort.as_deref(), pick.runtime.as_deref()), ("sonnet", Some("high"), None));
    // ...and in the mode the person had, not the Auto the column ran in.
    assert_eq!(rt.state.composer_mode.as_deref(), Some("accept-edits"));
}

#[test]
fn is_what_the_comparison_view_draws() {
    // case: is what the comparison view draws
    let (_dirs, _store, _id, rt) = window(true);
    let said = words(&rt);
    assert!(
        said.contains("You kept Sonnet · High; the conversation carries on with it in Edit, not in Auto as the comparison did, in copies of your folder."),
        "{said}"
    );
}
