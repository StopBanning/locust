//! Carries apps/desktop/src/main/a-folder-with-only-dot-files-holds-nothing-yet.test.ts (2026-10-30): the folder Locust
//! makes for a new person gets a `.locust` folder at the first turn and still holds nothing as far as the person is
//! concerned. Where the answer is used is idle_teammate.rs (`folder_holds_nothing_tests`).
use super::holds_nothing_yet;

#[test]
fn is_one_with_nothing_or_nothing_but_dot_files_in_it() {
    let folder = std::env::temp_dir().join(format!("locust-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let held = (|| {
        let mut said = vec![holds_nothing_yet(&folder)];
        std::fs::create_dir(folder.join(".locust")).unwrap();
        std::fs::write(folder.join(".locust").join("memory.md"), "nothing yet").unwrap();
        std::fs::write(folder.join(".DS_Store"), "").unwrap();
        said.push(holds_nothing_yet(&folder));
        std::fs::write(folder.join("index.html"), "<h1>Crumb</h1>").unwrap();
        said.push(holds_nothing_yet(&folder));
        said
    })();
    let _ = std::fs::remove_dir_all(&folder);
    assert_eq!(held, [true, true, false]);
}

#[test]
fn is_never_a_folder_that_cannot_be_read() {
    assert!(!holds_nothing_yet(&std::env::temp_dir().join("locust-no-such-folder-30-10")));
    // A file is not a folder, so it holds nothing to call empty either.
    let file = std::env::temp_dir().join(format!("locust-not-a-folder-{}", std::process::id()));
    std::fs::write(&file, "x").unwrap();
    let said = holds_nothing_yet(&file);
    let _ = std::fs::remove_file(&file);
    assert!(!said);
}
