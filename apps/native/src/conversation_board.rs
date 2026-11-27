//! Conversations as a board — port of `renderer/src/conversationBoard.ts`.

use crate::conversation_list::{SidebarMission, by_live_then_recent, conversation_keys};
use crate::look::controls::Look;
use rune::{El, TextStyle, col, el, row, text};
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

/// The sidebar's running mark (conversationBoard.ts `conversationIsRunning`): the row's phase view says RUNNING, the
/// same fact the sidebar draws its spark from, so a card never lands in a column its row's mark contradicts.
pub fn conversation_is_running(row: &SidebarMission) -> bool {
    crate::missions_screen::mission_phase_view(&row.phase, row.integrity_issue_count > 0).tag == "RUNNING"
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

#[cfg(test)]
pub fn conversation_board(
    rows: &[SidebarMission],
    rooms: &[BoardRoom],
    trashed: &HashSet<String>,
    facts: &ConversationBoardFacts,
) -> Option<Vec<ConversationBoardSection>> {
    conversation_board_with_extra(rows, rooms, trashed, facts, 0)
}

pub fn conversation_board_with_extra(
    rows: &[SidebarMission],
    rooms: &[BoardRoom],
    trashed: &HashSet<String>,
    facts: &ConversationBoardFacts,
    extra_needs: usize,
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
    if extra_needs == 0
        && sections
            .iter()
            .all(|section| section.key == ConversationBoardKey::Done || section.members.is_empty())
    {
        return None;
    }
    sections.retain(|section| {
        !section.members.is_empty()
            || (extra_needs > 0 && section.key == ConversationBoardKey::NeedsYou)
    });
    Some(sections)
}

#[cfg(test)]
pub fn board_meta(sections: Option<&[ConversationBoardSection]>) -> String {
    board_meta_with_extra(sections, 0)
}

pub fn board_meta_with_extra(
    sections: Option<&[ConversationBoardSection]>,
    extra_needs: usize,
) -> String {
    let Some(sections) = sections else {
        if extra_needs > 0 {
            return format!("{extra_needs} waiting on you");
        }
        return ALL_QUIET.into();
    };
    let needs = sections
        .iter()
        .find(|s| s.key == ConversationBoardKey::NeedsYou)
        .map(|s| s.members.len())
        .unwrap_or(0)
        + extra_needs;
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

// ---- The Board as Electron draws it (BoardScreen.tsx 0.882, shell.css `.lc-teamboard`, `.lc-boardsection`,
// `.lc-convgrid`, `.lc-convcard`). Pieces the Board's view is built from: the header, a column, a card, the row of
// columns, and which runs are never on it. The live hour (26 Nov, row 41) found one full-width column of tall rows (title,
// teammate and state on three lines), "Done 74" counting Scout's own runs, and "all quiet" floating mid-header.

/// A run Electron never lists (App.tsx:2016-2018 `isHiddenRun`): a judge's, Scout's own, a question on the side
/// (App.tsx:8250), and a panel's own run (shared/panel-runs.ts `isPanelRunKind`: a commit message, a walk through).
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn is_hidden_run(started_by_kind: Option<&str>) -> bool {
    matches!(started_by_kind, Some("judge" | "scout" | "side" | "commit-draft" | "walkthrough"))
}

/// The conversations the Board draws: a conversation every one of whose turns is a hidden run is not one
/// (`sidebarMissions` drops those runs before rows are made, App.tsx:8248-8250).
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn without_hidden_runs(missions: &[SidebarMission], kind_of: impl Fn(&str) -> Option<String>) -> Vec<SidebarMission> {
    missions
        .iter()
        .filter(|row| !conversation_keys(row).iter().all(|id| is_hidden_run(kind_of(id).as_deref())))
        .cloned()
        .collect()
}

/// A card's one line under its title (BoardScreen.tsx:196-202, `.lc-convcard__meta > span + span::before`): who, what
/// it is under, and its state, joined by " · ".
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn card_meta_line(who: Option<&str>, under: Option<&str>, state: &str) -> String {
    who.into_iter().chain(under).chain(std::iter::once(state)).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// The data name of a column (`data-column`), for keys.
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn column_name(key: ConversationBoardKey) -> &'static str {
    match key {
        ConversationBoardKey::NeedsYou => "needs-you",
        ConversationBoardKey::Working => "working",
        ConversationBoardKey::ToLookAt => "to-look-at",
        ConversationBoardKey::Done => "done",
    }
}

/// `.lc-screen__header` with BoardScreen.tsx:305-308: "Board", then its meta right beside it (shell.css:11538, the title
/// 20/650 at 26240, the meta 13 at 26062). Nothing pushes the meta to the middle.
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn board_header(l: &Look, meta: &str) -> El {
    row()
        .key("board-header")
        .w_full()
        .h(60.0)
        .px(24.0)
        .gap(12.0)
        .center_y()
        .fill(l.c("bg-chrome"))
        .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-hairline"))
        .child(text(BOARD_TITLE).key("board-title").font(TextStyle::new(20.0, 650.0).tracking(-0.3)).color(l.c("text-primary")))
        .child(text(meta.to_owned()).key("board-meta").font(TextStyle::new(13.0, 400.0)).color(l.c("text-muted")))
}

/// `.lc-convcard` (shell.css:11983-12056): the face spanning two lines, the title on the first, the meta line on the
/// second; hairline border (amber-tinted in Needs you), raised ground, the whole card pressed to open.
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn conv_card(l: &Look, column: ConversationBoardKey, mission_id: &str, face: Option<El>, title: &str, meta: &str) -> El {
    let needs = column == ConversationBoardKey::NeedsYou;
    let mark = face.unwrap_or_else(|| {
        // `.lc-conv__nobody`: an empty ring where a face would be.
        el().size(20.0, 20.0).radius(10.0).border(1.0, l.c("border-control")).a11y_hidden()
    });
    row()
        .key(format!("board-card-{mission_id}"))
        .w_full()
        .min_w(0.0)
        .px(16.0)
        .py(12.0)
        .gap(24.0)
        .center_y()
        .radius(l.t.px("radius-card"))
        .border(1.0, if needs { l.c("amber-tint") } else { l.c("border-hairline") })
        .fill(l.c("bg-raised"))
        .hover(l.c("hover-surface"))
        .role("button")
        .label(title.to_owned())
        .child(mark)
        .child(
            col()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .gap(4.0)
                .child(text(title.to_owned()).key(format!("board-card-title-{mission_id}")).font(TextStyle::new(13.0, 400.0)).color(l.c("text-primary")).truncate())
                .child(
                    text(meta.to_owned())
                        .key(format!("board-card-meta-{mission_id}"))
                        .font(TextStyle::new(12.0, 400.0))
                        .color(if needs { l.c("amber-text") } else { l.c("text-muted") })
                        .truncate(),
                ),
        )
}

/// `.lc-boardsection` with its title (BoardScreen.tsx:270-297): the coloured mark, the column's name in the section-label
/// voice, its count; then its cards one under another (`.lc-convgrid`), Done's in a grid of 240-wide cards. A column is
/// 256 wide at rest and 232 at least; Done takes what is left (`.lc-convboard .lc-boardsection--done { flex: 1 1 256px }`).
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn board_column(l: &Look, key: ConversationBoardKey, count: usize, cards: Vec<El>) -> El {
    let name = column_name(key);
    let mark = match key {
        ConversationBoardKey::NeedsYou => l.c("amber"),
        ConversationBoardKey::Working => l.c("lime"),
        ConversationBoardKey::ToLookAt => l.c("green"),
        ConversationBoardKey::Done => l.c("text-muted"),
    };
    let ink = if key == ConversationBoardKey::NeedsYou { l.c("amber-text") } else { l.c("text-muted") };
    let title = row()
        .key(format!("board-column-title/{name}"))
        .gap(6.0)
        .center_y()
        .child(el().size(7.0, 7.0).radius(3.5).fill(mark).a11y_hidden())
        .child(text(key.title().to_uppercase()).font(l.ty.label).color(ink))
        .child(text(count.to_string()).key(format!("board-{name}-count")).font(l.ty.label).color(ink).opacity(0.7));
    let grid = if key == ConversationBoardKey::Done {
        cards.into_iter().fold(row().w_full().gap(8.0).flex_wrap(), |grid, card| grid.child(el().min_w(240.0).grow(1.0).w(240.0).child(card)))
    } else {
        cards.into_iter().fold(col().w_full().gap(8.0), |grid, card| grid.child(card))
    };
    let section = col().key(format!("board-column/{name}")).role("group").label(key.title()).gap(8.0).min_w(232.0).child(title).child(grid);
    if key == ConversationBoardKey::Done { section.w(256.0).grow(1.0) } else { section.w(256.0).shrink() }
}

/// `.lc-teamboard.lc-convboard` (shell.css:11863): the columns side by side, wrapping onto a new line only when the
/// window is too narrow for them, tops aligned.
#[allow(dead_code)] // home_panel.rs `board_view` is to be built from it (notes/PAGES-NOTES.md, row 41)
pub fn board_columns(columns: Vec<El>) -> El {
    columns.into_iter().fold(row().key("board-columns").w_full().gap(16.0).flex_wrap().items_start(), |board, column| board.child(column))
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
            needs_you: false,
            app_started: false,
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

    // case: puts a live run in Working and every other phase in Done
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

    // case: orders a column newest first
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

    // case: leaves a room and a trashed conversation out
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

    // case: lists a sub-conversation once, with the conversation it sits under
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

    // case: draws nothing when nobody is working and nothing waits
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

    // case: puts a conversation waiting on you first, even while its run is live, and a finish not looked at since in Ready to look at
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

    // case: lands each conversation in the column the sidebar mark already gives that row
    #[test]
    fn lands_each_conversation_in_the_column_its_sidebar_mark_gives() {
        let rows = [
            row("live", "running"),
            row("finished", "completed"),
            row("broke", "failed"),
            SidebarMission { integrity_issue_count: 2, ..row("torn", "completed") },
        ];
        let board = conversation_board(&rows, &[], &HashSet::new(), &ConversationBoardFacts::default());
        let column_of: HashMap<String, &'static str> = ids_in(&board).into_iter().map(|(id, key, _)| (id, key)).collect();
        for entry in &rows {
            let running = conversation_is_running(entry);
            assert_eq!(running, crate::missions_screen::mission_phase_view(&entry.phase, entry.integrity_issue_count > 0).tag == "RUNNING");
            assert_eq!(column_of.get(&entry.mission_id).copied(), Some(if running { "working" } else { "done" }), "{}", entry.mission_id);
        }
        // Electron's phases: only "running" is RUNNING.
        assert_eq!(rows.iter().filter(|r| conversation_is_running(r)).map(|r| r.mission_id.as_str()).collect::<Vec<_>>(), ["live"]);
    }

    // case: a finish to look at keeps the board up on its own; one looked at since does not (control)
    #[test]
    fn a_finish_to_look_at_keeps_the_board_up_on_its_own() {
        let ended = row("ended", "completed");
        let facts = |needs: &[&str], look: &[&str]| ConversationBoardFacts {
            needs_you: needs.iter().map(|s| (*s).to_owned()).collect(),
            to_look_at: look.iter().map(|s| (*s).to_owned()).collect(),
        };
        let keys = conversation_board(std::slice::from_ref(&ended), &[], &HashSet::new(), &facts(&[], &["ended"]))
            .map(|sections| sections.iter().map(|s| s.key.as_str()).collect::<Vec<_>>());
        assert_eq!(keys, Some(vec!["to-look-at"]));
        assert!(conversation_board(std::slice::from_ref(&ended), &[], &HashSet::new(), &facts(&[], &["someone-else"])).is_none());
        assert_eq!(conversation_column(&ended, &facts(&["ended"], &[])), ConversationBoardKey::NeedsYou);
        assert_eq!(conversation_column(&ended, &ConversationBoardFacts::default()), ConversationBoardKey::Done);
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
                needs_you: false,
                app_started: false,
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

#[cfg(test)]
#[path = "board_shape_tests.rs"]
mod board_shape_tests;
