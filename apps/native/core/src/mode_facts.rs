//! THE FIVE MODES, IN THE WORDS THE COMPOSER USES (renderer/status.ts MODE_FACTS, RUNTIME_CAPABILITIES, modeRunsOn,
//! modeUnavailableReason). Which agent can run which mode is Electron's measured table, and the command grid holds
//! this one to it: every grid case records whether Electron offers its mode.

/// One mode, its facts written once (status.ts `ModeFacts`). The registers differ on purpose -- a picker row, a receipt
/// line, a summary card and a second-person sentence are not the same writing; the facts they say are these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeFact {
    pub mode: &'static str,
    /// Title case, the way a control names it.
    pub name: &'static str,
    /// The chip's word: the slash command's own name for the mode.
    pub chip: &'static str,
    /// The clause a receipt appends to say what it may touch, where one adds anything.
    pub scope: Option<&'static str>,
    /// A short lowercase clause, for a summary card that leads with the name.
    pub short: &'static str,
    /// One full third-person sentence, for a picker row that has the room.
    pub consequence: &'static str,
    /// Said to the person, second person, where they are about to choose.
    pub sentence: &'static str,
}

/// In the order the composer offers them.
pub const MODE_FACTS: [ModeFact; 5] = [
    ModeFact {
        mode: "ask",
        name: "Ask",
        chip: "Ask",
        scope: Some("read-only"),
        short: "reads and explains, every write refused",
        consequence: "Reads and explains. It cannot change any file.",
        sentence: "In Ask mode nothing is changed: every write is refused.",
    },
    ModeFact {
        mode: "accept-edits",
        name: "Edit",
        chip: "Edit",
        scope: None,
        short: "may change files in this workspace",
        consequence: "May edit files inside this workspace folder, and nowhere else.",
        sentence: "Edit is on, so it may change files here; switch to Ask below to keep it read-only.",
    },
    ModeFact {
        mode: "plan",
        name: "Plan",
        chip: "Plan",
        scope: Some("read-only"),
        short: "answers with the steps it would take, changes nothing",
        consequence: "Answers with the steps it would take, and changes nothing.",
        sentence: "In Plan mode it answers with the steps it would take and changes nothing.",
    },
    ModeFact {
        mode: "approve-each",
        name: "Approve each action",
        chip: "Approve",
        scope: None,
        short: "asks before every command or change",
        consequence: "Stops and asks before every command or file change.",
        sentence: "Every change waits for your approval before it lands.",
    },
    ModeFact {
        mode: "auto",
        name: "Auto",
        chip: "Auto",
        scope: Some("whole machine"),
        short: "may edit anything on this machine",
        consequence: "Runs without asking and may change files anywhere on this machine, not only this folder.",
        sentence: "Auto is on: it runs without asking and may change files anywhere on this machine, not only in this folder.",
    },
];

/// modeFacts: the row for a mode; Ask's for anything unknown, so a surface always says something.
pub fn fact(mode: &str) -> ModeFact {
    MODE_FACTS.iter().copied().find(|f| f.mode == mode).unwrap_or(MODE_FACTS[0])
}

/// modeLabel: the mode in a receipt's words, lowercase mid-sentence, the scope clause after it where it adds anything;
/// "not set yet" for a teammate with none.
pub fn mode_label(mode: Option<&str>) -> String {
    let Some(mode) = mode else { return "not set yet".into() };
    let f = fact(mode);
    let name = f.name.to_lowercase();
    match f.scope {
        Some(scope) => format!("{name} \u{b7} {scope}"),
        None => name,
    }
}

/// modeSummary: the summary card's line, the name, then what it does.
pub fn mode_summary(mode: &str) -> String {
    let f = fact(mode);
    format!("{} \u{b7} {}", f.name, f.short)
}

/// modeSentence: said to the person on the surface where they are about to choose.
pub fn mode_sentence(mode: &str) -> &'static str {
    fact(mode).sentence
}

/// RUNTIME_CAPABILITIES: the modes each agent was measured running (Plan follows Ask).
fn modes_of(runtime: &str) -> &'static [&'static str] {
    match runtime {
        "codex" | "claude" | "opencode" | "copilot" => &["ask", "accept-edits", "auto", "approve-each"],
        "cursor" | "antigravity" | "muse" | "kilo" => &["ask", "accept-edits", "auto"],
        "acp" => &["approve-each"],
        // It only talks (runtime::apple).
        "apple" => &["ask"],
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

/// `Composer.tsx` `connectorsNote`: what the chosen mode does to connectors, per runtime.
pub fn connectors_note(runtime: &str, mode: &str, has_connectors: bool) -> Option<String> {
    if runtime == "cursor" && mode != "auto" {
        return Some("Your connectors work in this mode too: Locust allows each server you configured in this workspace's Cursor settings, since nobody is here to answer Cursor's per-call prompt. What this mode limits is this machine.".into());
    }
    if runtime != "claude" {
        if !has_connectors {
            return None;
        }
        return Some(format!(
            "Your connectors are signed in on Claude Code. A teammate on {} uses that program's own instead \u{2014} a different set, signed in separately.",
            crate::teammate_tooltip::short_runtime_name(runtime)
        ));
    }
    if mode == "ask" || mode == "plan" {
        return Some("Your connectors work in this mode, and each call asks you first, as Claude Code does. A connector acts on the service it reaches, not this machine.".into());
    }
    Some(if mode == "auto" {
        "Your connectors work here, and so does everything else on this machine.".into()
    } else {
        "Your connectors work in this mode too. This mode only limits what happens on this machine \u{2014} a connector acts on the service it reaches.".into()
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

    // --- connectors-are-not-the-mode.test.tsx

    const RESTRICTED: [&str; 4] = ["ask", "accept-edits", "plan", "approve-each"];
    const EVERY_MODE: [&str; 5] = ["ask", "accept-edits", "plan", "approve-each", "auto"];

    #[test]
    fn says_they_work_in_every_mode() {
        for mode in EVERY_MODE {
            assert!(connectors_note("claude", mode, false).unwrap().contains("Your connectors work"), "{mode}");
        }
    }

    #[test]
    fn says_the_one_thing_the_mode_cannot_tell_you_a_connector_is_not_this_machine() {
        for mode in RESTRICTED {
            if mode == "ask" || mode == "plan" { continue; }
            let note = connectors_note("claude", mode, false).unwrap();
            assert!(note.contains("only limits what happens on this machine"), "{mode}");
            assert!(note.contains("a connector acts on the service it reaches"), "{mode}");
        }
        for mode in ["ask", "plan"] {
            let note = connectors_note("claude", mode, false).unwrap();
            assert!(note.contains("each call asks you first"), "{mode}");
            assert!(note.contains("not this machine"), "{mode}");
        }
        assert!(connectors_note("claude", "auto", false).unwrap().contains("everything else on this machine"));
    }

    #[test]
    fn never_repeats_any_of_the_three_sentences_that_turned_out_to_be_wrong() {
        for mode in EVERY_MODE {
            let note = connectors_note("claude", mode, false).unwrap_or_default();
            assert!(!note.contains("own Claude Code settings"), "{mode}");
            assert!(!note.contains("available in every mode"), "{mode}");
            assert!(!note.contains("will be refused"), "{mode}");
            assert!(!note.contains("Only Auto"), "{mode}");
        }
    }

    #[test]
    fn says_nothing_on_a_runtime_whose_launcher_never_touches_mcp() {
        for runtime in ["codex", "opencode", "copilot", "antigravity"] {
            assert_eq!(connectors_note(runtime, "accept-edits", false), None, "{runtime}");
        }
    }

    #[test]
    fn says_where_they_are_signed_in_once_there_are_any() {
        let said = connectors_note("cursor", "auto", true).unwrap();
        assert!(said.contains("Claude Code"));
        assert!(said.contains("Cursor"));
    }

    #[test]
    fn does_not_claim_the_other_route_has_none_because_that_is_not_true() {
        let said = connectors_note("cursor", "auto", true).unwrap();
        for wrong in ["no connectors", "cannot use", "without connectors"] {
            assert!(!said.to_lowercase().contains(wrong));
        }
        assert!(said.contains("different set"));
    }

    #[test]
    fn tells_a_cursor_route_outside_auto_that_its_connectors_work_and_how() {
        for mode in ["accept-edits", "ask", "plan"] {
            let said = connectors_note("cursor", mode, false).unwrap();
            assert!(said.contains("Your connectors work in this mode too"));
            assert!(said.contains("Cursor settings"));
            assert!(!said.contains("refused"));
        }
        assert_eq!(connectors_note("cursor", "auto", false), None);
    }

    #[test]
    fn stays_quiet_when_this_machine_has_none_because_then_there_is_nothing_to_place() {
        assert_eq!(connectors_note("cursor", "auto", false), None);
        assert_eq!(connectors_note("codex", "ask", false), None);
    }

    #[test]
    fn leaves_the_claude_sentence_exactly_as_it_was() {
        assert_eq!(connectors_note("claude", "auto", true), connectors_note("claude", "auto", false));
        assert!(connectors_note("claude", "auto", true).unwrap().contains("work here"));
    }
}

#[cfg(test)]
#[path = "mode_says_one_thing_tests.rs"]
mod mode_says_one_thing_tests;
