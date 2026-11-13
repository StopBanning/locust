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

#[test]
fn keep_this_one_brings_column_change_into_folder_and_other_copies_go() {
    let (dirs, store, mut rt) = window();
    let workspace = dirs.0.join("work");
    std::fs::write(workspace.join("notes.txt"), "before\n").unwrap();

    let copy_root = locust_core::compare_copies::compare_root(&store);
    let cmp_store = locust_core::compare::CompareStore::from_store(&store).unwrap();
    let cmp = cmp_store
        .create(locust_core::compare::CreateCompareInput {
            teammate_id: None,
            prompt: "Update notes.txt".into(),
            routes: vec![
                locust_core::compare::CompareRoute::new("codex", "gpt-6"),
                locust_core::compare::CompareRoute::new("codex", "sonnet"),
            ],
            changes: true,
            changes_in: Some(locust_core::compare::ChangesIn::Copy),
            blind: false,
            base: None,
            rerun_of: None,
        })
        .unwrap();

    let copy_a = locust_core::compare_copies::make_compare_copy(
        &workspace,
        &cmp.compare_id,
        locust_core::compare::CompareSlotId::A,
        &copy_root,
        None,
    )
    .unwrap();
    let copy_b = locust_core::compare_copies::make_compare_copy(
        &workspace,
        &cmp.compare_id,
        locust_core::compare::CompareSlotId::B,
        &copy_root,
        None,
    )
    .unwrap();
    std::fs::write(copy_a.join("notes.txt"), "kept from A\n").unwrap();
    std::fs::write(copy_b.join("notes.txt"), "discarded from B\n").unwrap();

    let ledger_dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger_dir).unwrap();
    for (slot, mid, text) in [
        (locust_core::compare::CompareSlotId::A, "m_keep_a", "Updated notes from A."),
        (locust_core::compare::CompareSlotId::B, "m_keep_b", "Updated notes from B."),
    ] {
        let l1 = json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":mid,"teammateId":"nobody","runtime":"codex","model":"gpt-6","prompt":"Update notes.txt","createdAt":"2026-11-13T00:00:00.000Z"}});
        let l2 = json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-11-13T00:00:01.000Z","type":"message.delta","payload":{"itemId":"m1","text":text,"operation":"replace","final":true}}});
        let l3 = json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":2,"occurredAt":"2026-11-13T00:00:02.000Z","type":"run.completed","payload":{}}});
        std::fs::write(ledger_dir.join(format!("{mid}.jsonl")), format!("{l1}\n{l2}\n{l3}\n")).unwrap();
        cmp_store.add_turn(&cmp.compare_id, slot, mid).unwrap();
    }

    rt.update(|s, tx| {
        s.home.folder_path = Some(workspace.to_string_lossy().into());
        s.compare.active_id = Some(cmp.compare_id.clone());
        crate::compare_view::reload_compares(s);
        poll(s, 1.0, tx);
    });
    settle(&mut rt);

    click(&mut rt, "compare-keep-a");
    settle(&mut rt);

    assert_eq!(std::fs::read_to_string(workspace.join("notes.txt")).unwrap(), "kept from A\n");
    assert!(!copy_a.exists(), "copy A should be removed after Keep");
    assert!(!copy_b.exists(), "copy B should be removed after Keep");
    assert_eq!(
        cmp_store.get(&cmp.compare_id).unwrap().unwrap().kept.unwrap().slot,
        locust_core::compare::CompareSlotId::A
    );
}

#[test]
fn a_column_run_again_answers_again() {
    let (_dirs, store, mut rt) = window();
    let ledger_dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger_dir).unwrap();

    let cmp_store = locust_core::compare::CompareStore::from_store(&store).unwrap();
    let cmp = cmp_store
        .create(locust_core::compare::CreateCompareInput {
            teammate_id: Some("tm_codec".into()),
            prompt: "Synthetic prompt".into(),
            routes: vec![
                locust_core::compare::CompareRoute::new("codex", "synthetic-model"),
                locust_core::compare::CompareRoute::new("codex", "synthetic-model"),
            ],
            changes: false,
            changes_in: None,
            blind: false,
            base: None,
            rerun_of: None,
        })
        .unwrap();

    // Column A completed; Column B failed.
    let write_turn = |mid: &str, text: &str, failed: bool| {
        let l1 = json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":mid,"teammateId":"nobody","runtime":"codex","model":"synthetic-model","prompt":"Explain worktrees","createdAt":"2026-11-13T00:00:00.000Z"}});
        let l2 = json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-11-13T00:00:01.000Z","type":"message.delta","payload":{"itemId":"m1","text":text,"operation":"replace","final":true}}});
        let end_type = if failed { "run.failed" } else { "run.completed" };
        let l3 = json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":2,"occurredAt":"2026-11-13T00:00:02.000Z","type":end_type,"payload":{}}});
        std::fs::write(ledger_dir.join(format!("{mid}.jsonl")), format!("{l1}\n{l2}\n{l3}\n")).unwrap();
    };
    write_turn("m_ok_a", "Worktrees isolate checkouts.", false);
    write_turn("m_fail_b", "", true);
    cmp_store.add_turn(&cmp.compare_id, locust_core::compare::CompareSlotId::A, "m_ok_a").unwrap();
    cmp_store.add_turn(&cmp.compare_id, locust_core::compare::CompareSlotId::B, "m_fail_b").unwrap();

    let _g = native::hooks::install(store, fake("m1-app-server.py", "success"));

    rt.update(|s, tx| {
        s.compare.active_id = Some(cmp.compare_id.clone());
        crate::compare_view::reload_compares(s);
        poll(s, 1.0, tx);
    });
    settle(&mut rt);

    // Failed column shows Try again
    let col_b = text_in(&rt, "compare-col-b").join(" ");
    assert!(col_b.contains("failed"), "expected failed state in column B: {col_b}");
    assert!(col_b.contains("Try again"), "expected Try again button on failed column B: {col_b}");

    click(&mut rt, "compare-retry-b");
    let after_click_b = rt.state.compare.compares.iter().find(|c| c.compare_id == cmp.compare_id).unwrap().slots[1].clone();
    assert!(
        after_click_b.mission_ids.last().map(String::as_str) != Some("m_fail_b"),
        "retry did not replace m_fail_b: problem={:?}, slot_b={:?}",
        rt.state.compare.problem,
        after_click_b
    );
    pump(&mut rt, "retried column B answer", |s| {
        s.compare
            .compares
            .iter()
            .find(|c| c.compare_id == cmp.compare_id)
            .and_then(|c| c.slots.get(1))
            .is_some_and(|slot| {
                slot.mission_ids.last().is_some_and(|mid| {
                    mid != "m_fail_b"
                        && s.compare.snapshots.get(mid).is_some_and(|snap| {
                            !snap.running
                                && snap.items.iter().any(|i| matches!(i, ledger::Item::Assistant { text, .. } if !text.trim().is_empty()))
                        })
                })
            })
    });

    let col_b_after = text_in(&rt, "compare-col-b").join(" ");
    assert!(col_b_after.contains("Hello world"), "retried column B should have fixture answer: {col_b_after}");
    assert!(!col_b_after.contains("Try again"), "retried column B should no longer be failed: {col_b_after}");
}

#[test]
fn a_benchmark_of_two_runs_reads_as_one_table() {
    let (_dirs, store, mut rt) = window();
    let ledger_dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger_dir).unwrap();

    let cmp_store = locust_core::compare::CompareStore::from_store(&store).unwrap();
    let r_a = locust_core::compare::CompareRoute::new("codex", "gpt-6");
    let r_b = locust_core::compare::CompareRoute::new("claude", "sonnet");
    let j_r = locust_core::compare::CompareRoute::new("codex", "judge-model");

    let origin = cmp_store
        .create(locust_core::compare::CreateCompareInput {
            teammate_id: None,
            prompt: "Fix the date parser".into(),
            routes: vec![r_a.clone(), r_b.clone()],
            changes: false,
            changes_in: None,
            blind: false,
            base: None,
            rerun_of: None,
        })
        .unwrap();
    let rerun = cmp_store
        .create(locust_core::compare::CreateCompareInput {
            teammate_id: None,
            prompt: "Fix the date parser".into(),
            routes: vec![r_a, r_b],
            changes: false,
            changes_in: None,
            blind: false,
            base: None,
            rerun_of: Some(origin.compare_id.clone()),
        })
        .unwrap();
    cmp_store.add_rerun(&origin.compare_id, &rerun.compare_id).unwrap();
    cmp_store.settle(&rerun.compare_id).unwrap();

    let write_mission = |mid: &str, text: &str| {
        let l1 = json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":mid,"teammateId":"nobody","runtime":"codex","model":"gpt-6","prompt":"Fix the date parser","createdAt":"2026-11-13T00:00:00.000Z"}});
        let l2 = json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-11-13T00:00:01.000Z","type":"message.delta","payload":{"itemId":"m1","text":text,"operation":"replace","final":true}}});
        let l3 = json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":2,"occurredAt":"2026-11-13T00:00:02.000Z","type":"run.completed","payload":{}}});
        std::fs::write(ledger_dir.join(format!("{mid}.jsonl")), format!("{l1}\n{l2}\n{l3}\n")).unwrap();
    };

    for (cid, suffix, j_body) in [
        (&origin.compare_id, "1", "I would keep Answer A.\n\nSCORE A 1: met.\nSCORE B 1: no."),
        (&rerun.compare_id, "2", "I would keep Answer A.\n\nSCORE A 1: met.\nSCORE B 1: met."),
    ] {
        let ma = format!("m_a_{suffix}");
        let mb = format!("m_b_{suffix}");
        let mj = format!("m_j_{suffix}");
        write_mission(&ma, "Answer A fix");
        write_mission(&mb, "Answer B fix");
        write_mission(&mj, j_body);
        cmp_store.add_turn(cid, locust_core::compare::CompareSlotId::A, &ma).unwrap();
        cmp_store.add_turn(cid, locust_core::compare::CompareSlotId::B, &mb).unwrap();
        cmp_store.judge(cid, j_r.clone(), &mj, Some("Handles leap years".into()), Vec::new()).unwrap();
    }

    rt.update(|s, tx| {
        s.compare.active_id = Some(origin.compare_id.clone());
        crate::compare_view::reload_compares(s);
        poll(s, 1.0, tx);
    });
    settle(&mut rt);

    let bench_text = text_in(&rt, "compare-benchmark").join(" ");
    assert!(
        bench_text.contains("YOUR BENCHMARK · 2 RUNS"),
        "expected benchmark header: {bench_text}"
    );
    assert!(bench_text.contains("100"), "expected 100 score for gpt-6: {bench_text}");
    assert!(bench_text.contains("50"), "expected 50 score for sonnet: {bench_text}");
    assert!(bench_text.contains("Run 1 · this one"), "expected Run 1 chip: {bench_text}");
    assert!(bench_text.contains("Run 2"), "expected Run 2 chip: {bench_text}");

    // Open Benchmarks screen and verify it lists the benchmark
    rt.update(|s, _tx| {
        crate::benchmarks_panel::open(s);
    });
    settle(&mut rt);
    let screen_text = text_in(&rt, "benchmarks-screen").join(" ");
    assert!(screen_text.contains("1 benchmark · 2 runs"), "expected benchmark summary: {screen_text}");
    assert!(screen_text.contains("Fix the date parser"), "expected benchmark question: {screen_text}");
}
