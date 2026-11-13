//! Conversation groups (projects), their standing instructions, default routes,
//! leave history, chain lookup, and thread boundary placement.
//!
//! Port of `apps/desktop/src/main/group-store.ts`, `apps/desktop/src/main/conversation-chain.ts`,
//! `groupSection` in `workspace-brief.ts`, and `groupBoundary`/`groupJoins`/`groupLeavings` in
//! `apps/desktop/src/renderer/src/missionView.ts`.

use crate::store::Store;
use chrono::DateTime;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const GROUPS_UNREADABLE: &str = "GROUPS_UNREADABLE";
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_GROUPS: usize = 64;
pub const MAX_GROUP_NAME_LENGTH: usize = 60;
pub const MAX_GROUP_MEMBERS: usize = 5000;
pub const MAX_GROUP_INSTRUCTIONS_LENGTH: usize = 4000;
pub const MAX_LEFT_PER_CONVERSATION: usize = 8;
pub const MAX_CHAIN_HOPS: usize = 64;

const VALID_MODES: [&str; 5] = ["ask", "accept-edits", "approve-each", "plan", "auto"];
const VALID_EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
const HEADER_WINDOW: u64 = 128 * 1024;

static WRITES: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRoute {
    pub runtime: String,
    pub model: String,
    pub mode: String,
    pub account: Option<String>,
    pub effort: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub group_id: String,
    pub name: String,
    pub created_at: String,
    pub instructions: String,
    pub route: Option<GroupRoute>,
    /// Preserves partial/unrecognized legacy route objects read from disk when route was not edited.
    raw_route: Option<Value>,
}

impl Group {
    pub fn new(group_id: String, name: String, created_at: String, instructions: String, route: Option<GroupRoute>) -> Self {
        Self {
            group_id,
            name,
            created_at,
            instructions,
            route,
            raw_route: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupMembership {
    pub group_id: String,
    pub at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftMembership {
    pub group_id: String,
    pub name: String,
    pub instructions: String,
    pub at: Option<String>,
    pub until: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListedGroups {
    pub groups: Vec<Group>,
    pub members: HashMap<String, GroupMembership>,
    pub left: HashMap<String, Vec<LeftMembership>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupBrief {
    pub group_id: String,
    pub group_name: String,
    pub instructions: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupBoundary {
    pub group_name: String,
    pub instructions: String,
    pub before_turn: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupLeaving {
    pub group_name: String,
    pub before_turn: usize,
}

pub fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn utf16_prefix(value: &str, max: usize) -> String {
    let mut count = 0;
    value
        .chars()
        .take_while(|c| {
            count += c.len_utf16();
            count <= max
        })
        .collect()
}

pub fn clean_group_name(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    utf16_prefix(&collapsed, MAX_GROUP_NAME_LENGTH)
}

pub fn is_valid_route(route: &GroupRoute) -> bool {
    !route.runtime.trim().is_empty()
        && !route.model.trim().is_empty()
        && VALID_MODES.contains(&route.mode.as_str())
        && route
            .account
            .as_deref()
            .is_none_or(|a| !a.trim().is_empty())
        && route
            .effort
            .as_deref()
            .is_none_or(|e| VALID_EFFORTS.contains(&e))
}

fn parse_route_value(v: &Value) -> Option<GroupRoute> {
    let obj = v.as_object()?;
    let runtime = obj.get("runtime")?.as_str()?.trim();
    let model = obj.get("model")?.as_str()?.trim();
    let mode = obj.get("mode")?.as_str()?;
    let account = match obj.get("account") {
        None | Some(Value::Null) => None,
        Some(a) => {
            let s = a.as_str()?.trim();
            if s.is_empty() {
                return None;
            }
            Some(s.to_owned())
        }
    };
    let effort = match obj.get("effort") {
        None | Some(Value::Null) => None,
        Some(e) => {
            let s = e.as_str()?;
            if !VALID_EFFORTS.contains(&s) {
                return None;
            }
            Some(s.to_owned())
        }
    };
    let route = GroupRoute {
        runtime: runtime.to_owned(),
        model: model.to_owned(),
        mode: mode.to_owned(),
        account,
        effort,
    };
    is_valid_route(&route).then_some(route)
}

fn route_to_json(route: &GroupRoute) -> Value {
    let mut obj = Map::new();
    obj.insert("runtime".into(), json!(route.runtime));
    obj.insert("model".into(), json!(route.model));
    obj.insert("mode".into(), json!(route.mode));
    if let Some(account) = &route.account {
        obj.insert("account".into(), json!(account));
    }
    if let Some(effort) = &route.effort {
        obj.insert("effort".into(), json!(effort));
    }
    Value::Object(obj)
}

fn read_capped(path: &Path, max: u64) -> Result<Option<String>, String> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(GROUPS_UNREADABLE.into()),
    };
    let mut text = String::new();
    file.take(max + 1)
        .read_to_string(&mut text)
        .map_err(|_| GROUPS_UNREADABLE.to_string())?;
    if text.len() as u64 > max {
        return Err(GROUPS_UNREADABLE.into());
    }
    Ok(Some(text))
}

fn parse_groups_value(file: &Value) -> Result<ListedGroups, String> {
    if !file.is_object()
        || file
            .get("schemaVersion")
            .is_some_and(|v| v != &json!(1))
    {
        return Err(GROUPS_UNREADABLE.into());
    }
    let mut groups = Vec::new();
    for entry in file
        .get("groups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if groups.len() >= MAX_GROUPS {
            break;
        }
        let Some(group_id) = entry
            .get("groupId")
            .and_then(Value::as_str)
            .filter(|s| safe_id(s))
        else {
            continue;
        };
        if groups.iter().any(|g: &Group| g.group_id == group_id) {
            continue;
        }
        let Some(raw_name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        let name = clean_group_name(raw_name);
        if name.is_empty() {
            continue;
        }
        let created_at = entry
            .get("createdAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let instructions = entry
            .get("instructions")
            .and_then(Value::as_str)
            .map(|s| utf16_prefix(s.trim(), MAX_GROUP_INSTRUCTIONS_LENGTH))
            .unwrap_or_default();
        let raw_route_field = entry.get("route").filter(|v| !v.is_null());
        let route = raw_route_field.and_then(parse_route_value);
        let raw_route = if route.is_none() {
            raw_route_field.cloned()
        } else {
            None
        };
        groups.push(Group {
            group_id: group_id.to_owned(),
            name,
            created_at,
            instructions,
            route,
            raw_route,
        });
    }

    let mut members = HashMap::new();
    if let Some(obj) = file.get("members").and_then(Value::as_object) {
        for (mission_id, entry) in obj {
            if members.len() >= MAX_GROUP_MEMBERS {
                break;
            }
            if !safe_id(mission_id) {
                continue;
            }
            let (group_id, at) = if let Some(s) = entry.as_str() {
                (s, None)
            } else if let Some(eobj) = entry.as_object() {
                let gid = eobj.get("groupId").and_then(Value::as_str).unwrap_or("");
                let at = eobj
                    .get("at")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                (gid, at)
            } else {
                continue;
            };
            if safe_id(group_id) && groups.iter().any(|g| g.group_id == group_id) {
                members.insert(
                    mission_id.clone(),
                    GroupMembership {
                        group_id: group_id.to_owned(),
                        at,
                    },
                );
            }
        }
    }

    let mut left = HashMap::new();
    if let Some(obj) = file.get("left").and_then(Value::as_object) {
        for (mission_id, entries) in obj {
            if !safe_id(mission_id) {
                continue;
            }
            let Some(arr) = entries.as_array() else {
                continue;
            };
            let mut valid = Vec::new();
            for item in arr {
                let Some(iobj) = item.as_object() else {
                    continue;
                };
                let Some(group_id) = iobj
                    .get("groupId")
                    .and_then(Value::as_str)
                    .filter(|s| safe_id(s))
                else {
                    continue;
                };
                let Some(raw_name) = iobj.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let name = clean_group_name(raw_name);
                if name.is_empty() {
                    continue;
                }
                let Some(until) = iobj
                    .get("until")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                else {
                    continue;
                };
                let instructions = iobj
                    .get("instructions")
                    .and_then(Value::as_str)
                    .map(|s| utf16_prefix(s.trim(), MAX_GROUP_INSTRUCTIONS_LENGTH))
                    .unwrap_or_default();
                let at = iobj
                    .get("at")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                valid.push(LeftMembership {
                    group_id: group_id.to_owned(),
                    name,
                    instructions,
                    at,
                    until: until.to_owned(),
                });
            }
            if !valid.is_empty() {
                if valid.len() > MAX_LEFT_PER_CONVERSATION {
                    let drop = valid.len() - MAX_LEFT_PER_CONVERSATION;
                    valid.drain(0..drop);
                }
                left.insert(mission_id.clone(), valid);
            }
        }
    }

    Ok(ListedGroups {
        groups,
        members,
        left,
    })
}

pub fn parse_groups_str(text: &str) -> Result<ListedGroups, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| GROUPS_UNREADABLE.to_string())?;
    parse_groups_value(&value)
}

fn read_source_raw(store: &Store) -> Result<(Option<Value>, ListedGroups), String> {
    let native_path = store.groups_json_path().map_err(|_| GROUPS_UNREADABLE.to_string())?;
    if let Some(text) = read_capped(&native_path, MAX_FILE_BYTES)? {
        let raw: Value = serde_json::from_str(&text).map_err(|_| GROUPS_UNREADABLE.to_string())?;
        let parsed = parse_groups_value(&raw)?;
        return Ok((Some(raw), parsed));
    }
    let electron_path = store.electron().join("groups.json");
    if let Some(text) = read_capped(&electron_path, MAX_FILE_BYTES)? {
        let raw: Value = serde_json::from_str(&text).map_err(|_| GROUPS_UNREADABLE.to_string())?;
        let parsed = parse_groups_value(&raw)?;
        return Ok((Some(raw), parsed));
    }
    Ok((None, ListedGroups::default()))
}

/// Read groups from Electron's read-only store as base, or native's `groups.json` once created.
pub fn load_groups(store: &Store) -> Result<ListedGroups, String> {
    let electron_path = store.electron().join("groups.json");
    let mut result: Result<ListedGroups, String> = Ok(ListedGroups::default());
    match read_capped(&electron_path, MAX_FILE_BYTES) {
        Ok(Some(text)) => result = parse_groups_str(&text),
        Ok(None) => {}
        Err(e) => result = Err(e),
    }
    if let Ok(native_path) = store.groups_json_path() {
        match read_capped(&native_path, MAX_FILE_BYTES) {
            Ok(Some(text)) => result = parse_groups_str(&text),
            Ok(None) => {}
            Err(e) => result = Err(e),
        }
    }
    result
}

fn now_iso() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let secs = (ms / 1000) as i64;
    let millis = (ms % 1000) as u32;
    DateTime::from_timestamp(secs, millis * 1_000_000)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".into())
}

fn new_group_id() -> Result<String, String> {
    let u = crate::native::uuid_v4().map_err(|e| e.to_string())?;
    Ok(format!("grp_{}", u.replace('-', "").chars().take(12).collect::<String>()))
}

pub fn listed_to_value(listed: &ListedGroups, base: Option<&Value>) -> Value {
    let mut root = base
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let old_groups = root
        .get("groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    root.insert("schemaVersion".into(), json!(1));

    let groups_val: Vec<Value> = listed
        .groups
        .iter()
        .map(|g| {
            let mut obj = old_groups
                .iter()
                .find(|old| old.get("groupId").and_then(Value::as_str) == Some(g.group_id.as_str()))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            obj.insert("groupId".into(), json!(g.group_id));
            obj.insert("name".into(), json!(g.name));
            obj.insert("createdAt".into(), json!(g.created_at));
            obj.insert("instructions".into(), json!(g.instructions));
            if let Some(route) = &g.route {
                obj.insert("route".into(), route_to_json(route));
            } else if let Some(raw_route) = &g.raw_route {
                obj.insert("route".into(), raw_route.clone());
            } else {
                obj.remove("route");
            }
            Value::Object(obj)
        })
        .collect();
    root.insert("groups".into(), Value::Array(groups_val));

    let mut members_obj = Map::new();
    for (mission_id, mem) in &listed.members {
        let mut entry = Map::new();
        entry.insert("groupId".into(), json!(mem.group_id));
        if let Some(at) = &mem.at {
            entry.insert("at".into(), json!(at));
        }
        members_obj.insert(mission_id.clone(), Value::Object(entry));
    }
    root.insert("members".into(), Value::Object(members_obj));

    let mut left_obj = Map::new();
    for (mission_id, entries) in &listed.left {
        let arr: Vec<Value> = entries
            .iter()
            .map(|e| {
                let mut entry = Map::new();
                entry.insert("groupId".into(), json!(e.group_id));
                entry.insert("name".into(), json!(e.name));
                entry.insert("instructions".into(), json!(e.instructions));
                if let Some(at) = &e.at {
                    entry.insert("at".into(), json!(at));
                }
                entry.insert("until".into(), json!(e.until));
                Value::Object(entry)
            })
            .collect();
        left_obj.insert(mission_id.clone(), Value::Array(arr));
    }
    root.insert("left".into(), Value::Object(left_obj));
    Value::Object(root)
}

fn write_groups(store: &Store, listed: &ListedGroups, base: Option<&Value>) -> Result<(), String> {
    let path = store.groups_json_path().map_err(|e| e.to_string())?;
    let body = serde_json::to_string_pretty(&listed_to_value(listed, base)).map_err(|e| e.to_string())?
        + "\n";
    let dir = path.parent().ok_or("Missing store root")?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Missing file name")?;
    let tmp = format!(
        "{name}.{}.tmp",
        crate::native::uuid_v4().map_err(|e| e.to_string())?
    );
    let mut file = store.create_new_file_in(dir, &tmp).map_err(|e| e.to_string())?;
    let res = (|| {
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        store.check(&path)?;
        std::fs::rename(dir.join(&tmp), &path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(dir.join(tmp));
    }
    res.map_err(|e| e.to_string())
}

fn with_leave(
    listed: &mut ListedGroups,
    mission_id: &str,
    left_group: &Group,
    at: Option<&str>,
    until: &str,
) {
    let list = listed.left.entry(mission_id.to_owned()).or_default();
    list.push(LeftMembership {
        group_id: left_group.group_id.clone(),
        name: left_group.name.clone(),
        instructions: left_group.instructions.clone(),
        at: at.map(str::to_owned),
        until: until.to_owned(),
    });
    if list.len() > MAX_LEFT_PER_CONVERSATION {
        let drop = list.len() - MAX_LEFT_PER_CONVERSATION;
        list.drain(0..drop);
    }
}

pub fn create_group(store: &Store, name: &str) -> Result<Group, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let (raw, mut listed) = read_source_raw(store)?;
    let cleaned = clean_group_name(name);
    if cleaned.is_empty() {
        return Err("A group needs a name".into());
    }
    if listed.groups.len() >= MAX_GROUPS {
        return Err("Too many groups".into());
    }
    let group = Group {
        group_id: new_group_id()?,
        name: cleaned,
        created_at: now_iso(),
        instructions: String::new(),
        route: None,
        raw_route: None,
    };
    listed.groups.push(group.clone());
    write_groups(store, &listed, raw.as_ref())?;
    Ok(group)
}

pub fn rename_group(store: &Store, group_id: &str, name: &str) -> Result<Group, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !safe_id(group_id) {
        return Err("Group id is invalid".into());
    }
    let cleaned = clean_group_name(name);
    if cleaned.is_empty() {
        return Err("A group needs a name".into());
    }
    let (raw, mut listed) = read_source_raw(store)?;
    let Some(pos) = listed.groups.iter().position(|g| g.group_id == group_id) else {
        return Err("That group does not exist".into());
    };
    listed.groups[pos].name = cleaned;
    let updated = listed.groups[pos].clone();
    write_groups(store, &listed, raw.as_ref())?;
    Ok(updated)
}

pub fn remove_group(store: &Store, group_id: &str) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !safe_id(group_id) {
        return Err("Group id is invalid".into());
    }
    let (raw, mut listed) = read_source_raw(store)?;
    let Some(pos) = listed.groups.iter().position(|g| g.group_id == group_id) else {
        return Err("That group does not exist".into());
    };
    let removed_group = listed.groups.remove(pos);
    let until = now_iso();
    let affected: Vec<(String, GroupMembership)> = listed
        .members
        .iter()
        .filter(|(_, m)| m.group_id == group_id)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (mission_id, mem) in affected {
        listed.members.remove(&mission_id);
        with_leave(&mut listed, &mission_id, &removed_group, mem.at.as_deref(), &until);
    }
    write_groups(store, &listed, raw.as_ref())
}

pub fn assign_to_group(
    store: &Store,
    mission_id: &str,
    group_id: Option<&str>,
) -> Result<(), String> {
    assign_conversation(store, mission_id, &[mission_id.to_owned()], group_id)
}

/// Assign a conversation (`root` and any prior turn keys `keys`) to `group_id` (`None` leaves).
/// Records a leave entry under `root` when moving to another group or leaving, and keeps the
/// existing `at` without recording a leave when already in `group_id`.
pub fn assign_conversation(
    store: &Store,
    root: &str,
    keys: &[String],
    group_id: Option<&str>,
) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !safe_id(root) {
        return Err("Conversation id is invalid".into());
    }
    if let Some(gid) = group_id
        && !safe_id(gid)
    {
        return Err("Group id is invalid".into());
    }
    let (raw, mut listed) = read_source_raw(store)?;
    let prev_mem = listed.members.get(root).cloned().or_else(|| {
        keys.iter()
            .find_map(|k| listed.members.get(k).cloned())
    });
    let had_other_key = keys.iter().any(|k| k != root && listed.members.contains_key(k));
    let until = now_iso();
    match group_id {
        None => {
            listed.members.remove(root);
            for k in keys {
                listed.members.remove(k);
            }
            if let Some(prev) = prev_mem
                && let Some(prev_group) = listed
                    .groups
                    .iter()
                    .find(|g| g.group_id == prev.group_id)
                    .cloned()
            {
                with_leave(&mut listed, root, &prev_group, prev.at.as_deref(), &until);
            }
        }
        Some(gid) => {
            if !listed.groups.iter().any(|g| g.group_id == gid) {
                return Err("That group does not exist".into());
            }
            if prev_mem.as_ref().is_some_and(|m| m.group_id == gid) {
                if !had_other_key && listed.members.contains_key(root) {
                    return Ok(());
                }
                for k in keys {
                    if k != root {
                        listed.members.remove(k);
                    }
                }
                if let Some(prev) = prev_mem {
                    listed.members.insert(root.to_owned(), prev);
                }
                return write_groups(store, &listed, raw.as_ref());
            }
            if prev_mem.is_none() && listed.members.len() >= MAX_GROUP_MEMBERS {
                return Err("Too many conversations in groups".into());
            }
            listed.members.remove(root);
            for k in keys {
                listed.members.remove(k);
            }
            if let Some(prev) = prev_mem
                && let Some(prev_group) = listed
                    .groups
                    .iter()
                    .find(|g| g.group_id == prev.group_id)
                    .cloned()
            {
                with_leave(&mut listed, root, &prev_group, prev.at.as_deref(), &until);
            }
            listed.members.insert(
                root.to_owned(),
                GroupMembership {
                    group_id: gid.to_owned(),
                    at: Some(until),
                },
            );
        }
    }
    write_groups(store, &listed, raw.as_ref())
}

pub fn set_group_route(
    store: &Store,
    group_id: &str,
    route: Option<GroupRoute>,
) -> Result<Group, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !safe_id(group_id) {
        return Err("Group id is invalid".into());
    }
    if let Some(r) = &route
        && !is_valid_route(r)
    {
        return Err("That route is not one a mission can start on".into());
    }
    let (raw, mut listed) = read_source_raw(store)?;
    let Some(group) = listed.groups.iter_mut().find(|g| g.group_id == group_id) else {
        return Err("That group does not exist".into());
    };
    group.route = route.map(|r| GroupRoute {
        runtime: r.runtime.trim().to_owned(),
        model: r.model.trim().to_owned(),
        mode: r.mode,
        account: r.account.map(|a| a.trim().to_owned()),
        effort: r.effort,
    });
    group.raw_route = None;
    let updated = group.clone();
    write_groups(store, &listed, raw.as_ref())?;
    Ok(updated)
}

pub fn set_group_instructions(
    store: &Store,
    group_id: &str,
    instructions: &str,
) -> Result<Group, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !safe_id(group_id) {
        return Err("Group id is invalid".into());
    }
    let trimmed = utf16_prefix(instructions.trim(), MAX_GROUP_INSTRUCTIONS_LENGTH);
    if trimmed.contains('\0') {
        return Err("Standing instructions cannot contain null bytes".into());
    }
    let (raw, mut listed) = read_source_raw(store)?;
    let Some(group) = listed.groups.iter_mut().find(|g| g.group_id == group_id) else {
        return Err("That group does not exist".into());
    };
    if let Some(kind) = crate::secrets::secret_in(&trimmed) {
        return Err(crate::secrets::instructions_secret_refusal(&group.name, kind));
    }
    group.instructions = trimmed;
    let updated = group.clone();
    write_groups(store, &listed, raw.as_ref())?;
    Ok(updated)
}

/// The prompt block injected into a turn when its conversation belongs to a group with standing instructions.
pub fn group_section(group_name: &str, instructions: &str) -> Option<String> {
    let trimmed = instructions.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(format!(
        "Standing instructions for the group \"{group_name}\", which this conversation is in. Every turn of it is given these:\n{trimmed}"
    ))
}

fn read_continues_from(store: &Store, mission_id: &str) -> Option<String> {
    if !safe_id(mission_id) {
        return None;
    }
    let name = format!("{mission_id}.jsonl");
    let candidates = [
        store.ledger_dir().ok().map(|d| d.join(&name)),
        Some(store.electron().join("ledger").join(&name)),
    ];
    for path in candidates.into_iter().flatten() {
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut head = String::new();
        if file.take(HEADER_WINDOW).read_to_string(&mut head).is_err() {
            continue;
        }
        let first_line = head.lines().next().unwrap_or("");
        if let Ok(record) = serde_json::from_str::<Value>(first_line)
            && let Some(prev) = record["continuesFrom"]["missionId"].as_str()
            && !prev.is_empty()
        {
            return Some(prev.to_owned());
        }
        // Even if `continuesFrom` is absent, finding the file means we should not scan further.
        return None;
    }
    None
}

/// Every `missionId` in a conversation ending at `tip`, newest first, walking `continuesFrom.missionId`
/// across both native and Electron read-only ledgers up to `MAX_CHAIN_HOPS`.
pub fn keys_before(store: &Store, tip: Option<&str>) -> Vec<String> {
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    let mut next = tip.filter(|id| !id.is_empty()).map(str::to_owned);
    while let Some(id) = next.take() {
        if !seen.insert(id.clone()) || keys.len() >= MAX_CHAIN_HOPS {
            break;
        }
        keys.push(id.clone());
        next = read_continues_from(store, &id);
    }
    keys
}

/// The group instructions a conversation's next turn should be briefed with, given its chain of keys.
pub fn group_brief_for(keys: &[String], listed: &ListedGroups) -> Option<GroupBrief> {
    for key in keys {
        if let Some(membership) = listed.members.get(key)
            && let Some(group) = listed.groups.iter().find(|g| g.group_id == membership.group_id)
        {
            let trimmed = group.instructions.trim();
            if trimmed.is_empty() {
                return None;
            }
            return Some(GroupBrief {
                group_id: group.group_id.clone(),
                group_name: group.name.clone(),
                instructions: trimmed.to_owned(),
            });
        }
    }
    None
}

/// Convenience helper used by turns and `/setup`: resolves `keys_before` + `load_groups`.
/// An unreadable `groups.json` never blocks a turn or `/setup` (returns `None`, matching Electron).
pub fn group_brief_for_tip(store: &Store, tip: Option<&str>) -> Option<GroupBrief> {
    let keys = keys_before(store, tip);
    if keys.is_empty() {
        return None;
    }
    let listed = load_groups(store).ok()?;
    group_brief_for(&keys, &listed)
}

fn parse_ms(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// Which turn in `turn_starts` first runs under a membership joined at `at`.
/// Returns `None` when `at` or `group.instructions` is absent/blank or `at` cannot be parsed.
pub fn group_boundary(
    turn_starts: &[Option<&str>],
    membership_at: Option<&str>,
    group_name: &str,
    instructions: &str,
) -> Option<GroupBoundary> {
    let joined_at = membership_at.and_then(parse_ms)?;
    let trimmed = instructions.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut before_turn = turn_starts.len();
    for (index, started) in turn_starts.iter().enumerate() {
        let Some(ms) = started.and_then(parse_ms) else {
            continue;
        };
        if ms >= joined_at {
            before_turn = index;
            break;
        }
    }
    Some(GroupBoundary {
        group_name: group_name.to_owned(),
        instructions: trimmed.to_owned(),
        before_turn,
    })
}

/// Turn boundaries where past (left) memberships STARTED briefing this conversation, oldest first.
pub fn group_joins(turn_starts: &[Option<&str>], left: &[LeftMembership]) -> Vec<GroupBoundary> {
    let mut boundaries = Vec::new();
    for entry in left {
        if let Some(boundary) =
            group_boundary(turn_starts, entry.at.as_deref(), &entry.name, &entry.instructions)
        {
            boundaries.push(boundary);
        }
    }
    boundaries
}

/// Turn boundaries where past (left) memberships STOPPED briefing this conversation.
pub fn group_leavings(turn_starts: &[Option<&str>], left: &[LeftMembership]) -> Vec<GroupLeaving> {
    let mut out = Vec::new();
    for entry in left {
        if entry.instructions.trim().is_empty() {
            continue;
        }
        let Some(until_ms) = parse_ms(&entry.until) else {
            continue;
        };
        let mut before_turn = turn_starts.len();
        for (index, started) in turn_starts.iter().enumerate() {
            let Some(ms) = started.and_then(parse_ms) else {
                continue;
            };
            if ms >= until_ms {
                before_turn = index;
                break;
            }
        }
        out.push(GroupLeaving {
            group_name: entry.name.clone(),
            before_turn,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TICK: AtomicU64 = AtomicU64::new(0);

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let n = TICK.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!(
                "locust-groups-{}-{}",
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_store() -> (TempDir, Store) {
        let dir = TempDir::new();
        let electron = dir.path().join("electron");
        let native = dir.path().join("native");
        std::fs::create_dir_all(&electron).unwrap();
        std::fs::create_dir_all(&native).unwrap();
        let store = Store::new(native, electron).unwrap();
        (dir, store)
    }

    fn t(hour: u32) -> String {
        format!("2026-09-16T{hour:02}:00:00.000Z")
    }

    // --- Ported from main/groups-hold-conversations.test.ts (35 it cases) ---

    #[test]
    fn lists_empty_when_file_has_not_been_written() {
        let (_d, store) = temp_store();
        let listed = load_groups(&store).unwrap();
        assert!(listed.groups.is_empty());
        assert!(listed.members.is_empty());
    }

    #[test]
    fn creates_group_trims_name_and_lists_back() {
        let (_d, store) = temp_store();
        let created = create_group(&store, "  Bug   Hunt  ").unwrap();
        assert_eq!(created.name, "Bug Hunt");
        assert!(created.group_id.starts_with("grp_"));
        assert!(!created.created_at.is_empty());
        assert_eq!(created.instructions, "");
        let listed = load_groups(&store).unwrap();
        assert_eq!(listed.groups.len(), 1);
        assert_eq!(listed.groups[0].group_id, created.group_id);
        assert_eq!(listed.groups[0].name, "Bug Hunt");
    }

    #[test]
    fn refuses_empty_or_whitespace_name_and_caps_name_length() {
        let (_d, store) = temp_store();
        assert_eq!(create_group(&store, "").unwrap_err(), "A group needs a name");
        assert_eq!(create_group(&store, "   \n\t ").unwrap_err(), "A group needs a name");
        let long = "a".repeat(80);
        let created = create_group(&store, &long).unwrap();
        assert_eq!(created.name.encode_utf16().count(), MAX_GROUP_NAME_LENGTH);
    }

    #[test]
    fn refuses_creating_more_than_max_groups() {
        let (_d, store) = temp_store();
        for i in 0..MAX_GROUPS {
            create_group(&store, &format!("Group {i}")).unwrap();
        }
        assert_eq!(create_group(&store, "One Too Many").unwrap_err(), "Too many groups");
    }

    #[test]
    fn renames_group_and_rejects_bad_names_and_unknown_ids() {
        let (_d, store) = temp_store();
        let g = create_group(&store, "Before").unwrap();
        let renamed = rename_group(&store, &g.group_id, "  After  ").unwrap();
        assert_eq!(renamed.name, "After");
        assert_eq!(load_groups(&store).unwrap().groups[0].name, "After");
        assert_eq!(rename_group(&store, &g.group_id, "   ").unwrap_err(), "A group needs a name");
        assert_eq!(rename_group(&store, "grp_missing", "Nope").unwrap_err(), "That group does not exist");
        assert_eq!(rename_group(&store, "../bad", "Nope").unwrap_err(), "Group id is invalid");
    }

    #[test]
    fn assigns_moves_and_unassigns_conversations() {
        let (_d, store) = temp_store();
        let a = create_group(&store, "Alpha").unwrap();
        let b = create_group(&store, "Beta").unwrap();
        assign_to_group(&store, "m_1", Some(&a.group_id)).unwrap();
        assert_eq!(load_groups(&store).unwrap().members["m_1"].group_id, a.group_id);
        assign_to_group(&store, "m_1", Some(&b.group_id)).unwrap();
        assert_eq!(load_groups(&store).unwrap().members["m_1"].group_id, b.group_id);
        assign_to_group(&store, "m_1", None).unwrap();
        assert!(!load_groups(&store).unwrap().members.contains_key("m_1"));
    }

    #[test]
    fn rejects_invalid_mission_id_or_unknown_group_on_assign() {
        let (_d, store) = temp_store();
        let a = create_group(&store, "Alpha").unwrap();
        assert_eq!(
            assign_to_group(&store, "../escape", Some(&a.group_id)).unwrap_err(),
            "Conversation id is invalid"
        );
        assert_eq!(
            assign_to_group(&store, "m_1", Some("grp_none")).unwrap_err(),
            "That group does not exist"
        );
        assert_eq!(
            assign_to_group(&store, "m_1", Some("bad/id")).unwrap_err(),
            "Group id is invalid"
        );
    }

    #[test]
    fn enforces_max_group_members_except_moving_existing_member() {
        let (_d, store) = temp_store();
        let a = create_group(&store, "Alpha").unwrap();
        let b = create_group(&store, "Beta").unwrap();
        // Seed MAX_GROUP_MEMBERS directly for fast test setup
        let mut members = Map::new();
        for i in 0..MAX_GROUP_MEMBERS {
            members.insert(format!("m_{i}"), json!({"groupId": a.group_id}));
        }
        let path = store.groups_json_path().unwrap();
        std::fs::write(
            &path,
            json!({
                "schemaVersion": 1,
                "groups": [{"groupId": a.group_id, "name": "Alpha", "createdAt": t(1)},
                           {"groupId": b.group_id, "name": "Beta", "createdAt": t(1)}],
                "members": members,
                "left": {}
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            assign_to_group(&store, "m_extra", Some(&a.group_id)).unwrap_err(),
            "Too many conversations in groups"
        );
        // Moving an already-grouped conversation between groups still works at the cap.
        assign_to_group(&store, "m_0", Some(&b.group_id)).unwrap();
        assert_eq!(load_groups(&store).unwrap().members["m_0"].group_id, b.group_id);
    }

    #[test]
    fn remove_deletes_group_and_its_members_keeping_other_groups() {
        let (_d, store) = temp_store();
        let a = create_group(&store, "Alpha").unwrap();
        let b = create_group(&store, "Beta").unwrap();
        assign_to_group(&store, "m_1", Some(&a.group_id)).unwrap();
        assign_to_group(&store, "m_2", Some(&b.group_id)).unwrap();
        remove_group(&store, &a.group_id).unwrap();
        let listed = load_groups(&store).unwrap();
        assert_eq!(listed.groups.len(), 1);
        assert_eq!(listed.groups[0].group_id, b.group_id);
        assert!(!listed.members.contains_key("m_1"));
        assert_eq!(listed.members["m_2"].group_id, b.group_id);
        assert_eq!(remove_group(&store, &a.group_id).unwrap_err(), "That group does not exist");
        assert_eq!(remove_group(&store, "bad/id").unwrap_err(), "Group id is invalid");
    }

    #[test]
    fn unreadability_blocks_all_mutations_and_never_overwrites_file() {
        let (_d, store) = temp_store();
        let path = store.groups_json_path().unwrap();
        std::fs::write(&path, "{broken").unwrap();
        assert_eq!(load_groups(&store).unwrap_err(), GROUPS_UNREADABLE);
        assert_eq!(create_group(&store, "X").unwrap_err(), GROUPS_UNREADABLE);
        assert_eq!(rename_group(&store, "grp_x", "Y").unwrap_err(), GROUPS_UNREADABLE);
        assert_eq!(remove_group(&store, "grp_x").unwrap_err(), GROUPS_UNREADABLE);
        assert_eq!(assign_to_group(&store, "m_1", None).unwrap_err(), GROUPS_UNREADABLE);
        assert_eq!(set_group_instructions(&store, "grp_x", "hello").unwrap_err(), GROUPS_UNREADABLE);
        assert_eq!(set_group_route(&store, "grp_x", None).unwrap_err(), GROUPS_UNREADABLE);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken");
    }

    #[test]
    fn rejects_array_or_oversized_groups_json_as_unreadable() {
        let (_d, store) = temp_store();
        let path = store.groups_json_path().unwrap();
        std::fs::write(&path, "[]").unwrap();
        assert_eq!(load_groups(&store).unwrap_err(), GROUPS_UNREADABLE);

        let oversized = vec![b' '; (MAX_FILE_BYTES + 10) as usize];
        std::fs::write(&path, oversized).unwrap();
        assert_eq!(load_groups(&store).unwrap_err(), GROUPS_UNREADABLE);
    }

    #[test]
    fn parses_legacy_bare_string_memberships_without_fake_at_and_drops_dangling() {
        let (_d, store) = temp_store();
        let path = store.groups_json_path().unwrap();
        std::fs::write(
            &path,
            json!({
                "groups": [{"groupId": "grp_a", "name": "Alpha", "createdAt": t(1)}],
                "members": {
                    "m_legacy": "grp_a",
                    "m_with_at": {"groupId": "grp_a", "at": t(2)},
                    "m_orphan": "grp_missing",
                    "bad/id": "grp_a"
                }
            })
            .to_string(),
        )
        .unwrap();
        let listed = load_groups(&store).unwrap();
        assert_eq!(listed.members["m_legacy"], GroupMembership { group_id: "grp_a".into(), at: None });
        assert_eq!(listed.members["m_with_at"], GroupMembership { group_id: "grp_a".into(), at: Some(t(2)) });
        assert!(!listed.members.contains_key("m_orphan"));
        assert!(!listed.members.contains_key("bad/id"));
    }

    #[test]
    fn sets_and_clears_default_route_and_rejects_invalid_routes() {
        let (_d, store) = temp_store();
        let g = create_group(&store, "Alpha").unwrap();
        let route = GroupRoute {
            runtime: "codex".into(),
            model: "gpt-5".into(),
            mode: "accept-edits".into(),
            account: Some("acct_1".into()),
            effort: Some("high".into()),
        };
        let saved = set_group_route(&store, &g.group_id, Some(route.clone())).unwrap();
        assert_eq!(saved.route.as_ref(), Some(&route));
        assert_eq!(load_groups(&store).unwrap().groups[0].route.as_ref(), Some(&route));

        let bad_mode = GroupRoute {
            runtime: "codex".into(),
            model: "gpt-5".into(),
            mode: "yolo".into(),
            account: None,
            effort: None,
        };
        assert_eq!(
            set_group_route(&store, &g.group_id, Some(bad_mode)).unwrap_err(),
            "That route is not one a mission can start on"
        );
        let bad_runtime = GroupRoute {
            runtime: "  ".into(),
            model: "gpt-5".into(),
            mode: "ask".into(),
            account: None,
            effort: None,
        };
        assert_eq!(
            set_group_route(&store, &g.group_id, Some(bad_runtime)).unwrap_err(),
            "That route is not one a mission can start on"
        );
        let bad_effort = GroupRoute {
            runtime: "codex".into(),
            model: "gpt-5".into(),
            mode: "ask".into(),
            account: None,
            effort: Some("extreme".into()),
        };
        assert_eq!(
            set_group_route(&store, &g.group_id, Some(bad_effort)).unwrap_err(),
            "That route is not one a mission can start on"
        );
        assert_eq!(
            set_group_route(&store, "grp_missing", None).unwrap_err(),
            "That group does not exist"
        );

        let cleared = set_group_route(&store, &g.group_id, None).unwrap();
        assert_eq!(cleared.route, None);
        assert_eq!(load_groups(&store).unwrap().groups[0].route, None);
    }

    #[test]
    fn sets_trims_caps_clears_and_refuses_secrets_in_instructions() {
        let (_d, store) = temp_store();
        let g = create_group(&store, "Trading").unwrap();
        let saved = set_group_instructions(&store, &g.group_id, "  Run tests first.  ").unwrap();
        assert_eq!(saved.instructions, "Run tests first.");
        assert_eq!(load_groups(&store).unwrap().groups[0].instructions, "Run tests first.");

        let long = "x".repeat(MAX_GROUP_INSTRUCTIONS_LENGTH + 500);
        let capped = set_group_instructions(&store, &g.group_id, &long).unwrap();
        assert_eq!(capped.instructions.encode_utf16().count(), MAX_GROUP_INSTRUCTIONS_LENGTH);

        let secret = format!("Use {}-ant-api03-1234567890abcdefghijklmnop on CI", "sk");
        let err = set_group_instructions(&store, &g.group_id, &secret).unwrap_err();
        assert!(err.contains("Trading"), "err: {err}");
        // Previous instructions stayed intact after refused write
        assert_eq!(
            load_groups(&store).unwrap().groups[0].instructions.encode_utf16().count(),
            MAX_GROUP_INSTRUCTIONS_LENGTH
        );

        let cleared = set_group_instructions(&store, &g.group_id, "   ").unwrap();
        assert_eq!(cleared.instructions, "");
        assert_eq!(
            set_group_instructions(&store, "grp_missing", "hi").unwrap_err(),
            "That group does not exist"
        );
    }

    #[test]
    fn electron_groups_json_is_read_only_when_native_writes() {
        let (dir, store) = temp_store();
        let epath = dir.path().join("electron/groups.json");
        let original = json!({
            "schemaVersion": 1,
            "groups": [{"groupId": "grp_e", "name": "Electron Project", "createdAt": t(1), "instructions": "Keep safe"}],
            "members": {"m_1": {"groupId": "grp_e", "at": t(2)}}
        })
        .to_string();
        std::fs::write(&epath, &original).unwrap();
        let listed = load_groups(&store).unwrap();
        assert_eq!(listed.groups[0].name, "Electron Project");

        set_group_instructions(&store, "grp_e", "Native update").unwrap();
        assert_eq!(std::fs::read_to_string(&epath).unwrap(), original);
        assert_eq!(load_groups(&store).unwrap().groups[0].instructions, "Native update");
    }

    // --- Ported from main/groups-remember-leaving.test.ts (6 it cases) ---

    #[test]
    fn records_leaving_when_unassigned_and_preserves_at_and_instructions() {
        let (_d, store) = temp_store();
        let g = create_group(&store, "Trading").unwrap();
        set_group_instructions(&store, &g.group_id, "Ask before deleting.").unwrap();
        assign_to_group(&store, "m_1", Some(&g.group_id)).unwrap();
        let joined_at = load_groups(&store).unwrap().members["m_1"].at.clone();
        assert!(joined_at.is_some());

        assign_to_group(&store, "m_1", None).unwrap();
        let listed = load_groups(&store).unwrap();
        assert!(!listed.members.contains_key("m_1"));
        let left = &listed.left["m_1"];
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].group_id, g.group_id);
        assert_eq!(left[0].name, "Trading");
        assert_eq!(left[0].instructions, "Ask before deleting.");
        assert_eq!(left[0].at, joined_at);
        assert!(parse_ms(&left[0].until).is_some());
    }

    #[test]
    fn records_leaving_when_moving_to_another_group() {
        let (_d, store) = temp_store();
        let a = create_group(&store, "Alpha").unwrap();
        let b = create_group(&store, "Beta").unwrap();
        set_group_instructions(&store, &a.group_id, "Alpha rule").unwrap();
        set_group_instructions(&store, &b.group_id, "Beta rule").unwrap();

        assign_to_group(&store, "m_1", Some(&a.group_id)).unwrap();
        assign_to_group(&store, "m_1", Some(&b.group_id)).unwrap();

        let listed = load_groups(&store).unwrap();
        assert_eq!(listed.members["m_1"].group_id, b.group_id);
        assert_eq!(listed.left["m_1"].len(), 1);
        assert_eq!(listed.left["m_1"][0].group_id, a.group_id);
        assert_eq!(listed.left["m_1"][0].instructions, "Alpha rule");
    }

    #[test]
    fn assigning_to_same_group_twice_records_no_leaving_and_keeps_joined_at() {
        let (_d, store) = temp_store();
        let a = create_group(&store, "Alpha").unwrap();
        set_group_instructions(&store, &a.group_id, "Alpha rule").unwrap();
        assign_to_group(&store, "m_1", Some(&a.group_id)).unwrap();
        let first_at = load_groups(&store).unwrap().members["m_1"].at.clone();

        assign_to_group(&store, "m_1", Some(&a.group_id)).unwrap();
        let listed = load_groups(&store).unwrap();
        assert_eq!(listed.members["m_1"].at, first_at);
        assert!(!listed.left.contains_key("m_1"));
    }

    #[test]
    fn removing_group_records_leaving_for_all_its_members() {
        let (_d, store) = temp_store();
        let g = create_group(&store, "Trading").unwrap();
        set_group_instructions(&store, &g.group_id, "Risk check").unwrap();
        assign_to_group(&store, "m_1", Some(&g.group_id)).unwrap();
        assign_to_group(&store, "m_2", Some(&g.group_id)).unwrap();
        remove_group(&store, &g.group_id).unwrap();

        let listed = load_groups(&store).unwrap();
        assert!(listed.groups.is_empty());
        assert!(listed.members.is_empty());
        assert_eq!(listed.left["m_1"][0].name, "Trading");
        assert_eq!(listed.left["m_1"][0].instructions, "Risk check");
        assert_eq!(listed.left["m_2"][0].name, "Trading");
    }

    #[test]
    fn unassigning_bare_legacy_membership_records_leaving_without_at() {
        let (_d, store) = temp_store();
        let path = store.groups_json_path().unwrap();
        std::fs::write(
            &path,
            json!({
                "schemaVersion": 1,
                "groups": [{"groupId": "grp_a", "name": "Legacy", "createdAt": t(1), "instructions": "Old rule"}],
                "members": {"m_1": "grp_a"}
            })
            .to_string(),
        )
        .unwrap();
        assign_to_group(&store, "m_1", None).unwrap();
        let listed = load_groups(&store).unwrap();
        assert_eq!(listed.left["m_1"][0].at, None);
        assert_eq!(listed.left["m_1"][0].name, "Legacy");
    }

    #[test]
    fn caps_left_history_per_conversation_to_eight_newest() {
        let (_d, store) = temp_store();
        let g = create_group(&store, "Alpha").unwrap();
        for i in 0..11 {
            set_group_instructions(&store, &g.group_id, &format!("Rule {i}")).unwrap();
            assign_to_group(&store, "m_1", Some(&g.group_id)).unwrap();
            assign_to_group(&store, "m_1", None).unwrap();
        }
        let listed = load_groups(&store).unwrap();
        let left = &listed.left["m_1"];
        assert_eq!(left.len(), MAX_LEFT_PER_CONVERSATION);
        assert_eq!(left[0].instructions, "Rule 3");
        assert_eq!(left[7].instructions, "Rule 10");
    }

    // --- Chain lookup and briefing ---

    #[test]
    fn keys_before_walks_chain_stops_at_max_hops_and_breaks_cycles() {
        let (_d, store) = temp_store();
        let ledger = store.ledger_dir().unwrap();
        std::fs::create_dir_all(&ledger).unwrap();
        std::fs::write(ledger.join("m_1.jsonl"), json!({"missionId": "m_1"}).to_string() + "\n").unwrap();
        std::fs::write(
            ledger.join("m_2.jsonl"),
            json!({"missionId": "m_2", "continuesFrom": {"missionId": "m_1"}}).to_string() + "\n",
        )
        .unwrap();
        std::fs::write(
            ledger.join("m_3.jsonl"),
            json!({"missionId": "m_3", "continuesFrom": {"missionId": "m_2"}}).to_string() + "\n",
        )
        .unwrap();
        assert_eq!(keys_before(&store, Some("m_3")), vec!["m_3", "m_2", "m_1"]);

        // Cycle protection
        std::fs::write(
            ledger.join("cyc_a.jsonl"),
            json!({"missionId": "cyc_a", "continuesFrom": {"missionId": "cyc_b"}}).to_string() + "\n",
        )
        .unwrap();
        std::fs::write(
            ledger.join("cyc_b.jsonl"),
            json!({"missionId": "cyc_b", "continuesFrom": {"missionId": "cyc_a"}}).to_string() + "\n",
        )
        .unwrap();
        assert_eq!(keys_before(&store, Some("cyc_a")), vec!["cyc_a", "cyc_b"]);

        // MAX_CHAIN_HOPS bound
        for i in 0..70 {
            let line = if i == 0 {
                json!({"missionId": format!("hop_{i}")})
            } else {
                json!({"missionId": format!("hop_{i}"), "continuesFrom": {"missionId": format!("hop_{}", i - 1)}})
            };
            std::fs::write(ledger.join(format!("hop_{i}.jsonl")), line.to_string() + "\n").unwrap();
        }
        let keys = keys_before(&store, Some("hop_69"));
        assert_eq!(keys.len(), MAX_CHAIN_HOPS);
        assert_eq!(keys[0], "hop_69");
    }

    #[test]
    fn group_brief_for_resolves_newest_key_and_formats_section() {
        let mut listed = ListedGroups::default();
        listed.groups.push(Group::new(
            "grp_a".into(),
            "Alpha".into(),
            t(1),
            "Keep commits small.".into(),
            None,
        ));
        listed.members.insert(
            "m_1".into(),
            GroupMembership { group_id: "grp_a".into(), at: Some(t(2)) },
        );
        let brief = group_brief_for(&["m_3".into(), "m_2".into(), "m_1".into()], &listed).unwrap();
        assert_eq!(brief.group_name, "Alpha");
        assert_eq!(
            group_section(&brief.group_name, &brief.instructions).unwrap(),
            "Standing instructions for the group \"Alpha\", which this conversation is in. Every turn of it is given these:\nKeep commits small."
        );
    }

    // --- Ported from renderer/src/group-boundary.test.ts (4 it cases) ---

    #[test]
    fn group_boundary_places_line_before_first_turn_at_or_after_joined_at() {
        let t1 = t(10);
        let t2 = t(12);
        let t3 = t(14);
        let joined = "2026-09-16T11:00:00.000Z";
        let early = "2026-09-16T09:00:00.000Z";
        let late = "2026-09-16T15:00:00.000Z";
        let starts = [Some(t1.as_str()), Some(t2.as_str()), Some(t3.as_str())];

        assert_eq!(
            group_boundary(&starts, Some(joined), "Trading", "Ask before trading.").unwrap().before_turn,
            1
        );
        assert_eq!(
            group_boundary(&starts, Some(early), "Trading", "Ask before trading.").unwrap().before_turn,
            0
        );
        assert_eq!(
            group_boundary(&starts, Some(late), "Trading", "Ask before trading.").unwrap().before_turn,
            3
        );
    }

    #[test]
    fn group_boundary_returns_none_for_missing_at_blank_instructions_or_invalid_time() {
        let t1 = t(10);
        let starts = [Some(t1.as_str())];
        assert_eq!(group_boundary(&starts, None, "Trading", "Ask"), None);
        assert_eq!(group_boundary(&starts, Some(&t1), "Plain", "   "), None);
        assert_eq!(group_boundary(&starts, Some("not a time"), "Trading", "Ask"), None);
    }

    #[test]
    fn group_boundary_skips_turns_without_start_time() {
        let t2 = t(12);
        let joined = "2026-09-16T11:00:00.000Z";
        assert_eq!(
            group_boundary(&[None, Some(t2.as_str())], Some(joined), "Trading", "Ask")
                .unwrap()
                .before_turn,
            1
        );
        assert_eq!(
            group_boundary(&[None, None], Some(joined), "Trading", "Ask")
                .unwrap()
                .before_turn,
            2
        );
    }

    // --- Ported from renderer/src/group-join-line-survives-leaving.test.ts (4 it cases) ---

    fn make_left(at: Option<String>, until: String, instructions: &str) -> LeftMembership {
        LeftMembership {
            group_id: "grp_a".into(),
            name: "Trading".into(),
            instructions: instructions.into(),
            at,
            until,
        }
    }

    #[test]
    fn group_joins_keeps_join_boundary_after_leaving() {
        let starts_owned = [t(9), t(10), t(12)];
        let starts: Vec<Option<&str>> = starts_owned.iter().map(|s| Some(s.as_str())).collect();
        let joins = group_joins(&starts, &[make_left(Some(t(11)), t(13), "Ask before trading.")]);
        assert_eq!(
            joins,
            vec![GroupBoundary {
                before_turn: 2,
                group_name: "Trading".into(),
                instructions: "Ask before trading.".into(),
            }]
        );
    }

    #[test]
    fn group_joins_carries_the_instructions_that_actually_briefed() {
        let starts_owned = [t(9), t(10), t(12)];
        let starts: Vec<Option<&str>> = starts_owned.iter().map(|s| Some(s.as_str())).collect();
        let joins = group_joins(&starts, &[make_left(Some(t(11)), t(13), "Snapshot at leave time")]);
        assert_eq!(joins[0].instructions, "Snapshot at leave time");
    }

    #[test]
    fn group_joins_skips_missing_at_or_blank_instructions() {
        let starts_owned = [t(9), t(10)];
        let starts: Vec<Option<&str>> = starts_owned.iter().map(|s| Some(s.as_str())).collect();
        assert!(group_joins(&starts, &[make_left(None, t(11), "Ask")]).is_empty());
        assert!(group_joins(&starts, &[make_left(Some(t(9)), t(11), "   ")]).is_empty());
    }

    #[test]
    fn group_joins_keeps_rejoin_history_in_order() {
        let starts_owned = [t(1), t(5), t(9)];
        let starts: Vec<Option<&str>> = starts_owned.iter().map(|s| Some(s.as_str())).collect();
        let joins = group_joins(
            &starts,
            &[
                make_left(Some(t(2)), t(4), "Rule 1"),
                make_left(Some(t(6)), t(8), "Rule 2"),
            ],
        );
        assert_eq!(joins.len(), 2);
        assert_eq!(joins[0].before_turn, 1);
        assert_eq!(joins[1].before_turn, 2);
    }

    // --- Ported from renderer/src/group-leaving-line.test.ts (4 it cases) ---

    #[test]
    fn group_leavings_places_line_before_first_turn_at_or_after_until() {
        let starts_owned = [t(9), t(10), t(12)];
        let starts: Vec<Option<&str>> = starts_owned.iter().map(|s| Some(s.as_str())).collect();
        assert_eq!(
            group_leavings(&starts, &[make_left(Some(t(9)), t(11), "Ask before trading.")]),
            vec![GroupLeaving { before_turn: 2, group_name: "Trading".into() }]
        );
    }

    #[test]
    fn group_leavings_sits_at_end_when_conversation_has_not_continued_since() {
        let starts_owned = [t(9), t(10)];
        let starts: Vec<Option<&str>> = starts_owned.iter().map(|s| Some(s.as_str())).collect();
        assert_eq!(
            group_leavings(&starts, &[make_left(Some(t(9)), t(11), "Ask before trading.")]),
            vec![GroupLeaving { before_turn: 2, group_name: "Trading".into() }]
        );
    }

    #[test]
    fn group_leavings_skips_blank_instructions_and_unparseable_until() {
        let starts_owned = [t(9)];
        let starts: Vec<Option<&str>> = starts_owned.iter().map(|s| Some(s.as_str())).collect();
        assert!(group_leavings(&starts, &[make_left(Some(t(9)), t(10), "   ")]).is_empty());
        assert!(group_leavings(&starts, &[make_left(Some(t(9)), "not a time".into(), "Ask")]).is_empty());
    }

    #[test]
    fn group_leavings_skips_turns_without_start_time_and_handles_multiple() {
        let t10 = t(10);
        let t14 = t(14);
        assert_eq!(
            group_leavings(
                &[None, Some(t10.as_str()), Some(t14.as_str())],
                &[
                    make_left(None, t(9), "Ask"),
                    make_left(None, t(13), "Ask"),
                ],
            ),
            vec![
                GroupLeaving { before_turn: 1, group_name: "Trading".into() },
                GroupLeaving { before_turn: 2, group_name: "Trading".into() },
            ]
        );
    }
}
