//! Electron's `groups-hold-conversations.test.ts`, case by case, through the window's own path: the sidebar's
//! organization calls in `org.rs` (which write `groups.json` through `locust_core::groups`), `org::load_org` (the
//! read the window does when it opens), the row menu `context_menu::conversation_menu` builds, and the sidebar's
//! "Name this project" Save.
use super::*;
use locust_core::groups::{GroupRoute, MAX_GROUPS, MAX_GROUP_INSTRUCTIONS_LENGTH, MAX_GROUP_NAME_LENGTH};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh profile (native and Electron folders), its organization read the way the window reads it at startup.
struct Profile {
    temp: std::path::PathBuf,
    store: crate::store::Store,
    org: org::OrgState,
}

impl Profile {
    fn new() -> Profile {
        Profile::with_file(None)
    }

    /// `groups` is written as the native `groups.json` before anything reads it.
    fn with_file(groups: Option<&str>) -> Profile {
        let temp = std::env::temp_dir().join(format!(
            "locust-groups-hold-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&temp);
        let (electron, native) = (temp.join("electron"), temp.join("native"));
        std::fs::create_dir_all(&electron).unwrap();
        std::fs::create_dir_all(&native).unwrap();
        if let Some(body) = groups {
            std::fs::write(native.join("groups.json"), body).unwrap();
        }
        let store = crate::store::Store::new(native, electron).unwrap();
        let org = org::load_org(&store);
        Profile { temp, store, org }
    }

    fn file(&self) -> std::path::PathBuf {
        self.temp.join("native").join("groups.json")
    }

    /// What a window opened now would read.
    fn reread(&self) -> org::OrgState {
        org::load_org(&self.store)
    }

    fn create(&mut self, name: &str) -> Result<String, String> {
        org::create_group(&self.store, &mut self.org, name)
    }

    fn assign(&mut self, mission: &str, group: Option<&str>) -> Result<(), String> {
        org::assign_to_group(&self.store, &mut self.org, mission, group)
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp);
    }
}

fn names(state: &org::OrgState) -> Vec<String> {
    state.groups.iter().map(|g| g.name.clone()).collect()
}

// making and naming a group

#[test]
fn groups_hold_keeps_the_name_it_was_given() {
    // case: keeps what it was called
    let mut p = Profile::new();
    p.create("Investments").unwrap();
    assert_eq!(names(&p.reread()), vec!["Investments".to_owned()]);
}

#[test]
fn groups_hold_refuses_a_blank_name() {
    // case: refuses a name that is only spaces
    let mut p = Profile::new();
    assert!(p.create("   ").is_err());
    assert!(p.reread().groups.is_empty());
}

#[test]
fn groups_hold_flattens_a_name_onto_one_line() {
    // case: flattens a name to one line, because a row is one line
    let mut p = Profile::new();
    p.create("Trading\n\nideas").unwrap();
    assert_eq!(names(&p.reread()), vec!["Trading ideas".to_owned()]);
}

#[test]
fn groups_hold_caps_a_long_name() {
    // case: caps the length rather than letting a name become a paragraph
    let mut p = Profile::new();
    p.create(&"x".repeat(60 + 100)).unwrap();
    assert_eq!(MAX_GROUP_NAME_LENGTH, 60);
    assert_eq!(p.reread().groups[0].name.chars().count(), 60);
}

#[test]
fn groups_hold_renames_a_group() {
    // case: renames one
    let mut p = Profile::new();
    let made = p.create("Old").unwrap();
    org::rename_group(&p.store, &mut p.org, &made, "New").unwrap();
    assert_eq!(names(&p.reread()), vec!["New".to_owned()]);
}

#[test]
fn groups_hold_says_so_renaming_a_missing_group() {
    // case: says so when renaming one that is not there
    let mut p = Profile::new();
    let err = org::rename_group(&p.store, &mut p.org, "grp_nope", "x").unwrap_err();
    assert!(err.contains("does not exist"), "{err}");
}

#[test]
fn groups_hold_will_not_make_more_than_the_cap() {
    // case: will not make more than it can list
    let mut p = Profile::new();
    assert_eq!(MAX_GROUPS, 64);
    for index in 0..64 {
        p.create(&format!("g{index}")).unwrap();
    }
    assert!(p.create("one too many").is_err());
    assert_eq!(p.reread().groups.len(), 64);
}

// putting a conversation in one

#[test]
fn groups_hold_remembers_where_a_conversation_went() {
    // case: remembers where it went
    let mut p = Profile::new();
    let made = p.create("Investments").unwrap();
    p.assign("m_1", Some(&made)).unwrap();
    assert_eq!(p.reread().members.get("m_1").map(|m| m.group_id.clone()), Some(made));
}

#[test]
fn groups_hold_takes_a_conversation_out_with_no_group() {
    // case: takes it out again with no group
    let mut p = Profile::new();
    let made = p.create("Investments").unwrap();
    p.assign("m_1", Some(&made)).unwrap();
    p.assign("m_1", None).unwrap();
    assert!(p.reread().members.get("m_1").is_none());
}

#[test]
fn groups_hold_moves_rather_than_doubling() {
    // case: moves it rather than putting it in two places
    let mut p = Profile::new();
    let one = p.create("One").unwrap();
    let two = p.create("Two").unwrap();
    p.assign("m_1", Some(&one)).unwrap();
    p.assign("m_1", Some(&two)).unwrap();
    let listed = p.reread();
    assert_eq!(listed.members.get("m_1").map(|m| m.group_id.clone()), Some(two));
    assert_eq!(listed.members.len(), 1);
}

#[test]
fn groups_hold_refuses_assigning_to_a_missing_group() {
    // case: refuses a group that does not exist
    let mut p = Profile::new();
    assert!(p.assign("m_1", Some("grp_nope")).is_err());
    assert!(p.reread().members.is_empty());
}

#[test]
fn groups_hold_refuses_a_conversation_id_that_escapes() {
    // case: refuses an id that could not be one
    let mut p = Profile::new();
    assert!(p.assign("../escape", None).is_err());
}

// removing a group keeps the conversations

#[test]
fn groups_hold_removing_a_group_leaves_its_conversations_ungrouped() {
    // case: lets them fall back to ungrouped rather than deleting them
    let mut p = Profile::new();
    let made = p.create("Temporary").unwrap();
    p.assign("m_1", Some(&made)).unwrap();
    org::remove_group(&p.store, &mut p.org, &made).unwrap();
    let listed = p.reread();
    assert!(listed.groups.is_empty());
    assert!(listed.members.get("m_1").is_none());
    assert!(p.org.members.get("m_1").is_none(), "the window's own state shows it ungrouped too");
}

#[test]
fn groups_hold_removing_a_missing_group_is_fine() {
    // case: removing one that is not there is not an error
    let mut p = Profile::new();
    assert_eq!(org::remove_group(&p.store, &mut p.org, "grp_nope"), Ok(()));
}

// the file is read as untrusted

#[test]
fn groups_hold_drops_a_dangling_membership() {
    // case: drops a membership pointing at a group that is gone
    let p = Profile::with_file(Some(r#"{"schemaVersion":1,"groups":[],"members":{"m_1":"grp_vanished"}}"#));
    assert!(!p.org.groups_unreadable);
    assert!(p.org.members.get("m_1").is_none());
}

#[test]
fn groups_hold_drops_a_nameless_group_keeps_others() {
    // case: drops a group with no usable name and keeps the rest
    let p = Profile::with_file(Some(
        r#"{"schemaVersion":1,"groups":[{"groupId":"grp_a","name":""},{"groupId":"grp_b","name":"Fine"}],"members":{}}"#,
    ));
    let ids: Vec<&str> = p.org.groups.iter().map(|g| g.group_id.as_str()).collect();
    assert_eq!(ids, vec!["grp_b"]);
}

#[test]
fn groups_hold_a_torn_file_is_unreadable_and_kept() {
    // case: treats a file that does not parse as unreadable too, and refuses to write over it
    let torn = r#"{"schemaVersion":1,"groups":["#;
    let mut p = Profile::with_file(Some(torn));
    assert!(p.org.groups_unreadable, "a torn file is unreadable, never no groups");
    assert!(p.create("Trading").is_err());
    assert!(p.assign("mission_1", None).is_err());
    // The store under the window says why, in the word Electron's store throws.
    assert_eq!(locust_core::groups::create_group(&p.store, "Trading").unwrap_err(), "GROUPS_UNREADABLE");
    assert_eq!(locust_core::groups::assign_to_group(&p.store, "mission_1", None).unwrap_err(), "GROUPS_UNREADABLE");
    assert_eq!(std::fs::read_to_string(p.file()).unwrap(), torn);
}

// what a group is, beyond a folder

#[test]
fn groups_hold_takes_refuses_and_clears_a_default_route() {
    // case: takes a default route, refuses one a mission could not start on, and clears it
    let mut p = Profile::new();
    let made = p.create("Trading").unwrap();
    let route = GroupRoute {
        runtime: "opencode".into(),
        model: "opencode/muse-spark-1.3-contributor-free".into(),
        mode: "ask".into(),
        account: None,
        effort: Some("low".into()),
    };
    org::set_group_route(&p.store, &mut p.org, &made, Some(route.clone())).unwrap();
    assert_eq!(p.reread().groups[0].route, Some(route.clone()));
    let bad = GroupRoute { model: String::new(), effort: None, ..route.clone() };
    let err = org::set_group_route(&p.store, &mut p.org, &made, Some(bad)).unwrap_err();
    assert!(err.contains("not one a mission can start on"), "{err}");
    let err = org::set_group_route(&p.store, &mut p.org, "grp_missing", Some(route.clone())).unwrap_err();
    assert!(err.contains("does not exist"), "{err}");
    assert_eq!(p.reread().groups[0].route, Some(route));
    org::set_group_route(&p.store, &mut p.org, &made, None).unwrap();
    assert_eq!(p.reread().groups[0].route, None);
}

#[test]
fn groups_hold_a_new_group_has_empty_instructions() {
    // case: carries standing instructions from the start
    let mut p = Profile::new();
    let made = p.create("Trading").unwrap();
    let held = p.org.groups.iter().find(|g| g.group_id == made).unwrap();
    assert_eq!(held.instructions, "");
    let raw: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p.file()).unwrap()).unwrap();
    assert_eq!(raw["groups"][0]["instructions"], "", "written from the start, not added later");
}

#[test]
fn groups_hold_keeps_what_the_file_already_carries() {
    // case: keeps instructions and a route that are already in the file
    let mut p = Profile::with_file(Some(
        r#"{"schemaVersion":1,"groups":[{"groupId":"grp_a","name":"Trading","createdAt":"2026-09-15T00:00:00.000Z","instructions":"Analysis only. Never place an order.","route":{"runtime":"claude","model":"sonnet","mode":"read-only"}}],"members":{}}"#,
    ));
    let held = &p.org.groups[0];
    assert!(held.instructions.contains("Never place an order"), "{}", held.instructions);
    // Native reads a route for use only when a start could take it, and "read-only" is not a mode a start takes; the
    // route is still kept as the file had it, through a later write.
    org::rename_group(&p.store, &mut p.org, "grp_a", "Trading desk").unwrap();
    let raw: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p.file()).unwrap()).unwrap();
    assert_eq!(raw["groups"][0]["route"]["runtime"], "claude");
    assert_eq!(raw["groups"][0]["instructions"], "Analysis only. Never place an order.");
}

// groups are read back when the app opens

#[test]
fn groups_hold_the_startup_load_reads_groups() {
    // case: asks for groups in the load that runs at startup
    // The load the window runs when it opens its store is the one that reads organization, groups with it.
    let wiring = include_str!("composer_wiring.rs");
    let start = wiring.find("fn load_composer_store").unwrap();
    let body = &wiring[start..start + 800];
    assert!(body.contains("s.org = org::load_org(&store);"), "{body}");
    let p = Profile::with_file(Some(
        r#"{"schemaVersion":1,"groups":[{"groupId":"grp_a","name":"Trading","createdAt":"2026-09-15T00:00:00.000Z"}],"members":{}}"#,
    ));
    assert_eq!(names(&p.org), vec!["Trading".to_owned()]);
}

#[test]
fn groups_hold_the_startup_load_applies_names_and_members() {
    // case: applies both halves of the answer, not just the names
    let p = Profile::with_file(Some(
        r#"{"schemaVersion":1,"groups":[{"groupId":"grp_a","name":"Trading","createdAt":"2026-09-15T00:00:00.000Z"}],"members":{"m_1":{"groupId":"grp_a","at":"2026-09-15T01:00:00.000Z"}}}"#,
    ));
    assert_eq!(names(&p.org), vec!["Trading".to_owned()]);
    assert_eq!(p.org.members.get("m_1").map(|m| m.group_id.as_str()), Some("grp_a"));
}

// moving a conversation is one row, not one row per group

/// A window on `p`'s profile with one conversation, `m_1`, in the sidebar.
fn window_on(p: &Profile) -> State {
    let mut s = State::default();
    s.org = p.reread();
    s.data = Some(p.store.clone());
    s.recent.push(ConversationRow {
        id: "m_1".into(),
        title: "Read the quarterly".into(),
        teammate: String::new(),
        started_by: "native".into(),
        state: "completed".into(),
        updated: "2026-09-15T05:00:00.000Z".into(),
    });
    s
}

fn move_list(s: &State) -> Vec<crate::context_menu::Item> {
    let menu = crate::context_menu::conversation_menu(s, "m_1", "Read the quarterly", (10.0, 10.0));
    let row = menu.items.iter().find(|i| i.label == "Move to project").expect("a Move to project row");
    row.submenu.clone().expect("Move to project opens a list")
}

/// "New project…" chosen on `m_1`, then the sidebar's Save pressed with `name` typed.
fn new_project_from_menu(s: State, name: &str) -> State {
    let item = move_list(&s).into_iter().find(|i| i.label == "New project…").expect("New project… in the list");
    let mut s = s;
    apply_org_action(&mut s, "m_1", &item.action);
    assert_eq!(s.rename_id.as_deref(), Some("@new-project:m_1"));
    s.rename_draft.replace_text(name);
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let (x, y) = rt.find("rename-save").expect("the Save under Name this project");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    rt.state
}

#[test]
fn groups_hold_move_to_project_is_always_offered_and_makes_one() {
    // case: is offered always, as Move to project, and can make a new project
    let p = Profile::new();
    let s = window_on(&p);
    let labels: Vec<String> = move_list(&s).iter().map(|i| i.label.clone()).collect();
    assert!(labels.contains(&"New project…".to_owned()), "{labels:?}");
    let s = new_project_from_menu(s, "Investments");
    let listed = p.reread();
    assert_eq!(names(&listed), vec!["Investments".to_owned()]);
    assert_eq!(listed.members.get("m_1").map(|m| m.group_id.clone()), Some(listed.groups[0].group_id.clone()));
    assert_eq!(s.org.members.get("m_1").map(|m| m.group_id.clone()), Some(listed.groups[0].group_id.clone()));
}

#[test]
fn groups_hold_the_move_list_ticks_the_current_project() {
    // case: ticks the one it is already in, so the list also answers where it is
    let mut p = Profile::new();
    let trading = p.create("Trading").unwrap();
    p.create("Reading").unwrap();
    p.assign("m_1", Some(&trading)).unwrap();
    let s = window_on(&p);
    let ticked: Vec<(String, bool)> = move_list(&s).iter().map(|i| (i.label.clone(), i.checked)).collect();
    assert!(ticked.contains(&("Trading".to_owned(), true)), "{ticked:?}");
    assert!(ticked.contains(&("Reading".to_owned(), false)), "{ticked:?}");
    assert!(ticked.contains(&("Ungrouped".to_owned(), false)), "{ticked:?}");
}

#[test]
fn groups_hold_ungrouped_is_a_destination_in_the_list() {
    // case: offers Ungrouped as a destination rather than a separate verb
    let mut p = Profile::new();
    let trading = p.create("Trading").unwrap();
    p.assign("m_1", Some(&trading)).unwrap();
    let mut s = window_on(&p);
    let menu = crate::context_menu::conversation_menu(&s, "m_1", "Read the quarterly", (10.0, 10.0));
    assert!(
        !menu.items.iter().any(|i| i.label.to_lowercase().contains("ungroup") || i.label.starts_with("Remove from")),
        "no separate verb at the top level"
    );
    let ungrouped = move_list(&s).into_iter().find(|i| i.label == "Ungrouped").expect("Ungrouped in the list");
    apply_org_action(&mut s, "m_1", &ungrouped.action);
    assert!(p.reread().members.get("m_1").is_none(), "choosing Ungrouped takes it out");
}

#[test]
fn groups_hold_a_new_project_named_like_an_old_one_gets_the_conversation() {
    // case: picks the newest match when two groups share a name
    let mut p = Profile::new();
    let older = p.create("Investments").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let s = window_on(&p);
    new_project_from_menu(s, "Investments");
    let listed = p.reread();
    assert_eq!(names(&listed), vec!["Investments".to_owned(), "Investments".to_owned()]);
    let newest = listed.groups.iter().max_by_key(|g| g.created_at.clone()).unwrap();
    assert_ne!(newest.group_id, older);
    assert_eq!(listed.members.get("m_1").map(|m| m.group_id.clone()), Some(newest.group_id.clone()));
}

// standing instructions on a group

#[test]
fn groups_hold_instructions_keep_what_was_written() {
    // case: keeps what was written
    let mut p = Profile::new();
    let made = p.create("Trading").unwrap();
    org::set_group_instructions(&p.store, &mut p.org, &made, "Analysis only. Never place an order.").unwrap();
    assert!(p.reread().groups[0].instructions.contains("Never place an order"));
}

#[test]
fn groups_hold_instructions_clear_with_blank_text() {
    // case: clears with empty text, so a group can stop briefing
    let mut p = Profile::new();
    let made = p.create("Trading").unwrap();
    org::set_group_instructions(&p.store, &mut p.org, &made, "something").unwrap();
    org::set_group_instructions(&p.store, &mut p.org, &made, "   ").unwrap();
    assert_eq!(p.reread().groups[0].instructions, "");
}

#[test]
fn groups_hold_instructions_are_capped() {
    // case: caps the length
    let mut p = Profile::new();
    let made = p.create("Trading").unwrap();
    org::set_group_instructions(&p.store, &mut p.org, &made, &"x".repeat(9_000)).unwrap();
    assert_eq!(MAX_GROUP_INSTRUCTIONS_LENGTH, 4000);
    assert_eq!(p.reread().groups[0].instructions.chars().count(), 4000);
}

#[test]
fn groups_hold_instructions_refuse_a_missing_group() {
    // case: refuses a group that does not exist, rather than writing nowhere
    let mut p = Profile::new();
    assert!(org::set_group_instructions(&p.store, &mut p.org, "grp_nope", "x").is_err());
    assert!(!p.file().exists(), "nothing was written");
}

// when a conversation joined

#[test]
fn groups_hold_the_join_moment_is_kept() {
    // case: is kept with the membership
    let mut p = Profile::new();
    let made = p.create("Trading").unwrap();
    p.assign("m_1", Some(&made)).unwrap();
    let held = p.reread().members.get("m_1").cloned().unwrap();
    assert_eq!(held.group_id, made);
    let at = held.at.expect("a joining moment");
    assert!(chrono::DateTime::parse_from_rfc3339(&at).is_ok(), "{at}");
}

#[test]
fn groups_hold_the_join_moment_moves_with_the_conversation() {
    // case: is updated when a conversation moves to another group
    let mut p = Profile::new();
    let one = p.create("One").unwrap();
    let two = p.create("Two").unwrap();
    p.assign("m_1", Some(&one)).unwrap();
    let first = p.reread().members.get("m_1").and_then(|m| m.at.clone());
    std::thread::sleep(std::time::Duration::from_millis(5));
    p.assign("m_1", Some(&two)).unwrap();
    let second = p.reread().members.get("m_1").and_then(|m| m.at.clone());
    assert!(first.is_some() && second.is_some());
    assert_ne!(second, first);
}

#[test]
fn groups_hold_an_older_membership_has_no_moment() {
    // case: reads an older file that recorded no moment
    let p = Profile::with_file(Some(
        r#"{"schemaVersion":1,"groups":[{"groupId":"grp_a","name":"Trading","createdAt":"2026-09-15T00:00:00.000Z"}],"members":{"m_1":"grp_a"}}"#,
    ));
    let held = p.org.members.get("m_1").cloned().unwrap();
    assert_eq!(held.group_id, "grp_a");
    assert_eq!(held.at, None);
}
