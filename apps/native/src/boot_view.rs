//! Boot screen as a value — port of `renderer/src/bootView.ts`.
//! Discovery reuses PATH lookup from `runtime::discovery`.

use crate::runtime::discovery;

pub const SETTLE_DELAY_MS: u64 = 200;
pub const SETTLE_MS: u64 = 420;
pub const SETTLED_HOLD_MS: u64 = 350;
pub const DISSOLVE_MS: u64 = 260;
pub const STALLED_AFTER_MS: u64 = 6_000;

pub const LOOKING_AGENTS: &str = "looking for the AI agents on this machine";
pub const LOOKING_BOOT: &str = "looking for AI agents on this machine ";
pub const DISCOVERY_ERROR: &str =
    "Locust could not look for AI agents — nothing of yours was read";
pub const INTRO_WITH_FREE: &str =
    "Your AI agents, on your own accounts. OpenCode works without one.";
pub const INTRO: &str = "Your AI agents, on your own accounts.";
pub const USE_FREE: &str = "Use a free model";
pub const CONNECTED_ACCOUNTS: &str = "Connected accounts";
pub const SHOW_ALL: &str = "Show all";
pub const READY_TO_INSTALL: &str = "ready to install";
pub const SIGN_IN_REQUIRED: &str = "sign-in required";
pub const DID_NOT_ANSWER: &str = "did not answer";
pub const READY: &str = "ready";
pub const COVER_CLAIM: &str = "Autonomous teammates on your own machine";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BootPhase {
    #[default]
    Idle,
    Probing,
    Settling,
    Settled,
    Dissolving,
    Gone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootProbe {
    pub id: String,
    pub bin: String,
    pub product: String,
    pub at: u64,
    pub outcome: Option<&'static str>,
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootContext {
    pub version: String,
    pub platform: String,
    pub workspace: String,
    pub branch: Option<String>,
    pub clean: Option<bool>,
    pub ledger_path: String,
    pub ledger_ok: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BootState {
    pub phase: BootPhase,
    pub started_at: Option<u64>,
    pub context: Option<BootContext>,
    pub probes: Vec<BootProbe>,
    pub finished: Option<(u64, usize, usize)>, // at, ready, needs_you
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootRow {
    pub bin: String,
    pub product: String,
    pub started_at: u64,
    pub result: Option<String>,
    pub tone: &'static str,
    pub elapsed: String,
    pub stalled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootPre {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
    pub tone: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootView {
    pub phase: BootPhase,
    pub preamble: Vec<BootPre>,
    pub rows: Vec<BootRow>,
    pub progress: String,
    pub summary: String,
    pub all_answered: bool,
}

/// One discovery event (shared/ipc.ts DiscoveryEvent), as the boot screen folds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryEvent {
    Started { at: u64 },
    ProbeStarted { id: String, bin: String, product: String, at: u64 },
    ProbeFinished { id: String, at: u64, outcome: &'static str, version: Option<String> },
    Finished { at: u64, ready: usize, needs_you: usize },
}

/// applyDiscoveryEvent: fold one discovery event into the screen's state. `Started` resets the probes, because a second
/// sweep is a second log; the launch's context survives it (native sends no context event yet). Probes are keyed by id, not by the command name (those
/// differ for Cursor), and a repeat replaces rather than appends.
pub fn apply_discovery_event(state: BootState, event: DiscoveryEvent) -> BootState {
    match event {
        DiscoveryEvent::Started { at } => BootState { phase: BootPhase::Probing, started_at: Some(at), probes: Vec::new(), ..state },
        DiscoveryEvent::ProbeStarted { id, bin, product, at } => {
            let phase = if state.phase == BootPhase::Idle { BootPhase::Probing } else { state.phase };
            let mut probes: Vec<BootProbe> = state.probes.into_iter().filter(|probe| probe.id != id).collect();
            probes.push(BootProbe { id, bin, product, at, outcome: None, version: None });
            BootState { phase, probes, ..state }
        }
        DiscoveryEvent::ProbeFinished { id, outcome, version, .. } => {
            let probes = state
                .probes
                .into_iter()
                .map(|probe| {
                    if probe.id == id {
                        BootProbe { outcome: Some(outcome), version: version.clone().or(probe.version.clone()), ..probe }
                    } else {
                        probe
                    }
                })
                .collect();
            BootState { probes, ..state }
        }
        DiscoveryEvent::Finished { at, ready, needs_you } => BootState { finished: Some((at, ready, needs_you)), ..state },
    }
}

pub fn home_relative(path: &str) -> String {
    let separator = if path.contains('/') && !path.contains('\\') {
        '/'
    } else {
        '\\'
    };
    let parts: Vec<&str> = path.split(separator).collect();
    let users = parts.iter().position(|p| *p == "Users" || *p == "home");
    if let Some(users) = users
        && parts.len() > users + 2
    {
        let mut out = vec!["~"];
        out.extend(&parts[users + 2..]);
        return out.join(&separator.to_string());
    }
    path.to_owned()
}

fn result_text(outcome: &str, version: Option<&str>) -> String {
    match outcome {
        "missing" => READY_TO_INSTALL.into(),
        "needs-signin" => SIGN_IN_REQUIRED.into(),
        "error" => DID_NOT_ANSWER.into(),
        _ => match version {
            Some(v) => format!("ready · {v}"),
            None => READY.into(),
        },
    }
}

pub fn boot_view(state: &BootState, phase: BootPhase, now: u64) -> BootView {
    let mut preamble = Vec::new();
    if let Some(context) = &state.context {
        preamble.push(BootPre {
            key: "locust".into(),
            value: format!("{} · {}", context.version, context.platform),
            tag: None,
            tone: "lime",
        });
        preamble.push(BootPre {
            key: "workspace".into(),
            value: match &context.branch {
                Some(branch) => format!("{} · git {branch}", context.workspace),
                None => context.workspace.clone(),
            },
            tag: context.clean.map(|c| {
                if c {
                    "clean".into()
                } else {
                    "uncommitted".into()
                }
            }),
            tone: "plain",
        });
        preamble.push(BootPre {
            key: "ledger".into(),
            value: home_relative(&context.ledger_path),
            tag: Some(if context.ledger_ok {
                "ok".into()
            } else {
                "not writable".into()
            }),
            tone: if context.ledger_ok { "green" } else { "plain" },
        });
    }

    // The log lists what this build can use: a roadmap agent's probe result is not offered (integrationOf 'planned').
    let rows: Vec<BootRow> = state
        .probes
        .iter()
        .filter(|probe| locust_core::agents::integration_of(&probe.id) != locust_core::agents::Integration::Planned)
        .map(|probe| {
            let waited = now.saturating_sub(probe.at);
            BootRow {
                bin: probe.bin.clone(),
                product: probe.product.clone(),
                started_at: probe.at,
                result: probe
                    .outcome
                    .map(|o| result_text(o, probe.version.as_deref())),
                tone: match probe.outcome {
                    None => "pending",
                    Some("ready") => "ready",
                    Some("needs-signin") => "needs-signin",
                    Some("missing") => "missing",
                    _ => "error",
                },
                elapsed: format!("{:.1}s", waited as f64 / 1000.0),
                stalled: probe.outcome.is_none() && waited >= STALLED_AFTER_MS,
            }
        })
        .collect();

    if !rows.is_empty() {
        preamble.push(BootPre {
            key: "scanning".into(),
            value: "PATH".into(),
            tag: Some(format!("{} so far", rows.len())),
            tone: "plain",
        });
    }

    let answered = rows.iter().filter(|r| r.result.is_some()).count();
    let summary = match state.finished {
        Some((_, ready, needs_you)) => format!(
            "{ready} ready · {needs_you} needs you · nothing pooled, proxied, or sent anywhere you have not connected"
        ),
        None => String::new(),
    };
    let progress = if rows.is_empty() {
        LOOKING_BOOT.into()
    } else {
        format!("checking {answered} of {} AI agents ", rows.len())
    };

    BootView {
        phase,
        preamble,
        all_answered: !rows.is_empty() && answered == rows.len(),
        rows,
        progress,
        summary,
    }
}

pub fn phase_at(finished_at: u64, now: u64) -> BootPhase {
    let since = now.saturating_sub(finished_at);
    if since < SETTLE_DELAY_MS {
        BootPhase::Probing
    } else if since < SETTLE_DELAY_MS + SETTLE_MS {
        BootPhase::Settling
    } else if since < SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS {
        BootPhase::Settled
    } else if since < SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS + DISSOLVE_MS {
        BootPhase::Dissolving
    } else {
        BootPhase::Gone
    }
}

/// Known Locust runtimes probed via PATH (`discovery::find_named`).
pub const PROBE_RUNTIME: &[(&str, &str, &str)] = &[
    ("codex", "codex", "Codex"),
    ("claude", "claude", "Claude Code"),
    ("opencode", "opencode", "OpenCode"),
    ("cursor", "cursor-agent", "Cursor"),
    ("copilot", "copilot", "Copilot"),
    ("kilo", "kilo", "Kilo"),
    ("muse", "muse", "Muse"),
    ("antigravity", "agy", "Antigravity"),
];

/// Sweep installed runtimes using PATH discovery (no invented facts), told as the discovery events the screen folds.
pub fn path_discovery_sweep(now: u64) -> BootState {
    let mut state = apply_discovery_event(BootState::default(), DiscoveryEvent::Started { at: now });
    let mut ready = 0usize;
    let mut needs_you = 0usize;
    for (id, bin, product) in PROBE_RUNTIME {
        state = apply_discovery_event(
            state,
            DiscoveryEvent::ProbeStarted { id: (*id).into(), bin: (*bin).into(), product: (*product).into(), at: now },
        );
        let outcome = match discovery::find_named(bin) {
            Ok(_) => {
                ready += 1;
                "ready"
            }
            Err(_) => {
                needs_you += 1;
                "missing"
            }
        };
        state = apply_discovery_event(
            state,
            DiscoveryEvent::ProbeFinished { id: (*id).into(), at: now, outcome, version: None },
        );
    }
    apply_discovery_event(state, DiscoveryEvent::Finished { at: now, ready, needs_you })
}

pub fn deferred_others_sentence(installs_from_here: &[bool]) -> String {
    // status.ts deferredOthersSentence: derived from the rows it hides, so each claim holds for every one of them.
    let count = installs_from_here.len();
    let head = format!("{count} other{} Locust can drive", if count == 1 { "" } else { "s" });
    let from_here = installs_from_here.iter().filter(|here| **here).count();
    if from_here == count {
        format!("{head} — each installs from here, then signs in.")
    } else if from_here == 0 {
        format!("{head} — each installs from its own vendor.")
    } else {
        format!("{head} — some install from here, some from their own vendor.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the-bare-machine.test.ts: "says what it is doing when there is nothing to count".
    #[test]
    fn says_what_it_is_doing_when_there_is_nothing_to_count() {
        let said = boot_view(&BootState::default(), BootPhase::Probing, 0).progress;
        assert!(!said.contains("0 of 0"), "{said}");
        assert!(said.contains("looking for"), "{said}");
    }

    /// the-bare-machine.test.ts: "still counts once there is something to count".
    #[test]
    fn still_counts_once_there_is_something_to_count() {
        let mut probing = BootState { phase: BootPhase::Probing, ..BootState::default() };
        probing.probes.push(BootProbe { id: "codex".into(), bin: "codex".into(), product: "Codex CLI".into(), at: 0, outcome: None, version: None });
        let said = boot_view(&probing, BootPhase::Probing, 0).progress;
        assert!(said.contains("of 1"), "{said}");
    }

    #[test]
    fn progress_and_summary_words() {
        let mut state = BootState::default();
        let empty = boot_view(&state, BootPhase::Probing, 0);
        assert_eq!(empty.progress, LOOKING_BOOT);
        state.probes.push(BootProbe {
            id: "codex".into(),
            bin: "codex".into(),
            product: "Codex".into(),
            at: 0,
            outcome: Some("ready"),
            version: Some("1.2.3".into()),
        });
        state.probes.push(BootProbe {
            id: "claude".into(),
            bin: "claude".into(),
            product: "Claude Code".into(),
            at: 0,
            outcome: None,
            version: None,
        });
        state.finished = Some((100, 1, 1));
        let view = boot_view(&state, BootPhase::Probing, 100);
        assert_eq!(view.progress, "checking 1 of 2 AI agents ");
        assert_eq!(
            view.summary,
            "1 ready · 1 needs you · nothing pooled, proxied, or sent anywhere you have not connected"
        );
        assert_eq!(view.rows[0].result.as_deref(), Some("ready · 1.2.3"));
        assert_eq!(view.rows[1].result, None);
    }

    #[test]
    fn home_relative_and_phases() {
        assert_eq!(
            home_relative("/Users/colin/Library/Application Support/Locust"),
            "~/Library/Application Support/Locust"
        );
        assert_eq!(phase_at(0, 100), BootPhase::Probing);
        assert_eq!(phase_at(0, 250), BootPhase::Settling);
        assert_eq!(phase_at(0, 700), BootPhase::Settled);
        assert_eq!(phase_at(0, 1000), BootPhase::Dissolving);
        assert_eq!(phase_at(0, 1400), BootPhase::Gone);
    }

    #[test]
    fn result_words_exact() {
        assert_eq!(result_text("missing", None), READY_TO_INSTALL);
        assert_eq!(result_text("needs-signin", None), SIGN_IN_REQUIRED);
        assert_eq!(result_text("error", None), DID_NOT_ANSWER);
        assert_eq!(LOOKING_AGENTS, "looking for the AI agents on this machine");
        assert_eq!(DISCOVERY_ERROR, "Locust could not look for AI agents — nothing of yours was read");
        assert_eq!(INTRO_WITH_FREE, "Your AI agents, on your own accounts. OpenCode works without one.");
        assert_eq!(INTRO, "Your AI agents, on your own accounts.");
        assert_eq!(USE_FREE, "Use a free model");
        assert_eq!(CONNECTED_ACCOUNTS, "Connected accounts");
        assert_eq!(SHOW_ALL, "Show all");
    }

    #[test]
    fn path_sweep_uses_discovery() {
        let state = path_discovery_sweep(1_000);
        assert_eq!(state.probes.len(), PROBE_RUNTIME.len());
        assert!(state.finished.is_some());
        for probe in &state.probes {
            assert!(probe.outcome == Some("ready") || probe.outcome == Some("missing"));
        }
    }
}

#[cfg(test)]
#[path = "others_line_tests.rs"]
mod others_line_tests;

#[cfg(test)]
#[path = "the_boot_screen_tests.rs"]
mod the_boot_screen_tests;
