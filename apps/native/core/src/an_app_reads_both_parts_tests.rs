//! Electron's `main/an-app-reads-the-answer-in-both-parts-of-the-reply.test.ts` (0.882):
//! read_reply carries the same words in `structuredContent.message` as in the text part, so a client that shows the
//! structured part in place of the text still gets the answer, the failure, the activity or the wait for the person.
use super::*;

fn answer(text: &str) -> Value {
    json!({
        "id": "run_1:codex:1",
        "runId": "run_1",
        "missionId": "mission_1",
        "sequence": 1,
        "occurredAt": "2026-11-22T12:00:00.000Z",
        "sourceAdapter": "codex",
        "type": "message.delta",
        "payload": {"itemId": "item_1", "operation": "replace", "text": text, "final": true}
    })
}

fn door_with(t: Turn, live: bool, waiting: bool) -> Tools<Fake> {
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert(t.mission_id.clone(), t.clone());
    if live {
        fake.live.lock().unwrap().insert(t.mission_id.clone());
    }
    if waiting {
        fake.waiting.lock().unwrap().insert(t.mission_id);
    }
    Tools::new(fake)
}

fn read(tools: &Tools<Fake>) -> (String, Value) {
    let result = tools.call("read_reply", &json!({"conversation_id": "mission_1"}));
    let wire = result.to_json();
    let text = wire["content"][0]["text"].as_str().unwrap().to_owned();
    let structured = wire["structuredContent"].clone();
    assert_eq!(text, result.text);
    assert_eq!(structured, result.structured.unwrap());
    (text, structured)
}

// case: gives a finished answer in the structured part too, beside its status
#[test]
fn gives_a_finished_answer_in_the_structured_part_too_beside_its_status() {
    let t = turn("mission_1", true, vec![answer("The four rows are carried."), ev("run.completed", json!({}))]);
    let (text, structured) = read(&door_with(t, false, false));
    assert_eq!(text, "The four rows are carried.");
    assert_eq!(structured["status"], "done");
    assert_eq!(structured["message"], "The four rows are carried.");
}

// case: gives the failure, the activity and the wait for the person in both parts too
#[test]
fn gives_the_failure_the_activity_and_the_wait_for_the_person_in_both_parts_too() {
    // A host failure record read from the ledger carries its message in both parts.
    let failed_records = vec![
        json!({"recordType": "mission.created", "occurredAt": "2026-11-22T12:00:00.000Z", "metadata": {"missionId": "mission_1", "prompt": "Go", "runtime": "codex", "startedBy": {"kind": "mcp"}}}),
        json!({"recordType": "mission.host_failure", "occurredAt": "2026-11-22T12:00:01.000Z", "failure": {"message": "Codex ran out of usage."}}),
    ];
    let failed_turn = Turn::from_records(&failed_records).unwrap();
    let (failed_text, failed_structured) = read(&door_with(failed_turn, false, false));
    assert_eq!(failed_text, "Codex ran out of usage.");
    assert_eq!(failed_structured["status"], "failed");
    assert_eq!(failed_structured["message"], failed_text);

    let (working_text, working_structured) = read(&door_with(turn("mission_1", true, vec![]), true, false));
    assert!(working_text.starts_with("Still working"), "{working_text}");
    assert_eq!(working_structured["status"], "working");
    assert_eq!(working_structured["message"], working_text);

    let (waiting_text, waiting_structured) = read(&door_with(turn("mission_1", true, vec![]), true, true));
    assert!(waiting_text.contains("approve or deny"), "{waiting_text}");
    assert_eq!(waiting_structured["status"], "waiting_for_person");
    assert_eq!(waiting_structured["message"], waiting_text);
}
