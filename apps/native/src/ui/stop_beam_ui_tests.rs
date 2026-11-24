//! `the-beams-are-mono-and-quiet.test.tsx`, the stop button's 3 cases, through the composer (`thread_view` and
//! `crate::view`): while a turn runs, the Stop button wears a quiet mono beam stepping at 59 steps per 1.96 s (~30.1 a
//! second, the faces' rate); when nothing runs (or typing turns the button into Queue), no beam and no repaints; under
//! reduced motion, a calm still frame and no repaints.

use super::*;

fn running_state(dark: bool) -> State {
    let mut s = State::default();
    ledger_live_scene(&mut s);
    s.demo = false;
    s.appearance = Some(dark);
    s.own_turn = Some(OwnTurn {
        conversation: "synthetic-ledger".into(),
        tip: Some("synthetic-ledger".into()),
        before: None,
    });
    s
}

fn idle_state(dark: bool) -> State {
    let mut s = State::default();
    ledger_transcript_scene(&mut s);
    s.demo = false;
    s.appearance = Some(dark);
    for mate in &mut s.teammates {
        mate.busy = false;
    }
    for row in &mut s.recent {
        row.state = "completed".into();
    }
    s
}

fn thread_runtime(state: State, reduced_motion: bool) -> Box<rune::Runtime<State>> {
    let dark = state.appearance.unwrap_or(true);
    let mut rt = Box::new(rune::Runtime::new(
        state,
        |s, cx| {
            s.reduced_motion.set(cx.is_reduced_motion());
            thread_view(s, &Palette::new(s.appearance.unwrap_or(true)))
        },
        (1200.0, 900.0),
        rune::Env::new(dark, 1.0),
    ));
    rt.set_system_reduced_motion(reduced_motion);
    rt.frame(0.0);
    rt
}

fn window_runtime(state: State, reduced_motion: bool) -> Box<rune::Runtime<State>> {
    let dark = state.appearance.unwrap_or(true);
    let mut rt = Box::new(rune::Runtime::new(state, crate::view, (1280.0, 900.0), rune::Env::new(dark, 1.0)));
    rt.set_system_reduced_motion(reduced_motion);
    rt.frame(0.0);
    rt
}

fn wedge_opacities(rt: &rune::Runtime<State>) -> Vec<f32> {
    let all = rt.elements();
    (0..59)
        .map(|k| {
            let key = format!("stop-beam-wedge-{k}");
            all.iter()
                .find(|e| e.key.as_deref() == Some(key.as_str()) && !e.exiting)
                .unwrap_or_else(|| panic!("missing {key}"))
                .opacity
        })
        .collect()
}

fn peak_wedge(opacities: &[f32]) -> (usize, f32) {
    opacities
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

fn stop_beam_elements(rt: &rune::Runtime<State>) -> Vec<rune::ElementInfo> {
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("stop-beam")))
        .collect()
}

// case: wears the beam while a run goes
#[test]
fn wears_the_beam_while_a_run_goes() {
    for dark in [true, false] {
        let rt = window_runtime(running_state(dark), false);
        let all = rt.elements();
        let beam_idx = all
            .iter()
            .position(|e| e.key.as_deref() == Some("stop-beam") && !e.exiting)
            .expect("stop-beam wrapper while a run goes");
        let send_idx = all
            .iter()
            .position(|e| e.key.as_deref() == Some("send") && !e.exiting)
            .expect("stop button");
        assert!(send_idx > beam_idx, "stop-beam wraps the stop button");
        assert_eq!(all[send_idx].parent, Some(beam_idx));
        assert_eq!(all[send_idx].label.as_deref(), Some("Stop"));
        assert!(all.iter().any(|e| e.key.as_deref() == Some("stop-square") && !e.exiting));

        // The wrapper keeps the Stop button's 40×40 seat; the ring sits 1.5 px outside on every side.
        let beam = all[beam_idx].rect;
        let send = all[send_idx].rect;
        assert_eq!((beam[2], beam[3]), (40.0, 40.0));
        assert_eq!((send[2], send[3]), (40.0, 40.0));
        let light = all
            .iter()
            .find(|e| e.key.as_deref() == Some("stop-beam-light") && !e.exiting)
            .expect("stop-beam-light");
        assert!((send[0] - light.rect[0] - 1.5).abs() < 0.6, "left edge: light {:?} send {:?}", light.rect, send);
        assert!((send[1] - light.rect[1] - 1.5).abs() < 0.6, "top edge: light {:?} send {:?}", light.rect, send);
        assert!((light.rect[2] - 43.0).abs() < 0.6 && (light.rect[3] - 43.0).abs() < 0.6, "ring {:?}", light.rect);

        // 59 active mono wedges with a single bright conic peak at step 0.
        let wedges: Vec<_> = all
            .iter()
            .filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("stop-beam-wedge-")))
            .collect();
        assert_eq!(wedges.len(), 59);
        assert!(wedges.iter().all(|w| w.loop_phase.is_some()), "every wedge rides the frame clock");
        let opacities = wedge_opacities(&rt);
        let (peak, peak_alpha) = peak_wedge(&opacities);
        assert_eq!(peak, 41);
        assert!(peak_alpha > 0.70 && peak_alpha <= 0.75, "peak alpha {peak_alpha}");
        assert_eq!(opacities.iter().filter(|&&a| a > 0.0).count(), 14);
    }
}

fn assert_idle_has_no_beam(rt: &mut rune::Runtime<State>) {
    assert!(stop_beam_elements(rt).is_empty(), "no stop-beam when idle");
    let send = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("send") && !e.exiting)
        .expect("send button");
    assert_eq!(send.label.as_deref(), Some("Send"));
    assert!(!rt.frame(0.2), "idle composer must not request continuous frames");
    assert_eq!(rt.wake_deadline(), None, "idle composer must request no repaint");
}

// case: has no beam when there is nothing to stop
#[test]
fn has_no_beam_when_there_is_nothing_to_stop() {
    // Idle conversation through the full window and through thread_view: no beam, no repaints requested.
    assert_idle_has_no_beam(&mut window_runtime(idle_state(true), false));
    assert_idle_has_no_beam(&mut thread_runtime(idle_state(true), false));

    // Typing a follow-up while a run goes turns the button into Queue, so Stop and its beam disappear.
    let mut queueing = running_state(true);
    queueing.draft.replace_text("Follow up next");
    let rt = thread_runtime(queueing, false);
    assert!(stop_beam_elements(&rt).is_empty(), "queue button wears no stop-beam");
    let send = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("send") && !e.exiting)
        .expect("queue button");
    assert_eq!(send.label.as_deref(), Some("Queue"));
    drop(rt);

    // A `!` own command turns the button into Send, so no stop-beam.
    let mut bang = running_state(true);
    bang.draft.replace_text("!echo hi");
    let rt = thread_runtime(bang, false);
    assert!(stop_beam_elements(&rt).is_empty(), "bang command wears no stop-beam");
}

// case: travels at the faces' rate, not the screen's (0.698)
#[test]
fn travels_at_the_faces_rate_not_the_screens_0_698() {
    let mut rt = thread_runtime(running_state(true), false);

    // Cycle wakes at 1.96 / 59 ≈ 0.03322 s (~30.1 Hz, BOT_FRAMES_PER_SECOND), not 60 or 120 Hz.
    let wake = rt.wake_deadline().expect("active stop-beam wakes at the faces' rate");
    assert!(
        (wake - 1.96 / 59.0).abs() < 1e-5,
        "expected wake at 1.96/59 s (~0.03322 s), got {wake}"
    );
    assert!(wake > 1.0 / 40.0, "must not wake at 60 Hz or 120 Hz: {wake}");

    let op0 = wedge_opacities(&rt);
    assert_eq!(peak_wedge(&op0).0, 41);

    // Stepping at 120 Hz (8 ms) and 60 Hz (16 ms) inside the first 33.2 ms step must NOT glide.
    assert!(!rt.frame(0.008));
    assert_eq!(wedge_opacities(&rt), op0, "120 Hz frame inside step 0 must not glide");
    assert!(!rt.frame(0.008));
    assert_eq!(wedge_opacities(&rt), op0, "60 Hz frame inside step 0 must not glide");

    // Past 1.96 / 59 s (34 ms total): snaps clockwise by 1/59 of a turn (wedge 41 -> 42).
    assert!(!rt.frame(0.018));
    let op1 = wedge_opacities(&rt);
    assert_ne!(op1, op0, "beam advances on the next 30 Hz step");
    assert_eq!(peak_wedge(&op1).0, 42);

    // One full 1.96 s turn returns to step 0 (wedge 41).
    assert!(!rt.frame(1.96 - 0.034));
    assert_eq!(wedge_opacities(&rt), op0);
}

#[test]
fn reduced_motion_holds_a_still_stop_beam_and_requests_no_repaint() {
    let mut rt = window_runtime(running_state(true), true);
    assert!(rt.find("stop-beam").is_some());
    assert!(rt.find("stop-beam-light").is_some());

    let still = wedge_opacities(&rt);
    let (peak, peak_alpha) = peak_wedge(&still);
    assert_eq!(peak, 41, "reduced motion draws the calm still frame");
    assert!(peak_alpha > 0.70 && peak_alpha <= 0.75);
    assert!(
        rt.elements()
            .iter()
            .filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("stop-beam-wedge-")))
            .all(|e| e.loop_phase.is_none()),
        "reduced motion attaches no frame-clock loop to stop-beam wedges"
    );
    assert_eq!(rt.wake_deadline(), None, "reduced motion requests no repaint");
    assert!(!rt.frame(0.05));
    assert!(!rt.frame(0.5));
    assert_eq!(rt.wake_deadline(), None);
    assert_eq!(wedge_opacities(&rt), still, "still frame never moves");
}

#[test]
fn ending_a_turn_removes_the_beam_and_stops_requesting_repaints() {
    let mut rt = thread_runtime(running_state(true), false);
    assert!(rt.find("stop-beam").is_some());
    assert!(rt.wake_deadline().is_some());

    // Turn finishes: beam disappears and the thread asks for no further repaints.
    rt.update(|s, tx| {
        tx.instant();
        s.flight = None;
        s.own_turn = None;
        if let Some(pane) = s.ledger.as_mut() {
            pane.snapshot.running = false;
            pane.snapshot.live = None;
        }
    });
    rt.frame(0.016);
    rt.frame(0.016);
    assert!(stop_beam_elements(&rt).is_empty(), "finished turn drops the stop-beam");
    assert!(!rt.frame(0.2));
    assert_eq!(rt.wake_deadline(), None, "finished turn requests no repaint");
}
