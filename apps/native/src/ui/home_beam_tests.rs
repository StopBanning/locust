//! the-beams-are-mono-and-quiet.test.tsx, the title box's 2 cases: the beam round Home's cover machine, drawn headless
//! at 1215 wide. The stop button's 3 cases are not here (the composer's stop button is drawn in thread_view.rs).
use super::home_lockup_tests::{home, info, settle};
use super::*;

fn cover(ready: bool, reduced: bool) -> rune::Runtime<State> {
    let mut rt = home("full", reduced, (1215.0, 1600.0));
    if !ready {
        rt.update(|s, _| s.settings_panel.agents = None);
        settle(&mut rt);
        settle(&mut rt);
    }
    rt
}

fn beams(rt: &rune::Runtime<State>) -> Vec<rune::ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.contains("beam"))).collect()
}

// case: wears a beam round the machine, and round nothing else
#[test]
fn wears_a_beam_round_the_machine_and_round_nothing_else() {
    for ready in [false, true] {
        let rt = cover(ready, false);
        let wrappers: Vec<_> = beams(&rt).into_iter().filter(|e| e.key.as_deref() == Some("home-cover-beam")).collect();
        assert_eq!(wrappers.len(), 1, "one beam (ready {ready})");
        let (beam, machine) = (wrappers[0].rect, info(&rt, "home-cover-machine").rect);
        // Round the machine: its own box, 1.5px out on every side.
        for (side, (b, m)) in [(beam[0], machine[0]), (beam[1], machine[1])].into_iter().enumerate() {
            assert!((m - b - 1.5).abs() < 0.6, "edge {side}: beam {beam:?} machine {machine:?}");
        }
        assert!((beam[2] - machine[2] - 3.0).abs() < 1.1 && (beam[3] - machine[3] - 3.0).abs() < 1.1, "beam {beam:?} machine {machine:?}");
        // And nothing else on Home wears one.
        assert!(beams(&rt).iter().all(|e| e.key.as_deref().is_some_and(|k| k.starts_with("home-cover-beam"))), "{:?}", beams(&rt).iter().map(|e| e.key.clone()).collect::<Vec<_>>());
    }
}

// case: travels while the runtimes are being found, and goes out when the lockup lights
#[test]
fn travels_while_the_runtimes_are_being_found_and_goes_out_when_the_lockup_lights() {
    let finding = cover(false, false);
    let light = info(&finding, "home-cover-beam-light");
    assert!(light.loop_phase.is_some(), "it travels: the light turns on the renderer's loop");
    let lit = cover(true, false);
    assert!(lit.find("home-cover-beam-light").is_none(), "the agents answered: out");
    // Reduced motion: none, even while finding (Beam.tsx).
    assert!(cover(false, true).find("home-cover-beam-light").is_none());
}
