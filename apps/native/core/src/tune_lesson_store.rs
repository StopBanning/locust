//! Per-teammate store of tune lessons (Electron `main/tune-lesson-store.ts`).
//! Persisted in `tune-lessons.json` under the native data folder (read fallback to Electron if absent).

use crate::store::Store;
use crate::tune::{PublicTuneLesson, TuneLessonConfidence, TuneLessonDraft};
use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

pub const MAX_LESSONS_PER_TEAMMATE: usize = 20;
const SCHEMA_VERSION: u64 = 1;
const MAX_FILE_BYTES: usize = 1_048_576;
const FILE_NAME: &str = "tune-lessons.json";

static LOCK: Mutex<()> = Mutex::new(());

/// A lesson as Electron's `tune-lesson-store.ts` keeps it: `{when, do, because, confidence}` with its tune's ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredTuneLesson {
    pub lesson_id: String,
    pub teammate_id: String,
    pub compare_id: String,
    pub created_at: String,
    pub when: String,
    pub r#do: String,
    pub because: String,
    pub confidence: TuneLessonConfidence,
}

/// Electron's `cleanField`: one line, whitespace collapsed, refused when empty, too long or holding a control character.
fn checked_field(value: Option<&Value>, max: usize) -> Option<String> {
    let text = value?.as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
    let bad = text.chars().any(|c| (c as u32) <= 0x1f || c as u32 == 0x7f);
    (!text.is_empty() && text.chars().count() <= max && !bad).then_some(text)
}

impl StoredTuneLesson {
    fn to_json(&self) -> Value {
        json!({
            "lessonId": self.lesson_id,
            "teammateId": self.teammate_id,
            "compareId": self.compare_id,
            "createdAt": self.created_at,
            "when": self.when,
            "do": self.r#do,
            "because": self.because,
            "confidence": self.confidence.as_str(),
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let lesson_id = checked_field(obj.get("lessonId"), 80)?;
        let teammate_id = checked_field(obj.get("teammateId"), 200)?;
        let compare_id = checked_field(obj.get("compareId"), 80)?;
        let created_at = checked_field(obj.get("createdAt"), 40)?;
        if obj.contains_key("when") || obj.contains_key("do") {
            // Electron's isConfidence: the word exactly as it is written, never read loosely as the parser does.
            let confidence = match obj.get("confidence").and_then(Value::as_str) {
                Some(word @ ("high" | "medium" | "low")) => TuneLessonConfidence::parse(word)?,
                _ => return None,
            };
            return Some(Self {
                lesson_id,
                teammate_id,
                compare_id,
                created_at,
                when: checked_field(obj.get("when"), 400)?,
                r#do: checked_field(obj.get("do"), 400)?,
                because: checked_field(obj.get("because"), 400)?,
                confidence,
            });
        }
        // Rust's own older shape (before 21 Nov): one "When …, …" line and the kept column's verdict. Electron never
        // wrote it, so this reading is Rust's: the line splits at its first comma into when and do, the verdict is what
        // it rested on, and one tune read it, so its confidence is "low".
        let line = checked_field(obj.get("lesson"), 400)?;
        let rest = line.strip_prefix("When ").or_else(|| line.strip_prefix("when "))?;
        let (when, action) = rest.split_once(',')?;
        let (when, action) = (when.trim(), action.trim().trim_end_matches('.'));
        if when.is_empty() || action.is_empty() {
            return None;
        }
        let verdict = obj.get("verdict").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).unwrap_or("kept");
        Some(Self {
            lesson_id,
            teammate_id,
            compare_id,
            created_at,
            when: when.to_owned(),
            r#do: action.to_owned(),
            because: format!("The kept setup's edit {verdict}, in an earlier note of this tune."),
            confidence: TuneLessonConfidence::Low,
        })
    }
}

pub struct NewTuneLesson<'a> {
    pub teammate_id: &'a str,
    pub compare_id: &'a str,
    pub lesson: &'a TuneLessonDraft,
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
            when: s.when,
            r#do: s.r#do,
            because: s.because,
            confidence: s.confidence,
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
    let field = |text: &str, max| checked_field(Some(&Value::String(text.to_owned())), max);
    let (Some(teammate_id), Some(compare_id), Some(when), Some(action), Some(because)) = (
        field(input.teammate_id, 200),
        field(input.compare_id, 80),
        field(&input.lesson.when, 400),
        field(&input.lesson.r#do, 400),
        field(&input.lesson.because, 400),
    ) else {
        return Err("Write a valid tune lesson before saving.".to_owned());
    };
    let entry = StoredTuneLesson {
        lesson_id: new_lesson_id(),
        teammate_id: teammate_id.clone(),
        compare_id: compare_id.clone(),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        when,
        r#do: action,
        because,
        confidence: input.lesson.confidence,
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

    fn lesson(when: &str, action: &str, because: &str, confidence: TuneLessonConfidence) -> TuneLessonDraft {
        TuneLessonDraft { when: when.into(), r#do: action.into(), because: because.into(), confidence }
    }

    #[test]
    fn dedupes_by_compare_caps_at_20_and_removes_per_teammate() {
        let (_tmp, store) = test_store("cap");
        for i in 1..=25 {
            let l = lesson(&format!("situation {i}"), &format!("change {i}"), &format!("trial {i}"), TuneLessonConfidence::Low);
            add(&store, NewTuneLesson { teammate_id: "tm_1", compare_id: &format!("cmp_{i}"), lesson: &l }).unwrap();
        }
        let updated = lesson("updated note", "updated rule", "how helped", TuneLessonConfidence::Medium);
        add(&store, NewTuneLesson { teammate_id: "tm_1", compare_id: "cmp_25", lesson: &updated }).unwrap();

        let listed = list(&store, Some("tm_1"));
        assert_eq!(listed.len(), MAX_LESSONS_PER_TEAMMATE);
        assert_eq!(listed[0].compare_id, "cmp_6");
        assert_eq!(listed.last().unwrap().compare_id, "cmp_25");
        assert_eq!(listed.last().unwrap().when, "updated note");
        assert_eq!(listed.last().unwrap().confidence, TuneLessonConfidence::Medium);

        let other = lesson("reviewing a diff", "name each file changed", "knows helped", TuneLessonConfidence::High);
        add(&store, NewTuneLesson { teammate_id: "tm_2", compare_id: "cmp_other", lesson: &other }).unwrap();

        let first_id = listed[0].lesson_id.clone();
        assert!(remove(&store, &first_id).unwrap());
        assert_eq!(list(&store, Some("tm_1")).len(), MAX_LESSONS_PER_TEAMMATE - 1);

        remove_for_teammate(&store, "tm_1").unwrap();
        assert!(list(&store, Some("tm_1")).is_empty());
        assert_eq!(list(&store, Some("tm_2")).len(), 1);
    }

    #[test]
    fn a_lesson_that_will_not_read_whole_is_refused() {
        let (_tmp, store) = test_store("refused");
        for bad in [
            lesson("", "check", "how helped", TuneLessonConfidence::Low),
            lesson("answering", "check\u{0}it", "how helped", TuneLessonConfidence::Low),
            lesson("answering", "check", &"x".repeat(401), TuneLessonConfidence::Low),
        ] {
            let err = add(&store, NewTuneLesson { teammate_id: "tm_1", compare_id: "cmp_1", lesson: &bad }).unwrap_err();
            assert_eq!(err, "Write a valid tune lesson before saving.");
        }
        assert!(list(&store, Some("tm_1")).is_empty());
    }

    // Electron's file is written in this shape, and Rust's own older rows ("When …, …" with a verdict) still read.
    #[test]
    fn reads_electrons_lessons_and_rusts_older_ones() {
        let (_tmp, store) = test_store("older");
        std::fs::write(
            store.electron().join(FILE_NAME),
            serde_json::to_string(&json!({"schemaVersion": 1, "lessons": [
                {"lessonId": "les_1", "teammateId": "tm_wren", "compareId": "cmp_1", "createdAt": "2026-10-21T00:00:00.000Z",
                 "when": "explaining a cause", "do": "require a final check", "because": "How it works helped", "confidence": "medium"},
                {"lessonId": "les_2", "teammateId": "tm_wren", "compareId": "cmp_2", "createdAt": "2026-10-21T00:01:00.000Z",
                 "when": "x", "do": "y", "because": "z", "confidence": "certain"},
                {"lessonId": "les_3", "teammateId": "tm_wren", "compareId": "cmp_3", "createdAt": "2026-10-21T00:02:00.000Z",
                 "when": "x", "do": "y", "because": "z", "confidence": "Medium"}
            ]})).unwrap(),
        )
        .unwrap();
        let electron = list(&store, Some("tm_wren"));
        assert_eq!(electron.len(), 1);
        assert_eq!((electron[0].when.as_str(), electron[0].r#do.as_str()), ("explaining a cause", "require a final check"));
        assert_eq!(electron[0].confidence, TuneLessonConfidence::Medium);

        std::fs::write(
            store.root().join(FILE_NAME),
            serde_json::to_string(&json!({"schemaVersion": 1, "lessons": [
                {"lessonId": "tln_1", "teammateId": "tm_codec", "compareId": "cmp_9", "createdAt": "2026-11-20T00:00:00.000Z",
                 "verdict": "helped", "lesson": "When editing core modules, check tests before replying.", "ask": "Fix it"},
                {"lessonId": "tln_2", "teammateId": "tm_codec", "compareId": "cmp_10", "createdAt": "2026-11-20T00:00:00.000Z",
                 "verdict": "helped", "lesson": "Always check.", "ask": "Fix it"}
            ]})).unwrap(),
        )
        .unwrap();
        let older = list(&store, Some("tm_codec"));
        assert_eq!(older.len(), 1);
        assert_eq!(older[0].when, "editing core modules");
        assert_eq!(older[0].r#do, "check tests before replying");
        assert!(older[0].because.contains("helped"));
        assert_eq!(older[0].confidence, TuneLessonConfidence::Low);
        assert_eq!(
            crate::tune::tune_lesson_line(&older[0].when, &older[0].r#do),
            "When editing core modules, check tests before replying."
        );
        // A new lesson rewrites the file in the new shape, the older row with it.
        let l = lesson("reviewing", "name files", "knows helped", TuneLessonConfidence::Low);
        add(&store, NewTuneLesson { teammate_id: "tm_codec", compare_id: "cmp_11", lesson: &l }).unwrap();
        let disk = std::fs::read_to_string(store.root().join(FILE_NAME)).unwrap();
        assert!(!disk.contains("\"verdict\""), "{disk}");
        assert_eq!(list(&store, Some("tm_codec")).len(), 2);
    }
}
