//! packages/runtime-adapters/test/a-run-is-handed-its-own-skills.test.ts, case for case.
//!
//! The Codex cases run the real driver against a fake app-server (python3) that logs what it is asked, in order. The
//! others build the argv and the OpenCode config the way `native::command_for_turn` does, without an agent installed.
//! Electron's run and serve routes for OpenCode 1 are one route here (native always runs `opencode serve`), so "both
//! routes" is the one config per mode, Approve each's among them. Every native Copilot run is given Locust's command
//! guard after its argv is built (`native.rs`), so "nowhere when there is none" reads: no skills plugin, the guard's
//! alone.
use super::{
    Runtime, Turn,
    codex::Codex,
    opencode_config::{legacy_config, with_references, with_skills},
};
use crate::approval::Mode;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("locust-run-skills-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Forget(String);
impl Drop for Forget {
    fn drop(&mut self) {
        crate::skills::forget_roots(&self.0);
    }
}

/// A Codex app-server that logs each method as it arrives (and `answered:<method>` as it answers), answering
/// `skills/extraRoots/set` as `answer` says: "ok", or "refuse" with Method not found.
const FAKE_CODEX: &str = r#"import json, sys, time
log_path, answer = sys.argv[1], sys.argv[2]
log = open(log_path, "a")
def note(text):
    log.write(text + "\n"); log.flush()
def send(v):
    sys.stdout.write(json.dumps(dict(jsonrpc="2.0", **v)) + "\n")
    sys.stdout.flush()
def notify(method, **params):
    send(dict(method=method, params=dict(threadId="th_1", **params)))
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    m = msg.get("method")
    rid = msg.get("id")
    note(m + " " + json.dumps(msg.get("params")))
    if m == "initialize":
        send(dict(id=rid, result={"userAgent": "codex"}))
    elif m == "skills/extraRoots/set":
        # Slow, so a thread started without waiting for the answer would be logged first.
        time.sleep(0.3)
        note("answered:" + m)
        if answer == "refuse":
            send(dict(id=rid, error={"code": -32601, "message": "Method not found"}))
        else:
            send(dict(id=rid, result={}))
    elif m == "thread/start":
        send(dict(id=rid, result={"thread": {"id": "th_1"}}))
    elif m == "turn/start":
        send(dict(id=rid, result={"turn": {"id": "tu_1"}}))
        notify("turn/started", turn={"id": "tu_1"})
        notify("item/completed", item={"id": "it_1", "type": "agentMessage", "text": "ok"})
        notify("turn/completed", turn={"id": "tu_1", "status": "completed", "usage": {"input_tokens": 1, "output_tokens": 1}})
"#;

/// Runs one Codex turn against the fake and returns what it was asked, in order.
fn codex_run(roots: Option<Vec<String>>, answer: &str) -> Vec<String> {
    let temp = Temp::new();
    let mission = format!("run_skills_{}", crate::invariants::unique_nanos());
    let _forget = Forget(mission.clone());
    if let Some(roots) = roots {
        crate::skills::set_roots(&mission, roots);
    }
    let (script, log) = (temp.0.join("fake_codex.py"), temp.0.join("asked.log"));
    std::fs::write(&script, FAKE_CODEX).unwrap();
    let mut command = Command::new("python3");
    command.arg(&script).arg(&log).arg(answer);
    let mut runtime = Codex::spawn(&mut command).unwrap();
    runtime
        .start(&Turn {
            prompt: "use the skill".into(),
            folder: temp.0.clone(),
            model: None,
            effort: None,
            resume: None,
            mode: Mode::AcceptEdits,
            approvals: None,
            mission_id: mission.clone(),
            run_id: format!("run_{mission}"),
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "the fake Codex run did not finish");
        if let Some(e) = runtime.next_event().unwrap()
            && ["run.completed", "run.failed", "run.cancelled"].contains(&e.kind.as_str())
        {
            assert_eq!(e.kind, "run.completed");
            break;
        }
    }
    std::fs::read_to_string(&log).unwrap().lines().map(str::to_owned).collect()
}

fn position(asked: &[String], method: &str) -> Option<usize> {
    asked.iter().position(|l| l == method || l.starts_with(&format!("{method} ")))
}

#[test]
fn names_the_runs_skill_folder_after_the_handshake_and_before_the_thread_starts() {
    let asked = codex_run(Some(vec!["/profile/run-skills/abc/skills".into()]), "ok");
    let set = position(&asked, "skills/extraRoots/set").expect("the skills folder was named");
    let params: Value = serde_json::from_str(asked[set].split_once(' ').unwrap().1).unwrap();
    assert_eq!(params, json!({"extraRoots": ["/profile/run-skills/abc/skills"]}));
    assert!(position(&asked, "initialized").unwrap() < set, "{asked:?}");
    // The thread waits for the answer, so its first turn already lists them.
    let answered = position(&asked, "answered:skills/extraRoots/set").unwrap();
    assert!(answered < position(&asked, "thread/start").unwrap(), "{asked:?}");
}

#[test]
fn runs_the_turn_without_them_when_the_server_does_not_know_the_request() {
    let asked = codex_run(Some(vec!["/profile/run-skills/abc/skills".into()]), "refuse");
    assert!(position(&asked, "skills/extraRoots/set").is_some());
    assert!(position(&asked, "thread/start").is_some(), "{asked:?}");
    assert!(position(&asked, "turn/start").is_some(), "{asked:?}");
}

#[test]
fn says_nothing_about_skills_to_a_run_that_has_none() {
    let asked = codex_run(None, "ok");
    assert_eq!(position(&asked, "skills/extraRoots/set"), None, "{asked:?}");
    assert!(position(&asked, "thread/start").is_some());
}

/// The run's OpenCode 1 config for a mode, as `command_for_turn` builds it.
fn opencode(mode: Mode, references: &[&str], skills: Option<&str>, person: &[String]) -> Value {
    with_skills(with_references(legacy_config(mode, Some("file:///guard/index.js")), mode, references), skills, person)
}

const MODES: [Mode; 3] = [Mode::Ask, Mode::AcceptEdits, Mode::Auto];

#[test]
fn in_its_per_run_configs_skills_paths_in_every_mode_and_on_both_routes_beside_the_rest_of_that_config() {
    for mode in MODES.into_iter().chain([Mode::ApproveEach]) {
        let config = opencode(mode, &[], Some("/profile/run-skills/abc/skills"), &[]);
        assert_eq!(config["skills"], json!({"paths": ["/profile/run-skills/abc/skills"]}), "{mode:?}");
        // The mode's own config is still there.
        assert!(config["permission"].is_object() && config["plugin"].is_array(), "{mode:?}");
    }
}

#[test]
fn never_as_a_config_folder_opencode_would_install_its_plugin_package_into_it_before_every_run() {
    let temp = Temp::new();
    let store = crate::store::Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    for mode in MODES.into_iter().chain([Mode::ApproveEach]) {
        let mut c = Command::new("opencode");
        super::opencode_config::configure(&mut c, mode, &store, false).unwrap();
        let envs: Vec<(String, Option<String>)> =
            c.get_envs().map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))).collect();
        assert!(!envs.iter().any(|(k, _)| k == "OPENCODE_CONFIG_DIR"), "{mode:?}: {envs:?}");
    }
    assert!(opencode(Mode::AcceptEdits, &[], None, &[]).get("skills").is_none());
    assert!(opencode(Mode::AcceptEdits, &[], None, &["/personal".into()]).get("skills").is_none(), "no folder, nothing carried");
}

#[test]
fn adds_to_configured_paths_on_both_routes_and_preserves_the_skill_urls() {
    let mut config = json!({"skills": {"paths": ["/existing"], "urls": ["https://example.com/skills/"]}, "permission": {"external_directory": "deny", "edit": "deny", "bash": "ask"}});
    config = with_skills(config, Some("/run/skills"), &["/personal".into(), "/existing".into()]);
    assert_eq!(config["skills"], json!({"paths": ["/personal", "/existing", "/run/skills"], "urls": ["https://example.com/skills/"]}));
    for mode in [Mode::Ask, Mode::ApproveEach] {
        assert_eq!(opencode(mode, &[], Some("/run/skills"), &["/personal".into()])["skills"]["paths"], json!(["/personal", "/run/skills"]));
    }
}

#[test]
fn allows_only_the_runs_skills_subtree_and_leaves_every_other_mode_rule_in_place() {
    for mode in MODES {
        let mut before = opencode(mode, &["C:/reference"], None, &[]);
        let mut after = opencode(mode, &["C:/reference"], Some("C:/profile/run-skills/abc/skills"), &[]);
        let old_external = before["permission"].as_object_mut().unwrap().remove("external_directory").unwrap();
        let external = after["permission"].as_object_mut().unwrap().remove("external_directory").unwrap();
        assert_eq!(after["permission"], before["permission"], "{mode:?}");
        if mode == Mode::Auto {
            assert_eq!(external, "allow");
            continue;
        }
        assert_eq!(external["*"], "deny");
        assert_eq!(external["C:/profile/run-skills/abc/skills/**"], "allow");
        assert_eq!(external["C:\\profile\\run-skills\\abc\\skills\\**"], "allow");
        assert!(external.get("C:/profile/**").is_none());
        for (k, v) in old_external.as_object().into_iter().flatten() {
            assert_eq!(&external[k], v, "{mode:?}: {k}");
        }
    }
    let asking = opencode(Mode::ApproveEach, &[], Some("/profile/run/skills"), &[]);
    assert_eq!(asking["permission"]["edit"], "ask");
    assert_eq!(asking["permission"]["bash"], "ask");
    assert_eq!(asking["permission"]["external_directory"]["*"], "ask");
    assert_eq!(asking["permission"]["external_directory"]["/profile/run/skills/**"], "allow");
}

fn after_flag(args: &[String], flag: &str) -> Vec<String> {
    args.iter().enumerate().filter(|(_, a)| *a == flag).filter_map(|(i, _)| args.get(i + 1).cloned()).collect()
}

const SKILLS: &str = "/locust-run-skills-abc";

#[test]
fn names_the_runs_plugin_on_the_print_route_and_the_acp_route_beside_the_command_guard_and_nowhere_when_there_is_none() {
    // native.rs puts the guard's --plugin-dir after the argv; the skills plugin must come first on either route.
    let guard = |mut args: Vec<String>| {
        args.extend(["--plugin-dir".into(), "/guard".into()]);
        args
    };
    for mode in MODES {
        let print = guard(super::copilot::prompt_args("go", mode, None, None, None, None, &[], Some(SKILLS)));
        assert_eq!(after_flag(&print, "--plugin-dir"), [SKILLS, "/guard"], "{mode:?}");
        let none = guard(super::copilot::prompt_args("go", mode, None, None, None, None, &[], None));
        assert_eq!(after_flag(&none, "--plugin-dir"), ["/guard"], "{mode:?}");
    }
    let acp = |skills: Option<&str>| -> Vec<String> {
        let c = super::copilot::command(Path::new("/work"), "go", Mode::ApproveEach, None, None, None, None, &[], skills);
        c.map(|c| c.get_args().map(|a| a.to_string_lossy().into_owned()).collect()).unwrap_or_default()
    };
    // The ACP route needs the copilot binary to build its command; without one there is nothing to look at.
    if super::discovery::find_named("copilot").is_ok() {
        assert_eq!(after_flag(&guard(acp(Some(SKILLS))), "--plugin-dir"), [SKILLS, "/guard"]);
        assert_eq!(after_flag(&guard(acp(None)), "--plugin-dir"), ["/guard"]);
    }
}

#[test]
fn adds_the_runs_folder_on_every_mode_as_an_absolute_add_dir_and_still_only_lends_a_reference_folder_to_a_read_only_run() {
    for mode in MODES {
        let args = super::antigravity::print_args(mode, None, None, None, &[], Some(SKILLS));
        assert_eq!(after_flag(&args, "--add-dir"), [SKILLS], "{mode:?}");
    }
    let reading = super::antigravity::print_args(Mode::Ask, None, None, None, &["/refs"], Some(SKILLS));
    assert_eq!(after_flag(&reading, "--add-dir"), [SKILLS, "/refs"]);
    let writing = super::antigravity::print_args(Mode::AcceptEdits, None, None, None, &["/refs"], Some(SKILLS));
    assert_eq!(after_flag(&writing, "--add-dir"), [SKILLS]);
    assert!(!super::antigravity::print_args(Mode::AcceptEdits, None, None, None, &[], None).contains(&"--add-dir".to_owned()));
    let relative = super::antigravity::command(Path::new("/work"), Mode::AcceptEdits, None, None, None, &[], Some("skills"));
    assert!(relative.is_err_and(|e| e.to_string().contains("absolute")));
}
