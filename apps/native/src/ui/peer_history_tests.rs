//! Electron's `main/mission-history.test.ts`, "peer messages in history": a mission's peer links joined with the
//! workroom's text. The native window joins them where it draws them (`messages_for_mission`), from the links its
//! ledger pane read and the workroom messages `main.rs` read beside the history (`Workroom::for_store(..).read()`).
use super::*;

const NOW: &str = "2026-08-31T15:00:00.000Z";

fn link(direction: &str, message_id: &str) -> ledger::PeerLink {
    ledger::PeerLink { direction: direction.into(), message_id: message_id.into(), peer_teammate_id: "tm_atlas".into(), occurred_at: NOW.into() }
}

fn atlas_message(message_id: &str) -> locust_core::workroom::Message {
    locust_core::workroom::Message {
        message_id: message_id.into(),
        sequence: 1,
        from: locust_core::workroom::Party { teammate_id: "tm_atlas".into(), name: "Atlas".into() },
        from_mission: "mission_a".into(),
        to: locust_core::workroom::Party { teammate_id: "tm_wren".into(), name: "Wren".into() },
        text: "pnpm check runs everything.".into(),
        posted_at: NOW.into(),
        observed: None,
    }
}

/// The window with mission_1's links read, Wren owning mission_1, and the workroom messages it holds.
fn state(links: Vec<ledger::PeerLink>, workroom: Vec<locust_core::workroom::Message>) -> State {
    let mut s = State::default();
    let header = Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Builder".into(), model: None, runtime: Some("codex".into()), folder: None, runs_in: "ask".into(), account: None, busy: false };
    s.teammates.push(header.clone());
    s.mission_owners.insert("mission_1".into(), "tm_wren".into());
    s.workroom_messages = workroom.into_iter().map(|m| (m.message_id.clone(), m)).collect();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some("mission_1".into()), turns: Vec::new() },
        items: Vec::new(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot {
            turn_metas: vec![ledger::Metadata { mission_id: "mission_1".into(), ..Default::default() }],
            peer_links: links.into_iter().map(|l| ("mission_1".to_owned(), l)).collect(),
            ..Default::default()
        },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s
}

// case: joins a mission's links with the workroom text, keeping direction
#[test]
fn a_received_link_is_drawn_with_the_workroom_text_and_its_sender() {
    let s = state(vec![link("received", "wm_1")], vec![atlas_message("wm_1")]);
    let drawn = messages_for_mission(&s, "mission_1");
    assert_eq!(drawn.len(), 1);
    let m = &drawn[0];
    assert_eq!((m.message_id.as_str(), m.direction.as_str()), ("wm_1", "received"));
    // With the conversation it was sent from (0.463).
    assert_eq!((m.from_id.as_str(), m.from_name.as_str(), m.from_mission.as_deref()), ("tm_atlas", "Atlas", Some("mission_a")));
    // To the teammate whose mission received it (the window calls its own side "You").
    assert_eq!(m.to_id, "tm_wren");
    assert_eq!(m.text.as_deref(), Some("pnpm check runs everything."));
    assert_eq!(m.posted_at, NOW);
}

// case: keeps a link whose message the workroom no longer holds, with no text
#[test]
fn a_link_whose_message_is_gone_is_kept_with_no_text() {
    // Dropping it would show a mission that was briefed from a claim as if it had been briefed from nothing.
    let s = state(vec![link("received", "wm_gone")], Vec::new());
    let drawn = messages_for_mission(&s, "mission_1");
    assert_eq!(drawn.len(), 1);
    assert_eq!((drawn[0].message_id.as_str(), drawn[0].direction.as_str()), ("wm_gone", "received"));
    assert_eq!(drawn[0].text, None);
}

// case: reads history even when the workroom itself cannot be read
#[test]
fn the_history_draws_its_links_when_the_workroom_cannot_be_read() {
    // A workroom whose channel is not a file it can read: what main.rs reads beside the history.
    let root = std::env::temp_dir().join(format!("locust-peer-history-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let workroom = locust_core::workroom::Workroom::for_store(&store);
    let channel = store.root().join(locust_core::workroom::FOLDER).join(locust_core::workroom::FILE_NAME);
    std::fs::create_dir_all(&channel).unwrap();
    let read = workroom.read();
    assert!(!read.issues.is_empty(), "the workroom read as whole");
    let s = state(vec![link("posted", "wm_1")], read.messages);
    let drawn = messages_for_mission(&s, "mission_1");
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].direction, "posted");
    assert_eq!(drawn[0].text, None);
}
