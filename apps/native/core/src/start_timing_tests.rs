//! Electron `start-timing.test.ts` (0.602): where the seconds before "started" went.
use super::*;

const T0: i64 = 1_000_000;
fn timing(phases: &[(Phase, i64)], first: Option<i64>) -> StartTiming {
    StartTiming { sent_at: T0, phase_at: phases.iter().map(|(p, ms)| (*p, T0 + ms)).collect(), first_event_at: first.map(|ms| T0 + ms) }
}

// case: names every phase that ran, in order, and the runtime boot last
#[test]
fn names_every_phase_that_ran_in_order_and_the_runtime_boot_last() {
    let t = timing(&[(Phase::Looking, 50), (Phase::Briefing, 150), (Phase::ReadingFolder, 1_550), (Phase::StartingRuntime, 2_350)], Some(6_250));
    assert_eq!(
        sentence(&t, "Codex").as_deref(),
        Some("Started in 6.3 s: looked for Codex 0.1 s, briefed 1.4 s, read the folder 0.8 s; Codex took 3.9 s to say it had started.")
    );
    // Marked out of order, they are still said in the start path's order.
    let shuffled = timing(&[(Phase::StartingRuntime, 2_350), (Phase::Briefing, 150), (Phase::ReadingFolder, 1_550), (Phase::Looking, 50)], Some(6_250));
    assert_eq!(sentence(&shuffled, "Codex"), sentence(&t, "Codex"));
}

// case: leaves out a phase that did not run (a read-only run reads no folder)
#[test]
fn leaves_out_a_phase_that_did_not_run_a_read_only_run_reads_no_folder() {
    let t = timing(&[(Phase::Looking, 100), (Phase::Briefing, 200), (Phase::StartingRuntime, 1_200)], Some(4_200));
    assert_eq!(sentence(&t, "Claude Code").as_deref(), Some("Started in 4.2 s: looked for Claude Code 0.1 s, briefed 1.0 s; Claude Code took 3.0 s to say it had started."));
    // Phases with no runtime start: the last runs to the first word.
    let no_spawn = timing(&[(Phase::Looking, 100), (Phase::Briefing, 200)], Some(1_200));
    assert_eq!(sentence(&no_spawn, "Codex").as_deref(), Some("Started in 1.2 s: looked for Codex 0.1 s, briefed 1.0 s."));
}

// case: says nothing before the runtime has said anything, and dates a start that marked no phase by its total
#[test]
fn says_nothing_before_the_runtime_has_said_anything_and_dates_a_start_that_marked_no_phase_by_its_total() {
    assert_eq!(sentence(&timing(&[], None), "Codex"), None);
    assert_eq!(sentence(&timing(&[], Some(2_000)), "Codex").as_deref(), Some("Started in 2.0 s."));
    // A clock that went backwards reads as nothing spent, never as a negative.
    assert_eq!(sentence(&timing(&[], Some(-500)), "Codex").as_deref(), Some("Started in 0.0 s."));
}

// case: is written as an info note from the host, after the run, with the next sequence
#[test]
fn is_written_as_an_info_note_from_the_host_after_the_run_with_the_next_sequence() {
    let t = timing(&[(Phase::StartingRuntime, 500)], Some(3_500));
    let n = note("run_1", "mission_1", "codex", 41, "2026-10-04T20:00:00.000Z", "Codex", &t).unwrap();
    assert_eq!(n["type"], "adapter.diagnostic");
    assert_eq!(n["sequence"], 41);
    assert_eq!(n["id"], "run_1:host:start-timing:41");
    assert_eq!((&n["runId"], &n["missionId"], &n["sourceAdapter"], &n["occurredAt"]), (&json!("run_1"), &json!("mission_1"), &json!("codex"), &json!("2026-10-04T20:00:00.000Z")));
    assert_eq!(n["payload"], json!({"level": "info", "code": "host.start-timing", "terminal": false, "message": "Started in 3.5 s: Codex took 3.0 s to say it had started.", "evidence": {"redacted": true}}));
    assert_eq!(note("run_1", "mission_1", "codex", 1, "2026-10-04T20:00:00.000Z", "Codex", &timing(&[], None)), None);
}

// The clock: a phase marked twice keeps its first time; only the first word counts.
#[test]
fn a_clock_keeps_the_first_mark_of_each_phase_and_the_first_word() {
    let clock = Clock::sent();
    clock.mark(Phase::Looking);
    let first = clock.timing().phase_at[0].1;
    std::thread::sleep(std::time::Duration::from_millis(5));
    clock.mark(Phase::Looking);
    clock.began();
    let word = clock.timing().first_event_at.unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    clock.began();
    let t = clock.timing();
    assert_eq!(t.phase_at, vec![(Phase::Looking, first)]);
    assert_eq!(t.first_event_at, Some(word));
    assert!(t.sent_at <= first && first <= word);
}
