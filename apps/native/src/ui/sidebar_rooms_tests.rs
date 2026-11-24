//! A ROOM STAYS IN THE SIDEBAR (`renderer/src/a-room-stays-in-the-sidebar.test.ts`, 9 cases), with Electron's own
//! fixture: the room "Release", one post answered by Wren and Booty.
use super::*;
use crate::conversation_list::SidebarMission;
use locust_core::room_store::{PublicRoom, RoomPost};
use rune::{Env, Runtime};

fn conversation(mission_id: &str, last_at: Option<&str>, title: &str, phase: &str) -> SidebarMission {
    SidebarMission {
        mission_id: mission_id.into(),
        title: title.into(),
        phase: phase.into(),
        last_at: last_at.map(Into::into),
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

const POST_TITLE: &str = "In one sentence each: what does README.md say?";

fn answer(mission_id: &str, last_at: &str) -> SidebarMission {
    conversation(mission_id, Some(last_at), POST_TITLE, "completed")
}

fn titled(mission_id: &str, last_at: &str, title: &str) -> SidebarMission {
    conversation(mission_id, Some(last_at), title, "completed")
}

fn release() -> PublicRoom {
    PublicRoom {
        room_id: "room_release".into(),
        name: "Release".into(),
        teammate_ids: vec!["tm_wren".into(), "tm_booty".into()],
        created_at: "2026-09-23T04:30:00.000Z".into(),
        posts: vec![RoomPost {
            post_id: "post_1".into(),
            text: POST_TITLE.into(),
            at: "2026-09-23T04:31:00.000Z".into(),
            missions: [("tm_wren".to_owned(), "mission_wren".to_owned()), ("tm_booty".to_owned(), "mission_booty".to_owned())].into(),
            queued: Vec::new(),
            refused: Default::default(),
            to: Vec::new(),
        }],
        tasks: Vec::new(),
    }
}

fn empty_room(room_id: &str, name: &str, created_at: &str) -> PublicRoom {
    PublicRoom { room_id: room_id.into(), name: name.into(), teammate_ids: vec![], created_at: created_at.into(), posts: vec![], tasks: vec![] }
}

/// Each entry as Electron's test spells it: `room:<name>` for a room, the title for a conversation.
fn spelled(entries: &[ListEntry<'_>]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| match entry {
            ListEntry::Room { room, .. } => format!("room:{}", room.name),
            ListEntry::Conversation(mission) => mission.title.clone(),
        })
        .collect()
}

#[test]
fn is_one_row_for_its_answers_where_its_newest_answer_was() {
    // case: is one row for its answers, where its newest answer was
    let list = [
        titled("mission_other_new", "2026-09-23T05:00:00.000Z", "Fix the tests"),
        answer("mission_wren", "2026-09-23T04:32:00.000Z"),
        titled("mission_between", "2026-09-23T04:31:30.000Z", "Something else"),
        answer("mission_booty", "2026-09-23T04:31:20.000Z"),
    ];
    let rooms = [release()];
    let entries = with_rooms_folded(&list, &rooms);
    assert_eq!(spelled(&entries), ["Fix the tests", "room:Release", "Something else"]);
    let ListEntry::Room { answers, .. } = &entries[1] else { panic!("the second entry is the room") };
    assert_eq!(answers.iter().map(|m| m.mission_id.as_str()).collect::<Vec<_>>(), ["mission_wren", "mission_booty"]);
}

#[test]
fn finds_a_room_answer_that_was_replied_to_by_any_turn_of_the_conversation() {
    // case: finds a room answer that was replied to, by any turn of the conversation
    let mut replied = answer("mission_reply", "2026-09-23T04:40:00.000Z");
    replied.member_ids = vec!["mission_wren".into(), "mission_reply".into()];
    let list = [replied];
    let rooms = [release()];
    assert!(matches!(with_rooms_folded(&list, &rooms)[0], ListEntry::Room { .. }));
}

#[test]
fn keeps_a_room_nobody_has_posted_to_yet_placed_by_when_it_was_made() {
    // case: keeps a room nobody has posted to yet, placed by when it was made
    let list = [
        conversation("mission_running", Some("2026-09-23T04:00:00.000Z"), "Still going", "running"),
        titled("mission_new", "2026-09-23T05:00:00.000Z", "Newer"),
        titled("mission_old", "2026-09-23T04:10:00.000Z", "Older"),
    ];
    let rooms = [empty_room("room_fresh", "Fresh", "2026-09-23T04:45:00.000Z")];
    let entries = with_rooms_folded(&list, &rooms);
    assert_eq!(spelled(&entries), ["Still going", "Newer", "room:Fresh", "Older"]);
    let ListEntry::Room { answers, .. } = &entries[2] else { panic!("the third entry is the room") };
    assert!(answers.is_empty());
}

#[test]
fn leaves_every_other_conversation_as_it_was() {
    // case: leaves every other conversation as it was
    let list = [titled("a", "2026-09-23T05:00:00.000Z", "A"), titled("b", "2026-09-23T04:00:00.000Z", "B")];
    for rooms in [vec![], vec![release()]] {
        let entries = with_rooms_folded(&list, &rooms);
        assert!(entries.iter().all(|entry| matches!(entry, ListEntry::Conversation(_))));
        assert_eq!(spelled(&entries), ["A", "B"]);
    }
}

#[test]
fn are_newest_first_like_every_other_row() {
    // case: are newest first, like every other row
    let list = [titled("mission_old", "2026-09-20T00:00:00.000Z", "Old work")];
    let rooms = [
        empty_room("room_a", "Made first", "2026-09-23T01:00:00.000Z"),
        empty_room("room_b", "Made second", "2026-09-23T02:00:00.000Z"),
        empty_room("room_c", "Made third", "2026-09-23T03:00:00.000Z"),
    ];
    assert_eq!(spelled(&with_rooms_folded(&list, &rooms)), ["room:Made third", "room:Made second", "room:Made first", "Old work"]);
}

fn untimed(mission_id: &str) -> SidebarMission {
    conversation(mission_id, None, "x", "completed")
}

#[test]
fn is_its_newest_post_when_no_answer_has_a_time() {
    // case: is its newest post when no answer has a time
    let (wren, booty) = (untimed("mission_wren"), untimed("mission_booty"));
    assert_eq!(room_last_at(&release(), &[&wren, &booty]), "2026-09-23T04:31:00.000Z");
}

#[test]
fn is_its_newest_answer_when_that_came_after_the_post() {
    // case: is its newest answer when that came after the post
    let wren = conversation("mission_wren", Some("2026-09-23T04:35:00.000Z"), "x", "completed");
    let booty = conversation("mission_booty", Some("2026-09-23T04:33:00.000Z"), "x", "completed");
    assert_eq!(room_last_at(&release(), &[&wren, &booty]), "2026-09-23T04:35:00.000Z");
}

#[test]
fn is_its_making_only_when_nobody_has_posted_to_it() {
    // case: is its making only when nobody has posted to it
    let empty = PublicRoom { posts: vec![], ..release() };
    assert_eq!(room_last_at(&empty, &[]), "2026-09-23T04:30:00.000Z");
}

#[test]
fn ignores_a_time_that_is_not_one() {
    // case: ignores a time that is not one
    let wren = conversation("mission_wren", Some("not a time"), "x", "completed");
    assert_eq!(room_last_at(&release(), &[&wren]), "2026-09-23T04:31:00.000Z");
    let unposted = PublicRoom { posts: vec![], ..release() };
    assert_eq!(room_last_at(&unposted, &[&wren]), "2026-09-23T04:30:00.000Z");
}

// --- through the sidebar --------------------------------------------------------------------------------------------

fn mate(id: &str, name: &str) -> Teammate {
    Teammate { id: id.into(), name: name.into(), role: "Custom".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None }
}

fn listed(id: &str, title: &str, teammate: &str, updated: &str, state: &str) -> ConversationRow {
    ConversationRow { id: id.into(), title: title.into(), teammate: teammate.into(), started_by: "native".into(), state: state.into(), updated: updated.into() }
}

fn drawn(rt: &Runtime<State>) -> Vec<String> {
    let mut found: Vec<(f32, String)> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| {
            let key = e.key?;
            (key.starts_with("recent-") || (key.starts_with("sidebar-room-") && !key.starts_with("sidebar-room-title-") && !key.starts_with("sidebar-room-mark-") && !key.starts_with("sidebar-room-working-")))
                .then_some((e.rect[1], key))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, key)| key).collect()
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key)).map(|e| e.rect).unwrap_or_else(|| panic!("{key} is drawn"))
}

fn release_sidebar(wren_state: &str) -> Runtime<State> {
    let mut s = State::default();
    s.demo = true;
    s.teammates = vec![mate("tm_wren", "Wren"), mate("tm_booty", "Booty")];
    s.recent = vec![
        listed("mission_other_new", "Fix the tests", "Wren", "2026-09-23T05:00:00.000Z", "completed"),
        listed("mission_wren", POST_TITLE, "Wren", "2026-09-23T04:32:00.000Z", wren_state),
        listed("mission_between", "Something else", "Booty", "2026-09-23T04:31:30.000Z", "completed"),
        listed("mission_booty", POST_TITLE, "Booty", "2026-09-23T04:31:20.000Z", "completed"),
    ];
    s.rooms.rooms = vec![release(), empty_room("room_fresh", "Fresh", "2026-09-22T00:00:00.000Z")];
    let mut rt = Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

#[test]
fn the_sidebar_draws_a_room_as_one_row_among_its_conversations_and_opens_it() {
    let mut rt = release_sidebar("completed");
    assert_eq!(
        drawn(&rt),
        ["recent-mission_other_new", "sidebar-room-room_release", "recent-mission_between", "sidebar-room-room_fresh"],
        "the room where its newest answer was, its answers not drawn again, the empty one where its making puts it"
    );
    // No Rooms section of its own: the places list is the way to the Rooms screen.
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("sidebar-rooms")));
    // A conversation's row: one line, its name on the titles' x, the people glyph in the face's place.
    let room = rect(&rt, "sidebar-room-room_release");
    assert_eq!(room[3], 32.0);
    assert_eq!(rect(&rt, "sidebar-room-title-room_release")[0], rect(&rt, "conversation-title-mission_other_new")[0]);
    assert_eq!(rect(&rt, "sidebar-room-mark-room_release")[2], 20.0);
    let help = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("sidebar-room-room_release")).and_then(|e| e.help);
    assert_eq!(help.as_deref(), Some("Release: a room with Wren and Booty"));
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("sidebar-room-working-room_release")), "nothing in it works");
    let (x, y) = rt.find("sidebar-room-room_release").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert!(matches!(&rt.state.rooms.active, crate::room_screen::RoomView::Room(id) if id == "room_release"));
}

#[test]
fn a_room_whose_answer_is_working_works_and_stands_where_that_live_answer_does() {
    // Wren's answer is running: the list puts a live conversation first, so the room stands there, wearing the
    // working footprint (`lc-conv__working`) and its name's shimmer; the empty room does neither.
    let rt = release_sidebar("running");
    assert_eq!(drawn(&rt)[0], "sidebar-room-room_release");
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("sidebar-room-working-room_release")));
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("sidebar-room-working-room_fresh")));
}

#[test]
fn a_room_holding_the_open_conversation_is_drawn_however_far_down_the_list_it_is() {
    // Sidebar.tsx: `openAt` finds a room by any of its answers, and the list draws at least that far.
    let mut s = State::default();
    s.demo = true;
    s.teammates = vec![mate("tm_wren", "Wren"), mate("tm_booty", "Booty")];
    let base = chrono::DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap();
    s.recent = (0..LIST_PAGE + 5)
        .map(|i| listed(&format!("m_{i}"), &format!("Conversation {i}"), "Booty", &(base - chrono::Duration::minutes(i as i64)).to_rfc3339(), "completed"))
        .collect();
    s.recent.push(listed("mission_wren", POST_TITLE, "Wren", "2026-09-23T04:32:00.000Z", "completed"));
    s.rooms.rooms = vec![release()];
    let mut open = Conversation::default();
    open.conversation_id = Some("mission_wren".into());
    s.conversations.insert("tm_wren".into(), open);
    let mut rt = Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 8000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(drawn(&rt).iter().any(|k| k == "sidebar-room-room_release"), "the open answer's room is drawn past the first page");
    assert!(drawn(&rt).iter().any(|k| k == &format!("recent-m_{}", LIST_PAGE + 4)), "and every row above it");
}
