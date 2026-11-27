//! A window behind another app's asks for no frames for its loops; they start again when it has focus. Locust's Home
//! cover kept drawing at the display's rate while its window sat behind other apps (speed-1 row 3).
use rune::{Color, Cx, El, Env, LoopSpec, Runtime, col, el};

fn view(_: &(), _: &mut Cx<()>) -> El {
    col().key("root").size(100.0, 100.0).child(el().key("spin").size(20.0, 20.0).fill(Color::WHITE).looping(LoopSpec::rotation(4.0)))
}

#[test]
fn loops_stand_still_in_a_window_without_focus_and_start_again_with_it() {
    let mut rt = Runtime::new((), view, (100.0, 100.0), Env::new(true, 2.0));
    rt.frame(0.0);
    assert!(rt.animating, "a window never told its focus runs its loops");
    rt.window_focus(true);
    rt.frame(1.0 / 60.0);
    assert!(rt.animating, "focused: the loop turns");
    rt.window_focus(false);
    rt.frame(1.0 / 60.0);
    // The shell's scheduling gate: a frame is drawn only while the runtime animates or has input queued.
    let wants_frame = |rt: &Runtime<()>| rt.animating || rt.has_pending_input();
    let mut requested = 0;
    for _ in 0..600 {
        if wants_frame(&rt) {
            requested += 1;
        }
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(requested, 0, "behind another app's window: ten seconds, no frames");
    rt.window_focus(true);
    // Nothing else changed, so only the focus itself can ask the shell for the frame that starts the loop again.
    assert!(wants_frame(&rt), "focus back: the window asks for a frame before any other event");
    rt.frame(1.0 / 60.0);
    assert!(rt.animating, "focus back: the loop turns again");
}
