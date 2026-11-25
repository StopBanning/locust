//! Carries `apps/desktop/src/main/every-folder-is-kept-by-its-path.test.ts` (all 9 cases).
use super::*;
use crate::people::workspace_id;
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_store() -> (Temp, Store) {
    let root = std::env::temp_dir().join(format!(
        "locust-folders-kept-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    ));
    let electron = root.join("electron");
    let native = root.join("native");
    std::fs::create_dir_all(&electron).unwrap();
    std::fs::create_dir_all(&native).unwrap();
    let store = Store::new(native, electron).unwrap();
    (Temp(root), store)
}

const WIN_PROJECT: &str = r"C:\Users\someone\Documents\Codex\locust-astra";

// case: is found from a file path its conversation named, JSON-escaped, deep inside it
#[test]
fn is_found_from_a_file_path_its_conversation_named_json_escaped_deep_inside_it() {
    let id = workspace_id(Path::new(WIN_PROJECT));
    let record = json!({
        "type": "tool",
        "input": { "file_path": format!(r"{WIN_PROJECT}\apps\desktop\src\main\index.ts") }
    })
    .to_string();
    assert_eq!(recover_folder_path(&id, &record).as_deref(), Some(WIN_PROJECT));
}

// case: is found from forward slashes too, as some runtimes write Windows paths
#[test]
fn is_found_from_forward_slashes_too_as_some_runtimes_write_windows_paths() {
    let id = workspace_id(Path::new(WIN_PROJECT));
    let text = r#""cwd":"C:/Users/someone/Documents/Codex/locust-astra/docs""#;
    assert_eq!(recover_folder_path(&id, text).as_deref(), Some(WIN_PROJECT));
}

// case: is never guessed: no path that hashes to it, no answer
#[test]
fn is_never_guessed_no_path_that_hashes_to_it_no_answer() {
    let id = workspace_id(Path::new(WIN_PROJECT));
    assert_eq!(
        recover_folder_path(&id, r#""C:\\Users\\someone\\Documents\\elsewhere\\a.txt""#),
        None
    );
    assert_eq!(recover_folder_path(&id, "no paths at all"), None);
}

// case: is found on macOS and Linux paths
#[test]
fn is_found_on_macos_and_linux_paths() {
    let unix = "/Users/someone/projects/site";
    let id = workspace_id(Path::new(unix));
    assert_eq!(
        recover_folder_path(&id, r#""path":"/Users/someone/projects/site/index.html""#).as_deref(),
        Some(unix)
    );
}

// case: keeps each folder once, newest used first, and what was only recovered after
#[test]
fn keeps_each_folder_once_newest_used_first_and_what_was_only_recovered_after() {
    let (temp, store) = temp_store();
    let a = temp.0.join("alpha");
    let b = temp.0.join("beta");
    let old = temp.0.join("old");
    use_folder(&store, &a, "2026-09-29T10:01:00.000Z").unwrap();
    use_folder(&store, &b, "2026-09-29T10:02:00.000Z").unwrap();
    use_folder(&store, &a, "2026-09-29T10:03:00.000Z").unwrap();
    learn(
        &store,
        &[
            (workspace_id(&old), old.to_string_lossy().into_owned()),
            (workspace_id(&a), "ignored".to_owned()),
        ],
    )
    .unwrap();
    let names: Vec<String> = list(&store).unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["alpha", "beta", "old"]);
    assert_eq!(
        path_of(&store, &workspace_id(&b)).unwrap().as_deref(),
        Some(b.to_string_lossy().as_ref())
    );
    // Written to disk, and read back by a fresh Store.
    let again = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let again_names: Vec<String> = list(&again).unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(again_names, ["alpha", "beta", "old"]);
}

fn row(mission_id: &str, folder_id: Option<&str>, last_at: &str) -> FolderRow {
    FolderRow {
        mission_id: mission_id.to_owned(),
        folder_id: folder_id.map(str::to_owned),
        last_at: last_at.to_owned(),
    }
}

// case: puts the window's folder first, then the others by their newest conversation
#[test]
fn puts_the_windows_folder_first_then_the_others_by_their_newest_conversation() {
    let sections = folder_sections_of(
        &[
            row("m1", Some("ws_old"), "2026-09-01"),
            row("m2", Some("ws_here"), "2026-09-02"),
            row("m3", Some("ws_recent"), "2026-09-20"),
            row("m4", None, "2026-09-29"),
        ],
        Some("ws_here"),
    );
    let ids: Vec<&str> = sections.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["ws_here", "ws_recent", "ws_old"]);
    let here_missions: Vec<&str> = sections[0].missions.iter().map(|m| m.mission_id.as_str()).collect();
    assert_eq!(here_missions, ["m2", "m4"]);
}

// case: is one section when there is one folder, so it draws as it always did
#[test]
fn is_one_section_when_there_is_one_folder_so_it_draws_as_it_always_did() {
    let sections = folder_sections_of(&[row("m1", Some("ws_here"), "2026-09-01")], Some("ws_here"));
    assert_eq!(sections.len(), 1);
}

// case: keeps the window's folder even with nothing in it yet: a folder just switched to
#[test]
fn keeps_the_windows_folder_even_with_nothing_in_it_yet_a_folder_just_switched_to() {
    let sections = folder_sections_of(&[row("m1", Some("ws_old"), "2026-09-01")], Some("ws_new"));
    let summary: Vec<(&str, usize)> = sections.iter().map(|s| (s.id.as_str(), s.missions.len())).collect();
    assert_eq!(summary, [("ws_new", 0), ("ws_old", 1)]);
}

// case: are told apart by the folder above each; a name nobody shares stays bare
#[test]
fn are_told_apart_by_the_folder_above_each_a_name_nobody_shares_stays_bare() {
    let make = |id: &str, path: &str, name: &str| KnownFolder {
        id: id.to_owned(),
        path: path.to_owned(),
        name: name.to_owned(),
        previous_path: None,
        remote: None,
        last_used_at: None,
    };
    let labels: Vec<String> = folder_labels(&[
        make("a", "C:/Users/someone/Documents/Locust", "Locust"),
        make("b", "C:/Users/someone/work/locust", "locust"),
        make("c", "/Users/someone/site", "site"),
    ])
    .into_iter()
    .map(|f| f.name)
    .collect();
    assert_eq!(labels, ["Locust (Documents)", "locust (work)", "site"]);
}
