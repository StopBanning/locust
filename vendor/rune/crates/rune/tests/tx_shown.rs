//! `Tx::shown`: a poll or an update hears whether any of the window is on screen, so motion it steps itself can hold
//! while the window is hidden, minimised or covered, and run while it shows behind another app's window.
use rune::{Cx, El, Env, Runtime, col};

fn view(_: &(), _: &mut Cx<()>) -> El {
    col().key("root").size(100.0, 100.0)
}

#[test]
fn an_update_hears_whether_the_window_shows() {
    let mut rt = Runtime::new((), view, (100.0, 100.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let mut heard = Vec::new();
    rt.update(|_, tx| heard.push(tx.shown()));
    rt.window_focus(false);
    rt.update(|_, tx| heard.push(tx.shown()));
    rt.set_live_visible(false);
    rt.update(|_, tx| heard.push(tx.shown()));
    rt.set_live_visible(true);
    rt.update(|_, tx| heard.push(tx.shown()));
    assert_eq!(heard, [true, true, false, true], "shown; behind another window, still shown; covered; back");
}
