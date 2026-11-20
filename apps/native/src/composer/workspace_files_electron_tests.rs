//! Electron's `workspace-files.test.ts` (4 cases): the files the box offers for @, git's own list in a repository and a
//! walk elsewhere, never a path out of the folder or Locust's own, at most the cap.
use super::*;

#[test]
fn are_gits_own_list_in_a_repository_and_never_a_path_that_climbs_out_or_locusts_own_folder() {
    let listed = from_git(["README.md", "src\\cart.py", "../secret.txt", "/etc/passwd", "C:/x.txt", ".locust/memory.md"].into_iter());
    assert_eq!(listed, (vec!["README.md".to_owned(), "src/cart.py".to_owned()], false));
}

#[test]
fn are_walked_outside_a_repository_skipping_the_folders_nobody_attaches_from() {
    let root = std::env::temp_dir().join(format!("locust-files-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    for file in ["README.md", "src/cart.py", "node_modules/x/index.js", ".git/HEAD", "dist/app.js", ".locust/memory.md"] {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x").unwrap();
    }
    let (paths, _) = list(&root);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(paths, vec!["README.md".to_owned(), "src/cart.py".to_owned()]);
}

#[test]
fn stop_at_the_cap_and_say_so() {
    let many: Vec<String> = (0..MAX + 5).map(|i| format!("f{i}.ts")).collect();
    let (paths, truncated) = from_git(many.iter().map(String::as_str));
    assert_eq!(paths.len(), MAX);
    assert!(truncated);
}

#[test]
fn reach_past_docs_in_a_repository_the_size_of_locust() {
    let rest = ["apps/desktop/src/main/runtime-commands.ts", "packages/runtime-adapters/src/commands.ts", "tsconfig.base.json"];
    let docs: Vec<String> = (0..15_575).map(|i| format!("docs/record-{i}.md")).collect();
    let listed: Vec<&str> = std::iter::once(rest[0]).chain(docs.iter().map(String::as_str)).chain(rest[1..].iter().copied()).collect();
    let (paths, truncated) = from_git(listed.into_iter());
    assert!(!truncated);
    for r in rest {
        assert!(paths.iter().any(|p| p == r), "{r}");
    }
}
