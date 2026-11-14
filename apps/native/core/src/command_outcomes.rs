//! What ran and what came back. Names are facts, never a grade of the work.
use crate::ledger::{Item, Status};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandsRun {
    pub ran: usize,
    pub non_zero: usize,
    pub unsettled: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    pub text: String,
    pub amber: bool,
}
pub fn commands_run(items: &[Item], finished: bool) -> CommandsRun {
    let mut run = CommandsRun::default();
    for i in items {
        if let Item::Tool {
            name,
            status,
            detail,
            ..
        } = i
            && is_shell(name)
            && !crate::activity_summary::is_edit(i)
            && !matches!(detail.runtime_status.as_str(), "refused" | "declined")
        {
            run.ran += 1;
            if *status == Status::Failed || detail.exit_code.is_some_and(|c| c != 0) {
                run.non_zero += 1;
            }
            if finished && *status == Status::Running {
                run.unsettled += 1;
            }
        }
    }
    run
}
pub fn is_shell(name: &str) -> bool {
    crate::mission_export::is_shell_tool(name, None)
        || matches!(
            name.to_lowercase().as_str(),
            "exec_command" | "commandexecution" | "command_execution"
        )
}
fn count(n: usize) -> String {
    format!("ran {n} command{}", if n == 1 { "" } else { "s" })
}
pub fn command_head(name: &str) -> String {
    let text = crate::shell_words::shell_command_text(name);
    let mut line = text.lines().next().unwrap_or("").trim();
    let hop = crate::invariants::regex(
        r#"(?i)^(?:cd|pushd|chdir|set-location|sl)\s+(?:"[^"]*"|'[^']*'|\S+)\s*(?:&&|;)\s*"#,
    );
    while let Some(m) = hop.find(line) {
        line = &line[m.end()..];
    }
    line.split_whitespace().next().unwrap_or("").into()
}
pub fn command_list(names: &[String], ran: usize) -> Option<String> {
    let mut heads = Vec::new();
    for name in names {
        let head = command_head(name);
        if !head.is_empty() && head.chars().count() <= 24 && !heads.contains(&head) {
            heads.push(head);
        }
    }
    if heads.is_empty() {
        return None;
    }
    let kinds = if heads.len() > 2 {
        format!("{}, {} and {} more", heads[0], heads[1], heads.len() - 2)
    } else {
        heads.join(" and ")
    };
    Some(if ran == heads.len() && heads.len() <= 2 {
        format!("ran {kinds}")
    } else {
        format!("{}: {kinds}", count(ran))
    })
}
pub fn commands_run_text(run: CommandsRun, names: &[String]) -> Option<Words> {
    if run.ran == 0 {
        return None;
    }
    let (text, amber) = if run.non_zero > 0 {
        (
            format!(
                "{} · {} exited non-zero",
                count(run.ran),
                if run.ran == 1 {
                    "it".into()
                } else {
                    run.non_zero.to_string()
                }
            ),
            true,
        )
    } else if run.unsettled > 0 {
        (
            format!(
                "{} · {} did not report",
                count(run.ran),
                if run.ran == 1 {
                    "it".into()
                } else {
                    run.unsettled.to_string()
                }
            ),
            true,
        )
    } else {
        (
            command_list(names, run.ran).unwrap_or_else(|| count(run.ran)),
            false,
        )
    };
    Some(Words { text, amber })
}
#[cfg(test)]
#[path = "command_outcomes_tests.rs"]
mod tests;
