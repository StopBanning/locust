//! Per-teammate store of skills saved from tunes (Electron `main/teammate-skills.ts`).
//! Persisted under `<native>/teammate-skills/` (reading `<electron>/teammate-skills/` if native has none).

use crate::store::Store;
use crate::tune::{
    PublicTeammateSkill, TuneSkill, is_valid_skill_name, skill_markdown, tune_skill_of,
};
use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MAX_SKILLS_PER_TEAMMATE: usize = 20;
const SCHEMA_VERSION: u64 = 1;
const STORE_DIR: &str = "teammate-skills";
const INDEX_FILE: &str = "skills.json";

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredTeammateSkill {
    pub skill_id: String,
    pub teammate_id: String,
    pub name: String,
    pub description: String,
    pub added_at: String,
    pub compare_id: Option<String>,
}

impl StoredTeammateSkill {
    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("skillId".into(), json!(self.skill_id));
        obj.insert("teammateId".into(), json!(self.teammate_id));
        obj.insert("name".into(), json!(self.name));
        obj.insert("description".into(), json!(self.description));
        obj.insert("addedAt".into(), json!(self.added_at));
        if let Some(c) = &self.compare_id {
            obj.insert("compareId".into(), json!(c));
        }
        Value::Object(obj)
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        Some(Self {
            skill_id: obj.get("skillId")?.as_str()?.to_owned(),
            teammate_id: obj.get("teammateId")?.as_str()?.to_owned(),
            name: obj.get("name")?.as_str()?.to_owned(),
            description: obj.get("description")?.as_str()?.to_owned(),
            added_at: obj.get("addedAt")?.as_str()?.to_owned(),
            compare_id: obj
                .get("compareId")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

fn new_skill_id() -> String {
    format!("tsk_{}", crate::invariants::unique_nanos())
}

fn is_safe_segment(s: &str) -> bool {
    !s.is_empty()
        && !s.contains('/')
        && !s.contains('\\')
        && !s.contains("..")
        && !s.contains('\0')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn active_base_dir(store: &Store) -> PathBuf {
    let native_dir = store.root().join(STORE_DIR);
    if native_dir.join(INDEX_FILE).exists() {
        native_dir
    } else if store.electron().join(STORE_DIR).join(INDEX_FILE).exists() {
        store.electron().join(STORE_DIR)
    } else {
        native_dir
    }
}

/// Ensures native `teammate-skills/` holds any pre-existing Electron skills before we mutate it,
/// so Electron's folder remains untouched and read-only.
fn ensure_native_seed(store: &Store) -> Result<PathBuf, String> {
    let native_dir = store.root().join(STORE_DIR);
    let native_idx = native_dir.join(INDEX_FILE);
    if native_idx.exists() {
        return Ok(native_dir);
    }
    fs::create_dir_all(&native_dir).map_err(|e| e.to_string())?;
    let electron_dir = store.electron().join(STORE_DIR);
    let electron_idx = electron_dir.join(INDEX_FILE);
    if electron_idx.exists() {
        let skills = read_index_at(&electron_dir);
        for sk in &skills {
            if is_safe_segment(&sk.teammate_id) && is_valid_skill_name(&sk.name) {
                let src = electron_dir
                    .join(&sk.teammate_id)
                    .join(&sk.name)
                    .join("SKILL.md");
                if let Ok(content) = fs::read_to_string(&src) {
                    let dst_dir = native_dir.join(&sk.teammate_id).join(&sk.name);
                    let _ = fs::create_dir_all(&dst_dir);
                    let _ = fs::write(dst_dir.join("SKILL.md"), content);
                }
            }
        }
        save_index_at(&native_dir, &skills)?;
    }
    Ok(native_dir)
}

fn read_index_at(base: &Path) -> Vec<StoredTeammateSkill> {
    let Ok(raw) = fs::read_to_string(base.join(INDEX_FILE)) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    v.get("skills")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(StoredTeammateSkill::from_json).collect())
        .unwrap_or_default()
}

fn save_index_at(base: &Path, skills: &[StoredTeammateSkill]) -> Result<(), String> {
    fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let payload = json!({
        "schemaVersion": SCHEMA_VERSION,
        "skills": skills.iter().map(StoredTeammateSkill::to_json).collect::<Vec<_>>(),
    });
    let data = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    let tmp = base.join(format!("{INDEX_FILE}.tmp"));
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    fs::rename(&tmp, base.join(INDEX_FILE)).map_err(|e| e.to_string())
}

fn read_skill_body_at(base: &Path, teammate_id: &str, name: &str) -> Option<String> {
    if !is_safe_segment(teammate_id) || !is_valid_skill_name(name) {
        return None;
    }
    let path = base.join(teammate_id).join(name).join("SKILL.md");
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if let Some(after_open) = trimmed.strip_prefix("---\n")
        && let Some(end) = after_open.find("\n---")
    {
        let body = after_open[end + 4..].trim();
        return (!body.is_empty()).then(|| body.to_owned());
    }
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn write_skill_dir_atomic(base: &Path, teammate_id: &str, skill: &TuneSkill) -> Result<PathBuf, String> {
    if !is_safe_segment(teammate_id) || !is_valid_skill_name(&skill.name) {
        return Err("That teammate or skill name is not safe for a folder.".into());
    }
    let tm_dir = base.join(teammate_id);
    fs::create_dir_all(&tm_dir).map_err(|e| e.to_string())?;
    let final_dir = tm_dir.join(&skill.name);
    let tmp_dir = tm_dir.join(format!(".{}.tmp-{}", skill.name, crate::invariants::unique_nanos()));
    let old_dir = tm_dir.join(format!(".{}.old-{}", skill.name, crate::invariants::unique_nanos()));

    fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;
    if let Err(e) = fs::write(tmp_dir.join("SKILL.md"), skill_markdown(skill)) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(e.to_string());
    }
    let had_old = final_dir.exists();
    if had_old && let Err(e) = fs::rename(&final_dir, &old_dir) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(e.to_string());
    }
    if let Err(e) = fs::rename(&tmp_dir, &final_dir) {
        if had_old {
            let _ = fs::rename(&old_dir, &final_dir);
        }
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(e.to_string());
    }
    if had_old {
        let _ = fs::remove_dir_all(&old_dir);
    }
    Ok(final_dir)
}

fn cap_per_teammate(
    skills: Vec<StoredTeammateSkill>,
) -> (Vec<StoredTeammateSkill>, Vec<StoredTeammateSkill>) {
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for sk in skills.into_iter().rev() {
        let key = format!("{}:{}", sk.teammate_id, sk.name);
        if seen.insert(key) {
            deduped.push(sk);
        }
    }
    deduped.reverse();

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut kept_rev = Vec::new();
    let mut evicted = Vec::new();
    for sk in deduped.into_iter().rev() {
        let c = counts.entry(sk.teammate_id.clone()).or_insert(0);
        if *c < MAX_SKILLS_PER_TEAMMATE {
            *c += 1;
            kept_rev.push(sk);
        } else {
            evicted.push(sk);
        }
    }
    kept_rev.reverse();
    (kept_rev, evicted)
}

fn to_public(base: &Path, sk: StoredTeammateSkill) -> PublicTeammateSkill {
    let file_path = base.join(&sk.teammate_id).join(&sk.name).join("SKILL.md");
    let body = read_skill_body_at(base, &sk.teammate_id, &sk.name);
    PublicTeammateSkill {
        skill_id: sk.skill_id,
        teammate_id: sk.teammate_id,
        name: sk.name,
        description: sk.description,
        body,
        added_at: sk.added_at,
        compare_id: sk.compare_id,
        file: file_path.exists().then(|| file_path.to_string_lossy().into_owned()),
    }
}

pub fn list(store: &Store, teammate_id: Option<&str>) -> Vec<PublicTeammateSkill> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = active_base_dir(store);
    read_index_at(&base)
        .into_iter()
        .filter(|s| teammate_id.is_none_or(|id| s.teammate_id == id))
        .map(|s| to_public(&base, s))
        .collect()
}

pub fn folder_of(store: &Store, teammate_id: &str) -> Option<PathBuf> {
    if !is_safe_segment(teammate_id) {
        return None;
    }
    let dir = active_base_dir(store).join(teammate_id);
    dir.is_dir().then_some(dir)
}

pub fn skill_names_for(store: &Store, teammate_id: &str) -> Vec<String> {
    list(store, Some(teammate_id))
        .into_iter()
        .map(|s| s.name)
        .collect()
}

pub fn kept_skill_text(store: &Store, teammate_id: &str, name: &str) -> Option<String> {
    if !is_safe_segment(teammate_id) || !is_valid_skill_name(name) {
        return None;
    }
    let base = active_base_dir(store);
    let path = base.join(teammate_id).join(name).join("SKILL.md");
    fs::read_to_string(path).ok()
}

pub fn add(
    store: &Store,
    teammate_id: &str,
    skill: &TuneSkill,
    compare_id: Option<&str>,
) -> Result<PublicTeammateSkill, String> {
    let clean = tune_skill_of(&json!({
        "name": skill.name,
        "description": skill.description,
        "body": skill.body,
    }))
    .ok_or_else(|| crate::tune::TUNE_SKILL_INCOMPLETE.to_owned())?;
    if !is_safe_segment(teammate_id) {
        return Err("That teammate id cannot hold a skill.".into());
    }

    let _g = LOCK.lock().map_err(|e| e.to_string())?;
    let base = ensure_native_seed(store)?;
    write_skill_dir_atomic(&base, teammate_id, &clean)?;

    let mut skills = read_index_at(&base);
    let entry = StoredTeammateSkill {
        skill_id: new_skill_id(),
        teammate_id: teammate_id.to_owned(),
        name: clean.name.clone(),
        description: clean.description.clone(),
        added_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        compare_id: compare_id.map(str::to_owned),
    };
    skills.push(entry.clone());
    let (kept, evicted) = cap_per_teammate(skills);
    for ev in evicted {
        if !kept
            .iter()
            .any(|k| k.teammate_id == ev.teammate_id && k.name == ev.name)
        {
            let _ = fs::remove_dir_all(base.join(&ev.teammate_id).join(&ev.name));
        }
    }
    save_index_at(&base, &kept)?;
    let stored = kept
        .into_iter()
        .find(|s| s.teammate_id == teammate_id && s.name == clean.name)
        .unwrap_or(entry);
    Ok(to_public(&base, stored))
}

pub fn remove(store: &Store, skill_id: &str) -> Result<bool, String> {
    let _g = LOCK.lock().map_err(|e| e.to_string())?;
    let base = ensure_native_seed(store)?;
    let mut skills = read_index_at(&base);
    let Some(pos) = skills.iter().position(|s| s.skill_id == skill_id) else {
        return Ok(false);
    };
    let removed = skills.remove(pos);
    save_index_at(&base, &skills)?;
    if is_safe_segment(&removed.teammate_id) && is_valid_skill_name(&removed.name) {
        let _ = fs::remove_dir_all(base.join(&removed.teammate_id).join(&removed.name));
    }
    Ok(true)
}

pub fn remove_for_teammate(store: &Store, teammate_id: &str) -> Result<usize, String> {
    if !is_safe_segment(teammate_id) {
        return Ok(0);
    }
    let _g = LOCK.lock().map_err(|e| e.to_string())?;
    let base = ensure_native_seed(store)?;
    let mut skills = read_index_at(&base);
    let before = skills.len();
    skills.retain(|s| s.teammate_id != teammate_id);
    let removed = before - skills.len();
    if removed > 0 {
        save_index_at(&base, &skills)?;
    }
    let _ = fs::remove_dir_all(base.join(teammate_id));
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRoot(PathBuf);
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_store(name: &str) -> (TempRoot, Store) {
        let dir = std::env::temp_dir().join(format!(
            "locust-teammate-skills-{name}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        let native = dir.join("native");
        let electron = dir.join("electron");
        fs::create_dir_all(&native).unwrap();
        fs::create_dir_all(&electron).unwrap();
        (TempRoot(dir), Store::new(native, electron).unwrap())
    }

    #[test]
    fn saves_replaces_same_name_caps_at_20_and_removes() {
        let (_tmp, store) = test_store("cap");
        for i in 0..23 {
            add(
                &store,
                "tm_1",
                &TuneSkill {
                    name: format!("skill-{i}"),
                    description: format!("Use for {i}"),
                    body: format!("Step {i}"),
                },
                Some(&format!("cmp_{i}")),
            )
            .unwrap();
        }
        add(
            &store,
            "tm_1",
            &TuneSkill {
                name: "skill-22".into(),
                description: "Updated desc".into(),
                body: "Updated body".into(),
            },
            Some("cmp_new"),
        )
        .unwrap();

        let listed = list(&store, Some("tm_1"));
        assert_eq!(listed.len(), MAX_SKILLS_PER_TEAMMATE);
        let newest = listed.last().unwrap();
        assert_eq!(newest.name, "skill-22");
        assert_eq!(newest.description, "Updated desc");
        assert_eq!(newest.body.as_deref(), Some("Updated body"));
        assert!(!store.root().join(STORE_DIR).join("tm_1").join("skill-0").exists());

        let md = kept_skill_text(&store, "tm_1", "skill-22").unwrap();
        assert!(md.contains("description: Updated desc"));
        assert!(md.contains("Updated body"));

        remove(&store, &newest.skill_id).unwrap();
        assert!(!store.root().join(STORE_DIR).join("tm_1").join("skill-22").exists());

        remove_for_teammate(&store, "tm_1").unwrap();
        assert!(list(&store, Some("tm_1")).is_empty());
        assert!(!store.root().join(STORE_DIR).join("tm_1").exists());
    }
}
