//! Native room host operations (`main/index.ts` room handlers & `rooms.rs`):
//! builds room history, briefs and launches room member turns through `EngineApi::send`,
//! posts to a room (`post_to_room`) queuing past the live cap, and hooks run completion.

use crate::approval::Mode;
use crate::engine::Teammate;
use crate::engine_api::api;
use crate::native::{self, Origin, Send as SendJob};
use crate::room_history::{
    RoomHistory, RoomHistoryAnswer, RoomHistoryPost, RoomHistoryRecipient,
};
use crate::room_store::{
    self, NewPostInput, PublicRoom, RecipientsResult, RoomPost, RoomTaskRequest, RosterEntry,
    TaskActor,
};
use crate::room_task::{BoardTaskRef, TaskBoardState, board_lines, fitted_task_section, utf16_len};
use crate::room_tasks::{
    self, MissionOutcomeRecord, RoomMemberInfo, RoomNotice, RoomTasksHost, StartMemberOutcome,
    try_claim_row_at_start,
};
use crate::store::Store;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const MAX_PROMPT_LENGTH: usize = 8_000;
const GONE_ANSWER_NAME: &str = "A teammate no longer on the team";
const GONE_TO_NAME: &str = "a teammate no longer on the team";

#[derive(Debug, Clone)]
pub struct RoomPostOutcome {
    pub post: RoomPost,
    pub refusal_message: Option<String>,
}

fn task_state_from_str(s: &str) -> TaskBoardState {
    match s {
        "done" => TaskBoardState::Done,
        "in-hand" => TaskBoardState::InHand,
        _ => TaskBoardState::Open,
    }
}

fn roster_entries(store: &Store) -> Vec<(Teammate, Option<String>, Option<String>)> {
    let Ok(team_file) = crate::people::team(store) else {
        return Vec::new();
    };
    team_file["teammates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let id = row["teammateId"].as_str()?.to_owned();
            let name = row["name"].as_str()?.to_owned();
            let route = &row["route"];
            let runtime = route["runtime"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| Some("codex".to_owned()));
            let model = route["model"]
                .as_str()
                .filter(|m| *m != "account-default")
                .map(str::to_owned);
            let mode = route["mode"].as_str().unwrap_or("ask").to_owned();
            let account = route["account"].as_str().map(str::to_owned);
            let effort = route["effort"].as_str().map(str::to_owned);
            let folder = row["folder"].as_str().map(str::to_owned);
            let tm = Teammate {
                id,
                name,
                role: row["role"].as_str().unwrap_or("").to_owned(),
                runtime,
                model,
                runs_in: mode,
                account,
                busy: false,
                folder,
            };
            Some((tm, effort, row["role"].as_str().map(str::to_owned)))
        })
        .collect()
}

/// Earlier posts and finished answers in `room` strictly before `before_post_id` (`roomHistoryOf`).
pub fn room_history_of(
    store: &Store,
    room: &PublicRoom,
    before_post_id: &str,
    roster: &[RoomMemberInfo],
) -> RoomHistory {
    let end = room
        .posts
        .iter()
        .position(|p| p.post_id == before_post_id)
        .unwrap_or(room.posts.len());
    let posts = room.posts[..end]
        .iter()
        .map(|entry| {
            let mut answers = Vec::new();
            for (teammate_id, mission_id) in &entry.missions {
                let Some(rec) = mission_record(store, mission_id) else {
                    continue;
                };
                if !matches!(rec.phase.as_str(), "completed" | "failed" | "cancelled") {
                    continue;
                }
                let Some(text) = rec.final_text else {
                    continue;
                };
                let name = roster
                    .iter()
                    .find(|m| m.teammate_id == *teammate_id)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| GONE_ANSWER_NAME.to_owned());
                answers.push(RoomHistoryAnswer {
                    teammate_id: teammate_id.clone(),
                    name,
                    text,
                });
            }
            let to = entry
                .to
                .iter()
                .map(|teammate_id| RoomHistoryRecipient {
                    teammate_id: teammate_id.clone(),
                    name: roster
                        .iter()
                        .find(|m| m.teammate_id == *teammate_id)
                        .map(|m| m.name.clone())
                        .unwrap_or_else(|| GONE_TO_NAME.to_owned()),
                })
                .collect();
            RoomHistoryPost {
                text: entry.text.clone(),
                to,
                answers,
            }
        })
        .collect();
    RoomHistory {
        room_name: room.name.clone(),
        posts,
    }
}

fn mission_record(store: &Store, mission_id: &str) -> Option<MissionOutcomeRecord> {
    let path = store.ledger_dir().ok()?.join(format!("{mission_id}.jsonl"));
    let text = std::fs::read_to_string(path).ok()?;
    let mut events = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            events.push(v);
        }
    }
    let phase = crate::carry_on::how_turn_ended(&events)
        .unwrap_or("running")
        .to_owned();
    let final_text = crate::carry_on::latest_final(&events);
    Some(MissionOutcomeRecord { phase, final_text })
}

fn classify_send_error(error: &str) -> StartMemberOutcome {
    if error == native::too_many_live() {
        return StartMemberOutcome::NoSlot;
    }
    if native::is_room_teammate_busy_error(error)
        || error == "A turn is already running in this conversation."
    {
        return StartMemberOutcome::Busy;
    }
    StartMemberOutcome::Refused {
        message: error.to_owned(),
    }
}

/// Start one room member's mission (`startRoomMember`).
pub fn start_room_member(
    store: &Store,
    room: &PublicRoom,
    teammate_id: &str,
    post: &RoomPost,
) -> (StartMemberOutcome, String) {
    let roster_full = roster_entries(store);
    let roster_info: Vec<RoomMemberInfo> = roster_full
        .iter()
        .map(|(t, _, _)| RoomMemberInfo {
            teammate_id: t.id.clone(),
            name: t.name.clone(),
        })
        .collect();
    let Some((teammate, effort, _)) = roster_full
        .iter()
        .find(|(t, _, _)| t.id == teammate_id)
        .cloned()
    else {
        return (
            StartMemberOutcome::Refused {
                message: "No longer on the roster.".to_owned(),
            },
            teammate_id.to_owned(),
        );
    };
    let name = teammate.name.clone();
    let runtime = teammate.runtime.as_deref().unwrap_or("codex");
    if runtime == "antigravity" && crate::runtime::find_executable("agy").is_err() {
        return (
            StartMemberOutcome::Refused {
                message: "Antigravity can be posted to from a room once Antigravity CLI is installed."
                    .to_owned(),
            },
            name,
        );
    }

    let member_names: Vec<String> = room
        .teammate_ids
        .iter()
        .map(|id| {
            roster_info
                .iter()
                .find(|m| m.teammate_id == *id)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| id.clone())
        })
        .collect();
    let member_name_refs: Vec<&str> = member_names.iter().map(String::as_str).collect();
    let board_refs: Vec<BoardTaskRef<'_>> = room
        .tasks
        .iter()
        .map(|t| BoardTaskRef {
            text: &t.text,
            state: task_state_from_str(&t.state),
            owner_id: t.owner_id.as_deref(),
            handed_by: t.handed_by.as_deref(),
            handed_without_note: t.handed_without_note,
        })
        .collect();
    let line_refs: Vec<crate::room_task::RosterNameRef<'_>> = roster_info
        .iter()
        .map(|m| crate::room_task::RosterNameRef {
            teammate_id: &m.teammate_id,
            name: &m.name,
        })
        .collect();
    let lines = board_lines(&board_refs, &line_refs);
    let budget = MAX_PROMPT_LENGTH.saturating_sub(utf16_len(&post.text) + 2);
    let board = fitted_task_section(
        &crate::room_task::TaskSectionInput {
            room_name: &room.name,
            self_name: &name,
            member_names: &member_name_refs,
            tasks: &lines,
        },
        budget,
    );
    let briefed = if board.is_empty() {
        post.text.clone()
    } else {
        format!("{}\n\n{board}", post.text)
    };
    let history = room_history_of(store, room, &post.post_id, &roster_info);
    let room_history = (!history.posts.is_empty()).then_some(history);

    let mode = Mode::parse(&teammate.runs_in);
    let account = teammate.account.clone();
    let job = SendJob {
        teammate,
        conversation: None,
        prompt: briefed,
        model: None,
        effort,
    };
    let origin = Origin {
        mode,
        folder: None,
        started_by: Some(json!({
            "kind": "room",
            "roomId": room.room_id,
            "postId": post.post_id,
        })),
        runtime: None,
        account: account.map(Some),
        reference_ids: None,
        slot: None,
        include_messages: Vec::new(),
        relay: None,
        left_out_by_you: Vec::new(),
        room_history,
        rewind: None,
    };
    match api().send(job, origin) {
        Ok(started) => (
            StartMemberOutcome::Started {
                mission_id: started.conversation_id,
            },
            name,
        ),
        Err(err) => (classify_send_error(&err), name),
    }
}

/// Post to a room: write the post immediately with asked members queued, then start as many as fit.
pub fn post_to_room(
    store: &Store,
    room_id: &str,
    text: &str,
    to: Option<&[String]>,
) -> Result<RoomPostOutcome, String> {
    if !room_store::valid_post_text(text) {
        return Err("Write something to post.".to_owned());
    }
    let Some(room) = room_store::get(store, room_id)? else {
        return Err("That room no longer exists.".to_owned());
    };
    let (asked, normalized_to) = match room_store::recipients_of(&room.teammate_ids, to) {
        RecipientsResult::Ok { asked, to } => (asked, to.unwrap_or_default()),
        RecipientsResult::Refused { message } => return Err(message),
    };
    let added = room_store::add_post(
        store,
        room_id,
        NewPostInput {
            text: text.to_owned(),
            missions: BTreeMap::new(),
            queued: asked.clone(),
            refused: BTreeMap::new(),
            to: normalized_to,
        },
    )?;

    let mut refused_parts: Vec<(String, String)> = Vec::new();
    for teammate_id in asked {
        let fresh_room = room_store::get(store, room_id)?.unwrap_or_else(|| room.clone());
        let (outcome, name) = start_room_member(store, &fresh_room, &teammate_id, &added);
        match outcome {
            StartMemberOutcome::NoSlot | StartMemberOutcome::Busy => {}
            StartMemberOutcome::Refused { message } => {
                let _ =
                    room_store::refuse_queued(store, room_id, &added.post_id, &teammate_id, &message);
                refused_parts.push((name, message));
            }
            StartMemberOutcome::Started { mission_id } => {
                let _ = room_store::start_queued(
                    store,
                    room_id,
                    &added.post_id,
                    &teammate_id,
                    &mission_id,
                );
                let roster_info: Vec<RoomMemberInfo> = roster_entries(store)
                    .into_iter()
                    .map(|(t, _, _)| RoomMemberInfo {
                        teammate_id: t.id,
                        name: t.name,
                    })
                    .collect();
                if let Ok(Some(latest_room)) = room_store::get(store, room_id) {
                    try_claim_row_at_start(
                        &added.text,
                        &latest_room,
                        &roster_info,
                        &teammate_id,
                        |req| room_store::update_task(store, req).map(|_| ()),
                    );
                }
            }
        }
    }
    let final_post = room_store::get(store, room_id)?
        .and_then(|r| r.posts.into_iter().find(|p| p.post_id == added.post_id))
        .unwrap_or(added);
    let refusal_message = format_refusal_notice(&refused_parts);
    Ok(RoomPostOutcome {
        post: final_post,
        refusal_message,
    })
}

pub fn format_refusal_notice(refused: &[(String, String)]) -> Option<String> {
    if refused.is_empty() {
        return None;
    }
    let mut order: Vec<String> = Vec::new();
    let mut by_reason: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, message) in refused {
        if !by_reason.contains_key(message) {
            order.push(message.clone());
        }
        by_reason
            .entry(message.clone())
            .or_default()
            .push(name.clone());
    }
    let sentences: Vec<String> = order
        .into_iter()
        .map(|reason| {
            let names = &by_reason[&reason];
            let lower = reason
                .chars()
                .next()
                .map(|c| c.to_lowercase().to_string() + &reason[c.len_utf8()..])
                .unwrap_or(reason);
            let joined = match names.len() {
                0 => String::new(),
                1 => names[0].clone(),
                _ => format!(
                    "{} and {}",
                    names[..names.len() - 1].join(", "),
                    names[names.len() - 1]
                ),
            };
            format!("{joined} could not start: {lower}")
        })
        .collect();
    Some(sentences.join(" "))
}

struct NativeRoomHost<'a> {
    store: &'a Store,
}

impl RoomTasksHost for NativeRoomHost<'_> {
    fn list_rooms(&self) -> Result<Vec<PublicRoom>, String> {
        room_store::list(self.store)
    }
    fn list_teammates(&self) -> Result<Vec<RoomMemberInfo>, String> {
        Ok(roster_entries(self.store)
            .into_iter()
            .map(|(t, _, _)| RoomMemberInfo {
                teammate_id: t.id,
                name: t.name,
            })
            .collect())
    }
    fn start_queued_member(
        &self,
        room: &PublicRoom,
        teammate_id: &str,
        post: &RoomPost,
    ) -> StartMemberOutcome {
        start_room_member(self.store, room, teammate_id, post).0
    }
    fn mark_start_queued(
        &self,
        room_id: &str,
        post_id: &str,
        teammate_id: &str,
        mission_id: &str,
    ) -> Result<(), String> {
        room_store::start_queued(self.store, room_id, post_id, teammate_id, mission_id)
    }
    fn mark_refuse_queued(
        &self,
        room_id: &str,
        post_id: &str,
        teammate_id: &str,
        message: &str,
    ) -> Result<(), String> {
        room_store::refuse_queued(self.store, room_id, post_id, teammate_id, message)
    }
    fn assign_task_at_start(&self, request: RoomTaskRequest) -> Result<(), String> {
        room_store::update_task(self.store, request).map(|_| ())
    }
    fn get_mission(&self, mission_id: &str) -> Result<Option<MissionOutcomeRecord>, String> {
        Ok(mission_record(self.store, mission_id))
    }
    fn apply_task_ops(
        &self,
        room_id: &str,
        ops: &[crate::room_task::TaskOp],
        actor: &TaskActor<'_>,
        roster: &[RosterEntry<'_>],
    ) -> Result<room_store::ApplyTaskOpsResult, String> {
        room_store::apply_task_ops(self.store, room_id, ops, actor, roster)
    }
    fn notify(&self, notice: RoomNotice) {
        native::push_room_notice(notice);
    }
}

/// Called from `native.rs` when any non-side run finishes (`roomTasks.onRunEnded`).
pub fn on_run_ended(store: &Store, mission_id: &str) {
    let host = NativeRoomHost { store };
    room_tasks::on_run_ended(&host, mission_id);
}

#[cfg(test)]
mod review_tests {
    use super::*;

    /// Review (13 Nov): a member already busy in another run is Busy (asked again later), not a refusal said in the room.
    #[test]
    fn a_member_busy_in_another_run_is_busy_not_refused() {
        assert!(matches!(classify_send_error("Wren is already active in another run."), StartMemberOutcome::Busy));
        assert!(matches!(classify_send_error("A turn is already running in this conversation."), StartMemberOutcome::Busy));
        assert!(matches!(classify_send_error("Something else went wrong."), StartMemberOutcome::Refused { .. }));
    }
}
