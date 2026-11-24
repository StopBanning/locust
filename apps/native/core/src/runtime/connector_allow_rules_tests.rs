//! The second half of Electron's `packages/runtime-adapters/test/connectors.test.ts`: the person's connectors, read
//! off `claude mcp list` by `connector_reader::parse_claude_connectors`, become the prefix their tools carry and the
//! allow rule `print_args` puts on a Claude Code turn's command line (runtime/claude.rs). Checked against ten real
//! tool names from a live session, never against the derivation itself.
use super::*;
use crate::connector_reader::parse_claude_connectors;

/// `claude mcp list` on claude 2.1.267, verbatim.
const REAL_OUTPUT: &str = "Checking MCP server health…

claude.ai Anthropic Economic Index: https://econ-index.mcp.claude.com/mcp - ✔ Connected
claude.ai Google Calendar: https://calendarmcp.googleapis.com/mcp/v1 - ✔ Connected
claude.ai Notion: https://mcp.notion.com/mcp - ! Needs authentication
claude.ai Zapier: https://mcp.zapier.com/api/v1/connect - ✔ Connected
claude.ai Clay: https://api.clay.com/v3/mcp - ✔ Connected
claude.ai Canva: https://mcp.canva.com/mcp - ! Needs authentication
claude.ai Robinhood: https://agent.robinhood.com/mcp/trading - ✔ Connected
claude.ai Gmail: https://gmailmcp.googleapis.com/mcp/v1 - ✔ Connected
claude.ai Figma: https://mcp.figma.com/mcp - ✔ Connected
claude.ai Google Drive: https://drivemcp.googleapis.com/mcp/v1 - ✔ Connected";

/// Real tool names from the same account, one per connector above.
const REAL_TOOL_NAMES: [&str; 10] = [
    "mcp__claude_ai_Anthropic_Economic_Index__econ_index_list_countries",
    "mcp__claude_ai_Google_Calendar__list_calendars",
    "mcp__claude_ai_Notion__authenticate",
    "mcp__claude_ai_Zapier__list_zapier_skills",
    "mcp__claude_ai_Clay__get-current-workspace",
    "mcp__claude_ai_Canva__authenticate",
    "mcp__claude_ai_Robinhood__get_watchlists",
    "mcp__claude_ai_Gmail__list_labels",
    "mcp__claude_ai_Figma__whoami",
    "mcp__claude_ai_Google_Drive__list_recent_files",
];

fn names() -> Vec<String> {
    parse_claude_connectors(REAL_OUTPUT).into_iter().map(|c| c.name).collect()
}

/// The allow rules a turn in Accept edits is launched with for these connectors.
fn launched_rules(connectors: &[&str]) -> Option<String> {
    let args = print_args(&PrintOptions { mode: Mode::AcceptEdits, connectors, ..Default::default() }).unwrap();
    let at = args.iter().position(|a| a == "--allowedTools")?;
    args.get(at + 1).cloned()
}

#[test]
fn derive_the_prefix_every_one_of_their_tools_really_carries() {
    let derived: Vec<String> = names().iter().map(|n| tool_prefix_for(n)).collect();
    assert_eq!(derived.len(), REAL_TOOL_NAMES.len());
    for (prefix, tool) in derived.iter().zip(REAL_TOOL_NAMES) {
        assert!(tool.starts_with(prefix.as_str()), "{prefix} vs {tool}");
    }
}

#[test]
fn become_an_allow_rule_shaped_the_way_the_cli_demands() {
    assert_eq!(allow_rule_for("claude.ai Robinhood").as_deref(), Some("mcp__claude_ai_Robinhood__*"));
    let shape = regex::Regex::new(r"^mcp__[A-Za-z0-9_-]+__\*$").unwrap();
    let names = names();
    for name in &names {
        let rule = allow_rule_for(name).expect("a rule for every real connector");
        assert!(shape.is_match(&rule), "{rule}");
        assert_ne!(rule, "mcp__*");
    }
    let named: Vec<&str> = names.iter().map(String::as_str).collect();
    let launched = launched_rules(&named).expect("the allow rules on the command line");
    assert_eq!(launched.split(',').count(), 10, "{launched}");
    for rule in launched.split(',') {
        assert!(shape.is_match(rule), "{rule}");
    }
}

#[test]
fn refuse_to_make_a_rule_at_all_from_a_name_that_sanitises_to_nothing() {
    for hostile in ["", "   ", "...", "___", "!!!", "•", "--"] {
        assert_eq!(allow_rule_for(hostile), None, "{hostile:?}");
        assert_eq!(launched_rules(&[hostile]), None, "{hostile:?}");
    }
}

#[test]
fn keep_a_local_servers_hyphens_and_underscores_as_claude_code_does() {
    assert_eq!(tool_prefix_for("robinhood-trading"), "mcp__robinhood-trading__");
    assert_eq!(tool_prefix_for("my_server"), "mcp__my_server__");
    assert_eq!(tool_prefix_for("my server.v2"), "mcp__my_server_v2__");
    assert_eq!(tool_prefix_for("a  b"), "mcp__a__b__");
    assert_eq!(tool_prefix_for("claude.ai Google  Drive"), "mcp__claude_ai_Google_Drive__");
    assert_eq!(allow_rule_for("robinhood-trading").as_deref(), Some("mcp__robinhood-trading__*"));
    assert_eq!(launched_rules(&["robinhood-trading"]).as_deref(), Some("mcp__robinhood-trading__*"));
}
