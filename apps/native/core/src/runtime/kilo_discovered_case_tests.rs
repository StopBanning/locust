//! Electron's `packages/runtime-adapters/test/kilo-is-discovered-and-starts-on-auto-free.test.ts`, case by case.
//! Discovery through `agents::discover_one("kilo", ..)` with Electron's runner answers; the run through the command
//! the app builds (`command_at`, which `command` calls with the found `kilo` and this process's PATH) and `Kilo`.
//!
//! The native app runs on macOS and Linux, where the shim is a `git` shell script run with stdin from /dev/null:
//! Electron's Windows-only choice between `git.exe` and `git.cmd` has nothing to carry here, and its `git.cmd` case is
//! carried as the same promise for the `git` shim (first on the child's PATH, this process's PATH left alone).
use super::*;
use crate::agents::{Answer, Found, Probe, Seams, discover_one};
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;

const ACP_HELP_TEXT: &str = "kilo acp: Agent Client Protocol over stdin and stdout";
const INSTALLED: &str = "/tools/kilo";

fn ok(stdout: &str) -> Answer {
    Answer { exit: Some(0), stdout: stdout.into(), stderr: String::new(), timed_out: false }
}

/// Electron's `discover(version, help)`: every call's arguments, and every file whose presence was asked.
fn discover(version: &str, help: Answer) -> (Found, Vec<Vec<String>>, Vec<PathBuf>) {
    let calls = Mutex::new(Vec::new());
    let looked = Mutex::new(Vec::new());
    let find = |c: &str| (c == "kilo").then(|| PathBuf::from(INSTALLED));
    let run = |_: &Path, args: &[&str]| {
        crate::invariants::lock(&calls).push(args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
        Some(if args == ["--version"] {
            ok(version)
        } else if args.contains(&"acp") {
            help.clone()
        } else {
            ok("")
        })
    };
    let exists = |p: &Path| {
        crate::invariants::lock(&looked).push(p.to_path_buf());
        true
    };
    let seams = Seams { find: &find, run: &run, exists: &exists, home: Some("/home/someone".into()), xdg_config_home: None, recall: None };
    let found = discover_one("kilo", &seams).unwrap();
    (found, calls.into_inner().unwrap(), looked.into_inner().unwrap())
}

#[test]
fn is_ready_from_the_version_and_acp_help_and_does_not_list_models_or_look_for_a_credential_file() {
    // case: is ready from the version and acp help, and does not list models or look for a credential file
    let (found, calls, looked) = discover("7.8.17", ok(ACP_HELP_TEXT));
    assert_eq!(found.id, "kilo");
    assert!(found.installed);
    assert_eq!((found.status, found.ready), (Probe::Ready, true), "{found:?}");
    assert_eq!(found.version.as_deref(), Some("7.8.17"));
    // The two are asked together natively; Electron's order is theirs.
    let mut asked = calls;
    asked.sort();
    assert_eq!(asked, vec![vec!["--version".to_owned()], vec!["acp".to_owned(), "--help".to_owned()]]);
    assert!(looked.is_empty(), "{looked:?}");
}

#[test]
fn is_not_ready_when_acp_help_does_not_name_the_protocol() {
    // case: is not ready when acp help does not name the protocol
    let (found, _, _) = discover("7.8.17", ok("usage: kilo acp"));
    // Electron's "unsupported" for a required feature its help did not show.
    assert_eq!((found.status, found.ready), (Probe::ProbeFailed, false));
}

#[test]
fn is_not_installed_when_the_command_is_absent() {
    // case: is not installed when the command is absent
    let find = |_: &str| None;
    let run = |_: &Path, _: &[&str]| Some(ok("7.8.17"));
    let exists = |_: &Path| true;
    let seams = Seams { find: &find, run: &run, exists: &exists, home: None, xdg_config_home: None, recall: None };
    let found = discover_one("kilo", &seams).unwrap();
    assert!(!found.installed);
    assert!(!found.ready);
    assert_eq!(found.version, None);
}

#[test]
fn never_keeps_the_session_default() {
    // case: never keeps the session default
    assert_eq!(model_for_run(None), AUTO_FREE_MODEL);
    assert_eq!(model_for_run(Some("account-default")), AUTO_FREE_MODEL);
    assert_eq!(model_for_run(Some("  ")), AUTO_FREE_MODEL);
    assert_eq!(model_for_run(Some("kilo/google/gemini-3-pro-image")), "kilo/google/gemini-3-pro-image");
    assert_eq!(AUTO_FREE_MODEL, "kilo/kilo-auto/free");
}

#[test]
fn maps_ask_and_plan_onto_ask_and_edit_and_auto_onto_code() {
    // case: maps Ask and Plan onto ask, and Edit and Auto onto code
    assert_eq!(session_mode(Mode::Ask), "ask");
    assert_eq!(session_mode(Mode::Plan), "ask");
    assert_eq!(session_mode(Mode::AcceptEdits), "code");
    assert_eq!(session_mode(Mode::Auto), "code");
}

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A store, a work folder, a run's own Kilo folder and a `git` in `gitbin/` (or in `gitdir`, when it is named).
struct Place {
    t: Temp,
    store: Store,
}
impl Place {
    fn new() -> Self {
        let t = Temp(std::env::temp_dir().join(format!("locust-kilo-case-{}", crate::invariants::unique_nanos())));
        for dir in ["bin", "scratch", "kilo-config", "gitbin"] {
            std::fs::create_dir_all(t.0.join(dir)).unwrap();
        }
        std::fs::write(t.0.join("gitbin/git"), "").unwrap();
        let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
        std::fs::create_dir_all(store.electron()).unwrap();
        Self { t, store }
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.t.0.join(rel)
    }
    /// What the app builds for a turn, with `path_env` as this process's PATH.
    fn command(&self, kilo: PathBuf, path_env: &std::ffi::OsStr) -> Command {
        command_at(kilo, &self.path("scratch"), &self.store, &self.path("kilo-config"), path_env).unwrap()
    }
}

fn env_of(command: &Command, key: &str) -> Option<String> {
    command
        .get_envs()
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
}

#[test]
fn launches_kilo_acp_and_keeps_a_per_run_environment_on_the_command() {
    // case: launches kilo acp and keeps a per-run environment on the command
    let place = Place::new();
    let command = place.command(PathBuf::from(INSTALLED), place.path("gitbin").as_os_str());
    assert_eq!(command.get_program(), INSTALLED);
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["acp"]);
    assert_eq!(command.get_current_dir(), Some(place.path("scratch").as_path()));
    assert_eq!(env_of(&command, "KILO_CONFIG_DIR"), Some(place.path("kilo-config").to_string_lossy().into_owned()));
}

#[test]
fn puts_git_cmd_first_on_the_child_path_and_leaves_the_process_path_alone() {
    // case: puts git.cmd first on the child PATH and leaves the process PATH alone
    let before = std::env::var_os("PATH");
    let place = Place::new();
    let given = format!("{}:/usr/bin", place.path("gitbin").display());
    let command = place.command(PathBuf::from(INSTALLED), given.as_ref());
    assert_eq!(env_of(&command, "KILO_CONFIG_DIR"), Some(place.path("kilo-config").to_string_lossy().into_owned()));
    let path = env_of(&command, "PATH").unwrap();
    let (shim, rest) = path.split_once(':').unwrap();
    assert_eq!(rest, given);
    assert!(Path::new(shim).join("git").is_file());
    assert_ne!(shim, place.path("gitbin").to_string_lossy());
    assert_eq!(std::env::var_os("PATH"), before);
}

#[test]
fn does_not_write_a_shim_whose_path_could_be_read_as_a_command() {
    // case: does not write a shim whose path could be read as a command
    // Electron's `git&.exe` is a Windows shell's command; a `$` is the shell script's.
    let place = Place::new();
    let unsafe_dir = place.path("git$bin");
    std::fs::create_dir_all(&unsafe_dir).unwrap();
    std::fs::write(unsafe_dir.join("git"), "").unwrap();
    let command = place.command(PathBuf::from(INSTALLED), unsafe_dir.as_os_str());
    assert_eq!(env_of(&command, "PATH"), None);
    assert_eq!(env_of(&command, "KILO_CONFIG_DIR"), Some(place.path("kilo-config").to_string_lossy().into_owned()));
    // And it is not swapped for a later git on that PATH: the shim would run a git PATH does not.
    let both = format!("{}:{}", unsafe_dir.display(), place.path("gitbin").display());
    assert_eq!(env_of(&place.command(PathBuf::from(INSTALLED), both.as_ref()), "PATH"), None);
    let root = place.store.ledger_dir().unwrap().parent().unwrap().to_path_buf();
    // The profile folder a shim would go in may not even exist.
    let shims: Vec<_> = std::fs::read_dir(root)
        .map(|entries| entries.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("kilo-git-")).collect())
        .unwrap_or_default();
    assert!(shims.is_empty(), "no shim is written");
}

#[test]
fn on_another_platform_redirects_stdin_from_dev_null() {
    // case: on another platform redirects stdin from /dev/null
    let place = Place::new();
    let given = format!("{}:/usr/bin", place.path("gitbin").display());
    let command = place.command(PathBuf::from(INSTALLED), given.as_ref());
    let path = env_of(&command, "PATH").unwrap();
    let shim = path.split_once(':').unwrap().0;
    assert_eq!(path, format!("{shim}:{given}"));
    let git = place.path("gitbin/git");
    assert_eq!(
        std::fs::read_to_string(Path::new(shim).join("git")).unwrap(),
        format!("#!/bin/sh\nexec \"{}\" \"$@\" </dev/null\n", git.display())
    );
}

/// A Kilo ACP peer that keeps every line written to it, as written, and answers as Electron's spawn stand-in does.
const PEER: &str = r#"#!/usr/bin/env python3
import json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
def send(v):
    print(json.dumps(dict(jsonrpc='2.0', **v)), flush=True)
options = [dict(id='model', currentValue='kilo/google/gemini-3-pro-image'), dict(id='mode', currentValue='code')]
while True:
    line = sys.stdin.readline()
    if line == '':
        break
    with open(os.path.join(HERE, 'lines.jsonl'), 'a') as log:
        log.write(json.dumps(line) + '\n')
    if not line.startswith('{'):
        continue
    v = json.loads(line)
    method = v.get('method')
    if method == 'initialize':
        # What a run's agent says here is kept for Kilo in the app's data; acp_capabilities_tests keeps the same.
        result = dict(protocolVersion=1, agentCapabilities=dict(loadSession=True, promptCapabilities=dict(image=True, audio=False, embeddedContext=True), mcpCapabilities=dict(http=True, sse=False)))
    elif method in ('session/new', 'session/load'):
        result = dict(sessionId='s1', configOptions=options)
    elif method == 'session/set_config_option':
        for row in options:
            if row['id'] == v['params']['configId']:
                row['currentValue'] = v['params']['value']
        result = dict(configOptions=options)
    elif method == 'session/prompt':
        result = dict(stopReason='end_turn')
    elif 'id' in v:
        result = {}
    else:
        continue
    send(dict(id=v['id'], result=result))
"#;

#[test]
fn does_not_write_a_blank_line_after_the_prompt() {
    // case: does not write a blank line after the prompt
    let place = Place::new();
    let kilo = place.path("bin/kilo");
    std::fs::write(&kilo, PEER).unwrap();
    std::fs::set_permissions(&kilo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let before = std::env::var_os("PATH");
    // The peer's own python3 is found on the PATH the child is given.
    let given = format!("{}:{}", place.path("gitbin").display(), before.as_deref().unwrap_or_default().to_string_lossy());
    let mut command = place.command(kilo, given.as_ref());
    let mut rt = Kilo::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Reply with pebble.".into(),
        folder: place.path("scratch"),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Auto,
        approvals: None,
        mission_id: "kilo_blank".into(),
        run_id: "run_kilo_blank".into(),
    };
    rt.start(&turn).unwrap();
    let until = Instant::now() + Duration::from_secs(15);
    let ended = loop {
        assert!(Instant::now() < until, "the run did not end");
        if let Some(e) = rt.next_event().unwrap()
            && matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled")
        {
            break e;
        }
    };
    assert_eq!(ended.kind, "run.completed", "{:?}", ended.payload);
    // Electron's 750 ms after the prompt, while the peer is still there to read.
    std::thread::sleep(Duration::from_millis(750));
    let lines: Vec<String> = std::fs::read_to_string(place.path("bin/lines.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(lines.iter().any(|l| l.contains("session/prompt")), "{lines:?}");
    assert!(lines.iter().all(|l| l.trim() != ""), "a blank line was written: {lines:?}");
    rt.stop().unwrap();
    assert_eq!(std::env::var_os("PATH"), before);
}
