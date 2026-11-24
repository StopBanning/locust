//! Electron `main/memory-store.test.ts`, case by case, through the app's own paths into the memory store: a finished
//! reply's memory block (`record_reply_for`, as a run keeps and forgets), the Memory screen's edit, switch, keep, Put it
//! back and Remove (`change_memory`), its Forget card (`clear_memories`), and what a brief stands on (`briefed_rows`).
use super::*;
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A store with Wren on the team, and two project folders, shop and ledger.
fn fixture() -> (Temp, Store, PathBuf, PathBuf) {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("locust-memory-store-cases-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["electron", "native", "shop", "ledger"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(
        root.join("native").join("teammates.json"),
        json!({ "schemaVersion": 1, "teammates": [{ "teammateId": "tm_wren", "name": "Wren" }], "missionOwners": {}, "settings": {} }).to_string(),
    )
    .unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let (shop, ledger) = (root.join("shop"), root.join("ledger"));
    (Temp(root), store, shop, ledger)
}

fn rows(store: &Store) -> Vec<Value> {
    memories(store).unwrap()["memories"].as_array().cloned().unwrap_or_default()
}

fn file_on_disk(t: &Temp) -> PathBuf {
    t.0.join("native").join("memories.json")
}

fn wren() -> Value {
    json!({ "teammateId": "tm_wren", "name": "Wren" })
}

/// A run of Wren's in `folder` keeps `text` there, as its reply's memory block says.
fn wren_remembers(store: &Store, folder: &Path, text: &str, mission: Option<&str>) -> Result<Vec<(String, &'static str)>, String> {
    record_reply_for(store, "tm_wren", folder, &format!("Done.\n<locust-memory>\nremember :: {text}\n</locust-memory>"), mission, None)
}

fn id_of(store: &Store, text: &str) -> String {
    rows(store).iter().find(|m| m["text"] == text).unwrap()["memoryId"].as_str().unwrap().to_owned()
}

fn recent(at: &Value) -> bool {
    at.as_str()
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .is_some_and(|at| (chrono::Utc::now() - at.with_timezone(&chrono::Utc)).num_seconds().abs() < 60)
}

#[test]
fn keeps_one_step_back_and_says_when_so_put_it_back_is_a_swap_and_can_be_undone() {
    // case: keeps one step back and says when, so Put it back is a swap and can be undone
    let (_t, store, shop, _) = fixture();
    wren_remembers(&store, &shop, "Orb suite: 29/31, two flaky.", None).unwrap();
    let id = id_of(&store, "Orb suite: 29/31, two flaky.");
    change_memory(&store, &id, "edit", Some("Orb suite: 31/31 passing.")).unwrap();
    let edited = rows(&store).into_iter().find(|m| m["memoryId"] == id).unwrap();
    assert_eq!(edited["previousText"], "Orb suite: 29/31, two flaky.");
    assert!(recent(&edited["updatedAt"]), "{edited}");
    // Put it back, as the Memory screen's button does: the same edit with the previous text -- and the edit is now the
    // step back.
    change_memory(&store, &id, "edit", Some(edited["previousText"].as_str().unwrap())).unwrap();
    let back = rows(&store).into_iter().find(|m| m["memoryId"] == id).unwrap();
    assert_eq!(back["text"], "Orb suite: 29/31, two flaky.");
    assert_eq!(back["previousText"], "Orb suite: 31/31 passing.");
}

#[test]
fn changes_nothing_of_the_kind_when_only_the_switch_moves() {
    // case: changes nothing of the kind when only the switch moves
    let (_t, store, shop, _) = fixture();
    wren_remembers(&store, &shop, "Tests run with pnpm test.", None).unwrap();
    let id = id_of(&store, "Tests run with pnpm test.");
    change_memory(&store, &id, "toggle", None).unwrap();
    let off = rows(&store).into_iter().find(|m| m["memoryId"] == id).unwrap();
    assert_eq!(off["enabled"], false);
    assert!(off.get("previousText").is_none(), "{off}");
    assert!(off.get("updatedAt").is_none(), "{off}");
}

#[test]
fn is_refused_not_written_over_when_it_is_torn() {
    // case: is refused, not written over, when it is torn
    let (t, store, shop, _) = fixture();
    let torn = r#"{"schemaVersion":1,"memories":[{"memoryId":"mem_a","text":"Tests run"#;
    std::fs::write(file_on_disk(&t), torn).unwrap();
    assert_eq!(memories(&store).unwrap_err(), MEMORY_UNREADABLE);
    // The run's new memory is refused, and the thread says why.
    let said = wren_remembers(&store, &shop, "A new one.", None).unwrap();
    assert!(said.iter().any(|(words, _)| words.contains(MEMORY_UNREADABLE)), "{said:?}");
    assert_eq!(std::fs::read_to_string(file_on_disk(&t)).unwrap(), torn);
}

#[test]
fn is_refused_when_it_is_a_schema_this_build_does_not_know_which_a_later_build_may_have_written() {
    // case: is refused when it is a schema this build does not know, which a later build may have written
    let (t, store, shop, _) = fixture();
    let newer = json!({ "schemaVersion": 99, "memories": [{ "memoryId": "mem_a", "text": "kept by a newer build" }] }).to_string();
    std::fs::write(file_on_disk(&t), &newer).unwrap();
    // The run's new memory is refused, and the thread says why.
    let said = wren_remembers(&store, &shop, "A new one.", None).unwrap();
    assert!(said.iter().any(|(words, _)| words.contains(MEMORY_UNREADABLE)), "{said:?}");
    assert_eq!(std::fs::read_to_string(file_on_disk(&t)).unwrap(), newer);
}

#[test]
fn is_genuinely_empty_when_there_is_simply_no_file_yet() {
    // case: is genuinely empty when there is simply no file yet
    let (_t, store, shop, _) = fixture();
    assert_eq!(rows(&store), Vec::<Value>::new());
    let created = add_memory_in(&store, "The first one.", None, "workspace", &workspace_id(&shop), "shop", wren(), "kept", None).unwrap();
    assert_eq!(created, Added::Kept);
}

#[test]
fn keeps_a_memory_with_who_wrote_it_where_and_from_which_conversation_on_disk() {
    // case: keeps a memory with who wrote it, where, and from which conversation, on disk
    let (t, store, shop, _) = fixture();
    wren_remembers(&store, &shop, "  Tests run with pnpm test.  ", Some("mission_1")).unwrap();
    let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(file_on_disk(&t)).unwrap()).unwrap();
    assert_eq!(on_disk["schemaVersion"], 1);
    let mut memory = on_disk["memories"][0].clone();
    assert!(memory["memoryId"].as_str().unwrap().starts_with("mem_"), "{memory}");
    assert!(recent(&memory["createdAt"]), "{memory}");
    let fields = memory.as_object_mut().unwrap();
    fields.remove("memoryId");
    fields.remove("createdAt");
    assert_eq!(
        memory,
        json!({
            "text": "Tests run with pnpm test.",
            "scope": "workspace",
            "workspaceId": workspace_id(&shop),
            "workspaceName": "shop",
            "by": { "name": "Wren", "teammateId": "tm_wren" },
            "missionId": "mission_1",
            "status": "kept",
            "enabled": true
        })
    );
    // Read again from disk, it is the same memory.
    assert_eq!(rows(&store), on_disk["memories"].as_array().unwrap().clone());
}

#[test]
fn does_not_write_the_same_memory_twice_in_the_same_place_and_a_proposed_one_stays_proposed() {
    // case: does not write the same memory twice in the same place, and a proposed one stays proposed
    let (_t, store, shop, ledger) = fixture();
    // Everywhere waits for the person, so Wren's run in shop proposes it.
    record_reply_for(&store, "tm_wren", &shop, "Done.\n<locust-memory>\nremember everywhere :: Colin wants diffs, not prose.\n</locust-memory>", None, None).unwrap();
    let first = rows(&store)[0].clone();
    assert_eq!((first["scope"].as_str(), first["status"].as_str()), (Some("global"), Some("proposed")));
    // The same words kept by Juno from another folder change nothing: one memory, still proposed.
    let again = add_memory_in(&store, "colin wants DIFFS not prose", None, "global", &workspace_id(&ledger), "ledger", json!({ "name": "Juno" }), "kept", None).unwrap();
    assert_eq!(again, Added::Nothing);
    assert_eq!(rows(&store), vec![first]);
    // The same text in another FOLDER is another memory.
    let elsewhere = add_memory_in(&store, "Colin wants diffs, not prose.", None, "workspace", &workspace_id(&ledger), "ledger", wren(), "kept", None).unwrap();
    assert_eq!(elsewhere, Added::Kept);
    assert_eq!(rows(&store).len(), 2);
}

#[test]
fn briefs_a_folders_mission_with_that_folders_kept_memories_and_everyones_never_a_proposed_or_switched_off_one() {
    // case: briefs a folder's mission with that folder's kept memories and everyone's, never a proposed or switched-off one
    let (t, store, shop, ledger) = fixture();
    wren_remembers(&store, &shop, "shop kept", None).unwrap();
    add_memory_in(&store, "everywhere", None, "global", &workspace_id(&ledger), "ledger", wren(), "kept", None).unwrap();
    wren_remembers(&store, &ledger, "ledger kept", None).unwrap();
    add_memory_in(&store, "shop proposed", None, "workspace", &workspace_id(&shop), "shop", wren(), "proposed", None).unwrap();
    wren_remembers(&store, &shop, "shop off", None).unwrap();
    change_memory(&store, &id_of(&store, "shop off"), "toggle", None).unwrap();
    let texts = |folder: &Path| -> Vec<String> {
        briefed_rows(&store, folder, None).unwrap().iter().map(|m| m["text"].as_str().unwrap().to_owned()).collect()
    };
    assert_eq!(texts(&shop), vec!["shop kept".to_owned(), "everywhere".to_owned()]);
    let other = t.0.join("other");
    assert_eq!(texts(&other), vec!["everywhere".to_owned()]);
}

#[test]
fn a_person_edits_the_text_switches_one_off_and_on_keeps_a_proposed_one_and_removes_one() {
    // case: a person edits the text, switches one off and on, keeps a proposed one, and removes one
    let (_t, store, shop, _) = fixture();
    add_memory_in(&store, "Port is 3000", None, "workspace", &workspace_id(&shop), "shop", wren(), "proposed", None).unwrap();
    let id = id_of(&store, "Port is 3000");
    change_memory(&store, &id, "edit", Some("Port is 3001")).unwrap();
    change_memory(&store, &id, "keep", None).unwrap();
    let kept = rows(&store).into_iter().find(|m| m["memoryId"] == id).unwrap();
    assert_eq!((kept["text"].as_str(), kept["status"].as_str(), kept["enabled"].as_bool()), (Some("Port is 3001"), Some("kept"), Some(true)));
    change_memory(&store, &id, "toggle", None).unwrap();
    assert_eq!(rows(&store).into_iter().find(|m| m["memoryId"] == id).unwrap()["enabled"], false);
    change_memory(&store, &id, "toggle", None).unwrap();
    assert_eq!(rows(&store).into_iter().find(|m| m["memoryId"] == id).unwrap()["enabled"], true);
    assert!(change_memory(&store, &id, "edit", Some("")).unwrap_err().contains("one line"));
    assert!(change_memory(&store, "mem_nope", "edit", Some("x")).unwrap_err().contains("no longer exists"));
    change_memory(&store, &id, "remove", None).unwrap();
    assert_eq!(rows(&store), Vec::<Value>::new());
}

#[test]
fn forget_by_text_hits_the_folder_and_everywhere_not_another_folder_clear_takes_a_folder_or_all() {
    // case: forget by text hits the folder and everywhere, not another folder; clear takes a folder or all
    let (_t, store, shop, ledger) = fixture();
    wren_remembers(&store, &shop, "The API is on port 3000", None).unwrap();
    add_memory_in(&store, "The API is on port 3000", None, "global", &workspace_id(&shop), "shop", wren(), "kept", None).unwrap();
    wren_remembers(&store, &ledger, "The API is on port 3000", None).unwrap();
    // A run in shop forgets it: this folder's copy and everywhere's both go, each named.
    let (said, changed) = record_reply_changes(
        &store,
        "tm_wren",
        &shop,
        "Done.\n<locust-memory>\nforget :: the api is on port 3000!\n</locust-memory>",
        None,
        None,
    )
    .unwrap();
    assert_eq!(said, Vec::<(String, &'static str)>::new());
    assert_eq!(changed.forgotten, vec!["The API is on port 3000".to_owned()]);
    let left: Vec<String> = rows(&store).iter().map(|m| m["workspaceId"].as_str().unwrap().to_owned()).collect();
    assert_eq!(left, vec![workspace_id(&ledger)]);
    let shop_id = workspace_id(&shop);
    add_memory_in(&store, "a", None, "workspace", &shop_id, "shop", wren(), "kept", None).unwrap();
    add_memory_in(&store, "b", None, "global", &shop_id, "shop", wren(), "kept", None).unwrap();
    assert_eq!(clear_memories(&store, Some(&shop_id)).unwrap(), 1);
    let texts: Vec<String> = rows(&store).iter().map(|m| m["text"].as_str().unwrap().to_owned()).collect();
    assert_eq!(texts, vec!["The API is on port 3000".to_owned(), "b".to_owned()]);
    assert_eq!(clear_memories(&store, None).unwrap(), 2);
    assert_eq!(rows(&store), Vec::<Value>::new());
}

#[test]
fn refuses_what_it_cannot_keep_and_a_malformed_file_is_unreadable_rather_than_empty() {
    // case: refuses what it cannot keep, and a malformed file is unreadable rather than empty
    let (t, store, shop, _) = fixture();
    let ws = workspace_id(&shop);
    assert!(add_memory_in(&store, "", None, "workspace", &ws, "shop", wren(), "kept", None).unwrap_err().contains("one line"));
    assert_eq!(
        add_memory_in(&store, "x", None, "team", &ws, "shop", wren(), "kept", None).unwrap_err(),
        "A memory is for this folder or for everywhere."
    );
    // A torn FILE is unreadable, never empty.
    std::fs::write(file_on_disk(&t), "{{{").unwrap();
    assert_eq!(memories(&store).unwrap_err(), MEMORY_UNREADABLE);
    // A bad MEMORY inside a good file is still just dropped.
    std::fs::write(file_on_disk(&t), json!({ "schemaVersion": 1, "memories": [{ "memoryId": "mem_1", "text": "no scope" }] }).to_string()).unwrap();
    assert_eq!(memories(&store).unwrap()["memories"], json!([]));
    std::fs::write(file_on_disk(&t), "not json").unwrap();
    assert_eq!(memories(&store).unwrap_err(), MEMORY_UNREADABLE);
}

#[test]
fn is_bounded() {
    // case: is bounded
    let (t, store, shop, _) = fixture();
    let ws = workspace_id(&shop);
    let full: Vec<Value> = (0..MAX_MEMORIES)
        .map(|i| {
            json!({
                "memoryId": format!("mem_seed{i}"), "text": format!("memory {i}"), "scope": "workspace", "workspaceId": ws,
                "workspaceName": "shop", "by": wren(), "createdAt": "2026-09-05T12:00:00.000Z", "status": "kept", "enabled": true
            })
        })
        .collect();
    std::fs::write(file_on_disk(&t), json!({ "schemaVersion": 1, "memories": full }).to_string()).unwrap();
    assert_eq!(rows(&store).len(), 400);
    let refused = wren_remembers(&store, &shop, "one more", None).unwrap();
    assert!(refused.iter().any(|(said, _)| said.contains("at most")), "{refused:?}");
    assert_eq!(rows(&store).len(), 400);
    assert!(add_memory_in(&store, "one more", None, "workspace", &ws, "shop", wren(), "kept", None).unwrap_err().contains("at most"));
}
