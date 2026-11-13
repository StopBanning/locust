//! Mission trash: reversible deletion, listing, restoring, and emptying for good
//! (Electron `packages/mission-store/src/index.ts` `removeMissionFile`, `listTrashedMissions`,
//! `restoreMission`, `emptyTrash` + `apps/desktop/src/main/index.ts` lines 9912-10003).

use crate::store::Store;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const TRASH_DIR: &str = ".trash";

pub const ERR_READ: &str = "The trash could not be read. Nothing in it has been deleted for good.";
pub const ERR_RESTORE: &str = "That conversation could not be put back. It is still in the trash.";
pub const ERR_RESTORE_REFUSED: &str =
    "That conversation is not in the trash, or one with the same id is live again.";
pub const ERR_EMPTY: &str = "The trash was not emptied. Everything in it is still there.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashedMission {
    pub mission_id: String,
    pub deleted_at: String,
    pub bytes: u64,
    pub prompt: Option<String>,
    pub created_at: Option<String>,
}

/// Matches Electron `SAFE_ID = /^[A-Za-z0-9_-]{1,128}$/`.
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn mission_path(ledger_dir: &Path, mission_id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(mission_id) {
        return Err("missionId is invalid".into());
    }
    Ok(ledger_dir.join(format!("{mission_id}.jsonl")))
}

fn trash_path(ledger_dir: &Path, mission_id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(mission_id) {
        return Err("missionId is invalid".into());
    }
    Ok(ledger_dir.join(TRASH_DIR).join(format!("{mission_id}.jsonl")))
}

fn header_of(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(mut file) = File::open(path) else {
        return (None, None);
    };
    let mut buf = [0u8; 16_384];
    let Ok(n) = file.read(&mut buf) else {
        return (None, None);
    };
    let text = String::from_utf8_lossy(&buf[..n]);
    let line = text.lines().next().unwrap_or("");
    let Ok(val) = serde_json::from_str::<Value>(line) else {
        return (None, None);
    };
    let prompt = val
        .get("metadata")
        .and_then(|m| m.get("prompt"))
        .and_then(Value::as_str)
        .map(|s| s.chars().take(200).collect::<String>());
    let created_at = val
        .get("metadata")
        .and_then(|m| m.get("createdAt"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    (prompt, created_at)
}

/// Move a native mission record into `mission-ledger/.trash/`.
/// Returns `Ok(false)` when no live file exists for `mission_id`.
/// Owner (`teammates.json`) and references (`references.json`) are intentionally kept
/// until `empty_trash` so a restore brings the conversation back whole.
pub fn delete_mission(
    ledger_dir: &Path,
    mission_id: &str,
    running_ids: &[String],
) -> Result<bool, String> {
    let from = mission_path(ledger_dir, mission_id)?;
    if running_ids.iter().any(|id| id == mission_id) {
        return Err("That conversation is still running. Stop it before deleting.".into());
    }
    let to = trash_path(ledger_dir, mission_id)?;
    let trash_dir = ledger_dir.join(TRASH_DIR);
    fs::create_dir_all(&trash_dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&trash_dir, fs::Permissions::from_mode(0o700));
    }
    let _ = fs::remove_file(&to);
    match fs::rename(&from, &to) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.to_string()),
    }
    if let Ok(file) = fs::OpenOptions::new().write(true).open(&to) {
        let _ = file.set_modified(SystemTime::now());
    }
    Ok(true)
}

/// List every mission in `mission-ledger/.trash/`, newest deletion first.
pub fn list_trashed_missions(ledger_dir: &Path) -> Result<Vec<TrashedMission>, String> {
    let dir = ledger_dir.join(TRASH_DIR);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(ERR_READ.into()),
    };
    let mut found = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else { continue };
        let Some(mission_id) = name_str.strip_suffix(".jsonl") else {
            continue;
        };
        let path = entry.path();
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let deleted_at = DateTime::<Utc>::from(mtime).to_rfc3339_opts(SecondsFormat::Millis, true);
        let (prompt, created_at) = header_of(&path);
        found.push(TrashedMission {
            mission_id: mission_id.to_owned(),
            deleted_at,
            bytes: meta.len(),
            prompt,
            created_at,
        });
    }
    found.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(found)
}

/// Put a trashed mission back into `mission-ledger/`.
/// Returns `Ok(false)` when it is not in the trash or a live record with the same id exists.
pub fn restore_mission(ledger_dir: &Path, mission_id: &str) -> Result<bool, String> {
    if !is_safe_id(mission_id) {
        return Err(ERR_RESTORE.into());
    }
    let from = trash_path(ledger_dir, mission_id).map_err(|_| ERR_RESTORE.to_string())?;
    let to = mission_path(ledger_dir, mission_id).map_err(|_| ERR_RESTORE.to_string())?;
    if fs::symlink_metadata(&to).is_ok() {
        return Ok(false);
    }
    match fs::rename(&from, &to) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(ERR_RESTORE.into()),
    }
}

/// Put a trashed mission back, returning Electron's exact refusal message on `false`.
pub fn restore_mission_or_err(ledger_dir: &Path, mission_id: &str) -> Result<(), String> {
    match restore_mission(ledger_dir, mission_id)? {
        true => Ok(()),
        false => Err(ERR_RESTORE_REFUSED.into()),
    }
}

/// Delete all trashed `.jsonl` files in `ledger_dir` without touching `teammates.json` / `references.json`.
pub fn empty_trash_dir(ledger_dir: &Path) -> Result<(usize, Vec<String>), String> {
    let held = list_trashed_missions(ledger_dir).map_err(|_| ERR_EMPTY.to_string())?;
    let mut gone = 0usize;
    let mut ids = Vec::with_capacity(held.len());
    for entry in held {
        ids.push(entry.mission_id.clone());
        if let Ok(path) = trash_path(ledger_dir, &entry.mission_id)
            && fs::remove_file(path).is_ok()
        {
            gone += 1;
        }
    }
    Ok((gone, ids))
}

/// Empty the native Trash for good, then drop owners from `teammates.json` and forget references.
pub fn empty_trash(store: &Store) -> Result<usize, String> {
    let ledger_dir = store.ledger_dir().map_err(|_| ERR_EMPTY.to_string())?;
    let (gone, ids) = empty_trash_dir(&ledger_dir)?;
    let _ = crate::references::forget(store, &ids);
    let _ = crate::people::unassign_missions(store, &ids);
    Ok(gone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retention;
    use serde_json::json;

    fn temp_store() -> (PathBuf, Store) {
        let dir = std::env::temp_dir().join(format!(
            "locust-trash-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        let native = dir.join("native");
        let electron = dir.join("electron");
        fs::create_dir_all(&native).unwrap();
        fs::create_dir_all(&electron).unwrap();
        let store = Store::new(native, electron).unwrap();
        (dir, store)
    }

    fn write_mission(ledger_dir: &Path, mission_id: &str, created_at: &str, prompt: &str) {
        fs::create_dir_all(ledger_dir).unwrap();
        let line = json!({
            "schemaVersion": 1,
            "recordType": "mission.created",
            "occurredAt": created_at,
            "metadata": {
                "missionId": mission_id,
                "runId": format!("run_{mission_id}"),
                "prompt": prompt,
                "runtime": "codex",
                "model": "gpt-6-astra",
                "requestedRouteId": "codex:account-default",
                "resolvedRouteId": "codex:gpt-6-astra",
                "workspaceId": "ws_test",
                "sandbox": "read-only",
                "executionPolicyVersion": 1,
                "createdAt": created_at,
            }
        });
        fs::write(
            ledger_dir.join(format!("{mission_id}.jsonl")),
            format!("{line}\n"),
        )
        .unwrap();
    }

    #[test]
    fn deleting_takes_it_out_of_storage_report_and_keeps_bytes() {
        let (root, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        write_mission(&ledger, "mission_a", "2026-09-17T10:00:00.000Z", "the stocks one");
        write_mission(&ledger, "mission_b", "2026-09-17T11:00:00.000Z", "do the thing");
        let before = fs::read_to_string(ledger.join("mission_a.jsonl")).unwrap();

        assert!(delete_mission(&ledger, "mission_a", &[]).unwrap());
        assert!(!ledger.join("mission_a.jsonl").exists());
        assert_eq!(retention::read_storage_report(&ledger).unwrap().mission_count, 1);

        let trashed = list_trashed_missions(&ledger).unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].mission_id, "mission_a");
        assert_eq!(trashed[0].prompt.as_deref(), Some("the stocks one"));
        assert_eq!(
            fs::read_to_string(ledger.join(TRASH_DIR).join("mission_a.jsonl")).unwrap(),
            before
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn can_be_put_back_exactly_as_it_was() {
        let (root, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        write_mission(&ledger, "mission_a", "2026-09-17T10:00:00.000Z", "the stocks one");
        let before = fs::read_to_string(ledger.join("mission_a.jsonl")).unwrap();

        delete_mission(&ledger, "mission_a", &[]).unwrap();
        assert!(restore_mission(&ledger, "mission_a").unwrap());

        assert_eq!(
            fs::read_to_string(ledger.join("mission_a.jsonl")).unwrap(),
            before
        );
        assert!(list_trashed_missions(&ledger).unwrap().is_empty());
        assert_eq!(retention::read_storage_report(&ledger).unwrap().mission_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn says_no_rather_than_writing_over_a_live_record() {
        let (root, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        write_mission(&ledger, "mission_a", "2026-09-17T10:00:00.000Z", "the old one");
        delete_mission(&ledger, "mission_a", &[]).unwrap();
        write_mission(&ledger, "mission_a", "2026-09-17T12:00:00.000Z", "the new one");

        assert!(!restore_mission(&ledger, "mission_a").unwrap());
        assert_eq!(
            restore_mission_or_err(&ledger, "mission_a").unwrap_err(),
            ERR_RESTORE_REFUSED
        );

        let live = fs::read_to_string(ledger.join("mission_a.jsonl")).unwrap();
        assert!(live.contains("the new one"));
        let trashed = list_trashed_missions(&ledger).unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].mission_id, "mission_a");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_nothing_to_restore_for_missing_mission() {
        let (root, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        fs::create_dir_all(&ledger).unwrap();
        assert!(!delete_mission(&ledger, "mission_missing", &[]).unwrap());
        assert!(!restore_mission(&ledger, "mission_missing").unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn empties_for_good_and_drops_owners_and_references_only_then() {
        let (root, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        write_mission(&ledger, "mission_a", "2026-09-17T10:00:00.000Z", "a");
        write_mission(&ledger, "mission_b", "2026-09-17T11:00:00.000Z", "b");

        crate::people::record_owner(&store, "mission_a", "tm_wren").unwrap();
        crate::people::record_owner(&store, "mission_b", "tm_wren").unwrap();
        let folder = root.join("ref_folder");
        fs::create_dir_all(&folder).unwrap();
        let item = crate::references::add_folder(&store, &folder).unwrap();
        crate::references::bind(&store, "mission_a", None, std::slice::from_ref(&item.id), None).unwrap();

        delete_mission(&ledger, "mission_a", &[]).unwrap();
        delete_mission(&ledger, "mission_b", &[]).unwrap();

        // Owner is still kept while in trash.
        assert_eq!(crate::people::owner_of(&store, "mission_a").as_deref(), Some("tm_wren"));

        assert_eq!(empty_trash(&store).unwrap(), 2);
        assert!(list_trashed_missions(&ledger).unwrap().is_empty());
        assert!(!restore_mission(&ledger, "mission_a").unwrap());
        assert_eq!(crate::people::owner_of(&store, "mission_a"), None);
        assert_eq!(crate::people::owner_of(&store, "mission_b"), None);
        assert!(crate::references::select(&store, None, Some(std::slice::from_ref(&item.id))).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn survives_deleted_restored_and_deleted_again() {
        let (root, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        write_mission(&ledger, "mission_a", "2026-09-17T10:00:00.000Z", "a");
        delete_mission(&ledger, "mission_a", &[]).unwrap();
        assert!(restore_mission(&ledger, "mission_a").unwrap());
        assert!(delete_mission(&ledger, "mission_a", &[]).unwrap());
        assert_eq!(list_trashed_missions(&ledger).unwrap().len(), 1);
        assert!(restore_mission(&ledger, "mission_a").unwrap());
        assert!(ledger.join("mission_a.jsonl").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unreadable_trashed_file_is_still_listed_by_id_and_unsafe_ids_refuse() {
        let (root, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        fs::create_dir_all(ledger.join(TRASH_DIR)).unwrap();
        fs::write(ledger.join(TRASH_DIR).join("mission_bad.jsonl"), "not-json\n").unwrap();

        let trashed = list_trashed_missions(&ledger).unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].mission_id, "mission_bad");
        assert_eq!(trashed[0].prompt, None);

        assert_eq!(restore_mission(&ledger, "../escape").unwrap_err(), ERR_RESTORE);
        assert_eq!(restore_mission(&ledger, "").unwrap_err(), ERR_RESTORE);
        let _ = fs::remove_dir_all(root);
    }
}
