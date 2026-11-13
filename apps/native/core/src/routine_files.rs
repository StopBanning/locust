//! FOLDER WATCHERS (main/routine-file-watch.ts, 0.522), for routines that run "when a new file arrives in a folder".
//!
//!   - The first look is a BASELINE: what is in the folder then starts nothing.
//!   - A new file counts once it has STOPPED CHANGING for [`SETTLE_MS`].
//!   - A file a teammate wrote is not an arrival. While any run is live, a file that settles is HELD; each turn's
//!     checkpoint record names what it wrote, and those are let go ([`FileArrivals::wrote`]). What is left once runs
//!     have been quiet for [`RELEASE_MS`] came from someone else.
//!   - At most [`MAX_FIRES_PER_HOUR`] runs an hour, per routine.
//!   - Files arriving together go to ONE run, named together.
//!   - Top level only, no hidden or temporary names, and only inside the project folder.
//!
//! Polled, never a file-system watch, and every look is given the time: tests pass a clock, as `routines/schedule.rs`
//! does. One difference from Electron: a native teammate works in a folder of its own, so each watched routine names
//! the project folder its schedule's folder is inside, rather than one folder for the whole window.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

pub const POLL_MS: i64 = 10_000;
pub const SETTLE_MS: i64 = 20_000;
pub const MAX_FIRES_PER_HOUR: usize = 6;
pub const RELEASE_MS: i64 = 15_000;
const MAX_ENTRIES: usize = 2_000;
const MAX_PENDING: usize = 20;
const HOUR_MS: i64 = 3_600_000;

/// `PASSING`: hidden, Office lock and partly downloaded names.
fn passing(name: &str) -> bool {
    if name.starts_with('.') || name.starts_with("~$") {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    ["tmp", "temp", "crdownload", "part", "partial", "download"]
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

/// `path.resolve` without touching the disk: `.` and `..` folded away.
fn resolve(base: &Path, rest: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in base.join(rest).components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The watched folder, absolute, when it is inside the project; None otherwise (`watchedPath`).
pub fn watched_path(project: &Path, folder: &str) -> Option<PathBuf> {
    let root = resolve(project, Path::new(""));
    let path = resolve(&root, Path::new(folder));
    (path != root && path.starts_with(&root)).then_some(path)
}

/// One path, the same however it was spelled: Windows does not tell `C:\Work` from `c:\work` (`samePath`). Electron
/// compares exactly everywhere else, macOS included, and so does this.
fn same_path(one: &Path, other: &Path) -> bool {
    if cfg!(windows) {
        one.to_string_lossy().to_lowercase() == other.to_string_lossy().to_lowercase()
    } else {
        one == other
    }
}

/// What step 1 is told, before its own words: the files, by name, inside the folder (`arrivalNote`).
pub fn arrival_note(folder: &str, files: &[String]) -> String {
    let named = named(folder, files);
    if files.len() == 1 {
        format!("A new file just arrived: {named}. This run is for it.")
    } else {
        format!("{} new files just arrived: {named}. This run is for them.", files.len())
    }
}

/// `inbox/a.pdf, inbox/b.pdf`: each file inside its folder, with forward slashes.
pub fn named(folder: &str, files: &[String]) -> String {
    let where_ = folder.replace('\\', "/");
    files.iter().map(|name| format!("{where_}/{name}")).collect::<Vec<_>>().join(", ")
}

/// One routine to watch: its id, the project folder, and the folder inside it its schedule names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watch {
    pub routine_id: String,
    pub project: PathBuf,
    pub folder: String,
}

#[derive(Default)]
struct Watched {
    project: PathBuf,
    folder: String,
    seen: BTreeSet<String>,
    settling: BTreeMap<String, (u64, i64, i64)>,
    pending: Vec<String>,
    held: Vec<String>,
    fires: Vec<i64>,
    baselined: bool,
}

/// `createFileArrivals`.
#[derive(Default)]
pub struct FileArrivals {
    by_routine: BTreeMap<String, Watched>,
    writing_at: Option<i64>,
}

fn mtime_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

impl FileArrivals {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look at every watched folder. `writing`: a teammate's run is live now, so what settles is held until that
    /// turn says what it wrote.
    pub fn poll(&mut self, watching: &[Watch], now: i64, writing: bool) {
        if writing {
            self.writing_at = Some(now);
        }
        let quiet = self.writing_at.is_none_or(|at| now - at >= RELEASE_MS);
        // A routine no longer watching -- removed, or its schedule changed -- is forgotten.
        self.by_routine.retain(|id, held| {
            watching.iter().any(|w| &w.routine_id == id && w.folder == held.folder && w.project == held.project)
        });
        for watch in watching {
            let Some(path) = watched_path(&watch.project, &watch.folder) else { continue };
            let watched = self.by_routine.entry(watch.routine_id.clone()).or_insert_with(|| Watched {
                project: watch.project.clone(),
                folder: watch.folder.clone(),
                ..Watched::default()
            });
            let names: Vec<String> = match std::fs::read_dir(&path) {
                Ok(entries) => entries
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|name| !passing(name))
                    .take(MAX_ENTRIES)
                    .collect(),
                // Not there yet: a folder made later is watched from the moment it appears.
                Err(_) if !watched.baselined => continue,
                Err(_) => Vec::new(),
            };
            if !watched.baselined {
                watched.seen.extend(names);
                watched.baselined = true;
                continue;
            }
            for name in &names {
                if watched.seen.contains(name) {
                    continue;
                }
                let Ok(meta) = std::fs::metadata(path.join(name)) else { continue };
                let (size, mtime) = (meta.len(), mtime_ms(&meta));
                match watched.settling.get(name) {
                    Some(&(s, m, since)) if s == size && m == mtime => {
                        if now - since < SETTLE_MS {
                            continue;
                        }
                    }
                    _ => {
                        watched.settling.insert(name.clone(), (size, mtime, now));
                        continue;
                    }
                }
                watched.settling.remove(name);
                watched.seen.insert(name.clone());
                if watched.held.len() + watched.pending.len() < MAX_PENDING {
                    watched.held.push(name.clone());
                }
            }
            // A name that went away while settling is let go.
            watched.settling.retain(|name, _| names.contains(name));
            // Runs quiet long enough for every turn to have said what it wrote: what is held came from someone else.
            if quiet && !watched.held.is_empty() {
                for name in std::mem::take(&mut watched.held) {
                    if !watched.pending.contains(&name) {
                        watched.pending.push(name);
                    }
                }
            }
        }
    }

    /// A turn ended having changed these paths, relative to `workspace` with `/`: none of them is an arrival.
    pub fn wrote(&mut self, workspace: &Path, paths: &[String]) {
        for watched in self.by_routine.values_mut() {
            let Some(folder) = watched_path(&watched.project, &watched.folder) else { continue };
            for path in paths {
                let full = resolve(workspace, Path::new(path));
                if !full.parent().is_some_and(|p| same_path(p, &folder)) {
                    continue;
                }
                let Some(name) = full.file_name().and_then(|n| n.to_str()).map(str::to_owned) else { continue };
                watched.settling.remove(&name);
                watched.held.retain(|one| one != &name);
                watched.pending.retain(|one| one != &name);
                // Seen, so it is never new.
                watched.seen.insert(name);
            }
        }
    }

    /// The settled new files a routine may run for now, or none (none waiting, or its hour is full).
    pub fn ready(&mut self, routine_id: &str, now: i64) -> Vec<String> {
        let Some(watched) = self.by_routine.get_mut(routine_id) else { return Vec::new() };
        if watched.pending.is_empty() {
            return Vec::new();
        }
        watched.fires.retain(|at| now - at < HOUR_MS);
        if watched.fires.len() >= MAX_FIRES_PER_HOUR { Vec::new() } else { watched.pending.clone() }
    }

    /// A run started for these files.
    pub fn fired(&mut self, routine_id: &str, files: &[String], now: i64) {
        if let Some(watched) = self.by_routine.get_mut(routine_id) {
            watched.pending.retain(|name| !files.contains(name));
            watched.fires.push(now);
        }
    }
}

#[cfg(test)]
mod tests {
    //! a-file-arriving-starts-its-routine.test.ts: the watcher's own cases.
    use super::*;
    use std::fs;

    const T0: i64 = 1_000_000;

    struct Project(PathBuf);
    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn project() -> Project {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "locust-watch-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("inbox")).unwrap();
        Project(root.canonicalize().unwrap())
    }
    fn watching(root: &Path) -> Vec<Watch> {
        vec![Watch { routine_id: "rt_1".into(), project: root.into(), folder: "inbox".into() }]
    }
    fn set_mtime(path: &Path, ms: i64) {
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64);
        fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
    }

    #[test]
    fn starts_nothing_for_what_is_there_at_the_first_look_and_counts_a_new_file_once_it_stops_changing() {
        let p = project();
        let root = &p.0;
        fs::write(root.join("inbox/old.csv"), "old\n").unwrap();
        let mut arrivals = FileArrivals::new();
        let w = watching(root);
        arrivals.poll(&w, T0, false);
        assert!(arrivals.ready("rt_1", T0).is_empty());
        fs::write(root.join("inbox/new.csv"), "new\n").unwrap();
        arrivals.poll(&w, T0 + 1_000, false);
        assert!(arrivals.ready("rt_1", T0 + 1_000).is_empty());
        // Still changing: the clock of settling starts again.
        fs::write(root.join("inbox/new.csv"), "new, longer\n").unwrap();
        arrivals.poll(&w, T0 + SETTLE_MS, false);
        assert!(arrivals.ready("rt_1", T0 + SETTLE_MS).is_empty());
        arrivals.poll(&w, T0 + SETTLE_MS * 2 + 1, false);
        assert_eq!(arrivals.ready("rt_1", T0 + SETTLE_MS * 2 + 1), vec!["new.csv"]);
    }

    #[test]
    fn skips_hidden_temporary_and_partly_downloaded_names_and_folders() {
        let p = project();
        let root = &p.0;
        let mut arrivals = FileArrivals::new();
        let w = watching(root);
        arrivals.poll(&w, T0, false);
        for name in [".DS_Store", "~$budget.xlsx", "a.tmp", "b.crdownload", "c.part"] {
            fs::write(root.join("inbox").join(name), "x").unwrap();
        }
        fs::create_dir(root.join("inbox/sub")).unwrap();
        arrivals.poll(&w, T0 + 1, false);
        arrivals.poll(&w, T0 + SETTLE_MS + 2, false);
        assert!(arrivals.ready("rt_1", T0 + SETTLE_MS + 2).is_empty());
    }

    #[test]
    fn runs_at_most_six_times_an_hour_and_files_waiting_past_that_wait_for_the_hour() {
        let p = project();
        let root = &p.0;
        let mut arrivals = FileArrivals::new();
        let w = watching(root);
        arrivals.poll(&w, T0, false);
        let mut now = T0;
        for index in 0..=MAX_FIRES_PER_HOUR {
            let path = root.join(format!("inbox/f{index}.txt"));
            fs::write(&path, index.to_string()).unwrap();
            set_mtime(&path, now);
            arrivals.poll(&w, now + 1, false);
            arrivals.poll(&w, now + SETTLE_MS + 2, false);
            now += SETTLE_MS + 3;
            let ready = arrivals.ready("rt_1", now);
            if index < MAX_FIRES_PER_HOUR {
                assert_eq!(ready, vec![format!("f{index}.txt")]);
                arrivals.fired("rt_1", &ready, now);
            } else {
                assert!(ready.is_empty());
            }
        }
        assert_eq!(arrivals.ready("rt_1", now + 3_600_000), vec![format!("f{MAX_FIRES_PER_HOUR}.txt")]);
    }

    #[test]
    fn is_a_folder_inside_the_project_by_name_never_outside_it() {
        let valid = |f: &str| crate::routines::schedule::valid(&serde_json::json!({"kind": "files", "folder": f}));
        assert!(valid("inbox"));
        assert!(valid("docs/incoming"));
        for bad in ["", "..", "../x", "a/../../x", "C:\\Windows", "/etc", "in:box", "."] {
            assert!(!valid(bad), "{bad}");
        }
        let p = project();
        let root = &p.0;
        assert_eq!(watched_path(root, "inbox"), Some(root.join("inbox")));
        assert_eq!(watched_path(root, "../other"), None);
        assert_eq!(watched_path(root, "."), None);
    }

    #[test]
    fn a_file_a_teammate_wrote_starts_nothing_held_while_a_run_is_live_and_let_go_when_its_turn_says_it_wrote_it() {
        let p = project();
        let root = &p.0;
        let mut arrivals = FileArrivals::new();
        let w = watching(root);
        arrivals.poll(&w, T0, false);
        fs::write(root.join("inbox/summary.md"), "Three invoices.\n").unwrap();
        arrivals.poll(&w, T0 + 1, true);
        arrivals.poll(&w, T0 + SETTLE_MS + 2, true);
        assert!(arrivals.ready("rt_1", T0 + SETTLE_MS + 2).is_empty());
        arrivals.wrote(root, &["inbox/summary.md".into(), "notes/elsewhere.md".into()]);
        arrivals.poll(&w, T0 + SETTLE_MS + RELEASE_MS + 10, false);
        assert!(arrivals.ready("rt_1", T0 + SETTLE_MS + RELEASE_MS + 10).is_empty());
    }

    #[test]
    fn a_persons_file_that_landed_while_it_worked_is_told_apart_and_still_starts_the_routine() {
        let p = project();
        let root = &p.0;
        let mut arrivals = FileArrivals::new();
        let w = watching(root);
        arrivals.poll(&w, T0, false);
        fs::write(root.join("inbox/summary.md"), "mine\n").unwrap();
        fs::write(root.join("inbox/invoice-1060.pdf"), "theirs\n").unwrap();
        arrivals.poll(&w, T0 + 1, true);
        arrivals.poll(&w, T0 + SETTLE_MS + 2, true);
        // The turn's record is written as it ends, so a moment of quiet is waited for before anything held counts.
        arrivals.poll(&w, T0 + SETTLE_MS + 3, false);
        assert!(arrivals.ready("rt_1", T0 + SETTLE_MS + 3).is_empty());
        arrivals.wrote(root, &["inbox/summary.md".into()]);
        arrivals.poll(&w, T0 + SETTLE_MS + 2 + RELEASE_MS, false);
        assert_eq!(arrivals.ready("rt_1", T0 + SETTLE_MS + 2 + RELEASE_MS), vec!["invoice-1060.pdf"]);
    }

    #[test]
    fn a_file_is_let_go_even_when_its_turn_ends_before_it_has_stopped_changing() {
        let p = project();
        let root = &p.0;
        let mut arrivals = FileArrivals::new();
        let w = watching(root);
        arrivals.poll(&w, T0, false);
        fs::write(root.join("inbox/report.csv"), "a,b\n").unwrap();
        arrivals.wrote(root, &["inbox/report.csv".into()]);
        arrivals.poll(&w, T0 + 1, false);
        arrivals.poll(&w, T0 + SETTLE_MS + 2, false);
        assert!(arrivals.ready("rt_1", T0 + SETTLE_MS + 2).is_empty());
    }

    #[test]
    fn step_one_is_told_the_files_by_name() {
        let ready = vec!["invoice-1043.pdf".to_owned(), "invoice-1051.pdf".to_owned()];
        assert_eq!(
            arrival_note("inbox", &ready),
            "2 new files just arrived: inbox/invoice-1043.pdf, inbox/invoice-1051.pdf. This run is for them."
        );
        assert_eq!(arrival_note("in\\box", &ready[..1]), "A new file just arrived: in/box/invoice-1043.pdf. This run is for it.");
    }
}
