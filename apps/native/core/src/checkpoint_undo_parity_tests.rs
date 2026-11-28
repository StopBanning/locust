//! Cases from Electron's a-turn-can-be-undone{,-without-git}.test.ts.
use super::*;

const CHANGED: [&str; 3] = ["gone.md", "made.md", "notes.txt"];
const BEFORE: &[u8] = b"one\r\ntwo\r\n";
const AFTER: &[u8] = b"one\r\nTWO\r\n";
// Electron checkpoints.ts MAX_KEPT_FILE_BYTES.
const ELECTRON_MAX_BYTES: u64 = 10 * 1024 * 1024;

struct Workspace {
    _temp: tests::Temp,
    store: Store,
    path: PathBuf,
    git: bool,
}

impl Workspace {
    fn new(git: bool) -> Self {
        let (temp, store, path) = tests::setup();
        fs::remove_file(path.join("notes")).unwrap();
        fs::remove_file(path.join("gone")).unwrap();
        fs::write(path.join("notes.txt"), BEFORE).unwrap();
        fs::write(path.join("gone.md"), b"here before\n").unwrap();
        fs::create_dir(path.join("sub")).unwrap();
        fs::write(path.join("sub/keep.md"), b"keep\n").unwrap();
        fs::create_dir(path.join("node_modules")).unwrap();
        fs::write(path.join("node_modules/big.js"), b"ignored\n").unwrap();
        if git {
            fs::write(path.join(".gitignore"), b"node_modules/\n").unwrap();
        } else {
            fs::create_dir(path.join(".locust")).unwrap();
            fs::write(path.join(".locust/memory.md"), b"remembered\n").unwrap();
        }
        Self {
            _temp: temp,
            store,
            path,
            git,
        }
    }

    fn begin(&self) -> Turn {
        if self.git {
            Turn::begin(&self.store, &self.path)
        } else {
            Turn::begin_with(&self.store, &self.path, take_plain)
        }
    }

    fn finish(&self, turn: Turn, id: &str) -> Option<Vec<String>> {
        let (_, changed) = if self.git {
            turn.finish_reporting_changes(&self.store, id)
        } else {
            turn.finish_with(&self.store, id, take_plain)
        }
        .unwrap();
        changed
    }

    fn edit(&self) {
        fs::write(self.path.join("notes.txt"), AFTER).unwrap();
        fs::write(self.path.join("made.md"), b"new\n").unwrap();
        fs::remove_file(self.path.join("gone.md")).unwrap();
        if !self.git {
            fs::write(self.path.join(".locust/memory.md"), b"remembered more\n").unwrap();
        }
    }

    fn run(&self) -> Vec<String> {
        let turn = self.begin();
        self.edit();
        self.finish(turn, "run_1").unwrap()
    }

    #[cfg(unix)]
    fn undo(&self) -> Value {
        undo(&self.store, &self.path, "run_1").unwrap()
    }

    #[cfg(unix)]
    fn assert_restored(&self) {
        assert_eq!(self.undo(), json!({"restored": CHANGED, "leftAlone": []}));
        assert_eq!(fs::read(self.path.join("notes.txt")).unwrap(), BEFORE);
        assert_eq!(
            fs::read(self.path.join("gone.md")).unwrap(),
            b"here before\n"
        );
        assert!(!self.path.join("made.md").exists());
        assert_eq!(fs::read(self.path.join("sub/keep.md")).unwrap(), b"keep\n");
    }

    fn large(&self, name: &str) {
        fs::File::create(self.path.join(name))
            .unwrap()
            .set_len(ELECTRON_MAX_BYTES + 1)
            .unwrap();
    }
}

fn take_plain(store: &Store, workspace: &Path) -> io::Result<Snapshot> {
    take_with_git(store, workspace, false)
}

// case: names what the turn changed, and nothing git ignores
#[test]
fn names_changes_and_respects_gitignore() {
    let ws = Workspace::new(true);
    let turn = ws.begin();
    ws.edit();
    fs::write(
        ws.path.join("node_modules/big.js"),
        b"changed but ignored\n",
    )
    .unwrap();
    assert_eq!(ws.finish(turn, "run_1").unwrap(), CHANGED);
}

// case: puts every file back, byte for byte: the edit, the new file, the removed one
#[cfg(unix)]
#[test]
fn restores_edited_created_and_deleted_files_byte_for_byte() {
    let ws = Workspace::new(true);
    ws.run();
    ws.assert_restored();
}

// case: leaves alone a file changed since the turn, and says so
#[cfg(unix)]
#[test]
fn leaves_a_persons_later_edit_and_says_why() {
    let ws = Workspace::new(true);
    ws.run();
    let later = b"the person wrote this after\n";
    fs::write(ws.path.join("notes.txt"), later).unwrap();
    assert_eq!(
        ws.undo(),
        json!({
            "restored": ["gone.md", "made.md"],
            "leftAlone": [{"path": "notes.txt", "why": "changed since the turn"}]
        })
    );
    assert_eq!(fs::read(ws.path.join("notes.txt")).unwrap(), later);
}

// case: keeps nothing of the person's repository: the store is under its own root
#[test]
fn keeps_checkpoints_outside_the_workspace() {
    let ws = Workspace::new(true);
    ws.run();
    assert!(!ws.path.join(".git").exists());
    assert!(
        ws.store
            .ledger_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join("checkpoints")
            .is_dir()
    );
}

// case: names a file too large to keep instead of keeping it
#[test]
fn git_listing_names_but_does_not_keep_an_oversized_file() {
    let ws = Workspace::new(true);
    ws.large("data.bin");
    let kept = take(&ws.store, &ws.path).unwrap();
    assert_eq!(kept.not_kept, ["data.bin"]);
    assert!(!kept.files.contains_key("data.bin"));
}

// case: is ready with its count, then undone once, and says what it did
#[cfg(unix)]
#[test]
fn turn_record_is_ready_then_reports_the_same_undo_on_a_second_press() {
    let ws = Workspace::new(true);
    ws.run();
    assert_eq!(
        undo_state(&ws.store, &ws.path, "run_1"),
        UndoState::Ready {
            files: 3,
            not_kept: vec![]
        }
    );
    let receipt = ws.undo();
    assert_eq!(receipt, json!({"restored": CHANGED, "leftAlone": []}));
    assert_eq!(
        undo_state(&ws.store, &ws.path, "run_1"),
        UndoState::Undone {
            restored: CHANGED.map(str::to_owned).to_vec(),
            left_alone: vec![],
        }
    );
    let later = b"made again by hand\n";
    fs::write(ws.path.join("made.md"), later).unwrap();
    assert_eq!(ws.undo(), receipt);
    assert_eq!(fs::read(ws.path.join("made.md")).unwrap(), later);
}

// case: a turn that shared its folder with another run is not offered
#[test]
fn neither_overlapping_turn_offers_undo() {
    let ws = Workspace::new(true);
    let first = ws.begin();
    let second = ws.begin();
    ws.edit();
    assert_eq!(ws.finish(first, "run_1"), None);
    assert_eq!(ws.finish(second, "run_2"), None);
    for id in ["run_1", "run_2"] {
        assert_eq!(undo_state(&ws.store, &ws.path, id), UndoState::Shared);
        assert!(undo(&ws.store, &ws.path, id).is_err());
        assert!(undone(&ws.store, &ws.path, id).is_none());
    }
    assert_eq!(fs::read(ws.path.join("notes.txt")).unwrap(), AFTER);
    assert!(ws.path.join("made.md").is_file());
    assert!(!ws.path.join("gone.md").exists());
}

// case: is kept and undone, byte for byte, without git
#[cfg(unix)]
#[test]
fn restores_plain_checkpoint_bytes_and_keeps_excluded_files() {
    let ws = Workspace::new(false);
    assert_eq!(ws.run(), CHANGED);
    assert_eq!(
        file_at(&ws.store, &ws.path, "run_1", "notes.txt").as_deref(),
        Some("one\r\ntwo\r\n")
    );
    ws.assert_restored();
    assert_eq!(
        fs::read(ws.path.join(".locust/memory.md")).unwrap(),
        b"remembered more\n"
    );
    assert!(
        !base(&ws.store, &ws.path)
            .unwrap()
            .join("listing.git")
            .exists()
    );
}

// case: leaves alone a file changed since the turn, and names it
#[cfg(unix)]
#[test]
fn plain_undo_names_and_keeps_a_persons_later_edit() {
    let ws = Workspace::new(false);
    ws.run();
    let later = b"the person wrote this\n";
    fs::write(ws.path.join("notes.txt"), later).unwrap();
    assert_eq!(
        ws.undo()["leftAlone"],
        json!([{"path": "notes.txt", "why": "changed since the turn"}])
    );
    assert_eq!(fs::read(ws.path.join("notes.txt")).unwrap(), later);
}

// case: names a file too large to keep, and keeps the rest
#[test]
fn plain_checkpoint_names_an_oversized_file_and_keeps_the_rest() {
    let ws = Workspace::new(false);
    ws.large("video.bin");
    let kept = take_plain(&ws.store, &ws.path).unwrap();
    assert_eq!(kept.not_kept, ["video.bin"]);
    let sub = Path::new("sub").join("keep.md");
    assert_eq!(
        kept.files.keys().map(String::as_str).collect::<Vec<_>>(),
        ["gone.md", "notes.txt", sub.to_str().unwrap()]
    );
    ws.run();
    assert_eq!(
        undo_state(&ws.store, &ws.path, "run_1"),
        UndoState::Ready {
            files: 3,
            not_kept: vec!["video.bin".into()]
        }
    );
    #[cfg(unix)]
    ws.assert_restored();
    assert_eq!(
        fs::metadata(ws.path.join("video.bin")).unwrap().len(),
        ELECTRON_MAX_BYTES + 1
    );
}

// case: gives the same folder the same name, and reads back no binary as text
#[test]
fn plain_checkpoint_reuses_content_names_and_reads_only_text() {
    let ws = Workspace::new(false);
    fs::write(ws.path.join("image.png"), [0x89, 0x50, 0x00, 0x47]).unwrap();
    let first = ws.begin();
    assert_eq!(ws.finish(first, "run_1"), Some(vec![]));
    let record = records(&ws.store, &ws.path).unwrap().pop().unwrap().1;
    // Native names the blobs by content and stores the maps in the turn record.
    assert_eq!(record["before"], record["after"]);
    assert_eq!(file_at(&ws.store, &ws.path, "run_1", "image.png"), None);
    let sub = Path::new("sub").join("keep.md");
    assert_eq!(
        file_at(&ws.store, &ws.path, "run_1", sub.to_str().unwrap()).as_deref(),
        Some("keep\n")
    );
}

// case: is undone through the turn record, as a turn with git is
#[cfg(unix)]
#[test]
fn plain_turn_record_offers_undo_and_restores_a_deleted_file() {
    let ws = Workspace::new(false);
    ws.run();
    assert_eq!(
        undo_state(&ws.store, &ws.path, "run_1"),
        UndoState::Ready {
            files: 3,
            not_kept: vec![]
        }
    );
    ws.undo();
    assert!(undo_state(&ws.store, &ws.path, "run_1").put_back());
    assert_eq!(fs::read(ws.path.join("gone.md")).unwrap(), b"here before\n");
}

// case: is git wherever git runs, exactly as before
#[test]
fn uses_git_ignore_rules_when_git_is_available() {
    let ws = Workspace::new(true);
    fs::write(ws.path.join(".gitignore"), b"scratch.txt\n").unwrap();
    fs::write(ws.path.join("scratch.txt"), b"ignored by git only\n").unwrap();
    let kept = take(&ws.store, &ws.path).unwrap();
    assert!(!kept.files.contains_key("scratch.txt"));
    assert!(kept.files.contains_key("notes.txt"));
}
