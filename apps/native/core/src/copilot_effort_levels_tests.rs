// Carries packages/runtime-adapters/test/copilot-effort-levels.test.ts (Electron 4ce2f37a): parseEffortChoices.
use crate::models::parse_effort_choices;

const COPILOT_1_0_88: &str = "      --reasoning-effort <level>\n          Set the reasoning effort level [possible values: none, minimal, low, medium, high, xhigh, max]\n      --context <tier>";

#[test]
fn copilots_effort_levels_are_read_from_its_1_0_88_help() {
    assert_eq!(parse_effort_choices(COPILOT_1_0_88), ["none", "minimal", "low", "medium", "high", "xhigh", "max"]);
}

#[test]
fn copilots_effort_levels_are_still_read_from_the_older_parenthesised_shape() {
    assert_eq!(
        parse_effort_choices(r#"--effort, --reasoning-effort <level>  Set the effort (choices: "low", "high")"#),
        ["low", "high"]
    );
}
