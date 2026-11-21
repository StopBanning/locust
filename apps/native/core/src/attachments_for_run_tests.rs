//! Electron's `main/attachments-for-run.test.ts` ("attached files, placed where the run reads"), one Rust test for each
//! of its six cases, in its order, with its files and words. Some carry more than Electron asks, so that breaking a
//! rule fails a test: a second file of the same name, the exclude file, and a real file outside the project.
use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project {
    /// The test's own folder, holding the project and a file outside it.
    outer: PathBuf,
    root: PathBuf,
    tree: PathBuf,
    own: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.outer);
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn project() -> Project {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let outer = std::env::temp_dir().join(format!("locust-attach-run-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&outer);
    let root = outer.join("project");
    let tree = root.join(".locust").join("worktrees").join("tm_wren");
    let own = root.join("elsewhere");
    std::fs::create_dir_all(root.join(".locust").join("attachments")).unwrap();
    std::fs::create_dir_all(tree.join("src")).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(&own).unwrap();
    write(&root.join(".locust/attachments/shot.png"), "PNG");
    write(&root.join("src/app.ts"), "project copy");
    write(&tree.join("src/app.ts"), "branch copy");
    write(&root.join("NOTES.md"), "notes");
    write(&outer.join("secret.txt"), "secret");
    Project { outer, root, tree, own }
}

fn files(of: &[&str]) -> Vec<String> {
    of.iter().map(|p| (*p).to_owned()).collect()
}

// case: changes nothing for a run in the project folder
#[test]
fn changes_nothing_for_a_run_in_the_project_folder() {
    let p = project();
    let prompt = with_attachments("Look at this.", &files(&[".locust/attachments/shot.png"]));
    assert_eq!(attachments_for_run(&prompt, &p.root, &p.root), prompt);
    // Named the same way with a trailing `.`, it is still the project folder.
    assert_eq!(attachments_for_run(&prompt, &p.root, &p.root.join(".")), prompt);
    // Nothing is copied onto itself, and a repository's exclude file is not touched.
    std::fs::create_dir_all(p.root.join(".git")).unwrap();
    attachments_for_run(&with_attachments("Look at these.", &files(&[".locust/attachments/shot.png", "NOTES.md"])), &p.root, &p.root);
    assert_eq!(read(&p.root.join(".locust/attachments/shot.png")), "PNG");
    assert!(!p.root.join(".git/info/exclude").exists());
}

// case: copies a pasted file into a worktree at the same path
#[test]
fn copies_a_pasted_file_into_a_worktree_at_the_same_path() {
    let p = project();
    let prompt = with_attachments("Look at this.", &files(&[".locust/attachments/shot.png"]));
    assert_eq!(attachments_for_run(&prompt, &p.root, &p.tree), prompt);
    assert_eq!(read(&p.tree.join(".locust/attachments/shot.png")), "PNG");
}

// case: leaves a project file the worktree has alone: its own branch’s copy
#[test]
fn leaves_a_project_file_the_worktree_has_alone_its_own_branchs_copy() {
    let p = project();
    let prompt = with_attachments("Fix this.", &files(&["src/app.ts"]));
    assert_eq!(attachments_for_run(&prompt, &p.root, &p.tree), prompt);
    assert_eq!(read(&p.tree.join("src/app.ts")), "branch copy");
    assert!(!p.tree.join(".locust/attachments/app.ts").exists());
}

// case: copies a project file a folder of its own lacks, and names the copy
#[test]
fn copies_a_project_file_a_folder_of_its_own_lacks_and_names_the_copy() {
    let p = project();
    let sent = attachments_for_run(&with_attachments("Read these.", &files(&["NOTES.md", "src/app.ts"])), &p.root, &p.own);
    assert_eq!(sent, with_attachments("Read these.", &files(&[".locust/attachments/NOTES.md", ".locust/attachments/app.ts"])));
    assert_eq!(read(&p.own.join(".locust/attachments/NOTES.md")), "notes");
    assert_eq!(read(&p.own.join(".locust/attachments/app.ts")), "project copy");

    // Two files of one name get two copies, the second numbered; and in a repository, `.locust/` is kept out of its
    // commits through `.git/info/exclude`, once.
    let p = project();
    write(&p.root.join("lib/app.ts"), "lib copy");
    std::fs::create_dir_all(p.own.join(".git")).unwrap();
    write(&p.own.join(".git/info/exclude"), "# mine");
    let sent = attachments_for_run(&with_attachments("Read these.", &files(&["src/app.ts", "lib/app.ts"])), &p.root, &p.own);
    assert_eq!(sent, with_attachments("Read these.", &files(&[".locust/attachments/app.ts", ".locust/attachments/app-2.ts"])));
    assert_eq!(read(&p.own.join(".locust/attachments/app.ts")), "project copy");
    assert_eq!(read(&p.own.join(".locust/attachments/app-2.ts")), "lib copy");
    assert_eq!(read(&p.own.join(".git/info/exclude")), "# mine\n.locust/\n");
    attachments_for_run(&with_attachments("Again.", &files(&["NOTES.md"])), &p.root, &p.own);
    assert_eq!(read(&p.own.join(".git/info/exclude")), "# mine\n.locust/\n");
}

// case: never reads outside the project for a path that climbs out of it
#[test]
fn never_reads_outside_the_project_for_a_path_that_climbs_out_of_it() {
    let p = project();
    let prompt = with_attachments("Read this.", &files(&["../../secret.txt"]));
    assert_eq!(attachments_for_run(&prompt, &p.root, &p.own), prompt);
    // One that climbs to a file that IS there is not copied either.
    let prompt = with_attachments("Read this.", &files(&["../secret.txt"]));
    assert_eq!(attachments_for_run(&prompt, &p.root, &p.own), prompt);
    assert!(!p.own.join(".locust/attachments/secret.txt").exists());
    // Nor is a path named from the root of the disk.
    let outside = p.outer.join("secret.txt").to_string_lossy().into_owned();
    let prompt = with_attachments("Read this.", &[outside]);
    assert_eq!(attachments_for_run(&prompt, &p.root, &p.own), prompt);
    assert!(!p.own.join(".locust/attachments/secret.txt").exists());
}

// case: leaves a message without attachments exactly as typed
#[test]
fn leaves_a_message_without_attachments_exactly_as_typed() {
    let p = project();
    assert_eq!(attachments_for_run("Just words.", &p.root, &p.tree), "Just words.");
}
