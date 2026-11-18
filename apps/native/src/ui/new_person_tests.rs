//! Electron's `a-new-person-is-met-where-they-are.test.tsx`, the cases Rust builds, with Electron's values and words:
//! a teammate's starters in an empty folder (`idle_teammate.rs`'s `starters`, Electron's `startersFor`), which
//! teammate a starter row goes to (`starters::teammate_for_starter`), and Home's usage line
//! (`usage::usage_summary`, `usage_line_facts`). Two cases read Electron's `App.tsx` and `FirstLaunch.tsx` source.
use super::*;
use crate::starters::{TEAM_TEMPLATES, TemplateTeammate, teammate_for_starter};
use locust_core::usage::{UsageRange, UsageTurn, usage_line_facts, usage_summary};
use serde_json::json;

fn mate(team: &str, name: &str) -> &'static TemplateTeammate {
    TEAM_TEMPLATES.iter().find(|t| t.template_id == team).unwrap().teammates.iter().find(|m| m.name == name).unwrap()
}
fn starters_for(m: &TemplateTeammate, fresh: bool) -> Vec<String> {
    starters(&json!({ "starters": m.starters }), m.role, fresh)
}
fn owned(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| s.to_string()).collect()
}

#[test]
fn in_an_empty_folder_offers_to_make_something_not_to_read_a_project_that_is_not_there() {
    let wren = mate("software", "Wren");
    let fresh = starters_for(wren, true);
    assert_eq!(fresh, owned(wren.fresh_starters.expect("Wren has fresh starters")));
    let joined = fresh.join(" ").to_lowercase();
    assert!(!["this project", "this repo", "riskiest"].iter().any(|w| joined.contains(w)), "{joined}");
    assert!(starters(&json!({}), "Code & Migrations", true).join(" ").contains("Make a small web page here"));
}

#[test]
fn offers_its_own_starters_once_the_folder_holds_something_and_one_without_fresh_ones_keeps_its_own() {
    let wren = mate("software", "Wren");
    assert_eq!(starters_for(wren, false), owned(wren.starters));
    let sable = mate("money", "Sable");
    assert_eq!(starters_for(sable, true), owned(sable.starters));
}

/// The function's half; that the window passes the starter's id and the team's roles is Electron's `App.tsx` source.
#[test]
fn sends_a_starter_row_to_the_teammate_whose_work_it_is() {
    let team: Vec<(String, String)> = [("wren", "Code & Migrations"), ("juno", "Docs & QA"), ("atlas", "Research & Briefs")]
        .iter().map(|(id, role)| (id.to_string(), role.to_string())).collect();
    assert_eq!(teammate_for_starter("page", &team).as_deref(), Some("wren"));
    assert_eq!(teammate_for_starter("debug", &team).as_deref(), Some("wren"));
    assert_eq!(teammate_for_starter("review", &team).as_deref(), Some("juno"));
    assert_eq!(teammate_for_starter("review", &team[..1]).as_deref(), Some("wren"));
    assert_eq!(teammate_for_starter("page", &[("sable".into(), "Research & Briefs".into())]), None);
}

fn at(hour: u32) -> chrono::DateTime<chrono::Local> {
    use chrono::TimeZone;
    chrono::Local.with_ymd_and_hms(2026, 10, 30, hour, 0, 0).single().unwrap()
}
fn turn(runtime: &str, model: &str, conversation: &str, plan: bool) -> UsageTurn {
    UsageTurn { at: at(20).to_rfc3339(), runtime: runtime.into(), model: model.into(), conversation: conversation.into(),
        input_tokens: Some(58_000), output_tokens: Some(1_000), cache_read_tokens: None, cache_write_tokens: None, usd: None,
        premium_requests: None, plan }
}

#[test]
fn the_usage_line_says_all_free_when_every_turn_was_on_a_free_model() {
    let turns = [turn("opencode", "deepseek-v4.1-flash-free", "c1", false)];
    assert_eq!(usage_line_facts(&usage_summary(&turns, UsageRange::Days30, at(21))), "1 turn · 59k tokens · all free");
}

#[test]
fn the_usage_line_says_nothing_of_free_when_any_turn_was_not() {
    let turns = [turn("opencode", "deepseek-v4.1-flash-free", "c1", false), turn("claude", "claude-opus-4-6", "c2", true)];
    assert!(!usage_line_facts(&usage_summary(&turns, UsageRange::Days30, at(21))).contains("free"));
}
