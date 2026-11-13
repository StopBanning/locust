//! Room run-end orchestration (`main/room-tasks.ts`):
//! drain queued room members as run slots free up, emit completion notices,
//! and apply `<locust-task>` blocks from finished replies to the room board.

use crate::peer_share::parse_share_blocks;
use crate::room_store::{
    ApplyTaskOpsResult, PublicRoom, RoomPost, RoomTaskRequest, RosterEntry, TaskActor,
};
use crate::room_task::{
    ClaimCandidateTask, RosterNameRef, TaskOp, parse_task_blocks, row_to_claim_at_start,
};

/// Never start more than this many waiting room members on one run ending (`MAX_DRAIN_PER_RUN`).
pub const MAX_DRAIN_PER_RUN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomMissionLocation {
    pub room: PublicRoom,
    pub teammate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomNotice {
    pub room_id: String,
    pub room_name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomMemberInfo {
    pub teammate_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionOutcomeRecord {
    pub phase: String,
    pub final_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartMemberOutcome {
    Started { mission_id: String },
    NoSlot,
    Busy,
    Refused { message: String },
}

pub trait RoomTasksHost {
    fn list_rooms(&self) -> Result<Vec<PublicRoom>, String>;
    fn list_teammates(&self) -> Result<Vec<RoomMemberInfo>, String>;
    fn start_queued_member(
        &self,
        room: &PublicRoom,
        teammate_id: &str,
        post: &RoomPost,
    ) -> StartMemberOutcome;
    fn mark_start_queued(
        &self,
        room_id: &str,
        post_id: &str,
        teammate_id: &str,
        mission_id: &str,
    ) -> Result<(), String>;
    fn mark_refuse_queued(
        &self,
        room_id: &str,
        post_id: &str,
        teammate_id: &str,
        message: &str,
    ) -> Result<(), String>;
    fn assign_task_at_start(
        &self,
        request: RoomTaskRequest,
    ) -> Result<(), String>;
    fn get_mission(&self, mission_id: &str) -> Result<Option<MissionOutcomeRecord>, String>;
    fn apply_task_ops(
        &self,
        room_id: &str,
        ops: &[TaskOp],
        actor: &TaskActor<'_>,
        roster: &[RosterEntry<'_>],
    ) -> Result<ApplyTaskOpsResult, String>;
    fn notify(&self, notice: RoomNotice);
}

/// Which room and teammate started `mission_id`, if any (`roomMissionOf`).
pub fn room_mission_of(rooms: &[PublicRoom], mission_id: &str) -> Option<RoomMissionLocation> {
    for room in rooms {
        for post in &room.posts {
            for (teammate_id, id) in &post.missions {
                if id == mission_id {
                    return Some(RoomMissionLocation {
                        room: room.clone(),
                        teammate_id: teammate_id.clone(),
                    });
                }
            }
        }
    }
    None
}

fn phase_is_ended(phase: &str) -> bool {
    matches!(phase, "completed" | "failed" | "cancelled")
}

/// Claim an unambiguous named row at start without letting an assignment failure undo a started run.
pub fn try_claim_row_at_start<F>(
    post_text: &str,
    room: &PublicRoom,
    roster: &[RoomMemberInfo],
    started_teammate_id: &str,
    assign_fn: F,
) where
    F: FnOnce(RoomTaskRequest) -> Result<(), String>,
{
    let task_refs: Vec<ClaimCandidateTask<'_>> = room
        .tasks
        .iter()
        .map(|t| ClaimCandidateTask {
            task_id: &t.task_id,
            text: &t.text,
            state: &t.state,
            owner_id: t.owner_id.as_deref(),
        })
        .collect();
    let member_refs: Vec<RosterNameRef<'_>> = roster
        .iter()
        .filter(|m| room.teammate_ids.contains(&m.teammate_id))
        .map(|m| RosterNameRef {
            teammate_id: &m.teammate_id,
            name: &m.name,
        })
        .collect();
    if let Some(task_id) = row_to_claim_at_start(post_text, &task_refs, &member_refs, started_teammate_id)
    {
        let _ = assign_fn(RoomTaskRequest::Assign {
            room_id: room.room_id.clone(),
            task_id: task_id.to_owned(),
            owner_id: Some(started_teammate_id.to_owned()),
        });
    }
}

pub fn on_run_ended<H: RoomTasksHost>(host: &H, ended_mission_id: &str) {
    let mut rooms = match host.list_rooms() {
        Ok(r) => r,
        Err(_) => return,
    };

    // 1. Drain queued room members in room/post/member order.
    let has_queued = rooms
        .iter()
        .any(|room| room.posts.iter().any(|post| !post.queued.is_empty()));
    if has_queued {
        let mut started = 0usize;
        let roster = host.list_teammates().unwrap_or_default();
        'drain: for room in &rooms {
            for post in &room.posts {
                for teammate_id in &post.queued {
                    if started >= MAX_DRAIN_PER_RUN {
                        break 'drain;
                    }
                    let outcome = host.start_queued_member(room, teammate_id, post);
                    match outcome {
                        StartMemberOutcome::NoSlot => {
                            break 'drain;
                        }
                        StartMemberOutcome::Busy => continue,
                        StartMemberOutcome::Refused { message } => {
                            let _ = host.mark_refuse_queued(
                                &room.room_id,
                                &post.post_id,
                                teammate_id,
                                &message,
                            );
                            host.notify(RoomNotice {
                                room_id: room.room_id.clone(),
                                room_name: room.name.clone(),
                                message: String::new(),
                            });
                        }
                        StartMemberOutcome::Started { mission_id } => {
                            started += 1;
                            if host
                                .mark_start_queued(
                                    &room.room_id,
                                    &post.post_id,
                                    teammate_id,
                                    &mission_id,
                                )
                                .is_ok()
                            {
                                try_claim_row_at_start(
                                    &post.text,
                                    room,
                                    &roster,
                                    teammate_id,
                                    |req| host.assign_task_at_start(req),
                                );
                                host.notify(RoomNotice {
                                    room_id: room.room_id.clone(),
                                    room_name: room.name.clone(),
                                    message: String::new(),
                                });
                            }
                        }
                    }
                }
            }
        }
        // Re-read after draining so completion checks reflect newly started members.
        if let Ok(fresh) = host.list_rooms() {
            rooms = fresh;
        }
    }

    // 2. Check if the ended mission belongs to a room post.
    let Some(found) = room_mission_of(&rooms, ended_mission_id) else {
        return;
    };
    let post = found
        .room
        .posts
        .iter()
        .find(|entry| entry.missions.values().any(|id| id == ended_mission_id));

    let Ok(Some(recovered)) = host.get_mission(ended_mission_id) else {
        return;
    };

    if let Some(post) = post {
        let mut all_done = true;
        for id in post.missions.values() {
            if id == ended_mission_id {
                continue;
            }
            match host.get_mission(id) {
                Ok(Some(other)) if phase_is_ended(&other.phase) => {}
                _ => {
                    all_done = false;
                    break;
                }
            }
        }
        let waiting = post.queued.len();
        let missing = found
            .room
            .teammate_ids
            .iter()
            .filter(|id| !post.missions.contains_key(*id) && !post.queued.contains(*id))
            .count();
        if all_done && post.missions.len() > 1 {
            let answered = post.missions.len();
            let total = found.room.teammate_ids.len();
            let message = if waiting > 0 {
                format!(
                    "{answered} of {total} in {} answered; {waiting} still waiting for a slot.",
                    found.room.name
                )
            } else if missing == 0 {
                format!("Everyone in {} has answered.", found.room.name)
            } else {
                format!(
                    "{answered} of {total} in {} answered; {missing} never started.",
                    found.room.name
                )
            };
            host.notify(RoomNotice {
                room_id: found.room.room_id.clone(),
                room_name: found.room.name.clone(),
                message,
            });
        }
    }

    // 3. Apply any `<locust-task>` blocks from the finished final reply.
    let Some(text) = recovered.final_text else {
        return;
    };
    let ops = parse_task_blocks(&text);
    if ops.is_empty() {
        return;
    }
    let roster = host.list_teammates().unwrap_or_default();
    let actor_name = roster
        .iter()
        .find(|entry| entry.teammate_id == found.teammate_id)
        .map(|entry| entry.name.as_str())
        .unwrap_or(&found.teammate_id);
    let noted_to: Vec<String> = parse_share_blocks(&text)
        .into_iter()
        .filter_map(|block| {
            roster
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(block.to.trim()))
                .map(|entry| entry.teammate_id.clone())
        })
        .collect();
    let actor = TaskActor {
        teammate_id: &found.teammate_id,
        name: actor_name,
        mission_id: ended_mission_id,
        noted_to: Some(&noted_to),
    };
    let roster_refs: Vec<RosterEntry<'_>> = roster
        .iter()
        .map(|m| RosterEntry {
            teammate_id: &m.teammate_id,
            name: &m.name,
        })
        .collect();
    if let Ok(res) = host.apply_task_ops(&found.room.room_id, &ops, &actor, &roster_refs)
        && (!res.changed.is_empty() || !res.refused.is_empty())
    {
        let mut parts = res.changed;
        parts.extend(res.refused);
        host.notify(RoomNotice {
            room_id: found.room.room_id.clone(),
            room_name: found.room.name.clone(),
            message: parts.join(" "),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    struct FakeHost {
        rooms: RefCell<Vec<PublicRoom>>,
        teammates: Vec<RoomMemberInfo>,
        missions: RefCell<BTreeMap<String, Option<MissionOutcomeRecord>>>,
        started: RefCell<Vec<String>>,
        fail_start_queued: RefCell<bool>,
        start_outcomes: RefCell<BTreeMap<String, StartMemberOutcome>>,
        notices: RefCell<Vec<RoomNotice>>,
        ops_calls: RefCell<Vec<(String, Vec<TaskOp>, String)>>,
    }

    impl FakeHost {
        fn new(rooms: Vec<PublicRoom>) -> Self {
            Self {
                rooms: RefCell::new(rooms),
                teammates: vec![
                    RoomMemberInfo {
                        teammate_id: "tm_wren".into(),
                        name: "Wren".into(),
                    },
                    RoomMemberInfo {
                        teammate_id: "tm_booty".into(),
                        name: "Booty".into(),
                    },
                    RoomMemberInfo {
                        teammate_id: "tm_ash".into(),
                        name: "Ash".into(),
                    },
                    RoomMemberInfo {
                        teammate_id: "tm_quill".into(),
                        name: "Quill".into(),
                    },
                ],
                missions: RefCell::new(BTreeMap::new()),
                started: RefCell::new(Vec::new()),
                fail_start_queued: RefCell::new(false),
                start_outcomes: RefCell::new(BTreeMap::new()),
                notices: RefCell::new(Vec::new()),
                ops_calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl RoomTasksHost for FakeHost {
        fn list_rooms(&self) -> Result<Vec<PublicRoom>, String> {
            Ok(self.rooms.borrow().clone())
        }
        fn list_teammates(&self) -> Result<Vec<RoomMemberInfo>, String> {
            Ok(self.teammates.clone())
        }
        fn start_queued_member(
            &self,
            _room: &PublicRoom,
            teammate_id: &str,
            _post: &RoomPost,
        ) -> StartMemberOutcome {
            self.started.borrow_mut().push(teammate_id.to_owned());
            self.start_outcomes
                .borrow()
                .get(teammate_id)
                .cloned()
                .unwrap_or_else(|| StartMemberOutcome::Started {
                    mission_id: format!("m_{teammate_id}"),
                })
        }
        fn mark_start_queued(
            &self,
            room_id: &str,
            post_id: &str,
            teammate_id: &str,
            mission_id: &str,
        ) -> Result<(), String> {
            if *self.fail_start_queued.borrow() {
                return Err("write failed".into());
            }
            for room in self.rooms.borrow_mut().iter_mut() {
                if room.room_id == room_id {
                    for post in room.posts.iter_mut() {
                        if post.post_id == post_id {
                            post.queued.retain(|id| id != teammate_id);
                            post.missions
                                .insert(teammate_id.to_owned(), mission_id.to_owned());
                        }
                    }
                }
            }
            Ok(())
        }
        fn mark_refuse_queued(
            &self,
            room_id: &str,
            post_id: &str,
            teammate_id: &str,
            message: &str,
        ) -> Result<(), String> {
            for room in self.rooms.borrow_mut().iter_mut() {
                if room.room_id == room_id {
                    for post in room.posts.iter_mut() {
                        if post.post_id == post_id {
                            post.queued.retain(|id| id != teammate_id);
                            post.refused.insert(teammate_id.to_owned(), message.to_owned());
                        }
                    }
                }
            }
            Ok(())
        }
        fn assign_task_at_start(&self, _request: RoomTaskRequest) -> Result<(), String> {
            Ok(())
        }
        fn get_mission(&self, mission_id: &str) -> Result<Option<MissionOutcomeRecord>, String> {
            Ok(self
                .missions
                .borrow()
                .get(mission_id)
                .cloned()
                .flatten())
        }
        fn apply_task_ops(
            &self,
            room_id: &str,
            ops: &[TaskOp],
            actor: &TaskActor<'_>,
            _roster: &[RosterEntry<'_>],
        ) -> Result<ApplyTaskOpsResult, String> {
            self.ops_calls.borrow_mut().push((
                room_id.to_owned(),
                ops.to_vec(),
                actor.teammate_id.to_owned(),
            ));
            Ok(ApplyTaskOpsResult {
                changed: ops
                    .iter()
                    .map(|o| format!("{} did {}", actor.name, o.kind_str()))
                    .collect(),
                refused: Vec::new(),
            })
        }
        fn notify(&self, notice: RoomNotice) {
            self.notices.borrow_mut().push(notice);
        }
    }

    fn make_room(members: &[&str], posts: Vec<RoomPost>) -> PublicRoom {
        PublicRoom {
            room_id: "room_release".into(),
            name: "Release".into(),
            teammate_ids: members.iter().map(|s| (*s).to_owned()).collect(),
            created_at: "2026-09-24T12:00:00.000Z".into(),
            posts,
            tasks: Vec::new(),
        }
    }

    fn make_post(id: &str, missions: &[(&str, &str)], queued: &[&str]) -> RoomPost {
        RoomPost {
            post_id: id.into(),
            text: "Ship it".into(),
            at: "2026-09-24T12:00:00.000Z".into(),
            missions: missions
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
            queued: queued.iter().map(|s| (*s).to_owned()).collect(),
            refused: BTreeMap::new(),
            to: Vec::new(),
        }
    }

    // ---- main/room-tasks.test.ts (6 cases) ----

    #[test]
    fn finds_room_and_teammate_by_mission_id_recorded_on_post() {
        let room = make_room(
            &["tm_wren", "tm_booty"],
            vec![make_post("post_1", &[("tm_wren", "m_w")], &[])],
        );
        let found = room_mission_of(&[room.clone()], "m_w").unwrap();
        assert_eq!(found.room.room_id, room.room_id);
        assert_eq!(found.teammate_id, "tm_wren");
        assert!(room_mission_of(&[room], "m_unknown").is_none());
    }

    #[test]
    fn reads_reply_task_block_and_moves_board_as_teammate() {
        let room = make_room(
            &["tm_wren"],
            vec![make_post("post_1", &[("tm_wren", "m_w")], &[])],
        );
        let host = FakeHost::new(vec![room]);
        host.missions.borrow_mut().insert(
            "m_w".into(),
            Some(MissionOutcomeRecord {
                phase: "completed".into(),
                final_text: Some("All set.\n<locust-task>\nclaim :: Release notes\n</locust-task>".into()),
            }),
        );
        on_run_ended(&host, "m_w");
        let calls = host.ops_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].2, "tm_wren");
        let notices = host.notices.borrow();
        assert_eq!(notices.len(), 1);
        assert!(notices[0].message.contains("Wren did claim"));
    }

    #[test]
    fn does_nothing_for_no_block_non_room_mission_or_missing_ledger() {
        let room = make_room(
            &["tm_wren"],
            vec![make_post("post_1", &[("tm_wren", "m_w")], &[])],
        );
        let host = FakeHost::new(vec![room]);
        host.missions.borrow_mut().insert(
            "m_w".into(),
            Some(MissionOutcomeRecord {
                phase: "completed".into(),
                final_text: Some("Plain answer.".into()),
            }),
        );
        on_run_ended(&host, "m_w");
        on_run_ended(&host, "m_outside");
        host.missions.borrow_mut().insert("m_missing".into(), None);
        on_run_ended(&host, "m_missing");
        assert!(host.ops_calls.borrow().is_empty());
        assert!(host.notices.borrow().is_empty());
    }

    #[test]
    fn says_everyone_answered_once_others_ended_on_record() {
        let room = make_room(
            &["tm_wren", "tm_booty"],
            vec![make_post(
                "post_1",
                &[("tm_wren", "m_w"), ("tm_booty", "m_b")],
                &[],
            )],
        );
        let host = FakeHost::new(vec![room]);
        host.missions.borrow_mut().insert(
            "m_w".into(),
            Some(MissionOutcomeRecord {
                phase: "completed".into(),
                final_text: Some("Done.".into()),
            }),
        );
        host.missions.borrow_mut().insert(
            "m_b".into(),
            Some(MissionOutcomeRecord {
                phase: "failed".into(),
                final_text: None,
            }),
        );
        on_run_ended(&host, "m_w");
        let notices = host.notices.borrow();
        assert_eq!(
            notices.as_slice(),
            &[RoomNotice {
                room_id: "room_release".into(),
                room_name: "Release".into(),
                message: "Everyone in Release has answered.".into()
            }]
        );
    }

    #[test]
    fn does_not_say_everyone_while_another_running_interrupted_or_missing() {
        for other_phase in [Some("running"), Some("interrupted"), None] {
            let room = make_room(
                &["tm_wren", "tm_booty"],
                vec![make_post(
                    "post_1",
                    &[("tm_wren", "m_w"), ("tm_booty", "m_b")],
                    &[],
                )],
            );
            let host = FakeHost::new(vec![room]);
            host.missions.borrow_mut().insert(
                "m_w".into(),
                Some(MissionOutcomeRecord {
                    phase: "completed".into(),
                    final_text: Some("Done.".into()),
                }),
            );
            host.missions.borrow_mut().insert(
                "m_b".into(),
                other_phase.map(|p| MissionOutcomeRecord {
                    phase: p.into(),
                    final_text: None,
                }),
            );
            on_run_ended(&host, "m_w");
            assert!(host.notices.borrow().is_empty());
        }
    }

    #[test]
    fn does_not_call_four_of_six_everyone() {
        let room = make_room(
            &["tm_1", "tm_2", "tm_3", "tm_4", "tm_5", "tm_6"],
            vec![make_post(
                "post_1",
                &[
                    ("tm_1", "m_1"),
                    ("tm_2", "m_2"),
                    ("tm_3", "m_3"),
                    ("tm_4", "m_4"),
                ],
                &[],
            )],
        );
        let host = FakeHost::new(vec![room]);
        for i in 1..=4 {
            host.missions.borrow_mut().insert(
                format!("m_{i}"),
                Some(MissionOutcomeRecord {
                    phase: "completed".into(),
                    final_text: Some("ok".into()),
                }),
            );
        }
        on_run_ended(&host, "m_1");
        let notices = host.notices.borrow();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].message, "4 of 6 in Release answered; 2 never started.");
    }

    // ---- main/room-queue-drains.test.ts (8 cases) ----

    #[test]
    fn queue_starts_waiting_members_in_room_order() {
        let room = make_room(
            &["tm_wren", "tm_booty", "tm_ash"],
            vec![make_post(
                "post_1",
                &[("tm_wren", "m_finished")],
                &["tm_booty", "tm_ash"],
            )],
        );
        let host = FakeHost::new(vec![room]);
        on_run_ended(&host, "m_outside");
        assert_eq!(host.started.borrow().as_slice(), &["tm_booty", "tm_ash"]);
        let rooms = host.rooms.borrow();
        assert!(rooms[0].posts[0].queued.is_empty());
        assert_eq!(
            rooms[0].posts[0].missions.get("tm_booty").map(String::as_str),
            Some("m_tm_booty")
        );
    }

    #[test]
    fn queue_stops_asking_once_pool_is_full_again() {
        let room = make_room(
            &["tm_wren", "tm_booty", "tm_ash"],
            vec![make_post("post_1", &[], &["tm_wren", "tm_booty", "tm_ash"])],
        );
        let host = FakeHost::new(vec![room]);
        host.start_outcomes
            .borrow_mut()
            .insert("tm_booty".into(), StartMemberOutcome::NoSlot);
        on_run_ended(&host, "m_outside");
        assert_eq!(host.started.borrow().as_slice(), &["tm_wren", "tm_booty"]);
        assert_eq!(
            host.rooms.borrow()[0].posts[0].queued,
            vec!["tm_booty", "tm_ash"]
        );
    }

    #[test]
    fn queue_passes_refused_records_why_and_keeps_going() {
        let room = make_room(
            &["tm_wren", "tm_booty", "tm_ash"],
            vec![make_post("post_1", &[], &["tm_booty", "tm_ash"])],
        );
        let host = FakeHost::new(vec![room]);
        host.start_outcomes.borrow_mut().insert(
            "tm_booty".into(),
            StartMemberOutcome::Refused {
                message: "Antigravity missing.".into(),
            },
        );
        on_run_ended(&host, "m_outside");
        let rooms = host.rooms.borrow();
        assert!(rooms[0].posts[0].queued.is_empty());
        assert_eq!(
            rooms[0].posts[0].refused.get("tm_booty").map(String::as_str),
            Some("Antigravity missing.")
        );
        assert_eq!(
            rooms[0].posts[0].missions.get("tm_ash").map(String::as_str),
            Some("m_tm_ash")
        );
    }

    #[test]
    fn queue_skips_busy_member_asks_next_and_records_nothing_for_busy() {
        let room = make_room(
            &["tm_booty", "tm_ash"],
            vec![make_post("post_1", &[], &["tm_booty", "tm_ash"])],
        );
        let host = FakeHost::new(vec![room]);
        host.start_outcomes
            .borrow_mut()
            .insert("tm_booty".into(), StartMemberOutcome::Busy);
        on_run_ended(&host, "m_outside");
        let rooms = host.rooms.borrow();
        assert_eq!(rooms[0].posts[0].queued, vec!["tm_booty"]);
        assert!(rooms[0].posts[0].refused.is_empty());
        assert_eq!(
            rooms[0].posts[0].missions.get("tm_ash").map(String::as_str),
            Some("m_tm_ash")
        );
    }

    #[test]
    fn queue_does_nothing_when_nobody_is_waiting() {
        let room = make_room(
            &["tm_wren"],
            vec![make_post("post_1", &[("tm_wren", "m_w")], &[])],
        );
        let host = FakeHost::new(vec![room]);
        on_run_ended(&host, "m_outside");
        assert!(host.started.borrow().is_empty());
    }

    #[test]
    fn queue_leaves_run_alive_when_post_update_fails() {
        let room = make_room(
            &["tm_wren", "tm_booty"],
            vec![make_post("post_1", &[], &["tm_booty"])],
        );
        let host = FakeHost::new(vec![room]);
        *host.fail_start_queued.borrow_mut() = true;
        on_run_ended(&host, "m_outside");
        assert_eq!(host.started.borrow().as_slice(), &["tm_booty"]);
        // Because start_queued failed, queued still has tm_booty visible rather than losing track.
        assert_eq!(host.rooms.borrow()[0].posts[0].queued, vec!["tm_booty"]);
    }

    #[test]
    fn queue_survives_host_error_without_throwing() {
        let host = FakeHost::new(vec![make_room(
            &["tm_wren"],
            vec![make_post("post_1", &[("tm_wren", "m_w")], &["tm_booty"])],
        )]);
        host.missions.borrow_mut().insert("m_w".into(), None);
        on_run_ended(&host, "m_w");
        assert_eq!(host.started.borrow().as_slice(), &["tm_booty"]);
    }

    #[test]
    fn queue_still_reads_finished_members_own_task_block() {
        let room = make_room(
            &["tm_wren", "tm_booty"],
            vec![make_post("post_1", &[("tm_wren", "m_w")], &["tm_booty"])],
        );
        let host = FakeHost::new(vec![room]);
        host.missions.borrow_mut().insert(
            "m_w".into(),
            Some(MissionOutcomeRecord {
                phase: "completed".into(),
                final_text: Some(
                    "Done.\n<locust-task>\ndone :: Build installer\n</locust-task>".into(),
                ),
            }),
        );
        on_run_ended(&host, "m_w");
        assert_eq!(host.started.borrow().as_slice(), &["tm_booty"]);
        assert_eq!(host.ops_calls.borrow().len(), 1);
    }

    /// Review (13 Nov): one run ending starts at most MAX_DRAIN_PER_RUN waiting members; the rest stay queued.
    #[test]
    fn queue_starts_no_more_than_eight_on_one_run_ending() {
        let ids: Vec<String> = (0..10).map(|i| format!("tm_{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let room = make_room(&refs, vec![make_post("post_1", &[], &refs)]);
        let host = FakeHost::new(vec![room]);
        on_run_ended(&host, "m_outside");
        assert_eq!(host.started.borrow().len(), MAX_DRAIN_PER_RUN);
        assert_eq!(host.rooms.borrow()[0].posts[0].queued, vec!["tm_8", "tm_9"]);
    }
}
