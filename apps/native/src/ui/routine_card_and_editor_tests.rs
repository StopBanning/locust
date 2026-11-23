//! Routines round 2 (19 Nov), drawn headless: a-routine-names-its-folder.test.tsx,
//! a-routine-says-what-it-may-do.test.tsx and the row and editor parts of
//! a-routine-waiting-for-your-ok-asks-plainly.test.tsx.
use super::*;
use rune::{Env, Runtime};

struct Folder(std::path::PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn folder(name: &str) -> Folder {
    let root = std::env::temp_dir().join(format!("locust-routine-folder-{}-{}", std::process::id(), name));
    let dir = root.join("sol-long-project");
    std::fs::create_dir_all(&dir).unwrap();
    Folder(root)
}
fn project(f: &Folder) -> std::path::PathBuf {
    f.0.join("sol-long-project").canonicalize().unwrap()
}

fn routine(workspace: Option<&std::path::Path>) -> Value {
    let mut r = json!({
        "routineId": "rt_1", "name": "Run the tests", "teammateId": "codec",
        "route": {"runtime": "codex", "model": "gpt-6.1-sol", "mode": "accept-edits"},
        "steps": ["npm test"], "learnedFrom": [], "createdAt": "2026-09-30T21:00:00.000Z", "runs": 0,
        "schedule": {"kind": "daily", "at": "09:00"}
    });
    if let Some(w) = workspace {
        r["workspaceId"] = json!(locust_core::people::workspace_id(w));
    }
    r
}

fn draw(s: State) -> Vec<rune::ElementInfo> {
    let mut rt = Runtime::new(s, crate::view, (1200.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}
fn shown(all: &[rune::ElementInfo], key: &str) -> Option<String> {
    all.iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone())
}
fn screen(routines: Vec<Value>, teammate_folder: Option<&std::path::Path>) -> State {
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.teammates[0].folder = teammate_folder.map(|p| p.to_string_lossy().into_owned());
    s.routines.open = true;
    s.routines.file = json!({"schemaVersion": 1, "routines": routines});
    s
}

// ---- a routine names the folder it runs in ----------------------------------------------------------------------

#[test]
fn on_its_card_with_the_whole_path_on_hover() {
    let f = folder("card");
    let dir = project(&f);
    let all = draw(screen(vec![routine(Some(&dir))], Some(&dir)));
    assert_eq!(shown(&all, "routine-folder-rt_1").as_deref(), Some(" · in sol-long-project"));
    // The hover is the whole path: the same pair the card draws, read where it is made.
    let known = vec![dir.clone()];
    assert_eq!(
        locust_core::routines::routine_folder(&routine(Some(&dir)), &known),
        Some(("sol-long-project".to_owned(), dir.to_string_lossy().into_owned()))
    );
}

#[test]
fn and_says_nothing_it_does_not_know() {
    let f = folder("unknown");
    let dir = project(&f);
    // A folder Locust does not know, and a routine that names none.
    let all = draw(screen(vec![routine(Some(&dir))], None));
    assert!(shown(&all, "routine-folder-rt_1").is_none());
    assert!(!all.iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains(" · in "))));
    assert_eq!(locust_core::routines::routine_folder(&routine(None), &[dir]), None);
}

#[test]
fn and_in_its_editor_beside_when_it_runs() {
    let f = folder("editor");
    let dir = project(&f);
    let mut s = screen(vec![], Some(&dir));
    edit(&mut s, routine(Some(&dir)));
    let all = draw(s);
    assert_eq!(shown(&all, "routine-schedule-folder").as_deref(), Some(" On a schedule it runs in sol-long-project."));
    assert!(all.iter().any(|e| e.text.as_deref().is_some_and(|t| t.starts_with("Only while Locust is open, and only when Codec is free."))));
}

// ---- a routine says what it may do ------------------------------------------------------------------------------

fn file_routine(mode: &str) -> State {
    let mut s = screen(vec![], None);
    let mut r = routine(None);
    r["name"] = json!("Inbox receipt");
    r["steps"] = json!(["Write a receipt for the file."]);
    r["schedule"] = json!({"kind": "files", "folder": "inbox"});
    r["route"]["mode"] = json!(mode);
    edit(&mut s, r);
    s
}
fn chosen(all: &[rune::ElementInfo], key: &str) -> bool {
    all.iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label.clone()).is_some_and(|l| l.ends_with(", chosen"))
}
fn any_text(all: &[rune::ElementInfo], words: &str) -> bool {
    all.iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains(words)))
}

#[test]
fn in_edit_change_files_is_chosen_and_the_file_trigger_says_what_lands_in_the_folder() {
    let all = draw(file_routine("accept-edits"));
    assert!(any_text(&all, "What a run may do"));
    assert!(chosen(&all, "routine-may-change") && !chosen(&all, "routine-may-read"));
    assert!(any_text(&all, "What a run changes lands in the folder straight away"));
    assert!(any_text(&all, "It may change files, and what it changes lands in the folder straight away."));
    assert!(!any_text(&all, "changes nothing, as every routine runs in Ask"));
    assert_eq!(shown(&all, "routine-runs-it").as_deref(), Some("Codec runs it, in Edit."));
}

#[test]
fn in_ask_only_read_is_chosen_and_the_trigger_says_it_changes_nothing() {
    let all = draw(file_routine("ask"));
    assert!(chosen(&all, "routine-may-read") && !chosen(&all, "routine-may-change"));
    assert!(any_text(&all, "It reads the file and changes nothing."));
    assert!(any_text(&all, "nothing in the folder changes"));
    assert!(!any_text(&all, "It may change files"));
    // A run that only reads is not asked where it changes things.
    assert!(shown(&all, "routine-in-copy-true").is_none());
}

#[test]
fn the_choice_is_saved_as_the_mode_only_when_it_moved() {
    // Moved, it is written as the mode; left where it was, the saved mode stays exactly as it is.
    assert_eq!(mode_to_save(Some(false), Some(true)), Some("accept-edits"));
    assert_eq!(mode_to_save(Some(true), Some(false)), Some("ask"));
    assert_eq!(mode_to_save(Some(true), Some(true)), None);
    assert_eq!(mode_to_save(Some(false), Some(false)), None);
    assert_eq!(mode_to_save(None, None), None);
    let mut s = file_routine("ask");
    s.routines.reads_only = Some(false);
    assert_eq!(shown_mode(&s), Some("Edit"));
    let all = draw(s);
    assert_eq!(shown(&all, "routine-runs-it").as_deref(), Some("Codec runs it, in Edit."));
    assert!(any_text(&all, "It may change files, and what it changes lands in the folder straight away."));
    // A routine not saved yet has no mode, and the choice is not offered or said.
    let mut fresh = screen(vec![], None);
    edit(&mut fresh, json!({"name": "", "steps": [""], "teammateId": "codec"}));
    assert_eq!(fresh.routines.reads_only, None);
    let all = draw(fresh);
    assert!(!any_text(&all, "What a run may do"));
}

// ---- a routine waiting for your OK ------------------------------------------------------------------------------

fn waiting() -> Value {
    json!({
        "routineId": "rt_ship", "name": "Ship the fix", "teammateId": "codec",
        "route": {"runtime": "codex", "model": "gpt-6.1-sol", "mode": "ask"},
        "steps": ["Fix the failing test.", "Push the branch and open a pull request."], "learnedFrom": [],
        "createdAt": "2026-10-28T00:00:00.000Z", "runs": 3, "handOffs": [{}, {"askFirst": true}],
        "execution": {"attemptId": "attempt_one", "status": "held", "step": 1, "of": 2,
            "startedAt": "2026-10-28T09:00:00.000Z", "updatedAt": "2026-10-28T09:05:00.000Z",
            "steps": ["Fix the failing test.", "Push the branch and open a pull request."],
            "route": {"runtime": "codex", "model": "gpt-6.1-sol", "mode": "ask"}, "missionId": "mission_one",
            "reason": "Step 2 waits for your OK. Read what step 1 did, then go ahead or stop here.",
            "canContinue": true, "settledAtDispatch": true, "waitsForOk": true}
    })
}

#[test]
fn says_waiting_for_your_ok_on_the_routine_row() {
    assert_eq!(locust_core::routines::run_summary(&waiting()), "2 steps · waiting for your OK · 3 completed runs");
    let mut daily = waiting();
    daily["schedule"] = json!({"kind": "daily", "at": "09:00"});
    let all = draw(screen(vec![daily], None));
    assert!(shown(&all, "routine-chain-rt_ship").is_some_and(|t| t.ends_with(" · 2 steps · waiting for your OK · 3 completed runs")));
    assert_eq!(shown(&all, "routine-when-rt_ship").as_deref(), Some("daily at 9:00 AM · waiting for your OK"));
}

#[test]
fn asks_with_the_next_step_in_its_own_words_and_two_answers_ready_to_press() {
    let all = draw(screen(vec![waiting()], None));
    assert!(any_text(&all, "Waiting for your OK · Step 2 of 2"));
    assert!(any_text(&all, "Step 2: Push the branch and open a pull request."));
    assert!(all.iter().any(|e| e.key.as_deref() == Some("routine-go-ahead-rt_ship") && e.text.as_deref() == Some("Go ahead with step 2")));
    assert!(all.iter().any(|e| e.key.as_deref() == Some("routine-stop-rt_ship") && e.text.as_deref() == Some("Stop here")));
    assert!(!any_text(&all, "I reviewed"));
    assert!(!any_text(&all, "Waiting for your review"));
    assert!(any_text(&all, "the routine runs again from step 1 when you press Run"));
}

#[test]
fn a_held_routine_that_did_not_ask_is_still_the_recovery_card() {
    let mut held = waiting();
    held["execution"].as_object_mut().unwrap().remove("waitsForOk");
    let all = draw(screen(vec![held.clone()], None));
    assert!(any_text(&all, "Waiting for your review"));
    assert!(any_text(&all, "I reviewed"));
    assert_eq!(locust_core::routines::run_summary(&held), "2 steps · waiting for review · 3 completed runs");
}

#[test]
fn the_other_run_summaries_read_as_electrons() {
    let mut r = routine(None);
    assert_eq!(locust_core::routines::run_summary(&r), "1 step · not run yet");
    r["runs"] = json!(1);
    assert_eq!(locust_core::routines::run_summary(&r), "1 step · run 1 time");
    r["runs"] = json!(4);
    assert_eq!(locust_core::routines::run_summary(&r), "1 step · run 4 times");
    r["execution"] = json!({"status": "running"});
    assert_eq!(locust_core::routines::run_summary(&r), "1 step · in progress · 4 completed runs");
    r["execution"] = json!({"status": "abandoned"});
    assert_eq!(locust_core::routines::run_summary(&r), "1 step · last attempt abandoned · 4 completed runs");
}

#[test]
fn is_set_per_step_from_step_2_on_in_the_routine_editor_for_one_teammate_too() {
    let mut s = screen(vec![], None);
    s.teammates.truncate(1);
    edit(&mut s, waiting());
    let all = draw(s);
    assert!(!all.iter().any(|e| e.key.as_deref() == Some("routine-step-ok-0")), "step 1 has nothing to wait after");
    assert!(all.iter().any(|e| e.key.as_deref() == Some("routine-step-ok-1") && e.text.as_deref().is_some_and(|t| t.contains("Wait for my OK first"))));
    // And it is kept for one teammate: the store keeps askFirst from step 2 on, with no hand-off to anyone.
    assert_eq!(
        locust_core::routines::progress::kept_hand_offs(&json!([{"askFirst": true}, {"askFirst": true}]), "codec"),
        Some(json!([{}, {"askFirst": true}]))
    );
}

// ---- routines round 3: routine-recovery.test.tsx's screen cases, and when Run can be pressed ---------------------

fn held() -> Value {
    let route = json!({"runtime": "codex", "model": "account-default", "mode": "ask"});
    json!({
        "routineId": "rt_one", "name": "Release", "teammateId": "codec", "route": route,
        "steps": ["Open PR", "Summarise", "Report"], "learnedFrom": [], "createdAt": "2026-09-01T00:00:00.000Z", "runs": 0,
        "schedule": {"kind": "every", "hours": 6},
        "execution": {"attemptId": "attempt_one", "status": "held", "step": 2, "of": 3, "workspaceId": "ws_test",
            "startedAt": "2026-09-04T00:00:00.000Z", "updatedAt": "2026-09-04T00:00:00.000Z",
            "steps": ["Open PR", "Summarise", "Report"], "route": route,
            "missionId": "mission_two", "runId": "run_two", "reason": "The dispatch outcome is uncertain.", "canContinue": false}
    })
}

#[test]
fn the_routines_screen_draws_a_held_attempt_from_its_saved_record_with_run_off() {
    // Read back from the file as written: nothing but the saved record, no notice from a run.
    let saved: Value = serde_json::from_str(&held().to_string()).unwrap();
    let s = screen(vec![saved.clone()], None);
    assert!(s.routines.notices.is_empty());
    let all = draw(s);
    assert!(any_text(&all, "Waiting for your review · Step 2 of 3 · Attempt started"));
    assert!(shown(&all, "routine-when-rt_one").is_some_and(|t| t.ends_with("no automatic retry")));
    assert!(!any_text(&all, "not run yet") && !any_text(&all, "due now"));
    // Run is off while the attempt waits: drawn faded, and pressing it starts nothing.
    let screen_state = screen(vec![saved.clone()], None);
    assert_eq!(run_refusal(&screen_state, &saved), Some(None));
    let run = all.iter().find(|e| e.key.as_deref() == Some("routine-run-rt_one")).expect("Run is drawn");
    assert!(run.opacity < 0.5, "Run is drawn off while the attempt waits");
}

#[test]
fn run_is_off_while_changes_wait_or_the_teammate_is_gone_and_says_why() {
    let s = screen(vec![], None);
    let mut staged = routine(None);
    staged["staged"] = json!({"files": ["a.md"]});
    assert_eq!(run_refusal(&s, &staged), Some(Some("Its last run's changes are waiting: Keep or Discard them first.")));
    let mut orphan = routine(None);
    orphan["teammateId"] = json!("tm_gone");
    assert_eq!(run_refusal(&s, &orphan), Some(Some("The teammate this was taught to is gone, so it has no route to run on.")));
    let mut abandoned = held();
    abandoned["execution"]["status"] = json!("abandoned");
    assert_eq!(run_refusal(&s, &abandoned), None, "an abandoned attempt leaves Run free");
    assert_eq!(run_refusal(&s, &routine(None)), None);
}

#[test]
fn the_held_attempt_is_a_card_of_its_own_never_the_rows_meta_line() {
    let all = draw(screen(vec![held()], None));
    let chain = shown(&all, "routine-chain-rt_one").unwrap();
    assert!(!chain.contains("Waiting for your review") && !chain.contains("I reviewed"), "{chain}");
    // Its own lines: the heading, the reason and the review box each wrap at the card's width.
    let heading = all.iter().find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("Waiting for your review"))).unwrap();
    let card = all.iter().find(|e| e.key.as_deref() == Some("routine-rt_one")).unwrap();
    assert!(heading.rect[2] > card.rect[2] * 0.5, "the heading has the card's width, not a clipped cell");
}

// ---- a-routine-keeps-what-you-keep.test.tsx ---------------------------------------------------------------------

fn keeps(mode: &str, in_copy: bool) -> State {
    let mut s = file_routine(mode);
    s.routines.entry["inCopy"] = json!(in_copy);
    s
}

#[test]
fn offers_the_copy_only_to_a_routine_that_may_change_files() {
    assert!(any_text(&draw(keeps("accept-edits", false)), "Where it works"));
    let ask = draw(keeps("ask", false));
    assert!(!any_text(&ask, "Where it works"));
    assert!(shown(&ask, "routine-in-copy-true").is_none());
    // Nor is it asked what happens when its steps are done: a run that only reads has nothing to fix.
    assert!(!any_text(&ask, "When its steps are done"));
    assert!(any_text(&draw(keeps("accept-edits", false)), "When its steps are done"));
}

#[test]
fn in_a_copy_every_sentence_says_nothing_lands_until_you_keep_it() {
    let all = draw(keeps("accept-edits", true));
    assert!(chosen(&all, "routine-in-copy-true") && !chosen(&all, "routine-in-copy-false"));
    assert!(any_text(&all, "It changes files in a copy of the folder. When a run finishes, its changes wait under Routines"));
    assert!(any_text(&all, "It may change files in its copy; nothing lands in the folder until you keep it."));
    assert!(!any_text(&all, "lands in the folder straight away"));
}

#[test]
fn in_the_folder_it_says_the_change_lands_straight_away() {
    let all = draw(keeps("accept-edits", false));
    assert!(chosen(&all, "routine-in-copy-false"));
    assert!(any_text(&all, "lands in the folder straight away"));
}

#[test]
fn with_no_check_command_says_where_to_set_one_and_cannot_be_chosen() {
    let s = keeps("accept-edits", false);
    assert_eq!(s.routines.check_command, None);
    let all = draw(s);
    assert!(any_text(&all, "This folder has no check command yet. Set one in Settings > Project folder"));
    let goal = all.iter().find(|e| e.key.as_deref() == Some("routine-goal")).unwrap();
    assert!(goal.opacity < 0.5, "drawn off");
    // Pressing it chooses nothing; the control: with a check command the same press chooses three fixes.
    let press = |s: State| {
        let mut rt = Runtime::new(s, crate::view, (1200.0, 6000.0), Env::new(true, 1.0));
        rt.frame(0.016);
        let (x, y) = rt.find("routine-goal").expect("drawn");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.032);
        rt.state.routines.entry["untilCheck"]["tries"].as_u64()
    };
    assert_eq!(press(keeps("accept-edits", false)), None);
    let mut with_check = keeps("accept-edits", false);
    with_check.routines.check_command = Some("npm test".into());
    assert_eq!(press(with_check), Some(3));
}

#[test]
fn chosen_it_names_the_command_the_fixes_it_may_make_and_that_each_is_one_more_turn() {
    let mut s = keeps("accept-edits", false);
    s.routines.check_command = Some("npm test".into());
    s.routines.entry["untilCheck"] = json!({"tries": 3});
    let all = draw(s);
    assert!(chosen(&all, "routine-goal"));
    assert_eq!(
        shown(&all, "routine-goal-note").as_deref(),
        Some("When its steps are done it runs `npm test`. While that fails, it is asked to fix what it says, up to 3 times; then it stops and says what still fails. Each fix is one more turn.")
    );
    let mut one = keeps("accept-edits", false);
    one.routines.check_command = Some("npm test".into());
    one.routines.entry["untilCheck"] = json!({"tries": 1});
    assert!(shown(&draw(one), "routine-goal-note").is_some_and(|t| t.contains("up to 1 time;")));
}

#[test]
fn names_what_changed_says_nothing_has_reached_the_folder_and_offers_keep_discard_and_the_copy() {
    let mut r = routine(None);
    r["routineId"] = json!("rt_inbox");
    r["inCopy"] = json!(true);
    r["staged"] = json!({"attemptId": "a1", "finishedAt": "2026-10-01T10:00:00.000Z", "folder": "/work",
        "changed": ["receipts/a.md", "receipts/b.md", "notes.md", "x.md", "y.md"], "deleted": ["old.md"]});
    let all = draw(screen(vec![r.clone()], None));
    assert!(any_text(&all, "its last run changed 6 files in its copy"));
    assert!(any_text(&all, "receipts/a.md, receipts/b.md, notes.md, x.md and 2 more"));
    assert!(any_text(&all, "Nothing has reached the folder."));
    for key in ["routine-keep-rt_inbox", "routine-discard-rt_inbox", "routine-open-copy-rt_inbox"] {
        assert!(all.iter().any(|e| e.key.as_deref() == Some(key)), "{key}");
    }
    // Draws nothing when nothing waits.
    r.as_object_mut().unwrap().remove("staged");
    let quiet = draw(screen(vec![r], None));
    assert!(!quiet.iter().any(|e| e.key.as_deref() == Some("routine-changes-rt_inbox")));
}

// ---- a-routine-previews-its-inputs-and-file.test.tsx ------------------------------------------------------------

fn notes() -> Value {
    json!({"routineId": "rt_one", "name": "Notes", "teammateId": "codec", "route": {"runtime": "opencode", "model": "free", "mode": "ask"},
        "steps": ["Read {{topic}} in {{folder}}"], "learnedFrom": [], "createdAt": "2026-10-03T00:00:00Z", "runs": 0,
        "inputs": [{"key": "topic", "label": "Topic", "kind": "text", "required": true, "default": "Daily notes"},
            {"key": "folder", "label": "Folder", "kind": "folder", "required": true},
            {"key": "tone", "label": "Tone", "kind": "choice", "required": false, "choices": ["Brief", "Detailed"], "default": "Brief"},
            {"key": "note", "label": "Note", "kind": "long-text", "required": false, "default": "two\nlines"}]})
}
fn labelled(all: &[rune::ElementInfo], label: &str) -> bool {
    all.iter().any(|e| e.label.as_deref() == Some(label))
}

#[test]
fn fills_defaults_draws_each_kind_and_a_folder_has_a_picker_with_no_typed_path_field() {
    let mut s = screen(vec![notes()], None);
    start_run(&mut s, "rt_one");
    let dlg = s.routines.run_dialog.as_ref().expect("Run asks for the values first");
    assert_eq!(dlg.values.iter().map(|v| v.text.clone()).collect::<Vec<_>>(), ["Daily notes", "", "Brief", "two\nlines"]);
    let all = draw(s);
    assert!(any_text(&all, "Run Notes"));
    assert!(labelled(&all, "Value for Topic") && labelled(&all, "Value for Note"));
    assert!(all.iter().any(|e| e.key.as_deref() == Some("routine-run-choice-2-0") && e.fill != rune::Color::TRANSPARENT), "Brief is chosen");
    assert!(all.iter().any(|e| e.text.as_deref() == Some("Choose Folder")));
    assert!(!labelled(&all, "Value for Folder"), "a folder is chosen with the picker, never typed");
    assert!(any_text(&all, "No folder chosen"));
    let run = all.iter().find(|e| e.key.as_deref() == Some("routine-run-submit")).unwrap();
    assert_eq!(run.text.as_deref(), Some("Run routine"));
    assert!(run.opacity < 0.5, "Run routine waits for the folder");
    assert!(shown(&all, "routine-run-unsettled").is_some(), "and says what is missing");
}

#[test]
fn the_routine_editor_shows_declarations_and_refuses_a_step_with_an_undeclared_input() {
    let mut s = screen(vec![], None);
    let mut r = notes();
    r["steps"] = json!(["Read {{missing}}"]);
    edit(&mut s, r);
    assert!(save_refusal(&s).is_some_and(|why| why.contains("not declared as an input")));
    let all = draw(s);
    assert!(labelled(&all, "Input 1 key"));
    assert!(any_text(&all, "not declared as an input"));
    assert!(any_text(&all, "Add input"));
    assert!(all.iter().any(|e| e.key.as_deref() == Some("routine-save")));
    // Save waits: pressed, it does nothing (with no data folder here, a save that ran would say so).
    let mut s = screen(vec![], None);
    let mut r = notes();
    r["steps"] = json!(["Read {{missing}}"]);
    edit(&mut s, r);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.016);
    let (x, y) = rt.find("routine-save").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.032);
    assert!(rt.state.routines.error.is_empty() && !rt.state.routines.fields.is_empty(), "nothing was saved or tried");
}

fn import_state(from_template: bool) -> State {
    let mut s = screen(vec![], None);
    let inputs = locust_core::routine_inputs::parse_inputs_array(&notes()["inputs"]).unwrap();
    s.routines.import_preview = Some(ImportPreviewState {
        preview: locust_core::routine_io::RoutineImportPreview {
            token: "token".into(), name: "Notes".into(), steps: vec!["Read {{topic}} in {{folder}}".into()], inputs,
            hand_off_roles: vec![Some("Finance Bro".into())], hand_off_checks: vec![false], hand_off_asks: None,
            changes_files: false, hand_off_on_another_agent: None, runtime: Some("opencode".into()),
            connectors: vec![
                locust_core::routine_io::ConnectorPresence { name: "available".into(), present: true },
                locust_core::routine_io::ConnectorPresence { name: "needed".into(), present: false },
            ],
        },
        teammate_id: String::new(),
        error: String::new(),
        from_template,
    });
    s
}

#[test]
fn the_import_preview_names_steps_inputs_choices_source_roles_and_present_or_missing_connectors() {
    let all = draw(import_state(false));
    for saying in ["Read {{topic}} in {{folder}}", "Daily notes", "Brief, Detailed", "Source role: Finance Bro", "available · present", "needed · missing", "Give it to", "with no schedule, and runs nothing", "Made for OpenCode."] {
        assert!(any_text(&all, saying), "{saying}");
    }
    assert!(labelled(&all, "Give routine to"));
    let import = all.iter().find(|e| e.key.as_deref() == Some("routine-import-confirm")).unwrap();
    assert!(import.opacity < 0.5, "Import waits for an owner");
    // Chosen, it can be pressed.
    let mut s = import_state(false);
    s.routines.import_preview.as_mut().unwrap().teammate_id = "codec".into();
    let all = draw(s);
    assert!(chosen(&all, "routine-import-owner-codec"));
    assert!(all.iter().find(|e| e.key.as_deref() == Some("routine-import-confirm")).unwrap().opacity > 0.9);
    // A starter is added, not imported, with its own words.
    let starter = draw(import_state(true));
    assert!(any_text(&starter, "Add routine") && any_text(&starter, "runs nothing until you press Run."));
}

#[test]
fn export_paths_are_shown_with_a_conversion_offer_and_the_routines_screen_has_file_actions() {
    let mut s = screen(vec![notes()], None);
    s.routines.export_dialog = Some(ExportDialogState { routine: notes(), paths: vec!["Step 1: C:\\private\\folder".into()] });
    let all = draw(s);
    assert!(any_text(&all, "C:\\private\\folder"));
    assert!(any_text(&all, "Make paths inputs") && any_text(&all, "Keep paths"));
    assert!(any_text(&all, "Import routine"));
    assert!(labelled(&all, "Export Notes"));
}

#[test]
fn a_scheduled_routine_in_approve_each_says_on_its_card_that_it_waits_for_you() {
    let mut r = routine(None);
    r["route"]["mode"] = json!("approve-each");
    let all = draw(screen(vec![r.clone()], None));
    assert_eq!(shown(&all, "routine-waits-rt_1").as_deref(), locust_core::routines::waits_for_you(&r));
    r["route"]["mode"] = json!("accept-edits");
    assert!(shown(&draw(screen(vec![r], None)), "routine-waits-rt_1").is_none());
}
