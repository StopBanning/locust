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
        account: None,
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
            account: None,
            effort: None,
            label: Some("model-alpha".into()),
        },
        crate::compare_view::ComparePick {
            runtime: "codex".into(),
            model: "synthetic-model".into(),
            account: None,
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
    // 60 s, not 20: alone each test answers in under a second, but the workspace's release run beside another build
    // took one past 20 twice on 20 Nov. A real hang still fails, only later.
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
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
            "schemaVersion": 27,
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
            "schemaVersion": 27,
            "recordType": "mission.event",
            "event": {
                "sequence": 1,
                "type": "message.delta",
                "payload": {"itemId": "m1", "text": body, "operation": "replace"}
            }
        });
        let line3 = json!({
            "schemaVersion": 27,
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
        skill: None,
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

/// builtPageOf: a column's page is the first `.html` a tool wrote or added, never one its answer only mentions.
#[test]
fn a_columns_page_is_the_one_a_tool_made_not_one_it_mentions() {
    use locust_core::ledger::{Item, Snapshot, Status, ToolDetail};
    let tool = |name: &str, arguments: &str| Item::Tool { name: name.into(), status: Status::Done, summary: String::new(), detail: Box::new(ToolDetail { arguments: arguments.into(), ..Default::default() }) };
    let said = Item::Assistant { item_id: "a1".into(), text: "Open docs/index.html to see it.".into(), r#final: true };
    let mut snap = Snapshot { items: vec![said.clone(), tool("Read", r#"{"file_path":"other.html"}"#.replace("file_path", "pattern").as_str())], ..Default::default() };
    assert!(crate::compare_view::column_html_pages(&snap).is_empty(), "a page only mentioned, or only searched for, is not one it made");
    snap.items.push(tool("Write", r#"{"file_path":"site/page.html","content":"<a href=\"x.html\">"}"#));
    snap.items.push(tool("Write", r#"{"file_path":"second.html","content":""}"#));
    assert_eq!(crate::compare_view::column_html_pages(&snap), vec!["site/page.html".to_owned()]);
    let patch = Snapshot { items: vec![said, tool("apply_patch", "*** Begin Patch\n*** Add File: web/app.htm\n+<p>hi</p>\n*** End Patch")], ..Default::default() };
    assert_eq!(crate::compare_view::column_html_pages(&patch), vec!["web/app.htm".to_owned()]);
}

#[test]
fn compares_the_same_agent_and_model_on_two_distinct_accounts_and_keeps_both_across_reload() {
    use locust_core::agent_accounts::Accounts;
    use locust_core::compare::CompareStore;
    let _clear = crate::agent_accounts_card::test_guard();

    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    let accounts = Accounts::new(
        store.root(),
        std::sync::Arc::new(|_, _, _, _| Ok(Box::new(|| {}))),
        Box::new(|| None),
        Box::new(native::account_in_use),
    );
    let work = accounts.add("codex", "Work").unwrap();
    let personal = accounts.add("codex", "Personal").unwrap();
    crate::agent_accounts_card::use_accounts(accounts);

    rt.update(|s, _| {
        s.models.insert(
            "tm_codec".into(),
            vec![locust_core::engine::Models {
                teammate: Some("tm_codec".into()),
                runtime: "codex".into(),
                model: Some("synthetic-model".into()),
                effort: None,
                models: vec![locust_core::engine::Model {
                    id: "synthetic-model".into(),
                    name: "Synthetic".into(),
                    efforts: vec![],
                }],
            }],
        );
        crate::agent_accounts_card::reload(s);
        s.compare.composer_mode = crate::compare_view::CompareComposerMode::Compare;
    });
    settle(&mut rt);

    // Pick Work on slot 0 and Personal on slot 1 via the UI picker menu.
    click(&mut rt, "compare-pick-0");
    click(&mut rt, &format!("compare-slot-choice-codex:synthetic-model:{}", work.id));
    click(&mut rt, "compare-pick-1");
    click(&mut rt, &format!("compare-slot-choice-codex:synthetic-model:{}", personal.id));

    assert_eq!(rt.state.compare.picks[0].account.as_deref(), Some(work.id.as_str()));
    assert_eq!(rt.state.compare.picks[1].account.as_deref(), Some(personal.id.as_str()));

    // Send prompt to launch comparison across the two accounts.
    click(&mut rt, "composer");
    rt.paste("Compare on two accounts");
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    pump(&mut rt, "compare created", |s| !s.compare.compares.is_empty());

    let cmp_id = rt.state.compare.compares[0].compare_id.clone();
    let restored = CompareStore::from_store(&store).unwrap().get(&cmp_id).unwrap().unwrap();
    let restored_accounts: Vec<Option<String>> = restored.slots.iter().map(|s| s.route.account.clone()).collect();
    assert_eq!(restored_accounts, vec![Some(work.id.clone()), Some(personal.id.clone())]);

    // Adding a duplicate (same agent, model, account) is refused.
    let dup = CompareStore::from_store(&store).unwrap().add_column(
        &cmp_id,
        locust_core::compare::CompareRoute {
            account: Some(work.id.clone()),
            ..locust_core::compare::CompareRoute::new("codex", "synthetic-model")
        },
    );
    assert!(dup.unwrap_err().contains("already"));
}

// a-comparison-shows-its-models-on-hover.test.tsx
#[test]
fn a_comparison_shows_its_models_on_hover() {
    let (_dirs, _store, mut rt) = window();
    settle(&mut rt);
    let pick = |label: &str| crate::compare_view::ComparePick {
        runtime: "opencode".into(),
        model: label.into(),
        account: None,
        effort: None,
        label: Some(label.into()),
    };

    rt.update(|s, _| {
        s.compare.composer_mode = crate::compare_view::CompareComposerMode::Compare;
        s.compare.picks = vec![
            pick("Mimo V2.6 Flash Free"),
            pick("Ling 3.0 Flash Fin Free"),
            pick("Muse Spark 1.3 Contributor Free"),
        ];
    });
    rt.frame(0.016);
    let (x, y) = rt.find("model-chip").unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.016);
    rt.frame(0.6);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("compare-route-tooltip"));
    assert_eq!(
        tip.and_then(|e| e.label).as_deref(),
        Some("Mimo V2.6 Flash Free vs Ling 3.0 Flash Fin Free vs Muse Spark 1.3 Contributor Free")
    );

    rt.update(|s, _| {
        s.compare.composer_mode = crate::compare_view::CompareComposerMode::Blind;
    });
    rt.frame(0.016);
    let (x, y) = rt.find("model-chip").unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.016);
    rt.frame(0.6);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("compare-route-tooltip"));
    assert_eq!(
        tip.and_then(|e| e.label).as_deref(),
        Some("Model A vs Model B vs Model C")
    );
}

fn recorded_comparison(store: &Store) -> locust_core::compare::Compare {
    use locust_core::compare::{CompareRoute, CompareSlotId, CompareStore, CreateCompareInput};
    let compares = CompareStore::from_store(store).unwrap();
    let compare = compares.create(CreateCompareInput {
        teammate_id: Some("tm_codec".into()),
        prompt: "Add a discount code field.".into(),
        routes: vec![CompareRoute::new("claude", "fable"), CompareRoute::new("codex", "gpt-6-astra")],
        changes: false,
        changes_in: None,
        blind: false,
        base: None,
        rerun_of: None,
    }).unwrap();
    compares.add_turn(&compare.compare_id, CompareSlotId::A, "m_a1").unwrap();
    compares.add_turn(&compare.compare_id, CompareSlotId::B, "m_b1").unwrap()
}

fn recorded_sidebar_turn(s: &mut State, id: &str, parent: Option<&str>, minute: u8) {
    let at = format!("2026-09-28T12:{minute:02}:00.000Z");
    let title = if id == "m_other" { "Fix the cart total" } else { "Add a discount code field." };
    s.recent.push(ConversationRow {
        id: id.into(), title: title.into(), teammate: "Codec".into(),
        started_by: "native".into(), state: "completed".into(), updated: at.clone(),
    });
    s.ledger_summaries.insert(id.into(), ledger::Summary {
        mission_id: id.into(), title: title.into(), parent: parent.map(Into::into),
        created_at: at.clone(), updated: at, ..Default::default()
    });
}

fn sidebar_ids(s: &State) -> Vec<String> {
    sidebar::sidebar_missions_now(s).into_iter().map(|row| row.mission_id).collect()
}

#[test]
fn comparison_sidebar_opens_undecided_once() {
    // case: lists an undecided comparison once, as the row that opens it
    let (_dirs, store, mut rt) = window();
    let _hooks = native::hooks::install(store.clone(), None);
    let compare = recorded_comparison(&store);
    let id = compare.compare_id.clone();
    rt.update(move |s, _| {
        s.compare.compares = vec![compare];
        recorded_sidebar_turn(s, "m_a1", None, 2);
        recorded_sidebar_turn(s, "m_b1", None, 1);
        recorded_sidebar_turn(s, "m_other", None, 0);
    });
    settle(&mut rt);
    assert_eq!(sidebar_ids(&rt.state), ["m_a1", "m_other"]);
    assert!(rt.find("recent-m_a1").is_none());
    assert!(rt.find("recent-m_b1").is_none());
    assert!(rt.find("recent-m_other").is_some());
    let compare_key = format!("sidebar-compare-{id}");
    let count = rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref() == Some(compare_key.as_str())).count();
    assert_eq!(count, 1);
    click(&mut rt, &compare_key);
    assert_eq!(rt.state.compare.active_id.as_deref(), Some(id.as_str()));
    assert!(rt.find("compare-bar").is_some());
}

#[test]
fn comparison_sidebar_kept_column_is_a_conversation() {
    // case: lists a kept one as the kept column, an ordinary conversation, with its later turns
    use locust_core::compare::{CompareKept, CompareSlotId};
    let (_dirs, store, mut rt) = window();
    let _hooks = native::hooks::install(store.clone(), None);
    let mut compare = recorded_comparison(&store);
    let id = compare.compare_id.clone();
    compare.kept = Some(CompareKept { slot: CompareSlotId::B, at: "2026-09-28T12:05:00Z".into(), brought: None });
    rt.update(move |s, _| {
        s.compare.compares = vec![compare];
        recorded_sidebar_turn(s, "m_b2", Some("m_b1"), 3);
        recorded_sidebar_turn(s, "m_b1", None, 2);
        recorded_sidebar_turn(s, "m_a1", None, 1);
        recorded_sidebar_turn(s, "m_other", None, 0);
    });
    settle(&mut rt);
    assert_eq!(sidebar_ids(&rt.state), ["m_b2", "m_other"]);
    assert!(rt.find(&format!("sidebar-compare-{id}")).is_none());
    let rows = sidebar::sidebar_missions_now(&rt.state);
    assert_eq!(rows[0].member_ids, ["m_b1", "m_b2"]);
    assert_eq!(sidebar::comparison_of(&rt.state, &rows[0].member_ids).unwrap().compare_id, id);
    assert!(sidebar::comparison_of(&rt.state, &rows[1].member_ids).is_none());
    click(&mut rt, "recent-m_b2");
    assert!(rt.state.compare.active_id.is_none());
    assert_eq!(rt.state.ledger.as_ref().unwrap().conversation.conversation_id.as_deref(), Some("m_b2"));

    rt.update(|s, _| {
        let pane = s.ledger.as_mut().unwrap();
        pane.loading = false;
        pane.items = vec![ledger::Item::User("Add a discount code field.".into())];
        pane.snapshot.turn_metas = vec![
            ledger::Metadata { mission_id: "m_b1".into(), ..Default::default() },
            ledger::Metadata { mission_id: "m_b2".into(), ..Default::default() },
        ];
    });
    click(&mut rt, "open-thread-comparison");
    assert_eq!(rt.state.compare.active_id.as_deref(), Some(id.as_str()));
}

#[test]
fn comparison_sidebar_retries_only_show_current_answer() {
    // case: still lists it once after columns were tried again, as a row holding a current answer (0.444)
    let (_dirs, store, mut rt) = window();
    let _hooks = native::hooks::install(store.clone(), None);
    let mut compare = recorded_comparison(&store);
    let id = compare.compare_id.clone();
    for slot in &mut compare.slots {
        slot.retried = std::mem::take(&mut slot.mission_ids);
        slot.mission_ids = vec![format!("{}_again", slot.retried[0])];
    }
    rt.update(move |s, _| {
        s.compare.compares = vec![compare];
        for (minute, mission) in ["m_b1_again", "m_a1_again", "m_other", "m_b1", "m_a1"].into_iter().enumerate() {
            recorded_sidebar_turn(s, mission, None, minute as u8);
        }
    });
    settle(&mut rt);
    assert_eq!(sidebar_ids(&rt.state), ["m_other", "m_a1_again"]);
    assert!(rt.find("recent-m_a1").is_none());
    assert!(rt.find("recent-m_b1").is_none());
    assert!(rt.find("recent-m_b1_again").is_none());
    assert_eq!(sidebar::comparison_of(&rt.state, &["m_b1".into()]).unwrap().compare_id, id);
    click(&mut rt, &format!("sidebar-compare-{id}"));
    assert_eq!(rt.state.compare.active_id.as_deref(), Some(id.as_str()));
}

fn default_catalog(models: &[(&str, &[&str])]) -> Vec<engine::Models> {
    models.iter().map(|(runtime, ids)| engine::Models {
        teammate: None, runtime: (*runtime).into(), model: None, effort: None,
        models: ids.iter().map(|id| engine::Model {
            id: (*id).into(), name: (*id).into(), efforts: vec!["low".into(), "medium".into()],
        }).collect(),
    }).collect()
}

fn default_pick_state(models: &[(&str, &[&str])]) -> State {
    let mut s = State::default();
    s.models.insert("tm_codec".into(), default_catalog(models));
    s
}

fn composer_model(s: &mut State, runtime: &str, model: &str, effort: Option<&str>) {
    let mut mate = teammate(Path::new("."));
    mate.runtime = Some(runtime.into());
    apply_team(s, vec![mate]);
    s.picks.insert("tm_codec".into(), TurnPick {
        model: model.into(), runtime: Some(runtime.into()), effort: effort.map(Into::into), account: None,
    });
}

fn default_routes(s: &State) -> Vec<String> {
    s.compare.picks.iter().map(|pick| format!("{}:{}", pick.runtime, pick.model)).collect()
}

const DEFAULT_CATALOG: &[(&str, &[&str])] = &[
    ("claude", &["opus", "sonnet"]),
    ("codex", &["gpt-6-sol"]),
    ("opencode", &["opencode/nemotron-3-ultra-free"]),
];

#[test]
fn compare_view_starts_with_the_composer_and_a_recent_other_maker() {
    // case: are what is in the box, then another maker this person has run
    let (_dirs, store, mut rt) = window();
    let _hooks = native::hooks::install(store, None);
    rt.update(|s, _| {
        s.compare.picks.clear();
        composer_model(s, "claude", "opus", None);
        s.models.insert("tm_codec".into(), default_catalog(&[
            ("claude", &["sonnet", "opus"]),
            ("opencode", &["opencode/nemotron-3-ultra-free"]),
            ("codex", &["gpt-6-sol"]),
        ]));
        s.recent_route_keys = vec!["claude:sonnet".into(), "codex:gpt-6-sol".into()];
    });
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-compare");
    assert_eq!(default_routes(&rt.state), ["claude:opus", "codex:gpt-6-sol"]);
    assert_eq!(rt.state.compare.picks[0].label.as_deref(), Some("opus"));
    for index in 0..2 {
        click(&mut rt, &format!("compare-pick-{index}"));
        assert_eq!(rt.state.compare.model_menu, Some(index));
        click(&mut rt, &format!("compare-pick-{index}"));
    }
}

#[test]
fn compare_view_does_not_default_to_account_default() {
    // case: never start on "Account default", which names no model
    let mut s = default_pick_state(DEFAULT_CATALOG);
    composer_model(&mut s, "claude", "account-default", None);
    s.recent_route_keys = vec!["claude:account-default".into(), "claude:sonnet".into()];
    compare_view::ensure_default_picks(&mut s);
    assert_eq!(default_routes(&s), ["claude:sonnet", "codex:gpt-6-sol"]);
}

#[test]
fn compare_view_skips_refused_and_not_ready_agents() {
    // case: skip a model that cannot join, or whose runtime is not ready
    use locust_core::agents::{Auth, Found, Probe};
    let mut s = default_pick_state(&[("apple", &["on-device"]), ("codex", &["gpt-6-sol"]), ("claude", &["sonnet"]), ("opencode", &["free"])]);
    composer_model(&mut s, "apple", "on-device", None);
    s.compare.changes = true;
    s.recent_route_keys = vec!["codex:gpt-6-sol".into()];
    s.settings_panel.agents = Some(["apple", "codex", "claude", "opencode"].into_iter().map(|id| Found {
        id: id.into(), display_name: id.into(), installed: true, version: None,
        auth: Auth::Unknown, ready: id != "codex", status: if id == "codex" { Probe::Offline } else { Probe::Ready },
        unsupported_note: None, through_cli: false, checking: false,
    }).collect());
    compare_view::ensure_default_picks(&mut s);
    assert_eq!(default_routes(&s), ["claude:sonnet", "opencode:free"]);
}

#[test]
fn compare_view_uses_two_from_one_agent_or_none_when_only_one_can_join() {
    // case: are two of one maker when only one is ready, and none when fewer than two can join
    let mut s = default_pick_state(&[("claude", &["opus", "sonnet"])]);
    compare_view::ensure_default_picks(&mut s);
    assert_eq!(default_routes(&s), ["claude:opus", "claude:sonnet"]);
    let mut s = default_pick_state(&[("claude", &["opus"])]);
    compare_view::ensure_default_picks(&mut s);
    assert!(s.compare.picks.is_empty());
}

#[test]
fn compare_view_stays_free_when_first_is_free() {
    // case: stay free when the first is free (0.515): a free teammate is not compared against a paid model nobody chose
    let mimo = "opencode/mimo-v2.6-flash-free";
    let longcat = "opencode/longcat-2.5-preview-free";
    for (catalog, second) in [(&[mimo, longcat][..], "opencode:opencode/longcat-2.5-preview-free"), (&[mimo][..], "codex:gpt-6.1-sol")] {
        let mut s = default_pick_state(&[("opencode", catalog), ("codex", &["gpt-6.1-sol"])]);
        composer_model(&mut s, "opencode", mimo, None);
        compare_view::ensure_default_picks(&mut s);
        assert_eq!(default_routes(&s), ["opencode:opencode/mimo-v2.6-flash-free", second]);
    }
}

#[test]
fn compare_view_prioritizes_answered_free_models_then_steady_ones() {
    // case: take, as the second free one, a free model that has answered here over the first listed (0.517)
    let mimo = "opencode/mimo-v2.6-flash-free";
    let ling = "opencode/ling-3.0-flash-fin-free";
    let glm = "opencode/glm-5.3-flash-free";
    for (answered, recent, second) in [(Some(glm), Some(ling), glm), (None, None, glm), (Some(ling), None, ling)] {
        let mut s = default_pick_state(&[("opencode", &[mimo, ling, glm])]);
        composer_model(&mut s, "opencode", mimo, None);
        if let Some(answered) = answered {
            s.ledger_summaries.insert("answer".into(), ledger::Summary {
                runtime: "opencode".into(), model: answered.into(), updated: "2026-09-28T12:00:00.000Z".into(),
                ..Default::default()
            });
        }
        if let Some(recent) = recent {
            s.recent_route_keys.push(format!("opencode:{recent}"));
        }
        compare_view::ensure_default_picks(&mut s);
        assert_eq!(s.compare.picks.iter().map(|pick| pick.model.as_str()).collect::<Vec<_>>(), [mimo, second]);
    }
}

#[test]
fn compare_view_catalog_does_not_suggest_older_or_own_routes() {
    // case: leave out older models and the person's own
    let mut s = default_pick_state(&[
        ("claude", &["claude-haiku-4-5-20251001", "opus"]),
        ("opencode", &["own-7bb317a3/mine"]),
        ("codex", &["gpt-6-sol"]),
    ]);
    compare_view::ensure_default_picks(&mut s);
    assert_eq!(default_routes(&s), ["claude:opus", "codex:gpt-6-sol"]);
}

#[test]
fn compare_view_keeps_the_composer_effort_only_on_column_a() {
    // case: keeps the box's effort on the first column (0.545, Sol: Flash Low opened Compare on Medium)
    let mut s = default_pick_state(DEFAULT_CATALOG);
    composer_model(&mut s, "claude", "opus", Some("low"));
    s.recent_route_keys.push("codex:gpt-6-sol".into());
    compare_view::ensure_default_picks(&mut s);
    assert_eq!(default_routes(&s), ["claude:opus", "codex:gpt-6-sol"]);
    assert_eq!(s.compare.picks[0].effort.as_deref(), Some("low"));
    assert_eq!(s.compare.picks[1].effort, None);
}

fn argon_catalog() -> Vec<engine::Models> {
    vec![engine::Models {
        teammate: None,
        runtime: "cursor".into(),
        model: None,
        effort: None,
        models: vec![engine::Model {
            id: "gemini-4-argon-low".into(),
            name: "Gemini 4 Argon".into(),
            efforts: vec!["low".into(), "medium".into(), "high".into()],
        }],
    }]
}

#[test]
fn compare_view_cursor_header_and_picker_name_the_same_family() {
    // case: Cursor’s Argon on its Low id is "Gemini 4 Argon", the level being its own control
    let (_dirs, store, mut rt) = window();
    let mut compare = recorded_comparison(&store);
    let id = compare.compare_id.clone();
    compare.slots[0].route = locust_core::compare::CompareRoute::new("cursor", "gemini-4-argon-low");
    compare.slots[0].route.label = Some("Gemini 4 Argon Low".into());
    rt.update(move |s, _| {
        s.models.insert("tm_codec".into(), argon_catalog());
        s.resolved_model_names.insert("cursor:gemini-4-argon-low".into(), "Gemini 4 Argon Low".into());
        s.compare.composer_mode = compare_view::CompareComposerMode::Compare;
        s.compare.picks[0] = compare_view::ComparePick {
            runtime: "cursor".into(), model: "gemini-4-argon-low".into(), account: None,
            effort: Some("high".into()), label: Some("Gemini 4 Argon Low".into()),
        };
        s.compare.compares = vec![compare];
    });
    settle(&mut rt);
    assert_eq!(text_in(&rt, "compare-pick-0"), ["Gemini 4 Argon"]);
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("compare-effort-0") &&
        e.label.as_deref() == Some("Effort for model A: High")));
    rt.update(move |s, _| s.compare.active_id = Some(id.clone()));
    settle(&mut rt);
    assert!(text_in(&rt, "compare-head-a").contains(&"Gemini 4 Argon".to_owned()));
    assert!(!text_in(&rt, "compare-head-a").contains(&"Gemini 4 Argon Low".to_owned()));

    rt.update(|s, _| {
        s.resolved_model_names.clear();
        let route = &mut s.compare.compares[0].slots[0].route;
        route.model = "gemini-4-argon-high".into();
        route.label = None;
    });
    settle(&mut rt);
    assert!(text_in(&rt, "compare-head-a").contains(&"Gemini 4 Argon".to_owned()));
}

#[test]
fn compare_view_cursor_route_without_catalog_keeps_its_previous_name() {
    // case: before the catalogue answers, the route is named as before
    let (_dirs, store, mut rt) = window();
    let mut compare = recorded_comparison(&store);
    compare.slots[0].route = locust_core::compare::CompareRoute::new("cursor", "gemini-4-argon-low");
    compare.slots[0].route.label = Some("Gemini 4 Argon Low".into());
    rt.update(move |s, _| {
        s.compare.active_id = Some(compare.compare_id.clone());
        s.compare.compares = vec![compare];
    });
    settle(&mut rt);
    assert!(text_in(&rt, "compare-head-a").contains(&"Gemini 4 Argon Low".to_owned()));
}

fn whole_compare_window(blind: bool, long_answer: bool) -> (Dirs, Runtime<State>) {
    use locust_core::compare::{CompareRoute, CompareSlotId};
    let (dirs, store, mut rt) = window();
    let mut compare = recorded_comparison(&store);
    compare.blind = blind;
    compare.changes = true;
    for (slot, (model, label)) in compare.slots.iter_mut().zip([
        ("opencode/mimo-v2.6-flash-free", "Mimo V2.6 Flash"),
        ("opencode/ling-3.0-flash-fin-free", "Ling 3.0 Flash"),
    ]) {
        slot.route = CompareRoute { label: Some(label.into()), ..CompareRoute::new("opencode", model) };
    }
    rt.update(move |s, _| {
        s.compare.active_id = Some(compare.compare_id.clone());
        s.compare.changes_lines.insert((compare.compare_id.clone(), CompareSlotId::A), "+1273 -0 in 1 file".into());
        s.compare.compares = vec![compare];
        for (id, text) in [
            ("m_a1", "Done.".to_owned()),
            ("m_b1", if long_answer { "A line from the long answer.\n".repeat(140) } else { "Done.".to_owned() }),
        ] {
            s.compare.snapshots.insert(id.into(), ledger::Snapshot {
                items: vec![ledger::Item::Assistant { item_id: "reply".into(), text, r#final: true }],
                timings: vec![locust_core::time_marker::TurnTiming {
                    before_item: 0, first_at: Some("2026-10-05T00:00:00Z".into()),
                    last_at: Some("2026-10-05T01:18:00Z".into()),
                }],
                cost: Some(locust_core::benchmark::RunCost { usd: Some(2.18), ..Default::default() }),
                ..Default::default()
            });
        }
    });
    settle(&mut rt);
    (dirs, rt)
}

#[test]
fn compare_view_footer_wraps_between_whole_parts() {
    // case: keeps each of its foot's numbers whole, as parts the foot can wrap between
    let (_dirs, mut rt) = whole_compare_window(false, false);
    rt.set_viewport((600.0, 850.0), 1.0);
    rt.update(|s, _| s.layout_pref = "wide".into());
    settle(&mut rt);
    let parts: Vec<_> = (0..3).map(|index| {
        let key = format!("compare-number-a-{index}");
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key.as_str())).expect("a whole footer part")
    }).collect();
    assert_eq!(parts.iter().map(|part| part.text.as_deref().unwrap().trim_end_matches("\u{a0}·")).collect::<Vec<_>>(),
        ["+1273 -0 in 1 file", "1h 18m", "$2.18"]);
    assert!(parts[0].rect[1] < parts[2].rect[1], "the whole parts wrap between words: {:?}", parts.iter().map(|part| part.rect).collect::<Vec<_>>());
    let (foot, _) = rt.presented("compare-foot-a").unwrap();
    for part in parts {
        assert!(part.rect[0] + part.rect[2] <= foot[0] + foot[2] + 0.5, "{:?} overflows {foot:?}", part.rect);
    }
}

#[test]
fn compare_view_keeps_the_body_in_view_beside_a_longer_answer() {
    // case: holds everything it says in one body, the part that stays in view
    let (_dirs, mut rt) = whole_compare_window(false, true);
    assert_eq!(text_in(&rt, "compare-body-a"), ["Done."]);
    assert!(rt.find("compare-body-b").is_some());
    rt.update(|_, tx| tx.scroll_to_instant("compare-scroll", 0.0, 610.0));
    settle(&mut rt);
    let (port, _) = rt.presented("compare-scroll").unwrap();
    let (head, _) = rt.presented("compare-head-a").unwrap();
    let (body, _) = rt.presented("compare-body-a").unwrap();
    assert!(body[1] >= head[1] + head[3] && body[1] + body[3] < port[1] + port[3], "port {port:?}, head {head:?}, body {body:?}");
    assert_eq!(text_in(&rt, "compare-body-a"), ["Done."]);
}

fn column_name_hover(rt: &mut Runtime<State>) -> String {
    let (x, y) = rt.find("compare-name-text-a").unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.016);
    rt.frame(0.6);
    rt.elements().into_iter()
        .find(|e| !e.exiting && e.key.as_deref() == Some("compare-name-a-tooltip"))
        .and_then(|e| e.label)
        .unwrap_or_else(|| panic!("the name's hover at ({x}, {y}), pointer over it: {}", rt.state.hover.is_open("compare-name-a")))
}

#[test]
fn compare_view_column_hover_names_the_runtime() {
    // case: says its runtime in the name's hover, for the head too narrow to show it
    let (_dirs, mut rt) = whole_compare_window(false, false);
    assert_eq!(column_name_hover(&mut rt), "Mimo V2.6 Flash · OpenCode");
}

#[test]
fn compare_view_blind_column_hover_names_only_the_model() {
    // case: names no runtime in a blind head, hover or not
    let (_dirs, mut rt) = whole_compare_window(true, false);
    assert_eq!(column_name_hover(&mut rt), "Model A");
    assert!(!text_in(&rt, "compare-head-a").contains(&"OpenCode".to_owned()));
}

#[test]
fn compare_view_a_short_column_stays_under_the_heads() {
    // case: waits under the heads when it fits
    assert_eq!(compare_view::column_stick_at(46.0, 513.0, 200.0), 54.0);
}

#[test]
fn compare_view_a_long_column_keeps_its_end_at_the_bottom() {
    // case: stops with its end at the bottom edge when it is longer than the view
    let top = compare_view::column_stick_at(46.0, 513.0, 1400.0);
    assert!(top < 0.0);
    assert_eq!(top + 1400.0, 505.0);
}

#[test]
fn compare_view_an_empty_column_stays_under_the_heads() {
    // case: never sits lower than under the heads, however short
    assert_eq!(compare_view::column_stick_at(46.0, 513.0, 0.0), 54.0);
}

fn compare_column_ids(s: &State) -> Vec<String> {
    s.compare.compares[0].slots.iter().filter_map(|slot| slot.mission_ids.last().cloned()).collect()
}

fn age_compare_ledger(file: &Path) {
    std::fs::File::options().write(true).open(file).unwrap().set_times(
        std::fs::FileTimes::new().set_modified(std::time::SystemTime::now() - ledger::ACTIVE_WINDOW - Duration::from_secs(1))
    ).unwrap();
}

fn column_ledger_records(id: &str, event: &str) -> bool {
    let kind = format!("\"type\":\"{event}\"");
    native::find_ledger(id).and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|records| records.contains(&kind))
}

fn launch_fixture_comparison(rt: &mut Runtime<State>) {
    click(rt, "compare-mode-chip");
    click(rt, "compare-mode-compare");
    type_and_send(rt, "Synthetic prompt");
}

#[test]
fn compare_view_restores_both_silent_native_columns_as_running() {
    // case: restores both silent host-owned columns as running without an interrupted error
    let (_dirs, store, mut rt) = window();
    let _hooks = native::hooks::install(store, fake("m1-app-server.py", "interrupt"));
    launch_fixture_comparison(&mut rt);
    pump(&mut rt, "two live comparison column records", |s| {
        let Some(cmp) = s.compare.compares.first() else { return false };
        let running = locust_core::engine_api::api().running();
        cmp.slots.len() == 2 && cmp.slots.iter().all(|slot| slot.mission_ids.last().is_some_and(|id|
            running.contains(id) && column_ledger_records(id, "run.started")))
    });
    let ids = compare_column_ids(&rt.state);
    for id in &ids {
        age_compare_ledger(&native::find_ledger(id).unwrap());
    }
    rt.update(|s, _| {
        s.compare.snapshots.clear();
        compare_view::reload_compares(s);
        compare_view::poll(s);
    });
    settle(&mut rt);
    assert_eq!(compare_column_ids(&rt.state), ids);
    for (id, letter) in ids.iter().zip(["a", "b"]) {
        let snap = &rt.state.compare.snapshots[id];
        assert!(snap.running && !snap.interrupted);
        assert!(snap.recovery.is_none());
        assert!(text_in(&rt, &format!("compare-head-{letter}")).contains(&"working".to_owned()));
    }
}

fn compare_ledger(store: &Store, id: &str) -> PathBuf {
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{id}.jsonl"));
    let created = json!({"schemaVersion":27,"recordType":"mission.created","metadata":{
        "missionId":id,"runtime":"opencode","model":"opencode/nemotron-3-ultra-free",
        "prompt":"Add a discount code field.","createdAt":"2026-10-05T17:22:29.756Z"
    }});
    std::fs::write(&file, format!("{created}\n")).unwrap();
    file
}

fn append_compare_event(file: &Path, event: serde_json::Value) {
    use std::io::Write;
    let record = json!({"schemaVersion":27,"recordType":"mission.event","event":event});
    writeln!(std::fs::OpenOptions::new().append(true).open(file).unwrap(), "{record}").unwrap();
}

fn recorded_completion(id: &str) -> serde_json::Value {
    let mut event: serde_json::Value = serde_json::from_str(
        include_str!("../../../desktop/test/fixtures/compare-column-completed.json")
    ).unwrap();
    event["missionId"] = json!(id);
    event
}

fn reopen_recorded_compare(rt: &mut Runtime<State>, id: String) {
    rt.update(move |s, _| {
        s.compare.active_id = Some(id.clone());
        s.compare.loaded = false;
        compare_view::poll(s);
    });
    settle(rt);
}

#[test]
fn compare_view_leaves_unowned_interruption_as_an_interruption() {
    // case: leaves a genuinely interrupted record alone when the host owns no process
    let (_dirs, store, mut rt) = window();
    let cmp = recorded_comparison(&store);
    let file = compare_ledger(&store, "m_a1");
    age_compare_ledger(&file);
    reopen_recorded_compare(&mut rt, cmp.compare_id);
    let snap = &rt.state.compare.snapshots["m_a1"];
    assert!(!snap.running && snap.interrupted);
    assert!(text_in(&rt, "compare-head-a").contains(&"interrupted".to_owned()));
}

#[test]
fn compare_view_terminal_receipt_beats_the_engines_still_live_answer() {
    // case: keeps an already-held terminal receipt ahead of an older live history answer
    let (_dirs, store, mut rt) = window();
    let _hooks = native::hooks::install(store, fake("m1-app-server.py", "success"));
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            native::hooks::HOLD_WIND_DOWN.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let _release = Release;
    native::hooks::HOLD_WIND_DOWN.store(true, std::sync::atomic::Ordering::Relaxed);
    launch_fixture_comparison(&mut rt);
    pump(&mut rt, "both completed receipts", |s| {
        let Some(cmp) = s.compare.compares.first() else { return false };
        cmp.slots.len() == 2 && cmp.slots.iter().all(|slot| slot.mission_ids.last()
            .is_some_and(|id| column_ledger_records(id, "run.completed")))
    });
    let ids = compare_column_ids(&rt.state);
    assert!(ids.iter().all(|id| locust_core::engine_api::api().owns(id)));
    let id = rt.state.compare.active_id.clone().unwrap();
    reopen_recorded_compare(&mut rt, id.clone());
    let before = rt.state.compare.snapshots.clone();
    assert!(ids.iter().all(|id| !before[id].running));
    reopen_recorded_compare(&mut rt, id);
    for id in ids {
        let after = &rt.state.compare.snapshots[&id];
        assert_eq!(after.items, before[&id].items);
        assert!(!after.running && !after.interrupted && !after.failed);
    }
}

#[test]
fn compare_view_recovers_a_completion_recorded_before_the_column_is_opened() {
    // case: replays the recorded completion that arrived before history named its run
    let (_dirs, store, mut rt) = window();
    let file = compare_ledger(&store, "m_a1");
    let history = recorded_completion("m_a1");
    assert_eq!(history["payload"]["process"]["exitCode"], 0);
    append_compare_event(&file, history);
    let cmp = recorded_comparison(&store);
    reopen_recorded_compare(&mut rt, cmp.compare_id.clone());
    let receipt = rt.state.compare.snapshots["m_a1"].clone();
    assert!(!receipt.running && !receipt.interrupted && !receipt.failed);
    reopen_recorded_compare(&mut rt, cmp.compare_id);
    let after = &rt.state.compare.snapshots["m_a1"];
    assert_eq!(after.items, receipt.items);
    assert!(!after.running && !after.interrupted && !after.failed);
}

#[test]
fn compare_view_reads_the_ledger_in_order_through_completion() {
    // case: replays every queued update in order without replacing a completed ledger phase with running
    let (_dirs, store, mut rt) = window();
    let cmp = recorded_comparison(&store);
    let file = compare_ledger(&store, "m_a1");
    reopen_recorded_compare(&mut rt, cmp.compare_id.clone());
    assert!(rt.state.compare.snapshots["m_a1"].running);
    for (sequence, words) in [(1, "Ordered"), (2, " reply.")] {
        append_compare_event(&file, json!({"sequence":sequence,"occurredAt":"2026-10-05T17:22:31.000Z",
            "type":"message.delta","payload":{"itemId":"reply","operation":"append","text":words,"final":sequence == 2}}));
    }
    append_compare_event(&file, recorded_completion("m_a1"));
    reopen_recorded_compare(&mut rt, cmp.compare_id);
    assert!(text_in(&rt, "compare-body-a").contains(&"Ordered reply.".to_owned()));
    let snap = &rt.state.compare.snapshots["m_a1"];
    assert!(!snap.running && !snap.interrupted);
}

#[test]
fn compare_view_keeps_tail_updates_wired_after_each_comparison_reload() {
    // case: connects every history read and late known-run update to reload recovery
    let (_dirs, store, mut rt) = window();
    let cmp = recorded_comparison(&store);
    let a = compare_ledger(&store, "m_a1");
    let b = compare_ledger(&store, "m_b1");
    reopen_recorded_compare(&mut rt, cmp.compare_id.clone());
    assert_eq!(compare_column_ids(&rt.state), ["m_a1", "m_b1"]);
    assert!(rt.state.compare.snapshots["m_a1"].running && rt.state.compare.snapshots["m_b1"].running);
    append_compare_event(&b, recorded_completion("m_b1"));
    rt.update(|s, _| { compare_view::poll(s); });
    assert!(rt.state.compare.snapshots["m_a1"].running && !rt.state.compare.snapshots["m_b1"].running);
    reopen_recorded_compare(&mut rt, cmp.compare_id);
    append_compare_event(&a, recorded_completion("m_a1"));
    rt.update(|s, _| { compare_view::poll(s); });
    assert!(!rt.state.compare.snapshots["m_a1"].running && !rt.state.compare.snapshots["m_b1"].running);
}
