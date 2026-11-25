//! Sidebar's `Cloud tasks` badge (`Sidebar.tsx:1149-1156`, `App.tsx:9372`):
//! shows the current folder's Codex Cloud task count in the sidebar rail when non-zero and opens the cloud panel.

use super::*;
use locust_core::cloud_tasks::Ran;
use rune::{Env, Key, Runtime};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct World {
    root: PathBuf,
    other: String,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fake_service() -> MakeCodex {
    Arc::new(|store: &Store| {
        let file = cloud_task_service::store_path_of(store);
        CloudTaskService::from_runners(
            file,
            move |args: &[&str], _cwd: &Path| match args.get(1).copied() {
                Some("exec") => Ok(Ran::ok("https://chatgpt.com/codex/tasks/task_e_sidebar1\n")),
                _ => Ok(Ran::ok("")),
            },
            move |args: &[&str], _cwd: &Path| match args.first().copied() {
                Some("remote") => Ok(Ran::ok("https://github.com/acme/widgets.git\n")),
                Some("rev-parse") => Ok(Ran::ok("main\n")),
                _ => Ok(Ran::ok("")),
            },
        )
        .with_ready(|| true)
    })
}

fn task_json(id: &str, folder: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "taskId": id,
        "url": format!("https://chatgpt.com/codex/tasks/{id}"),
        "runtime": "codex",
        "repo": "acme/widgets",
        "branch": "main",
        "prompt": title,
        "folder": folder,
        "createdAt": "2026-11-25T03:00:00.000Z",
        "status": { "state": "ready", "title": title }
    })
}

fn window(tasks: impl FnOnce(&str, &str) -> serde_json::Value) -> (World, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-cloud5-sidebar-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    std::fs::create_dir_all(root.join("other")).unwrap();
    let root = root.canonicalize().unwrap();
    let here = root.join("work").to_string_lossy().into_owned();
    let other = root.join("other").to_string_lossy().into_owned();

    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({
                "schemaVersion": 1,
                "teammates": [{"teammateId": "tm_codex", "name": "Chip"}],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();
    let payload = tasks(&here, &other);
    std::fs::write(
        cloud_task_service::store_path_of(&store),
        serde_json::to_string(&payload).unwrap(),
    )
    .unwrap();

    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    s.home.folder_path = Some(here.clone());
    s.cloud.codex.make = Some(fake_service());
    crate::apply_team(
        &mut s,
        vec![Teammate {
            id: "tm_codex".into(),
            name: "Chip".into(),
            role: "Coder".into(),
            runtime: Some("codex".into()),
            model: Some("gpt-5-codex".into()),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: Some(here.clone()),
        }],
    );
    let mut rt = Runtime::new(s, crate::view, (1280.0, 820.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (World { root, other }, rt)
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..10 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element keyed {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut now = 0.0;
    while !done(&rt.state) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        rt.update(move |s, tx| {
            crate::poll(s, now, tx);
        });
        rt.frame(0.05);
        now += 0.3;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(rt);
}

fn text_for(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements()
        .iter()
        .find(|e| e.key.as_deref() == Some(key))
        .and_then(|e| e.text.clone())
}

#[test]
fn sidebar_hides_cloud_tasks_badge_when_folder_has_no_tasks() {
    let (_world, mut rt) = window(|_, _| serde_json::json!([]));
    pump(&mut rt, "empty folder tasks listed", |s| {
        s.cloud.codex.listed_folder.is_some()
    });
    assert!(
        rt.find("sidebar-place-cloud-tasks").is_none(),
        "badge must stay hidden when this folder has 0 cloud tasks"
    );
}

#[test]
fn sidebar_shows_folder_cloud_task_count_and_opens_cloud_panel_from_another_screen() {
    let (_world, mut rt) = window(|here, other| {
        serde_json::json!([
            task_json("task_e_a1", here, "Fix billing"),
            task_json("task_e_a2", here, "Tighten tests"),
            task_json("task_e_other", other, "Other folder task"),
        ])
    });

    // Even before opening the cloud panel, poll loads this folder's task count for the sidebar badge.
    pump(&mut rt, "folder task count loaded", |s| {
        s.cloud.codex.tasks.len() == 2
    });
    assert!(
        !rt.state.cloud.open,
        "panel stays closed until the badge is clicked"
    );
    assert!(
        rt.find("sidebar-place-cloud-tasks").is_some(),
        "sidebar should show the Cloud tasks badge"
    );
    assert_eq!(
        text_for(&rt, "sidebar-cloud-tasks-count").as_deref(),
        Some("2"),
        "badge count must match only this folder's tasks"
    );

    // Move to another screen (Routines) and click the sidebar badge: it returns to workroom and opens the cloud panel.
    rt.update(|s, _| {
        s.routines.open = true;
    });
    settle(&mut rt);
    assert!(rt.state.routines.open);

    click(&mut rt, "sidebar-place-cloud-tasks");
    assert!(
        !rt.state.routines.open,
        "clicking Cloud tasks leaves the Routines screen"
    );
    assert!(
        rt.state.cloud.open,
        "clicking Cloud tasks opens the cloud panel"
    );
    assert!(
        rt.find("cloud-tasks-beside").is_some(),
        "cloud panel should be drawn beside the conversation"
    );
    assert!(rt.find("codex-task-task_e_a1").is_some());
    assert!(rt.find("codex-task-task_e_a2").is_some());
    assert!(rt.find("codex-task-task_e_other").is_none());
}

#[test]
fn sidebar_badge_appears_when_a_cloud_task_is_sent_and_updates_when_folder_changes() {
    let (world, mut rt) = window(|_, _| serde_json::json!([]));
    pump(&mut rt, "initial folder listed", |s| {
        s.cloud.codex.listed_folder.is_some()
    });
    assert!(rt.find("sidebar-place-cloud-tasks").is_none());

    // Start a task from the composer -> badge appears with count 1.
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    click(&mut rt, "composer");
    rt.paste("Ship sidebar badge");
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    settle(&mut rt);

    assert!(rt.find("sidebar-place-cloud-tasks").is_some());
    assert_eq!(
        text_for(&rt, "sidebar-cloud-tasks-count").as_deref(),
        Some("1")
    );

    // Close the panel and switch to another folder with no tasks -> badge disappears on poll.
    let other = world.other.clone();
    rt.update(move |s, _| {
        s.cloud.open = false;
        s.home.folder_path = Some(other.clone());
        if let Some(mate) = s.teammates.get_mut(0) {
            mate.folder = Some(other);
        }
    });
    pump(&mut rt, "switched folder listed", |s| {
        s.cloud.codex.tasks.is_empty()
    });
    assert!(
        rt.find("sidebar-place-cloud-tasks").is_none(),
        "badge should hide when switching to a folder with no cloud tasks"
    );
}
