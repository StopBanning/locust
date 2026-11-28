//! Version gates from a-card-answered-is-kept-with-its-turn.test.ts.
use super::*;
use crate::approval_record_note::{AnswerLedger, FileLedger};

fn answer_line(schema: u64, approval: Value) -> Value {
    json!({"schemaVersion": schema, "recordType": "mission.approval", "ledgerSequence": 2,
        "occurredAt": approval["occurredAt"], "approval": approval})
}

// case: is refused on a mission written before version 20, which its own readers would stop at
#[test]
fn an_old_writer_refuses_a_card_answer_on_v19() {
    let (_t, store) = temp("approval-v19-writer");
    let path = older(&store, 19, &[]);
    let before = std::fs::read(&path).unwrap();
    let refusal = FileLedger { path: path.clone() }.append_approval("mission_1", &allowed()).unwrap_err();
    assert!(refusal.contains("cannot hold approvals"), "{refusal}");
    assert_eq!(std::fs::read(path).unwrap(), before);
}

fn assert_damaged(schema: u64, approval: Value) {
    let (_t, store) = temp("approval-old-reader");
    let path = older(&store, schema, &[answer_line(schema, approval)]);
    let read = recovered(&path);
    assert_eq!(read["approvals"], json!([]));
    assert_eq!(read["issues"], json!(["invalid-record"]));
}

// case: is read as damage in a file written before version 20
#[test]
fn a_card_answer_in_v19_is_damage() {
    assert_damaged(19, allowed());
}

// case: keeps an answer by an earlier Always on a v21 mission, beside the card that gave it
#[test]
fn a_v21_writer_keeps_the_earlier_always_beside_its_card() {
    let (_t, store) = temp("approval-v21-writer");
    let path = older(&store, 21, &[]);
    let mut writer = FileLedger { path: path.clone() };
    let mut always = allowed();
    always["answer"] = json!("allowed-always");
    writer.append_approval("mission_1", &always).unwrap();
    writer.append_approval("mission_1", &by_earlier_always()).unwrap();
    let read = recovered(&path);
    assert_eq!(read["schemaVersion"], 21);
    assert_eq!(read["approvals"], json!([always, by_earlier_always()]));
    assert_eq!(read["issues"], json!([]));
}

// case: refuses an answer by an earlier Always on a v20 mission, whose readers would stop at it
#[test]
fn a_v20_writer_allows_card_answers_but_refuses_an_earlier_always() {
    let (_t, store) = temp("approval-v20-writer");
    let path = older(&store, 20, &[]);
    let mut writer = FileLedger { path: path.clone() };
    writer.append_approval("mission_1", &allowed()).unwrap();
    let refusal = writer.append_approval("mission_1", &by_earlier_always()).unwrap_err();
    assert!(refusal.contains("cannot hold an answer by an earlier Always"), "{refusal}");
    assert_eq!(recovered(&path)["approvals"], json!([allowed()]));
}

// case: reads an answer by an earlier Always in a v20 file as damage, not as an answer
#[test]
fn an_earlier_always_in_v20_is_damage() {
    assert_damaged(20, by_earlier_always());
}
