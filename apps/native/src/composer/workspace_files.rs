//! Read-only workspace listing for @.
pub use locust_core::workspace_files::list;
#[cfg(test)]
use locust_core::workspace_files::{MAX, from_git};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn walk_excludes_builds_and_symlinks() {
        let root = std::env::temp_dir().join(format!("locust-file-list-{}", std::process::id()));
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "").unwrap();
        fs::write(root.join("node_modules/no"), "").unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        let (paths, partial) = list(&root);
        assert!(!partial);
        assert_eq!(paths, vec!["README.md", "src/main.rs"]);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "workspace_files_electron_tests.rs"]
mod workspace_files_electron_tests;
