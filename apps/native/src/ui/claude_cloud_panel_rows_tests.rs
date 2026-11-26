//! Claude's cloud session rows in the window (Cloud 3 row 2, `ClaudeCloudSessions` / `ClaudeCloudRow` in
//! CloudTasks.tsx), drawn through the whole window (`crate::view`) and driven by its poll (`crate::poll`): Show what it
//! did, Apply, Tell it more, Bring it home, Remove, See it on claude.ai, and a Watch ticked by the window's clock.
//! Claude Code never runs: its teleport is played by a fake terminal with real git, as core's read tests do, and its
//! follow-up receipt and window are recorded.

use super::*;
use locust_core::claude_cloud::{
    ClaudeLaunch, CloudPlatform, ExecOptions, ExecReply, PseudoTerminalResult, PseudoTerminalRun, WindowRun,
};
use rune::{Env, Runtime};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

const ID: &str = "cc_0123456789abcdef0123";
const SESSION: &str = "session_01G8RT9yXr7dbhVM7EUzpvEa";
const UNKNOWN: &str = "cc_fedcba9876543210fedc";
const TRANSCRIPT: &str = concat!(
    r#"{"type":"user","entrypoint":"cli","promptSource":"sdk","remoteSourced":true,"timestamp":"2026-10-02T20:00:01.000Z","message":{"role":"user","content":"Fix the cart total"}}"#,
    "\n",
    r#"{"type":"assistant","entrypoint":"cli","timestamp":"2026-10-02T20:00:09.000Z","message":{"role":"assistant","model":"claude-haiku-4-5-20251001","stop_reason":"end_turn","content":[{"type":"text","text":"Fixed: the total now sums prices."}]}}"#
);

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(cwd).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn slug(path: &Path) -> String {
    path.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// What Claude Code was asked to do, recorded.
#[derive(Default)]
struct Seen {
    teleports: AtomicUsize,
    sends: Mutex<Vec<Vec<String>>>,
    windows: Mutex<Vec<WindowRun>>,
}

struct World {
    root: PathBuf,
    seen: Arc<Seen>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl World {
    fn folder(&self) -> PathBuf {
        self.root.join("work")
    }
    fn stored(&self, s: &State) -> String {
        std::fs::read_to_string(locust_core::claude_cloud::store_path_of(s.data.as_ref().unwrap())).unwrap_or_default()
    }
}

/// Claude Code's teleport, as measured (core's claude_cloud_read_tests): the worktree on its own branch, the session's
/// change committed in it, the transcript written.
fn teleport(folder: PathBuf, home: PathBuf, seen: Arc<Seen>) -> impl Fn(&PseudoTerminalRun) -> PseudoTerminalResult {
    move |run: &PseudoTerminalRun| {
        seen.teleports.fetch_add(1, Ordering::SeqCst);
        let name = run.line.rsplit(' ').next().unwrap_or_default().trim_end_matches('"').to_owned();
        let place = folder.join(".claude").join("worktrees").join(&name);
        let place_s = place.to_string_lossy().into_owned();
        git(&folder, &["worktree", "add", "-q", "-b", &format!("worktree-{name}"), &place_s]);
        git(&folder, &["worktree", "lock", &place_s]);
        git(&place, &["checkout", "-q", "-B", "claude/fix-cart-total"]);
        std::fs::write(place.join("cart.js"), "export const total = (items) => items.reduce((sum, item) => sum + item.price, 0)\n").unwrap();
        git(&place, &["commit", "-q", "-am", "Sum prices"]);
        let dir = home.join("projects").join(slug(&place));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("26be863b-6ead-4c62-932c-07388873cd26.jsonl"), TRANSCRIPT).unwrap();
        PseudoTerminalResult::Ok { drawn: " \u{2714} Checking out branch\r\n\u{25cf} Session resumed\r\n\u{276f} ".into() }
    }
}

fn fake_claude(root: &Path, seen: &Arc<Seen>) -> MakeClaude {
    let (folder, home, seen) = (root.join("work"), root.join("claude-home"), seen.clone());
    Arc::new(move |store: &Store, reads| {
        let (sends, windows) = (seen.clone(), seen.clone());
        ClaudeCloud::new(locust_core::claude_cloud::store_path_of(store), CloudPlatform::Win32, || {
            Some(ClaudeLaunch {
                discovered_path: "C:\\npm\\claude.cmd".into(),
                executable_path: "C:\\npm\\claude.cmd".into(),
                prefix_args: Vec::new(),
                env: None,
            })
        })
        .with_reads(reads)
        .with_terminal(teleport(folder.clone(), home.clone(), seen.clone()))
        .with_claude_home(home.clone())
        .with_process_env(Vec::new)
        .with_git_changes(|_| Some(0))
        .with_exec(move |_file: &str, args: &[String], options: &ExecOptions| {
            let mut seen = args.to_vec();
            seen.extend(options.env.get("LOCUST_CLAUDE_CLOUD_TASK").cloned());
            sends.sends.lock().unwrap().push(seen);
            ExecReply { code: 0, output: "{\"ok\":true}\n".into(), stdout: Some("{\"ok\":true}\n".into()) }
        })
        .with_run_window(move |run: &WindowRun| {
            windows.windows.lock().unwrap().push(run.clone());
            Ok(())
        })
    })
}

/// A window whose teammate works in a git folder that already sent one known session and one Locust does not know the
/// id of; `make` None is this computer's own Claude Code.
fn window(fake: bool) -> (World, Runtime<State>) {
    let root = std::env::temp_dir().join(format!("locust-cloud3r-{}-{}", std::process::id(), TEMP_ID.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let root = root.canonicalize().unwrap();
    let work = root.join("work");
    git(&work, &["init", "-q", "-b", "main"]);
    git(&work, &["config", "user.email", "test@example.com"]);
    git(&work, &["config", "user.name", "Test"]);
    std::fs::write(work.join("cart.js"), "export const total = (items) => items.length\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "first"]);
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    let here = work.to_string_lossy();
    let sessions = serde_json::json!({ "sessions": [
        { "id": ID, "startedAt": chrono::Utc::now().to_rfc3339(), "prompt": "Fix the cart total", "folder": here,
          "sessionId": SESSION, "url": format!("https://claude.ai/code/{SESSION}"), "title": "Cart total" },
        { "id": UNKNOWN, "startedAt": chrono::Utc::now().to_rfc3339(), "prompt": "Tidy the readme", "folder": here },
    ] });
    std::fs::write(locust_core::claude_cloud::store_path_of(&store), sessions.to_string()).unwrap();
    let seen = Arc::new(Seen::default());
    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    s.cloud.make = fake.then(|| fake_claude(&root, &seen));
    let mate = Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Coder".into(),
        runtime: Some("claude".into()),
        model: Some("claude-sonnet-5-5".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(here.into_owned()),
    };
    crate::apply_team(&mut s, vec![mate]);
    let mut rt = Runtime::new(s, crate::view, (1280.0, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (World { root, seen }, rt)
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..20 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

/// The window's own poll, its clock moving `step` seconds a turn, until `done`.
fn pump_by(rt: &mut Runtime<State>, what: &str, step: f64, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut now = rt.state.cloud.now_ms as f64 / 1000.0;
    while !done(&rt.state) {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
        now += step;
        rt.update(move |s, tx| {
            crate::poll(s, now, tx);
        });
        rt.frame(0.05);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(rt);
}

fn pump(rt: &mut Runtime<State>, what: &str, done: impl FnMut(&State) -> bool) {
    pump_by(rt, what, 0.3, done);
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n")
}

fn row<'a>(s: &'a State, id: &str) -> Option<&'a super::Row> {
    s.cloud.rows.get(id)
}

/// The chat type menu's Cloud, as the person picks it, and the panel's sessions read.
fn open_cloud(rt: &mut Runtime<State>) {
    click(rt, "compare-mode-chip");
    click(rt, "compare-mode-cloud");
    pump(rt, "the sessions listed", |s| s.cloud.panel.claude.as_ref().is_some_and(|side| side.sessions.len() == 2));
}

fn key(what: &str, id: &str) -> String {
    format!("cloud-session-{what}-{id}")
}

#[test]
fn the_rows_say_where_each_session_is_and_offer_what_its_id_allows() {
    let (_w, mut rt) = window(true);
    open_cloud(&mut rt);
    let said = words(&rt);
    assert!(said.contains("Describe a task and send it. Claude does the work on Anthropic\u{2019}s machines"), "{said}");
    assert!(said.contains("Cart total") && said.contains("Fix the cart total"), "{said}");
    assert!(said.contains("In Claude\u{2019}s cloud since just now."), "{said}");
    assert!(said.contains("Sent to Claude\u{2019}s cloud just now. Follow it on claude.ai or in the Claude app."), "{said}");
    for what in ["check", "watch", "more", "web", "home", "forget"] {
        assert!(rt.find(&key(what, ID)).is_some(), "{what} on the known row");
    }
    for what in ["check", "watch", "more"] {
        assert!(rt.find(&key(what, UNKNOWN)).is_none(), "{what} on the row without an id");
    }
    assert!(said.contains("Show what it did") && !said.contains("Hide what it did"), "{said}");
    click(&mut rt, &key("web", ID));
    click(&mut rt, &key("web", UNKNOWN));
    assert_eq!(rt.state.opened_links, [format!("https://claude.ai/code/{SESSION}"), "https://claude.ai/code".to_string()]);
}

#[test]
fn show_what_it_did_reads_the_session_and_apply_brings_its_change_into_the_folder() {
    let (w, mut rt) = window(true);
    open_cloud(&mut rt);
    click(&mut rt, &key("check", ID));
    assert!(row(&rt.state, ID).is_some_and(|r| r.checking && r.expanded));
    pump(&mut rt, "the reading", |s| row(s, ID).is_some_and(|r| r.reading.is_some() && !r.checking));
    let said = words(&rt);
    assert!(said.contains("Updated 0 min ago"), "{said}");
    assert!(said.contains("What it said, as of just now:"), "{said}");
    assert!(said.contains("Fixed: the total now sums prices."), "{said}");
    assert!(said.contains("cart.js  +1 \u{2212}1"), "{said}");
    assert!(said.contains("Check again") && said.contains("Hide what it did"), "{said}");
    assert_eq!(w.seen.teleports.load(Ordering::SeqCst), 1);
    click(&mut rt, &key("apply", ID));
    pump(&mut rt, "the change applied", |s| row(s, ID).is_some_and(|r| r.applied));
    assert!(words(&rt).contains("Its change is in this folder, not committed. Look it over before you keep it."));
    assert!(words(&rt).contains("Applied"));
    let cart = std::fs::read_to_string(w.folder().join("cart.js")).unwrap();
    assert!(cart.contains("item.price"), "{cart}");
    click(&mut rt, &key("hide", ID));
    // Hidden, the reading goes; the closed row keeps only its glance, the reply's first line (Cloud view 1).
    assert!(rt.find(&key("read", ID)).is_none());
    assert!(!words(&rt).contains("cart.js  +1"));
    assert!(rt.find(&key("glance", ID)).is_some_and(|_| words(&rt).contains(": Fixed: the total now sums prices.")));
}

#[test]
fn tell_it_more_sends_a_follow_up_by_the_sessions_id_and_says_it_was_sent() {
    let (w, mut rt) = window(true);
    open_cloud(&mut rt);
    click(&mut rt, &key("more", ID));
    rt.paste("Also round to cents");
    rt.frame(0.016);
    click(&mut rt, &key("send", ID));
    pump(&mut rt, "the follow-up sent", |s| row(s, ID).is_some_and(|r| !r.sending && r.said.is_some()));
    assert!(words(&rt).contains("Sent. Check again in a moment to read its reply."), "{}", words(&rt));
    assert!(row(&rt.state, ID).unwrap().draft.text.is_empty());
    let sends = w.seen.sends.lock().unwrap().clone();
    assert_eq!(sends.len(), 1);
    // Windows' command line names the session; the words go only in LOCUST_CLAUDE_CLOUD_TASK (recorded last).
    assert!(sends[0][0].contains(&format!("--cloud {SESSION}")), "{sends:?}");
    assert_eq!(sends[0].last().map(String::as_str), Some("Also round to cents"), "{sends:?}");
    // Enter in the field sends too (the form's submit).
    click(&mut rt, &key("more", ID));
    rt.paste("And the tax");
    rt.frame(0.016);
    rt.key(rune::Key::Enter.into());
    rt.frame(0.016);
    pump(&mut rt, "the second follow-up sent", |_| w.seen.sends.lock().unwrap().len() == 2);
    assert_eq!(w.seen.sends.lock().unwrap()[1].last().map(String::as_str), Some("And the tax"));
}

#[test]
fn bring_it_home_opens_claude_code_here_and_remove_forgets_the_session() {
    let (w, mut rt) = window(true);
    open_cloud(&mut rt);
    click(&mut rt, &key("home", ID));
    pump(&mut rt, "Claude Code opened", |s| s.cloud.note.is_some());
    assert!(words(&rt).contains("Claude Code opened in this folder and is bringing this session\u{2019}s work into it."));
    let windows = w.seen.windows.lock().unwrap().clone();
    assert_eq!(windows.len(), 1);
    assert!(windows[0].args.iter().any(|a| a.contains("--teleport")), "{:?}", windows[0]);
    click(&mut rt, &key("forget", UNKNOWN));
    assert!(rt.find(&key("home", UNKNOWN)).is_none());
    assert!(!words(&rt).contains("Tidy the readme"));
    pump(&mut rt, "the session forgotten in the store", |s| !w.stored(s).contains(UNKNOWN));
    assert!(w.stored(&rt.state).contains(ID));
}

#[test]
fn a_watch_reads_on_the_windows_clock_and_stops_after_two_reads_with_nothing_new() {
    let (w, mut rt) = window(true);
    open_cloud(&mut rt);
    click(&mut rt, &key("watch", ID));
    assert!(words(&rt).contains("Stop watching"));
    // No reading on the row: the first read is at once, the next ones 90 s apart, on the window's own clock.
    pump_by(&mut rt, "the watch stopped", 15.0, |s| row(s, ID).is_some_and(|r| r.watch.is_none() && r.said.is_some()));
    assert_eq!(w.seen.teleports.load(Ordering::SeqCst), 3);
    let said = words(&rt);
    assert!(said.contains("Watch stopped after two reads with nothing new."), "{said}");
    assert!(said.contains("Fixed: the total now sums prices."), "{said}");
    assert!(!said.contains("Stop watching"), "{said}");
}

#[test]
fn closing_the_panel_ends_its_rows_and_their_watches() {
    let (w, mut rt) = window(true);
    open_cloud(&mut rt);
    click(&mut rt, &key("watch", ID));
    pump(&mut rt, "the first read", |s| row(s, ID).is_some_and(|r| r.reading.is_some()));
    click(&mut rt, "cloud-tasks-close");
    pump_by(&mut rt, "the rows gone", 100.0, |s| s.cloud.rows.is_empty());
    // Ten more turns of the window's clock, 100 s each: a watch still running would have read again.
    let mut turns = 0;
    pump_by(&mut rt, "ten turns", 100.0, |_| {
        turns += 1;
        turns > 10
    });
    assert_eq!(w.seen.teleports.load(Ordering::SeqCst), 1);
}

#[cfg(unix)]
#[test]
fn a_follow_up_on_this_computer_runs_claude_code_for_its_receipt() {
    use std::os::unix::fs::PermissionsExt;
    let (w, mut rt) = window(false);
    let bin = w.root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let claude = bin.join("claude");
    std::fs::write(&claude, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$(dirname \"$0\")/args.txt\"\necho '{\"ok\":true}'\n").unwrap();
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    // nextest runs each test in a process of its own; nothing else here reads PATH meanwhile.
    unsafe { std::env::set_var("PATH", path) };
    open_cloud(&mut rt);
    click(&mut rt, &key("more", ID));
    rt.paste("Also round to cents");
    rt.frame(0.016);
    click(&mut rt, &key("send", ID));
    pump(&mut rt, "the follow-up sent", |s| row(s, ID).is_some_and(|r| !r.sending && r.said.is_some()));
    assert_eq!(row(&rt.state, ID).unwrap().said.as_deref(), Some("Sent. Check again in a moment to read its reply."));
    let args = std::fs::read_to_string(bin.join("args.txt")).unwrap();
    assert_eq!(args, format!("-p\n--cloud\n{SESSION}\n--output-format\njson\nAlso round to cents\n"));
}

#[test]
fn a_watch_on_a_row_already_read_waits_ninety_seconds_for_its_first_read() {
    let (w, mut rt) = window(true);
    open_cloud(&mut rt);
    click(&mut rt, &key("check", ID));
    pump(&mut rt, "the reading", |s| row(s, ID).is_some_and(|r| r.reading.is_some() && !r.checking));
    click(&mut rt, &key("watch", ID));
    let started = rt.state.cloud.now_ms;
    let mut turns = 0;
    pump_by(&mut rt, "75 s of the window's clock", 15.0, |_| {
        turns += 1;
        turns > 5
    });
    assert_eq!(w.seen.teleports.load(Ordering::SeqCst), 1, "read again before 90 s");
    pump_by(&mut rt, "the watch's first read", 15.0, |_| w.seen.teleports.load(Ordering::SeqCst) == 2);
    assert!(rt.state.cloud.now_ms - started >= 90_000);
}

/// Cloud view 1: the known session's record as a read left it: a reply kept, learned at `news_at`, opened at `opened`.
fn kept_reading(w: &World, s: &State, news_at: &str, opened: Option<&str>) {
    let path = locust_core::claude_cloud::store_path_of(s.data.as_ref().unwrap());
    let mut saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let known = &mut saved["sessions"][0];
    known["lastReply"] = "Looking at cart.js now.".into();
    known["lastReplyAt"] = "2026-11-26T10:42:00.000Z".into();
    known["newsAt"] = news_at.into();
    if let Some(opened) = opened {
        known["openedAt"] = opened.into();
    }
    std::fs::write(&path, saved.to_string()).unwrap();
    assert!(w.stored(s).contains("lastReply"));
}

#[test]
fn a_glance_is_the_newest_replys_first_line_and_its_time() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-26T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let at = chrono::DateTime::parse_from_rfc3339("2026-11-26T10:42:00Z").unwrap().with_timezone(&chrono::Utc);
    let mut one = ClaudeCloudSession::default();
    assert_eq!(super::glance(&one, now), None, "nothing read yet: nothing said");
    one.last_reply = Some("Fixed: the total now sums prices.".into());
    assert_eq!(super::glance(&one, now).as_deref(), Some("Fixed: the total now sums prices."), "no time: the line alone");
    one.last_reply_at = Some("2026-11-26T10:42:00.000Z".into());
    let clock = locust_core::usage_words::clock_time(at);
    let local = |t: chrono::DateTime<chrono::Utc>| t.with_timezone(&chrono::Local).date_naive();
    let expected = if local(at) == local(now) {
        format!("As of {clock}: Fixed: the total now sums prices.")
    } else {
        format!("As of {} {clock}: Fixed: the total now sums prices.", at.with_timezone(&chrono::Local).format("%a"))
    };
    assert_eq!(super::glance(&one, now), Some(expected));
    let days_later = now + chrono::Duration::days(3);
    let later = super::glance(&one, days_later).unwrap();
    assert!(later.starts_with(&format!("As of {} {clock}: ", at.with_timezone(&chrono::Local).format("%a"))), "{later}");
}

#[test]
fn a_closed_row_says_the_last_reply_and_new_until_the_person_opens_it() {
    let (w, mut rt) = window(true);
    kept_reading(&w, &rt.state, "2026-11-26T10:45:00.000Z", Some("2026-11-26T10:00:00.000Z"));
    open_cloud(&mut rt);
    let said = words(&rt);
    assert!(said.contains(": Looking at cart.js now.") && said.contains("As of "), "{said}");
    assert!(rt.find(&key("glance", ID)).is_some());
    assert!(rt.find(&key("new", ID)).is_some(), "learned after it was last opened: new");
    assert!(rt.find(&key("glance", UNKNOWN)).is_none() && rt.find(&key("new", UNKNOWN)).is_none(), "nothing read: nothing said");
    assert_eq!(w.seen.teleports.load(Ordering::SeqCst), 0, "the glance is what Locust holds: no read");
    // Opening the row: no longer new, here and in the record.
    click(&mut rt, &key("check", ID));
    assert!(rt.find(&key("new", ID)).is_none());
    assert!(rt.find(&key("glance", ID)).is_none(), "open, the reading says it all");
    // Marked opened by the press itself, before any read lands (the window's poll has not run since).
    let kept = |s: &State| s.cloud.panel.claude.as_ref().unwrap().sessions.iter().find(|one| one.id == ID).unwrap().clone();
    assert!(kept(&rt.state).opened_at.as_deref().is_some_and(|at| at > "2026-11-26T10:00:00.000Z"), "{:?}", kept(&rt.state));
    assert!(!kept(&rt.state).has_news());
    pump(&mut rt, "the reading", |s| row(s, ID).is_some_and(|r| r.reading.is_some() && !r.checking));
    click(&mut rt, &key("hide", ID));
    // Closed again: the newest reply the read brought, and nothing new, since it came in while the row was open.
    let said = words(&rt);
    assert!(said.contains(": Fixed: the total now sums prices."), "{said}");
    assert!(!said.contains("Looking at cart.js now."), "{said}");
    assert!(rt.find(&key("new", ID)).is_none(), "{said}");
    pump(&mut rt, "the record kept", |s| {
        let saved = w.stored(s);
        saved.contains("\"lastReply\": \"Fixed: the total now sums prices.\"") && saved.contains("openedAt")
            && !saved.contains("2026-11-26T10:00:00.000Z")
    });
    let kept = ClaudeCloud::new(locust_core::claude_cloud::store_path_of(rt.state.data.as_ref().unwrap()), CloudPlatform::Linux, || None).all();
    let one = kept.iter().find(|one| one.id == ID).unwrap();
    assert!(!one.has_news(), "{one:?}");
    assert_eq!(one.last_reply_at.as_deref(), Some("2026-10-02T20:00:09.000Z"));
}

#[test]
fn a_reply_learned_before_the_row_was_last_opened_is_not_new() {
    let (w, mut rt) = window(true);
    kept_reading(&w, &rt.state, "2026-11-26T10:45:00.000Z", Some("2026-11-26T11:00:00.000Z"));
    open_cloud(&mut rt);
    assert!(rt.find(&key("glance", ID)).is_some());
    assert!(rt.find(&key("new", ID)).is_none(), "{}", words(&rt));
}

/// Cloud view 1, row 2: git as Claude's cloud left it. A bare repository is the folder's origin (on disk: nothing
/// reaches the network), and another clone pushes the session's branch to it, so its commits are not in the folder
/// until fetched.
struct Remote {
    bare: PathBuf,
    pusher: PathBuf,
}

impl Remote {
    fn new(w: &World) -> Self {
        let bare = w.root.join("origin.git");
        let pusher = w.root.join("pusher");
        git(&w.root, &["init", "-q", "--bare", "-b", "main", &bare.to_string_lossy()]);
        git(&w.folder(), &["remote", "add", "origin", &bare.to_string_lossy()]);
        git(&w.folder(), &["push", "-q", "origin", "main"]);
        git(&w.root, &["clone", "-q", &bare.to_string_lossy(), &pusher.to_string_lossy()]);
        git(&pusher, &["config", "user.email", "cloud@example.com"]);
        git(&pusher, &["config", "user.name", "Cloud"]);
        git(&pusher, &["checkout", "-q", "-b", "claude/fix-cart-total"]);
        let remote = Self { bare, pusher };
        remote.push("Sum prices in the cart");
        remote
    }
    fn push(&self, subject: &str) {
        std::fs::write(self.pusher.join("cart.js"), format!("// {subject}\n")).unwrap();
        git(&self.pusher, &["commit", "-q", "-am", subject]);
        git(&self.pusher, &["push", "-q", "origin", "claude/fix-cart-total"]);
    }
}

/// What git was asked, and a gate that holds every `ls-remote` until it opens.
#[derive(Default)]
struct GitSeen {
    ls_remote: AtomicUsize,
    fetch: AtomicUsize,
    held: std::sync::atomic::AtomicBool,
}

/// The window's `ClaudeCloud` with real git, counted.
fn counting_claude(seen: &Arc<GitSeen>) -> MakeClaude {
    let seen = seen.clone();
    Arc::new(move |store: &Store, reads| {
        let seen = seen.clone();
        ClaudeCloud::new(locust_core::claude_cloud::store_path_of(store), CloudPlatform::Linux, || None)
            .with_reads(reads)
            .with_git(move |args: &[&str], cwd: &str, _input: Option<&str>| {
                match args.first().copied() {
                    Some("ls-remote") => {
                        seen.ls_remote.fetch_add(1, Ordering::SeqCst);
                        while seen.held.load(Ordering::SeqCst) {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                    }
                    Some("fetch") => {
                        seen.fetch.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => {}
                }
                let out = Command::new("git").args(args).current_dir(cwd).output().expect("git");
                locust_core::claude_cloud::GitRan {
                    code: out.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                }
            })
    })
}

/// The known session's branch is known (a read found it), and the row was opened long ago.
fn branch_known(s: &State) {
    let path = locust_core::claude_cloud::store_path_of(s.data.as_ref().unwrap());
    let mut saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    saved["sessions"][0]["branch"] = "claude/fix-cart-total".into();
    saved["sessions"][0]["openedAt"] = "2026-01-01T00:00:00.000Z".into();
    std::fs::write(&path, saved.to_string()).unwrap();
}

fn pushed_said(rt: &Runtime<State>) -> Option<String> {
    rt.find(&key("pushed", ID))?;
    words(rt).lines().find(|line| line.starts_with("Pushed ")).map(str::to_owned)
}

#[test]
fn a_pushed_line_says_how_long_ago_and_the_subject() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-26T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let mut one = ClaudeCloudSession::default();
    assert_eq!(super::pushed_line(&one, now), None, "no branch looked at: nothing said");
    let at = |at: &str| locust_core::claude_cloud::Pushed { hash: "1".repeat(40), subject: "Sum prices".into(), at: at.into() };
    one.pushed = Some(at("2026-11-26T11:56:00+00:00"));
    assert_eq!(super::pushed_line(&one, now).as_deref(), Some("Pushed 4 min ago: Sum prices"));
    one.pushed = Some(at("2026-11-26T11:59:40Z"));
    assert_eq!(super::pushed_line(&one, now).as_deref(), Some("Pushed just now: Sum prices"));
    one.pushed = Some(at("2026-11-26T09:00:00Z"));
    assert_eq!(super::pushed_line(&one, now).as_deref(), Some("Pushed 3 hours ago: Sum prices"));
}

#[test]
fn an_open_panel_looks_at_the_sessions_branch_every_three_minutes_and_shows_its_newest_push() {
    let (w, mut rt) = window(true);
    let remote = Remote::new(&w);
    branch_known(&rt.state);
    let seen = Arc::new(GitSeen::default());
    rt.state.cloud.make = Some(counting_claude(&seen));
    open_cloud(&mut rt);
    pump(&mut rt, "the first look", |s| s.cloud.looking.is_empty() && pushed_said_in(s).is_some());
    assert_eq!(pushed_said(&rt).as_deref(), Some("Pushed just now: Sum prices in the cart"), "{}", words(&rt));
    assert!(rt.find(&key("new", ID)).is_some(), "a push since the row was opened is new");
    assert_eq!((seen.ls_remote.load(Ordering::SeqCst), seen.fetch.load(Ordering::SeqCst)), (1, 1));
    assert_eq!(w.seen.teleports.load(Ordering::SeqCst), 0, "never a teleport");
    // Nothing more for three minutes; then a look that finds the same commit and fetches nothing.
    let started = rt.state.cloud.now_ms;
    pump_by(&mut rt, "170 s", 10.0, |s| s.cloud.now_ms - started >= 170_000);
    assert_eq!(seen.ls_remote.load(Ordering::SeqCst), 1);
    pump_by(&mut rt, "the second look", 10.0, |_| seen.ls_remote.load(Ordering::SeqCst) == 2);
    pump(&mut rt, "the second look ended", |s| s.cloud.looking.is_empty());
    assert_eq!(seen.fetch.load(Ordering::SeqCst), 1, "same hash: no fetch");
    // A new push is fetched and said.
    remote.push("Round to cents");
    pump_by(&mut rt, "the new push", 10.0, |s| pushed_said_in(s).is_some_and(|line| line.ends_with("Round to cents")));
    assert_eq!(seen.fetch.load(Ordering::SeqCst), 2);
    assert!(words(&rt).contains("Pushed just now: Round to cents"), "{}", words(&rt));
    assert!(w.stored(&rt.state).contains("Round to cents"), "kept in the record");
    // The remote gone: the look fails, the last line stays and nothing alarming is said.
    std::fs::rename(&remote.bare, w.root.join("moved.git")).unwrap();
    let before = words(&rt);
    pump_by(&mut rt, "a failed look", 10.0, |_| seen.ls_remote.load(Ordering::SeqCst) == 4);
    pump(&mut rt, "the failed look ended", |s| s.cloud.looking.is_empty());
    assert!(words(&rt).contains("Pushed just now: Round to cents"));
    assert_eq!(words(&rt), before);
    assert_eq!(w.seen.teleports.load(Ordering::SeqCst), 0);
}

fn pushed_said_in(s: &State) -> Option<String> {
    let side = s.cloud.panel.claude.as_ref()?;
    super::pushed_line(side.sessions.iter().find(|one| one.id == ID)?, chrono::Utc::now())
}

#[test]
fn the_window_gaining_focus_looks_once_and_never_two_looks_of_one_folder_at_once() {
    let (w, mut rt) = window(true);
    let _remote = Remote::new(&w);
    branch_known(&rt.state);
    let seen = Arc::new(GitSeen::default());
    seen.held.store(true, Ordering::SeqCst);
    rt.state.cloud.make = Some(counting_claude(&seen));
    open_cloud(&mut rt);
    pump(&mut rt, "the first look under way", |_| seen.ls_remote.load(Ordering::SeqCst) == 1);
    // Focus lost and gained, and three more minutes, while that look is held: no second look of the folder.
    let focus = |rt: &mut Runtime<State>, on: bool| {
        rt.update(move |s, _| s.window_focused = on);
        let mut turns = 0;
        pump(rt, "a turn", |_| {
            turns += 1;
            turns > 3
        });
    };
    focus(&mut rt, false);
    focus(&mut rt, true);
    let started = rt.state.cloud.now_ms;
    pump_by(&mut rt, "three minutes", 20.0, |s| s.cloud.now_ms - started >= 200_000);
    assert_eq!(seen.ls_remote.load(Ordering::SeqCst), 1);
    seen.held.store(false, Ordering::SeqCst);
    pump(&mut rt, "the look ended", |s| s.cloud.looking.is_empty());
    // The panel closed: no looks on the clock, one when the window gains focus.
    click(&mut rt, "cloud-tasks-close");
    let after = seen.ls_remote.load(Ordering::SeqCst);
    let started = rt.state.cloud.now_ms;
    pump_by(&mut rt, "ten minutes closed", 60.0, |s| s.cloud.now_ms - started >= 600_000);
    assert_eq!(seen.ls_remote.load(Ordering::SeqCst), after, "closed: nothing on the clock");
    focus(&mut rt, false);
    focus(&mut rt, true);
    pump(&mut rt, "the focus look", |s| seen.ls_remote.load(Ordering::SeqCst) == after + 1 && s.cloud.looking.is_empty());
    focus(&mut rt, true);
    assert_eq!(seen.ls_remote.load(Ordering::SeqCst), after + 1, "once per focus");
}
