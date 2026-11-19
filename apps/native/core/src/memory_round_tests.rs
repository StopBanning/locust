// Memory round 1 (19 Nov 2026): the Electron memory cases under apps/desktop/src/main, one Rust test per case, named
// for the case's own words. Each block names the Electron file it carries.
use crate::people::{ForgetRefusal, add_memory, forget_target, memories, record_reply, workspace_id};
use crate::store::Store;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// A store and a project folder, both in a fresh temp folder.
fn fixture() -> (Temp, Store, PathBuf) {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("locust-memory-round-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let folder = root.join("shop");
    (Temp(root), store, folder)
}
fn rows(store: &Store) -> Vec<Value> {
    memories(store).unwrap()["memories"].as_array().cloned().unwrap_or_default()
}

// ---- a-forget-with-nothing-remembered-says-nothing.test.ts (0.785) ----
const FORGET_REPLY: &str = "Done.\n<locust-memory>\nforget :: laptop-setup.md, the checklist the person undid\n</locust-memory>";

#[test]
fn is_answered_as_nothing_remembered_by_the_store() {
    let (_t, store, folder) = fixture();
    assert_eq!(forget_target(&rows(&store), &workspace_id(&folder), "anything at all"), Err(ForgetRefusal::NothingRemembered));
    // A memory in another folder is not this one's to forget, so here is still nothing remembered.
    add_memory(&store, "Elsewhere only.", None, "workspace", "ws_other", json!({"name":"you"}), "kept").unwrap();
    assert_eq!(forget_target(&rows(&store), &workspace_id(&folder), "anything at all"), Err(ForgetRefusal::NothingRemembered));
}

#[test]
fn says_nothing_in_the_thread() {
    let (_t, store, folder) = fixture();
    assert_eq!(record_reply(&store, "unknown", &folder, FORGET_REPLY), Ok(vec![]));
}

#[test]
fn still_says_a_miss_when_something_is_remembered_opening_with_a_capital() {
    let (_t, store, folder) = fixture();
    let ws = workspace_id(&folder);
    add_memory(&store, "Tests run with pnpm test.", None, "workspace", &ws, json!({"name":"you"}), "kept").unwrap();
    assert_eq!(forget_target(&rows(&store), &ws, "anything at all"), Err(ForgetRefusal::NothingMatched));
    // A miss no longer stops the lines after it: the remember below the forget is still kept.
    let reply = format!("{}\n<locust-memory>\nremember :: The port is 3001.\n</locust-memory>", FORGET_REPLY);
    let said = record_reply(&store, "unknown", &folder, &reply).unwrap();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("A conversation tried to forget something and could not."), "{}", said[0]);
    assert!(said[0].contains("It is still remembered."));
    assert!(rows(&store).iter().any(|m| m["text"] == "The port is 3001."), "the later line was still read");
}

// ---- a-new-project-is-told-how-to-write-its-first-memory.test.ts (0.810) ----
use crate::briefing::{MAX_RUNTIME_PROMPT_LENGTH, assemble, standing_formats};
use crate::people::{MEMORY_HEADING, MEMORY_RULES, MEMORY_WRITE, memory_section};

fn section(rows: &[Value]) -> String {
    memory_section(Some("Wren"), Some("shop"), rows, false, false, chrono::Utc::now(), None, None)
}
fn one_kept() -> Vec<Value> {
    vec![json!({"text":"Port is 3001.","scope":"workspace","by":{"name":"you"},"status":"kept","enabled":true})]
}

#[test]
fn says_nothing_is_remembered_and_how_to_write_the_first_memory_block_and_all() {
    let empty = section(&[]);
    assert!(empty.contains(&format!("{MEMORY_HEADING} What is remembered for the folder \"shop\" and everywhere:\nNothing is remembered yet.")), "{empty}");
    assert!(empty.contains(MEMORY_WRITE));
    assert!(empty.contains("<locust-memory>"));
    assert!(empty.contains("kept at once"));
}

#[test]
fn leaves_out_how_to_read_memories_until_there_is_one_and_brings_it_back_with_the_first() {
    let (empty, one) = (section(&[]), section(&one_kept()));
    for reading in [MEMORY_RULES, "colleague's notes", "the file wins"] {
        assert!(!empty.contains(reading), "{reading}");
        assert!(one.contains(reading), "{reading}");
    }
    assert!(empty.encode_utf16().count() < 1_800, "{}", empty.len());
    assert!(one.encode_utf16().count() - empty.encode_utf16().count() > 1_000);
}

#[test]
fn still_gives_the_memory_paragraphs_up_first_when_a_run_that_belongs_to_nobody_is_over_the_cap() {
    let mut standing = vec![section(&[])];
    standing.extend(standing_formats(None, false));
    let (over, _) = assemble(&standing, &[], 0, &[], None, &"x".repeat(MAX_RUNTIME_PROMPT_LENGTH));
    assert!(!over.contains(MEMORY_HEADING));
    assert!(!over.contains(MEMORY_WRITE));
    // The reply formats are not the part to lose.
    assert!(over.contains("To give the person a file"));
}

// ---- a-run-that-is-nobodys-still-knows-the-project.test.ts ----
// Electron's first case also checks that the memory slot is told the previous mission; the Rust brief has no such
// hint to pass (nothing in it reads one), so that half is not carried.
#[test]
fn is_briefed_with_the_folder_and_the_project_memory_and_with_no_roster() {
    let (_t, store, folder) = fixture();
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"Code"},{"teammateId":"tm_atlas","name":"Atlas","role":"Research"}],"missionOwners":{},"settings":{}}).to_string(),
    )
    .unwrap();
    add_memory(&store, "The secret word is PELICAN.", None, "workspace", &workspace_id(&folder), json!({"name":"you"}), "kept").unwrap();
    let (briefed, _) = crate::people::turn_brief(&store, "", &folder, "What is the secret word?", None, Some("opencode"), &[], 0).unwrap();
    assert!(briefed.contains("The secret word is PELICAN."));
    assert!(briefed.contains("You are working in the folder"), "the folder is the project's, so it goes");
    assert!(briefed.ends_with("What is the secret word?"));
    assert!(!briefed.contains("Teammates in this workspace besides you"));
    assert!(!briefed.contains("locust-share"));
}

#[test]
fn runs_on_the_persons_words_alone_when_there_is_no_memory_to_read() {
    let (_t, store, folder) = fixture();
    std::fs::write(store.electron().join("memories.json"), "{ not json").unwrap();
    let (briefed, _) = crate::people::turn_brief(&store, "", &folder, "Read the README.", None, Some("opencode"), &[], 0)
        .expect("a briefing that cannot be read is never a refusal to run");
    assert!(briefed.ends_with("Read the README."));
    assert!(!briefed.contains(MEMORY_HEADING));
}

#[test]
fn keeps_the_persons_words_when_the_briefing_would_not_fit() {
    let (composed, _) = assemble(&["x".repeat(13_000)], &[], 0, &[], None, "Two plus two?");
    assert!(composed.ends_with("Two plus two?"));
}

#[test]
fn sheds_the_team_memory_first_when_it_would_not_fit_and_keeps_locust_md_and_the_formats() {
    let project = "The folder \"shop\" has a LOCUST.md: always run pnpm check before you finish.".to_owned();
    let listing = format!("{MEMORY_HEADING} What is remembered for the folder \"shop\" and everywhere:\n{}", "- a long remembered note\n".repeat(600));
    let rules = format!("{MEMORY_RULES} each with when it was written. Use them as notes.");
    let mut standing = vec![[project, listing, rules].join("\n\n")];
    standing.extend(standing_formats(None, false));
    let (composed, _) = assemble(&standing, &[], 0, &[], None, "Fix the test.");
    assert!(composed.contains("always run pnpm check before you finish"));
    assert!(!composed.contains(MEMORY_HEADING));
    assert!(!composed.contains(MEMORY_RULES));
    assert!(composed.contains("<locust-ask>"));
    assert!(composed.ends_with("Fix the test."));
    assert!(composed.encode_utf16().count() < 12_000);
}

// ---- a-memory-whose-file-changed-may-be-out-of-date.test.ts (A1.3) ----
use crate::memory_file::{MEMORY_FILE_IN_FOLDER, MEMORY_OFF_TEXT, changed_since, cited_paths, memory_file_text, out_of_date, retire_memory_file};

fn set_saved(path: &Path, at: chrono::DateTime<chrono::Utc>) {
    std::fs::File::options().write(true).open(path).unwrap().set_modified(at.into()).unwrap();
}
fn at(text: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(text).unwrap().into()
}

#[test]
fn takes_paths_and_project_file_names_not_urls_versions_or_abbreviations() {
    assert_eq!(cited_paths("The retry logic lives in src/net/fetch.ts, see also `README.md` and .env."), ["src/net/fetch.ts", "README.md", ".env"]);
    assert!(cited_paths("Docs are at https://example.com/a.md; we pin v1.2.3, e.g. for tests.").is_empty());
    assert!(cited_paths("Tests run with pnpm test.").is_empty());
    assert_eq!(cited_paths("see C:\\proj\\src\\net.ts for the fix"), ["C:\\proj\\src\\net.ts"]);
    assert!(cited_paths("open mailto:someone or file:x").is_empty());
    // A scheme in front of what would otherwise read as a path is still a link, not a file in this folder.
    assert!(cited_paths("see file:src/net.ts or vscode:apps/desktop/main.ts").is_empty());
    assert_eq!(cited_paths("Windows paths too: apps\\desktop\\src\\main.ts"), ["apps\\desktop\\src\\main.ts"]);
}

#[test]
fn names_them_in_a_sentence() {
    assert_eq!(out_of_date(&["src/net.ts".into()]), "src/net.ts changed since");
    assert_eq!(out_of_date(&["a.ts".into(), "b.ts".into(), "c.ts".into()]), "a.ts, b.ts and c.ts changed since");
}

#[test]
fn a_named_file_saved_after_the_memory_is_listed_one_saved_before_missing_or_outside_the_folder_is_not() {
    let (t, _store, _) = fixture();
    let folder = t.0.join("project");
    std::fs::create_dir_all(folder.join("src")).unwrap();
    // A real, newer file just OUTSIDE the folder says nothing about this project, however recently it changed.
    for (path, body) in [(t.0.join("outside.ts"), "export {}"), (folder.join("src/net.ts"), "export {}"), (folder.join("README.md"), "# x")] {
        std::fs::write(path, body).unwrap();
    }
    let written = at("2026-09-20T12:00:00.000Z");
    set_saved(&folder.join("README.md"), written - chrono::Duration::hours(1));
    set_saved(&folder.join("src/net.ts"), written + chrono::Duration::hours(1));
    set_saved(&t.0.join("outside.ts"), written + chrono::Duration::hours(1));
    let text = "Retries live in src/net.ts; setup is in README.md; the plan is in docs/gone.md and ../outside.ts.";
    assert_eq!(changed_since(&folder, text, "2026-09-20T12:00:00.000Z"), ["src/net.ts"]);
}

#[test]
fn a_date_that_does_not_read_says_nothing() {
    assert!(changed_since(&std::env::temp_dir(), "src/net.ts", "not a date").is_empty());
}

fn stale_line() -> Value {
    json!({"memoryId":"mem_r","text":"Retries live in src/net.ts.","scope":"workspace","by":{"name":"you"},"createdAt":"2026-09-20T12:00:00.000Z","changedSince":["src/net.ts"],"status":"kept","enabled":true})
}

#[test]
fn the_brief_says_so_beside_the_memory() {
    let section = memory_section(None, Some("shop"), &[stale_line()], false, false, at("2026-09-24T12:00:00.000Z"), None, None);
    assert!(section.contains("- Retries live in src/net.ts. (this folder, by the person, 4 days ago; may be out of date: src/net.ts changed since)"), "{section}");
}

#[test]
fn and_so_does_the_file() {
    let file = memory_file_text(&[stale_line()], at("2026-09-24T12:00:00.000Z"));
    assert!(file.contains("- Retries live in src/net.ts. (by the person -- 2026-09-20 (4 days ago) -- may be out of date: src/net.ts changed since) [mem_r]"), "{file}");
}

#[test]
fn a_brief_marks_a_memory_whose_file_changed_and_names_the_memory_file_it_wrote() {
    // The whole path: the store, the folder's own file, the memory file written before the run, and the brief.
    let (_t, store, folder) = fixture();
    std::fs::create_dir_all(folder.join("src")).unwrap();
    std::fs::write(folder.join("src/net.ts"), "export {}").unwrap();
    add_memory(&store, "Retries live in src/net.ts.", None, "workspace", &workspace_id(&folder), json!({"name":"you"}), "kept").unwrap();
    set_saved(&folder.join("src/net.ts"), chrono::Utc::now() + chrono::Duration::hours(1));
    assert_eq!(crate::people::write_brief_memory_file(&store, &folder, None), Some(MEMORY_FILE_IN_FOLDER));
    let file = std::fs::read_to_string(folder.join(MEMORY_FILE_IN_FOLDER)).unwrap();
    assert!(file.contains("may be out of date: src/net.ts changed since"), "{file}");
    let briefed = crate::people::brief(&store, "", &folder, "Fix retries.").unwrap();
    assert!(briefed.contains("; may be out of date: src/net.ts changed since)"), "{briefed}");
}

// ---- a-rewrite-is-credited-to-its-writer.test.ts (A1.6, A1.7) ----
fn the_one(store: &Store) -> Value {
    rows(store).into_iter().find(|m| m["status"] == "kept").unwrap()
}

#[test]
fn a_teammates_rewrite_keep_and_tell_me_credited_to_the_teammate_by_stays_who_first_kept_it() {
    let (_t, store, _) = fixture();
    add_memory(&store, "Deploys go out on Fridays.", Some("deploy-day"), "workspace", "ws_shop", json!({"name":"you"}), "kept").unwrap();
    add_memory(&store, "Deploys go out on Thursdays.", Some("deploy-day"), "workspace", "ws_shop", json!({"teammateId":"tm_booty","name":"Booty"}), "kept").unwrap();
    let held = the_one(&store);
    assert_eq!(held["by"], json!({"name":"you"}));
    assert_eq!(held["updatedBy"], json!({"teammateId":"tm_booty","name":"Booty"}));
    let brief = memory_section(None, Some("shop"), &[held], false, false, chrono::Utc::now(), None, None);
    assert!(brief.contains("(this folder, by Booty"), "the brief names who wrote the words: {brief}");
}

#[test]
fn a_change_the_person_kept_keep_the_change_credited_to_the_teammate_who_proposed_it() {
    let (_t, store, _) = fixture();
    add_memory(&store, "Deploys go out on Fridays.", Some("deploy-day"), "workspace", "ws_shop", json!({"name":"you"}), "kept").unwrap();
    add_memory(&store, "Deploys go out on Thursdays.", Some("deploy-day"), "workspace", "ws_shop", json!({"teammateId":"tm_booty","name":"Booty"}), "proposed").unwrap();
    let proposal = rows(&store).into_iter().find(|m| m["status"] == "proposed").unwrap();
    crate::people::change_memory(&store, proposal["memoryId"].as_str().unwrap(), "keep", None).unwrap();
    let held = the_one(&store);
    assert_eq!(held["text"], "Deploys go out on Thursdays.");
    assert_eq!(held["updatedBy"], json!({"teammateId":"tm_booty","name":"Booty"}));
}

#[test]
fn the_persons_own_edit_and_put_it_back_credited_to_the_person() {
    let (_t, store, _) = fixture();
    add_memory(&store, "Deploys go out on Fridays.", None, "workspace", "ws_shop", json!({"teammateId":"tm_booty","name":"Booty"}), "kept").unwrap();
    let id = the_one(&store)["memoryId"].as_str().unwrap().to_owned();
    crate::people::change_memory(&store, &id, "edit", Some("Deploys go out on Mondays.")).unwrap();
    assert_eq!(the_one(&store)["updatedBy"], json!({"name":"you"}));
    // Switching it off writes no words, so credits nobody new.
    crate::people::change_memory(&store, &id, "toggle", None).unwrap();
    assert_eq!(the_one(&store)["updatedBy"], json!({"name":"you"}));
}

#[test]
fn a_writer_in_the_file_that_does_not_read_is_dropped_not_trusted() {
    let (_t, store, _) = fixture();
    let memory = json!({"memoryId":"mem_a","text":"Kept.","scope":"workspace","workspaceId":"ws_shop","workspaceName":"shop","by":{"name":"you"},"createdAt":"2026-09-24T12:00:00.000Z","status":"kept","enabled":true,"updatedBy":{"name":""}});
    std::fs::write(store.electron().join("memories.json"), json!({"schemaVersion":1,"memories":[memory]}).to_string()).unwrap();
    assert!(rows(&store)[0].get("updatedBy").is_none());
}

#[test]
fn rewrites_a_file_that_still_lists_memories_so_a_teammate_cannot_read_them_there() {
    let (_t, store, folder) = fixture();
    std::fs::create_dir_all(folder.join(".locust")).unwrap();
    std::fs::write(folder.join(MEMORY_FILE_IN_FOLDER), "# Team memory\n\n- The secret word is PELICAN. (by the person -- 2026-09-05)\n").unwrap();
    assert!(retire_memory_file(&folder));
    assert_eq!(std::fs::read_to_string(folder.join(MEMORY_FILE_IN_FOLDER)).unwrap(), MEMORY_OFF_TEXT);
    assert!(!MEMORY_OFF_TEXT.contains("PELICAN"));
    // And a run with memory off retires it on its way to the brief.
    std::fs::write(folder.join(MEMORY_FILE_IN_FOLDER), "- PELICAN").unwrap();
    std::fs::write(store.electron().join("teammates.json"), json!({"schemaVersion":1,"teammates":[],"missionOwners":{},"settings":{"memoryMode":"off"}}).to_string()).unwrap();
    assert_eq!(crate::people::write_brief_memory_file(&store, &folder, None), None);
    assert_eq!(std::fs::read_to_string(folder.join(MEMORY_FILE_IN_FOLDER)).unwrap(), MEMORY_OFF_TEXT);
}

#[test]
fn writes_nothing_in_a_folder_that_never_had_the_file_or_where_it_already_says_so() {
    let (_t, _store, folder) = fixture();
    assert!(!retire_memory_file(&folder));
    assert!(!folder.join(MEMORY_FILE_IN_FOLDER).exists());
    std::fs::create_dir_all(folder.join(".locust")).unwrap();
    std::fs::write(folder.join(MEMORY_FILE_IN_FOLDER), MEMORY_OFF_TEXT).unwrap();
    let before = std::fs::metadata(folder.join(MEMORY_FILE_IN_FOLDER)).unwrap().modified().unwrap();
    assert!(!retire_memory_file(&folder));
    assert_eq!(std::fs::metadata(folder.join(MEMORY_FILE_IN_FOLDER)).unwrap().modified().unwrap(), before);
}
