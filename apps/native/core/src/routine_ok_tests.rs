//! A ROUTINE STEP WAITS FOR YOUR OK (Electron `a-routine-step-waits-for-your-ok.test.ts`, 0.840): the three cases the
//! runner and store tests did not name -- the routine file, the ways a routine arrives, and the phone.
use super::*;
use crate::phone_alerts::{AlertKind, routine_ok_alert};
use crate::routine_file::{parse_routine_file, routine_export_text, routine_file_text, routine_to_file};

const STEP_ONE_ASKS: &str = "Step 1 cannot wait for your OK: it starts when you run the routine.";

fn waiting_routine() -> Value {
    json!({"routineId": "rt_1", "name": "Nightly tidy", "teammateId": "tm_wren",
        "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"},
        "steps": ["Read status.ts.", "Delete the old builds."], "handOffs": [{}, {"askFirst": true}],
        "learnedFrom": [], "createdAt": "2026-09-05T00:00:00.000Z", "runs": 0})
}
fn asks(file: &crate::routine_file::RoutineFile) -> Vec<bool> {
    file.hand_offs.iter().map(|h| h.ask_first == Some(true)).collect()
}

// case: travels in the routine file, as Markdown, and step 1 asking is refused on the way in
#[test]
fn travels_in_the_routine_file_as_markdown_and_step_1_asking_is_refused_on_the_way_in() {
    let r = waiting_routine();
    let steps: Vec<String> = r["steps"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect();
    let file = routine_to_file(&r, &steps, &[], &|_| None, &[]);
    assert_eq!(asks(&file), [false, true]);
    let (text, markdown) = routine_export_text(&file);
    assert!(markdown);
    assert!(text.contains("## Step 2 · waits for your OK\n"), "{text}");
    assert_eq!(asks(&parse_routine_file(&text).unwrap()), [false, true]);
    // Step 1 asking, in either form, is refused with the reason.
    let mut first = file.clone();
    first.hand_offs[0].ask_first = Some(true);
    first.hand_offs[1].ask_first = None;
    assert_eq!(parse_routine_file(&routine_file_text(&first)).unwrap_err(), STEP_ONE_ASKS);
    assert_eq!(parse_routine_file(&text.replace("## Step 1\n", "## Step 1 · waits for your OK\n")).unwrap_err(), STEP_ONE_ASKS);
}

// case: still waits after an import, a starter, a team card, and is listed to other apps
#[test]
fn still_waits_after_an_import_a_starter_a_team_card_and_is_listed_to_other_apps() {
    // Electron pins its source here; here the import is run. A file whose step 2 waits arrives waiting: the preview
    // says which step waits, and the saved routine keeps it. (Starters: routine_templates' hand_off_asks; a team
    // card: team_card's handOffs; other apps: door_routines_tests' waits_for_your_ok.)
    let (store, dir) = tests::test_store();
    let team = people::team(&store).unwrap();
    let tm_id = team["teammates"][0]["teammateId"].as_str().unwrap().to_owned();
    let r = waiting_routine();
    let steps: Vec<String> = r["steps"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect();
    let file = routine_to_file(&r, &steps, &[], &|_| None, &[]);
    let mut io = RoutineIO::new();
    let preview = io.preview_file_text(&routine_export_text(&file).0, Some("ws_test"), &[]).unwrap();
    assert_eq!(preview.hand_off_asks, Some(vec![false, true]));
    let imported = io.import_preview(&store, &preview.token, &tm_id, Some("ws_test")).unwrap();
    assert_eq!(imported["handOffs"], json!([{}, {"askFirst": true}]), "{imported}");
    // A file with no step waiting brings no hand-offs of its own.
    let mut plain = file.clone();
    plain.hand_offs[1].ask_first = None;
    let preview = io.preview_file_text(&routine_file_text(&plain), Some("ws_test"), &[]).unwrap();
    assert_eq!(preview.hand_off_asks, None);
    let _ = std::fs::remove_dir_all(&dir);
}

// case: buzzes the phone as pressing, naming who but never the routine or the step's words
#[test]
fn buzzes_the_phone_as_pressing_naming_who_but_never_the_routine_or_the_steps_words() {
    let alert = routine_ok_alert(Some("Wren"));
    assert_eq!(alert.kind, AlertKind::Approval);
    assert_eq!((alert.title.as_str(), alert.body.as_str(), alert.tag.as_str()), ("Wren waits for your OK", "A routine’s next step starts when you go ahead.", "waiting"));
    assert_eq!(routine_ok_alert(None).title, "A teammate waits for your OK");
}
