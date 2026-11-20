//! judges-see-the-check.test.tsx, "the benchmark, as Colin's screenshot of 10/21 found it": a compared model's name.
use super::*;

// case: reads a model added under its own id by its name, and leaves a name the person wrote alone
#[test]
fn reads_a_model_added_under_its_own_id_by_its_name_and_leaves_a_name_the_person_wrote_alone() {
    assert_eq!(shown_route_label("own-0a1b2c3d/claude-haiku-5-5", Some("claude-haiku-5-5")).as_deref(), Some("Claude Haiku 5.5"));
    assert_eq!(shown_route_label("own-0a1b2c3d/claude-sonnet-5-5", Some("Sonnet at work")).as_deref(), Some("Sonnet at work"));
    // A label that is the route's whole id is no name (0.811): the caller spells the model, as the picker does.
    assert_eq!(shown_route_label("opencode/deepseek-v4.1-flash-free", Some("opencode/deepseek-v4.1-flash-free")), None);
    assert_eq!(shown_route_label("gpt-6-sol", None), None);
    assert_eq!(shown_route_label("gpt-6-sol", Some("  ")), None);
    // A name of the person's own that only looks like an id elsewhere stands as written.
    assert_eq!(shown_route_label("opencode/claude-haiku-5-5", Some("claude-haiku-5-5")).as_deref(), Some("claude-haiku-5-5"));
}

#[test]
fn a_compared_model_is_named_by_its_label_else_its_spelled_id() {
    let route = |model: &str, label: Option<&str>| CompareRoute {
        runtime: "opencode".into(),
        model: model.into(),
        account: None,
        effort: None,
        label: label.map(str::to_owned),
        mode: None,
        instructions: None,
        why: None,
        skill: None,
    };
    assert_eq!(compare_route_name(&route("own-0a1b2c3d/claude-haiku-5-5", Some("claude-haiku-5-5"))), "Claude Haiku 5.5");
    assert_eq!(compare_route_name(&route("own-0a1b2c3d/claude-sonnet-5-5", Some("Sonnet at work"))), "Sonnet at work");
    let spelled = compare_route_name(&route("opencode/deepseek-v4.1-flash-free", Some("opencode/deepseek-v4.1-flash-free")));
    assert_ne!(spelled, "opencode/deepseek-v4.1-flash-free", "never the raw id");
}
