use super::{tests::temp_store, *};
use crate::{ConversationRow, Palette, State, sidebar};
use rune::{Env, Runtime};

fn name(store: &Store, id: &str, title: &str) -> Result<(), String> {
    rename_mission(store, &mut load_org(store), id, title)
}

fn saved(store: &Store, titles: Value) {
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion":1, "teammates":[], "missionOwners": {"m_1":"tm_ash"},
                "missionTitles": titles
            }),
        )
        .unwrap();
}

fn row() -> ConversationRow {
    ConversationRow {
        id: "m_1".into(),
        teammate: "Ash".into(),
        title: "Original prompt".into(),
        started_by: "native".into(),
        state: "completed".into(),
        updated: "2026-11-28T00:00:00Z".into(),
    }
}

#[test]
// case: keeps the name
fn keeps_the_name() {
    let (_dir, store) = temp_store();
    let mut s = State::default();
    s.data = Some(store.clone());
    s.recent.push(row());
    sidebar::apply_org_action(&mut s, "m_1", "rename");
    assert_eq!(s.rename_id.as_deref(), Some("m_1"));
    s.rename_draft.replace_text("LOCUST.md — what it is for");
    let mut rt = Runtime::new(
        s,
        |s, _| sidebar::sidebar(s, &Palette::new(true)),
        (400.0, 900.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.05);
    let (x, y) = rt.find("rename-save").expect("Save the conversation title");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(load_org(&store).titles["m_1"], "LOCUST.md — what it is for");
}

#[test]
// case: does not touch the mission ledger
fn does_not_touch_the_mission_ledger() {
    let (_dir, store) = temp_store();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let ledger = dir.join("m_1.jsonl");
    let kept = b"{\"recordType\":\"mission.created\",\"metadata\":{\"missionId\":\"m_1\",\"prompt\":\"Original words\"}}\n";
    std::fs::write(&ledger, kept).unwrap();
    name(&store, "m_1", "Something else").unwrap();
    assert_eq!(std::fs::read(ledger).unwrap(), kept);
}

#[test]
// case: trims, because a name that is only spaces is not a name
fn trims_because_a_name_that_is_only_spaces_is_not_a_name() {
    let (_dir, store) = temp_store();
    name(&store, "m_1", "   Release notes   ").unwrap();
    assert_eq!(load_org(&store).titles["m_1"], "Release notes");
}

#[test]
// case: clears with an empty name, which is how you get the typed words back
fn clears_with_an_empty_name_which_is_how_you_get_the_typed_words_back() {
    let (_dir, store) = temp_store();
    name(&store, "m_1", "Named").unwrap();
    assert_eq!(load_org(&store).titles["m_1"], "Named");
    name(&store, "m_1", "").unwrap();
    let mut s = State::default();
    s.org = load_org(&store);
    s.recent.push(row());
    assert!(!s.org.titles.contains_key("m_1"));
    assert_eq!(sidebar::recent_as_missions(&s)[0].title, "Original prompt");
}

#[test]
// case: treats blank as empty
fn treats_blank_as_empty() {
    let (_dir, store) = temp_store();
    name(&store, "m_1", "Named").unwrap();
    name(&store, "m_1", "   ").unwrap();
    assert!(!load_org(&store).titles.contains_key("m_1"));
}

#[test]
// case: clearing a name that was never set is not an error
fn clearing_a_name_that_was_never_set_is_not_an_error() {
    let (_dir, store) = temp_store();
    assert_eq!(name(&store, "m_never", ""), Ok(()));
    assert!(!store.teammates_json_path().unwrap().exists());
}

#[test]
// case: refuses an id that is not one
fn refuses_an_id_that_is_not_one() {
    let (_dir, store) = temp_store();
    assert_eq!(
        name(&store, "../escape", "x"),
        Err("Mission id is invalid".into())
    );
    assert!(load_org(&store).titles.is_empty());
}

#[test]
// case: caps how long one name may be
fn caps_how_long_one_name_may_be() {
    let (_dir, store) = temp_store();
    name(&store, "m_1", &"x".repeat(MAX_MISSION_TITLE_LENGTH + 500)).unwrap();
    assert_eq!(
        load_org(&store).titles["m_1"].len(),
        MAX_MISSION_TITLE_LENGTH
    );
}

fn almost_full(store: &Store, title: &str) {
    let entries = (0..MAX_MISSION_TITLES - 1)
        .map(|i| (format!("m_{i}"), json!(title)))
        .collect();
    saved(store, Value::Object(entries));
}

#[test]
// case: caps how many conversations may carry one
fn caps_how_many_conversations_may_carry_one() {
    let (_dir, store) = temp_store();
    let longest = "x".repeat(MAX_MISSION_TITLE_LENGTH);
    almost_full(&store, &longest);
    name(&store, "m_last", &longest).unwrap();
    let mut full = load_org(&store);
    assert_eq!(full.titles.len(), MAX_MISSION_TITLES);
    assert_eq!(
        rename_mission(&store, &mut full, "m_one_too_many", "nope"),
        Err("Too many renamed conversations".into())
    );
    assert_eq!(load_org(&store).titles.len(), MAX_MISSION_TITLES);
}

#[test]
// case: still lets an existing one be renamed at the cap
fn still_lets_an_existing_one_be_renamed_at_the_cap() {
    let (_dir, store) = temp_store();
    almost_full(&store, "first");
    name(&store, "m_last", "first").unwrap();
    assert_eq!(load_org(&store).titles.len(), MAX_MISSION_TITLES);
    name(&store, "m_0", "second").unwrap();
    assert_eq!(load_org(&store).titles["m_0"], "second");
}

#[test]
// case: drops a title that is not a string
fn drops_a_title_that_is_not_a_string() {
    let (_dir, store) = temp_store();
    saved(&store, json!({"m_1":42, "m_2":"fine"}));
    let titles = load_org(&store).titles;
    assert!(!titles.contains_key("m_1"));
    assert_eq!(titles["m_2"], "fine");
}

#[test]
// case: drops one whose id could not be an id
fn drops_one_whose_id_could_not_be_an_id() {
    let (_dir, store) = temp_store();
    saved(&store, json!({"../../etc":"no", "m_ok":"yes"}));
    let titles = load_org(&store).titles;
    assert!(!titles.contains_key("../../etc"));
    assert_eq!(titles["m_ok"], "yes");
}

#[test]
// case: survives the field being absent, which every file written before today is
fn survives_the_field_being_absent_which_every_file_written_before_today_is() {
    let (_dir, store) = temp_store();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion":1,"teammates":[],"missionOwners":{}}),
        )
        .unwrap();
    let loaded = load_org(&store);
    assert!(loaded.titles.is_empty() && !loaded.teammates_org_unreadable);
}

#[test]
// case: never applies ownership from a roster read without applying titles too
fn never_applies_ownership_from_a_roster_read_without_applying_titles_too() {
    // Native reads owners and organization separately; the invariant is that the same row
    // keeps both facts, not that a React roster response updates adjacent cells.
    let (_dir, store) = temp_store();
    saved(&store, json!({"m_1":"The person's name"}));
    let mut s = State::default();
    s.data = Some(store);
    s.org = load_org(s.data.as_ref().unwrap());
    s.recent.push(row());
    let rows = sidebar::recent_as_missions(&s);
    assert_eq!(sidebar::row_owner(&s, &rows[0]).as_deref(), Some("tm_ash"));
    assert_eq!(rows[0].title, "The person's name");
}

#[test]
// case: reads them in the load that runs at startup, not only on a refresh
fn reads_them_in_the_load_that_runs_at_startup_not_only_on_a_refresh() {
    const CHILD: &str = "LOCUST_TEST_STARTUP_CONVERSATION_NAME";
    if std::env::var_os(CHILD).is_some() {
        let mut s = State::default();
        assert!(s.data.is_none());
        crate::composer_wiring::load_composer_store(&mut s);
        assert!(
            s.data.is_some(),
            "the first startup load must find the isolated data"
        );
        s.recent.push(row());
        assert_eq!(
            sidebar::recent_as_missions(&s)[0].title,
            "Kept before startup"
        );
        return;
    }
    let (_dir, store) = temp_store();
    saved(&store, json!({"m_1":"Kept before startup"}));
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "org::conversation_naming_tests::reads_them_in_the_load_that_runs_at_startup_not_only_on_a_refresh", "--nocapture"])
        .env(CHILD, "1")
        .env("LOCUST_NATIVE_DATA", store.root())
        .output().unwrap();
    let output = String::from_utf8_lossy(&child.stdout);
    assert!(
        child.status.success() && output.contains("1 passed"),
        "{output}\n{}",
        String::from_utf8_lossy(&child.stderr)
    );
}
