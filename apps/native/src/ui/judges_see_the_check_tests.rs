//! judges-see-the-check.test.tsx (0.810, 0.811), the window half: in a comparison that edits, each column shows what
//! your check said in its copy, with nothing to send from there; the judges' ask says they read it, where to set one,
//! or that a judge asked now waits for one still running; the same failure in every copy is said once; the judges'
//! table has a "Your check" row; and the benchmark a Check passed column. Drawn headless (`compare_view::view`,
//! `render_scorecard`, `check_passed_cell`). The core half is in core's `compare_judge_check_tests`,
//! `benchmark_check_tests` and `compare_route_label_tests`.
use super::*;
use locust_core::compare::{CompareSlot, CompareSlotId};
use locust_core::compare_score::{ScoreVerdict, Scorecard};
use locust_core::edit_check::EditCheckOutcome;
use rune::{Env, Runtime};
use std::collections::BTreeMap;

const FOLDER: &str = "/tmp/locust-judges-see-the-check";
const MISSING: &str = "sh: vitest: command not found";

struct Harness {
    state: State,
    build: Box<dyn Fn(&State) -> El>,
}
fn harness_view(h: &Harness, _: &mut Cx<Harness>) -> El {
    col().w_full().h_full().child((h.build)(&h.state))
}

fn slot(id: CompareSlotId, model: &str, label: &str) -> CompareSlot {
    CompareSlot {
        slot: id,
        route: CompareRoute { runtime: "opencode".into(), model: model.into(), account: None, effort: None, label: Some(label.into()),
            mode: None, instructions: None, why: None, skill: None },
        mission_ids: vec![format!("m_{}", id.as_str())],
        refused: None,
        retried: vec![],
        folder: None,
    }
}

fn compare() -> Compare {
    Compare {
        compare_id: "cmp_1".into(),
        teammate_id: None,
        prompt: "Split total into parseRow and summarize; keep the tests passing.".into(),
        created_at: "2026-10-21T12:00:00.000Z".into(),
        slots: vec![
            slot(CompareSlotId::A, "own-0a1b2c3d/claude-haiku-5-5", "claude-haiku-5-5"),
            slot(CompareSlotId::B, "own-0a1b2c3d/claude-sonnet-5-5", "Sonnet at work"),
        ],
        changes: true,
        changes_in: None,
        blind: false,
        kept: None,
        judge: None,
        base: Some("b".repeat(40)),
        rerun_of: None,
        reruns: vec![],
        settled: false,
        point_drafts: vec![],
    }
}

/// What a column's check said: none, still running, or an outcome with its new lines.
#[derive(Clone, Copy)]
enum Said<'a> {
    None,
    Running,
    Ran(EditCheckOutcome, &'a [&'a str]),
}
const PASSED: Said = Said::Ran(EditCheckOutcome::Passed, &[]);
const FAILED: Said = Said::Ran(EditCheckOutcome::Failed, &["FAIL totals.test.mjs", "TypeError: summarize is not a function"]);
const FAILED_MISSING: Said = Said::Ran(EditCheckOutcome::Failed, &[MISSING]);

fn state(said: [Said; 2], folder_command: Option<&str>) -> State {
    let mut s = State::default();
    s.home.folder_path = Some(FOLDER.into());
    if let Some(command) = folder_command {
        let ws = locust_core::workspace_settings::workspace_id_for(std::path::Path::new(FOLDER));
        s.settings_panel.settings.check_commands.insert(ws, command.into());
    }
    for (id, said) in ["m_a", "m_b"].into_iter().zip(said) {
        let mut items = vec![ledger::Item::Assistant { item_id: format!("{id}_1"), text: "Done.".into(), r#final: true }];
        match said {
            Said::None => {}
            Said::Running => {
                s.running_checks.insert(id.into(), "npm test".into());
            }
            Said::Ran(outcome, lines) => items.push(ledger::Item::EditCheck {
                mission_id: id.into(),
                command: "npm test".into(),
                outcome,
                new_lines: lines.iter().map(|l| (*l).to_owned()).collect(),
                unchanged: false,
                first: true,
                fix: None,
            }),
        }
        s.compare.snapshots.insert(id.into(), ledger::Snapshot { tip: id.into(), items, ..Default::default() });
    }
    s.compare.compares = vec![compare()];
    s.compare.active_id = Some("cmp_1".into());
    s.compare.loaded = true;
    s
}

fn drawn(state: State, build: impl Fn(&State) -> El + 'static) -> Vec<(String, Option<rune::Color>)> {
    let mut rt = Runtime::new(Harness { state, build: Box::new(build) }, harness_view, (1400.0, 2400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|n| n.text.map(|t| (t, n.text_color))).collect()
}
fn compare_window(s: &State, _: &mut Cx<State>) -> El {
    view(s, &Palette::new(true))
}
/// The compare screen's words, drawn as the window draws it: its buttons' handlers are the app's own.
fn words(state: State) -> Vec<String> {
    let mut rt = Runtime::new(state, compare_window, (1400.0, 2400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|n| n.text).collect()
}
fn window(said: [Said; 2], folder_command: Option<&str>) -> Vec<String> {
    words(state(said, folder_command))
}
fn has(seen: &[String], words: &str) -> bool {
    seen.iter().any(|t| t.contains(words))
}

fn check(outcome: JudgedCheckOutcome) -> Option<JudgedCheck> {
    Some(JudgedCheck { outcome, lines: vec![], unchanged: false })
}
fn table(checks: [Option<JudgedCheck>; 2]) -> Vec<(String, Option<rune::Color>)> {
    let card = Scorecard {
        points: vec!["Splits total into parseRow and summarize".into()],
        by_judge: false,
        verdicts: BTreeMap::from([
            ("a".to_owned(), vec![Some(ScoreVerdict { met: true, why: "Split as asked.".into(), votes: None })]),
            ("b".to_owned(), vec![Some(ScoreVerdict { met: true, why: "Split as asked.".into(), votes: None })]),
        ]),
        weights: None,
    };
    drawn(State::default(), move |_| {
        let p = Palette::new(true);
        let names = vec!["Claude Haiku 5.5".to_owned(), "Sonnet at work".to_owned()];
        render_scorecard(&card, &compare().slots, &names, &[None, None], &[None, None], &checks, false, &p, &crate::look::controls::Look::new(true))
    })
}
fn after<'a>(seen: &'a [(String, Option<rune::Color>)], head: &str) -> Option<&'a [(String, Option<rune::Color>)]> {
    let at = seen.iter().position(|(t, _)| t == head)?;
    seen.get(at + 1..at + 3)
}

// case: shows what it said in each copy, under that column's answer
#[test]
fn shows_what_it_said_in_each_copy_under_that_columns_answer() {
    let seen = window([PASSED, FAILED], Some("npm test"));
    assert!(has(&seen, "The check npm test passed after this turn."), "{seen:?}");
    assert!(has(&seen, "The check npm test failed after this turn:"), "{seen:?}");
    assert!(has(&seen, "TypeError: summarize is not a function"), "{seen:?}");
    // A column is not a conversation: nothing is sent back from here.
    assert!(!has(&seen, "Send to"), "{seen:?}");
    // A model added under its own id reads by its name; a name the person wrote stands.
    assert!(seen.iter().any(|t| t == "Claude Haiku 5.5") && seen.iter().any(|t| t == "Sonnet at work"), "{seen:?}");
    // A turn not checked says nothing: an older turn's check is not this one's.
    let mut s = state([PASSED, PASSED], Some("npm test"));
    s.compare.snapshots.get_mut("m_a").unwrap().tip = "m_a2".into();
    assert_eq!(snapshot_check(&s.compare.snapshots["m_a"], &s.running_checks), None);
    assert!(snapshot_check(&s.compare.snapshots["m_b"], &s.running_checks).is_some_and(|c| c.outcome == JudgedCheckOutcome::Passed));
}

// case: tells the person the judges read it, or where to set one
#[test]
fn tells_the_person_the_judges_read_it_or_where_to_set_one() {
    assert!(has(&window([PASSED, FAILED], Some("npm test")), "Each judge also reads what your check, npm test, said in each copy."));
    assert!(has(
        &window([Said::None, Said::None], None),
        "Set a check command in Settings > Project folder, and each judge also reads whether every copy passes it."
    ));
    // Set, but no copy has run it yet: nothing to promise and nothing to ask.
    let quiet = window([Said::None, Said::None], Some("npm test"));
    assert!(!has(&quiet, "Each judge also reads") && !has(&quiet, "Set a check command"), "{quiet:?}");
}

// case: puts it beside the judges' score
#[test]
fn puts_it_beside_the_judges_score() {
    let p = Palette::new(true);
    let seen = table([check(JudgedCheckOutcome::Passed), check(JudgedCheckOutcome::Failed)]);
    assert_eq!(
        after(&seen, "Your check").map(|c| c.to_vec()),
        Some(vec![("passed".to_owned(), Some(p.green)), ("failed".to_owned(), Some(p.amber))]),
        "{seen:?}"
    );
    assert!(!table([None, None]).iter().any(|(t, _)| t == "Your check"));
}

// case: says once when it failed the same way in every copy, and what to do about it
#[test]
fn says_once_when_it_failed_the_same_way_in_every_copy_and_what_to_do_about_it() {
    let seen = window([FAILED_MISSING, FAILED_MISSING], Some("npm test"));
    assert_eq!(seen.iter().filter(|t| t.as_str() == CHECK_FAILED_EVERYWHERE).count(), 1, "{seen:?}");
    assert!(CHECK_FAILED_EVERYWHERE.starts_with("Your check failed the same way in every copy, so it tells them apart on nothing."));
    assert!(!has(&window([PASSED, FAILED], Some("npm test")), "failed the same way in every copy"));
    // Working in the folder itself, no copy ran it.
    let mut s = state([FAILED_MISSING, FAILED_MISSING], Some("npm test"));
    s.compare.compares[0].changes_in = Some(ChangesIn::Folder);
    let folder = words(s);
    assert!(!has(&folder, "failed the same way in every copy"), "{folder:?}");
}

// case: says so under the turn, with nothing to send yet
#[test]
fn says_so_under_the_turn_with_nothing_to_send_yet() {
    let seen = window([Said::Running, PASSED], Some("npm test"));
    assert!(has(&seen, "Running the check npm test after this turn…"), "{seen:?}");
    assert!(!has(&seen, "Send to"), "{seen:?}");
}

// case: tells the person a judge asked now waits for it, and reads it as running, in no colour
#[test]
fn tells_the_person_a_judge_asked_now_waits_for_it_and_reads_it_as_running_in_no_colour() {
    let seen = window([Said::Running, PASSED], Some("npm test"));
    assert!(
        has(&seen, "Your check, npm test, is still running in a copy. A judge asked now waits for it to finish, then reads what it said in each copy."),
        "{seen:?}"
    );
    let p = Palette::new(true);
    let row = table([check(JudgedCheckOutcome::Running), check(JudgedCheckOutcome::Passed)]);
    assert_eq!(
        after(&row, "Your check").map(|c| c.to_vec()),
        Some(vec![("running".to_owned(), Some(p.text)), ("passed".to_owned(), Some(p.green))]),
        "{row:?}"
    );
    // It does wait: a judge asked while a copy's check runs is asked again once it has said.
    let mut s = state([Said::Running, PASSED], Some("npm test"));
    assert!(checks_running(&s, &compare()));
    s.compare.judge_waiting = Some("cmp_1".into());
    let waiting = words(s);
    assert!(waiting.iter().any(|t| t == "Asking…"), "{waiting:?}");
    assert!(!checks_running(&state([PASSED, PASSED], Some("npm test")), &compare()));
}

#[test]
fn a_judge_waiting_on_a_check_is_asked_once_it_has_said_and_only_once() {
    let mut s = state([Said::Running, PASSED], Some("npm test"));
    s.compare.judge_waiting = Some("cmp_1".into());
    assert!(!ask_waiting_judge(&mut s), "still running: it waits");
    assert_eq!(s.compare.judge_waiting.as_deref(), Some("cmp_1"));
    s.running_checks.clear();
    // Asked now; with nothing to ask it on here it is refused, and the wait is over rather than asked every tick.
    assert!(ask_waiting_judge(&mut s));
    assert_eq!(s.compare.judge_waiting, None);
    assert!(!ask_waiting_judge(&mut s));
}

// case: counts the runs each model's copy passed it
#[test]
fn the_check_passed_cell_is_green_when_every_run_passed_and_amber_otherwise() {
    let p = Palette::new(true);
    let row = |checked, check_passed| BenchmarkRow {
        key: "k".into(), runtime: "opencode".into(), model: "m".into(), effort: None, name: "m".into(), runs: 2, scored: 2,
        score: Some(100), every_point: 2, span_ms: None, cost: None, checked, check_passed, slot: None,
    };
    assert_eq!(check_passed_cell(&row(2, 2), &p), ("2 of 2".to_owned(), p.green));
    assert_eq!(check_passed_cell(&row(2, 1), &p), ("1 of 2".to_owned(), p.amber));
    assert_eq!(check_passed_cell(&row(0, 0), &p), ("\u{2014}".to_owned(), p.text));
}
