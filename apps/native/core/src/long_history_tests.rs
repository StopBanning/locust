//! a-long-history-says-what-it-lists.test.ts (2 cases). Fixed 20 Nov: the window listed with no cap (usize::MAX),
//! reading every record of a huge history and never saying how much of it was listed. Now it lists the newest
//! MAX_LISTED_MISSIONS and, past that, says how many the records keep.
use super::*;

fn history(count: usize) -> Vec<ledger::Summary> {
    (1..=count)
        .map(|i| ledger::Summary {
            mission_id: format!("mission_{i:05}"),
            title: format!("Task {i}"),
            ..Default::default()
        })
        .collect()
}

/// Newer by index: an ISO time that sorts by `i`.
fn stamped(count: usize) -> Vec<ledger::Summary> {
    history(count)
        .into_iter()
        .enumerate()
        .map(|(at, mut m)| {
            let t = chrono::DateTime::<chrono::Utc>::from_timestamp(1_788_220_800 + at as i64 * 60, 0).unwrap();
            m.updated = t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
            m
        })
        .collect()
}

// case: lists far past the old 300, and says nothing more when everything is listed
#[test]
fn lists_far_past_the_old_300_and_says_nothing_more_when_all_is_listed() {
    let (listed, total) = capped(stamped(1_201), MAX_LISTED_MISSIONS, || 1_201);
    assert_eq!(listed.len(), 1_201);
    assert!(listed.iter().any(|m| m.title == "Task 900") && listed.iter().any(|m| m.title == "Task 1"));
    assert_eq!(total, None);
    assert_eq!(listed_words(listed.len(), total), None);
}

// case: past 2,000, says how many the ledger keeps and how many are listed
#[test]
fn past_2000_says_how_many_are_kept_and_how_many_listed() {
    assert_eq!(MAX_LISTED_MISSIONS, 2_000);
    let (listed, total) = capped(stamped(2_005), MAX_LISTED_MISSIONS, || 2_005);
    assert_eq!((listed.len(), total), (2_000, Some(2_005)));
    assert_eq!(listed[0].title, "Task 2005", "the newest are the ones listed");
    assert!(!listed.iter().any(|m| m.title == "Task 5"));
    assert_eq!(listed_words(2_000, total).as_deref(), Some("Only the newest 2,000 of 2,005 turns are listed"));
}

#[test]
fn the_whole_count_is_asked_only_when_the_list_is_full() {
    let (_, total) = capped(stamped(10), MAX_LISTED_MISSIONS, || panic!("a short history is not counted again"));
    assert_eq!(total, None);
}

#[test]
fn the_records_are_counted_by_their_file_names() {
    let dir = std::env::temp_dir().join(format!("locust-long-history-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for i in 1..=12 {
        std::fs::write(dir.join(format!("mission_{i:05}.jsonl")), b"").unwrap();
    }
    std::fs::write(dir.join("notes.txt"), b"").unwrap();
    std::fs::write(dir.join("..bad.jsonl"), b"").unwrap();
    let ids = crate::history_cache::global().mission_ids(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(ids.len(), 12);
}

#[test]
fn exactly_as_many_as_are_listed_says_nothing_more() {
    let (listed, total) = capped(stamped(2_000), MAX_LISTED_MISSIONS, || 2_000);
    assert_eq!((listed.len(), total), (2_000, None));
    assert_eq!(listed_words(2_000, Some(2_000)), None);
    assert_eq!(listed_words(2_000, Some(1_999)), None);
}
