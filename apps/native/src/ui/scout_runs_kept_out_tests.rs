//! Electron `App.tsx` `isHiddenRun`, seen in live hour 1: Scout's own background looks ("You are Scout, a teammate who
//! keeps watch...") were listed among the conversations. Scout's looks, a comparison's judge and the runs a turn says
//! Scout or a judge started are not conversations; everything else is listed as before.
use super::*;
use locust_core::scout::types::{ScoutSettings, ScoutState};
use rune::{Env, Runtime};

const SCOUT_WORDS: &str = "You are Scout, a teammate who keeps watch for the person you work with. Locust found what is below.";

fn row(id: &str, title: &str, at: &str) -> ConversationRow {
    ConversationRow { id: id.into(), title: title.into(), teammate: String::new(), started_by: "native".into(), state: "completed".into(), updated: at.into() }
}

fn summary(id: &str, kind: Option<&str>) -> ledger::Summary {
    ledger::Summary { mission_id: id.into(), created_at: "2026-11-26T09:00:00.000Z".into(), started_by_kind: kind.map(Into::into), ..Default::default() }
}

fn scout_state(look_ids: &[&str]) -> ScoutState {
    ScoutState {
        settings: ScoutSettings::default(),
        cards: Vec::new(),
        look_ids: look_ids.iter().map(|id| (*id).to_owned()).collect(),
        last_look_at: None,
        model_said: None,
        today: None,
        notes: Vec::new(),
    }
}

/// The person's one conversation and three runs that are not (newest first: look, judged, started by Scout, mine).
fn state(tweak: impl FnOnce(&mut State)) -> State {
    let mut s = State::default();
    s.demo = true;
    s.recent = vec![
        row("mission_mine", "Fix the login", "2026-11-26T09:00:00.000Z"),
        row("mission_look", SCOUT_WORDS, "2026-11-26T09:30:00.000Z"),
        row("mission_judged", "Judge these answers", "2026-11-26T09:20:00.000Z"),
        row("mission_started_by_scout", SCOUT_WORDS, "2026-11-26T09:10:00.000Z"),
    ];
    for id in ["mission_mine", "mission_look", "mission_judged", "mission_started_by_scout"] {
        s.ledger_summaries.insert(id.into(), summary(id, None));
    }
    tweak(&mut s);
    s
}

fn listed(s: State) -> Vec<String> {
    let mut rt = Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 8000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let mut found: Vec<(f32, String)> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| e.key.and_then(|k| k.strip_prefix("recent-").map(str::to_owned)).map(|id| (e.rect[1], id)))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.dedup_by(|a, b| a.1 == b.1);
    found.into_iter().map(|(_, id)| id).collect()
}

#[test]
fn every_run_is_listed_when_none_is_a_background_one() {
    assert_eq!(listed(state(|_| {})), ["mission_look", "mission_judged", "mission_started_by_scout", "mission_mine"]);
}

#[test]
fn a_look_scout_names_in_its_state_is_not_listed() {
    let shown = listed(state(|s| s.scout.state = Some(scout_state(&["mission_look"]))));
    assert_eq!(shown, ["mission_judged", "mission_started_by_scout", "mission_mine"]);
}

#[test]
fn a_turn_started_by_scout_is_not_listed_even_when_its_state_does_not_name_it() {
    let shown = listed(state(|s| {
        s.ledger_summaries.insert("mission_started_by_scout".into(), summary("mission_started_by_scout", Some("scout")));
    }));
    assert_eq!(shown, ["mission_look", "mission_judged", "mission_mine"]);
}

#[test]
fn a_turn_a_judge_started_is_not_listed() {
    let shown = listed(state(|s| {
        s.ledger_summaries.insert("mission_judged".into(), summary("mission_judged", Some("judge")));
    }));
    assert_eq!(shown, ["mission_look", "mission_started_by_scout", "mission_mine"]);
}

#[test]
fn what_a_person_started_stays_listed_beside_scouts_looks() {
    let shown = listed(state(|s| {
        s.scout.state = Some(scout_state(&["mission_look", "mission_started_by_scout", "mission_judged"]));
        s.ledger_summaries.insert("mission_mine".into(), summary("mission_mine", Some("person")));
    }));
    assert_eq!(shown, ["mission_mine"]);
}

#[test]
fn scouts_looks_are_no_conversations_on_home_either() {
    let s = state(|s| s.scout.state = Some(scout_state(&["mission_look", "mission_started_by_scout"])));
    let ids: Vec<String> = recent_as_missions(&s).into_iter().map(|m| m.mission_id).collect();
    assert_eq!(ids, ["mission_judged", "mission_mine"]);
}
