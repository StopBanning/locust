//! Fail closed if tests bypass the Cargo runner or request non-temporary stores.
use std::path::Path;

pub fn require_temporary(path: &Path) {
    let root = std::env::var_os("LOCUST_TEST_ROOT")
        .expect("Test isolation guard: use cargo test with the configured isolated runner");
    let root = Path::new(&root).canonicalize().expect("test root");
    let mut normalized = std::path::PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            other => normalized.push(other),
        }
    }
    let path = normalized.as_path();
    assert!(
        path.starts_with(&root),
        "Test isolation guard refused {}",
        path.display()
    );
    let mut ancestor = path;
    while !ancestor.exists() {
        ancestor = ancestor.parent().expect("test path must be absolute");
    }
    let resolved = ancestor.canonicalize().expect("test path ancestor");
    assert!(
        resolved.starts_with(root),
        "Test isolation guard refused {}",
        path.display()
    );
}

#[test]
fn home_and_native_data_are_temporary() {
    for name in [
        "HOME",
        "LOCUST_NATIVE_DATA",
        "XDG_CONFIG_HOME",
        "APPDATA",
        "CODEX_HOME",
    ] {
        require_temporary(Path::new(&std::env::var_os(name).expect(name)));
    }
}

#[test]
#[should_panic(expected = "Test isolation guard refused")]
fn guard_rejects_non_temporary_store_paths() {
    require_temporary(Path::new(
        "/Users/locust/Library/Application Support/@teammate/desktop-native",
    ));
}
