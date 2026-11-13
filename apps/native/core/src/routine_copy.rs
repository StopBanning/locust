//! A ROUTINE THAT WORKS IN A COPY (main/routine-copy.ts, 0.533, with the named-copy half of main/compare-copies.ts).
//!
//! Its steps run in a copy of the folder, and when it finishes what it changed waits for the person to Keep (written
//! into the folder, refused if they have since changed the same files) or Discard. Nothing lands in the folder before
//! that. The copy is the folder as it is, uncommitted work included, without what is rebuilt or fetched
//! (dependencies, build output, git's own store), and refused for a folder too big to copy. One copy per routine,
//! made fresh for each run; beside it, the folder it came from, so Keep knows where to write after a restart.
//!
//! Electron keeps these under `~/.locust/routines`. The native app writes only its own data folder, so here they live
//! in `<native data>/routine-copies`.
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_COPY_FILES: u64 = 20_000;
pub const MAX_COPY_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_COPIED_FILE_BYTES: u64 = 20 * 1024 * 1024;
pub(crate) const LEFT_OUT: &[&str] = &[".git", ".locust", "node_modules", ".venv", "venv", "__pycache__", "dist", "build", ".next", "target", ".cache", ".turbo"];
/// The folder's dependencies, linked rather than copied, so a check can run in the copy (0.534).
pub const LINKED_DEPENDENCIES: &[&str] = &["node_modules", ".venv", "venv"];

/// What a run changed in its copy: new or changed files, and files it deleted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CopyChanges {
    pub changed: Vec<String>,
    pub deleted: Vec<String>,
}

/// What Keep did (`CopyBringIn`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BringIn {
    Brought(Vec<String>),
    Nothing,
    /// The person has since changed these files themselves, so nothing was written.
    YourChanges(Vec<String>),
}

/// How much a copy may hold; a test passes smaller ones rather than writing 20,000 files.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub files: u64,
    pub bytes: u64,
}
pub(crate) const LIMITS: Limits = Limits { files: MAX_COPY_FILES, bytes: MAX_COPY_BYTES };

pub(crate) fn too_big(limits: Limits) -> String {
    let files = limits.files.to_string();
    let mut grouped = String::new();
    for (i, c) in files.chars().enumerate() {
        if i > 0 && (files.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("more than {grouped} files or {} MB", (limits.bytes as f64 / (1024.0 * 1024.0)).round() as u64)
}

/// `nameFor`: `rt_` and letters or digits, so a copy can never climb out of its root.
fn name_for(routine_id: &str) -> Result<&str, String> {
    let ok = routine_id.strip_prefix("rt_").is_some_and(|rest| (1..=40).contains(&rest.len()) && rest.bytes().all(|b| b.is_ascii_alphanumeric()));
    if ok { Ok(routine_id) } else { Err("That routine cannot name a copy.".into()) }
}

pub(crate) fn measure(folder: &Path, limits: Limits) -> std::io::Result<bool> {
    fn walk(dir: &Path, files: &mut u64, bytes: &mut u64, limits: Limits) -> std::io::Result<bool> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if LEFT_OUT.iter().any(|l| name == *l) {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                if !walk(&entry.path(), files, bytes, limits)? {
                    return Ok(false);
                }
            } else if kind.is_file() {
                let size = entry.metadata()?.len();
                if size > MAX_COPIED_FILE_BYTES {
                    continue;
                }
                *files += 1;
                *bytes += size;
                if *files > limits.files || *bytes > limits.bytes {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    let (mut files, mut bytes) = (0, 0);
    walk(folder, &mut files, &mut bytes, limits)
}

pub(crate) fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if LEFT_OUT.iter().any(|l| name == *l) {
            continue;
        }
        let (source, target) = (entry.path(), to.join(&name));
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else if kind.is_file() {
            if entry.metadata()?.len() <= MAX_COPIED_FILE_BYTES {
                fs::copy(&source, &target)?;
            }
        } else if kind.is_symlink() {
            // Copied as the link it is, as Node's `cp` does.
            #[cfg(unix)]
            std::os::unix::fs::symlink(fs::read_link(&source)?, &target)?;
        }
    }
    Ok(())
}

pub(crate) fn hash_of(path: &Path) -> std::io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

/// Every file under a tree, by its path inside it (forward slashes), with its content's hash (`hashTree`).
pub(crate) fn hash_tree(tree: &Path) -> std::io::Result<BTreeMap<String, String>> {
    hash_tree_opts(tree, false)
}

pub(crate) fn hash_tree_opts(tree: &Path, name_large_files: bool) -> std::io::Result<BTreeMap<String, String>> {
    fn walk(root: &Path, dir: &Path, name_large: bool, out: &mut BTreeMap<String, String>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if LEFT_OUT.iter().any(|l| name == *l) {
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(root, &path, name_large, out)?;
            } else if kind.is_file() {
                let len = entry.metadata()?.len();
                let inside = path.strip_prefix(root).unwrap_or(&path);
                let key = inside.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                if len <= MAX_COPIED_FILE_BYTES {
                    out.insert(key, hash_of(&path)?);
                } else if name_large {
                    out.insert(key, "too-large-to-inline".to_owned());
                }
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(tree, tree, name_large_files, &mut out)?;
    Ok(out)
}

/// `createRoutineCopies(root)`.
#[derive(Clone, Debug)]
pub struct RoutineCopies {
    root: PathBuf,
    limits: Limits,
}

impl RoutineCopies {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), limits: LIMITS }
    }

    /// The copies of this data folder: `<native data>/routine-copies`.
    pub fn in_store(store: &crate::store::Store) -> Self {
        Self::new(store.root().join("routine-copies"))
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    fn source_file(&self, routine_id: &str) -> Result<PathBuf, String> {
        Ok(self.root.join(format!("{}.source.json", name_for(routine_id)?)))
    }

    fn manifest(&self, routine_id: &str) -> Result<PathBuf, String> {
        Ok(self.root.join(format!("{}.manifest.json", name_for(routine_id)?)))
    }

    /// Where this routine's copy is.
    pub fn path(&self, routine_id: &str) -> Result<PathBuf, String> {
        Ok(self.root.join(name_for(routine_id)?))
    }

    fn unlink_dependencies(copy: &Path) {
        for name in LINKED_DEPENDENCIES {
            let link = copy.join(name);
            if fs::symlink_metadata(&link).is_ok_and(|m| m.file_type().is_symlink()) {
                let _ = fs::remove_file(&link);
            }
        }
    }

    fn remove_named(&self, routine_id: &str) -> Result<(), String> {
        let path = self.path(routine_id)?;
        Self::unlink_dependencies(&path);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| format!("The routine's copy could not be removed: {e}"))?;
        }
        let _ = fs::remove_file(self.manifest(routine_id)?);
        Ok(())
    }

    /// A fresh copy of `folder` for this routine's run; any earlier one is removed first.
    pub fn make(&self, routine_id: &str, folder: &Path) -> Result<PathBuf, String> {
        let target = self.path(routine_id)?;
        self.remove_named(routine_id)?;
        let source = folder.canonicalize().map_err(|_| "The folder to copy could not be read.".to_string())?;
        if !measure(&source, self.limits).unwrap_or(false) {
            return Err(format!("It answers in a copy of the folder, and this folder is too big to copy ({}).", too_big(self.limits)));
        }
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        copy_tree(&source, &target).map_err(|e| format!("A copy of the folder could not be made: {e}"))?;
        let manifest = hash_tree(&target).map_err(|e| e.to_string())?;
        fs::write(self.manifest(routine_id)?, serde_json::to_string(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for dependency in LINKED_DEPENDENCIES {
            let there = source.join(dependency);
            if fs::symlink_metadata(&there).is_ok_and(|m| m.is_dir()) {
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(&there, target.join(dependency));
            }
        }
        fs::write(self.source_file(routine_id)?, serde_json::json!({"folder": source.to_string_lossy()}).to_string()).map_err(|e| e.to_string())?;
        Ok(target)
    }

    /// The folder the copy was made from.
    pub fn source(&self, routine_id: &str) -> Option<PathBuf> {
        let text = fs::read_to_string(self.source_file(routine_id).ok()?).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        value["folder"].as_str().filter(|f| !f.is_empty()).map(PathBuf::from)
    }

    /// What the run changed in its copy.
    pub fn changes(&self, routine_id: &str) -> Result<CopyChanges, String> {
        let before: BTreeMap<String, String> = serde_json::from_str(
            &fs::read_to_string(self.manifest(routine_id)?).map_err(|_| "The copy's record could not be read.".to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let now = hash_tree(&self.path(routine_id)?).map_err(|e| e.to_string())?;
        Ok(CopyChanges {
            changed: now.iter().filter(|(p, h)| before.get(*p) != Some(h)).map(|(p, _)| p.clone()).collect(),
            deleted: before.keys().filter(|p| !now.contains_key(*p)).cloned().collect(),
        })
    }

    /// Keep: the changed files written into the folder and the deleted ones removed -- nothing at all when the person
    /// has since changed any of those same files (`bringInCopy`).
    pub fn keep(&self, routine_id: &str) -> Result<BringIn, String> {
        let folder = self.source(routine_id).ok_or("Where this copy came from is not recorded, so it cannot be kept.")?;
        let before: BTreeMap<String, String> =
            serde_json::from_str(&fs::read_to_string(self.manifest(routine_id)?).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let CopyChanges { changed, deleted } = self.changes(routine_id)?;
        let mut touched: Vec<String> = changed.iter().chain(&deleted).cloned().collect();
        if touched.is_empty() {
            return Ok(BringIn::Nothing);
        }
        let in_folder = |path: &str| -> Result<PathBuf, String> {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.iter().any(|p| p.is_empty() || *p == "." || *p == "..") {
                return Err("A copy named a file outside the folder.".into());
            }
            Ok(parts.iter().fold(folder.clone(), |at, p| at.join(p)))
        };
        let mut yours = Vec::new();
        for path in &touched {
            let current = hash_of(&in_folder(path)?).ok();
            if current.as_ref() != before.get(path) {
                yours.push(path.clone());
            }
        }
        if !yours.is_empty() {
            return Ok(BringIn::YourChanges(yours));
        }
        let copy = self.path(routine_id)?;
        for path in &changed {
            let to = in_folder(path)?;
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let from = path.split('/').fold(copy.clone(), |at, p| at.join(p));
            fs::copy(from, &to).map_err(|e| e.to_string())?;
        }
        for path in &deleted {
            let _ = fs::remove_file(in_folder(path)?);
        }
        touched.sort();
        Ok(BringIn::Brought(touched))
    }

    /// Discard: the copy, its record and where it came from, gone. A dependency link is unlinked first, so removing
    /// the copy never reaches through it into the folder's own.
    pub fn discard(&self, routine_id: &str) -> Result<(), String> {
        self.remove_named(routine_id)?;
        let _ = fs::remove_file(self.source_file(routine_id)?);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! a-routine-works-in-a-copy.test.ts, "a routine's copy".
    use super::*;

    pub(crate) struct Temp(pub PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    pub(crate) fn temp(prefix: &str) -> Temp {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "{prefix}{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Temp(root.canonicalize().unwrap())
    }

    #[test]
    fn is_the_folder_as_it_is_keep_writes_its_changes_in_and_remembers_where_from_after_a_restart() {
        let (folder, root) = (temp("locust-rtcopy-folder-"), temp("locust-rtcopy-root-"));
        fs::write(folder.0.join("policy.md"), "Cutoff is 4 pm.\n").unwrap();
        let copies = RoutineCopies::new(&root.0);
        let copy = copies.make("rt_abc", &folder.0).unwrap();
        assert_eq!(fs::read_to_string(copy.join("policy.md")).unwrap(), "Cutoff is 4 pm.\n");
        fs::write(copy.join("policy.md"), "Cutoff is 3 pm.\n").unwrap();
        fs::write(copy.join("receipt.md"), "Received.\n").unwrap();
        assert_eq!(copies.changes("rt_abc").unwrap(), CopyChanges { changed: vec!["policy.md".into(), "receipt.md".into()], deleted: vec![] });
        // The folder is untouched until Keep.
        assert_eq!(fs::read_to_string(folder.0.join("policy.md")).unwrap(), "Cutoff is 4 pm.\n");
        let again = RoutineCopies::new(&root.0);
        assert_eq!(again.source("rt_abc"), Some(folder.0.clone()));
        assert_eq!(again.keep("rt_abc").unwrap(), BringIn::Brought(vec!["policy.md".into(), "receipt.md".into()]));
        assert_eq!(fs::read_to_string(folder.0.join("policy.md")).unwrap(), "Cutoff is 3 pm.\n");
        assert_eq!(fs::read_to_string(folder.0.join("receipt.md")).unwrap(), "Received.\n");
    }

    #[test]
    fn refuses_to_keep_over_a_file_the_person_changed_since_and_discard_leaves_no_copy_behind() {
        let (folder, root) = (temp("locust-rtcopy-folder-"), temp("locust-rtcopy-root-"));
        fs::write(folder.0.join("policy.md"), "Cutoff is 4 pm.\n").unwrap();
        let copies = RoutineCopies::new(&root.0);
        let copy = copies.make("rt_abc", &folder.0).unwrap();
        fs::write(copy.join("policy.md"), "Cutoff is 3 pm.\n").unwrap();
        fs::write(folder.0.join("policy.md"), "Cutoff is 5 pm, said the person.\n").unwrap();
        assert_eq!(copies.keep("rt_abc").unwrap(), BringIn::YourChanges(vec!["policy.md".into()]));
        assert_eq!(fs::read_to_string(folder.0.join("policy.md")).unwrap(), "Cutoff is 5 pm, said the person.\n");
        copies.discard("rt_abc").unwrap();
        assert!(!copy.exists());
        assert_eq!(copies.source("rt_abc"), None);
    }

    #[test]
    fn links_the_folders_dependencies_so_a_check_can_run_in_it_and_discarding_never_reaches_through_the_link() {
        let (folder, root) = (temp("locust-rtcopy-folder-"), temp("locust-rtcopy-root-"));
        fs::create_dir_all(folder.0.join("node_modules/pkg")).unwrap();
        fs::write(folder.0.join("node_modules/pkg/index.js"), "the real dependency").unwrap();
        let copies = RoutineCopies::new(&root.0);
        let copy = copies.make("rt_deps", &folder.0).unwrap();
        assert_eq!(fs::read_to_string(copy.join("node_modules/pkg/index.js")).unwrap(), "the real dependency");
        // Not counted as a change the routine made.
        assert_eq!(copies.changes("rt_deps").unwrap(), CopyChanges::default());
        copies.discard("rt_deps").unwrap();
        assert!(!copy.exists());
        assert_eq!(fs::read_to_string(folder.0.join("node_modules/pkg/index.js")).unwrap(), "the real dependency");
        // And a fresh copy over an old one unlinks first, too.
        copies.make("rt_deps", &folder.0).unwrap();
        copies.make("rt_deps", &folder.0).unwrap();
        assert_eq!(fs::read_to_string(folder.0.join("node_modules/pkg/index.js")).unwrap(), "the real dependency");
    }

    #[test]
    fn cannot_be_named_so_as_to_climb_out_of_its_root() {
        let root = temp("locust-rtcopy-root-");
        let copies = RoutineCopies::new(&root.0);
        assert!(copies.path("../rt_x").is_err());
        assert!(copies.path("rt_a/../../b").is_err());
    }

    #[test]
    fn a_folder_too_big_to_copy_says_so_and_copies_nothing() {
        let (folder, root) = (temp("locust-rtcopy-folder-"), temp("locust-rtcopy-root-"));
        for n in 0..3 {
            fs::write(folder.0.join(format!("f{n}.txt")), "x").unwrap();
        }
        let copies = RoutineCopies::new(&root.0).with_limits(Limits { files: 2, bytes: MAX_COPY_BYTES });
        assert_eq!(
            copies.make("rt_big", &folder.0).unwrap_err(),
            "It answers in a copy of the folder, and this folder is too big to copy (more than 2 files or 1024 MB)."
        );
        assert!(!copies.path("rt_big").unwrap().exists());
        assert_eq!(too_big(LIMITS), "more than 20,000 files or 1024 MB");
    }
}
