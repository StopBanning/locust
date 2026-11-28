use super::*;
use locust_core::ledger::Summary;
use locust_core::native::RelayStarting;
use rune::{Env, Runtime};

fn teammate(id: &str, name: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Engineer".into(),
        runtime: Some("cursor".into()),
        model: Some("composer-2.5".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

fn state_without_peer_links(starting: Option<RelayStarting>) -> State {
    let mut s = State::default();
    s.teammates = vec![teammate("tm_wren", "Wren"), teammate("tm_gem", "Gem")];
    s.relay_hop_cap = 6;
    s.relay_live.starting = starting;
    let post = RoomPost {
        post_id: "post_1".into(),
        text: "Tabs or spaces?".into(),
        at: "2026-09-11T10:00:00.000Z".into(),
        missions: BTreeMap::from([
            ("tm_wren".into(), "m_wren1".into()),
            ("tm_gem".into(), "m_gem1".into()),
        ]),
        queued: Vec::new(),
        refused: BTreeMap::new(),
        to: Vec::new(),
    };
    let room = PublicRoom {
        room_id: "room_style".into(),
        name: "Style".into(),
        teammate_ids: vec!["tm_wren".into(), "tm_gem".into()],
        created_at: "2026-09-11T09:59:00.000Z".into(),
        posts: vec![post],
        tasks: Vec::new(),
    };
    s.rooms.active = RoomView::Room(room.room_id.clone());
    s.rooms.rooms = vec![room];
    for (mid, owner, at, text) in [
        ("m_wren1", "tm_wren", "2026-09-11T10:00:01.000Z", "Tabs."),
        ("m_gem1", "tm_gem", "2026-09-11T10:00:02.000Z", "Until a line wraps."),
    ] {
        s.mission_owners.insert(mid.into(), owner.into());
        s.ledger_summaries.insert(
            mid.into(),
            Summary {
                mission_id: mid.into(),
                runtime: "cursor".into(),
                model: "composer-2.5".into(),
                created_at: at.into(),
                peer_links: Vec::new(),
                ..Default::default()
            },
        );
        s.rooms.transcripts.insert(
            mid.into(),
            RoomTranscriptView {
                phase: "completed".into(),
                answer_text: text.into(),
                note_error: None,
                started_at: at.into(),
                runtime: "cursor".into(),
                model: "composer-2.5".into(),
                live: None,
            },
        );
    }
    s
}

fn render(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(
        s,
        |s, _| view(s, &Palette::new(true)),
        (1100.0, 900.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

#[test]
fn room_post_becomes_an_exchange_when_first_reply_is_starting_without_peer_links() {
    let quiet = render(state_without_peer_links(None));
    assert!(quiet.presented("room-exchange-post_1").is_none());
    assert!(quiet.presented("room-answer-post_1-tm_wren").is_some());

    let starting = Some(RelayStarting {
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        answering: "m_gem1".into(),
        hop: 1,
    });
    let rt = render(state_without_peer_links(starting));
    assert!(
        rt.presented("room-exchange-post_1").is_some(),
        "a starting reply to one of the post's runs opens the post as an exchange even before peer links exist"
    );
    assert!(rt.presented("room-answer-post_1-tm_wren").is_none());
    assert!(rt.presented("room-answer-post_1-tm_gem").is_none());

    let shown: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).collect();
    let face = shown
        .iter()
        .find(|e| e.key.as_deref() == Some("room-exchange-face-post_1-replying_tm_wren_m_gem1"))
        .and_then(|e| e.label.as_deref());
    assert_eq!(face, Some("Wren"));
    let live = shown
        .iter()
        .find(|e| e.key.as_deref() == Some("room-exchange-live-post_1-replying_tm_wren_m_gem1"))
        .and_then(|e| e.text.as_deref());
    assert_eq!(live, Some("Starting\u{2026}"));
}
