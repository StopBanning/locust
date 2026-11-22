//! Electron `main/cold-start-is-not-an-alert.test.ts`: "A fresh session" is a note under the turn's opening, not an
//! alert at the end of the thread. Drawn headless through the message column: the reply that started cold is the
//! conversation's newest turn, a quiet hour after the one before it, with a question card waiting.
use super::*;
use locust_core::approval::{Kind, Request};
use locust_core::time_marker::TurnTiming;

const COLD: &str = "A fresh session: the turn before could not be picked up again";

fn meta(id: &str, prompt: &str) -> ledger::Metadata {
    ledger::Metadata { mission_id: id.into(), prompt: prompt.into(), runtime: "codex".into(), ..Default::default() }
}

fn waiting_card() -> Request {
    Request {
        approval_id: "ap_1".into(),
        mission_id: "mission_2".into(),
        runtime: "codex".into(),
        kind: Kind::Command,
        summary: "Run a command".into(),
        detail: "npm test".into(),
        cwd: None,
        requested_at: "2026-11-21T10:31:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: vec![],
    }
}

/// Who takes what? at 09:00, then "Now add tests." an hour and a half later on the same agent, cold when `cold` says
/// so; `tweak` changes the pane before it is drawn. Every drawn element with words, top to bottom.
fn thread(cold: Option<bool>, tweak: impl FnOnce(&mut State)) -> Vec<rune::ElementInfo> {
    let metas = vec![meta("mission_1", "Who takes what?"), meta("mission_2", "Now add tests.")];
    let items = vec![
        ledger::Item::User("Who takes what?".into()),
        ledger::Item::Assistant { item_id: "a1".into(), text: "Wren takes the parser.".into(), r#final: true },
        ledger::Item::User("Now add tests.".into()),
        ledger::Item::Assistant { item_id: "a2".into(), text: "Adding them now.".into(), r#final: false },
    ];
    let timings = vec![
        TurnTiming { before_item: 0, first_at: Some("2026-11-21T09:00:00.000Z".into()), last_at: Some("2026-11-21T09:01:00.000Z".into()) },
        TurnTiming { before_item: 2, first_at: Some("2026-11-21T10:30:00.000Z".into()), last_at: Some("2026-11-21T10:31:00.000Z".into()) },
    ];
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: cold.map(|c| ("mission_2".to_owned(), c)),
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("mission_1".into()), turns: Vec::new() },
        items: Vec::new(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { turn_metas: metas, timings, tip: "mission_2".into(), running: true, ..Default::default() },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s.pending_approval = Some(waiting_card());
    tweak(&mut s);
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Wren", &Palette::new(true), s), (800.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let mut drawn: Vec<_> = rt.elements().into_iter().filter(|e| e.text.is_some() || e.role == "separator").collect();
    drawn.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    drawn
}

fn words(drawn: &[rune::ElementInfo]) -> Vec<String> {
    drawn.iter().map(|e| e.text.clone().unwrap_or_else(|| format!("[{}]", e.label.clone().unwrap_or_default()))).collect()
}

/// Where in the thread, top to bottom, the first element saying `needle` is.
fn at(drawn: &[rune::ElementInfo], needle: &str) -> usize {
    let said = words(drawn);
    said.iter().position(|w| w.contains(needle)).unwrap_or_else(|| panic!("expected to find {needle:?} in {said:#?}"))
}

/// The time marker that opens the newest turn: the thread's only one, a quiet 89 minutes after the first turn ended.
fn marker(drawn: &[rune::ElementInfo]) -> usize {
    drawn.iter().position(|e| e.role == "separator" && e.label.as_deref().is_some_and(|l| l.contains("minutes in"))).expect("the newest turn's time marker")
}

/// What is wrong with where the note is drawn, if anything: it must open the newest turn, right under its time marker
/// (nothing between them but the marker's own words), above the turn's words, its work and the approval card.
fn misplaced(drawn: &[rune::ElementInfo]) -> Option<String> {
    let said = words(drawn);
    let find = |needle: &str| said.iter().position(|w| w.contains(needle));
    let (Some(note), Some(time)) = (find(COLD), Some(marker(drawn))) else { return Some("no note".into()) };
    if [find("Now add tests."), find("Adding them now."), find("Run a command")].into_iter().any(|at| at.is_none_or(|at| at < note)) {
        return Some(format!("below the work it describes or the approvals: {said:#?}"));
    }
    if time > note {
        return Some(format!("above its time marker: {said:#?}"));
    }
    let line = drawn[time].rect;
    if drawn[time + 1..note].iter().any(|e| e.rect[1] < line[1] || e.rect[1] + e.rect[3] > line[1] + line[3] + 0.5) {
        return Some(format!("not adjacent to its time marker: {:#?}", &said[time..=note]));
    }
    None
}

// case: sits above the work it describes, not below it
#[test]
fn sits_above_the_work_it_describes_not_below_it() {
    let drawn = thread(Some(true), |_| {});
    assert!(at(&drawn, COLD) < at(&drawn, "Now add tests."), "{:#?}", words(&drawn));
    assert!(at(&drawn, COLD) < at(&drawn, "Adding them now."));
    assert_eq!(misplaced(&drawn), None);
}

// case: is nowhere near the approvals, which are what the run is waiting on
#[test]
fn is_nowhere_near_the_approvals_which_are_what_the_run_is_waiting_on() {
    let drawn = thread(Some(true), |_| {});
    let card = at(&drawn, "Run a command");
    assert!(at(&drawn, COLD) < card);
    // The turn's own work lies between them: the note is at the turn's top, the card at the thread's foot.
    assert!(at(&drawn, COLD) < at(&drawn, "Adding them now.") && at(&drawn, "Adding them now.") < card);
}

// case: follows the time marker, adjacent to it, opening the turn
#[test]
fn follows_the_time_marker_adjacent_to_it_opening_the_turn() {
    let drawn = thread(Some(true), |_| {});
    let time = marker(&drawn);
    let note = at(&drawn, COLD);
    assert!(time < note, "{:#?}", words(&drawn));
    // Adjacent: nothing between the marker and the note but the marker's own words, inside its line.
    let line = drawn[time].rect;
    for between in &drawn[time + 1..note] {
        assert!(between.rect[1] >= line[1] && between.rect[1] + between.rect[3] <= line[1] + line[3] + 0.5, "{:#?}", &words(&drawn)[time..=note]);
    }
}

// case: is a NOTE, not a marker: the two registers must not be confused
#[test]
fn is_a_note_not_a_marker_the_two_registers_must_not_be_confused() {
    let drawn = thread(Some(true), |_| {});
    let note = &drawn[at(&drawn, COLD)];
    let time = &drawn[marker(&drawn)];
    // A marker is a full-width separator with its words centred; a note is words, left-aligned where the turn's words begin.
    assert_ne!(note.role, "separator");
    let prompt_left = drawn.iter().filter(|e| e.text.is_some()).map(|e| e.rect[0]).fold(f32::INFINITY, f32::min);
    assert_eq!(note.rect[0], prompt_left, "the note starts at the column's left edge");
    let marker_words = drawn.iter().find(|e| e.text.is_some() && e.rect[1] >= time.rect[1] && e.rect[1] <= time.rect[1] + time.rect[3]).expect("the marker's words");
    assert!(marker_words.rect[0] > note.rect[0] + 100.0, "the marker's words are centred, the note's are not");
}

// case: still says the thing worth saying
#[test]
fn still_says_the_thing_worth_saying() {
    let drawn = thread(Some(true), |_| {});
    assert_eq!(
        drawn[at(&drawn, COLD)].text.as_deref(),
        Some("A fresh session: the turn before could not be picked up again, so the teammate was given the conversation so far")
    );
}

// case: is a real control: it catches the placement that shipped
#[test]
fn is_a_real_control_it_catches_the_placement_that_shipped() {
    let drawn = thread(Some(true), |_| {});
    assert_eq!(misplaced(&drawn), None);
    // The order as it stood in Electron 0.120.0: the turn's work, the approvals, then the note against the composer.
    let mut shipped = drawn.clone();
    let note = shipped.remove(at(&drawn, COLD));
    shipped.push(note);
    assert!(misplaced(&shipped).is_some_and(|why| why.starts_with("below the work")));
    // Above the marker instead of under it, and with the turn's words between them: neither is the turn's opening.
    let mut above = drawn.clone();
    let note = above.remove(at(&drawn, COLD));
    above.insert(marker(&drawn), note);
    assert!(misplaced(&above).is_some_and(|why| why.starts_with("above its time marker")));
}

#[test]
fn is_drawn_only_where_the_turn_started_cold() {
    // Without the host saying so, nothing: the assertions above do not pass on a thread that never draws it.
    let said = |cold: Option<bool>, tweak: fn(&mut State)| words(&thread(cold, tweak)).iter().any(|w| w.contains(COLD));
    assert!(said(Some(true), |_| {}));
    assert!(!said(None, |_| {}));
    assert!(!said(Some(false), |_| {}), "a reply that resumed its session");
    // An edit says its own note ("Started again from an edited message").
    assert!(!said(Some(true), |s| s.ledger.as_mut().unwrap().snapshot.turn_metas[1].edited = true));
    // Said of the tip only: once a later turn is the tip, the flag read for this one no longer applies.
    assert!(!said(Some(true), |s| s.ledger.as_mut().unwrap().snapshot.tip = "mission_3".into()));
    // A flag read for the turn before is not this one's.
    assert!(!said(Some(true), |s| s.ledger.as_mut().unwrap().cold_tip = Some(("mission_1".into(), true))));
    // A first turn has no turn before it to have lost.
    assert!(!said(Some(true), |s| {
        let pane = s.ledger.as_mut().unwrap();
        pane.snapshot.turn_metas.remove(0);
    }));
}
