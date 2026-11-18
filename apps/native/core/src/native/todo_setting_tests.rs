//! The person's "keep a todo list" switch reaches the brief (Electron todo-list-is-opt-in, peer-exchange.ts): off, a
//! Codex teammate's brief carries no todo ask.
use super::*;

#[test]
fn a_teammates_brief_asks_for_no_todo_list_when_the_person_turned_it_off() {
    let t = Temp::new();
    let store = t.store();
    let mate = |id: &str, name: &str, role: &str| json!({"teammateId": id, "name": name, "role": role, "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}});
    store.save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [mate("tm_codec", "Codec", "Code & Migrations"), mate("tm_pip", "Pip", "Docs & QA")], "missionOwners": {}, "settings": {"relay": false, "keepATodoList": false}})).unwrap();
    // The fake Codex keeps the brief it was given in `cards-share`.
    let _g = hooks::install(store.clone(), fake("cards-share"));
    let prompt_file = t.0.join("last-prompt.txt");
    let first = send(job(&t, None, "Check the dev server")).unwrap();
    finished(&first.conversation_id);
    let brief = fs::read_to_string(&prompt_file).unwrap();
    assert!(brief.contains("<locust-ask>"), "the standing formats are still there: {brief}");
    assert!(!brief.contains("KEEP A TODO LIST"), "{brief}");
}
