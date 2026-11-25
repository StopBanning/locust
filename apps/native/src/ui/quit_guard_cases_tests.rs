//! quit-guard.test.ts, at the window: closing while a teammate works asks first, through the window's own close
//! (`request_close`, Cmd-Q and Cmd-W) and the dialog it draws, driven with the window's own keys.
use super::*;
use crate::Flight;
use rune::{Env, Key, KeyEvent, Mods, Runtime};

/// The whole window, with a send on its way (a live run the guard counts) and the window's own keys.
fn working() -> Runtime<State> {
    let mut s = State::default();
    s.flight = Some(Flight { teammate_id: "tm_wren".into(), conversation_id: None, header: None, after_tip: None, message: None });
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    rt.on_key = Some(crate::on_key);
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..8 {
        rt.frame(0.05);
    }
}

fn close(rt: &mut Runtime<State>) {
    rt.update(|s, tx| request_close(s, tx));
    settle(rt);
}

fn key(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    settle(rt);
}

fn press(rt: &mut Runtime<State>, at: &str) {
    let (x, y) = rt.find(at).unwrap_or_else(|| panic!("no {at}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

#[test]
fn never_stands_in_the_way_of_a_quit_already_under_way() {
    // case: never stands in the way of a quit already under way -- a relaunch, the updater
    let mut rt = working();
    rt.update(|s, _| s.quit_guard.quit_requested = true);
    close(&mut rt);
    assert!(!rt.state.quit_guard.open, "a quit under way is not asked about, though a teammate works");
    assert!(rt.state.quit_guard.quit_requested);
    // The same window, with no quit under way, asks.
    let mut rt = working();
    close(&mut rt);
    assert!(rt.state.quit_guard.open);
}

#[test]
fn asks_once_quit_anyway_closes() {
    // case: asks once: "Quit anyway" closes
    let mut rt = working();
    close(&mut rt);
    assert!(rt.state.quit_guard.open);
    press(&mut rt, "quit-guard-quit");
    assert!(!rt.state.quit_guard.open && rt.state.quit_guard.confirmed && rt.state.quit_guard.quit_requested);
    rt.update(|s, _| s.quit_guard.quit_requested = false);
    close(&mut rt);
    assert!(!rt.state.quit_guard.open, "answered once, it is not asked again");
}

#[test]
fn keeping_the_work_going_is_the_first_button_and_the_default_cancel_leaves_the_window_open() {
    // case: keeping the work going is the first button and the default; Cancel leaves the window open (0.397)
    let mut rt = working();
    close(&mut rt);
    let all: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).collect();
    let at = |k: &str| all.iter().find(|e| e.key.as_deref() == Some(k)).unwrap_or_else(|| panic!("no {k}")).rect;
    let said = |k: &str| {
        let r = at(k);
        all.iter()
            .filter(|e| e.text.is_some() && e.rect[0] >= r[0] - 0.5 && e.rect[0] + e.rect[2] <= r[0] + r[2] + 0.5 && e.rect[1] >= r[1] - 0.5 && e.rect[1] <= r[1] + r[3])
            .filter_map(|e| e.text.clone())
            .collect::<Vec<_>>()
    };
    let (keep, quit, cancel) = (at("quit-guard-keep"), at("quit-guard-quit"), at("quit-guard-cancel"));
    assert!(keep[0] < quit[0] && quit[0] < cancel[0], "Keep working first, then Quit anyway, then Cancel");
    assert!(said("quit-guard-keep").iter().any(|t| t == "Keep working in the background"));
    assert!(said("quit-guard-quit").iter().any(|t| t == "Quit anyway"));
    assert!(said("quit-guard-cancel").iter().any(|t| t == "Cancel"));
    // The default: Return answers with it.
    key(&mut rt, Key::Enter);
    assert!(!rt.state.quit_guard.open, "Return answers the question");
    assert!(rt.state.quit_guard.kept_in_background && !rt.state.quit_guard.quit_requested, "with Keep working");
    // Cancel (or Escape) leaves the window open, nothing quit and nothing hidden.
    let mut rt = working();
    close(&mut rt);
    press(&mut rt, "quit-guard-cancel");
    assert!(!rt.state.quit_guard.open && !rt.state.quit_guard.quit_requested && !rt.state.quit_guard.kept_in_background);
    let mut rt = working();
    close(&mut rt);
    key(&mut rt, Key::Escape);
    assert!(!rt.state.quit_guard.open && !rt.state.quit_guard.quit_requested && !rt.state.quit_guard.kept_in_background);
}
