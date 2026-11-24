//! Parity tests for `apps/desktop/src/main/teammate-store.test.ts` plus full Electron carry-over.
use crate::bot_spec::seed_avatar;
use crate::look_picker::avatar_json;
use crate::native::{parse_roster, roster_in};
use crate::people::{
    MAX_MISSION_OWNERS, assign_mission, mission_owners, parsed_teammate, remember_route,
    remove_teammate, save_teammate_id, set_connectors, set_folder, team, update_teammate,
    valid_teammate_name,
};
use crate::roster_dropped::SAID;
use crate::store::Store;
use crate::workspace_settings::{self, WorkspaceSettings, write_settings};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

struct Temp {
    root: PathBuf,
    store: Store,
}
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-tm-store-{}",
            crate::native::uuid_v4().unwrap()
        ));
        let native = root.join("native");
        let electron = root.join("electron");
        fs::create_dir_all(&native).unwrap();
        fs::create_dir_all(&electron).unwrap();
        let store = Store::new(native, electron).unwrap();
        Self { root, store }
    }
    fn team_path(&self) -> PathBuf {
        self.store.root().join("teammates.json")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn find_teammate(store: &Store, id: &str) -> Value {
    team(store).unwrap()["teammates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["teammateId"] == id)
        .cloned()
        .unwrap()
}

fn create_basic(store: &Store, name: &str, hue: &str, role: &str) -> Value {
    let id = save_teammate_id(
        store,
        json!({"name": name, "hue": hue, "role": role}),
    )
    .unwrap();
    find_teammate(store, &id)
}

// ---- a teammate as a bot ----

#[test]
// case: keeps the bot the person picked, and only the fields it knows
fn keeps_the_bot_the_person_picked_and_only_the_fields_it_knows() {
    let t = Temp::new();
    let mut av = avatar_json(&seed_avatar("draft"));
    av["bot"] = json!({"shape": "swarm", "face": "mouth", "glow": true});
    av["extra"] = json!("x");
    let id = save_teammate_id(
        &t.store,
        json!({"name": "Sable", "hue": "pearl", "role": "Code & Migrations", "avatar": av}),
    )
    .unwrap();
    let made = find_teammate(&t.store, &id);
    assert_eq!(made["hue"], "pearl");
    assert_eq!(made["avatar"]["bot"], json!({"shape": "swarm", "face": "mouth"}));
    let mut keys: Vec<&str> = made["avatar"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(keys, vec!["accessory", "bot", "headwear", "mouth"]);
}

#[test]
// case: refuses a shape there is no bot for
fn refuses_a_shape_there_is_no_bot_for() {
    let t = Temp::new();
    let mut av = avatar_json(&seed_avatar("draft"));
    av["bot"] = json!({"shape": "robot", "face": "eyes"});
    let err = save_teammate_id(
        &t.store,
        json!({"name": "Nope", "hue": "lime", "role": "Code & Migrations", "avatar": av}),
    )
    .unwrap_err();
    assert!(err.contains("avatar is invalid"), "{err}");
}

#[test]
// case: takes every one of the nine colours
fn takes_every_one_of_the_nine_colours() {
    let t = Temp::new();
    for hue in [
        "lime", "blue", "violet", "clay", "teal", "butter", "rose", "slate", "pearl",
    ] {
        let made = create_basic(&t.store, &format!("Mate {hue}"), hue, "Code & Migrations");
        assert_eq!(made["hue"], hue);
    }
}

// ---- teammate names ----

#[test]
// case: accepts an ordinary name and trims it
fn accepts_an_ordinary_name_and_trims_it() {
    let t = Temp::new();
    let created = create_basic(&t.store, "  Wren  ", "lime", "Code & Migrations");
    assert_eq!(created["name"], "Wren");
}

#[test]
// case: refuses control characters rather than silently rewriting them
fn refuses_control_characters_rather_than_silently_rewriting_them() {
    assert!(!valid_teammate_name("Wren\0"));
    assert!(!valid_teammate_name("Wren\x1b[31m"));
    assert!(valid_teammate_name("Wren"));
}

#[test]
// case: refuses empty, whitespace-only and over-long names
fn refuses_empty_whitespace_only_and_over_long_names() {
    assert!(!valid_teammate_name(""));
    assert!(!valid_teammate_name("   "));
    assert!(!valid_teammate_name(&"x".repeat(41)));
    assert!(valid_teammate_name(&"x".repeat(40)));
}

// ---- teammate store ----

#[test]
// case: persists a teammate and reads it back from disk
fn persists_a_teammate_and_reads_it_back_from_disk() {
    let t = Temp::new();
    let created = create_basic(&t.store, "Atlas", "blue", "Research & Briefs");
    let id = created["teammateId"].as_str().unwrap();
    assert!(id.starts_with("tm_"));
    assert_eq!(id.len(), 27);
    assert!(id[3..].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));

    let reopened = Store::new(t.store.root().to_path_buf(), t.store.electron().to_path_buf()).unwrap();
    let list = roster_in(&reopened).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Atlas");
    assert_eq!(list[0].role, "Research & Briefs");
}

#[test]
// case: rejects an unknown hue or role instead of storing it
fn rejects_an_unknown_hue_or_role_instead_of_storing_it() {
    let t = Temp::new();
    assert!(
        save_teammate_id(&t.store, json!({"name": "X", "hue": "chartreuse", "role": "Code & Migrations"})).is_err()
    );
    assert!(
        save_teammate_id(&t.store, json!({"name": "X", "hue": "lime", "role": "Overlord"})).is_err()
    );
    assert!(roster_in(&t.store).unwrap().is_empty());
}

#[test]
// case: drops a malformed record but keeps the rest of the roster usable
fn drops_a_malformed_record_but_keeps_the_rest_of_the_roster_usable() {
    let t = Temp::new();
    let good = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let mut file: Value = serde_json::from_str(&fs::read_to_string(t.team_path()).unwrap()).unwrap();
    file["teammates"].as_array_mut().unwrap().push(json!({
        "teammateId": "tm_bad", "name": "", "hue": "lime", "role": "Code & Migrations", "createdAt": "x"
    }));
    fs::write(t.team_path(), file.to_string()).unwrap();

    let list = roster_in(&t.store).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, good["teammateId"].as_str().unwrap());
}

#[test]
// case: says so when it drops a record, rather than dropping it in silence
fn says_so_when_it_drops_a_record_rather_than_dropping_it_in_silence() {
    let t = Temp::new();
    create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let mut file: Value = serde_json::from_str(&fs::read_to_string(t.team_path()).unwrap()).unwrap();
    file["teammates"].as_array_mut().unwrap().push(json!({
        "teammateId": "tm_gem", "name": "Gem", "hue": "clay", "role": "Scout", "createdAt": "2026-09-05T05:00:00.000Z"
    }));
    fs::write(t.team_path(), file.to_string()).unwrap();

    SAID.with(|s| s.borrow_mut().clear());
    let _ = roster_in(&t.store).unwrap();
    let said = SAID.with(|s| s.borrow().clone());
    assert!(said.iter().any(|l| l.contains("dropped 1 teammate record")), "{said:?}");
}

#[test]
// case: says nothing when every record parses
fn says_nothing_when_every_record_parses() {
    let t = Temp::new();
    create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    SAID.with(|s| s.borrow_mut().clear());
    let _ = roster_in(&t.store).unwrap();
    let said = SAID.with(|s| s.borrow().clone());
    assert!(said.is_empty(), "{said:?}");
}

#[test]
// case: treats a file from an unknown schema as unreadable rather than guessing -- or overwriting
fn treats_a_file_from_an_unknown_schema_as_unreadable_rather_than_guessing_or_overwriting() {
    let t = Temp::new();
    let foreign = json!({"schemaVersion": 99, "teammates": [{"teammateId": "tm_x"}]}).to_string();
    fs::write(t.team_path(), &foreign).unwrap();
    assert!(roster_in(&t.store).is_err());
    assert!(save_teammate_id(&t.store, json!({"name": "Wren", "hue": "lime", "role": "Code & Migrations"})).is_err());
    assert_eq!(fs::read_to_string(t.team_path()).unwrap(), foreign);
}

#[test]
// case: treats a corrupt file as unreadable, and refuses to write over it
fn treats_a_corrupt_file_as_unreadable_and_refuses_to_write_over_it() {
    let t = Temp::new();
    fs::write(t.team_path(), "not json at all").unwrap();
    assert!(roster_in(&t.store).is_err());
    assert!(assign_mission(&t.store, "tm_x", "mission_1").is_err());
    assert_eq!(fs::read_to_string(t.team_path()).unwrap(), "not json at all");
}

#[test]
// case: assigns missions and forgets them when the teammate is removed
fn assigns_missions_and_forgets_them_when_the_teammate_is_removed() {
    let t = Temp::new();
    let wren = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = wren["teammateId"].as_str().unwrap();
    assign_mission(&t.store, tid, "mission_1").unwrap();
    assert_eq!(mission_owners(&t.store).unwrap().get("mission_1").map(String::as_str), Some(tid));

    remove_teammate(&t.store, tid).unwrap();
    assert!(roster_in(&t.store).unwrap().is_empty());
    assert!(mission_owners(&t.store).unwrap().is_empty());
}

#[test]
// case: refuses to assign a mission to a teammate that does not exist
fn refuses_to_assign_a_mission_to_a_teammate_that_does_not_exist() {
    let t = Temp::new();
    assert!(assign_mission(&t.store, "tm_ghost", "mission_1").is_err());
}

#[test]
// case: refuses unsafe ids
fn refuses_unsafe_ids() {
    let t = Temp::new();
    let wren = create_basic(&t.store, "Wren", "lime", "Docs & QA");
    let tid = wren["teammateId"].as_str().unwrap();
    assert!(assign_mission(&t.store, tid, "../escape").is_err());
    assert!(remove_teammate(&t.store, "../../etc").is_err());
}

#[test]
// case: serializes concurrent creates without losing any
fn serializes_concurrent_creates_without_losing_any() {
    let t = Temp::new();
    let store = Arc::new(t.store.clone());
    let handles: Vec<_> = ["A", "B", "C"]
        .into_iter()
        .map(|name| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                save_teammate_id(&store, json!({"name": name, "hue": "lime", "role": "Docs & QA"})).unwrap();
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let mut names: Vec<String> = roster_in(&t.store).unwrap().into_iter().map(|m| m.name).collect();
    names.sort();
    assert_eq!(names, vec!["A", "B", "C"]);
}

#[test]
// case: leaves no temporary files behind
fn leaves_no_temporary_files_behind() {
    let t = Temp::new();
    create_basic(&t.store, "Wren", "lime", "Docs & QA");
    let tmps: Vec<_> = fs::read_dir(t.store.root())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(tmps.is_empty());
}

// ---- how many missions the roster will hold ----

#[test]
// case: refuses a new assignment past its cap, instead of growing until the file empties itself
fn refuses_a_new_assignment_past_its_cap_instead_of_growing_until_the_file_empties_itself() {
    let t = Temp::new();
    let wren = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = wren["teammateId"].as_str().unwrap();
    let mut owners = serde_json::Map::new();
    for i in 0..MAX_MISSION_OWNERS {
        owners.insert(format!("mission_{i}"), json!(tid));
    }
    fs::write(
        t.team_path(),
        json!({"schemaVersion": 1, "teammates": [wren], "missionOwners": owners, "settings": {"swarm": false}}).to_string(),
    )
    .unwrap();
    assert!(assign_mission(&t.store, tid, "mission_one_too_many").is_err());
    assign_mission(&t.store, tid, "mission_0").unwrap();
}

#[test]
// case: reads no more assignments than it would write
fn reads_no_more_assignments_than_it_would_write() {
    let t = Temp::new();
    let wren = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = wren["teammateId"].as_str().unwrap();
    let mut owners = serde_json::Map::new();
    for i in 0..(MAX_MISSION_OWNERS + 50) {
        owners.insert(format!("mission_{i}"), json!(tid));
    }
    fs::write(
        t.team_path(),
        json!({"schemaVersion": 1, "teammates": [wren], "missionOwners": owners, "settings": {"swarm": false}}).to_string(),
    )
    .unwrap();
    assert_eq!(mission_owners(&t.store).unwrap().len(), MAX_MISSION_OWNERS);
}

// ---- teammate record parsing ----

#[test]
// case: accepts a well-formed record
fn accepts_a_well_formed_record() {
    assert!(parsed_teammate(&json!({
        "teammateId": "tm_1",
        "name": "Wren",
        "hue": "lime",
        "role": "Code & Migrations",
        "createdAt": "2026-08-31T16:00:00.000Z"
    }))
    .is_some());
}

#[test]
// case: refuses records missing or malforming any field
fn refuses_records_missing_or_malforming_any_field() {
    let base = json!({
        "teammateId": "tm_1",
        "name": "Wren",
        "hue": "lime",
        "role": "Code & Migrations",
        "createdAt": "2026-08-31T16:00:00.000Z"
    });
    for (k, v) in [
        ("teammateId", json!("../x")),
        ("hue", json!("gold")),
        ("role", json!("Anything")),
        ("createdAt", json!("not a date")),
    ] {
        let mut bad = base.clone();
        bad[k] = v;
        assert!(parsed_teammate(&bad).is_none(), "should refuse {k}");
    }
    assert!(parsed_teammate(&Value::Null).is_none());
}

// ---- a teammate's own route ----

#[test]
// case: is remembered when a person starts them, survives a rename, and is ignored when malformed
fn is_remembered_when_a_person_starts_them_survives_a_rename_and_is_ignored_when_malformed() {
    let t = Temp::new();
    let booty = create_basic(&t.store, "Booty", "violet", "Custom");
    let tid = booty["teammateId"].as_str().unwrap();
    assert!(booty.get("route").is_none());

    let route = json!({"runtime": "claude", "model": "sonnet", "mode": "ask"});
    remember_route(&t.store, tid, &route).unwrap();
    assert_eq!(find_teammate(&t.store, tid)["route"], route);

    let mut rename = booty.clone();
    rename["name"] = json!("Boots");
    let renamed = update_teammate(&t.store, rename).unwrap();
    assert_eq!(renamed["route"], route);

    remember_route(&t.store, tid, &json!({"runtime": "grok-desktop", "model": "x", "mode": "ask"})).unwrap();
    remember_route(&t.store, tid, &json!("cursor")).unwrap();
    remember_route(&t.store, "tm_nobody", &json!({"runtime": "cursor", "model": "composer-2.5", "mode": "accept-edits"})).unwrap();
    assert_eq!(find_teammate(&t.store, tid)["route"], route);
}

// ---- a Custom teammate's own words for their role ----

#[test]
// case: keeps a title on a Custom role, trimmed and bounded, and drops one on a built-in role
fn keeps_a_title_on_a_custom_role_trimmed_and_bounded_and_drops_one_on_a_built_in_role() {
    let t = Temp::new();
    let cid = save_teammate_id(
        &t.store,
        json!({"name": "Sable", "hue": "clay", "role": "Custom", "roleTitle": "  Release manager  "}),
    )
    .unwrap();
    let custom = find_teammate(&t.store, &cid);
    assert_eq!(custom["roleTitle"], "Release manager");

    let bid = save_teammate_id(
        &t.store,
        json!({"name": "Wren", "hue": "lime", "role": "Docs & QA", "roleTitle": "ignored"}),
    )
    .unwrap();
    assert!(find_teammate(&t.store, &bid).get("roleTitle").is_none());

    let mut edit = custom.clone();
    edit["roleTitle"] = json!("Reviews every PR for security");
    let edited = update_teammate(&t.store, edit).unwrap();
    assert_eq!(edited["roleTitle"], "Reviews every PR for security");
}

#[test]
// case: a title that is too long, empty, or not text is dropped rather than rejected
fn a_title_that_is_too_long_empty_or_not_text_is_dropped_rather_than_rejected() {
    let t = Temp::new();
    for bad in [
        json!("x".repeat(61)),
        json!("   "),
        json!(42),
        json!({"title": "x"}),
        json!("line\nbreak"),
    ] {
        let id = save_teammate_id(
            &t.store,
            json!({"name": "T", "hue": "lime", "role": "Custom", "roleTitle": bad}),
        )
        .unwrap();
        assert!(find_teammate(&t.store, &id).get("roleTitle").is_none());
        remove_teammate(&t.store, &id).unwrap();
    }
}

// ---- workspace settings ----

#[test]
// case: keeps the autonomy budget inside its bounds, and reads garbage as the default
fn keeps_the_autonomy_budget_inside_its_bounds_and_reads_garbage_as_the_default() {
    let t = Temp::new();
    assert_eq!(workspace_settings::load(&t.store).unwrap().relay_hop_cap, 12);
    write_settings(&t.store, &json!({"swarm": false, "relay": true, "relayHopCap": 2})).unwrap();
    assert_eq!(workspace_settings::load(&t.store).unwrap().relay_hop_cap, 2);
    for value in [json!(0), json!(25), json!(2.5), json!("4"), json!(-1), Value::Null] {
        write_settings(&t.store, &json!({"swarm": false, "relay": true, "relayHopCap": value})).unwrap();
        assert_eq!(workspace_settings::load(&t.store).unwrap().relay_hop_cap, 12);
    }
    write_settings(&t.store, &json!({"swarm": false, "relay": true, "relayHopCap": 24})).unwrap();
    assert_eq!(workspace_settings::load(&t.store).unwrap().relay_hop_cap, 24);
}

#[test]
// case: has teammate replies on by default, and only a literal false turns them off
fn has_teammate_replies_on_by_default_and_only_a_literal_false_turns_them_off() {
    let t = Temp::new();
    assert!(workspace_settings::load(&t.store).unwrap().relay);
    for value in [json!("false"), json!(0), Value::Null] {
        write_settings(&t.store, &json!({"swarm": false, "relay": value})).unwrap();
        assert!(workspace_settings::load(&t.store).unwrap().relay);
    }
    write_settings(&t.store, &json!({"swarm": false, "relay": false})).unwrap();
    let got = workspace_settings::load(&t.store).unwrap();
    assert_eq!(got, WorkspaceSettings { relay: false, ..WorkspaceSettings::default() });
}

#[test]
// case: keeps what a write leaves out
fn keeps_what_a_write_leaves_out() {
    let t = Temp::new();
    write_settings(
        &t.store,
        &json!({"layout": "wide", "tube": "subtle", "metal": "gold", "metalStrength": "strong"}),
    )
    .unwrap();
    write_settings(&t.store, &json!({"autoMode": true})).unwrap();
    let got = workspace_settings::load(&t.store).unwrap();
    assert!(got.auto_mode);
    assert_eq!(got.layout, "wide");
    assert_eq!(got.tube, "subtle");
    assert_eq!(got.metal, "gold");
    assert_eq!(got.metal_strength, "strong");

    write_settings(&t.store, &json!({"autoMode": "yes"})).unwrap();
    assert!(!workspace_settings::load(&t.store).unwrap().auto_mode);
}

#[test]
// case: defaults swarm off and persists a change
fn defaults_swarm_off_and_persists_a_change() {
    let t = Temp::new();
    assert_eq!(workspace_settings::load(&t.store).unwrap(), WorkspaceSettings::default());
    assert!(!workspace_settings::load(&t.store).unwrap().swarm);
    write_settings(&t.store, &json!({"swarm": true})).unwrap();
    let got = workspace_settings::load(&t.store).unwrap();
    assert_eq!(got, WorkspaceSettings { swarm: true, ..WorkspaceSettings::default() });
}

#[test]
// case: only a literal true turns it on
fn only_a_literal_true_turns_it_on() {
    let t = Temp::new();
    for value in [json!("true"), json!(1), json!({}), json!([]), Value::Null] {
        write_settings(&t.store, &json!({"swarm": value, "relay": false})).unwrap();
        let got = workspace_settings::load(&t.store).unwrap();
        assert_eq!(got, WorkspaceSettings { swarm: false, relay: false, ..WorkspaceSettings::default() });
    }
}

#[test]
// case: reads a corrupt settings block as the defaults without losing the roster
fn reads_a_corrupt_settings_block_as_the_defaults_without_losing_the_roster() {
    let t = Temp::new();
    let wren = create_basic(&t.store, "Wren", "lime", "Docs & QA");
    let mut file: Value = serde_json::from_str(&fs::read_to_string(t.team_path()).unwrap()).unwrap();
    file["settings"] = json!("not an object");
    fs::write(t.team_path(), file.to_string()).unwrap();

    assert_eq!(workspace_settings::load(&t.store).unwrap(), WorkspaceSettings::default());
    let list = roster_in(&t.store).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, wren["teammateId"].as_str().unwrap());
}

#[test]
// case: keeps teammates when settings are written
fn keeps_teammates_when_settings_are_written() {
    let t = Temp::new();
    create_basic(&t.store, "Wren", "lime", "Docs & QA");
    write_settings(&t.store, &json!({"swarm": true})).unwrap();
    assert_eq!(roster_in(&t.store).unwrap().len(), 1);
}

// ---- teammate faces ----

#[test]
// case: seeds a face from the id, never the name
fn seeds_a_face_from_the_id_never_the_name() {
    let t = Temp::new();
    let first = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let id1 = first["teammateId"].as_str().unwrap();
    remove_teammate(&t.store, id1).unwrap();
    let second = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let id2 = second["teammateId"].as_str().unwrap();
    assert_eq!(first["avatar"], avatar_json(&seed_avatar(id1)));
    assert_eq!(second["avatar"], avatar_json(&seed_avatar(id2)));
    assert_ne!(id1, id2);
}

#[test]
// case: keeps the look chosen in the dialog, and refuses one outside the tables
fn keeps_the_look_chosen_in_the_dialog_and_refuses_one_outside_the_tables() {
    let t = Temp::new();
    let chosen = json!({"headwear": 4, "accessory": 1, "mouth": 3});
    let id = save_teammate_id(
        &t.store,
        json!({"name": "Atlas", "hue": "blue", "role": "Research & Briefs", "avatar": chosen}),
    )
    .unwrap();
    assert_eq!(find_teammate(&t.store, &id)["avatar"], chosen);
    let err = save_teammate_id(
        &t.store,
        json!({"name": "Nova", "hue": "violet", "role": "Docs & QA", "avatar": {"headwear": 9, "accessory": 0, "mouth": 0}}),
    )
    .unwrap_err();
    assert!(err.contains("avatar is invalid"), "{err}");
}

#[test]
// case: keeps a persisted face rather than re-seeding it, and seeds one for a record without
fn keeps_a_persisted_face_rather_than_re_seeding_it_and_seeds_one_for_a_record_without() {
    let base = json!({
        "teammateId": "tm_abc", "name": "Wren", "hue": "lime",
        "role": "Code & Migrations", "createdAt": "2026-09-01T00:00:00.000Z"
    });
    let chosen = json!({"headwear": 2, "accessory": 2, "mouth": 1});
    let mut with_chosen = base.clone();
    with_chosen["avatar"] = chosen.clone();
    assert_eq!(parsed_teammate(&with_chosen).unwrap()["avatar"], chosen);
    assert_eq!(parsed_teammate(&base).unwrap()["avatar"], avatar_json(&seed_avatar("tm_abc")));
    let mut with_bad = base.clone();
    with_bad["avatar"] = json!({"headwear": "cap"});
    assert_eq!(parsed_teammate(&with_bad).unwrap()["avatar"], avatar_json(&seed_avatar("tm_abc")));
}

// ---- editing a teammate ----

#[test]
// case: changes name, hue, role and face while the id and its missions stay
fn changes_name_hue_role_and_face_while_the_id_and_its_missions_stay() {
    let t = Temp::new();
    let created = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = created["teammateId"].as_str().unwrap();
    assign_mission(&t.store, tid, "mission_1").unwrap();
    let look = json!({"headwear": 5, "accessory": 2, "mouth": 2});

    let updated = update_teammate(
        &t.store,
        json!({"teammateId": tid, "name": "  Wrenna ", "hue": "violet", "role": "Docs & QA", "avatar": look}),
    )
    .unwrap();
    assert_eq!(updated["teammateId"], tid);
    assert_eq!(updated["createdAt"], created["createdAt"]);
    assert_eq!(updated["name"], "Wrenna");
    assert_eq!(updated["hue"], "violet");
    assert_eq!(updated["role"], "Docs & QA");
    assert_eq!(updated["avatar"], look);
    assert_eq!(find_teammate(&t.store, tid), updated);
    assert_eq!(mission_owners(&t.store).unwrap().get("mission_1").map(String::as_str), Some(tid));
}

#[test]
// case: refuses an unknown teammate and an invalid field, changing nothing
fn refuses_an_unknown_teammate_and_an_invalid_field_changing_nothing() {
    let t = Temp::new();
    let created = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = created["teammateId"].as_str().unwrap();
    let err1 = update_teammate(
        &t.store,
        json!({"teammateId": "tm_nobody", "name": "X", "hue": "lime", "role": "Custom", "avatar": created["avatar"]}),
    )
    .unwrap_err();
    assert!(err1.contains("Unknown teammate"), "{err1}");
    let err2 = update_teammate(
        &t.store,
        json!({"teammateId": tid, "name": "X", "hue": "lime", "role": "Custom", "avatar": {"headwear": 99}}),
    )
    .unwrap_err();
    assert!(err2.contains("avatar is invalid"), "{err2}");
    assert_eq!(find_teammate(&t.store, tid), created);
}

// ---- memory mode ----

#[test]
// case: defaults to keeping memory and telling, keeps ask or off, and reads anything else as the default
fn defaults_to_keeping_memory_and_telling_keeps_ask_or_off_and_reads_anything_else_as_the_default() {
    let t = Temp::new();
    assert_eq!(workspace_settings::load(&t.store).unwrap().memory_mode, "auto");
    assert_eq!(write_settings(&t.store, &json!({"memoryMode": "ask"})).unwrap().memory_mode, "ask");
    assert_eq!(write_settings(&t.store, &json!({"memoryMode": "off"})).unwrap().memory_mode, "off");
    assert_eq!(write_settings(&t.store, &json!({"memoryMode": "sometimes"})).unwrap().memory_mode, "auto");
    assert_eq!(workspace_settings::load(&t.store).unwrap().memory_mode, "auto");
}

// ---- a teammate's own folder ----

#[test]
// case: is kept, cleared, and never left behind as an empty key
fn is_kept_cleared_and_never_left_behind_as_an_empty_key() {
    let t = Temp::new();
    let made = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = made["teammateId"].as_str().unwrap();
    assert!(made.get("folder").is_none());

    let home = "/home/<home>/claude";
    let pointed = set_folder(&t.store, tid, Some(home)).unwrap();
    assert_eq!(pointed["folder"], home);
    assert_eq!(find_teammate(&t.store, tid)["folder"], home);

    let cleared = set_folder(&t.store, tid, None).unwrap();
    assert!(!cleared.as_object().unwrap().contains_key("folder"));
    assert!(!find_teammate(&t.store, tid).as_object().unwrap().contains_key("folder"));
}

#[test]
// case: survives an edit of the name, the face and the branch switch
fn survives_an_edit_of_the_name_the_face_and_the_branch_switch() {
    let t = Temp::new();
    let made = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = made["teammateId"].as_str().unwrap();
    let home = "/home/<home>/claude";
    set_folder(&t.store, tid, Some(home)).unwrap();

    let edited = update_teammate(
        &t.store,
        json!({
            "teammateId": tid,
            "name": "Wren the second",
            "hue": "blue",
            "role": "Code & Migrations",
            "worktree": true,
            "avatar": avatar_json(&seed_avatar(tid))
        }),
    )
    .unwrap();
    assert_eq!(edited["name"], "Wren the second");
    assert_eq!(edited["worktree"], true);
    assert_eq!(edited["folder"], home);
}

#[test]
// case: refuses anything that is not an absolute path
fn refuses_anything_that_is_not_an_absolute_path() {
    let t = Temp::new();
    let made = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let tid = made["teammateId"].as_str().unwrap();
    assert!(set_folder(&t.store, tid, Some("claude")).is_err());
    assert!(set_folder(&t.store, tid, Some("")).is_err());
    assert!(set_folder(&t.store, tid, Some("   ")).is_err());
    assert!(set_folder(&t.store, "tm_nobody", Some("/tmp")).is_err());
    assert!(find_teammate(&t.store, tid).get("folder").is_none());
}

#[test]
// case: drops a relative folder read back from a hand-edited file
fn drops_a_relative_folder_read_back_from_a_hand_edited_file() {
    let base = json!({
        "teammateId": "tm_wren", "name": "Wren", "hue": "lime",
        "role": "Code & Migrations", "createdAt": "2026-09-05T05:00:00.000Z"
    });
    let mut rel = base.clone();
    rel["folder"] = json!("claude");
    assert!(parsed_teammate(&rel).unwrap().get("folder").is_none());
    let mut num = base.clone();
    num["folder"] = json!(12);
    assert!(parsed_teammate(&num).unwrap().get("folder").is_none());
    let home = "/home/<home>/claude";
    let mut abs = base.clone();
    abs["folder"] = json!(home);
    assert_eq!(parsed_teammate(&abs).unwrap()["folder"], home);
}

// ---- asking before every connector call ----

#[test]
// case: is off unless the file says exactly true
fn is_off_unless_the_file_says_exactly_true() {
    let t = Temp::new();
    assert!(!workspace_settings::load(&t.store).unwrap().ask_connectors);
    for wrong in [json!("true"), json!(1), json!("yes"), json!({})] {
        let written = write_settings(&t.store, &json!({"askConnectors": wrong})).unwrap();
        assert!(!written.ask_connectors);
    }
}

#[test]
// case: stays on across a write of some other setting
fn stays_on_across_a_write_of_some_other_setting() {
    let t = Temp::new();
    write_settings(&t.store, &json!({"askConnectors": true})).unwrap();
    let after = write_settings(&t.store, &json!({"swarm": true, "layout": "rail"})).unwrap();
    assert!(after.ask_connectors);
    assert!(workspace_settings::load(&t.store).unwrap().ask_connectors);
}

// ---- a teammate's own connectors ----

#[test]
// case: narrow, widen back, and never leave an empty key behind
fn narrow_widen_back_and_never_leave_an_empty_key_behind() {
    let t = Temp::new();
    let made = create_basic(&t.store, "Jimothy", "lime", "Custom");
    let tid = made["teammateId"].as_str().unwrap();
    assert!(made.get("connectors").is_none());
    let narrowed = set_connectors(
        &t.store,
        tid,
        &json!(["claude.ai Robinhood", "claude.ai Robinhood", "  "]),
    )
    .unwrap();
    assert_eq!(narrowed["connectors"], json!(["claude.ai Robinhood"]));
    let widened = set_connectors(&t.store, tid, &json!([])).unwrap();
    assert!(!widened.as_object().unwrap().contains_key("connectors"));
}

#[test]
// case: survive an edit of the name, and drop anything that is not a plain name
fn survive_an_edit_of_the_name_and_drop_anything_that_is_not_a_plain_name() {
    let t = Temp::new();
    let made = create_basic(&t.store, "Jimothy", "lime", "Custom");
    let tid = made["teammateId"].as_str().unwrap();
    set_connectors(
        &t.store,
        tid,
        &json!(["claude.ai Gmail", 12, "x".repeat(200), "ok\x07"]),
    )
    .unwrap();
    let edited = update_teammate(
        &t.store,
        json!({"teammateId": tid, "name": "Jim", "hue": "blue", "role": "Custom", "avatar": avatar_json(&seed_avatar(tid))}),
    )
    .unwrap();
    assert_eq!(edited["connectors"], json!(["claude.ai Gmail"]));
    assert!(set_connectors(&t.store, "tm_nobody", &json!(["claude.ai Gmail"])).is_err());
}

// ---- the plan a teammate keeps ----

#[test]
// case: is on when nobody has said otherwise
fn is_on_when_nobody_has_said_otherwise() {
    let t = Temp::new();
    assert!(workspace_settings::load(&t.store).unwrap().keep_a_todo_list);
}

#[test]
// case: is on for a settings file written before the setting existed
fn is_on_for_a_settings_file_written_before_the_setting_existed() {
    let t = Temp::new();
    fs::write(
        t.team_path(),
        json!({"schemaVersion": 1, "teammates": [], "missionOwners": {}, "settings": {"swarm": false, "relay": true}}).to_string(),
    )
    .unwrap();
    assert!(workspace_settings::load(&t.store).unwrap().keep_a_todo_list);
}

#[test]
// case: stays off once somebody turns it off
fn stays_off_once_somebody_turns_it_off() {
    let t = Temp::new();
    write_settings(&t.store, &json!({"keepATodoList": false})).unwrap();
    assert!(!workspace_settings::load(&t.store).unwrap().keep_a_todo_list);
}

#[test]
// case: reads anything that is not a boolean as the default
fn reads_anything_that_is_not_a_boolean_as_the_default() {
    let t = Temp::new();
    for value in [json!("false"), json!(0), json!({}), json!([]), Value::Null] {
        write_settings(&t.store, &json!({"keepATodoList": value})).unwrap();
        assert!(workspace_settings::load(&t.store).unwrap().keep_a_todo_list);
    }
}

// ---- the cursor bend is off until somebody asks for it ----

#[test]
// case: is off on a fresh profile
fn is_off_on_a_fresh_profile() {
    let t = Temp::new();
    assert!(!workspace_settings::load(&t.store).unwrap().metal_bend);
}

#[test]
// case: still honours a stored choice, in both directions
fn still_honours_a_stored_choice_in_both_directions() {
    let t = Temp::new();
    write_settings(&t.store, &json!({"metalBend": true})).unwrap();
    assert!(workspace_settings::load(&t.store).unwrap().metal_bend);
    write_settings(&t.store, &json!({"metalBend": false})).unwrap();
    assert!(!workspace_settings::load(&t.store).unwrap().metal_bend);
}

// ---- terminal faces are on until somebody turns them off ----

#[test]
// case: is on on a fresh profile
fn is_on_on_a_fresh_profile() {
    let t = Temp::new();
    assert!(workspace_settings::load(&t.store).unwrap().terminal_faces);
}

#[test]
// case: stays off once turned off, across a write of something else
fn stays_off_once_turned_off_across_a_write_of_something_else() {
    let t = Temp::new();
    write_settings(&t.store, &json!({"terminalFaces": false})).unwrap();
    write_settings(&t.store, &json!({"swarm": true})).unwrap();
    assert!(!workspace_settings::load(&t.store).unwrap().terminal_faces);
    write_settings(&t.store, &json!({"swarm": true, "terminalFaces": true})).unwrap();
    assert!(workspace_settings::load(&t.store).unwrap().terminal_faces);
}

// ---- a teammate's monthly limit ----

#[test]
// case: is kept when set, and an edit that never mentions money keeps it
fn is_kept_when_set_and_an_edit_that_never_mentions_money_keeps_it() {
    let t = Temp::new();
    let id = save_teammate_id(
        &t.store,
        json!({"name": "Wren", "hue": "lime", "role": "Code & Migrations", "monthlyLimitUsd": 5}),
    )
    .unwrap();
    let made = find_teammate(&t.store, &id);
    assert_eq!(made["monthlyLimitUsd"], 5);
    let renamed = update_teammate(
        &t.store,
        json!({"teammateId": id, "name": "Wren II", "hue": "lime", "role": "Code & Migrations", "avatar": made["avatar"]}),
    )
    .unwrap();
    assert_eq!(renamed["monthlyLimitUsd"], 5);
    assert_eq!(find_teammate(&t.store, &id)["monthlyLimitUsd"], 5);
}

#[test]
// case: is changed by an amount and lifted by null
fn is_changed_by_an_amount_and_lifted_by_null() {
    let t = Temp::new();
    let id = save_teammate_id(
        &t.store,
        json!({"name": "Wren", "hue": "lime", "role": "Code & Migrations", "monthlyLimitUsd": 5}),
    )
    .unwrap();
    let made = find_teammate(&t.store, &id);
    let raised = update_teammate(
        &t.store,
        json!({"teammateId": id, "name": "Wren", "hue": "lime", "role": "Code & Migrations", "avatar": made["avatar"], "monthlyLimitUsd": 12.5}),
    )
    .unwrap();
    assert_eq!(raised["monthlyLimitUsd"], 12.5);
    let lifted = update_teammate(
        &t.store,
        json!({"teammateId": id, "name": "Wren", "hue": "lime", "role": "Code & Migrations", "avatar": made["avatar"], "monthlyLimitUsd": null}),
    )
    .unwrap();
    assert!(!lifted.as_object().unwrap().contains_key("monthlyLimitUsd"));
    assert!(!find_teammate(&t.store, &id).as_object().unwrap().contains_key("monthlyLimitUsd"));
}

#[test]
// case: refuses an amount that is not one, rather than saving a limit nobody meant
fn refuses_an_amount_that_is_not_one_rather_than_saving_a_limit_nobody_meant() {
    let t = Temp::new();
    for bad in [json!(0), json!(-5), json!("5")] {
        let err = save_teammate_id(
            &t.store,
            json!({"name": "Wren", "hue": "lime", "role": "Code & Migrations", "monthlyLimitUsd": bad}),
        )
        .unwrap_err();
        assert!(err.contains("limit is invalid"), "{err}");
    }
    let made = create_basic(&t.store, "Wren", "lime", "Code & Migrations");
    let err = update_teammate(
        &t.store,
        json!({"teammateId": made["teammateId"], "name": "Wren", "hue": "lime", "role": "Code & Migrations", "avatar": made["avatar"], "monthlyLimitUsd": "NaN"}),
    )
    .unwrap_err();
    assert!(err.contains("limit is invalid"), "{err}");
}

#[test]
// case: reads a limit that does not parse as NO limit -- never as zero, which would refuse every run
fn reads_a_limit_that_does_not_parse_as_no_limit_never_as_zero_which_would_refuse_every_run() {
    let record = json!({
        "teammateId": "tm_x", "name": "X", "hue": "lime",
        "role": "Code & Migrations", "createdAt": "2026-09-26T00:00:00.000Z",
        "monthlyLimitUsd": "lots"
    });
    assert!(parsed_teammate(&record).unwrap().get("monthlyLimitUsd").is_none());
    let mut valid = record.clone();
    valid["monthlyLimitUsd"] = json!(7.25);
    assert_eq!(parsed_teammate(&valid).unwrap()["monthlyLimitUsd"], 7.25);
}

// ---- Full Electron carry-over test (every field Electron's teammate-store knows) ----

#[test]
fn electron_teammates_json_carries_over_every_field_and_edits_only_what_changed() {
    let t = Temp::new();
    let electron_file = json!({
        "schemaVersion": 1,
        "teammates": [
            {
                "teammateId": "tm_0123456789abcdef01234567",
                "name": "Sable",
                "hue": "coral",
                "role": "Custom",
                "roleTitle": "Release manager",
                "starters": ["Check release notes", "Summarize blockers"],
                "worktree": true,
                "folder": "/Users/dev/project-sable",
                "connectors": ["claude.ai Gmail", "claude.ai Linear"],
                "ownBrowser": true,
                "avatar": {
                    "headwear": 3,
                    "accessory": 2,
                    "mouth": 1,
                    "bot": {"shape": "swarm", "face": "mouth"}
                },
                "createdAt": "2026-10-01T12:34:56.789Z",
                "route": {
                    "runtime": "codex",
                    "model": "gpt-5.3-codex",
                    "mode": "accept-edits",
                    "account": "account_0123456789abcdef0123456789abcdef",
                    "effort": "high"
                },
                "hubMissionId": "mission_hub_01",
                "monthlyLimitUsd": 42.5,
                "instructions": "Always run tests before merging.",
                "voice": "maple"
            }
        ],
        "missionOwners": {
            "mission_01": "tm_0123456789abcdef01234567"
        },
        "missionTitles": {
            "mission_01": "Release prep"
        },
        "missionPins": ["mission_01"],
        "missionSettled": {
            "mission_02": {"at": "2026-10-02T00:00:00.000Z", "until": "2026-10-03T00:00:00.000Z"}
        },
        "settings": {
            "swarm": true,
            "relay": false,
            "relayHopCap": 18,
            "interrupt": true,
            "memoryMode": "ask",
            "autoMode": true,
            "askConnectors": true,
            "keepATodoList": false,
            "layout": "wide",
            "tube": "subtle",
            "replySize": "large",
            "metal": "gold",
            "metalStrength": "strong",
            "metalMotion": "always",
            "metalBend": true,
            "terminalFaces": false,
            "plush": true,
            "claudeOwnSkills": true,
            "checkCommands": {"ws_abc": "cargo test"},
            "checkFixes": {"ws_abc": 3},
            "aboutYou": "Prefers short answers.",
            "aboutYouSuggestions": [{"id": "sug_1", "text": "Uses macOS"}],
            "newChatRoute": {"runtime": "claude", "model": "sonnet", "mode": "ask"},
            "planRunsOut": {"runtime": "claude", "automatic": true, "model": "sonnet"}
        }
    });

    // Write Electron's teammates.json into Electron's folder (native seeds from it once).
    fs::write(t.store.electron().join("teammates.json"), electron_file.to_string()).unwrap();

    // 1. Loads cleanly via people::team, native::roster_in, and workspace_settings::load.
    let loaded = team(&t.store).unwrap();
    assert_eq!(loaded, electron_file);
    let roster = roster_in(&t.store).unwrap();
    assert_eq!(roster.len(), 1);
    assert_eq!(roster[0].name, "Sable");
    assert_eq!(roster[0].role, "Release manager");
    assert_eq!(roster[0].folder.as_deref(), Some("/Users/dev/project-sable"));
    assert_eq!(roster[0].runtime.as_deref(), Some("codex"));
    assert_eq!(roster[0].model.as_deref(), Some("gpt-5.3-codex"));
    assert_eq!(roster[0].account.as_deref(), Some("account_0123456789abcdef0123456789abcdef"));

    let settings = workspace_settings::load(&t.store).unwrap();
    assert!(settings.swarm);
    assert!(!settings.relay);
    assert_eq!(settings.relay_hop_cap, 18);
    assert!(settings.interrupt);
    assert_eq!(settings.memory_mode, "ask");
    assert!(settings.auto_mode);
    assert!(settings.ask_connectors);
    assert!(!settings.keep_a_todo_list);
    assert_eq!(settings.layout, "wide");
    assert_eq!(settings.tube, "subtle");
    assert_eq!(settings.reply_size, "large");
    assert_eq!(settings.metal, "gold");
    assert_eq!(settings.metal_strength, "strong");
    assert_eq!(settings.metal_motion, "always");
    assert!(settings.metal_bend);
    assert!(!settings.terminal_faces);
    assert!(settings.plush);
    assert!(settings.claude_own_skills);
    assert_eq!(settings.check_commands.get("ws_abc").map(String::as_str), Some("cargo test"));
    assert_eq!(settings.check_fixes.get("ws_abc").copied(), Some(3));
    assert_eq!(settings.about_you.as_deref(), Some("Prefers short answers."));

    // 2. Saving settings unchanged preserves every Electron field in teammates.json.
    workspace_settings::save(&t.store, &settings).unwrap();
    let after_settings_save = team(&t.store).unwrap();
    assert_eq!(after_settings_save["teammates"], electron_file["teammates"]);
    assert_eq!(after_settings_save["missionOwners"], electron_file["missionOwners"]);
    assert_eq!(after_settings_save["missionTitles"], electron_file["missionTitles"]);
    assert_eq!(after_settings_save["missionPins"], electron_file["missionPins"]);
    assert_eq!(after_settings_save["missionSettled"], electron_file["missionSettled"]);
    for (k, v) in electron_file["settings"].as_object().unwrap() {
        assert_eq!(
            after_settings_save["settings"].get(k),
            Some(v),
            "settings.{k} changed on save"
        );
    }

    // 3. Editing ONLY the teammate's name changes only `name` and preserves every other field.
    let before_tm = electron_file["teammates"][0].clone();
    let edited = update_teammate(
        &t.store,
        json!({
            "teammateId": before_tm["teammateId"],
            "name": "Sable II",
            "hue": before_tm["hue"],
            "role": before_tm["role"],
            "roleTitle": before_tm["roleTitle"],
            "worktree": before_tm["worktree"],
            "avatar": before_tm["avatar"]
        }),
    )
    .unwrap();
    let mut expected_tm = before_tm.clone();
    expected_tm["name"] = json!("Sable II");
    assert_eq!(edited, expected_tm);
    assert_eq!(find_teammate(&t.store, "tm_0123456789abcdef01234567"), expected_tm);

    // And top-level non-teammate fields stayed untouched.
    let final_file = team(&t.store).unwrap();
    assert_eq!(final_file["missionOwners"], electron_file["missionOwners"]);
    assert_eq!(final_file["missionTitles"], electron_file["missionTitles"]);
    assert_eq!(final_file["missionPins"], electron_file["missionPins"]);
    assert_eq!(final_file["missionSettled"], electron_file["missionSettled"]);
    for (k, v) in electron_file["settings"].as_object().unwrap() {
        assert_eq!(final_file["settings"].get(k), Some(v), "settings.{k} lost on edit");
    }

    // Also verify `people::save_teammate_id` directly keeps every field on a full-record edit.
    let mut direct = expected_tm.clone();
    direct["name"] = json!("Sable III");
    save_teammate_id(&t.store, direct.clone()).unwrap();
    assert_eq!(find_teammate(&t.store, "tm_0123456789abcdef01234567"), direct);
    assert_eq!(parse_roster(&team(&t.store).unwrap().to_string()).unwrap()[0].name, "Sable III");
}

#[test]
fn people_save_teammate_id_carries_omitted_fields() {
    let t = Temp::new();
    let id = save_teammate_id(
        &t.store,
        json!({
            "name": "Sable",
            "hue": "coral",
            "role": "Custom",
            "roleTitle": "Ops",
            "starters": ["Hi"],
            "connectors": ["claude.ai Gmail"],
            "ownBrowser": true,
            "route": {"runtime": "claude", "model": "sonnet", "mode": "ask"},
            "voice": "maple"
        }),
    )
    .unwrap();
    // Minimal save_teammate_id edit omitting starters/connectors/ownBrowser/route/voice carries them.
    save_teammate_id(
        &t.store,
        json!({"teammateId": id, "name": "Sable II", "hue": "coral", "role": "Custom", "roleTitle": "Ops"}),
    )
    .unwrap();
    let after = find_teammate(&t.store, &id);
    assert_eq!(after["name"], "Sable II");
    assert_eq!(after["starters"], json!(["Hi"]));
    assert_eq!(after["connectors"], json!(["claude.ai Gmail"]));
    assert_eq!(after["ownBrowser"], true);
    assert_eq!(after["route"], json!({"runtime": "claude", "model": "sonnet", "mode": "ask"}));
    assert_eq!(after["voice"], "maple");
}
