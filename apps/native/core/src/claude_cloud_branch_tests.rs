//! Cloud view 1, row 2: the cheap signal. A light look at a cloud session's own branch on the folder's remote, with
//! git played by a fake (`with_git`): `ls-remote` for its newest commit, and a fetch only when that changed. Never a
//! teleport, never a Claude run.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{BranchLook, ClaudeCloud, CloudPlatform, GitRan, Pushed};

const ID: &str = "cc_0123456789abcdef0123";
const OLD: &str = "1111111111111111111111111111111111111111";
const NEW: &str = "2222222222222222222222222222222222222222";

fn store_file() -> std::path::PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "locust-cloud-branch-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("claude-cloud.json")
}

/// A session whose branch is known, and the push the last look found (if any).
fn known(path: &std::path::Path, branch: Option<&str>, pushed: Option<&str>) {
    let mut one = serde_json::json!({
        "id": ID, "startedAt": "2026-11-26T09:00:00.000Z", "prompt": "Fix the cart total", "folder": "/work",
        "sessionId": "session_01G8RT9yXr7dbhVM7EUzpvEa", "lastReply": "Looking at cart.js.",
        "openedAt": "2026-11-26T10:00:00.000Z"
    });
    if let Some(branch) = branch {
        one["branch"] = branch.into();
    }
    if let Some(hash) = pushed {
        one["pushed"] = serde_json::json!({ "hash": hash, "subject": "Read the cart", "at": "2026-11-26T09:30:00+00:00" });
    }
    std::fs::write(path, serde_json::json!({ "sessions": [one] }).to_string()).unwrap();
}

fn ran(code: i32, stdout: &str) -> GitRan {
    GitRan { code, stdout: stdout.into(), stderr: String::new() }
}

/// git as the remote has it: `remote` is the branch's newest commit on origin (None: ls-remote fails).
fn fake_git<'a>(seen: &'a RefCell<Vec<String>>, remote: Option<&'a str>, fetch_ok: bool) -> impl Fn(&[&str], &str, Option<&str>) -> GitRan + 'a {
    move |args: &[&str], cwd: &str, _input: Option<&str>| {
        assert_eq!(cwd, "/work");
        seen.borrow_mut().push(args.join(" "));
        match args.first().copied() {
            Some("ls-remote") => match remote {
                Some(hash) => ran(0, &format!("{hash}\trefs/heads/claude/fix-cart-total\n")),
                None => ran(128, ""),
            },
            Some("fetch") => ran(if fetch_ok { 0 } else { 1 }, ""),
            Some("log") if args.last() == Some(&NEW) => ran(0, "2026-11-26T10:38:00+00:00\nSum prices in the cart\n"),
            _ => ran(1, ""),
        }
    }
}

fn cloud<'a>(path: &std::path::Path, git: impl Fn(&[&str], &str, Option<&str>) -> GitRan + 'a) -> ClaudeCloud<'a> {
    ClaudeCloud::new(path, CloudPlatform::Linux, || None).with_git(git).with_now(|| "2026-11-26T10:40:00.000Z".into())
}

#[test]
fn a_new_commit_on_the_sessions_branch_is_fetched_kept_and_new() {
    let path = store_file();
    known(&path, Some("claude/fix-cart-total"), Some(OLD));
    let seen = RefCell::new(Vec::new());
    let look = cloud(&path, fake_git(&seen, Some(NEW), true)).look_at_branch(ID);
    let BranchLook::New(session) = look else { panic!("{look:?}") };
    assert_eq!(
        session.pushed,
        Some(Pushed { hash: NEW.into(), subject: "Sum prices in the cart".into(), at: "2026-11-26T10:38:00+00:00".into() })
    );
    assert!(session.has_news(), "a push after the row was opened is news");
    assert_eq!(
        *seen.borrow(),
        [
            "ls-remote --quiet origin refs/heads/claude/fix-cart-total".to_string(),
            "fetch --quiet --no-tags --no-write-fetch-head origin refs/heads/claude/fix-cart-total".to_string(),
            format!("log -1 --format=%cI%n%s {NEW}"),
        ]
    );
    let kept = cloud(&path, |_: &[&str], _: &str, _: Option<&str>| ran(1, "")).all().remove(0);
    assert_eq!(kept, session, "kept in the record as it was returned");
    assert_eq!(kept.last_reply.as_deref(), Some("Looking at cart.js."), "the rest of the record stays");
}

#[test]
fn the_same_commit_as_last_time_fetches_nothing_and_changes_nothing() {
    let path = store_file();
    known(&path, Some("claude/fix-cart-total"), Some(NEW));
    let before = std::fs::read_to_string(&path).unwrap();
    let seen = RefCell::new(Vec::new());
    assert_eq!(cloud(&path, fake_git(&seen, Some(NEW), true)).look_at_branch(ID), BranchLook::Same);
    assert_eq!(*seen.borrow(), ["ls-remote --quiet origin refs/heads/claude/fix-cart-total"]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
}

#[test]
fn a_failed_look_keeps_the_last_known_push() {
    let path = store_file();
    known(&path, Some("claude/fix-cart-total"), Some(OLD));
    let before = std::fs::read_to_string(&path).unwrap();
    for (remote, fetch_ok) in [(None, true), (Some(NEW), false)] {
        let seen = RefCell::new(Vec::new());
        assert_eq!(cloud(&path, fake_git(&seen, remote, fetch_ok)).look_at_branch(ID), BranchLook::Failed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "{remote:?} {fetch_ok}");
    }
    let kept = cloud(&path, |_: &[&str], _: &str, _: Option<&str>| ran(1, "")).all().remove(0);
    assert_eq!(kept.pushed.map(|p| p.hash), Some(OLD.to_string()));
}

#[test]
fn no_branch_known_asks_git_nothing() {
    let path = store_file();
    known(&path, None, None);
    let seen = RefCell::new(Vec::new());
    assert_eq!(cloud(&path, fake_git(&seen, Some(NEW), true)).look_at_branch(ID), BranchLook::NoBranch);
    assert!(seen.borrow().is_empty());
}

#[test]
fn a_branch_that_is_not_plain_ref_words_is_never_passed_to_git() {
    let path = store_file();
    for bad in ["--upload-pack=touch /tmp/x", "claude/../main", "-x", "claude/fix\nmain"] {
        known(&path, Some(bad), None);
        let seen = RefCell::new(Vec::new());
        assert_eq!(cloud(&path, fake_git(&seen, Some(NEW), true)).look_at_branch(ID), BranchLook::NoBranch, "{bad}");
        assert!(seen.borrow().is_empty(), "{bad}");
    }
}
