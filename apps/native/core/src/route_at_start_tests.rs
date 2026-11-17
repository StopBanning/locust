// Carries apps/desktop/src/shared/a-run-started-for-a-teammate-is-theirs.test.ts (Electron 4ce2f37a): who, on what, a
// run started for a teammate goes out as.
use crate::route_at_start::{PickerRoute, StartAs, TeammateRoute, start_as};
use std::collections::HashMap;

fn route(runtime: &str, model: &str) -> PickerRoute {
    PickerRoute { runtime: runtime.into(), model: model.into(), account: None }
}
fn composer() -> PickerRoute {
    route("claude", "sonnet")
}
fn rue() -> TeammateRoute {
    TeammateRoute { runtime: "cursor".into(), model: "composer-2.5".into(), mode: "ask".into(), account: None, effort: Some("medium".into()) }
}
fn start(id: &str, saved: Option<&TeammateRoute>, picked: &HashMap<String, PickerRoute>) -> StartAs {
    start_as(id, saved, &composer(), "accept-edits", Some("high"), picked)
}

#[test]
fn a_run_started_for_a_teammate_goes_out_on_their_route_mode_and_effort() {
    assert_eq!(start("tm_rue", Some(&rue()), &HashMap::new()), StartAs {
        teammate_id: "tm_rue".into(),
        route: route("cursor", "composer-2.5"),
        mode: "ask".into(),
        effort: Some("medium".into()),
        one_off: false,
    });
}

#[test]
fn a_route_picked_for_their_new_chat_wins() {
    let picked = HashMap::from([("new:tm_rue".to_owned(), route("opencode", "opencode/muse-spark-1.3-contributor-free"))]);
    assert_eq!(start("tm_rue", Some(&rue()), &picked).route, route("opencode", "opencode/muse-spark-1.3-contributor-free"));
}

#[test]
fn a_pick_made_in_another_conversation_of_theirs_does_not_win() {
    let picked = HashMap::from([("mission_other".to_owned(), route("opencode", "opencode/muse-spark-1.3-contributor-free"))]);
    assert_eq!(start("tm_rue", Some(&rue()), &picked).route, route("cursor", "composer-2.5"));
}

#[test]
fn a_teammate_who_has_never_run_takes_the_composers_route_mode_and_effort_under_their_own_id() {
    assert_eq!(start("tm_new", None, &HashMap::new()), StartAs {
        teammate_id: "tm_new".into(),
        route: route("claude", "sonnet"),
        mode: "accept-edits".into(),
        effort: Some("high".into()),
        one_off: false,
    });
}

#[test]
fn a_saved_route_with_no_effort_sends_none_rather_than_the_composers() {
    let saved = TeammateRoute { effort: None, ..rue() };
    assert_eq!(start("tm_rue", Some(&saved), &HashMap::new()).effort, None);
}
