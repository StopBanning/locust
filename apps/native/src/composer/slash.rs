//! Slash commands — port of `renderer/src/slashCommands.ts`.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlashAction {
    Mode(&'static str),
    Route,
    Stop,
    Swarm,
    Issues,
    Tune,
    Setup,
    /// One of the runtime's own commands (0.426): written into the box, to be sent as the message.
    Runtime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    pub detail: String,
    pub action: SlashAction,
    pub group: Option<String>,
    pub hint: Option<String>,
    pub weighted: bool,
}

/// Every built-in command. Order matches Electron; `/auto` is last.
pub fn slash_commands() -> Vec<SlashCommand> {
    vec![
        cmd(
            "ask",
            "Reads and explains. Every write is refused.",
            SlashAction::Mode("ask"),
            false,
        ),
        cmd(
            "plan",
            "Answers with the steps it would take, and changes nothing.",
            SlashAction::Mode("plan"),
            false,
        ),
        cmd(
            "edit",
            "May edit files inside this folder.",
            SlashAction::Mode("accept-edits"),
            false,
        ),
        cmd(
            "approve",
            "Stops and asks before every command or file change.",
            SlashAction::Mode("approve-each"),
            false,
        ),
        cmd(
            "model",
            "Choose the runtime and model.",
            SlashAction::Route,
            false,
        ),
        cmd(
            "swarm",
            "Every run at its model maximum.",
            SlashAction::Swarm,
            false,
        ),
        cmd(
            "stop",
            "Stop the running mission.",
            SlashAction::Stop,
            false,
        ),
        cmd(
            "issue",
            "Start from an open GitHub issue in this folder.",
            SlashAction::Issues,
            false,
        ),
        cmd(
            "tune",
            "Try this teammate’s instructions three ways on your last ask, and keep the one that worked best.",
            SlashAction::Tune,
            false,
        ),
        cmd(
            "setup",
            "See what shapes this teammate’s turns: its instructions, files, memory, skills and connectors.",
            SlashAction::Setup,
            false,
        ),
        cmd(
            "auto",
            "Runs without asking, and may change anything on this machine.",
            SlashAction::Mode("auto"),
            true,
        ),
    ]
}

fn cmd(name: &str, detail: &str, action: SlashAction, weighted: bool) -> SlashCommand {
    SlashCommand {
        name: name.into(),
        detail: detail.into(),
        action,
        group: None,
        hint: None,
        weighted,
    }
}

/// What the person has typed, if it is a command being typed.
pub fn slash_query(text: &str) -> Option<String> {
    let re = crate::invariants::regex(r"^/([A-Za-z0-9:._/-]*)$");
    re.captures(text.trim()).map(|c| {
        c.get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default()
    })
}

pub struct AvailableInput<'a> {
    pub modes: &'a [&'a str],
    pub running: bool,
    pub can_swarm: bool,
    pub issues: bool,
    pub tune: bool,
    pub setup: bool,
    pub runtime_commands: &'a [SlashCommand],
}

/// The commands to offer, given what is actually possible right now.
pub fn available_commands(input: AvailableInput<'_>) -> Vec<SlashCommand> {
    let own: std::collections::HashSet<String> =
        slash_commands().into_iter().map(|c| c.name).collect();
    let theirs: Vec<SlashCommand> = input
        .runtime_commands
        .iter()
        .filter(|c| !own.contains(&c.name.to_ascii_lowercase()))
        .cloned()
        .collect();
    let mut out: Vec<SlashCommand> = slash_commands()
        .into_iter()
        .filter(|command| match &command.action {
            SlashAction::Mode(mode) => input.modes.iter().any(|m| m == mode),
            SlashAction::Stop => input.running,
            SlashAction::Swarm => input.can_swarm,
            SlashAction::Issues => input.issues,
            SlashAction::Tune => input.tune && !input.running,
            SlashAction::Setup => input.setup,
            _ => true,
        })
        .collect();
    if !input.running {
        out.extend(theirs);
    }
    out
}

/// A runtime's own commands as menu rows (0.426), under the runtime's name (`runtimeSlashCommands`). Choosing one
/// writes `/name ` into the box, where its arguments go, and the message is sent to the runtime as that command.
pub fn runtime_slash_commands(group: &str, rows: &[(String, String, String)]) -> Vec<SlashCommand> {
    rows.iter()
        .map(|(name, description, hint)| SlashCommand {
            name: name.clone(),
            detail: if description.is_empty() { format!("{group}'s own command.") } else { description.clone() },
            action: SlashAction::Runtime,
            group: Some(group.to_owned()),
            hint: (!hint.is_empty()).then(|| hint.clone()),
            weighted: false,
        })
        .collect()
}

/// Those whose name starts with what has been typed so far.
pub fn matching_commands(query: &str, available: &[SlashCommand]) -> Vec<SlashCommand> {
    available
        .iter()
        .filter(|c| c.name.to_ascii_lowercase().starts_with(query))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_query_only_whole_message() {
        assert_eq!(slash_query("/plan"), Some("plan".into()));
        assert_eq!(slash_query("run /plan on this"), None);
        assert_eq!(slash_query("/"), Some("".into()));
    }

    #[test]
    fn stop_only_when_running() {
        let idle = available_commands(AvailableInput {
            modes: &["ask", "plan", "accept-edits", "approve-each", "auto"],
            running: false,
            can_swarm: false,
            issues: false,
            tune: false,
            setup: true,
            runtime_commands: &[],
        });
        assert!(!idle.iter().any(|c| c.name == "stop"));
        let busy = available_commands(AvailableInput {
            modes: &["ask"],
            running: true,
            can_swarm: false,
            issues: false,
            tune: false,
            setup: true,
            runtime_commands: &[],
        });
        assert!(busy.iter().any(|c| c.name == "stop"));
    }

    #[test]
    fn matching_prefix() {
        let all = slash_commands();
        let matched = matching_commands("ap", &all);
        assert_eq!(
            matched.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["approve"]
        );
    }

    #[test]
    fn auto_is_last_and_weighted() {
        let all = slash_commands();
        assert_eq!(all.last().unwrap().name, "auto");
        assert!(all.last().unwrap().weighted);
    }
}
