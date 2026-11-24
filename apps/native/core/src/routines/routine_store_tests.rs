//! Parity tests for `apps/desktop/src/main/routine-store.test.ts`.
use super::*;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

struct Temp {
    root: PathBuf,
    store: Store,
}
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-rt-store-{}",
            crate::native::uuid_v4().unwrap()
        ));
        let native = root.join("native");
        let electron = root.join("electron");
        fs::create_dir_all(&native).unwrap();
        fs::create_dir_all(&electron).unwrap();
        let folder = root.to_string_lossy().into_owned();
        fs::write(
            electron.join("teammates.json"),
            json!({
                "schemaVersion": 1,
                "teammates": [
                    {
                        "teammateId": "tm_wren",
                        "name": "Wren",
                        "hue": "lime",
                        "role": "Code & Migrations",
                        "createdAt": "2026-09-01T00:00:00.000Z",
                        "folder": folder,
                        "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"}
                    },
                    {
                        "teammateId": "tm_booty",
                        "name": "Booty",
                        "hue": "blue",
                        "role": "Docs & QA",
                        "createdAt": "2026-09-01T00:00:00.000Z",
                        "folder": folder,
                        "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"}
                    }
                ],
                "missionOwners": {},
                "settings": {}
            })
            .to_string(),
        )
        .unwrap();
        let store = Store::new(native, electron).unwrap();
        Self { root, store }
    }
    fn routines_path(&self) -> PathBuf {
        self.store.root().join("routines.json")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn route() -> Value {
    json!({"runtime": "cursor", "model": "composer-2.5", "mode": "ask"})
}

fn fresh(overrides: Value) -> Value {
    let mut base = json!({
        "name": "Nightly tidy",
        "teammateId": "tm_wren",
        "route": route(),
        "steps": ["Read status.ts and summarise it.", "List every file, one per line."],
        "learnedFrom": ["mission_1", "mission_2"]
    });
    if let (Some(dst), Some(src)) = (base.as_object_mut(), overrides.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    base
}

fn latest(store: &Store) -> Value {
    progress::all(store).unwrap().pop().unwrap()
}

#[test]
// case: a name is one line of printable text, bounded
fn a_name_is_one_line_of_printable_text_bounded() {
    assert!(valid_name(&json!("Nightly tidy")));
    assert!(valid_name(&json!("  spaced  ")));
    assert!(!valid_name(&json!("")));
    assert!(!valid_name(&json!("a".repeat(81))));
    assert!(!valid_name(&json!("two\nlines")));
    assert!(!valid_name(&json!(42)));
}

#[test]
// case: steps are the person's words: newlines allowed, empties and control bytes refused, count bounded
fn steps_are_the_persons_words_newlines_allowed_empties_and_control_bytes_refused_count_bounded() {
    assert!(valid_steps(&json!(["one", "two\nlines\tindented"])));
    assert!(!valid_steps(&json!([])));
    assert!(!valid_steps(&json!(["ok", "   "])));
    assert!(!valid_steps(&json!(["bell\x07"])));
    let too_many: Vec<&str> = vec!["step"; MAX_STEPS + 1];
    assert!(!valid_steps(&json!(too_many)));
    assert!(!valid_steps(&json!("not a list")));
}

#[test]
// case: keeps the folder a routine was made in, and drops one that is not a workspace id (M15)
fn keeps_the_folder_a_routine_was_made_in_and_drops_one_that_is_not_a_workspace_id() {
    let t = Temp::new();
    save(
        &t.store,
        fresh(json!({"workspaceId": "ws_0123456789abcdef0123456789abcdef"})),
    )
    .unwrap();
    let here_id = latest(&t.store)["routineId"].as_str().unwrap().to_owned();
    save(
        &t.store,
        fresh(json!({"teammateId": "tm_booty", "workspaceId": "../elsewhere"})),
    )
    .unwrap();
    let junk_id = latest(&t.store)["routineId"].as_str().unwrap().to_owned();

    assert_eq!(
        progress::get(&t.store, &here_id).unwrap().unwrap()["workspaceId"],
        "ws_0123456789abcdef0123456789abcdef"
    );
    assert!(
        progress::get(&t.store, &junk_id)
            .unwrap()
            .unwrap()
            .get("workspaceId")
            .is_none()
    );
}

#[test]
fn progress_drops_invalid_workspace_id_from_disk() {
    let t = Temp::new();
    save(&t.store, fresh(json!({}))).unwrap();
    let rid = latest(&t.store)["routineId"].as_str().unwrap().to_owned();
    let mut raw: Value = serde_json::from_str(&fs::read_to_string(t.routines_path()).unwrap()).unwrap();
    raw["routines"][0]["workspaceId"] = json!("../elsewhere");
    fs::write(t.routines_path(), raw.to_string()).unwrap();
    assert!(
        progress::get(&t.store, &rid)
            .unwrap()
            .unwrap()
            .get("workspaceId")
            .is_none()
    );
}

#[test]
// case: creates, lists, gets, corrects, counts runs and removes
fn creates_lists_gets_corrects_counts_runs_and_removes() {
    let t = Temp::new();
    save(&t.store, fresh(json!({}))).unwrap();
    let listed = progress::all(&t.store).unwrap();
    assert_eq!(listed.len(), 1);
    let routine = listed[0].clone();
    let rid = routine["routineId"].as_str().unwrap().to_owned();
    assert!(rid.starts_with("rt_"));
    assert_eq!(rid.len(), 27);
    assert!(rid[3..].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert_eq!(routine["runs"], 0);
    assert!(routine.get("lastRunAt").is_none());
    assert_eq!(progress::get(&t.store, &rid).unwrap().unwrap(), routine);

    save(
        &t.store,
        json!({"routineId": rid, "name": "Nightly tidy (v2)", "steps": ["Only this now."]}),
    )
    .unwrap();
    let corrected = progress::get(&t.store, &rid).unwrap().unwrap();
    let mut expected = routine.clone();
    expected["name"] = json!("Nightly tidy (v2)");
    expected["steps"] = json!(["Only this now."]);
    assert_eq!(corrected, expected);

    progress::save_progress(
        &t.store,
        &rid,
        &json!({
            "attemptId": "attempt_1",
            "step": 1,
            "of": 1,
            "status": "running",
            "missionId": "mission_final",
            "runId": "run_final",
            "workspaceId": "ws_test",
            "startedAt": routine["createdAt"],
            "updatedAt": routine["createdAt"],
            "steps": corrected["steps"],
            "route": corrected["route"]
        }),
        None,
    )
    .unwrap();
    progress::record_run(&t.store, &rid, "attempt_1", None, None).unwrap();
    let ran = progress::get(&t.store, &rid).unwrap().unwrap();
    assert_eq!(ran["runs"], 1);
    assert!(ran["lastRunAt"].is_string());

    remove(&t.store, &rid).unwrap();
    assert!(progress::all(&t.store).unwrap().is_empty());
}

#[test]
// case: refuses what would not round-trip, by name
fn refuses_what_would_not_round_trip_by_name() {
    let t = Temp::new();
    assert!(save(&t.store, fresh(json!({"name": ""}))).unwrap_err().contains("name"));
    assert!(save(&t.store, fresh(json!({"steps": []}))).unwrap_err().contains("steps"));
    assert!(
        save(&t.store, fresh(json!({"route": {"runtime": "cursor", "model": "x", "mode": "yolo"}})))
            .unwrap_err()
            .contains("route")
    );
    assert!(
        save(&t.store, fresh(json!({"teammateId": "../etc"})))
            .unwrap_err()
            .contains("Teammate id")
    );
    assert!(
        save(&t.store, fresh(json!({"learnedFrom": ["fine", "not fine/../"]})))
            .unwrap_err()
            .contains("provenance")
    );
    assert!(
        save(&t.store, json!({"routineId": "rt_missing", "name": "x", "steps": ["y"]}))
            .unwrap_err()
            .contains("not found")
    );
}

#[test]
// case: is bounded
fn is_bounded() {
    let t = Temp::new();
    let seeded: Vec<Value> = (0..MAX_ROUTINES)
        .map(|i| {
            json!({
                "routineId": format!("rt_seed{i}"),
                "name": format!("r{i}"),
                "teammateId": "tm_wren",
                "route": route(),
                "steps": ["Read status.ts and summarise it."],
                "learnedFrom": ["mission_1"],
                "createdAt": "2026-09-05T00:00:00.000Z",
                "runs": 0
            })
        })
        .collect();
    fs::write(
        t.routines_path(),
        json!({"schemaVersion": 1, "routines": seeded}).to_string(),
    )
    .unwrap();
    assert_eq!(progress::all(&t.store).unwrap().len(), MAX_ROUTINES);
    assert!(save(&t.store, fresh(json!({}))).unwrap_err().contains("Too many"));
}

#[test]
// case: drops every routine of a teammate who is gone, and only theirs
fn drops_every_routine_of_a_teammate_who_is_gone_and_only_theirs() {
    let t = Temp::new();
    save(&t.store, fresh(json!({"teammateId": "tm_wren"}))).unwrap();
    save(&t.store, fresh(json!({"teammateId": "tm_booty", "name": "Booty routine"}))).unwrap();
    let kept = progress::all(&t.store)
        .unwrap()
        .into_iter()
        .find(|r| r["teammateId"] == "tm_booty")
        .unwrap();

    // Removing the teammate through people::remove_teammate invokes routines::remove_for_teammate.
    crate::people::remove_teammate(&t.store, "tm_wren").unwrap();
    assert_eq!(progress::all(&t.store).unwrap(), vec![kept]);
}

#[test]
// case: writes a file a fresh store reads back identically
fn writes_a_file_a_fresh_store_reads_back_identically() {
    let t = Temp::new();
    save(&t.store, fresh(json!({}))).unwrap();
    let routine = latest(&t.store);
    let again = Store::new(t.store.root().to_path_buf(), t.store.electron().to_path_buf()).unwrap();
    assert_eq!(progress::all(&again).unwrap(), vec![routine]);
    let raw: Value = serde_json::from_str(&fs::read_to_string(t.routines_path()).unwrap()).unwrap();
    assert_eq!(raw["schemaVersion"], 1);
}

#[test]
// case: treats a damaged or foreign file as unreadable, and a damaged routine as absent
fn treats_a_damaged_or_foreign_file_as_unreadable_and_a_damaged_routine_as_absent() {
    assert_eq!(parsed_file("not json").unwrap_err(), UNREADABLE);
    assert_eq!(
        parsed_file(&json!({"schemaVersion": 2, "routines": []}).to_string()).unwrap_err(),
        UNREADABLE
    );
    let t = Temp::new();
    let torn = r#"{"schemaVersion":1,"routines":["#;
    fs::write(t.routines_path(), torn).unwrap();
    assert_eq!(list(&t.store).unwrap_err(), UNREADABLE);
    assert_eq!(save(&t.store, fresh(json!({}))).unwrap_err(), UNREADABLE);
    assert_eq!(fs::read_to_string(t.routines_path()).unwrap(), torn);

    let bad = json!({
        "schemaVersion": 1,
        "routines": [
            fresh(json!({"routineId": "rt_ok", "createdAt": "2026-09-05T00:00:00.000Z", "runs": 0})),
            fresh(json!({"routineId": "rt_ok", "createdAt": "2026-09-05T00:00:00.000Z", "runs": 0})),
            fresh(json!({"routineId": "rt_bad", "steps": [], "createdAt": "2026-09-05T00:00:00.000Z", "runs": 0})),
            fresh(json!({"routineId": "rt_neg", "createdAt": "2026-09-05T00:00:00.000Z", "runs": -1}))
        ]
    })
    .to_string();
    let ids: Vec<String> = parsed_file(&bad).unwrap()["routines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["routineId"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, vec!["rt_ok"]);

    let t2 = Temp::new();
    fs::write(t2.routines_path(), "{{{").unwrap();
    assert_eq!(list(&t2.store).unwrap_err(), UNREADABLE);
}

#[test]
// case: keeps a schedule, corrects it, clears it with null, and leaves it alone when unmentioned
fn keeps_a_schedule_corrects_it_clears_it_with_null_and_leaves_it_alone_when_unmentioned() {
    let t = Temp::new();
    save(&t.store, fresh(json!({"schedule": {"kind": "every", "hours": 4}}))).unwrap();
    let made = latest(&t.store);
    let rid = made["routineId"].as_str().unwrap().to_owned();
    assert_eq!(made["schedule"], json!({"kind": "every", "hours": 4}));

    save(
        &t.store,
        json!({"routineId": rid, "name": made["name"], "steps": made["steps"], "schedule": {"kind": "daily", "at": "07:30"}}),
    )
    .unwrap();
    assert_eq!(
        progress::get(&t.store, &rid).unwrap().unwrap()["schedule"],
        json!({"kind": "daily", "at": "07:30"})
    );

    save(
        &t.store,
        json!({"routineId": rid, "name": "Renamed", "steps": made["steps"]}),
    )
    .unwrap();
    assert_eq!(
        progress::get(&t.store, &rid).unwrap().unwrap()["schedule"],
        json!({"kind": "daily", "at": "07:30"})
    );

    save(
        &t.store,
        json!({"routineId": rid, "name": "Renamed", "steps": made["steps"], "schedule": null}),
    )
    .unwrap();
    assert!(progress::get(&t.store, &rid).unwrap().unwrap().get("schedule").is_none());
}

#[test]
// case: refuses a schedule it cannot keep, and drops a malformed one from disk without dropping the routine
fn refuses_a_schedule_it_cannot_keep_and_drops_a_malformed_one_from_disk_without_dropping_the_routine() {
    let t = Temp::new();
    assert!(
        save(&t.store, fresh(json!({"schedule": {"kind": "every", "hours": 0}})))
            .unwrap_err()
            .contains("schedule")
    );
    save(&t.store, fresh(json!({}))).unwrap();
    let made = latest(&t.store);
    let rid = made["routineId"].as_str().unwrap().to_owned();
    assert!(
        save(
            &t.store,
            json!({"routineId": rid, "name": made["name"], "steps": made["steps"], "schedule": {"kind": "daily", "at": "25:00"}})
        )
        .unwrap_err()
        .contains("schedule")
    );

    let mut raw: Value = serde_json::from_str(&fs::read_to_string(t.routines_path()).unwrap()).unwrap();
    raw["routines"][0]["schedule"] = json!({"kind": "weekly"});
    fs::write(t.routines_path(), raw.to_string()).unwrap();

    let back = progress::get(&t.store, &rid).unwrap().unwrap();
    assert_eq!(back["name"], "Nightly tidy");
    assert!(back.get("schedule").is_none());
}

#[test]
// case: round-trips it through the store
fn round_trips_it_through_the_store() {
    let t = Temp::new();
    save(
        &t.store,
        json!({
            "name": "Nightly sweep",
            "teammateId": "tm_wren",
            "route": {"runtime": "codex", "model": "gpt-5.6-sol", "mode": "ask", "effort": "high"},
            "steps": ["Summarise what changed"],
            "learnedFrom": ["mission_taught"]
        }),
    )
    .unwrap();
    let saved = latest(&t.store);
    assert_eq!(saved["route"]["effort"], "high");
    let listed = progress::all(&t.store).unwrap();
    let found = listed.iter().find(|r| r["routineId"] == saved["routineId"]).unwrap();
    assert_eq!(found["route"]["effort"], "high");
}

#[test]
// case: leaves a route without one alone
fn leaves_a_route_without_one_alone() {
    let t = Temp::new();
    save(
        &t.store,
        json!({
            "name": "Nightly sweep",
            "teammateId": "tm_wren",
            "route": {"runtime": "opencode", "model": "big-pickle", "mode": "ask"},
            "steps": ["Summarise what changed"],
            "learnedFrom": ["mission_taught"]
        }),
    )
    .unwrap();
    let saved = latest(&t.store);
    assert!(saved["route"].get("effort").is_none());
}
