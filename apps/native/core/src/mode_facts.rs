//! THE FIVE MODES, IN THE WORDS THE COMPOSER USES (renderer/status.ts MODE_FACTS, RUNTIME_CAPABILITIES, modeRunsOn,
//! modeUnavailableReason). Which agent can run which mode is Electron's measured table, and the command grid holds
//! this one to it: every grid case records whether Electron offers its mode.

/// One mode as the menu shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeFact {
    pub mode: &'static str,
    pub name: &'static str,
    pub chip: &'static str,
    pub consequence: &'static str,
}

/// In the order the composer offers them.
pub const MODE_FACTS: [ModeFact; 5] = [
    ModeFact { mode: "ask", name: "Ask", chip: "Ask", consequence: "Reads and explains. It cannot change any file." },
    ModeFact { mode: "accept-edits", name: "Edit", chip: "Edit", consequence: "May edit files inside this workspace folder, and nowhere else." },
    ModeFact { mode: "plan", name: "Plan", chip: "Plan", consequence: "Answers with the steps it would take, and changes nothing." },
    ModeFact { mode: "approve-each", name: "Approve each action", chip: "Approve", consequence: "Stops and asks before every command or file change." },
    ModeFact {
        mode: "auto",
        name: "Auto",
        chip: "Auto",
        consequence: "Runs without asking and may change files anywhere on this machine, not only this folder.",
    },
];

/// modeFacts: the row for a mode; Ask's for anything unknown, so a surface always says something.
pub fn fact(mode: &str) -> ModeFact {
    MODE_FACTS.iter().copied().find(|f| f.mode == mode).unwrap_or(MODE_FACTS[0])
}

/// RUNTIME_CAPABILITIES: the modes each agent was measured running (Plan follows Ask).
fn modes_of(runtime: &str) -> &'static [&'static str] {
    match runtime {
        "codex" | "claude" | "opencode" | "copilot" => &["ask", "accept-edits", "auto", "approve-each"],
        "cursor" | "antigravity" | "muse" | "kilo" => &["ask", "accept-edits", "auto"],
        "acp" => &["approve-each"],
        _ => &[],
    }
}

/// modeRunsOn: Plan exactly where read-only containment is real (where Ask runs).
pub fn runs_on(mode: &str, runtime: &str) -> bool {
    if mode == "plan" {
        return runs_on("ask", runtime);
    }
    modes_of(runtime).contains(&mode)
}

/// modeUnavailableReason: why the menu greys a mode out for this agent, or None when it runs.
pub fn unavailable_reason(mode: &str, runtime: &str) -> Option<String> {
    if runs_on(mode, runtime) {
        return None;
    }
    let label = crate::carry_on::display_name(runtime);
    Some(match mode {
        "approve-each" => format!("Claude Code, Codex CLI, OpenCode and Copilot CLI only. {label} cannot stop and ask yet."),
        "auto" => format!("{label} runs its own agent under its own permissions; Locust has no handle to widen."),
        _ if runtime == "antigravity" => "Antigravity CLI runs this mode; without it, Antigravity runs only in Edit.".into(),
        _ => "Cursor Agent cannot be kept read-only on Windows: that needs macOS or Linux, and its plan mode alone does not stop it editing files.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_grid_case_offers_its_mode_as_electron_does() {
        let grid: serde_json::Value =
            serde_json::from_str(include_str!("../../parity/commands.json")).expect("the command grid");
        let cases = grid["cases"].as_array().unwrap();
        assert!(cases.len() >= 300);
        for case in cases {
            let (runtime, mode) = (case["runtime"].as_str().unwrap(), case["mode"].as_str().unwrap());
            assert_eq!(runs_on(mode, runtime), case["offered"].as_bool().unwrap(), "{}", case["id"]);
        }
    }

    #[test]
    fn the_menu_says_what_electron_says() {
        assert_eq!(MODE_FACTS.map(|f| f.chip), ["Ask", "Edit", "Plan", "Approve", "Auto"]);
        assert_eq!(fact("approve-each").name, "Approve each action");
        assert_eq!(fact("nonsense").chip, "Ask");
        assert_eq!(unavailable_reason("ask", "codex"), None);
        assert_eq!(
            unavailable_reason("approve-each", "cursor").as_deref(),
            Some("Claude Code, Codex CLI, OpenCode and Copilot CLI only. Cursor Agent cannot stop and ask yet.")
        );
        assert!(unavailable_reason("ask", "acp").is_some());
    }
}
