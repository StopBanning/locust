//! The selected window conversation and its own queue, not whichever teammate last sent.
use super::*;

struct TempRoot(std::path::PathBuf);
impl Drop for TempRoot {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn selected_pane(running: bool, run_end: Option<ledger::RunEnd>) -> (State, TempRoot) {
    let mut s = State::default();
    team_scene(&mut s);
    s.demo = false;
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = TempRoot(std::env::temp_dir().join(format!("locust-queue-owner-{}-{id}", std::process::id())));
    std::fs::create_dir_all(root.0.join("electron")).unwrap();
    s.data = Some(store::Store::new(root.0.join("native"), root.0.join("electron")).unwrap());
    s.ledger = Some(LedgerPane {
        header: s.teammates[0].clone(),
        conversation: Conversation {
            conversation_id: Some("run_x".into()),
            turns: vec![("You".into(), "Original question".into())],
        },
        items: vec![],
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { tip: "codec_tip".into(), running, run_end, ..Default::default() },
        cold_tip: None,
        due: f64::INFINITY,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    (s, root)
}

fn rendered(s: State) -> Vec<String> {
    let mut rt = rune::Runtime::new(
        s, |s, _| crate::thread_view::thread_view(s, &Palette::new(true)),
        (1200.0, 1000.0), rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    assert!(rt.find("queued").is_some(), "the selected composer draws its real queue");
    rt.elements().iter().filter_map(|el| el.text.clone()).collect()
}

// case: replies into the live run on screen, whoever else is busy
#[test]
fn replies_into_the_live_run_on_screen_whoever_else_is_busy() {
    let (mut s, _root) = selected_pane(true, None);
    s.flight = Some(Flight {
        teammate_id: "argon".into(), conversation_id: Some("run_h".into()),
        header: Some(s.teammates[1].clone()), after_tip: None, message: None,
    });
    let mut rt = rune::Runtime::new(
        s, |s, _| crate::thread_view::thread_view(s, &Palette::new(true)),
        (1200.0, 1000.0), rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let (x, y) = rt.find("composer").expect("the selected conversation's message box");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.paste("Add the integration test.");
    rt.frame(0.016);
    rt.key(rune::Key::Enter.into());
    rt.frame(0.016);
    assert_eq!(rt.state.queue_rows.len(), 1);
    assert_eq!(rt.state.queue_rows[0].key, "run_x");
    assert!(rt.state.draft.text.is_empty());
    let words: Vec<_> = rt.elements().into_iter().filter_map(|el| el.text).collect();
    assert!(words.iter().any(|text| text == "Add the integration test."));
    assert!(words.iter().any(|text| text == "Sends when Codec finishes"));
    assert!(!words.iter().any(|text| text == "Sends when Argon finishes"));
}

#[test]
fn the_selected_terminal_snapshot_sets_the_queue_status_while_its_worker_is_out() {
    let (mut s, _root) = selected_pane(true, None);
    s.draft.replace_text("Add the integration test.");
    submit_composer(&mut s, &mut Tx::default(), false);

    let pane = s.ledger.as_mut().unwrap();
    pane.snapshot.running = false;
    pane.snapshot.run_end = Some(ledger::RunEnd::Cancelled);
    assert_eq!(queue_phase(&s), "cancelled");
    drain_queue_after_turn(&mut s, &mut Tx::default());
    assert_eq!(s.queue_rows.len(), 1, "a stopped selected run must leave its row queued");

    let words = rendered(s);
    assert!(words.iter().any(|text| text == "Held — the run was stopped"), "{words:?}");
}

#[test]
fn a_completed_selected_snapshot_does_not_borrow_another_search_or_turn_status() {
    let (mut s, _root) = selected_pane(true, None);
    s.draft.replace_text("Add the integration test.");
    submit_composer(&mut s, &mut Tx::default(), false);
    let pane = s.ledger.as_mut().unwrap();
    pane.snapshot.running = false;
    pane.snapshot.run_end = Some(ledger::RunEnd::Completed);
    s.status_tone = StatusTone::Red;
    s.status = "Argon: failed".into();
    assert_eq!(queue_phase(&s), "completed");
}

#[test]
fn an_interrupted_selected_snapshot_holds_the_queue_as_interrupted() {
    // Lane check of B25: a selected conversation whose run was interrupted (the engine went away mid-turn) reads as
    // interrupted, not as its run end or the worker's phase, and leaves the queued row where it is.
    let (mut s, _root) = selected_pane(true, None);
    s.draft.replace_text("Add the integration test.");
    submit_composer(&mut s, &mut Tx::default(), false);

    let pane = s.ledger.as_mut().unwrap();
    pane.snapshot.running = false;
    pane.snapshot.interrupted = true;
    pane.snapshot.run_end = Some(ledger::RunEnd::Completed);
    assert_eq!(queue_phase(&s), "interrupted");
    drain_queue_after_turn(&mut s, &mut Tx::default());
    assert_eq!(s.queue_rows.len(), 1, "an interrupted selected run must leave its row queued");
}
