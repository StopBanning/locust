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
    assert!(said[0].0.starts_with("A conversation tried to forget something and could not."), "{}", said[0].0);
    assert!(said[0].0.contains("It is still remembered."));
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

// ---- what a brief was given (A1.4; the store half of a-memory-nobody-is-given-says-so, row 145) ----
#[test]
fn a_brief_notes_what_it_gave_once_a_day_and_starts_the_count() {
    let (_t, store, folder) = fixture();
    let ws = workspace_id(&folder);
    add_memory(&store, "Port is 3001.", None, "workspace", &ws, json!({"name":"you"}), "kept").unwrap();
    add_memory(&store, "Maybe the port.", None, "workspace", &ws, json!({"name":"Wren"}), "proposed").unwrap();
    crate::people::note_brief_given(&store, &folder, None, "What port?").unwrap();
    let file = memories(&store).unwrap();
    assert!(file["briefTrackingSince"].is_string(), "{file}");
    let given = rows(&store);
    let first = given.iter().find(|m| m["status"] == "kept").unwrap()["lastBriefedAt"].clone();
    assert!(first.is_string());
    assert!(given.iter().find(|m| m["status"] == "proposed").unwrap().get("lastBriefedAt").is_none(), "a proposal is not noted");
    // A second run the same day writes nothing new.
    crate::people::note_brief_given(&store, &folder, None, "What port?").unwrap();
    assert_eq!(rows(&store).iter().find(|m| m["status"] == "kept").unwrap()["lastBriefedAt"], first);
    // And a month without one says so, counted from the latest of the brief, the write and the start of counting.
    let old = json!({"createdAt":"2026-01-01T00:00:00.000Z","lastBriefedAt":"2026-02-01T00:00:00.000Z"});
    let now = at("2026-04-01T00:00:00.000Z");
    assert_eq!(crate::people::days_unused(&old, Some("2026-01-15T00:00:00.000Z"), now), Some(59));
    assert_eq!(crate::people::days_unused(&old, Some("2026-03-15T00:00:00.000Z"), now), None);
    assert_eq!(crate::people::days_unused(&old, None, now), None);
}

// ---- a-tidy-pass-only-suggests.test.ts (A1.2, 0.372) ----
use crate::memory_tidy::TidySuggestion;
use crate::people::{change_memory, propose_tidy_from, record_reply_for};

const SEED: [&str; 4] = ["Deploys go out on Thursdays.", "We deploy every Thursday.", "The API is on port 3000.", "Tests run with npm test."];
/// The four memories most cases start from, in `ws`, and their ids.
fn seeded(store: &Store, ws: &str) -> Vec<String> {
    for text in SEED {
        add_memory(store, text, None, "workspace", ws, json!({"name":"you"}), "kept").unwrap();
    }
    rows(store).iter().map(|m| m["memoryId"].as_str().unwrap().to_owned()).collect()
}
fn wren() -> Value {
    json!({"teammateId":"tm_wren","name":"Wren"})
}
fn waiting(store: &Store) -> Vec<Value> {
    rows(store).into_iter().filter(|m| m["status"] == "proposed").collect()
}
fn kept_texts(store: &Store) -> Vec<String> {
    rows(store).iter().filter(|m| m["status"] == "kept").map(|m| m["text"].as_str().unwrap().to_owned()).collect()
}
fn three(ids: &[String], rewrite: &str) -> Vec<TidySuggestion> {
    vec![
        TidySuggestion::Merge { ids: vec![ids[0].clone(), ids[1].clone()], text: "Deploys go out every Thursday.".into() },
        TidySuggestion::Retire { id: ids[2].clone(), reason: "the API moved to 3001".into() },
        TidySuggestion::Rewrite { id: ids[3].clone(), text: rewrite.into() },
    ]
}

#[test]
fn a_merge_a_retirement_and_a_rewrite_wait_beside_the_memories_which_stay_briefed() {
    let (_t, store, _) = fixture();
    let ids = seeded(&store, "ws_shop");
    assert_eq!(propose_tidy_from(&store, "ws_shop", wren(), Some("mission_tidy"), &three(&ids, "Tests run with pnpm test.")), Ok((3, vec![])));
    assert_eq!(kept_texts(&store), SEED);
    let shown: Vec<Value> = waiting(&store)
        .iter()
        .map(|m| json!([m.get("merges").or(m.get("forgets")).or(m.get("replaces")), m["text"], m.get("reason"), m["by"]["name"], m["missionId"]]))
        .collect();
    assert_eq!(
        shown,
        [
            json!([[ids[0], ids[1]], "Deploys go out every Thursday.", null, "Wren", "mission_tidy"]),
            json!([ids[2], SEED[2], "the API moved to 3001", "Wren", "mission_tidy"]),
            json!([ids[3], "Tests run with pnpm test.", null, "Wren", "mission_tidy"]),
        ]
    );
    assert!(waiting(&store).iter().all(|m| m["basis"].is_string()));
}

#[test]
fn refuses_what_it_cannot_stand_behind_and_says_why() {
    let (_t, store, _) = fixture();
    let ids = seeded(&store, "ws_shop");
    add_memory(&store, "Another project.", None, "workspace", "ws_other", json!({"name":"you"}), "kept").unwrap();
    add_memory(&store, "Colin reads on his phone.", None, "global", "ws_shop", json!({"name":"you"}), "kept").unwrap();
    let all = rows(&store);
    let id_of = |text: &str| all.iter().find(|m| m["text"] == text).unwrap()["memoryId"].as_str().unwrap().to_owned();
    let (proposed, refused) = propose_tidy_from(
        &store,
        "ws_shop",
        wren(),
        None,
        &[
            TidySuggestion::Retire { id: "mem_nothing".into(), reason: "x".into() },
            TidySuggestion::Rewrite { id: id_of("Another project."), text: "Not this folder.".into() },
            TidySuggestion::Merge { ids: vec![ids[0].clone(), id_of("Colin reads on his phone.")], text: "Two places.".into() },
        ],
    )
    .unwrap();
    assert_eq!(proposed, 0);
    assert_eq!(
        refused,
        [
            "A suggestion to forget a memory named one this folder does not keep.",
            "A suggestion to change a memory named one this folder does not keep.",
            "A suggested merge joined memories kept in different places -- this folder and everywhere.",
        ]
    );
}

#[test]
fn asked_twice_still_one_of_each_a_newer_rewrite_updates_the_one_waiting() {
    let (_t, store, _) = fixture();
    let ids = seeded(&store, "ws_shop");
    propose_tidy_from(&store, "ws_shop", wren(), None, &three(&ids, "Tests run with pnpm test.")).unwrap();
    propose_tidy_from(&store, "ws_shop", wren(), None, &three(&ids, "Tests run with pnpm vitest.")).unwrap();
    let held = waiting(&store);
    assert_eq!(held.len(), 3);
    assert_eq!(held.iter().find(|m| m["replaces"] == ids[3].as_str()).unwrap()["text"], "Tests run with pnpm vitest.");
}

fn merge_proposal(store: &Store, ids: &[String], text: &str) -> String {
    let merge = TidySuggestion::Merge { ids: vec![ids[0].clone(), ids[1].clone()], text: text.into() };
    propose_tidy_from(store, "ws_shop", wren(), Some("mission_tidy"), &[merge]).unwrap();
    waiting(store).into_iter().find(|m| m.get("merges").is_some()).unwrap()["memoryId"].as_str().unwrap().to_owned()
}

#[test]
fn merge_them_the_first_takes_the_words_the_rest_go_to_recently_forgotten_by_who_suggested_it() {
    let (_t, store, _) = fixture();
    let ids = seeded(&store, "ws_shop");
    let proposal = merge_proposal(&store, &ids, "Deploys go out every Thursday.");
    change_memory(&store, &proposal, "keep", None).unwrap();
    let file = memories(&store).unwrap();
    let merged = rows(&store).into_iter().find(|m| m["memoryId"] == ids[0].as_str()).unwrap();
    assert_eq!(merged["text"], "Deploys go out every Thursday.");
    assert_eq!(merged["previousText"], SEED[0]);
    assert_eq!(merged["updatedBy"], wren());
    assert_eq!(merged["missionId"], "mission_tidy");
    assert!(!rows(&store).iter().any(|m| m["memoryId"] == ids[1].as_str()));
    assert!(waiting(&store).is_empty());
    let bin: Vec<Value> = file["forgotten"].as_array().unwrap().iter().map(|e| json!([e["memory"]["memoryId"], e["forgottenBy"]["name"]])).collect();
    assert_eq!(bin, [json!([ids[1], "Wren"])]);
}

#[test]
fn keep_them_apart_only_the_suggestion_goes() {
    let (_t, store, _) = fixture();
    let ids = seeded(&store, "ws_shop");
    let proposal = merge_proposal(&store, &ids, "One.");
    change_memory(&store, &proposal, "remove", None).unwrap();
    assert_eq!(rows(&store).iter().map(|m| m["text"].as_str().unwrap()).collect::<Vec<_>>(), SEED);
}

#[test]
fn the_hash_gate_a_suggestion_about_words_that_changed_is_refused_when_kept_and_dropped() {
    let (_t, store, _) = fixture();
    let ids = seeded(&store, "ws_shop");
    let proposal = merge_proposal(&store, &ids, "Deploys go out every Thursday.");
    // The person corrects one of them after the suggestion was made.
    change_memory(&store, &ids[1], "edit", Some("We deploy every Wednesday now.")).unwrap();
    let refused = change_memory(&store, &proposal, "keep", None).unwrap_err();
    assert!(refused.starts_with("What this suggestion would change has changed since it was made"), "{refused}");
    let after = rows(&store);
    assert!(!after.iter().any(|m| m["memoryId"] == proposal.as_str()));
    assert_eq!(after.iter().find(|m| m["memoryId"] == ids[1].as_str()).unwrap()["text"], "We deploy every Wednesday now.");
    assert_eq!(after.iter().find(|m| m["memoryId"] == ids[0].as_str()).unwrap()["text"], SEED[0]);
}

#[test]
fn the_gate_covers_an_ask_me_first_rewrite_too_kept_after_the_person_edited_it_refused() {
    let (_t, store, _) = fixture();
    add_memory(&store, "Deploys go out on Fridays.", Some("deploy-day"), "workspace", "ws_shop", json!({"name":"you"}), "kept").unwrap();
    add_memory(&store, "Deploys go out on Thursdays.", Some("deploy-day"), "workspace", "ws_shop", wren(), "proposed").unwrap();
    let kept = rows(&store).into_iter().find(|m| m["status"] == "kept").unwrap()["memoryId"].as_str().unwrap().to_owned();
    let proposal = waiting(&store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&store, &kept, "edit", Some("Deploys go out on Mondays.")).unwrap();
    assert!(change_memory(&store, &proposal, "keep", None).is_err());
    assert_eq!(rows(&store).iter().map(|m| m["text"].as_str().unwrap()).collect::<Vec<_>>(), ["Deploys go out on Mondays."]);
}

/// A finished run of Wren's in `folder`: the reply read into a store seeded with the four memories.
fn read_reply(reply: impl Fn(&[String]) -> String, prompt: Option<&str>) -> (Temp, Store, Vec<String>, Vec<(String, &'static str)>) {
    let (t, store, folder) = fixture();
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren"}],"missionOwners":{"mission_tidy":"tm_wren"},"settings":{"memoryMode":"auto"}}).to_string(),
    )
    .unwrap();
    let ids = seeded(&store, &workspace_id(&folder));
    let said = record_reply_for(&store, "tm_wren", &folder, &reply(&ids), Some("mission_tidy"), prompt).unwrap();
    (t, store, ids, said)
}

#[test]
fn makes_a_tidy_reply_proposals_even_in_keep_and_tell_me_and_says_what_it_could_not_make() {
    let (_t, store, _, said) = read_reply(
        |ids| format!("Two say the same thing.\n<locust-tidy>\nmerge {} {} :: Deploys go out every Thursday.\nretire mem_gone :: stale\n</locust-tidy>", ids[0], ids[1]),
        None,
    );
    let held = waiting(&store);
    assert_eq!(held.len(), 1);
    assert_eq!((held[0]["by"]["name"].as_str(), held[0]["missionId"].as_str()), (Some("Wren"), Some("mission_tidy")));
    // What it could not make is said on the Memory screen's notice, not in the thread (memory-changed's tidyRefused).
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(kept_texts(&store), SEED);
}

#[test]
fn says_what_a_tidy_reply_proposed_and_could_not_make_on_the_memory_screens_notice() {
    let (_t, store, folder) = fixture();
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren"}],"missionOwners":{},"settings":{"memoryMode":"auto"}}).to_string(),
    )
    .unwrap();
    let ids = seeded(&store, &workspace_id(&folder));
    let reply = format!("<locust-tidy>\nmerge {} {} :: Deploys go out every Thursday.\nretire mem_gone :: stale\n</locust-tidy>", ids[0], ids[1]);
    let (_, changed) = crate::people::record_reply_changes(&store, "tm_wren", &folder, &reply, Some("mission_tidy"), None).unwrap();
    assert_eq!(changed.proposed_tidy, 1);
    assert_eq!(changed.tidy_refused, vec!["A suggestion to forget a memory named one this folder does not keep.".to_owned()]);
    assert_eq!(changed.by, "Wren");
}

#[test]
fn reads_a_block_in_a_code_fence_with_bulleted_and_numbered_lines() {
    let (_t, store, _, said) = read_reply(
        |ids| format!("Here are my suggestions:\n\n```\n<locust-tidy>\n- merge {} {} :: Deploys go out every Thursday.\n2. retire {} :: the API moved\n</locust-tidy>\n```", ids[0], ids[1], ids[2]),
        None,
    );
    assert_eq!(waiting(&store).len(), 2);
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn says_in_the_conversation_how_many_lines_it_could_not_read_and_still_proposes_the_rest() {
    let (_t, store, _, said) = read_reply(
        |ids| format!("Found some.\n<locust-tidy>\nmerge {} {} :: Deploys go out every Thursday.\nmerge: the two port notes -> one\nretire the old test command\n</locust-tidy>", ids[0], ids[1]),
        None,
    );
    assert_eq!(waiting(&store).len(), 1);
    assert_eq!(said, [("2 of Wren's suggestions were not written in a form Locust reads, so they were not put to you. Ask Wren to write them again, one per line.".to_owned(), "warning")]);
}

#[test]
fn says_so_even_when_nothing_at_all_could_be_read() {
    let (_t, _store, _, said) = read_reply(|_| "Here are my suggestions:\n<locust-tidy>\nmerge the deploy ones\n</locust-tidy>".into(), None);
    assert_eq!(said, [("One of Wren's suggestions was not written in a form Locust reads, so it was not put to you. Ask Wren to write it again, one per line.".to_owned(), "warning")]);
}

#[test]
fn says_quietly_when_a_tidy_pass_put_nothing_to_the_person_and_says_nothing_for_any_other_reply() {
    // The 0.372 drive: the block went into a shell command, and the reply was a summary.
    let summary = |_: &[String]| "Summary: two deploy-day memories should merge; the port memory is stale.".to_owned();
    let tidy = crate::memory_tidy::tidy_prompt();
    assert_eq!(read_reply(summary, Some(&tidy)).3, [("Nothing from this tidy pass waits on the Memory screen.".to_owned(), "info")]);
    assert!(read_reply(summary, Some("What does this repo do?")).3.is_empty());
    assert!(read_reply(summary, None).3.is_empty());
}

#[test]
fn says_nothing_about_the_briefs_own_example_repeated() {
    let (_t, store, _, said) = read_reply(
        |_| "Nothing needs tidying. The form was:\n<locust-tidy>\nmerge mem_a mem_b :: the one sentence that replaces them\n</locust-tidy>".into(),
        None,
    );
    assert!(said.is_empty(), "{said:?}");
    assert!(waiting(&store).is_empty());
}

#[test]
fn names_each_memory_by_its_id_so_a_tidy_pass_can() {
    let line = json!({"memoryId":"mem_abc","text":"Deploys go out on Thursdays.","scope":"workspace","by":{"name":"you"},"createdAt":"2026-09-24T12:00:00.000Z"});
    let text = memory_file_text(&[line], at("2026-09-24T12:00:00.000Z"));
    assert!(text.contains("- Deploys go out on Thursdays. (by the person -- 2026-09-24 (today)) [mem_abc]"), "{text}");
}

// ---- memory-recall.test.ts (0.454): the brief ranked by meaning. Recall itself (the WordPiece tokenizer, the
// all-MiniLM model through onnxruntime, its cache and its wait) needs an ONNX runtime this app does not have. ----
fn recall_notes() -> Vec<Value> {
    [
        ("The site deploys from the gh-pages branch; pushing there makes it live.", "2026-09-01T00:00:00.000Z"),
        ("The person likes short replies, no headers.", "2026-09-02T00:00:00.000Z"),
        ("Unit tests run with vitest from apps/desktop.", "2026-09-03T00:00:00.000Z"),
        ("The website hero has a rainbow border around the dashboard link.", "2026-09-04T00:00:00.000Z"),
    ]
    .iter()
    .map(|(text, at)| json!({"text": text, "scope": "workspace", "by": {"name": "Wren"}, "createdAt": at}))
    .collect()
}
const RECALL_QUERY: &str = "how do I publish the website?";

#[test]
fn pastes_the_closest_first_the_newer_first_on_a_tie() {
    let notes = recall_notes();
    let order: Vec<&str> = crate::people::briefed_memories_by(&notes, Some(RECALL_QUERY), None, Some(&[0.4, 0.1, 0.1, 0.2]))
        .iter()
        .map(|m| m["text"].as_str().unwrap())
        .collect();
    let text = |i: usize| notes[i]["text"].as_str().unwrap();
    assert_eq!(order, [text(0), text(3), text(2), text(1)]);
}

// case: is exactly the keyword brief when recall did not answer
#[test]
fn is_exactly_the_keyword_brief_when_recall_did_not_answer() {
    use crate::people::{briefed_memories, briefed_memories_by};
    let notes = recall_notes();
    assert_eq!(briefed_memories_by(&notes, Some(RECALL_QUERY), None, Some(&[0.4, 0.1])), briefed_memories(&notes, Some(RECALL_QUERY), None));
    assert_eq!(briefed_memories_by(&notes, None, None, Some(&[0.4, 0.1, 0.1, 0.2])), briefed_memories(&notes, None, None));
    // The keyword brief leads with the note that says "website".
    assert_eq!(briefed_memories(&notes, Some(RECALL_QUERY), None)[0]["text"], notes[3]["text"]);
}

// ---- a-memory-nobody-is-given-says-so.test.ts (A1.4) ----
// The brief pastes a few memories; the rest sit in the file. One that has gone a month without being in anyone's brief
// -- and without being written -- is marked, for the person and for a tidy pass. Never deleted for it.
use crate::people::{UNUSED_AFTER_DAYS, briefed_memories, days_unused, note_briefed};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
fn memories_bytes(store: &Store) -> Vec<u8> {
    std::fs::read(store.root().join("memories.json")).expect("memories.json is written")
}
fn ms_ago(at: &str) -> i64 {
    chrono::Utc::now().timestamp_millis() - chrono::DateTime::parse_from_rfc3339(at).unwrap().timestamp_millis()
}
/// Lay `lastBriefedAt` on the first memory, as a brief `hours` ago would have left it.
fn briefed_hours_ago(store: &Store, hours: i64) {
    let mut file = memories(store).unwrap();
    let at = chrono::Utc::now() - chrono::Duration::hours(hours);
    file["memories"][0]["lastBriefedAt"] = json!(at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
    store.save_json("memories.json", &file).unwrap();
}

// case: records it, and when counting began; at most once a day, so a day of runs is one write
#[test]
fn records_it_and_when_counting_began_at_most_once_a_day_so_a_day_of_runs_is_one_write() {
    let (_t, store, folder) = fixture();
    add_memory(&store, "Deploys go out on Thursdays.", None, "workspace", &workspace_id(&folder), json!({"name":"you"}), "kept").unwrap();
    assert!(memories(&store).unwrap().get("briefTrackingSince").is_none());
    let id = rows(&store)[0]["memoryId"].as_str().unwrap().to_owned();
    note_briefed(&store, std::slice::from_ref(&id)).unwrap();
    let file = memories(&store).unwrap();
    let since = file["briefTrackingSince"].as_str().unwrap().to_owned();
    assert!(ms_ago(&since) < 60_000, "counting began now: {since}");
    assert_eq!(rows(&store)[0]["lastBriefedAt"].as_str(), Some(since.as_str()));
    // Three hours later (a brief that recent), another run is no write at all.
    briefed_hours_ago(&store, 3);
    let written = memories_bytes(&store);
    note_briefed(&store, std::slice::from_ref(&id)).unwrap();
    assert_eq!(memories_bytes(&store), written);
    // A day on, it is noted again; when counting began stays.
    briefed_hours_ago(&store, 25);
    note_briefed(&store, std::slice::from_ref(&id)).unwrap();
    assert!(ms_ago(rows(&store)[0]["lastBriefedAt"].as_str().unwrap()) < 60_000);
    assert_eq!(memories(&store).unwrap()["briefTrackingSince"].as_str(), Some(since.as_str()));
}

// case: a proposal, or an id it does not keep, is not noted
#[test]
fn a_proposal_or_an_id_it_does_not_keep_is_not_noted() {
    let (_t, store, folder) = fixture();
    add_memory(&store, "Only proposed.", None, "workspace", &workspace_id(&folder), json!({"name":"Wren"}), "proposed").unwrap();
    let id = rows(&store)[0]["memoryId"].as_str().unwrap().to_owned();
    note_briefed(&store, &[id, "mem_nothing".into()]).unwrap();
    assert!(rows(&store)[0].get("lastBriefedAt").is_none());
}

// case: is counted from the last brief, the last write, or when counting began -- whichever is latest -- past ${String(UNUSED_AFTER_DAYS)} days
#[test]
fn is_counted_from_the_last_brief_the_last_write_or_when_counting_began_whichever_is_latest_past_30_days() {
    assert_eq!(UNUSED_AFTER_DAYS, 30);
    let now = at("2026-09-24T12:00:00.000Z");
    let ago = |days: i64| (now - chrono::Duration::milliseconds(days * DAY_MS)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    assert_eq!(days_unused(&json!({"createdAt": ago(90)}), Some(&ago(40)), now), Some(40));
    assert_eq!(days_unused(&json!({"createdAt": ago(90), "lastBriefedAt": ago(45)}), Some(&ago(60)), now), Some(45));
    assert_eq!(days_unused(&json!({"createdAt": ago(90), "lastBriefedAt": ago(10)}), Some(&ago(60)), now), None);
    assert_eq!(days_unused(&json!({"createdAt": ago(90), "updatedAt": ago(5)}), Some(&ago(60)), now), None);
    // The first month after counting began marks nothing it could not have seen.
    assert_eq!(days_unused(&json!({"createdAt": ago(90)}), Some(&ago(10)), now), None);
    assert_eq!(days_unused(&json!({"createdAt": ago(90)}), None, now), None);
    // Exactly at the line is not past it.
    assert_eq!(days_unused(&json!({"createdAt": ago(90)}), Some(&ago(30)), now), None);
    assert_eq!(days_unused(&json!({"createdAt": ago(90)}), Some(&ago(31)), now), Some(31));
}

// case: says so in the file every teammate reads, so a tidy pass can see it
#[test]
fn says_so_in_the_file_every_teammate_reads_so_a_tidy_pass_can_see_it() {
    let now = at("2026-09-24T12:00:00.000Z");
    let row = json!({"memoryId": "mem_x", "text": "Old note.", "scope": "workspace", "by": {"name": "you"},
        "createdAt": "2026-06-26T12:00:00.000Z", "status": "kept", "unusedDays": 40});
    let text = crate::memory_file::memory_file_text(&[row], now);
    assert!(text.contains("-- not given to a teammate in 40 days) [mem_x]"), "{text}");
}

// case: the same choice, made by the same function
#[test]
fn the_same_choice_made_by_the_same_function() {
    let (_t, store, folder) = fixture();
    let ws = workspace_id(&folder);
    for index in 0..12 {
        add_memory(&store, &format!("Note number {index} about the build."), None, "workspace", &ws, json!({"name":"you"}), "kept").unwrap();
    }
    let lines = rows(&store);
    let file = Some(".locust/memory.md");
    let given = briefed_memories(&lines, None, file);
    let section = crate::people::memory_section(None, Some("shop"), &lines, false, false, at("2026-09-24T12:00:00.000Z"), file, None);
    assert!(!given.is_empty());
    assert!(given.len() < lines.len(), "{} of {}", given.len(), lines.len());
    for line in &lines {
        let text = line["text"].as_str().unwrap();
        assert_eq!(section.contains(text), given.iter().any(|g| g["memoryId"] == line["memoryId"]), "{text}");
    }
}

// ---- a-named-memory-replaces-the-one-before-it.test.ts ----
// Measured on Colin's own store, 2026-09-21: 89 memories, 26 of them in a near-duplicate pair, the worst one teammate
// writing "orb round N, unit tests 31/31" once per test round. The fix is NOT similarity matching: identity is asserted
// by the writer, and a memory given a name replaces the one before it under that name.
use crate::people::{Added, add_memory_for, memory_name, memory_ops};

fn op(kind: &str, name: Option<&str>, text: &str) -> (String, Option<String>, String) {
    (kind.into(), name.map(str::to_owned), text.into())
}
fn yurt() -> Value {
    json!({"name": "Yurt"})
}
/// One `remember` into the folder `ws_one` (or everywhere), as a run writes it.
fn named(store: &Store, text: &str, scope: &str, name: Option<&str>, status: &str) -> Added {
    add_memory_for(store, text, name, scope, "ws_one", yurt(), status, Some("mission_7")).unwrap()
}

// case: reads `remember as <name> ::`
#[test]
fn reads_remember_as_name() {
    assert_eq!(memory_ops("<locust-memory>\nremember as orb-suite-status :: round 10, 38/38\n</locust-memory>"), vec![op("remember", Some("orb-suite-status"), "round 10, 38/38")]);
}

// case: reads the everywhere form, and still reads the unnamed ones exactly as before
#[test]
fn reads_the_everywhere_form_and_still_reads_the_unnamed_ones_exactly_as_before() {
    let ops = memory_ops("<locust-memory>\nremember everywhere as house-style :: diffs, not prose\nremember :: tests run with pnpm\nforget :: the API is on port 3000\n</locust-memory>");
    assert_eq!(ops, vec![
        op("remember everywhere", Some("house-style"), "diffs, not prose"),
        op("remember", None, "tests run with pnpm"),
        op("forget", None, "the API is on port 3000"),
    ]);
    // However the verb is spaced, it is one verb.
    assert_eq!(memory_ops("<locust-memory>\nremember   everywhere :: x\nabout  you :: short answers\n</locust-memory>")[1].0, "about you");
}

// case: refuses a name that is not a slug, keeping the memory rather than the name
#[test]
fn refuses_a_name_that_is_not_a_slug_keeping_the_memory_rather_than_the_name() {
    assert_eq!(memory_name("Orb Suite Status"), None);
    assert_eq!(memory_name("orb-suite-status").as_deref(), Some("orb-suite-status"));
    assert_eq!(memory_name("ORB-Suite").as_deref(), Some("orb-suite"));
    assert_eq!(memory_name("-leading"), None);
    assert_eq!(memory_name(&"a".repeat(65)), None);
    assert_eq!(memory_name(&"a".repeat(64)).map(|n| n.len()), Some(64));
    // Kept, without the name.
    let (_t, store, _) = fixture();
    named(&store, "kept anyway", "workspace", Some("Not A Slug"), "kept");
    let row = &rows(&store)[0];
    assert_eq!(row["text"], "kept anyway");
    assert!(row.get("name").is_none());
}

// case: does not let `forget` take a name: it quotes, it does not file
#[test]
fn does_not_let_forget_take_a_name_it_quotes_it_does_not_file() {
    assert_eq!(memory_ops("<locust-memory>\nforget as thing :: the API is on port 3000\n</locust-memory>"), vec![op("forget", None, "the API is on port 3000")]);
}

// case: rewrites in place: one row, the new words, the old ones kept
#[test]
fn rewrites_in_place_one_row_the_new_words_the_old_ones_kept() {
    let (_t, store, _) = fixture();
    assert_eq!(add_memory_for(&store, "orb round 4, tests 31/31", Some("orb-suite-status"), "workspace", "ws_one", yurt(), "kept", Some("mission_1")).unwrap(), Added::Kept);
    let first = rows(&store)[0].clone();
    assert_eq!(add_memory_for(&store, "orb round 10, tests 38/38", Some("orb-suite-status"), "workspace", "ws_one", json!({"name": "Wren"}), "kept", Some("mission_2")).unwrap(), Added::Rewritten);
    let listed = rows(&store);
    assert_eq!(listed.len(), 1);
    let second = &listed[0];
    // The same row: the id, the author and the birthday survive a rewrite.
    assert_eq!(second["memoryId"], first["memoryId"]);
    assert_eq!(second["createdAt"], first["createdAt"]);
    assert_eq!(second["by"], first["by"]);
    assert_eq!(second["text"], "orb round 10, tests 38/38");
    assert_eq!(second["previousText"], "orb round 4, tests 31/31");
    // Credit for the words, and the turn that changed it.
    assert_eq!(second["updatedBy"]["name"], "Wren");
    assert_eq!(second["missionId"], "mission_2");
}

// case: is what stops the store growing: nine rounds, one memory
#[test]
fn is_what_stops_the_store_growing_nine_rounds_one_memory() {
    let (_t, store, _) = fixture();
    for round in 4..=12 {
        named(&store, &format!("orb round {round}, tests 38/38"), "workspace", Some("orb-suite-status"), "kept");
    }
    let listed = rows(&store);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["text"], "orb round 12, tests 38/38");
}

// case: leaves an unnamed memory behaving exactly as it always did
#[test]
fn leaves_an_unnamed_memory_behaving_exactly_as_it_always_did() {
    let (_t, store, _) = fixture();
    assert_eq!(named(&store, "tests run with pnpm", "workspace", None, "kept"), Added::Kept);
    // Same text, punctuation-blind: de-duplicated, not rewritten.
    assert_eq!(named(&store, "Tests run with pnpm.", "workspace", None, "kept"), Added::Nothing);
    assert_eq!(rows(&store).len(), 1);
    assert_eq!(rows(&store)[0]["text"], "tests run with pnpm");
    // A name given to words already kept without one files nothing new beside them.
    assert_eq!(named(&store, "Tests run with pnpm", "workspace", Some("test-runner"), "kept"), Added::Nothing);
    assert_eq!(rows(&store).len(), 1);
}

// case: does not churn the file when the same name is given the same words
#[test]
fn does_not_churn_the_file_when_the_same_name_is_given_the_same_words() {
    let (_t, store, _) = fixture();
    named(&store, "round 10, 38/38", "workspace", Some("orb-suite-status"), "kept");
    let written = memories_bytes(&store);
    assert_eq!(named(&store, "round 10, 38/38", "workspace", Some("orb-suite-status"), "kept"), Added::Nothing);
    // The same words however punctuated or cased are the same words.
    assert_eq!(named(&store, "Round 10 -- 38/38.", "workspace", Some("orb-suite-status"), "kept"), Added::Nothing);
    assert_eq!(memories_bytes(&store), written);
    let row = &rows(&store)[0];
    assert!(row.get("previousText").is_none());
    assert!(row.get("updatedAt").is_none());
}

// case: keeps a name to its own place: this folder and everywhere are different memories
#[test]
fn keeps_a_name_to_its_own_place_this_folder_and_everywhere_are_different_memories() {
    let (_t, store, _) = fixture();
    named(&store, "here", "workspace", Some("status"), "kept");
    named(&store, "anywhere", "global", Some("status"), "kept");
    let mut texts: Vec<String> = rows(&store).iter().map(|m| m["text"].as_str().unwrap().to_owned()).collect();
    texts.sort();
    assert_eq!(texts, ["anywhere", "here"]);
}

// case: does not promote a proposed memory by rewriting it
#[test]
fn does_not_promote_a_proposed_memory_by_rewriting_it() {
    // Ask me first exists so a person sees a memory before it counts. A name must not be a way around that.
    let (_t, store, _) = fixture();
    named(&store, "first", "workspace", Some("thing"), "proposed");
    assert_eq!(named(&store, "second", "workspace", Some("thing"), "kept"), Added::Rewritten);
    let listed = rows(&store);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["status"], "proposed");
    assert_eq!(listed[0]["text"], "second");
}

// case: survives a reload, name and history intact
#[test]
fn survives_a_reload_name_and_history_intact() {
    let (t, store, _) = fixture();
    named(&store, "one", "workspace", Some("thing"), "kept");
    named(&store, "two", "workspace", Some("thing"), "kept");
    drop(store);
    let reopened = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    let listed = rows(&reopened);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["name"], "thing");
    assert_eq!(listed[0]["text"], "two");
    assert_eq!(listed[0]["previousText"], "one");
}

// ---- a-forgotten-memory-can-come-back.test.ts (A1.8) ----
// A forget -- the person's Remove, a teammate's forget in "Keep and tell me", a forget the person agreed to, Forget
// everything -- was final the moment it landed. Every KEPT memory that leaves is held in Recently forgotten for 7 days,
// with Restore. A proposal nobody kept is not held: it was never the person's.
use crate::people::{FORGOTTEN_DAYS, MAX_MEMORIES, MEMORY_UNREADABLE, clear_memories};

fn booty_team(store: &Store, mode: &str) {
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_booty","name":"Booty"}],"missionOwners":{},"settings":{"memoryMode":mode}}).to_string(),
    )
    .unwrap();
}
fn kept(store: &Store, folder: &Path, text: &str) -> String {
    add_memory(store, text, None, "workspace", &workspace_id(folder), json!({"teammateId":"tm_yurt","name":"Yurt"}), "kept").unwrap();
    rows(store).iter().find(|m| m["text"] == text).unwrap()["memoryId"].as_str().unwrap().to_owned()
}
fn held(store: &Store) -> Vec<Value> {
    memories(store).unwrap()["forgotten"].as_array().cloned().unwrap_or_default()
}
fn held_as(store: &Store) -> Vec<(String, String)> {
    held(store).iter().map(|e| (e["memory"]["text"].as_str().unwrap().to_owned(), e["forgottenBy"]["name"].as_str().unwrap().to_owned())).collect()
}
fn recent(at: &Value) -> bool {
    chrono::DateTime::parse_from_rfc3339(at.as_str().unwrap()).is_ok_and(|at| (chrono::Utc::now() - at.with_timezone(&chrono::Utc)).num_seconds().abs() < 60)
}
const FORGET_PORT: &str = "Done.\n<locust-memory>\nforget :: The API is on port 3000\n</locust-memory>";

// case: a person’s Remove: held, by you -- and Restore puts it back exactly as it was, briefed again
#[test]
fn a_persons_remove_held_by_you_and_restore_puts_it_back_exactly_as_it_was_briefed_again() {
    let (_t, store, folder) = fixture();
    add_memory_for(&store, "Deploys go out on Fridays.", Some("deploy-day"), "workspace", &workspace_id(&folder), json!({"teammateId":"tm_yurt","name":"Yurt"}), "kept", Some("mission_1")).unwrap();
    let id = rows(&store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&store, &id, "toggle", None).unwrap();
    let before = rows(&store)[0].clone();
    assert_eq!(before["enabled"], false);
    change_memory(&store, &id, "remove", None).unwrap();
    assert!(rows(&store).is_empty());
    let bin = held(&store);
    assert_eq!(bin.len(), 1);
    assert_eq!(bin[0]["memory"], before);
    assert_eq!(bin[0]["forgottenBy"], json!({"name": "you"}));
    assert!(recent(&bin[0]["forgottenAt"]));
    change_memory(&store, &id, "restore", None).unwrap();
    assert_eq!(rows(&store), vec![before]);
    assert!(held(&store).is_empty());
}

// case: a teammate’s forget in "Keep and tell me": held, by that teammate
#[test]
fn a_teammates_forget_in_keep_and_tell_me_held_by_that_teammate() {
    let (_t, store, folder) = fixture();
    booty_team(&store, "auto");
    kept(&store, &folder, "The API is on port 3000.");
    record_reply(&store, "tm_booty", &folder, FORGET_PORT).unwrap();
    assert!(rows(&store).is_empty());
    assert_eq!(held_as(&store), vec![("The API is on port 3000.".into(), "Booty".into())]);
    assert_eq!(held(&store)[0]["forgottenBy"]["teammateId"], "tm_booty");
}

// case: a forget the person agreed to ("Forget it"): held, by the teammate who asked
#[test]
fn a_forget_the_person_agreed_to_forget_it_held_by_the_teammate_who_asked() {
    let (_t, store, folder) = fixture();
    booty_team(&store, "ask");
    kept(&store, &folder, "The API is on port 3000.");
    record_reply(&store, "tm_booty", &folder, FORGET_PORT).unwrap();
    let proposal = rows(&store).into_iter().find(|m| m["status"] == "proposed").expect("the forget waits for the person");
    change_memory(&store, proposal["memoryId"].as_str().unwrap(), "keep", None).unwrap();
    assert!(rows(&store).is_empty());
    assert_eq!(held_as(&store), vec![("The API is on port 3000.".into(), "Booty".into())]);
}

// case: Forget everything for a folder: every kept one held, by you; the proposals are not
#[test]
fn forget_everything_for_a_folder_every_kept_one_held_by_you_the_proposals_are_not() {
    let (_t, store, folder) = fixture();
    let ws = workspace_id(&folder);
    kept(&store, &folder, "One.");
    kept(&store, &folder, "Two.");
    add_memory(&store, "Three, only proposed.", None, "workspace", &ws, json!({"name":"Yurt"}), "proposed").unwrap();
    add_memory(&store, "Everywhere.", None, "global", &ws, json!({"name":"Yurt"}), "kept").unwrap();
    add_memory(&store, "Another folder's.", None, "workspace", "ws_other", json!({"name":"Yurt"}), "kept").unwrap();
    assert_eq!(clear_memories(&store, Some(&ws)).unwrap(), 3);
    let mut left: Vec<String> = rows(&store).iter().map(|m| m["text"].as_str().unwrap().to_owned()).collect();
    left.sort();
    assert_eq!(left, ["Another folder's.", "Everywhere."]);
    let mut gone = held_as(&store);
    gone.sort();
    assert_eq!(gone, vec![("One.".into(), "you".into()), ("Two.".into(), "you".into())]);
    // The proposal is not written into the file either, where it would take a kept memory's place.
    // Read raw: the loader drops a held proposal, but only at the next write, after it took a place in this one.
    let raw = store.seeded_json("memories.json", json!({})).unwrap();
    assert_eq!(raw["forgotten"].as_array().map_or(0, Vec::len), 2, "{}", raw["forgotten"]);
    // And Forget everything, every folder and everywhere.
    assert_eq!(clear_memories(&store, None).unwrap(), 2);
    assert!(rows(&store).is_empty());
    assert_eq!(held(&store).len(), 4);
    // Nothing left: nothing written, nothing counted.
    assert_eq!(clear_memories(&store, None).unwrap(), 0);
}

// case: a proposal dropped -- Keep the old one, or a new one forgotten -- holds nothing
#[test]
fn a_proposal_dropped_keep_the_old_one_or_a_new_one_forgotten_holds_nothing() {
    let (_t, store, folder) = fixture();
    add_memory(&store, "Only proposed.", None, "workspace", &workspace_id(&folder), json!({"name":"Yurt"}), "proposed").unwrap();
    let id = rows(&store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&store, &id, "remove", None).unwrap();
    assert!(rows(&store).is_empty());
    assert!(held(&store).is_empty());
    // Nor is it written into the file, where it would take one of Recently forgotten's places from a kept memory.
    let file = store.seeded_json("memories.json", json!({})).unwrap();
    assert!(file["forgotten"].as_array().is_none_or(|bin| bin.is_empty()), "{}", file["forgotten"]);
}

/// The first held entry, laid `ms` before now.
fn forgotten_ms_ago(store: &Store, ms: i64) {
    let mut file = memories(store).unwrap();
    let at = chrono::Utc::now() - chrono::Duration::milliseconds(ms);
    file["forgotten"][0]["forgottenAt"] = json!(at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
    store.save_json("memories.json", &file).unwrap();
}

// case: is held ${String(FORGOTTEN_DAYS)} days, then shown no more and dropped at the next write
#[test]
fn is_held_7_days_then_shown_no_more_and_dropped_at_the_next_write() {
    assert_eq!(FORGOTTEN_DAYS, 7);
    let (_t, store, folder) = fixture();
    let id = kept(&store, &folder, "Old news.");
    change_memory(&store, &id, "remove", None).unwrap();
    forgotten_ms_ago(&store, FORGOTTEN_DAYS * DAY_MS - 60_000);
    assert_eq!(held(&store).len(), 1);
    forgotten_ms_ago(&store, FORGOTTEN_DAYS * DAY_MS + 1_000);
    assert!(held(&store).is_empty());
    assert_eq!(change_memory(&store, &id, "restore", None).unwrap_err(), "That memory is no longer in Recently forgotten.");
    kept(&store, &folder, "Something new.");
    let on_disk: Value = serde_json::from_slice(&memories_bytes(&store)).unwrap();
    assert_eq!(on_disk["forgotten"], json!([]));
}

// case: does not come back twice: the same words remembered again mean nothing to put back
#[test]
fn does_not_come_back_twice_the_same_words_remembered_again_mean_nothing_to_put_back() {
    let (_t, store, folder) = fixture();
    let id = kept(&store, &folder, "Tests run with pnpm test.");
    change_memory(&store, &id, "remove", None).unwrap();
    kept(&store, &folder, "Tests run with pnpm test.");
    change_memory(&store, &id, "restore", None).unwrap();
    let texts: Vec<Value> = rows(&store).iter().map(|m| m["text"].clone()).collect();
    assert_eq!(texts, vec![json!("Tests run with pnpm test.")]);
    assert!(held(&store).is_empty());
}

// case: comes back without its name when a newer memory has taken it
#[test]
fn comes_back_without_its_name_when_a_newer_memory_has_taken_it() {
    let (_t, store, folder) = fixture();
    let ws = workspace_id(&folder);
    add_memory(&store, "Deploys go out on Fridays.", Some("deploy-day"), "workspace", &ws, json!({"name":"Yurt"}), "kept").unwrap();
    let id = rows(&store)[0]["memoryId"].as_str().unwrap().to_owned();
    change_memory(&store, &id, "remove", None).unwrap();
    add_memory(&store, "Deploys go out on Thursdays.", Some("deploy-day"), "workspace", &ws, json!({"name":"Yurt"}), "kept").unwrap();
    change_memory(&store, &id, "restore", None).unwrap();
    let back = rows(&store).into_iter().find(|m| m["memoryId"] == id.as_str()).expect("restored");
    assert!(back.get("name").is_none(), "{back}");
    let named: Vec<Value> = rows(&store).iter().filter(|m| m["name"] == "deploy-day").map(|m| m["text"].clone()).collect();
    assert_eq!(named, vec![json!("Deploys go out on Thursdays.")]);
}

// case: is refused, and stays held, when memory is full
#[test]
fn is_refused_and_stays_held_when_memory_is_full() {
    let (_t, store, folder) = fixture();
    let id = kept(&store, &folder, "The one to restore.");
    change_memory(&store, &id, "remove", None).unwrap();
    let mut file = memories(&store).unwrap();
    file["memories"] = (0..MAX_MEMORIES)
        .map(|i| json!({"memoryId": format!("mem_filler{i}"), "text": format!("Filler number {i}."), "scope": "workspace", "workspaceId": workspace_id(&folder), "by": {"name": "you"}, "createdAt": "2026-09-24T12:00:00.000Z", "status": "kept", "enabled": true}))
        .collect();
    store.save_json("memories.json", &file).unwrap();
    assert!(change_memory(&store, &id, "restore", None).unwrap_err().contains("at most 400"));
    assert_eq!(held(&store).len(), 1);
    assert_eq!(rows(&store).len(), MAX_MEMORIES);
}

// case: from before 0.316 reads as nothing forgotten; a torn entry is dropped; one holding a proposal is dropped
#[test]
fn from_before_0316_reads_as_nothing_forgotten_a_torn_entry_is_dropped_one_holding_a_proposal_is_dropped() {
    let (_t, store, _) = fixture();
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let memory = json!({"memoryId": "mem_a", "text": "Kept.", "scope": "workspace", "workspaceId": "ws_shop", "workspaceName": "shop", "by": {"name": "you"}, "createdAt": now, "status": "kept", "enabled": true});
    store.save_json("memories.json", &json!({"schemaVersion": 1, "memories": [memory]})).unwrap();
    assert!(held(&store).is_empty());
    let with = |id: &str, status: &str| {
        let mut m = memory.clone();
        m["memoryId"] = json!(id);
        m["status"] = json!(status);
        m
    };
    store
        .save_json("memories.json", &json!({"schemaVersion": 1, "memories": [], "forgotten": [
            {"memory": memory, "forgottenAt": now, "forgottenBy": {"name": "you"}},
            {"memory": with("mem_b", "proposed"), "forgottenAt": now, "forgottenBy": {"name": "you"}},
            {"memory": with("mem_c", "kept"), "forgottenAt": "not a date", "forgottenBy": {"name": "you"}},
            {"memory": with("mem_d", "kept"), "forgottenAt": now, "forgottenBy": {"name": "  "}},
            "torn"
        ]}))
        .unwrap();
    let ids: Vec<Value> = held(&store).iter().map(|e| e["memory"]["memoryId"].clone()).collect();
    assert_eq!(ids, vec![json!("mem_a")]);
}

// case: with Recently forgotten that is not a list is unreadable, and nothing is written over it
#[test]
fn with_recently_forgotten_that_is_not_a_list_is_unreadable_and_nothing_is_written_over_it() {
    let (_t, store, folder) = fixture();
    let text = json!({"schemaVersion": 1, "memories": [], "forgotten": "everything"}).to_string();
    kept(&store, &folder, "First.");
    std::fs::write(store.root().join("memories.json"), &text).unwrap();
    assert_eq!(memories(&store).unwrap_err(), MEMORY_UNREADABLE);
    assert_eq!(add_memory(&store, "New.", None, "workspace", &workspace_id(&folder), json!({"name":"you"}), "kept").unwrap_err(), MEMORY_UNREADABLE);
    assert_eq!(std::fs::read_to_string(store.root().join("memories.json")).unwrap(), text);
}
