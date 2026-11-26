//! Electron's `packages/runtime-adapters/test/opencode-commands.test.ts`, its list cases ("OpenCode's own commands,
//! listed"), through the caller: `RuntimeCommands::learn_opencode` on the command the app builds
//! (`command_for_version`, in the teammate's folder), OpenCode 2 known by that command (`uses_acp`). The program is a
//! stand-in for `opencode serve`, in Python like Electron's `fakeServer`: it logs how it was started (`spawn.json`)
//! and each request (`calls.jsonl`), and answers `GET /command` as each test says. Electron's `state.killed` is the
//! stand-in's process being gone once the list is read. The run cases are in opencode_serve_run_case_tests.rs.
use super::*;
use crate::approval::Mode;
use crate::runtime::opencode::{command_for_version, uses_acp};
use crate::store::Store;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const SERVER: &str = r#"#!/usr/bin/env python3
import json, os, sys
from http.server import BaseHTTPRequestHandler, HTTPServer
HERE = os.path.dirname(os.path.abspath(__file__))
S = json.load(open(os.path.join(HERE, 'script.json')))
json.dump({'argv': sys.argv[1:], 'cwd': os.getcwd(), 'env': dict(os.environ)}, open(os.path.join(HERE, 'spawn.json'), 'w'))
open(os.path.join(HERE, 'pid'), 'w').write(str(os.getpid()))

class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass
    def do_GET(self):
        with open(os.path.join(HERE, 'calls.jsonl'), 'a') as f:
            f.write(json.dumps({'path': self.path, 'auth': self.headers.get('Authorization')}) + '\n')
        data = json.dumps(S.get('json')).encode() if 'json' in S else b''
        self.send_response(S.get('http', 200))
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

httpd = HTTPServer(('127.0.0.1', 0), H)
print('opencode server listening on http://127.0.0.1:%d' % httpd.server_address[1], flush=True)
httpd.serve_forever()
"#;

/// 1.18.27's `GET /command`, as Electron's LISTED.
fn listed() -> Value {
    json!([
        {"name": "init", "description": "guided AGENTS.md setup", "source": "command", "template": "Create or update AGENTS.md ... $ARGUMENTS", "hints": ["$ARGUMENTS"]},
        {"name": "review", "description": "review changes [commit|branch|pr], defaults to uncommitted", "source": "command", "template": "You are a code reviewer ... $ARGUMENTS", "hints": ["$ARGUMENTS"]},
        {"name": "deploy", "description": "Ship it", "source": "command", "template": "Deploy $1 to $2", "hints": ["$1", "$2"]},
        {"name": "a-skill", "description": "A skill", "source": "skill", "template": "..."},
        {"name": "not a name", "description": "x", "source": "command", "template": "x"},
    ])
}

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Electron's `fakeServer(answer)`: the stand-in in `bin/`, the teammate's folder `pebble/`, and the app's store.
struct Fake {
    t: Temp,
    store: Store,
}
impl Fake {
    fn new(answer: Value) -> Self {
        let t = Temp(std::env::temp_dir().join(format!("locust-oc-commands-{}", crate::invariants::unique_nanos())));
        for dir in ["bin", "pebble"] {
            std::fs::create_dir_all(t.0.join(dir)).unwrap();
        }
        let exe = t.0.join("bin/opencode");
        std::fs::write(&exe, SERVER).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(t.0.join("bin/script.json"), answer.to_string()).unwrap();
        let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
        std::fs::create_dir_all(store.electron()).unwrap();
        Self { t, store }
    }
    fn folder(&self) -> PathBuf {
        self.t.0.join("pebble")
    }
    /// `createOpenCodeServeCommand(EXECUTABLE, { workspacePath })` for an installed `version`.
    fn learn(&self, version: &str) -> Result<Vec<CommandInfo>, String> {
        let command = command_for_version(self.t.0.join("bin/opencode"), version, &self.folder(), Mode::ApproveEach, &self.store, false).unwrap();
        let v2 = uses_acp(&command);
        crate::runtime_commands::for_store(&self.store).learn_opencode(command, v2)
    }
    fn spawned(&self) -> Option<Value> {
        std::fs::read_to_string(self.t.0.join("bin/spawn.json")).ok().map(|s| serde_json::from_str(&s).unwrap())
    }
    fn calls(&self) -> Vec<Value> {
        std::fs::read_to_string(self.t.0.join("bin/calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    /// `server.state.killed`.
    fn assert_killed(&self) {
        let pid = std::fs::read_to_string(self.t.0.join("bin/pid")).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while Path::new(&format!("/proc/{}", pid.trim())).exists() {
            assert!(Instant::now() < until, "the server was left running");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// JavaScript's `encodeURIComponent`, as Electron's expected URL is written.
fn encode_uri_component(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[test]
fn never_starts_the_v1_command_server_when_opencode_2_is_installed() {
    // case: never starts the V1 command server when OpenCode 2 is installed
    let fake = Fake::new(json!({"json": listed()}));
    assert_eq!(fake.learn("2.1.1"), Ok(vec![]));
    assert!(fake.spawned().is_none(), "a V1 server must not start for V2");
    assert!(fake.calls().is_empty());
}

#[test]
fn are_read_from_a_server_started_in_the_folder_with_a_password_and_no_plugins_which_is_then_stopped() {
    // case: are read from a server started in the folder, with a password and no plugins, which is then stopped
    let fake = Fake::new(json!({"json": listed()}));
    let commands = fake.learn("1.18.27").unwrap();
    let info = |name: &str, description: &str, hint: &str| CommandInfo {
        name: name.into(),
        description: description.into(),
        argument_hint: hint.into(),
    };
    let expected = vec![
        info("init", "guided AGENTS.md setup", "[arguments]"),
        info("review", "review changes [commit|branch|pr], defaults to uncommitted", "[arguments]"),
        info("deploy", "Ship it", "$1 $2"),
        info("a-skill", "A skill", ""),
    ];
    assert_eq!(commands, expected);
    // Kept as the `/` menu reads them.
    assert_eq!(crate::runtime_commands::for_store(&fake.store).list().get("opencode"), Some(&expected));
    let spawned = fake.spawned().unwrap();
    let cwd = PathBuf::from(spawned["cwd"].as_str().unwrap());
    assert_eq!(cwd.canonicalize().unwrap(), fake.folder().canonicalize().unwrap());
    let folder = fake.folder().to_string_lossy().into_owned();
    let paths: Vec<String> = fake.calls().iter().map(|c| c["path"].as_str().unwrap().to_owned()).collect();
    assert_eq!(paths, [format!("/command?directory={}", encode_uri_component(&folder))]);
    assert_eq!(spawned["env"]["OPENCODE_PURE"], "1");
    assert!(spawned["env"]["OPENCODE_SERVER_PASSWORD"].as_str().unwrap().len() > 20);
    assert!(fake.calls()[0]["auth"].as_str().unwrap().starts_with("Basic "));
    fake.assert_killed();
}

#[test]
fn is_a_refusal_not_an_empty_menu_when_the_server_will_not_say_and_the_server_is_still_stopped() {
    // case: is a refusal, not an empty menu, when the server will not say, and the server is still stopped
    let fake = Fake::new(json!({"http": 401}));
    let refused = fake.learn("1.18.27").unwrap_err();
    assert!(refused.contains("401"), "{refused}");
    fake.assert_killed();
}
