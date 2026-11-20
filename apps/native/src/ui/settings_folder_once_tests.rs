//! a-conversation-in-a-settings-folder-is-asked-about-once.test.ts (4 cases). Fixed 20 Nov: a relative path in
//! folder-cautions.json counted as an answer; a damaged file made every answer fail to save (Electron reads it as none
//! and writes over it); and a trailing separator made the same folder read as another. Cases 3 and 4 (nothing starts
//! until the folder is said yes to, asked in the window; only the current folder kept) are drawn by this module's own
//! tests; here they are read through `hold_send` and `keep_answer`.
use super::*;

struct Root(std::path::PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn state() -> (State, Root) {
    let root = std::env::temp_dir().join(format!("folder-once-{}", locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    let mut s = State::default();
    crate::composer_base_scene(&mut s, false);
    s.demo = false;
    s.data = Some(store::Store::new(root.join("native"), root.join("electron")).unwrap());
    s.selected = 0;
    s.ledger = None;
    for mate in &mut s.teammates {
        mate.folder = None;
    }
    for conversation in s.conversations.values_mut() {
        conversation.conversation_id = None;
    }
    s.home.folder_path = Some(format!("{}/.claude", std::env::var("HOME").unwrap()));
    (s, Root(root))
}

// case: keeps the folders said yes to, and reads nothing odd as one
#[test]
fn keeps_the_folders_said_yes_to_and_reads_nothing_odd_as_one() {
    let home = "/Users/person";
    assert!(kept_folders(&serde_json::json!({"schemaVersion": 1, "confirmed": []})).is_empty());
    assert_eq!(kept_folders(&serde_json::json!({"schemaVersion": 1, "confirmed": [format!("{home}/.claude")]})), [format!("{home}/.claude")]);
    assert_eq!(kept_folders(&serde_json::json!({"schemaVersion": 1, "confirmed": ["relative\\folder", 7, null, home]})), [home]);

    // A damaged file is read as no answers, asked about again, and written over by the answer.
    let (mut s, root) = state();
    std::fs::create_dir_all(root.0.join("native")).unwrap();
    std::fs::write(root.0.join("native").join(FILE), "not json").unwrap();
    assert!(hold_send(&mut s), "the folder is asked about");
    assert!(keep_answer(&mut s), "the answer is kept: {:?}", s.composer_note);
    assert!(!hold_send(&mut s), "and never asked again");
}

// case: matches the same folder only, as Windows spells it either way
#[test]
fn matches_the_same_folder_only_as_windows_spells_it() {
    assert!(same_folder_on(r"C:\Users\person\.claude", r"c:\users\PERSON\.claude\", true));
    assert!(!same_folder_on(r"C:\Users\person\.claude", r"C:\Users\person\.claude\skills", true));
    assert!(!same_folder_on("/home/person/.claude", "/home/person/.Claude", false));
    assert!(same_folder_on("/home/person/.claude/", "/home/person/.claude", false));
}

// case: starts nothing for a new conversation there until the folder is said yes to, and asks in the window
#[test]
fn starts_nothing_until_the_folder_is_said_yes_to() {
    let (mut s, _root) = state();
    assert!(hold_send(&mut s), "a new conversation in .claude is held");
    assert!(s.folder_question.is_some(), "and asked about in the window");
    for conversation in s.conversations.values_mut() {
        conversation.conversation_id = Some("c_1".into());
    }
    assert!(!hold_send(&mut s), "a conversation already under way is not asked about");
}

// case: keeps only the current folder, only from its own window, and only one that needed asking
#[test]
fn keeps_only_the_current_folder_and_only_one_that_needed_asking() {
    let (mut s, _root) = state();
    assert!(!keep_answer(&mut s), "nothing was asked, so nothing is kept");
    assert!(hold_send(&mut s));
    s.home.folder_path = Some("/projects/shop".into());
    assert!(!keep_answer(&mut s), "the question was about another folder");
    assert!(confirmed(s.data.as_ref().unwrap()).unwrap().is_empty());
    s.home.folder_path = Some(format!("{}/.claude", std::env::var("HOME").unwrap()));
    assert!(hold_send(&mut s));
    assert!(keep_answer(&mut s));
    assert_eq!(confirmed(s.data.as_ref().unwrap()).unwrap().len(), 1);
}
