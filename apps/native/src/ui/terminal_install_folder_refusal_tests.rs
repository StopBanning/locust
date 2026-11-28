use super::*;
use serde_json::json;
use std::{fs, path::PathBuf};

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn terminal_refuses_locust_install_folders_containing_app_asar() {
    let root = TempDir(std::env::temp_dir().join(format!(
        "locust-terminal-asar-{}",
        locust_core::invariants::unique_nanos()
    )));
    let install = root.0.join("LocustInstall");
    let nested = install.join("sub");
    let project = root.0.join("project");
    fs::create_dir_all(install.join("resources")).unwrap();
    fs::write(install.join("resources/app.asar"), b"").unwrap();
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&project).unwrap();

    assert_eq!(
        existing_folder(project.to_str().unwrap()),
        Some(project.canonicalize().unwrap()),
        "a normal project directory is accepted"
    );
    assert_eq!(
        existing_folder(install.to_str().unwrap()),
        None,
        "a Locust install folder holding resources/app.asar is refused"
    );
    assert_eq!(
        existing_folder(nested.to_str().unwrap()),
        None,
        "a folder nested inside a Locust install folder is also refused"
    );

    // Through the window's folder resolution: a teammate pointing inside an install folder resolves to None,
    // so opening a terminal or running a bang command refuses instead of starting inside Locust's install.
    let store = locust_core::store::Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [{"teammateId": "tm_wren", "name": "Wren", "folder": nested.to_string_lossy()}],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();
    let mut s = State::default();
    s.data = Some(store);
    s.teammates = vec![Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: String::new(),
        runtime: Some("claude".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(nested.to_string_lossy().into_owned()),
    }];
    s.selected = 0;
    assert_eq!(
        folder(&s),
        None,
        "terminal folder resolution refuses a teammate folder inside a Locust install"
    );
}
