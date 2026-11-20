//! routines-shelf.test.tsx, "the routines shelf", drawn headless (routines round 6, 19 Nov). The second describe,
//! CliArtifacts, has no screen here yet: the window does not read a runtime's own agents and commands.
use super::*;
use locust_core::engine::ConversationRow;
use locust_core::routine_draft::{RoutineDraft, Saved, NOTHING_TO_SAVE_YET};
use rune::{Env, Runtime};

fn draw(s: State) -> Vec<rune::ElementInfo> {
    let mut rt = Runtime::new(s, crate::view, (1200.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}
fn shown(all: &[rune::ElementInfo], key: &str) -> Option<String> {
    all.iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone())
}
fn any_text(all: &[rune::ElementInfo], words: &str) -> bool {
    all.iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains(words)) || e.label.as_deref().is_some_and(|t| t.contains(words)))
}
/// What is drawn inside the element `key`: the shelf alone, as Electron's test renders it, without the sidebar's list.
fn inside(all: &[rune::ElementInfo], key: &str) -> Vec<rune::ElementInfo> {
    let Some(outer) = all.iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect) else { return Vec::new() };
    all.iter()
        .filter(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            cx >= outer[0] && cx <= outer[0] + outer[2] && cy >= outer[1] && cy <= outer[1] + outer[3]
        })
        .cloned()
        .collect()
}
fn row(id: &str, title: &str, state: &str, teammate: &str, updated: &str) -> ConversationRow {
    ConversationRow { id: id.into(), teammate: teammate.into(), title: title.into(), started_by: "ledger".into(), state: state.into(), updated: updated.into() }
}
fn shelf() -> State {
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.routines.open = true;
    s.routines.file = json!({"schemaVersion": 1, "routines": []});
    s
}

/// The ledger as these tests have it: m1 is four turns, typed by the person, its owner recorded on its first turn.
fn read(id: &str) -> Option<Saved> {
    let draft = |steps: Vec<&str>, truncated| RoutineDraft {
        name: "Friday release checks".into(),
        steps: steps.into_iter().map(str::to_owned).collect(),
        learned_from: vec!["m1a".into(), "m1".into()],
        truncated,
    };
    match id {
        "m1" => Some(Saved { turns: 4, draft: Some(draft(vec!["Check the release.", "Tag it."], false)), chain: vec!["m1".into(), "m1b".into(), "m1a".into()] }),
        "long" => Some(Saved { turns: 15, draft: Some(draft(vec!["One."; 12], true)), chain: vec!["long".into()] }),
        "relayed" => Some(Saved { turns: 1, draft: None, chain: vec!["relayed".into()] }),
        _ => None,
    }
}
fn offered(rows: Vec<ConversationRow>) -> State {
    let mut s = shelf();
    s.recent = rows;
    s.routines.read_saved = read;
    offer_savable(&mut s);
    s
}
fn press(s: State, key: &str) -> State {
    let mut rt = Runtime::new(s, crate::view, (1200.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.016);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.032);
    rt.state
}

#[test]
fn is_called_routines_in_its_heading_and_to_a_screen_reader() {
    let all = draw(shelf());
    assert_eq!(shown(&all, "routines-heading").as_deref(), Some("Routines"));
    let named = |key: &str| all.iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label.clone());
    assert_eq!(named("routines-shelf").as_deref(), Some("Routines"), "the shelf is named Routines to a screen reader");
    // Nowhere in the window: the heading, the sidebar's door and the narrow rail's.
    assert!(!all.iter().any(|e| e.text.as_deref() == Some("Automations") || e.label.as_deref() == Some("Automations")));
    // A narrow window draws the rail instead of the sidebar, and its door says the same.
    let mut rt = Runtime::new(shelf(), crate::view, (900.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let narrow: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).collect();
    assert!(narrow.iter().any(|e| e.role == "button" && e.label.as_deref() == Some("Routines")), "the rail's door says Routines");
    assert!(!narrow.iter().any(|e| e.label.as_deref() == Some("Automations")));
}

#[test]
fn no_longer_holds_the_things_a_person_cannot_run() {
    let all = draw(offered(vec![]));
    assert!(!any_text(&all, "Set up in your CLIs"));
    assert!(!any_text(&all, "does not run them"));
}

#[test]
fn offers_finished_conversations_instead_of_describing_a_right_click() {
    let s = offered(vec![
        row("m1", "Friday release checks", "completed", "Codec", "2026-09-10T04:00:00.000Z"),
        row("m2", "Still going", "running", "Codec", "2026-09-10T05:00:00.000Z"),
    ]);
    let all = draw(s);
    assert_eq!(shown(&all, "routine-savable-m1").as_deref(), Some("Friday release checks"));
    assert_eq!(shown(&all, "routine-savable-turns-m1").as_deref(), Some("4 turns"));
    // Says what it makes: a bare "Save" did not.
    assert_eq!(shown(&all, "routine-save-from-m1").as_deref(), Some("Save as routine"));
    // The running one is not on offer (the sidebar's own list still shows it), and the gesture is not narrated.
    let on_shelf = inside(&all, "routines-empty");
    assert!(any_text(&on_shelf, "Friday release checks"));
    assert!(!any_text(&on_shelf, "Still going"));
    assert!(!any_text(&all, "right-click"));
    assert!(!any_text(&all, NOTHING_TO_SAVE_YET));
    assert!(any_text(&on_shelf, "Save one of your finished conversations to start."));
    // With templates offered as well, they are offered as the other way, not the first.
    if !locust_core::routine_templates::list_shipped_templates().is_empty() {
        assert!(any_text(&on_shelf, "Or start from a template."));
        assert!(!any_text(&on_shelf, "Start from a template, or press New routine"));
    }
}

#[test]
fn says_the_honest_sentence_when_a_workspace_has_finished_nothing_at_all() {
    let all = draw(offered(vec![row("m2", "Still going", "running", "Codec", "2026-09-10T05:00:00.000Z")]));
    assert!(!any_text(&all, "right-click"));
    assert!(any_text(&all, "A routine is a job a teammate does again"));
    if locust_core::routine_templates::list_shipped_templates().is_empty() {
        assert_eq!(shown(&all, "routines-nothing-yet").as_deref(), Some(NOTHING_TO_SAVE_YET));
    } else {
        // With starter templates to offer, they are the material instead (0.615).
        assert!(shown(&all, "routines-nothing-yet").is_none());
        assert!(any_text(&all, "Start from a template, or press New routine to write your own."));
    }
}

#[test]
fn offers_new_routine_written_from_nothing_where_the_screen_is_given_one() {
    assert_eq!(shown(&draw(shelf()), "routine-new").as_deref(), Some("New routine"));
    let mut nobody = shelf();
    nobody.teammates.clear();
    assert!(shown(&draw(nobody), "routine-new").is_none());
}

// ---- Save as routine, from the shelf (App.tsx openSaveRoutine) ----------------------------------------------------

#[test]
fn save_as_routine_opens_the_editor_on_the_draft_for_the_owner_walked_back() {
    let mut s = offered(vec![row("m1", "Friday release checks", "completed", "Someone gone", "2026-09-10T04:00:00.000Z")]);
    // The owner is recorded on the conversation's first turn, not on the follow-up its row shows.
    s.mission_owners.insert("m1a".into(), "argon".into());
    let s = press(s, "routine-save-from-m1");
    assert_eq!(s.routines.fields[0].text, "Friday release checks");
    assert_eq!(s.routines.steps.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), ["Check the release.", "Tag it."]);
    assert_eq!(s.routines.entry["teammateId"], "argon");
    assert!(!s.routines.choose_runner);
    // Where it came from goes with it to the store.
    assert_eq!(s.routines.entry["learnedFrom"], json!(["m1a", "m1"]));
    assert!(!s.routines.truncated);
}

#[test]
fn a_conversation_nobody_owns_can_still_be_saved_and_the_editor_asks_who_runs_it() {
    let s = press(offered(vec![row("m1", "Friday release checks", "completed", "Codex CLI", "2026-09-10T04:00:00.000Z")]), "routine-save-from-m1");
    assert_eq!(s.routines.entry["teammateId"], "");
    assert!(s.routines.choose_runner);
    // Its row's name is the owner when no turn records one.
    let s = press(offered(vec![row("m1", "Friday release checks", "completed", "Scout", "2026-09-10T04:00:00.000Z")]), "routine-save-from-m1");
    assert_eq!(s.routines.entry["teammateId"], "scout");
}

#[test]
fn a_draft_cut_at_twelve_steps_says_so_in_the_editor() {
    let s = press(offered(vec![row("long", "A long one", "completed", "Codec", "2026-09-10T04:00:00.000Z")]), "routine-save-from-long");
    assert!(s.routines.truncated);
    assert_eq!(s.routines.steps.len(), 12);
    let all = draw(s);
    assert_eq!(shown(&all, "routine-truncated").as_deref(), Some("This conversation had more turns than a routine can hold, so the first 12 are here."));
    // A fresh editor forgets it.
    let mut s = offered(vec![]);
    s.routines.truncated = true;
    edit(&mut s, json!({"name": "", "steps": [""], "teammateId": "codec"}));
    assert!(!s.routines.truncated);
}

#[test]
fn a_conversation_with_nothing_typed_in_it_has_nothing_to_save() {
    let s = press(offered(vec![row("relayed", "Relayed", "completed", "Codec", "2026-09-10T04:00:00.000Z")]), "routine-save-from-relayed");
    assert!(s.routines.fields.is_empty(), "the editor did not open");
}

#[test]
fn the_shelf_reads_conversations_back_only_while_it_is_empty() {
    let mut s = shelf();
    s.recent = vec![row("m1", "Friday release checks", "completed", "Codec", "2026-09-10T04:00:00.000Z")];
    s.routines.read_saved = read;
    s.routines.file = json!({"schemaVersion": 1, "routines": [{"routineId": "rt_1", "name": "Kept", "teammateId": "codec", "steps": ["a"], "learnedFrom": [], "createdAt": "2026-09-01T00:00:00.000Z", "runs": 0}]});
    offer_savable(&mut s);
    assert!(s.routines.savable.is_empty());
    s.routines.file = json!({"schemaVersion": 1, "routines": []});
    offer_savable(&mut s);
    assert_eq!(s.routines.savable.len(), 1);
}

#[test]
fn the_header_counts_what_is_saved_and_what_goes_on_its_own() {
    assert_eq!(shelf_meta(&json!({"routines": []})), "none saved");
    let r = |schedule: Value, paused: bool| json!({"schedule": schedule, "paused": paused});
    assert_eq!(shelf_meta(&json!({"routines": [r(Value::Null, false)]})), "1 saved · none on a schedule");
    let daily = json!({"kind": "daily", "at": "09:00"});
    assert_eq!(
        shelf_meta(&json!({"routines": [r(daily.clone(), false), r(daily.clone(), true), r(Value::Null, false)]})),
        "3 saved · 1 on a schedule · 1 paused"
    );
    assert_eq!(shown(&draw(shelf()), "routines-meta").as_deref(), Some("none saved"));
}
