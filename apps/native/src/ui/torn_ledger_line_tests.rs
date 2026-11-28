//! The sidebar's half of `main/torn-ledger.test.ts`: the records' damage is said under the conversations in
//! Screens.tsx's words (`native::ListDamage`), and nothing is said when the records are whole. Rust has no
//! "N conversations, M in this folder" header for these words to follow (`missions-lists-conversations.test.tsx`).
use super::*;

fn drawn(rows: usize, damage: native::ListDamage) -> Option<String> {
    let mut s = State::default();
    s.recent = (1..=rows)
        .map(|i| ConversationRow {
            id: format!("m_{i}"),
            teammate: "Wren".into(),
            title: format!("Task {i}"),
            started_by: "native".into(),
            state: "completed".into(),
            updated: format!("2026-11-18T10:0{i}:00Z"),
        })
        .collect();
    s.recent_damage = damage;
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("sidebar-records-damage")).and_then(|e| e.text)
}

#[test]
fn damaged_records_are_said_under_the_list() {
    let mixed = native::ListDamage { incomplete: 2, unreadable: 1, folder_unreadable: false };
    assert_eq!(drawn(2, mixed).as_deref(), Some("2 with an incomplete receipt · 1 file could not be read"));
    let unreadable = native::ListDamage { incomplete: 0, unreadable: 1, folder_unreadable: false };
    assert_eq!(drawn(0, unreadable).as_deref(), Some("1 file could not be read"), "said with nothing listed too");
}

#[test]
fn a_folder_that_could_not_be_listed_is_said_with_nothing_listed() {
    let folder = native::ListDamage { incomplete: 0, unreadable: 0, folder_unreadable: true };
    assert_eq!(drawn(0, folder).as_deref(), Some("the records could not be read"));
}

#[test]
fn whole_records_say_nothing_about_the_records() {
    assert_eq!(drawn(1, native::ListDamage::default()), None);
}

// The Electron mixed-damage case is already named in core. Here the real disk listing
// feeds the actual Conversations header instead of substituting a synthetic damage counter.
#[test]
fn missions_lists_recovered_receipts_separately_from_an_unreadable_file() {
    use serde_json::json;
    let dir = crate::tests::TempDir::new();
    let store = locust_core::store::Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    let records = dir.0.join("electron/mission-ledger");
    std::fs::create_dir_all(&records).unwrap();
    let at = "2026-11-23T10:00:00Z";
    for id in ["m_first", "m_second"] {
        let header = json!({
            "schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":at,
            "metadata":{"missionId":id,"runId":format!("run_{id}"),"prompt":"Do the work","runtime":"codex",
            "model":"account-default","createdAt":at,"workspaceId":"ws_test","sandbox":"read-only",
            "requestedRouteId":"codex","resolvedRouteId":"codex-account:default","executionPolicyVersion":1}
        });
        std::fs::write(records.join(format!("{id}.jsonl")), format!("{header}\n{{broken\n")).unwrap();
    }
    std::fs::write(records.join("m_header.jsonl"), "{\"schemaVersion\":").unwrap();
    let _guard = native::hooks::install(store, None);
    let listed = native::list_recent(20, true).expect("two receipts are recoverable");
    assert_eq!(listed.summaries.len(), 2);
    let mut s = State::default();
    s.missions.open = true;
    s.recent_damage = listed.damage;
    for summary in listed.summaries {
        let id = summary.mission_id.clone();
        s.recent.push(ConversationRow {
            id: id.clone(), title: summary.title.clone(), teammate: "Wren".into(),
            started_by: "native".into(), state: "completed".into(), updated: at.into(),
        });
        s.ledger_summaries.insert(id, summary);
    }
    let mut rt = rune::Runtime::new(s, crate::view, (1100.0, 760.0), rune::Env::new(true, 1.0));
    for _ in 0..8 { rt.frame(0.05); }
    let words = rt.elements().into_iter().filter_map(|el| el.text).collect::<Vec<_>>().join("\n");
    assert!(words.contains("2 with an incomplete receipt · 1 file could not be read"), "{words}");
    assert!(!words.contains("all readable"), "{words}");
}
