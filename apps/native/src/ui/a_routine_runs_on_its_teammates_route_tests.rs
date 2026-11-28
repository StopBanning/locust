//! a-routine-runs-on-the-route-its-teammate-runs-on.test.ts (Colin, 2026-09-21: a routine that could never run). A
//! routine saved from a conversation nobody owned was stored on a guessed route; the dialog then asked who runs it, and
//! the answer never reached the route. Electron's test pins App.tsx's source; here the same rule is driven through
//! the window: the empty shelf's Save as routine on an ownerless conversation, the editor's "Who runs it", Save, and
//! the routine the native store then holds.
use super::*;
use locust_core::engine::ConversationRow;
use locust_core::routine_draft::{RoutineDraft, Saved};
use rune::{Env, Runtime};

struct Root(std::path::PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The native store with two teammates on different routes: Codec on Codex in Auto, Scout on OpenCode in Edit.
fn store() -> (Root, store::Store) {
    let root = std::env::temp_dir().join(format!("locust-routine-route-{}", locust_core::native::uuid_v4().unwrap()));
    let electron = root.join("electron");
    std::fs::create_dir_all(&electron).unwrap();
    let folder = root.to_string_lossy().into_owned();
    std::fs::write(electron.join("routines.json"), json!({"schemaVersion": 1, "routines": []}).to_string()).unwrap();
    std::fs::write(
        electron.join("teammates.json"),
        json!({"schemaVersion": 1, "teammates": [
            {"teammateId": "codec", "name": "Codec", "hue": "blue", "role": "Code & Migrations", "createdAt": "2026-09-01T00:00:00.000Z",
                "folder": folder, "route": {"runtime": "codex", "model": "gpt-6.1-sol", "mode": "auto"}},
            {"teammateId": "scout", "name": "Scout", "hue": "lime", "role": "Custom", "createdAt": "2026-09-01T00:00:00.000Z",
                "folder": folder, "route": {"runtime": "opencode", "model": "opencode/deepseek-v4.1-flash-free", "mode": "accept-edits"}}
        ]})
        .to_string(),
    )
    .unwrap();
    let store = store::Store::new(root.join("native"), electron).unwrap();
    (Root(root), store)
}

/// The ledger as this test has it: one finished conversation, typed by the person, that no turn records an owner for.
fn read(id: &str) -> Option<Saved> {
    (id == "m1").then(|| Saved {
        turns: 2,
        draft: Some(RoutineDraft { name: "Tidy the notes".into(), steps: vec!["Tidy the notes folder.".into()], learned_from: vec!["m1".into()], truncated: false }),
        chain: vec!["m1".into()],
    })
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

// case: saves the route of the teammate the person named in the dialog
#[test]
fn saves_the_route_of_the_teammate_the_person_named_in_the_dialog() {
    let (_root, store) = store();
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.data = Some(store.clone());
    s.routines.open = true;
    s.routines.file = crate::routines::list(&store).unwrap();
    // Its row names a runtime, not a teammate: nobody owns it.
    s.recent = vec![ConversationRow { id: "m1".into(), teammate: "Codex CLI".into(), title: "Tidy the notes".into(), started_by: "ledger".into(),
        state: "completed".into(), updated: "2026-09-21T04:00:00.000Z".into() }];
    s.routines.read_saved = read;
    offer_savable(&mut s);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.016);
    press(&mut rt, "routine-save-from-m1");
    assert!(rt.state.routines.choose_runner, "an ownerless conversation asks who runs it");
    press(&mut rt, "routine-runner-scout");
    press(&mut rt, "routine-save");
    assert_eq!(rt.state.routines.error, "", "saved");
    let saved = crate::routines::list(&store).unwrap()["routines"][0].clone();
    assert_eq!(saved["teammateId"], "scout");
    // Scout's own route, mode and all -- not Codex, the runtime the conversation's row named, and not a guessed Ask.
    assert_eq!(saved["route"], json!({"runtime": "opencode", "model": "opencode/deepseek-v4.1-flash-free", "mode": "accept-edits"}));
    assert_eq!(saved["learnedFrom"], json!(["m1"]));
}
