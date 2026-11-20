//! Electron `renderer/src/a-busy-provider-offers-continue.test.ts`, in the window: the card says it is the provider,
//! and Continue sends Locust's words into the same conversation. The rule's cases are in
//! `core/src/busy_continue_tests.rs`.

use crate::*;
use rune::{Env, Key, Runtime};
use std::path::PathBuf;
use std::time::Duration;

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let mut now = 0.0;
    while !done(&rt.state) {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}: status {:?}", rt.state.status);
        rt.update(move |s, tx| {
            poll(s, now, tx);
        });
        rt.frame(0.3);
        now += 0.3;
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

// case: the card says it is the provider, and Continue sends into the same conversation
#[test]
fn the_busy_card_says_it_is_the_provider_and_continue_sends_into_the_same_conversation() {
    let root = std::env::temp_dir().join(format!("locust-busy-window-{}", std::process::id()));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let _dirs = Dirs(root.clone());
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json("teammates.json", &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec"}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let fake = (
        PathBuf::from("python3"),
        vec![format!("{}/scripts/fixtures/m1-app-server.py", env!("CARGO_MANIFEST_DIR")), "success".into()],
    );
    let _hook = native::hooks::install(store.clone(), Some(fake));
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    apply_team(
        &mut s,
        vec![Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Code & Migrations".into(),
            runtime: Some("codex".into()),
            model: Some("synthetic-model".into()),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: Some(root.join("work").to_string_lossy().into()),
        }],
    );
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.frame(0.0);
    click(&mut rt, "composer");
    rt.paste("Synthetic prompt");
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    rt.frame(0.016);
    pump(&mut rt, "the reply", |s| {
        s.flight.is_none() && s.ledger.as_ref().is_some_and(|p| !p.loading && !p.snapshot.running && !p.items.is_empty())
    });
    settle(&mut rt);
    // A turn that ended well offers nothing.
    assert!(rt.find("busy-continue").is_none());
    let conversation = rt.state.ledger.as_ref().and_then(|p| p.conversation.conversation_id.clone());
    assert!(conversation.is_some());

    // The same turn as the record of one the provider dropped would read (core's case reads it from such a record).
    rt.update(|s, _| {
        if let Some(pane) = s.ledger.as_mut() {
            pane.snapshot.busy_continue = true;
        }
    });
    settle(&mut rt);
    assert!(
        texts(&rt).iter().any(|t| t == "The model's servers were busy: not your account, and nothing you did. What it finished is kept; Continue picks up where it stopped."),
        "{:?}",
        texts(&rt)
    );
    rt.update(|s, _| s.draft.replace_text("half a thought"));
    click(&mut rt, "busy-continue-action");
    let flight = rt.state.flight.as_ref().expect("Continue sends");
    assert_eq!(flight.message.as_deref(), Some("Continue from where you stopped."));
    assert_eq!(flight.conversation_id, conversation, "into the same conversation");
    assert_eq!(rt.state.draft.text, "half a thought", "the person's draft is kept");
    assert!(rt.find("busy-continue").is_none(), "not offered while the follow-up goes");
    pump(&mut rt, "the follow-up", |s| s.flight.is_none() && s.ledger.as_ref().is_some_and(|p| !p.loading && !p.snapshot.running));
}
