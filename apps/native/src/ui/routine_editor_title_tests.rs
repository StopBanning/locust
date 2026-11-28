use super::*;
use rune::{Env, Runtime};

fn title_of(rt: &Runtime<State>) -> Option<String> {
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .find(|e| e.key.as_deref() == Some("routine-editor-title"))
        .and_then(|e| e.text)
}

#[test]
fn new_routine_editor_is_titled_new_routine_not_save_as_routine() {
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.routines.open = true;
    s.routines.file = json!({"schemaVersion": 1, "routines": []});

    let mut rt = Runtime::new(
        s,
        |s, _| view(s, &Palette::new(true)),
        (1200.0, 1400.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);

    let (x, y) = rt.find("routine-new").expect("New routine button in header");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(
        title_of(&rt).as_deref(),
        Some("New routine"),
        "pressing New routine must open an editor titled New routine"
    );

    rt.update(|s, _| {
        edit(
            s,
            json!({"name": "", "steps": [""], "teammateId": "codec", "learnedFrom": []}),
        );
    });
    rt.frame(0.05);
    assert_eq!(
        title_of(&rt).as_deref(),
        Some("New routine"),
        "an unsaved routine with an empty learnedFrom list is still New routine"
    );

    rt.update(|s, _| {
        edit(
            s,
            json!({"name": "Daily check", "steps": ["Check status"], "teammateId": "codec", "learnedFrom": ["m_1"]}),
        );
    });
    rt.frame(0.05);
    assert_eq!(title_of(&rt).as_deref(), Some("Save as routine"));

    rt.update(|s, _| {
        edit(
            s,
            json!({"routineId": "rt_1", "name": "Daily check", "steps": ["Check status"], "teammateId": "codec", "learnedFrom": []}),
        );
    });
    rt.frame(0.05);
    assert_eq!(title_of(&rt).as_deref(), Some("Edit routine"));
}
