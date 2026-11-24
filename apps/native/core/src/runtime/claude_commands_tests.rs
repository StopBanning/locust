//! Electron's `packages/runtime-adapters/test/claude-commands.test.ts`, case by case, through the caller
//! (`runtime_commands::RuntimeCommands::learn_claude`, which reads the list and keeps it as a run's is). Claude Code is a
//! stand-in that writes down its arguments, folder and everything it is sent, prints the case's chunks five
//! milliseconds apart (Electron's fake), and then waits to be stopped.
use super::*;
use crate::runtime_commands::for_store;
use crate::store::Store;
use std::path::PathBuf;

const STAND_IN: &str = r#"
import json, os, sys, threading, time
here = os.path.dirname(os.path.abspath(__file__))
open(os.path.join(here, 'argv.json'), 'w').write(json.dumps(dict(args=sys.argv[1:], cwd=os.getcwd(), pid=os.getpid())))
def hear():
    with open(os.path.join(here, 'written.txt'), 'a') as w:
        for line in sys.stdin:
            w.write(line); w.flush()
threading.Thread(target=hear, daemon=True).start()
for chunk in json.load(open(os.path.join(here, 'chunks.json'))):
    sys.stdout.write(chunk); sys.stdout.flush(); time.sleep(0.005)
time.sleep(60)
"#;

struct Claude {
    dir: PathBuf,
    store: Store,
}
impl Drop for Claude {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
impl Claude {
    fn printing(chunks: &[String]) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-claude-commands-{}", crate::invariants::unique_nanos()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("pebble")).unwrap();
        std::fs::write(dir.join("bin/claude.py"), STAND_IN).unwrap();
        std::fs::write(dir.join("bin/chunks.json"), serde_json::to_string(chunks).unwrap()).unwrap();
        let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
        Self { dir, store }
    }
    fn learn(&self, wait: Wait, on_models: &mut dyn FnMut(Vec<Value>)) -> Result<Vec<CommandInfo>, String> {
        let mut claude = Command::new("python3");
        claude.arg(self.dir.join("bin/claude.py"));
        for_store(&self.store).learn_claude(claude, &self.dir.join("pebble"), wait, on_models)
    }
    fn started(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.dir.join("bin/argv.json")).unwrap()).unwrap()
    }
    fn written(&self) -> Vec<String> {
        // What it heard is written down by its own thread; give that a moment.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let heard = std::fs::read_to_string(self.dir.join("bin/written.txt")).unwrap_or_default();
            if !heard.is_empty() || Instant::now() > deadline {
                return heard.lines().map(str::to_owned).collect();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    /// Whether the stand-in was stopped (it would otherwise sleep a minute).
    fn stopped(&self) -> bool {
        let pid = self.started()["pid"].to_string();
        !Command::new("kill").args(["-0", &pid]).stderr(Stdio::null()).status().unwrap().success()
    }
    fn kept(&self) -> Vec<String> {
        for_store(&self.store).held_for_test("claude").into_iter().map(|c| c.name).collect()
    }
}

fn listed(names: &[&str]) -> String {
    let commands: Vec<Value> = names.iter().map(|n| json!({"name": n, "description": format!("the {n} command"), "argumentHint": ""})).collect();
    format!("{}\n", json!({"type":"system","subtype":"commands_changed","commands":commands}))
}

fn settling(ms: u64) -> Wait {
    Wait { settle: Duration::from_millis(ms), ..Wait::default() }
}

fn names(commands: &[CommandInfo]) -> Vec<&str> {
    commands.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn are_the_last_list_it_announces_read_without_ever_sending_it_a_message_and_it_is_then_stopped() {
    // case: are the last list it announces, read without ever sending it a message, and it is then stopped
    let second = listed(&["compact", "init", "security-review"]);
    let claude = Claude::printing(&[listed(&["compact"]), second[..30].to_owned(), second[30..].to_owned()]);
    let commands = claude.learn(settling(40), &mut |_| {}).unwrap();
    assert_eq!(names(&commands), ["compact", "init", "security-review"]);
    assert_eq!(claude.kept(), ["compact", "init", "security-review"]);
    let written = claude.written();
    assert_eq!(written, [REQUEST]);
    assert_eq!(
        serde_json::from_str::<Value>(&written[0]).unwrap(),
        json!({"type":"control_request","request_id":"locust_commands","request":{"subtype":"initialize"}})
    );
    assert!(claude.stopped());
    assert_eq!(PathBuf::from(claude.started()["cwd"].as_str().unwrap()), claude.dir.join("pebble").canonicalize().unwrap());
}

#[test]
fn hears_the_answer_to_initialize_as_2_1_292_gives_it_nothing_until_asked_then_its_whole_list_0694() {
    // case: hears the answer to initialize, as 2.1.292 gives it: nothing until asked, then its whole list (0.694)
    let answer = json!({"type":"control_response","response":{"subtype":"success","request_id":"locust_commands",
        "response":{"commands":[{"name":"deep-research","description":"Deep research","argumentHint":""},{"name":"review","description":"Review a pull request","argumentHint":"<pr>"}],"agents":[]}}});
    let claude = Claude::printing(&[format!("{answer}\n")]);
    let commands = claude.learn(settling(20), &mut |_| {}).unwrap();
    assert_eq!(names(&commands), ["deep-research", "review"]);
    assert!(claude.stopped());
}

#[test]
fn passes_on_the_models_the_same_answer_lists_as_2_1_293_gives_them_0697() {
    // case: passes on the models the same answer lists, as 2.1.293 gives them (0.697)
    let answer = json!({"type":"control_response","response":{"subtype":"success","request_id":"locust_commands",
        "response":{"commands":[{"name":"review","description":"Review a pull request","argumentHint":"<pr>"}],"models":[
            {"value":"haiku","resolvedModel":"claude-haiku-5-5","displayName":"Haiku 5.5","description":"Fastest for quick answers","supportsEffort":true,"supportedEffortLevels":["low","medium","high","xhigh","max"]},
            {"value":"claude-haiku-4-5-20251001","resolvedModel":"claude-haiku-4-5-20251001","displayName":"Haiku 4.5","description":"Fastest for quick answers"},
            {"value":"","displayName":"nothing to pass"},
            {"value":"opus","displayName":"Opus 5.5","supportsEffort":false,"supportedEffortLevels":["low"]},
        ]}}});
    let claude = Claude::printing(&[format!("{answer}\n")]);
    let mut told = vec![];
    let commands = claude.learn(settling(20), &mut |models| told.push(models)).unwrap();
    assert_eq!(names(&commands), ["review"]);
    assert_eq!(
        told,
        [vec![
            json!({"value":"haiku","resolvedModel":"claude-haiku-5-5","displayName":"Haiku 5.5","efforts":["low","medium","high","xhigh","max"]}),
            json!({"value":"claude-haiku-4-5-20251001","resolvedModel":"claude-haiku-4-5-20251001","displayName":"Haiku 4.5"}),
            json!({"value":"opus","resolvedModel":"opus","displayName":"Opus 5.5"}),
        ]]
    );
}

#[test]
fn is_started_as_every_run_outside_auto_is_restricted_and_leaves_no_session_behind() {
    // case: is started as every run outside Auto is, restricted, and leaves no session behind
    let claude = Claude::printing(&[listed(&["review"])]);
    claude.learn(settling(20), &mut |_| {}).unwrap();
    let args: Vec<String> = serde_json::from_value(claude.started()["args"].clone()).unwrap();
    assert_eq!(
        args,
        ["--restricted", "--print", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--no-session-persistence"]
    );
    assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
}

#[test]
fn is_a_refusal_not_an_empty_menu_when_no_list_comes_and_claude_code_is_still_stopped() {
    // case: is a refusal, not an empty menu, when no list comes, and Claude Code is still stopped
    let claude = Claude::printing(&[format!("{}\n", json!({"type":"system","subtype":"init"}))]);
    let refused = claude.learn(Wait { timeout: Duration::from_millis(600), ..Wait::default() }, &mut |_| {}).unwrap_err();
    assert!(refused.contains("did not list"), "{refused}");
    assert!(claude.stopped());
    assert!(claude.kept().is_empty());
}
