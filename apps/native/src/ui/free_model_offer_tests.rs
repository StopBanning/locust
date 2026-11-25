//! Electron's `a-retired-free-model-is-not-offered.test.ts`, `a-free-model-that-is-down-is-passed-over.test.ts` and
//! `a-new-person-starts-on-the-steadiest-free-model.test.ts`, through the window: the catalogue the window keeps
//! (`ModelCatalogues`, with the retired set main's history refresh gives it), the card a failed turn shows
//! (`limit_replacement`) and Compare's first two columns (`ensure_default_picks`), with Electron's values.
use super::*;
use crate::tests::TempDir;
use locust_core::connectors_off::{
    FREE_MODELS_BEST_FIRST, KNOWN_RETIRED_MODELS, down_free_models, free_models_best_first, free_start_model,
    model_retired, retired_free_models,
};
use locust_core::route_rows::next_free_model;
use serde_json::json;
use std::collections::HashSet;

fn catalogue(ids: &[&str]) -> Vec<engine::Models> {
    vec![engine::Models {
        teammate: None, runtime: "opencode".into(), model: None, effort: None,
        models: ids.iter().map(|id| engine::Model { id: (*id).into(), name: (*id).into(), efforts: vec![] }).collect(),
    }]
}
fn listed(ids: &[&str]) -> Vec<engine::Model> {
    catalogue(ids).remove(0).models
}
/// What the window shows of OpenCode's catalogue.
fn shown(s: &State) -> Vec<engine::Model> {
    s.models["tm_wren"][0].models.clone()
}
fn shown_ids(s: &State) -> Vec<String> {
    shown(s).into_iter().map(|m| m.id).collect()
}
/// A finished run as the window's history keeps it (`ledger::Summary`).
fn summary(model: &str, at: &str, failed: Option<&str>) -> ledger::Summary {
    ledger::Summary {
        runtime: "opencode".into(), model: model.into(), updated: at.into(), failed: failed.is_some(),
        run_end: Some(match failed {
            Some(message) => ledger::RunEnd::Failed { message: message.into(), at_limit: false },
            None => ledger::RunEnd::Completed,
        }),
        ..Default::default()
    }
}
/// The retired set main's history refresh hands the catalogue (`update_retired`).
fn refresh_retired(s: &mut State, runs: &[ledger::Summary]) {
    let missions: Vec<_> = runs.iter().filter_map(RetiredMission::from_summary).collect();
    s.models.update_retired(retired_free_models(&missions));
}
fn state(ids: &[&str]) -> State {
    let mut s = State::default();
    s.models.insert("tm_wren".into(), catalogue(ids));
    s
}
/// Compare opened over a box on `model`: its first two columns.
fn compare_picks(s: &mut State, model: &str) -> Vec<String> {
    let mate = Teammate {
        id: "tm_wren".into(), name: "Wren".into(), role: "Code".into(), runtime: Some("opencode".into()),
        model: Some(model.into()), account: None, busy: false, runs_in: "ask".into(), folder: None,
    };
    s.teammates = vec![mate];
    s.selected = 0;
    s.conversations.insert("tm_wren".into(), Conversation::default());
    s.picks.insert("tm_wren".into(), TurnPick { model: model.into(), runtime: Some("opencode".into()), effort: None, account: None });
    s.compare.picks.clear();
    crate::compare_view::ensure_default_picks(s);
    s.compare.picks.iter().map(|pick| pick.model.clone()).collect()
}
fn iso(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).expect("a time").to_rfc3339()
}

// a-retired-free-model-is-not-offered.test.ts
const EXO: &str = "opencode/exo-free";
const LING: &str = "opencode/ling-3.0-flash-fin-free";
const GLM: &str = "opencode/glm-5.3-flash-free";
const RETIRED_CATALOG: [&str; 4] = [EXO, LING, GLM, "opencode/zz-later-free"];

#[test]
fn reads_opencodes_own_words_and_nothing_else_as_retired() {
    assert!(model_retired("OpenCode stopped: Model exo-free has been deprecated."));
    assert!(!model_retired("OpenCode stopped: Endpoint is unavailable."));
    assert!(!model_retired("This API has been deprecated in favour of v2"));
    assert!(!model_retired(""));
    // A run with no failure message is never read as retired.
    let none = RetiredMission { runtime: "opencode", model: EXO, phase: "failed", last_updated_at: "2026-10-20T10:00:00Z", failed_message: None };
    assert_eq!(retired_free_models(&[none]), KNOWN_RETIRED_MODELS.iter().map(|m| m.to_string()).collect::<HashSet<_>>());
}

#[test]
fn exo_free_is_left_out_before_anyone_here_has_run_it_a_new_person_starts_on_a_free_model_that_can_answer() {
    let mut want: Vec<String> = retired_free_models(&[]).into_iter().collect();
    want.sort();
    assert_eq!(want, ["opencode/exo-free", "opencode/fledge-alpha-free"]);
    let s = state(&RETIRED_CATALOG);
    assert!(!shown_ids(&s).contains(&EXO.to_owned()));
    // Best first since 0.712: GLM 5.3 Flash is named; Ling 3.0 is not.
    assert_eq!(free_start_model("opencode", &shown(&s), &[], &HashSet::new()).as_deref(), Some(GLM));
}

#[test]
fn a_model_whose_run_ended_has_been_deprecated_is_left_out_from_then_on() {
    let mut s = state(&RETIRED_CATALOG);
    refresh_retired(&mut s, &[
        summary(LING, "2026-10-01T10:00:00Z", None),
        summary(LING, "2026-10-09T01:00:00Z", Some("OpenCode stopped: Model ling-3.0-flash-fin-free has been deprecated.")),
    ]);
    assert!(!shown_ids(&s).contains(&LING.to_owned()));
    assert_eq!(free_start_model("opencode", &shown(&s), &[], &HashSet::new()).as_deref(), Some(GLM));
}

#[test]
fn one_that_answered_again_afterwards_is_back_another_failure_is_not_a_retirement() {
    let mut s = state(&RETIRED_CATALOG);
    refresh_retired(&mut s, &[
        summary(EXO, "2026-10-20T10:00:00Z", None),
        summary(GLM, "2026-10-09T01:00:00Z", Some("OpenCode stopped: Endpoint is unavailable.")),
    ]);
    assert!(shown_ids(&s).contains(&EXO.to_owned()));
    assert!(shown_ids(&s).contains(&GLM.to_owned()));
}

#[test]
fn compare_on_a_free_model_puts_another_working_free_model_beside_it() {
    // What 0.709 did: Exo, the first other free model.
    let mut s = state(&RETIRED_CATALOG);
    s.models.update_retired(HashSet::new());
    assert_eq!(compare_picks(&mut s, GLM), [GLM, EXO]);
    let mut s = state(&RETIRED_CATALOG);
    assert_eq!(compare_picks(&mut s, GLM), [GLM, LING]);
}

// a-free-model-that-is-down-is-passed-over.test.ts
const BEAM: &str = "opencode/beam-free";
const MIMO: &str = "opencode/mimo-v2.6-flash-free";

#[test]
fn is_down_after_its_latest_run_said_so_for_six_hours_and_not_once_it_answers_again() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-09T17:00:00Z").unwrap().timestamp_millis();
    let at = |message: &'static str| RetiredMission { runtime: "opencode", model: LING, phase: "failed", last_updated_at: "2026-10-09T16:00:00Z", failed_message: Some(message) };
    let ling_down = at("OpenCode stopped: Upstream request failed: Model is unavailable.");
    assert_eq!(down_free_models(&[ling_down], now), HashSet::from([LING.to_owned()]));
    let six_hours = 6 * 60 * 60 * 1000;
    assert!(down_free_models(&[ling_down], now - 60 * 60 * 1000 + six_hours).is_empty());
    let answered = RetiredMission { runtime: "opencode", model: LING, phase: "completed", last_updated_at: "2026-10-09T16:30:00Z", failed_message: None };
    assert!(down_free_models(&[ling_down, answered], now).is_empty());
    // Any other failure is not down: a limit, a retirement, a plain error.
    assert!(down_free_models(&[at("OpenCode stopped: Model ling has been deprecated.")], now).is_empty());
    assert!(down_free_models(&[at("Rate limit exceeded")], now).is_empty());
}

fn record(dir: &std::path::Path, model: &str, message: &str) {
    let at = "2026-11-23T11:00:00Z";
    let created = json!({"schemaVersion": 24, "recordType": "mission.created", "occurredAt": at,
        "metadata": {"missionId": "m_down", "prompt": "Review the notes.", "runtime": "opencode", "model": model, "createdAt": at}});
    let end = json!({"schemaVersion": 24, "recordType": "mission.event", "occurredAt": at,
        "event": {"id": "end", "sequence": 1, "type": "run.failed", "sourceAdapter": "opencode", "occurredAt": at,
            "payload": {"message": message}}});
    std::fs::write(dir.join("m_down.jsonl"), format!("{created}\n{end}\n")).unwrap();
}

/// The model the card under a failed turn on Ling offers, with `down` failed an hour before `now`.
fn card_offers(down: &[&str], now: i64) -> Option<String> {
    let dir = TempDir::new();
    record(&dir.0, LING, "OpenCode stopped: Endpoint is unavailable.");
    let mut worker = ledger::Session::new(dir.0.clone(), "m_down".into());
    let snapshot = worker.poll().unwrap();
    let mut s = State::default();
    team_scene(&mut s);
    let mut header = s.teammates[0].clone();
    header.runtime = Some("opencode".into());
    header.model = Some(LING.into());
    s.models.insert(header.id.clone(), catalogue(&[LING, MIMO, BEAM]));
    for (i, model) in down.iter().enumerate() {
        s.ledger_summaries.insert(format!("m_{i}"), summary(model, &iso(now - 60 * 60 * 1000), Some("Endpoint is unavailable.")));
    }
    s.ledger = Some(LedgerPane {
        header, conversation: Conversation { conversation_id: Some("m_down".into()), turns: vec![] },
        items: snapshot.items.clone(), dir: dir.0.clone(), worker: Some(worker), snapshot, cold_tip: None,
        due: f64::INFINITY, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    limit_replacement(&s, now).map(|(_, next)| next)
}

#[test]
fn the_card_after_a_failure_offers_the_next_one_that_is_up() {
    let catalog = listed(&[LING, MIMO, BEAM]);
    assert_eq!(next_free_model("opencode", Some(LING), &catalog, &[]).map(|m| m.id.as_str()), Some(MIMO));
    assert_eq!(next_free_model("opencode", Some(LING), &catalog, &[MIMO.to_owned()]).map(|m| m.id.as_str()), Some(BEAM));
    let now = chrono::Utc::now().timestamp_millis();
    assert_eq!(card_offers(&[], now).as_deref(), Some(MIMO));
    assert_eq!(card_offers(&[MIMO], now).as_deref(), Some(BEAM));
}

#[test]
fn compares_second_column_is_not_one_that_is_down() {
    // Best first since 0.712: Beam beside Mimo, and Ling 3.0, which the order does not name, only when Mimo is down.
    let mut s = state(&[LING, MIMO, BEAM]);
    assert_eq!(compare_picks(&mut s, BEAM), [BEAM, MIMO]);
    let an_hour_ago = iso(chrono::Utc::now().timestamp_millis() - 60 * 60 * 1000);
    s.ledger_summaries.insert("m_mimo".into(), summary(MIMO, &an_hour_ago, Some("OpenCode stopped: Endpoint is unavailable.")));
    assert_eq!(compare_picks(&mut s, BEAM), [BEAM, LING]);
}

// a-new-person-starts-on-the-steadiest-free-model.test.ts: what OpenCode listed as free on 2026-10-22.
const FRESH: [&str; 9] = [
    "opencode/beam-free", "opencode/deepseek-v4.1-flash-free", "opencode/flint-40b-free",
    "opencode/glm-5.3-flash-free", "opencode/mimo-v2.6-flash-free", "opencode/minimax-m3.1-flash-free",
    "opencode/nemotron-3-ultra-free", "opencode/nemotron-3.5-lightning-free", "opencode/nemotron-3.5-super-vl-free",
];

#[test]
fn and_a_failure_there_offers_the_next_steadiest_with_the_unnamed_after_all_of_them() {
    let s = state(&FRESH);
    let shown = shown(&s);
    assert_eq!(
        next_free_model("opencode", Some("opencode/deepseek-v4.1-flash-free"), &shown, &[]).map(|m| m.id.as_str()),
        Some("opencode/nemotron-3.5-lightning-free")
    );
    let order: Vec<&str> = free_models_best_first(&shown).into_iter().map(|m| m.id.as_str()).collect();
    assert_eq!(order[..FREE_MODELS_BEST_FIRST.len()], FREE_MODELS_BEST_FIRST[..]);
    // Not yet asked, so not named: after the named ones, in the order OpenCode lists them.
    assert_eq!(order[FREE_MODELS_BEST_FIRST.len()..], ["opencode/flint-40b-free", "opencode/minimax-m3.1-flash-free", "opencode/nemotron-3-ultra-free"]);
}

#[test]
fn and_compare_puts_the_next_steadiest_beside_it() {
    let mut s = state(&FRESH);
    assert_eq!(compare_picks(&mut s, "opencode/nemotron-3.5-lightning-free"), ["opencode/nemotron-3.5-lightning-free", "opencode/deepseek-v4.1-flash-free"]);
}
