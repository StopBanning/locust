//! Carries apps/desktop/src/renderer/src/the-cover-counts-only-work.test.ts (0.623), the cases about the rule. Not
//! carried: "is what the app hands the cover, never the map itself" -- the Rust app has no Home cover that follows the
//! rest rule yet (Home's bots run on `bot_stage`, and `cover_watch` drives only Settings' face legend, which Electron
//! hands no work either), so nothing hands a cover work. `CoverActivity::work` is what that cover will be handed.
use super::*;
use std::collections::HashMap;

/// A run as the renderer's map holds it: its phase and its events.
#[derive(Clone)]
struct Run {
    phase: &'static str,
    events: Vec<usize>,
}
fn run(phase: &'static str, events: usize) -> Run {
    Run { phase, events: (0..events).collect() }
}
fn signature(runs: &HashMap<&str, Run>) -> String {
    work_signature(runs.iter().map(|(key, run)| (*key, run.phase, run.events.len())), is_stoppable)
}

#[test]
fn is_the_same_for_a_rebuilt_map_that_holds_the_same_runs() {
    let one = HashMap::from([("run_a", run("running", 3)), ("run_b", run("completed", 9))]);
    // Rebuilt from scratch, its runs copied and inserted the other way round.
    let mut copied: Vec<(&str, Run)> = one.iter().map(|(k, v)| (*k, Run { phase: v.phase, events: v.events.clone() })).collect();
    copied.reverse();
    let rebuilt: HashMap<&str, Run> = copied.into_iter().collect();
    assert_eq!(signature(&rebuilt), signature(&one));
    // So handing it over again is not work arriving: the cover still rests at 45 s.
    let mut cover = CoverActivity::new(0.0, true, false, false);
    assert!(cover.work(&signature(&one), 0.0));
    assert!(!cover.work(&signature(&rebuilt), 30.0));
    assert!(cover.away(45.0));
    // Many runs, many rebuilds: always the same.
    let many: HashMap<&str, Run> = ["r1", "r2", "r3", "r4", "r5", "r6"].into_iter().map(|k| (k, run("running", k.len()))).collect();
    for _ in 0..20 {
        let again: HashMap<&str, Run> = many.iter().map(|(k, v)| (*k, v.clone())).collect();
        assert_eq!(signature(&again), signature(&many));
    }
}

#[test]
fn changes_when_a_running_run_gets_an_event_starts_or_ends() {
    let before = signature(&HashMap::from([("run_a", run("running", 3))]));
    assert_ne!(signature(&HashMap::from([("run_a", run("running", 4))])), before);
    assert_ne!(signature(&HashMap::from([("run_a", run("running", 3)), ("run_b", run("starting", 0))])), before);
    assert_ne!(signature(&HashMap::from([("run_a", run("completed", 3))])), before);
    // Each of those is work arriving: it wakes a cover that had rested.
    let mut cover = CoverActivity::new(0.0, true, false, false);
    cover.work(&before, 0.0);
    assert!(cover.away(50.0));
    assert!(cover.work(&signature(&HashMap::from([("run_a", run("running", 4))])), 50.0));
    assert!(!cover.away(50.0));
    assert!(cover.away(95.0));
}

#[test]
fn is_empty_for_good_when_nothing_is_running() {
    let ended = HashMap::from([("run_a", run("completed", 12)), ("run_b", run("failed", 2)), ("run_c", run("cancelled", 1))]);
    assert_eq!(signature(&ended), "");
    assert_eq!(signature(&HashMap::new()), "");
    assert!(is_stoppable("cancelling") && !is_stoppable("completed"));
    // An empty signature handed over and over never keeps the cover awake.
    let mut cover = CoverActivity::new(0.0, true, false, false);
    for at in [10.0, 20.0, 30.0, 40.0, 44.0] {
        assert!(!cover.work(&signature(&ended), at));
    }
    assert!(cover.away(45.0));
}
