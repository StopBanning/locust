//! Electron's `main/a-reference-stays-with-its-conversation.test.ts`: a reference is kept by its conversation (across a
//! restart and later turns, never another conversation's), refused when the window names a path, an unknown id, a
//! repeat or a sixth, and never widens the mode it runs in. The cancelled picker is the window's
//! (`window_drive_tests`); the GitHub copies are not ported (Rust adds folders on this computer only).
use super::*;

struct Dirs(std::path::PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Dirs, Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("locust-reference-test-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(dir.join("electron")).unwrap();
    let picked = dir.join("picked");
    std::fs::create_dir_all(&picked).unwrap();
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    let picked = picked.canonicalize().unwrap();
    (Dirs(dir), store, picked)
}

// case: survives a restart and later turns, but not another conversation of the same teammate
#[test]
fn survives_a_restart_and_later_turns_but_not_another_conversation() {
    let (dir, store, picked) = fixture();
    let item = add_folder(&store, &picked).unwrap();
    bind(&store, "mission_first", None, std::slice::from_ref(&item.id), Some("/actual-working-folder")).unwrap();
    // A restart: a new store over the same folders.
    let restarted = Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    assert_eq!(load(&restarted).unwrap()["folders"]["mission_first"], "/actual-working-folder");
    bind(&restarted, "mission_second", Some("mission_first"), std::slice::from_ref(&item.id), None).unwrap();
    assert_eq!(select(&restarted, Some("mission_second"), None).unwrap().iter().map(|r| r.path.clone()).collect::<Vec<_>>(), vec![picked.to_string_lossy().into_owned()]);
    bind(&restarted, "mission_third", Some("mission_second"), std::slice::from_ref(&item.id), None).unwrap();
    assert_eq!(select(&restarted, Some("mission_third"), None).unwrap(), vec![item]);
    assert_eq!(select(&restarted, Some("mission_other"), None).unwrap(), vec![]);
}

// case: removal affects the conversation, not the chosen folder or another conversation
#[test]
fn removal_affects_the_conversation_not_the_folder_or_another_conversation() {
    let (_dir, store, picked) = fixture();
    let item = add_folder(&store, &picked).unwrap();
    bind(&store, "mission_first", None, std::slice::from_ref(&item.id), None).unwrap();
    bind(&store, "mission_other", None, std::slice::from_ref(&item.id), None).unwrap();
    bind(&store, "mission_second", Some("mission_first"), std::slice::from_ref(&item.id), None).unwrap();
    set(&store, "mission_second", &[]).unwrap();
    assert_eq!(select(&store, Some("mission_first"), None).unwrap(), vec![]);
    assert_eq!(select(&store, Some("mission_other"), None).unwrap(), vec![item]);
    assert!(picked.is_dir());
}

// case: refuses paths or invented identities from the window, duplicate IDs, and a sixth reference
#[test]
fn refuses_paths_invented_identities_duplicates_and_a_sixth() {
    let (_dir, store, picked) = fixture();
    let items: Vec<Item> = (0..6).map(|_| add_folder(&store, &picked).unwrap()).collect();
    let path = picked.to_string_lossy().into_owned();
    assert!(bind(&store, "mission_first", None, &[path], None).unwrap_err().contains("using Add a reference"));
    assert!(select(&store, None, Some(&["__proto__".to_owned()])).unwrap_err().contains("using Add a reference"));
    let six: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
    assert!(bind(&store, "mission_first", None, &six, None).unwrap_err().contains("five"));
    assert!(bind(&store, "mission_first", None, &six[..5], None).is_ok(), "five is allowed");
    set(&store, "mission_first", &[]).unwrap();
    assert!(bind(&store, "mission_first", None, &[six[0].clone(), six[0].clone()], None).unwrap_err().contains("five"));
    assert_eq!(select(&store, Some("mission_first"), None).unwrap(), vec![]);
}

const AUTO_NOTE: &str = "This mode can change it. The brief asks for reading, not changing.";

// case: full-access modes keep the reference with a plain write warning
#[test]
fn full_access_modes_keep_the_reference_with_a_plain_write_warning() {
    for runtime in ["codex", "claude", "opencode", "copilot", "antigravity", "cursor", "muse"] {
        assert_eq!(access(runtime, "auto", "/reference", None, false), (true, AUTO_NOTE), "{runtime}");
    }
}

// case: Codex reads outside its workspace without granting add-dir write access
#[test]
fn codex_reads_outside_its_workspace_without_granting_write_access() {
    let (include, note) = access("codex", "accept-edits", "/reference", Some("/project"), false);
    assert!(include);
    assert!(note.contains("No extra writable folder"), "{note}");
}

// case: names references without changing an unextended runtime or inventing a write-access reason to omit them
#[test]
fn names_references_without_changing_an_unextended_runtime() {
    for runtime in ["cursor", "muse", "another-runtime"] {
        for mode in ["ask", "plan", "accept-edits", "approve-each"] {
            assert_eq!(
                access(runtime, mode, "/reference", Some("/project"), false),
                (true, "Named in the brief; this runtime's existing permissions still apply."),
                "{runtime} {mode}"
            );
        }
    }
}

// case: a restricted runtime omits a grant that would make the reference writable and says why
#[test]
fn a_restricted_runtime_omits_a_grant_that_would_make_it_writable() {
    for runtime in ["opencode", "copilot", "antigravity"] {
        assert!(access(runtime, "ask", "/reference", None, false).0, "{runtime}");
        let (include, note) = access(runtime, "accept-edits", "/reference", None, false);
        assert!(!include, "{runtime}");
        assert!(note.contains("also grant write access"), "{note}");
    }
}

// case: does not pretend a folder already writable in the workspace is read-only
#[test]
fn does_not_pretend_a_folder_already_writable_in_the_workspace_is_read_only() {
    assert!(access("opencode", "accept-edits", "C:\\project\\design", Some("C:/project"), false).1.contains("This mode can change it"));
    assert!(!access("opencode", "accept-edits", "/Project/design", Some("/project"), false).0);
    assert!(!access("opencode", "accept-edits", "/project-other", Some("/project"), false).0);
    assert!(!access("codex", "auto", "/reference", Some("/project"), true).0);
}

// case: Claude Edit and Approve each keep references with their scoped read-only promise, but Auto keeps its warning
#[test]
fn claude_edit_and_approve_each_keep_their_read_only_promise_but_auto_warns() {
    for mode in ["accept-edits", "approve-each"] {
        assert_eq!(access("claude", mode, "C:/reference", None, false), (true, "For reading, not changing."), "{mode}");
    }
    assert_eq!(access("claude", "accept-edits", "C:/project/reference", Some("C:/project"), false).1, "For reading, not changing.");
    assert!(access("claude", "auto", "C:/reference", None, false).1.contains("This mode can change it"));
}

// case: the every-turn brief names only attached folders and explicitly withdraws old references
#[test]
fn the_every_turn_brief_names_only_attached_folders_and_withdraws_old_ones() {
    assert!(section(&["/reference one", "/reference two"]).contains("for reading, not changing: \"/reference one\", \"/reference two\""));
    assert!(section(&[]).contains("earlier references are not attached now"));
}
