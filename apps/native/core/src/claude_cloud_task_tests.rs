//! a-task-goes-to-claudes-cloud.test.ts: `claude --cloud` needs a real terminal, so Locust opens one, and what the
//! person wrote reaches Claude Code as words, never as part of a command line cmd reads. Windows are recorded, never
//! opened.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    ClaudeCloud, ClaudeCloudAnswer, ClaudeLaunch, CloudAction, CloudPlatform, CloudSendResult, TASK_VARIABLE,
    WindowKind, WindowRun, WindowsCommandOptions, cloud_task_text, windows_command_line,
};

struct Store(PathBuf);

impl Store {
    fn new() -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("locust-claude-cloud-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> PathBuf {
        self.0.join("claude-cloud.json")
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const CLAUDE: &str = "C:\\Users\\a b\\AppData\\Roaming\\npm\\claude.cmd";

fn claude() -> Option<ClaudeLaunch> {
    Some(ClaudeLaunch { discovered_path: CLAUDE.into(), executable_path: CLAUDE.into(), prefix_args: Vec::new(), env: None })
}

fn cloud<'a>(store: &Store, spawned: &'a RefCell<Vec<WindowRun>>, discover: fn() -> Option<ClaudeLaunch>) -> ClaudeCloud<'a> {
    ClaudeCloud::new(store.path(), CloudPlatform::Win32, discover).with_run_window(move |run: &WindowRun| {
        spawned.borrow_mut().push(run.clone());
        Ok(())
    })
}

#[test]
// case: is one line with no double quote
fn is_one_line_with_no_double_quote() {
    assert_eq!(cloud_task_text("Fix \"the\" bug\r\nthen\ttest  it"), "Fix 'the' bug then test it");
}

#[test]
// case: is never in the command line: cmd sees only a quoted variable
fn is_never_in_the_command_line_cmd_sees_only_a_quoted_variable() {
    let line = |action| windows_command_line("C:\\a b\\claude.cmd", &[], action, &WindowsCommandOptions::default()).unwrap();
    assert_eq!(line(CloudAction::Start), "/d /k \"\"C:\\a b\\claude.cmd\" --cloud \"%LOCUST_CLAUDE_CLOUD_TASK%\"\"");
    assert_eq!(line(CloudAction::Home), "/d /k \"\"C:\\a b\\claude.cmd\" --teleport\"");
}

#[test]
// case: opens Claude Code in a window of its own, in the folder, with the task in the variable -- & and | stay words
fn opens_claude_code_in_a_window_of_its_own_in_the_folder_with_the_task_in_the_variable_and_and_stay_words() {
    let store = Store::new();
    let spawned = RefCell::new(Vec::new());
    let cloud = cloud(&store, &spawned, claude);
    let sent = cloud.start("C:/work/app", "Fix the cart & run tests | report \"done\"", Some("tm_wren"), None);
    assert!(matches!(sent, ClaudeCloudAnswer::Ok { .. }));
    let spawned = spawned.borrow();
    assert_eq!(spawned.len(), 1);
    let call = &spawned[0];
    assert_eq!(call.file, "cmd.exe");
    assert_eq!(call.cwd, "C:/work/app");
    // Electron's `detached: true, windowsVerbatimArguments: true`: a console of its own, its line passed as is.
    assert_eq!(call.kind, WindowKind::Console);
    assert!(!call.args.join(" ").contains("Fix the cart"));
    assert_eq!(call.env.get(TASK_VARIABLE).map(String::as_str), Some("Fix the cart & run tests | report 'done'"));
    // Listed for this folder only, without saying where it is.
    let prompts: Vec<String> = cloud.list("C:/work/app").into_iter().map(|s| s.prompt).collect();
    assert_eq!(prompts, vec!["Fix the cart & run tests | report 'done'"]);
    assert!(cloud.list("C:/elsewhere").is_empty());
}

#[test]
// case: brings one home with --teleport in its own folder, and forgets one on request
fn brings_one_home_with_teleport_in_its_own_folder_and_forgets_one_on_request() {
    let store = Store::new();
    let spawned = RefCell::new(Vec::new());
    let cloud = cloud(&store, &spawned, claude).with_git_changes(|_: &str| None);
    let ClaudeCloudAnswer::Ok { session } = cloud.start("C:/work/app", "Write the docs", None, None) else { panic!("not sent") };
    assert_eq!(cloud.home(&session.id), CloudSendResult::Ok);
    {
        let spawned = spawned.borrow();
        assert!(spawned[1].args[0].ends_with("--teleport\""), "{}", spawned[1].args[0]);
        assert_eq!(spawned[1].cwd, "C:/work/app");
        assert_eq!(spawned[1].env.get(TASK_VARIABLE), None);
    }
    cloud.forget(&session.id);
    assert!(cloud.list("C:/work/app").is_empty());
    assert!(matches!(cloud.home(&session.id), CloudSendResult::Err { .. }));
}

#[test]
// case: says so, and opens nothing, when Claude Code is not installed or the task is empty
fn says_so_and_opens_nothing_when_claude_code_is_not_installed_or_the_task_is_empty() {
    let spawned = RefCell::new(Vec::new());
    let none_store = Store::new();
    assert_eq!(
        cloud(&none_store, &spawned, || None).start("C:/work", "Do it", None, None),
        ClaudeCloudAnswer::Err { message: "Claude Code is not installed here. Settings > AI agents shows how to add it.".into() }
    );
    let store = Store::new();
    assert!(matches!(cloud(&store, &spawned, claude).start("C:/work", "  \n ", None, None), ClaudeCloudAnswer::Err { .. }));
    assert!(spawned.borrow().is_empty());
}
