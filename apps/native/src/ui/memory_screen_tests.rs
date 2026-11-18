//! Electron's `a-forgotten-memory-shows-with-restore.test.tsx` (2 of 2, first in part) and
//! `a-memory-shows-what-it-said-before.test.tsx` (2 of 2, first in part), drawn headless through `people_panel::memory_screen`.
//! In both, Rust draws the core fact and action (`Recently forgotten`, the memory's text, `Restore`; `Before: …`) without
//! Electron's extra line ("Kept for 7 days, then gone for good.", "forgotten by Booty", date, `Put it back` button).
use super::*;
use rune::{Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(memory_screen(s, &Palette::new(true)))
}
fn drawn(memories: Value) -> (String, Vec<String>) {
    let mut state = State::default();
    state.people_panel.memories = memories;
    let mut rt = Runtime::new(state, view, (900.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let words = all.iter().filter_map(|n| n.text.clone()).collect::<Vec<_>>().join("\n");
    let presses = all.iter().filter(|n| n.role == "button").filter_map(|n| n.label.clone()).collect();
    (words, presses)
}
fn kept_memory(extra: Value) -> Value {
    let mut base = json!({
        "memoryId": "mem_1",
        "text": "Orb suite: 31/31 passing.",
        "scope": "workspace",
        "workspaceId": "ws_shop",
        "workspaceName": "shop",
        "by": {"name": "Yurt"},
        "createdAt": "2026-09-01T10:00:00.000Z",
        "status": "kept",
        "enabled": true
    });
    if let (Some(dst), Some(src)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    base
}

#[test]
fn lists_what_was_forgotten_with_restore() {
    let (said, presses) = drawn(json!({
        "memories": [],
        "forgotten": [{
            "memory": {
                "memoryId": "mem_1",
                "text": "The API is on port 3000.",
                "scope": "workspace",
                "workspaceId": "ws_shop",
                "workspaceName": "shop",
                "by": {"name": "you"},
                "createdAt": "2026-09-01T10:00:00.000Z",
                "status": "kept",
                "enabled": true
            },
            "forgottenAt": "2026-09-24T05:00:00.000Z",
            "forgottenBy": {"teammateId": "tm_booty", "name": "Booty"}
        }]
    }));
    assert!(said.contains("Recently forgotten"), "{said}");
    assert!(said.contains("The API is on port 3000."), "{said}");
    assert!(said.contains("Restore"), "{said}");
    let _ = presses;
}

#[test]
fn recently_forgotten_is_not_drawn_when_nothing_was_forgotten() {
    let (said, _) = drawn(json!({"memories": [], "forgotten": []}));
    assert!(!said.contains("Recently forgotten"));
}

#[test]
fn shows_what_a_rewritten_memory_said_before() {
    let (said, _) = drawn(json!({
        "memories": [kept_memory(json!({
            "previousText": "Orb suite: 29/31, two flaky.",
            "updatedAt": "2026-09-24T05:00:00.000Z",
            "name": "orb-suite-status"
        }))],
        "forgotten": []
    }));
    assert!(said.contains("Before: Orb suite: 29/31, two flaky."), "{said}");
}

#[test]
fn shows_nothing_extra_for_a_memory_never_changed() {
    let (said, _) = drawn(json!({"memories": [kept_memory(json!({}))], "forgotten": []}));
    assert!(!said.contains("Before: "));
}
