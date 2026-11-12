//! Conversations as a board — port of `renderer/src/conversationBoard.ts`.

use crate::conversation_list::{SidebarMission, by_live_then_recent, conversation_keys};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConversationBoardKey {
    NeedsYou,
    Working,
    ToLookAt,
    Done,
}

impl ConversationBoardKey {
    /// The column's key, as Electron's data names it (the tests compare against it).
    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsYou => "needs-you",
            Self::Working => "working",
            Self::ToLookAt => "to-look-at",
            Self::Done => "done",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::NeedsYou => "Needs you",
            Self::Working => "Working",
            Self::ToLookAt => "Ready to look at",
            Self::Done => "Done",
        }
    }
}

pub const BOARD_TITLE: &str = "Board";
pub const BOARD_CLEAR: &str = "Your board is clear";
pub const BOARD_CLEAR_LINE: &str = "Nothing is waiting on you and nothing is running. Conversations land here by what they need from you: a card to answer, work under way, a finish you have not looked at.";
pub const ALLOW_ONCE: &str = "Allow once";
pub const DENY: &str = "Deny";
pub const ANSWER: &str = "Answer";
pub const REVIEW: &str = "Review";
pub const ALL_QUIET: &str = "all quiet";
pub const NOTHING_RUNNING: &str = "nothing running";
pub const WAITING_ON_YOU: &str = "waiting on you";
pub const WORKING: &str = "working";
pub const WANTS_TO_RUN: &str = "wants to run";

const ORDER: [ConversationBoardKey; 4] = [
    ConversationBoardKey::NeedsYou,
    ConversationBoardKey::Working,
    ConversationBoardKey::ToLookAt,
    ConversationBoardKey::Done,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationCard {
    pub row: SidebarMission,
    pub parent_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationBoardSection {
    pub key: ConversationBoardKey,
    pub title: &'static str,
    pub members: Vec<ConversationCard>,
}

#[derive(Clone, Debug, Default)]
pub struct ConversationBoardFacts {
    pub needs_you: HashSet<String>,
    pub to_look_at: HashSet<String>,
}

/// Room posts map mission ids → room id (enough for folding rooms out of the board).
#[derive(Clone, Debug)]
pub struct BoardRoom {
    pub room_id: String,
    pub mission_ids: Vec<String>,
}

fn any_of(row: &SidebarMission, ids: &HashSet<String>) -> bool {
    !ids.is_empty() && conversation_keys(row).iter().any(|id| ids.contains(id))
}

/// The sidebar's running mark: RUNNING phase (the spark).
pub fn conversation_is_running(row: &SidebarMission) -> bool {
    row.phase == "running"
}

pub fn conversation_column(
    row: &SidebarMission,
    facts: &ConversationBoardFacts,
) -> ConversationBoardKey {
    if any_of(row, &facts.needs_you) {
        return ConversationBoardKey::NeedsYou;
    }
    if conversation_is_running(row) {
        return ConversationBoardKey::Working;
    }
    if any_of(row, &facts.to_look_at) {
        return ConversationBoardKey::ToLookAt;
    }
    ConversationBoardKey::Done
}

fn card_of(row: &SidebarMission) -> ConversationCard {
    ConversationCard {
        parent_id: row.nested_under.clone(),
        row: row.clone(),
    }
}

fn in_trash(row: &SidebarMission, trashed: &HashSet<String>) -> bool {
    conversation_keys(row).iter().any(|id| trashed.contains(id))
}

fn room_mission_ids(rooms: &[BoardRoom]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for room in rooms {
        for id in &room.mission_ids {
            map.insert(id.clone(), room.room_id.clone());
        }
    }
    map
}

/// Fold rooms out: only plain conversations remain (Electron `withRoomsFolded` then keep conversation entries).
fn conversations_without_rooms(
    rows: &[SidebarMission],
    rooms: &[BoardRoom],
) -> Vec<SidebarMission> {
    let room_of = room_mission_ids(rooms);
    rows.iter()
        .filter(|mission| {
            let ids = if mission.member_ids.is_empty() {
                vec![mission.mission_id.clone()]
            } else {
                mission.member_ids.clone()
            };
            !ids.iter().any(|id| room_of.contains_key(id))
        })
        .cloned()
        .collect()
}

pub fn conversation_board(
    rows: &[SidebarMission],
    rooms: &[BoardRoom],
    trashed: &HashSet<String>,
    facts: &ConversationBoardFacts,
) -> Option<Vec<ConversationBoardSection>> {
    let kept: Vec<SidebarMission> = rows
        .iter()
        .filter(|row| !in_trash(row, trashed))
        .cloned()
        .collect();
    let conversations = conversations_without_rooms(&kept, rooms);
    let mut sections: Vec<ConversationBoardSection> = ORDER
        .iter()
        .map(|key| {
            let mut members: Vec<ConversationCard> = conversations
                .iter()
                .filter(|row| conversation_column(row, facts) == *key)
                .map(card_of)
                .collect();
            members.sort_by(|a, b| by_live_then_recent(&a.row, &b.row));
            ConversationBoardSection {
                key: *key,
                title: key.title(),
                members,
            }
        })
        .collect();
    if sections
        .iter()
        .all(|section| section.key == ConversationBoardKey::Done || section.members.is_empty())
    {
        return None;
    }
    sections.retain(|section| !section.members.is_empty());
    Some(sections)
}

pub fn board_meta(sections: Option<&[ConversationBoardSection]>) -> String {
    let Some(sections) = sections else {
        return ALL_QUIET.into();
    };
    let needs = sections
        .iter()
        .find(|s| s.key == ConversationBoardKey::NeedsYou)
        .map(|s| s.members.len())
        .unwrap_or(0);
    let working = sections
        .iter()
        .find(|s| s.key == ConversationBoardKey::Working)
        .map(|s| s.members.len())
        .unwrap_or(0);
    let mut parts = Vec::new();
    if needs > 0 {
        parts.push(format!("{needs} waiting on you"));
    }
    if working > 0 {
        parts.push(format!("{working} working"));
    }
    if parts.is_empty() {
        NOTHING_RUNNING.into()
    } else {
        parts.join(" · ")
    }
}

pub fn under_title(parent_title: &str) -> String {
    format!("under {parent_title}")
}

pub fn in_where(where_: &str) -> String {
    format!("in {where_}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(mission_id: &str, phase: &str) -> SidebarMission {
        SidebarMission {
            mission_id: mission_id.into(),
            title: mission_id.into(),
            phase: phase.into(),
            last_at: None,
            root_id: None,
            member_ids: Vec::new(),
            owner_id: None,
            said: String::new(),
            nested_under: None,
            integrity_issue_count: 0,
        }
    }

    fn ids_in(
        board: &Option<Vec<ConversationBoardSection>>,
    ) -> Vec<(String, &'static str, Option<String>)> {
        board
            .as_ref()
            .into_iter()
            .flatten()
            .flat_map(|section| {
                section.members.iter().map(move |card| {
                    (
                        card.row.mission_id.clone(),
                        section.key.as_str(),
                        card.parent_id.clone(),
                    )
                })
            })
            .collect()
    }

    #[test]
    fn puts_live_run_in_working_and_others_in_done() {
        let board = conversation_board(
            &[
                SidebarMission {
                    last_at: Some("2026-10-04T01:00:00.000Z".into()),
                    ..row("live", "running")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T02:00:00.000Z".into()),
                    ..row("finished", "completed")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T03:00:00.000Z".into()),
                    ..row("broke", "failed")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T04:00:00.000Z".into()),
                    ..row("stopped", "interrupted")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T05:00:00.000Z".into()),
                    ..row("called", "cancelled")
                },
            ],
            &[],
            &HashSet::new(),
            &ConversationBoardFacts::default(),
        );
        let mapped: Vec<_> = board
            .as_ref()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s.key.as_str(),
                    s.title,
                    s.members
                        .iter()
                        .map(|c| c.row.mission_id.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(
            mapped,
            vec![
                ("working", "Working", vec!["live"]),
                (
                    "done",
                    "Done",
                    vec!["called", "stopped", "broke", "finished"]
                ),
            ]
        );
    }

    #[test]
    fn orders_column_newest_first() {
        let board = conversation_board(
            &[
                SidebarMission {
                    last_at: Some("2026-10-04T01:00:00.000Z".into()),
                    ..row("older", "running")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T03:00:00.000Z".into()),
                    ..row("newer", "running")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T02:00:00.000Z".into()),
                    ..row("middle", "running")
                },
            ],
            &[],
            &HashSet::new(),
            &ConversationBoardFacts::default(),
        );
        assert_eq!(
            board.unwrap()[0]
                .members
                .iter()
                .map(|c| c.row.mission_id.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "middle", "older"]
        );
    }

    #[test]
    fn leaves_room_and_trashed_out() {
        let rooms = [BoardRoom {
            room_id: "room_release".into(),
            mission_ids: vec!["room-answer".into()],
        }];
        let trashed: HashSet<String> = ["root-trashed".into()].into_iter().collect();
        let board = conversation_board(
            &[
                SidebarMission {
                    last_at: Some("2026-10-04T03:00:00.000Z".into()),
                    ..row("live", "running")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T04:00:00.000Z".into()),
                    ..row("room-answer", "running")
                },
                SidebarMission {
                    last_at: Some("2026-10-04T05:00:00.000Z".into()),
                    root_id: Some("root-trashed".into()),
                    ..row("trashed", "running")
                },
            ],
            &rooms,
            &trashed,
            &ConversationBoardFacts::default(),
        );
        assert_eq!(
            ids_in(&board)
                .iter()
                .map(|(id, _, _)| id.as_str())
                .collect::<Vec<_>>(),
            vec!["live"]
        );
    }

    #[test]
    fn lists_sub_conversation_once_with_parent() {
        let board = conversation_board(
            &[
                SidebarMission {
                    title: "Boss".into(),
                    last_at: Some("2026-10-04T02:00:00.000Z".into()),
                    ..row("boss", "running")
                },
                SidebarMission {
                    title: "Tasks".into(),
                    last_at: Some("2026-10-04T03:00:00.000Z".into()),
                    nested_under: Some("boss".into()),
                    ..row("tasks", "completed")
                },
            ],
            &[],
            &HashSet::new(),
            &ConversationBoardFacts::default(),
        );
        assert_eq!(
            ids_in(&board),
            vec![
                ("boss".into(), "working", None),
                ("tasks".into(), "done", Some("boss".into())),
            ]
        );
    }

    #[test]
    fn draws_nothing_when_quiet_done_only() {
        assert!(
            conversation_board(
                &[row("finished", "completed"), row("broke", "failed")],
                &[],
                &HashSet::new(),
                &ConversationBoardFacts::default(),
            )
            .is_none()
        );
        assert!(
            conversation_board(
                &[],
                &[],
                &HashSet::new(),
                &ConversationBoardFacts::default()
            )
            .is_none()
        );
    }

    #[test]
    fn needs_you_and_to_look_at_columns() {
        let at = |hour: u32| format!("2026-10-04T0{hour}:00:00.000Z");
        let facts = ConversationBoardFacts {
            needs_you: ["asks".into()].into_iter().collect(),
            to_look_at: ["ended-1".into()].into_iter().collect(),
        };
        let board = conversation_board(
            &[
                SidebarMission {
                    last_at: Some(at(3)),
                    ..row("asks", "running")
                },
                SidebarMission {
                    last_at: Some(at(2)),
                    ..row("live", "running")
                },
                SidebarMission {
                    last_at: Some(at(4)),
                    member_ids: vec!["ended-1".into(), "ended".into()],
                    ..row("ended", "completed")
                },
                SidebarMission {
                    last_at: Some(at(1)),
                    ..row("old", "completed")
                },
            ],
            &[],
            &HashSet::new(),
            &facts,
        );
        let board = board.expect("board");
        let mapped: Vec<_> = board
            .iter()
            .map(|s| {
                (
                    s.key.as_str(),
                    s.members
                        .iter()
                        .map(|c| c.row.mission_id.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(
            mapped,
            vec![
                ("needs-you", vec!["asks"]),
                ("working", vec!["live"]),
                ("to-look-at", vec!["ended"]),
                ("done", vec!["old"]),
            ]
        );
    }

    #[test]
    fn board_words_exact() {
        assert_eq!(ALLOW_ONCE, "Allow once");
        assert_eq!(DENY, "Deny");
        assert_eq!(BOARD_TITLE, "Board");
        assert_eq!(BOARD_CLEAR, "Your board is clear");
        assert_eq!(ConversationBoardKey::NeedsYou.title(), "Needs you");
        assert_eq!(ConversationBoardKey::ToLookAt.title(), "Ready to look at");
        assert_eq!(under_title("Boss"), "under Boss");
        assert_eq!(ANSWER, "Answer");
        assert_eq!(REVIEW, "Review");
    }
}

#[cfg(test)]
mod review_parity_tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn board_and_away_match_electron_on_the_same_synthetic_rows() {
        let rows: Vec<_> = ["completed", "failed", "running", "cancelled", "completed"]
            .into_iter()
            .enumerate()
            .map(|(i, phase)| SidebarMission {
                mission_id: format!("m{i}"),
                title: format!("synthetic {i}"),
                phase: phase.into(),
                last_at: Some(format!("2026-11-10T0{}:00:00Z", i + 1)),
                root_id: Some(format!("root{i}")),
                member_ids: vec![format!("old{i}"), format!("m{i}")],
                owner_id: None,
                said: String::new(),
                nested_under: None,
                integrity_issue_count: 0,
            })
            .collect();
        let facts = ConversationBoardFacts {
            needs_you: HashSet::from(["old2".into(), "root3".into()]),
            to_look_at: HashSet::from(["old1".into()]),
        };
        let trashed = HashSet::from(["old4".into()]);
        let away_rows: Vec<_> = rows
            .iter()
            .map(|r| crate::away::AwayRow {
                mission_id: r.mission_id.clone(),
                title: r.title.clone(),
                phase: r.phase.clone(),
                last_at: r.last_at.clone(),
                owner_id: None,
                routine_id: None,
                member_ids: r.member_ids.clone(),
                root_id: r.root_id.clone(),
            })
            .collect();
        let since = "2026-11-10T00:00:00Z";
        let summary =
            crate::away::since_you_were_away(since, &away_rows, &[], &facts.needs_you).unwrap();
        let sections = conversation_board(&rows, &[], &trashed, &facts).unwrap();
        assert_eq!(board_meta(Some(&sections)), "2 waiting on you");
        let mut with_working = sections.clone();
        with_working.push(ConversationBoardSection {
            key: ConversationBoardKey::Working,
            title: "Working",
            members: vec![ConversationCard {
                row: rows[2].clone(),
                parent_id: None,
            }],
        });
        assert_eq!(
            board_meta(Some(&with_working)),
            "2 waiting on you · 1 working"
        );
        let columns: Vec<_> = conversation_board(&rows, &[], &trashed, &facts)
            .unwrap()
            .iter()
            .map(|s| {
                json!([
                    s.key.as_str(),
                    s.members
                        .iter()
                        .map(|c| c.row.mission_id.clone())
                        .collect::<Vec<_>>()
                ])
            })
            .collect();
        let encoded: Vec<_> = rows.iter().map(|r| json!({"missionId":r.mission_id,"title":r.title,
            "phase":r.phase,"lastAt":r.last_at,"rootId":r.root_id,"memberIds":r.member_ids,"integrityIssueCount":0})).collect();
        let input = json!({"since":since,"rows":encoded,"routines":[],"needsYou":facts.needs_you,
            "toLookAt":facts.to_look_at,"trashed":trashed});
        let mut child = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/home-oracle.mjs"
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            actual,
            json!({"columns":columns,"counts":{"ran":summary.ran.len(),
            "failed":summary.failed.len(),"missed":summary.missed.len(),"waiting":summary.waiting}})
        );
    }
}
