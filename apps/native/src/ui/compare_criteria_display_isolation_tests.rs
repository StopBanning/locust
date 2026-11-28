use super::*;
use rune::{Env, Runtime};
use std::path::PathBuf;

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn seed_compare(store: &locust_core::compare::CompareStore, s: &mut State, prompt: &str) -> String {
    let cmp = store
        .create(CreateCompareInput {
            teammate_id: None,
            prompt: prompt.into(),
            routes: vec![
                CompareRoute::new("opencode", "opencode/m1"),
                CompareRoute::new("opencode", "opencode/m2"),
            ],
            changes: false,
            changes_in: None,
            blind: true,
            base: None,
            rerun_of: None,
        })
        .unwrap();
    for (slot, suffix) in [(CompareSlotId::A, "a"), (CompareSlotId::B, "b")] {
        let mid = format!("{}_{suffix}", cmp.compare_id);
        store.add_turn(&cmp.compare_id, slot, &mid).unwrap();
        s.compare.snapshots.insert(
            mid.clone(),
            ledger::Snapshot {
                tip: mid,
                items: vec![ledger::Item::Assistant {
                    item_id: "answer".into(),
                    text: "Done.".into(),
                    r#final: true,
                }],
                ..Default::default()
            },
        );
    }
    s.compare.compares.push(store.get(&cmp.compare_id).unwrap().unwrap());
    cmp.compare_id
}

#[test]
fn compare_criteria_box_does_not_show_another_comparisons_points() {
    let dir = Dirs(std::env::temp_dir().join(format!(
        "locust-compare-display-iso-{}",
        locust_core::invariants::unique_nanos()
    )));
    std::fs::create_dir_all(dir.0.join("electron/mission-ledger")).unwrap();
    let store = crate::store::Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    let compare_store = locust_core::compare::CompareStore::from_store(&store).unwrap();

    let mut s = State::default();
    s.data = Some(store);
    let first = seed_compare(&compare_store, &mut s, "First comparison");
    let second = seed_compare(&compare_store, &mut s, "Second comparison");
    s.compare
        .criteria
        .entry(first)
        .or_default()
        .replace_text("Only for the first comparison");
    s.compare.active_id = Some(second);

    let mut rt = Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1450., 1500.), Env::new(true, 1.));
    for _ in 0..4 {
        rt.frame(0.05);
    }

    assert!(
        rt.find("compare-draft-points").is_some(),
        "an unedited second comparison must treat its criteria box as empty and offer Draft the points"
    );
    let tree = rt.accessibility_json().to_string();
    assert!(
        !tree.contains("Only for the first comparison"),
        "the second comparison's criteria field must not display the first comparison's points"
    );
}
