//! The restore card (App.tsx `restoreFolders`), drawn and pressed as a person meets it, over a temporary home and data
//! folder: the folder found is offered, Use turns it off and on, Apply folders moves it and records the move, Later
//! puts the card away.

use super::*;
use serde_json::json;

const OLD: &str = "C:\\work\\demo";

struct Setup {
    _dir: crate::org::tests::Temp,
    home: PathBuf,
    local: PathBuf,
    ledger: PathBuf,
    rt: rune::Runtime<State>,
}

fn setup(open_folder: Option<&str>) -> Setup {
    let (dir, store) = crate::org::tests::temp_store();
    let home = store.root().parent().unwrap().join("home");
    let local = home.join("work").join("demo");
    std::fs::create_dir_all(&local).unwrap();
    let old_id = locust_core::people::workspace_id(Path::new(OLD));
    locust_core::folders::learn(&store, &[(old_id.clone(), OLD.to_owned())]).unwrap();
    // Written as the backup brought it: a teammate set to work in the other computer's folder.
    let teammate = |id: &str, name: &str, folder: &str| json!({"teammateId": id, "name": name, "role": "Custom", "hue": "teal", "createdAt": "2026-09-23T05:00:00.000Z", "folder": folder});
    let team = json!({"schemaVersion": 1, "teammates": [teammate("tm_wren", "Wren", OLD), teammate("tm_robin", "Robin", "/Users/someone/elsewhere")], "missionOwners": {}, "settings": {}});
    store.save_json("teammates.json", &team).unwrap();
    let mut settings = locust_core::workspace_settings::load(&store).unwrap();
    settings.check_commands.insert(old_id.clone(), "npm test".into());
    settings.check_fixes.insert(old_id.clone(), 2);
    locust_core::workspace_settings::save(&store, &settings).unwrap();
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    let ledger = store.ledger_dir().unwrap().join("mission_old.jsonl");
    let created = json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": "2026-11-01T00:00:00.000Z",
        "metadata": {"missionId": "mission_old", "runId": "run_old", "runtime": "codex", "workspaceId": old_id}});
    std::fs::write(&ledger, format!("{created}\n")).unwrap();
    let other = json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": "2026-11-01T00:00:00.000Z",
        "metadata": {"missionId": "mission_other", "runId": "run_other", "runtime": "codex", "workspaceId": "ws_another_folder"}});
    std::fs::write(store.ledger_dir().unwrap().join("mission_other.jsonl"), format!("{other}\n")).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    s.home.folder_path = open_folder.map(str::to_owned);
    assert!(show_in(&mut s, &home), "the missing folder is listed");
    let mut rt = rune::Runtime::new(
        s,
        |s, _| card(s, &Palette::new(true)).unwrap_or_else(|| col().key("no-card")),
        (900.0, 400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    Setup { _dir: dir, home, local, ledger, rt }
}

fn press(rt: &mut rune::Runtime<State>, key: &str) {
    rt.frame(0.016);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..40 {
        rt.frame(0.016);
    }
}

fn words(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n")
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn the_card_lists_the_missing_folder_with_the_one_found_already_chosen() {
    let t = setup(None);
    let said = words(&t.rt);
    assert!(said.contains(TITLE), "{said}");
    assert!(said.contains(OLD), "{said}");
    assert!(said.contains(&format!("Use {} ✓", path_text(&t.local))), "{said}");
    assert!(said.contains(LATER_NOTE), "{said}");
    for key in ["restore-folders-choose-0", "restore-folders-apply", "restore-folders-later"] {
        assert!(t.rt.find(key).is_some(), "{key}");
    }
    let _ = &t.home;
}

#[test]
fn use_turns_the_found_folder_off_and_on_and_apply_waits_for_a_choice() {
    let mut t = setup(None);
    press(&mut t.rt, "restore-folders-use-0");
    let said = words(&t.rt);
    assert!(said.contains(&format!("Use {}", path_text(&t.local))) && !said.contains("✓"), "{said}");
    assert!(t.rt.state.restore_folders.choices.is_empty());
    press(&mut t.rt, "restore-folders-apply");
    let store = t.rt.state.data.clone().unwrap();
    assert_eq!(locust_core::folders::path_of(&store, &locust_core::people::workspace_id(Path::new(OLD))).unwrap().as_deref(), Some(OLD), "nothing chosen, nothing moved");
    press(&mut t.rt, "restore-folders-use-0");
    assert!(words(&t.rt).contains(&format!("Use {} ✓", path_text(&t.local))));
}

#[test]
fn apply_folders_moves_the_folder_its_check_and_its_conversations_and_opens_it() {
    let mut t = setup(None);
    press(&mut t.rt, "restore-folders-apply");
    assert!(t.rt.find("restore-folders").is_none(), "the card is put away");
    let store = t.rt.state.data.clone().unwrap();
    let old_id = locust_core::people::workspace_id(Path::new(OLD));
    assert_eq!(locust_core::folders::path_of(&store, &old_id).unwrap(), Some(path_text(&t.local)), "the conversations keep their id");
    let settings = locust_core::workspace_settings::load(&store).unwrap();
    assert_eq!(settings.check_commands.get(&locust_core::people::workspace_id(&t.local)).map(String::as_str), Some("npm test"));
    assert_eq!(settings.check_fixes.get(&locust_core::people::workspace_id(&t.local)), Some(&2));
    let team = locust_core::people::team(&store).unwrap();
    let folder_of = |name: &str| team["teammates"].as_array().unwrap().iter().find(|m| m["name"] == name).unwrap()["folder"].clone();
    assert_eq!(folder_of("Wren"), json!(path_text(&t.local)), "the teammate that worked there works there still");
    assert_eq!(folder_of("Robin"), json!("/Users/someone/elsewhere"), "another teammate's folder is left alone");
    let text = std::fs::read_to_string(&t.ledger).unwrap();
    let moved: serde_json::Value = serde_json::from_str(text.lines().nth(1).expect("the move is recorded")).unwrap();
    assert_eq!(moved["event"]["type"], "adapter.diagnostic");
    assert_eq!(moved["event"]["payload"]["code"], "host.folder_moved");
    assert_eq!(moved["event"]["payload"]["message"], format!("Folder moved from {OLD} to {}.", path_text(&t.local)));
    assert_eq!(moved["event"]["id"], "run_old:folder:1");
    let untouched = std::fs::read_to_string(t.ledger.with_file_name("mission_other.jsonl")).unwrap();
    assert_eq!(untouched.lines().count(), 1, "another folder's conversation is not told it moved");
    assert_eq!(t.rt.state.home.folder_path.as_deref(), Some(path_text(&t.local).as_str()), "no folder was open, so this one is");
}

#[test]
fn apply_leaves_the_open_folder_alone_when_it_was_another_one() {
    let mut t = setup(Some("/Users/someone/elsewhere"));
    press(&mut t.rt, "restore-folders-apply");
    assert_eq!(t.rt.state.home.folder_path.as_deref(), Some("/Users/someone/elsewhere"));
}

#[test]
fn apply_waits_while_a_conversation_runs_and_says_so() {
    let mut t = setup(None);
    t.rt.update(|s, _| apply(s, true));
    t.rt.frame(0.016);
    assert!(words(&t.rt).contains(locust_core::restored_folders::WAIT_FOR_RUNS));
    let store = t.rt.state.data.clone().unwrap();
    assert_eq!(locust_core::folders::path_of(&store, &locust_core::people::workspace_id(Path::new(OLD))).unwrap().as_deref(), Some(OLD));
    assert_eq!(std::fs::read_to_string(&t.ledger).unwrap().lines().count(), 1, "no conversation is told it moved");
}

#[test]
fn a_folder_that_is_back_or_gone_again_is_not_applied() {
    let mut t = setup(None);
    std::fs::remove_dir_all(&t.local).unwrap();
    press(&mut t.rt, "restore-folders-apply");
    assert!(words(&t.rt).contains(locust_core::restored_folders::FOLDERS_CHANGED));
    // A path nobody offered for the row is refused whole.
    std::fs::create_dir_all(&t.local).unwrap();
    let id = t.rt.state.restore_folders.matches[0].id.clone();
    let unoffered = path_text(&t.home);
    t.rt.update(move |s, _| {
        s.restore_folders.choices.insert(id, unoffered);
        apply(s, false);
    });
    t.rt.frame(0.016);
    assert!(words(&t.rt).contains(NOT_APPLIED));
    // Offered or not, the home folder the scan looked under is not a project folder.
    let id = t.rt.state.restore_folders.matches[0].id.clone();
    let home = path_text(&t.home);
    t.rt.update(move |s, _| {
        s.restore_folders.offers.get_mut(&id).unwrap().insert(home.clone());
        s.restore_folders.choices.insert(id, home);
        s.restore_folders.message.clear();
        apply(s, false);
    });
    t.rt.frame(0.016);
    assert!(words(&t.rt).contains(locust_core::restored_folders::FOLDERS_CHANGED));
}

#[test]
fn a_picked_folder_is_offered_for_its_row_and_chosen() {
    let mut t = setup(None);
    let picked_folder = t.home.join("Projects").join("demo-copy");
    std::fs::create_dir_all(&picked_folder).unwrap();
    let id = t.rt.state.restore_folders.matches[0].id.clone();
    press(&mut t.rt, "restore-folders-choose-0");
    let title = t.rt.file_dialog().expect("the folder dialog opens").1.title.clone();
    assert_eq!(title, PICK_TITLE);
    t.rt.resolve_file_dialog(Some(picked_folder.clone())).unwrap();
    for _ in 0..40 {
        t.rt.frame(0.016);
    }
    assert_eq!(t.rt.state.restore_folders.choices.get(&id), Some(&path_text(&picked_folder)));
    assert!(t.rt.state.restore_folders.offers[&id].contains(&path_text(&picked_folder)));
    assert!(words(&t.rt).contains(&format!("Use {} ✓", path_text(&picked_folder))));
    // A row the card does not hold takes nothing, and nor does a folder a teammate should not work in.
    picked(&mut t.rt.state, "ws_other", &picked_folder);
    let before = t.rt.state.restore_folders.choices.clone();
    picked(&mut t.rt.state, &id, Path::new("/"));
    picked(&mut t.rt.state, &id, &t.home);
    assert_eq!(t.rt.state.restore_folders.choices, before);
    assert!(!t.rt.state.restore_folders.choices.contains_key("ws_other"));
}

#[test]
fn later_puts_the_card_away_and_nothing_is_moved() {
    let mut t = setup(None);
    press(&mut t.rt, "restore-folders-later");
    assert!(t.rt.find("restore-folders").is_none());
    let store = t.rt.state.data.clone().unwrap();
    assert_eq!(locust_core::folders::path_of(&store, &locust_core::people::workspace_id(Path::new(OLD))).unwrap().as_deref(), Some(OLD));
}

#[test]
fn nothing_missing_shows_no_card() {
    let (_dir, store) = crate::org::tests::temp_store();
    let top = store.root().parent().unwrap().to_path_buf();
    let here = top.join("here");
    std::fs::create_dir_all(&here).unwrap();
    locust_core::folders::learn(&store, &[(locust_core::people::workspace_id(&here), path_text(&here))]).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    assert!(!show_in(&mut s, &top));
    assert!(card(&s, &Palette::new(true)).is_none());
}

#[test]
fn choose_folder_always_opens_the_chooser_with_the_missing_folders_beside_it() {
    let mut t = setup(None);
    let home = t.home.clone();
    t.rt.update(move |s, tx| {
        s.restore_folders = Finding::default();
        choose_workspace_in(s, tx, Some(&home));
    });
    t.rt.frame(0.016);
    assert!(t.rt.find("restore-folders").is_some(), "the missing folder is listed");
    assert_eq!(t.rt.file_dialog().expect("the chooser opens beside the card").1.title, "Choose a project folder");
    // Nothing missing: the chooser alone, and no card.
    let (_dir, store) = crate::org::tests::temp_store();
    let mut s = State::default();
    s.data = Some(store);
    let mut rt = rune::Runtime::new(s, |_, _| col().key("empty"), (200.0, 200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let top = rt.state.data.as_ref().unwrap().root().parent().unwrap().to_path_buf();
    rt.update(move |s, tx| choose_workspace_in(s, tx, Some(&top)));
    rt.frame(0.016);
    assert_eq!(rt.file_dialog().expect("the folder dialog opens").1.title, "Choose a project folder");
    assert!(rt.state.restore_folders.matches.is_empty());
}
