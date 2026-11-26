//! Live hour 1, faults 19-21: the Team screen's cards say what Electron's say (Screens.tsx `TeammatesScreen`): Runs is
//! every conversation the record gives the teammate, a route is "Runtime / Model" in the model's own name, and the
//! screen carries no line about a copy.
use crate::*;
use rune::{Env, Runtime};

/// Colin's four teammates as the live hour showed them: the route ids are the raw ones his teammates.json holds.
fn state() -> State {
    let mut s = State::default();
    crate::team_scene(&mut s);
    s.ledger_summaries.clear();
    let route = |runtime: &str, model: &str| serde_json::json!({"runtime": runtime, "model": model, "mode": "ask"});
    let owners: serde_json::Map<String, serde_json::Value> = [("codec", 167), ("argon", 76), ("scout", 5)]
        .into_iter()
        .flat_map(|(who, n)| (0..n).map(move |i| (format!("{who}-{i}"), serde_json::json!(who))))
        .collect();
    s.people_panel.team = serde_json::json!({
        "teammates": [
            {"teammateId": "codec", "name": "Codec", "role": "Code & Migrations", "route": route("codex", "gpt-6.1-sol")},
            {"teammateId": "argon", "name": "Argon", "role": "Code & Migrations", "route": route("opencode", "opencode/deepseek-v4.1-flash-free")},
            {"teammateId": "scout", "name": "Scout", "role": "Custom", "route": route("opencode", "own-bf9f2516/claude-sonnet-5-5")},
        ],
        "missionOwners": owners,
    });
    s.people_panel.open = true;
    s.people_panel.tab = 0;
    // What the catalogues say, as the window reads them: Codex's own spelling, and the name Colin gave his own model.
    locust_core::teammate_tooltip::remember_own_models([
        ("codex", "gpt-6.1-sol", "GPT-6.1-Sol"),
        ("opencode", "own-bf9f2516/claude-sonnet-5-5", "Claude API"),
    ]);
    s
}

fn words(s: State) -> Vec<String> {
    let mut rt = Runtime::new(s, |s, _| crate::people_panel::view(s, &Palette::new(true)), (1200.0, 1800.0), Env::new(true, 1.0));
    for _ in 0..40 {
        rt.frame(0.05);
    }
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

#[test]
fn runs_are_every_conversation_the_record_gives_a_teammate_not_the_ones_this_window_read() {
    // The window has read none of them (`ledger_summaries` is empty), and the record names 167, 76 and 5.
    let said = words(state());
    for count in ["167", "76", "5"] {
        assert!(said.iter().any(|t| t == count), "Runs {count} is on a card: {said:?}");
    }
    assert!(!said.iter().any(|t| t == "0"), "no teammate with work reads 0 runs: {said:?}");
}

#[test]
fn a_teammate_with_no_conversations_says_zero() {
    let mut s = state();
    s.people_panel.team["missionOwners"] = serde_json::json!({"other": "someone-else"});
    let said = words(s);
    assert_eq!(said.iter().filter(|t| *t == "0").count(), 3, "{said:?}");
}

#[test]
fn a_route_is_the_runtime_and_the_models_own_name_never_a_raw_id() {
    let said = words(state());
    assert!(said.iter().any(|t| t == "Codex / GPT-6.1-Sol"), "{said:?}");
    assert!(said.iter().any(|t| t == "OpenCode / DeepSeek V4.1 Flash Free"), "{said:?}");
    assert!(said.iter().any(|t| t == "Claude API"), "a model of your own is its own name alone: {said:?}");
    for raw in ["gpt-6.1-sol", "opencode/deepseek-v4.1-flash-free", "own-bf9f2516/claude-sonnet-5-5", "Codex CLI / gpt-6.1-sol"] {
        assert!(!said.iter().any(|t| t.contains(raw)), "{raw}: {said:?}");
    }
}

#[test]
fn the_screen_has_no_line_about_a_copy_seeded_from_electron() {
    let said = words(state());
    assert!(!said.iter().any(|t| t.contains("Native copy") || t.contains("seeded once")), "{said:?}");
    assert!(said.iter().any(|t| t == "Teammates & Memory"), "{said:?}");
}

#[test]
fn a_model_of_your_own_has_no_runtime_mark_in_front_of_its_name() {
    // Screens.tsx draws `RuntimeMark` unless `isOwnRoute`: a company's own model meets no other product's mark.
    let mut rt = Runtime::new(state(), |s, _| crate::people_panel::view(s, &Palette::new(true)), (1200.0, 1800.0), Env::new(true, 1.0));
    for _ in 0..40 {
        rt.frame(0.05);
    }
    let elements = rt.elements();
    let offset = |card: &str, route: &str| {
        let card = elements.iter().find(|e| e.key.as_deref() == Some(card)).unwrap().rect[0];
        let text = elements.iter().find(|e| e.text.as_deref() == Some(route)).unwrap_or_else(|| panic!("{route}")).rect[0];
        text - card
    };
    let own = offset("team-card-scout", "Claude API");
    let plain = offset("team-card-codec", "Codex / GPT-6.1-Sol");
    assert!(plain - own >= 12.0, "the plain route has its mark, the own one starts {own}px in and the plain {plain}px");
}
