//! Electron's `a-claude-route-names-its-version.test.tsx`, the cases Rust builds: `claudeModelName`
//! (`models::claude_model_name`), with Electron's values.
use crate::models::claude_model_name;

#[test]
fn spells_family_and_version_the_way_anthropic_writes_them() {
    assert_eq!(claude_model_name("claude-opus-5-5").as_deref(), Some("Opus 5.5"));
    assert_eq!(claude_model_name("claude-fable-5-1").as_deref(), Some("Fable 5.1"));
    assert_eq!(claude_model_name("claude-sonnet-5").as_deref(), Some("Sonnet 5"));
    assert_eq!(claude_model_name("claude-haiku-4-5-20251001").as_deref(), Some("Haiku 4.5"));
    assert_eq!(claude_model_name("claude-opus-4-1-20250805-v1").as_deref(), Some("Opus 4.1"));
    assert_eq!(claude_model_name("claude-opus-4-20250514").as_deref(), Some("Opus 4"));
}

#[test]
fn guesses_at_nothing_it_does_not_recognise() {
    assert_eq!(claude_model_name("opus"), None);
    assert_eq!(claude_model_name("claude-fable-5-mythos-5"), None);
    assert_eq!(claude_model_name("gpt-6"), None);
}

#[test]
fn carries_claude_codes_own_alias_table_2_1_293_haiku_5_5() {
    assert_eq!(
        crate::models::CLAUDE_ALIAS_DEFAULTS,
        [("fable", "claude-fable-5-1"), ("opus", "claude-opus-5-5"), ("sonnet", "claude-sonnet-5-5"), ("haiku", "claude-haiku-5-5")]
    );
}

// what a route reads as
#[test]
fn is_the_version_the_alias_means_until_a_run_on_it_says_otherwise() {
    use crate::models::claude_route_model_name as named;
    assert_eq!(named("opus", None, None).as_deref(), Some("Opus 5.5"));
    // A gateway maps opus elsewhere; the run's own report wins.
    assert_eq!(named("opus", Some("claude-opus-4-7"), None).as_deref(), Some("Opus 4.7"));
    // A pinned full id reads as itself.
    assert_eq!(named("claude-opus-4-8", None, None).as_deref(), Some("Opus 4.8"));
}

#[test]
fn takes_claude_codes_own_name_for_it_over_the_copied_table_and_a_runs_report_over_both_0_697() {
    use crate::models::claude_route_model_name as named;
    assert_eq!(named("haiku", None, Some("Haiku 6")).as_deref(), Some("Haiku 6"));
    assert_eq!(named("haiku", Some("claude-haiku-4-5-20251001"), Some("Haiku 6")).as_deref(), Some("Haiku 4.5"));
    assert_eq!(named("haiku", None, Some("")).as_deref(), Some("Haiku 5.5"));
    // The same through the route: the name Claude Code listed for the alias, read from the catalogue.
    crate::teammate_tooltip::remember_own_models([("claude", "haiku", "Haiku 6")]);
    assert_eq!(crate::teammate_tooltip::route_model_name("claude", "haiku"), "Haiku 6");
    assert_eq!(crate::teammate_tooltip::route_model_name_earned("claude", "haiku", Some("claude-haiku-4-5-20251001")), "Haiku 4.5");
}

#[test]
fn names_the_version_on_the_controls_that_pick_a_route_and_nothing_else_changes() {
    use crate::teammate_tooltip::{model_display_name, route_model_name, route_model_name_earned};
    assert_eq!(route_model_name("claude", "fable"), "Fable 5.1");
    assert_eq!(route_model_name_earned("claude", "opus", Some("claude-opus-5-5-20260901")), "Opus 5.5");
    assert_eq!(route_model_name("cursor", "cursor-grok-4.6"), "Grok 4.6");
    // A past mission's row keeps the alias it ran on: what "opus" meant in August is not today's version.
    assert_eq!(model_display_name("claude", "opus"), "Opus");
    // The picker's row says the same words (RoutePicker's label: routeModelName with the run's report).
    assert_eq!(crate::route_rows::picker_row_label("claude", "opus", "Opus 5.5", Some("claude-opus-5-5-20260901")), "Opus 5.5");
}
