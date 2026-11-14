//! Bounded byte checkpoints owned by native. Never resets a user's index or branch.
use crate::{changes, store::Store};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
const MAX_FILE: u64 = 10 * 1024 * 1024;
type Files = BTreeMap<String, String>;
type Active = BTreeMap<PathBuf, Vec<Arc<AtomicBool>>>;
static ACTIVE: OnceLock<Mutex<Active>> = OnceLock::new();
static UNDO: Mutex<()> = Mutex::new(());
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn base(store: &Store, workspace: &Path) -> io::Result<PathBuf> {
    let workspace = workspace.canonicalize()?;
    store.check(
        &store
            .ledger_dir()?
            .parent()
            .ok_or_else(|| io::Error::other("No data folder"))?
            .join("checkpoints")
            .join(format!(
                "{}.plain",
                &hash(workspace.to_string_lossy().to_lowercase().as_bytes())[..32]
            )),
    )
}
/// Write beside, then rename into place so a reader never sees half a file (Electron `writeWhole`).
/// A leftover corrupt final name is replaced; a complete match is left alone.
fn save(store: &Store, dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let path = dir.join(name);
    if path.is_file()
        && fs::read(&path)
            .ok()
            .is_some_and(|existing| existing == bytes)
    {
        return Ok(());
    }
    let tmp = format!(
        "{name}.{}.part",
        crate::native::uuid_v4().unwrap_or_else(|_| format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    );
    let mut file = store.create_new_file_in(dir, &tmp)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(dir.join(&tmp), &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(dir.join(&tmp));
    }
    result
}
fn safe(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let rel = Path::new(relative);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) || relative.is_empty() {
        return Err(io::Error::other("outside this folder"));
    }
    let mut path = root.to_owned();
    for c in rel.components() {
        path.push(c);
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(io::Error::other("symbolic link left alone"));
        }
    }
    Ok(path)
}
fn bytes(path: &Path) -> io::Result<Vec<u8>> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    let mut out = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_FILE + 1)
        .read_to_end(&mut out)?;
    if out.len() as u64 > MAX_FILE {
        return Err(io::Error::other("file too large"));
    }
    Ok(out)
}
fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<String>) -> io::Result<()> {
    if depth > 32 {
        return Err(io::Error::other(
            "This folder is nested more than 32 folders deep, too deep to keep a copy of before each turn.",
        ));
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if kind.is_dir() {
            if !name.starts_with('.')
                && !matches!(
                    name.as_str(),
                    "node_modules" | "__pycache__" | "venv" | "site-packages"
                )
            {
                walk(root, &entry.path(), depth + 1, out)?
            }
        } else if kind.is_file()
            && !name.starts_with("~$")
            && !matches!(
                name.to_lowercase().as_str(),
                "desktop.ini" | "thumbs.db" | ".ds_store"
            )
        {
            out.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_string_lossy()
                    .into_owned(),
            )
        }
        if out.len() > 20_000 {
            return Err(io::Error::other(
                "This folder has more than 20,000 files, too many to keep a copy of before each turn.",
            ));
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub files: Files,
    pub not_kept: Vec<String>,
}
pub fn take(store: &Store, workspace: &Path) -> io::Result<Snapshot> {
    take_with_git(
        store,
        workspace,
        changes::git(workspace, &["--version"]).is_ok(),
    )
}
fn take_with_git(store: &Store, workspace: &Path, git_here: bool) -> io::Result<Snapshot> {
    let dir = base(store, workspace)?;
    if dir.starts_with(workspace) {
        return Err(io::Error::other(
            "Checkpoint data cannot be inside the folder being kept.",
        ));
    }
    let mut listed = vec![];
    // A private bare index lists every file the project's ignore rules allow, even tracked ignored files,
    // exactly as Electron's checkpoint store does. No project filters are executed.
    if git_here {
        let git_dir = dir.join("listing.git");
        // Create the directory safely before asking git to initialise its own metadata.
        save(store, &git_dir, "locust-owner", b"native")?;
        changes::git(
            workspace,
            &["init", "-q", "--bare", &git_dir.to_string_lossy()],
        )
        .map_err(io::Error::other)?;
        let output = changes::git(
            workspace,
            &[
                &format!("--git-dir={}", git_dir.display()),
                &format!("--work-tree={}", workspace.display()),
                "ls-files",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        )
        .map_err(io::Error::other)?;
        listed.extend(
            output
                .split('\0')
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
        );
    } else {
        walk(workspace, workspace, 0, &mut listed)?
    }
    if listed.len() > 20_000 {
        return Err(io::Error::other(
            "This folder has more than 20,000 files, too many to keep a copy of before each turn.",
        ));
    }
    let mut files = Files::new();
    let mut not_kept = vec![];
    let mut total = 0u64;
    for p in listed {
        let full = match safe(workspace, &p) {
            Ok(p) => p,
            Err(_) => continue,
        };
        // One unreadable path must not kill the whole snapshot (Electron skips with lstat/read catch).
        let info = match fs::symlink_metadata(&full) {
            Ok(info) => info,
            Err(_) => continue,
        };
        if !info.is_file() {
            continue;
        }
        if info.len() > MAX_FILE {
            not_kept.push(p);
            continue;
        }
        total += info.len();
        if total > 300 * 1024 * 1024 {
            return Err(io::Error::other(
                "This folder holds more than 300 MB of files, too much to keep a copy of before each turn.",
            ));
        }
        let content = match bytes(&full) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let hash = hash(&content);
        save(store, &dir.join("blobs"), &hash, &content)?;
        files.insert(p, hash);
    }
    Ok(Snapshot { files, not_kept })
}
pub struct Turn {
    workspace: PathBuf,
    before: io::Result<Snapshot>,
    shared: Arc<AtomicBool>,
}
impl Turn {
    pub fn begin(store: &Store, workspace: &Path) -> Self {
        let _undo = UNDO.lock().unwrap_or_else(|e| e.into_inner());
        let workspace = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.into());
        let shared = Arc::new(AtomicBool::new(false));
        if let Ok(mut active) = ACTIVE.get_or_init(Mutex::default).lock() {
            let entries = active.entry(workspace.clone()).or_default();
            if !entries.is_empty() {
                shared.store(true, Ordering::SeqCst);
                for e in entries.iter() {
                    e.store(true, Ordering::SeqCst)
                }
            }
            entries.push(shared.clone())
        } else {
            shared.store(true, Ordering::SeqCst)
        }
        Self {
            before: take(store, &workspace),
            workspace,
            shared,
        }
    }
    pub fn finish(self, store: &Store, id: &str) -> io::Result<()> {
        self.finish_reporting_edits(store, id).map(|_| ())
    }
    pub fn finish_reporting_edits(self, store: &Store, id: &str) -> io::Result<bool> {
        self.finish_reporting_changes(store, id).map(|(edited, _)| edited)
    }

    /// As `finish_reporting_edits`, with the paths the host itself saw change (A2.17): None when another run shared
    /// the folder, since then the host cannot say whose change a path was.
    pub fn finish_reporting_changes(self, store: &Store, id: &str) -> io::Result<(bool, Option<Vec<String>>)> {
        let after = take(store, &self.workspace);
        let before = self
            .before
            .as_ref()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let after = after?;
        let edited = before.files != after.files;
        let mut not = before.not_kept.clone();
        not.extend(after.not_kept);
        not.sort();
        not.dedup();
        let value = json!({"workspace":self.workspace,"before":before.files,"after":after.files,"notKept":not,"shared":self.shared.load(Ordering::SeqCst)});
        save(
            store,
            &base(store, &self.workspace)?.join("turns"),
            &format!("{}.json", valid_id(id)?),
            value.to_string().as_bytes(),
        )?;
        let seen = (!self.shared.load(Ordering::SeqCst)).then(|| changed(&value));
        Ok((edited, seen))
    }
}
impl Drop for Turn {
    fn drop(&mut self) {
        if let Ok(mut a) = ACTIVE.get_or_init(Mutex::default).lock()
            && let Some(v) = a.get_mut(&self.workspace)
        {
            v.retain(|e| !Arc::ptr_eq(e, &self.shared));
            if v.is_empty() {
                a.remove(&self.workspace);
            }
        }
    }
}
fn valid_id(id: &str) -> io::Result<&str> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Err(io::Error::other("Invalid turn"))
    } else {
        Ok(id)
    }
}
pub fn records(store: &Store, workspace: &Path) -> io::Result<Vec<(String, Value)>> {
    let dir = base(store, workspace)?.join("turns");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out: Vec<(String, Value)> = vec![];
    for e in fs::read_dir(dir)? {
        let e = e?;
        if let Some(id) = e.path().file_stem().and_then(|p| p.to_str()) {
            let mut value: Value = serde_json::from_slice(&fs::read(e.path())?)?;
            let receipt = base(store, workspace)?
                .join("receipts")
                .join(format!("{id}.json"));
            if receipt.exists() {
                value["undone"] = serde_json::from_slice(&fs::read(receipt)?)?;
            }
            out.push((id.into(), value));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out)
}

/// Read-only per-turn diffs reconstructed from checkpoint before/after blobs. Empty when shared or absent.
pub fn turn_diffs(
    store: &Store,
    workspace: &Path,
    id: &str,
) -> io::Result<Vec<crate::changes::File>> {
    let id = valid_id(id)?;
    let workspace = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
    let dir = base(store, &workspace)?;
    let turn_path = dir.join("turns").join(format!("{id}.json"));
    if !turn_path.exists() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_slice(&fs::read(turn_path)?)?;
    if value["shared"] == true {
        return Ok(Vec::new());
    }
    let before: Files = serde_json::from_value(value["before"].clone()).unwrap_or_default();
    let after: Files = serde_json::from_value(value["after"].clone()).unwrap_or_default();
    let mut out = Vec::new();
    for path in changed(&value) {
        let b_hash = before.get(&path);
        let a_hash = after.get(&path);
        if b_hash.is_none() && a_hash.is_none() {
            continue;
        }
        let status = match (b_hash, a_hash) {
            (None, Some(_)) => "ADDED",
            (Some(_), None) => "DELETED",
            _ => "MODIFIED",
        };
        let b_bytes = match b_hash {
            Some(h) => blob(&dir, h).unwrap_or_default(),
            None => Vec::new(),
        };
        let a_bytes = match a_hash {
            Some(h) => blob(&dir, h).unwrap_or_default(),
            None => Vec::new(),
        };
        let hunks = if b_bytes.contains(&0) || a_bytes.contains(&0) {
            Vec::new()
        } else if let (Ok(old), Ok(new)) = (std::str::from_utf8(&b_bytes), std::str::from_utf8(&a_bytes)) {
            unified_hunks(old, new)
        } else {
            Vec::new()
        };
        out.push(crate::changes::File {
            path,
            status: status.into(),
            hunks,
        });
    }
    Ok(out)
}

fn split_diff_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = s.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// Build unified-diff hunks (3-line context) between two text versions.
pub(crate) fn unified_hunks(old: &str, new: &str) -> Vec<crate::changes::Hunk> {
    use crate::changes::{Hunk, Row};
    let a = split_diff_lines(old);
    let b = split_diff_lines(new);
    let mut prefix = 0usize;
    while prefix < a.len() && prefix < b.len() && a[prefix] == b[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while prefix + suffix < a.len()
        && prefix + suffix < b.len()
        && a[a.len() - 1 - suffix] == b[b.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let am = a.len() - prefix - suffix;
    let bm = b.len() - prefix - suffix;
    if am == 0 && bm == 0 {
        return Vec::new();
    }
    let mut ops: Vec<(char, Option<usize>, Option<usize>, String)> = Vec::with_capacity(a.len() + b.len());
    for i in 0..prefix {
        ops.push((' ', Some(i + 1), Some(i + 1), a[i].to_owned()));
    }
    let a_mid = &a[prefix..prefix + am];
    let b_mid = &b[prefix..prefix + bm];
    if (am + 1).saturating_mul(bm + 1) <= 2_000_000 {
        let cols = bm + 1;
        let mut dp = vec![0u32; (am + 1) * cols];
        for i in (0..am).rev() {
            for j in (0..bm).rev() {
                dp[i * cols + j] = if a_mid[i] == b_mid[j] {
                    dp[(i + 1) * cols + (j + 1)] + 1
                } else {
                    dp[(i + 1) * cols + j].max(dp[i * cols + (j + 1)])
                };
            }
        }
        let (mut i, mut j) = (0usize, 0usize);
        while i < am || j < bm {
            if i < am && j < bm && a_mid[i] == b_mid[j] {
                ops.push((' ', Some(prefix + i + 1), Some(prefix + j + 1), a_mid[i].to_owned()));
                i += 1;
                j += 1;
            } else if i < am && (j == bm || dp[(i + 1) * cols + j] >= dp[i * cols + (j + 1)]) {
                ops.push(('-', Some(prefix + i + 1), None, a_mid[i].to_owned()));
                i += 1;
            } else {
                ops.push(('+', None, Some(prefix + j + 1), b_mid[j].to_owned()));
                j += 1;
            }
        }
    } else {
        for (i, line) in a_mid.iter().enumerate() {
            ops.push(('-', Some(prefix + i + 1), None, (*line).to_owned()));
        }
        for (j, line) in b_mid.iter().enumerate() {
            ops.push(('+', None, Some(prefix + j + 1), (*line).to_owned()));
        }
    }
    for k in 0..suffix {
        ops.push((
            ' ',
            Some(a.len() - suffix + k + 1),
            Some(b.len() - suffix + k + 1),
            a[a.len() - suffix + k].to_owned(),
        ));
    }

    const CTX: usize = 3;
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (idx, op) in ops.iter().enumerate() {
        if op.0 == ' ' {
            continue;
        }
        let start = idx.saturating_sub(CTX);
        let end = (idx + CTX + 1).min(ops.len());
        if let Some(last) = ranges.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            ranges.push((start, end));
        }
    }

    ranges
        .into_iter()
        .map(|(start, end)| {
            let slice = &ops[start..end];
            let old_start = slice.iter().find_map(|o| o.1).unwrap_or(0);
            let new_start = slice.iter().find_map(|o| o.2).unwrap_or(0);
            let rows = slice
                .iter()
                .map(|(kind, old_no, new_no, text)| Row {
                    kind: *kind,
                    old: *old_no,
                    new: *new_no,
                    text: text.clone(),
                })
                .collect();
            Hunk {
                old: old_start,
                new: new_start,
                heading: String::new(),
                rows,
            }
        })
        .collect()
}
/// Undo's receipt for a turn, if it was undone: what was put back and what was left (read only; no lock is taken).
pub fn undone(store: &Store, workspace: &Path, id: &str) -> Option<Value> {
    let workspace = workspace.canonicalize().ok()?;
    let receipt = base(store, &workspace).ok()?.join("receipts").join(format!("{}.json", valid_id(id).ok()?));
    serde_json::from_slice(&fs::read(receipt).ok()?).ok()
}
/// A receipt, as Undo leaves one, for a test of what follows an undone turn.
#[cfg(test)]
pub(crate) fn put_receipt(store: &Store, workspace: &Path, id: &str, receipt: &Value) {
    let dir = base(store, workspace).unwrap().join("receipts");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{id}.json")), receipt.to_string()).unwrap();
}
pub fn changed(v: &Value) -> Vec<String> {
    let before: Files = serde_json::from_value(v["before"].clone()).unwrap_or_default();
    let after: Files = serde_json::from_value(v["after"].clone()).unwrap_or_default();
    let mut paths: Vec<_> = before
        .keys()
        .chain(after.keys())
        .filter(|p| {
            before.get(*p) != after.get(*p)
                && !v["notKept"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|n| n.as_str() == Some(p.as_str())))
        })
        .cloned()
        .collect();
    paths.sort();
    paths.dedup();
    paths
}
pub fn undo(store: &Store, workspace: &Path, id: &str) -> io::Result<Value> {
    let canonical = workspace.canonicalize()?;
    let workspace = canonical.as_path();
    let _lock = UNDO
        .lock()
        .map_err(|_| io::Error::other("Undo unavailable"))?;
    if ACTIVE
        .get_or_init(Mutex::default)
        .lock()
        .map_err(|_| io::Error::other("Undo unavailable"))?
        .contains_key(workspace)
    {
        return Err(io::Error::other("A turn is still running in this folder."));
    }
    let dir = base(store, workspace)?;
    let value: Value = serde_json::from_slice(&fs::read(
        dir.join("turns").join(format!("{}.json", valid_id(id)?)),
    )?)?;
    let receipt = dir.join("receipts").join(format!("{id}.json"));
    if receipt.exists() {
        return serde_json::from_slice(&fs::read(receipt)?).map_err(io::Error::other);
    }
    if value["shared"] == true {
        return Err(io::Error::other(
            "This turn shared its folder with another run.",
        ));
    }
    let before: Files = serde_json::from_value(value["before"].clone())?;
    let after: Files = serde_json::from_value(value["after"].clone())?;
    // Claim before any filesystem mutation: an interrupted Undo is never retried.
    let mut claim = store.create_new_file_in(&dir.join("claims"), &format!("{id}.json"))?;
    claim.write_all(b"Undo started; do not retry")?;
    claim.sync_all()?;
    let mut restored = vec![];
    let mut left = vec![];
    for p in changed(&value) {
        let result = (|| -> io::Result<()> {
            let full = safe(workspace, &p)?;
            store.check(&full)?;
            let current = match bytes(&full) {
                Ok(b) => Some(hash(&b)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            if current.as_ref() != after.get(&p) {
                return Err(io::Error::other(if current.is_none() {
                    "removed since the turn"
                } else {
                    "changed since the turn"
                }));
            }
            let content = before
                .get(&p)
                .map(|was| {
                    let content = fs::read(dir.join("blobs").join(was))?;
                    if hash(&content) != *was {
                        return Err(io::Error::other("kept bytes are damaged"));
                    }
                    Ok(content)
                })
                .transpose()?;
            restore(workspace, &p, after.get(&p), content.as_deref())?;
            Ok(())
        })();
        match result {
            Ok(()) => restored.push(p),
            Err(e) => left.push(json!({"path":p,"why":e.to_string()})),
        }
    }
    let receipt = json!({"restored":restored,"leftAlone":left});
    save(
        store,
        &dir.join("receipts"),
        &format!("{id}.json"),
        receipt.to_string().as_bytes(),
    )?;
    Ok(receipt)
}

/// What putting back the files some turns changed would do (Electron 0.502 `reverseChanges`, 0.512 all or none): the
/// files that can go back exactly as they were before the first of those turns, and each that cannot, with why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PutBackPlan {
    /// Files that would go back, sorted.
    pub files: Vec<String>,
    /// Files (or turns with no record) that cannot, with why. Any one of them holds back the whole set.
    pub cannot: Vec<(String, String)>,
    steps: Vec<PutBackStep>,
    /// The turns whose own changes the set holds, each with the files it changed that go back.
    turns: Vec<(String, Vec<String>)>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PutBackStep {
    path: String,
    /// The hash the file has now (the newest turn's after), which the write checks again.
    expected: Option<String>,
    /// The hash and bytes it goes back to; None: the file did not exist, so it goes.
    target_hash: Option<String>,
    target: Option<Vec<u8>>,
    /// The bytes it has now, to write back if a later file in the set cannot be written.
    now: Option<Vec<u8>>,
}

fn blob(dir: &Path, hash_wanted: &str) -> io::Result<Vec<u8>> {
    let content = fs::read(dir.join("blobs").join(hash_wanted))?;
    if hash(&content) != hash_wanted {
        return Err(io::Error::other("kept bytes are damaged"));
    }
    Ok(content)
}

/// Read only. The turns are the ones an edit sets aside, oldest first. A turn already undone is skipped (its files are
/// back); a turn with no record of what it changed, or one that shared its folder with another run, holds the set
/// back; a file that changed since the newest turn left it, or whose turns' records do not follow one another, cannot
/// go back exactly.
pub fn plan_put_back(store: &Store, workspace: &Path, turns: &[String]) -> io::Result<PutBackPlan> {
    let workspace = workspace.canonicalize()?;
    let dir = base(store, &workspace)?;
    let mut chains: BTreeMap<String, Vec<(String, Option<String>, Option<String>)>> = BTreeMap::new();
    let mut plan = PutBackPlan::default();
    for id in turns {
        if valid_id(id).is_err() {
            plan.cannot.push((id.clone(), "it is not a reply of this folder".into()));
            continue;
        }
        if dir.join("receipts").join(format!("{id}.json")).exists() {
            continue;
        }
        let Some(value) = fs::read(dir.join("turns").join(format!("{id}.json"))).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
            plan.cannot.push((format!("a reply ({id})"), "no record of what it changed was kept".into()));
            continue;
        };
        let paths = changed(&value);
        if value["shared"] == true && !paths.is_empty() {
            plan.cannot.extend(paths.into_iter().map(|p| (p, "the reply shared this folder with another run".into())));
            continue;
        }
        let before: Files = serde_json::from_value(value["before"].clone()).unwrap_or_default();
        let after: Files = serde_json::from_value(value["after"].clone()).unwrap_or_default();
        for p in paths {
            chains.entry(p.clone()).or_default().push((id.clone(), before.get(&p).cloned(), after.get(&p).cloned()));
        }
    }
    'files: for (path, chain) in chains {
        if chain.windows(2).any(|w| w[0].2 != w[1].1) {
            plan.cannot.push((path, "the replies' recorded changes do not follow one another".into()));
            continue;
        }
        let checked = (|| -> io::Result<Option<PutBackStep>> {
            let full = safe(&workspace, &path)?;
            store.check(&full)?;
            let now = match bytes(&full) {
                Ok(b) => Some(b),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            let (_, _, newest) = chain.last().expect("a chain has a turn");
            let (_, oldest, _) = &chain[0];
            let current = now.as_ref().map(|b| hash(b));
            if current != *newest {
                return Err(io::Error::other(if current.is_none() { "removed since the replies" } else { "changed since the replies" }));
            }
            if current == *oldest {
                return Ok(None);
            }
            let target = oldest.as_ref().map(|h| blob(&dir, h)).transpose()?;
            Ok(Some(PutBackStep { path: path.clone(), expected: current, target_hash: oldest.clone(), target, now }))
        })();
        match checked {
            Ok(Some(step)) => plan.steps.push(step),
            Ok(None) => continue 'files,
            Err(e) => plan.cannot.push((path, e.to_string())),
        }
    }
    plan.files = plan.steps.iter().map(|s| s.path.clone()).collect();
    for id in turns {
        let own: Vec<String> = plan.steps.iter().map(|s| s.path.clone()).filter(|p| changed_by(&dir, id, p)).collect();
        if !own.is_empty() {
            plan.turns.push((id.clone(), own));
        }
    }
    Ok(plan)
}

/// Whether the turn's record lists this path as changed.
fn changed_by(dir: &Path, id: &str, path: &str) -> bool {
    fs::read(dir.join("turns").join(format!("{id}.json"))).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v| changed(&v).iter().any(|p| p == path))
}

/// 0.502 and 0.512: put back what the set-aside turns changed, all of it or none. Nothing is written when any file
/// cannot go back (the answer says which could have: `heldBack`); a write that fails puts back the ones already written.
/// The turns whose files went back are then marked undone, as Undo marks one. Never while a turn runs in the folder.
/// Answers `{"putBack": [...], "leftAlone": [{path, why}], "heldBack": [...]}`.
pub fn put_back(store: &Store, workspace: &Path, turns: &[String]) -> io::Result<Value> {
    let canonical = workspace.canonicalize()?;
    let workspace = canonical.as_path();
    let _lock = UNDO.lock().map_err(|_| io::Error::other("Undo unavailable"))?;
    if ACTIVE.get_or_init(Mutex::default).lock().map_err(|_| io::Error::other("Undo unavailable"))?.contains_key(workspace) {
        return Err(io::Error::other("A turn is still running in this folder."));
    }
    let plan = plan_put_back(store, workspace, turns)?;
    let words = |held: &[(String, String)]| held.iter().map(|(path, why)| json!({"path": path, "why": why})).collect::<Vec<_>>();
    if !plan.cannot.is_empty() {
        return Ok(json!({"putBack": [], "leftAlone": words(&plan.cannot), "heldBack": plan.files}));
    }
    let mut done: Vec<&PutBackStep> = Vec::new();
    for step in &plan.steps {
        if let Err(error) = restore(workspace, &step.path, step.expected.as_ref(), step.target.as_deref()) {
            // The ones already written go back to how they were, so the folder is never half put back.
            for undo in done.iter().rev() {
                let _ = restore(workspace, &undo.path, undo.target_hash.as_ref(), undo.now.as_deref());
            }
            let others: Vec<&String> = plan.files.iter().filter(|p| **p != step.path).collect();
            return Ok(json!({"putBack": [], "leftAlone": [{"path": step.path, "why": format!("it could not be written ({error})")}], "heldBack": others}));
        }
        done.push(step);
    }
    // Each turn's files are back: Undo has nothing to do on it, and the panel shows it undone.
    let dir = base(store, workspace)?;
    for (id, own) in &plan.turns {
        let receipt = json!({"restored": own, "leftAlone": [], "putBackByEdit": true});
        if let Err(error) = save(store, &dir.join("receipts"), &format!("{id}.json"), receipt.to_string().as_bytes()) {
            eprintln!("The put-back receipt for {id} was not kept: {error}");
        }
    }
    Ok(json!({"putBack": plan.files, "leftAlone": [], "heldBack": []}))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "undo-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(p.join("workspace")).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn setup() -> (Temp, Store, PathBuf) {
        let t = Temp::new();
        let store = Store::with_codex(t.0.join("native"), t.0.join("electron"), vec![]).unwrap();
        let ws = t.0.join("workspace");
        fs::write(ws.join("notes"), b"one\r\ntwo\r\n").unwrap();
        fs::write(ws.join("gone"), b"\0binary\xff").unwrap();
        (t, store, ws)
    }
    fn edit(ws: &Path) {
        fs::write(ws.join("notes"), b"one\r\nTWO\r\n").unwrap();
        fs::remove_file(ws.join("gone")).unwrap();
        fs::write(ws.join("made"), b"new\n").unwrap();
    }
    #[test]
    fn restores_bytes_edits_additions_and_deletions_once() {
        let (_t, s, w) = setup();
        let turn = Turn::begin(&s, &w);
        edit(&w);
        turn.finish(&s, "run_1").unwrap();
        let records = records(&s, &w).unwrap();
        assert_eq!(changed(&records[0].1), vec!["gone", "made", "notes"]);
        let result = undo(&s, &w, "run_1").unwrap();
        assert_eq!(result["restored"].as_array().unwrap().len(), 3);
        assert_eq!(fs::read(w.join("notes")).unwrap(), b"one\r\ntwo\r\n");
        assert_eq!(fs::read(w.join("gone")).unwrap(), b"\0binary\xff");
        assert!(!w.join("made").exists());
        fs::write(w.join("made"), "made again").unwrap();
        assert_eq!(undo(&s, &w, "run_1").unwrap(), result);
        assert_eq!(fs::read_to_string(w.join("made")).unwrap(), "made again");
        assert!(!w.join(".git").exists());
    }
    #[test]
    fn later_edits_and_removals_are_left_alone() {
        let (_t, s, w) = setup();
        let turn = Turn::begin(&s, &w);
        edit(&w);
        turn.finish(&s, "run_1").unwrap();
        fs::write(w.join("notes"), "the person wrote this").unwrap();
        fs::remove_file(w.join("made")).unwrap();
        let r = undo(&s, &w, "run_1").unwrap();
        assert_eq!(r["leftAlone"].as_array().unwrap().len(), 2);
        assert_eq!(
            fs::read_to_string(w.join("notes")).unwrap(),
            "the person wrote this"
        );
        assert!(
            r["leftAlone"]
                .to_string()
                .contains("removed since the turn")
        );
    }
    #[test]
    fn overlapping_turns_are_never_offered_or_undone() {
        let (_t, s, w) = setup();
        let a = Turn::begin(&s, &w);
        let b = Turn::begin(&s, &w);
        edit(&w);
        assert!(undo(&s, &w, "absent").is_err());
        a.finish(&s, "run_a").unwrap();
        b.finish(&s, "run_b").unwrap();
        for (_, r) in records(&s, &w).unwrap() {
            assert_eq!(r["shared"], true)
        }
        assert!(undo(&s, &w, "run_a").is_err());
    }
    #[test]
    fn without_git_walk_skips_dot_folders_and_dependencies() {
        let (_t, s, w) = setup();
        for name in [
            ".locust",
            ".git",
            "node_modules",
            "venv",
            "__pycache__",
            "site-packages",
        ] {
            fs::create_dir(w.join(name)).unwrap();
            fs::write(w.join(name).join("x"), "skip").unwrap()
        }
        let a = take_with_git(&s, &w, false).unwrap();
        assert_eq!(a.files.len(), 2);
        edit(&w);
        let b = take_with_git(&s, &w, false).unwrap();
        save(
            &s,
            &base(&s, &w).unwrap().join("turns"),
            "plain.json",
            json!({"before":a.files,"after":b.files,"notKept":[]})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(
            undo(&s, &w, "plain").unwrap()["restored"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn git_ignore_and_raw_bytes_never_run_filters_or_fsmonitor() {
        let (_t, s, w) = setup();
        fs::write(w.join(".gitignore"), "ignored\n").unwrap();
        fs::write(w.join("ignored"), "skip").unwrap();
        fs::write(w.join(".gitattributes"), "* filter=evil\n").unwrap();
        changes::git(&w, &["init", "-q"]).unwrap();
        changes::git(&w, &["config", "core.fsmonitor", "touch should-not-exist"]).unwrap();
        let a = take(&s, &w).unwrap();
        assert!(!a.files.contains_key("ignored"));
        assert!(!w.join("should-not-exist").exists());
        assert_eq!(
            fs::read(base(&s, &w).unwrap().join("blobs").join(&a.files["notes"])).unwrap(),
            b"one\r\ntwo\r\n"
        );
    }
    #[test]
    fn too_large_files_named_never_deleted_by_undo() {
        let (_t, s, w) = setup();
        fs::write(w.join("large"), vec![0; MAX_FILE as usize + 1]).unwrap();
        let a = Turn::begin(&s, &w);
        assert_eq!(a.before.as_ref().unwrap().not_kept, vec!["large"]);
        fs::write(w.join("large"), "small now").unwrap();
        edit(&w);
        a.finish(&s, "large_turn").unwrap();
        undo(&s, &w, "large_turn").unwrap();
        assert_eq!(fs::read_to_string(w.join("large")).unwrap(), "small now");
    }
    #[test]
    fn refuses_traversal_and_symlink_restore() {
        let (_t, s, w) = setup();
        assert!(safe(&w, "../outside").is_err());
        assert!(safe(&w, "/absolute").is_err());
        let a = Turn::begin(&s, &w);
        edit(&w);
        a.finish(&s, "turn").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("gone", w.join("notes-link")).unwrap();
            assert!(safe(&w, "notes-link").is_err());
            fs::remove_file(w.join("notes")).unwrap();
            std::os::unix::fs::symlink("gone", w.join("notes")).unwrap();
            let r = undo(&s, &w, "turn").unwrap();
            assert!(r["leftAlone"].to_string().contains("symbolic link"));
        }
    }
    #[test]
    fn save_replaces_corrupt_blob_atomically() {
        let (_t, s, w) = setup();
        let dir = base(&s, &w).unwrap().join("blobs");
        fs::create_dir_all(&dir).unwrap();
        let content = b"hello-checkpoint-bytes";
        let name = hash(content);
        fs::write(dir.join(&name), b"partial").unwrap();
        save(&s, &dir, &name, content).unwrap();
        assert_eq!(fs::read(dir.join(&name)).unwrap(), content);
        save(&s, &dir, &name, content).unwrap();
        assert_eq!(fs::read(dir.join(&name)).unwrap(), content);
        assert!(
            !fs::read_dir(&dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().ends_with(".part"))
        );
    }
    #[test]
    fn unreadable_paths_are_skipped_not_fatal() {
        let (_t, s, w) = setup();
        fs::write(w.join("ok"), b"yes").unwrap();
        fs::write(w.join("secret"), b"no").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(w.join("secret"), fs::Permissions::from_mode(0o000)).unwrap();
        }
        let snap = take_with_git(&s, &w, false).unwrap();
        assert!(snap.files.contains_key("ok"));
        assert!(!snap.files.contains_key("secret"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(w.join("secret"), fs::Permissions::from_mode(0o644));
        }
    }
    #[test]
    fn restore_keeps_existing_file_mode() {
        let (_t, s, w) = setup();
        fs::write(w.join("tool.sh"), b"#!/bin/sh\necho a\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(w.join("tool.sh"), fs::Permissions::from_mode(0o755)).unwrap();
            let turn = Turn::begin(&s, &w);
            fs::write(w.join("tool.sh"), b"#!/bin/sh\necho b\n").unwrap();
            fs::set_permissions(w.join("tool.sh"), fs::Permissions::from_mode(0o755)).unwrap();
            turn.finish(&s, "mode_turn").unwrap();
            undo(&s, &w, "mode_turn").unwrap();
            assert_eq!(fs::read(w.join("tool.sh")).unwrap(), b"#!/bin/sh\necho a\n");
            assert_eq!(
                fs::metadata(w.join("tool.sh"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }
    }
}

/// Hold each parent directory open without following links, including during the final write/removal.
#[cfg(unix)]
fn restore(
    root: &Path,
    relative: &str,
    expected: Option<&String>,
    content: Option<&[u8]>,
) -> io::Result<()> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd, OwnedFd},
            unix::ffi::OsStrExt,
        },
    };
    let c = |p: &std::ffi::OsStr| CString::new(p.as_bytes()).map_err(io::Error::other);
    let root_c = c(root.as_os_str())?;
    let raw = unsafe {
        libc::open(
            root_c.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut parent = unsafe { OwnedFd::from_raw_fd(raw) };
    let parts: Vec<_> = Path::new(relative).components().collect();
    for part in &parts[..parts.len() - 1] {
        let name = c(part.as_os_str())?;
        let mut raw = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
            )
        };
        if raw < 0
            && content.is_some()
            && io::Error::last_os_error().kind() == io::ErrorKind::NotFound
        {
            if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o755) } < 0 {
                return Err(io::Error::last_os_error());
            }
            raw = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                )
            };
        }
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        parent = unsafe { OwnedFd::from_raw_fd(raw) };
    }
    let name = c(parts
        .last()
        .ok_or_else(|| io::Error::other("outside this folder"))?
        .as_os_str())?;
    let raw = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW,
        )
    };
    // Keep the existing mode across replace-by-rename (Electron truncates in place and preserves it).
    let mut mode: libc::mode_t = 0o644;
    let current = if raw < 0 {
        if io::Error::last_os_error().kind() != io::ErrorKind::NotFound {
            return Err(io::Error::last_os_error());
        }
        None
    } else {
        let mut f = unsafe { fs::File::from_raw_fd(raw) };
        let meta = f.metadata()?;
        if !meta.is_file() {
            return Err(io::Error::other("not a regular file"));
        }
        use std::os::unix::fs::PermissionsExt;
        mode = meta.permissions().mode() as libc::mode_t & 0o7777;
        let mut b = Vec::new();
        Read::by_ref(&mut f)
            .take(MAX_FILE + 1)
            .read_to_end(&mut b)?;
        Some(hash(&b))
    };
    if current.as_ref() != expected {
        return Err(io::Error::other("changed since the turn"));
    }
    if let Some(content) = content {
        // Replace the directory entry so a hard link cannot change another file.
        let temp = CString::new(format!(
            ".locust-undo-{}",
            crate::checkpoints::hash(relative.as_bytes())
        ))
        .map_err(io::Error::other)?;
        let raw = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                temp.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
                0o644,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut f = unsafe { fs::File::from_raw_fd(raw) };
        f.write_all(content)?;
        f.sync_all()?;
        if unsafe { libc::fchmod(f.as_raw_fd(), mode) } < 0 {
            let err = io::Error::last_os_error();
            let _ = unsafe { libc::unlinkat(parent.as_raw_fd(), temp.as_ptr(), 0) };
            return Err(err);
        }
        if unsafe {
            libc::renameat(
                parent.as_raw_fd(),
                temp.as_ptr(),
                parent.as_raw_fd(),
                name.as_ptr(),
            )
        } < 0
        {
            let err = io::Error::last_os_error();
            let _ = unsafe { libc::unlinkat(parent.as_raw_fd(), temp.as_ptr(), 0) };
            return Err(err);
        }
    } else if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(unix))]
fn restore(
    _root: &Path,
    _relative: &str,
    _expected: Option<&String>,
    _content: Option<&[u8]>,
) -> io::Result<()> {
    Err(io::Error::other(
        "Safe Undo is unavailable on this platform.",
    ))
}
