//! The Rooms screen with no room open, as Electron draws it (`RoomScreen.tsx` 0.882, lines 684-851), driven headless
//! in the real window. The owner's live hour (26 Nov, row 37) found the Rust page saying "Rooms" alone with a bare
//! "No rooms yet…" line, and New room opening a left-aligned list of teammate rows with their roles in spaced mono.
//! Electron's page is one screen: "Rooms" with its meta beside it, "Your rooms" when there are any, and "New room" as
//! a card in three bands whose teammates are chips (face, name, a tick when picked), never rows with roles.
use super::*;
use rune::{ElementInfo, Env, Runtime};

fn window(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, crate::view, (1200.0, 900.0), Env::new(true, 1.0));
    for _ in 0..4 {
        rt.frame(0.016);
    }
    rt
}

fn shown(rt: &Runtime<State>) -> Vec<ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    shown(rt).into_iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect).unwrap_or_else(|| panic!("no {key} on screen"))
}

fn words_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    shown(rt).into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text)
}

/// Every word drawn inside the element `key`, top to bottom.
fn text_in(rt: &Runtime<State>, key: &str) -> String {
    let [x, y, w, h] = rect(rt, key);
    shown(rt)
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn any_words(rt: &Runtime<State>, words: &str) -> bool {
    shown(rt).iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains(words)))
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..30 {
        rt.frame(0.05);
    }
}

fn room(id: &str, name: &str, members: &[&str]) -> locust_core::room_store::PublicRoom {
    locust_core::room_store::PublicRoom {
        room_id: id.into(),
        name: name.into(),
        teammate_ids: members.iter().map(|m| (*m).to_owned()).collect(),
        created_at: "2026-11-20T12:00:00.000Z".into(),
        posts: Vec::new(),
        tasks: Vec::new(),
    }
}

fn rooms_screen(rooms: Vec<locust_core::room_store::PublicRoom>) -> State {
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.rooms.active = crate::room_screen::RoomView::List;
    s.rooms.rooms = rooms;
    s
}

#[test]
fn the_header_says_rooms_and_none_yet_beside_it() {
    // RoomScreen.tsx:688-691: `<span className="lc-screen__title">Rooms</span>` then
    // `{rooms.length === 0 ? 'none yet' : …} · one post, every teammate answers`.
    let rt = window(rooms_screen(Vec::new()));
    assert_eq!(words_of(&rt, "rooms-title").as_deref(), Some("Rooms"));
    assert_eq!(words_of(&rt, "rooms-meta").as_deref(), Some("none yet · one post, every teammate answers"));
    // `.lc-screen__header` (shell.css:11538): one 60-point row, the meta right after the title on its line, not
    // pushed to the far side.
    let header = rect(&rt, "rooms-header");
    assert_eq!(header[3], 60.0, "{header:?}");
    let (title, meta) = (rect(&rt, "rooms-title"), rect(&rt, "rooms-meta"));
    assert!((title[1] + title[3] / 2.0 - (meta[1] + meta[3] / 2.0)).abs() < 3.0, "one line: {title:?} {meta:?}");
    let gap = meta[0] - (title[0] + title[2]);
    assert!((0.0..=24.0).contains(&gap), "the meta sits beside the title, {gap} apart");
}

#[test]
fn the_meta_counts_rooms_in_electrons_words() {
    // RoomScreen.tsx:690: `${String(rooms.length)} room${rooms.length === 1 ? '' : 's'}`.
    assert_eq!(crate::room_screen::rooms_meta(0), "none yet · one post, every teammate answers");
    assert_eq!(crate::room_screen::rooms_meta(1), "1 room · one post, every teammate answers");
    assert_eq!(crate::room_screen::rooms_meta(3), "3 rooms · one post, every teammate answers");
    let rt = window(rooms_screen(vec![room("r1", "Release", &["codec"]), room("r2", "Standup", &["argon", "scout"])]));
    assert_eq!(words_of(&rt, "rooms-meta").as_deref(), Some("2 rooms · one post, every teammate answers"));
}

#[test]
fn with_no_rooms_the_new_room_card_comes_first_and_the_none_yet_note_sits_under_it() {
    let rt = window(rooms_screen(Vec::new()));
    // RoomScreen.tsx:693: "Your rooms" only when there are rooms.
    assert!(rt.find("rooms-yours").is_none(), "no Your rooms with no rooms");
    // RoomScreen.tsx:713: the block's label, "New room", in the section-label voice (capitals, as `text-transform`).
    assert!(text_in(&rt, "rooms-new").starts_with("NEW ROOM"), "{}", text_in(&rt, "rooms-new"));
    // RoomScreen.tsx:740-743, 752, 765: band one says what a room is; band two's fields are labelled.
    assert_eq!(text_in(&rt, "room-create-about"), crate::room_screen::ROOM_ABOUT);
    assert!(crate::room_screen::ROOM_ABOUT.starts_with("Name it and pick who is in it. A post goes to everyone in the room at once"));
    let fields = text_in(&rt, "room-create-fields");
    assert!(fields.contains("ROOM NAME") && fields.contains("WHO IS IN IT"), "{fields}");
    // RoomScreen.tsx:843-846: under the card, not instead of it.
    assert_eq!(words_of(&rt, "rooms-none-yet").as_deref(), Some("No rooms yet. The first one you make appears in the sidebar under its own name."));
    let (card, note) = (rect(&rt, "room-create-card"), rect(&rt, "rooms-none-yet"));
    assert!(note[1] >= card[1] + card[3], "the note is under the card: {card:?} {note:?}");
}

#[test]
fn rooms_there_are_are_cards_under_your_rooms_above_the_new_room_card() {
    // RoomScreen.tsx:693-704: "Your rooms", a card per room (name, then `roomCardMeta`), then New room.
    let rt = window(rooms_screen(vec![room("r1", "Release", &["codec", "argon"])]));
    let yours = text_in(&rt, "rooms-yours");
    assert!(yours.starts_with("YOUR ROOMS"), "{yours}");
    assert!(yours.contains("Release") && yours.contains("Codec · Argon · nothing posted yet"), "{yours}");
    assert!(rect(&rt, "rooms-yours")[1] < rect(&rt, "rooms-new")[1], "Your rooms comes first");
    // RoomScreen.tsx:843: the note is only for a person with no rooms.
    assert!(rt.find("rooms-none-yet").is_none());
    assert!(!any_words(&rt, "No rooms yet"));
}

#[test]
fn new_room_opens_this_screen_not_a_screen_of_its_own() {
    // App.tsx:1014: the `+` menu's New room calls `openRoomsScreen`: the same screen, its form in the card.
    let mut s = rooms_screen(Vec::new());
    crate::room_screen::open_new(&mut s);
    let rt = window(s);
    assert_eq!(words_of(&rt, "rooms-title").as_deref(), Some("Rooms"));
    assert!(rt.find("room-create-card").is_some());
    assert!(!any_words(&rt, "Cancel"), "Electron's screen has no Cancel");
    assert!(rt.find("room-create-cancel").is_none());
}

#[test]
fn teammates_are_chips_with_a_face_and_a_name_side_by_side_never_rows_with_roles() {
    // RoomScreen.tsx:767-799 and `.lc-roomform__members` (shell.css:4116, `flex-wrap: wrap`): one chip per teammate,
    // its face and name, in a row that wraps.
    let rt = window(rooms_screen(Vec::new()));
    let chips: Vec<[f32; 4]> = ["codec", "argon", "scout"].iter().map(|id| rect(&rt, &format!("room-create-pick-{id}"))).collect();
    assert!(chips.iter().all(|c| (c[1] - chips[0][1]).abs() < 1.0), "the chips share a line: {chips:?}");
    assert!(chips[0][0] < chips[1][0] && chips[1][0] < chips[2][0], "left to right: {chips:?}");
    // A chip is as wide as its words, not the card's width.
    assert!(chips[0][2] < 200.0, "{chips:?}");
    assert_eq!(text_in(&rt, "room-create-pick-codec"), "Codec");
    // No role under the name: "Code & Migrations" (Codec's and Argon's role) is not on the screen at all.
    assert!(!any_words(&rt, "Code & Migrations"), "roles are not drawn in the picker");
}

#[test]
fn a_picked_chip_wears_a_tick_and_the_summary_counts_it() {
    // RoomScreen.tsx:796 `{on && <Icon name="check" size={13} />}` and 822-823 the summary beside Create room.
    let mut rt = window(rooms_screen(Vec::new()));
    assert_eq!(words_of(&rt, "room-create-summary").as_deref(), Some("0 of 3 teammates · each answers on its own model"));
    assert!(rt.find("room-create-tick-codec").is_none());
    press(&mut rt, "room-create-pick-codec");
    assert!(rt.find("room-create-tick-codec").is_some(), "the picked chip has its tick");
    assert!(rt.find("room-create-tick-argon").is_none());
    assert_eq!(words_of(&rt, "room-create-summary").as_deref(), Some("1 of 3 teammates · each answers on its own model"));
    let (tick, chip) = (rect(&rt, "room-create-tick-codec"), rect(&rt, "room-create-pick-codec"));
    assert!(tick[0] >= chip[0] && tick[0] + tick[2] <= chip[0] + chip[2], "the tick is inside the chip: {tick:?} {chip:?}");
    // The summary and Create room share the action band, the button after the words.
    let (summary, create) = (rect(&rt, "room-create-summary"), rect(&rt, "room-create-submit"));
    assert!(create[0] > summary[0]);
    assert!((create[1] + create[3] / 2.0 - (summary[1] + summary[3] / 2.0)).abs() < 4.0);
}

#[test]
fn the_card_says_why_create_room_is_not_offered() {
    // RoomScreen.tsx:806-814: `roomBlockedReason(draftName, draftMembers.length)` is said in the card.
    let mut rt = window(rooms_screen(Vec::new()));
    assert_eq!(words_of(&rt, "room-full-note").as_deref(), Some("Name the room to create it."));
    press(&mut rt, "room-create-name");
    rt.paste("Release");
    rt.frame(0.016);
    rt.frame(0.016);
    assert_eq!(words_of(&rt, "room-full-note").as_deref(), Some("Pick at least one teammate."));
    press(&mut rt, "room-create-pick-scout");
    assert!(rt.find("room-full-note").is_none(), "nothing to say once it can be made");
}

#[test]
fn with_no_teammates_it_says_make_one_first_and_draws_no_card() {
    // RoomScreen.tsx:714-715 and 843: no card, no none-yet note.
    let mut s = rooms_screen(Vec::new());
    s.teammates.clear();
    let rt = window(s);
    assert_eq!(words_of(&rt, "rooms-make-a-teammate").as_deref(), Some("Make a teammate first; a room is a set of them."));
    assert!(rt.find("room-create-card").is_none());
    assert!(rt.find("rooms-none-yet").is_none());
}
