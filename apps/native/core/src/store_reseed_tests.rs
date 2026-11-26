//! Cutover 1, row 3: seeding the Rust data folder again from Electron's. Made-up folders under the test's temp dir.
use super::*;
use std::fs;

struct Folders {
    temp: PathBuf,
    electron: PathBuf,
    root: PathBuf,
}
impl Folders {
    fn new(tag: &str) -> Self {
        let temp = std::env::temp_dir().join(format!(
            "locust-reseed-{tag}-{}-{}",
            std::process::id(),
            crate::native::uuid_v4().unwrap()
        ));
        let (electron, root) = (temp.join("electron"), temp.join("native"));
        fs::create_dir_all(&electron).unwrap();
        fs::create_dir_all(&root).unwrap();
        Self { temp, electron, root }
    }
    fn store(&self) -> Store {
        Store::with_codex(self.root.clone(), self.electron.clone(), vec![]).unwrap()
    }
    /// Every file under Electron's folder, with its bytes and modified time.
    fn electron_as_it_is(&self) -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
        let mut out = Vec::new();
        let mut stack = vec![self.electron.clone()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                let meta = fs::symlink_metadata(&path).unwrap();
                if meta.is_dir() {
                    stack.push(path.clone());
                    out.push((path, Vec::new(), meta.modified().unwrap()));
                } else {
                    out.push((path.clone(), fs::read(&path).unwrap_or_default(), meta.modified().unwrap()));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}
impl Drop for Folders {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp);
    }
}

const STALE: &str = r#"{"schemaVersion":1,"teammates":[{"teammateId":"old","name":"Old"}],"missionOwners":{},"settings":{}}"#;
const FRESH: &str = r#"{"schemaVersion":1,"teammates":[{"teammateId":"new","name":"New"}],"missionOwners":{},"settings":{}}"#;

#[test]
fn a_fresh_copy_replaces_a_stale_one_and_the_stale_one_is_kept_aside() {
    let f = Folders::new("fresh");
    fs::write(f.electron.join("teammates.json"), FRESH).unwrap();
    fs::write(f.electron.join("memories.json"), r#"{"schemaVersion":1,"memories":[{"text":"new"}]}"#).unwrap();
    fs::write(f.electron.join("folders.json"), r#"{"schemaVersion":1,"folders":[]}"#).unwrap();
    fs::write(f.root.join("teammates.json"), STALE).unwrap();
    fs::write(f.root.join("folders.json"), r#"{"schemaVersion":1,"folders":[]}"#).unwrap();
    let store = f.store();
    // Seeded once, the old way: the stale copy is what the app reads.
    assert_eq!(store.seeded_json("teammates.json", serde_json::json!({})).unwrap()["teammates"][0]["name"], "Old");

    let done = store.reseed_from_electron("20261125-120000").unwrap();

    assert_eq!(fs::read_to_string(f.root.join("teammates.json")).unwrap(), FRESH);
    assert_eq!(store.seeded_json("teammates.json", serde_json::json!({})).unwrap()["teammates"][0]["name"], "New");
    let aside = f.root.join("seeded-before-20261125-120000");
    assert_eq!(fs::read_to_string(aside.join("teammates.json")).unwrap(), STALE, "the stale copy is kept, not deleted");
    assert!(fs::read_to_string(f.root.join("memories.json")).unwrap().contains("new"), "one never seeded is copied too");
    assert_eq!(done.copied, ["teammates.json", "memories.json"]);
    assert_eq!(done.moved_aside, ["teammates.json"]);
    assert_eq!(done.aside.as_deref(), Some(aside.as_path()));
    assert_eq!(done.unchanged, ["folders.json"], "a copy already the same is not moved aside");
    assert!(!aside.join("folders.json").exists() && !aside.join("memories.json").exists());
    assert!(done.skipped.iter().any(|(name, why)| name == "routines.json" && why == "Electron's folder has none"));
    // No half-written copy is left behind.
    assert!(fs::read_dir(&f.root).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".seeding")));
}

#[test]
fn an_unreadable_or_invalid_electron_file_leaves_the_rust_copy_untouched() {
    let f = Folders::new("bad");
    fs::write(f.electron.join("teammates.json"), "{\"schemaVersion\":1, cut off").unwrap();
    // A folder where a file should be: it cannot be read as one.
    fs::create_dir_all(f.electron.join("memories.json")).unwrap();
    fs::write(f.electron.join("routines.json"), "x".repeat(2 * 1024 * 1024 + 1)).unwrap();
    for name in ["teammates.json", "memories.json", "routines.json"] {
        fs::write(f.root.join(name), STALE).unwrap();
    }

    let done = f.store().reseed_from_electron("20261125-120001").unwrap();

    for name in ["teammates.json", "memories.json", "routines.json"] {
        assert_eq!(fs::read_to_string(f.root.join(name)).unwrap(), STALE, "{name}");
    }
    assert!(!f.root.join("seeded-before-20261125-120001").exists(), "nothing was moved aside");
    assert!(done.copied.is_empty() && done.moved_aside.is_empty() && done.aside.is_none());
    let why = |name: &str| done.skipped.iter().find(|(n, _)| n == name).map(|(_, w)| w.clone()).unwrap_or_default();
    assert!(why("teammates.json").starts_with("Electron's copy is not JSON"), "{done:?}");
    assert!(why("memories.json").starts_with("Electron's copy could not be read"), "{done:?}");
    assert_eq!(why("routines.json"), "Electron's copy is too large");
}

#[test]
fn electrons_folder_is_never_written() {
    let f = Folders::new("electron");
    fs::write(f.electron.join("teammates.json"), FRESH).unwrap();
    fs::write(f.electron.join("appearance.json"), "not json").unwrap();
    fs::create_dir_all(f.electron.join("mission-ledger")).unwrap();
    fs::write(f.root.join("teammates.json"), STALE).unwrap();
    let before = f.electron_as_it_is();

    f.store().reseed_from_electron("20261125-120002").unwrap();
    f.store().reseed_from_electron("20261125-120003").unwrap();

    assert_eq!(f.electron_as_it_is(), before);
    // And a data folder inside Electron's can never be the one seeded.
    assert!(Store::with_codex(f.electron.join("native"), f.electron.clone(), vec![]).is_err());
}

#[test]
fn seeding_again_is_refused_while_an_engine_holds_the_folder() {
    let f = Folders::new("engine");
    fs::write(f.electron.join("teammates.json"), FRESH).unwrap();
    fs::write(f.root.join("teammates.json"), STALE).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(f.root.join(crate::engine_process::LOCK))
        .unwrap();
    // SAFETY: flock on a descriptor this test owns.
    assert_eq!(unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock), libc::LOCK_EX | libc::LOCK_NB) }, 0);

    let refused = f.store().reseed_from_electron("20261125-120004").unwrap_err();

    assert!(refused.contains("Quit it"), "{refused}");
    assert_eq!(fs::read_to_string(f.root.join("teammates.json")).unwrap(), STALE);
    drop(lock);
    assert!(f.store().reseed_from_electron("20261125-120005").is_ok());
}

#[test]
fn every_file_the_app_seeds_from_electron_is_seeded_again() {
    // The nine names the app's own readers ask `seeded_json` for (people, folders, routines, approval rules, references,
    // appearance, terminal imports, folder cautions): one left out would stay days old on the switch.
    let names = [
        "teammates.json",
        "memories.json",
        "folders.json",
        "routines.json",
        "approval-rules.json",
        "conversation-references.json",
        "appearance.json",
        "terminal-imports.json",
        "folder-cautions.json",
    ];
    let f = Folders::new("all");
    for name in names {
        fs::write(f.electron.join(name), format!(r#"{{"from":"electron","file":"{name}"}}"#)).unwrap();
        fs::write(f.root.join(name), r#"{"from":"rust"}"#).unwrap();
    }

    let done = f.store().reseed_from_electron("20261125-120006").unwrap();

    let mut copied = done.copied.clone();
    copied.sort();
    let mut expected: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    expected.sort();
    assert_eq!(copied, expected, "{done:?}");
    for name in names {
        assert_eq!(fs::read_to_string(f.root.join(name)).unwrap(), fs::read_to_string(f.electron.join(name)).unwrap(), "{name}");
        assert_eq!(fs::read_to_string(f.root.join("seeded-before-20261125-120006").join(name)).unwrap(), r#"{"from":"rust"}"#, "{name}");
    }
}

#[test]
fn a_stamp_that_is_not_a_plain_name_is_refused_before_anything_is_touched() {
    let f = Folders::new("stamp");
    fs::write(f.electron.join("teammates.json"), FRESH).unwrap();
    fs::write(f.root.join("teammates.json"), STALE).unwrap();
    for stamp in ["", "a/b", "../elsewhere", "two words", "x\u{0}y"] {
        assert!(f.store().reseed_from_electron(stamp).is_err(), "{stamp:?}");
    }
    assert_eq!(fs::read_to_string(f.root.join("teammates.json")).unwrap(), STALE);
    assert!(fs::read_dir(&f.root).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with("seeded-before")));
}

#[test]
fn a_copy_that_is_really_a_link_into_electrons_folder_is_not_touched() {
    // `Store::refuse`: Electron's folder is only read. A Rust copy that is a link into it is where a replace could land.
    let f = Folders::new("link");
    fs::write(f.electron.join("teammates.json"), FRESH).unwrap();
    fs::write(f.electron.join("other.json"), STALE).unwrap();
    std::os::unix::fs::symlink(f.electron.join("other.json"), f.root.join("teammates.json")).unwrap();
    let before = f.electron_as_it_is();

    let done = f.store().reseed_from_electron("20261125-120007").unwrap();

    assert!(done.copied.is_empty() && done.moved_aside.is_empty() && done.aside.is_none(), "{done:?}");
    assert!(
        done.skipped.iter().any(|(name, why)| name == "teammates.json" && why.contains("Refusing to write inside Electron's data folder")),
        "{done:?}"
    );
    assert!(fs::symlink_metadata(f.root.join("teammates.json")).unwrap().file_type().is_symlink(), "the link is left where it was");
    assert_eq!(f.electron_as_it_is(), before);
}
