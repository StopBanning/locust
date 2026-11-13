//! Electron's worktrees.test.ts and a-turn-is-committed-on-its-own-branch.test.ts, against real git in temporary
//! repositories. Nothing here touches a repository outside the temporary folder.
use super::*;

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(prefix: &str) -> Temp {
    let p = std::env::temp_dir().join(format!("{prefix}-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&p).unwrap();
    Temp(std::fs::canonicalize(&p).unwrap())
}

fn git(args: &[&str], cwd: &Path) -> String {
    let out = std::process::Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A repository with one commit on `main`, a person of its own, and `cart.py`.
fn repository() -> Temp {
    let t = temp("locust-turns");
    git(&["init", "-q", "-b", "main"], &t.0);
    git(&["config", "user.email", "person@locust.test"], &t.0);
    git(&["config", "user.name", "The person"], &t.0);
    std::fs::write(t.0.join("cart.py"), "def total(items):\n    return 0\n").unwrap();
    git(&["add", "."], &t.0);
    git(&["commit", "-q", "-m", "first"], &t.0);
    t
}

fn message(subject: &str) -> CheckpointMessage {
    CheckpointMessage {
        subject: subject.into(),
        body: None,
        trailers: vec![("Locust-Mission".into(), "m_1".into()), ("Locust-Route".into(), "opencode / nemotron".into())],
        author_name: "Wren".into(),
        author_email: "tm_wren@teammates.locust".into(), // made-up teammate author; public-guard: allow
    }
}

#[test]
fn names_a_branch_from_the_teammate_folded_to_what_git_accepts() {
    assert_eq!(branch_name_for("Wren", None), "locust/wren");
    assert_eq!(branch_name_for("  Booty Call! ", None), "locust/booty-call");
    assert_eq!(branch_name_for("...", None), "locust/teammate");
    // A name that folds to nothing is named by the teammate instead, so two such teammates never share a branch.
    assert_eq!(branch_name_for("小明", Some("tm_1a2b3c4d")), "locust/tm_1a2b3c4d");
    assert_eq!(branch_name_for("小红", Some("tm_9f8e7d6c")), "locust/tm_9f8e7d6c");
    assert_eq!(branch_name_for("Wren", Some("tm_1a2b3c4d")), "locust/wren");
    assert!(branch_name_for(&"x".repeat(80), None).len() <= "locust/".len() + 40);
    assert_eq!(distinct_branch_name_for("Dev-1", "tm_dev_one_b2"), "locust/dev-1-dev_one_b2");
}

#[test]
fn reads_the_git_version_and_knows_which_ones_have_worktrees() {
    assert_eq!(parse_git_version("git version 2.45.1.windows.1\n").as_deref(), Some("2.45.1"));
    assert!(git_version_supports_worktrees("2.45.1"));
    assert!(!git_version_supports_worktrees("2.4.9"));
    assert!(git_version_supports_worktrees("3.0"));
}

#[test]
fn keeps_only_the_trees_locust_made_under_the_folder_out_of_a_porcelain_list() {
    let output = [
        "worktree /w/shop", "HEAD abc", "branch refs/heads/main", "",
        "worktree /w/shop/.locust/worktrees/tm_wren", "HEAD def", "branch refs/heads/locust/wren", "",
        "worktree /elsewhere/other", "HEAD 123", "branch refs/heads/feature", "",
    ]
    .join("\n");
    assert_eq!(
        parse_worktree_list(&output, Path::new("/w/shop"), WORKTREE_DIR),
        vec![WorktreeInfo { teammate_id: "tm_wren".into(), path: "/w/shop/.locust/worktrees/tm_wren".into(), branch: "locust/wren".into() }]
    );
}

#[test]
fn names_changes_the_way_git_status_does_a_quoted_path_unquoted() {
    assert_eq!(changed_paths_of(" M NOTES.md\n?? REPORT.md\r\n?? \"with space.md\"\n"), vec!["NOTES.md", "REPORT.md", "with space.md"]);
    assert!(changed_paths_of("").is_empty());
}

#[test]
fn reads_status_z_a_renames_old_name_skipped() {
    let got = status_entries_of(" M cart.py\0?? new file.txt\0R  b.py\0a.py\0");
    assert_eq!(got, vec![(" M".into(), "cart.py".into()), ("??".into(), "new file.txt".into()), ("R ".into(), "b.py".into())]);
}

#[test]
fn reads_the_turns_of_a_log() {
    let sha = "a".repeat(40);
    let merge = "b".repeat(40);
    let log = format!("\x1e{sha}\x1fTurn one\x1f2026-11-12T10:00:00Z\x1fp1\n\ncart.py\nnotes.md\n\x1e{merge}\x1fMerge\x1f2026-11-12T11:00:00Z\x1fp1 p2\n\x1enot-a-sha\x1fx\x1fy\x1fz\n");
    let turns = branch_turns_of(&log);
    assert_eq!(turns.len(), 2);
    assert_eq!((turns[0].subject.as_str(), turns[0].files.clone(), turns[0].merge), ("Turn one", vec!["cart.py".to_string(), "notes.md".to_string()], false));
    assert!(turns[1].merge);
}

#[test]
fn refuses_a_folder_that_is_not_a_repository_with_the_reason() {
    let plain = temp("locust-notrepo");
    let manager = Manager::new(&plain.0);
    let probe = manager.probe();
    assert!(!probe.repository);
    assert_eq!(probe.reason.as_deref(), Some("The project folder is not a git repository."));
    assert!(manager.ensure("tm_wren", "Wren", None).unwrap_err().contains("not a git repository"));
}

#[test]
fn makes_the_tree_under_locust_worktrees_on_its_own_branch_excludes_locust_and_is_idempotent() {
    let root = repository();
    let manager = Manager::new(&root.0);
    assert!(manager.probe().repository);
    let path = manager.ensure("tm_wren", "Wren", None).unwrap();
    assert_eq!(path, root.0.join(".locust/worktrees/tm_wren"));
    assert!(path.join("cart.py").is_file());
    assert_eq!(git(&["rev-parse", "--abbrev-ref", "HEAD"], &path).trim(), "locust/wren");
    assert!(std::fs::read_to_string(root.0.join(".git/info/exclude")).unwrap().contains(".locust/"));
    // The person's checkout does not see the new folder as untracked.
    assert_eq!(git(&["status", "--porcelain"], &root.0).trim(), "");
    assert_eq!(manager.ensure("tm_wren", "Wren", None).unwrap(), path);
    let listed: Vec<(String, String)> = manager.list().into_iter().map(|w| (w.teammate_id, w.branch)).collect();
    assert_eq!(listed, vec![("tm_wren".to_string(), "locust/wren".to_string())]);
}

#[test]
fn two_teammates_whose_names_fold_to_one_branch_each_get_a_tree() {
    let root = repository();
    let manager = Manager::new(&root.0);
    manager.ensure("tm_dev_one_a1", "Dev 1", None).unwrap();
    let second = manager.ensure("tm_dev_one_b2", "Dev-1", None).unwrap();
    assert_eq!(git(&["rev-parse", "--abbrev-ref", "HEAD"], &second).trim(), "locust/dev-1-dev_one_b2");
    manager.ensure("tm_ming_c3", "小明", None).unwrap();
    manager.ensure("tm_hong_d4", "小红", None).unwrap();
    assert_eq!(manager.list().len(), 4);
}

#[test]
fn refuses_to_remove_a_copy_with_uncommitted_work_naming_it_and_keeps_every_file() {
    let root = repository();
    let manager = Manager::new(&root.0);
    let wren = manager.ensure("tm_wren", "Wren", None).unwrap();
    std::fs::write(wren.join("cart.py"), "changed\n").unwrap();
    std::fs::write(wren.join("REPORT.md"), "the findings\n").unwrap();
    let refused = manager.remove("tm_wren", None).unwrap_err();
    assert_eq!(refused, "It has 2 uncommitted changes, and removing it would delete them.");
    assert_eq!(std::fs::read_to_string(wren.join("REPORT.md")).unwrap(), "the findings\n");
    assert_eq!(manager.list().len(), 1);
}

#[test]
fn removes_it_only_for_the_exact_changes_agreed_to_one_made_since_is_refused_again() {
    let root = repository();
    let manager = Manager::new(&root.0);
    let wren = manager.ensure("tm_wren", "Wren", None).unwrap();
    std::fs::write(wren.join("REPORT.md"), "the findings\n").unwrap();
    std::fs::write(wren.join("LATER.md"), "written after\n").unwrap();
    assert!(manager.remove("tm_wren", Some(&["REPORT.md".into()])).is_err());
    assert_eq!(std::fs::read_to_string(wren.join("LATER.md")).unwrap(), "written after\n");
    manager.remove("tm_wren", Some(&["LATER.md".into(), "REPORT.md".into()])).unwrap();
    assert!(manager.list().is_empty());
    // The branch stays: removing a copy never deletes the teammate's work on its branch.
    assert!(git(&["branch", "--list", "locust/wren"], &root.0).contains("locust/wren"));
}

#[test]
fn removes_a_clean_copy_at_once() {
    let root = repository();
    let manager = Manager::new(&root.0);
    manager.ensure("tm_wren", "Wren", None).unwrap();
    manager.remove("tm_wren", None).unwrap();
    assert!(manager.list().is_empty());
}

#[test]
fn refuses_an_id_that_could_name_a_path_outside_the_folder() {
    let root = repository();
    let manager = Manager::new(&root.0);
    assert!(manager.ensure("../escape", "x", None).unwrap_err().contains("cannot name a worktree"));
    assert!(manager.remove("..", None).unwrap_err().contains("cannot name a worktree"));
}

#[test]
fn commits_nothing_when_the_turn_changed_nothing() {
    let root = repository();
    let manager = Manager::new(&root.0);
    manager.ensure("tm_wren", "Wren", None).unwrap();
    assert_eq!(manager.checkpoint("tm_wren", &message("Nothing")).unwrap(), Checkpoint::Clean);
}

#[test]
fn commits_every_change_new_files_too_as_the_teammate_past_a_hook_that_would_refuse_and_leaves_the_persons_branch_alone() {
    let root = repository();
    let hook = root.0.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let manager = Manager::new(&root.0);
    let tree = manager.ensure("tm_wren", "Wren", None).unwrap();
    let main_before = git(&["rev-parse", "main"], &root.0).trim().to_string();
    std::fs::write(tree.join("cart.py"), "def total(items):\n    return sum(items)\n").unwrap();
    std::fs::write(tree.join("cart_test.py"), "assert True\n").unwrap();
    let Checkpoint::Committed { sha, branch, mut files, .. } = manager.checkpoint("tm_wren", &message("Fix the cart total")).unwrap() else {
        panic!("committed")
    };
    assert_eq!(branch, "locust/wren");
    files.sort();
    assert_eq!(files, vec!["cart.py", "cart_test.py"]);
    let shown = git(&["log", "-1", "--format=%an <%ae>%n%B", &sha], &root.0);
    for want in ["Wren <tm_wren@teammates.locust>", "Fix the cart total", "Locust-Mission: m_1", "Locust-Route: opencode / nemotron"] { // made-up teammate author; public-guard: allow
        assert!(shown.contains(want), "{want} in {shown}");
    }
    assert_eq!(git(&["status", "--porcelain"], &tree).trim(), "");
    assert_eq!(git(&["rev-parse", "main"], &root.0).trim(), main_before);
    assert_eq!(std::fs::read_to_string(root.0.join("cart.py")).unwrap(), "def total(items):\n    return 0\n");
}

#[test]
fn refuses_a_teammate_with_no_tree_of_its_own() {
    let root = repository();
    let manager = Manager::new(&root.0);
    assert!(manager.checkpoint("tm_nobody", &message("x")).unwrap_err().contains("no own branch"));
}

#[test]
fn shows_the_branch_against_where_it_left_the_persons_branch_whole_and_turn_by_turn_oldest_first() {
    let root = repository();
    let manager = Manager::new(&root.0);
    let tree = manager.ensure("tm_wren", "Wren", None).unwrap();
    std::fs::write(tree.join("cart.py"), "def total(items):\n    return sum(items)\n").unwrap();
    manager.checkpoint("tm_wren", &message("Turn one")).unwrap();
    std::fs::write(tree.join("notes.md"), "why\n").unwrap();
    manager.checkpoint("tm_wren", &message("Turn two")).unwrap();
    std::fs::write(root.0.join("README.md"), "mine\n").unwrap();
    git(&["add", "."], &root.0);
    git(&["commit", "-q", "-m", "the person's own"], &root.0);
    std::fs::write(tree.join("scratch.txt"), "not yet\n").unwrap();

    let review = manager.review("tm_wren").unwrap();
    assert_eq!(review.branch, "locust/wren");
    assert_eq!(review.against.as_deref(), Some("main"));
    assert_eq!(review.turns.iter().map(|t| t.subject.as_str()).collect::<Vec<_>>(), vec!["Turn one", "Turn two"]);
    assert_eq!(review.turns[1].files, vec!["notes.md"]);
    let diff = review.diff.clone().unwrap();
    assert!(diff.contains("+    return sum(items)") && diff.contains("+++ b/notes.md") && !diff.contains("README.md"), "{diff}");
    assert_eq!(review.uncommitted, vec!["scratch.txt"]);

    let one = manager.turn_diff("tm_wren", &review.turns[0].sha).unwrap();
    assert!(one.contains("cart.py") && !one.contains("notes.md"));
    assert!(manager.turn_diff("tm_wren", &review.base).unwrap_err().contains("not a turn"));
    let main = git(&["rev-parse", "main"], &root.0).trim().to_string();
    assert!(manager.turn_diff("tm_wren", &main).unwrap_err().contains("not a turn"));
    assert!(manager.turn_diff("tm_wren", "HEAD~1").unwrap_err().contains("not a turn"));
}

#[test]
fn never_runs_a_program_the_repository_names_to_draw_a_diff() {
    let root = repository();
    let marker = root.0.join("ran.txt");
    git(&["config", "diff.external", &format!("sh -c \"echo ran > '{}'\"", marker.display())], &root.0);
    let manager = Manager::new(&root.0);
    let tree = manager.ensure("tm_wren", "Wren", None).unwrap();
    std::fs::write(tree.join("cart.py"), "changed\n").unwrap();
    manager.checkpoint("tm_wren", &message("Turn")).unwrap();
    let review = manager.review("tm_wren").unwrap();
    assert!(review.diff.unwrap().contains("+changed"));
    assert!(!marker.exists(), "the repository's diff program never ran");
}

#[test]
fn a_half_made_tree_is_finished_keeping_what_is_in_it_rather_than_handed_out() {
    // git's own `initializing` lock left on a tree whose checkout was cut off: every file reads as deleted.
    let root = repository();
    let manager = Manager::new(&root.0);
    let path = manager.ensure("tm_wren", "Wren", None).unwrap();
    std::fs::write(path.join("KEEP.md"), "made by the teammate\n").unwrap();
    std::fs::remove_file(path.join("cart.py")).unwrap();
    let admin = admin_directory_of(&path.join(".git"), &root.0).expect("its admin folder");
    std::fs::write(admin.join("locked"), "initializing\n").unwrap();
    assert!(manager.checkpoint("tm_wren", &message("x")).unwrap_err().contains("not finished being made"), "never committed half-made");
    assert_eq!(manager.ensure("tm_wren", "Wren", None).unwrap(), path);
    assert!(path.join("cart.py").is_file(), "the checkout was finished");
    assert_eq!(std::fs::read_to_string(path.join("KEEP.md")).unwrap(), "made by the teammate\n", "what was in it is kept");
    assert!(!admin.join("locked").exists(), "and unlocked");
}
