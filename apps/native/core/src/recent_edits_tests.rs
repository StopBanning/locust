//! A2.9: THE OVERLAP NOTE (Electron `two-teammates-hear-about-each-others-edits.test.ts`). Two teammates in one folder
//! who each changed the same file are told about each other in their next brief, so the second edit is made knowing
//! about the first.
use super::*;

const FOLDER: &str = "C:/work/app";
const T0: i64 = 1_790_251_200_000; // 2026-09-24T12:00:00Z
fn later(minutes: i64) -> i64 {
    T0 + minutes * 60_000
}
fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|p| (*p).to_owned()).collect()
}

// case: names the file another teammate changed that you changed too, and who, and when
#[test]
fn names_the_file_another_teammate_changed_that_you_changed_too_and_who_and_when() {
    let mut edits = RecentEdits::default();
    edits.record(FOLDER, "tm_wren", "Wren", &paths(&["src/app.ts", "src/only-wren.ts"]), T0);
    edits.record(FOLDER, "tm_ash", "Ash", &paths(&["src/app.ts", "README.md"]), later(20));
    let note = edits.overlap_for(FOLDER, "tm_wren", later(60)).unwrap();
    assert!(note.contains("- Ash changed src/app.ts (last 40 minutes ago)."), "{note}");
    assert!(note.starts_with("Files you changed recently in this folder were also changed by another teammate"), "{note}");
    assert!(!note.contains("README.md"));
    assert!(!note.contains("only-wren"));
    // And the other way round: Ash hears about Wren.
    assert!(edits.overlap_for(FOLDER, "tm_ash", later(60)).unwrap().contains("- Wren changed src/app.ts"));
}

// case: says nothing when nobody else touched your files
#[test]
fn says_nothing_when_nobody_else_touched_your_files() {
    let mut edits = RecentEdits::default();
    edits.record(FOLDER, "tm_wren", "Wren", &paths(&["src/app.ts"]), T0);
    edits.record(FOLDER, "tm_ash", "Ash", &paths(&["README.md"]), T0);
    assert_eq!(edits.overlap_for(FOLDER, "tm_wren", later(5)), None);
    // Nor to a teammate who changed nothing here.
    assert_eq!(edits.overlap_for(FOLDER, "tm_new", later(5)), None);
}

// case: is about one folder: the same path in another folder is another file
#[test]
fn is_about_one_folder_the_same_path_in_another_folder_is_another_file() {
    let mut edits = RecentEdits::default();
    // A real overlap in the other folder, recorded first, must not leak into this one.
    edits.record("C:/work/other", "tm_wren", "Wren", &paths(&["src/app.ts"]), T0);
    edits.record("C:/work/other", "tm_ash", "Ash", &paths(&["src/app.ts"]), T0);
    edits.record(FOLDER, "tm_wren", "Wren", &paths(&["src/app.ts"]), T0);
    assert_eq!(edits.overlap_for(FOLDER, "tm_wren", later(5)), None);
    assert!(edits.overlap_for("C:/work/other", "tm_wren", later(5)).unwrap().contains("Ash"));
}

// case: treats one folder as one, however Windows spells it
#[test]
fn treats_one_folder_as_one_however_windows_spells_it() {
    let mut edits = RecentEdits::default();
    // A trailing slash and backslashes are one folder everywhere.
    edits.record("C:\\work\\app\\", "tm_wren", "Wren", &paths(&["src/app.ts"]), T0);
    edits.record("C:/work/app/", "tm_ash", "Ash", &paths(&["src/app.ts"]), T0);
    assert!(edits.overlap_for(FOLDER, "tm_wren", later(5)).unwrap().contains("Ash"));
    // Letter case only on Windows, as Electron (process.platform === 'win32').
    let mut cased = RecentEdits::default();
    cased.record("C:\\Work\\App\\", "tm_wren", "Wren", &paths(&["src/app.ts"]), T0);
    cased.record("c:/work/app", "tm_ash", "Ash", &paths(&["src/app.ts"]), T0);
    assert_eq!(cased.overlap_for(FOLDER, "tm_wren", later(5)).is_some(), cfg!(windows));
}

// case: forgets a change once it is no longer recent
#[test]
fn forgets_a_change_once_it_is_no_longer_recent() {
    let mut edits = RecentEdits::default();
    edits.record(FOLDER, "tm_wren", "Wren", &paths(&["src/app.ts"]), T0);
    edits.record(FOLDER, "tm_ash", "Ash", &paths(&["src/app.ts"]), T0);
    assert!(edits.overlap_for(FOLDER, "tm_wren", T0 + OVERLAP_WINDOW_MS).is_some(), "the edge of the window still counts");
    assert_eq!(edits.overlap_for(FOLDER, "tm_wren", T0 + OVERLAP_WINDOW_MS + 60_000), None);
    assert_eq!(OVERLAP_WINDOW_MS, 6 * 60 * 60 * 1000);
    // How long ago reads in hours past the first one.
    let mut hours = RecentEdits::default();
    hours.record(FOLDER, "tm_wren", "Wren", &paths(&["a"]), T0);
    hours.record(FOLDER, "tm_ash", "Ash", &paths(&["a"]), T0);
    assert!(hours.overlap_for(FOLDER, "tm_wren", later(60)).unwrap().contains("(last an hour ago)"));
    assert!(hours.overlap_for(FOLDER, "tm_wren", later(150)).unwrap().contains("(last 3 hours ago)"));
    assert!(hours.overlap_for(FOLDER, "tm_wren", T0 + 10_000).unwrap().contains("(last a minute ago)"));
}

// case: names eight files per teammate and counts the rest
#[test]
fn names_eight_files_per_teammate_and_counts_the_rest() {
    let mut edits = RecentEdits::default();
    let many: Vec<String> = (0..11).map(|i| format!("src/f{i}.ts")).collect();
    edits.record(FOLDER, "tm_wren", "Wren", &many, T0);
    edits.record(FOLDER, "tm_ash", "Ash", &many, T0);
    let note = edits.overlap_for(FOLDER, "tm_wren", later(5)).unwrap();
    assert!(note.contains(" and 3 more (last"), "{note}");
    let line = note.lines().nth(1).unwrap();
    assert_eq!(line.matches("src/f").count(), 8, "{line}");
    // The newest are the ones named.
    let mut newest = RecentEdits::default();
    newest.record(FOLDER, "tm_wren", "Wren", &many, T0);
    newest.record(FOLDER, "tm_ash", "Ash", &many, T0);
    newest.record(FOLDER, "tm_ash", "Ash", &paths(&["src/f10.ts"]), later(30));
    let note = newest.overlap_for(FOLDER, "tm_wren", later(40)).unwrap();
    assert!(note.contains("Ash changed src/f10.ts, "), "{note}");
    assert!(note.contains("(last 10 minutes ago)"), "{note}");
}

// case: goes right before the person’s words, with what arrived this turn
#[test]
fn goes_right_before_the_persons_words_with_what_arrived_this_turn() {
    use crate::briefing::{PeerEntry, assemble_held, roster_section};
    let wren = PeerEntry { teammate_id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into() };
    let ash = PeerEntry { teammate_id: "tm_ash".into(), name: "Ash".into(), role: "Docs & QA".into() };
    let standing = vec![roster_section(&wren, std::slice::from_ref(&ash))];
    let roster = vec![wren.clone(), ash.clone()];
    let (brief, _, _) = assemble_held(&standing, &[], 0, &roster, None, "Tidy app.ts.", None, None, None, Some("NOTE: Ash changed src/app.ts."));
    let note = brief.find("NOTE: Ash").expect("the note is in the brief");
    assert!(note < brief.find("Tidy app.ts.").unwrap(), "{brief}");
    assert!(note > brief.find("Ash (Docs & QA)").expect("the roster names Ash"), "{brief}");
    // No note, nothing said.
    let (plain, _, _) = assemble_held(&standing, &[], 0, &roster, None, "Tidy app.ts.", None, None, None, None);
    assert!(!plain.contains("NOTE"));
}

// The note reaches a real teammate brief: the brief asks for it with its own folder and teammate.
#[test]
fn a_teammates_brief_carries_the_note_for_its_own_folder() {
    let root = std::env::temp_dir().join(format!("locust-overlap-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::write(
        store.electron().join("teammates.json"),
        serde_json::json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"Code & Migrations"},{"teammateId":"tm_ash","name":"Ash","role":"Docs & QA"}],"missionOwners":{},"settings":{}}).to_string(),
    )
    .unwrap();
    let shop = root.join("shop");
    crate::recent_edits::record(&shop.to_string_lossy(), "tm_wren", "Wren", &["src/app.ts".to_owned()]);
    crate::recent_edits::record(&shop.to_string_lossy(), "tm_ash", "Ash", &["src/app.ts".to_owned()]);
    let (briefed, _) = crate::people::turn_brief(&store, "tm_wren", &shop, "Tidy app.ts.", None, Some("claude"), &[], 0).unwrap();
    let (ashs, _) = crate::people::turn_brief(&store, "tm_ash", &shop, "Tidy app.ts.", None, Some("claude"), &[], 0).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    let note = briefed.find("- Ash changed src/app.ts (last").unwrap_or_else(|| panic!("{briefed}"));
    assert!(note < briefed.find("Tidy app.ts.").unwrap());
    assert!(ashs.contains("- Wren changed src/app.ts (last"), "{ashs}");
}
