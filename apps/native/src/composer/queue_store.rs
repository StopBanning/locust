//! Durable queue file — port of `main/queued-message-store.ts`.
//! Lives only under the native data folder (`Store`).

use crate::composer::queue::{Origin, QueuedRow};
use crate::store::Store;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const SCHEMA_VERSION: u64 = 1;

static ID_TICK: AtomicU64 = AtomicU64::new(1);

pub fn new_queue_id() -> String {
    let n = ID_TICK.fetch_add(1, Ordering::Relaxed);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("q-{ms}-{n}")
}

pub fn queue_path(store: &Store) -> io::Result<PathBuf> {
    store.queued_messages_path()
}

/// Strict parse of Electron's `{ schemaVersion: 1, rows }` file.
pub fn parse_queued_file(text: &str) -> Result<Vec<QueuedRow>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "Invalid queue file".to_string())?;
    let obj = value
        .as_object()
        .ok_or_else(|| "Invalid queue file".to_string())?;
    if obj.get("schemaVersion").and_then(|v| v.as_u64()) != Some(SCHEMA_VERSION) {
        return Err("Invalid queue version".into());
    }
    let rows = obj
        .get("rows")
        .ok_or_else(|| "Invalid queue file".to_string())?;
    saved_queued_messages(rows)
}

fn saved_issue(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let row = value?.as_object()?;
    let number = row
        .get("number")?
        .as_u64()
        .filter(|n| (1..=1_000_000_000).contains(n))?;
    let title = row.get("title")?.as_str()?.trim();
    let url = row.get("url")?.as_str()?;
    if title.is_empty() || title.encode_utf16().count() > 300 {
        return None;
    }
    // URL authority must be github.com; accept query/fragment as Electron's URL parser does.
    let path = url.strip_prefix("https://github.com")?;
    if !path.starts_with('/')
        || !path
            .split(['?', '#'])
            .next()?
            .ends_with(&format!("/issues/{number}"))
    {
        return None;
    }
    Some(serde_json::json!({"number":number,"title":title,"url":url}))
}

pub fn saved_queued_messages(value: &serde_json::Value) -> Result<Vec<QueuedRow>, String> {
    let arr = value
        .as_array()
        .ok_or_else(|| "Invalid queue".to_string())?;
    if arr.len() > 256 {
        return Err("Invalid queue".into());
    }
    arr.iter()
        .map(|entry| {
            let row = entry
                .as_object()
                .ok_or_else(|| "Invalid queue row".to_string())?;
            let id = row
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && s.encode_utf16().count() <= 256)
                .ok_or_else(|| "Invalid queue row".to_string())?
                .to_owned();
            let key = row
                .get("key")
                .and_then(|v| v.as_str())
                .filter(|s| {
                    !s.is_empty()
                        && s.len() <= 128
                        && s.chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '-')
                })
                .ok_or_else(|| "Invalid queue row".to_string())?
                .to_owned();
            let text = row
                .get("text")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty() && s.encode_utf16().count() <= 8_000)
                .ok_or_else(|| "Invalid queue row".to_string())?
                .to_owned();
            let attachments = match row.get("attachments") {
                None => Vec::new(),
                Some(v) => {
                    let list = v
                        .as_array()
                        .ok_or_else(|| "Invalid queue attachments".to_string())?;
                    if list.len() > 32 {
                        return Err("Invalid queue attachments".into());
                    }
                    list.iter()
                        .map(|p| {
                            p.as_str()
                                .filter(|s| !s.is_empty() && s.encode_utf16().count() <= 4096)
                                .map(str::to_owned)
                                .ok_or_else(|| "Invalid queue attachments".to_string())
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
            };
            Ok(QueuedRow {
                id,
                key,
                text,
                origin: Origin::Person,
                attachments,
                issue: saved_issue(row.get("issue")),
                restored: true,
                held: None,
            })
        })
        .collect()
}

pub fn serialize_queued(rows: &[QueuedRow]) -> Result<String, String> {
    let person: Vec<_> = rows
        .iter()
        .filter(|r| r.origin == Origin::Person)
        .map(|r| {
            let mut obj = serde_json::json!({
                "id": r.id,
                "key": r.key,
                "text": r.text,
            });
            if let Some(issue) = &r.issue {
                obj["issue"] = issue.clone();
            }
            if !r.attachments.is_empty() {
                obj["attachments"] = serde_json::json!(r.attachments);
            }
            obj
        })
        .collect();
    saved_queued_messages(&serde_json::Value::Array(person.clone()))?;
    let text = serde_json::to_string(&serde_json::json!({
        "schemaVersion": SCHEMA_VERSION,
        "rows": person,
    }))
    .map_err(|e| e.to_string())?;
    if text.len() > MAX_BYTES {
        return Err("Queue too large".into());
    }
    Ok(text)
}

/// Read the queue. Missing file → empty. Damaged file → error (never overwrite).
pub fn read_queue(store: &Store) -> Result<Vec<QueuedRow>, String> {
    let path = queue_path(store).map_err(|e| e.to_string())?;
    match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
        Ok(text) => {
            if text.len() > MAX_BYTES {
                return Err("Queue too large".into());
            }
            parse_queued_file(&text)
        }
    }
}

/// Atomic replace. Refuses to overwrite when the existing file is unreadable.
pub fn write_queue(store: &Store, rows: &[QueuedRow]) -> Result<Vec<QueuedRow>, String> {
    let path = queue_path(store).map_err(|e| e.to_string())?;
    // Refuse to overwrite a damaged file (Electron).
    let _ = read_queue(store)?;
    let text = serialize_queued(rows)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid queue path".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = path.with_extension(format!("{}.tmp", new_queue_id()));
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> io::Result<()> {
        let mut file = options.open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    std::fs::rename(&temporary, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        e.to_string()
    })?;
    Ok(rows
        .iter()
        .filter(|r| r.origin == Origin::Person)
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::queue::QueuedRow;

    #[test]
    fn persistence_matches_electron_utf16_bounds_issue_and_private_file() {
        let value = serde_json::json!([{"id":"id", "key":"conv", "text":"😀".repeat(4000),
            "issue":{"number":17,"title":" Issue title ","url":"https://github.com/a/b/issues/17"}}]);
        let rows = saved_queued_messages(&value).unwrap();
        let again = parse_queued_file(&serialize_queued(&rows).unwrap()).unwrap();
        assert_eq!(again[0].issue.as_ref().unwrap()["title"], "Issue title");
        let mut bad = rows;
        bad[0].text.push('a');
        assert!(serialize_queued(&bad).is_err());
    }

    #[test]
    fn round_trip_and_reject_bad() {
        let rows = vec![QueuedRow {
            id: "1".into(),
            key: "conv-1".into(),
            text: "hello".into(),
            origin: Origin::Person,
            attachments: vec!["a.md".into()],
            issue: None,
            restored: false,
            held: None,
        }];
        let text = serialize_queued(&rows).unwrap();
        let parsed = parse_queued_file(&text).unwrap();
        assert_eq!(parsed[0].text, "hello");
        assert!(parsed[0].restored);
        assert!(parse_queued_file("{}").is_err());
        assert!(parse_queued_file(r#"{"schemaVersion":2,"rows":[]}"#).is_err());
    }

    #[test]
    fn write_read_under_store() {
        let temp = std::env::temp_dir().join(format!("locust-q-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let electron = temp.join("electron");
        let native = temp.join("native");
        std::fs::create_dir_all(&electron).unwrap();
        let store = Store::with_codex(native, electron, vec![]).unwrap();
        let rows = vec![QueuedRow {
            id: "abc".into(),
            key: "k1".into(),
            text: "queued line".into(),
            origin: Origin::Person,
            attachments: Vec::new(),
            issue: None,
            restored: false,
            held: None,
        }];
        write_queue(&store, &rows).unwrap();
        let back = read_queue(&store).unwrap();
        assert_eq!(back[0].text, "queued line");
        assert!(back[0].restored);
        let _ = std::fs::remove_dir_all(temp);
    }
}
