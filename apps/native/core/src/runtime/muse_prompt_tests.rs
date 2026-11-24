//! Electron's `packages/runtime-adapters/src/the-prompt-muse-cannot-be-told.test.ts`, case by case, through the caller:
//! a native Muse turn builds its command with `muse::command` (here `command_from`, the same body with the launcher
//! handed in, so no `muse` need be on PATH), writes the prompt with `prepare_prompt_file`, and the runtime's `start`
//! refuses a turn with nothing to say.
use super::*;
use crate::store::Store;

const WORKSPACE: &str = "C:\\work";

fn build(mode: Mode, effort: Option<&str>, resume: Option<&str>) -> io::Result<Command> {
    command_from(|| Ok(PathBuf::from("muse")), Path::new(WORKSPACE), mode, None, effort, resume)
}

fn args(command: &Command) -> Vec<String> {
    command.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
}

fn joined(mode: Mode) -> String {
    args(&build(mode, None, None).unwrap()).join(" ")
}

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp() -> Temp {
    let root = std::env::temp_dir().join(format!("locust-muse-prompt-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    Temp(root)
}

#[test]
fn names_where_the_prompt_file_path_goes_and_never_puts_the_prompt_in_argv() {
    // case: names where the prompt file path goes, and never puts the prompt in argv
    let root = temp();
    let store = Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let mut short = build(Mode::Ask, None, None).unwrap();
    assert!(args(&short).iter().any(|a| a == PROMPT_FILE_PLACEHOLDER));
    let mut long = build(Mode::Ask, None, None).unwrap();
    let short_file = prepare_prompt_file(&mut short, "Reply with exactly PING", &store).unwrap();
    let long_prompt = "x".repeat(40_000);
    let long_file = prepare_prompt_file(&mut long, &long_prompt, &store).unwrap();
    // The prompt is in the file, and the command line is not a character wider for a 40,000-character brief.
    assert_eq!(std::fs::read_to_string(&long_file).unwrap(), long_prompt);
    assert_eq!(std::fs::read_to_string(&short_file).unwrap(), "Reply with exactly PING");
    assert!(!args(&long).join(" ").contains("xxxx"));
    assert_eq!(args(&long).join(" ").len(), args(&short).join(" ").len());
}

#[test]
fn still_refuses_to_build_a_run_with_nothing_to_say() {
    // case: still refuses to build a run with nothing to say
    let root = temp();
    let mut command = Command::new("python3");
    command.arg("-c").arg("pass").current_dir(&root.0);
    let mut runtime = Muse::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "  ".into(),
        folder: root.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::AcceptEdits,
        approvals: None,
        mission_id: "m".into(),
        run_id: "r".into(),
    };
    assert!(runtime.start(&turn).is_err());
}

#[test]
fn holds_a_read_only_run_with_the_flags_that_actually_hold_it() {
    // case: holds a read-only run with the flags that actually hold it
    let said = args(&build(Mode::Ask, None, None).unwrap());
    assert!(said.contains(&"--disable-write".into()));
    assert!(said.contains(&"--disable-shell".into()));
    assert!(!said.contains(&"--approval-mode".into()));
}

#[test]
fn lets_a_writing_run_finish_without_waiting_for_an_approval_nobody_can_give() {
    // case: lets a writing run finish without waiting for an approval nobody can give
    let said = args(&build(Mode::AcceptEdits, None, None).unwrap());
    assert!(said.join(" ").contains("--approval-mode never"));
    for flag in ["--disable-write", "--yolo", "--disable-sandbox"] {
        assert!(!said.contains(&flag.into()), "{flag}");
    }
}

#[test]
fn runs_autos_commands_outside_muses_own_sandbox_and_only_autos_0_773() {
    // case: runs Auto's commands outside Muse's own sandbox, and only Auto's (0.773)
    let auto = joined(Mode::Auto);
    assert!(auto.contains("--approval-mode never"));
    assert!(auto.contains("--disable-sandbox"));
    assert!(!auto.contains("--yolo"));
    // Electron also refuses an Edit spec someone appended `--disable-sandbox` to, at its last gate. Rust's command is
    // built whole from the mode and nothing appends to it, so the Edit run is checked to carry none.
    assert!(!joined(Mode::AcceptEdits).contains("--disable-sandbox"));
    assert!(!joined(Mode::Ask).contains("--disable-sandbox"));
}

#[test]
fn refuses_an_effort_the_cli_would_reject_the_whole_run_for() {
    // case: refuses an effort the CLI would reject the whole run for
    assert!(args(&build(Mode::Ask, Some("xhigh"), None).unwrap()).join(" ").contains("--reasoning-effort xhigh"));
    let refused = build(Mode::Ask, Some("blistering"), None).unwrap_err().to_string();
    assert!(refused.to_lowercase().contains("reasoning effort"), "{refused}");
}

#[test]
fn continues_a_conversation_by_its_own_session_id() {
    // case: continues a conversation by its own session id
    let said = args(&build(Mode::Ask, None, Some("01a0c5fc-bc34-7583-8bce-9e480f66e20b")).unwrap()).join(" ");
    assert!(said.contains("--session-id 01a0c5fc-bc34-7583-8bce-9e480f66e20b"), "{said}");
}

#[test]
fn roots_the_policy_gated_tools_at_the_workspace_not_just_the_cwd() {
    // case: roots the policy-gated tools at the workspace, not just the cwd
    assert!(joined(Mode::Ask).contains("--workspace C:\\work"));
}
