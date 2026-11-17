// Carries packages/runtime-adapters/test/effort-choices.test.ts (Electron 4ce2f37a): the effort levels a CLI says it
// takes are the ones Locust offers, and Copilot is sent the one chosen.
use crate::{approval::Mode, models::parse_effort_choices, runtime::copilot::prompt_args};

// Verbatim from copilot 1.0.83, wrapping and quotes included.
const COPILOT_HELP: &str = "
  --allow-all-tools                     Allow all tools without confirmation
  --effort, --reasoning-effort <level>  Set the reasoning effort level (choices:
                                        \"none\", \"minimal\", \"low\", \"medium\",
                                        \"high\", \"xhigh\", \"max\")
  --enable-all-github-mcp-tools         Enable all GitHub MCP server tools
";

/// Claude writes the same fact a different way: bare words, one line.
const CLAUDE_HELP: &str = "
  --effort <level>  Effort level for the current session (low, medium, high)
";

#[test]
fn copilots_seven_levels_are_read_through_the_quotes_and_the_line_wraps() {
    assert_eq!(parse_effort_choices(COPILOT_HELP), ["none", "minimal", "low", "medium", "high", "xhigh", "max"]);
}

#[test]
fn the_bare_single_line_form_is_still_read() {
    assert_eq!(parse_effort_choices(CLAUDE_HELP), ["low", "medium", "high"]);
}

#[test]
fn a_cli_that_names_no_levels_is_given_none() {
    assert!(parse_effort_choices("--help\n  --model <name>  The model to use\n").is_empty());
    assert!(parse_effort_choices("").is_empty());
}

fn copilot(effort: Option<&str>) -> Vec<String> {
    prompt_args("Summarise this repository.", Mode::Ask, None, effort, None, None, &[], None)
}

#[test]
fn copilot_is_passed_the_effort_as_effort() {
    let args = copilot(Some("high"));
    let at = args.iter().position(|a| a == "--effort").expect("--effort");
    assert_eq!(args[at + 1], "high");
}

#[test]
fn copilot_is_sent_no_effort_flag_when_none_was_chosen() {
    assert!(!copilot(None).iter().any(|a| a == "--effort"));
}

// Rust's own (commands.ts reads only plain lowercase words): an empty entry in the list is no level.
#[test]
fn an_empty_entry_in_the_list_is_no_level() {
    assert_eq!(parse_effort_choices("  --effort <level>  Effort level (low, , high,)"), ["low", "high"]);
}
