//! Browser access rules for commit/land, routines/teammates/compare/rooms, and settings
//! (`shared/commit-and-land-from-a-browser.test.ts`, `shared/routines-and-rooms-from-a-browser.test.ts`,
//! `shared/settings-from-a-browser.test.ts`).

use crate::desk::{DeskAccess, WINDOW_ONLY, WINDOW_ONLY_SAYS, desk_access};
use crate::desk_host::{
    DeskAsk, DeskHandler, DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskTell, DeskValue,
    DeskWindow, desk_encode_args,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ---- commit-and-land-from-a-browser.test.ts -------------------------------------------------------------------------

// case: drafts and makes a commit, and lands a teammate’s branch or begins its merge
#[test]
fn drafts_and_makes_a_commit_and_lands_a_teammates_branch_or_begins_its_merge() {
    assert_eq!(
        desk_access("folder:commit", &[json!("Fix the header"), json!("commit"), json!(["a.ts"])]),
        Some(DeskAccess::Act)
    );
    for channel in ["folder:commit-draft", "worktrees:land", "worktrees:resolve"] {
        assert_eq!(desk_access(channel, &[json!("tm_one")]), Some(DeskAccess::Act), "{channel}");
        assert!(!WINDOW_ONLY.contains(&channel), "{channel}");
    }
}

// case: never pushes or opens a pull request, and never throws a branch away
#[test]
fn never_pushes_or_opens_a_pull_request_and_never_throws_a_branch_away() {
    assert_eq!(
        desk_access("folder:commit", &[json!("Fix the header"), json!("push"), json!(["a.ts"])]),
        None
    );
    assert_eq!(
        desk_access(
            "folder:commit",
            &[json!("Fix the header"), json!("pull-request"), json!(["a.ts"])]
        ),
        None
    );
    assert_eq!(desk_access("folder:commit", &[]), None);
    assert_eq!(desk_access("worktrees:remove", &[json!("tm_one")]), None);
    assert!(WINDOW_ONLY.contains(&"worktrees:remove"));
}

// ---- routines-and-rooms-from-a-browser.test.ts ----------------------------------------------------------------------

// case: leaves a called link on this computer, and still saves the hook kind
#[test]
fn leaves_a_called_link_on_this_computer_and_still_saves_the_hook_kind() {
    assert_eq!(desk_access("routine:hook-read", &[json!("rt_a")]), None);
    assert_eq!(desk_access("routine:hook-new", &[json!("rt_a")]), None);
    assert!(WINDOW_ONLY.contains(&"routine:hook-read"));
    assert!(WINDOW_ONLY.contains(&"routine:hook-new"));
    assert_eq!(
        desk_access("routines:update", &[json!({ "schedule": { "kind": "hook" } })]),
        Some(DeskAccess::Act)
    );
}

// case: makes, changes, pauses, runs and removes a routine, and settles what it was asked to
#[test]
fn makes_changes_pauses_runs_and_removes_a_routine_and_settles_what_it_was_asked_to() {
    for channel in [
        "routines:create",
        "routines:update",
        "routines:pause",
        "routines:run",
        "routines:remove",
        "routine:recovery",
    ] {
        assert_eq!(desk_access(channel, &[json!({})]), Some(DeskAccess::Act), "{channel}");
        assert!(!WINDOW_ONLY.contains(&channel), "{channel}");
    }
    assert_eq!(desk_access("routines:list", &[]), Some(DeskAccess::Read));
    assert_eq!(
        desk_access("routines:settle", &[json!({ "routineId": "r", "decision": "keep" })]),
        Some(DeskAccess::Act)
    );
    assert_eq!(
        desk_access("routines:settle", &[json!({ "routineId": "r", "decision": "discard" })]),
        Some(DeskAccess::Act)
    );
}

// case: leaves on the computer what opens something there
#[test]
fn leaves_on_the_computer_what_opens_something_there() {
    assert_eq!(
        desk_access("routines:settle", &[json!({ "routineId": "r", "decision": "open" })]),
        None
    );
    assert_eq!(desk_access("routines:settle", &[]), None);
    for channel in [
        "routines:chooseFolder",
        "routines:export",
        "routines:import",
        "routines:importPreview",
    ] {
        assert_eq!(desk_access(channel, &[json!({})]), None, "{channel}");
        assert!(WINDOW_ONLY.contains(&channel), "{channel}");
    }
}

// case: makes, changes and removes a teammate, but never what it may spend, its folder, connectors or skills
#[test]
fn makes_changes_and_removes_a_teammate_but_never_what_it_may_spend_its_folder_connectors_or_skills() {
    for channel in ["teammates:create", "teammates:remove"] {
        assert_eq!(desk_access(channel, &[json!({})]), Some(DeskAccess::Act), "{channel}");
    }
    assert_eq!(
        desk_access(
            "teammates:update",
            &[json!({ "teammateId": "t", "name": "Ada", "instructions": "Be brief." })]
        ),
        Some(DeskAccess::Act)
    );
    assert_eq!(
        desk_access(
            "teammates:update",
            &[json!({ "teammateId": "t", "name": "Ada", "monthlyLimitUsd": 500 })]
        ),
        None
    );
    assert_eq!(
        desk_access(
            "teammates:update",
            &[json!({ "teammateId": "t", "name": "Ada", "monthlyLimitUsd": null })]
        ),
        None
    );
    assert_eq!(desk_access("teammates:update", &[]), None);
    for channel in ["teammates:folder", "teammates:connectors", "teammate:skills-use"] {
        assert_eq!(desk_access(channel, &[json!({})]), None, "{channel}");
        assert!(WINDOW_ONLY.contains(&channel), "{channel}");
    }
}

// case: starts, asks, judges, runs again and keeps a comparison (0.835)
#[test]
fn starts_asks_judges_runs_again_and_keeps_a_comparison_0835() {
    assert_eq!(desk_access("compare:list", &[]), Some(DeskAccess::Read));
    for channel in [
        "compare:start",
        "compare:ask",
        "compare:add-model",
        "compare:retry",
        "compare:judge",
        "compare:draft-points",
        "compare:keep",
        "compare:rerun",
        "compare:rerun-checked",
        "compare:rerun-stop",
    ] {
        assert_eq!(desk_access(channel, &[json!({})]), Some(DeskAccess::Act), "{channel}");
        assert!(!WINDOW_ONLY.contains(&channel), "{channel}");
    }
}

// case: makes, renames, removes and posts to a room, and moves its board
#[test]
fn makes_renames_removes_and_posts_to_a_room_and_moves_its_board() {
    assert_eq!(desk_access("rooms:list", &[]), Some(DeskAccess::Read));
    for channel in [
        "rooms:create",
        "rooms:rename",
        "rooms:remove",
        "rooms:post",
        "rooms:task",
    ] {
        assert_eq!(desk_access(channel, &[json!({})]), Some(DeskAccess::Act), "{channel}");
        assert!(!WINDOW_ONLY.contains(&channel), "{channel}");
    }
}

// ---- settings-from-a-browser.test.ts --------------------------------------------------------------------------------

// case: changes the look, groups, memories, and Scout’s notes and tune-up
#[test]
fn changes_the_look_groups_memories_and_scouts_notes_and_tune_up() {
    for channel in [
        "appearance:set",
        "groups:create",
        "groups:rename",
        "groups:remove",
        "groups:instructions",
        "groups:route",
        "memory:add",
        "memory:update",
        "memory:remove",
        "memory:restore",
        "scout:notes",
        "scout:tune",
    ] {
        assert_eq!(desk_access(channel, &[json!({})]), Some(DeskAccess::Act), "{channel}");
        assert!(!WINDOW_ONLY.contains(&channel), "{channel}");
    }
}

// case: changes Scout’s settings, but never turns on its preparing fixes
#[test]
fn changes_scouts_settings_but_never_turns_on_its_preparing_fixes() {
    assert_eq!(
        desk_access("scout:set", &[json!({ "everyMinutes": 30, "quiet": null })]),
        Some(DeskAccess::Act)
    );
    assert_eq!(desk_access("scout:set", &[json!({ "fixes": true })]), None);
    assert_eq!(desk_access("scout:set", &[json!({ "fixes": false, "on": true })]), None);
    assert_eq!(desk_access("scout:set", &[]), None);
}

// case: leaves forgetting everything, and the folder’s own settings, on the computer
#[test]
fn leaves_forgetting_everything_and_the_folders_own_settings_on_the_computer() {
    for channel in ["memory:clear", "workspace-settings:write"] {
        assert_eq!(desk_access(channel, &[json!({})]), None, "{channel}");
        assert!(WINDOW_ONLY.contains(&channel), "{channel}");
    }
}

// ---- Caller check through DeskHost (core/src/desk_host.rs) ----------------------------------------------------------

#[test]
fn desk_host_enforces_commit_routine_teammate_and_scout_access_rules() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut handlers: HashMap<String, DeskHandler> = HashMap::new();
    for channel in [
        "folder:commit",
        "routines:settle",
        "teammates:update",
        "scout:set",
        "worktrees:remove",
    ] {
        let calls_c = calls.clone();
        handlers.insert(
            channel.to_owned(),
            Arc::new(move |_, _| {
                calls_c.fetch_add(1, Ordering::SeqCst);
                Ok(DeskValue::Json(json!({ "ok": true })))
            }),
        );
    }

    let told: Arc<Mutex<Vec<DeskTell>>> = Arc::new(Mutex::new(Vec::new()));
    let told_c = told.clone();
    let win = DeskWindow {
        destroyed: false,
        web_contents: DeskSender {
            id: 1,
            main_frame_id: 2,
            process_id: 3,
            destroyed: false,
        },
    };
    let host = DeskHost::new(DeskHostDeps {
        handlers: Arc::new(Mutex::new(handlers)),
        window: Arc::new(move || Some(win.clone())),
        tell: Arc::new(move |_, items| {
            told_c.lock().unwrap().extend_from_slice(items);
            true
        }),
        cover: Arc::new(|_| {}),
        hello: Arc::new(|| {
            Ok(DeskHelloInfo {
                version: "0.835.0".into(),
                computer: "DESK-PC".into(),
                theme: "dark".into(),
            })
        }),
        waiting: Arc::new(Vec::new),
        changed: None,
    });

    host.handle("dev-1", &[DeskAsk::Take { id: "take".into() }]);
    host.handle(
        "dev-1",
        &[
            DeskAsk::Call {
                id: "commit-ok".into(),
                channel: "folder:commit".into(),
                args: desk_encode_args(&[
                    DeskValue::Json(json!("Fix the header")),
                    DeskValue::Json(json!("commit")),
                    DeskValue::Json(json!(["a.ts"])),
                ]),
            },
            DeskAsk::Call {
                id: "push-refused".into(),
                channel: "folder:commit".into(),
                args: desk_encode_args(&[
                    DeskValue::Json(json!("Fix the header")),
                    DeskValue::Json(json!("push")),
                    DeskValue::Json(json!(["a.ts"])),
                ]),
            },
            DeskAsk::Call {
                id: "spend-refused".into(),
                channel: "teammates:update".into(),
                args: desk_encode_args(&[DeskValue::Json(
                    json!({ "teammateId": "t", "monthlyLimitUsd": 500 }),
                )]),
            },
            DeskAsk::Call {
                id: "scout-fixes-refused".into(),
                channel: "scout:set".into(),
                args: desk_encode_args(&[DeskValue::Json(json!({ "fixes": true }))]),
            },
            DeskAsk::Call {
                id: "remove-refused".into(),
                channel: "worktrees:remove".into(),
                args: desk_encode_args(&[DeskValue::Json(json!("tm_one"))]),
            },
        ],
    );
    host.flush();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let replies = told.lock().unwrap();
    assert!(replies.contains(&DeskTell::ResOk {
        id: "commit-ok".into(),
        value: r#"{"ok":true}"#.into(),
    }));
    for id in [
        "push-refused",
        "spend-refused",
        "scout-fixes-refused",
        "remove-refused",
    ] {
        assert!(
            replies.contains(&DeskTell::ResErr {
                id: id.into(),
                message: WINDOW_ONLY_SAYS.into(),
                refused: true,
            }),
            "expected refusal for {id}"
        );
    }
}
