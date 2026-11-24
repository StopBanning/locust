//! a-cloud-session-is-known-by-its-id.test.ts: what `claude --cloud` draws is read for the session's id and link,
//! a start opens no window unless Claude Code asks the person something, and a known session is brought home and
//! sent to by its id. Claude Code is played by closures, as Electron's test plays it; nothing real runs.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use regex::Regex;

use super::{
    ASKS_THE_PERSON, ClaudeCloud, ClaudeCloudAnswer, ClaudeLaunch, CloudAction, CloudChoice,
    CloudPlatform, CloudSendResult, CloudStartReading, ExecOptions, ExecReply, PseudoTerminalResult,
    PseudoTerminalRun, TASK_VARIABLE, WindowKind, WindowRun, WindowsCommandOptions, choice_args,
    plain_terminal_text, read_cloud_start, windows_command_line,
};

// What Claude Code 2.1.288 drew under a pseudo console, 2026-10-02, verbatim (from Electron's test).
const CREATED: &str = "\u{1b}[2J\u{1b}[m\u{1b}[H\u{1b}]0;C:\\Windows\\SYSTEM32\\cmd.exe\u{7}\u{1b}[?25h\u{1b}]0;claude\u{7}\u{1b}[?2004h\u{1b}[?2031h\u{1b}[?1004h\u{1b}[?9001l\u{1b}[<u\u{1b}[>5u\u{1b}[>4;2m\u{1b}[?25l\u{1b}]11;?\u{7}\u{1b}[>0q\u{1b}[?u\u{1b}[>4m\u{1b}[<u\u{1b}[?1004l\u{1b}[?2031l\u{1b}[?2004lCreated cloud session: Session confirmation\u{1b}[?1016l\u{1b}[>4m\u{1b}[?1004l\u{1b}[?2031l\u{1b}[?2004l\u{1b}[<u\u{d}\u{a}View: https://claude.ai/code/session_01Jed6HzK3Np9xeyKnSQ7oBG?from=cli&m=0\u{1b}[49X\u{d}\u{a}Resume with: claude --teleport session_01Jed6HzK3Np9xeyKnSQ7oBG\u{1b}[60X\u{d}\u{a}This checkout is on a detached HEAD, so the cloud session starts from its own commit, which origin/main has (da8b7798a7a7).\u{d}\u{a}\u{f}\u{1b}[122X\u{1b}]0;\u{7}\u{1b}[?25h";
const TRUST: &str = "\u{1b}[2J\u{1b}[m\u{1b}[H\u{1b}]0;C:\\Windows\\SYSTEM32\\cmd.exe\u{7}\u{1b}[?25h\u{1b}]0;claude\u{7}\u{1b}[?2004h\u{1b}[?2031h\u{1b}[?1004h\u{1b}[?9001l\u{1b}[<u\u{1b}[>5u\u{1b}[>4;2m\u{1b}[?25l\u{1b}[?2026h\u{1b}]8;id=zaxmda;https://code.claude.com/docs/en/security\u{7}\u{1b}]8;;\u{7}\u{1b}[?2026l\u{1b}]11;?\u{7}\u{1b}[>0q\u{1b}[?u\u{d}\u{a}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{d}\u{a} Accessing workspace:\u{1b}[K\u{d}\u{a}\u{1b}[K\u{d}\u{a} C:\\Users\\<home>\\Documents\\locust-scratch\\cloud-untrusted\u{1b}[K\u{d}\u{a}\u{1b}[K\u{d}\u{a} Quick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source project, or work from    \u{d}\u{a} your team). If not, take a moment to review what's in this folder first.\u{1b}[K\u{d}\u{a}\u{1b}[K\u{d}\u{a} Claude Code'll be able to read, edit, and execute files here.\u{1b}[K\u{d}\u{a}\u{1b}[K\u{d}\u{a} Security guide\u{1b}[K\u{d}\u{a}\u{1b}[K\u{d}\u{a} \u{276f} No, exit\u{1b}[K\u{d}\u{a}   Yes, I trust this folder\u{1b}[K\u{d}\u{a}\u{1b}[K\u{d}\u{a} Enter to confirm \u{b7} Esc to cancel\u{1b}[K\u{1b}[107C\u{1b}[?1004l\u{1b}[>4m\u{1b}[<u\u{1b}[?2031l\u{1b}[?2004l\u{1b}[?1016l\u{d}\u{a}\u{f}\u{1b}]0;\u{7}\u{1b}[?25h";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("locust-claude-cloud-id-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }
    fn store(&self) -> PathBuf {
        self.0.join("claude-cloud.json")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

/// What the fakes saw: windows opened, hidden terminal runs, commands run for their output.
#[derive(Default)]
struct Seen {
    windows: RefCell<Vec<WindowRun>>,
    runs: RefCell<Vec<PseudoTerminalRun>>,
    execs: RefCell<Vec<(String, Vec<String>, ExecOptions)>>,
}

/// Electron's `createClaudeCloud({ discover: claude, platform: 'win32', run: fakeSpawn, terminal })`.
fn cloud<'a>(dir: &TempDir, seen: &'a Seen, drawn: Option<&'a str>) -> ClaudeCloud<'a> {
    let cloud = ClaudeCloud::new(dir.store(), CloudPlatform::Win32, claude).with_run_window(
        move |run: &WindowRun| {
            seen.windows.borrow_mut().push(run.clone());
            Ok(())
        },
    );
    match drawn {
        None => cloud,
        Some(drawn) => cloud.with_terminal(move |run: &PseudoTerminalRun| {
            seen.runs.borrow_mut().push(run.clone());
            PseudoTerminalResult::Ok { drawn: drawn.into() }
        }),
    }
}

fn started(answer: ClaudeCloudAnswer) -> super::ClaudeCloudSession {
    match answer {
        ClaudeCloudAnswer::Ok { session } => session,
        ClaudeCloudAnswer::Err { message } => panic!("not started: {message}"),
    }
}

#[test]
// case: is read for the id, the link, the title and where it starts from
fn is_read_for_the_id_the_link_the_title_and_where_it_starts_from() {
    assert_eq!(
        read_cloud_start(CREATED),
        CloudStartReading::Created {
            session_id: "session_01Jed6HzK3Np9xeyKnSQ7oBG".into(),
            url: "https://claude.ai/code/session_01Jed6HzK3Np9xeyKnSQ7oBG".into(),
            title: Some("Session confirmation".into()),
            note: Some("This checkout is on a detached HEAD, so the cloud session starts from its own commit, which origin/main has (da8b7798a7a7).".into()),
        }
    );
}

#[test]
// case: knows the trust question for what it is
fn knows_the_trust_question_for_what_it_is() {
    assert_eq!(read_cloud_start(TRUST), CloudStartReading::Asks);
    let asks = Regex::new(&format!("(?i){ASKS_THE_PERSON}")).unwrap();
    assert!(asks.is_match(&plain_terminal_text(TRUST)));
}

#[test]
// case: passes on what Claude Code said when it refused
fn passes_on_what_claude_code_said_when_it_refused() {
    assert_eq!(
        read_cloud_start("\u{1b}[2JError: --cloud requires a description.\r\n"),
        CloudStartReading::Failed { message: Some("--cloud requires a description.".into()) }
    );
}

#[test]
// case: opens no window, keeps the id, and never puts the task on the command line
fn opens_no_window_keeps_the_id_and_never_puts_the_task_on_the_command_line() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let cloud = cloud(&dir, &seen, Some(CREATED));
    let session = started(cloud.start("C:/work/app", "Fix the cart & run tests | report", None, None));
    assert!(seen.windows.borrow().is_empty());
    assert_eq!(session.session_id.as_deref(), Some("session_01Jed6HzK3Np9xeyKnSQ7oBG"));
    assert_eq!(session.url.as_deref(), Some("https://claude.ai/code/session_01Jed6HzK3Np9xeyKnSQ7oBG"));
    assert_eq!(session.title.as_deref(), Some("Session confirmation"));
    let runs = seen.runs.borrow();
    assert!(!runs[0].line.contains("Fix the cart"));
    assert!(runs[0].line.contains("--cloud \"%LOCUST_CLAUDE_CLOUD_TASK%\""));
    assert_eq!(runs[0].env.get(TASK_VARIABLE).map(String::as_str), Some("Fix the cart & run tests | report"));
    assert_eq!(runs[0].cwd, "C:/work/app");
    assert_eq!(runs[0].stop_when, "Quick safety check|trust this folder|Do you trust|Workspace trust not yet accepted");
    assert_eq!(
        cloud.list("C:/work/app").first().and_then(|s| s.session_id.clone()).as_deref(),
        Some("session_01Jed6HzK3Np9xeyKnSQ7oBG")
    );
}

#[test]
// case: opens Claude Code's window when it asks whether the folder is trusted, and says why
fn opens_claude_codes_window_when_it_asks_whether_the_folder_is_trusted_and_says_why() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let session = started(cloud(&dir, &seen, Some(TRUST)).start("C:/work/new", "Write the docs", None, None));
    let windows = seen.windows.borrow();
    assert_eq!(windows.len(), 1);
    assert!(windows[0].args[0].contains("--cloud"));
    assert_eq!(session.session_id, None);
    assert!(session.note.as_deref().unwrap_or("").contains("trust this folder"));
}

#[test]
// case: refuses, opening and keeping nothing, when Claude Code says no
fn refuses_opening_and_keeping_nothing_when_claude_code_says_no() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let cloud = cloud(&dir, &seen, Some("Error: Unable to create cloud session\r\n"));
    assert_eq!(
        cloud.start("C:/work/app", "Do it", None, None),
        ClaudeCloudAnswer::Err { message: "Claude Code did not start a cloud session: Unable to create cloud session".into() }
    );
    assert!(seen.windows.borrow().is_empty());
    assert!(cloud.list("C:/work/app").is_empty());
}

#[test]
// case: falls back to the window when the hidden terminal cannot run
fn falls_back_to_the_window_when_the_hidden_terminal_cannot_run() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let cloud = cloud(&dir, &seen, None).with_terminal(|_: &PseudoTerminalRun| PseudoTerminalResult::Err);
    assert!(matches!(cloud.start("C:/work/app", "Do it", None, None), ClaudeCloudAnswer::Ok { .. }));
    assert_eq!(seen.windows.borrow().len(), 1);
}

/// Electron's `known(gitChanges)`: a session started out of sight, then the windows list emptied.
fn known<'a>(
    dir: &TempDir,
    seen: &'a Seen,
    changes: Option<usize>,
    reply: &'a RefCell<ExecReply>,
) -> (ClaudeCloud<'a>, String) {
    let cloud = cloud(dir, seen, Some(CREATED))
        .with_git_changes(move |_: &str| changes)
        .with_exec(move |file: &str, args: &[String], options: &ExecOptions| {
            seen.execs.borrow_mut().push((file.into(), args.to_vec(), options.clone()));
            reply.borrow().clone()
        });
    let id = started(cloud.start("C:/work/app", "Fix it", None, None)).id;
    seen.windows.borrow_mut().clear();
    (cloud, id)
}

fn receipt() -> RefCell<ExecReply> {
    RefCell::new(ExecReply {
        code: 0,
        output: "{\"ok\":true,\"session_id\":\"session_01Jed6HzK3Np9xeyKnSQ7oBG\"}\n".into(),
        stdout: None,
    })
}

#[test]
// case: is brought home by its id, with no picker
fn is_brought_home_by_its_id_with_no_picker() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let reply = receipt();
    let (cloud, id) = known(&dir, &seen, Some(0), &reply);
    assert_eq!(cloud.home(&id), CloudSendResult::Ok);
    let windows = seen.windows.borrow();
    assert!(windows[0].args[0].ends_with("--teleport session_01Jed6HzK3Np9xeyKnSQ7oBG\""), "{}", windows[0].args[0]);
}

#[test]
// case: is not brought home into a folder with changes: it says so and opens nothing
fn is_not_brought_home_into_a_folder_with_changes_it_says_so_and_opens_nothing() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let reply = receipt();
    let (cloud, id) = known(&dir, &seen, Some(2), &reply);
    let CloudSendResult::Err { message } = cloud.home(&id) else { panic!("brought home into a changed folder") };
    assert!(message.contains("2 changed files"), "{message}");
    assert!(seen.windows.borrow().is_empty());
}

#[test]
// case: takes a follow-up, the words in a variable and never on the command line
fn takes_a_follow_up_the_words_in_a_variable_and_never_on_the_command_line() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let reply = receipt();
    let (cloud, id) = known(&dir, &seen, Some(0), &reply);
    assert_eq!(cloud.send(&id, "Also run the tests & report"), CloudSendResult::Ok);
    let execs = seen.execs.borrow();
    assert_eq!(execs[0].0, "cmd.exe");
    assert_eq!(
        execs[0].1[0],
        "/d /c \"\"C:\\npm\\claude.cmd\" -p --cloud session_01Jed6HzK3Np9xeyKnSQ7oBG --output-format json \"%LOCUST_CLAUDE_CLOUD_TASK%\"\""
    );
    assert_eq!(execs[0].2.env.get(TASK_VARIABLE).map(String::as_str), Some("Also run the tests & report"));
}

#[test]
// case: says what Claude Code said when a follow-up does not go
fn says_what_claude_code_said_when_a_follow_up_does_not_go() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let reply = receipt();
    let (cloud, id) = known(&dir, &seen, Some(0), &reply);
    *reply.borrow_mut() = ExecReply {
        code: 1,
        output: "Error: failed to send message to cloud session: Session not found\n".into(),
        stdout: None,
    };
    assert_eq!(
        cloud.send(&id, "More"),
        CloudSendResult::Err {
            message: "Claude Code did not send that: failed to send message to cloud session: Session not found".into()
        }
    );
}

#[test]
// case: go with the task, hidden or in the window
fn go_with_the_task_hidden_or_in_the_window() {
    let dir = TempDir::new();
    let seen = Seen::default();
    let sonnet = CloudChoice { model: Some("claude-sonnet-5-5".into()), effort: Some("low".into()), environment: None };
    cloud(&dir, &seen, Some(CREATED)).start("C:/work/app", "Fix it", None, Some(&sonnet));
    assert!(seen.runs.borrow()[0].line.contains("--model claude-sonnet-5-5 --effort low --cloud \"%"));
    let other = TempDir::new();
    let shown = Seen::default();
    let haiku = CloudChoice { model: Some("haiku".into()), ..Default::default() };
    cloud(&other, &shown, None).start("C:/work/app", "Fix it", None, Some(&haiku));
    let windows = shown.windows.borrow();
    assert_eq!(windows[0].kind, WindowKind::Console);
    assert!(windows[0].args[0].contains("--model haiku --cloud \"%"), "{}", windows[0].args[0]);
}

#[test]
// case: pass only a plain name and a known level: the account default, and anything else, is left out
fn pass_only_a_plain_name_and_a_known_level_the_account_default_and_anything_else_is_left_out() {
    let choice = |model: &str, effort: Option<&str>| CloudChoice {
        model: Some(model.into()),
        effort: effort.map(str::to_string),
        environment: None,
    };
    assert_eq!(choice_args(Some(&choice("account-default", Some("low")))).unwrap(), vec!["--effort", "low"]);
    assert_eq!(choice_args(Some(&choice("opus & calc", Some("turbo")))).unwrap(), Vec::<String>::new());
    assert_eq!(choice_args(Some(&choice("claude-opus-5-5[1m]", None))).unwrap(), vec!["--model", "claude-opus-5-5[1m]"]);
    assert_eq!(choice_args(None).unwrap(), Vec::<String>::new());
}

#[test]
// case: carries a session id only when it is one: anything else is never passed to cmd
fn carries_a_session_id_only_when_it_is_one_anything_else_is_never_passed_to_cmd() {
    let home = |session: &str| {
        windows_command_line(
            "C:\\c.cmd",
            &[],
            CloudAction::Home,
            &WindowsCommandOptions { session: Some(session), ..Default::default() },
        )
        .unwrap()
    };
    assert_eq!(home("session_01Jed6HzK3Np9xeyKnSQ7oBG"), "/d /k \"\"C:\\c.cmd\" --teleport session_01Jed6HzK3Np9xeyKnSQ7oBG\"");
    assert_eq!(home("session_1 & calc"), "/d /k \"\"C:\\c.cmd\" --teleport\"");
}
