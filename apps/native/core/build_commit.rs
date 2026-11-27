//! What `build.rs` passes to `build_info` as `LOCUST_BUILD_COMMIT`, kept apart so the crate's tests read the same code
//! the build runs (`build_info`'s tests include it by path).

/// The commit a build was given (`LOCUST_BUILD_COMMIT` at build time), or `dev` when none was. One line only: a value
/// carrying a newline must not become a second instruction to cargo.
pub fn given_commit(given: Option<&str>) -> &str {
    let commit = given.and_then(|g| g.lines().next()).unwrap_or("").trim();
    if commit.is_empty() { "dev" } else { commit }
}
