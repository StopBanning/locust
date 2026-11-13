//! The rooms store (`main/room-store.ts`): `rooms.json` in `Store::root()`,
//! fail-closed reads, atomic writes via `Store::create_new_file_in`,
//! queue transitions (`start_queued`, `refuse_queued`), board mutations (`apply_task_ops`, `update_task`),
//! no-bounce guard on handoffs, `recipients_of`, and `exchange_of_room_post`.

use crate::room_task::{TaskOp, bounded_task_text, task_key, utf16_len};
use crate::store::Store;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::sync::Mutex;

pub const SCHEMA_VERSION: u64 = 1;
pub const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_ROOMS: usize = 32;
/// Agrees with `native::MAX_LIVE_TURNS` (8) so a full room can always answer a post.
pub const MAX_ROOM_TEAMMATES: usize = crate::native::MAX_LIVE_TURNS;
pub const MAX_ROOM_POSTS: usize = 200;
pub const MAX_ROOM_NAME_LENGTH: usize = 60;
pub const MAX_POST_LENGTH: usize = 8_000;
pub const MAX_ROOM_TASKS: usize = 100;

/// Said when the rooms file exists and cannot be read.
pub const UNREADABLE: &str =
    "The rooms file could not be read, so nothing was changed. Every room is as it was.";

static WRITES: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomTask {
    pub task_id: String,
    pub text: String,
    pub owner_id: Option<String>,
    pub state: String,
    pub mission_id: Option<String>,
    pub handed_by: Option<String>,
    pub handed_without_note: bool,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomPost {
    pub post_id: String,
    pub text: String,
    pub at: String,
    pub missions: BTreeMap<String, String>,
    pub queued: Vec<String>,
    pub refused: BTreeMap<String, String>,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicRoom {
    pub room_id: String,
    pub name: String,
    pub teammate_ids: Vec<String>,
    pub created_at: String,
    pub posts: Vec<RoomPost>,
    pub tasks: Vec<RoomTask>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRoomsFile {
    pub schema_version: u64,
    pub rooms: Vec<PublicRoom>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipientsResult {
    Ok {
        asked: Vec<String>,
        to: Option<Vec<String>>,
    },
    Refused {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPostInput {
    pub text: String,
    pub missions: BTreeMap<String, String>,
    pub queued: Vec<String>,
    pub refused: BTreeMap<String, String>,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskActor<'a> {
    pub teammate_id: &'a str,
    pub name: &'a str,
    pub mission_id: &'a str,
    pub noted_to: Option<&'a [String]>,
}

#[derive(Debug, Clone, Copy)]
pub struct RosterEntry<'a> {
    pub teammate_id: &'a str,
    pub name: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyTaskOpsResult {
    pub changed: Vec<String>,
    pub refused: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomTaskRequest {
    Add {
        room_id: String,
        text: String,
        owner_id: Option<String>,
    },
    Assign {
        room_id: String,
        task_id: String,
        owner_id: Option<String>,
    },
    Done {
        room_id: String,
        task_id: String,
    },
    Reopen {
        room_id: String,
        task_id: String,
    },
    Remove {
        room_id: String,
        task_id: String,
    },
    Unknown {
        room_id: String,
        task_id: String,
    },
}

pub fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn slice_utf16_max(s: &str, max_u16: usize) -> String {
    let mut used = 0;
    let mut end_byte = 0;
    for (idx, ch) in s.char_indices() {
        let n = ch.len_utf16();
        if used + n > max_u16 {
            break;
        }
        used += n;
        end_byte = idx + ch.len_utf8();
    }
    s[..end_byte].to_owned()
}

pub fn valid_room_name(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || utf16_len(trimmed) > MAX_ROOM_NAME_LENGTH {
        return false;
    }
    !trimmed.chars().any(|c| matches!(c as u32, 0x00..=0x1f | 0x7f))
}

pub fn valid_teammate_ids(ids: &[String]) -> bool {
    if ids.is_empty() || ids.len() > MAX_ROOM_TEAMMATES {
        return false;
    }
    let mut seen = HashSet::new();
    for id in ids {
        if !safe_id(id) || !seen.insert(id.as_str()) {
            return false;
        }
    }
    true
}

pub fn valid_post_text(value: &str) -> bool {
    !value.trim().is_empty()
        && utf16_len(value) <= MAX_POST_LENGTH
        && !value
            .chars()
            .any(|c| matches!(c as u32, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f))
}

fn valid_iso_date(s: &str) -> bool {
    DateTime::parse_from_rfc3339(s).is_ok()
}

fn parsed_task(value: &Value) -> Option<RoomTask> {
    let obj = value.as_object()?;
    let task_id = obj.get("taskId")?.as_str()?;
    if !safe_id(task_id) {
        return None;
    }
    let raw_text = obj.get("text")?.as_str()?;
    let text = bounded_task_text(raw_text);
    if text.is_empty() {
        return None;
    }
    let state = obj.get("state")?.as_str()?;
    if !matches!(state, "open" | "in-hand" | "done") {
        return None;
    }
    let owner_id = match obj.get("ownerId") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if safe_id(s) => Some(s.clone()),
        _ => return None,
    };
    let mission_id = match obj.get("missionId") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if safe_id(s) => Some(s.clone()),
        _ => return None,
    };
    let handed_by = match obj.get("handedBy") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if safe_id(s) => Some(s.clone()),
        _ => return None,
    };
    let handed_without_note = obj.get("handedWithoutNote") == Some(&Value::Bool(true));
    let at = obj.get("at")?.as_str()?;
    if !valid_iso_date(at) {
        return None;
    }
    Some(RoomTask {
        task_id: task_id.to_owned(),
        text,
        owner_id,
        state: state.to_owned(),
        mission_id,
        handed_by,
        handed_without_note,
        at: at.to_owned(),
    })
}

fn parsed_post(value: &Value) -> Option<RoomPost> {
    let obj = value.as_object()?;
    let post_id = obj.get("postId")?.as_str()?;
    let text = obj.get("text")?.as_str()?;
    if !safe_id(post_id) || !valid_post_text(text) {
        return None;
    }
    let at = obj.get("at")?.as_str()?;
    if !valid_iso_date(at) {
        return None;
    }
    let missions_obj = obj.get("missions")?.as_object()?;
    let mut missions = BTreeMap::new();
    for (tid, mid) in missions_obj {
        let mid_str = mid.as_str()?;
        if !safe_id(tid) || !safe_id(mid_str) {
            return None;
        }
        missions.insert(tid.clone(), mid_str.to_owned());
    }
    let queued: Vec<String> = obj
        .get("queued")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|id| safe_id(id) && !missions.contains_key(*id))
        .map(str::to_owned)
        .collect();
    let mut refused = BTreeMap::new();
    if let Some(ref_obj) = obj.get("refused").and_then(|v| v.as_object()) {
        for (tid, reason) in ref_obj {
            if safe_id(tid)
                && let Some(r) = reason.as_str()
                && !r.is_empty()
            {
                refused.insert(tid.clone(), slice_utf16_max(r, 200));
            }
        }
    }
    let mut to = Vec::new();
    let mut to_seen = HashSet::new();
    if let Some(to_arr) = obj.get("to").and_then(|v| v.as_array()) {
        for entry in to_arr {
            if let Some(id) = entry.as_str()
                && safe_id(id)
                && to_seen.insert(id.to_owned())
            {
                to.push(id.to_owned());
            }
        }
    }
    Some(RoomPost {
        post_id: post_id.to_owned(),
        text: text.to_owned(),
        at: at.to_owned(),
        missions,
        queued,
        refused,
        to,
    })
}

pub fn parsed_room(value: &Value) -> Option<PublicRoom> {
    let obj = value.as_object()?;
    let room_id = obj.get("roomId")?.as_str()?;
    let name = obj.get("name")?.as_str()?;
    if !safe_id(room_id) || !valid_room_name(name) {
        return None;
    }
    let raw_ids = obj.get("teammateIds")?.as_array()?;
    let mut teammate_ids = Vec::with_capacity(raw_ids.len());
    for entry in raw_ids {
        teammate_ids.push(entry.as_str()?.to_owned());
    }
    if !valid_teammate_ids(&teammate_ids) {
        return None;
    }
    let created_at = obj.get("createdAt")?.as_str()?;
    if !valid_iso_date(created_at) {
        return None;
    }
    let raw_posts = obj.get("posts")?.as_array()?;
    let mut posts = Vec::new();
    for entry in raw_posts {
        if let Some(post) = parsed_post(entry) {
            posts.push(post);
            if posts.len() >= MAX_ROOM_POSTS {
                break;
            }
        }
    }
    let mut tasks = Vec::new();
    if let Some(raw_tasks_val) = obj.get("tasks") {
        let raw_tasks = raw_tasks_val.as_array()?;
        for entry in raw_tasks {
            if let Some(task) = parsed_task(entry) {
                tasks.push(task);
                if tasks.len() >= MAX_ROOM_TASKS {
                    break;
                }
            }
        }
    }
    Some(PublicRoom {
        room_id: room_id.to_owned(),
        name: name.trim().to_owned(),
        teammate_ids,
        created_at: created_at.to_owned(),
        posts,
        tasks,
    })
}

pub fn parsed_file(text: &str) -> Result<StoredRoomsFile, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| UNREADABLE.to_owned())?;
    let obj = value.as_object().ok_or_else(|| UNREADABLE.to_owned())?;
    if obj.get("schemaVersion").and_then(|v| v.as_u64()) != Some(SCHEMA_VERSION) {
        return Err(UNREADABLE.to_owned());
    }
    let raw_rooms = obj
        .get("rooms")
        .and_then(|v| v.as_array())
        .ok_or_else(|| UNREADABLE.to_owned())?;
    let mut rooms = Vec::new();
    let mut seen = HashSet::new();
    for entry in raw_rooms {
        let Some(room) = parsed_room(entry) else {
            continue;
        };
        if !seen.insert(room.room_id.clone()) {
            continue;
        }
        rooms.push(room);
        if rooms.len() >= MAX_ROOMS {
            break;
        }
    }
    Ok(StoredRoomsFile {
        schema_version: SCHEMA_VERSION,
        rooms,
    })
}

fn task_to_json(t: &RoomTask) -> Value {
    let mut obj = Map::new();
    obj.insert("taskId".into(), Value::String(t.task_id.clone()));
    obj.insert("text".into(), Value::String(t.text.clone()));
    if let Some(owner) = &t.owner_id {
        obj.insert("ownerId".into(), Value::String(owner.clone()));
    }
    obj.insert("state".into(), Value::String(t.state.clone()));
    if let Some(mid) = &t.mission_id {
        obj.insert("missionId".into(), Value::String(mid.clone()));
    }
    if let Some(by) = &t.handed_by {
        obj.insert("handedBy".into(), Value::String(by.clone()));
    }
    if t.handed_without_note {
        obj.insert("handedWithoutNote".into(), Value::Bool(true));
    }
    obj.insert("at".into(), Value::String(t.at.clone()));
    Value::Object(obj)
}

fn post_to_json(p: &RoomPost) -> Value {
    let mut obj = Map::new();
    obj.insert("postId".into(), Value::String(p.post_id.clone()));
    obj.insert("text".into(), Value::String(p.text.clone()));
    obj.insert("at".into(), Value::String(p.at.clone()));
    let mut missions = Map::new();
    for (k, v) in &p.missions {
        missions.insert(k.clone(), Value::String(v.clone()));
    }
    obj.insert("missions".into(), Value::Object(missions));
    // Important: omit `"queued"` altogether once the queue is empty.
    if !p.queued.is_empty() {
        obj.insert(
            "queued".into(),
            Value::Array(p.queued.iter().cloned().map(Value::String).collect()),
        );
    }
    if !p.refused.is_empty() {
        let mut refused = Map::new();
        for (k, v) in &p.refused {
            refused.insert(k.clone(), Value::String(v.clone()));
        }
        obj.insert("refused".into(), Value::Object(refused));
    }
    if !p.to.is_empty() {
        obj.insert(
            "to".into(),
            Value::Array(p.to.iter().cloned().map(Value::String).collect()),
        );
    }
    Value::Object(obj)
}

fn room_to_json(r: &PublicRoom) -> Value {
    json!({
        "roomId": r.room_id,
        "name": r.name,
        "teammateIds": r.teammate_ids,
        "createdAt": r.created_at,
        "posts": r.posts.iter().map(post_to_json).collect::<Vec<_>>(),
        "tasks": r.tasks.iter().map(task_to_json).collect::<Vec<_>>(),
    })
}

fn file_to_json(f: &StoredRoomsFile) -> Value {
    json!({
        "schemaVersion": f.schema_version,
        "rooms": f.rooms.iter().map(room_to_json).collect::<Vec<_>>(),
    })
}

fn read_locked(store: &Store) -> Result<StoredRoomsFile, String> {
    let path = store.rooms_json_path().map_err(|_| UNREADABLE.to_owned())?;
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(StoredRoomsFile {
                schema_version: SCHEMA_VERSION,
                rooms: Vec::new(),
            });
        }
        Err(_) => return Err(UNREADABLE.to_owned()),
    };
    let mut text = String::new();
    let read_res = file
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_string(&mut text);
    if read_res.is_err() || text.len() > MAX_FILE_BYTES {
        return Err(UNREADABLE.to_owned());
    }
    parsed_file(&text)
}

fn write_locked(store: &Store, file: &StoredRoomsFile) -> Result<(), String> {
    use std::io::Write;
    let path = store.rooms_json_path().map_err(|e| e.to_string())?;
    let uuid = crate::native::uuid_v4().map_err(|e| e.to_string())?;
    let tmp = format!("rooms.json.{uuid}.tmp");
    let mut handle = store
        .create_new_file_in(store.root(), &tmp)
        .map_err(|e| e.to_string())?;
    let body = serde_json::to_string_pretty(&file_to_json(file)).map_err(|e| e.to_string())?;
    let res = (|| -> std::io::Result<()> {
        handle.write_all(body.as_bytes())?;
        handle.write_all(b"\n")?;
        handle.sync_all()?;
        store.check(&path)?;
        std::fs::rename(store.root().join(&tmp), path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(store.root().join(&tmp));
    }
    res.map_err(|e| e.to_string())
}

fn new_id_suffix() -> Result<String, String> {
    let u = crate::native::uuid_v4().map_err(|e| e.to_string())?;
    Ok(u.replace('-', "").chars().take(20).collect())
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// The exchange a room post's runs share, for the relay's budget (`exchangeOfRoomPost`).
pub fn exchange_of_room_post(rooms: &[PublicRoom], mission_id: &str) -> Option<String> {
    for room in rooms {
        if let Some(post) = room
            .posts
            .iter()
            .find(|entry| entry.missions.values().any(|mid| mid == mission_id))
        {
            return Some(format!("room:{}:{}", room.room_id, post.post_id));
        }
    }
    None
}

/// Who a post asks (`recipientsOf`): the members the person named, or everyone.
pub fn recipients_of(teammate_ids: &[String], to: Option<&[String]>) -> RecipientsResult {
    let Some(list) = to else {
        return RecipientsResult::Ok {
            asked: teammate_ids.to_vec(),
            to: None,
        };
    };
    if list.is_empty() {
        return RecipientsResult::Ok {
            asked: teammate_ids.to_vec(),
            to: None,
        };
    }
    let named: HashSet<&str> = list
        .iter()
        .map(String::as_str)
        .filter(|id| safe_id(id))
        .collect();
    let asked: Vec<String> = teammate_ids
        .iter()
        .filter(|id| named.contains(id.as_str()))
        .cloned()
        .collect();
    if asked.is_empty() {
        return RecipientsResult::Refused {
            message: "Nobody you named is in this room any more, so nothing was posted.".to_owned(),
        };
    }
    if asked.len() == teammate_ids.len() {
        return RecipientsResult::Ok { asked, to: None };
    }
    let to_some = asked.clone();
    RecipientsResult::Ok {
        asked,
        to: Some(to_some),
    }
}

pub fn list(store: &Store) -> Result<Vec<PublicRoom>, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    Ok(read_locked(store)?.rooms)
}

pub fn get(store: &Store, room_id: &str) -> Result<Option<PublicRoom>, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    Ok(read_locked(store)?
        .rooms
        .into_iter()
        .find(|r| r.room_id == room_id))
}

pub fn create(store: &Store, name: &str, teammate_ids: &[String]) -> Result<PublicRoom, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !valid_room_name(name) {
        return Err("Give the room a name of up to 60 characters.".to_owned());
    }
    if !valid_teammate_ids(teammate_ids) {
        return Err(format!(
            "A room needs between 1 and {MAX_ROOM_TEAMMATES} teammates."
        ));
    }
    let mut file = read_locked(store)?;
    if file.rooms.len() >= MAX_ROOMS {
        return Err(format!("Locust keeps at most {MAX_ROOMS} rooms."));
    }
    let room = PublicRoom {
        room_id: format!("room_{}", new_id_suffix()?),
        name: name.trim().to_owned(),
        teammate_ids: teammate_ids.to_vec(),
        created_at: now_iso(),
        posts: Vec::new(),
        tasks: Vec::new(),
    };
    file.rooms.push(room.clone());
    write_locked(store, &file)?;
    Ok(room)
}

pub fn rename(store: &Store, room_id: &str, name: &str) -> Result<PublicRoom, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !valid_room_name(name) {
        return Err("Give the room a name of up to 60 characters.".to_owned());
    }
    let mut file = read_locked(store)?;
    let Some(room) = file.rooms.iter_mut().find(|r| r.room_id == room_id) else {
        return Err("That room does not exist.".to_owned());
    };
    room.name = name.trim().to_owned();
    let renamed = room.clone();
    write_locked(store, &file)?;
    Ok(renamed)
}

pub fn remove(store: &Store, room_id: &str) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = read_locked(store)?;
    let before = file.rooms.len();
    file.rooms.retain(|r| r.room_id != room_id);
    if file.rooms.len() != before {
        write_locked(store, &file)?;
    }
    Ok(())
}

pub fn add_post(store: &Store, room_id: &str, post: NewPostInput) -> Result<RoomPost, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    if !valid_post_text(&post.text) {
        return Err("Write something to post.".to_owned());
    }
    let mut file = read_locked(store)?;
    let Some(room) = file.rooms.iter_mut().find(|r| r.room_id == room_id) else {
        return Err("That room no longer exists.".to_owned());
    };
    let mut missions = BTreeMap::new();
    for (tid, mid) in post.missions {
        if safe_id(&tid) && safe_id(&mid) {
            missions.insert(tid, mid);
        }
    }
    let mut refused = BTreeMap::new();
    for (tid, reason) in post.refused {
        if safe_id(&tid) && !reason.is_empty() {
            refused.insert(tid, slice_utf16_max(&reason, 200));
        }
    }
    let queued: Vec<String> = post
        .queued
        .into_iter()
        .filter(|tid| safe_id(tid) && !missions.contains_key(tid))
        .collect();
    let mut to = Vec::new();
    let mut seen_to = HashSet::new();
    for tid in post.to {
        if safe_id(&tid) && room.teammate_ids.contains(&tid) && seen_to.insert(tid.clone()) {
            to.push(tid);
        }
    }
    let added = RoomPost {
        post_id: format!("post_{}", new_id_suffix()?),
        text: post.text,
        at: now_iso(),
        missions,
        queued,
        refused,
        to,
    };
    room.posts.push(added.clone());
    if room.posts.len() > MAX_ROOM_POSTS {
        let drop_count = room.posts.len() - MAX_ROOM_POSTS;
        room.posts.drain(..drop_count);
    }
    write_locked(store, &file)?;
    Ok(added)
}

pub fn refuse_queued(
    store: &Store,
    room_id: &str,
    post_id: &str,
    teammate_id: &str,
    message: &str,
) -> Result<(), String> {
    if !safe_id(teammate_id) || message.is_empty() {
        return Ok(());
    }
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = read_locked(store)?;
    let Some(room) = file.rooms.iter_mut().find(|r| r.room_id == room_id) else {
        return Ok(());
    };
    let Some(post) = room.posts.iter_mut().find(|p| p.post_id == post_id) else {
        return write_locked(store, &file);
    };
    post.queued.retain(|id| id != teammate_id);
    post.refused
        .insert(teammate_id.to_owned(), slice_utf16_max(message, 200));
    write_locked(store, &file)
}

pub fn start_queued(
    store: &Store,
    room_id: &str,
    post_id: &str,
    teammate_id: &str,
    mission_id: &str,
) -> Result<(), String> {
    if !safe_id(teammate_id) || !safe_id(mission_id) {
        return Ok(());
    }
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = read_locked(store)?;
    let Some(room) = file.rooms.iter_mut().find(|r| r.room_id == room_id) else {
        return Ok(());
    };
    if let Some(post) = room.posts.iter_mut().find(|p| p.post_id == post_id) {
        post.queued.retain(|id| id != teammate_id);
        post.missions
            .insert(teammate_id.to_owned(), mission_id.to_owned());
    }
    write_locked(store, &file)
}

pub fn apply_task_ops(
    store: &Store,
    room_id: &str,
    ops: &[TaskOp],
    actor: &TaskActor<'_>,
    roster: &[RosterEntry<'_>],
) -> Result<ApplyTaskOpsResult, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = read_locked(store)?;
    let Some(room_idx) = file.rooms.iter().position(|r| r.room_id == room_id) else {
        return Ok(ApplyTaskOpsResult {
            changed: Vec::new(),
            refused: vec!["That room no longer exists.".to_owned()],
        });
    };
    let mut changed = Vec::new();
    let mut refused = Vec::new();
    let at = now_iso();
    let room_members = file.rooms[room_idx].teammate_ids.clone();
    let mut tasks = file.rooms[room_idx].tasks.clone();
    for op in ops {
        let existing_idx = tasks
            .iter()
            .position(|t| task_key(&t.text) == task_key(op.text()));
        match op {
            TaskOp::New { text } => {
                if existing_idx.is_some() {
                    refused.push(format!("{}: \"{}\" is already on the board.", actor.name, text));
                    continue;
                }
                if tasks.len() >= MAX_ROOM_TASKS {
                    refused.push(format!("The board is full ({MAX_ROOM_TASKS} tasks)."));
                    continue;
                }
                tasks.push(RoomTask {
                    task_id: format!("task_{}", new_id_suffix()?),
                    text: text.clone(),
                    owner_id: None,
                    state: "open".to_owned(),
                    mission_id: Some(actor.mission_id.to_owned()),
                    handed_by: None,
                    handed_without_note: false,
                    at: at.clone(),
                });
                changed.push(format!("{} added \"{}\".", actor.name, text));
            }
            TaskOp::Claim { text } => {
                let Some(idx) = existing_idx else {
                    if tasks.len() >= MAX_ROOM_TASKS {
                        refused.push(format!("The board is full ({MAX_ROOM_TASKS} tasks)."));
                        continue;
                    }
                    tasks.push(RoomTask {
                        task_id: format!("task_{}", new_id_suffix()?),
                        text: text.clone(),
                        owner_id: Some(actor.teammate_id.to_owned()),
                        state: "in-hand".to_owned(),
                        mission_id: Some(actor.mission_id.to_owned()),
                        handed_by: None,
                        handed_without_note: false,
                        at: at.clone(),
                    });
                    changed.push(format!("{} took on \"{}\".", actor.name, text));
                    continue;
                };
                if tasks[idx].state == "done" {
                    refused.push(format!(
                        "{} tried to claim \"{}\", which is done.",
                        actor.name, tasks[idx].text
                    ));
                    continue;
                }
                let task_text = tasks[idx].text.clone();
                tasks[idx].owner_id = Some(actor.teammate_id.to_owned());
                tasks[idx].state = "in-hand".to_owned();
                tasks[idx].mission_id = Some(actor.mission_id.to_owned());
                tasks[idx].handed_by = None;
                tasks[idx].handed_without_note = false;
                tasks[idx].at = at.clone();
                changed.push(format!("{} took on \"{}\".", actor.name, task_text));
            }
            TaskOp::Done { text } => {
                let Some(idx) = existing_idx else {
                    refused.push(format!(
                        "{} marked \"{}\" done, but it is not on the board.",
                        actor.name, text
                    ));
                    continue;
                };
                if tasks[idx].owner_id.is_none() {
                    tasks[idx].owner_id = Some(actor.teammate_id.to_owned());
                }
                tasks[idx].state = "done".to_owned();
                tasks[idx].mission_id = Some(actor.mission_id.to_owned());
                tasks[idx].at = at.clone();
                changed.push(format!("{} finished \"{}\".", actor.name, tasks[idx].text));
            }
            TaskOp::Handoff { text, to } => {
                let target = roster.iter().find(|entry| {
                    room_members.iter().any(|id| id == entry.teammate_id)
                        && entry.name.to_lowercase() == to.to_lowercase()
                });
                let Some(target) = target else {
                    refused.push(format!(
                        "{} handed \"{}\" to \"{}\", who is not in this room.",
                        actor.name, text, to
                    ));
                    continue;
                };
                if target.teammate_id == actor.teammate_id {
                    refused.push(format!(
                        "{} handed \"{}\" to themselves; to take it on, claim it.",
                        actor.name, text
                    ));
                    continue;
                }
                // A2.15: Never bounce a task back to the teammate who handed it over.
                if let Some(idx) = existing_idx
                    && tasks[idx].owner_id.as_deref() == Some(actor.teammate_id)
                    && tasks[idx].handed_by.as_deref() == Some(target.teammate_id)
                {
                    refused.push(format!(
                        "{} tried to hand \"{}\" back to {}, who handed it over. It stays with {}.",
                        actor.name, tasks[idx].text, target.name, actor.name
                    ));
                    continue;
                }
                let unnoted = actor
                    .noted_to
                    .is_some_and(|list| !list.iter().any(|id| id == target.teammate_id));
                if let Some(idx) = existing_idx {
                    tasks[idx].owner_id = Some(target.teammate_id.to_owned());
                    tasks[idx].state = "open".to_owned();
                    tasks[idx].mission_id = Some(actor.mission_id.to_owned());
                    tasks[idx].handed_by = Some(actor.teammate_id.to_owned());
                    tasks[idx].handed_without_note = unnoted;
                    tasks[idx].at = at.clone();
                } else {
                    if tasks.len() >= MAX_ROOM_TASKS {
                        refused.push(format!("The board is full ({MAX_ROOM_TASKS} tasks)."));
                        continue;
                    }
                    tasks.push(RoomTask {
                        task_id: format!("task_{}", new_id_suffix()?),
                        text: text.clone(),
                        owner_id: Some(target.teammate_id.to_owned()),
                        state: "open".to_owned(),
                        mission_id: Some(actor.mission_id.to_owned()),
                        handed_by: Some(actor.teammate_id.to_owned()),
                        handed_without_note: unnoted,
                        at: at.clone(),
                    });
                }
                changed.push(format!(
                    "{} handed \"{}\" to {}.",
                    actor.name, text, target.name
                ));
            }
        }
    }
    if !changed.is_empty() {
        file.rooms[room_idx].tasks = tasks;
        write_locked(store, &file)?;
    }
    Ok(ApplyTaskOpsResult { changed, refused })
}

pub fn update_task(store: &Store, request: RoomTaskRequest) -> Result<PublicRoom, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let room_id = match &request {
        RoomTaskRequest::Add { room_id, .. }
        | RoomTaskRequest::Assign { room_id, .. }
        | RoomTaskRequest::Done { room_id, .. }
        | RoomTaskRequest::Reopen { room_id, .. }
        | RoomTaskRequest::Remove { room_id, .. }
        | RoomTaskRequest::Unknown { room_id, .. } => room_id.clone(),
    };
    let mut file = read_locked(store)?;
    let Some(room) = file.rooms.iter_mut().find(|r| r.room_id == room_id) else {
        return Err("That room no longer exists.".to_owned());
    };
    let at = now_iso();
    match request {
        RoomTaskRequest::Add { text, owner_id, .. } => {
            let clean = bounded_task_text(&text);
            if clean.is_empty() {
                return Err("Write the task first.".to_owned());
            }
            if room
                .tasks
                .iter()
                .any(|t| task_key(&t.text) == task_key(&clean))
            {
                return Err("That task is already on the board.".to_owned());
            }
            if room.tasks.len() >= MAX_ROOM_TASKS {
                return Err(format!("The board is full ({MAX_ROOM_TASKS} tasks)."));
            }
            let owner = owner_id.filter(|id| room.teammate_ids.contains(id));
            let state = if owner.is_none() { "open" } else { "in-hand" };
            room.tasks.push(RoomTask {
                task_id: format!("task_{}", new_id_suffix()?),
                text: clean,
                owner_id: owner,
                state: state.to_owned(),
                mission_id: None,
                handed_by: None,
                handed_without_note: false,
                at,
            });
        }
        RoomTaskRequest::Assign {
            task_id, owner_id, ..
        } => {
            let Some(idx) = room.tasks.iter().position(|t| t.task_id == task_id) else {
                return Err("That task is no longer on the board.".to_owned());
            };
            if let Some(id) = &owner_id
                && !room.teammate_ids.contains(id)
            {
                return Err("Only a teammate in the room can own its task.".to_owned());
            }
            let next_state = if room.tasks[idx].state == "done" {
                "done".to_owned()
            } else if owner_id.is_none() {
                "open".to_owned()
            } else {
                "in-hand".to_owned()
            };
            room.tasks[idx].owner_id = owner_id;
            room.tasks[idx].state = next_state;
            room.tasks[idx].handed_by = None;
            room.tasks[idx].handed_without_note = false;
            room.tasks[idx].at = at;
        }
        RoomTaskRequest::Done { task_id, .. } => {
            let Some(idx) = room.tasks.iter().position(|t| t.task_id == task_id) else {
                return Err("That task is no longer on the board.".to_owned());
            };
            room.tasks[idx].state = "done".to_owned();
            room.tasks[idx].at = at;
        }
        RoomTaskRequest::Reopen { task_id, .. } => {
            let Some(idx) = room.tasks.iter().position(|t| t.task_id == task_id) else {
                return Err("That task is no longer on the board.".to_owned());
            };
            room.tasks[idx].state = if room.tasks[idx].owner_id.is_none() {
                "open".to_owned()
            } else {
                "in-hand".to_owned()
            };
            room.tasks[idx].at = at;
        }
        RoomTaskRequest::Remove { task_id, .. } => {
            if !room.tasks.iter().any(|t| t.task_id == task_id) {
                return Err("That task is no longer on the board.".to_owned());
            }
            room.tasks.retain(|t| t.task_id != task_id);
        }
        RoomTaskRequest::Unknown { task_id, .. } => {
            if !room.tasks.iter().any(|t| t.task_id == task_id) {
                return Err("That task is no longer on the board.".to_owned());
            }
            return Err("That is not a way to move a task.".to_owned());
        }
    }
    let next = room.clone();
    write_locked(store, &file)?;
    Ok(next)
}

pub fn remove_teammate(store: &Store, teammate_id: &str) -> Result<(), String> {
    if teammate_id.is_empty() {
        return Ok(());
    }
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let mut file = read_locked(store)?;
    let mut changed = false;
    let mut rooms = Vec::with_capacity(file.rooms.len());
    for mut room in file.rooms.drain(..) {
        if !room.teammate_ids.iter().any(|id| id == teammate_id) {
            rooms.push(room);
            continue;
        }
        changed = true;
        room.teammate_ids.retain(|id| id != teammate_id);
        if !room.teammate_ids.is_empty() {
            rooms.push(room);
        }
    }
    if changed {
        file.rooms = rooms;
        write_locked(store, &file)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempStore {
        dir: PathBuf,
        store: Store,
    }

    impl TempStore {
        fn new(label: &str) -> Self {
            let unique = format!(
                "locust-rooms-{label}-{}-{}",
                std::process::id(),
                crate::native::uuid_v4().unwrap_or_else(|_| "0".into())
            );
            let dir = std::env::temp_dir().join(unique);
            let root = dir.join("root");
            let electron = dir.join("electron");
            std::fs::create_dir_all(&root).unwrap();
            std::fs::create_dir_all(&electron).unwrap();
            let store = Store::new(root, electron).unwrap();
            Self { dir, store }
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn roster_entries() -> Vec<RosterEntry<'static>> {
        vec![
            RosterEntry {
                teammate_id: "tm_wren",
                name: "Wren",
            },
            RosterEntry {
                teammate_id: "tm_booty",
                name: "Booty",
            },
            RosterEntry {
                teammate_id: "tm_ash",
                name: "Ash",
            },
        ]
    }

    fn post_with_missions(text: &str, pairs: &[(&str, &str)]) -> NewPostInput {
        let mut missions = BTreeMap::new();
        for (k, v) in pairs {
            missions.insert((*k).to_owned(), (*v).to_owned());
        }
        NewPostInput {
            text: text.to_owned(),
            missions,
            queued: Vec::new(),
            refused: BTreeMap::new(),
            to: Vec::new(),
        }
    }

    // ---- main/room-store.test.ts (12 cases) ----

    #[test]
    fn store_is_named_set_of_teammates_on_disk_with_no_posts_initially() {
        let t = TempStore::new("c1");
        let made = create(
            &t.store,
            "Release",
            &["tm_wren".into(), "tm_booty".into()],
        )
        .unwrap();
        assert_eq!(made.name, "Release");
        assert_eq!(made.teammate_ids, vec!["tm_wren", "tm_booty"]);
        assert!(made.posts.is_empty());

        let renamed = rename(&t.store, &made.room_id, "Release prep").unwrap();
        assert_eq!(renamed.name, "Release prep");
        let fetched = get(&t.store, &made.room_id).unwrap().unwrap();
        assert_eq!(fetched.name, "Release prep");
        assert_eq!(list(&t.store).unwrap().len(), 1);
    }

    #[test]
    fn store_refuses_bad_name_no_teammates_repeated_teammate_or_too_many() {
        let t = TempStore::new("c2");
        assert!(create(&t.store, "   ", &["tm_wren".into()]).is_err());
        assert!(create(&t.store, &"x".repeat(MAX_ROOM_NAME_LENGTH + 1), &["tm_wren".into()]).is_err());
        assert!(create(&t.store, "Empty", &[]).is_err());
        assert!(create(&t.store, "Dupes", &["tm_wren".into(), "tm_wren".into()]).is_err());
        let too_many: Vec<String> = (0..=MAX_ROOM_TEAMMATES)
            .map(|i| format!("tm_{i}"))
            .collect();
        assert!(create(&t.store, "Crowded", &too_many).is_err());

        for i in 0..MAX_ROOMS {
            create(&t.store, &format!("Room {i}"), &["tm_wren".into()]).unwrap();
        }
        let err = create(&t.store, "One too many", &["tm_wren".into()]).unwrap_err();
        assert_eq!(err, format!("Locust keeps at most {MAX_ROOMS} rooms."));
    }

    #[test]
    fn store_records_post_newest_last_and_keeps_last_200() {
        let t = TempStore::new("c3");
        let room = create(&t.store, "Release", &["tm_wren".into()]).unwrap();
        for i in 0..(MAX_ROOM_POSTS + 5) {
            add_post(
                &t.store,
                &room.room_id,
                post_with_missions(&format!("Post {i}"), &[("tm_wren", &format!("mission_{i}"))]),
            )
            .unwrap();
        }
        let stored = get(&t.store, &room.room_id).unwrap().unwrap();
        assert_eq!(stored.posts.len(), MAX_ROOM_POSTS);
        assert_eq!(stored.posts[0].text, "Post 5");
        assert_eq!(
            stored.posts.last().unwrap().text,
            format!("Post {}", MAX_ROOM_POSTS + 4)
        );
    }

    #[test]
    fn store_refuses_empty_post_missing_room_and_drops_malformed_mission_ids() {
        let t = TempStore::new("c4");
        let room = create(&t.store, "Release", &["tm_wren".into()]).unwrap();
        assert!(add_post(&t.store, &room.room_id, post_with_missions("   ", &[])).is_err());
        assert!(add_post(&t.store, "missing", post_with_missions("Hello", &[])).is_err());
        let bad = add_post(
            &t.store,
            &room.room_id,
            post_with_missions(
                "Hello",
                &[
                    ("tm_wren", "mission_ok"),
                    ("../evil", "mission_no"),
                    ("tm_booty", "../evil"),
                ],
            ),
        )
        .unwrap();
        assert_eq!(bad.missions.len(), 1);
        assert_eq!(bad.missions.get("tm_wren").map(String::as_str), Some("mission_ok"));
    }

    #[test]
    fn store_drops_gone_teammate_and_removes_room_left_empty() {
        let t = TempStore::new("c5");
        let both = create(&t.store, "Both", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        let solo = create(&t.store, "Solo", &["tm_wren".into()]).unwrap();
        remove_teammate(&t.store, "tm_wren").unwrap();
        let remaining = list(&t.store).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].room_id, both.room_id);
        assert_eq!(remaining[0].teammate_ids, vec!["tm_booty"]);
        assert!(get(&t.store, &solo.room_id).unwrap().is_none());
    }

    #[test]
    fn store_removes_room_by_id_and_ignores_unknown() {
        let t = TempStore::new("c6");
        let room = create(&t.store, "Release", &["tm_wren".into()]).unwrap();
        remove(&t.store, "unknown").unwrap();
        assert_eq!(list(&t.store).unwrap().len(), 1);
        remove(&t.store, &room.room_id).unwrap();
        assert!(list(&t.store).unwrap().is_empty());
    }

    #[test]
    fn store_treats_corrupt_file_as_unreadable_refuses_overwrite_and_skips_bad_room() {
        let t = TempStore::new("c7");
        let path = t.store.rooms_json_path().unwrap();
        std::fs::write(&path, "{not valid json").unwrap();
        let err = list(&t.store).unwrap_err();
        assert_eq!(err, UNREADABLE);
        let create_err = create(&t.store, "New", &["tm_wren".into()]).unwrap_err();
        assert_eq!(create_err, UNREADABLE);
        let rename_err = rename(&t.store, "r1", "New").unwrap_err();
        assert_eq!(rename_err, UNREADABLE);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{not valid json");

        // Directory instead of file -> UNREADABLE.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir_all(&path).unwrap();
        assert_eq!(rename(&t.store, "r1", "New").unwrap_err(), UNREADABLE);
        std::fs::remove_dir_all(&path).unwrap();

        // Mixed good + malformed room -> bad room skipped, good kept.
        let mixed = json!({
            "schemaVersion": 1,
            "rooms": [
                {"roomId": "good_1", "name": "Good", "teammateIds": ["tm_wren"], "createdAt": "2026-09-24T12:00:00.000Z", "posts": []},
                {"roomId": "bad_1", "name": " ", "teammateIds": []}
            ]
        });
        std::fs::write(&path, mixed.to_string()).unwrap();
        let rooms = list(&t.store).unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].room_id, "good_1");
    }

    #[test]
    fn store_apply_task_ops_moves_claim_done_handoff_new() {
        let t = TempStore::new("c8");
        let room = create(
            &t.store,
            "Release",
            &["tm_wren".into(), "tm_booty".into(), "tm_ash".into()],
        )
        .unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        let booty = TaskActor {
            teammate_id: "tm_booty",
            name: "Booty",
            mission_id: "m_b",
            noted_to: None,
        };

        let r1 = apply_task_ops(
            &t.store,
            &room.room_id,
            &[
                TaskOp::New {
                    text: "Write notes".to_owned(),
                },
                TaskOp::Claim {
                    text: "Tag build".to_owned(),
                },
            ],
            &wren,
            &roster,
        )
        .unwrap();
        assert_eq!(r1.changed.len(), 2);
        assert!(r1.refused.is_empty());

        let r2 = apply_task_ops(
            &t.store,
            &room.room_id,
            &[
                TaskOp::Done {
                    text: "tag build!".to_owned(),
                },
                TaskOp::Handoff {
                    text: "Write notes".to_owned(),
                    to: "Booty".to_owned(),
                },
            ],
            &wren,
            &roster,
        )
        .unwrap();
        assert_eq!(r2.changed.len(), 2);
        let board = get(&t.store, &room.room_id).unwrap().unwrap().tasks;
        assert_eq!(board[0].owner_id.as_deref(), Some("tm_booty"));
        assert_eq!(board[0].state, "open");
        assert_eq!(board[1].state, "done");

        let r3 = apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Claim {
                text: "Write notes".to_owned(),
            }],
            &booty,
            &roster,
        )
        .unwrap();
        assert_eq!(r3.changed, vec!["Booty took on \"Write notes\"."]);
    }

    #[test]
    fn store_apply_task_ops_refuses_invalid_ops_by_name_without_dropping_rest() {
        let t = TempStore::new("c9");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Claim {
                text: "Release notes".to_owned(),
            }],
            &wren,
            &roster,
        )
        .unwrap();
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Done {
                text: "Release notes".to_owned(),
            }],
            &wren,
            &roster,
        )
        .unwrap();

        let res = apply_task_ops(
            &t.store,
            &room.room_id,
            &[
                TaskOp::New {
                    text: "Release notes".to_owned(),
                },
                TaskOp::Claim {
                    text: "Release notes".to_owned(),
                },
                TaskOp::Done {
                    text: "Missing row".to_owned(),
                },
                TaskOp::Handoff {
                    text: "Thing".to_owned(),
                    to: "Ash".to_owned(),
                },
                TaskOp::Handoff {
                    text: "Thing".to_owned(),
                    to: "Wren".to_owned(),
                },
                TaskOp::New {
                    text: "Valid task".to_owned(),
                },
            ],
            &wren,
            &roster,
        )
        .unwrap();
        assert_eq!(res.refused.len(), 5);
        assert!(res.refused[0].contains("already on the board"));
        assert!(res.refused[1].contains("which is done"));
        assert!(res.refused[2].contains("not on the board"));
        assert!(res.refused[3].contains("not in this room"));
        assert!(res.refused[4].contains("to themselves"));
        assert_eq!(res.changed, vec!["Wren added \"Valid task\"."]);
    }

    #[test]
    fn store_update_task_add_assign_finish_reopen_remove() {
        let t = TempStore::new("c10");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        let after_add = update_task(
            &t.store,
            RoomTaskRequest::Add {
                room_id: room.room_id.clone(),
                text: "Deploy".to_owned(),
                owner_id: Some("tm_wren".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(after_add.tasks.len(), 1);
        let tid = after_add.tasks[0].task_id.clone();
        assert_eq!(after_add.tasks[0].state, "in-hand");

        let after_assign = update_task(
            &t.store,
            RoomTaskRequest::Assign {
                room_id: room.room_id.clone(),
                task_id: tid.clone(),
                owner_id: None,
            },
        )
        .unwrap();
        assert_eq!(after_assign.tasks[0].state, "open");

        assert!(
            update_task(
                &t.store,
                RoomTaskRequest::Assign {
                    room_id: room.room_id.clone(),
                    task_id: tid.clone(),
                    owner_id: Some("tm_stranger".to_owned()),
                },
            )
            .is_err()
        );

        let after_done = update_task(
            &t.store,
            RoomTaskRequest::Done {
                room_id: room.room_id.clone(),
                task_id: tid.clone(),
            },
        )
        .unwrap();
        assert_eq!(after_done.tasks[0].state, "done");

        let after_reopen = update_task(
            &t.store,
            RoomTaskRequest::Reopen {
                room_id: room.room_id.clone(),
                task_id: tid.clone(),
            },
        )
        .unwrap();
        assert_eq!(after_reopen.tasks[0].state, "open");

        let after_remove = update_task(
            &t.store,
            RoomTaskRequest::Remove {
                room_id: room.room_id.clone(),
                task_id: tid,
            },
        )
        .unwrap();
        assert!(after_remove.tasks.is_empty());
    }

    #[test]
    fn store_reads_tasks_back_from_disk_and_old_file_without_tasks() {
        let t = TempStore::new("c11");
        let path = t.store.rooms_json_path().unwrap();
        let legacy = json!({
            "schemaVersion": 1,
            "rooms": [{
                "roomId": "room_old",
                "name": "Legacy",
                "teammateIds": ["tm_wren"],
                "createdAt": "2026-09-01T12:00:00.000Z",
                "posts": []
            }]
        });
        std::fs::write(&path, legacy.to_string()).unwrap();
        let old = get(&t.store, "room_old").unwrap().unwrap();
        assert!(old.tasks.is_empty());

        let updated = update_task(
            &t.store,
            RoomTaskRequest::Add {
                room_id: "room_old".to_owned(),
                text: "Keep me".to_owned(),
                owner_id: None,
            },
        )
        .unwrap();
        assert_eq!(updated.tasks.len(), 1);
        let reread = get(&t.store, "room_old").unwrap().unwrap();
        assert_eq!(reread.tasks[0].text, "Keep me");
    }

    #[test]
    fn store_leaves_no_queue_key_in_file_once_last_waiting_member_started() {
        let t = TempStore::new("c12");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        let post = add_post(
            &t.store,
            &room.room_id,
            NewPostInput {
                text: "Check".to_owned(),
                missions: BTreeMap::new(),
                queued: vec!["tm_wren".to_owned(), "tm_booty".to_owned()],
                refused: BTreeMap::new(),
                to: Vec::new(),
            },
        )
        .unwrap();
        start_queued(&t.store, &room.room_id, &post.post_id, "tm_wren", "m_wren").unwrap();
        start_queued(&t.store, &room.room_id, &post.post_id, "tm_booty", "m_booty").unwrap();
        let raw = std::fs::read_to_string(t.store.rooms_json_path().unwrap()).unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert!(v["rooms"][0]["posts"][0].get("queued").is_none());
        assert_eq!(v["rooms"][0]["posts"][0]["missions"]["tm_wren"], "m_wren");
        assert_eq!(v["rooms"][0]["posts"][0]["missions"]["tm_booty"], "m_booty");
    }

    // ---- main/a-room-task-is-never-bounced-back.test.ts (6 store cases; brief rule in room_task) ----

    #[test]
    fn bounce_keeps_room_and_other_tasks_when_one_task_has_unknown_state() {
        let t = TempStore::new("b1");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[
                TaskOp::New {
                    text: "Check the version string".to_owned(),
                },
                TaskOp::New {
                    text: "Sign the installer".to_owned(),
                },
            ],
            &wren,
            &roster,
        )
        .unwrap();
        let path = t.store.rooms_json_path().unwrap();
        let mut raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        raw["rooms"][0]["tasks"][0]["state"] = json!("parked");
        std::fs::write(&path, raw.to_string()).unwrap();

        create(&t.store, "Second", &["tm_wren".into()]).unwrap();
        let kept = get(&t.store, &room.room_id).unwrap().unwrap();
        let texts: Vec<&str> = kept.tasks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["Sign the installer"]);
    }

    #[test]
    fn bounce_cannot_hand_task_straight_back_to_sender() {
        let t = TempStore::new("b2");
        let room = create(
            &t.store,
            "Release",
            &["tm_wren".into(), "tm_booty".into(), "tm_ash".into()],
        )
        .unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        let booty = TaskActor {
            teammate_id: "tm_booty",
            name: "Booty",
            mission_id: "m_b",
            noted_to: None,
        };
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Handoff {
                text: "Check the version string".to_owned(),
                to: "Booty".to_owned(),
            }],
            &wren,
            &roster,
        )
        .unwrap();
        let back = apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Handoff {
                text: "Check the version string".to_owned(),
                to: "wren".to_owned(),
            }],
            &booty,
            &roster,
        )
        .unwrap();
        assert!(back.changed.is_empty());
        assert_eq!(
            back.refused,
            vec![
                "Booty tried to hand \"Check the version string\" back to Wren, who handed it over. It stays with Booty."
            ]
        );
        let task = &get(&t.store, &room.room_id).unwrap().unwrap().tasks[0];
        assert_eq!(task.owner_id.as_deref(), Some("tm_booty"));
        assert_eq!(task.handed_by.as_deref(), Some("tm_wren"));
    }

    #[test]
    fn bounce_can_go_to_third_member_and_rule_follows_latest_hand() {
        let t = TempStore::new("b3");
        let room = create(
            &t.store,
            "Release",
            &["tm_wren".into(), "tm_booty".into(), "tm_ash".into()],
        )
        .unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        let booty = TaskActor {
            teammate_id: "tm_booty",
            name: "Booty",
            mission_id: "m_b",
            noted_to: None,
        };
        let ash = TaskActor {
            teammate_id: "tm_ash",
            name: "Ash",
            mission_id: "m_a",
            noted_to: None,
        };
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Handoff {
                text: "Check the version string".to_owned(),
                to: "Booty".to_owned(),
            }],
            &wren,
            &roster,
        )
        .unwrap();
        let on = apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Handoff {
                text: "Check the version string".to_owned(),
                to: "Ash".to_owned(),
            }],
            &booty,
            &roster,
        )
        .unwrap();
        assert_eq!(on.changed, vec!["Booty handed \"Check the version string\" to Ash."]);
        assert_eq!(
            apply_task_ops(
                &t.store,
                &room.room_id,
                &[TaskOp::Handoff {
                    text: "Check the version string".to_owned(),
                    to: "Booty".to_owned(),
                }],
                &ash,
                &roster,
            )
            .unwrap()
            .refused
            .len(),
            1
        );
        assert_eq!(
            apply_task_ops(
                &t.store,
                &room.room_id,
                &[TaskOp::Handoff {
                    text: "Check the version string".to_owned(),
                    to: "Wren".to_owned(),
                }],
                &ash,
                &roster,
            )
            .unwrap()
            .changed
            .len(),
            1
        );
    }

    #[test]
    fn bounce_clears_after_claim_or_person_reassign() {
        let t = TempStore::new("b4");
        let room = create(
            &t.store,
            "Release",
            &["tm_wren".into(), "tm_booty".into(), "tm_ash".into()],
        )
        .unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        let booty = TaskActor {
            teammate_id: "tm_booty",
            name: "Booty",
            mission_id: "m_b",
            noted_to: None,
        };
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Handoff {
                text: "Check the version string".to_owned(),
                to: "Booty".to_owned(),
            }],
            &wren,
            &roster,
        )
        .unwrap();
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Claim {
                text: "Check the version string".to_owned(),
            }],
            &booty,
            &roster,
        )
        .unwrap();
        assert!(
            get(&t.store, &room.room_id).unwrap().unwrap().tasks[0]
                .handed_by
                .is_none()
        );
        assert_eq!(
            apply_task_ops(
                &t.store,
                &room.room_id,
                &[TaskOp::Handoff {
                    text: "Check the version string".to_owned(),
                    to: "Wren".to_owned(),
                }],
                &booty,
                &roster,
            )
            .unwrap()
            .changed
            .len(),
            1
        );
        let task_id = get(&t.store, &room.room_id).unwrap().unwrap().tasks[0]
            .task_id
            .clone();
        update_task(
            &t.store,
            RoomTaskRequest::Assign {
                room_id: room.room_id.clone(),
                task_id,
                owner_id: Some("tm_booty".to_owned()),
            },
        )
        .unwrap();
        let moved = &get(&t.store, &room.room_id).unwrap().unwrap().tasks[0];
        assert_eq!(moved.owner_id.as_deref(), Some("tm_booty"));
        assert!(moved.handed_by.is_none());
        assert_eq!(
            apply_task_ops(
                &t.store,
                &room.room_id,
                &[TaskOp::Handoff {
                    text: "Check the version string".to_owned(),
                    to: "Wren".to_owned(),
                }],
                &booty,
                &roster,
            )
            .unwrap()
            .changed
            .len(),
            1
        );
    }

    #[test]
    fn bounce_is_not_handed_to_yourself() {
        let t = TempStore::new("b5");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        let self_res = apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Handoff {
                text: "Check the version string".to_owned(),
                to: "Wren".to_owned(),
            }],
            &wren,
            &roster,
        )
        .unwrap();
        assert_eq!(
            self_res.refused,
            vec!["Wren handed \"Check the version string\" to themselves; to take it on, claim it."]
        );
        assert!(get(&t.store, &room.room_id).unwrap().unwrap().tasks.is_empty());
    }

    #[test]
    fn bounce_handed_by_persists_across_reread_from_disk() {
        let t = TempStore::new("b6");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        let roster = roster_entries();
        let wren = TaskActor {
            teammate_id: "tm_wren",
            name: "Wren",
            mission_id: "m_w",
            noted_to: None,
        };
        let booty = TaskActor {
            teammate_id: "tm_booty",
            name: "Booty",
            mission_id: "m_b",
            noted_to: None,
        };
        apply_task_ops(
            &t.store,
            &room.room_id,
            &[TaskOp::Handoff {
                text: "Check the version string".to_owned(),
                to: "Booty".to_owned(),
            }],
            &wren,
            &roster,
        )
        .unwrap();
        let re_store = Store::new(t.store.root().to_path_buf(), t.store.electron().to_path_buf()).unwrap();
        assert_eq!(
            apply_task_ops(
                &re_store,
                &room.room_id,
                &[TaskOp::Handoff {
                    text: "Check the version string".to_owned(),
                    to: "Wren".to_owned(),
                }],
                &booty,
                &roster,
            )
            .unwrap()
            .refused
            .len(),
            1
        );
    }

    // ---- main/a-room-post-asks-who-was-named.test.ts (store/recipients: 6 cases) ----

    #[test]
    fn recipients_is_everyone_when_nobody_named() {
        let room = vec!["tm_wren".into(), "tm_pip".into(), "tm_booty".into()];
        assert_eq!(
            recipients_of(&room, None),
            RecipientsResult::Ok {
                asked: room.clone(),
                to: None
            }
        );
        assert_eq!(
            recipients_of(&room, Some(&[])),
            RecipientsResult::Ok {
                asked: room,
                to: None
            }
        );
    }

    #[test]
    fn recipients_is_only_who_was_named_once_each_in_room_order() {
        let room = vec!["tm_wren".into(), "tm_pip".into(), "tm_booty".into()];
        assert_eq!(
            recipients_of(&room, Some(&["tm_wren".into()])),
            RecipientsResult::Ok {
                asked: vec!["tm_wren".into()],
                to: Some(vec!["tm_wren".into()])
            }
        );
        assert_eq!(
            recipients_of(
                &room,
                Some(&["tm_booty".into(), "tm_wren".into(), "tm_booty".into()])
            ),
            RecipientsResult::Ok {
                asked: vec!["tm_wren".into(), "tm_booty".into()],
                to: Some(vec!["tm_wren".into(), "tm_booty".into()])
            }
        );
    }

    #[test]
    fn recipients_leaves_out_non_member_and_is_everyone_when_all_named() {
        let room = vec!["tm_wren".into(), "tm_pip".into(), "tm_booty".into()];
        assert_eq!(
            recipients_of(
                &room,
                Some(&["tm_wren".into(), "tm_stranger".into(), "../x".into()])
            ),
            RecipientsResult::Ok {
                asked: vec!["tm_wren".into()],
                to: Some(vec!["tm_wren".into()])
            }
        );
        assert_eq!(
            recipients_of(
                &room,
                Some(&["tm_pip".into(), "tm_booty".into(), "tm_wren".into()])
            ),
            RecipientsResult::Ok {
                asked: room,
                to: None
            }
        );
    }

    #[test]
    fn recipients_refused_never_widened_when_nobody_named_is_in_room() {
        let room = vec!["tm_wren".into(), "tm_pip".into(), "tm_booty".into()];
        match recipients_of(&room, Some(&["tm_stranger".into()])) {
            RecipientsResult::Refused { message } => {
                assert!(message.contains("nothing was posted"));
            }
            RecipientsResult::Ok { .. } => panic!("expected refusal"),
        }
    }

    #[test]
    fn post_to_keeps_members_only_and_reads_back() {
        let t = TempStore::new("to1");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_pip".into()]).unwrap();
        let post = add_post(
            &t.store,
            &room.room_id,
            NewPostInput {
                text: "Say more.".to_owned(),
                missions: BTreeMap::new(),
                queued: vec!["tm_wren".to_owned()],
                refused: BTreeMap::new(),
                to: vec![
                    "tm_wren".to_owned(),
                    "tm_stranger".to_owned(),
                    "tm_wren".to_owned(),
                ],
            },
        )
        .unwrap();
        assert_eq!(post.to, vec!["tm_wren"]);
        let everyone = add_post(
            &t.store,
            &room.room_id,
            post_with_missions("All of you.", &[]),
        )
        .unwrap();
        assert!(everyone.to.is_empty());
        let reread = get(&t.store, &room.room_id).unwrap().unwrap();
        assert_eq!(reread.posts[0].to, vec!["tm_wren"]);
        assert!(reread.posts[1].to.is_empty());
    }

    #[test]
    fn post_to_malformed_reads_as_everyone_and_keeps_post() {
        let t = TempStore::new("to2");
        let room = create(&t.store, "Release", &["tm_wren".into(), "tm_pip".into()]).unwrap();
        add_post(
            &t.store,
            &room.room_id,
            NewPostInput {
                text: "Say more.".to_owned(),
                missions: BTreeMap::from([("tm_wren".to_owned(), "mission_w".to_owned())]),
                queued: Vec::new(),
                refused: BTreeMap::new(),
                to: vec!["tm_wren".to_owned()],
            },
        )
        .unwrap();
        let path = t.store.rooms_json_path().unwrap();
        let mut stored: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(stored["rooms"][0]["posts"][0]["to"], json!(["tm_wren"]));
        stored["rooms"][0]["posts"][0]["to"] = json!("tm_wren");
        std::fs::write(&path, stored.to_string()).unwrap();

        let read = get(&t.store, &room.room_id).unwrap().unwrap();
        assert_eq!(read.posts.len(), 1);
        assert!(read.posts[0].to.is_empty());
        assert_eq!(
            read.posts[0].missions.get("tm_wren").map(String::as_str),
            Some("mission_w")
        );
    }

    // ---- main/room-and-mission-caps-agree.test.ts (2 cases) ----

    #[test]
    fn room_limit_and_live_mission_cap_are_real_numbers() {
        assert!(MAX_ROOM_TEAMMATES > 1);
        assert!(crate::native::MAX_LIVE_TURNS > 1);
    }

    #[test]
    fn room_limit_and_live_mission_cap_agree() {
        assert!(MAX_ROOM_TEAMMATES <= crate::native::MAX_LIVE_TURNS);
    }

    /// Review (13 Nov): removing a teammate from the team takes it out of its rooms too.
    #[test]
    fn a_teammate_removed_from_the_team_leaves_its_rooms() {
        let t = TempStore::new("c-people");
        let room = create(&t.store, "Both", &["tm_wren".into(), "tm_booty".into()]).unwrap();
        crate::people::remove_teammate(&t.store, "tm_wren").unwrap();
        assert_eq!(get(&t.store, &room.room_id).unwrap().unwrap().teammate_ids, vec!["tm_booty"]);
    }
}
