//! every-teammate-is-a-bot.test.tsx, "what a teammate bot draws", where the app draws it: the sidebar's line of faces
//! (Sidebar.tsx, `TeammateBot` with the teammate's presence) and a conversation row's owner (`TeammateBot name`). The
//! three cases read `faces::bot_face_with_look` itself in src/look/teammate_bot_markup_tests.rs; these read the same
//! hooks off the real window, so a sidebar that stopped handing the face its teammate, activity or presence fails here.

use super::*;
use locust_core::bot_rig::FaceActivity;
use locust_core::engine::ConversationRow;
use rune::{Env, Runtime};
use serde_json::Value;

fn codec() -> Teammate {
    Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

fn asking(mission_id: &str) -> locust_core::approval::Request {
    locust_core::approval::Request {
        approval_id: "ask_1".into(),
        mission_id: mission_id.into(),
        runtime: "codex".into(),
        kind: locust_core::approval::Kind::Command,
        summary: "Run a command".into(),
        detail: "ls -la".into(),
        cwd: None,
        requested_at: "2026-11-26T12:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: Vec::new(),
    }
}

/// The real window with Codec on the team and one conversation of theirs; `waiting` puts an approval card on it.
fn window(waiting: bool) -> Runtime<State> {
    window_owned(waiting, Owner::Listed)
}

/// Where the window learns the conversation is Codec's: the listing's row names them, or only the window's own owners.
#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Listed,
    Kept,
}

fn window_owned(waiting: bool, owner: Owner) -> Runtime<State> {
    let mut s = State::default();
    // The wide sidebar, whatever the test window's width (the narrow rail draws the same face as `rail-…`).
    s.layout_pref = "wide".into();
    apply_team(&mut s, vec![codec()]);
    s.recent = vec![ConversationRow {
        id: "m_codec".into(),
        // The listing names the teammate, as the engine's rows do.
        teammate: if owner == Owner::Listed { "Codec".into() } else { String::new() },
        title: "Tidy the migrations".into(),
        started_by: "the person".into(),
        state: "running".into(),
        updated: "2026-11-26T12:00:00.000Z".into(),
    }];
    if owner == Owner::Kept {
        s.mission_owners.insert("m_codec".into(), "tm_codec".into());
    }
    if waiting {
        s.pending_approval = Some(asking("m_codec"));
    }
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| node(child, key))
}

/// The amber ring (`.lc-bot__ring`) is the one stroke drawn under the body, so it comes first.
fn ringed(slot: &str) -> bool {
    format!("{:?}", crate::look::faces::drawn_shape(slot).expect("drawn")).starts_with("Shape { ops: [Stroke")
}

#[test]
fn in_the_sidebar_a_teammates_bot_keeps_its_hooks_the_activity_the_teammate_and_its_own_shape() {
    let rt = window(false);
    let slot = "roster-tm_codec";
    // data-teammate: the face is Codec's own, its rig seeded from the id.
    assert_eq!(rt.state.bot_stage.seed(slot), Some(locust_core::bot_spec::seed_of("tm_codec")));
    // data-bot: the shape derived from Codec's seeded look, as botFor derives it.
    let derived = locust_core::bot_spec::bot_for(&locust_core::bot_spec::seed_avatar("tm_codec"));
    assert_eq!(crate::look::faces::drawn_look(slot).map(|(shape, _)| shape), Some(derived.shape.to_string()));
    // data-activity: at rest.
    assert_eq!(rt.state.bot_stage.desired_activity(slot), Some(FaceActivity::Idle));
    // aria-hidden: the strip's face is drawn but not read out; the button around it names Codec.
    assert!(rt.presented(&format!("bot-{slot}")).is_some(), "drawn");
    let tree = rt.accessibility_json();
    assert!(node(&tree, &format!("bot-{slot}")).is_none(), "aria-hidden");
    assert_eq!(node(&tree, "teammate-tm_codec").expect("the face's button")["name"], "Codec — open their conversation");
}

#[test]
fn in_the_sidebar_a_teammate_waiting_on_you_wears_the_amber_ring_and_dot() {
    let slot = "roster-tm_codec";
    // Whether the listing names the conversation's teammate or only the window keeps who owns it.
    for owner in [Owner::Listed, Owner::Kept] {
        let rt = window_owned(true, owner);
        assert_eq!(rt.state.bot_stage.desired_activity(slot), Some(FaceActivity::Waiting), "data-activity=\"waiting\"");
        assert!(ringed(slot), "lc-bot__ring");
        assert_eq!(crate::look::faces::drawn_presence(slot), Some(Some(crate::look::bot_stage::PresenceDot::Waiting)), "lc-presence--amber");
    }
    let rt = window(false);
    let _ = rt;
    assert!(!ringed(slot), "no ring once nothing waits");
    assert_eq!(crate::look::faces::drawn_presence(slot), Some(None));
}

#[test]
fn a_conversations_owner_bot_answers_to_the_teammates_name() {
    let rt = window(false);
    let tree = rt.accessibility_json();
    let owner = node(&tree, "conversation-owner-m_codec").expect("the row's owner face");
    assert_eq!(owner["role"], "image");
    assert_eq!(owner["name"], "Codec");
}
