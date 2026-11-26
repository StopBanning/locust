//! Which build this is: the version and the commit it was built from, so a fault can be tied to a build.
//!
//! The commit is given at build time in `LOCUST_BUILD_COMMIT` (a release script sets it; `build.rs` passes it through
//! and rebuilds when it changes). An ordinary build is not given one and says `dev`: git is never read and `.git` is
//! never watched, so a lane build does not rebuild on every commit.
//!
//! The commit goes into the error log's first line, which may be built from nothing that could name a person (no
//! path, user name or host name), so only a commit-shaped value is said: hex digits, 4 to 40 of them. Anything else
//! given is said as `dev`, as if none had been.

/// What `build.rs` gave: the value of `LOCUST_BUILD_COMMIT` at build time, or `dev` when it was not set.
const GIVEN_COMMIT: &str = env!("LOCUST_BUILD_COMMIT");

/// Said when a build was given no commit, or one that is not commit-shaped.
pub const NO_COMMIT: &str = "dev";

/// The app's own version, the one the window shows (Electron's `app.getVersion()`).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The commit this build was made from, or `dev`.
pub fn commit() -> &'static str {
    commit_of(GIVEN_COMMIT)
}

/// `0.883.0 (1a2b3c4d)`, or `0.883.0 (dev)`: what `--version` prints and every report and log run carries.
pub fn line() -> String {
    line_of(version(), commit())
}

fn commit_of(given: &str) -> &str {
    let given = given.trim();
    if (4..=40).contains(&given.len()) && given.bytes().all(|b| b.is_ascii_hexdigit()) { given } else { NO_COMMIT }
}

fn line_of(version: &str, commit: &str) -> String {
    format!("{version} ({commit})")
}

/// `build.rs`'s own decision, read here by its tests.
#[cfg(test)]
#[path = "../build_commit.rs"]
mod build_commit;

#[cfg(test)]
mod tests {
    use super::*;

    // The words are the lane's own (Electron says no commit anywhere): "0.883.0 (1a2b3c4d)" or "0.883.0 (dev)".
    #[test]
    fn the_line_is_the_version_then_the_commit_in_brackets() {
        assert_eq!(line_of("0.883.0", "1a2b3c4d"), "0.883.0 (1a2b3c4d)");
        assert_eq!(line_of("0.883.0", "dev"), "0.883.0 (dev)");
        assert_eq!(line(), format!("{} ({})", env!("CARGO_PKG_VERSION"), commit()));
    }

    #[test]
    fn the_version_is_the_crates_own() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn a_commit_given_at_build_time_is_said_and_none_given_is_dev() {
        assert_eq!(commit_of("1a2b3c4d"), "1a2b3c4d");
        assert_eq!(commit_of("8bb5e909c3f1d2a4b5c6d7e8f9a0b1c2d3e4f5a6"), "8bb5e909c3f1d2a4b5c6d7e8f9a0b1c2d3e4f5a6");
        assert_eq!(commit_of("ABCDEF12"), "ABCDEF12");
        assert_eq!(commit_of(" 1a2b3c4d\n"), "1a2b3c4d");
        assert_eq!(commit_of("dev"), "dev");
        assert_eq!(commit_of(""), "dev");
        // What build.rs passes on: the value given at build time, one line of it, or dev when none was given.
        use build_commit::given_commit;
        assert_eq!(given_commit(Some("1a2b3c4d")), "1a2b3c4d");
        assert_eq!(given_commit(Some(" 1a2b3c4d \n")), "1a2b3c4d");
        assert_eq!(given_commit(Some("1a2b3c4d\ncargo::rustc-cfg=apple_helper")), "1a2b3c4d");
        assert_eq!(given_commit(None), "dev");
        assert_eq!(given_commit(Some("")), "dev");
        assert_eq!(given_commit(Some("   ")), "dev");
        // And what this build was given comes through it to the line.
        assert_eq!(commit(), commit_of(GIVEN_COMMIT));
        assert!(commit() == "dev" || commit().bytes().all(|b| b.is_ascii_hexdigit()), "{}", commit());
    }

    #[test]
    fn nothing_that_could_name_a_person_is_said_as_a_commit() {
        for given in ["/Users/colin/locust", "colin", "colins-mac-mini.local", "C:\\Users\\colin", "1a2b 3c4d", "abc", &"a".repeat(41)] {
            assert_eq!(commit_of(given), "dev", "{given:?}");
        }
    }

    /// What `build.rs` tells cargo about the commit, run as cargo runs it: the script compiled and started with the
    /// environment cargo gives it (OUT_DIR, the target, `LOCUST_BUILD_COMMIT`), its lines read. A target that is not macOS
    /// returns before the Swift helper, so this builds nothing else.
    fn build_script_says(commit: Option<&str>) -> Vec<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = std::env::temp_dir().join(format!("locust-build-script-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join("build-script");
        let built = std::process::Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
            .args(["--edition", "2024", "--crate-name", "build_script", "-o"])
            .arg(&program)
            .arg(root.join("build.rs"))
            .output()
            .unwrap();
        assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
        let mut run = std::process::Command::new(&program);
        run.current_dir(root).env("OUT_DIR", &dir).env("CARGO_CFG_TARGET_OS", "linux").env_remove("LOCUST_BUILD_COMMIT");
        if let Some(commit) = commit {
            run.env("LOCUST_BUILD_COMMIT", commit);
        }
        let out = run.output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect()
    }

    #[test]
    fn the_build_script_passes_the_commit_it_was_given_to_the_crate_and_rebuilds_when_it_changes() {
        let said = build_script_says(Some("1a2b3c4d"));
        assert!(said.contains(&"cargo::rustc-env=LOCUST_BUILD_COMMIT=1a2b3c4d".to_owned()), "{said:?}");
        assert!(said.contains(&"cargo::rerun-if-env-changed=LOCUST_BUILD_COMMIT".to_owned()), "{said:?}");
        // A value that runs on into a second line is never a second instruction to cargo.
        let sneaky = build_script_says(Some("1a2b3c4d\ncargo::rustc-cfg=apple_helper"));
        assert!(sneaky.contains(&"cargo::rustc-env=LOCUST_BUILD_COMMIT=1a2b3c4d".to_owned()), "{sneaky:?}");
        assert!(!sneaky.iter().any(|line| line.contains("rustc-cfg=apple_helper")), "{sneaky:?}");
        // Given nothing, a build says dev.
        let none = build_script_says(None);
        assert!(none.contains(&"cargo::rustc-env=LOCUST_BUILD_COMMIT=dev".to_owned()), "{none:?}");
    }

    // A commit is 4 to 40 hex digits (the short form git prints is 7; four is the least `git rev-parse --short=4` gives).
    #[test]
    fn four_hex_digits_are_a_commit_and_three_are_not() {
        assert_eq!(commit_of("1a2b"), "1a2b");
        assert_eq!(commit_of("1a2"), "dev");
        assert_eq!(commit_of(&"f".repeat(40)), "f".repeat(40));
        assert_eq!(commit_of(&"f".repeat(41)), "dev");
    }
}
