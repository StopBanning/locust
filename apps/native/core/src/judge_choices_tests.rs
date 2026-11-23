//! your-own-models-can-judge.test.ts (2 cases). Electron reads `judgeChoicesFor` out of App.tsx; these hold the same two
//! promises against what the function offers.
use super::*;

fn listed(runtime: &str, id: &str, name: &str) -> JudgeModel {
    JudgeModel { runtime: runtime.into(), id: id.into(), display_name: name.into(), ..Default::default() }
}

fn own(id: &str, name: &str, chat_only: bool) -> JudgeModel {
    JudgeModel { own: true, chat_only, ..listed("opencode", id, name) }
}

fn runtime_name(runtime: &str) -> String {
    match runtime {
        "opencode" => "OpenCode".into(),
        "claude" => "Claude Code".into(),
        other => other.into(),
    }
}

fn labels(choices: &[JudgeChoice]) -> Vec<&str> {
    choices.iter().map(|one| one.label.as_str()).collect()
}

#[test]
fn offers_the_persons_own_models_that_take_tools_under_your_models_and_by_their_own_names() {
    // case: offers the person’s own models that take tools, under Your models and by their own names
    let models = [
        listed("claude", "claude-sonnet-5-5", "Sonnet 5.5"),
        own("own-0a1b2c3d/acme-70b", "Acme 70B", false),
        own("own-1b2c3d4e/claude-haiku-5-5", "claude-haiku-5-5", false),
        own("own-2c3d4e5f/llama3:8b", "Chatty", true),
    ];
    let choices = judge_choices_for(&[], None, &models, |_| true, runtime_name);
    assert_eq!(labels(&choices), ["Claude Code / Sonnet 5.5", "Your models / Acme 70B", "Your models / Claude Haiku 5.5"]);
    let acme = &choices[1];
    assert_eq!((acme.group.as_str(), acme.short.as_str()), ("Your models", "Acme 70B"));
    assert_eq!((acme.runtime.as_str(), acme.model.as_str()), ("opencode", "own-0a1b2c3d/acme-70b"));
    // One that only chats is left out, whatever else is offered.
    assert!(choices.iter().all(|one| !one.model.contains("llama3")));
    // Not ready, not offered: an own model rides OpenCode.
    assert!(judge_choices_for(&[], None, &models, |runtime| runtime != "opencode", runtime_name).iter().all(|one| one.group != "Your models"));
}

#[test]
fn still_offers_a_free_model_first_and_the_catalogue_before_any_model_of_the_persons_own() {
    // case: still offers a free model first, and the catalogue before any model of the person’s own
    let models = [
        own("own-0a1b2c3d/acme-70b", "Acme 70B", false),
        listed("opencode", "big-pickle", "Big Pickle"),
        listed("opencode", "nemotron-3-5-free", "Nemotron 3.5 Free"),
        JudgeModel { older: true, ..listed("claude", "claude-opus-4-1", "Opus 4.1") },
        listed("claude", "claude-sonnet-5-5", "Sonnet 5.5"),
    ];
    let choices = judge_choices_for(&["grok-code-free".to_owned()], Some(("claude", "claude-sonnet-5-5")), &models, |_| true, runtime_name);
    assert_eq!(
        labels(&choices),
        ["OpenCode / Grok Code Free", "Claude Code / Sonnet 5.5", "OpenCode / Nemotron 3.5 Free", "OpenCode / Big Pickle", "Your models / Acme 70B"]
    );
    // Without a free model asked before, the catalogue's free one still leads it, and the person's own still comes last.
    let catalogue_only = judge_choices_for(&[], None, &models, |_| true, runtime_name);
    assert_eq!(labels(&catalogue_only), ["OpenCode / Nemotron 3.5 Free", "OpenCode / Big Pickle", "Claude Code / Sonnet 5.5", "Your models / Acme 70B"]);
}
