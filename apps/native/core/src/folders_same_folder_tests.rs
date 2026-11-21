//! A FOLDER IS ONE FOLDER HOWEVER IT IS SPELLED (Electron `a-folder-is-one-folder-however-spelled.test.ts`, QA-2026-09-29
//! round 2, N13). The id hashes the path as written, so a link to a project was a second folder with none of the
//! conversations. The known folder is found by what it is on disk, and its first spelling (and so its id) is kept.
//! (Electron's Windows-only "in other letters" case does not run here: no Windows.)
use super::*;
use std::path::PathBuf;

struct Fixture {
    root: PathBuf,
    store: Store,
    real: PathBuf,
    other: PathBuf,
    link: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture() -> Fixture {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!("locust-same-folder-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&root);
    let electron = root.join("electron");
    std::fs::create_dir_all(&electron).unwrap();
    let store = Store::new(root.join("native"), electron).unwrap();
    let (real, other, link) = (root.join("Workspace"), root.join("Elsewhere"), root.join("alpha-link"));
    std::fs::create_dir_all(&real).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let real = absolute_path(&real).unwrap();
    let other = absolute_path(&other).unwrap();
    let link = absolute_path(&link).unwrap();
    use_folder(&store, &real, "2026-11-21T12:00:00.000Z").unwrap();
    Fixture { root, store, real, other, link }
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn file(f: &Fixture) -> PathBuf {
    f.store.root().join(FOLDERS_FILE)
}

// case: is the known folder, spelled as it was first known
#[test]
fn is_the_known_folder_spelled_as_it_was_first_known() {
    let f = fixture();
    let found = same_as(&f.store, &f.link).unwrap();
    assert_eq!(found, text(&f.real));
    assert_eq!(workspace_id(Path::new(&found)), workspace_id(&f.real));
    assert_ne!(workspace_id(&f.link), workspace_id(&f.real), "the link's own spelling is another id");
}

// case: and at start, before the registry is open
#[test]
fn and_at_start_before_the_registry_is_open() {
    let f = fixture();
    assert_eq!(same_folder_at_start(&file(&f), &f.link), text(&f.real));
}

// case: a different folder stays itself
#[test]
fn a_different_folder_stays_itself() {
    let f = fixture();
    assert_eq!(same_as(&f.store, &f.other).unwrap(), text(&f.other));
    assert_eq!(same_folder_at_start(&file(&f), &f.other), text(&f.other));
}

// case: a folder that is not there, or no registry yet, is left as written
#[test]
fn a_folder_that_is_not_there_or_no_registry_yet_is_left_as_written() {
    let f = fixture();
    let gone = f.root.join("gone");
    assert_eq!(same_as(&f.store, &gone).unwrap(), text(&absolute_path(&gone).unwrap()));
    assert_eq!(same_folder_at_start(&f.root.join("none.json"), &f.link), text(&f.link));
}
