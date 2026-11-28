use super::*;
use locust_core::ledger::{PeerLink, Summary};
use locust_core::ledger_write::LedgerWriter;
use locust_core::room_store::NewPostInput;
use locust_core::store::Store;
use rune::{Env, Runtime};
use serde_json::json;
use std::path::PathBuf;

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_store() -> (TempDir, Store) {
    let root = std::env::temp_dir().join(format!(
        "locust-room-relayed-{}",
        locust_core::invariants::unique_nanos()
    ));
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    (TempDir(root), store)
}

fn write_reply_ledger(store: &Store, mission_id: &str, answer: &str) {
    let mut writer =
        LedgerWriter::create(store, mission_id, "Tabs or spaces?", "composer-2.5", "cursor")
            .unwrap();
    writer
        .append("run.started", json!({"runtimeThreadId": mission_id}), Some(mission_id))
        .unwrap();
    writer
        .append(
            "message.delta",
            json!({"itemId": "msg_1", "text": answer, "operation": "replace", "final": true}),
            Some(mission_id),
        )
        .unwrap();
    writer
        .append("run.completed", json!({}), Some(mission_id))
        .unwrap();
}

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

#[test]
fn room_snapshot_loads_relayed_reply_transcripts_across_peer_links() {
    let (_tmp, store) = temp_store();
    let room = room_store::create(
        &store,
        "Style",
        &["tm_wren".to_owned(), "tm_gem".to_owned()],
    )
    .unwrap();
    room_store::add_post(
        &store,
        &room.room_id,
        NewPostInput {
            text: "Tabs or spaces?".into(),
            missions: BTreeMap::from([
                ("tm_wren".into(), "m_wren1".into()),
                ("tm_gem".into(), "m_gem1".into()),
            ]),
            queued: Vec::new(),
            refused: BTreeMap::new(),
            to: Vec::new(),
        },
    )
    .unwrap();

    write_reply_ledger(&store, "m_wren1", "Tabs.");
    write_reply_ledger(&store, "m_gem1", "Until a line wraps.");
    write_reply_ledger(&store, "m_wren2", "Do not align past the indent.");
    write_reply_ledger(&store, "m_gem2", "Formatters emit spaces.");

    let mut s = State::default();
    s.teammates = vec![teammate("tm_wren", "Wren"), teammate("tm_gem", "Gem")];
    s.relay_hop_cap = 6;
    for (mid, owner, at, hop, links) in [
        ("m_wren1", "tm_wren", "2026-09-11T10:00:01.000Z", None, &["wm_1"][..]),
        ("m_gem1", "tm_gem", "2026-09-11T10:00:02.000Z", None, &["wm_1", "wm_2"][..]),
        ("m_wren2", "tm_wren", "2026-09-11T10:00:03.000Z", Some(3), &["wm_2", "wm_3"][..]),
        ("m_gem2", "tm_gem", "2026-09-11T10:00:04.000Z", Some(4), &["wm_3"][..]),
    ] {
        s.mission_owners.insert(mid.into(), owner.into());
        s.ledger_summaries.insert(
            mid.into(),
            Summary {
                mission_id: mid.into(),
                runtime: "cursor".into(),
                model: "composer-2.5".into(),
                created_at: at.into(),
                started_by_hop: hop,
                peer_links: links
                    .iter()
                    .map(|msg| PeerLink {
                        message_id: (*msg).into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
        );
    }

    let peer_links = peer_links_from_state(&s);
    let snap = load_snapshot_for(&store, &[], &peer_links).unwrap();
    assert_eq!(
        snap.transcripts.get("m_wren2").map(|t| t.answer_text.as_str()),
        Some("Do not align past the indent."),
        "relayed reply reachable by peer_links must have its transcript loaded"
    );
    assert_eq!(
        snap.transcripts.get("m_gem2").map(|t| t.answer_text.as_str()),
        Some("Formatters emit spaces."),
        "multi-hop relayed reply reachable by peer_links must have its transcript loaded"
    );

    s.rooms.rooms = snap.rooms;
    s.rooms.transcripts = snap.transcripts;
    s.rooms.active = RoomView::Room(room.room_id);
    let mut rt = Runtime::new(
        s,
        |s, _| view(s, &Palette::new(true)),
        (1100.0, 900.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let words: Vec<String> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| e.text)
        .collect();
    assert!(
        words.iter().any(|w| w.contains("Do not align past the indent.")),
        "rendered exchange must show Wren's relayed reply: {words:?}"
    );
    assert!(
        words.iter().any(|w| w.contains("Formatters emit spaces.")),
        "rendered exchange must show Gem's multi-hop relayed reply: {words:?}"
    );
    assert!(
        !words.iter().any(|w| w.contains("ended without a reply")),
        "relayed replies with loaded transcripts must not render as silent turns: {words:?}"
    );
}
