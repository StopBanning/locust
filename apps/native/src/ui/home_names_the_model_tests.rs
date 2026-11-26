//! home-names-the-model-it-runs.test.tsx (9 cases): a teammate's route on Home names the model it runs, by the name the
//! chip and Team give it, with the level only in the whole route (the card's hover and accessible name). Pure cases go
//! through `home_route_of` / `route_effort_of`; the card's through Home as the window draws it (`crate::view`).
//! Expected words are Electron's (routeName.ts, HomeTeam.tsx and the test).
use super::*;
use rune::{Env, Runtime};

fn model(id: &str, name: &str, efforts: &[&str]) -> engine::Model {
    engine::Model { id: id.into(), name: name.into(), efforts: efforts.iter().map(|e| (*e).to_owned()).collect() }
}

/// Electron's OPUS: Claude's alias with five levels, medium its default.
fn opus() -> Vec<engine::Model> {
    vec![model("opus", "Opus", &["low", "medium", "high", "xhigh", "max"])]
}

/// Electron's GROK: a Cursor family, one entry for its three level ids (the catalogue folds variants into a family).
fn grok() -> Vec<engine::Model> {
    vec![model("grok-4.7", "Grok 4.7", &["low", "medium", "high"])]
}

/// Electron's FLASH: an Antigravity tier with no levels.
fn flash() -> Vec<engine::Model> {
    vec![model("flash", "Gemini 3.8 Flash", &[])]
}

// a teammate's route on Home

#[test]
fn names_the_version_an_alias_runs_as_the_team_screen_does() {
    let line = home_route_of("claude", "opus", None, &[]);
    assert_eq!(line.model, "Opus 5.5");
    assert_eq!(line.route, "Claude · Opus 5.5");
    assert_eq!(line.runtime.as_deref(), Some("claude"));
    assert_eq!(line.model, locust_core::teammate_tooltip::route_model_name("claude", "opus"));
}

#[test]
fn names_antigravitys_flash_tier_by_the_model_it_runs_and_guesses_no_version_for_the_others() {
    assert_eq!(home_route_of("antigravity", "flash", None, &[]).route, "Antigravity · Gemini 3.8 Flash");
    assert_eq!(locust_core::teammate_tooltip::route_model_name("antigravity", "pro"), "Gemini Pro");
    assert_eq!(locust_core::teammate_tooltip::route_model_name("antigravity", "flash_lite"), "Gemini Flash Lite");
    assert_eq!(locust_core::teammate_tooltip::route_model_name("antigravity", "account-default"), "Account Default");
}

#[test]
fn names_the_model_alone_on_the_line_every_card_alike_and_the_level_in_the_whole_route_the_one_chosen_else_the_models_default() {
    let line = home_route_of("claude", "opus", None, &opus());
    assert_eq!(line.model, "Opus 5.5");
    assert_eq!(line.route, "Claude · Opus 5.5 · Medium");
    assert_eq!(home_route_of("claude", "opus", Some("xhigh"), &opus()).route, "Claude · Opus 5.5 · Extra high");
}

#[test]
fn names_a_cursor_family_by_the_family_so_its_line_carries_no_level_where_the_others_carry_none() {
    let robin = home_route_of("cursor", "grok-4.7-medium", None, &grok());
    assert_eq!(robin.model, "Grok 4.7");
    assert_eq!(robin.route, "Cursor · Grok 4.7 · Medium");
    assert_eq!(home_route_of("cursor", "grok-4.7-high", None, &grok()).route, "Cursor · Grok 4.7 · High");
}

#[test]
fn says_no_level_for_a_model_that_has_none_and_guesses_none_before_the_catalog_has_answered() {
    assert_eq!(home_route_of("antigravity", "flash", None, &flash()).route, "Antigravity · Gemini 3.8 Flash");
    assert_eq!(home_route_of("claude", "opus", None, &[]).route, "Claude · Opus 5.5");
    assert_eq!(route_effort_of("claude", "opus", None, &[]), None);
}

#[test]
fn shows_no_makers_mark_for_a_model_of_the_persons_own() {
    assert_eq!(home_route_of("opencode", "own-1a2b3c4d/acme-70b", None, &[]).runtime, None);
}

// Home's teammate card

fn mate(id: &str, name: &str, role: &str, runtime: Option<&str>, model: Option<&str>) -> Teammate {
    Teammate { id: id.into(), name: name.into(), role: role.into(), runtime: runtime.map(Into::into), model: model.map(Into::into), account: None, busy: false, runs_in: "ask".into(), folder: None }
}

fn home(team: Vec<Teammate>) -> Runtime<State> {
    let mut s = State::default();
    home_scene(&mut s);
    s.home.splash_open = false;
    s.home.splash_entries.clear();
    s.teammates = team;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 2000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.0);
    rt
}

fn keyed(rt: &Runtime<State>, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key))
}

#[test]
fn is_what_homes_cards_are_given_not_the_alias_spelled() {
    // App.tsx hands each card homeRouteOf(mate.route, ...): the card's line and hover are home_route_of's.
    let rt = home(vec![mate("tm_wren", "Wren", "Code", Some("claude"), Some("opus"))]);
    let want = home_route_of("claude", "opus", None, &[]);
    assert_eq!(keyed(&rt, "home-card-model-tm_wren").and_then(|e| e.text), Some(want.model.clone()));
    assert_eq!(keyed(&rt, "home-card-route-tm_wren").and_then(|e| e.help), Some(want.route));
    assert_ne!(want.model, "Opus", "not the alias, spelled");
}

#[test]
fn shows_the_runtime_as_its_mark_and_the_model_in_words_the_whole_route_on_hover_and_for_a_screen_reader() {
    let rt = home(vec![mate("tm_robin", "Robin", "Finance Bro", Some("cursor"), Some("grok-4.7-medium"))]);
    assert!(keyed(&rt, "home-card-mark-tm_robin").is_some(), "Cursor's mark");
    // The words are the model alone: the mark already said Cursor.
    assert_eq!(keyed(&rt, "home-card-model-tm_robin").and_then(|e| e.text).as_deref(), Some("Grok 4.7 Medium"));
    assert_eq!(keyed(&rt, "home-card-route-tm_robin").and_then(|e| e.help).as_deref(), Some("Cursor · Grok 4.7 Medium"));
    assert_eq!(keyed(&rt, "home-card-tm_robin").and_then(|e| e.label).as_deref(), Some("Message Robin, Finance Bro, on Cursor · Grok 4.7 Medium"));
}

#[test]
fn with_no_model_of_its_own_draws_no_route_line_and_no_mark() {
    let rt = home(vec![mate("tm_new", "Newt", "Docs & QA", None, None)]);
    assert!(keyed(&rt, "home-card-tm_new").is_some(), "the card is drawn");
    for key in ["home-card-route-tm_new", "home-card-model-tm_new", "home-card-mark-tm_new"] {
        assert!(keyed(&rt, key).is_none(), "{key}");
    }
    let said: String = rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
    assert!(!said.contains("runs on the model you pick"), "{said}");
}

#[test]
fn says_a_fast_level_in_the_whole_route_and_a_chosen_level_beats_the_one_a_cursor_id_carries() {
    // routeName.ts homeRouteOf: `${name} · ${effortName(base)}${fast ? ' · Fast' : ''}`; routeEffortOf: route.effort ?? ofId ?? default.
    assert_eq!(home_route_of("claude", "opus", Some("high-fast"), &opus()).route, "Claude · Opus 5.5 · High · Fast");
    assert_eq!(home_route_of("claude", "opus", Some("high"), &opus()).route, "Claude · Opus 5.5 · High");
    assert_eq!(home_route_of("cursor", "grok-4.7-medium", Some("high"), &grok()).route, "Cursor · Grok 4.7 · High");
    assert_eq!(route_effort_of("cursor", "grok-4.7-medium", Some("high"), &grok()).as_deref(), Some("high"));
    assert_eq!(route_effort_of("cursor", "grok-4.7-medium", None, &grok()).as_deref(), Some("medium"));
}

#[test]
fn a_card_for_a_teammate_working_now_says_so_in_its_accessible_name() {
    // HomeTeam.tsx: `, ${mate.state.spoken}` where there is a state, else ', working now' for a teammate at work.
    assert_eq!(home_card_label("Wren", "Code", None, true, Some("Claude · Opus 5.5")), "Message Wren, Code, working now, on Claude · Opus 5.5");
    assert_eq!(home_card_label("Wren", "Code", Some("waiting for you"), true, None), "Message Wren, Code, waiting for you");
    assert_eq!(home_card_label("Wren", "Code", None, false, None), "Message Wren, Code");
    let mut busy = mate("tm_wren", "Wren", "Code", Some("claude"), Some("opus"));
    busy.busy = true;
    let rt = home(vec![busy]);
    assert_eq!(keyed(&rt, "home-card-tm_wren").and_then(|e| e.label).as_deref(), Some("Message Wren, Code, working now, on Claude · Opus 5.5"));
}
