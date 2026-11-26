//! a-cloud-session-is-read-in-its-own-worktree.test.ts: `claude --teleport <id> --worktree <name>` played by a
//! terminal that does what Claude Code did (makes and locks the worktree on its branch, commits the session's change,
//! writes the transcript). git is real; Claude Code never runs.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    ClaudeCloud, ClaudeCloudReading, ClaudeLaunch, CloudAction, CloudExchange, CloudPlatform,
    CloudSendResult, GitRan, PseudoTerminalResult, PseudoTerminalRun, ReadingProblem,
    WindowsCommandOptions, read_teleport, reading_worktree_name, windows_command_line,
};

struct Temp(PathBuf);

impl Temp {
    /// The real spelling: Claude Code slugs the folder's real path, so /var must be /private/var here.
    fn new(label: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("locust-cloud-read-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp");
        Self(path.canonicalize().expect("real path"))
    }
    fn text(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(cwd).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn slug(path: &Path) -> String {
    path.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

const ID: &str = "cc_0123456789abcdef0123";
const SESSION: &str = "session_01G8RT9yXr7dbhVM7EUzpvEa";

/// A repository with one commit, and a store that already knows a session sent from it.
struct World {
    repo: Temp,
    home: Temp,
    store_dir: Temp,
    name: String,
}

impl World {
    fn new() -> Self {
        let repo = Temp::new("repo");
        git(&repo.0, &["init", "-q", "-b", "main"]);
        git(&repo.0, &["config", "user.email", "test@example.com"]);
        git(&repo.0, &["config", "user.name", "Test"]);
        git(&repo.0, &["config", "core.autocrlf", "false"]);
        std::fs::write(repo.0.join("cart.js"), "export const total = (items) => items.length\n").unwrap();
        git(&repo.0, &["add", "."]);
        git(&repo.0, &["commit", "-q", "-m", "first"]);
        let world = Self { repo, home: Temp::new("home"), store_dir: Temp::new("store"), name: reading_worktree_name(ID) };
        world.know(&world.repo.text());
        world
    }
    fn store(&self) -> PathBuf {
        self.store_dir.0.join("claude-cloud.json")
    }
    fn know(&self, folder: &str) {
        let sessions = serde_json::json!({ "sessions": [{ "id": ID, "startedAt": "2026-10-02T20:00:00.000Z",
            "prompt": "Fix the cart total", "folder": folder, "sessionId": SESSION,
            "url": format!("https://claude.ai/code/{SESSION}") }] });
        std::fs::write(self.store(), sessions.to_string()).unwrap();
    }
    fn worktree(&self) -> PathBuf {
        self.repo.0.join(".claude").join("worktrees").join(&self.name)
    }
}

const TRANSCRIPT: &str = concat!(
    r#"{"type":"permission-mode","permissionMode":"default","sessionId":"local"}"#, "\n",
    r#"{"type":"user","entrypoint":"cli","promptSource":"sdk","remoteSourced":true,"timestamp":"2026-10-02T20:00:01.000Z","message":{"role":"user","content":"Fix the cart total"}}"#, "\n",
    r#"{"type":"assistant","entrypoint":"cli","timestamp":"2026-10-02T20:00:09.000Z","message":{"role":"assistant","model":"claude-haiku-4-5-20251001","stop_reason":"end_turn","content":[{"type":"thinking","thinking":"never shown"},{"type":"text","text":"Fixed: **total** now sums prices."}]}}"#, "\n",
    r#"{"type":"user","entrypoint":"cli","isMeta":true,"timestamp":"2026-10-02T20:01:00.000Z","message":{"role":"user","content":"This session is being continued from another machine."}}"#
);

const RESUMED: &str = " \u{2714} Validating session\r\n \u{2714} Fetching session logs\r\n \u{2714} Checking out branch\r\n\u{25cf} Session resumed\r\n\u{276f} ";

struct Teleport {
    drawn: Option<&'static str>,
    change: bool,
    transcript: bool,
    worktree: bool,
}

const TELEPORT: Teleport = Teleport { drawn: None, change: true, transcript: true, worktree: true };

/// What Claude Code's teleport did, measured 10/02, played with real git.
fn teleporting<'a>(w: &'a World, runs: &'a RefCell<Vec<PseudoTerminalRun>>, options: Teleport) -> impl Fn(&PseudoTerminalRun) -> PseudoTerminalResult + 'a {
    move |run: &PseudoTerminalRun| {
        runs.borrow_mut().push(run.clone());
        let drawn = options.drawn.unwrap_or(RESUMED).to_string();
        if !options.worktree {
            return PseudoTerminalResult::Ok { drawn };
        }
        let place = w.worktree();
        let place_s = place.to_string_lossy().into_owned();
        git(&w.repo.0, &["worktree", "add", "-q", "-b", &format!("worktree-{}", w.name), &place_s]);
        git(&w.repo.0, &["worktree", "lock", &place_s]);
        if options.change {
            git(&place, &["checkout", "-q", "-B", "claude/fix-cart-total"]);
            std::fs::write(place.join("cart.js"), "export const total = (items) => items.reduce((sum, item) => sum + item.price, 0)\n").unwrap();
            git(&place, &["commit", "-q", "-am", "Sum prices"]);
        }
        if options.transcript {
            let dir = w.home.0.join("projects").join(slug(&place));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("26be863b-6ead-4c62-932c-07388873cd26.jsonl"), TRANSCRIPT).unwrap();
        }
        PseudoTerminalResult::Ok { drawn }
    }
}

fn claude() -> Option<ClaudeLaunch> {
    Some(ClaudeLaunch {
        discovered_path: "C:\\npm\\claude.cmd".into(),
        executable_path: "C:\\npm\\claude.cmd".into(),
        prefix_args: Vec::new(),
        env: None,
    })
}

fn cloud<'a>(w: &'a World, terminal: impl Fn(&PseudoTerminalRun) -> PseudoTerminalResult + 'a) -> ClaudeCloud<'a> {
    let mut cloud = ClaudeCloud::new(w.store(), CloudPlatform::Win32, claude)
        .with_terminal(terminal)
        .with_claude_home(&w.home.0)
        .with_process_env(Vec::new);
    // Ten looks for a transcript that never comes, without waiting between them.
    cloud.transcript_pause = std::time::Duration::ZERO;
    cloud
}

fn read_ok(reading: ClaudeCloudReading) -> (Vec<CloudExchange>, Option<String>, Option<String>) {
    match reading {
        ClaudeCloudReading::Ok { exchanges, diff, note, .. } => (exchanges, diff, note),
        ClaudeCloudReading::Err { message } => panic!("not read: {message}"),
    }
}

fn read_err(reading: ClaudeCloudReading) -> String {
    match reading {
        ClaudeCloudReading::Err { message } => message,
        ClaudeCloudReading::Ok { .. } => panic!("read, though it should not be"),
    }
}

#[test]
// case: teleports into a worktree of its own, out of sight, and stops once Claude Code is up
fn teleports_into_a_worktree_of_its_own_out_of_sight_and_stops_once_claude_code_is_up() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    cloud(&w, teleporting(&w, &runs, TELEPORT)).check(ID);
    let runs = runs.borrow();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].line, format!("cmd.exe /d /c \"\"C:\\npm\\claude.cmd\" --teleport {SESSION} --worktree {}\"", w.name));
    assert_eq!(runs[0].cwd, w.repo.text());
    // Done when the transcript is written -- not at "Session resumed", which comes first (measured).
    let written = w.home.0.join("projects").join(slug(&w.worktree()));
    assert_eq!(runs[0].stop_when_written.as_deref(), Some(written.to_string_lossy().as_ref()));
    assert!(!regex::Regex::new("resumed|shortcuts").unwrap().is_match(&runs[0].stop_when));
}

#[test]
// case: reads the conversation from the transcript Claude Code saved, replies as words, never thinking
fn reads_the_conversation_from_the_transcript_claude_code_saved_replies_as_words_never_thinking() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let reading = cloud(&w, teleporting(&w, &runs, TELEPORT)).check(ID);
    let shown = format!("{reading:?}");
    let (exchanges, _, _) = read_ok(reading);
    assert_eq!(
        exchanges,
        vec![CloudExchange {
            prompt: "Fix the cart total".into(),
            answer: Some("Fixed: **total** now sums prices.".into()),
            at: "2026-10-02T20:00:09.000Z".into(),
            model: Some("claude-haiku-4-5-20251001".into()),
        }]
    );
    assert!(!shown.contains("never shown"));
    assert!(!shown.contains("another machine"));
}

#[test]
// case: shows the session branch’s change against the folder, and leaves the folder alone until Apply
fn shows_the_session_branchs_change_against_the_folder_and_leaves_the_folder_alone_until_apply() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let (_, diff, _) = read_ok(cloud(&w, teleporting(&w, &runs, TELEPORT)).check(ID));
    let diff = diff.expect("a change");
    assert!(diff.contains("diff --git a/cart.js b/cart.js"));
    assert!(diff.contains("+export const total = (items) => items.reduce("));
    assert_eq!(std::fs::read_to_string(w.repo.0.join("cart.js")).unwrap(), "export const total = (items) => items.length\n");
    assert_eq!(git(&w.repo.0, &["status", "--porcelain", "--untracked-files=no"]), "");
}

#[test]
// case: Apply brings the change in uncommitted and removes the worktree and its branch
fn apply_brings_the_change_in_uncommitted_and_removes_the_worktree_and_its_branch() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let cloud = cloud(&w, teleporting(&w, &runs, TELEPORT));
    read_ok(cloud.check(ID));
    assert_eq!(cloud.apply(ID), CloudSendResult::Ok);
    assert!(std::fs::read_to_string(w.repo.0.join("cart.js")).unwrap().contains("items.reduce("));
    assert!(regex::Regex::new(r"^\w+ first\n$").unwrap().is_match(&git(&w.repo.0, &["log", "--oneline"])));
    assert!(!w.worktree().exists());
    assert_eq!(git(&w.repo.0, &["branch", "--list", &format!("worktree-{}", w.name)]), "");
    // The session's own branch is the person's to keep or not.
    assert!(git(&w.repo.0, &["branch", "--list", "claude/fix-cart-total"]).contains("claude/fix-cart-total"));
}

#[test]
// case: Apply that does not fit changes nothing and says why
fn apply_that_does_not_fit_changes_nothing_and_says_why() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let cloud = cloud(&w, teleporting(&w, &runs, TELEPORT));
    cloud.check(ID);
    std::fs::write(w.repo.0.join("cart.js"), "export const total = () => 0 // mine\n").unwrap();
    let CloudSendResult::Err { message } = cloud.apply(ID) else { panic!("applied over the person's own change") };
    assert!(regex::Regex::new(r"does not apply cleanly.*Nothing was changed\.$").unwrap().is_match(&message), "{message}");
    assert_eq!(std::fs::read_to_string(w.repo.0.join("cart.js")).unwrap(), "export const total = () => 0 // mine\n");
}

#[test]
// case: checking again starts from a fresh worktree, not the last one
fn checking_again_starts_from_a_fresh_worktree_not_the_last_one() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let cloud = cloud(&w, teleporting(&w, &runs, TELEPORT));
    assert!(matches!(cloud.check(ID), ClaudeCloudReading::Ok { .. }));
    // The teleport would refuse a worktree that is already there; Locust removes it first.
    assert!(matches!(cloud.check(ID), ClaudeCloudReading::Ok { .. }));
    assert_eq!(runs.borrow().len(), 2);
}

#[test]
// case: Forget removes the worktree too
fn forget_removes_the_worktree_too() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let cloud = cloud(&w, teleporting(&w, &runs, TELEPORT));
    cloud.check(ID);
    assert!(w.worktree().exists());
    cloud.forget(ID);
    assert!(!w.worktree().exists());
    assert!(cloud.list(&w.repo.text()).is_empty());
}

#[test]
// case: a session that pushed no branch is read, with no change and the reason
fn a_session_that_pushed_no_branch_is_read_with_no_change_and_the_reason() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let drawn = "\u{25cf} Session resumed without branch: Failed to checkout branch 'claude/session-confirmation-hb9mlu'\r\n ? for shortcuts";
    let (exchanges, diff, note) = read_ok(cloud(&w, teleporting(&w, &runs, Teleport { drawn: Some(drawn), change: false, ..TELEPORT })).check(ID));
    assert_eq!(exchanges.len(), 1);
    assert_eq!(diff, None);
    assert!(note.unwrap_or_default().contains("could not check out its branch"));
}

#[test]
// case: runs as the person’s own Claude Code, not as a child of one, so the transcript is saved
fn runs_as_the_persons_own_claude_code_not_as_a_child_of_one_so_the_transcript_is_saved() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    cloud(&w, teleporting(&w, &runs, TELEPORT))
        .with_process_env(|| vec![("CLAUDE_CODE_CHILD_SESSION".into(), "1".into()), ("PATH".into(), "/bin".into())])
        .check(ID);
    let runs = runs.borrow();
    assert!(runs[0].env_remove.iter().any(|key| key == "CLAUDE_CODE_CHILD_SESSION"));
    assert_eq!(runs[0].env.get("CLAUDE_CODE_CHILD_SESSION"), None);
}

#[test]
// case: brought in but no transcript saved (measured, 2 runs of 6): the change is shown, and why there are no words
fn brought_in_but_no_transcript_saved_the_change_is_shown_and_why_there_are_no_words() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let (exchanges, diff, note) = read_ok(cloud(&w, teleporting(&w, &runs, Teleport { transcript: false, ..TELEPORT })).check(ID));
    assert_eq!(exchanges, Vec::new());
    assert!(diff.unwrap_or_default().contains("cart.js"));
    assert!(note.unwrap_or_default().contains("did not save its conversation this time. Check again"));
}

#[test]
// case: not brought in at all: says what Claude Code said, and leaves no worktree behind
fn not_brought_in_at_all_says_what_claude_code_said_and_leaves_no_worktree_behind() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let teleport = Teleport { transcript: false, worktree: false, drawn: Some("Error: Could not reach Claude\r\n"), ..TELEPORT };
    assert_eq!(
        cloud(&w, teleporting(&w, &runs, teleport)).check(ID),
        ClaudeCloudReading::Err { message: "Claude Code did not bring the session in: Could not reach Claude".into() }
    );
    assert!(!w.worktree().exists());
}

#[test]
// case: a session archived or deleted (measured 10/03): the worktree Claude Code made first is removed, and it says the session is gone
fn a_session_archived_or_deleted_the_worktree_claude_code_made_first_is_removed_and_it_says_the_session_is_gone() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    // Claude Code 2.1.288 makes and locks the worktree, then looks the session up: no branch, no transcript.
    let teleport = Teleport { transcript: false, change: false, drawn: Some("Session not found: session_01ARukdj17v2vKLMYm6pc6WE\r\n"), ..TELEPORT };
    let message = read_err(cloud(&w, teleporting(&w, &runs, teleport)).check(ID));
    assert!(regex::Regex::new("not found in Claude.s cloud: it was archived or deleted").unwrap().is_match(&message), "{message}");
    assert!(!message.contains("did not save its conversation"));
    assert!(!w.worktree().exists());
    assert_eq!(git(&w.repo.0, &["branch", "--list", &format!("worktree-{}", w.name)]), "");
}

#[test]
// case: the trust question is left to the person; nothing is answered
fn the_trust_question_is_left_to_the_person_nothing_is_answered() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    let teleport = Teleport { drawn: Some("Quick safety check: Is this a project you created or one you trust?"), ..TELEPORT };
    let message = read_err(cloud(&w, teleporting(&w, &runs, teleport)).check(ID));
    assert!(message.contains("asks whether you trust this folder"), "{message}");
}

#[test]
// case: a folder that is not a git checkout is told so, and nothing runs
fn a_folder_that_is_not_a_git_checkout_is_told_so_and_nothing_runs() {
    let w = World::new();
    let plain = Temp::new("plain");
    w.know(&plain.text());
    let runs = RefCell::new(Vec::new());
    let cloud = cloud(&w, teleporting(&w, &runs, TELEPORT)).with_git(|_: &[&str], _: &str, _: Option<&str>| GitRan {
        code: 128,
        stdout: String::new(),
        stderr: "fatal: not a git repository".into(),
    });
    assert!(matches!(cloud.check(ID), ClaudeCloudReading::Err { .. }));
    assert!(runs.borrow().is_empty());
}

#[test]
// case: knows the long-paths failure and offers the fix in words, never setting it
fn knows_the_long_paths_failure_and_offers_the_fix_in_words_never_setting_it() {
    assert_eq!(read_teleport("error: unable to create file x/y/z: Filename too long").problem, Some(ReadingProblem::LongPaths));
}

#[test]
// case: knows a folder Claude Code was never told to trust, as 2.1.29x says it, and leaves the answer to the person
fn knows_a_folder_claude_code_was_never_told_to_trust_as_2_1_29x_says_it_and_leaves_the_answer_to_the_person() {
    // Drawn verbatim by Claude Code 2.1.293 on 2026-10-07 in a checkout it had never been opened in.
    let drawn = "\u{1b}[2J\u{1b}[m\u{1b}[H\u{1b}]0;claude\u{7}\u{1b}[31mError creating worktree: Workspace trust not yet accepted. Run `claude` once in this directory and accept the trust dialog, then retry with --worktree.\r\n\u{1b}[m";
    let said = read_teleport(drawn);
    assert_eq!(said.problem, Some(ReadingProblem::Asks));
    assert!(!said.gone);
    assert!(said.error.unwrap_or_default().starts_with("Workspace trust not yet accepted"));
}

#[test]
// case: keeps the words of an error it does not know, for the message
fn keeps_the_words_of_an_error_it_does_not_know_for_the_message() {
    assert_eq!(read_teleport("Error: Something new went wrong").error.as_deref(), Some("Something new went wrong"));
    assert_eq!(read_teleport("Error creating worktree: Something else").error.as_deref(), Some("Something else"));
}

#[test]
// case: the worktree on the command line is only ever Locust’s own name
fn the_worktree_on_the_command_line_is_only_ever_locusts_own_name() {
    let line = windows_command_line(
        "C:\\c.cmd",
        &[],
        CloudAction::Read,
        &WindowsCommandOptions { session: Some(SESSION), worktree: Some("x & del *"), stay: Some(false), ..Default::default() },
    )
    .unwrap();
    assert_eq!(line, "/d /c \"\"C:\\c.cmd\" --teleport session_01G8RT9yXr7dbhVM7EUzpvEa --worktree \"");
    assert_eq!(reading_worktree_name("cc_0123456789abcdef0123"), "locust-cloud-0123456789ab");
}

/// Claude Code's teleport played by a script where Claude Code would be: real git makes and locks the worktree and
/// commits the session's change, the transcript is written, "Session resumed" is drawn, and it never exits.
#[cfg(target_os = "macos")]
const FAKE_CLAUDE: &str = r#"#!/bin/sh
trap '' HUP TERM INT
echo $$ > "$FAKE_PID"
[ -t 1 ] || { echo "Error: not a terminal"; exit 1; }
[ "$1" = --teleport ] && [ "$2" = "$FAKE_SESSION" ] && [ "$3" = --worktree ] || { echo "Error: wrong words: $*"; exit 1; }
place="$(pwd -P)/.claude/worktrees/$4"
git worktree add -q -b "worktree-$4" "$place" && git worktree lock "$place" || exit 1
cd "$place" && git checkout -q -B claude/fix-cart-total || exit 1
printf 'export const total = (items) => items.reduce((sum, item) => sum + item.price, 0)\n' > cart.js
git commit -q -am "Sum prices" || exit 1
mkdir -p "$FAKE_TRANSCRIPTS" && cp "$FAKE_TRANSCRIPT" "$FAKE_TRANSCRIPTS/26be863b-6ead-4c62-932c-07388873cd26.jsonl"
printf ' \342\234\224 Checking out branch\r\n\342\227\217 Session resumed\r\n\342\235\257 '
while :; do sleep 1; done
"#;

#[cfg(target_os = "macos")]
#[test]
// case: teleports into a worktree of its own, out of sight, and stops once Claude Code is up
fn on_a_mac_the_app_reads_a_session_through_a_hidden_terminal_and_ends_claude_code_once_its_transcript_is_written() {
    use std::os::unix::fs::PermissionsExt;
    let w = World::new();
    let tools = Temp::new("tools");
    let data = Temp::new("data");
    let script = tools.0.join("claude");
    std::fs::write(&script, FAKE_CLAUDE).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let transcript = tools.0.join("transcript.jsonl");
    std::fs::write(&transcript, TRANSCRIPT).unwrap();
    let pid_file = tools.0.join("pid");
    let env: std::collections::BTreeMap<String, String> = [
        ("FAKE_PID", pid_file.to_string_lossy().into_owned()),
        ("FAKE_SESSION", SESSION.to_string()),
        ("FAKE_TRANSCRIPT", transcript.to_string_lossy().into_owned()),
        ("FAKE_TRANSCRIPTS", w.home.0.join("projects").join(slug(&w.worktree())).to_string_lossy().into_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let store = crate::store::Store::new(data.0.join("native"), data.0.join("electron")).unwrap();
    let kept = super::store_path_of(&store);
    std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
    std::fs::copy(w.store(), &kept).unwrap();
    let path = script.to_string_lossy().into_owned();
    // The app's own builder, as the panel makes it; only where Claude Code is found differs.
    let cloud = super::on_this_computer_with(&store, std::sync::Arc::default(), move || {
        Some(ClaudeLaunch { discovered_path: path.clone(), executable_path: path.clone(), prefix_args: Vec::new(), env: Some(env.clone()) })
    })
    .with_claude_home(&w.home.0);
    let started = std::time::Instant::now();
    let (exchanges, diff, note) = read_ok(cloud.check(ID));
    // Ended once the transcript stopped growing, long before the read's two minutes.
    assert!(started.elapsed() < std::time::Duration::from_secs(30), "waited {:?}", started.elapsed());
    assert_eq!(exchanges.len(), 1);
    assert_eq!(exchanges[0].prompt, "Fix the cart total");
    assert_eq!(exchanges[0].answer.as_deref(), Some("Fixed: **total** now sums prices."));
    assert!(diff.as_deref().is_some_and(|d| d.contains("item.price")), "{diff:?}");
    assert_eq!(note, None);
    // The folder itself is untouched until Apply.
    assert_eq!(std::fs::read_to_string(w.repo.0.join("cart.js")).unwrap(), "export const total = (items) => items.length\n");
    let pid: i32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while unsafe { libc::kill(pid, 0) } == 0 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "Claude Code was left running");
}

#[test]
fn on_a_mac_the_read_runs_claude_code_by_its_words_with_no_shell_between() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    fn mac<'a>(w: &'a World, runs: &'a RefCell<Vec<PseudoTerminalRun>>) -> ClaudeCloud<'a> {
        let launch = || {
            Some(ClaudeLaunch {
                discovered_path: "/usr/local/bin/claude".into(),
                executable_path: "/usr/local/bin/claude".into(),
                prefix_args: Vec::new(),
                env: None,
            })
        };
        let mut cloud = ClaudeCloud::new(w.store(), CloudPlatform::Darwin, launch)
            .with_terminal(teleporting(w, runs, TELEPORT))
            .with_claude_home(&w.home.0)
            .with_process_env(Vec::new);
        cloud.transcript_pause = std::time::Duration::ZERO;
        cloud
    }
    read_ok(mac(&w, &runs).check(ID));
    let runs = runs.borrow();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].argv, ["/usr/local/bin/claude", "--teleport", SESSION, "--worktree", w.name.as_str()]);
    assert_eq!(runs[0].line, "");
    assert_eq!(runs[0].cwd, w.repo.text());
}

#[test]
// case: Cloud view 1: a read keeps the session's own branch, never the worktree's made-up one, for the light look
fn a_read_keeps_the_sessions_own_branch_in_its_record_and_one_without_a_branch_keeps_none() {
    let w = World::new();
    let runs = RefCell::new(Vec::new());
    read_ok(cloud(&w, teleporting(&w, &runs, TELEPORT)).check(ID));
    let kept = cloud(&w, teleporting(&w, &runs, TELEPORT)).all().remove(0);
    assert_eq!(kept.branch.as_deref(), Some("claude/fix-cart-total"));
    assert_eq!(kept.last_reply.as_deref(), Some("Fixed: **total** now sums prices."));
    let w = World::new();
    let drawn = "\u{25cf} Session resumed without branch: Failed to checkout branch 'claude/session-confirmation-hb9mlu'\r\n ? for shortcuts";
    read_ok(cloud(&w, teleporting(&w, &runs, Teleport { drawn: Some(drawn), change: false, ..TELEPORT })).check(ID));
    let kept = cloud(&w, teleporting(&w, &runs, TELEPORT)).all().remove(0);
    assert_eq!(kept.branch, None, "{kept:?}");
}
