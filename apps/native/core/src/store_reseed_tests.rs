//! Cutover 1, row 3: seeding the Rust data folder again from Electron's. Made-up folders under the test's temp dir.
use super::*;
use std::fs;
use std::path::Path;
use std::sync::Arc;

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

// ---- Live hour 1, faults 2 and 24: the looks the teammates chose, and the two settings that stayed Off. ----

/// A lossless WebP header for a 9-row (version 1) pet sheet: what `webp_size` reads, 64 bytes long.
fn pet_sheet(tag: u8) -> Vec<u8> {
    let length = 64usize;
    let mut b = vec![tag; length];
    b[0..4].copy_from_slice(b"RIFF");
    b[4..8].copy_from_slice(&((length - 8) as u32).to_le_bytes());
    b[8..12].copy_from_slice(b"WEBP");
    b[12..16].copy_from_slice(b"VP8L");
    b[16..20].copy_from_slice(&((length - 20) as u32).to_le_bytes());
    b[20] = 0x2f;
    let bits = (1536 - 1) | ((1872 - 1) << 14) | (1 << 28);
    b[21..25].copy_from_slice(&u32::to_le_bytes(bits));
    b
}

fn put_pet(root: &Path, id: &str, name: &str, tag: u8) {
    let folder = root.join("pets").join(id);
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("pet.json"),
        serde_json::json!({"id": id, "displayName": name, "spritesheetPath": "spritesheet.webp"}).to_string(),
    )
    .unwrap();
    fs::write(folder.join("spritesheet.webp"), pet_sheet(tag)).unwrap();
}

const MCP_ON: &str = r#"{"enabled":true,"ownMode":true,"conversations":true}"#;
const MCP_OFF: &str = r#"{"enabled":true,"ownMode":false}"#;
const FAR: i64 = 4_102_444_800_000;
fn remote_on() -> String {
    format!(
        r#"{{"v":1,"enabled":true,"grants":[{{"id":"g1","name":"Claude","clientId":"c","host":"claude.ai","added":1,"used":2,"access":"aHash","accessUntil":{FAR},"refresh":"rHash","refreshUntil":{FAR}}}]}}"#
    )
}

#[test]
fn the_teammates_chosen_looks_come_across_with_the_pets_electron_added() {
    // Electron's `pets/` holds what Argon (the Macintosh) and Scout (the cat) wear; the Rust app draws a face from its
    // own `pets/`, so a pet that never came across is a face that falls back to a bot.
    let f = Folders::new("pets");
    put_pet(&f.temp.join("electron"), "macintosh", "Macintosh", 1);
    put_pet(&f.temp.join("electron"), "rainbow-terminal-cat", "Rainbow Terminal Cat", 2);
    let store = f.store();
    let library = crate::pet_library::PetLibrary::new(f.root.join("pets"), f.root.join("pets-bundled"), None, None);
    assert!(library.added("macintosh").is_none(), "before seeding the Rust app has no such pet");

    let done = store.reseed_from_electron("20261126-100000").unwrap();

    assert_eq!(library.added("macintosh").map(|p| p.display_name), Some("Macintosh".into()));
    assert_eq!(library.added("rainbow-terminal-cat").map(|p| p.display_name), Some("Rainbow Terminal Cat".into()));
    assert_eq!(library.sheet(crate::bot_spec::PetSource::Gallery, "macintosh").unwrap().0, pet_sheet(1));
    let mut copied = done.copied.clone();
    copied.sort();
    assert_eq!(copied, ["pets/macintosh", "pets/rainbow-terminal-cat"], "{done:?}");
    assert!(done.moved_aside.is_empty() && done.aside.is_none(), "{done:?}");
    assert!(fs::read_dir(f.root.join("pets")).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".incoming")));
}

#[test]
fn a_pet_that_changed_replaces_the_old_one_which_is_kept_aside_and_one_that_is_the_same_is_left() {
    let f = Folders::new("petsaside");
    let electron = f.temp.join("electron");
    put_pet(&electron, "macintosh", "Macintosh", 9);
    put_pet(&electron, "nori", "Nori", 4);
    put_pet(&f.temp.join("native"), "macintosh", "Old Macintosh", 3);
    put_pet(&f.temp.join("native"), "nori", "Nori", 4);
    put_pet(&f.temp.join("native"), "only-here", "Only Here", 5);
    let before = f.electron_as_it_is();

    let done = f.store().reseed_from_electron("20261126-100001").unwrap();

    let read = |p: PathBuf| fs::read_to_string(p).unwrap();
    assert!(read(f.root.join("pets/macintosh/pet.json")).contains(r#""displayName":"Macintosh""#));
    assert_eq!(fs::read(f.root.join("pets/macintosh/spritesheet.webp")).unwrap(), pet_sheet(9));
    let aside = f.root.join("seeded-before-20261126-100001");
    assert!(read(aside.join("pets/macintosh/pet.json")).contains("Old Macintosh"), "the old pet is kept, not deleted");
    assert_eq!(done.copied, ["pets/macintosh"]);
    assert_eq!(done.moved_aside, ["pets/macintosh"]);
    assert_eq!(done.aside.as_deref(), Some(aside.as_path()));
    assert_eq!(done.unchanged, ["pets/nori"], "a pet already the same is not moved aside");
    assert!(!aside.join("pets/nori").exists());
    assert!(f.root.join("pets/only-here/pet.json").exists(), "a pet only Rust has stays");
    assert_eq!(f.electron_as_it_is(), before, "Electron's folder is only read");
}

#[test]
fn a_pet_that_does_not_read_is_not_carried_and_the_rust_one_stays() {
    let f = Folders::new("petsbad");
    let electron = f.temp.join("electron");
    put_pet(&electron, "macintosh", "Macintosh", 1);
    // A sheet that is not a WebP, and a pet.json that names another pet.
    fs::write(electron.join("pets/macintosh/spritesheet.webp"), b"not a webp").unwrap();
    put_pet(&electron, "nori", "Nori", 2);
    fs::write(electron.join("pets/nori/pet.json"), r#"{"id":"someone-else","displayName":"Nori","spritesheetPath":"spritesheet.webp"}"#).unwrap();
    put_pet(&f.temp.join("native"), "macintosh", "Kept", 7);

    let done = f.store().reseed_from_electron("20261126-100002").unwrap();

    assert!(done.copied.is_empty() && done.aside.is_none(), "{done:?}");
    assert!(fs::read_to_string(f.root.join("pets/macintosh/pet.json")).unwrap().contains("Kept"));
    assert!(!f.root.join("pets/nori").exists());
    assert!(!f.root.join("seeded-before-20261126-100002").exists());
}

#[test]
fn a_pet_that_is_a_link_in_electrons_folder_is_not_followed() {
    let f = Folders::new("petslink");
    let electron = f.temp.join("electron");
    put_pet(&f.temp, "outside", "Outside", 6);
    fs::create_dir_all(electron.join("pets")).unwrap();
    std::os::unix::fs::symlink(f.temp.join("pets/outside"), electron.join("pets/outside")).unwrap();

    let done = f.store().reseed_from_electron("20261126-100003").unwrap();

    assert!(done.copied.is_empty(), "{done:?}");
    assert!(!f.root.join("pets/outside").exists());
}

/// Lane check (fix-b): a pet folder that is real but whose pet.json is a link to a file elsewhere is not carried: the
/// copy reads only plain files of a pet's size from Electron's folder, never what a link points at.
#[test]
fn a_pet_whose_pet_json_is_a_link_is_not_carried() {
    let f = Folders::new("petsjsonlink");
    let electron = f.temp.join("electron");
    put_pet(&electron, "linked", "Linked", 8);
    let elsewhere = f.temp.join("elsewhere-pet.json");
    fs::rename(electron.join("pets/linked/pet.json"), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, electron.join("pets/linked/pet.json")).unwrap();

    let done = f.store().reseed_from_electron("20261126-100004").unwrap();

    assert!(!done.copied.iter().any(|c| c == "pets/linked"), "{done:?}");
    assert!(!f.root.join("pets/linked").exists(), "nothing behind a link is copied");
}

#[test]
fn the_door_and_from_anywhere_switches_come_across_and_each_reader_sees_them_on() {
    let f = Folders::new("mcp");
    fs::write(f.electron.join("locust-mcp.json"), MCP_ON).unwrap();
    fs::write(f.electron.join("remote-mcp.json"), remote_on()).unwrap();
    fs::write(f.root.join("locust-mcp.json"), MCP_OFF).unwrap();
    let before = f.electron_as_it_is();
    let remote = |file: PathBuf| {
        let options = crate::remote_mcp::Options::new(
            file,
            Arc::new(crate::remote_mcp::StubMcpRelay),
            Arc::new(|_: &str, _: &serde_json::Value| crate::door_tools::ToolResult::refused("No tools here.")),
        );
        crate::remote_mcp::RemoteMcp::new(options).load()
    };
    assert!(!remote(f.root.join("remote-mcp.json")).enabled, "Off before: nothing to read");

    let done = f.store().reseed_from_electron("20261126-100004").unwrap();

    let door: serde_json::Value = serde_json::from_str(&fs::read_to_string(f.root.join("locust-mcp.json")).unwrap()).unwrap();
    assert_eq!(door["conversations"], true, "Let apps see your conversations");
    assert_eq!(door["ownMode"], true);
    let state = remote(f.root.join("remote-mcp.json"));
    assert!(state.enabled, "Use from anywhere");
    assert_eq!(state.apps.len(), 1);
    assert_eq!(state.apps[0].name, "Claude");
    assert_eq!(done.copied, ["locust-mcp.json", "remote-mcp.json"]);
    assert_eq!(done.moved_aside, ["locust-mcp.json"]);
    assert_eq!(fs::read_to_string(f.root.join("seeded-before-20261126-100004/locust-mcp.json")).unwrap(), MCP_OFF);
    assert_eq!(f.electron_as_it_is(), before);
}

#[test]
fn sign_in_and_key_files_are_never_carried() {
    // Own models' keys, agent accounts and the teammates' keys stay where they are made: the seed names none of them.
    let f = Folders::new("secrets");
    for name in ["own-models.json", "teammate-keys.json", "account-sync.json", "phone.json", "phone-relay.json"] {
        fs::write(f.electron.join(name), r#"{"secret":"x"}"#).unwrap();
    }
    fs::create_dir_all(f.electron.join("agent-accounts/one")).unwrap();
    fs::write(f.electron.join("agent-accounts/one/auth.json"), r#"{"token":"x"}"#).unwrap();

    let done = f.store().reseed_from_electron("20261126-100005").unwrap();

    assert!(done.copied.is_empty(), "{done:?}");
    for name in ["own-models.json", "teammate-keys.json", "account-sync.json", "phone.json", "phone-relay.json", "agent-accounts"] {
        assert!(!f.root.join(name).exists(), "{name}");
    }
}
