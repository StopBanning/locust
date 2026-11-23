//! the-way-in-stays-on-the-shelf.test.tsx: the offer to save a finished conversation as a routine stays on the shelf
//! after the first routine is saved, and hides when nothing is savable or the screen cannot open the dialog.
use super::*;
use locust_core::routine_draft::{RoutineDraft, Saved};
use rune::{Env, Runtime};

const OFFER: &str = "Save a routine from a finished conversation";

fn routine_json() -> Value {
    json!({
        "routineId": "rt_1",
        "name": "roth check",
        "teammateId": "tm_wren",
        "route": { "runtime": "opencode", "model": "opencode/muse-spark-1.3-contributor-free", "mode": "ask" },
        "steps": ["Check the ladder."],
        "learnedFrom": [],
        "createdAt": "2026-09-20T09:00:00.000Z",
        "runs": 1
    })
}

fn savable_row() -> locust_core::engine::ConversationRow {
    locust_core::engine::ConversationRow {
        id: "mission_1".into(),
        teammate: "Wren".into(),
        title: "Check the Roth ladder".into(),
        started_by: "native".into(),
        state: "completed".into(),
        updated: "2026-09-21T09:00:00.000Z".into(),
    }
}

fn read_stub(id: &str) -> Option<Saved> {
    if id != "mission_1" {
        return None;
    }
    Some(Saved {
        turns: 2,
        chain: vec!["mission_1".into()],
        draft: Some(RoutineDraft {
            name: "Check the Roth ladder".into(),
            steps: vec!["Check the Roth ladder".into(), "Confirm the amounts".into()],
            learned_from: vec!["mission_1".into()],
            truncated: false,
        }),
    })
}

fn state_for(routines: Vec<Value>, recent: Vec<locust_core::engine::ConversationRow>, can_save: bool) -> State {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Custom".into(),
        runtime: Some("opencode".into()),
        model: Some("opencode/muse-spark-1.3-contributor-free".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.recent = recent;
    s.routines.open = true;
    s.routines.file = json!({ "schemaVersion": 1, "routines": routines });
    s.routines.read_saved = read_stub;
    s.routines.save_from_shelf = if can_save { Some(save_conversation_from_shelf) } else { None };
    offer_savable(&mut s);
    s
}

fn draw(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn all_text(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

#[test]
// case: offers to save one even when routines already exist
fn offers_to_save_one_even_when_routines_already_exist() {
    let mut rt = draw(state_for(vec![routine_json()], vec![savable_row()], true));
    let words = all_text(&rt);
    assert!(words.contains(OFFER), "missing offer in:\n{words}");
    assert!(words.contains("roth check"), "routine still beside it:\n{words}");
    let (x, y) = rt.find("routine-save-from-conversation-offer").expect("offer row on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(!rt.state.routines.fields.is_empty(), "pressing the offer opens the routine editor");
    assert_eq!(rt.state.routines.fields[0].text, "Check the Roth ladder");
}

#[test]
// case: offers nothing when there is nothing to save
fn offers_nothing_when_there_is_nothing_to_save() {
    let rt = draw(state_for(vec![routine_json()], vec![], true));
    let words = all_text(&rt);
    assert!(!words.contains(OFFER), "should not offer when nothing is savable:\n{words}");
    assert!(rt.find("routine-save-from-conversation-offer").is_none());
}

#[test]
// case: offers nothing when the screen cannot open the dialog
fn offers_nothing_when_the_screen_cannot_open_the_dialog() {
    let rt = draw(state_for(vec![routine_json()], vec![savable_row()], false));
    let words = all_text(&rt);
    assert!(!words.contains(OFFER), "should not offer when handler is absent:\n{words}");
    assert!(rt.find("routine-save-from-conversation-offer").is_none());
}

#[test]
// case: still names the way in on an empty shelf
fn still_names_the_way_in_on_an_empty_shelf() {
    let rt = draw(state_for(vec![], vec![savable_row()], true));
    let words = all_text(&rt);
    assert!(words.contains("finished conversations"), "empty shelf still offers finished conversations:\n{words}");
}
