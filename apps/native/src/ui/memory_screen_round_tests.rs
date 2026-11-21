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
