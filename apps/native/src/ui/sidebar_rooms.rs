//! A ROOM STAYS IN THE SIDEBAR, AS ITSELF (conversationList.ts `withRoomsFolded` and `roomLastAt`, Sidebar.tsx
//! `roomRow`). A post to a room starts a conversation per teammate, each titled with the post; the flat sidebar has no
//! Rooms section, so each room is one row among the conversations, where its newest answer was, in place of the
//! answers themselves, and a room nobody has posted to yet is a row where its making puts it.

use super::*;
use crate::conversation_list::SidebarMission;
use locust_core::room_store::PublicRoom;

/// One row of the ungrouped list: a conversation, or a room with the answers it stands for.
pub(super) enum ListEntry<'a> {
    Conversation(&'a SidebarMission),
    Room { room: &'a PublicRoom, answers: Vec<&'a SidebarMission> },
}

fn time(at: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(at).ok().map(|at| at.timestamp_millis())
}

/// How old a room reads (`roomLastAt`): its newest answer or post, whichever came last; a time that is not one is
/// passed over; its making only when there is neither.
pub(super) fn room_last_at(room: &PublicRoom, answers: &[&SidebarMission]) -> String {
    let mut newest: Option<&str> = None;
    let mut newest_at = i64::MIN;
    let times = answers.iter().filter_map(|m| m.last_at.as_deref()).chain(room.posts.iter().map(|post| post.at.as_str()));
    for at in times {
        let Some(parsed) = time(at) else { continue };
        if parsed <= newest_at {
            continue;
        }
        newest = Some(at);
        newest_at = parsed;
    }
    newest.unwrap_or(&room.created_at).to_owned()
}

/// The list with each room drawn as the room (`withRoomsFolded`): one row where its newest answer was, holding the
/// answers in the order they came; a conversation found by any of its turns (`memberIds`); every other conversation
/// as it was; and each room nobody has posted to, newest made first, above the first row that is older than its making
/// and not running.
pub(super) fn with_rooms_folded<'a>(conversations: &'a [SidebarMission], rooms: &'a [PublicRoom]) -> Vec<ListEntry<'a>> {
    let mut room_of: HashMap<&str, &PublicRoom> = HashMap::new();
    for room in rooms {
        for post in &room.posts {
            for mission_id in post.missions.values() {
                room_of.insert(mission_id.as_str(), room);
            }
        }
    }
    let mut entries: Vec<ListEntry<'a>> = Vec::new();
    let mut placed: HashMap<&str, usize> = HashMap::new();
    for mission in conversations {
        let own = [mission.mission_id.clone()];
        let keys: &[String] = if mission.member_ids.is_empty() { &own } else { &mission.member_ids };
        let Some(room) = keys.iter().find_map(|id| room_of.get(id.as_str()).copied()) else {
            entries.push(ListEntry::Conversation(mission));
            continue;
        };
        match placed.get(room.room_id.as_str()) {
            Some(&at) => {
                if let ListEntry::Room { answers, .. } = &mut entries[at] {
                    answers.push(mission);
                }
            }
            None => {
                placed.insert(room.room_id.as_str(), entries.len());
                entries.push(ListEntry::Room { room, answers: vec![mission] });
            }
        }
    }
    let time_of = |entry: &ListEntry<'_>| match entry {
        ListEntry::Conversation(mission) => mission.last_at.as_deref().and_then(time).unwrap_or(0),
        ListEntry::Room { room, answers } => time(&room_last_at(room, answers)).unwrap_or(0),
    };
    let running = |entry: &ListEntry<'_>| match entry {
        ListEntry::Conversation(mission) => mission.phase == "running",
        ListEntry::Room { answers, .. } => answers.iter().any(|mission| mission.phase == "running"),
    };
    let mut empty: Vec<&PublicRoom> = rooms.iter().filter(|room| room.posts.is_empty()).collect();
    empty.sort_by_key(|room| std::cmp::Reverse(time(&room.created_at).unwrap_or(0)));
    for room in empty {
        let made = time(&room.created_at).unwrap_or(0);
        let row = ListEntry::Room { room, answers: Vec::new() };
        match entries.iter().position(|entry| !running(entry) && time_of(entry) < made) {
            Some(at) => entries.insert(at, row),
            None => entries.push(row),
        }
    }
    entries
}

/// A room's row (Sidebar.tsx `roomRow`, `.lc-conv--room`): one line as every conversation's is, the people glyph in
/// the owner's face footprint (`.lc-conv__room`, 20 wide) so its name starts where every title does, the name
/// shimmering while one of its answers works, "Release: a room with Wren and Booty" on hover, and a press opens it.
pub(super) fn room_row(s: &State, p: &Palette, room: &PublicRoom, answers: &[&SidebarMission]) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let running = answers.iter().any(|mission| mission.phase == "running");
    let members: Vec<&str> = room
        .teammate_ids
        .iter()
        .filter_map(|id| s.teammates.iter().find(|mate| &mate.id == id).map(|mate| mate.name.as_str()))
        .collect();
    let here = matches!(&s.rooms.active, crate::room_screen::RoomView::Room(id) if id == &room.room_id);
    let rid = room.room_id.clone();
    let name = if running {
        crate::working_shimmer::title(&room.name, l.c("text-primary"), s.reduced_motion.get(), s.frame_clock)
    } else {
        text(room.name.clone()).color(l.c("text-name"))
    };
    row()
        .key(format!("sidebar-room-{rid}"))
        .w_full()
        .h(CONVERSATION_ROW_HEIGHT)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(if here { l.c("bg-selected") } else { crate::window_glass::sidebar_row_ground(s, p) })
        .hover(l.c("hover-surface"))
        .role("button")
        .label(room.name.clone())
        .help(format!("{}: a room with {}", room.name, members.join(" and ")))
        .child(
            row().key(format!("sidebar-room-mark-{rid}")).size(OWNER_FACE, OWNER_FACE).center().a11y_hidden()
                .child(crate::look::controls::glyph(&l, &crate::look::icons::USERS, 13.0, Some(l.c("text-muted")))),
        )
        .child(
            name.font(rune::TextStyle::new(14.0, 400.0))
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .truncate()
                .key(format!("sidebar-room-title-{rid}")),
        )
        // The working footprint while it works (`.lc-conv__working`), kept empty as a conversation's is.
        .children(running.then(|| el().size(6.0, 6.0).a11y_hidden().key(format!("sidebar-room-working-{rid}"))))
        .on_press(move |s: &mut State| crate::room_screen::open_room(s, rid.clone()))
}

#[cfg(test)]
#[path = "sidebar_rooms_tests.rs"]
mod sidebar_rooms_tests;
