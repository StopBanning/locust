//! Electron `the-home-screen-is-the-cover.test.tsx`, "the three play their parts" (0.562), case for case.
use super::*;

#[test]
fn runs_the_worker_on_a_17_s_loop_and_the_thinker_on_a_13_s_one() {
    assert!((loop_seconds(&WORKER) - 17.0).abs() < 0.005);
    assert!((loop_seconds(&THINKER) - 13.0).abs() < 0.005);
}

#[test]
fn keeps_the_worker_and_the_thinker_in_different_faces_nearly_all_of_the_time() {
    let face = |b: &CoverBeat| format!("{}{:?}", b.eyes.map_or("rest".to_owned(), |e| e.concat()), b.phosphor);
    let (mut same, mut samples) = (0, 0);
    let mut t = 0.0;
    while t < 221.0 {
        let worker = beat_at(&WORKER, t + WORKER_OFFSET).0;
        let thinker = beat_at(&THINKER, t + THINKER_OFFSET).0;
        samples += 1;
        if face(&worker) == face(&thinker) {
            same += 1;
        }
        t += 0.25;
    }
    assert!((same as f64) / (samples as f64) < 0.12, "{same} of {samples}");
}

#[test]
fn shows_the_states_a_teammate_really_has_in_their_real_colours() {
    let done = WORKER.iter().find(|b| b.name == "done").unwrap();
    assert_eq!(done.eyes.map(|e| e.concat()).as_deref(), Some("^^"));
    assert_eq!(done.phosphor, Phosphor::Green);
    assert!(done.hop);
    let waiting = WORKER.iter().find(|b| b.waiting).unwrap();
    assert_eq!(waiting.phosphor, Phosphor::Amber);
    // The ghost floats: it never hops.
    assert!(!THINKER.iter().any(|b| b.hop));
}

#[test]
fn says_how_long_until_the_next_beat_so_the_cover_renders_only_at_a_change() {
    assert_eq!(beat_at(&WORKER, 0.0).0.name, "typing");
    assert_eq!(beat_at(&WORKER, 6.5).0.name, "done");
    assert!((beat_at(&WORKER, 6.0).2 - 0.5).abs() < 0.005);
    assert_eq!(beat_at(&WORKER, 17.0 + 6.5).0.name, "done");
    // And the cover's one wake is the nearer of the two changes: at 0 the worker types for 6.5 s, the thinker
    // (4.1 s into its loop) thinks for 1.4 s more.
    let (_, _, left) = cover_beats(0.0);
    assert!((left - 1.4).abs() < 0.005, "{left}");
}

#[test]
fn a_held_clock_keeps_its_place_and_picks_up_from_it() {
    let mut clock = CoverClock::default();
    clock.set_playing(true, 10.0);
    assert_eq!(clock.elapsed(13.0), 3.0);
    clock.set_playing(false, 13.0);
    assert_eq!(clock.elapsed(100.0), 3.0);
    clock.set_playing(true, 100.0);
    assert_eq!(clock.elapsed(101.5), 4.5);
}
