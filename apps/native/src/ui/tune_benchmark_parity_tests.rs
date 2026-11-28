use super::*;
use locust_core::ledger::{Item, Snapshot};
use rune::{Env, Runtime};

fn sample(cmp_id: &str, setups: bool, reruns: Vec<String>) -> Compare {
    use locust_core::compare::{CompareJudge, CompareRoute, CompareSlot};
    Compare {
        compare_id: cmp_id.into(),
        teammate_id: Some("tm_wren".into()),
        prompt: "Explain the cause.".into(),
        created_at: "2026-10-21T12:00:00.000Z".into(),
        slots: [CompareSlotId::A, CompareSlotId::B, CompareSlotId::C]
            .into_iter()
            .map(|slot| {
                let mut route = CompareRoute::new(
                    "opencode",
                    if setups {
                        "opencode/example-free"
                    } else {
                        slot.as_str()
                    },
                );
                route.instructions = setups.then(|| format!("Instructions for {}", slot.as_str()));
                CompareSlot {
                    slot,
                    route,
                    mission_ids: vec![format!("{cmp_id}_{}", slot.as_str())],
                    refused: None,
                    retried: vec![],
                    folder: None,
                }
            })
            .collect(),
        changes: false,
        changes_in: None,
        blind: true,
        kept: None,
        judge: Some(CompareJudge {
            route: CompareRoute::new("codex", "judge-model"),
            mission_ids: vec![format!("{cmp_id}_judge")],
            criteria: None,
            panel: vec![],
            panel_mission_ids: vec![],
        }),
        base: None,
        rerun_of: None,
        reruns,
        settled: false,
        point_drafts: vec![],
    }
}

fn said(words: &str) -> Snapshot {
    Snapshot {
        items: vec![Item::Assistant {
            item_id: "answer".into(),
            text: words.into(),
            r#final: true,
        }],
        ..Default::default()
    }
}

fn state(setups: bool, again: bool) -> State {
    let mut s = State::default();
    s.demo = true;
    crate::team_scene(&mut s);
    s.teammates[0].id = "tm_wren".into();
    s.compare.active_id = Some("first".into());
    s.compare.compares = vec![sample(
        "first",
        setups,
        if again { vec!["second".into()] } else { vec![] },
    )];
    if again {
        s.compare.compares.push(sample("second", setups, vec![]));
    }
    for id in ["first", "second"] {
        for slot in ["a", "b", "c"] {
            s.compare
                .snapshots
                .insert(format!("{id}_{slot}"), said("An answer."));
        }
        let c2 = if id == "first" { "not met" } else { "met" };
        let judge = format!(
            "POINT 1: The cause\nPOINT 2: The evidence\nSCORE A 1: met. yes\nSCORE A 2: not met. no\nSCORE B 1: met. yes\nSCORE B 2: met. yes\nSCORE C 1: met. yes\nSCORE C 2: {c2}. checked"
        );
        s.compare
            .snapshots
            .insert(format!("{id}_judge"), said(&judge));
    }
    s
}

fn drawn(s: State) -> Vec<String> {
    let mut rt = Runtime::new(
        s,
        |s, _| view(s, &Palette::new(true)),
        (1280.0, 4000.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|el| el.text).collect()
}

// case: keeps a row a setup, by its column, though all three are one model
#[test]
fn same_model_tune_benchmark_keeps_three_scored_setup_rows() {
    let s = state(true, true);
    let runs = benchmark_runs_for(&s, &s.compare.compares[0]);
    let rows = locust_core::benchmark::benchmark_rows(&runs);
    let shown: Vec<_> = rows
        .iter()
        .map(|row| (row.name.as_str(), row.slot, row.score))
        .collect();
    assert_eq!(
        shown,
        [
            ("Setup B", Some(CompareSlotId::B), Some(100)),
            ("Setup C", Some(CompareSlotId::C), Some(75)),
            ("Setup A", Some(CompareSlotId::A), Some(50)),
        ]
    );
}

// case: reads as setups, with nothing to use for the teammate but Keep
#[test]
fn tune_benchmark_draws_setup_language_and_only_keeps_a_setup() {
    let once = drawn(state(true, false));
    assert!(
        once.iter().any(|w| w.starts_with(
            "One run can be luck. Run the three setups again and keep the one whose scores hold"
        )),
        "{once:?}"
    );
    let reran = drawn(state(true, true));
    assert!(reran.iter().any(|w| w == "Setup"), "{reran:?}");
    assert!(
        reran.iter().any(|w| w.starts_with(
            "Each run asks the same model the same question under each setup’s instructions"
        )),
        "{reran:?}"
    );
    assert!(
        !reran
            .iter()
            .any(|w| w.contains("Use for") || w.contains("opencode/example-free")),
        "{reran:?}"
    );
}

// case: says it reads the setups without their names, and that one run of each can be luck
#[test]
fn a_tunes_ask_judge_form_says_answers_are_blind_and_one_run_can_be_luck() {
    let words = drawn(state(true, false));
    // CompareView.tsx:771-773 and :800: the judge note names the setups, and the luck line is its own paragraph.
    assert!(words.iter().any(|w| w.starts_with("It reads the answers without the setups’ names")), "{words:?}");
    assert!(
        words.iter().any(|w| w
            == "One run of each setup can be luck. Once the judge has scored them, run the three again, and keep the setup whose scores hold up."),
        "{words:?}"
    );
}

// case: says neither of a comparison of models
#[test]
fn an_ordinary_comparison_asks_the_judge_about_models_not_setups() {
    let words = drawn(state(false, false));
    assert!(
        words
            .iter()
            .any(|w| w.starts_with("It reads the answers without the models’ names")),
        "{words:?}"
    );
    assert!(
        !words
            .iter()
            .any(|w| w.contains("One run of each setup can be luck")),
        "{words:?}"
    );
}
