//! `apps/desktop/src/renderer/src/the-boot-screen.test.ts`: the boot screen is a view of events, never of a timer.
//! Every expected word and value below is Electron's (`renderer/src/bootView.ts` and the test itself).

use super::*;

fn started(at: u64) -> DiscoveryEvent {
    DiscoveryEvent::Started { at }
}

fn probe(id: &str, bin: &str, product: &str, at: u64) -> DiscoveryEvent {
    DiscoveryEvent::ProbeStarted { id: id.into(), bin: bin.into(), product: product.into(), at }
}

fn answered(id: &str, at: u64, outcome: &'static str, version: Option<&str>) -> DiscoveryEvent {
    DiscoveryEvent::ProbeFinished { id: id.into(), at, outcome, version: version.map(str::to_owned) }
}

fn fold(events: Vec<DiscoveryEvent>) -> BootState {
    events.into_iter().fold(BootState::default(), apply_discovery_event)
}

const BACKSLASH: &str = "\\";

#[test]
fn draws_one_row_per_runtime_from_the_moment_its_command_is_issued() {
    let state = fold(vec![started(0), probe("codex", "codex", "Codex CLI", 100), probe("claude", "claude", "Claude Code", 600)]);
    let view = boot_view(&state, BootPhase::Probing, 900);
    assert_eq!(view.rows.iter().map(|row| row.bin.as_str()).collect::<Vec<_>>(), ["codex", "claude"]);
    // Neither has answered, so neither has a result.
    assert!(view.rows.iter().all(|row| row.result.is_none()));
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
    assert!(boot_view(&state, BootPhase::Probing, 6_000 + 10).rows[0].stalled);
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
        answered("codex", 400, "ready", Some("0.153.0")),
        DiscoveryEvent::Finished { at: 500, ready: 1, needs_you: 2 },
    ]);
    let view = boot_view(&state, BootPhase::Settled, 900);
    assert_eq!(view.rows[0].result.as_deref(), Some("ready · 0.153.0"));
    assert!(view.summary.contains("1 ready · 2 needs you"), "{}", view.summary);
    assert!(view.summary.contains("nothing pooled, proxied, or sent anywhere you have not connected"), "{}", view.summary);
}

#[test]
fn does_not_claim_a_version_it_was_never_given() {
    let state = fold(vec![started(0), probe("opencode", "opencode", "OpenCode", 0), answered("opencode", 10, "ready", None)]);
    assert_eq!(boot_view(&state, BootPhase::Settled, 20).rows[0].result.as_deref(), Some("ready"));
}

#[test]
fn says_an_agent_not_added_yet_is_ready_to_install_never_not_installed() {
    let state = fold(vec![started(0), probe("muse", "muse", "Muse Code", 0), answered("muse", 10, "missing", None)]);
    let result = boot_view(&state, BootPhase::Settled, 20).rows[0].result.clone().unwrap_or_default();
    assert_eq!(result, "ready to install");
    assert!(!result.to_lowercase().contains("not installed"), "{result}");
}

#[test]
fn a_second_sweep_is_a_new_log_not_more_rows_under_the_old_one() {
    let state = fold(vec![
        started(0),
        probe("codex", "codex", "Codex CLI", 0),
        started(900),
        probe("codex", "codex", "Codex CLI", 900),
    ]);
    assert_eq!(boot_view(&state, BootPhase::Probing, 1_000).rows.len(), 1);
}

#[test]
fn waits_a_beat_before_settling_so_the_screen_does_not_flinch() {
    // SETTLE_DELAY_MS = 200 in bootView.ts.
    assert_eq!(phase_at(0, 200 - 1), BootPhase::Probing);
    assert_eq!(phase_at(0, 200), BootPhase::Settling);
}

#[test]
fn runs_settling_settled_dissolving_then_gone_in_that_order() {
    // bootView.ts: SETTLE_DELAY_MS 200, SETTLE_MS 420, SETTLED_HOLD_MS 350, DISSOLVE_MS 260.
    assert_eq!(phase_at(0, 200 + 420 - 1), BootPhase::Settling);
    assert_eq!(phase_at(0, 200 + 420), BootPhase::Settled);
    assert_eq!(phase_at(0, 200 + 420 + 350), BootPhase::Dissolving);
    assert_eq!(phase_at(0, 200 + 420 + 350 + 260), BootPhase::Gone);
}

#[test]
fn has_no_minimum_dwell() {
    let fast = fold(vec![
        started(0),
        probe("codex", "codex", "Codex CLI", 0),
        answered("codex", 200, "ready", None),
        DiscoveryEvent::Finished { at: 200, ready: 1, needs_you: 0 },
    ]);
    assert_eq!(fast.finished.map(|(at, _, _)| at), Some(200));
    // The whole sequence from a 200ms discovery is under two seconds.
    assert!(SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS + DISSOLVE_MS < 2_000);
}

#[test]
fn shortens_a_home_path_the_way_a_terminal_would() {
    let windows = ["C:", "Users", "<home>", "Documents", "locust", "mission-ledger"].join(BACKSLASH);
    assert_eq!(home_relative(&windows), ["~", "Documents", "locust", "mission-ledger"].join(BACKSLASH));
    assert_eq!(home_relative("/home/colin/.locust/missions"), "~/.locust/missions");
}

#[test]
fn leaves_a_path_it_does_not_recognise_exactly_as_it_is() {
    assert_eq!(home_relative("/var/lib/locust"), "/var/lib/locust");
    let drive = format!("D:{BACKSLASH}ledger");
    assert_eq!(home_relative(&drive), drive);
}

#[test]
fn finds_the_row_for_a_runtime_whose_binary_is_not_its_id() {
    let state = fold(vec![
        started(0),
        probe("cursor", "cursor-agent", "Cursor Agent", 0),
        answered("cursor", 900, "ready", Some("2026.09.10")),
    ]);
    let view = boot_view(&state, BootPhase::Settled, 1_000);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].result.as_deref(), Some("ready · 2026.09.10"));
    // And the row still shows the command, because that is what ran.
    assert_eq!(view.rows[0].bin, "cursor-agent");
    assert!(!view.rows[0].stalled);
}

#[test]
fn does_not_report_a_probe_result_for_a_runtime_this_build_cannot_run() {
    let state = fold(vec![
        started(0),
        probe("gemini", "gemini", "Gemini CLI", 0),
        answered("gemini", 100, "needs-signin", None),
        probe("codex", "codex", "Codex CLI", 100),
    ]);
    let view = boot_view(&state, BootPhase::Probing, 200);
    assert_eq!(view.rows.iter().map(|row| row.bin.as_str()).collect::<Vec<_>>(), ["codex"]);
}

#[test]
fn counts_only_the_runtimes_it_is_actually_showing() {
    let state = fold(vec![
        started(0),
        probe("gemini", "gemini", "Gemini CLI", 0),
        probe("codex", "codex", "Codex CLI", 0),
        answered("codex", 10, "ready", None),
    ]);
    assert_eq!(boot_view(&state, BootPhase::Probing, 20).progress, "checking 1 of 1 AI agents ");
}

#[test]
fn keeps_antigravity_which_is_checked_by_the_host_rather_than_by_a_cli() {
    let state = fold(vec![
        started(0),
        probe("antigravity", "antigravity", "Antigravity", 0),
        answered("antigravity", 50, "ready", Some("2.11.0")),
    ]);
    let products: Vec<_> = boot_view(&state, BootPhase::Settled, 60).rows.into_iter().map(|row| row.product).collect();
    assert_eq!(products, ["Antigravity"]);
}
