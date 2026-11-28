//! mission-store's `unreadable-count.test.ts`: how many record files produced no conversation, counted where that is
//! known, in the reader (`HistoryCache::list_recent_with_damage`'s `Damage::unreadable`, which `native::list_recent`
//! says under the list). Never derived from the page: a conversation that merely fell off the page is not unreadable,
//! and a recovered one with a damaged body is not either (Astra, 2026-09-09).
use super::*;
use crate::ledger::torn_ledger_tests::Folder;
use serde_json::json;

/// A record as the host writes one: its header and one event. Read back below, so a header the reader would not take
/// fails the listing rather than passing as "unreadable".
fn record(id: &str, created_at: &str) -> String {
    let header = json!({ "schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": created_at,
        "metadata": { "missionId": id, "runId": format!("run_{id}"), "prompt": "do the thing", "runtime": "codex",
            "model": "gpt-6-astra", "requestedRouteId": "codex:account-default", "resolvedRouteId": "codex:gpt-6-astra",
            "cliVersion": null, "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1,
            "createdAt": created_at } });
    let event = json!({ "schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": created_at,
        "event": { "id": "e1", "runId": format!("run_{id}"), "missionId": id, "sequence": 1, "occurredAt": created_at,
            "sourceAdapter": "codex", "type": "run.completed", "payload": {} } });
    format!("{header}\n{event}\n")
}

// History chooses the page by file mtime, not by the dates inside the JSON.
fn set_file_order(path: &Path, index: usize) {
    let at = UNIX_EPOCH + std::time::Duration::from_secs(60 * (1 + index as u64));
    fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
}

/// `count` clean records, m_00 the oldest, each file newer than the one before.
fn ledger_of(folder: &Folder, count: usize, day: &str) {
    for index in 0..count {
        let id = format!("m_{index:02}");
        let path = folder.write(&id, &record(&id, &format!("2026-09-{day}T00:00:{index:02}.000Z")), false);
        set_file_order(&path, index + 1);
    }
}

fn listed(folder: &Folder, limit: usize) -> (Vec<String>, Damage) {
    let (rows, damage) = HistoryCache::new().list_recent_with_damage(&folder.0, limit).expect("the folder is read");
    (rows.into_iter().map(|s| s.mission_id).collect(), damage)
}

// case: is zero when every file recovered, however many are on the page
#[test]
fn is_zero_when_every_file_recovered_however_many_are_on_the_page() {
    let folder = Folder::new();
    ledger_of(&folder, 21, "08");
    let (page, damage) = listed(&folder, 5);
    assert_eq!(page.len(), 5);
    assert_eq!(damage.unreadable, 0);
    // The control: unpaged, the same records say the same zero.
    let (whole, damage) = listed(&folder, 30);
    assert_eq!(whole.len(), 21);
    assert_eq!(damage.unreadable, 0);
}

// case: does not call a DAMAGED but recovered mission unreadable when it falls off the page
#[test]
fn does_not_call_a_damaged_but_recovered_mission_unreadable_when_it_falls_off_the_page() {
    let folder = Folder::new();
    ledger_of(&folder, 1, "08");
    // Body damage on the OLDEST: header intact, then garbage.
    let oldest = folder.0.join("m_00.jsonl");
    let mut text = std::fs::read_to_string(&oldest).unwrap();
    text.push_str("not a record at all\n");
    std::fs::write(&oldest, text).unwrap();
    set_file_order(&oldest, 0);
    for index in 1..=20 {
        let id = format!("m_{index:02}");
        let path = folder.write(&id, &record(&id, &format!("2026-09-09T00:00:{index:02}.000Z")), false);
        set_file_order(&path, index);
    }
    let (page, damage) = listed(&folder, 20);
    assert_eq!(page.len(), 20);
    // It raised an issue, and it is not on the page.
    assert!(!ledger::read_ledger(&oldest).unwrap().issues.is_empty());
    assert!(!page.iter().any(|id| id == "m_00"));
    // It recovered, so nothing here was unreadable.
    let (whole, _) = listed(&folder, 30);
    assert!(whole.iter().any(|id| id == "m_00"));
    assert_eq!(damage.unreadable, 0);
}

// case: counts a file whose header is unreadable, and only that file
#[test]
fn counts_a_file_whose_header_is_unreadable_and_only_that_file() {
    let folder = Folder::new();
    ledger_of(&folder, 1, "08");
    folder.write("m_bad", "this is not json at all\n", false);
    let (page, damage) = listed(&folder, 10);
    assert_eq!(page, ["m_00"]);
    assert_eq!(damage.unreadable, 1);
    // Counted however far past the page it sits: older than three good records, under a page of one.
    let folder = Folder::new();
    let oldest = folder.write("m_bad", "this is not json at all\n", false);
    set_file_order(&oldest, 0);
    ledger_of(&folder, 3, "08");
    let (page, damage) = listed(&folder, 1);
    assert_eq!(page, ["m_02"]);
    assert_eq!(damage.unreadable, 1);
}

// case: counts one file, not one per issue it raised
#[test]
fn counts_one_file_not_one_per_issue_it_raised() {
    let folder = Folder::new();
    // A header cut off mid-JSON: unparseable AND unterminated.
    let torn = folder.write("m_torn", "{\"schemaVersion\":1,\"recordType\":\"mission.crea", false);
    let (page, damage) = listed(&folder, 10);
    assert!(page.is_empty());
    assert!(ledger::read_ledger(&torn).unwrap().issues.len() > 1, "the torn header raises more than one issue");
    assert_eq!(damage.unreadable, 1);
}

// case: is zero when the ledger directory itself cannot be read
#[test]
fn is_zero_when_the_ledger_directory_itself_cannot_be_read() {
    // A plain FILE where the folder belongs: it is refused, not counted, and the list says "the records could not be
    // read" for it (`ListDamage::folder_unreadable`, torn_words_tests).
    let folder = Folder::new();
    let blocked = folder.0.join("mission-ledger");
    std::fs::write(&blocked, "not a directory").unwrap();
    assert!(HistoryCache::new().list_recent_with_damage(&blocked, 10).is_err());
}
