//! Electron's `an-app-names-a-folder-locust-knows.test.ts` (5 cases), with real folders and a real `git worktree add`,
//! through `check_app_folder` and git's own common directory (`common_dir`), as Electron's `commonDir` asks git.
use super::*;

struct Place {
    root: PathBuf,
    project: PathBuf,
    worktree: PathBuf,
    stranger: PathBuf,
}
impl Drop for Place {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn git(args: &[&str], cwd: &Path) {
    let out = std::process::Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Electron's `beforeAll`: a project with one commit, a worktree of it under locust-wt, and a stranger's repository.
fn place() -> Place {
    let made = std::env::temp_dir().join(format!("locust-app-folder-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&made).unwrap();
    let root = made.canonicalize().unwrap();
    let (project, worktree, stranger) = (root.join("now-coding"), root.join("locust-wt").join("readme-pass"), root.join("someone-else"));
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&stranger).unwrap();
    git(&["init", "-q", "-b", "main"], &project);
    std::fs::write(project.join("README.md"), "hi\n").unwrap();
    git(&["-c", "user.email=t@example.invalid", "-c", "user.name=t", "add", "README.md"], &project);
    git(&["-c", "user.email=t@example.invalid", "-c", "user.name=t", "commit", "-q", "-m", "one"], &project);
    std::fs::create_dir_all(root.join("locust-wt")).unwrap();
    git(&["worktree", "add", "-q", "-b", "exec/readme-pass", worktree.to_str().unwrap(), "HEAD"], &project);
    git(&["init", "-q", "-b", "main"], &stranger);
    Place { root, project, worktree, stranger }
}

fn check(p: &Place, folder: &Path) -> Result<PathBuf, String> {
    check_app_folder(folder.to_str().unwrap(), std::slice::from_ref(&p.project), &common_dir)
}

#[test]
fn is_taken_when_it_is_the_open_folder_or_inside_it() {
    let p = place();
    assert_eq!(check(&p, &p.project).unwrap(), p.project);
    assert_eq!(check(&p, &p.project.join("src")).unwrap(), p.project.join("src"));
}

#[test]
fn is_taken_when_it_is_a_worktree_of_the_open_folders_repository_wherever_it_was_made() {
    let p = place();
    assert_eq!(check(&p, &p.worktree).unwrap(), p.worktree);
}

#[test]
fn is_refused_when_locust_never_knew_it_even_a_repository_of_its_own() {
    let p = place();
    assert!(check(&p, &p.stranger).unwrap_err().contains("is not a folder Locust knows"));
}

#[test]
fn is_refused_when_it_is_the_folder_above_a_known_one_a_relative_path_missing_or_a_file() {
    let p = place();
    assert!(check(&p, &p.root).is_err());
    assert_eq!(check_app_folder("now-coding", std::slice::from_ref(&p.project), &common_dir).unwrap_err(), "folder is a full path, from the top of the disk.");
    let gone = p.root.join("gone");
    assert_eq!(check(&p, &gone).unwrap_err(), format!("There is no folder at {}.", gone.display()));
    let file = p.project.join("README.md");
    assert_eq!(check(&p, &file).unwrap_err(), format!("{} is a file, not a folder.", file.display()));
}

#[test]
fn is_refused_when_a_path_climbs_out_of_a_known_folder() {
    let p = place();
    assert!(check(&p, &p.project.join("..").join("someone-else")).is_err());
}
