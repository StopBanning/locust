//! Retention: deleting old conversations in bulk (Electron `main/retention.ts`; PARITY row 115).
//!
//! - The cutoff is computed from the host's own clock from a validated number of days (1..=3650).
//! - Which conversations are running is answered by the engine (`protect_mission_ids`).
//! - Continuity is preserved: an old turn that a kept or running conversation continues from is kept.
//! - Confirming a prune is bounded by the exact IDs shown in the preview (`only`).
//! - Deleting moves the native ledger file into `mission-ledger/.trash/`, never touching Electron's folder.

use crate::ledger;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MIN_RETENTION_DAYS: i64 = 1;
pub const MAX_RETENTION_DAYS: i64 = 3650;
pub const MAX_KNOWN_SCHEMA_VERSION: u64 = 27;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageReport {
    pub mission_count: usize,
    pub byte_total: u64,
    pub unreadable_count: usize,
    pub oldest_updated_at: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PruneResult {
    pub deleted: Vec<String>,
    pub failed: Vec<String>,
    pub unreadable: Vec<String>,
    pub kept_for_continuity: Vec<String>,
    pub kept_as_running: Vec<String>,
    pub previewed: bool,
}

/// Bounds on the age a window may ask for. A day is the shortest honest unit.
pub fn retention_cutoff(older_than_days: Option<f64>, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let raw = older_than_days?;
    if !raw.is_finite() {
        return None;
    }
    let days = raw.trunc() as i64;
    if !(MIN_RETENTION_DAYS..=MAX_RETENTION_DAYS).contains(&days) {
        return None;
    }
    Some(now - Duration::days(days))
}

struct ParsedHeader {
    readable: bool,
    openable: bool,
    last_updated_at: Option<DateTime<Utc>>,
    continues_from: Option<String>,
}

fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn inspect_ledger_file(path: &Path) -> ParsedHeader {
    let Ok(raw) = fs::read_to_string(path) else {
        return ParsedHeader {
            readable: false,
            openable: false,
            last_updated_at: None,
            continues_from: None,
        };
    };

    let mut lines = raw.lines().filter(|l| !l.trim().is_empty());
    let Some(first_line) = lines.next() else {
        return ParsedHeader {
            readable: false,
            openable: true,
            last_updated_at: None,
            continues_from: None,
        };
    };

    let Ok(first_val) = serde_json::from_str::<Value>(first_line) else {
        return ParsedHeader {
            readable: false,
            openable: true,
            last_updated_at: None,
            continues_from: None,
        };
    };

    let continues_from = first_val
        .get("metadata")
        .and_then(|m| m.get("continuesFrom"))
        .and_then(|c| c.get("missionId"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    let schema_ok = first_val
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .is_some_and(|v| v <= MAX_KNOWN_SCHEMA_VERSION);
    let created_ok = first_val.get("recordType").and_then(Value::as_str) == Some("mission.created");

    if !schema_ok || !created_ok {
        return ParsedHeader {
            readable: false,
            openable: true,
            last_updated_at: None,
            continues_from,
        };
    }

    let mut latest = first_val
        .get("occurredAt")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339)
        .or_else(|| {
            first_val
                .get("metadata")
                .and_then(|m| m.get("createdAt"))
                .and_then(Value::as_str)
                .and_then(parse_rfc3339)
        });

    for line in lines {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            return ParsedHeader {
                readable: false,
                openable: true,
                last_updated_at: None,
                continues_from,
            };
        };
        if let Some(ts) = val
            .get("occurredAt")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339)
        {
            latest = Some(match latest {
                Some(prev) if prev > ts => prev,
                _ => ts,
            });
        }
    }

    ParsedHeader {
        readable: latest.is_some(),
        openable: true,
        last_updated_at: latest,
        continues_from,
    }
}

/// Measure native mission history size, count, and oldest record date.
pub fn read_storage_report(ledger_dir: &Path) -> Result<StorageReport, String> {
    let entries = match fs::read_dir(ledger_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(StorageReport::default()),
        Err(_) => return Err("The local mission history could not be measured.".into()),
    };

    let mut report = StorageReport::default();
    let mut oldest: Option<DateTime<Utc>> = None;

    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(id) = name.strip_suffix(".jsonl") else {
            continue;
        };
        if !ledger::safe_id(id) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        report.byte_total += meta.len();
        report.mission_count += 1;

        let parsed = inspect_ledger_file(&path);
        match parsed.last_updated_at {
            Some(ts) => {
                oldest = Some(match oldest {
                    Some(prev) if prev <= ts => prev,
                    _ => ts,
                });
            }
            None => {
                report.unreadable_count += 1;
            }
        }
    }

    report.oldest_updated_at =
        oldest.map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
    Ok(report)
}

fn move_to_trash(ledger_dir: &Path, mission_id: &str) -> io::Result<bool> {
    let from = ledger_dir.join(format!("{mission_id}.jsonl"));
    let trash_dir = ledger_dir.join(".trash");
    let to = trash_dir.join(format!("{mission_id}.jsonl"));
    fs::create_dir_all(&trash_dir)?;
    if to.exists() {
        let _ = fs::remove_file(&to);
    }
    match fs::rename(&from, &to) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Preview or execute pruning of old native conversations, preserving running turns and continuity chains.
pub fn prune_missions(
    ledger_dir: &Path,
    older_than_days: Option<f64>,
    dry_run: bool,
    only: Option<&[String]>,
    protect_mission_ids: &[String],
    now: DateTime<Utc>,
) -> Result<PruneResult, String> {
    prune_missions_with_move(
        ledger_dir,
        older_than_days,
        dry_run,
        only,
        protect_mission_ids,
        now,
        move_to_trash,
    )
}

pub fn prune_missions_with_move<F>(
    ledger_dir: &Path,
    older_than_days: Option<f64>,
    dry_run: bool,
    only: Option<&[String]>,
    protect_mission_ids: &[String],
    now: DateTime<Utc>,
    mut move_fn: F,
) -> Result<PruneResult, String>
where
    F: FnMut(&Path, &str) -> io::Result<bool>,
{
    let Some(cutoff) = retention_cutoff(older_than_days, now) else {
        return Err(format!(
            "Choose an age between {MIN_RETENTION_DAYS} and {MAX_RETENTION_DAYS} days."
        ));
    };

    let only_set: Option<HashSet<&str>> = if dry_run {
        None
    } else {
        only.filter(|list| list.len() <= 10_000)
            .map(|list| list.iter().map(String::as_str).collect())
    };
    let running: HashSet<&str> = protect_mission_ids.iter().map(String::as_str).collect();

    let entries = match fs::read_dir(ledger_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(PruneResult {
                previewed: dry_run,
                ..Default::default()
            });
        }
        Err(_) => return Err("The missions could not be pruned.".into()),
    };

    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut occupied: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(id) = name.strip_suffix(".jsonl") else {
            continue;
        };
        if !ledger::safe_id(id) {
            continue;
        }
        let is_file = entry.file_type().is_ok_and(|ft| ft.is_file());
        if !is_file {
            occupied.push(id.to_owned());
        } else {
            files.push((id.to_owned(), entry.path()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut unreadable = occupied;
    let mut held: Vec<(String, ParsedHeader)> = Vec::with_capacity(files.len());
    for (id, path) in files {
        let parsed = inspect_ledger_file(&path);
        if !parsed.openable {
            unreadable.push(id);
        } else {
            held.push((id, parsed));
        }
    }

    if !unreadable.is_empty() {
        unreadable.sort();
        return Ok(PruneResult {
            unreadable,
            previewed: dry_run,
            ..Default::default()
        });
    }

    let mut parents: HashMap<String, String> = HashMap::new();
    for (id, parsed) in &held {
        if let Some(parent) = &parsed.continues_from {
            parents.insert(id.clone(), parent.clone());
        }
    }

    let mut kept_as_running = Vec::new();
    let mut candidates: HashSet<String> = HashSet::new();
    let mut ordered_candidates: Vec<String> = Vec::new();
    let mut survivors: Vec<String> = Vec::new();

    for (id, parsed) in held {
        let old = parsed.readable && parsed.last_updated_at.is_some_and(|ts| ts < cutoff);
        if !old || only_set.as_ref().is_some_and(|s| !s.contains(id.as_str())) {
            survivors.push(id);
            continue;
        }
        if running.contains(id.as_str()) {
            kept_as_running.push(id.clone());
            survivors.push(id);
            continue;
        }
        candidates.insert(id.clone());
        ordered_candidates.push(id);
    }

    let mut kept_for_continuity_set: HashSet<String> = HashSet::new();
    for survivor in survivors {
        let mut cursor = parents.get(&survivor).cloned();
        let mut seen: HashSet<String> = HashSet::from([survivor]);
        while let Some(c) = cursor {
            if !seen.insert(c.clone()) {
                break;
            }
            if candidates.remove(&c) {
                kept_for_continuity_set.insert(c.clone());
            }
            cursor = parents.get(&c).cloned();
        }
    }

    let mut kept_for_continuity: Vec<String> = kept_for_continuity_set.into_iter().collect();
    kept_for_continuity.sort();

    let mut deleted = Vec::new();
    let mut failed = Vec::new();
    for id in ordered_candidates {
        if !candidates.contains(&id) {
            continue;
        }
        if dry_run {
            deleted.push(id);
            continue;
        }
        match move_fn(ledger_dir, &id) {
            Ok(true) => deleted.push(id),
            Ok(false) => {}
            Err(_) => failed.push(id),
        }
    }

    Ok(PruneResult {
        deleted,
        failed,
        unreadable: Vec::new(),
        kept_for_continuity,
        kept_as_running,
        previewed: dry_run,
    })
}

/// What a prune preview says out loud, before anyone agrees to it (Electron `status.ts`).
pub fn prune_preview_summary(preview: &PruneResult) -> String {
    let turns = |count: usize| -> String {
        format!("{count} turn{}", if count == 1 { "" } else { "s" })
    };
    if preview.deleted.is_empty() {
        let mut because = Vec::new();
        if !preview.kept_for_continuity.is_empty() {
            because.push(format!(
                "{} still part of a conversation you are keeping",
                turns(preview.kept_for_continuity.len())
            ));
        }
        if !preview.kept_as_running.is_empty() {
            because.push(format!(
                "{} running right now",
                turns(preview.kept_as_running.len())
            ));
        }
        return if because.is_empty() {
            "Nothing is old enough to delete.".into()
        } else {
            format!("Nothing would be deleted: {}.", because.join(", and "))
        };
    }
    let mut kept = Vec::new();
    if !preview.kept_for_continuity.is_empty() {
        kept.push(format!(
            "{} kept as part of a conversation you are keeping",
            turns(preview.kept_for_continuity.len())
        ));
    }
    if !preview.kept_as_running.is_empty() {
        kept.push(format!(
            "{} kept because they are running",
            turns(preview.kept_as_running.len())
        ));
    }
    let suffix = if kept.is_empty() {
        String::new()
    } else {
        format!(", with {}", kept.join(" and "))
    };
    format!("Delete {} for good{suffix}.", turns(preview.deleted.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        parse_rfc3339("2026-09-02T12:00:00.000Z").unwrap()
    }

    fn at(days_ago: i64) -> String {
        (now() - Duration::days(days_ago)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }

    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "locust-prune-{}-{}",
            std::process::id(),
            crate::native::uuid_v4().unwrap_or_else(|_| "0".into())
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed(root: &Path, specs: &[(&str, i64, Option<&str>)]) {
        for &(id, days_ago, parent) in specs {
            let created = at(days_ago);
            let mut meta = json!({
                "missionId": id,
                "runId": format!("run_{id}"),
                "prompt": "Inspect the workspace without changing it.",
                "runtime": "codex",
                "model": "account-default",
                "createdAt": created,
            });
            if let Some(p) = parent {
                meta["continuesFrom"] = json!({"missionId": p, "checkpointEpoch": 1, "reason": "follow-up"});
            }
            let line = json!({
                "schemaVersion": 27,
                "recordType": "mission.created",
                "ledgerSequence": 1,
                "occurredAt": created,
                "metadata": meta,
            });
            fs::write(root.join(format!("{id}.jsonl")), format!("{line}\n")).unwrap();
        }
    }

    fn remaining(root: &Path) -> Vec<String> {
        let mut out: Vec<String> = fs::read_dir(root)
            .unwrap()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".jsonl").map(str::to_owned))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn cutoff_computed_from_host_clock_and_bounded() {
        assert_eq!(
            retention_cutoff(Some(30.0), now())
                .unwrap()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "2026-08-03T12:00:00.000Z"
        );
        for bad in [None, Some(f64::NAN), Some(f64::INFINITY), Some(0.0), Some(-1.0), Some((MAX_RETENTION_DAYS + 1) as f64)] {
            assert!(retention_cutoff(bad, now()).is_none(), "{bad:?}");
        }
        assert!(retention_cutoff(Some(MIN_RETENTION_DAYS as f64), now()).is_some());
        assert!(retention_cutoff(Some(MAX_RETENTION_DAYS as f64), now()).is_some());
    }

    #[test]
    fn prunes_old_keeps_recent_and_continuity_chains() {
        let root = temp_root();
        seed(&root, &[("mission_old", 40, None), ("mission_recent", 2, None)]);
        let res = prune_missions(&root, Some(30.0), false, None, &[], now()).unwrap();
        assert_eq!(res.deleted, vec!["mission_old"]);
        assert_eq!(remaining(&root), vec!["mission_recent"]);
        assert!(root.join(".trash/mission_old.jsonl").exists());
        let _ = fs::remove_dir_all(&root);

        // Continuity chain
        let root = temp_root();
        seed(
            &root,
            &[
                ("mission_one", 60, None),
                ("mission_two", 50, Some("mission_one")),
                ("mission_three", 45, Some("mission_two")),
                ("mission_four", 1, Some("mission_three")),
            ],
        );
        let res = prune_missions(&root, Some(30.0), false, None, &[], now()).unwrap();
        assert!(res.deleted.is_empty());
        assert_eq!(
            res.kept_for_continuity,
            vec!["mission_one", "mission_three", "mission_two"]
        );
        assert_eq!(remaining(&root).len(), 4);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn deletes_whole_conversation_when_every_turn_is_old() {
        let root = temp_root();
        seed(
            &root,
            &[
                ("mission_first", 60, None),
                ("mission_reply", 55, Some("mission_first")),
                ("mission_new", 1, None),
            ],
        );
        let res = prune_missions(&root, Some(30.0), false, None, &[], now()).unwrap();
        assert_eq!(res.deleted, vec!["mission_first", "mission_reply"]);
        assert!(res.kept_for_continuity.is_empty());
        assert_eq!(remaining(&root), vec!["mission_new"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn preview_deletes_nothing_and_confirm_is_bounded_by_only() {
        let root = temp_root();
        seed(
            &root,
            &[
                ("mission_old", 40, None),
                ("mission_aged_since", 40, None),
                ("mission_first", 40, None),
                ("mission_reply", 1, Some("mission_first")),
            ],
        );
        let preview = prune_missions(
            &root,
            Some(30.0),
            true,
            Some(&["mission_old".into()]),
            &[],
            now(),
        )
        .unwrap();
        assert!(preview.previewed);
        // A preview is never bounded by `only`.
        assert_eq!(preview.deleted, vec!["mission_aged_since", "mission_old"]);
        assert_eq!(preview.kept_for_continuity, vec!["mission_first"]);
        assert_eq!(remaining(&root).len(), 4);

        let only = vec!["mission_old".to_owned()];
        let done = prune_missions(&root, Some(30.0), false, Some(&only), &[], now()).unwrap();
        assert!(!done.previewed);
        assert_eq!(done.deleted, vec!["mission_old"]);
        assert_eq!(
            remaining(&root),
            vec!["mission_aged_since", "mission_first", "mission_reply"]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn protects_running_missions_and_their_parents() {
        let root = temp_root();
        seed(
            &root,
            &[
                ("mission_first", 40, None),
                ("mission_running", 40, Some("mission_first")),
                ("mission_old", 40, None),
            ],
        );
        let res = prune_missions(
            &root,
            Some(30.0),
            false,
            None,
            &["mission_running".into()],
            now(),
        )
        .unwrap();
        assert_eq!(res.deleted, vec!["mission_old"]);
        assert_eq!(res.kept_as_running, vec!["mission_running"]);
        assert_eq!(res.kept_for_continuity, vec!["mission_first"]);
        assert_eq!(remaining(&root), vec!["mission_first", "mission_running"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unreadable_and_locked_files_and_hint_protection() {
        // Broken jsonl is left alone
        let root = temp_root();
        seed(&root, &[("mission_old", 40, None)]);
        fs::write(root.join("mission_broken.jsonl"), "not json at all\n").unwrap();
        let res = prune_missions(&root, Some(30.0), false, None, &[], now()).unwrap();
        assert_eq!(res.deleted, vec!["mission_old"]);
        assert_eq!(remaining(&root), vec!["mission_broken"]);
        let _ = fs::remove_dir_all(&root);

        // Unknown schema version still protects parent via continuation hint
        let root = temp_root();
        seed(&root, &[("mission_first", 40, None)]);
        let newer = json!({
            "schemaVersion": MAX_KNOWN_SCHEMA_VERSION + 1,
            "recordType": "mission.created",
            "ledgerSequence": 1,
            "occurredAt": at(1),
            "metadata": {
                "missionId": "mission_reply",
                "continuesFrom": {"missionId": "mission_first"}
            }
        });
        fs::write(root.join("mission_reply.jsonl"), format!("{newer}\n")).unwrap();
        let res = prune_missions(&root, Some(30.0), false, None, &[], now()).unwrap();
        assert!(res.deleted.is_empty());
        assert_eq!(res.kept_for_continuity, vec!["mission_first"]);
        let _ = fs::remove_dir_all(&root);

        // Directory wearing a mission's name aborts everything
        let root = temp_root();
        seed(&root, &[("mission_old", 40, None)]);
        fs::create_dir_all(root.join("mission_locked.jsonl")).unwrap();
        let res = prune_missions(&root, Some(30.0), false, None, &[], now()).unwrap();
        assert!(res.deleted.is_empty());
        assert_eq!(res.unreadable, vec!["mission_locked"]);
        assert!(remaining(&root).contains(&"mission_old".to_owned()));
        let _ = fs::remove_dir_all(&root);

        // Move failure on one file reports both deleted and failed
        let root = temp_root();
        seed(
            &root,
            &[
                ("mission_one", 40, None),
                ("mission_two", 40, None),
                ("mission_three", 40, None),
            ],
        );
        let res = prune_missions_with_move(&root, Some(30.0), false, None, &[], now(), |dir, id| {
            if id == "mission_two" {
                return Err(io::Error::other("EBUSY"));
            }
            move_to_trash(dir, id)
        })
        .unwrap();
        assert_eq!(res.deleted, vec!["mission_one", "mission_three"]);
        assert_eq!(res.failed, vec!["mission_two"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn storage_report_and_preview_summary() {
        let root = temp_root();
        assert_eq!(read_storage_report(&root).unwrap(), StorageReport::default());

        seed(&root, &[("mission_one", 40, None), ("mission_two", 1, None)]);
        fs::write(root.join("mission_broken.jsonl"), "not json at all\n").unwrap();
        let report = read_storage_report(&root).unwrap();
        assert_eq!(report.mission_count, 3);
        assert_eq!(report.unreadable_count, 1);
        assert!(report.byte_total > 0);
        assert_eq!(report.oldest_updated_at.as_deref(), Some(at(40).as_str()));
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            prune_preview_summary(&PruneResult::default()),
            "Nothing is old enough to delete."
        );
        assert_eq!(
            prune_preview_summary(&PruneResult {
                deleted: vec!["m1".into()],
                kept_for_continuity: vec!["m2".into()],
                kept_as_running: vec!["m3".into()],
                ..Default::default()
            }),
            "Delete 1 turn for good, with 1 turn kept as part of a conversation you are keeping and 1 turn kept because they are running."
        );
    }
}
