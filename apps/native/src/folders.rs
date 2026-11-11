//! Remembered conversation folders — port of `main/folders.ts`.

use crate::people::workspace_id;
use crate::store::Store;
use serde_json::{Value, json};
use std::path::Path;

pub const FOLDERS_FILE: &str = "folders.json";
pub const UNKNOWN_FOLDER: &str = "Unknown folder";
pub const CHOOSE_EXISTING: &str = "Choose an existing folder";
pub const NO_FOLDER_CHOSEN: &str = "No folder chosen";
pub const CHOOSE_FOLDER: &str = "Choose folder";
pub const FOLDER_LEDE: &str =
    "Every teammate works inside one project folder. Pick it before the first conversation.";
pub const MADE_FOLDER: &str = "Locust made a folder to work in";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownFolder {
    pub id: String,
    pub path: String,
    pub name: String,
    pub previous_path: Option<String>,
    pub remote: Option<String>,
    pub last_used_at: Option<String>,
}

pub fn folder_name(path: &str) -> String {
    path.trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
        .to_owned()
}

// Electron's path.resolve keeps a symlink's spelling; identity checks are separate.
fn absolute_path(path: &Path) -> Result<std::path::PathBuf, String> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut full = std::path::PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                full.pop();
            }
            std::path::Component::CurDir => {}
            other => full.push(other),
        }
    }
    Ok(full)
}

/// Recover a folder path from absolute paths in conversation text that hash to `id`.
pub fn recover_folder_path(id: &str, text: &str) -> Option<String> {
    let mut tried = std::collections::HashSet::new();
    let re = regex::Regex::new(
        r#"[A-Za-z]:(?:\\\\|\\|/)[^"\s<>|*?\r\n]+|/(?:Users|home|Volumes|mnt|opt|srv|tmp|var)/[^"\s<>|*?\r\n]+"#,
    )
    .ok()?;
    for found in re.find_iter(text) {
        let mut path = found.as_str().replace("\\\\", "\\");
        if path.chars().nth(1) == Some(':') {
            path = path.replace('/', "\\");
        }
        loop {
            if tried.insert(path.clone()) && workspace_id(Path::new(&path)) == id {
                return Some(path);
            }
            let parent = if path.chars().nth(1) == Some(':') {
                windows_parent(&path)
            } else {
                Path::new(&path)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
            };
            match parent {
                Some(p) if p != path => path = p,
                _ => break,
            }
        }
    }
    None
}

fn windows_parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('\\');
    let cut = trimmed.rfind('\\')?;
    if cut <= 2 {
        return None;
    }
    Some(trimmed[..cut].to_owned())
}

fn identity_of(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_dir() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let ino = meta.ino();
        if ino == 0 {
            return None;
        }
        Some(format!("{}:{ino}", meta.dev()))
    }
    #[cfg(not(unix))]
    {
        Some(path.to_string_lossy().into_owned())
    }
}

fn parse_folders(value: &Value) -> Vec<KnownFolder> {
    value
        .get("folders")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?.to_owned();
            let path = entry.get("path")?.as_str()?.to_owned();
            if !Path::new(&path).is_absolute() {
                return None;
            }
            Some(KnownFolder {
                id,
                name: folder_name(&path),
                previous_path: entry
                    .get("previousPath")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                remote: entry
                    .get("remote")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                last_used_at: entry
                    .get("lastUsedAt")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                path,
            })
        })
        .collect()
}

fn to_json(folders: &[KnownFolder]) -> Value {
    json!({
        "schemaVersion": 1,
        "folders": folders.iter().map(|f| {
            let mut row = json!({
                "id": f.id,
                "path": f.path,
                "name": f.name,
            });
            if let Some(prev) = &f.previous_path {
                row["previousPath"] = json!(prev);
            }
            if let Some(remote) = &f.remote {
                row["remote"] = json!(remote);
            }
            if let Some(at) = &f.last_used_at {
                row["lastUsedAt"] = json!(at);
            }
            row
        }).collect::<Vec<_>>()
    })
}

pub fn load(store: &Store) -> Result<Vec<KnownFolder>, String> {
    let value = store.seeded_json(FOLDERS_FILE, json!({"schemaVersion": 1, "folders": []}))?;
    Ok(parse_folders(&value))
}

pub fn save(store: &Store, folders: &[KnownFolder]) -> Result<(), String> {
    store.save_json(FOLDERS_FILE, &to_json(folders))
}

pub fn list(store: &Store) -> Result<Vec<KnownFolder>, String> {
    let mut folders = load(store)?;
    folders.sort_by(|a, b| {
        b.last_used_at
            .as_deref()
            .unwrap_or("")
            .cmp(a.last_used_at.as_deref().unwrap_or(""))
    });
    Ok(folders)
}

pub fn path_of(store: &Store, id: &str) -> Result<Option<String>, String> {
    Ok(load(store)?
        .into_iter()
        .find(|f| f.id == id)
        .map(|f| f.path))
}

pub fn use_folder(store: &Store, path: &Path, now: &str) -> Result<KnownFolder, String> {
    let full = absolute_path(path)?.to_string_lossy().into_owned();
    let entry = KnownFolder {
        id: workspace_id(Path::new(&full)),
        path: full.clone(),
        name: folder_name(&full),
        previous_path: None,
        remote: None,
        last_used_at: Some(now.to_owned()),
    };
    let mut folders = load(store)?;
    folders.retain(|f| f.id != entry.id);
    folders.push(entry.clone());
    save(store, &folders)?;
    Ok(entry)
}

pub fn learn(store: &Store, found: &[(String, String)]) -> Result<(), String> {
    let mut folders = load(store)?;
    for (id, path) in found {
        if folders.iter().any(|f| f.id == *id) {
            continue;
        }
        folders.push(KnownFolder {
            id: id.clone(),
            path: path.clone(),
            name: folder_name(path),
            previous_path: None,
            remote: None,
            last_used_at: None,
        });
    }
    save(store, &folders)
}

pub fn move_folder(store: &Store, id: &str, path: &Path) -> Result<(), String> {
    let full = absolute_path(path)?.to_string_lossy().into_owned();
    if identity_of(Path::new(&full)).is_none() {
        return Err(CHOOSE_EXISTING.into());
    }
    let mut folders = load(store)?;
    let Some(old) = folders.iter().find(|f| f.id == id).cloned() else {
        return Err(UNKNOWN_FOLDER.into());
    };
    for entry in &mut folders {
        if entry.id == id {
            entry.previous_path = Some(old.previous_path.clone().unwrap_or(old.path.clone()));
            entry.path = full.clone();
            entry.name = folder_name(&full);
        }
    }
    save(store, &folders)
}

/// Same folder however spelled: inode identity, or previousPath match.
pub fn same_as(store: &Store, path: &Path) -> Result<String, String> {
    let folders = load(store)?;
    let path_str = path.to_string_lossy();
    if let Some(moved) = folders
        .iter()
        .find(|f| f.previous_path.as_deref() == Some(path_str.as_ref()))
    {
        return Ok(moved.path.clone());
    }
    let full = absolute_path(path)?.to_string_lossy().into_owned();
    if folders.iter().any(|f| f.path == full) {
        return Ok(full);
    }
    let Some(identity) = identity_of(Path::new(&full)) else {
        return Ok(full);
    };
    Ok(folders
        .iter()
        .find(|f| identity_of(Path::new(&f.path)).as_deref() == Some(identity.as_str()))
        .map(|f| f.path.clone())
        .unwrap_or(full))
}

pub fn same_folder_at_start(file: &Path, path: &Path) -> String {
    let full = absolute_path(path)
        .unwrap_or_else(|_| path.to_owned())
        .to_string_lossy()
        .into_owned();
    let Ok(text) = std::fs::read_to_string(file) else {
        return full;
    };
    let Ok(body) = serde_json::from_str::<Value>(&text) else {
        return full;
    };
    let known: Vec<String> = parse_folders(&body).into_iter().map(|f| f.path).collect();
    if known.iter().any(|p| p == &full) {
        return full;
    }
    let Some(identity) = identity_of(Path::new(&full)) else {
        return full;
    };
    known
        .into_iter()
        .find(|entry| identity_of(Path::new(entry)).as_deref() == Some(identity.as_str()))
        .unwrap_or(full)
}

pub fn made_folder_note(path: &str) -> String {
    format!(
        "Teammates will work in {path} — it is new and empty. Point them at your own project instead."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pair() -> (PathBuf, Store) {
        let root = std::env::temp_dir().join(format!(
            "locust-folders-{}",
            crate::invariants::unique_nanos()
        ));
        let electron = root.join("electron");
        let native = root.join("native");
        std::fs::create_dir_all(&electron).expect("e");
        std::fs::create_dir_all(&native).expect("n");
        let store = Store::new(native, electron).expect("store");
        (root, store)
    }

    #[test]
    fn folder_name_and_words() {
        assert_eq!(folder_name("/Users/colin/work/locust"), "locust");
        assert_eq!(UNKNOWN_FOLDER, "Unknown folder");
        assert_eq!(CHOOSE_EXISTING, "Choose an existing folder");
        assert_eq!(NO_FOLDER_CHOSEN, "No folder chosen");
        assert_eq!(CHOOSE_FOLDER, "Choose folder");
        assert_eq!(MADE_FOLDER, "Locust made a folder to work in");
        assert!(made_folder_note("/tmp/x").contains("/tmp/x"));
    }

    #[test]
    fn recover_finds_path_that_hashes_to_id() {
        let path = "/Users/colin/work/demo-project";
        let id = workspace_id(Path::new(path));
        let text = format!("opened {path}/src/main.rs and also /tmp/other");
        assert_eq!(recover_folder_path(&id, &text).as_deref(), Some(path));
        assert!(recover_folder_path("ws_nope", &text).is_none());
    }

    #[test]
    fn use_list_and_same_as_round_trip() {
        let (root, store) = temp_pair();
        let project = root.join("project");
        std::fs::create_dir_all(&project).expect("project");
        let entry = use_folder(&store, &project, "2026-10-04T01:00:00.000Z").expect("use");
        assert_eq!(entry.name, "project");
        let listed = list(&store).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, entry.id);
        assert_eq!(
            path_of(&store, &entry.id).expect("path").as_deref(),
            Some(entry.path.as_str())
        );
        assert_eq!(same_as(&store, &project).expect("same"), entry.path);
        assert_eq!(
            move_folder(&store, "missing", &project).unwrap_err(),
            UNKNOWN_FOLDER
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn same_folder_at_start_reads_file() {
        let (root, store) = temp_pair();
        let project = root.join("alpha");
        std::fs::create_dir_all(&project).expect("p");
        let entry = use_folder(&store, &project, "2026-10-04T01:00:00.000Z").expect("use");
        let file = store.root().join(FOLDERS_FILE);
        assert_eq!(same_folder_at_start(&file, &project), entry.path);
        learn(
            &store,
            &[("ws_extra".into(), root.join("beta").display().to_string())],
        )
        .expect("learn");
        assert!(list(&store).expect("list").len() >= 2);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn registry_operations_leave_electron_bytes_unchanged_and_keep_link_spelling() {
        let root = { static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let root = std::env::temp_dir().join(format!("home-registry-review-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))); let _ = std::fs::remove_dir_all(&root); root };
        let electron = root.join("electron");
        std::fs::create_dir_all(&electron).unwrap();
        let seed = br#"{"schemaVersion":1,"folders":[]}"#;
        std::fs::write(electron.join(FOLDERS_FILE), seed).unwrap();
        let store = Store::new(root.join("native"), electron.clone()).unwrap();
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        #[cfg(unix)]
        let project = {
            let link = root.join("linked");
            std::os::unix::fs::symlink(&project, &link).unwrap();
            link
        };
        let used = use_folder(&store, &project, "2026-11-10T12:00:00Z").unwrap();
        assert_eq!(used.path, project.to_string_lossy());
        let moved = root.join("moved");
        std::fs::create_dir_all(&moved).unwrap();
        move_folder(&store, &used.id, &moved).unwrap();
        assert_eq!(same_as(&store, &project).unwrap(), moved.to_string_lossy());
        learn(
            &store,
            &[("ws_learned".into(), project.to_string_lossy().into_owned())],
        )
        .unwrap();
        assert_eq!(list(&store).unwrap().len(), 2);
        assert_eq!(std::fs::read(electron.join(FOLDERS_FILE)).unwrap(), seed);
        assert_eq!(std::fs::read_dir(&electron).unwrap().count(), 1);
        assert_eq!(folder_name(r"C:\work\demo"), "demo");
        std::fs::remove_dir_all(root).unwrap();
    }
}
