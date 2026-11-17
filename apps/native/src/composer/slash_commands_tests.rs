//! Electron's `slashCommands.test.ts`: recognising a command being typed, which are offered, narrowing, the commands
//! themselves, a runtime's own commands (0.426) and /setup.

use super::*;

const ALL_MODES: [&str; 5] = ["ask", "plan", "accept-edits", "approve-each", "auto"];

fn input<'a>(modes: &'a [&'a str], running: bool, can_swarm: bool) -> AvailableInput<'a> {
    AvailableInput { modes, running, can_swarm, issues: false, tune: false, setup: false, runtime_commands: &[] }
}
fn names(commands: &[SlashCommand]) -> Vec<String> {
    commands.iter().map(|c| c.name.clone()).collect()
}
fn every() -> Vec<SlashCommand> {
    available_commands(input(&ALL_MODES, true, true))
}
fn claude() -> Vec<SlashCommand> {
    let row = |n: &str, d: &str, h: &str| (n.to_owned(), d.to_owned(), h.to_owned());
    runtime_slash_commands(
        "Claude Code",
        &[
            row("compact", "Clear conversation history but keep a summary in context", "<optional custom summarization instructions>"),
            row("security-review", "", ""),
            row("plugin:skill", "A skill from a plugin", ""),
            row("plan", "Something else", ""),
        ],
    )
}

#[test]
fn reads_a_bare_slash_as_the_menu_opening() {
    assert_eq!(slash_query("/").as_deref(), Some(""));
}

#[test]
fn reads_what_has_been_typed_after_it() {
    assert_eq!(slash_query("/pl").as_deref(), Some("pl"));
    assert_eq!(slash_query("/PLAN").as_deref(), Some("plan"));
    assert_eq!(slash_query("  /stop  ").as_deref(), Some("stop"));
}

#[test]
fn ignores_a_slash_inside_a_sentence() {
    assert_eq!(slash_query("run /plan on this"), None);
    assert_eq!(slash_query("look at src/main/index.ts"), None);
    assert_eq!(slash_query("what does / mean here"), None);
}

#[test]
fn is_not_a_command_when_there_is_no_slash_at_all() {
    assert_eq!(slash_query("plan"), None);
    assert_eq!(slash_query(""), None);
}

#[test]
fn offers_tune_only_with_a_finished_teammate_conversation() {
    let has_tune = |running: bool, tune: bool| {
        available_commands(AvailableInput { tune, ..input(&["ask"], running, false) }).iter().any(|c| c.name == "tune")
    };
    assert!(!has_tune(false, false));
    assert!(has_tune(false, true));
    assert!(!has_tune(true, true));
}

#[test]
fn offers_only_modes_this_runtime_can_honour() {
    let limited = names(&available_commands(input(&["ask", "plan"], false, true)));
    assert!(limited.contains(&"ask".into()));
    assert!(limited.contains(&"plan".into()));
    assert!(!limited.contains(&"auto".into()));
    assert!(!limited.contains(&"approve".into()));
}

#[test]
fn offers_stop_only_while_something_is_running() {
    assert!(!names(&available_commands(input(&ALL_MODES, false, true))).contains(&"stop".into()));
    assert!(names(&every()).contains(&"stop".into()));
}

#[test]
fn offers_swarm_only_where_it_can_be_turned_on() {
    assert!(!names(&available_commands(input(&ALL_MODES, false, false))).contains(&"swarm".into()));
}

#[test]
fn always_offers_the_route_picker_which_is_always_reachable() {
    assert_eq!(names(&available_commands(input(&[], false, false))), ["model"]);
}

#[test]
fn offers_an_issue_only_for_a_folder_whose_issues_can_be_read() {
    assert!(!available_commands(input(&ALL_MODES, false, false)).iter().any(|c| c.name == "issue"));
    assert!(available_commands(AvailableInput { issues: true, ..input(&ALL_MODES, false, false) }).iter().any(|c| c.name == "issue"));
}

#[test]
fn keeps_the_ones_that_start_with_what_was_typed() {
    assert_eq!(names(&matching_commands("a", &every())), ["ask", "approve", "auto"]);
    assert_eq!(names(&matching_commands("st", &every())), ["stop"]);
}

#[test]
fn shows_everything_for_a_bare_slash() {
    assert_eq!(matching_commands("", &every()).len(), every().len());
}

#[test]
fn shows_nothing_rather_than_something_wrong_for_an_unknown_word() {
    assert!(matching_commands("zzz", &every()).is_empty());
}

#[test]
fn every_one_says_what_it_does() {
    for command in slash_commands() {
        assert!(command.detail.chars().count() > 10, "{}", command.name);
        assert!(!command.name.is_empty() && command.name.chars().all(|c| c.is_ascii_lowercase()), "{}", command.name);
    }
}

#[test]
fn names_no_command_twice() {
    let all = slash_commands();
    let unique: std::collections::HashSet<&str> = all.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(unique.len(), all.len());
}

#[test]
fn a_runtimes_own_commands_sit_under_the_runtime_after_locusts_own_with_what_goes_after_them() {
    let theirs = claude();
    let offered = available_commands(AvailableInput { runtime_commands: &theirs, ..input(&ALL_MODES, false, true) });
    let all = names(&offered);
    let auto_at = all.iter().position(|n| n == "auto").unwrap();
    let own: Vec<String> = slash_commands()
        .into_iter()
        .filter(|c| c.name != "stop" && !matches!(c.action, SlashAction::Issues | SlashAction::Tune | SlashAction::Setup))
        .map(|c| c.name)
        .collect();
    assert_eq!(all[..=auto_at], own[..]);
    assert_eq!(all[auto_at + 1..], ["compact", "security-review", "plugin:skill"]);
    let compact = offered.iter().find(|c| c.name == "compact").unwrap();
    assert_eq!(compact.group.as_deref(), Some("Claude Code"));
    assert_eq!(compact.hint.as_deref(), Some("<optional custom summarization instructions>"));
    assert_eq!(compact.action, SlashAction::Runtime);
    assert_eq!(offered.iter().find(|c| c.name == "security-review").unwrap().detail, "Claude Code's own command.");
    assert_eq!(offered.iter().filter(|c| c.name == "plan").count(), 1);
    assert!(matches!(offered.iter().find(|c| c.name == "plan").unwrap().action, SlashAction::Mode(_)));
}

#[test]
fn a_runtimes_own_commands_are_not_offered_while_a_run_is_going() {
    let theirs = claude();
    let offered = available_commands(AvailableInput { runtime_commands: &theirs, ..input(&ALL_MODES, true, true) });
    assert!(!offered.iter().any(|c| c.action == SlashAction::Runtime));
}

#[test]
fn a_runtimes_own_commands_are_found_by_what_is_typed_names_with_marks_included() {
    let theirs = claude();
    let offered = available_commands(AvailableInput { runtime_commands: &theirs, ..input(&ALL_MODES, false, true) });
    assert_eq!(slash_query("/security-r").as_deref(), Some("security-r"));
    assert_eq!(slash_query("/plugin:").as_deref(), Some("plugin:"));
    assert_eq!(slash_query("/team/review").as_deref(), Some("team/review"));
    assert_eq!(names(&matching_commands("security-r", &offered)), ["security-review"]);
    assert_eq!(names(&matching_commands("plugin:", &offered)), ["plugin:skill"]);
    assert_eq!(names(&matching_commands("co", &offered)), ["compact"]);
}

#[test]
fn setup_is_offered_in_a_teammates_conversation_while_it_runs_too_and_not_elsewhere() {
    let has = |setup: bool, running: bool| {
        names(&available_commands(AvailableInput { setup, ..input(&["ask"], running, false) })).contains(&"setup".into())
    };
    assert!(has(true, false));
    assert!(has(true, true));
    assert!(!has(false, false));
}
