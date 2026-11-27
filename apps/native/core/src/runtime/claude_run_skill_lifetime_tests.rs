//! A kept skill follows the real Claude mission and disappears only when its fake process finishes.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::{engine::Teammate, native, store::Store};

struct Temp(PathBuf);

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while !path.is_file() {
        assert!(Instant::now() < deadline, "did not get {}", path.display());
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
// case: leaves a registered run its skills until process completion owns their cleanup
fn leaves_a_registered_run_its_skills_until_process_completion_owns_their_cleanup() {
    let temp = Temp(std::env::temp_dir().join(format!(
        "locust-claude-skill-lifetime-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos(),
    )));
    let root = &temp.0;
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    let electron = root.join("electron");
    fs::create_dir_all(&electron).unwrap();
    let store = Store::new(root.join("native"), electron).unwrap();
    store.save_json(
        "teammates.json",
        &json!({"schemaVersion": 1, "teammates": [], "missionOwners": {}, "settings": {"autoMode": true}}),
    ).unwrap();

    let original = store.root().join("skill-library/skills/review/SKILL.md");
    let skill = "---\nname: review\ndescription: Review a change.\n---\nRead twice.\n";
    fs::create_dir_all(original.parent().unwrap()).unwrap();
    fs::write(&original, skill).unwrap();

    let fake = root.join("claude.py");
    fs::write(&fake, r#"
import json, pathlib, sys, time
root = pathlib.Path(sys.argv[1])
turn = json.loads(sys.stdin.readline())
assert turn["type"] == "user"
tmp = root / "argv.tmp"
tmp.write_text(json.dumps(sys.argv[3:]))
tmp.replace(root / "argv.json")
deadline = time.monotonic() + int(sys.argv[2])
while not (root / "finish").exists():
    if time.monotonic() >= deadline:
        raise SystemExit("fixture never received its finish marker")
    time.sleep(0.01)
print(json.dumps({"type":"result","subtype":"success","is_error":False,"result":"Read twice."}), flush=True)
"#).unwrap();
    let _guard = native::hooks::install(
        store.clone(),
        Some((
            PathBuf::from("python3"),
            vec![
                fake.to_string_lossy().into_owned(),
                root.to_string_lossy().into_owned(),
                (crate::load_waits::UNDER_LOAD.as_secs() + 2).to_string(),
            ],
        )),
    );

    let (observed, after_disk) = mpsc::channel();
    let scratch = store.root().join("claude-skills");
    *crate::invariants::lock(&native::hooks::ON_AFTER_DISK) = Some(Box::new(move || {
        let remaining = fs::read_dir(scratch).unwrap().count();
        let _ = observed.send(remaining);
    }));

    let started = native::send(native::Send {
        teammate: Teammate {
            id: "tm_review".into(),
            name: "Review".into(),
            role: "Read".into(),
            runtime: Some("claude".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "auto".into(),
            folder: Some(work.to_string_lossy().into_owned()),
        },
        conversation: None,
        prompt: "Review the change.".into(),
        model: None,
        effort: None,
    })
    .unwrap();

    let argv_path = root.join("argv.json");
    wait_for_file(&argv_path);
    let argv: Vec<String> = serde_json::from_slice(&fs::read(argv_path).unwrap()).unwrap();
    let plugin_dirs: Vec<_> = argv
        .windows(2)
        .filter(|args| args[0] == "--plugin-dir")
        .map(|args| Path::new(&args[1]))
        .collect();
    assert_eq!(plugin_dirs.len(), 1, "{argv:?}");
    let plugin = plugin_dirs[0];
    assert_eq!(plugin.file_name().unwrap(), "library");
    assert!(!argv.iter().any(|arg| arg == "--add-dir"), "{argv:?}");
    assert!(native::owns(&started.conversation_id));
    let manifest: Value =
        serde_json::from_slice(&fs::read(plugin.join(".claude-plugin/plugin.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["name"], "library");
    assert_eq!(
        fs::read_to_string(plugin.join("skills/review/SKILL.md")).unwrap(),
        skill
    );

    fs::write(root.join("finish"), "").unwrap();
    assert_eq!(
        after_disk
            .recv_timeout(crate::load_waits::UNDER_LOAD)
            .unwrap(),
        0,
        "Claude's copies must be gone when the completed turn starts its disk observation"
    );
    assert!(!plugin.exists());
    assert_eq!(fs::read_to_string(original).unwrap(), skill);
    let lines = fs::read_to_string(store.write_path(&started.conversation_id).unwrap()).unwrap();
    assert!(
        lines
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .any(|line| line["event"]["type"] == "run.completed")
    );
}
