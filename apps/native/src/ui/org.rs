//! Conversation organization persistence.
//!
//! Electron keeps titles, pins and settled marks in `teammates.json`, and projects in
//! `groups.json` (`main/teammate-store.ts`, `main/group-store.ts`). Native reads those
//! files from Electron's data folder read-only, and writes only under the native data
//! folder via `store::Store` — same field shapes so existing pins/names show.

use crate::settled::SettledEntry;
use crate::store::Store;
pub use locust_core::groups::{Group, GroupMembership, GroupRoute, LeftMembership};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_MISSION_PINS: usize = 50;
pub const MAX_MISSION_TITLES: usize = 1_000;
pub const MAX_MISSION_TITLE_LENGTH: usize = 120;
pub const MAX_MISSION_SETTLED: usize = 1_000;
const MAX_FILE_BYTES: u64 = 1_000_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrgState {
    pub titles: HashMap<String, String>,
    pub pins: Vec<String>,
    pub settled: HashMap<String, SettledEntry>,
    pub groups: Vec<Group>,
    pub members: HashMap<String, GroupMembership>,
    pub left: HashMap<String, Vec<LeftMembership>>,
    pub groups_unreadable: bool,
    pub teammates_org_unreadable: bool,
}

fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn read_capped(path: &Path, max: u64) -> Result<Option<String>, String> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("unreadable".into()),
    };
    let mut text = String::new();
    file.take(max + 1)
        .read_to_string(&mut text)
        .map_err(|_| "unreadable".to_string())?;
    if text.len() as u64 > max {
        return Err("unreadable".into());
    }
    Ok(Some(text))
}

fn utf16_prefix(value: &str, max: usize) -> String {
    let mut count = 0;
    value
        .chars()
        .take_while(|c| {
            count += c.len_utf16();
            count <= max
        })
        .collect()
}

fn parse_titles(record: &Value) -> HashMap<String, String> {
    let mut titles = HashMap::new();
    let Some(obj) = record.get("missionTitles").and_then(|v| v.as_object()) else {
        return titles;
    };
    for (mission_id, title) in obj {
        if titles.len() >= MAX_MISSION_TITLES {
            break;
        }
        if !safe_id(mission_id) {
            continue;
        }
        let Some(raw) = title.as_str() else {
            continue;
        };
        let trimmed = utf16_prefix(raw.trim(), MAX_MISSION_TITLE_LENGTH);
        // Electron uses UTF-16 length; ASCII titles match. Cap by chars for parity tests.
        let utf16 = trimmed.encode_utf16().count();
        if utf16 == 0 || utf16 > MAX_MISSION_TITLE_LENGTH {
            continue;
        }
        titles.insert(mission_id.clone(), trimmed);
    }
    titles
}

fn parse_pins(record: &Value) -> Vec<String> {
    let mut pins = Vec::new();
    let Some(arr) = record.get("missionPins").and_then(|v| v.as_array()) else {
        return pins;
    };
    for mission_id in arr {
        if pins.len() >= MAX_MISSION_PINS {
            break;
        }
        let Some(id) = mission_id.as_str() else {
            continue;
        };
        if safe_id(id) && !pins.iter().any(|p| p == id) {
            pins.push(id.to_owned());
        }
    }
    pins
}

fn parse_settled(record: &Value) -> HashMap<String, SettledEntry> {
    let mut settled = HashMap::new();
    let Some(obj) = record.get("missionSettled").and_then(|v| v.as_object()) else {
        return settled;
    };
    for (mission_id, entry) in obj {
        if settled.len() >= MAX_MISSION_SETTLED {
            break;
        }
        if !safe_id(mission_id) || !entry.is_object() {
            continue;
        }
        let Some(at) = entry.get("at").and_then(|v| v.as_str()) else {
            continue;
        };
        if chrono::DateTime::parse_from_rfc3339(at).is_err() {
            continue;
        }
        let until = entry.get("until").and_then(|v| v.as_str()).and_then(|u| {
            chrono::DateTime::parse_from_rfc3339(u)
                .ok()
                .map(|_| u.to_owned())
        });
        settled.insert(
            mission_id.clone(),
            SettledEntry {
                at: at.to_owned(),
                until,
            },
        );
    }
    settled
}

type TeammatesOrg = (
    HashMap<String, String>,
    Vec<String>,
    HashMap<String, SettledEntry>,
);
fn parse_teammates_org(text: &str) -> Result<TeammatesOrg, String> {
    let file: Value = serde_json::from_str(text).map_err(|_| "unreadable".to_string())?;
    if !file.is_object() || file.get("schemaVersion") != Some(&json!(1)) {
        return Err("unreadable".into());
    }
    Ok((parse_titles(&file), parse_pins(&file), parse_settled(&file)))
}

/// Load organization state: Electron (read-only) as base, native overlay on top.
pub fn load_org(store: &Store) -> OrgState {
    let mut state = OrgState::default();
    let electron = store.electron();

    match read_capped(&electron.join("teammates.json"), MAX_FILE_BYTES) {
        Ok(Some(text)) => match parse_teammates_org(&text) {
            Ok((titles, pins, settled)) => {
                state.titles = titles;
                state.pins = pins;
                state.settled = settled;
            }
            Err(_) => state.teammates_org_unreadable = true,
        },
        Ok(None) => {}
        Err(_) => state.teammates_org_unreadable = true,
    }

    if let Ok(path) = store.teammates_json_path() {
        match read_capped(&path, MAX_FILE_BYTES) {
            Ok(None) => {}
            Ok(Some(text)) => match parse_teammates_org(&text) {
                Ok((titles, pins, settled)) => {
                    state.titles = titles;
                    state.pins = pins;
                    state.settled = settled;
                    state.teammates_org_unreadable = false;
                }
                Err(_) => state.teammates_org_unreadable = true,
            },
            Err(_) => state.teammates_org_unreadable = true,
        }
    }

    match locust_core::groups::load_groups(store) {
        Ok(listed) => {
            state.groups = listed.groups;
            state.members = listed.members;
            state.left = listed.left;
        }
        Err(_) => state.groups_unreadable = true,
    }
    state
}

fn now_iso() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    // Approximate RFC3339 UTC; sufficient for settle `at` and tests that check parseability.
    let secs = (ms / 1000) as i64;
    let millis = (ms % 1000) as u32;
    chrono::DateTime::from_timestamp(secs, millis * 1_000_000)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".into())
}

fn write_atomic(store: &Store, path: &Path, body: &str) -> io::Result<()> {
    let path = store.check(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid org path"))?;
    std::fs::create_dir_all(parent)?;
    // Refuse if create_dir somehow pointed at Electron/Codex (check the resolved parent).
    let _ = store.check(parent)?;
    let temporary = path.with_extension(format!(
        "{}.tmp",
        crate::invariants::unique_nanos()
    ));
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
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    std::fs::rename(&temporary, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })?;
    Ok(())
}

fn teammates_body(state: &OrgState) -> String {
    let titles = Value::Object(
        state
            .titles
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect(),
    );
    let settled = Value::Object(
        state
            .settled
            .iter()
            .map(|(k, v)| {
                let mut obj = serde_json::Map::new();
                obj.insert("at".into(), json!(v.at));
                if let Some(until) = &v.until {
                    obj.insert("until".into(), json!(until));
                }
                (k.clone(), Value::Object(obj))
            })
            .collect(),
    );
    serde_json::to_string_pretty(&json!({
        "schemaVersion": 1,
        "teammates": [],
        "missionOwners": {},
        "missionTitles": titles,
        "missionPins": state.pins,
        "missionSettled": settled,
        "settings": {}
    }))
    .unwrap_or_else(|_| "{\"schemaVersion\":1}\n".into())
        + "\n"
}

fn source_file(store: &Store, path: &Path, name: &str, max: u64) -> Result<Value, String> {
    let text = match read_capped(path, max)? {
        Some(text) => Some(text),
        None => read_capped(&store.electron().join(name), max)?,
    };
    match text {
        None => Ok(json!({"schemaVersion": 1})),
        Some(text) => {
            let value: Value = serde_json::from_str(&text).map_err(|_| "unreadable".to_owned())?;
            if !value.is_object() {
                return Err("unreadable".into());
            }
            Ok(value)
        }
    }
}

fn persist_teammates(store: &Store, state: &OrgState) -> Result<(), String> {
    if state.teammates_org_unreadable {
        return Err("Teammates could not be read".into());
    }
    let path = store.teammates_json_path().map_err(|e| e.to_string())?;
    let mut body = source_file(store, &path, "teammates.json", MAX_FILE_BYTES)?;
    let org: Value = serde_json::from_str(&teammates_body(state)).map_err(|e| e.to_string())?;
    for key in ["missionTitles", "missionPins", "missionSettled"] {
        body[key] = org[key].clone();
    }
    let text = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
    write_atomic(store, &path, &text).map_err(|e| e.to_string())
}

fn pin_mission_inner(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
    pinned: bool,
) -> Result<(), String> {
    if !safe_id(mission_id) {
        return Err("Mission id is invalid".into());
    }
    let others: Vec<String> = state
        .pins
        .iter()
        .filter(|id| id.as_str() != mission_id)
        .cloned()
        .collect();
    if !pinned {
        if others.len() == state.pins.len() {
            return Ok(());
        }
        state.pins = others;
        return persist_teammates(store, state);
    }
    if others.len() >= MAX_MISSION_PINS {
        return Err(format!(
            "At most {MAX_MISSION_PINS} conversations can be pinned"
        ));
    }
    state.pins = std::iter::once(mission_id.to_owned())
        .chain(others)
        .collect();
    persist_teammates(store, state)
}

fn rename_mission_inner(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
    title: &str,
) -> Result<(), String> {
    if !safe_id(mission_id) {
        return Err("Mission id is invalid".into());
    }
    let trimmed = utf16_prefix(title.trim(), MAX_MISSION_TITLE_LENGTH);
    if trimmed.is_empty() {
        if !state.titles.contains_key(mission_id) {
            return Ok(());
        }
        state.titles.remove(mission_id);
        return persist_teammates(store, state);
    }
    if state.titles.len() >= MAX_MISSION_TITLES && !state.titles.contains_key(mission_id) {
        return Err("Too many renamed conversations".into());
    }
    state.titles.insert(mission_id.to_owned(), trimmed);
    persist_teammates(store, state)
}

fn settle_mission_inner(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
    settled: Option<Option<String>>,
) -> Result<(), String> {
    if !safe_id(mission_id) {
        return Err("Mission id is invalid".into());
    }
    match settled {
        None => {
            if !state.settled.contains_key(mission_id) {
                return Ok(());
            }
            state.settled.remove(mission_id);
            persist_teammates(store, state)
        }
        Some(until) => {
            if let Some(ref until) = until
                && chrono::DateTime::parse_from_rfc3339(until).is_err()
            {
                return Err("Snooze time is invalid".into());
            }
            if !state.settled.contains_key(mission_id) && state.settled.len() >= MAX_MISSION_SETTLED
            {
                return Err("Too many settled conversations".into());
            }
            state.settled.insert(
                mission_id.to_owned(),
                SettledEntry {
                    at: now_iso(),
                    until,
                },
            );
            persist_teammates(store, state)
        }
    }
}

#[cfg(test)]
fn clear_org_for_deleted_inner(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
) -> Result<(), String> {
    if !safe_id(mission_id) {
        return Err("Mission id is invalid".into());
    }
    let had_pin = state.pins.iter().any(|id| id == mission_id);
    let had_settled = state.settled.contains_key(mission_id);
    let had_title = state.titles.contains_key(mission_id);
    if !had_pin && !had_settled && !had_title {
        return Ok(());
    }
    state.pins.retain(|id| id != mission_id);
    state.settled.remove(mission_id);
    state.titles.remove(mission_id);
    persist_teammates(store, state)
}

fn refresh_groups(store: &Store, state: &mut OrgState) -> Result<(), String> {
    let listed = locust_core::groups::load_groups(store)?;
    state.groups = listed.groups;
    state.members = listed.members;
    state.left = listed.left;
    state.groups_unreadable = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TICK: AtomicU64 = AtomicU64::new(0);

    pub(super) struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let n = TICK.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "locust-org-{}-{}-{}",
                std::process::id(),
                n,
                crate::invariants::unique_nanos()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(super) fn temp_store() -> (Temp, Store) {
        let dir = Temp::new();
        let electron = dir.path().join("electron");
        let native = dir.path().join("native");
        std::fs::create_dir_all(&electron).unwrap();
        std::fs::create_dir_all(&native).unwrap();
        let store = Store::new(native, electron).unwrap();
        (dir, store)
    }

    // main/a-conversation-can-be-pinned.test.ts
    #[test]
    fn a_conversation_can_be_pinned_persists() {
        let (_dir, store) = temp_store();
        let mut state = OrgState::default();
        pin_mission(&store, &mut state, "m_1", true).unwrap();
        pin_mission(&store, &mut state, "m_2", true).unwrap();
        pin_mission(&store, &mut state, "m_1", true).unwrap();
        assert_eq!(state.pins, vec!["m_1", "m_2"]);
        pin_mission(&store, &mut state, "m_2", false).unwrap();
        let again = load_org(&store);
        assert_eq!(again.pins, vec!["m_1".to_string()]);
    }

    #[test]
    fn pin_rejects_bad_id_and_caps_at_fifty() {
        let (_dir, store) = temp_store();
        let mut state = OrgState::default();
        assert!(pin_mission(&store, &mut state, "../escape", true).is_err());
        for at in 0..MAX_MISSION_PINS {
            pin_mission(&store, &mut state, &format!("m_{at}"), true).unwrap();
        }
        let err = pin_mission(&store, &mut state, "m_one_more", true).unwrap_err();
        assert!(err.contains("At most 50"));
        pin_mission(&store, &mut state, "m_0", false).unwrap();
        pin_mission(&store, &mut state, "m_one_more", true).unwrap();
        assert_eq!(state.pins[0], "m_one_more");
    }

    #[test]
    fn deleted_conversation_clears_pin_and_settled() {
        let (_dir, store) = temp_store();
        let mut state = OrgState::default();
        for at in 0..MAX_MISSION_PINS {
            pin_mission(&store, &mut state, &format!("m_{at}"), true).unwrap();
        }
        settle_mission(&store, &mut state, "m_3", Some(None)).unwrap();
        for at in 0..MAX_MISSION_PINS {
            clear_org_for_deleted(&store, &mut state, &format!("m_{at}")).unwrap();
        }
        assert!(state.pins.is_empty());
        assert!(state.settled.is_empty());
        pin_mission(&store, &mut state, "m_new", true).unwrap();
        assert_eq!(state.pins, vec!["m_new".to_string()]);
    }

    // main/a-conversation-can-be-put-away.test.ts
    #[test]
    fn a_conversation_can_be_put_away_persists() {
        let (_dir, store) = temp_store();
        let mut state = OrgState::default();
        settle_mission(&store, &mut state, "m_1", Some(None)).unwrap();
        settle_mission(
            &store,
            &mut state,
            "m_2",
            Some(Some("2026-10-11T13:00:00.000Z".into())),
        )
        .unwrap();
        let again = load_org(&store);
        let mut keys: Vec<_> = again.settled.keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, vec!["m_1", "m_2"]);
        assert!(again.settled["m_1"].until.is_none());
        assert_eq!(
            again.settled["m_2"].until.as_deref(),
            Some("2026-10-11T13:00:00.000Z")
        );
        assert!(chrono::DateTime::parse_from_rfc3339(&again.settled["m_1"].at).is_ok());
        settle_mission(&store, &mut state, "m_1", None).unwrap();
        assert_eq!(
            load_org(&store).settled.keys().collect::<Vec<_>>(),
            vec!["m_2"]
        );
        assert!(settle_mission(&store, &mut state, "../escape", Some(None)).is_err());
        assert!(
            settle_mission(&store, &mut state, "m_1", Some(Some("tomorrow-ish".into()))).is_err()
        );
    }

    #[test]
    fn electron_pins_show_until_native_writes() {
        let (dir, store) = temp_store();
        let electron_file = dir.path().join("electron").join("teammates.json");
        std::fs::write(
            &electron_file,
            r#"{"schemaVersion":1,"teammates":[],"missionOwners":{},"missionTitles":{"m_e":"From Electron"},"missionPins":["m_e"],"missionSettled":{},"settings":{}}"#,
        )
        .unwrap();
        let loaded = load_org(&store);
        assert_eq!(loaded.pins, vec!["m_e".to_string()]);
        assert_eq!(
            loaded.titles.get("m_e").map(String::as_str),
            Some("From Electron")
        );
        // Native write must not touch Electron's file.
        let before = std::fs::read(&electron_file).unwrap();
        let mut state = loaded;
        pin_mission(&store, &mut state, "m_n", true).unwrap();
        let after = std::fs::read(&electron_file).unwrap();
        assert_eq!(before, after);
        assert!(store.teammates_json_path().unwrap().is_file());
    }
}

pub fn pin_mission(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
    pinned: bool,
) -> Result<(), String> {
    let mut next = state.clone();
    pin_mission_inner(store, &mut next, mission_id, pinned)?;
    *state = next;
    Ok(())
}

pub fn rename_mission(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
    title: &str,
) -> Result<(), String> {
    let mut next = state.clone();
    rename_mission_inner(store, &mut next, mission_id, title)?;
    *state = next;
    Ok(())
}

pub fn settle_mission(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
    settled: Option<Option<String>>,
) -> Result<(), String> {
    let mut next = state.clone();
    settle_mission_inner(store, &mut next, mission_id, settled)?;
    *state = next;
    Ok(())
}

pub fn assign_to_group(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
    group_id: Option<&str>,
) -> Result<(), String> {
    if state.groups_unreadable {
        return Err("Projects could not be read".into());
    }
    locust_core::groups::assign_to_group(store, mission_id, group_id)?;
    refresh_groups(store, state)
}

pub fn create_group(store: &Store, state: &mut OrgState, name: &str) -> Result<String, String> {
    if state.groups_unreadable {
        return Err("Projects could not be read".into());
    }
    let group = locust_core::groups::create_group(store, name)?;
    refresh_groups(store, state)?;
    Ok(group.group_id)
}

pub fn rename_group(
    store: &Store,
    state: &mut OrgState,
    group_id: &str,
    name: &str,
) -> Result<(), String> {
    if state.groups_unreadable {
        return Err("Projects could not be read".into());
    }
    locust_core::groups::rename_group(store, group_id, name)?;
    refresh_groups(store, state)
}

pub fn remove_group(store: &Store, state: &mut OrgState, group_id: &str) -> Result<(), String> {
    if state.groups_unreadable {
        return Err("Projects could not be read".into());
    }
    locust_core::groups::remove_group(store, group_id)?;
    refresh_groups(store, state)
}

pub fn set_group_instructions(
    store: &Store,
    state: &mut OrgState,
    group_id: &str,
    instructions: &str,
) -> Result<(), String> {
    if state.groups_unreadable {
        return Err("Projects could not be read".into());
    }
    locust_core::groups::set_group_instructions(store, group_id, instructions)?;
    refresh_groups(store, state)
}

pub fn set_group_route(
    store: &Store,
    state: &mut OrgState,
    group_id: &str,
    route: Option<GroupRoute>,
) -> Result<(), String> {
    if state.groups_unreadable {
        return Err("Projects could not be read".into());
    }
    locust_core::groups::set_group_route(store, group_id, route)?;
    refresh_groups(store, state)
}

#[cfg(test)]
pub fn clear_org_for_deleted(
    store: &Store,
    state: &mut OrgState,
    mission_id: &str,
) -> Result<(), String> {
    let mut next = state.clone();
    clear_org_for_deleted_inner(store, &mut next, mission_id)?;
    *state = next;
    Ok(())
}

#[cfg(test)]
mod review_tests {
    use super::tests::temp_store;
    use super::*;

    #[test]
    fn clearing_imported_marks_survives_reload_and_preserves_roster() {
        let (dir, store) = temp_store();
        let source = json!({"schemaVersion":1,"teammates":[{"id":"codec"}],"settings":{"theme":"light"},"missionOwners":{"m":"codec"},"missionTitles":{"m":"Named"},"missionPins":["m"],"missionSettled":{"m":{"at":"2026-01-01T00:00:00Z"}}});
        let path = dir.path().join("electron/teammates.json");
        std::fs::write(&path, source.to_string()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut state = load_org(&store);
        rename_mission(&store, &mut state, "m", "").unwrap();
        settle_mission(&store, &mut state, "m", None).unwrap();
        pin_mission(&store, &mut state, "m", false).unwrap();
        let again = load_org(&store);
        assert!(again.titles.is_empty() && again.settled.is_empty() && again.pins.is_empty());
        let native: Value = serde_json::from_str(
            &std::fs::read_to_string(store.teammates_json_path().unwrap()).unwrap(),
        )
        .unwrap();
        for key in ["teammates", "settings", "missionOwners"] {
            assert_eq!(native[key], source[key]);
        }
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn unreadable_native_files_refuse_writes_without_mutating_state() {
        let (_dir, store) = temp_store();
        let path = store.teammates_json_path().unwrap();
        std::fs::write(&path, "{broken").unwrap();
        let mut state = load_org(&store);
        assert!(state.teammates_org_unreadable);
        let before = state.clone();
        assert!(pin_mission(&store, &mut state, "m", true).is_err());
        assert_eq!(state, before);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{broken");
        let path = store.groups_json_path().unwrap();
        std::fs::write(&path, "{broken").unwrap();
        let mut state = load_org(&store);
        let before = state.clone();
        assert!(create_group(&store, &mut state, "Project").is_err());
        assert_eq!(state, before);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{broken");
    }

    #[test]
    fn group_move_preserves_routes_moments_history_and_stays_ungrouped() {
        let (dir, store) = temp_store();
        let source = json!({"schemaVersion":1,"groups":[{"groupId":"g","name":"Project","createdAt":"old","instructions":"Be careful","route":{"runtime":"codex"}}],"members":{"m":{"groupId":"g","at":"joined"}},"left":{"other":[{"groupId":"old","name":"Past","instructions":"Old","until":"then"}]}});
        let path = dir.path().join("electron/groups.json");
        std::fs::write(&path, source.to_string()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut state = load_org(&store);
        rename_group(&store, &mut state, "g", "  New\n name  ").unwrap();
        let read = || -> Value {
            serde_json::from_str(
                &std::fs::read_to_string(store.groups_json_path().unwrap()).unwrap(),
            )
            .unwrap()
        };
        assert_eq!(read()["members"]["m"]["at"], "joined");
        assert_eq!(read()["groups"][0]["route"], source["groups"][0]["route"]);
        assign_to_group(&store, &mut state, "m", None).unwrap();
        assert!(load_org(&store).members.is_empty());
        let native = read();
        assert_eq!(native["left"]["m"][0]["name"], "New name");
        assert_eq!(native["left"]["m"][0]["instructions"], "Be careful");
        assert_eq!(native["left"]["m"][0]["at"], "joined");
        assert_eq!(native["left"]["other"], source["left"]["other"]);
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn names_bound_utf16_and_ids_and_refusals_match_electron() {
        let (_dir, store) = temp_store();
        let mut state = OrgState::default();
        let id = "a".repeat(128);
        rename_mission(&store, &mut state, &id, &"😀".repeat(100)).unwrap();
        assert_eq!(load_org(&store).titles[&id].encode_utf16().count(), 120);
        assert_eq!(
            pin_mission(&store, &mut state, "../bad", true).unwrap_err(),
            "Mission id is invalid"
        );
        state.pins = (0..50).map(|i| format!("m{i}")).collect();
        assert_eq!(
            pin_mission(&store, &mut state, "extra", true).unwrap_err(),
            "At most 50 conversations can be pinned"
        );
        let listed =
            locust_core::groups::parse_groups_str(r#"{"schemaVersion":1,"groups":[],"members":{"m":"gone"}}"#).unwrap();
        assert!(listed.members.is_empty());
    }
}

/// Toggle once for a conversation, clearing pins saved under older turn identities.
pub fn toggle_conversation_pin(
    store: &Store,
    state: &mut OrgState,
    root: &str,
    keys: &[String],
) -> Result<(), String> {
    let mut next = state.clone();
    let pinned = next.pins.iter().any(|key| keys.contains(key));
    next.pins.retain(|key| !keys.contains(key));
    if !pinned {
        return pin_mission(store, state, root, true);
    }
    if next.pins.len() > MAX_MISSION_PINS {
        return Err("At most 50 conversations can be pinned".into());
    }
    persist_teammates(store, &next)?;
    *state = next;
    Ok(())
}

pub fn toggle_conversation_settled(
    store: &Store,
    state: &mut OrgState,
    root: &str,
    keys: &[String],
) -> Result<(), String> {
    let mut next = state.clone();
    let settled = keys.iter().any(|key| next.settled.contains_key(key));
    for key in keys {
        next.settled.remove(key);
    }
    if !settled {
        return settle_mission(store, state, root, Some(None));
    }
    persist_teammates(store, &next)?;
    *state = next;
    Ok(())
}

pub fn rename_conversation(
    store: &Store,
    state: &mut OrgState,
    root: &str,
    keys: &[String],
    title: &str,
) -> Result<(), String> {
    if keys.iter().all(|key| key == root) {
        return rename_mission(store, state, root, title);
    }
    if !safe_id(root) {
        return Err("Mission id is invalid".into());
    }
    let mut next = state.clone();
    for key in keys {
        next.titles.remove(key);
    }
    let title = utf16_prefix(title.trim(), MAX_MISSION_TITLE_LENGTH);
    if !title.is_empty() {
        if next.titles.len() >= MAX_MISSION_TITLES {
            return Err("Too many renamed conversations".into());
        }
        next.titles.insert(root.into(), title);
    }
    persist_teammates(store, &next)?;
    *state = next;
    Ok(())
}

pub fn assign_conversation(
    store: &Store,
    state: &mut OrgState,
    root: &str,
    keys: &[String],
    group: Option<&str>,
) -> Result<(), String> {
    if state.groups_unreadable {
        return Err("Projects could not be read".into());
    }
    locust_core::groups::assign_conversation(store, root, keys, group)?;
    refresh_groups(store, state)
}

#[cfg(test)]
mod identity_review_tests {
    use super::*;
    #[test]
    fn clearing_names_groups_pins_and_settled_marks_clears_old_turn_keys() {
        let (_dir, store) = super::tests::temp_store();
        let mut state = OrgState::default();
        let keys = vec!["root".into(), "leaf".into()];
        rename_mission(&store, &mut state, "leaf", "Saved on a turn").unwrap();
        pin_mission(&store, &mut state, "leaf", true).unwrap();
        settle_mission(&store, &mut state, "leaf", Some(None)).unwrap();
        let group = create_group(&store, &mut state, "Project").unwrap();
        assign_to_group(&store, &mut state, "leaf", Some(&group)).unwrap();
        rename_conversation(&store, &mut state, "root", &keys, "").unwrap();
        toggle_conversation_pin(&store, &mut state, "root", &keys).unwrap();
        toggle_conversation_settled(&store, &mut state, "root", &keys).unwrap();
        assign_conversation(&store, &mut state, "root", &keys, None).unwrap();
        let again = load_org(&store);
        assert!(
            again.titles.is_empty()
                && again.pins.is_empty()
                && again.settled.is_empty()
                && again.members.is_empty()
        );
    }
}
