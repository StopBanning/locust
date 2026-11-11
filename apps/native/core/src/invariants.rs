//! Assertions for program data, never for filesystem, network, or runtime output.
pub fn regex(pattern: &'static str) -> regex::Regex {
    // Callers supply only source literals (or entries in literal arrays), exercised by tests.
    // A bad pattern is a programming error; no external input can make compilation fail.
    regex::Regex::new(pattern).expect("valid source regex")
}
pub fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // These locks protect internal state. Poison requires an earlier panic during mutation;
    // continuing with that partially updated state would violate the owner's invariants.
    mutex
        .lock()
        .expect("internal state mutation must not panic")
}

/// Nanoseconds since 1970 that never repeat within this process: the clock, or one past the last value handed out,
/// whichever is later. Names and ids built from the clock alone collided when two were made in the same tick: two
/// approval cards raised together got the same id, the second overwrote the first, and both turns failed with
/// "Approval channel closed" (found 11/11 by a Board test). Every clock-based id or temp name uses this.
pub fn unique_nanos() -> u128 {
    static LAST: std::sync::Mutex<u128> = std::sync::Mutex::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut last = lock(&LAST);
    *last = (*last + 1).max(now);
    *last
}

#[cfg(test)]
mod unique_tests {
    #[test]
    fn unique_nanos_never_repeat_even_across_threads() {
        let threads: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..2000).map(|_| super::unique_nanos()).collect::<Vec<_>>()))
            .collect();
        let mut all: Vec<u128> = threads.into_iter().flat_map(|t| t.join().expect("thread")).collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "a value repeated");
    }
}

/// The longest path a Unix socket may have on macOS is 104 bytes (SUN_LEN), counting the terminator; Linux allows 108.
const SOCKET_PATH_MAX: usize = 103;

/// A private (0700) folder for a local socket, with a path short enough to bind `<folder>/host`. An app opened from
/// Finder has a long TMPDIR (/var/folders/xx/…/T/), and a folder named with a whole UUID inside it made the socket
/// path 105 bytes: "path must be shorter than SUN_LEN", found live on 11/11 when a native OpenCode turn started. The
/// name is short and unique, and when TMPDIR is still too long the folder goes under /tmp instead.
#[cfg(unix)]
pub fn socket_dir(prefix: &str) -> std::io::Result<std::path::PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let name = format!("{prefix}-{}-{:x}", std::process::id(), unique_nanos() % 0xffff_ffff_ffff);
    let fits = |base: &std::path::Path| base.join(&name).join("host").as_os_str().len() <= SOCKET_PATH_MAX;
    let temp = std::env::temp_dir();
    let base = if fits(&temp) { temp } else { std::path::PathBuf::from("/tmp") };
    if !fits(&base) {
        return Err(std::io::Error::other("No folder is short enough for a local socket."));
    }
    let dir = base.join(name);
    std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(dir)
}

#[cfg(all(test, unix))]
mod socket_tests {
    #[test]
    fn a_socket_folder_is_short_enough_even_under_a_long_tmpdir() {
        let dir = super::socket_dir("lcb").expect("folder");
        assert!(dir.join("host").as_os_str().len() <= super::SOCKET_PATH_MAX);
        let listener = std::os::unix::net::UnixListener::bind(dir.join("host")).expect("binds");
        drop(listener);
        let _ = std::fs::remove_dir_all(&dir);
        // The macOS TMPDIR shape an app opened from Finder gets, padded: too long, so /tmp is used.
        let long = std::path::Path::new("/var/folders/fs/3vxqwxln38s78yklvph78d_40000gn/T/a-much-longer-tmp-folder-name");
        assert!(long.join("lcb-99999-ffffffffffff").join("host").as_os_str().len() > super::SOCKET_PATH_MAX);
    }
}
