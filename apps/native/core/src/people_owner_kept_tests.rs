//! speed-1 row 5: a conversation's owner is read from the teammates files once while they are unchanged, and a change is
//! seen at once.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Temp, Store) {
    let path = std::env::temp_dir().join(id("native-owner-kept").unwrap());
    std::fs::create_dir_all(path.join("electron")).unwrap();
    let store = Store::new(path.join("native"), path.join("electron")).unwrap();
    (Temp(path), store)
}

fn reads() -> usize {
    KEPT_READS.with(std::cell::Cell::get)
}

#[test]
fn a_row_s_owner_asked_every_frame_is_read_from_disk_once() {
    let (_t, store) = fixture();
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[],"missionOwners":{"m_old":"tm_codec"}}).to_string(),
    )
    .unwrap();
    record_owner(&store, "m_new", "tm_scout").unwrap();
    let before = reads();
    for _ in 0..200 {
        assert_eq!(owner_of(&store, "m_new").as_deref(), Some("tm_scout"));
        assert_eq!(owner_of(&store, "m_old").as_deref(), Some("tm_codec"), "Electron's owners, read only");
        assert_eq!(owner_of(&store, "m_nobody"), None);
    }
    assert_eq!(reads() - before, 2, "each file once: the native one and Electron's");
}

#[test]
fn a_changed_teammates_file_is_seen_at_once() {
    let (_t, store) = fixture();
    record_owner(&store, "m_one", "tm_aaaa").unwrap();
    assert_eq!(owner_of(&store, "m_one").as_deref(), Some("tm_aaaa"));
    // The same length, so only the file's time tells the change.
    record_owner(&store, "m_one", "tm_bbbb").unwrap();
    assert_eq!(owner_of(&store, "m_one").as_deref(), Some("tm_bbbb"));
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[],"missionOwners":{"m_two":"tm_cccc"}}).to_string(),
    )
    .unwrap();
    assert_eq!(owner_of(&store, "m_two").as_deref(), Some("tm_cccc"));
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[],"missionOwners":{"m_two":"tm_dddd"}}).to_string(),
    )
    .unwrap();
    assert_eq!(owner_of(&store, "m_two").as_deref(), Some("tm_dddd"), "Electron's file changed in place");
}

#[test]
fn a_native_teammates_file_of_another_version_names_no_owner() {
    let (_t, store) = fixture();
    std::fs::create_dir_all(store.root()).unwrap();
    std::fs::write(store.root().join("teammates.json"), json!({"schemaVersion":2,"teammates":[],"missionOwners":{"m_one":"tm_new"}}).to_string()).unwrap();
    std::fs::write(
        store.electron().join("teammates.json"),
        json!({"schemaVersion":1,"teammates":[],"missionOwners":{"m_one":"tm_old"}}).to_string(),
    )
    .unwrap();
    assert_eq!(owner_of(&store, "m_one").as_deref(), Some("tm_old"), "a file this version cannot read is not believed");
}
