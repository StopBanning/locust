//! A conversation with no teammate, continued on the engine (Electron 0.878, `an-app-continues-a-conversation-with-no-
//! teammate.test.ts`): the turn runs on the conversation's own agent and model, and stays nobody's.
use super::*;

fn owners(t: &Temp) -> Value {
    let file: Value = serde_json::from_str(&fs::read_to_string(t.store().root().join("teammates.json")).unwrap_or_else(|_| "{}".into())).unwrap();
    file["missionOwners"].clone()
}

#[test]
fn a_turn_for_nobody_continues_the_conversation_on_its_own_agent_and_records_no_owner() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let work = t.0.join("work");
    fs::create_dir_all(&work).unwrap();
    let first = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&first.conversation_id);
    assert_eq!(owners(&t)[&first.conversation_id], "tm_codec", "a teammate's turn is that teammate's");

    // Its teammate is removed: the conversation has no owner now.
    let mut file: Value = serde_json::from_str(&fs::read_to_string(t.store().root().join("teammates.json")).unwrap()).unwrap();
    file["missionOwners"] = json!({});
    t.store().save_json("teammates.json", &file).unwrap();

    *crate::invariants::lock(&hooks::STATE) = (Some(t.store()), fake("resume"));
    let next = send_from(
        Send {
            teammate: nobody(Some("codex".into()), Some("synthetic-model".into())),
            conversation: Some(first.conversation_id.clone()),
            prompt: "Synthetic follow-up".into(),
            model: None,
            effort: None,
        },
        Origin { mode: Some(Mode::Ask), folder: Some(work), started_by: Some(json!({"kind": "mcp"})), ..Default::default() },
    )
    .unwrap();
    assert_eq!(next.conversation_id, first.conversation_id, "the same conversation goes on");
    let tip = real_tip(&first.conversation_id).unwrap();
    assert_ne!(tip, first.conversation_id);
    finished(&tip);
    let child = path_of(&t, &tip);
    let failed: Vec<Value> = records(&child).into_iter().filter(|r| r["event"]["type"] == "run.failed").map(|r| r["event"]["payload"].clone()).collect();
    assert_eq!(kinds(&child).last().map(String::as_str), Some("run.completed"), "{:?} {failed:?}", kinds(&child));
    let meta = &records(&child)[0]["metadata"];
    assert_eq!((meta["runtime"].as_str(), meta["model"].as_str()), (Some("codex"), Some("synthetic-model")));
    assert_eq!(meta["continuesFrom"]["runtimeThreadId"], "thread_fake", "the conversation's own session is resumed");
    assert!(owners(&t).get(&tip).is_none(), "nobody's turn records no owner: {}", owners(&t));
    assert_eq!(crate::people::owner_of(&t.store(), &tip), None);
}

#[test]
fn nobody_is_the_empty_id_no_roster_entry_can_have() {
    let none = nobody(Some("claude".into()), Some("opus".into()));
    assert!(is_nobody(&none));
    assert_eq!((none.runtime.as_deref(), none.model.as_deref(), none.runs_in.as_str()), (Some("claude"), Some("opus"), ""));
    assert!(none.account.is_none() && none.folder.is_none());
    let t = Temp::new();
    assert!(!is_nobody(&codex(&t)));
}
