//! Electron's `a-comparison-runs-one-teammate-on-two-models.test.ts` (5 cases): one turn at a time per conversation
//! stands, but a comparison's column carries its own run slot, so one teammate answers on two models at once; each
//! column's own instructions reach its brief; and a second run in the same column is refused while its first is going.
use super::*;
use crate::compare::{CompareSlotId, compare_slot_key};

fn wren_with_standing_words(t: &Temp) {
    t.store()
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren", "role": "Code & Migrations",
                "instructions": "Saved standing words.", "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}}],
                "missionOwners": {}}),
        )
        .unwrap();
}

fn wren_turn(t: &Temp, prompt: &str, model: &str) -> Send {
    let mut mine = job(t, None, prompt);
    mine.teammate.id = "tm_wren".into();
    mine.teammate.name = "Wren".into();
    mine.model = Some(model.into());
    mine
}

fn column(key: String, instructions: Option<&str>) -> Origin {
    Origin {
        mode: Some(Mode::Ask),
        slot: Some(SlotOrigin {
            key,
            instructions: instructions.map(str::to_owned),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Every turn keeps its slot after it answers, until this is dropped: the turns are still going while the test looks.
struct Hold;
impl Drop for Hold {
    fn drop(&mut self) {
        hooks::HOLD_WIND_DOWN.store(false, Ordering::Relaxed);
    }
}
fn turns_wait(_t: &Temp) -> Hold {
    hooks::HOLD_WIND_DOWN.store(true, Ordering::Relaxed);
    Hold
}

fn turn_starts(t: &Temp) -> Vec<Value> {
    codex_requests(t)
        .into_iter()
        .filter(|r| r["method"] == "turn/start")
        .collect()
}

fn wait_for_turn_starts(t: &Temp, n: usize) {
    wait("every turn to reach its runtime", || {
        turn_starts(t).len() >= n
    });
}

fn release(hold: Hold, started: &[&Started]) {
    drop(hold);
    for s in started {
        finished(&s.conversation_id);
    }
}

// case: runs one teammate on two models at once, a column each
#[test]
fn runs_one_teammate_on_two_models_at_once_a_column_each() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    wren_with_standing_words(&t);
    let hold = turns_wait(&t);
    let first = send_from(
        wren_turn(&t, "Add a discount code field.", "gpt-6-luna"),
        column(
            compare_slot_key(Some("tm_wren"), "cmp_1", CompareSlotId::A),
            None,
        ),
    )
    .unwrap();
    let second = send_from(
        wren_turn(&t, "Add a discount code field.", "gpt-6-astra"),
        column(
            compare_slot_key(Some("tm_wren"), "cmp_1", CompareSlotId::B),
            None,
        ),
    );
    let second = second.unwrap();
    wait_for_turn_starts(&t, 2);
    assert!(
        owns(&first.conversation_id) && owns(&second.conversation_id),
        "both columns are going at once"
    );
    let mut models: Vec<String> = turn_starts(&t)
        .iter()
        .map(|r| r["params"]["model"].as_str().unwrap_or("").to_owned())
        .collect();
    models.sort();
    assert_eq!(
        models,
        ["gpt-6-astra", "gpt-6-luna"],
        "each column on its own model"
    );
    release(hold, &[&first, &second]);
}

// case: passes each column’s own instructions to its brief, including empty, without changing an ordinary turn
#[test]
fn passes_each_columns_own_instructions_to_its_brief_including_empty_without_changing_an_ordinary_turn()
 {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    wren_with_standing_words(&t);
    for (index, instructions) in [Some("Check the result."), Some(""), None]
        .into_iter()
        .enumerate()
    {
        let answer = send_from(
            wren_turn(&t, "One ask.", "synthetic-model"),
            column(format!("compare:trial:{index}"), instructions),
        )
        .unwrap();
        finished(&answer.conversation_id);
    }
    let normal = send_from(
        wren_turn(&t, "An ordinary turn.", "synthetic-model"),
        Origin {
            mode: Some(Mode::Ask),
            ..Default::default()
        },
    )
    .unwrap();
    finished(&normal.conversation_id);
    let told: Vec<String> = turn_starts(&t)
        .iter()
        .map(|r| r["params"]["input"][0]["text"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(told.len(), 4, "four turns were sent");
    let standing = "Saved standing words.";
    assert!(
        told[0].contains("Check the result.") && !told[0].contains(standing),
        "a column's words replace the teammate's: {}",
        told[0]
    );
    assert!(
        !told[1].contains(standing) && !told[1].contains("Check the result."),
        "an empty column has none: {}",
        told[1]
    );
    assert!(
        told[2].contains(standing),
        "a column with no words of its own keeps the teammate's: {}",
        told[2]
    );
    assert!(
        told[3].contains(standing) && !told[3].contains("Check the result."),
        "an ordinary turn is unchanged: {}",
        told[3]
    );
}

// case: starts a second conversation with the same teammate while one is going (0.551)
#[test]
fn starts_a_second_conversation_with_the_same_teammate_while_one_is_going() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    wren_with_standing_words(&t);
    let hold = turns_wait(&t);
    let one = send_from(
        wren_turn(&t, "One", "synthetic-model"),
        Origin {
            mode: Some(Mode::Ask),
            ..Default::default()
        },
    )
    .unwrap();
    let two = send_from(
        wren_turn(&t, "Two", "synthetic-model"),
        Origin {
            mode: Some(Mode::Ask),
            ..Default::default()
        },
    )
    .unwrap();
    wait_for_turn_starts(&t, 2);
    assert_ne!(one.conversation_id, two.conversation_id);
    assert!(
        owns(&one.conversation_id) && owns(&two.conversation_id),
        "both conversations are going at once"
    );
    release(hold, &[&one, &two]);
}

// case: runs on two models with no teammate at all -- none is needed to compare
#[test]
fn runs_on_two_models_with_no_teammate_at_all_none_is_needed_to_compare() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    let hold = turns_wait(&t);
    let work = t.0.join("work");
    let nobodys = |prompt: &str, slot: Option<CompareSlotId>, model: &str| {
        send_from(
            Send {
                teammate: nobody(Some("codex".into()), Some(model.into())),
                conversation: None,
                prompt: prompt.into(),
                model: None,
                effort: None,
            },
            Origin {
                mode: Some(Mode::Ask),
                folder: Some(work.clone()),
                runtime: Some("codex".into()),
                slot: slot.map(|s| SlotOrigin {
                    key: compare_slot_key(None, "cmp_1", s),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
    };
    // Two conversations of nobody's run side by side too (0.551).
    let one = nobodys("One", None, "synthetic-model").unwrap();
    let two = nobodys("Two", None, "synthetic-model").unwrap();
    let a = nobodys("Go", Some(CompareSlotId::A), "gpt-6-luna").unwrap();
    let b = nobodys("Go", Some(CompareSlotId::B), "gpt-6-astra").unwrap();
    wait_for_turn_starts(&t, 4);
    for s in [&one, &two, &a, &b] {
        assert!(
            owns(&s.conversation_id),
            "every one of nobody's turns is going at once"
        );
    }
    let requests = turn_starts(&t);
    for model in ["gpt-6-luna", "gpt-6-astra"] {
        assert_eq!(
            requests
                .iter()
                .filter(|r| r["params"]["model"] == model)
                .count(),
            1
        );
    }
    release(hold, &[&one, &two, &a, &b]);
}

// case: refuses a second run in the same column while its first is going
#[test]
fn refuses_a_second_run_in_the_same_column_while_its_first_is_going() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    wren_with_standing_words(&t);
    let hold = turns_wait(&t);
    let key = compare_slot_key(Some("tm_wren"), "cmp_1", CompareSlotId::A);
    let one = send_from(
        wren_turn(&t, "One", "synthetic-model"),
        column(key.clone(), None),
    )
    .unwrap();
    let before = ledgers(&t).len();
    let again = send_from(
        wren_turn(&t, "Two", "synthetic-model"),
        column(key.clone(), None),
    )
    .unwrap_err();
    assert_eq!(again, "A turn is already running in this conversation.");
    assert_eq!(
        ledgers(&t).len(),
        before,
        "nothing was recorded for the refused run"
    );
    // The other column of the same comparison is not refused.
    let other = send_from(
        wren_turn(&t, "Two", "synthetic-model"),
        column(
            compare_slot_key(Some("tm_wren"), "cmp_1", CompareSlotId::B),
            None,
        ),
    )
    .unwrap();
    release(hold, &[&one, &other]);
    // Once the first has ended, the column takes its next run.
    let next = send_from(wren_turn(&t, "Three", "synthetic-model"), column(key, None)).unwrap();
    finished(&next.conversation_id);
}
