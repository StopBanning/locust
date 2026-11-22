//! Folders restored from another computer's backup, found again on this one — port of `main/restored-folders.ts`.
//!
//! A backup made on Windows names `C:\work\demo`; on this Mac the same project is `~/work/demo`. The scan looks in the
//! places people keep projects, matches a missing folder by its git remote when one was saved and by its name when not,
//! and offers a candidate only when exactly one fits: ambiguity is left to the picker.

use crate::folders::{KnownFolder, folder_name};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoredFolderMatch {
    pub id: String,
    pub path: String,
    pub candidate: Option<String>,
}

/// Where the scan starts, under the person's home folder.
pub const SCAN_ROOTS: [&str; 6] = ["work", "Documents", "Projects", "code", "src", "Developer"];
/// How many folders one scan may look in.
const SCAN_LIMIT: usize = 2000;
/// How far below a start folder it goes.
const SCAN_DEPTH: usize = 3;
const SKIPPED: [&str; 3] = ["node_modules", "vendor", "Library"];

pub fn directory_exists(path: &str) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_dir())
}

/// The origin remote, read from the configuration only: never run git or follow repository hooks during discovery.
pub fn folder_remote(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path.join(".git").join("config")).unwrap_or_default();
    let re = regex::Regex::new(r#"\[remote "origin"\][^\[]*?\burl\s*=\s*([^\r\n]+)"#).ok()?;
    Some(re.captures(&text)?.get(1)?.as_str().trim().to_owned())
}

/// One spelling for one repository: `git@host:team/x.git`, `ssh://git@host/team/x` and `https://host/team/x/` agree.
pub fn remote_key(remote: &str) -> String {
    let key = remote.trim();
    let key = regex::Regex::new(r"^git@([^:]+):").expect("scp remote").replace(key, "https://$1/");
    let key = key.strip_prefix("ssh://git@").map_or(key.to_string(), |rest| format!("https://{rest}"));
    let key = regex::Regex::new(r"\.git/?$").expect("git suffix").replace(&key, "").into_owned();
    key.strip_suffix('/').unwrap_or(&key).to_lowercase()
}

fn same_name(a: &str, b: &str) -> bool {
    folder_name(a).to_lowercase() == folder_name(b).to_lowercase()
}

struct Candidate {
    path: String,
    remote: Option<String>,
}

/// A small, bounded scan. Links and hidden directories are never traversed.
pub fn find_restored_folders(folders: &[KnownFolder], home: &Path) -> Vec<RestoredFolderMatch> {
    let missing: Vec<&KnownFolder> = folders.iter().filter(|f| !directory_exists(&f.path)).collect();
    if missing.is_empty() {
        return Vec::new();
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut queue: std::collections::VecDeque<(PathBuf, usize)> =
        SCAN_ROOTS.iter().map(|name| (home.join(name), 0)).collect();
    let mut visited = 0;
    while visited < SCAN_LIMIT {
        let Some((next, depth)) = queue.pop_front() else { break };
        visited += 1;
        let entries: Vec<std::fs::DirEntry> =
            std::fs::read_dir(&next).map(|read| read.filter_map(Result::ok).collect()).unwrap_or_default();
        let next_str = next.to_string_lossy().into_owned();
        if missing.iter().any(|f| same_name(&f.path, &next_str)) || entries.iter().any(|e| e.file_name() == ".git") {
            candidates.push(Candidate { remote: folder_remote(&next), path: next_str });
        }
        if depth >= SCAN_DEPTH {
            continue;
        }
        for entry in &entries {
            if queue.len() + visited >= SCAN_LIMIT {
                break;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            // The entry's own kind: a link to a folder is a link, and is not followed.
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir && !name.starts_with('.') && !SKIPPED.contains(&name.as_str()) {
                queue.push_back((next.join(&name), depth + 1));
            }
        }
    }
    missing
        .into_iter()
        .map(|folder| {
            let matches: Vec<&Candidate> = match &folder.remote {
                Some(remote) => {
                    let key = remote_key(remote);
                    candidates.iter().filter(|c| c.remote.as_deref().is_some_and(|r| remote_key(r) == key)).collect()
                }
                None => candidates.iter().filter(|c| same_name(&c.path, &folder.path)).collect(),
            };
            RestoredFolderMatch {
                id: folder.id.clone(),
                path: folder.path.clone(),
                candidate: (matches.len() == 1).then(|| matches[0].path.clone()),
            }
        })
        .collect()
}

/// What the restore card says when its list no longer holds (index.ts `FOLDER_RESTORE_APPLY_CHANNEL`).
pub const FOLDERS_CHANGED: &str = "The folders changed. Open the folder list again.";
pub const WAIT_FOR_RUNS: &str = "Wait for the running conversations to finish, then apply these folders.";
pub const NOT_SAVED: &str = "Some folders could not be saved. Open the folder list to review what remains.";

/// The scan the restore card is filled from (index.ts `FOLDER_RESTORE_LIST_CHANNEL`): teammates' own folders are
/// learned first, so a teammate set to work in a folder from the other computer is looked for too.
pub fn check(store: &crate::store::Store, home: &Path) -> Vec<RestoredFolderMatch> {
    if let Ok(team) = crate::native::roster_in(store) {
        let found: Vec<(String, String)> = team
            .into_iter()
            .filter_map(|t| t.folder)
            .map(|folder| (crate::people::workspace_id(Path::new(&folder)), folder))
            .collect();
        let _ = crate::folders::learn(store, &found);
    }
    crate::folders::list(store).map(|known| find_restored_folders(&known, home)).unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderMove {
    pub id: String,
    pub path: String,
    pub old_path: String,
}

/// Every chosen folder checked before any is moved: each was offered for its row (found, or picked by the person),
/// the old one is still missing, the new one is there and is not a folder a teammate should work in.
pub fn moves_for(
    store: &crate::store::Store,
    chosen: &[(String, String)],
    offered: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    home: Option<&str>,
) -> Result<Vec<FolderMove>, Option<&'static str>> {
    if chosen.len() > SCAN_LIMIT {
        return Err(None);
    }
    let known = crate::folders::load(store).map_err(|_| None)?;
    let mut moves = Vec::new();
    for (id, path) in chosen {
        if !offered.get(id).is_some_and(|paths| paths.contains(path)) {
            return Err(None);
        }
        let Some(old) = known.iter().find(|f| f.id == *id) else { return Err(Some(FOLDERS_CHANGED)) };
        if directory_exists(&old.path)
            || !directory_exists(path)
            || crate::thread_view::folder_caution(path, home, cfg!(windows), None).is_some()
        {
            return Err(Some(FOLDERS_CHANGED));
        }
        moves.push(FolderMove { id: id.clone(), path: path.clone(), old_path: old.path.clone() });
    }
    Ok(moves)
}

/// One move, everywhere the old folder was named: the registry (the conversations keep their id), the folder's check
/// command and fix count, the teammates set to work there, and each of the folder's conversations, which say so in
/// their own record so the next turn starts a fresh agent session rather than resuming one from the other computer.
pub fn apply_move(store: &crate::store::Store, ledgers: &[PathBuf], one: &FolderMove) -> Result<(), String> {
    crate::folders::move_folder(store, &one.id, Path::new(&one.path))?;
    let new_id = crate::people::workspace_id(Path::new(&one.path));
    let mut settings = crate::workspace_settings::load(store)?;
    if let Some(command) = settings.check_commands.get(&one.id).cloned() {
        settings.check_commands.insert(new_id.clone(), command);
    }
    if let Some(fixes) = settings.check_fixes.get(&one.id).copied() {
        settings.check_fixes.insert(new_id, fixes);
    }
    crate::workspace_settings::save(store, &settings)?;
    let team = crate::people::team(store)?;
    for entry in team["teammates"].as_array().into_iter().flatten() {
        if entry["folder"].as_str() == Some(one.old_path.as_str()) {
            let mut entry = entry.clone();
            entry["folder"] = serde_json::json!(one.path);
            crate::people::save_teammate(store, entry)?;
        }
    }
    let message = format!("Folder moved from {} to {}.", one.old_path, one.path);
    for dir in ledgers {
        let Ok(files) = std::fs::read_dir(dir) else { continue };
        for file in files.filter_map(Result::ok).map(|e| e.path()) {
            if file.extension().is_none_or(|ext| ext != "jsonl") || !in_workspace(&file, &one.id) {
                continue;
            }
            crate::ledger_write::append_events(&file, |run, mission, adapter, sequence, at| {
                vec![serde_json::json!({
                    "id": format!("{run}:folder:{sequence}"), "runId": run, "missionId": mission, "sequence": sequence,
                    "type": "adapter.diagnostic", "occurredAt": at, "sourceAdapter": adapter,
                    "payload": {"level": "info", "code": "host.folder_moved", "message": message, "terminal": false, "evidence": {"redacted": true}}
                })]
            })
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Whether a conversation's first record names this folder.
fn in_workspace(file: &Path, id: &str) -> bool {
    use std::io::BufRead;
    let Ok(open) = std::fs::File::open(file) else { return false };
    let mut first = String::new();
    if std::io::BufReader::new(open).read_line(&mut first).is_err() {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(&first)
        .is_ok_and(|record| record["metadata"]["workspaceId"].as_str() == Some(id))
}

#[cfg(test)]
#[path = "restored_folders_tests.rs"]
mod restored_folders_tests;
