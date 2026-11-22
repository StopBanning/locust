// Carries apps/desktop/src/main/a-resumed-session-is-told-only-what-changed.test.ts (A2.5): a CLI session is briefed
// once, then told what changed. The mission-service case is in native/codex_mission_tests.rs.
use crate::brief_sessions::*;
use crate::people::{add_memory, turn_brief_held, workspace_id};
use crate::store::Store;
use crate::workroom::{Message, Party};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;

const ROSTER_RULE: &str = "Writing a teammate's name in your reply does NOT reach them.";
const MEMORY_RULE: &str = "These are notes your team wrote earlier";
const STILL_HOLDS: &str = "You were given standing instructions earlier in this conversation";

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn fixture(team: &[(&str, &str, &str)], memories: &[&str]) -> (Temp, Store, PathBuf) {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("locust-brief-sessions-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let teammates: Vec<Value> = team.iter().map(|(id, name, role)| json!({"teammateId": id, "name": name, "role": role})).collect();
    store.save_json("teammates.json", &json!({"schemaVersion":1,"teammates":teammates,"missionOwners":{},"settings":{}})).unwrap();
    let folder = root.join("shop");
    for text in memories {
        add_memory(&store, text, None, "workspace", &workspace_id(&folder), json!({"teammateId":"tm_wren","name":"Wren"}), "kept").unwrap();
    }
    (Temp(root), store, folder)
}
const WREN: (&str, &str, &str) = ("tm_wren", "Wren", "Code & Migrations");
const ATLAS: (&str, &str, &str) = ("tm_atlas", "Atlas", "Research & Briefs");
const NOVA: (&str, &str, &str) = ("tm_nova", "Nova", "Docs & QA");

fn brief(store: &Store, folder: &std::path::Path, who: &str, prompt: &str, inbound: &[Message], held: Option<&HashSet<String>>) -> (String, Vec<String>) {
    let (prompt, _, given) = turn_brief_held(store, who, folder, prompt, None, None, Some("codex"), inbound, 0, None, None, held).unwrap();
    (prompt, given)
}
fn waiting(text: &str) -> Message {
    Message {
        message_id: "wm_1".into(),
        sequence: 1,
        from: Party { teammate_id: "tm_atlas".into(), name: "Atlas".into() },
        from_mission: "mission_a".into(),
        to: Party { teammate_id: "tm_wren".into(), name: "Wren".into() },
        text: text.into(),
        posted_at: "2026-09-24T15:00:00.000Z".into(),
        observed: None,
    }
}
fn first_brief() -> (Temp, Store, PathBuf, String, Vec<String>) {
    let (t, store, folder) = fixture(&[WREN, ATLAS], &["The build is pnpm check."]);
    let (prompt, given) = brief(&store, &folder, "tm_wren", "Look around.", &[], None);
    (t, store, folder, prompt, given)
}

#[test]
fn is_the_whole_brief_on_the_first_turn_and_says_which_paragraphs_it_stands_on() {
    let (_t, _store, _folder, first, given) = first_brief();
    assert!(first.contains(ROSTER_RULE));
    assert!(first.contains(MEMORY_RULE));
    assert!(!first.contains(STILL_HOLDS));
    assert!(given.len() > 5, "{given:?}");
    // Keys of the paragraphs themselves: the same words give the same key.
    assert!(given.contains(&paragraph_key(first.split("\n\n").next().unwrap())));
}

#[test]
fn is_only_what_the_session_was_not_given_on_a_later_turn_after_one_line_that_says_the_rest_holds() {
    let (_t, store, folder, first, given) = first_brief();
    let held: HashSet<String> = given.iter().cloned().collect();
    let (second, again) = brief(&store, &folder, "tm_wren", "And the tests?", &[waiting("pnpm check also lints.")], Some(&held));
    assert!(second.starts_with(&format!("You are Wren; your teammates here are Atlas (Research & Briefs). {STILL_HOLDS}")), "{second}");
    // The tags are named, so a runtime that quietly started fresh can still answer.
    for tag in ["<locust-share to=\"Name\">", "<locust-ask>", "<locust-file>", "<locust-memory>"] {
        assert!(second.contains(tag), "{tag}: {second}");
    }
    assert!(!second.contains(ROSTER_RULE));
    assert!(!second.contains(MEMORY_RULE));
    assert!(second.contains("pnpm check also lints."));
    assert!(second.ends_with("And the tests?"));
    assert!(second.len() < first.len() / 4, "{} vs {}", second.len(), first.len());
    assert_eq!(again, given);
}

#[test]
fn sends_again_the_paragraph_that_changed_and_only_that_one() {
    let (_t, store, folder, _first, given) = first_brief();
    let held: HashSet<String> = given.iter().cloned().collect();
    // A memory was added: its listing moves, the rules for memory do not.
    add_memory(&store, "Releases go out on Fridays.", None, "workspace", &workspace_id(&folder), json!({"name":"Wren"}), "kept").unwrap();
    let (moved, _) = brief(&store, &folder, "tm_wren", "Go on.", &[], Some(&held));
    assert!(moved.contains("Releases go out on Fridays."));
    assert!(moved.contains(MEMORY_LIST_REPLACES), "a list sent again says it replaces the one held");
    assert!(!moved.contains(MEMORY_RULE));
    assert!(!moved.contains(ROSTER_RULE));
    // A teammate joined: the roster is sent again, with its rules.
    let (_t2, store2, folder2) = fixture(&[WREN, ATLAS, NOVA], &["The build is pnpm check."]);
    let (joined, _) = brief(&store2, &folder2, "tm_wren", "Go on.", &[], Some(&held));
    assert!(joined.contains("Atlas (Research & Briefs), Nova (Docs & QA)."), "{joined}");
    assert!(!joined.contains(MEMORY_RULE));
}

#[test]
fn is_the_whole_brief_word_for_word_when_the_session_holds_none_of_it() {
    let (_t, store, folder, first, _) = first_brief();
    let (cold, _) = brief(&store, &folder, "tm_wren", "Look around.", &[], Some(&HashSet::new()));
    assert_eq!(cold, first);
}

#[test]
fn works_the_same_for_a_run_that_belongs_to_nobody() {
    let (_t, store, folder) = fixture(&[WREN, ATLAS], &["The build is pnpm check."]);
    let (_, given) = brief(&store, &folder, "", "Look around.", &[], None);
    let held: HashSet<String> = given.into_iter().collect();
    let (later, _) = brief(&store, &folder, "", "More?", &[], Some(&held));
    assert!(later.starts_with(STILL_HOLDS), "{later}");
    assert!(!later.contains("locust-share"));
    assert!(later.contains("<locust-ask>"));
    assert!(!later.contains(MEMORY_RULE));
    assert!(later.ends_with("More?"));
}

fn record() -> BriefRecord {
    BriefRecord { given: vec!["aaaaaaaaaaaaaaaa".into()], turns: 0 }
}

#[test]
fn is_trusted_only_for_a_turn_that_resumes_a_session_that_did_not_compact() {
    let r = record();
    assert_eq!(brief_plan(false, false, Some(&r)).already_given, None);
    assert_eq!(brief_plan(true, true, Some(&r)).already_given, None);
    assert_eq!(brief_plan(true, false, None).already_given, None);
    assert_eq!(brief_plan(true, false, Some(&r)), BriefPlan { already_given: Some(r.given.iter().cloned().collect()), turns: 1 });
}

#[test]
fn is_set_aside_every_few_turns_regardless_so_the_whole_brief_comes_back() {
    let near = BriefRecord { turns: FULL_BRIEF_EVERY - 2, ..record() };
    assert!(brief_plan(true, false, Some(&near)).already_given.is_some());
    let due = BriefRecord { turns: FULL_BRIEF_EVERY - 1, ..record() };
    assert_eq!(brief_plan(true, false, Some(&due)), BriefPlan { already_given: None, turns: 0 });
}

#[test]
fn keeps_what_the_session_held_with_this_briefs_paragraphs_newest() {
    let held: HashSet<String> = ["a", "b"].map(String::from).into();
    assert_eq!(brief_held(Some(&held), &["b".into(), "c".into()]), ["a", "b", "c"]);
    assert_eq!(brief_held(None, &["c".into()]), ["c"]);
}

#[test]
fn reads_a_compaction_from_any_runtime_that_reports_one() {
    let diagnostic = |code: &str| json!({"type":"adapter.diagnostic","payload":{"code":code,"level":"info","message":"x","terminal":false}});
    assert!(compacted_during(&[diagnostic("opencode.context_compacted")]));
    assert!(compacted_during(&[diagnostic("claude.context_compacted")]));
    assert!(!compacted_during(&[diagnostic("opencode.unknown_event")]));
}

#[test]
fn is_read_back_by_the_next_start_of_the_app() {
    let (_t, store, _) = fixture(&[], &[]);
    let rec = BriefRecord { given: vec!["0123456789abcdef".into()], turns: 2 };
    BriefSessions::for_store(&store).record("mission_1", &rec).unwrap();
    let again = Store::new(store.root().to_path_buf(), store.electron().to_path_buf()).unwrap();
    assert_eq!(BriefSessions::for_store(&again).after("mission_1"), Some(rec));
    assert_eq!(BriefSessions::for_store(&again).after("mission_2"), None);
}

#[test]
fn is_no_record_at_all_when_it_cannot_be_read_so_the_turn_is_briefed_in_full() {
    let (_t, store, _) = fixture(&[], &[]);
    let path = store.root().join(BRIEF_SESSIONS_FILE);
    std::fs::write(&path, "{ not json").unwrap();
    assert_eq!(BriefSessions::for_store(&store).after("mission_1"), None);
    std::fs::write(&path, json!({"version":1,"sessions":{"mission_1":{"given":["not a key"],"turns":0,"at":"2026-09-24T15:00:00.000Z"}}}).to_string()).unwrap();
    assert_eq!(BriefSessions::for_store(&store).after("mission_1"), None);
}

#[test]
fn a_brief_shed_for_length_is_recorded_as_told_only_what_went() {
    // Electron composeSoloPrompt: over the cap the memory gives way, and the session is not recorded as holding it,
    // so a later resumed turn sends it again.
    use crate::briefing::{MAX_RUNTIME_PROMPT_LENGTH, assemble_held};
    use crate::people::{MEMORY_HEADING, MEMORY_RULES};
    let listing = format!("{MEMORY_HEADING} What is remembered:\n- a note");
    let rules = format!("{MEMORY_RULES} each with when it was written.");
    let standing = vec![format!("The folder has a LOCUST.md.\n\n{listing}\n\n{rules}")];
    let (prompt, _, given) = assemble_held(&standing, &[], 0, &[], None, &"x".repeat(MAX_RUNTIME_PROMPT_LENGTH - 60), None, None, None, None);
    assert!(!prompt.contains(MEMORY_HEADING));
    assert_eq!(given, [paragraph_key("The folder has a LOCUST.md.")]);
}
