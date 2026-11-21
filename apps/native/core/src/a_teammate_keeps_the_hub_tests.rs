//! Electron's `main/a-teammate-keeps-the-hub-their-replies-live-in.test.ts`: the hub (the conversation a teammate's
//! replies to other teammates land in) is identity, like a route. It survives a rename, a new face and a relaunch, and
//! reads as absent, never as garbage, from a file anything could have edited.
//!
//! Rust's `hub_of` also asks that the hub's ledger still be there, so each hub a test expects to read back is given
//! one; a bad id is given one too, so only the id's own check can keep it out.
use super::*;

fn fixture() -> (std::path::PathBuf, Store) {
    let path = std::env::temp_dir().join(id("native-hub-test").unwrap());
    std::fs::create_dir_all(path.join("electron/mission-ledger")).unwrap();
    let store = Store::new(path.join("native"), path.join("electron")).unwrap();
    (path, store)
}

/// A turn's ledger for `mission`, where `hub_of` looks for it (a bad id in Electron's ledger folder, where its own path
/// leads).
fn ledger(store: &Store, mission: &str) {
    let path = match store.write_path(mission) {
        Ok(p) => p,
        Err(_) => store.electron().join("mission-ledger").join(format!("{mission}.jsonl")),
    };
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "{}\n").unwrap();
}

fn wren(store: &Store) -> String {
    save_teammate_id(store, json!({"teammateId": "", "name": "Wren", "hue": "lime", "role": "Code & Migrations"})).unwrap()
}

fn file_of(store: &Store) -> Value {
    team(store).unwrap()
}

/// 65 characters: a safe id's characters, one past teammate-store's `safeId` length.
const TOO_LONG: &str = "mission_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

// case: records the newest turn, and reads it back after a relaunch
#[test]
fn records_the_newest_turn_and_reads_it_back_after_a_relaunch() {
    let (root, store) = fixture();
    let wren = wren(&store);
    ledger(&store, "mission_hub1");
    ledger(&store, "mission_hub2");
    remember_hub(&store, &wren, "mission_hub1").unwrap();
    remember_hub(&store, &wren, "mission_hub2").unwrap();
    assert_eq!(hub_of(&store, &wren).as_deref(), Some("mission_hub2"));
    let again = Store::new(root.join("native"), root.join("electron")).unwrap();
    assert_eq!(hub_of(&again, &wren).as_deref(), Some("mission_hub2"));
    let _ = std::fs::remove_dir_all(root);
}

// case: survives a rename and a new face, the way the route does
#[test]
fn survives_a_rename_and_a_new_face_the_way_the_route_does() {
    let (root, store) = fixture();
    let wren = wren(&store);
    ledger(&store, "mission_hub1");
    ledger(&store, "mission_other");
    remember_hub(&store, &wren, "mission_hub1").unwrap();
    let avatar = file_of(&store)["teammates"][0]["avatar"].clone();
    // The edit never names the hub; one that does is not taken from it (the dialog never names one).
    save_teammate_id(&store, json!({"teammateId": wren, "name": "Wrenna", "hue": "blue", "role": "Docs & QA", "avatar": avatar})).unwrap();
    assert_eq!(hub_of(&store, &wren).as_deref(), Some("mission_hub1"));
    assert_eq!(file_of(&store)["teammates"][0]["name"], "Wrenna");
    save_teammate_id(&store, json!({"teammateId": wren, "name": "Wrenna", "role": "Docs & QA", "hubMissionId": "mission_other"})).unwrap();
    assert_eq!(file_of(&store)["teammates"][0]["hubMissionId"], "mission_hub1");
    // A new teammate starts with none, whatever the request says.
    let pip = save_teammate_id(&store, json!({"teammateId": "", "name": "Pip", "role": "Custom", "hubMissionId": "mission_other"})).unwrap();
    assert_eq!(hub_of(&store, &pip), None);
    let _ = std::fs::remove_dir_all(root);
}

// case: ignores an unknown teammate and a bad id rather than throwing at the relay
#[test]
fn ignores_an_unknown_teammate_and_a_bad_id_rather_than_throwing_at_the_relay() {
    let (root, store) = fixture();
    let wren = wren(&store);
    ledger(&store, "mission_hub1");
    ledger(&store, TOO_LONG);
    ledger(&store, "../hub");
    remember_hub(&store, "tm_nobody", "mission_hub1").unwrap();
    remember_hub(&store, &wren, "not a mission id").unwrap();
    remember_hub(&store, &wren, TOO_LONG).unwrap();
    remember_hub(&store, &wren, "../hub").unwrap();
    remember_hub(&store, "../tm", "mission_hub1").unwrap();
    // Electron's `7` (not text at all) cannot reach a `&str`.
    assert_eq!(hub_of(&store, &wren), None);
    let file = file_of(&store);
    assert_eq!(file["teammates"].as_array().unwrap().len(), 1, "no teammate was made up");
    assert_eq!(file["teammates"][0].get("hubMissionId"), None);
    let _ = std::fs::remove_dir_all(root);
}

// case: reads a hub that is not a mission id as no hub
#[test]
fn reads_a_hub_that_is_not_a_mission_id_as_no_hub() {
    let (root, store) = fixture();
    let wren = wren(&store);
    ledger(&store, "mission_hub1");
    ledger(&store, TOO_LONG);
    ledger(&store, "../hub");
    let with = |hub: Value| {
        let mut file = file_of(&store);
        file["teammates"][0]["hubMissionId"] = hub;
        store.save_json("teammates.json", &file).unwrap();
        hub_of(&store, &wren)
    };
    assert_eq!(with(json!("mission_hub1")).as_deref(), Some("mission_hub1"));
    assert_eq!(with(json!("../hub")), None);
    assert_eq!(with(json!(TOO_LONG)), None);
    assert_eq!(with(json!(42)), None);
    let mut file = file_of(&store);
    file["teammates"][0].as_object_mut().unwrap().remove("hubMissionId");
    store.save_json("teammates.json", &file).unwrap();
    assert_eq!(hub_of(&store, &wren), None);
    // A hub that does not read is not carried by the next edit either.
    with(json!("../hub"));
    save_teammate_id(&store, json!({"teammateId": wren, "name": "Wren", "role": "Code & Migrations"})).unwrap();
    assert_eq!(file_of(&store)["teammates"][0].get("hubMissionId"), None);
    let _ = std::fs::remove_dir_all(root);
}

// case: goes with the teammate when they are removed
#[test]
fn goes_with_the_teammate_when_they_are_removed() {
    let (root, store) = fixture();
    let wren = wren(&store);
    ledger(&store, "mission_hub1");
    remember_hub(&store, &wren, "mission_hub1").unwrap();
    remove_teammate(&store, &wren).unwrap();
    let text = std::fs::read_to_string(store.root().join("teammates.json")).unwrap_or_default();
    assert!(!text.contains("mission_hub1"), "{text}");
    let _ = std::fs::remove_dir_all(root);
}

// case: is untouched by an edit of the file that drops it
#[test]
fn is_untouched_by_an_edit_of_the_file_that_drops_it() {
    // A hand-edited roster with no hub reads as "no hub yet", and the next relayed reply begins one. Nothing crashes,
    // nothing is invented.
    let (root, store) = fixture();
    let wren = wren(&store);
    ledger(&store, "mission_hub1");
    ledger(&store, "mission_hub2");
    remember_hub(&store, &wren, "mission_hub1").unwrap();
    let mut file = file_of(&store);
    file["teammates"][0].as_object_mut().unwrap().remove("hubMissionId");
    std::fs::write(store.root().join("teammates.json"), file.to_string()).unwrap();
    assert_eq!(hub_of(&store, &wren), None);
    remember_hub(&store, &wren, "mission_hub2").unwrap();
    assert_eq!(hub_of(&store, &wren).as_deref(), Some("mission_hub2"));
    let _ = std::fs::remove_dir_all(root);
}
