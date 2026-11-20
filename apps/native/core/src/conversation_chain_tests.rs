//! Electron's `conversation-chain.test.ts` (7 cases): walking a conversation back to its root through each turn's
//! `continuesFrom`, and the group it is briefed by.
use super::*;

struct Temp(std::path::PathBuf, Store);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A store whose ledger holds these turns: each id and the turn it continues (None for a root).
fn ledger(parents: &[(&str, Option<&str>)]) -> Temp {
    let dir = std::env::temp_dir().join(format!("locust-chain-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(dir.join("native")).unwrap();
    std::fs::create_dir_all(dir.join("electron")).unwrap();
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    let ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger).unwrap();
    for (id, parent) in parents {
        let first = match parent {
            Some(p) => serde_json::json!({"recordType": "mission.created", "continuesFrom": {"missionId": p}}),
            None => serde_json::json!({"recordType": "mission.created"}),
        };
        std::fs::write(ledger.join(format!("{id}.jsonl")), format!("{first}\n")).unwrap();
    }
    Temp(dir, store)
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ---- walking a conversation back to its root ----

#[test]
fn lists_every_id_from_the_previous_turn_to_the_root_newest_first() {
    let t = ledger(&[("m3", Some("m2")), ("m2", Some("m1")), ("m1", None)]);
    assert_eq!(keys_before(&t.1, Some("m3")), ids(&["m3", "m2", "m1"]));
    assert!(keys_before(&t.1, None).is_empty());
}

#[test]
fn reads_each_turn_once_for_the_life_of_the_process() {
    let t = ledger(&[("m3", Some("m2")), ("m2", Some("m1")), ("m1", None)]);
    assert_eq!(keys_before(&t.1, Some("m3")), ids(&["m3", "m2", "m1"]));
    // Read once: with the files gone, the walk is the same, from what was read.
    let _ = std::fs::remove_dir_all(t.1.ledger_dir().unwrap());
    assert_eq!(keys_before(&t.1, Some("m3")), ids(&["m3", "m2", "m1"]));
    assert_eq!(keys_before(&t.1, Some("m2")), ids(&["m2", "m1"]));
}

#[test]
fn stops_where_a_turn_is_gone_and_where_a_read_fails_without_guessing() {
    let gone = ledger(&[("m3", Some("m2"))]);
    assert_eq!(keys_before(&gone.1, Some("m3")), ids(&["m3", "m2"]));
    // A turn whose record cannot be read: a folder where its file should be.
    let failing = ledger(&[]);
    std::fs::create_dir_all(failing.1.ledger_dir().unwrap().join("m3.jsonl")).unwrap();
    assert_eq!(keys_before(&failing.1, Some("m3")), ids(&["m3"]));
}

#[test]
fn does_not_loop_on_a_cycle_and_does_not_walk_forever() {
    let cycle = ledger(&[("a", Some("b")), ("b", Some("a"))]);
    assert_eq!(keys_before(&cycle.1, Some("a")), ids(&["a", "b"]));
    let names: Vec<(String, String)> = (0..200).map(|i| (format!("m{i}"), format!("m{}", i + 1))).collect();
    let deep: Vec<(&str, Option<&str>)> = names.iter().map(|(a, b)| (a.as_str(), Some(b.as_str()))).collect();
    let deep = ledger(&deep);
    assert_eq!(keys_before(&deep.1, Some("m0")).len(), MAX_CHAIN_HOPS);
}

// ---- the group a conversation is briefed by ----

fn listed(group: Group, member: &str) -> ListedGroups {
    let mut members = HashMap::new();
    members.insert(member.to_owned(), GroupMembership { group_id: group.group_id.clone(), at: Some("2026-09-16T01:00:00.000Z".into()) });
    ListedGroups { groups: vec![group], members, left: HashMap::new() }
}

#[test]
fn finds_the_membership_on_any_id_the_conversation_has_worn_root_included() {
    let trading = Group::new("g1".into(), "Trading".into(), "2026-09-16T00:00:00.000Z".into(), "Quote sizes in shares.".into(), None);
    let l = listed(trading, "m1");
    let brief = group_brief_for(&ids(&["m3", "m2", "m1"]), &l).unwrap();
    assert_eq!((brief.group_name.as_str(), brief.instructions.as_str()), ("Trading", "Quote sizes in shares."));
    assert!(group_brief_for(&ids(&["m3", "m2"]), &l).is_none());
}

#[test]
fn briefs_nothing_for_a_group_with_no_instructions_it_is_a_folder() {
    let plain = Group::new("g2".into(), "Plain".into(), "2026-09-16T00:00:00.000Z".into(), "   ".into(), None);
    assert!(group_brief_for(&ids(&["m1"]), &listed(plain, "m1")).is_none());
}

#[test]
fn is_one_short_section_the_group_named_then_its_words() {
    let section = group_section("Trading", "Quote sizes in shares.").unwrap();
    assert!(section.contains("\"Trading\""));
    assert!(section.ends_with("Quote sizes in shares."));
    assert!(section.encode_utf16().count() - "Quote sizes in shares.".len() < 140, "{section}");
}
