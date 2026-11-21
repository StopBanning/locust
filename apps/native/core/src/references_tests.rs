//! Electron's `main/a-reference-stays-with-its-conversation.test.ts`: a reference is kept by its conversation (across a
//! restart and later turns, never another conversation's), refused when the window names a path, an unknown id, a
//! repeat or a sixth, and never widens the mode it runs in. The cancelled picker is the window's
//! (`window_drive_tests`); the GitHub copies are not ported (Rust adds folders on this computer only).
use super::*;
use std::sync::Mutex;

struct Dirs(std::path::PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Dirs, Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("locust-reference-test-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(dir.join("electron")).unwrap();
    let picked = dir.join("picked");
    std::fs::create_dir_all(&picked).unwrap();
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    let picked = picked.canonicalize().unwrap();
    (Dirs(dir), store, picked)
}

// case: survives a restart and later turns, but not another conversation of the same teammate
#[test]
fn survives_a_restart_and_later_turns_but_not_another_conversation() {
    let (dir, store, picked) = fixture();
    let item = add_folder(&store, &picked).unwrap();
    bind(&store, "mission_first", None, std::slice::from_ref(&item.id), Some("/actual-working-folder")).unwrap();
    // A restart: a new store over the same folders.
    let restarted = Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    assert_eq!(load(&restarted).unwrap()["folders"]["mission_first"], "/actual-working-folder");
    bind(&restarted, "mission_second", Some("mission_first"), std::slice::from_ref(&item.id), None).unwrap();
    assert_eq!(select(&restarted, Some("mission_second"), None).unwrap().iter().map(|r| r.path.clone()).collect::<Vec<_>>(), vec![picked.to_string_lossy().into_owned()]);
    bind(&restarted, "mission_third", Some("mission_second"), std::slice::from_ref(&item.id), None).unwrap();
    assert_eq!(select(&restarted, Some("mission_third"), None).unwrap(), vec![item]);
    assert_eq!(select(&restarted, Some("mission_other"), None).unwrap(), vec![]);
}

// case: removal affects the conversation, not the chosen folder or another conversation
#[test]
fn removal_affects_the_conversation_not_the_folder_or_another_conversation() {
    let (_dir, store, picked) = fixture();
    let item = add_folder(&store, &picked).unwrap();
    bind(&store, "mission_first", None, std::slice::from_ref(&item.id), None).unwrap();
    bind(&store, "mission_other", None, std::slice::from_ref(&item.id), None).unwrap();
    bind(&store, "mission_second", Some("mission_first"), std::slice::from_ref(&item.id), None).unwrap();
    set(&store, "mission_second", &[]).unwrap();
    assert_eq!(select(&store, Some("mission_first"), None).unwrap(), vec![]);
    assert_eq!(select(&store, Some("mission_other"), None).unwrap(), vec![item]);
    assert!(picked.is_dir());
}

// case: refuses paths or invented identities from the window, duplicate IDs, and a sixth reference
#[test]
fn refuses_paths_invented_identities_duplicates_and_a_sixth() {
    let (_dir, store, picked) = fixture();
    let items: Vec<Item> = (0..6).map(|_| add_folder(&store, &picked).unwrap()).collect();
    let path = picked.to_string_lossy().into_owned();
    assert!(bind(&store, "mission_first", None, &[path], None).unwrap_err().contains("using Add a reference"));
    assert!(select(&store, None, Some(&["__proto__".to_owned()])).unwrap_err().contains("using Add a reference"));
    let six: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
    assert!(bind(&store, "mission_first", None, &six, None).unwrap_err().contains("five"));
    assert!(bind(&store, "mission_first", None, &six[..5], None).is_ok(), "five is allowed");
    set(&store, "mission_first", &[]).unwrap();
    assert!(bind(&store, "mission_first", None, &[six[0].clone(), six[0].clone()], None).unwrap_err().contains("five"));
    assert_eq!(select(&store, Some("mission_first"), None).unwrap(), vec![]);
}

const AUTO_NOTE: &str = "This mode can change it. The brief asks for reading, not changing.";

// case: full-access modes keep the reference with a plain write warning
#[test]
fn full_access_modes_keep_the_reference_with_a_plain_write_warning() {
    for runtime in ["codex", "claude", "opencode", "copilot", "antigravity", "cursor", "muse"] {
        assert_eq!(access(runtime, "auto", "/reference", None, false), (true, AUTO_NOTE), "{runtime}");
    }
}

// case: Codex reads outside its workspace without granting add-dir write access
#[test]
fn codex_reads_outside_its_workspace_without_granting_write_access() {
    let (include, note) = access("codex", "accept-edits", "/reference", Some("/project"), false);
    assert!(include);
    assert!(note.contains("No extra writable folder"), "{note}");
}

// case: names references without changing an unextended runtime or inventing a write-access reason to omit them
#[test]
fn names_references_without_changing_an_unextended_runtime() {
    for runtime in ["cursor", "muse", "another-runtime"] {
        for mode in ["ask", "plan", "accept-edits", "approve-each"] {
            assert_eq!(
                access(runtime, mode, "/reference", Some("/project"), false),
                (true, "Named in the brief; this runtime's existing permissions still apply."),
                "{runtime} {mode}"
            );
        }
    }
}

// case: a restricted runtime omits a grant that would make the reference writable and says why
#[test]
fn a_restricted_runtime_omits_a_grant_that_would_make_it_writable() {
    for runtime in ["opencode", "copilot", "antigravity"] {
        assert!(access(runtime, "ask", "/reference", None, false).0, "{runtime}");
        let (include, note) = access(runtime, "accept-edits", "/reference", None, false);
        assert!(!include, "{runtime}");
        assert!(note.contains("also grant write access"), "{note}");
    }
}

// case: does not pretend a folder already writable in the workspace is read-only
#[test]
fn does_not_pretend_a_folder_already_writable_in_the_workspace_is_read_only() {
    assert!(access("opencode", "accept-edits", "C:\\project\\design", Some("C:/project"), false).1.contains("This mode can change it"));
    assert!(!access("opencode", "accept-edits", "/Project/design", Some("/project"), false).0);
    assert!(!access("opencode", "accept-edits", "/project-other", Some("/project"), false).0);
    assert!(!access("codex", "auto", "/reference", Some("/project"), true).0);
}

// case: Claude Edit and Approve each keep references with their scoped read-only promise, but Auto keeps its warning
#[test]
fn claude_edit_and_approve_each_keep_their_read_only_promise_but_auto_warns() {
    for mode in ["accept-edits", "approve-each"] {
        assert_eq!(access("claude", mode, "C:/reference", None, false), (true, "For reading, not changing."), "{mode}");
    }
    assert_eq!(access("claude", "accept-edits", "C:/project/reference", Some("C:/project"), false).1, "For reading, not changing.");
    assert!(access("claude", "auto", "C:/reference", None, false).1.contains("This mode can change it"));
}

// case: the every-turn brief names only attached folders and explicitly withdraws old references
#[test]
fn the_every_turn_brief_names_only_attached_folders_and_withdraws_old_ones() {
    assert!(section(&["/reference one", "/reference two"]).contains("for reading, not changing: \"/reference one\", \"/reference two\""));
    assert!(section(&[]).contains("earlier references are not attached now"));
}

// ---- a GitHub reference is a bounded, recoverable host copy ----------------------------------------------------------

type Calls = std::sync::Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// A fake download: records the call, then `make(program, args, calls so far)`.
fn copies(make: impl Fn(&str, &[String], usize) -> Result<(), String> + Send + Sync + 'static, now: std::sync::Arc<std::sync::atomic::AtomicU64>) -> (Copies, Calls) {
    let calls: Calls = Default::default();
    let seen = calls.clone();
    let run = move |program: &str, args: &[String], _cwd: &std::path::Path, _check: &dyn Fn() -> Result<(), String>| {
        let n = {
            let mut c = seen.lock().unwrap();
            c.push((program.to_owned(), args.to_vec()));
            c.len()
        };
        make(program, args, n)
    };
    let copies = Copies { run: Box::new(run), now: Box::new(move || now.load(std::sync::atomic::Ordering::SeqCst)), max_bytes: SIZE_CAP };
    (copies, calls)
}

fn clock(at: u64) -> std::sync::Arc<std::sync::atomic::AtomicU64> {
    std::sync::Arc::new(std::sync::atomic::AtomicU64::new(at))
}

fn write_copy(path: &str, file: &str, text: &str) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    std::fs::write(std::path::Path::new(path).join(file), text).map_err(|e| e.to_string())
}

fn entries(path: std::path::PathBuf) -> usize {
    std::fs::read_dir(path).map(|d| d.count()).unwrap_or(0)
}

// case: accepts only repository identities, not arbitrary git URLs, paths, options or shell fragments
#[test]
fn accepts_only_repository_identities() {
    assert_eq!(repository("owner/repo").as_deref(), Some("owner/repo"));
    assert_eq!(repository("https://github.com/owner/repo.git/").as_deref(), Some("owner/repo"));
    for value in ["../repo", "--upload-pack=evil/repo", "https://user:secret@github.com/owner/repo", "file:///repo", "https://evil.test/owner/repo", "owner/..", "owner/repo;evil", "owner/repo/tree/main"] {
        assert_eq!(repository(value), None, "{value}");
    }
}

// case: uses gh first, shallow-clones into its cache, refreshes after an hour, and keeps the old copy in trash
#[test]
fn uses_gh_first_shallow_clones_into_its_cache_refreshes_after_an_hour_and_keeps_the_old_copy_in_trash() {
    let (_dir, store, _) = fixture();
    let time = clock(1000);
    let (copies, calls) = copies(|_, args, n| write_copy(&args[3], "source.txt", &format!("copy {n}")), time.clone());
    let item = github(&store, &copies, "owner/private").unwrap();
    {
        let c = calls.lock().unwrap();
        assert_eq!(c[0].0, "gh");
        assert_eq!(c[0].1[..3], ["repo", "clone", "owner/private"]);
        assert!(c[0].1.contains(&"--depth".to_owned()));
    }
    let cache = store.root().join("reference-repositories").canonicalize().unwrap();
    assert!(std::path::Path::new(&item.path).starts_with(&cache), "{}", item.path);
    bind(&store, "mission_first", None, std::slice::from_ref(&item.id), None).unwrap();
    list(&store, &copies, Some("mission_first")).unwrap();
    assert_eq!(calls.lock().unwrap().len(), 1, "fresh: not downloaded again");
    time.fetch_add(REFRESH_MS + 1, std::sync::atomic::Ordering::SeqCst);
    hold("run_live_refresh", std::slice::from_ref(&item.id));
    list(&store, &copies, Some("mission_first")).unwrap();
    assert_eq!(calls.lock().unwrap().len(), 1, "a live run's copy is not swapped under it");
    release(&store, "run_live_refresh").unwrap();
    let refreshed = list(&store, &copies, Some("mission_first")).unwrap().remove(0);
    assert_ne!(refreshed.path, item.path);
    assert_eq!(std::fs::read_to_string(std::path::Path::new(&refreshed.path).join("source.txt")).unwrap(), "copy 2");
    assert_eq!(entries(store.root().join("reference-trash")), 1);
    // The new path is what is saved: a later turn reads it.
    assert_eq!(select(&store, Some("mission_first"), None).unwrap()[0].path, refreshed.path);
    // Refreshed now: the next hour reads this copy, not another download.
    list(&store, &copies, Some("mission_first")).unwrap();
    assert_eq!(calls.lock().unwrap().len(), 2, "a copy just downloaded is not downloaded again");
}

// case: falls back to an HTTPS public clone without shell interpretation
#[test]
fn falls_back_to_an_https_public_clone_without_shell_interpretation() {
    let (_dir, store, _) = fixture();
    let (copies, calls) = copies(
        |program, args, _| if program == "gh" { Err("not installed".into()) } else { write_copy(args.last().unwrap(), "README.md", "public") },
        clock(0),
    );
    github(&store, &copies, "owner/public").unwrap();
    let c = calls.lock().unwrap();
    assert_eq!(c.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["gh", "git"]);
    let git = &c[1].1;
    assert_eq!(git[git.len() - 3..git.len() - 1], ["--", "https://github.com/owner/public.git"]);
}

// case: refuses an oversized clone and recycles it, never the local folder
#[test]
fn refuses_an_oversized_clone_and_recycles_it_never_the_local_folder() {
    let (_dir, store, picked) = fixture();
    let (mut copies, _) = copies(|_, args, _| write_copy(&args[3], "large", "too large"), clock(0));
    copies.max_bytes = 5;
    assert!(github(&store, &copies, "owner/large").unwrap_err().contains("larger than 500 MB"));
    assert_eq!(entries(store.root().join("reference-repositories")), 0);
    assert_eq!(entries(store.root().join("reference-trash")), 1);
    assert!(picked.is_dir());
}

// case: only recycles a clone when its last conversation and live run release it
#[test]
fn only_recycles_a_clone_when_its_last_conversation_and_live_run_release_it() {
    let (_dir, store, _) = fixture();
    let (copies, _) = copies(|_, args, _| write_copy(&args[3], "README.md", "reference"), clock(0));
    let item = github(&store, &copies, "owner/shared").unwrap();
    bind(&store, "mission_first", None, std::slice::from_ref(&item.id), None).unwrap();
    bind(&store, "mission_other", None, std::slice::from_ref(&item.id), None).unwrap();
    set(&store, "mission_first", &[]).unwrap();
    assert!(std::path::Path::new(&item.path).is_dir());
    hold("run_live_shared", std::slice::from_ref(&item.id));
    set(&store, "mission_other", &[]).unwrap();
    assert!(std::path::Path::new(&item.path).is_dir(), "the live run still reads it");
    release(&store, "run_live_shared").unwrap();
    assert!(!std::path::Path::new(&item.path).exists());
    assert_eq!(entries(store.root().join("reference-trash")), 1);
}

// case: keeps an unsent reference after a failed start releases its lease, until the person removes it
#[test]
fn keeps_an_unsent_reference_after_a_failed_start_releases_its_lease() {
    let (_dir, store, _) = fixture();
    let (copies, _) = copies(|_, args, _| write_copy(&args[3], "README.md", "reference"), clock(0));
    let item = github(&store, &copies, "owner/draft").unwrap();
    drop(lease(&store, "run_failed_before_binding", std::slice::from_ref(&item.id)));
    assert_eq!(select(&store, None, Some(std::slice::from_ref(&item.id))).unwrap(), vec![item.clone()]);
    assert!(std::path::Path::new(&item.path).is_dir());
    discard(&store, &item.id).unwrap();
    assert!(!std::path::Path::new(&item.path).exists());
}

// case: does not follow a repository junction while counting bytes
#[test]
fn does_not_follow_a_repository_link_while_counting_bytes() {
    let (dir, _store, picked) = fixture();
    let clone = dir.0.join("clone");
    std::fs::create_dir_all(&clone).unwrap();
    std::fs::write(picked.join("large"), "large outside").unwrap();
    std::os::unix::fs::symlink(&picked, clone.join("link")).unwrap();
    assert_eq!(bytes(&clone, 1), Ok(0));
}

/// The real runner (no fake): a clone that grows past the cap is stopped, and the size check's words are the answer.
#[test]
fn the_real_runner_stops_a_clone_the_size_check_refuses() {
    let (dir, _store, _) = fixture();
    let started = std::time::Instant::now();
    let refused = run_clone("sh", &["-c".into(), "sleep 20".into()], &dir.0, &|| Err(TOO_LARGE.to_owned()));
    assert_eq!(refused, Err(TOO_LARGE.to_owned()));
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "stopped, not waited out: {:?}", started.elapsed());
    assert_eq!(run_clone("sh", &["-c".into(), "exit 0".into()], &dir.0, &|| Ok(())), Ok(()));
    assert_eq!(run_clone("sh", &["-c".into(), "exit 3".into()], &dir.0, &|| Ok(())), Err(DOWNLOAD_FAILED.to_owned()));
}

/// A gh clone stopped for its size is the answer: it is not tried again over HTTPS, and its partial copy is trashed.
#[test]
fn a_clone_stopped_for_its_size_is_not_tried_again() {
    let (_dir, store, _) = fixture();
    let calls: Calls = Default::default();
    let seen = calls.clone();
    let run = move |program: &str, args: &[String], _: &std::path::Path, check: &dyn Fn() -> Result<(), String>| {
        seen.lock().unwrap().push((program.to_owned(), args.to_vec()));
        write_copy(&args[3], "large", "too large")?;
        check()
    };
    let copies = Copies { run: Box::new(run), now: Box::new(|| 0), max_bytes: 5 };
    assert!(github(&store, &copies, "owner/large").unwrap_err().contains("larger than 500 MB"));
    assert_eq!(calls.lock().unwrap().iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["gh"]);
    assert_eq!(entries(store.root().join("reference-repositories")), 0);
}
