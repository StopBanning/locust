//! Electron `main/route-at-start.test.ts`: which route a turn starts on, and which route the composer shows. One test
//! per Electron case, Electron's values.

use crate::route_at_start::{composer_route_for, pick_key_for, route_at_start, PickerRoute, StartRoute, StartRouteInput, TeammateRoute};
use std::collections::HashMap;

fn saved() -> TeammateRoute {
    TeammateRoute { runtime: "codex".into(), model: "gpt-5.6-luna".into(), mode: "accept-edits".into(), account: None, effort: Some("low".into()) }
}

fn input(runtime: Option<&str>, model: Option<&str>) -> StartRouteInput {
    StartRouteInput { runtime: runtime.map(Into::into), model: model.map(Into::into), ..Default::default() }
}

/// `fallback`: `{ runtime: 'opencode', model: 'account-default' }`.
fn fallback() -> StartRouteInput {
    input(Some("opencode"), Some("account-default"))
}

fn start(runtime: &str, model: Option<&str>, effort: Option<&str>) -> StartRoute {
    StartRoute { runtime: runtime.into(), model: model.map(Into::into), effort: effort.map(Into::into), account: None }
}

fn picker(runtime: &str, model: &str) -> PickerRoute {
    PickerRoute { runtime: runtime.into(), model: model.into(), account: None }
}

fn as_picker(route: &TeammateRoute) -> PickerRoute {
    PickerRoute { runtime: route.runtime.clone(), model: route.model.clone(), account: route.account.clone() }
}

#[test]
fn a_start_honours_an_explicit_picker_override_for_this_teammate() {
    let saved = saved();
    let asked = StartRouteInput { route_override_for: Some("tm_wren".into()), ..fallback() };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved)))), start("opencode", Some("account-default"), None));
}

#[test]
fn a_start_uses_the_saved_route_over_discovery_and_default_selections_including_saved_effort() {
    let saved = saved();
    assert_eq!(route_at_start(&fallback(), Some(("tm_wren", Some(&saved)))), start("codex", Some("gpt-5.6-luna"), Some("low")));
}

#[test]
fn a_start_for_nobody_stays_on_the_composer_route_whatever_the_old_picker_intent() {
    let asked = StartRouteInput { effort: Some("high".into()), route_override_for: Some("tm_wren".into()), ..fallback() };
    assert_eq!(route_at_start(&asked, None), start("opencode", Some("account-default"), Some("high")));
}

#[test]
fn a_start_does_not_mistake_another_teammates_picker_change_for_this_teammates_choice() {
    let saved = saved();
    let asked = StartRouteInput { route_override_for: Some("tm_gem".into()), ..fallback() };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved)))).runtime, "codex");
}

#[test]
fn a_start_keeps_account_default_when_the_picker_chose_it() {
    let saved = saved();
    let asked = StartRouteInput { route_override_for: Some("tm_wren".into()), ..input(Some("codex"), Some("account-default")) };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved)))), start("codex", Some("account-default"), None));
}

#[test]
fn a_start_does_not_promote_a_seeded_account_default_to_picker_intent() {
    let saved = saved();
    let route = route_at_start(&input(Some("codex"), Some("account-default")), Some(("tm_wren", Some(&saved))));
    assert_eq!(route.model.as_deref(), Some("gpt-5.6-luna"));
}

#[test]
fn a_start_for_a_new_teammate_without_a_saved_route_keeps_the_existing_defaults() {
    assert_eq!(route_at_start(&fallback(), Some(("tm_new", None))), start("opencode", Some("account-default"), None));
    assert_eq!(route_at_start(&StartRouteInput::default(), None), start("codex", None, None));
}

#[test]
fn a_start_on_the_saved_picker_identity_keeps_its_effort_change() {
    let saved = saved();
    let asked = StartRouteInput { effort: Some("high".into()), ..input(Some("codex"), Some("gpt-5.6-luna")) };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved)))), start("codex", Some("gpt-5.6-luna"), Some("high")));
}

#[test]
fn a_start_keeps_a_concrete_effort_variant_without_calling_it_a_new_picker_choice() {
    let cursor = TeammateRoute { runtime: "cursor".into(), model: "grok-low".into(), ..saved() };
    let asked = StartRouteInput { model_choice: Some("grok-low".into()), ..input(Some("cursor"), Some("grok-high")) };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&cursor)))), start("cursor", Some("grok-high"), None));
}

#[test]
fn a_start_cannot_carry_a_stale_effort_or_model_transform_across_a_different_saved_route() {
    let saved = saved();
    let asked = StartRouteInput {
        model_choice: Some("grok-low".into()),
        effort: Some("high".into()),
        ..input(Some("cursor"), Some("grok-high"))
    };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved)))), start("codex", Some("gpt-5.6-luna"), Some("low")));
}

#[test]
fn a_start_on_the_saved_identity_keeps_the_requests_own_account_when_the_saved_route_has_none() {
    // `{ ...requested, ...(saved.account === undefined ? {} : { account: saved.account }) }`.
    let saved = saved();
    let acct = "account_0123456789abcdef0123456789abcdef";
    let asked = StartRouteInput { account: Some(acct.into()), ..input(Some("codex"), Some("gpt-5.6-luna")) };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved)))).account.as_deref(), Some(acct));
}

// ---- a pick belongs to the conversation it was made in (0.552) ----

fn choices() -> HashMap<String, PickerRoute> {
    HashMap::from([("mission_a".into(), picker("opencode", "account-default")), ("new:tm_gem".into(), picker("opencode", "account-default"))])
}

#[test]
fn a_new_chat_with_a_teammate_starts_on_their_saved_model() {
    let saved = saved();
    let key = pick_key_for(None, Some("tm_wren"));
    let shown = composer_route_for(&picker("opencode", "account-default"), Some(("tm_wren", Some(&saved))), &choices(), Some(&key), None);
    assert_eq!(shown, as_picker(&saved));
}

#[test]
fn a_pick_made_in_a_conversation_is_shown_there_again() {
    let saved = saved();
    let shown = composer_route_for(&picker("codex", "gpt-5.6-luna"), Some(("tm_wren", Some(&saved))), &choices(), Some("mission_a"), None);
    assert_eq!(shown, picker("opencode", "account-default"));
}

#[test]
fn a_conversation_shows_what_it_ran_on_not_the_saved_model_of_its_teammate() {
    let saved = saved();
    let ran = picker("opencode", "opencode/free");
    let shown = composer_route_for(&picker("opencode", "account-default"), Some(("tm_wren", Some(&saved))), &choices(), Some("mission_b"), Some(&ran));
    assert_eq!(shown, ran);
}

#[test]
fn a_pick_for_a_new_chat_with_one_teammate_is_not_for_another() {
    let saved = saved();
    let fallback = picker("opencode", "account-default");
    let gem = pick_key_for(None, Some("tm_gem"));
    assert_eq!(composer_route_for(&fallback, Some(("tm_gem", Some(&saved))), &choices(), Some(&gem), None), fallback);
    let wren = pick_key_for(None, Some("tm_wren"));
    assert_eq!(composer_route_for(&fallback, Some(("tm_wren", Some(&saved))), &choices(), Some(&wren), None), as_picker(&saved));
}

#[test]
fn home_does_not_inherit_a_teammate_override() {
    let home = picker("claude", "account-default");
    let key = pick_key_for(None, None);
    assert_eq!(key, "new");
    assert_eq!(composer_route_for(&home, None, &choices(), Some(&key), None), home);
}

#[test]
fn a_kilo_teammate_saved_on_the_account_default_shows_as_auto_free() {
    let kilo = TeammateRoute { runtime: "kilo".into(), model: "account-default".into(), mode: "ask".into(), account: None, effort: None };
    let key = pick_key_for(None, Some("tm_wren"));
    let shown = composer_route_for(&picker("opencode", "account-default"), Some(("tm_wren", Some(&kilo))), &HashMap::new(), Some(&key), None);
    assert_eq!((shown.runtime.as_str(), shown.model.as_str()), ("kilo", "kilo/kilo-auto/free"));
    let home = composer_route_for(&picker("kilo", "account-default"), None, &HashMap::new(), Some("new"), None);
    assert_eq!((home.runtime.as_str(), home.model.as_str()), ("kilo", "kilo/kilo-auto/free"));
    let started = route_at_start(&input(Some("kilo"), Some("account-default")), Some(("tm_wren", Some(&kilo))));
    assert_eq!((started.runtime.as_str(), started.model.as_deref()), ("kilo", Some("kilo/kilo-auto/free")));
}
