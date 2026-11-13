//! Per-teammate store of tune lessons (Electron `main/tune-lesson-store.ts`).
//! Persisted in `tune-lessons.json` under the native data folder (read fallback to Electron if absent).

use crate::store::Store;
use crate::tune::PublicTuneLesson;
use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

pub const MAX_LESSONS_PER_TEAMMATE: usize = 20;
const SCHEMA_VERSION: u64 = 1;
const MAX_FILE_BYTES: usize = 1_048_576;
const FILE_NAME: &str = "tune-lessons.json";

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredTuneLesson {
    pub lesson_id: String,
    pub teammate_id: String,
    pub compare_id: String,
    pub created_at: String,
    pub verdict: String,
    pub lesson: String,
    pub ask: String,
}

impl StoredTuneLesson {
    fn to_json(&self) -> Value {
        json!({
            "lessonId": self.lesson_id,
            "teammateId": self.teammate_id,
            "compareId": self.compare_id,
            "createdAt": self.created_at,
            "verdict": self.verdict,
            "lesson": self.lesson,
            "ask": self.ask,
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        Some(Self {
            lesson_id: obj.get("lessonId")?.as_str()?.to_owned(),
            teammate_id: obj.get("teammateId")?.as_str()?.to_owned(),
            compare_id: obj.get("compareId")?.as_str()?.to_owned(),
            created_at: obj.get("createdAt")?.as_str()?.to_owned(),
            verdict: obj.get("verdict")?.as_str()?.to_owned(),
            lesson: obj.get("lesson")?.as_str()?.to_owned(),
            ask: obj.get("ask").and_then(Value::as_str).unwrap_or("").to_owned(),
        })
    }
}

pub struct NewTuneLesson<'a> {
    pub teammate_id: &'a str,
    pub compare_id: &'a str,
    pub verdict: &'a str,
    pub lesson: &'a str,
    pub ask: &'a str,
}

pub fn clean_field(text: &str, max: usize) -> String {
    let cleaned: String = text
        .chars()
        .filter(|&c| {
            let u = c as u32;
            !((u <= 0x1f) || u == 0x7f)
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    cleaned.chars().take(max).collect()
}

fn new_lesson_id() -> String {
    format!("tln_{}", crate::invariants::unique_nanos())
}

fn read_lessons_raw(store: &Store) -> Vec<StoredTuneLesson> {
    let native_path = store.root().join(FILE_NAME);
    let path = if native_path.exists() {
        native_path
    } else {
        let electron_path = store.electron().join(FILE_NAME);
        if electron_path.exists() {
            electron_path
        } else {
            return Vec::new();
        }
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        return Vec::new();
    };
    if meta.len() as usize > MAX_FILE_BYTES {
        return Vec::new();
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    v.get("lessons")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(StoredTuneLesson::from_json).collect())
        .unwrap_or_default()
}

fn save_lessons_raw(store: &Store, lessons: &[StoredTuneLesson]) -> Result<(), String> {
    let payload = json!({
        "schemaVersion": SCHEMA_VERSION,
        "lessons": lessons.iter().map(StoredTuneLesson::to_json).collect::<Vec<_>>(),
    });
    store.save_json(FILE_NAME, &payload)
}

/// Dedupes by `(teammateId, compareId)` keeping the newest last, capped at `MAX_LESSONS_PER_TEAMMATE` per teammate.
fn cap_per_teammate(lessons: Vec<StoredTuneLesson>) -> Vec<StoredTuneLesson> {
    let mut deduped: Vec<StoredTuneLesson> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for item in lessons.into_iter().rev() {
        let key = format!("{}:{}", item.teammate_id, item.compare_id);
        if seen.insert(key) {
            deduped.push(item);
        }
    }
    deduped.reverse();

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut kept_rev: Vec<StoredTuneLesson> = Vec::new();
    for item in deduped.into_iter().rev() {
        let c = counts.entry(item.teammate_id.clone()).or_insert(0);
        if *c < MAX_LESSONS_PER_TEAMMATE {
            *c += 1;
            kept_rev.push(item);
        }
    }
    kept_rev.reverse();
    kept_rev
}

impl From<StoredTuneLesson> for PublicTuneLesson {
    fn from(s: StoredTuneLesson) -> Self {
        Self {
            lesson_id: s.lesson_id,
            teammate_id: s.teammate_id,
            compare_id: s.compare_id,
            created_at: s.created_at,
            verdict: s.verdict,
            lesson: s.lesson,
            ask: s.ask,
        }
    }
}

pub fn list(store: &Store, teammate_id: Option<&str>) -> Vec<PublicTuneLesson> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_lessons_raw(store)
        .into_iter()
        .filter(|l| teammate_id.is_none_or(|id| l.teammate_id == id))
        .map(PublicTuneLesson::from)
        .collect()
}

pub fn add(store: &Store, input: NewTuneLesson<'_>) -> Result<PublicTuneLesson, String> {
    let lesson_line = crate::tune::tune_lesson_line(input.lesson)
        .ok_or_else(|| "A lesson must start with 'When ..., ...' and fit in 400 characters.".to_owned())?;
    let teammate_id = clean_field(input.teammate_id, 120);
    let compare_id = clean_field(input.compare_id, 120);
    if teammate_id.is_empty() || compare_id.is_empty() {
        return Err("Choose a teammate and comparison before saving a lesson.".into());
    }
    let verdict = clean_field(input.verdict, 40);
    let ask = clean_field(input.ask, 240);
    let entry = StoredTuneLesson {
        lesson_id: new_lesson_id(),
        teammate_id: teammate_id.clone(),
        compare_id: compare_id.clone(),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        verdict,
        lesson: lesson_line,
        ask,
    };
    let _g = LOCK.lock().map_err(|e| e.to_string())?;
    let mut lessons = read_lessons_raw(store);
    lessons.push(entry.clone());
    lessons = cap_per_teammate(lessons);
    let saved = lessons
        .iter()
        .find(|l| l.teammate_id == teammate_id && l.compare_id == compare_id)
        .cloned()
        .unwrap_or(entry);
    save_lessons_raw(store, &lessons)?;
    Ok(PublicTuneLesson::from(saved))
}

pub fn remove(store: &Store, lesson_id: &str) -> Result<bool, String> {
    let _g = LOCK.lock().map_err(|e| e.to_string())?;
    let mut lessons = read_lessons_raw(store);
    let before = lessons.len();
    lessons.retain(|l| l.lesson_id != lesson_id);
    if lessons.len() == before {
        return Ok(false);
    }
    save_lessons_raw(store, &lessons)?;
    Ok(true)
}

pub fn remove_for_teammate(store: &Store, teammate_id: &str) -> Result<usize, String> {
    let _g = LOCK.lock().map_err(|e| e.to_string())?;
    let mut lessons = read_lessons_raw(store);
    let before = lessons.len();
    lessons.retain(|l| l.teammate_id != teammate_id);
    let removed = before - lessons.len();
    if removed > 0 {
        save_lessons_raw(store, &lessons)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempRoot(PathBuf);
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_store(name: &str) -> (TempRoot, Store) {
        let dir = std::env::temp_dir().join(format!(
            "locust-tune-lessons-{name}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        let native = dir.join("native");
        let electron = dir.join("electron");
        std::fs::create_dir_all(&native).unwrap();
        std::fs::create_dir_all(&electron).unwrap();
        (TempRoot(dir), Store::new(native, electron).unwrap())
    }

    #[test]
    fn dedupes_by_compare_caps_at_20_and_removes_per_teammate() {
        let (_tmp, store) = test_store("cap");
        for i in 0..25 {
            add(
                &store,
                NewTuneLesson {
                    teammate_id: "tm_1",
                    compare_id: &format!("cmp_{i}"),
                    verdict: "helped",
                    lesson: &format!("When task is #{i}, check condition {i}."),
                    ask: "Ask",
                },
            )
            .unwrap();
        }
        add(
            &store,
            NewTuneLesson {
                teammate_id: "tm_1",
                compare_id: "cmp_24",
                verdict: "hurt",
                lesson: "When task is #24, updated phrasing.",
                ask: "Ask 2",
            },
        )
        .unwrap();

        let listed = list(&store, Some("tm_1"));
        assert_eq!(listed.len(), MAX_LESSONS_PER_TEAMMATE);
        assert_eq!(listed.last().unwrap().compare_id, "cmp_24");
        assert!(listed.last().unwrap().lesson.contains("updated phrasing"));
        assert!(!listed.iter().any(|l| l.compare_id == "cmp_0"));

        add(
            &store,
            NewTuneLesson {
                teammate_id: "tm_2",
                compare_id: "cmp_other",
                verdict: "helped",
                lesson: "When other, keep separate.",
                ask: "Other",
            },
        )
        .unwrap();

        let first_id = listed[0].lesson_id.clone();
        assert!(remove(&store, &first_id).unwrap());
        assert_eq!(list(&store, Some("tm_1")).len(), MAX_LESSONS_PER_TEAMMATE - 1);

        remove_for_teammate(&store, "tm_1").unwrap();
        assert!(list(&store, Some("tm_1")).is_empty());
        assert_eq!(list(&store, Some("tm_2")).len(), 1);
    }
}
