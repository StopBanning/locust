//! The engine keeps each Claude turn's launch for the context menu (codex-mission.ts `claudeContext.remember`), as the
//! turn starts: a real start, on the fake Claude, read back from the store.
use crate::native::{Origin, Send, hooks, nobody, owns, send_from, set_open_folder};
use crate::store::Store;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_claude_turn_keeps_its_launch_for_the_context_menu_with_its_own_flags() {
    let t = Temp(std::env::temp_dir().join(format!("locust-context-kept-{}-{}", std::process::id(), crate::invariants::unique_nanos())));
    fs::create_dir_all(t.0.join("electron/mission-ledger")).unwrap();
    fs::create_dir_all(t.0.join("work")).unwrap();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("scripts/fixtures/m3-claude.py");
    let _guard = hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![fixture.to_string_lossy().into_owned(), "success".into()])));
    set_open_folder(&store, &t.0.join("work")).unwrap();

    let started = send_from(
        Send {
            teammate: nobody(Some("claude".into()), Some("opus".into())),
            conversation: None,
            prompt: "Reply with just the word ok.".into(),
            model: Some("opus".into()),
            effort: None,
        },
        Origin { started_by: Some(json!({"kind": "app", "name": "test"})), ..Default::default() },
    )
    .unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while owns(&started.conversation_id) {
        assert!(Instant::now() < deadline, "the turn never finished");
        std::thread::sleep(Duration::from_millis(10));
    }

    let kept: Value = serde_json::from_str(&fs::read_to_string(store.root().join("claude-context-launches.json")).expect("kept")).unwrap();
    let launch = &kept[started.conversation_id.as_str()];
    assert_eq!(launch["cwd"], json!(t.0.join("work").to_string_lossy()));
    assert_eq!(launch["sandbox"], json!("read-only"), "Ask runs read-only");
    let args: Vec<&str> = launch["args"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    // The turn's own Claude flags, model included.
    assert!(args.contains(&"--print"), "{args:?}");
    assert!(args.windows(2).any(|w| w == ["--model", "opus"]), "{args:?}");
    // Ask goes through the permission bridge: its config is named so a read can make it again.
    let permission = launch["permission"].as_str().expect("the bridge's config");
    assert!(args.contains(&permission), "{args:?}");
    // Paths and flags only: nothing of the environment.
    assert!(launch.get("env").is_none());
}
