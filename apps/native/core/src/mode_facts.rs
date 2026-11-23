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

/// How a runtime's declared capability row is backed (`status.ts` `RuntimeCapabilities.evidence`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityEvidence {
    Measured,
    Unproven,
}

/// One runtime's declared capabilities (`status.ts` `RuntimeCapabilities`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub runtime: &'static str,
    pub modes: &'static [&'static str],
    pub evidence: CapabilityEvidence,
    /// Platform-specific mode bars: `(platform, barred_modes)`.
    pub not_on: &'static [(&'static str, &'static [&'static str])],
}

/// RUNTIME_CAPABILITIES: the modes each agent was measured running (Plan follows Ask).
pub const RUNTIME_CAPABILITIES: &[RuntimeCapabilities] = &[
    RuntimeCapabilities {
        runtime: "codex",
        modes: &["ask", "accept-edits", "auto", "approve-each"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "claude",
        modes: &["ask", "accept-edits", "auto", "approve-each"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "cursor",
        modes: &["ask", "accept-edits", "auto"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "gemini",
        modes: &[],
        evidence: CapabilityEvidence::Unproven,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "opencode",
        modes: &["ask", "accept-edits", "auto", "approve-each"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "copilot",
        modes: &["ask", "accept-edits", "auto", "approve-each"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "antigravity",
        modes: &["ask", "accept-edits", "auto"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "muse",
        modes: &["ask", "accept-edits", "auto"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "kilo",
        modes: &["ask", "accept-edits", "auto"],
        evidence: CapabilityEvidence::Unproven,
        not_on: &[],
    },
    RuntimeCapabilities {
        runtime: "acp",
        modes: &["approve-each"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
    // It only talks (runtime::apple).
    RuntimeCapabilities {
        runtime: "apple",
        modes: &["ask"],
        evidence: CapabilityEvidence::Measured,
        not_on: &[],
    },
];

/// Look up a runtime's declared capability row.
pub fn runtime_capabilities(runtime: &str) -> Option<&'static RuntimeCapabilities> {
    RUNTIME_CAPABILITIES.iter().find(|entry| entry.runtime == runtime)
}

/// modeRunsOn with an explicit platform (`status.ts` `modeRunsOn`): Plan runs exactly where Ask runs.
pub fn runs_on_platform(mode: &str, runtime: &str, platform: Option<&str>) -> bool {
    if mode == "plan" {
        return runs_on_platform("ask", runtime, platform);
    }
    let Some(facts) = runtime_capabilities(runtime) else {
        return false;
    };
    if !facts.modes.contains(&mode) {
        return false;
    }
    if let Some(p) = platform {
        if facts.not_on.iter().any(|(plat, barred)| *plat == p && barred.contains(&mode)) {
            return false;
        }
    }
    true
}

/// modeRunsOn: Plan exactly where read-only containment is real (where Ask runs).
pub fn runs_on(mode: &str, runtime: &str) -> bool {
    runs_on_platform(mode, runtime, None)
}

/// modeUnavailableReason with an explicit platform (`status.ts` `modeUnavailableReason`).
pub fn unavailable_reason_on(mode: &str, runtime: &str, platform: Option<&str>) -> Option<String> {
    if runs_on_platform(mode, runtime, platform) {
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

/// modeUnavailableReason: why the menu greys a mode out for this agent, or None when it runs.
pub fn unavailable_reason(mode: &str, runtime: &str) -> Option<String> {
    unavailable_reason_on(mode, runtime, None)
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

    // --- a-runtime-declares-what-it-can-do.test.ts

    // case: answers exactly as the if-chain did, except where a claim was corrected on purpose
    #[test]
    fn answers_exactly_as_the_if_chain_did_except_where_a_claim_was_corrected_on_purpose() {
        let expected_by_runtime: &[(&str, &[&str])] = &[
            ("codex", &["ask", "accept-edits", "approve-each", "plan", "auto"]),
            ("claude", &["ask", "accept-edits", "approve-each", "plan", "auto"]),
            ("cursor", &["ask", "accept-edits", "plan", "auto"]),
            ("gemini", &[]),
            ("opencode", &["ask", "accept-edits", "approve-each", "plan", "auto"]),
            ("copilot", &["ask", "accept-edits", "approve-each", "plan", "auto"]),
            ("antigravity", &["ask", "plan", "accept-edits", "auto"]),
        ];
        for platform in ["win32", "darwin"] {
            for (runtime, want) in expected_by_runtime {
                let mut actual: Vec<&str> = EVERY_MODE
                    .iter()
                    .copied()
                    .filter(|mode| runs_on_platform(mode, runtime, Some(platform)))
                    .collect();
                let mut want_sorted = want.to_vec();
                actual.sort_unstable();
                want_sorted.sort_unstable();
                assert_eq!(actual, want_sorted, "{runtime} on {platform}");
            }
        }
    }

    // case: claims nothing for a runtime no mission can run under
    #[test]
    fn claims_nothing_for_a_runtime_no_mission_can_run_under() {
        assert_eq!(runtime_capabilities("gemini").unwrap().modes, &[] as &[&str]);
        for mode in EVERY_MODE {
            assert!(!runs_on_platform(mode, "gemini", Some("win32")), "gemini must not claim {mode}");
        }
    }

    // case: offers a runtime only the modes someone has actually run under it
    #[test]
    fn offers_a_runtime_only_the_modes_someone_has_actually_run_under_it() {
        let muse = runtime_capabilities("muse").unwrap();
        assert_eq!(muse.modes, &["ask", "accept-edits", "auto"]);
        assert_eq!(muse.evidence, CapabilityEvidence::Measured);
        assert!(runs_on_platform("plan", "muse", Some("win32")));
        assert!(runs_on_platform("auto", "muse", Some("win32")));
        assert!(!runs_on_platform("approve-each", "muse", Some("win32")));
    }

    // case: keeps plan available exactly where ask is, and never apart from it
    #[test]
    fn keeps_plan_available_exactly_where_ask_is_and_never_apart_from_it() {
        for platform in [Some("win32"), Some("darwin"), None] {
            for entry in RUNTIME_CAPABILITIES {
                assert_eq!(
                    runs_on_platform("plan", entry.runtime, platform),
                    runs_on_platform("ask", entry.runtime, platform),
                    "{} on {:?}",
                    entry.runtime,
                    platform
                );
            }
        }
    }

    // case: says how it knows, for every runtime
    #[test]
    fn says_how_it_knows_for_every_runtime() {
        for entry in RUNTIME_CAPABILITIES {
            assert!(
                matches!(entry.evidence, CapabilityEvidence::Measured | CapabilityEvidence::Unproven),
                "{} has no evidence",
                entry.runtime
            );
        }
        assert_eq!(runtime_capabilities("gemini").unwrap().evidence, CapabilityEvidence::Unproven);
    }

    // case: names the runtime the person chose when it refuses a mode
    #[test]
    fn names_the_runtime_the_person_chose_when_it_refuses_a_mode() {
        for entry in RUNTIME_CAPABILITIES {
            for mode in EVERY_MODE {
                let Some(reason) = unavailable_reason_on(mode, entry.runtime, Some("win32")) else {
                    continue;
                };
                if entry.runtime == "codex" {
                    continue;
                }
                let about_this_runtime = if mode == "approve-each" {
                    reason.replace("Claude Code, Codex CLI, OpenCode and Copilot CLI only.", "")
                } else {
                    reason.clone()
                };
                assert!(!about_this_runtime.contains("Codex CLI"), "{} / {mode}: {reason}", entry.runtime);
            }
        }
        assert!(
            unavailable_reason_on("approve-each", "muse", Some("win32"))
                .unwrap()
                .contains("Muse Code cannot stop and ask")
        );
    }

    // case: bars nothing by platform now: Cursor answers read-only everywhere (0.485)
    #[test]
    fn bars_nothing_by_platform_now_cursor_answers_read_only_everywhere_0_485() {
        for platform in [Some("win32"), Some("darwin"), Some("linux"), None] {
            assert!(runs_on_platform("ask", "cursor", platform));
        }
    }
}

#[cfg(test)]
#[path = "mode_says_one_thing_tests.rs"]
mod mode_says_one_thing_tests;
