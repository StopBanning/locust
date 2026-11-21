// Memory round 1 (19 Nov 2026): the Memory screen cases under apps/desktop/src/renderer/src, drawn headless.
use super::*;
use crate::memory_card::{MemoryChangedUpdate, memories_of_conversation, memory_card_summary, memory_changed_notice, notice_waits};
use rune::{Env, Runtime};

const DAY: i64 = 24 * 60 * 60 * 1000;
fn ago(days: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::milliseconds(days * DAY)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn kept(id: &str, text: &str) -> Value {
    json!({"memoryId":id,"text":text,"scope":"workspace","workspaceId":"ws_shop","workspaceName":"shop","by":{"name":"you"},"createdAt":"2026-09-01T10:00:00.000Z","status":"kept","enabled":true})
}
fn draw(prepare: impl FnOnce(&mut State)) -> Vec<String> {
    let mut s = State::default();
    s.people_panel.open = true;
    s.people_panel.tab = 1;
    s.people_panel.memory_tab = true;
    s.people_panel.memory_mode = "auto".into();
    s.home.folder_path = Some("/tmp/shop".into());
    prepare(&mut s);
    let mut rt = Runtime::new(s, |s, _| memory_screen(s, &Palette::new(true)), (900.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}
fn has(texts: &[String], words: &str) -> bool {
    texts.iter().any(|t| t.contains(words))
}

// ---- a-memory-nobody-is-given-says-so-on-screen.test.tsx (A1.4) ----
fn old_note(extra: impl FnOnce(&mut Value)) -> Value {
    let mut m = kept("mem_old", "An old note.");
    m["createdAt"] = json!(ago(90));
    extra(&mut m);
    m
}

#[test]
fn says_how_long() {
    let shown = draw(|s| s.people_panel.memories = json!({"memories":[old_note(|_| {})],"briefTrackingSince":ago(40)}));
    assert!(has(&shown, "No teammate has been given this in 40 days."), "{shown:?}");
}

#[test]
fn says_nothing_for_one_given_lately_or_before_a_month_of_counting() {
    let lately = draw(|s| s.people_panel.memories = json!({"memories":[old_note(|m| m["lastBriefedAt"] = json!(ago(3)))],"briefTrackingSince":ago(40)}));
    assert!(!has(&lately, "No teammate has been given this"), "{lately:?}");
    let early = draw(|s| s.people_panel.memories = json!({"memories":[old_note(|_| {})],"briefTrackingSince":ago(10)}));
    assert!(!has(&early, "No teammate has been given this"), "{early:?}");
    let never = draw(|s| s.people_panel.memories = json!({"memories":[old_note(|_| {})]}));
    assert!(!has(&never, "No teammate has been given this"), "nothing before counting began: {never:?}");
}

// ---- a-memory-says-when-it-may-be-out-of-date.test.tsx (A1.3) ----
fn retries() -> Value {
    let mut m = kept("mem_r", "Retries live in src/net.ts.");
    m["createdAt"] = json!("2026-09-20T12:00:00.000Z");
    m
}

#[test]
fn says_which_file_under_the_memory() {
    let shown = draw(|s| {
        s.people_panel.memories = json!({"memories":[retries()]});
        s.people_panel.changed_since.insert("mem_r".into(), vec!["src/net.ts".into()]);
    });
    assert!(has(&shown, "May be out of date: src/net.ts changed since."), "{shown:?}");
}

#[test]
fn says_nothing_when_nothing_it_names_changed() {
    let shown = draw(|s| s.people_panel.memories = json!({"memories":[retries()]}));
    assert!(!has(&shown, "May be out of date"), "{shown:?}");
}

#[test]
fn the_screen_checks_the_open_folder_once_when_it_loads() {
    // What reload() fills changed_since from: the folder's own file, saved after the memory was written.
    let root = std::env::temp_dir().join(format!("locust-memory-screen-{}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/net.ts"), "export {}").unwrap();
    let found = changed_in_folder(&json!({"memories":[retries()]}), root.to_str());
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(found.get("mem_r"), Some(&vec!["src/net.ts".to_owned()]));
}

// ---- a-rewrite-shows-its-writer.test.tsx (A1.6) ----
fn rewritten() -> Value {
    json!({"memoryId":"mem_1","text":"Deploys go out on Thursdays.","previousText":"Deploys go out on Fridays.","scope":"workspace","workspaceId":"ws_shop","workspaceName":"shop","by":{"name":"you"},"updatedBy":{"teammateId":"tm_booty","name":"Booty"},"updatedAt":"2026-09-24T05:00:00.000Z","missionId":"mission_2","createdAt":"2026-09-01T10:00:00.000Z","status":"kept","enabled":true})
}

#[test]
fn names_the_writer_of_the_words_shown_not_who_first_kept_it() {
    let shown = draw(|s| s.people_panel.memories = json!({"memories":[rewritten()]}));
    let meta: Vec<&String> = shown.iter().filter(|t| t.contains(" · shop · ")).collect();
    assert_eq!(meta.len(), 1, "{shown:?}");
    assert!(meta[0].starts_with("Booty · "), "{meta:?}");
    assert!(!shown.iter().any(|t| t.starts_with("you · ")), "{shown:?}");
}

#[test]
fn credits_the_rewrite_to_its_writer() {
    let turns = std::collections::HashSet::from(["mission_2".to_owned()]);
    let lines = memories_of_conversation(&[rewritten()], &turns);
    assert_eq!(lines.iter().map(|l| l.by.as_str()).collect::<Vec<_>>(), ["Booty"]);
    assert_eq!(memory_card_summary(&lines), "Booty updated 1 thing it already knew");
}

#[test]
fn names_every_writer_when_the_lines_are_more_than_one_persons() {
    let mut wren = rewritten();
    wren["memoryId"] = json!("mem_2");
    wren["text"] = json!("Tests run with pnpm test.");
    wren["by"] = json!({"name":"Wren"});
    let fields = wren.as_object_mut().unwrap();
    fields.remove("updatedBy");
    fields.remove("previousText");
    let turns = std::collections::HashSet::from(["mission_2".to_owned()]);
    let summary = memory_card_summary(&memories_of_conversation(&[wren, rewritten()], &turns));
    assert_eq!(summary, "Wren and Booty remembered 1 thing and updated 1 thing it already knew");
}

// ---- a-tidy-answered-leaves-no-banner.test.tsx (0.372) ----
fn tidied() -> MemoryChangedUpdate<'static> {
    MemoryChangedUpdate { by: "Wren", proposed_tidy: 2, ..Default::default() }
}
fn merge() -> Value {
    let mut m = kept("mem_m", "Deploys go out every Thursday.");
    m["status"] = json!("proposed");
    m["merges"] = json!(["mem_a", "mem_b"]);
    m["by"] = json!({"name":"Wren"});
    m["missionId"] = json!("mission_tidy");
    m["basis"] = json!("0123456789abcdef");
    m
}
fn tidy_screen(with_merge: bool, waits: bool) -> Vec<String> {
    let mut rows = vec![kept("mem_a", "Deploys go out on Thursdays."), kept("mem_b", "We deploy every Thursday.")];
    if with_merge {
        rows.push(merge());
    }
    draw(|s| {
        s.people_panel.memories = json!({"memories":rows});
        s.people_panel.memory_notice = memory_changed_notice(&tidied());
        s.people_panel.memory_notice_waits = waits;
    })
}

#[test]
fn points_at_what_is_waiting_and_says_so_of_itself() {
    assert_eq!(memory_changed_notice(&tidied()).as_deref(), Some("Wren suggested 2 changes to memory, waiting below."));
    assert!(notice_waits(&tidied()));
    assert!(notice_waits(&MemoryChangedUpdate { by: "Wren", proposed: &["x"], ..Default::default() }));
    assert!(notice_waits(&MemoryChangedUpdate { by: "Wren", proposed_forgets: &["x"], ..Default::default() }));
    assert!(!notice_waits(&MemoryChangedUpdate { by: "Wren", kept: &["x"], forgotten: &["y"], ..Default::default() }));
}

#[test]
fn is_shown_while_something_is_still_waiting() {
    assert!(has(&tidy_screen(true, true), "Wren suggested 2 changes to memory, waiting below."));
}

#[test]
fn is_gone_once_nothing_is_but_a_notice_about_what_already_happened_stays_until_dismissed() {
    assert!(!has(&tidy_screen(false, true), "waiting below"));
    assert!(has(&tidy_screen(false, false), "Wren suggested 2 changes to memory"));
}

#[test]
fn puts_what_is_waiting_for_an_answer_above_how_memory_works() {
    let shown = tidy_screen(true, true);
    let waiting = shown.iter().position(|t| t == "Waiting for you").expect("a Waiting for you heading");
    let how = shown.iter().position(|t| t == "How memory is kept").expect("a How memory is kept heading");
    assert!(waiting < how, "{shown:?}");
}

#[test]
fn the_screen_reads_what_changed_when_it_loads() {
    // reload() itself: the store's memory, checked against the open folder's files.
    let root = std::env::temp_dir().join(format!("locust-memory-reload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop/src")).unwrap();
    std::fs::write(root.join("shop/src/net.ts"), "export {}").unwrap();
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    store.save_json("memories.json", &json!({"schemaVersion": 1, "memories": [retries()]})).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    s.home.folder_path = Some(root.join("shop").to_string_lossy().into_owned());
    reload(&mut s);
    let found = s.people_panel.changed_since.get("mem_r").cloned();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(found, Some(vec!["src/net.ts".to_owned()]));
}

/// memory-reader.ts's `memory-changed`, through the engine: what a finished reply did to memory reaches the Memory
/// screen as its notice, the newest one saying it, and whether it points at something waiting.
#[test]
fn a_finished_replys_memory_change_becomes_the_memory_screens_notice() {
    let mut s = State::default();
    locust_core::native::take_memory_changes();
    locust_core::native::push_memory_changed(locust_core::people::MemoryChanged { by: "Wren".into(), kept: vec!["Old news.".into()], ..Default::default() });
    locust_core::native::push_memory_changed(locust_core::people::MemoryChanged {
        by: "Yurt".into(),
        proposed_changes: vec!["Deploys go out on Thursdays.".into()],
        proposed_forgets: vec!["The API is on port 3000.".into()],
        ..Default::default()
    });
    // Nothing to say is never queued.
    locust_core::native::push_memory_changed(locust_core::people::MemoryChanged { by: "Nobody".into(), ..Default::default() });
    assert!(crate::people_panel::poll(&mut s));
    let notice = s.people_panel.memory_notice.clone().unwrap();
    assert!(notice.starts_with("Yurt wants to change a memory to \"Deploys go out on Thursdays.\""), "{notice}");
    assert!(notice.contains("Yurt wants to forget \"The API is on port 3000.\""), "{notice}");
    assert!(s.people_panel.memory_notice_waits);
    assert!(locust_core::native::take_memory_changes().is_empty(), "the window took them all");
}

// ---- MemoryScreen.tsx "Forget" and "Recently forgotten" (A1.8): Forget everything asks once, and what it forgot is held
// for 7 days, each with who forgot it, the day, and where it applied ----
struct GoneRoot(std::path::PathBuf);
impl Drop for GoneRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..20 {
        rt.frame(0.05);
    }
}

fn press(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn screen_texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

#[test]
fn forget_everything_for_this_folder_asks_first_then_holds_what_it_forgot() {
    let root = std::env::temp_dir().join(format!("locust-memory-forget-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("shop");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let _gone = GoneRoot(root.clone());
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let ws = people::workspace_id(&folder);
    people::add_memory(&store, "Deploys go out on Thursdays.", None, "workspace", &ws, json!({"name": "you"}), "kept").unwrap();
    people::add_memory(&store, "Diffs, not prose.", None, "global", &ws, json!({"name": "you"}), "kept").unwrap();
    let mut s = State::default();
    s.people_panel.open = true;
    s.people_panel.tab = 1;
    s.people_panel.memory_tab = true;
    s.people_panel.memory_mode = "auto".into();
    s.home.folder_path = Some(folder.to_string_lossy().into_owned());
    s.people_panel.memories = people::memories(&store).unwrap();
    s.data = Some(store.clone());
    let mut rt = Runtime::new(s, |s, _| memory_screen(s, &Palette::new(true)), (900.0, 3000.0), Env::new(true, 1.0));
    press(&mut rt, "memory-forget-folder");
    // Asked first, and honest that it can come back.
    assert!(has(&screen_texts(&rt), "Everything remembered for shop, forgotten. You can restore them from Recently forgotten for 7 days."), "{:?}", screen_texts(&rt));
    assert_eq!(people::memories(&store).unwrap()["memories"].as_array().unwrap().len(), 2, "nothing forgotten on the first press");
    press(&mut rt, "memory-forget-cancel");
    assert!(rt.find("memory-forget-confirm").is_none());
    press(&mut rt, "memory-forget-folder");
    press(&mut rt, "memory-forget-yes");
    let file = people::memories(&store).unwrap();
    let left: Vec<&str> = file["memories"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap()).collect();
    assert_eq!(left, ["Diffs, not prose."], "everywhere's stays");
    let shown = screen_texts(&rt);
    assert!(has(&shown, "Recently forgotten"), "{shown:?}");
    assert!(has(&shown, "Deploys go out on Thursdays."), "{shown:?}");
    let today = chrono::Local::now().format("%b %-d").to_string();
    assert!(has(&shown, &format!("forgotten by you · {today} · ")), "{shown:?}");
    assert!(has(&shown, "Kept for 7 days, then gone for good. Teammates do not read these."), "{shown:?}");
    // And it comes back.
    let id = file["forgotten"][0]["memory"]["memoryId"].as_str().unwrap().to_owned();
    press(&mut rt, &format!("memory-restore-{id}"));
    assert_eq!(people::memories(&store).unwrap()["memories"].as_array().unwrap().len(), 2);
}

#[test]
fn forget_everything_says_every_folder_and_offers_no_folder_button_without_a_folder() {
    let shown = draw(|s| {
        s.home.folder_path = None;
        s.people_panel.memories = json!({"memories": [kept("mem_a", "One.")]});
    });
    assert!(has(&shown, "Forget everything"), "{shown:?}");
    assert!(!has(&shown, "Forget everything for this folder"), "{shown:?}");
    let asked = draw(|s| {
        s.people_panel.memories = json!({"memories": [kept("mem_a", "One.")]});
        s.people_panel.confirming_clear = Some("all");
    });
    assert!(has(&asked, "Every memory, every folder, forgotten. You can restore them from Recently forgotten for 7 days."), "{asked:?}");
    // Nothing remembered: nothing to forget, and no card for it.
    let empty = draw(|s| s.people_panel.memories = json!({"memories": []}));
    assert!(!has(&empty, "Forget everything"), "{empty:?}");
}

#[test]
fn a_forgotten_memory_says_who_forgot_it_when_and_where() {
    let held = |scope: &str| json!({"memory": {"memoryId": "mem_x", "text": "Old.", "scope": scope, "workspaceName": "shop", "status": "kept"}, "forgottenAt": "2026-11-03T15:00:00.000Z", "forgottenBy": {"name": "Booty", "teammateId": "tm_booty"}});
    let day = chrono::DateTime::parse_from_rfc3339("2026-11-03T15:00:00.000Z").unwrap().with_timezone(&chrono::Local).format("%b %-d").to_string();
    assert_eq!(forgotten_meta(&held("workspace")), format!("forgotten by Booty · {day} · shop"));
    assert_eq!(forgotten_meta(&held("global")), format!("forgotten by Booty · {day} · everywhere"));
}
