//! Claude's cloud reached from the window (Cloud 3 row 1), drawn through the whole window (`crate::view`) and its
//! poll (`crate::poll`): the chat type menu's Cloud opens the panel beside the conversation, the Claude start form
//! offers the organization environment field, and a send with Cloud on starts a Claude cloud session instead of a
//! turn. Claude Code is never run: its window is recorded, never opened.

use super::*;
use locust_core::claude_cloud::{ClaudeLaunch, CloudPlatform, WindowRun};
use rune::{Env, Key, Runtime};
use std::path::{Path, PathBuf};

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mate(root: &Path, runtime: &str) -> Teammate {
    Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Coder".into(),
        runtime: Some(runtime.into()),
        model: Some("claude-sonnet-5-5".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(root.join("work").to_string_lossy().into()),
    }
}

/// Claude Code's windows, recorded.
type Windows = Arc<Mutex<Vec<WindowRun>>>;

fn fake_claude(windows: &Windows) -> MakeClaude {
    let windows = windows.clone();
    Arc::new(move |store: &Store, reads| {
        let windows = windows.clone();
        ClaudeCloud::new(locust_core::claude_cloud::store_path_of(store), CloudPlatform::Darwin, || {
            Some(ClaudeLaunch {
                discovered_path: "/fake/claude".into(),
                executable_path: "/fake/claude".into(),
                prefix_args: Vec::new(),
                env: None,
            })
        })
        .with_reads(reads)
        .with_run_window(move |run: &WindowRun| {
            windows.lock().unwrap().push(run.clone());
            Ok(())
        })
    })
}

fn window(runtime: &str, make: Option<MakeClaude>) -> (Dirs, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-cloud3-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    s.cloud.make = make;
    crate::apply_team(&mut s, vec![mate(&root, runtime)]);
    let mut rt = Runtime::new(s, crate::view, (1280.0, 820.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (Dirs(root), rt)
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..20 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn type_and_send(rt: &mut Runtime<State>, words: &str) {
    click(rt, "composer");
    rt.paste(words);
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    rt.frame(0.016);
}

/// The window's own poll, until `done`.
fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut now = 0.0;
    while !done(&rt.state) {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
        rt.update(move |s, tx| {
            crate::poll(s, now, tx);
        });
        rt.frame(0.05);
        now += 0.3;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(rt);
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n")
}

fn read(s: &State) -> bool {
    s.cloud.panel.place.is_some()
}

#[test]
// case: Only a Claude cloud start form offers the organization environment field.
fn only_a_claude_cloud_start_form_offers_the_organization_environment_field() {
    for (on, runtime, present) in [(true, "claude", true), (false, "claude", false), (true, "codex", false)] {
        let (_dirs, mut rt) = window(runtime, None);
        rt.update(move |s, _| s.cloud.on = on);
        settle(&mut rt);
        assert_eq!(words(&rt).contains("organization's environment (ID)"), present, "{on} {runtime}");
        assert_eq!(rt.find("cloud-environment").is_some(), present, "{on} {runtime}");
    }
}

#[test]
fn the_chat_type_menu_offers_cloud_for_a_claude_model_and_choosing_it_opens_the_panel_beside() {
    let windows = Windows::default();
    let (_dirs, mut rt) = window("claude", Some(fake_claude(&windows)));
    assert!(rt.find("cloud-tasks-beside").is_none());
    click(&mut rt, "compare-mode-chip");
    let menu = words(&rt);
    assert!(menu.contains("Runs in Claude’s cloud; follow it on claude.ai"), "{menu}");
    click(&mut rt, "compare-mode-cloud");
    assert!(rt.state.cloud.on);
    assert!(rt.find("cloud-tasks-beside").is_some(), "the panel opens beside the conversation");
    assert!(words(&rt).contains("Comparison: Cloud"), "the chip says the chat type");
    assert!(rt.find("cloud-environment").is_some(), "the Claude start form gains the environment field");
    // Where a send goes is read off the window's thread and lands through the window's poll: this folder is not on
    // GitHub, so the panel says a task from here starts with no project.
    pump(&mut rt, "where a send goes", read);
    let panel = words(&rt);
    assert!(panel.contains("work is not on GitHub. Claude’s cloud works on a copy of a GitHub repository"), "{panel}");
    // Direct sets it back; the panel stays until closed.
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-direct");
    assert!(!rt.state.cloud.on);
    assert!(rt.find("cloud-environment").is_none());
    click(&mut rt, "cloud-tasks-close");
    assert!(rt.find("cloud-tasks-beside").is_none());
}

#[test]
fn cloud_is_not_offered_for_an_agent_whose_cloud_is_not_built_here() {
    let (_dirs, mut rt) = window("codex", None);
    click(&mut rt, "compare-mode-chip");
    assert!(rt.find("compare-mode-compare").is_some());
    assert!(rt.find("compare-mode-cloud").is_none());
}

#[test]
fn a_send_with_cloud_on_starts_a_claude_cloud_session_on_the_typed_environment_instead_of_a_turn() {
    let windows = Windows::default();
    let (_dirs, mut rt) = window("claude", Some(fake_claude(&windows)));
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "where a send goes", read);
    click(&mut rt, "cloud-environment");
    rt.paste("ccpool_team");
    rt.frame(0.016);
    type_and_send(&mut rt, "Tidy the README");
    let runs = windows.lock().unwrap().clone();
    assert_eq!(runs.len(), 1, "Claude Code's window opened once");
    let args = runs[0].args.join(" ");
    assert!(args.contains("--model claude-sonnet-5-5"), "{args}");
    assert!(args.contains("--environment ccpool_team"), "{args}");
    assert!(args.ends_with("--cloud Tidy the README"), "{args}");
    assert!(runs[0].cwd.ends_with("work"), "{}", runs[0].cwd);
    let s = &rt.state;
    assert!(s.flight.is_none(), "no turn started");
    assert!(s.draft.text.is_empty(), "the words left the box");
    assert!(!s.cloud.on, "Cloud is one message's choice");
    assert!(s.cloud.open);
    let sessions = &s.cloud.panel.claude.as_ref().expect("Claude's side").sessions;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].prompt, "Tidy the README");
    assert_eq!(sessions[0].teammate_id.as_deref(), Some("tm_wren"));
}

#[test]
fn a_refused_send_is_said_over_the_box_and_keeps_the_words() {
    let windows = Windows::default();
    let (_dirs, mut rt) = window("claude", Some(fake_claude(&windows)));
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "where a send goes", read);
    click(&mut rt, "cloud-environment");
    rt.paste("bad & calc");
    rt.frame(0.016);
    type_and_send(&mut rt, "Tidy the README");
    assert!(windows.lock().unwrap().is_empty(), "nothing opened");
    let s = &rt.state;
    assert!(s.composer_note.as_deref().is_some_and(|n| n.starts_with("Use an environment ID beginning ccpool_")), "{:?}", s.composer_note);
    assert_eq!(s.draft.text, "Tidy the README");
    assert!(s.cloud.on);
    assert!(s.flight.is_none());
}

#[test]
fn the_panel_lists_this_folders_sessions_and_the_field_its_saved_environment_from_this_computers_claude() {
    // A session sent earlier, kept where this computer's Claude Code keeps them.
    let windows = Windows::default();
    let (_dirs, mut rt) = window("claude", None);
    let store = rt.state.data.clone().unwrap();
    let here = rt.state.teammates[0].folder.clone().unwrap();
    // Electron's own file, so sessions sent from Electron Locust are found here too.
    let electron_file = store.root().join("claude-cloud.json");
    let windows_too = windows.clone();
    let earlier = ClaudeCloud::new(electron_file, CloudPlatform::Darwin, || {
        Some(ClaudeLaunch { discovered_path: "/fake/claude".into(), executable_path: "/fake/claude".into(), prefix_args: Vec::new(), env: None })
    })
    .with_run_window(move |run: &WindowRun| {
        windows_too.lock().unwrap().push(run.clone());
        Ok(())
    });
    let choice = CloudChoice { environment: Some("ccpool_saved".into()), ..CloudChoice::default() };
    assert!(matches!(earlier.start(&here, "Earlier task", None, Some(&choice)), ClaudeCloudAnswer::Ok { .. }));
    // No fake here: the window reads them through `claude_cloud::on_this_computer`.
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "where a send goes", read);
    let sessions = &rt.state.cloud.panel.claude.as_ref().expect("Claude's side").sessions;
    assert_eq!(sessions.iter().map(|one| one.prompt.as_str()).collect::<Vec<_>>(), ["Earlier task"]);
    assert_eq!(rt.state.cloud.environment.text, "ccpool_saved");
    let drawn = rt.presented("cloud-environment").is_some();
    assert!(drawn && rt.find("cloud-environment-help").is_some());
}

#[test]
fn cloud_is_set_back_when_the_agent_is_no_longer_claude() {
    let (_dirs, mut rt) = window("claude", None);
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    assert!(rt.state.cloud.on);
    rt.update(|s, _| {
        s.picks.insert(
            "tm_wren".into(),
            TurnPick { model: "gpt-5".into(), effort: None, runtime: Some("codex".into()), account: None },
        );
    });
    pump(&mut rt, "Cloud set back", |s| !s.cloud.on);
    assert!(rt.find("cloud-environment").is_none());
}
