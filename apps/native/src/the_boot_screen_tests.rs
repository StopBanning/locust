//! the-boot-screen.test.ts (18 cases): the boot screen is a view of discovery events, never of a timer. Each case folds
//! the same events Electron's does through `apply_discovery_event` and reads `boot_view` / `phase_at` / `home_relative`.
//! Expected words and numbers are Electron's (bootView.ts and its test).

use super::*;

fn started(at: u64) -> DiscoveryEvent {
    DiscoveryEvent::Started { at }
}

fn probe(id: &str, bin: &str, product: &str, at: u64) -> DiscoveryEvent {
    DiscoveryEvent::ProbeStarted { id: id.into(), bin: bin.into(), product: product.into(), at }
}

fn answer(id: &str, at: u64, outcome: &'static str, version: Option<&str>) -> DiscoveryEvent {
    DiscoveryEvent::ProbeFinished { id: id.into(), at, outcome, version: version.map(str::to_owned) }
}

fn fold(events: Vec<DiscoveryEvent>) -> BootState {
    events.into_iter().fold(BootState::default(), apply_discovery_event)
}

fn bins(view: &BootView) -> Vec<&str> {
    view.rows.iter().map(|row| row.bin.as_str()).collect()
}

// what the screen is made of

#[test]
fn draws_one_row_per_runtime_from_the_moment_its_command_is_issued() {
    let state = fold(vec![started(0), probe("codex", "codex", "Codex CLI", 100), probe("claude", "claude", "Claude Code", 600)]);
    let view = boot_view(&state, BootPhase::Probing, 900);
    assert_eq!(bins(&view), ["codex", "claude"]);
    assert!(view.rows.iter().all(|row| row.result.is_none()), "neither has answered, so neither has a result");
    assert_eq!(view.progress, "checking 0 of 2 AI agents ");
}

#[test]
fn counts_elapsed_from_the_moment_the_command_was_issued() {
    let state = fold(vec![started(0), probe("codex", "codex", "Codex CLI", 100)]);
    assert_eq!(boot_view(&state, BootPhase::Probing, 1_600).rows[0].elapsed, "1.5s");
}

#[test]
fn stops_counting_and_says_so_once_a_probe_has_taken_too_long() {
    let state = fold(vec![started(0), probe("codex", "codex", "Codex CLI", 0)]);
    assert!(boot_view(&state, BootPhase::Probing, STALLED_AFTER_MS + 10).rows[0].stalled);
    assert!(!boot_view(&state, BootPhase::Probing, 500).rows[0].stalled);
}

#[test]
fn counts_the_runtimes_it_shows_not_every_probe_that_ran() {
    let state = fold(vec![
        started(0),
        probe("codex", "codex", "Codex CLI", 0),
        probe("gemini", "gemini", "Gemini CLI", 0),
        probe("omniroute", "omniroute", "OmniRoute", 0),
    ]);
    let view = boot_view(&state, BootPhase::Probing, 100);
    assert_eq!(view.rows.len(), 1);
    let scanning = view.preamble.iter().find(|line| line.key == "scanning").expect("a scanning line");
    assert_eq!(scanning.tag.as_deref(), Some("1 so far"));
}

#[test]
fn never_invents_a_summary_before_discovery_has_finished() {
    let state = fold(vec![started(0), probe("codex", "codex", "Codex CLI", 0)]);
    assert_eq!(boot_view(&state, BootPhase::Probing, 500).summary, "");
}

#[test]
fn says_what_was_found_once_there_is_something_to_say() {
    let state = fold(vec![
        started(0),
        probe("codex", "codex", "Codex CLI", 0),
        answer("codex", 400, "ready", Some("0.153.0")),
        DiscoveryEvent::Finished { at: 500, ready: 1, needs_you: 2 },
    ]);
    let view = boot_view(&state, BootPhase::Settled, 900);
    assert_eq!(view.rows[0].result.as_deref(), Some("ready · 0.153.0"));
    assert!(view.summary.contains("1 ready · 2 needs you"), "{}", view.summary);
    assert!(view.summary.contains("nothing pooled, proxied, or sent anywhere you have not connected"), "{}", view.summary);
}

#[test]
fn does_not_claim_a_version_it_was_never_given() {
    let state = fold(vec![started(0), probe("opencode", "opencode", "OpenCode", 0), answer("opencode", 10, "ready", None)]);
    assert_eq!(boot_view(&state, BootPhase::Settled, 20).rows[0].result.as_deref(), Some("ready"));
}

#[test]
fn says_an_agent_not_added_yet_is_ready_to_install_never_not_installed() {
    let state = fold(vec![started(0), probe("muse", "muse", "Muse Code", 0), answer("muse", 10, "missing", None)]);
    let result = boot_view(&state, BootPhase::Settled, 20).rows[0].result.clone().expect("a result");
    assert_eq!(result, "ready to install");
    assert!(!result.to_lowercase().contains("not installed"), "{result}");
}

#[test]
fn a_second_sweep_is_a_new_log_not_more_rows_under_the_old_one() {
    let state = fold(vec![started(0), probe("codex", "codex", "Codex CLI", 0), started(900), probe("codex", "codex", "Codex CLI", 900)]);
    assert_eq!(boot_view(&state, BootPhase::Probing, 1_000).rows.len(), 1);
    // `started` resets the probes (applyDiscoveryEvent): the new log is empty until its own commands are issued.
    let restarted = fold(vec![started(0), probe("claude", "claude", "Claude Code", 0), started(900)]);
    assert!(boot_view(&restarted, BootPhase::Probing, 1_000).rows.is_empty());
    assert_eq!(restarted.started_at, Some(900));
}

// the phases

#[test]
fn waits_a_beat_before_settling_so_the_screen_does_not_flinch() {
    assert_eq!(phase_at(0, SETTLE_DELAY_MS - 1), BootPhase::Probing);
    assert_eq!(phase_at(0, SETTLE_DELAY_MS), BootPhase::Settling);
}

#[test]
fn runs_settling_settled_dissolving_then_gone_in_that_order() {
    assert_eq!(phase_at(0, SETTLE_DELAY_MS + SETTLE_MS - 1), BootPhase::Settling);
    assert_eq!(phase_at(0, SETTLE_DELAY_MS + SETTLE_MS), BootPhase::Settled);
    assert_eq!(phase_at(0, SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS), BootPhase::Dissolving);
    assert_eq!(phase_at(0, SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS + DISSOLVE_MS), BootPhase::Gone);
}

#[test]
fn has_no_minimum_dwell() {
    let fast = fold(vec![
        started(0),
        probe("codex", "codex", "Codex CLI", 0),
        answer("codex", 200, "ready", None),
        DiscoveryEvent::Finished { at: 200, ready: 1, needs_you: 0 },
    ]);
    assert_eq!(fast.finished.map(|(at, _, _)| at), Some(200));
    // The whole sequence from a 200ms discovery is under two seconds, and Electron's own lengths.
    assert_eq!((SETTLE_DELAY_MS, SETTLE_MS, SETTLED_HOLD_MS, DISSOLVE_MS, STALLED_AFTER_MS), (200, 420, 350, 260, 6_000));
    assert!(SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS + DISSOLVE_MS < 2_000);
}

// a path a person can read

#[test]
fn shortens_a_home_path_the_way_a_terminal_would() {
    assert_eq!(home_relative(r"C:\Users\<home>\Documents\locust\mission-ledger"), r"~\Documents\locust\mission-ledger");
    assert_eq!(home_relative("/home/colin/.locust/missions"), "~/.locust/missions");
}

#[test]
fn leaves_a_path_it_does_not_recognise_exactly_as_it_is() {
    assert_eq!(home_relative("/var/lib/locust"), "/var/lib/locust");
    assert_eq!(home_relative(r"D:\ledger"), r"D:\ledger");
}

// the two things Colin caught on a real launch

#[test]
fn finds_the_row_for_a_runtime_whose_binary_is_not_its_id() {
    let state = fold(vec![
        started(0),
        probe("cursor", "cursor-agent", "Cursor Agent", 0),
        answer("cursor", 900, "ready", Some("2026.09.10")),
    ]);
    let view = boot_view(&state, BootPhase::Settled, 1_000);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].result.as_deref(), Some("ready · 2026.09.10"));
    assert_eq!(view.rows[0].bin, "cursor-agent", "the row still shows the command, because that is what ran");
    assert!(!view.rows[0].stalled);
}

#[test]
fn does_not_report_a_probe_result_for_a_runtime_this_build_cannot_run() {
    let state = fold(vec![
        started(0),
        probe("gemini", "gemini", "Gemini CLI", 0),
        answer("gemini", 100, "needs-signin", None),
        probe("codex", "codex", "Codex CLI", 100),
    ]);
    assert_eq!(bins(&boot_view(&state, BootPhase::Probing, 200)), ["codex"]);
}

#[test]
fn counts_only_the_runtimes_it_is_actually_showing() {
    let state = fold(vec![
        started(0),
        probe("gemini", "gemini", "Gemini CLI", 0),
        probe("codex", "codex", "Codex CLI", 0),
        answer("codex", 10, "ready", None),
    ]);
    assert_eq!(boot_view(&state, BootPhase::Probing, 20).progress, "checking 1 of 1 AI agents ");
}

#[test]
fn keeps_antigravity_which_is_checked_by_the_host_rather_than_by_a_cli() {
    let state = fold(vec![
        started(0),
        probe("antigravity", "antigravity", "Antigravity", 0),
        answer("antigravity", 50, "ready", Some("2.11.0")),
    ]);
    let view = boot_view(&state, BootPhase::Settled, 60);
    assert_eq!(view.rows.iter().map(|row| row.product.as_str()).collect::<Vec<_>>(), ["Antigravity"]);
}

/// The window's own sweep (home_panel.rs calls `path_discovery_sweep`) is told as these events: every runtime it
/// probes is one row, each answered, and the sweep is finished.
#[test]
fn the_window_sweep_is_folded_from_discovery_events() {
    let state = path_discovery_sweep(1_000);
    let view = boot_view(&state, BootPhase::Settled, 1_000);
    assert_eq!(view.rows.len(), PROBE_RUNTIME.len());
    assert_eq!(state.started_at, Some(1_000));
    assert!(view.all_answered);
    let (_, ready, needs_you) = state.finished.expect("finished");
    assert_eq!(ready + needs_you, PROBE_RUNTIME.len());
}

#[test]
fn a_probe_belongs_to_its_runtime_by_id_not_by_the_command_it_runs() {
    // bootView.ts: "Keyed by ID, not by the command name: those differ for Cursor ... A repeat replaces rather than appends."
    let state = fold(vec![started(0), probe("cursor", "agent", "Cursor", 10), probe("other", "agent", "Other", 20)]);
    assert_eq!(state.probes.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["cursor", "other"], "one command name, two runtimes, two rows");
    let state = fold(vec![started(0), probe("cursor", "agent", "Cursor", 10), probe("cursor", "cursor-agent", "Cursor", 20)]);
    assert_eq!(state.probes.len(), 1, "a repeat of an id replaces its row");
    assert_eq!((state.probes[0].bin.as_str(), state.probes[0].at), ("cursor-agent", 20));
}

#[test]
fn a_later_answer_with_no_version_keeps_the_version_an_earlier_one_gave() {
    // bootView.ts probe.finished: `...(event.version === undefined ? {} : { version: event.version })`.
    let state = fold(vec![
        started(0),
        probe("codex", "codex", "Codex", 1),
        answer("codex", 2, "ready", Some("0.153.0")),
        answer("codex", 3, "ready", None),
    ]);
    assert_eq!(state.probes[0].version.as_deref(), Some("0.153.0"));
    assert_eq!(state.probes[0].outcome, Some("ready"));
}
