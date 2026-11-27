//! Big Brother's review of 26 Nov (`docs/REVIEW-ui-changes-2026-11-26.md`, change 3), which Colin approved as an
//! improvement on Electron's sidebar (`Sidebar.tsx:976-984`, `:1033-1041`, where a clean finish draws nothing and
//! `shell.css:20259-20261` keeps its dot invisible): "a small solid dot until the conversation is opened. The Team board
//! already tracks this (`Screens.tsx:637`, `finishedUnseen`)." Drawn headless through the sidebar, with the finish
//! recorded the way the window records it (`record_finishes`), kept across a restart in the data folder.
use super::*;
use rune::{Env, Runtime};

fn conv(id: &str, phase: &str) -> ConversationRow {
    ConversationRow {
        id: id.into(),
        title: format!("Conversation {id}"),
        teammate: String::new(),
        started_by: "native".into(),
        state: phase.into(),
        updated: "2026-09-15T00:00:00.000Z".into(),
    }
}

fn draw(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn dot<'a>(drawn: &'a [rune::ElementInfo], id: &str) -> Option<&'a rune::ElementInfo> {
    drawn.iter().find(|e| e.key.as_deref() == Some(format!("conversation-unseen-dot-{id}").as_str()))
}

fn state(rows: Vec<ConversationRow>, unseen: &[&str]) -> State {
    let mut s = State::default();
    s.demo = true;
    s.recent = rows;
    s.home.to_look_at = unseen.iter().map(|id| (*id).to_owned()).collect();
    s
}

/// A data folder of its own for a test: tests of one binary run side by side.
fn folder(label: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("locust-sidebar-unseen-{label}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    root
}

fn store_in(root: &std::path::Path) -> locust_core::store::Store {
    locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap()
}

#[test]
fn a_conversation_that_finished_while_it_was_not_open_wears_a_small_solid_dot() {
    let rt = draw(state(vec![conv("done", "completed"), conv("seen", "completed")], &["done"]));
    let drawn = rt.elements();
    let found = dot(&drawn, "done").unwrap_or_else(|| panic!("no dot on the finished row: {:?}", drawn.iter().filter_map(|e| e.key.clone()).filter(|k| k.contains("done")).collect::<Vec<_>>()));
    assert_eq!((found.rect[2], found.rect[3]), (6.0, 6.0), "a small dot");
    assert_eq!(found.fill, Palette::new(true).green, "solid, in the colour of a finish");
    assert!(dot(&drawn, "seen").is_none(), "a finish already seen has no dot");
}

#[test]
fn the_dot_sits_in_the_row_of_the_conversation_it_belongs_to() {
    let rt = draw(state(vec![conv("one", "completed"), conv("two", "completed")], &["two"]));
    let drawn = rt.elements();
    let title = |id: &str| drawn.iter().find(|e| e.key.as_deref() == Some(format!("conversation-title-{id}").as_str())).unwrap().rect;
    let at = dot(&drawn, "two").unwrap().rect;
    assert!((at[1] - title("two")[1]).abs() < 20.0, "beside its own title");
    assert!(at[0] > title("two")[0], "after the title starts");
}

#[test]
fn a_running_conversation_and_a_failed_one_keep_their_own_marks_not_this_dot() {
    let rt = draw(state(vec![conv("live", "running"), conv("broke", "failed")], &["live", "broke"]));
    let drawn = rt.elements();
    assert!(dot(&drawn, "live").is_none(), "a running row shimmers instead of wearing a dot");
    assert!(dot(&drawn, "broke").is_none(), "a failed row wears its red mark");
}

#[test]
fn opening_the_conversation_takes_the_dot_away() {
    let mut s = state(vec![conv("done", "completed")], &["done"]);
    assert!(dot(&draw(state(vec![conv("done", "completed")], &["done"])).elements(), "done").is_some());
    crate::home_panel::opened(&mut s, "done");
    assert!(dot(&draw(s).elements(), "done").is_none());
}

#[test]
fn a_finish_is_recorded_as_unseen_when_the_conversation_was_not_the_one_open() {
    let mut s = state(vec![conv("m1", "running")], &[]);
    crate::home_panel::record_finishes(&mut s, &[conv("m1", "completed")]);
    s.recent = vec![conv("m1", "completed")];
    assert!(dot(&draw(s).elements(), "m1").is_some());
}

#[test]
fn the_unseen_finishes_survive_a_restart_and_leave_when_opened() {
    let root = folder("restart");
    let mut s = state(vec![conv("m1", "running"), conv("m2", "running")], &[]);
    s.data = Some(store_in(&root));
    crate::home_panel::record_finishes(&mut s, &[conv("m1", "completed"), conv("m2", "completed")]);
    // The window closes; the next one starts on the same data folder, with its own list of rows.
    let restarted = |opened: &[&str]| {
        let mut next = state(vec![conv("m1", "completed"), conv("m2", "completed")], &[]);
        next.data = Some(store_in(&root));
        crate::home_panel::load_unseen(&mut next);
        for id in opened {
            crate::home_panel::opened(&mut next, id);
        }
        next
    };
    let drawn = draw(restarted(&[])).elements();
    assert!(dot(&drawn, "m1").is_some() && dot(&drawn, "m2").is_some(), "both still unseen after the restart");
    // Opening one is kept too: gone by the restart after.
    drop(restarted(&["m1"]));
    let drawn = draw(restarted(&[])).elements();
    assert!(dot(&drawn, "m1").is_none(), "opened before the second restart");
    assert!(dot(&drawn, "m2").is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_finish_recorded_after_a_restart_keeps_the_ones_from_before() {
    let root = folder("keeps");
    let mut first = state(vec![conv("m1", "running")], &[]);
    first.data = Some(store_in(&root));
    crate::home_panel::record_finishes(&mut first, &[conv("m1", "completed")]);
    // The next window records a finish of its own before anything has asked for the file.
    let mut second = state(vec![conv("m3", "running")], &[]);
    second.data = Some(store_in(&root));
    crate::home_panel::record_finishes(&mut second, &[conv("m1", "completed"), conv("m3", "completed")]);
    let mut third = state(vec![conv("m1", "completed"), conv("m3", "completed")], &[]);
    third.data = Some(store_in(&root));
    crate::home_panel::load_unseen(&mut third);
    let drawn = draw(third).elements();
    assert!(dot(&drawn, "m1").is_some(), "the finish from before the restart is still unseen");
    assert!(dot(&drawn, "m3").is_some());
    let _ = std::fs::remove_dir_all(root);
}
