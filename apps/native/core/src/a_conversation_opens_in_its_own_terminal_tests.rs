//! Electron `main/a-conversation-opens-in-its-own-terminal.test.ts`, case by case. Expected words and values are
//! Electron's (`main/open-in-terminal.ts`, `main/runtime-sign-in.ts`).

use super::*;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| (*w).to_owned()).collect()
}

fn node(script: &str) -> TerminalLaunch {
    TerminalLaunch {
        command_name: "codex".into(),
        executable_path: "C:\\Program Files\\nodejs\\node.exe".into(),
        prefix_args: words(&[script]),
        env: None,
    }
}
fn codex() -> TerminalLaunch {
    node("C:\\Users\\Jane Doe\\AppData\\Roaming\\npm\\node_modules\\@openai\\codex\\bin\\codex.js")
}
fn claude() -> TerminalLaunch {
    TerminalLaunch { command_name: "claude".into(), executable_path: "C:\\npm\\claude.exe".into(), prefix_args: Vec::new(), env: None }
}
fn cursor() -> TerminalLaunch {
    TerminalLaunch {
        command_name: "cursor-agent".into(),
        executable_path: "C:\\Windows\\System32\\cmd.exe".into(),
        prefix_args: words(&["/d", "/s", "/c", "C:\\Users\\Jane Doe\\AppData\\Local\\cursor-agent\\cursor-agent.cmd"]),
        env: None,
    }
}

const ID: &str = "28232cc1-50b4-4789-9d84-c756818e8eec";

// the words that resume a session, read off each CLI's own --help on 2026-09-26

#[test]
fn are_each_runtimes_own_flag() {
    assert_eq!(resume_args("codex", ID), Some(words(&["resume", ID])));
    assert_eq!(resume_args("claude", ID), Some(words(&["--resume", ID])));
    assert_eq!(resume_args("copilot", ID), Some(vec![format!("--resume={ID}")]));
    assert_eq!(resume_args("cursor", ID), Some(vec![format!("--resume={ID}")]));
    assert_eq!(resume_args("opencode", "ses_3f1a9c"), Some(words(&["--session", "ses_3f1a9c"])));
    assert_eq!(resume_args("muse", ID), Some(words(&["resume", ID])));
    // Antigravity CLI (0.543): agy --conversation <id>.
    assert_eq!(resume_args("antigravity", ID), Some(words(&["--conversation", ID])));
}

#[test]
fn do_not_exist_for_a_runtime_that_does_not_run_here() {
    assert_eq!(resume_args("gemini", ID), None);
    assert_eq!(resume_args("omniroute", ID), None);
}

#[test]
fn take_a_session_id_only_when_it_is_a_plain_token_never_an_option_never_a_shell_word() {
    for good in [ID, "ses_3f1a9c", "my-session.2", "a:b"] {
        assert!(is_resumable_session_id(good), "{good}");
    }
    let long = "x".repeat(201);
    for bad in ["", "-rf", "--resume", "a b", "a;b", "a&b", "a|b", "\"a\"", "%PATH%", "^a", "a\nb", long.as_str()] {
        assert!(!is_resumable_session_id(bad), "{bad}");
        assert_eq!(resume_args("claude", bad), None, "{bad}");
    }
}

// the program a terminal starts

// case: is the Node script, the native binary, or the .cmd launcher itself -- never cmd wrapping cmd
#[test]
fn is_the_node_script_the_native_binary_or_the_cmd_launcher_itself_never_cmd_wrapping_cmd() {
    let (codex, claude, cursor) = (codex(), claude(), cursor());
    assert_eq!(
        terminal_program(&codex, &words(&["resume", "abc"])),
        TerminalProgram { file: codex.executable_path.clone(), args: vec![codex.prefix_args[0].clone(), "resume".into(), "abc".into()] }
    );
    assert_eq!(
        terminal_program(&claude, &words(&["--resume", "abc"])),
        TerminalProgram { file: claude.executable_path.clone(), args: words(&["--resume", "abc"]) }
    );
    assert_eq!(
        terminal_program(&cursor, &words(&["--resume=abc"])),
        TerminalProgram { file: cursor.prefix_args[3].clone(), args: words(&["--resume=abc"]) }
    );
}

#[test]
fn goes_to_windows_terminal_as_a_new_window_titled_for_the_conversation_in_the_teammates_folder() {
    let program = terminal_program(&claude(), &words(&["--resume", "abc"]));
    assert_eq!(
        windows_terminal_args(&program, "C:\\work\\wren", "Wren · Claude Code"),
        Some(words(&["-w", "new", "new-tab", "--title", "Wren · Claude Code", "-d", "C:\\work\\wren", "C:\\npm\\claude.exe", "--resume", "abc"]))
    );
}

#[test]
fn does_not_go_to_windows_terminal_with_a_anywhere_in_it_wt_splits_its_own_commands_there() {
    let program = terminal_program(&claude(), &words(&["--resume", "abc"]));
    assert_eq!(windows_terminal_args(&program, "C:\\a;b", "Wren · Claude Code"), None);
    assert_eq!(windows_terminal_args(&program, "C:\\work", "Wren; Pip"), None);
}

#[test]
fn in_a_console_window_is_one_cmd_line_with_every_path_quoted_the_way_the_sign_in_window_does_it() {
    let (codex, cursor) = (codex(), cursor());
    let resume = words(&["resume", "abc"]);
    assert_eq!(
        console_line(&terminal_program(&codex, &resume), &resume),
        format!("/d /k \"\"{}\" \"{}\" resume abc\"", codex.executable_path, codex.prefix_args[0])
    );
    let resume = words(&["--resume=abc"]);
    assert_eq!(console_line(&terminal_program(&cursor, &resume), &resume), format!("/d /k \"\"{}\" --resume=abc\"", cursor.prefix_args[3]));
}

fn request() -> TerminalRequest {
    TerminalRequest { runtime: "claude".into(), session_id: "abc-123".into(), cwd: "C:\\work\\wren".into(), launch: claude(), title: "Wren · Claude Code".into() }
}

/// Opens `request` on `platform` with a spawn that starts or fails as told, and the calls it was asked.
fn open(request: &TerminalRequest, platform: &str, local: Option<&str>, outcomes: &'static [bool]) -> (Result<&'static str, String>, Vec<TerminalSpawn>) {
    // Electron's `fakeSpawn`: records what it was asked, and starts or fails as told.
    let mut calls = Vec::new();
    let result = open_in_terminal_with(request, platform, local, &mut |ask| {
        let start = outcomes.get(calls.len()).copied().unwrap_or(false);
        calls.push(ask.clone());
        if start { Ok(()) } else { Err("spawn wt.exe ENOENT".to_owned()) }
    });
    (result, calls)
}

// opening it

#[test]
fn starts_windows_terminal_when_it_is_there_detached_and_says_so() {
    let (answer, calls) = open(&request(), "win32", Some("C:\\Users\\J\\AppData\\Local"), &[true]);
    assert_eq!(answer, Ok("Windows Terminal"));
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].file, "C:\\Users\\J\\AppData\\Local\\Microsoft\\WindowsApps\\wt.exe");
    assert!(calls[0].detached);
}

#[test]
fn falls_back_to_a_console_window_of_its_own_when_windows_terminal_will_not_start() {
    let (answer, calls) = open(&request(), "win32", None, &[false, true]);
    assert_eq!(answer, Ok("a console window"));
    assert_eq!(calls[1].file, "cmd.exe");
    assert_eq!(calls[1].cwd.as_deref(), Some("C:\\work\\wren"));
    assert!(calls[1].detached && calls[1].windows_verbatim_arguments && !calls[1].windows_hide);
    assert_eq!(calls[1].args, words(&["/d /k \"\"C:\\npm\\claude.exe\" --resume abc-123\""]));
}

// case: goes straight to the console window for a CLI run under Locust's own Node, with its environment
#[test]
fn goes_straight_to_the_console_window_for_a_cli_run_under_locusts_own_node_with_its_environment() {
    let own_node = TerminalLaunch {
        executable_path: "C:\\Program Files\\Locust\\Locust.exe".into(),
        env: Some(vec![("ELECTRON_RUN_AS_NODE".into(), "1".into())]),
        ..codex()
    };
    let (answer, calls) = open(&TerminalRequest { runtime: "codex".into(), launch: own_node, ..request() }, "win32", None, &[true]);
    assert_eq!(answer, Ok("a console window"));
    assert_eq!(calls[0].file, "cmd.exe");
    assert!(calls[0].env.as_ref().is_some_and(|env| env.contains(&("ELECTRON_RUN_AS_NODE".into(), "1".into()))));
}

// case: starts nothing for a session id that is not a plain token, or on a system with no terminal Locust knows (macOS and Linux: a-mac-opens-its-own-terminal, a-linux-terminal-is-the-one-the-desktop-has)
#[test]
fn starts_nothing_for_a_session_id_that_is_not_a_plain_token_or_on_a_system_with_no_terminal_locust_knows() {
    let (answer, calls) = open(&TerminalRequest { session_id: "a & calc".into(), ..request() }, "win32", None, &[true, true]);
    assert!(answer.is_err());
    assert!(calls.is_empty());
    let (answer, calls) = open(&request(), "freebsd", None, &[true, true]);
    assert!(answer.is_err());
    assert!(calls.is_empty());
}

// which conversation, and where

struct Facts {
    live: Vec<&'static str>,
    mission: Option<TerminalMission>,
    owner: Option<(String, String)>,
    cwd: Option<String>,
    launch: Option<TerminalLaunch>,
}

fn facts() -> Facts {
    Facts {
        live: Vec::new(),
        mission: Some(TerminalMission { runtime: "claude".into(), model: "opus".into(), session: Some("abc-123".into()) }),
        owner: Some(("tm_wren".into(), "Wren".into())),
        cwd: Some("C:\\work\\wren-worktree".into()),
        launch: Some(claude()),
    }
}

fn request_for(f: &Facts) -> Result<TerminalRequest, String> {
    terminal_request_for(
        "m1",
        &TerminalFacts {
            live: &|id| f.live.contains(&id),
            mission: &|_| f.mission.clone(),
            owner_of: &|_| f.owner.clone(),
            cwd_for: &|_| f.cwd.clone(),
            workspace_path: "C:\\work".into(),
            launch_for: &|_| f.launch.clone(),
        },
    )
}

fn mission(runtime: &str, model: &str, session: Option<&str>) -> Option<TerminalMission> {
    Some(TerminalMission { runtime: runtime.into(), model: model.into(), session: session.map(str::to_owned) })
}

// case: is the mission's own session, in its teammate's folder, titled for them
#[test]
fn is_the_missions_own_session_in_its_teammates_folder_titled_for_them() {
    assert_eq!(
        request_for(&facts()),
        Ok(TerminalRequest {
            runtime: "claude".into(),
            session_id: "abc-123".into(),
            cwd: "C:\\work\\wren-worktree".into(),
            launch: claude(),
            title: "Wren · Claude Code".into()
        })
    );
    // Nobody's conversation, or a teammate with no folder of its own: the project folder.
    let nobody = request_for(&Facts { owner: None, ..facts() }).expect("a request");
    assert_eq!((nobody.cwd.as_str(), nobody.title.as_str()), ("C:\\work", "Locust · Claude Code"));
    assert_eq!(request_for(&Facts { cwd: None, ..facts() }).expect("a request").cwd, "C:\\work");
}

#[test]
fn is_refused_while_the_run_is_going_and_says_why() {
    let refused = request_for(&Facts { live: vec!["m1"], ..facts() }).expect_err("refused");
    assert!(refused.contains("still running"), "{refused}");
}

// case: is refused, with the reason, for Antigravity, a model of your own, a turn with no session, and a CLI not found
#[test]
fn is_refused_with_the_reason_for_antigravity_a_model_of_your_own_a_turn_with_no_session_and_a_cli_not_found() {
    let refused = |f: Facts| request_for(&f).expect_err("refused");
    // Through its app (no CLI found): refused. Through Antigravity CLI: opened with agy.
    assert!(refused(Facts { mission: mission("antigravity", "flash", Some("c1")), ..facts() }).contains("Antigravity's own window"));
    let agy = TerminalLaunch { command_name: "agy".into(), ..claude() };
    let opened = request_for(&Facts { mission: mission("antigravity", "gemini-3.8-flash", Some("c1")), launch: Some(agy.clone()), ..facts() })
        .expect("opened with agy");
    assert_eq!((opened.runtime.as_str(), opened.session_id.as_str(), &opened.launch), ("antigravity", "c1", &agy));
    assert!(refused(Facts { mission: mission("opencode", "own-1a2b3c4d/acme-70b", Some("ses_1")), ..facts() }).contains("model of your own"));
    assert!(refused(Facts { mission: mission("claude", "opus", None), ..facts() }).contains("no Claude Code session"));
    assert!(refused(Facts { launch: None, ..facts() }).contains("could not find Claude Code"));
    assert!(refused(Facts { mission: None, ..facts() }).contains("not in the ledger"));
}

// Lane check: Electron's `terminalProgram` unwraps only `cmd.exe /d /s /c <launcher>`; a cmd line with any other
// switch (`/k` keeps the window open, `/q` turns echo off) is not one it may take the launcher out of.
#[test]
fn takes_the_launcher_out_of_cmd_only_for_exactly_d_s_c() {
    let cursor = cursor();
    assert_eq!(&cursor.prefix_args[..3], ["/d", "/s", "/c"]);
    let resume = words(&["--resume=abc"]);
    for (at, other) in [(0, "/q"), (1, "/x"), (2, "/k")] {
        let mut launch = cursor.clone();
        launch.prefix_args[at] = other.into();
        let program = terminal_program(&launch, &resume);
        assert_eq!(program.file, launch.executable_path, "{other} at {at} stays wrapped");
        assert_eq!(program.args, launch.prefix_args.iter().chain(&resume).cloned().collect::<Vec<_>>());
    }
}

#[test]
fn takes_the_launcher_out_of_a_program_named_cmd_exe_and_never_one_that_only_ends_that_way() {
    let mut launch = cursor();
    for executable in ["C:\\Windows\\System32\\CMD.EXE", "cmd.exe", "/usr/bin/cmd.exe"] {
        launch.executable_path = executable.into();
        assert_eq!(terminal_program(&launch, &[]).file, launch.prefix_args[3], "{executable}");
    }
    launch.executable_path = "C:\\tools\\xcmd.exe".into();
    assert_eq!(terminal_program(&launch, &[]).file, launch.executable_path);
}
