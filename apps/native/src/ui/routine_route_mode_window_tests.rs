use super::*;

// case: says the model and the mode it runs in, not a rule about read-only
#[test]
fn the_edit_routine_window_says_the_live_model_and_the_saved_mode() {
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.teammates[0].name = "Wren".into();
    s.teammates[0].runtime = Some("cursor".into());
    s.teammates[0].model = Some("grok-4.7".into());
    s.teammates[0].runs_in = "ask".into();
    let owner = s.teammates[0].id.clone();
    s.routines.open = true;
    edit(&mut s, json!({
        "routineId":"rt_1", "name":"Weekly check", "teammateId":owner,
        "route":{"runtime":"codex","model":"old-model","mode":"accept-edits"},
        "steps":["Write the receipt."], "schedule":{"kind":"daily","at":"09:00"}
    }));
    let mut rt = rune::Runtime::new(s, crate::view, (1200.0, 4000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let elements = rt.elements();
    let row = elements.iter().find(|e| e.key.as_deref() == Some("routine-runs-it")).and_then(|e| e.text.as_deref());
    assert_eq!(row, Some("Wren runs it on Cursor / Grok 4.7, in Edit. Each run uses the model Wren last worked on, and always Edit."));
    assert!(!elements.iter().filter_map(|e| e.text.as_deref()).any(|text| text.contains("stays read-only") || text.contains("Old Model")));
}
