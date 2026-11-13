//! Rooms UI (`RoomScreen.tsx` & `roomMentions.ts`): create/rename/remove a room,
//! post to everyone or `@` targeted teammates, view whole answers stacked or side
//! by side with waiting/absent lines, Reply/Build on/Merge/Open actions, and the
//! shared task board.

use super::*;
use crate::look::controls::{Look, Tone, button, chip, label};
use crate::look::faces::face;
use chrono::{DateTime, Utc};
use locust_core::ledger::{self, Item};
use locust_core::room_store::{
    self, MAX_ROOM_TEAMMATES, PublicRoom, RoomPost, RoomTask, RoomTaskRequest,
};
use locust_core::room_task::strip_task_blocks;
use locust_core::rooms::{self, RoomPostOutcome};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, Instant};

/// Past this many rendered answers under one post, switch from grid/cards to a compact list.
pub const ANSWERS_BEFORE_A_LIST: usize = 6;
/// Wait this long without visible output before adding "· no word back yet" on a live ask.
const NO_WORD_BACK_SECS: i64 = 20;
/// Width floor per answer column when side-by-side mode is enabled.
pub const SIDE_BY_SIDE_COL_MIN_W: f32 = 320.0;
const MERGE_DRAFT: &str = "Merge the answers to my last post into one: keep what they agree on, say plainly where they differ, and name whose each part was.";

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(super) enum RoomView {
    #[default]
    None,
    List,
    New,
    Room(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct RoomTranscriptView {
    pub phase: String,
    pub answer_text: String,
    pub note_error: Option<String>,
    pub started_at: String,
    pub runtime: String,
    pub model: String,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RoomsSnapshot {
    pub rooms: Vec<PublicRoom>,
    pub readable: bool,
    pub transcripts: HashMap<String, RoomTranscriptView>,
}

#[derive(Clone, Debug)]
pub(super) enum RoomActionOutcome {
    Created {
        room: PublicRoom,
        snapshot: RoomsSnapshot,
    },
    Renamed {
        snapshot: RoomsSnapshot,
    },
    Removed {
        snapshot: RoomsSnapshot,
    },
    Posted {
        outcome: RoomPostOutcome,
        snapshot: RoomsSnapshot,
    },
    TaskUpdated {
        snapshot: RoomsSnapshot,
    },
}

#[derive(Default)]
pub(super) struct RoomsState {
    pub active: RoomView,
    pub rooms: Vec<PublicRoom>,
    pub readable: bool,
    pub transcripts: HashMap<String, RoomTranscriptView>,
    pub load_job: Latest<Result<RoomsSnapshot, String>>,
    pub action_job: Latest<Result<RoomActionOutcome, String>>,
    pub last_refresh: Option<Instant>,
    pub create_name: TextEdit,
    pub create_picks: Vec<String>,
    pub create_error: Option<String>,
    pub draft: TextEdit,
    pub ask_to: Vec<String>,
    pub at_dismissed: Option<String>,
    pub side_by_side: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub menu_open: bool,
    pub confirming_remove: bool,
    pub renaming: bool,
    pub rename_draft: TextEdit,
    pub open_task_menu: Option<String>,
    pub assigning_task: Option<String>,
    pub adding_task: bool,
    pub new_task_draft: TextEdit,
}

impl RoomsState {
    pub fn new() -> Self {
        Self {
            readable: true,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionMember {
    pub teammate_id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionMatch {
    pub start: usize,
    pub query: String,
}

/// Find an active `@name` mention right before `caret` (byte index clamped cleanly to UTF-8).
pub fn mention_at(text: &str, caret: usize) -> Option<MentionMatch> {
    let mut end = caret.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let before = &text[..end];
    let at = before.rfind('@')?;
    if at > 0 && !before[..at].chars().next_back().is_some_and(char::is_whitespace) {
        return None;
    }
    let query = &before[at + 1..];
    if query.chars().any(char::is_whitespace) {
        return None;
    }
    Some(MentionMatch {
        start: at,
        query: query.to_owned(),
    })
}

/// Room members not yet targeted whose names start with `query`, in room order.
pub fn mention_choices<'a>(
    members: &'a [MentionMember],
    ask_to: &[String],
    query: &str,
) -> Vec<&'a MentionMember> {
    let q = query.trim().to_lowercase();
    members
        .iter()
        .filter(|m| !ask_to.iter().any(|id| id == &m.teammate_id))
        .filter(|m| m.name.to_lowercase().starts_with(&q))
        .collect()
}

/// Strip the matched `@query` word, keeping clean spacing around it.
pub fn without_mention(text: &str, m: &MentionMatch) -> (String, usize) {
    let after_idx = (m.start + 1 + m.query.len()).min(text.len());
    let before = &text[..m.start];
    let after = &text[after_idx..];
    let next = if (before.ends_with(' ') && after.starts_with(' '))
        || (before.is_empty() && after.starts_with(' '))
    {
        format!("{before}{}", &after[1..])
    } else {
        format!("{before}{after}")
    };
    (next, before.len())
}

/// Complete an exact `@Name ` when a space follows a full member name.
pub fn mention_completed<'a>(
    members: &'a [MentionMember],
    ask_to: &[String],
    text: &str,
    caret: usize,
) -> Option<(&'a MentionMember, MentionMatch)> {
    let mut end = caret.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 || !text[..end].ends_with(' ') {
        return None;
    }
    let mention = mention_at(text, end - 1)?;
    if mention.query.is_empty() {
        return None;
    }
    let q = mention.query.to_lowercase();
    let member = members
        .iter()
        .find(|m| !ask_to.iter().any(|id| id == &m.teammate_id) && m.name.to_lowercase() == q)?;
    Some((member, mention))
}

/// Format names as prose (`"A"`, `"A and B"`, `"A, B and C"`).
pub fn names_said(names: &[String]) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        _ => format!(
            "{} and {}",
            names[..names.len() - 1].join(", "),
            names[names.len() - 1]
        ),
    }
}

/// Amber warning when the user ticks more teammates than a room can hold.
pub fn room_full_note(picked: usize) -> Option<String> {
    if picked <= MAX_ROOM_TEAMMATES {
        return None;
    }
    Some(format!(
        "A room holds {MAX_ROOM_TEAMMATES} teammates. Untick {} to make this one.",
        picked - MAX_ROOM_TEAMMATES
    ))
}

/// Reason a room cannot be created yet, if any.
pub fn room_blocked_reason(name: &str, picked: usize) -> Option<String> {
    if let Some(full) = room_full_note(picked) {
        return Some(full);
    }
    if name.trim().is_empty() {
        return Some("Name the room to create it.".to_owned());
    }
    if picked == 0 {
        return Some("Pick at least one teammate.".to_owned());
    }
    None
}

/// Sidebar / list meta line (`"Atlas · Quill · 1 post · 2m"`, `"Atlas · Quill +2 · nothing posted yet · 1h"`).
pub fn room_card_meta(room: &PublicRoom, roster: &[Teammate], now_ms: i64) -> String {
    let mut member_names: Vec<String> = room
        .teammate_ids
        .iter()
        .take(2)
        .map(|id| {
            roster
                .iter()
                .find(|t| &t.id == id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "Former teammate".to_owned())
        })
        .collect();
    if room.teammate_ids.len() > 2 && let Some(second) = member_names.last_mut() {
        *second = format!("{second} +{}", room.teammate_ids.len() - 2);
    }
    let who = if member_names.is_empty() {
        "No teammates".to_owned()
    } else {
        member_names.join(" · ")
    };
    let count = match room.posts.len() {
        0 => "nothing posted yet".to_owned(),
        1 => "1 post".to_owned(),
        n => format!("{n} posts"),
    };
    let latest_iso = room.posts.last().map(|p| p.at.as_str()).unwrap_or(&room.created_at);
    let age = crate::rail::short_ago(Some(latest_iso), now_ms).unwrap_or_default();
    if age.is_empty() {
        format!("{who} · {count}")
    } else {
        format!("{who} · {count} · {age}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbsentMember {
    pub name: String,
    pub reason: Option<String>,
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!("{}{}", first.to_lowercase(), chars.as_str())
}

/// Combine missed/unstarted members into one shared line grouped by recorded refusal reason.
pub fn absent_line(absent: &[AbsentMember]) -> Option<String> {
    if absent.is_empty() {
        return None;
    }
    let mut order: Vec<String> = Vec::new();
    let mut by_reason: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in absent {
        let key = entry.reason.clone().unwrap_or_default();
        if !by_reason.contains_key(&key) {
            order.push(key.clone());
        }
        by_reason.entry(key).or_default().push(entry.name.clone());
    }
    let sentences: Vec<String> = order
        .into_iter()
        .map(|key| {
            let names = &by_reason[&key];
            let verb = if names.len() == 1 { "was" } else { "were" };
            let joined = names_said(names);
            if key.is_empty() {
                format!("{joined} {verb} not asked.")
            } else {
                format!("{joined} {verb} not asked — {}", lowercase_first(&key))
            }
        })
        .collect();
    Some(sentences.join(" "))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusedMember {
    pub name: String,
    pub message: String,
}

/// Group start refusals by message so repeated cap reasons are said once.
pub fn refusal_notice(refused: &[RefusedMember]) -> Option<String> {
    if refused.is_empty() {
        return None;
    }
    let mut order: Vec<String> = Vec::new();
    let mut by_msg: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in refused {
        if !by_msg.contains_key(&entry.message) {
            order.push(entry.message.clone());
        }
        by_msg
            .entry(entry.message.clone())
            .or_default()
            .push(entry.name.clone());
    }
    let parts: Vec<String> = order
        .into_iter()
        .map(|msg| format!("{}: {msg}", by_msg[&msg].join(", ")))
        .collect();
    Some(parts.join(" · "))
}

pub fn waiting_line(names: &[String]) -> String {
    names.join(", ")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberPostState {
    Completed,
    Running { replied: bool },
    Failed,
    Queued,
    Absent,
    NotAsked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostMemberStatus {
    pub teammate_id: String,
    pub name: String,
    pub state: MemberPostState,
}

/// Summarize one post's statuses (`"3 asked · all answered"`, `"2 asked · Wren running"`).
pub fn post_headline(members: &[PostMemberStatus]) -> String {
    let active: Vec<&PostMemberStatus> = members
        .iter()
        .filter(|m| m.state != MemberPostState::NotAsked)
        .collect();
    if active.is_empty() {
        return "0 asked".to_owned();
    }
    let mut parts = vec![format!("{} asked", active.len())];
    let done = active
        .iter()
        .filter(|m| m.state == MemberPostState::Completed)
        .count();
    let replied: Vec<String> = active
        .iter()
        .filter(|m| m.state == MemberPostState::Running { replied: true })
        .map(|m| m.name.clone())
        .collect();
    let running: Vec<String> = active
        .iter()
        .filter(|m| m.state == MemberPostState::Running { replied: false })
        .map(|m| m.name.clone())
        .collect();
    let failed: Vec<String> = active
        .iter()
        .filter(|m| m.state == MemberPostState::Failed)
        .map(|m| m.name.clone())
        .collect();
    let queued = active
        .iter()
        .filter(|m| m.state == MemberPostState::Queued)
        .count();

    if done == active.len() {
        parts.push("all answered".to_owned());
    } else if done > 0 {
        parts.push(format!("{done} answered"));
    }
    if !replied.is_empty() {
        let who = if replied.len() <= 2 {
            names_said(&replied)
        } else {
            replied.len().to_string()
        };
        parts.push(format!("{who} replied, still working"));
    }
    if !running.is_empty() {
        let who = if running.len() <= 2 {
            names_said(&running)
        } else {
            running.len().to_string()
        };
        parts.push(format!("{who} running"));
    }
    if !failed.is_empty() {
        let who = if failed.len() <= 2 {
            names_said(&failed)
        } else {
            failed.len().to_string()
        };
        parts.push(format!("{who} failed"));
    }
    if queued > 0 {
        parts.push(format!("{queued} waiting for a slot"));
    }
    parts.join(" · ")
}

/// Next composer `(ask_to, draft)` when clicking Build on (clears recipients, writes prompt, never sends).
pub fn build_on_draft(name: &str) -> (Vec<String>, String) {
    (Vec::new(), format!("Build on {name}\u{2019}s answer above: "))
}

/// Next composer `(ask_to, draft)` when clicking Merge (targets one member, writes merge prompt, never sends).
pub fn merge_by_draft(teammate_id: &str) -> (Vec<String>, String) {
    (vec![teammate_id.to_owned()], MERGE_DRAFT.to_owned())
}

/// Close Rooms when another primary view takes the main canvas.
pub(super) fn close(s: &mut State) {
    s.rooms.active = RoomView::None;
    s.rooms.menu_open = false;
    s.rooms.confirming_remove = false;
    s.rooms.renaming = false;
    s.rooms.open_task_menu = None;
    s.rooms.assigning_task = None;
}

/// Open the Rooms list view cleanly.
pub(super) fn open_list(s: &mut State) {
    clear_other_panels(s);
    s.rooms.active = RoomView::List;
    s.rooms.menu_open = false;
    s.rooms.confirming_remove = false;
    s.rooms.renaming = false;
    refresh(s);
}

/// Open the Create room screen cleanly.
pub(super) fn open_new(s: &mut State) {
    clear_other_panels(s);
    s.rooms.active = RoomView::New;
    s.rooms.create_name.replace_text("");
    s.rooms.create_picks.clear();
    s.rooms.create_error = None;
    s.rooms.menu_open = false;
    s.rooms.confirming_remove = false;
    s.rooms.renaming = false;
}

/// Open a specific room by ID cleanly.
pub(super) fn open_room(s: &mut State, room_id: String) {
    clear_other_panels(s);
    s.rooms.active = RoomView::Room(room_id);
    s.rooms.menu_open = false;
    s.rooms.confirming_remove = false;
    s.rooms.renaming = false;
    s.rooms.open_task_menu = None;
    s.rooms.assigning_task = None;
    s.rooms.error = None;
    refresh(s);
}

fn clear_other_panels(s: &mut State) {
    s.benchmarks_open = false;
    s.compare.active_id = None;
    s.home.home_open = false;
    s.home.board_open = false;
    s.home.boot_open = false;
    s.people_panel.open = false;
    s.settings_panel.open = false;
    s.routines.open = false;
    s.changes.open = false;
}

/// Handle Escape while a Room screen is active. Returns true if it closed a menu or edit state.
pub(super) fn handle_escape(s: &mut State) -> bool {
    if s.rooms.active == RoomView::None {
        return false;
    }
    if s.rooms.open_task_menu.is_some() || s.rooms.assigning_task.is_some() {
        s.rooms.open_task_menu = None;
        s.rooms.assigning_task = None;
        return true;
    }
    if s.rooms.menu_open {
        s.rooms.menu_open = false;
        s.rooms.confirming_remove = false;
        return true;
    }
    if s.rooms.renaming {
        s.rooms.renaming = false;
        return true;
    }
    if s.rooms.adding_task {
        s.rooms.adding_task = false;
        return true;
    }
    if let Some(m) = mention_at(&s.rooms.draft.text, s.rooms.draft.text.len()) {
        s.rooms.at_dismissed = Some(format!("{}@{}", m.start, m.query));
        return true;
    }
    false
}

/// Load `rooms.json` and read transcripts for every run off the UI thread.
pub(super) fn load_snapshot_for(store: &locust_core::store::Store, live_ids: &[String]) -> Result<RoomsSnapshot, String> {
    let (rooms, readable) = match room_store::list(store) {
        Ok(list) => (list, true),
        Err(e) if e == room_store::UNREADABLE => (Vec::new(), false),
        Err(e) => return Err(e),
    };
    let live_set: HashSet<&str> = live_ids.iter().map(String::as_str).collect();
    let mut mission_ids: HashSet<String> = HashSet::new();
    for room in &rooms {
        for post in &room.posts {
            for mid in post.missions.values() {
                mission_ids.insert(mid.clone());
            }
        }
    }
    let mut transcripts = HashMap::new();
    let ledger_dir = store.ledger_dir().ok();
    for mid in mission_ids {
        let Some(ref dir) = ledger_dir else {
            continue;
        };
        let path = dir.join(format!("{mid}.jsonl"));
        if !path.is_file() {
            continue;
        }
        let tr = ledger::read_transcript(&path);
        let is_live = live_set.contains(mid.as_str()) || tr.running;
        let phase = if is_live {
            "running"
        } else if tr.failed {
            "failed"
        } else {
            "completed"
        }
        .to_owned();
        let answer_text = tr
            .items
            .iter()
            .rev()
            .find_map(|item| {
                let Item::Assistant { text, .. } = item else {
                    return None;
                };
                let stripped = strip_task_blocks(text);
                (!stripped.trim().is_empty()).then_some(stripped)
            })
            .unwrap_or_default();
        let note_error = if phase == "failed" && answer_text.is_empty() {
            tr.items.iter().rev().find_map(|item| {
                let Item::Note { level, text } = item else {
                    return None;
                };
                (level == "error" || level == "warning").then(|| text.clone())
            })
        } else {
            None
        };
        let (started_at, runtime, model) = match tr.metadata {
            Some(m) => (m.created_at, m.runtime, m.model),
            None => (String::new(), String::new(), String::new()),
        };
        transcripts.insert(
            mid,
            RoomTranscriptView {
                phase,
                answer_text,
                note_error,
                started_at,
                runtime,
                model,
            },
        );
    }
    Ok(RoomsSnapshot {
        rooms,
        readable,
        transcripts,
    })
}

/// Trigger a background reload of `rooms.json` and active mission transcripts.
pub(super) fn refresh(s: &mut State) {
    let Some(store) = s.data.clone() else {
        return;
    };
    if s.rooms.load_job.is_pending() {
        return;
    }
    let live_ids = s.seen_live.clone();
    s.rooms.last_refresh = Some(Instant::now());
    s.rooms
        .load_job
        .request((), move |()| load_snapshot_for(&store, &live_ids));
}

fn apply_snapshot(s: &mut State, snap: RoomsSnapshot) {
    s.rooms.readable = snap.readable;
    s.rooms.rooms = snap.rooms;
    s.rooms.transcripts = snap.transcripts;
    if let RoomView::Room(id) = &s.rooms.active
        && !s.rooms.rooms.iter().any(|r| &r.room_id == id)
    {
        s.rooms.active = RoomView::None;
    }
}

/// Sync background loads, actions, engine room notices, and elapsed timers for active runs.
pub(super) fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let mut changed = false;
    let mut need_refresh = false;

    let notices = locust_core::engine_api::api().room_notices();
    if !notices.is_empty() {
        let msgs: Vec<String> = notices
            .into_iter()
            .filter_map(|n| (!n.message.trim().is_empty()).then_some(n.message))
            .collect();
        if !msgs.is_empty() {
            s.rooms.notice = Some(msgs.join(" · "));
        }
        need_refresh = true;
        changed = true;
    }

    if let Some(res) = s.rooms.load_job.take() {
        changed = true;
        match res {
            Ok(snap) => {
                apply_snapshot(s, snap);
                s.rooms.error = None;
            }
            Err(e) => s.rooms.error = Some(e),
        }
    }

    if let Some(res) = s.rooms.action_job.take() {
        changed = true;
        match res {
            Ok(RoomActionOutcome::Created { room, snapshot }) => {
                apply_snapshot(s, snapshot);
                s.rooms.create_error = None;
                s.rooms.create_name.replace_text("");
                s.rooms.create_picks.clear();
                s.rooms.active = RoomView::Room(room.room_id);
            }
            Ok(RoomActionOutcome::Renamed { snapshot }) => {
                apply_snapshot(s, snapshot);
                s.rooms.renaming = false;
                s.rooms.error = None;
            }
            Ok(RoomActionOutcome::Removed { snapshot }) => {
                apply_snapshot(s, snapshot);
                s.rooms.menu_open = false;
                s.rooms.confirming_remove = false;
                s.rooms.active = RoomView::None;
            }
            Ok(RoomActionOutcome::Posted { outcome, snapshot }) => {
                apply_snapshot(s, snapshot);
                s.rooms.draft.replace_text("");
                s.rooms.ask_to.clear();
                s.rooms.at_dismissed = None;
                s.rooms.error = None;
                s.rooms.notice = outcome.refusal_message;
            }
            Ok(RoomActionOutcome::TaskUpdated { snapshot }) => {
                apply_snapshot(s, snapshot);
                s.rooms.open_task_menu = None;
                s.rooms.assigning_task = None;
                s.rooms.adding_task = false;
                s.rooms.new_task_draft.replace_text("");
                s.rooms.error = None;
            }
            Err(e) => {
                if s.rooms.active == RoomView::New {
                    s.rooms.create_error = Some(e);
                } else {
                    s.rooms.error = Some(e);
                }
            }
        }
    }

    let viewing_live = if let RoomView::Room(id) = &s.rooms.active {
        s.rooms.rooms.iter().find(|r| &r.room_id == id).is_some_and(|room| {
            room.posts.iter().any(|post| {
                !post.queued.is_empty()
                    || post.missions.values().any(|mid| {
                        s.seen_live.contains(mid)
                            || s.rooms
                                .transcripts
                                .get(mid)
                                .is_none_or(|t| t.phase == "running")
                    })
            })
        })
    } else {
        false
    };

    if viewing_live {
        let stale = s
            .rooms
            .last_refresh
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(1));
        if stale {
            need_refresh = true;
        }
        tx.wake_after(Duration::from_millis(500));
    }

    if need_refresh {
        refresh(s);
    }

    changed
}

pub(super) fn submit_create(s: &mut State) {
    let Some(store) = s.data.clone() else {
        return;
    };
    let name = s.rooms.create_name.text.trim().to_owned();
    let picks = s.rooms.create_picks.clone();
    if let Some(reason) = room_blocked_reason(&name, picks.len()) {
        s.rooms.create_error = Some(reason);
        return;
    }
    if s.rooms.action_job.is_pending() {
        return;
    }
    let live_ids = s.seen_live.clone();
    s.rooms.action_job.request((), move |()| {
        let room = room_store::create(&store, &name, &picks)?;
        let snapshot = load_snapshot_for(&store, &live_ids)?;
        Ok(RoomActionOutcome::Created { room, snapshot })
    });
}

pub(super) fn submit_rename(s: &mut State, room_id: String) {
    let Some(store) = s.data.clone() else {
        return;
    };
    let name = s.rooms.rename_draft.text.trim().to_owned();
    if !room_store::valid_room_name(&name) {
        s.rooms.error = Some("Give the room a name.".to_owned());
        return;
    }
    if s.rooms.action_job.is_pending() {
        return;
    }
    let live_ids = s.seen_live.clone();
    s.rooms.action_job.request((), move |()| {
        room_store::rename(&store, &room_id, &name)?;
        let snapshot = load_snapshot_for(&store, &live_ids)?;
        Ok(RoomActionOutcome::Renamed { snapshot })
    });
}

pub(super) fn submit_remove(s: &mut State, room_id: String) {
    let Some(store) = s.data.clone() else {
        return;
    };
    if s.rooms.action_job.is_pending() {
        return;
    }
    let live_ids = s.seen_live.clone();
    s.rooms.action_job.request((), move |()| {
        room_store::remove(&store, &room_id)?;
        let snapshot = load_snapshot_for(&store, &live_ids)?;
        Ok(RoomActionOutcome::Removed { snapshot })
    });
}

pub(super) fn submit_post(s: &mut State) {
    let Some(store) = s.data.clone() else {
        return;
    };
    let RoomView::Room(room_id) = s.rooms.active.clone() else {
        return;
    };
    let text = s.rooms.draft.text.clone();
    if text.trim().is_empty() || s.rooms.action_job.is_pending() {
        return;
    }
    let to = (!s.rooms.ask_to.is_empty()).then(|| s.rooms.ask_to.clone());
    let live_ids = s.seen_live.clone();
    s.rooms.notice = None;
    s.rooms.error = None;
    s.rooms.action_job.request((), move |()| {
        let outcome = rooms::post_to_room(&store, &room_id, &text, to.as_deref())?;
        let snapshot = load_snapshot_for(&store, &live_ids)?;
        Ok(RoomActionOutcome::Posted { outcome, snapshot })
    });
}

pub(super) fn submit_task(s: &mut State, request: RoomTaskRequest) {
    let Some(store) = s.data.clone() else {
        return;
    };
    if s.rooms.action_job.is_pending() {
        return;
    }
    let live_ids = s.seen_live.clone();
    s.rooms.action_job.request((), move |()| {
        room_store::update_task(&store, request)?;
        let snapshot = load_snapshot_for(&store, &live_ids)?;
        Ok(RoomActionOutcome::TaskUpdated { snapshot })
    });
}

fn elapsed_seconds(started_at: &str, post_at: &str, now_ms: i64) -> i64 {
    let iso = if started_at.is_empty() {
        post_at
    } else {
        started_at
    };
    let Ok(dt) = DateTime::parse_from_rfc3339(iso) else {
        return 0;
    };
    ((now_ms - dt.timestamp_millis()) / 1000).max(0)
}

fn roster_member_name(roster: &[Teammate], id: &str) -> String {
    roster
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.name.clone())
        .unwrap_or_else(|| "Former teammate".to_owned())
}

fn roster_model_line(tm: Option<&Teammate>, tr: Option<&RoomTranscriptView>) -> String {
    let runtime = tr
        .filter(|t| !t.runtime.is_empty())
        .map(|t| t.runtime.as_str())
        .or_else(|| tm.and_then(|m| m.runtime.as_deref()))
        .unwrap_or("");
    let model = tr
        .filter(|t| !t.model.is_empty())
        .map(|t| t.model.as_str())
        .or_else(|| tm.and_then(|m| m.model.as_deref()))
        .unwrap_or("default");
    if runtime.is_empty() {
        model.to_owned()
    } else {
        format!("{runtime}/{model}")
    }
}

fn on_room_draft_edit(s: &mut State, room_members: &[MentionMember]) {
    s.rooms.at_dismissed = None;
    let caret = s.rooms.draft.text.len();
    if let Some((member, m)) =
        mention_completed(room_members, &s.rooms.ask_to, &s.rooms.draft.text, caret)
    {
        let (next, _) = without_mention(&s.rooms.draft.text, &m);
        if !s.rooms.ask_to.contains(&member.teammate_id) {
            s.rooms.ask_to.push(member.teammate_id.clone());
        }
        s.rooms.draft.replace_text(&next);
    }
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    match &s.rooms.active {
        RoomView::New => view_create(s, p, &l),
        RoomView::Room(id) => {
            if let Some(room) = s.rooms.rooms.iter().find(|r| &r.room_id == id) {
                view_room(s, room, p, &l)
            } else {
                view_list(s, p, &l)
            }
        }
        RoomView::List | RoomView::None => view_list(s, p, &l),
    }
}

fn view_list(s: &State, p: &Palette, l: &Look) -> El {
    let now_ms = Utc::now().timestamp_millis();
    let mut root = col().key("rooms-screen").grow(1.0).h_full().fill(p.bg);
    root = root.child(
        row()
            .w_full()
            .px(20.0)
            .py(14.0)
            .gap(12.0)
            .center_y()
            .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
            .child(
                text("Rooms")
                    .font(rune::TextStyle::new(16.0, 600.0))
                    .color(p.text)
                    .grow(1.0),
            )
            .child(
                button(l, "New room", Tone::Active, false)
                    .key("rooms-list-new")
                    .on_press(|s: &mut State| open_new(s)),
            ),
    );
    let mut body = col().w_full().gap(10.0).padding(20.0);
    if !s.rooms.readable {
        body = body.child(
            text(room_store::UNREADABLE)
                .font(l.ty.label)
                .color(p.amber)
                .wrap(),
        );
    }
    if s.rooms.rooms.is_empty() {
        body = body.child(
            text("No rooms yet. The first one you make appears in the sidebar under its own name.")
                .font(l.ty.body)
                .color(p.muted)
                .wrap(),
        );
    } else {
        for room in &s.rooms.rooms {
            let rid = room.room_id.clone();
            let meta = room_card_meta(room, &s.teammates, now_ms);
            body = body.child(
                row()
                    .key(format!("room-list-card-{}", room.room_id))
                    .w_full()
                    .padding(14.0)
                    .gap(12.0)
                    .center_y()
                    .radius(8.0)
                    .fill(p.raised)
                    .border(1.0, p.hairline)
                    .hover(l.c("hover-surface"))
                    .role("button")
                    .label(room.name.clone())
                    .child(
                        col()
                            .grow(1.0)
                            .gap(2.0)
                            .child(text(room.name.clone()).font(type_::TITLE).color(p.text))
                            .child(text(meta).font(l.ty.label).color(p.muted).truncate()),
                    )
                    .on_press(move |s: &mut State| open_room(s, rid.clone())),
            );
        }
    }
    root.child(
        col()
            .grow(1.0)
            .w_full()
            .scroll(ScrollSpec { stick_bottom: false })
            .child(body),
    )
}

fn view_create(s: &State, p: &Palette, l: &Look) -> El {
    let picked = s.rooms.create_picks.len();
    let total = s.teammates.len();
    let plural = if total == 1 { "" } else { "s" };
    let blocked = room_blocked_reason(&s.rooms.create_name.text, picked);
    let full_note = room_full_note(picked);

    let form = col()
        .key("room-create-screen")
        .grow(1.0)
        .h_full()
        .fill(p.bg)
        .child(
            row()
                .w_full()
                .px(20.0)
                .py(14.0)
                .gap(12.0)
                .center_y()
                .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
                .child(
                    text("New room")
                        .font(rune::TextStyle::new(16.0, 600.0))
                        .color(p.text)
                        .grow(1.0),
                )
                .child(
                    button(l, "Cancel", Tone::Neutral, false)
                        .key("room-create-cancel")
                        .on_press(|s: &mut State| close(s)),
                ),
        );

    let mut body = col()
        .w_full()
        .max_w(640.0)
        .gap(14.0)
        .padding(20.0)
        .child(label(l, "Name"))
        .child(
            text_input(&s.rooms.create_name, type_::BODY)
                .key("room-create-name")
                .single_line()
                .w_full()
                .padding(10.0)
                .radius(8.0)
                .fill(p.field)
                .border(1.0, p.hairline)
                .color(p.text)
                .placeholder("Room name", p.muted)
                .on_edit(|s: &mut State| &mut s.rooms.create_name)
                .on_submit(|s: &mut State, _| submit_create(s)),
        )
        .child(
            text(format!(
                "{picked} of {total} teammate{plural} · each answers on its own model"
            ))
            .font(l.ty.label)
            .color(p.muted),
        );

    let mut picker = col().w_full().gap(6.0);
    for tm in &s.teammates {
        let tid = tm.id.clone();
        let is_picked = s.rooms.create_picks.contains(&tid);
        picker = picker.child(
            row()
                .key(format!("room-create-pick-{}", tm.id))
                .w_full()
                .px(12.0)
                .py(9.0)
                .gap(10.0)
                .center_y()
                .radius(8.0)
                .fill(if is_picked { p.selected } else { p.raised })
                .border(1.0, p.hairline)
                .hover(l.c("hover-surface"))
                .role("checkbox")
                .label(tm.name.clone())
                .child(
                    text(if is_picked { "✓" } else { "○" })
                        .font(l.ty.label)
                        .color(if is_picked { p.green } else { p.muted }),
                )
                .child(face(
                    s.image_cache.as_deref(),
                    &tm.id,
                    &tm.name,
                    s.looks.get(&tm.id),
                    24.0,
                    l,
                ))
                .child(
                    col()
                        .grow(1.0)
                        .child(text(tm.name.clone()).font(l.ty.body).color(p.text))
                        .child(text(tm.role.clone()).font(l.ty.label).color(p.muted)),
                )
                .on_press(move |s: &mut State| {
                    if let Some(pos) = s.rooms.create_picks.iter().position(|id| id == &tid) {
                        s.rooms.create_picks.remove(pos);
                    } else {
                        s.rooms.create_picks.push(tid.clone());
                    }
                    s.rooms.create_error = None;
                }),
        );
    }
    body = body.child(picker);

    if let Some(note) = full_note {
        body = body.child(
            text(note)
                .key("room-full-note")
                .font(l.ty.label)
                .color(p.amber)
                .wrap(),
        );
    }
    if let Some(err) = &s.rooms.create_error {
        body = body.child(text(err.clone()).font(l.ty.label).color(p.red).wrap());
    }

    let submit_btn = row().pt(4.0).child(
        button(l, "Create room", Tone::Active, blocked.is_some())
            .key("room-create-submit")
            .on_press(|s: &mut State| submit_create(s)),
    );
    body = body.child(submit_btn);

    form.child(
        col()
            .grow(1.0)
            .w_full()
            .scroll(ScrollSpec { stick_bottom: false })
            .child(body),
    )
}

fn view_room(s: &State, room: &PublicRoom, p: &Palette, l: &Look) -> El {
    let now_ms = Utc::now().timestamp_millis();
    let rid = room.room_id.clone();

    let mut header = row()
        .key("room-header")
        .w_full()
        .px(20.0)
        .py(12.0)
        .gap(10.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline);

    if s.rooms.renaming {
        let rid_save = rid.clone();
        header = header
            .child(
                text_input(&s.rooms.rename_draft, type_::BODY)
                    .key("room-rename-input")
                    .single_line()
                    .grow(1.0)
                    .padding(6.0)
                    .radius(6.0)
                    .fill(p.field)
                    .border(1.0, p.hairline)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.rooms.rename_draft)
                    .on_submit(move |s: &mut State, _| submit_rename(s, rid.clone())),
            )
            .child(
                button(l, "Save", Tone::Active, false)
                    .key("room-rename-save")
                    .on_press(move |s: &mut State| submit_rename(s, rid_save.clone())),
            )
            .child(
                button(l, "Cancel", Tone::Neutral, false)
                    .key("room-rename-cancel")
                    .on_press(|s: &mut State| s.rooms.renaming = false),
            );
    } else {
        let member_names: Vec<String> = room
            .teammate_ids
            .iter()
            .map(|id| roster_member_name(&s.teammates, id))
            .collect();
        header = header
            .child(
                col()
                    .grow(1.0)
                    .gap(2.0)
                    .child(text(room.name.clone()).font(rune::TextStyle::new(16.0, 600.0)).color(p.text))
                    .child(
                        text(member_names.join(" · "))
                            .font(l.ty.label)
                            .color(p.muted)
                            .truncate(),
                    ),
            )
            .child(
                row()
                    .key("room-side-by-side")
                    .px(10.0)
                    .py(6.0)
                    .radius(6.0)
                    .border(1.0, p.hairline)
                    .fill(if s.rooms.side_by_side {
                        p.selected
                    } else {
                        p.raised
                    })
                    .hover(l.c("hover-surface"))
                    .role("button")
                    .label("Answers side by side")
                    .child(
                        text("Side by side")
                            .font(l.ty.label)
                            .color(if s.rooms.side_by_side { p.text } else { p.secondary }),
                    )
                    .on_press(|s: &mut State| s.rooms.side_by_side = !s.rooms.side_by_side),
            )
            .child(
                button(l, "Room", Tone::Neutral, false)
                    .key("room-menu-btn")
                    .on_press(|s: &mut State| {
                        s.rooms.menu_open = !s.rooms.menu_open;
                        s.rooms.confirming_remove = false;
                    }),
            );
    }

    let mut root = col()
        .key(format!("room-view-{}", room.room_id))
        .grow(1.0)
        .h_full()
        .fill(p.bg)
        .child(header);

    if s.rooms.menu_open {
        let room_name = room.name.clone();
        let rid_rm = room.room_id.clone();
        let mut menu = row()
            .key("room-menu")
            .w_full()
            .px(20.0)
            .py(8.0)
            .gap(8.0)
            .center_y()
            .fill(p.raised)
            .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
            .child(
                button(l, "Rename room", Tone::Neutral, false)
                    .key("room-rename-open")
                    .on_press(move |s: &mut State| {
                        s.rooms.menu_open = false;
                        s.rooms.renaming = true;
                        s.rooms.rename_draft.replace_text(&room_name);
                    }),
            );
        if s.rooms.confirming_remove {
            menu = menu
                .child(
                    button(l, "Remove room permanently", Tone::Danger, false)
                        .key("room-remove-confirm")
                        .on_press(move |s: &mut State| submit_remove(s, rid_rm.clone())),
                )
                .child(
                    button(l, "Keep room", Tone::Neutral, false)
                        .on_press(|s: &mut State| s.rooms.confirming_remove = false),
                );
        } else {
            menu = menu.child(
                button(l, "Remove room", Tone::Neutral, false)
                    .key("room-remove-btn")
                    .on_press(|s: &mut State| s.rooms.confirming_remove = true),
            );
        }
        root = root.child(menu);
    }

    if !s.rooms.readable {
        root = root.child(
            row()
                .w_full()
                .px(20.0)
                .py(8.0)
                .fill(p.amber.alpha(0.16))
                .child(text(room_store::UNREADABLE).font(l.ty.label).color(p.amber).wrap()),
        );
    }

    let mut feed = col().key("room-feed").w_full().gap(16.0).padding(20.0);
    feed = feed.child(view_task_board(s, room, p, l));

    if room.posts.is_empty() {
        feed = feed.child(
            col()
                .key("room-empty-posts")
                .w_full()
                .padding(16.0)
                .radius(8.0)
                .fill(p.raised)
                .border(1.0, p.hairline)
                .child(
                    text("Nothing posted yet. Whatever you write below goes to everyone in the room, or type @ to ask one teammate.")
                        .font(l.ty.body)
                        .color(p.muted)
                        .wrap(),
                ),
        );
    } else {
        for post in &room.posts {
            feed = feed.child(view_post(s, room, post, now_ms, p, l));
        }
    }

    root = root.child(
        col()
            .key("room-thread-scroll")
            .grow(1.0)
            .min_h(0.0)
            .w_full()
            .scroll(ScrollSpec { stick_bottom: true })
            .child(feed),
    );

    if let Some(notice) = &s.rooms.notice {
        root = root.child(
            row()
                .key("room-notice")
                .w_full()
                .px(20.0)
                .py(8.0)
                .fill(p.amber.alpha(0.14))
                .child(text(notice.clone()).font(l.ty.label).color(p.amber).wrap()),
        );
    }
    if let Some(err) = &s.rooms.error {
        root = root.child(
            row()
                .key("room-error")
                .w_full()
                .px(20.0)
                .py(8.0)
                .fill(p.red.alpha(0.14))
                .child(text(err.clone()).font(l.ty.label).color(p.red).wrap()),
        );
    }

    root.child(view_composer(s, room, p, l))
}

fn view_task_board(s: &State, room: &PublicRoom, p: &Palette, l: &Look) -> El {
    let done_count = room.tasks.iter().filter(|t| t.state == "done").count();
    let count_label = format!("{done_count} of {} done", room.tasks.len());
    let mut board = col()
        .key("room-tasks")
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .radius(8.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(label(l, "TASKS"))
                .child(text(count_label).font(l.ty.label).color(p.muted).grow(1.0))
                .child(
                    button(l, "Add a task", Tone::Neutral, false)
                        .key("room-task-add-open")
                        .on_press(|s: &mut State| {
                            s.rooms.adding_task = !s.rooms.adding_task;
                            s.rooms.new_task_draft.replace_text("");
                        }),
                ),
        );

    if s.rooms.adding_task {
        let rid = room.room_id.clone();
        let rid_submit = room.room_id.clone();
        board = board.child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    text_input(&s.rooms.new_task_draft, type_::BODY)
                        .key("room-task-add-input")
                        .single_line()
                        .grow(1.0)
                        .padding(6.0)
                        .radius(6.0)
                        .fill(p.field)
                        .border(1.0, p.hairline)
                        .color(p.text)
                        .placeholder("What needs doing?", p.muted)
                        .on_edit(|s: &mut State| &mut s.rooms.new_task_draft)
                        .on_submit(move |s: &mut State, _| {
                            let text = s.rooms.new_task_draft.text.trim().to_owned();
                            if !text.is_empty() {
                                submit_task(
                                    s,
                                    RoomTaskRequest::Add {
                                        room_id: rid.clone(),
                                        text,
                                        owner_id: None,
                                    },
                                );
                            }
                        }),
                )
                .child(
                    button(l, "Add", Tone::Active, false)
                        .key("room-task-add-submit")
                        .on_press(move |s: &mut State| {
                            let text = s.rooms.new_task_draft.text.trim().to_owned();
                            if !text.is_empty() {
                                submit_task(
                                    s,
                                    RoomTaskRequest::Add {
                                        room_id: rid_submit.clone(),
                                        text,
                                        owner_id: None,
                                    },
                                );
                            }
                        }),
                ),
        );
    }

    for task in &room.tasks {
        board = board.child(view_task_row(s, room, task, p, l));
    }
    board
}

fn view_task_row(
    s: &State,
    room: &PublicRoom,
    task: &RoomTask,
    p: &Palette,
    l: &Look,
) -> El {
    let tid = task.task_id.clone();
    let tid_menu = task.task_id.clone();
    let menu_open = s.rooms.open_task_menu.as_deref() == Some(tid.as_str());
    let assigning = s.rooms.assigning_task.as_deref() == Some(tid.as_str());
    let (marker_glyph, marker_color, state_label) = match task.state.as_str() {
        "done" => ("✓", p.green, "done"),
        "in-hand" => ("●", p.amber, "in hand"),
        _ => ("○", p.muted, "open"),
    };
    let owner_text = task
        .owner_id
        .as_deref()
        .map(|id| roster_member_name(&s.teammates, id))
        .unwrap_or_else(|| "unassigned".to_owned());

    let mut step = col().key(format!("room-task-{}", task.task_id)).w_full().gap(4.0);
    step = step.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .py(4.0)
            .child(
                text(marker_glyph)
                    .key(format!("room-task-marker-{}", task.task_id))
                    .font(l.ty.label)
                    .color(marker_color)
                    .label(state_label),
            )
            .child(
                text(task.text.clone())
                    .font(l.ty.body)
                    .color(if task.state == "done" { p.muted } else { p.text })
                    .grow(1.0)
                    .wrap(),
            )
            .child(text(owner_text).font(l.ty.label).color(p.muted))
            .child(
                row()
                    .key(format!("room-task-actions-{}", task.task_id))
                    .px(8.0)
                    .py(4.0)
                    .radius(6.0)
                    .border(1.0, p.hairline)
                    .hover(l.c("hover-surface"))
                    .role("button")
                    .label(format!("Actions for {}", task.text))
                    .child(text("Actions").font(l.ty.label).color(p.secondary))
                    .on_press(move |s: &mut State| {
                        s.rooms.assigning_task = None;
                        s.rooms.open_task_menu = if menu_open {
                            None
                        } else {
                            Some(tid_menu.clone())
                        };
                    }),
            ),
    );

    if menu_open {
        let rid = room.room_id.clone();
        let tid_toggle = task.task_id.clone();
        let tid_assign = task.task_id.clone();
        let tid_rm = task.task_id.clone();
        let is_done = task.state == "done";
        let toggle_label = if is_done { "Reopen" } else { "Mark done" };
        let actions = row()
            .w_full()
            .gap(6.0)
            .pl(20.0)
            .child(
                button(l, "Assign", Tone::Neutral, false)
                    .key(format!("room-task-assign-{}", task.task_id))
                    .on_press(move |s: &mut State| {
                        s.rooms.assigning_task = Some(tid_assign.clone());
                    }),
            )
            .child({
                let rid2 = rid.clone();
                button(l, toggle_label, Tone::Neutral, false)
                    .key(format!("room-task-toggle-{}", task.task_id))
                    .on_press(move |s: &mut State| {
                        let req = if is_done {
                            RoomTaskRequest::Reopen {
                                room_id: rid2.clone(),
                                task_id: tid_toggle.clone(),
                            }
                        } else {
                            RoomTaskRequest::Done {
                                room_id: rid2.clone(),
                                task_id: tid_toggle.clone(),
                            }
                        };
                        submit_task(s, req);
                    })
            })
            .child({
                let rid3 = rid.clone();
                button(l, "Remove", Tone::Danger, false)
                    .key(format!("room-task-remove-{}", task.task_id))
                    .on_press(move |s: &mut State| {
                        submit_task(
                            s,
                            RoomTaskRequest::Remove {
                                room_id: rid3.clone(),
                                task_id: tid_rm.clone(),
                            },
                        );
                    })
            });
        if assigning {
            let mut who_row = row().w_full().gap(6.0).pl(20.0).wrap();
            let rid_un = room.room_id.clone();
            let tid_un = task.task_id.clone();
            who_row = who_row.child(button(l, "Unassigned", Tone::Neutral, false).on_press(
                move |s: &mut State| {
                    submit_task(
                        s,
                        RoomTaskRequest::Assign {
                            room_id: rid_un.clone(),
                            task_id: tid_un.clone(),
                            owner_id: None,
                        },
                    );
                },
            ));
            for member_id in &room.teammate_ids {
                let mid = member_id.clone();
                let rid_m = room.room_id.clone();
                let tid_m = task.task_id.clone();
                let name = roster_member_name(&s.teammates, member_id);
                who_row = who_row.child(button(l, &name, Tone::Neutral, false).on_press(
                    move |s: &mut State| {
                        submit_task(
                            s,
                            RoomTaskRequest::Assign {
                                room_id: rid_m.clone(),
                                task_id: tid_m.clone(),
                                owner_id: Some(mid.clone()),
                            },
                        );
                    },
                ));
            }
            step = step.child(actions).child(who_row);
        } else {
            step = step.child(actions);
        }
    }
    step
}

struct BuiltAnswer<'a> {
    teammate_id: &'a str,
    name: String,
    mission_id: String,
    phase: String,
    text: String,
    note_error: Option<String>,
    model_line: String,
    elapsed_s: i64,
}

fn view_post(
    s: &State,
    room: &PublicRoom,
    post: &RoomPost,
    now_ms: i64,
    p: &Palette,
    l: &Look,
) -> El {
    let targeted: Option<HashSet<&str>> = (!post.to.is_empty())
        .then(|| post.to.iter().map(String::as_str).collect());

    let mut statuses: Vec<PostMemberStatus> = Vec::with_capacity(room.teammate_ids.len());
    let mut answers: Vec<BuiltAnswer<'_>> = Vec::new();
    let mut queued_names: Vec<String> = Vec::new();
    let mut absent: Vec<AbsentMember> = Vec::new();
    let mut refused_members: Vec<RefusedMember> = Vec::new();

    for member_id in &room.teammate_ids {
        let name = roster_member_name(&s.teammates, member_id);
        if let Some(reason) = post.refused.get(member_id) {
            refused_members.push(RefusedMember {
                name: name.clone(),
                message: reason.clone(),
            });
        }
        let is_asked = targeted
            .as_ref()
            .is_none_or(|set| set.contains(member_id.as_str()));
        if !is_asked {
            statuses.push(PostMemberStatus {
                teammate_id: member_id.clone(),
                name,
                state: MemberPostState::NotAsked,
            });
            continue;
        }
        if let Some(mission_id) = post.missions.get(member_id) {
            let tr = s.rooms.transcripts.get(mission_id);
            let live = s.seen_live.contains(mission_id)
                || tr.is_none_or(|t| t.phase == "running");
            let phase = if live {
                "running".to_owned()
            } else {
                tr.map(|t| t.phase.clone())
                    .unwrap_or_else(|| "completed".to_owned())
            };
            let text = tr.map(|t| t.answer_text.clone()).unwrap_or_default();
            let tm = s.teammates.iter().find(|t| &t.id == member_id);
            let started_at = tr.map(|t| t.started_at.as_str()).unwrap_or("");
            let elapsed_s = elapsed_seconds(started_at, &post.at, now_ms);
            statuses.push(PostMemberStatus {
                teammate_id: member_id.clone(),
                name: name.clone(),
                state: if phase == "running" {
                    MemberPostState::Running {
                        replied: !text.is_empty(),
                    }
                } else if phase == "failed" {
                    MemberPostState::Failed
                } else {
                    MemberPostState::Completed
                },
            });
            answers.push(BuiltAnswer {
                teammate_id: member_id,
                name,
                mission_id: mission_id.clone(),
                phase,
                text,
                note_error: tr.and_then(|t| t.note_error.clone()),
                model_line: roster_model_line(tm, tr),
                elapsed_s,
            });
        } else if post.queued.contains(member_id) {
            queued_names.push(name.clone());
            statuses.push(PostMemberStatus {
                teammate_id: member_id.clone(),
                name,
                state: MemberPostState::Queued,
            });
        } else {
            let reason = post.refused.get(member_id).cloned();
            absent.push(AbsentMember {
                name: name.clone(),
                reason,
            });
            statuses.push(PostMemberStatus {
                teammate_id: member_id.clone(),
                name,
                state: MemberPostState::Absent,
            });
        }
    }

    let time_ago = crate::rail::short_ago(Some(&post.at), now_ms).unwrap_or_default();
    let mut meta_parts = Vec::new();
    if !post.to.is_empty() {
        let to_names: Vec<String> = post
            .to
            .iter()
            .map(|id| roster_member_name(&s.teammates, id))
            .collect();
        meta_parts.push(format!("To {}", names_said(&to_names)));
    }
    if !time_ago.is_empty() {
        meta_parts.push(time_ago);
    }
    meta_parts.push(post_headline(&statuses));

    let mut face_strip = row().w_full().gap(6.0).center_y();
    for st in &statuses {
        let a11y = match st.state {
            MemberPostState::NotAsked => format!("{}, not asked", st.name),
            MemberPostState::Completed => format!("{}, answered", st.name),
            MemberPostState::Running { .. } => format!("{}, running", st.name),
            MemberPostState::Failed => format!("{}, failed", st.name),
            MemberPostState::Queued => format!("{}, waiting for a slot", st.name),
            MemberPostState::Absent => format!("{}, not asked", st.name),
        };
        let mut pip = row()
            .key(format!("room-posthead-face-{}-{}", post.post_id, st.teammate_id))
            .label(a11y)
            .child(face(
                s.image_cache.as_deref(),
                &st.teammate_id,
                &st.name,
                s.looks.get(&st.teammate_id),
                20.0,
                l,
            ));
        if matches!(st.state, MemberPostState::NotAsked | MemberPostState::Absent) {
            pip = pip.opacity(0.45);
        }
        face_strip = face_strip.child(pip);
    }

    let mut post_el = col()
        .key(format!("room-post-{}", post.post_id))
        .w_full()
        .gap(10.0)
        .padding(14.0)
        .radius(10.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(
            col()
                .key(format!("room-posthead-{}", post.post_id))
                .w_full()
                .gap(6.0)
                .child(text(post.text.clone()).font(type_::BODY).color(p.text).wrap())
                .child(
                    row()
                        .w_full()
                        .gap(10.0)
                        .center_y()
                        .child(
                            text(meta_parts.join(" · "))
                                .font(l.ty.label)
                                .color(p.muted)
                                .grow(1.0),
                        )
                        .child(face_strip),
                ),
        );

    if !answers.is_empty() {
        let completed_count = answers.iter().filter(|a| a.phase == "completed").count();
        let is_list = answers.len() > ANSWERS_BEFORE_A_LIST;
        let is_columns = !is_list && s.rooms.side_by_side && answers.len() > 1;

        let mut cards_container = if is_columns {
            row()
                .key(format!("room-post-answers-columns-{}", post.post_id))
                .w_full()
                .gap(10.0)
                .scroll(ScrollSpec { stick_bottom: false })
        } else {
            col()
                .key(format!("room-post-answers-{}", post.post_id))
                .w_full()
                .gap(8.0)
        };

        for ans in &answers {
            let mut card = view_answer_card(s, post, ans, completed_count, p, l);
            if is_columns {
                card = card.min_w(SIDE_BY_SIDE_COL_MIN_W).grow(1.0);
            }
            cards_container = cards_container.child(card);
        }
        post_el = post_el.child(cards_container);
    }

    if !queued_names.is_empty() {
        post_el = post_el.child(
            row()
                .key(format!("room-post-waiting-{}", post.post_id))
                .w_full()
                .gap(8.0)
                .px(10.0)
                .py(6.0)
                .radius(6.0)
                .fill(p.bg)
                .child(
                    text(format!(
                        "Waiting for a slot ({}) · {}",
                        queued_names.len(),
                        waiting_line(&queued_names)
                    ))
                    .font(l.ty.label)
                    .color(p.muted)
                    .wrap(),
                ),
        );
    }

    if let Some(line) = absent_line(&absent) {
        post_el = post_el.child(
            text(line)
                .key(format!("room-post-absent-{}", post.post_id))
                .font(l.ty.label)
                .color(p.muted)
                .wrap(),
        );
    }

    if let Some(notice) = refusal_notice(&refused_members) {
        post_el = post_el.child(
            text(notice)
                .key(format!("room-post-refused-{}", post.post_id))
                .font(l.ty.label)
                .color(p.amber)
                .wrap(),
        );
    }

    post_el
}

fn view_answer_card(
    s: &State,
    post: &RoomPost,
    ans: &BuiltAnswer<'_>,
    completed_count: usize,
    p: &Palette,
    l: &Look,
) -> El {
    let tid_reply = ans.teammate_id.to_owned();
    let name_build = ans.name.clone();
    let tid_merge = ans.teammate_id.to_owned();
    let mid_open = ans.mission_id.clone();

    let (phase_label, phase_color) = if ans.phase == "running" {
        if ans.text.is_empty() {
            if ans.elapsed_s >= NO_WORD_BACK_SECS {
                (
                    format!("asked · {}s · no word back yet", ans.elapsed_s),
                    p.amber,
                )
            } else {
                (format!("asked · {}s", ans.elapsed_s), p.muted)
            }
        } else {
            (format!("running · {}s", ans.elapsed_s), p.muted)
        }
    } else if ans.phase == "failed" {
        ("failed".to_owned(), p.red)
    } else {
        ("completed".to_owned(), p.muted)
    };

    let mut acts = row().gap(6.0).center_y();
    acts = acts.child(
        button(l, "Reply", Tone::Neutral, false)
            .key(format!("room-answer-reply-{}-{}", post.post_id, ans.teammate_id))
            .label(format!("Reply to {}", ans.name))
            .on_press(move |s: &mut State| {
                s.rooms.ask_to = vec![tid_reply.clone()];
            }),
    );
    if ans.phase == "completed" {
        acts = acts.child(
            button(l, "Build on", Tone::Neutral, false)
                .key(format!(
                    "room-answer-build-{}-{}",
                    post.post_id, ans.teammate_id
                ))
                .on_press(move |s: &mut State| {
                    let (to, next) = build_on_draft(&name_build);
                    s.rooms.ask_to = to;
                    s.rooms.draft.replace_text(&next);
                }),
        );
        if completed_count >= 2 {
            acts = acts.child(
                button(l, "Merge", Tone::Neutral, false)
                    .key(format!(
                        "room-answer-merge-{}-{}",
                        post.post_id, ans.teammate_id
                    ))
                    .on_press(move |s: &mut State| {
                        let (to, next) = merge_by_draft(&tid_merge);
                        s.rooms.ask_to = to;
                        s.rooms.draft.replace_text(&next);
                    }),
            );
        }
    }
    acts = acts.child(
        button(l, "Open", Tone::Neutral, false)
            .key(format!("room-answer-open-{}-{}", post.post_id, ans.teammate_id))
            .on_press_tx(move |s: &mut State, tx| {
                close(s);
                crate::peer_thread::open_mission_by_id(s, mid_open.clone(), tx);
            }),
    );

    let mut card = col()
        .key(format!("room-answer-{}-{}", post.post_id, ans.teammate_id))
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.hairline)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(face(
                    s.image_cache.as_deref(),
                    ans.teammate_id,
                    &ans.name,
                    s.looks.get(ans.teammate_id),
                    24.0,
                    l,
                ))
                .child(
                    col()
                        .grow(1.0)
                        .gap(1.0)
                        .child(
                            row()
                                .gap(8.0)
                                .center_y()
                                .child(text(ans.name.clone()).font(type_::TITLE).color(p.text))
                                .child(text(ans.model_line.clone()).font(l.ty.label).color(p.muted)),
                        )
                        .child(text(phase_label).font(l.ty.label).color(phase_color)),
                )
                .child(acts),
        );

    if !ans.text.is_empty() {
        // Whole answer, never clamped or folded.
        card = card.child(
            Markdown::new(&ans.text)
                .selectable(true)
                .render(&markdown_style(p))
                .w_full(),
        );
    } else if let Some(err) = &ans.note_error {
        card = card.child(text(err.clone()).font(l.ty.label).color(p.red).wrap());
    }
    card
}

fn view_composer(s: &State, room: &PublicRoom, p: &Palette, l: &Look) -> El {
    let room_members: Vec<MentionMember> = room
        .teammate_ids
        .iter()
        .filter_map(|id| {
            s.teammates.iter().find(|t| &t.id == id).map(|t| MentionMember {
                teammate_id: t.id.clone(),
                name: t.name.clone(),
            })
        })
        .collect();

    let ask_names: Vec<String> = s
        .rooms
        .ask_to
        .iter()
        .map(|id| roster_member_name(&s.teammates, id))
        .collect();

    let placeholder = if s.rooms.ask_to.is_empty() {
        format!("Post to {}…", room.name)
    } else {
        format!("Ask {} in {}…", names_said(&ask_names), room.name)
    };

    let mention = mention_at(&s.rooms.draft.text, s.rooms.draft.text.len()).filter(|m| {
        s.rooms.at_dismissed.as_deref() != Some(&format!("{}@{}", m.start, m.query))
    });
    let choices = mention
        .as_ref()
        .map(|m| mention_choices(&room_members, &s.rooms.ask_to, &m.query))
        .unwrap_or_default();

    let mut box_col = col()
        .key("room-composer-box")
        .w_full()
        .px(20.0)
        .py(12.0)
        .gap(8.0)
        .fill(p.bg)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.hairline);

    if let Some(m) = &mention
        && !choices.is_empty()
    {
        let mut popup = col()
            .key("room-mention-popup")
            .w_full()
            .padding(6.0)
            .gap(4.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, p.hairline);
        for choice in choices {
            let cid = choice.teammate_id.clone();
            let cname = choice.name.clone();
            let m_clone = m.clone();
            popup = popup.child(
                row()
                    .key(format!("room-mention-choice-{cid}"))
                    .w_full()
                    .px(10.0)
                    .py(6.0)
                    .gap(8.0)
                    .center_y()
                    .radius(6.0)
                    .hover(l.c("hover-surface"))
                    .role("button")
                    .child(face(
                        s.image_cache.as_deref(),
                        &cid,
                        &cname,
                        s.looks.get(&cid),
                        20.0,
                        l,
                    ))
                    .child(text(cname).font(l.ty.body).color(p.text))
                    .on_press(move |s: &mut State| {
                        let (next, _) = without_mention(&s.rooms.draft.text, &m_clone);
                        if !s.rooms.ask_to.contains(&cid) {
                            s.rooms.ask_to.push(cid.clone());
                        }
                        s.rooms.draft.replace_text(&next);
                        s.rooms.at_dismissed = None;
                    }),
            );
        }
        box_col = box_col.child(popup);
    }

    if !s.rooms.ask_to.is_empty() {
        let mut strip = row()
            .key("room-ask-to")
            .w_full()
            .gap(6.0)
            .center_y()
            .child(text("To").font(l.ty.label).color(p.muted));
        for tid in &s.rooms.ask_to {
            let rm_id = tid.clone();
            let tname = roster_member_name(&s.teammates, tid);
            strip = strip.child(
                chip(l, &format!("{tname} ×"), true)
                    .key(format!("room-ask-chip-{tid}"))
                    .on_press(move |s: &mut State| {
                        s.rooms.ask_to.retain(|id| id != &rm_id);
                    }),
            );
        }
        box_col = box_col.child(strip);
    }

    let members_for_edit = room_members.clone();
    let can_post = !s.rooms.draft.text.trim().is_empty() && !s.rooms.action_job.is_pending();
    let input_row = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .padding(8.0)
        .radius(10.0)
        .fill(p.field)
        .border(1.0, p.hairline)
        .child(
            text_input(&s.rooms.draft, type_::BODY)
                .key("room-composer")
                .placeholder(placeholder, p.muted)
                .color(p.text)
                .grow(1.0)
                .on_edit(move |s: &mut State| {
                    on_room_draft_edit(s, &members_for_edit);
                    &mut s.rooms.draft
                })
                .on_submit(|s: &mut State, _| submit_post(s)),
        )
        .child(
            button(l, "Post", Tone::Active, !can_post)
                .key("room-send")
                .on_press(|s: &mut State| submit_post(s)),
        );

    box_col.child(input_row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};
    use std::collections::BTreeMap;

    const SOURCE: &str = include_str!("room_screen.rs");
    const SIDEBAR_SOURCE: &str = include_str!("sidebar.rs");

    fn tm(id: &str, name: &str) -> Teammate {
        Teammate {
            id: id.into(),
            name: name.into(),
            role: "Code & Migrations".into(),
            runtime: Some("opencode".into()),
            model: Some("free".into()),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        }
    }

    fn tr(text: &str, phase: &str) -> RoomTranscriptView {
        RoomTranscriptView {
            phase: phase.into(),
            answer_text: text.into(),
            note_error: None,
            started_at: "2026-09-26T05:00:00.000Z".into(),
            runtime: "opencode".into(),
            model: "free".into(),
        }
    }

    fn render_room_texts(
        room: PublicRoom,
        roster: Vec<Teammate>,
        transcripts: HashMap<String, RoomTranscriptView>,
        side_by_side: bool,
    ) -> Runtime<State> {
        let mut s = State::default();
        s.teammates = roster;
        s.rooms.active = RoomView::Room(room.room_id.clone());
        s.rooms.rooms = vec![room];
        s.rooms.transcripts = transcripts;
        s.rooms.side_by_side = side_by_side;
        let mut rt = Runtime::new(
            s,
            |s, _| view(s, &Palette::new(true)),
            (1100.0, 820.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt
    }

    fn code_without_comments(src: &str) -> String {
        let end = src.find("#[cfg(test)]").unwrap_or(src.len());
        src[..end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    // --- room-answer-is-whole.test.ts (3) ---
    #[test]
    fn answer_has_no_fold_clamp_or_newline_splitting() {
        let code = code_without_comments(SOURCE);
        assert!(!code.contains("line_clamp"));
        assert!(!code.contains("max_h"));
        assert!(!code.contains("Show the rest"));
        assert!(!code.contains("is_folded"));
        assert!(!code.contains(".split('\\n')"));
        assert!(!code.contains(".lines()"));
    }

    // --- room-answers-side-by-side.test.ts (4) ---
    #[test]
    fn answers_side_by_side_lay_out_in_a_horizontal_scrolling_row_with_wide_columns() {
        let r = PublicRoom {
            room_id: "room_pair".into(),
            name: "pair".into(),
            teammate_ids: vec!["tm_wren".into(), "tm_pip".into()],
            created_at: "2026-09-27T05:00:00.000Z".into(),
            posts: vec![RoomPost {
                post_id: "post_1".into(),
                text: "A fruit each.".into(),
                at: "2026-09-27T05:01:00.000Z".into(),
                missions: BTreeMap::from([
                    ("tm_wren".into(), "m1".into()),
                    ("tm_pip".into(), "m2".into()),
                ]),
                queued: Vec::new(),
                refused: BTreeMap::new(),
                to: Vec::new(),
            }],
            tasks: Vec::new(),
        };
        let transcripts = HashMap::from([
            ("m1".into(), tr("apple", "completed")),
            ("m2".into(), tr("pear", "completed")),
        ]);
        let rt_stacked = render_room_texts(
            r.clone(),
            vec![tm("tm_wren", "Wren"), tm("tm_pip", "Pip")],
            transcripts.clone(),
            false,
        );
        let stacked_wren = rt_stacked.presented("room-answer-post_1-tm_wren").unwrap().0;
        let stacked_pip = rt_stacked.presented("room-answer-post_1-tm_pip").unwrap().0;
        assert!(stacked_pip[1] > stacked_wren[1]);

        let rt_cols = render_room_texts(
            r,
            vec![tm("tm_wren", "Wren"), tm("tm_pip", "Pip")],
            transcripts,
            true,
        );
        assert!(rt_cols.presented("room-post-answers-columns-post_1").is_some());
        let cols_wren = rt_cols.presented("room-answer-post_1-tm_wren").unwrap().0;
        let cols_pip = rt_cols.presented("room-answer-post_1-tm_pip").unwrap().0;
        assert_eq!(cols_wren[1], cols_pip[1]);
        assert!(cols_wren[2] >= SIDE_BY_SIDE_COL_MIN_W);
        assert!(cols_pip[2] >= SIDE_BY_SIDE_COL_MIN_W);
    }

    // --- rooms-are-reachable.test.ts (2) ---
    #[test]
    fn sidebar_rooms_section_is_present_and_never_gated_behind_nonempty_rooms() {
        let code = code_without_comments(SIDEBAR_SOURCE);
        assert!(code.contains("New room"));
        assert!(!code.contains("!s.rooms.rooms.is_empty() &&"));
    }

    // --- a-room-post-can-ask-one-teammate.test.ts (5) ---
    #[test]
    fn at_mentions_find_offer_strip_autocomplete_and_format_names() {
        let members = vec![
            MentionMember { teammate_id: "tm_wren".into(), name: "Wren".into() },
            MentionMember { teammate_id: "tm_pip".into(), name: "Pip".into() },
            MentionMember { teammate_id: "tm_pippa".into(), name: "Pippa".into() },
        ];
        assert_eq!(mention_at("@", 1), Some(MentionMatch { start: 0, query: "".into() }));
        assert_eq!(mention_at("@Wr", 3), Some(MentionMatch { start: 0, query: "Wr".into() }));
        assert_eq!(mention_at("hi @P", 5), Some(MentionMatch { start: 3, query: "P".into() }));
        assert_eq!(mention_at("mail pip@example.com", 20), None);
        assert_eq!(mention_at("@Wren ", 6), None);
        assert_eq!(mention_at("@Wren and more", 14), None);

        let all_names: Vec<&str> = mention_choices(&members, &[], "").iter().map(|m| m.name.as_str()).collect();
        assert_eq!(all_names, vec!["Wren", "Pip", "Pippa"]);
        let pi: Vec<&str> = mention_choices(&members, &["tm_pip".into()], "pi").iter().map(|m| m.name.as_str()).collect();
        assert_eq!(pi, vec!["Pippa"]);

        assert_eq!(without_mention("@Wr", &MentionMatch { start: 0, query: "Wr".into() }), ("".into(), 0));
        assert_eq!(without_mention("@Wren tell me", &MentionMatch { start: 0, query: "Wren".into() }), ("tell me".into(), 0));
        assert_eq!(without_mention("so @Pi what now", &MentionMatch { start: 3, query: "Pi".into() }), ("so what now".into(), 3));
        assert_eq!(without_mention("ask @Pip", &MentionMatch { start: 4, query: "Pip".into() }), ("ask ".into(), 4));

        assert!(mention_completed(&members, &[], "@pip ", 5).is_some_and(|(m, _)| m.teammate_id == "tm_pip"));
        assert!(mention_completed(&members, &[], "@Pi ", 4).is_none());
        assert!(mention_completed(&members, &["tm_pip".into()], "@Pip ", 5).is_none());

        assert_eq!(names_said(&["Wren".into()]), "Wren");
        assert_eq!(names_said(&["Wren".into(), "Pip".into()]), "Wren and Pip");
        assert_eq!(names_said(&["Wren".into(), "Pip".into(), "Pippa".into()]), "Wren, Pip and Pippa");
    }

    // --- a-room-names-who-it-asked.test.tsx (5) ---
    #[test]
    fn targeted_post_draws_answers_reply_to_time_not_asked_faces_and_no_absent_report_for_untargeted() {
        let roster = vec![
            tm("tm_wren", "Wren"),
            tm("tm_pip", "Pip"),
            tm("tm_booty", "Booty"),
        ];
        let room = PublicRoom {
            room_id: "room_pair".into(),
            name: "pair".into(),
            teammate_ids: vec!["tm_wren".into(), "tm_pip".into(), "tm_booty".into()],
            created_at: "2026-09-26T05:00:00.000Z".into(),
            posts: vec![
                RoomPost {
                    post_id: "post_all".into(),
                    text: "Everyone, a word.".into(),
                    at: "2026-09-26T05:01:00.000Z".into(),
                    missions: BTreeMap::from([("tm_wren".into(), "m1".into()), ("tm_pip".into(), "m2".into())]),
                    queued: Vec::new(),
                    refused: BTreeMap::new(),
                    to: Vec::new(),
                },
                RoomPost {
                    post_id: "post_wren".into(),
                    text: "Wren, say more.".into(),
                    at: "2026-09-26T05:02:00.000Z".into(),
                    missions: BTreeMap::from([("tm_wren".into(), "m3".into())]),
                    queued: Vec::new(),
                    refused: BTreeMap::new(),
                    to: vec!["tm_wren".into()],
                },
            ],
            tasks: Vec::new(),
        };
        let transcripts = HashMap::from([
            ("m1".into(), tr("DONE", "completed")),
            ("m2".into(), tr("DONE", "completed")),
            ("m3".into(), tr("DONE", "completed")),
        ]);
        let rt = render_room_texts(room, roster, transcripts, false);
        let els = rt.elements();
        let reply_wren = els.iter().filter(|e| e.label.as_deref() == Some("Reply to Wren")).count();
        let reply_pip = els.iter().filter(|e| e.label.as_deref() == Some("Reply to Pip")).count();
        assert_eq!(reply_wren, 2);
        assert_eq!(reply_pip, 1);

        let all_texts: Vec<&str> = els.iter().filter_map(|e| e.text.as_deref()).collect();
        assert!(all_texts.iter().any(|t| t.contains("To Wren · ")));
        assert_eq!(all_texts.iter().filter(|t| t.contains("Booty was not asked")).count(), 1);
        assert!(!all_texts.iter().any(|t| t.contains("Pip and Booty were not asked")));
        assert!(els.iter().any(|e| e.label.as_deref() == Some("Pip, not asked")));
        assert!(rt.presented("room-ask-to").is_none());
    }

    // --- room-absent-is-a-line.test.tsx (20 checks grouped cleanly) ---
    #[test]
    fn absent_line_refusals_grid_switch_whole_answers_and_task_board() {
        let cap = "Up to 8 missions can run at once. Wait for one to finish or stop it first.";
        assert_eq!(
            absent_line(&[AbsentMember { name: "Otto".into(), reason: Some(cap.into()) }]).as_deref(),
            Some("Otto was not asked — up to 8 missions can run at once. Wait for one to finish or stop it first.")
        );
        assert_eq!(
            absent_line(&[
                AbsentMember { name: "Pike".into(), reason: Some("Up to 8 missions can run at once.".into()) },
                AbsentMember { name: "Dell".into(), reason: Some("Up to 8 missions can run at once.".into()) },
                AbsentMember { name: "Ember".into(), reason: Some("Up to 8 missions can run at once.".into()) },
            ]).as_deref(),
            Some("Pike, Dell and Ember were not asked — up to 8 missions can run at once.")
        );
        assert_eq!(
            absent_line(&[
                AbsentMember { name: "Otto".into(), reason: Some("Up to 8 missions can run at once.".into()) },
                AbsentMember { name: "Pike".into(), reason: None },
            ]).as_deref(),
            Some("Otto was not asked — up to 8 missions can run at once. Pike was not asked.")
        );
        assert_eq!(absent_line(&[]), None);

        assert_eq!(
            refusal_notice(&[
                RefusedMember { name: "Otto".into(), message: cap.into() },
                RefusedMember { name: "Pike".into(), message: cap.into() },
            ]).as_deref(),
            Some(format!("Otto, Pike: {cap}").as_str())
        );
        assert_eq!(
            refusal_notice(&[
                RefusedMember { name: "Otto".into(), message: "Up to 8 missions can run at once.".into() },
                RefusedMember { name: "Pike".into(), message: "No longer on the roster.".into() },
            ]).as_deref(),
            Some("Otto: Up to 8 missions can run at once. · Pike: No longer on the roster.")
        );
        assert_eq!(refusal_notice(&[]), None);

        let names = ["Wren", "Booty", "Gem", "Fen", "Otto", "Pike", "Dell", "Ember"];
        let roster: Vec<Teammate> = names
            .iter()
            .enumerate()
            .map(|(i, n)| tm(&format!("tm_{i}"), n))
            .collect();
        let missions: BTreeMap<String, String> = (0..4)
            .map(|i| (format!("tm_{i}"), format!("m_{i}")))
            .collect();
        let transcripts: HashMap<String, RoomTranscriptView> = (0..4)
            .map(|i| (format!("m_{i}"), tr("ALMANAC", "completed")))
            .collect();
        let partial_room = PublicRoom {
            room_id: "room_standup".into(),
            name: "Standup".into(),
            teammate_ids: roster.iter().map(|t| t.id.clone()).collect(),
            created_at: "2026-09-05T05:00:00.000Z".into(),
            posts: vec![RoomPost {
                post_id: "post_1".into(),
                text: "Say your word".into(),
                at: "2026-09-05T05:00:00.000Z".into(),
                missions,
                queued: Vec::new(),
                refused: BTreeMap::new(),
                to: Vec::new(),
            }],
            tasks: vec![
                RoomTask {
                    task_id: "t1".into(),
                    text: "Map v2 to v3 payloads".into(),
                    owner_id: None,
                    state: "done".into(),
                    mission_id: None,
                    handed_by: None,
                    handed_without_note: false,
                    at: "2026-09-05T05:00:00.000Z".into(),
                },
                RoomTask {
                    task_id: "t2".into(),
                    text: "Check migration".into(),
                    owner_id: None,
                    state: "in-hand".into(),
                    mission_id: None,
                    handed_by: None,
                    handed_without_note: false,
                    at: "2026-09-05T05:00:00.000Z".into(),
                },
            ],
        };
        let rt = render_room_texts(partial_room, roster, transcripts, false);
        let els = rt.elements();
        assert_eq!((4..8).filter(|i| rt.presented(&format!("room-answer-post_1-tm_{i}")).is_some()).count(), 0);
        assert_eq!((0..4).filter(|i| rt.presented(&format!("room-answer-post_1-tm_{i}")).is_some()).count(), 4);
        assert!(els.iter().filter_map(|e| e.text.as_deref()).any(|t| t.contains("Otto, Pike, Dell and Ember were not asked.")));
        assert!(els.iter().filter_map(|e| e.text.as_deref()).any(|t| t == "1 of 2 done"));
        assert!(els.iter().any(|e| e.label.as_deref() == Some("in hand")));
        assert!(els.iter().any(|e| e.label.as_deref() == Some("Actions for Map v2 to v3 payloads")));
        assert!(!els.iter().filter_map(|e| e.text.as_deref()).any(|t| matches!(t, "Assign" | "Remove" | "Reopen")));
    }

    // --- room-card-names-who.test.tsx (3) ---
    #[test]
    fn room_card_meta_names_members_counts_posts_and_dates() {
        let roster = vec![
            tm("a", "Atlas"),
            tm("q", "Quill"),
            tm("w", "Wren"),
            tm("m", "Marlow"),
        ];
        let now = DateTime::parse_from_rfc3339("2026-09-27T12:00:00.000Z")
            .unwrap()
            .timestamp_millis();
        let one_post = PublicRoom {
            room_id: "r1".into(),
            name: "r1".into(),
            teammate_ids: vec!["a".into(), "q".into()],
            created_at: "2026-09-26T12:00:00.000Z".into(),
            posts: vec![RoomPost {
                post_id: "p1".into(),
                text: "hi".into(),
                at: "2026-09-27T11:58:00.000Z".into(),
                missions: BTreeMap::new(),
                queued: Vec::new(),
                refused: BTreeMap::new(),
                to: Vec::new(),
            }],
            tasks: Vec::new(),
        };
        assert_eq!(room_card_meta(&one_post, &roster, now), "Atlas · Quill · 1 post · 2m");

        let empty_room = PublicRoom {
            room_id: "r2".into(),
            name: "r2".into(),
            teammate_ids: vec!["a".into()],
            created_at: "2026-09-27T09:00:00.000Z".into(),
            posts: Vec::new(),
            tasks: Vec::new(),
        };
        assert_eq!(room_card_meta(&empty_room, &roster, now), "Atlas · nothing posted yet · 3h");

        let four_room = PublicRoom {
            room_id: "r3".into(),
            name: "r3".into(),
            teammate_ids: vec!["a".into(), "q".into(), "w".into(), "m".into()],
            created_at: "2026-09-27T11:00:00.000Z".into(),
            posts: Vec::new(),
            tasks: Vec::new(),
        };
        assert_eq!(room_card_meta(&four_room, &roster, now), "Atlas · Quill +2 · nothing posted yet · 1h");
    }

    // --- room-round-two.test.tsx (7) ---
    #[test]
    fn round_two_offers_build_on_and_merge_without_auto_sending() {
        let room = PublicRoom {
            room_id: "room_pair".into(),
            name: "pair".into(),
            teammate_ids: vec!["tm_wren".into(), "tm_pip".into()],
            created_at: "2026-09-27T05:00:00.000Z".into(),
            posts: vec![RoomPost {
                post_id: "post_1".into(),
                text: "A fruit each.".into(),
                at: "2026-09-27T05:01:00.000Z".into(),
                missions: BTreeMap::from([("tm_wren".into(), "m1".into()), ("tm_pip".into(), "m2".into())]),
                queued: Vec::new(),
                refused: BTreeMap::new(),
                to: Vec::new(),
            }],
            tasks: Vec::new(),
        };
        let rt_done = render_room_texts(
            room.clone(),
            vec![tm("tm_wren", "Wren"), tm("tm_pip", "Pip")],
            HashMap::from([("m1".into(), tr("apple", "completed")), ("m2".into(), tr("pear", "completed"))]),
            false,
        );
        let texts_done: Vec<String> = rt_done.elements().into_iter().filter_map(|e| e.text).collect();
        assert_eq!(texts_done.iter().filter(|t| t.as_str() == "Build on").count(), 2);
        assert_eq!(texts_done.iter().filter(|t| t.as_str() == "Merge").count(), 2);
        assert!(rt_done.elements().iter().any(|e| e.label.as_deref() == Some("Answers side by side")));

        let rt_running = render_room_texts(
            room,
            vec![tm("tm_wren", "Wren"), tm("tm_pip", "Pip")],
            HashMap::from([("m1".into(), tr("apple", "completed")), ("m2".into(), tr("", "running"))]),
            false,
        );
        let els_run = rt_running.elements();
        assert_eq!(els_run.iter().filter(|e| e.text.as_deref() == Some("Build on")).count(), 1);
        assert_eq!(els_run.iter().filter(|e| e.text.as_deref() == Some("Merge")).count(), 0);

        let (to_b, draft_b) = build_on_draft("Wren");
        assert!(to_b.is_empty());
        assert_eq!(draft_b, "Build on Wren\u{2019}s answer above: ");
        let (to_m, draft_m) = merge_by_draft("tm_wren");
        assert_eq!(to_m, vec!["tm_wren".to_owned()]);
        assert!(draft_m.contains("keep what they agree on, say plainly where they differ, and name whose each part was."));
    }

    // --- room-too-big-to-make.test.tsx (4) ---
    #[test]
    fn room_full_note_warns_above_max_and_stays_quiet_at_or_under() {
        assert_eq!(
            room_full_note(MAX_ROOM_TEAMMATES + 1).as_deref(),
            Some("A room holds 8 teammates. Untick 1 to make this one.")
        );
        assert!(room_full_note(MAX_ROOM_TEAMMATES + 3).unwrap().contains("Untick 3"));
        assert_eq!(room_full_note(MAX_ROOM_TEAMMATES), None);
        assert_eq!(room_full_note(1), None);
        assert_eq!(room_full_note(0), None);
    }
}
