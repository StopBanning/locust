//! Carries apps/desktop/src/main/restored-folders-can-be-found-on-another-computer.test.ts: folders restored from
//! another computer's backup are kept whatever computer named them, and found again by name or git remote.
use super::*;
use crate::folders;
use crate::people::workspace_id;
use crate::store::Store;
use serde_json::json;

struct Setup {
    root: PathBuf,
    local: PathBuf,
    store: Store,
}
impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn setup() -> Setup {
    let root = std::env::temp_dir().join(format!("locust-restored-folders-{}", crate::invariants::unique_nanos()));
    let local = root.join("work").join("demo");
    std::fs::create_dir_all(&local).unwrap();
    let (native, electron) = (root.join("native"), root.join("electron"));
    std::fs::create_dir_all(&native).unwrap();
    std::fs::create_dir_all(&electron).unwrap();
    let store = Store::new(native, electron).unwrap();
    Setup { root, local, store }
}

fn id_of(path: &str) -> String {
    workspace_id(Path::new(path))
}

fn known(id: &str, path: &str, remote: Option<&str>) -> KnownFolder {
    KnownFolder {
        id: id.into(),
        path: path.into(),
        name: folder_name(path),
        previous_path: None,
        remote: remote.map(str::to_owned),
        last_used_at: None,
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

const PATHS: [&str; 2] = ["C:\\work\\demo", "/Users/old/work/demo"];

// case: keeps Windows and Mac paths and their names on either computer
#[test]
fn keeps_windows_and_mac_paths_and_their_names_on_either_computer() {
    let t = setup();
    let rows: Vec<_> = PATHS.iter().map(|p| json!({"id": id_of(p), "path": p})).collect();
    t.store.save_json(folders::FOLDERS_FILE, &json!({"schemaVersion": 1, "folders": rows})).unwrap();
    let listed: Vec<String> = folders::list(&t.store).unwrap().into_iter().map(|f| f.path).collect();
    assert_eq!(listed, PATHS);
    for path in PATHS {
        assert_eq!(folder_name(path), "demo");
        // teammate-store.ts isTeammateFolder: a teammate's folder from either computer is kept.
        let roster = json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_one", "name": "Wren", "role": "Custom", "hue": "teal", "createdAt": "2026-09-23T05:00:00.000Z", "folder": path}]});
        let read = crate::native::parse_roster(&roster.to_string()).unwrap();
        assert_eq!(read[0].folder.as_deref(), Some(path));
    }
    // Still not a path: a relative one, and a drive letter with nothing after it.
    for odd in ["work\\demo", "C:", "C:demo", "demo"] {
        t.store.save_json(folders::FOLDERS_FILE, &json!({"schemaVersion": 1, "folders": [{"id": id_of(odd), "path": odd}]})).unwrap();
        assert_eq!(folders::list(&t.store).unwrap(), [], "{odd}");
    }
    // A network share and a forward-slash drive are Windows paths too.
    for share in ["\\\\server\\share\\demo", "D:/work/demo"] {
        t.store.save_json(folders::FOLDERS_FILE, &json!({"schemaVersion": 1, "folders": [{"id": id_of(share), "path": share}]})).unwrap();
        assert_eq!(folders::list(&t.store).unwrap().len(), 1, "{share}");
    }
}

// case: offers a Windows folder on a Mac and a Mac folder on Windows without changing it first
#[test]
fn offers_a_windows_folder_on_a_mac_and_a_mac_folder_on_windows_without_changing_it_first() {
    let t = setup();
    for path in PATHS {
        folders::learn(&t.store, &[(id_of(path), path.to_owned())]).unwrap();
        let matches = find_restored_folders(&folders::list(&t.store).unwrap(), &t.root);
        assert_eq!(matches.iter().find(|m| m.path == path).and_then(|m| m.candidate.clone()), Some(path_text(&t.local)), "{path}");
        assert_eq!(folders::path_of(&t.store, &id_of(path)).unwrap().as_deref(), Some(path));
    }
}

// case: keeps the old conversation id after a chosen move and remembers the move after reopening
#[test]
fn keeps_the_old_conversation_id_after_a_chosen_move_and_remembers_the_move_after_reopening() {
    let t = setup();
    let old = "C:\\work\\demo";
    folders::learn(&t.store, &[(id_of(old), old.to_owned())]).unwrap();
    folders::move_folder(&t.store, &id_of(old), &t.local).unwrap();
    let reopened = Store::new(t.root.join("native"), t.root.join("electron")).unwrap();
    assert_eq!(folders::path_of(&reopened, &id_of(old)).unwrap(), Some(path_text(&t.local)));
    assert_eq!(folders::same_as(&reopened, Path::new(old)).unwrap(), path_text(&t.local));
    assert_eq!(folders::list(&reopened).unwrap()[0].previous_path.as_deref(), Some(old));
}

// case: leaves ambiguous folder names for the picker and uses a saved remote to distinguish them
#[test]
fn leaves_ambiguous_folder_names_for_the_picker_and_uses_a_saved_remote_to_distinguish_them() {
    let t = setup();
    let other = t.root.join("Projects").join("demo");
    std::fs::create_dir_all(other.join(".git")).unwrap();
    std::fs::write(other.join(".git").join("config"), "[remote \"origin\"]\n url = git@example.com:team/demo.git\n").unwrap();
    let folder = known("old", "C:\\work\\demo", None);
    assert_eq!(find_restored_folders(std::slice::from_ref(&folder), &t.root)[0].candidate, None);
    let by_remote = KnownFolder { remote: Some("https://example.com/team/demo".into()), ..folder.clone() };
    assert_eq!(find_restored_folders(&[by_remote], &t.root)[0].candidate, Some(path_text(&other)));
    let another = KnownFolder { remote: Some("https://example.com/another/demo".into()), ..folder.clone() };
    assert_eq!(find_restored_folders(&[another], &t.root)[0].candidate, None);
    let here = KnownFolder { path: path_text(&t.local), ..folder };
    assert_eq!(find_restored_folders(&[here], &t.root), []);
}

// case: starts a fresh agent session after a recorded folder move
#[test]
fn starts_a_fresh_agent_session_after_a_recorded_folder_move() {
    let created = json!({"recordType": "mission.created", "metadata": {"runtime": "codex", "continuesFrom": {"runtimeThreadId": "old-computer-session"}}});
    let moved = json!({"recordType": "event", "event": {"type": "adapter.diagnostic", "payload": {"code": "host.folder_moved"}}});
    assert_eq!(crate::native::prior_from_records(&[created.clone(), moved], false).unwrap().thread, None);
    assert_eq!(
        crate::native::prior_from_records(&[created], false).unwrap().thread.as_deref(),
        Some("old-computer-session")
    );
}

// The scan's own bounds, which Electron's cases do not reach.

#[test]
fn a_remote_is_the_same_repository_however_it_is_spelled() {
    for spelled in ["git@Example.com:team/demo.git", "ssh://git@example.com/team/demo", "https://example.com/team/demo/", " https://example.com/team/demo.git/ "] {
        assert_eq!(remote_key(spelled), "https://example.com/team/demo", "{spelled}");
    }
    let t = setup();
    std::fs::create_dir_all(t.local.join(".git")).unwrap();
    std::fs::write(t.local.join(".git").join("config"), "[core]\n url = not-this\n[remote \"upstream\"]\n url = nor-this\n[remote \"origin\"]\n\turl = git@example.com:team/demo.git\r\n").unwrap();
    assert_eq!(folder_remote(&t.local).as_deref(), Some("git@example.com:team/demo.git"));
    std::fs::write(t.local.join(".git").join("config"), "[remote \"origin\"]\n fetch = x\n[branch \"main\"]\n url = elsewhere\n").unwrap();
    assert_eq!(folder_remote(&t.local), None);
}

#[test]
fn a_start_folder_three_folders_down_is_looked_in_and_a_fourth_is_not() {
    let t = setup();
    let deep = t.root.join("code").join("a").join("b").join("deep");
    let deeper = t.root.join("src").join("a").join("b").join("c").join("deeper");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::create_dir_all(&deeper).unwrap();
    let found = find_restored_folders(&[known("d", "C:\\x\\Deep", None), known("e", "C:\\x\\deeper", None)], &t.root);
    assert_eq!(found[0].candidate, Some(path_text(&deep)));
    assert_eq!(found[1].candidate, None);
    // A start folder is itself a candidate.
    std::fs::create_dir_all(t.root.join("Developer")).unwrap();
    let found = find_restored_folders(&[known("w", "/Users/old/Developer", None)], &t.root);
    assert_eq!(found[0].candidate, Some(path_text(&t.root.join("Developer"))));
}

#[test]
fn hidden_folders_dependencies_the_library_and_links_are_never_looked_in() {
    let t = setup();
    for (skipped, tucked) in [(".hidden", "t1"), ("node_modules", "t2"), ("vendor", "t3"), ("Library", "t4")] {
        std::fs::create_dir_all(t.root.join("Documents").join(skipped).join(tucked)).unwrap();
    }
    let elsewhere = t.root.join("elsewhere").join("linked");
    std::fs::create_dir_all(&elsewhere).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(t.root.join("elsewhere"), t.root.join("Documents").join("link")).unwrap();
    let wanted: Vec<KnownFolder> = ["t1", "t2", "t3", "t4", "linked"].iter().map(|n| known(n, &format!("C:\\{n}"), None)).collect();
    for found in find_restored_folders(&wanted, &t.root) {
        assert_eq!(found.candidate, None, "{}", found.path);
    }
    // The same folders in plain sight are found.
    std::fs::create_dir_all(t.root.join("Documents").join("plain").join("t2")).unwrap();
    let found = find_restored_folders(&[known("t", "C:\\t2", None)], &t.root);
    assert_eq!(found[0].candidate, Some(path_text(&t.root.join("Documents").join("plain").join("t2"))));
}

#[test]
fn a_repository_is_a_candidate_by_its_remote_whatever_its_folder_is_called() {
    let t = setup();
    let renamed = t.root.join("work").join("renamed");
    std::fs::create_dir_all(renamed.join(".git")).unwrap();
    std::fs::write(renamed.join(".git").join("config"), "[remote \"origin\"]\n url = https://example.com/team/demo\n").unwrap();
    let found = find_restored_folders(&[known("r", "C:\\work\\demo", Some("git@example.com:team/demo.git"))], &t.root);
    assert_eq!(found[0].candidate, Some(path_text(&renamed)));
    // A name match without that remote is not offered for a folder that saved one.
    std::fs::remove_dir_all(&renamed).unwrap();
    let found = find_restored_folders(&[known("r", "C:\\work\\demo", Some("git@example.com:team/demo.git"))], &t.root);
    assert_eq!(found[0].candidate, None);
}

#[test]
fn the_scan_stops_at_two_thousand_folders() {
    let t = setup();
    let wide = t.root.join("work");
    for i in 0..2100 {
        std::fs::create_dir(wide.join(format!("f{i:04}"))).unwrap();
    }
    std::fs::create_dir_all(wide.join("f2099").join("late")).unwrap();
    std::fs::create_dir_all(wide.join("f0000").join("early")).unwrap();
    let found = find_restored_folders(&[known("a", "C:\\early", None), known("b", "C:\\late", None)], &t.root);
    assert_eq!(found[0].candidate, None, "past the limit, even early in name order, since the queue is full");
    assert_eq!(found[1].candidate, None);
    // Under the limit both are found.
    for i in 1..2099 {
        std::fs::remove_dir(wide.join(format!("f{i:04}"))).unwrap();
    }
    let found = find_restored_folders(&[known("a", "C:\\early", None), known("b", "C:\\late", None)], &t.root);
    assert_eq!(found[0].candidate, Some(path_text(&wide.join("f0000").join("early"))));
    assert_eq!(found[1].candidate, Some(path_text(&wide.join("f2099").join("late"))));
}

#[test]
fn a_teammates_own_folder_from_the_other_computer_is_looked_for_too() {
    let t = setup();
    let roster = json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_one", "name": "Wren", "role": "Custom", "hue": "teal", "createdAt": "2026-09-23T05:00:00.000Z", "folder": "C:\\work\\demo"}], "missionOwners": {}, "settings": {}});
    t.store.save_json("teammates.json", &roster).unwrap();
    let found = check(&t.store, &t.root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, id_of("C:\\work\\demo"));
    assert_eq!(found[0].candidate, Some(path_text(&t.local)));
}

#[test]
fn apply_takes_only_a_folder_offered_for_its_row_that_is_here_while_the_old_one_is_not() {
    let t = setup();
    let old = "C:\\work\\demo";
    let id = id_of(old);
    folders::learn(&t.store, &[(id.clone(), old.to_owned())]).unwrap();
    let here = path_text(&t.local);
    let offered: std::collections::HashMap<String, std::collections::HashSet<String>> = [(id.clone(), [here.clone()].into())].into();
    let home = path_text(&t.root);
    let chosen = vec![(id.clone(), here.clone())];
    assert_eq!(moves_for(&t.store, &chosen, &offered, Some(&home)), Ok(vec![FolderMove { id: id.clone(), path: here.clone(), old_path: old.into() }]));
    // Not offered for this row, or a row nobody listed: refused, with nothing said past the card's own line.
    let elsewhere = path_text(&t.root.join("work"));
    assert_eq!(moves_for(&t.store, &[(id.clone(), elsewhere)], &offered, Some(&home)), Err(None));
    assert_eq!(moves_for(&t.store, &[("ws_other".into(), here.clone())], &offered, Some(&home)), Err(None));
    // Offered for a row the registry no longer has.
    let stray: std::collections::HashMap<String, std::collections::HashSet<String>> = [("ws_gone".to_owned(), [here.clone()].into())].into();
    assert_eq!(moves_for(&t.store, &[("ws_gone".into(), here.clone())], &stray, Some(&home)), Err(Some(FOLDERS_CHANGED)));
    // The new folder gone again, or the old one back.
    std::fs::remove_dir_all(&t.local).unwrap();
    assert_eq!(moves_for(&t.store, &chosen, &offered, Some(&home)), Err(Some(FOLDERS_CHANGED)));
    std::fs::create_dir_all(&t.local).unwrap();
    let back = t.root.join("back");
    std::fs::create_dir_all(&back).unwrap();
    let back_id = id_of(&path_text(&back));
    folders::learn(&t.store, &[(back_id.clone(), path_text(&back))]).unwrap();
    let back_offered: std::collections::HashMap<String, std::collections::HashSet<String>> = [(back_id.clone(), [here.clone()].into())].into();
    assert_eq!(moves_for(&t.store, &[(back_id, here.clone())], &back_offered, Some(&home)), Err(Some(FOLDERS_CHANGED)));
    // The home folder itself is not a project folder.
    let home_offered: std::collections::HashMap<String, std::collections::HashSet<String>> = [(id.clone(), [home.clone()].into())].into();
    assert_eq!(moves_for(&t.store, &[(id.clone(), home.clone())], &home_offered, Some(&home)), Err(Some(FOLDERS_CHANGED)));
    // More rows than any card could list.
    let many = vec![(id, here); 2001];
    assert_eq!(moves_for(&t.store, &many, &offered, Some(&home)), Err(None));
}
