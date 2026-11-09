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

pub fn guard(path: &Path, electron: &Path) -> io::Result<PathBuf> {
    let path = resolved(path)?;
    if path.starts_with(resolved(electron)?) {
        return Err(io::Error::other(
            "Refusing to write inside Electron's data folder",
        ));
    }
    Ok(path)
}

pub struct Store {
    root: PathBuf,
    electron: PathBuf,
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
        Ok(Self {
            root: guard(&root, &electron)?,
            electron,
        })
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
            guard(&location, &self.electron).map_err(|e| {
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
    pub fn ledger_dir(&self) -> io::Result<PathBuf> {
        guard(&self.root.join("mission-ledger"), &self.electron)
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
        guard(
            &self.ledger_dir()?.join(format!("{id}.jsonl")),
            &self.electron,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_electron_descendants_and_symlink_aliases() {
        let temp = std::env::temp_dir().join(format!("locust-guard-{}", std::process::id()));
        let electron = temp.join("electron");
        std::fs::create_dir_all(&electron).unwrap();
        assert!(guard(&electron, &electron).is_err());
        assert!(guard(&electron.join("new/../ledger"), &electron).is_err());
        assert!(guard(&temp.join("electron-native"), &electron).is_ok());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&electron, temp.join("alias")).unwrap();
            assert!(guard(&temp.join("alias/new"), &electron).is_err());
        }
        std::fs::remove_dir_all(temp).unwrap();
    }
}
