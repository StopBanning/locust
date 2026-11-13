//! Electron's locust-goes-into-the-claude-app-in-one-press.test.ts, against a stand-in for the Claude app's settings
//! file in a scratch folder (Sol's findings by number where they apply here).
use super::*;
use std::sync::{Arc, Mutex};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A Claude folder (optionally with a settings file), and this app's program to point at.
fn claude_folder(contents: Option<&str>) -> (Scratch, PathBuf, Entry) {
    let root = std::env::temp_dir().join(format!("locust-claude-app-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("Claude")).unwrap();
    let file = root.join("Claude").join("claude_desktop_config.json");
    if let Some(text) = contents {
        std::fs::write(&file, text).unwrap();
    }
    let exe = root.join("locust-native");
    std::fs::write(&exe, "").unwrap();
    let entry = Entry { command: exe.to_string_lossy().into(), args: vec!["--door-bridge".into(), root.join("door.json").to_string_lossy().into()] };
    (Scratch(root), file, entry)
}

fn connector(file: &Path, entry: &Entry) -> Connector {
    connector_with(file, entry, Box::new(|| None), Box::new(|| 1_000))
}

fn connector_with(file: &Path, entry: &Entry, started_at: Box<StartedAt>, now: Box<dyn Fn() -> i64 + Send + Sync>) -> Connector {
    Connector { file: Some(file.to_path_buf()), entry: entry.clone(), marks_file: file.parent().unwrap().parent().unwrap().join("claude-app.json"), started_at, relaunch: None, now }
}

fn json_of(file: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn entry_json(entry: &Entry) -> Value {
    json!({"command": entry.command, "args": entry.args, "env": {}})
}

#[test]
fn is_where_the_claude_app_keeps_it_on_windows_and_on_a_mac() {
    assert_eq!(config_file("win32", Some("C:\\Users\\a\\AppData\\Roaming"), Path::new("C:\\Users\\a")), Some(Path::new("C:\\Users\\a\\AppData\\Roaming").join("Claude").join("claude_desktop_config.json")));
    assert_eq!(config_file("darwin", None, Path::new("/Users/a")), Some(PathBuf::from("/Users/a/Library/Application Support/Claude/claude_desktop_config.json")));
    assert_eq!(config_file("linux", None, Path::new("/home/a")), None);
}

#[test]
fn adds_one_entry_beside_the_persons_own_settings_and_keeps_the_original_once() {
    let original = r#"{"coworkUserFilesPath":"C:\\Users\\a\\Claude","preferences":{"theme":"dark"},"mcpServers":{"notes":{"command":"notes.exe"}}}"#;
    let (_s, file, entry) = claude_folder(Some(original));
    let app = connector(&file, &entry);
    assert_eq!(app.status(), Connection::NotAdded { restart: false });
    app.add().unwrap();
    assert_eq!(json_of(&file), json!({"coworkUserFilesPath": "C:\\Users\\a\\Claude", "preferences": {"theme": "dark"}, "mcpServers": {"notes": {"command": "notes.exe"}, "locust": entry_json(&entry)}}));
    let backup = PathBuf::from(format!("{}{BACKUP_SUFFIX}", file.display()));
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
    assert_eq!(app.status(), Connection::Added { restart: false, stale: false });
    // Again: the original copy stays the first one.
    app.remove().unwrap();
    app.add().unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
}

#[test]
fn makes_the_file_when_claude_has_not_written_one_yet() {
    let (_s, file, entry) = claude_folder(None);
    connector(&file, &entry).add().unwrap();
    assert_eq!(json_of(&file), json!({"mcpServers": {"locust": entry_json(&entry)}}));
}

#[test]
fn leaves_a_file_it_cannot_read_exactly_as_it_is() {
    let broken = "{ not json";
    let (_s, file, entry) = claude_folder(Some(broken));
    let app = connector(&file, &entry);
    assert!(matches!(app.status(), Connection::Unreadable(_)));
    assert_eq!(app.add(), Err(UNREADABLE.to_owned()));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
}

#[test]
fn is_not_offered_where_the_claude_app_is_not_installed() {
    let (s, _file, entry) = claude_folder(None);
    let missing = s.0.join("NoClaude").join("claude_desktop_config.json");
    let app = connector(&missing, &entry);
    assert_eq!(app.status(), Connection::NoApp);
    assert!(app.add().is_err());
}

#[test]
fn overlapping_presses_happen_one_at_a_time_and_the_original_copy_is_never_overwritten() {
    let original = r#"{"preferences":{"theme":"dark"}}"#;
    let (_s, file, entry) = claude_folder(Some(original));
    let app = Arc::new(Mutex::new(connector(&file, &entry)));
    let all: Vec<_> = (0..4).map(|_| { let app = app.clone(); std::thread::spawn(move || app.lock().unwrap().add()) }).collect();
    assert!(all.into_iter().all(|t| t.join().unwrap().is_ok()));
    assert_eq!(json_of(&file), json!({"preferences": {"theme": "dark"}, "mcpServers": {"locust": entry_json(&entry)}}));
    assert_eq!(std::fs::read_to_string(format!("{}{BACKUP_SUFFIX}", file.display())).unwrap(), original);
}

#[test]
fn a_different_add_on_the_person_named_locust_is_never_changed_or_removed() {
    let theirs = r#"{"mcpServers":{"locust":{"command":"python","args":["my_locust_tool.py"]}}}"#;
    let (_s, file, entry) = claude_folder(Some(theirs));
    let app = connector(&file, &entry);
    assert_eq!(app.status(), Connection::Taken(TAKEN.into()));
    assert!(app.add().is_err() && app.remove().is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), theirs);
}

#[test]
fn the_file_keeps_its_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let (_s, file, entry) = claude_folder(Some("{}"));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    connector(&file, &entry).add().unwrap();
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn a_settings_file_that_is_a_link_is_written_at_its_target_and_stays_a_link() {
    let (s, file, entry) = claude_folder(None);
    let real = s.0.join("real.json");
    std::fs::write(&real, "{}").unwrap();
    std::os::unix::fs::symlink(&real, &file).unwrap();
    connector(&file, &entry).add().unwrap();
    assert!(std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
    assert_eq!(json_of(&real), json!({"mcpServers": {"locust": entry_json(&entry)}}));
}

#[test]
fn an_entry_for_a_locust_that_is_not_this_one_or_whose_files_are_gone_says_so() {
    let (s, file, entry) = claude_folder(None);
    // Electron's entry, its bridge script beside it: Locust's own, but another copy.
    let script = s.0.join("resources").join("locust-mcp-bridge.mjs");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::write(&script, "").unwrap();
    std::fs::write(&file, json!({"mcpServers": {"locust": {"command": entry.command, "args": [script.to_string_lossy(), "/data/locust-mcp-connection.json"]}}}).to_string()).unwrap();
    assert_eq!(connector(&file, &entry).status(), Connection::Added { restart: false, stale: true });
    connector(&file, &entry).add().unwrap();
    assert_eq!(connector(&file, &entry).status(), Connection::Added { restart: false, stale: false });
    std::fs::remove_file(&entry.command).unwrap();
    assert_eq!(connector(&file, &entry).status(), Connection::Added { restart: false, stale: true });
}

#[test]
fn claude_still_has_to_restart_after_locust_restarts_and_not_after_claude_restarted() {
    let (_s, file, entry) = claude_folder(Some("{}"));
    // Claude open since before the change.
    connector_with(&file, &entry, Box::new(|| Some(Some(500))), Box::new(|| 1_000)).add().unwrap();
    // A new Locust reads the marks: Claude, started at 500, has not read the change at 1000.
    assert_eq!(connector_with(&file, &entry, Box::new(|| Some(Some(500))), Box::new(|| 2_000)).status(), Connection::Added { restart: true, stale: false });
    // Claude started after the change.
    assert_eq!(connector_with(&file, &entry, Box::new(|| Some(Some(1_500))), Box::new(|| 2_000)).status(), Connection::Added { restart: false, stale: false });
    // Claude not running.
    assert_eq!(connector_with(&file, &entry, Box::new(|| None), Box::new(|| 2_000)).status(), Connection::Added { restart: false, stale: false });
}

#[test]
fn remove_while_claude_is_open_says_claude_keeps_locust_until_it_restarts() {
    let (_s, file, entry) = claude_folder(Some("{}"));
    let open = || connector_with(&file, &entry, Box::new(|| Some(Some(1))), Box::new(|| 1_000));
    open().add().unwrap();
    open().remove().unwrap();
    assert_eq!(open().status(), Connection::NotAdded { restart: true });
    assert!(connection_words(&open().status()).starts_with("Removed. The Claude app keeps using Locust until it restarts"));
}

#[test]
fn settings_with_a_number_json_cannot_keep_exactly_are_left_alone() {
    let exact_text = r#"{"big": 9007199254740993}"#;
    let (_s, file, entry) = claude_folder(Some(exact_text));
    let app = connector(&file, &entry);
    assert!(matches!(app.status(), Connection::Unreadable(_)));
    assert_eq!(app.add(), Err(INEXACT.to_owned()));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), exact_text);
}

#[test]
fn takes_out_locusts_entry_and_leaves_the_rest_and_drops_an_emptied_list() {
    let (_s, file, entry) = claude_folder(Some(r#"{"preferences":{"theme":"dark"},"mcpServers":{"notes":{"command":"notes.exe"}}}"#));
    let app = connector(&file, &entry);
    app.add().unwrap();
    app.remove().unwrap();
    assert_eq!(json_of(&file), json!({"preferences": {"theme": "dark"}, "mcpServers": {"notes": {"command": "notes.exe"}}}));
    assert_eq!(app.status(), Connection::NotAdded { restart: false });
    let (_s2, file2, entry2) = claude_folder(Some(r#"{"preferences":{}}"#));
    let app2 = connector(&file2, &entry2);
    app2.add().unwrap();
    app2.remove().unwrap();
    assert_eq!(json_of(&file2), json!({"preferences": {}}));
}
