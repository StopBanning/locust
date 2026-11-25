//! Electron's `the-screens-say-less.test.ts` (0.723), the cases Rust has a surface for. A route on the account's own
//! default is the runtime's name alone on every surface that spells one: the composer chip's `route_chrome`, the
//! project dialog's route, the teammate hover, the rail and the empty conversation's agent line (`crate::agent`), and
//! the exchange strip. A teammate card's Recent lists conversations, not turns (`teammate_work`).
//!
//! Not carried: "the conversation list's cost cell". Electron's Missions screen has a cost column (`costGlanceCell`);
//! the Rust app has no Missions screen, so there is no cell to spell.
use super::*;
use locust_core::engine::Teammate;
use locust_core::groups::GroupRoute;
use locust_core::teammate_tooltip::{route_chrome, teammate_tooltip};

fn mate(runtime: &str, model: &str) -> Teammate {
    Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Code & Migrations".into(),
        runtime: Some(runtime.into()),
        model: Some(model.into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}
fn group_route(runtime: &str, model: &str) -> GroupRoute {
    GroupRoute { runtime: runtime.into(), model: model.into(), mode: "auto".into(), account: None, effort: None }
}

#[test]
fn is_the_runtimes_name_alone_wherever_routes_are_spelled() {
    assert_eq!(route_chrome("codex", "account-default", "Account Default", " / "), "Codex");
    assert_eq!(route_chrome("claude", "account-default", "default", " / "), "Claude");
    // Each surface that spells a route, on the account's own default.
    let s = State::default();
    assert_eq!(crate::group_settings_dialog::route_label(&s, &group_route("codex", "account-default")), "Codex");
    assert_eq!(crate::group_settings_dialog::route_label(&s, &group_route("claude", "account-default")), "Claude");
    assert_eq!(teammate_tooltip(&mate("codex", "account-default")), "Message Wren · Code & Migrations · Codex");
    assert_eq!(crate::agent(&mate("codex", "account-default")), "Codex");
    assert_eq!(participant_route("codex", "account-default"), "Codex");
    assert_eq!(participant_route("claude", "account-default"), "Claude");
    // And a model of its own still says which.
    assert_eq!(crate::agent(&mate("codex", "gpt-6-luna")), "Codex · gpt-6-luna");
    assert_eq!(participant_route("codex", "gpt-6-luna"), "Codex / GPT-6 Luna");
}

#[test]
fn keeps_what_follows_the_models_name_and_every_other_model_as_it_was() {
    assert_eq!(route_chrome("codex", "account-default", "Account Default · Medium", " · "), "Codex · Medium");
    assert_eq!(route_chrome("codex", "gpt-6-luna", "GPT-6 Luna", " / "), "Codex / GPT-6 Luna");
    assert_eq!(crate::group_settings_dialog::route_label(&State::default(), &group_route("codex", "gpt-6-luna")), "Codex / GPT-6 Luna");
}

fn turn(mission_id: &str, at: &str, prompt: &str, continues_from: Option<&str>) -> ledger::Summary {
    ledger::Summary {
        mission_id: mission_id.into(),
        title: prompt.into(),
        words: prompt.into(),
        runtime: "codex".into(),
        model: "gpt".into(),
        created_at: at.into(),
        updated: at.into(),
        parent: continues_from.map(str::to_owned),
        continues_reason: continues_from.map(|_| "follow-up".to_owned()),
        ..Default::default()
    }
}

#[test]
fn lists_conversations_each_named_by_its_first_words_not_each_turn_of_one() {
    let missions = [
        turn("a1", "2026-10-10T10:00:00.000Z", "Fix the login redirect loop", None),
        turn("a2", "2026-10-10T10:02:00.000Z", "Also check the signup form", Some("a1")),
        turn("a3", "2026-10-10T10:04:00.000Z", "Good, ship it", Some("a2")),
        turn("b1", "2026-10-10T09:00:00.000Z", "Rename the billing module", None),
    ];
    let owners: std::collections::HashMap<String, String> =
        ["a1", "a2", "a3", "b1"].iter().map(|m| (m.to_string(), "tm_wren".to_string())).collect();
    // The title the card is given is the one the people panel gives it.
    let work = locust_core::teammate_work::teammate_work("tm_wren", &missions, &owners, |m| m.title.clone());
    // The newest turn opens the conversation and gives its time; the first names it.
    let recent: Vec<(&str, &str)> = work.recent.iter().map(|r| (r.mission.mission_id.as_str(), r.title.as_str())).collect();
    assert_eq!(recent, [("a3", "Fix the login redirect loop"), ("b1", "Rename the billing module")]);
    // Runs still counts every turn.
    assert_eq!(work.mission_count, 4);
}

// case: says what a person pays, or that a model is free, and nothing for counted tokens
#[test]
fn missions_screen_cost_cell_says_what_a_person_pays_or_that_a_model_is_free_and_nothing_for_counted_tokens() {
    use crate::missions_screen::cost_glance_cell;
    use locust_core::benchmark::RunCost;

    assert_eq!(cost_glance_cell(&[Some(RunCost { usd: Some(0.42), ..Default::default() })], "gpt-6-luna"), "$0.42");
    assert_eq!(cost_glance_cell(&[Some(RunCost { premium_requests: Some(1.0), ..Default::default() })], "auto"), "1 premium request");
    assert_eq!(cost_glance_cell(&[None], "opencode/nemotron-3-ultra-free"), "free");
    assert_eq!(
        cost_glance_cell(&[Some(RunCost { input_tokens: Some(9000), output_tokens: Some(870), ..Default::default() })], "account-default"),
        ""
    );
    assert_eq!(
        cost_glance_cell(
            &[Some(RunCost { plan: true, input_tokens: Some(9000), output_tokens: Some(870), ..Default::default() })],
            "account-default"
        ),
        ""
    );
    assert_eq!(cost_glance_cell(&[None], "account-default"), "");

    // Drawn through missions_screen::view: priced row shows $0.42, free model shows free, tokens-only shows nothing.
    let mut s = State::default();
    let add = |s: &mut State, id: &str, model: &str, cost: Option<RunCost>| {
        s.recent.push(ConversationRow {
            id: id.into(),
            title: format!("Task {id}"),
            teammate: "Wren".into(),
            updated: "2026-10-10T10:00:00.000Z".into(),
            state: "completed".into(),
            started_by: String::new(),
        });
        s.ledger_summaries.insert(
            id.into(),
            ledger::Summary {
                mission_id: id.into(),
                title: format!("Task {id}"),
                runtime: "codex".into(),
                model: model.into(),
                created_at: "2026-10-10T10:00:00.000Z".into(),
                updated: "2026-10-10T10:00:05.000Z".into(),
                cost,
                ..Default::default()
            },
        );
    };
    add(&mut s, "m_paid", "gpt-6-luna", Some(RunCost { usd: Some(0.42), ..Default::default() }));
    add(&mut s, "m_free", "opencode/nemotron-3-ultra-free", None);
    add(&mut s, "m_tok", "account-default", Some(RunCost { input_tokens: Some(9000), output_tokens: Some(870), ..Default::default() }));

    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::missions_screen::view(s, &Palette::new(true), false),
        (1200.0, 600.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let words: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(words.iter().any(|w| w == "$0.42"), "{words:?}");
    assert!(words.iter().any(|w| w == "free"), "{words:?}");
    assert!(!words.iter().any(|w| w.contains("9,870") || w.contains("tokens")), "{words:?}");
}
