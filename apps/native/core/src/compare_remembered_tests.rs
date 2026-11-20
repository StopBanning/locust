//! Electron's `a-comparison-is-remembered.test.ts` (11 cases): one ask to two or three models, each column its own
//! mission, recorded outside the ledger, so the columns come back after a restart and an unreadable file is never
//! taken for an empty one.
use super::*;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn root() -> Temp {
    let d = std::env::temp_dir().join(format!("locust-compares-{}", crate::invariants::unique_nanos()));
    fs::create_dir_all(&d).unwrap();
    Temp(d)
}
fn routes() -> Vec<CompareRoute> {
    vec![
        CompareRoute { label: Some("Fable 5.1".into()), ..CompareRoute::new("claude", "fable") },
        CompareRoute { label: Some("GPT-6 Astra".into()), ..CompareRoute::new("codex", "gpt-6-astra") },
    ]
}
fn input(prompt: &str, routes: Vec<CompareRoute>) -> CreateCompareInput {
    CreateCompareInput { teammate_id: None, prompt: prompt.into(), routes, changes: false, changes_in: None, blind: false, base: None, rerun_of: None }
}
fn store(t: &Temp) -> CompareStore {
    CompareStore::new(t.0.clone(), None).unwrap()
}
use CompareSlotId::{A, B};

// case: is recorded with a column per model, each turn added as it starts, and one kept
#[test]
fn is_recorded_with_a_column_per_model_each_turn_added_and_one_kept() {
    let t = root();
    let s = store(&t);
    let made = s.create(CreateCompareInput { teammate_id: Some("tm_wren".into()), ..input("  Add a discount code field.  ", routes()) }).unwrap();
    assert_eq!(made.prompt, "Add a discount code field.");
    let columns: Vec<(CompareSlotId, Option<&str>)> = made.slots.iter().map(|c| (c.slot, c.route.label.as_deref())).collect();
    assert_eq!(columns, [(A, Some("Fable 5.1")), (B, Some("GPT-6 Astra"))]);
    let id = made.compare_id.clone();
    s.add_turn(&id, A, "m_a1").unwrap();
    s.add_turn(&id, B, "m_b1").unwrap();
    s.add_turn(&id, A, "m_a2").unwrap();
    let kept = s.keep(&id, A, None).unwrap().kept.unwrap();
    assert_eq!(kept.slot, A);
    assert!(chrono::DateTime::parse_from_rfc3339(&kept.at).is_ok());
    // A restart: a new store over the same file reads the same thing.
    let again = store(&t).get(&id).unwrap().unwrap();
    assert_eq!(again.slots.iter().find(|c| c.slot == A).unwrap().mission_ids, ["m_a1", "m_a2"]);
    assert_eq!(again.kept.as_ref().map(|k| k.slot), Some(A));
    let mut members: Vec<String> = compare_membership(&[again]).into_keys().collect();
    members.sort();
    assert_eq!(members, ["m_a1", "m_a2", "m_b1"]);
}

// case: says why a column could not start, and cannot keep a column that has nothing
#[test]
fn says_why_a_column_could_not_start_and_cannot_keep_one_with_nothing() {
    let t = root();
    let s = store(&t);
    let id = s.create(input("Go", routes())).unwrap().compare_id;
    let refused = s.refuse(&id, B, "This column is still answering.").unwrap();
    assert_eq!(refused.slots[1].refused.as_deref(), Some("This column is still answering."));
    let err = s.keep(&id, B, None).unwrap_err();
    assert!(err.contains("nothing to keep"), "{err}");
}

// case: is two or three models, never one or four
#[test]
fn is_two_or_three_models_never_one_or_four() {
    let t = root();
    let s = store(&t);
    assert!(s.create(input("Go", routes()[..1].to_vec())).unwrap_err().contains("2 or 3"));
    let four: Vec<CompareRoute> = routes().into_iter().chain(routes()).collect();
    assert!(s.create(input("Go", four)).unwrap_err().contains("2 or 3"));
}

// case: treats a file it cannot read as unreadable, never as empty
#[test]
fn treats_a_file_it_cannot_read_as_unreadable_never_as_empty() {
    let t = root();
    fs::write(t.0.join("compares.json"), r#"{"schemaVersion":1,"compares":["#).unwrap();
    let s = store(&t);
    assert!(s.list().unwrap_err().contains("could not be read"));
    assert!(s.create(input("Go", routes())).unwrap_err().contains("could not be read"));
}

// case: may belong to nobody: no teammate is needed to compare
#[test]
fn may_belong_to_nobody() {
    let t = root();
    let made = store(&t).create(input("Go", routes())).unwrap();
    assert_eq!(made.teammate_id, None);
    let again = store(&t).get(&made.compare_id).unwrap().unwrap();
    assert_eq!((again.compare_id.as_str(), again.teammate_id.as_deref()), (made.compare_id.as_str(), None));
    assert_ne!(compare_slot_key(None, &made.compare_id, A), compare_slot_key(None, &made.compare_id, B));
}

// case: remembers that it edits, and which files the kept one brought into the folder (0.445)
#[test]
fn remembers_that_it_edits_and_which_files_the_kept_one_brought() {
    let t = root();
    let s = store(&t);
    let made = s.create(CreateCompareInput { changes: true, ..input("Fix the cart total.", routes()) }).unwrap();
    assert!(made.changes);
    s.add_turn(&made.compare_id, A, "m_a1").unwrap();
    s.keep(&made.compare_id, A, Some(vec!["cart.py".into(), "notes.md".into()])).unwrap();
    let again = store(&t).get(&made.compare_id).unwrap().unwrap();
    assert!(again.changes);
    let kept = again.kept.unwrap();
    assert_eq!((kept.slot, kept.brought), (A, Some(vec!["cart.py".to_owned(), "notes.md".to_owned()])));
    assert!(!s.create(input("Why?", routes())).unwrap().changes, "one that only answers says neither");
}

// case: keeps a column on Auto only in a comparison that edits (0.451)
#[test]
fn keeps_a_column_on_auto_only_in_a_comparison_that_edits() {
    let t = root();
    let auto: Vec<CompareRoute> = routes().into_iter().map(|r| CompareRoute { mode: Some("auto".into()), ..r }).collect();
    let edits = store(&t).create(CreateCompareInput { changes: true, ..input("Edit.", auto.clone()) }).unwrap().compare_id;
    let asks = store(&t).create(input("Ask.", auto)).unwrap().compare_id;
    let modes = |id: &str| store(&t).get(id).unwrap().unwrap().slots.iter().map(|c| c.route.mode.clone()).collect::<Vec<_>>();
    assert_eq!(modes(&edits), [Some("auto".to_owned()), Some("auto".to_owned())]);
    assert_eq!(modes(&asks), [None, None]);
}

// case: remembers that its names are hidden (0.449)
#[test]
fn remembers_that_its_names_are_hidden() {
    let t = root();
    let id = store(&t).create(CreateCompareInput { blind: true, ..input("Blind.", routes()) }).unwrap().compare_id;
    assert!(store(&t).get(&id).unwrap().unwrap().blind);
}

// case: gives each column a run slot of its own
#[test]
fn gives_each_column_a_run_slot_of_its_own() {
    assert_ne!(compare_slot_key(Some("tm_wren"), "cmp_1", A), compare_slot_key(Some("tm_wren"), "cmp_1", B));
    assert_ne!(compare_slot_key(Some("tm_wren"), "cmp_1", A), "tm_wren");
}

// case: in copies, or in the folder itself, survives a restart
#[test]
fn in_copies_or_in_the_folder_itself_survives_a_restart() {
    let t = root();
    let copy = store(&t).create(CreateCompareInput { changes: true, changes_in: Some(ChangesIn::Copy), ..input("Make a landing page.", routes()) }).unwrap().compare_id;
    let folder = store(&t).create(CreateCompareInput { changes: true, changes_in: Some(ChangesIn::Folder), ..input("Tidy the notes.", routes()) }).unwrap().compare_id;
    assert_eq!(store(&t).get(&copy).unwrap().unwrap().changes_in, Some(ChangesIn::Copy));
    assert_eq!(store(&t).get(&folder).unwrap().unwrap().changes_in, Some(ChangesIn::Folder));
}

// case: is its copy while it edits, and the folder once one is kept -- and it is never stored
#[test]
fn is_its_copy_while_it_edits_and_the_folder_once_one_is_kept_and_never_stored() {
    let t = root();
    let copies = t.0.join("copies");
    let s = CompareStore::new(t.0.clone(), Some(copies.clone())).unwrap();
    let made = s.create(CreateCompareInput { changes: true, changes_in: Some(ChangesIn::Copy), ..input("Review it.", routes()) }).unwrap();
    let id = made.compare_id.clone();
    let want = |slot: &str| Some(copies.join(format!("{id}-{slot}")).to_string_lossy().into_owned());
    assert_eq!(made.slots.iter().map(|c| c.folder.clone()).collect::<Vec<_>>(), [want("a"), want("b")]);
    assert_eq!(s.get(&id).unwrap().unwrap().slots[1].folder, want("b"));
    assert!(!fs::read_to_string(t.0.join("compares.json")).unwrap().contains("copies"));
    s.add_turn(&id, A, "m_a").unwrap();
    assert!(s.keep(&id, A, None).unwrap().slots.iter().all(|c| c.folder.is_none()));
}

// case: is the folder itself for one that answers, or one too big to copy
#[test]
fn is_the_folder_itself_for_one_that_answers_or_one_too_big_to_copy() {
    let t = root();
    let s = CompareStore::new(t.0.clone(), Some(t.0.join("copies"))).unwrap();
    assert!(s.create(input("Which?", routes())).unwrap().slots.iter().all(|c| c.folder.is_none()));
    assert!(s.create(CreateCompareInput { changes: true, changes_in: Some(ChangesIn::Folder), ..input("Tidy.", routes()) }).unwrap().slots.iter().all(|c| c.folder.is_none()));
}
