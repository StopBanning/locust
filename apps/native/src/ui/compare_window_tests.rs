//! Headless window tests for Compare / Blind mode and 1 / 3 blind judges.
use super::*;
use crate::store::Store;
use rune::{Env, Key, Runtime};
use serde_json::json;
use std::path::{Path, PathBuf};

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn teammate(root: &Path) -> Teammate {
    Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        busy: false,
        runs_in: "ask".into(),
        folder: Some(root.join("work").to_string_lossy().into()),
    }
}

fn window() -> (Dirs, Store, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-compare-drive-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    s.compare.picks = vec![
        crate::compare_view::ComparePick {
            runtime: "codex".into(),
            model: "synthetic-model".into(),
            effort: None,
            label: Some("model-alpha".into()),
        },
        crate::compare_view::ComparePick {
            runtime: "codex".into(),
            model: "synthetic-model".into(),
            effort: None,
            label: Some("model-beta".into()),
        },
    ];
    apply_team(&mut s, vec![teammate(&root)]);
    let mut rt = Runtime::new(s, view, (1280.0, 820.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (Dirs(root), store, rt)
}

fn fake(script: &str, scenario: &str) -> Option<(PathBuf, Vec<String>)> {
    Some((
        PathBuf::from("python3"),
        vec![
            format!("{}/scripts/fixtures/{script}", env!("CARGO_MANIFEST_DIR")),
            scenario.into(),
        ],
    ))
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn type_and_send(rt: &mut Runtime<State>, text: &str) {
    click(rt, "composer");
    rt.paste(text);
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    rt.frame(0.016);
}

fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut now = 0.0;
    while !done(&rt.state) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}: status {:?}",
            rt.state.status
        );
        rt.update(move |s, tx| {
            poll(s, now, tx);
        });
        rt.frame(0.3);
        now += 0.3;
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn text_in(rt: &Runtime<State>, key: &str) -> Vec<String> {
    let ([x, y, w, h], _) = rt
        .presented(key)
        .unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect()
}

fn all_slots_answered(s: &State) -> bool {
    let Some(id) = &s.compare.active_id else {
        return false;
    };
    let Some(c) = s.compare.compares.iter().find(|c| &c.compare_id == id) else {
        return false;
    };
    c.slots.len() == 2
        && c.slots.iter().all(|slot| {
            slot.mission_ids.last().is_some_and(|mid| {
                s.compare.snapshots.get(mid).is_some_and(|snap| {
                    !snap.running
                        && snap
                            .items
                            .iter()
                            .any(|i| matches!(i, ledger::Item::Assistant { text, .. } if !text.trim().is_empty()))
                })
            })
        })
}

#[test]
fn compare_of_two_models_shows_two_columns_each_answering() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store, fake("m1-app-server.py", "success"));

    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-compare");
    type_and_send(&mut rt, "Synthetic prompt");

    pump(&mut rt, "both compare columns to answer", all_slots_answered);
    settle(&mut rt);

    let col0 = text_in(&rt, "compare-col-a").join(" ");
    let col1 = text_in(&rt, "compare-col-b").join(" ");
    assert!(col0.contains("model-alpha"), "col A header: {col0}");
    assert!(col1.contains("model-beta"), "col B header: {col1}");
    assert!(col0.contains("Hello world"), "col A answer: {col0}");
    assert!(col1.contains("Hello world"), "col B answer: {col1}");
}

#[test]
fn blind_comparison_hides_names_until_a_pick_then_shows_them() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store, fake("m1-app-server.py", "success"));

    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-blind");
    type_and_send(&mut rt, "Synthetic prompt");

    pump(&mut rt, "both blind columns to answer", all_slots_answered);
    settle(&mut rt);

    let col0_before = text_in(&rt, "compare-col-a").join(" ");
    let col1_before = text_in(&rt, "compare-col-b").join(" ");
    assert!(col0_before.contains("Model A"), "expected Model A before pick: {col0_before}");
    assert!(col1_before.contains("Model B"), "expected Model B before pick: {col1_before}");
    assert!(!col0_before.contains("model-alpha") && !col0_before.contains("model-beta"));
    assert!(!col1_before.contains("model-alpha") && !col1_before.contains("model-beta"));

    click(&mut rt, "compare-keep-a");
    settle(&mut rt);

    let col0_after = text_in(&rt, "compare-col-a").join(" ");
    let col1_after = text_in(&rt, "compare-col-b").join(" ");
    assert!(
        col0_after.contains("model-alpha") || col0_after.contains("model-beta"),
        "expected revealed model name after pick: {col0_after}"
    );
    assert!(
        col1_after.contains("model-alpha") || col1_after.contains("model-beta"),
        "expected revealed model name after pick: {col1_after}"
    );
    assert!(col0_after.contains("Kept"), "expected Kept badge: {col0_after}");
}

#[test]
fn three_judges_score_the_answers_and_their_pick_is_marked() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));

    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-compare");
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "both columns to answer", all_slots_answered);

    click(&mut rt, "compare-judge-three-toggle");
    let cid = rt.state.compare.active_id.clone().unwrap();

    let mut j_ids = Vec::new();
    for (idx, body) in [
        "A1: met — A handles errors cleanly.\nB1: not met — misses edge case.\nKeep answer A\nA handles errors cleanly.",
        "A1: met — A is more direct.\nB1: not met — verbose.\nKeep answer A\nA is more direct.",
        "A1: not met — long.\nB1: met — concise.\nKeep answer B\nB is concise.",
    ]
    .into_iter()
    .enumerate()
    {
        let jid = format!("judge-mission-{idx}");
        let dir = store.ledger_dir().unwrap();
        let path = dir.join(format!("{jid}.jsonl"));
        let line1 = json!({
            "schemaVersion": 1,
            "recordType": "mission.created",
            "metadata": {
                "missionId": jid,
                "teammateId": "nobody",
                "runtime": "codex",
                "model": format!("judge-model-{idx}"),
                "createdAt": "2026-04-12T10:00:00Z",
                "prompt": "Judge"
            }
        });
        let line2 = json!({
            "schemaVersion": 1,
            "recordType": "mission.event",
            "event": {
                "sequence": 1,
                "type": "message.delta",
                "payload": {"itemId": "m1", "text": body, "operation": "replace"}
            }
        });
        let line3 = json!({
            "schemaVersion": 1,
            "recordType": "mission.event",
            "event": {
                "sequence": 2,
                "type": "run.completed",
                "payload": {}
            }
        });
        std::fs::write(&path, format!("{line1}\n{line2}\n{line3}\n")).unwrap();
        j_ids.push(jid);
    }

    let make_route = |idx: usize| locust_core::compare::CompareRoute {
        runtime: "codex".into(),
        model: format!("judge-model-{idx}"),
        effort: None,
        label: Some(format!("judge-model-{idx}")),
        instructions: None,
        account: None,
        mode: None,
        why: None,
    };
    let panel = vec![
        locust_core::compare::JudgePanelMember {
            route: make_route(1),
            mission_id: j_ids[1].clone(),
        },
        locust_core::compare::JudgePanelMember {
            route: make_route(2),
            mission_id: j_ids[2].clone(),
        },
    ];
    let cmp_store = locust_core::compare::CompareStore::from_store(&store).unwrap();
    cmp_store
        .judge(&cid, make_route(0), &j_ids[0], Some("Clear"), panel)
        .unwrap();

    rt.update(|s, tx| {
        crate::compare_view::reload_compares(s);
        poll(s, 1.0, tx);
    });
    settle(&mut rt);

    let col0 = text_in(&rt, "compare-col-a").join(" ");
    let judge_box = text_in(&rt, "compare-judge-view").join(" ");
    assert!(
        col0.contains("Judges’ pick") || col0.contains("Judges' pick"),
        "expected Judges' pick badge on column A: {col0}"
    );
    assert!(
        judge_box.contains("The 3 judges scored every point"),
        "expected panel vote summary in judge section: {judge_box}"
    );
    assert!(
        judge_box.contains("A handles errors cleanly."),
        "expected judge reason in judge section: {judge_box}"
    );
}
