//! Electron's `main/ask-me-first-covers-changes.test.ts` (0.315): in "Ask me first", a named rewrite of a kept memory
//! and a forget wait beside it as proposals, and the kept memory stays as it was, and briefed, until the person
//! answers. One Rust test per case, with its values.
use crate::people::{add_memory, add_memory_for, change_memory, memories, record_reply_for, workspace_id};
use crate::store::Store;
use serde_json::{Value, json};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct Shop {
    _t: Temp,
    store: Store,
    folder: PathBuf,
    ws: String,
}

/// A store whose team has Yurt and memory in `mode`, and the folder "shop".
fn shop(mode: &str) -> Shop {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("locust-ask-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    std::fs::write(
        root.join("electron").join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_yurt","name":"Yurt"}],"missionOwners":{},"settings":{"memoryMode":mode}}).to_string(),
    )
    .unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let folder = root.join("shop");
    let ws = workspace_id(&folder);
    Shop { _t: Temp(root), store, folder, ws }
}

fn yurt() -> Value {
    json!({"name":"Yurt","teammateId":"tm_yurt"})
}

fn rows(store: &Store) -> Vec<Value> {
    memories(store).unwrap()["memories"].as_array().cloned().unwrap_or_default()
}

fn proposals(store: &Store) -> Vec<Value> {
    rows(store).into_iter().filter(|m| m["status"] == "proposed").collect()
}

/// What a brief of the shop carries, as the brief chooses it (kept, enabled, this folder's), sorted.
fn briefed_in(store: &Store, folder: &std::path::Path) -> Vec<String> {
    let rows = crate::people::briefed_rows(store, folder, None).unwrap_or_default();
    let mut texts: Vec<String> = crate::people::briefed_memories(&rows, None, None).iter().map(|m| m["text"].as_str().unwrap_or("").to_owned()).collect();
    texts.sort();
    texts
}

fn kept(shop: &Shop, text: &str, name: Option<&str>) -> Value {
    add_memory(&shop.store, text, name, "workspace", &shop.ws, yurt(), "kept").unwrap();
    rows(&shop.store).into_iter().find(|m| m["text"] == text).unwrap()
}

fn propose(shop: &Shop, text: &str, name: Option<&str>, mission: Option<&str>) {
    add_memory_for(&shop.store, text, name, "workspace", &shop.ws, yurt(), "proposed", mission).unwrap();
}

fn forget_reply(quote: &str) -> String {
    format!("Done.\n<locust-memory>\nforget :: {quote}\n</locust-memory>")
}

// ---- a named rewrite in ask mode ----

// case: a named rewrite in ask mode > waits beside the kept memory, which stays as it was and briefed
#[test]
fn a_named_rewrite_waits_beside_the_kept_memory_which_stays_as_it_was_and_briefed() {
    let s = shop("ask");
    let k = kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    propose(&s, "Deploys go out on Thursdays.", Some("deploy-day"), Some("mission_2"));
    let waiting = proposals(&s.store);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0]["replaces"], k["memoryId"]);
    assert_eq!(waiting[0]["text"], "Deploys go out on Thursdays.");
    assert_eq!(waiting[0]["missionId"], "mission_2");
    assert_eq!(briefed_in(&s.store, &s.folder), vec!["Deploys go out on Fridays."]);
    let held = rows(&s.store).into_iter().find(|m| m["memoryId"] == k["memoryId"]).unwrap();
    assert_eq!(held["text"], "Deploys go out on Fridays.");
    assert!(held.get("previousText").is_none());
}

// case: a named rewrite in ask mode > a second rewrite before the person answers updates the same proposal
#[test]
fn a_second_rewrite_before_the_person_answers_updates_the_same_proposal() {
    let s = shop("ask");
    kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    propose(&s, "Deploys go out on Thursdays.", Some("deploy-day"), None);
    let first = proposals(&s.store)[0]["memoryId"].clone();
    propose(&s, "Deploys go out on Wednesdays.", Some("deploy-day"), Some("mission_5"));
    let waiting = proposals(&s.store);
    assert_eq!(waiting.iter().map(|m| m["text"].as_str().unwrap()).collect::<Vec<_>>(), vec!["Deploys go out on Wednesdays."]);
    assert_eq!(waiting[0]["memoryId"], first);
    assert_eq!(waiting[0]["missionId"], "mission_5");
}

// case: a named rewrite in ask mode > Keep the change rewrites the kept memory, one step back kept, and the proposal goes
#[test]
fn keep_the_change_rewrites_the_kept_memory_one_step_back_kept_and_the_proposal_goes() {
    let s = shop("ask");
    let k = kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    propose(&s, "Deploys go out on Thursdays.", Some("deploy-day"), Some("mission_2"));
    let proposal = proposals(&s.store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&s.store, &proposal, "keep", None).unwrap();
    let all = rows(&s.store);
    assert_eq!(all.iter().map(|m| m["memoryId"].clone()).collect::<Vec<_>>(), vec![k["memoryId"].clone()]);
    assert_eq!(all[0]["text"], "Deploys go out on Thursdays.");
    assert_eq!(all[0]["previousText"], "Deploys go out on Fridays.");
    assert_eq!(all[0]["missionId"], "mission_2");
    assert_eq!(all[0]["name"], "deploy-day");
    assert!(all[0]["updatedAt"].is_string());
    assert_eq!(briefed_in(&s.store, &s.folder), vec!["Deploys go out on Thursdays."]);
}

// case: a named rewrite in ask mode > Keep the old one drops only the proposal
#[test]
fn keep_the_old_one_drops_only_the_proposal() {
    let s = shop("ask");
    let k = kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    propose(&s, "Deploys go out on Thursdays.", Some("deploy-day"), None);
    let proposal = proposals(&s.store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&s.store, &proposal, "remove", None).unwrap();
    assert_eq!(rows(&s.store), vec![k]);
}

// case: a named rewrite in ask mode > auto mode still rewrites at once, as it always did
#[test]
fn auto_mode_still_rewrites_at_once_as_it_always_did() {
    let s = shop("auto");
    let k = kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    add_memory(&s.store, "Deploys go out on Thursdays.", Some("deploy-day"), "workspace", &s.ws, yurt(), "kept").unwrap();
    let all = rows(&s.store);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["memoryId"], k["memoryId"]);
    assert_eq!(all[0]["text"], "Deploys go out on Thursdays.");
    assert_eq!(all[0]["previousText"], "Deploys go out on Fridays.");
}

// ---- a forget in ask mode ----

// case: a forget in ask mode > waits beside the kept memory, which stays briefed
#[test]
fn a_forget_waits_beside_the_kept_memory_which_stays_briefed() {
    let s = shop("ask");
    // The person kept it; Yurt asks to forget it, and the proposal is Yurt's.
    add_memory(&s.store, "The API is on port 3000.", None, "workspace", &s.ws, json!({"name":"you"}), "kept").unwrap();
    let k = rows(&s.store).remove(0);
    let said = record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("The API is on port 3000"), Some("mission_3"), None).unwrap();
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(briefed_in(&s.store, &s.folder), vec!["The API is on port 3000."]);
    let waiting = proposals(&s.store);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0]["forgets"], k["memoryId"]);
    assert_eq!(waiting[0]["text"], "The API is on port 3000.");
    assert_eq!(waiting[0]["missionId"], "mission_3");
    assert_eq!(waiting[0]["by"], yurt());
}

// case: a forget in ask mode > asked twice, still one proposal -- and the proposal itself never makes the quote ambiguous
#[test]
fn asked_twice_still_one_proposal_and_the_proposal_itself_never_makes_the_quote_ambiguous() {
    let s = shop("ask");
    kept(&s, "The API is on port 3000.", None);
    record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("The API is on port 3000"), Some("mission_3"), None).unwrap();
    let again = record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("The API is on port 3000"), Some("mission_4"), None).unwrap();
    assert!(again.is_empty(), "no refusal: {again:?}");
    let waiting = proposals(&s.store);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0]["missionId"], "mission_3");
}

// case: a forget in ask mode > Forget it removes the kept memory and the proposal
#[test]
fn forget_it_removes_the_kept_memory_and_the_proposal() {
    let s = shop("ask");
    kept(&s, "The API is on port 3000.", None);
    record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("The API is on port 3000"), None, None).unwrap();
    let proposal = proposals(&s.store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&s.store, &proposal, "keep", None).unwrap();
    assert_eq!(rows(&s.store), Vec::<Value>::new());
}

// case: a forget in ask mode > Keep it drops only the proposal
#[test]
fn keep_it_drops_only_the_forget_proposal() {
    let s = shop("ask");
    kept(&s, "The API is on port 3000.", None);
    record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("The API is on port 3000"), None, None).unwrap();
    let proposal = proposals(&s.store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&s.store, &proposal, "remove", None).unwrap();
    assert_eq!(rows(&s.store).iter().map(|m| m["text"].as_str().unwrap()).collect::<Vec<_>>(), vec!["The API is on port 3000."]);
}

// case: a forget in ask mode > a memory nobody kept yet simply goes: there is nothing of the person’s to protect
#[test]
fn a_memory_nobody_kept_yet_simply_goes() {
    let s = shop("ask");
    add_memory(&s.store, "The API is on port 3000.", None, "workspace", &s.ws, yurt(), "proposed").unwrap();
    let said = record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("The API is on port 3000"), None, None).unwrap();
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(rows(&s.store), Vec::<Value>::new());
}

// case: a forget in ask mode > without ask, a forget applies at once, as it always did
#[test]
fn without_ask_a_forget_applies_at_once_as_it_always_did() {
    let s = shop("auto");
    kept(&s, "The API is on port 3000.", None);
    record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("The API is on port 3000"), None, None).unwrap();
    assert_eq!(rows(&s.store), Vec::<Value>::new());
}

// ---- a proposal whose memory is gone ----

// case: a proposal whose memory is gone > goes with it, whichever way the memory went
#[test]
fn a_proposal_goes_with_its_memory_whichever_way_the_memory_went() {
    let s = shop("ask");
    let k = kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    propose(&s, "Deploys go out on Thursdays.", Some("deploy-day"), None);
    record_reply_for(&s.store, "tm_yurt", &s.folder, &forget_reply("Deploys go out on Fridays"), None, None).unwrap();
    assert_eq!(proposals(&s.store).len(), 2);
    // The person removes the memory itself: both questions about it are moot.
    change_memory(&s.store, k["memoryId"].as_str().unwrap(), "remove", None).unwrap();
    assert_eq!(rows(&s.store), Vec::<Value>::new());
}

// ---- words already waiting as a change ----

// case: words already waiting as a change > are not asked a second time by a plain remember
#[test]
fn words_already_waiting_as_a_change_are_not_asked_a_second_time_by_a_plain_remember() {
    let s = shop("ask");
    kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    propose(&s, "Deploys go out on Thursdays.", Some("deploy-day"), None);
    propose(&s, "Deploys go out on Thursdays.", None, None);
    assert_eq!(proposals(&s.store).len(), 1);
}

// ---- the reader, in ask mode, on a real store ----

// case: the reader, in ask mode, on a real store > proposes the rewrite and the forget, changes nothing kept, and names each for what it is
#[test]
fn the_reader_proposes_the_rewrite_and_the_forget_and_changes_nothing_kept() {
    let s = shop("ask");
    kept(&s, "Deploys go out on Fridays.", Some("deploy-day"));
    kept(&s, "The API is on port 3000.", None);
    let said = record_reply_for(
        &s.store,
        "tm_yurt",
        &s.folder,
        "Done.\n<locust-memory>\nremember as deploy-day :: Deploys go out on Thursdays.\nforget :: The API is on port 3000\n</locust-memory>",
        Some("mission_9"),
        None,
    )
    .unwrap();
    assert_eq!(briefed_in(&s.store, &s.folder), vec!["Deploys go out on Fridays.", "The API is on port 3000."]);
    let waiting = proposals(&s.store);
    assert_eq!(waiting.len(), 2);
    assert!(waiting.iter().any(|m| m["text"] == "Deploys go out on Thursdays." && m.get("replaces").is_some() && m["missionId"] == "mission_9"));
    assert!(waiting.iter().any(|m| m["text"] == "The API is on port 3000." && m.get("forgets").is_some() && m["missionId"] == "mission_9"));
    // Proposed is not missed: nothing amber, nothing for the person to fix.
    assert!(said.is_empty(), "{said:?}");
}
