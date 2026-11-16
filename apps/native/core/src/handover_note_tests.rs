//! Electron `main/a-handoff-without-a-note-says-so.test.ts` (A2.6): a task handed over arrives with the teammate's own
//! message, and when the reply wrote none the new owner's row says so in the host's words. The reply is read by the
//! real run-end path (`room_tasks::on_run_ended`) into the real room file (`room_store`), and the row is the real
//! board brief (`room_task::task_section`).

use crate::room_store::{self, ApplyTaskOpsResult, NewPostInput, PublicRoom, RoomPost, RoomTask, RoomTaskRequest, RosterEntry, TaskActor};
use crate::room_task::{board_lines, task_section, BoardTaskRef, RosterNameRef, TaskBoardState, TaskOp, TaskSectionInput};
use crate::room_tasks::{on_run_ended, MissionOutcomeRecord, RoomMemberInfo, RoomNotice, RoomTasksHost, StartMemberOutcome};
use crate::store::Store;
use std::collections::BTreeMap;
use std::path::PathBuf;

struct Host {
    dir: PathBuf,
    store: Store,
    reply: String,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn roster() -> Vec<RoomMemberInfo> {
    vec![
        RoomMemberInfo { teammate_id: "tm_wren".into(), name: "Wren".into() },
        RoomMemberInfo { teammate_id: "tm_booty".into(), name: "Booty".into() },
    ]
}

impl RoomTasksHost for Host {
    fn list_rooms(&self) -> Result<Vec<PublicRoom>, String> {
        room_store::list(&self.store)
    }
    fn list_teammates(&self) -> Result<Vec<RoomMemberInfo>, String> {
        Ok(roster())
    }
    fn start_queued_member(&self, _room: &PublicRoom, _teammate_id: &str, _post: &RoomPost) -> StartMemberOutcome {
        StartMemberOutcome::NoSlot
    }
    fn mark_start_queued(&self, _room_id: &str, _post_id: &str, _teammate_id: &str, _mission_id: &str) -> Result<(), String> {
        Ok(())
    }
    fn mark_refuse_queued(&self, _room_id: &str, _post_id: &str, _teammate_id: &str, _message: &str) -> Result<(), String> {
        Ok(())
    }
    fn assign_task_at_start(&self, _request: RoomTaskRequest) -> Result<(), String> {
        Ok(())
    }
    fn get_mission(&self, mission_id: &str) -> Result<Option<MissionOutcomeRecord>, String> {
        Ok((mission_id == "mission_w").then(|| MissionOutcomeRecord { phase: "completed".into(), final_text: Some(self.reply.clone()) }))
    }
    fn apply_task_ops(&self, room_id: &str, ops: &[TaskOp], actor: &TaskActor<'_>, roster: &[RosterEntry<'_>]) -> Result<ApplyTaskOpsResult, String> {
        room_store::apply_task_ops(&self.store, room_id, ops, actor, roster)
    }
    fn notify(&self, _notice: RoomNotice) {}
}

fn state(task: &RoomTask) -> TaskBoardState {
    match task.state.as_str() {
        "done" => TaskBoardState::Done,
        "open" if task.owner_id.is_none() => TaskBoardState::Open,
        _ => TaskBoardState::InHand,
    }
}

/// The test file's `handOver`: Wren answers "Ship it" in the room "Release" with `reply`; returns the board's first
/// task and Booty's row.
fn hand_over(label: &str, reply: &[&str]) -> (RoomTask, String) {
    let dir = std::env::temp_dir().join(format!("locust-handover-{label}-{}-{}", std::process::id(), crate::native::uuid_v4().unwrap()));
    std::fs::create_dir_all(dir.join("root")).unwrap();
    std::fs::create_dir_all(dir.join("electron")).unwrap();
    let store = Store::new(dir.join("root"), dir.join("electron")).unwrap();
    let host = Host { dir, store, reply: reply.join("\n") };
    let room = room_store::create(&host.store, "Release", &["tm_wren".into(), "tm_booty".into()]).unwrap();
    room_store::add_post(
        &host.store,
        &room.room_id,
        NewPostInput {
            text: "Ship it".into(),
            missions: BTreeMap::from([("tm_wren".into(), "mission_w".into())]),
            queued: vec![],
            refused: BTreeMap::new(),
            to: vec![],
        },
    )
    .unwrap();
    on_run_ended(&host, "mission_w");
    let task = room_store::get(&host.store, &room.room_id).unwrap().unwrap().tasks.into_iter().next().expect("a task on the board");
    let names: Vec<RosterNameRef<'_>> = [("tm_wren", "Wren"), ("tm_booty", "Booty")].map(|(teammate_id, name)| RosterNameRef { teammate_id, name }).to_vec();
    let lines = board_lines(
        &[BoardTaskRef {
            text: &task.text,
            state: state(&task),
            owner_id: task.owner_id.as_deref(),
            handed_by: task.handed_by.as_deref(),
            handed_without_note: task.handed_without_note,
        }],
        &names,
    );
    let row = task_section(&TaskSectionInput { room_name: "Release", self_name: "Booty", member_names: &["Wren", "Booty"], tasks: &lines });
    (task, row)
}

#[test]
fn a_task_handed_over_with_no_message_to_its_new_owner_says_so_on_their_row() {
    let (task, row) = hand_over("none", &["Over to Booty.", "<locust-task>", "handoff Booty :: Check the version string", "</locust-task>"]);
    assert!(task.handed_without_note);
    assert!(
        row.contains("Check the version string (yours, handed over by Wren with no note: ask them what is done and what is left before you start)"),
        "{row}"
    );
}

#[test]
fn a_task_handed_over_with_a_message_to_them_in_the_same_reply_says_only_who_handed_it_over() {
    let (task, row) = hand_over(
        "noted",
        &[
            "Over to Booty.",
            "<locust-share to=\"Booty\">I bumped it in package.json and ran the tests; the changelog still says the old version.</locust-share>",
            "<locust-task>",
            "handoff Booty :: Check the version string",
            "</locust-task>",
        ],
    );
    assert!(!task.handed_without_note);
    assert!(row.contains("Check the version string (yours, handed over by Wren)"), "{row}");
    assert!(!row.contains("with no note"), "{row}");
}

#[test]
fn a_message_to_somebody_else_is_not_a_note_to_the_new_owner() {
    let (task, _) = hand_over(
        "other",
        &["<locust-share to=\"Wren\">note to self</locust-share>", "<locust-task>", "handoff Booty :: Check the version string", "</locust-task>"],
    );
    assert!(task.handed_without_note);
}

#[test]
fn the_board_brief_asks_for_the_handover_in_the_teammates_own_words() {
    let brief = task_section(&TaskSectionInput { room_name: "Release", self_name: "Wren", member_names: &["Wren", "Booty"], tasks: &[] });
    assert!(
        brief.contains("When you hand a task over, send that teammate a message in the same reply: what you did, how you checked it, and what is left."),
        "{brief}"
    );
}
