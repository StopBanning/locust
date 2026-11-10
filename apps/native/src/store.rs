//! Native writes never share Electron's data directory, including through symlinks.
use std::{
    io,
    path::{Component, Path, PathBuf},
};

fn resolved(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            _ => {
                out.push(part);
                if out.symlink_metadata().is_ok() {
                    out = out.canonicalize()?;
                }
            }
        }
    }
    Ok(out)
}

/// Codex's own folders: `~/.codex` and `CODEX_HOME`. Native never writes there (Codex keeps its login and sessions in
/// them). Taken from the two variables, so a test can pass them in without touching the environment.
pub fn codex_homes(
    home: Option<std::ffi::OsString>,
    codex_home: Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = home
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .filter(|h| h.is_absolute())
    {
        out.push(home.join(".codex"));
    }
    if let Some(codex) = codex_home
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .filter(|h| h.is_absolute())
    {
        out.push(codex);
    }
    out
}

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    electron: PathBuf,
    /// Folders native refuses to write inside besides Electron's: Codex's own.
    codex: Vec<PathBuf>,
}
impl Store {
    pub fn local() -> io::Result<Self> {
        let electron = crate::engine::default_data_folder()
            .ok_or_else(|| io::Error::other("Electron data folder could not be determined"))?;
        let root = std::env::var_os("LOCUST_NATIVE_DATA")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut native = electron.as_os_str().to_owned();
                native.push("-native");
                PathBuf::from(native)
            });
        Self::new(root, electron)
    }
    pub fn new(root: PathBuf, electron: PathBuf) -> io::Result<Self> {
        let codex = codex_homes(std::env::var_os("HOME"), std::env::var_os("CODEX_HOME"));
        Self::with_codex(root, electron, codex)
    }
    pub fn with_codex(root: PathBuf, electron: PathBuf, codex: Vec<PathBuf>) -> io::Result<Self> {
        let mut store = Self {
            root: PathBuf::new(),
            electron,
            codex,
        };
        store.root = store.check(&root)?;
        Ok(store)
    }
    /// Resolve `path` (symlinks and `..`) and refuse it when it is, or lies inside, Electron's data folder or one of
    /// Codex's own folders.
    pub fn check(&self, path: &Path) -> io::Result<PathBuf> {
        let path = resolved(path)?;
        self.refuse(&path)?;
        Ok(path)
    }
    fn refuse(&self, resolved_path: &Path) -> io::Result<()> {
        if resolved_path.starts_with(resolved(&self.electron)?) {
            return Err(io::Error::other(
                "Refusing to write inside Electron's data folder",
            ));
        }
        for folder in &self.codex {
            if resolved_path.starts_with(resolved(folder)?) {
                return Err(io::Error::other(
                    "Refusing to write inside Codex's own folder",
                ));
            }
        }
        Ok(())
    }
    pub fn check_open_file(&self, file: &std::fs::File) -> io::Result<()> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
            let metadata = file.metadata()?; // fstat, never stat the requested pathname.
            if !metadata.is_file() {
                return Err(io::Error::other("Opened ledger is not a regular file"));
            }
            #[cfg(target_os = "macos")]
            let location = {
                use std::os::unix::ffi::OsStrExt;
                let mut path = [0u8; libc::PATH_MAX as usize];
                if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, path.as_mut_ptr()) }
                    == -1
                {
                    return Err(io::Error::last_os_error());
                }
                let end = path
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or_else(|| io::Error::other("Invalid F_GETPATH result"))?;
                PathBuf::from(std::ffi::OsStr::from_bytes(&path[..end]))
            };
            #[cfg(target_os = "linux")]
            let location = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
            self.check(&location).map_err(|e| {
                io::Error::other(format!(
                    "{e}; opened ledger at {}; empty file left in place (not deleted)",
                    location.display()
                ))
            })?;
            let located = std::fs::metadata(&location)?;
            if metadata.dev() != located.dev() || metadata.ino() != located.ino() {
                return Err(io::Error::other(
                    "Opened ledger location changed before validation",
                ));
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = file;
            Err(io::Error::other(
                "Opened ledger location verification is unsupported on this platform",
            ))
        }
    }
    pub fn electron(&self) -> &Path {
        &self.electron
    }
    #[cfg(test)]
    #[cfg(test)]
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn ledger_dir(&self) -> io::Result<PathBuf> {
        self.check(&self.root.join("mission-ledger"))
    }
    /// Durable busy-turn queue (Electron `queued-messages.json`), native folder only.
    pub fn queued_messages_path(&self) -> io::Result<PathBuf> {
        self.check(&self.root.join("queued-messages.json"))
    }
    /// Pasted / copied-in attachments (Electron `.locust/attachments`), native folder only.
    pub fn attachments_dir(&self) -> io::Result<PathBuf> {
        self.check(&self.root.join(".locust").join("attachments"))
    }
    /// Conversation titles/pins/settled — Electron shape (`teammates.json`), native folder only.
    pub fn teammates_json_path(&self) -> io::Result<PathBuf> {
        self.check(&self.root.join("teammates.json"))
    }
    /// Projects / groups — Electron shape (`groups.json`), native folder only.
    pub fn groups_json_path(&self) -> io::Result<PathBuf> {
        self.check(&self.root.join("groups.json"))
    }
    pub fn write_path(&self, id: &str) -> io::Result<PathBuf> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(io::Error::other("Invalid mission ID"));
        }
        self.check(&self.ledger_dir()?.join(format!("{id}.jsonl")))
    }

    /// Create `name` (exclusively, mode 0600) in the directory `dir`, creating missing directories, WITHOUT ever
    /// following a symlink and without creating anything inside a folder native must not write.
    ///
    /// `dir` is a path already resolved by `check`. It is walked from `/` one component at a time with
    /// `openat(O_DIRECTORY | O_NOFOLLOW)`, so a component swapped for a symlink after the check is refused rather than
    /// followed (`O_NOFOLLOW` on the whole path would guard only its last component). Each directory descriptor is
    /// compared with Electron's and Codex's folders BEFORE it is used, and the file is created with
    /// `openat(O_CREAT | O_EXCL | O_NOFOLLOW)` relative to the final descriptor. A refusal therefore happens before
    /// anything exists on the far side: nothing is created, nothing needs deleting.
    #[cfg(unix)]
    pub fn create_new_file_in(&self, dir: &Path, name: &str) -> io::Result<std::fs::File> {
        use std::ffi::CString;
        use std::os::fd::{FromRawFd, OwnedFd};
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        if name.is_empty()
            || name.contains('/')
            || name == "."
            || name == ".."
            || name.contains('\0')
        {
            return Err(io::Error::other("Invalid ledger file name"));
        }
        if !dir.is_absolute() {
            return Err(io::Error::other(
                "Ledger directory must be an absolute path",
            ));
        }
        // Identities native must never create inside: Electron's folder and Codex's, where they exist.
        let mut forbidden: Vec<(u64, u64)> = Vec::new();
        for folder in std::iter::once(&self.electron).chain(self.codex.iter()) {
            if let Ok(m) = std::fs::metadata(folder) {
                forbidden.push((m.dev(), m.ino()));
            }
        }
        let fail = || io::Error::last_os_error();
        let cstr = |bytes: &[u8]| {
            CString::new(bytes).map_err(|_| io::Error::other("Invalid path component"))
        };
        let identity = |fd: &OwnedFd| -> io::Result<()> {
            use std::os::fd::AsRawFd;
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(fd.as_raw_fd(), &mut st) } != 0 {
                return Err(io::Error::last_os_error());
            }
            if (st.st_mode & libc::S_IFMT) != libc::S_IFDIR {
                return Err(io::Error::other("Ledger path component is not a directory"));
            }
            if forbidden.contains(&(st.st_dev as u64, st.st_ino as u64)) {
                return Err(io::Error::other(
                    "Refusing to write inside Electron's or Codex's own folder; nothing was created",
                ));
            }
            Ok(())
        };
        use std::os::fd::AsRawFd;
        let root = cstr(b"/")?;
        let first = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if first < 0 {
            return Err(fail());
        }
        let mut current = unsafe { OwnedFd::from_raw_fd(first) };
        identity(&current)?;
        for part in dir.components() {
            let Component::Normal(part) = part else {
                if matches!(part, Component::RootDir) {
                    continue;
                }
                return Err(io::Error::other("Ledger directory must be a resolved path"));
            };
            let part = cstr(part.as_bytes())?;
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let mut next = unsafe { libc::openat(current.as_raw_fd(), part.as_ptr(), flags) };
            if next < 0 && io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                // Missing: made here, relative to a directory already verified; a racing symlink loses to NOFOLLOW.
                if unsafe { libc::mkdirat(current.as_raw_fd(), part.as_ptr(), 0o700) } != 0
                    && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists
                {
                    return Err(fail());
                }
                next = unsafe { libc::openat(current.as_raw_fd(), part.as_ptr(), flags) };
            }
            if next < 0 {
                return Err(io::Error::other(format!(
                    "Refusing to follow {}: {} (a symlink or changed directory in the ledger path); nothing was created",
                    String::from_utf8_lossy(part.to_bytes()),
                    io::Error::last_os_error()
                )));
            }
            current = unsafe { OwnedFd::from_raw_fd(next) };
            identity(&current)?;
        }
        let file = cstr(name.as_bytes())?;
        let fd = unsafe {
            libc::openat(
                current.as_raw_fd(),
                file.as_ptr(),
                libc::O_WRONLY
                    | libc::O_APPEND
                    | libc::O_CREAT
                    | libc::O_EXCL
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC,
                0o600 as libc::c_uint,
            )
        };
        if fd < 0 {
            return Err(fail());
        }
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }
    #[cfg(not(unix))]
    pub fn create_new_file_in(&self, _dir: &Path, _name: &str) -> io::Result<std::fs::File> {
        Err(io::Error::other(
            "Ledger creation is unsupported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store_for(electron: &Path, codex: Vec<PathBuf>) -> Store {
        // A store rooted somewhere harmless, to ask `check` about other paths.
        let root = electron.parent().unwrap().join("native-ok");
        Store::with_codex(root, electron.to_path_buf(), codex).unwrap()
    }
    #[test]
    fn rejects_electron_descendants_and_symlink_aliases() {
        let temp = std::env::temp_dir().join(format!("locust-guard-{}", std::process::id()));
        let electron = temp.join("electron");
        std::fs::create_dir_all(&electron).unwrap();
        let store = store_for(&electron, vec![]);
        assert!(store.check(&electron).is_err());
        assert!(store.check(&electron.join("new/../ledger")).is_err());
        assert!(store.check(&temp.join("electron-native")).is_ok());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&electron, temp.join("alias")).unwrap();
            assert!(store.check(&temp.join("alias/new")).is_err());
        }
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn codex_homes_are_refused_with_folders_inside_and_symlink_aliases() {
        let temp = std::env::temp_dir().join(format!("locust-codexguard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let electron = temp.join("electron");
        let home = temp.join("home");
        let codex = home.join(".codex");
        let custom = temp.join("custom-codex-home");
        for d in [&electron, &codex, &custom] {
            std::fs::create_dir_all(d).unwrap();
        }
        // `~/.codex` comes from HOME, and CODEX_HOME when set (both; empty or relative values are ignored).
        assert_eq!(
            codex_homes(Some(home.clone().into()), Some(custom.clone().into())),
            vec![codex.clone(), custom.clone()]
        );
        assert_eq!(
            codex_homes(Some(home.clone().into()), None),
            vec![codex.clone()]
        );
        assert!(codex_homes(Some("".into()), Some("relative".into())).is_empty());
        let homes = codex_homes(Some(home.clone().into()), Some(custom.clone().into()));
        // LOCUST_NATIVE_DATA pointing at either folder, or anywhere inside, cannot become a native root.
        for bad in [
            codex.clone(),
            codex.join("mission-ledger"),
            codex.join("a/b/../c"),
            custom.clone(),
            custom.join("sessions"),
        ] {
            let error = Store::with_codex(bad.clone(), electron.clone(), homes.clone())
                .err()
                .map(|e| e.to_string());
            assert!(
                error.as_deref().is_some_and(|e| e.contains("Codex")),
                "{}: {error:?}",
                bad.display()
            );
        }
        #[cfg(unix)]
        {
            // Symlink aliases resolving there are refused too (to the folder, to a folder inside it).
            std::fs::create_dir_all(codex.join("inner")).unwrap();
            std::os::unix::fs::symlink(&codex, temp.join("alias-codex")).unwrap();
            std::os::unix::fs::symlink(codex.join("inner"), temp.join("alias-inner")).unwrap();
            std::os::unix::fs::symlink(&custom, temp.join("alias-custom")).unwrap();
            for bad in [
                temp.join("alias-codex"),
                temp.join("alias-codex/x"),
                temp.join("alias-inner"),
                temp.join("alias-custom/y"),
            ] {
                assert!(
                    Store::with_codex(bad.clone(), electron.clone(), homes.clone()).is_err(),
                    "{}",
                    bad.display()
                );
            }
        }
        // A sibling that merely shares a prefix is fine, and so is the default `<electron>-native`.
        assert!(
            Store::with_codex(home.join(".codex-native"), electron.clone(), homes.clone()).is_ok()
        );
        assert!(
            Store::with_codex(
                temp.join("electron-native"),
                electron.clone(),
                homes.clone()
            )
            .is_ok()
        );
        // The ledger directory and every write path inherit the refusal.
        let store = Store::with_codex(temp.join("native"), electron, homes).unwrap();
        assert!(store.write_path("ok").is_ok());
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn local_store_refuses_the_real_codex_home_from_the_environment() {
        // The default build of the store reads HOME and CODEX_HOME itself: `Store::new` refuses what they name.
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap();
        let electron =
            std::env::temp_dir().join(format!("locust-env-electron-{}", std::process::id()));
        std::fs::create_dir_all(&electron).unwrap();
        let error = Store::new(home.join(".codex").join("mission-ledger"), electron.clone())
            .err()
            .map(|e| e.to_string());
        assert!(
            error.as_deref().is_some_and(|e| e.contains("Codex")),
            "{error:?}"
        );
        std::fs::remove_dir_all(electron).unwrap();
    }
}
