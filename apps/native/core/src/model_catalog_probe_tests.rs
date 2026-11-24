//! Electron's `main/model-catalog.test.ts`, "the catalog probe": the cases that start a server. Driven through the
//! app's own Codex probe (`codex_session`, what `codex_model_list` runs for `read_listed_checked`) and the app's own
//! cache (`kept_catalog`, what `local()` keeps the catalog in), against a fake app server written here. The fake also
//! starts a helper of its own, the way a `codex` launcher starts the real server, so "shut down" means all of it.
use super::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const FAKE_SERVER: &str = r#"
import json, os, subprocess, sys
with open(os.environ["FAKE_STARTS"], "a") as f:
    f.write("start\n")
helper = subprocess.Popen(["sleep", "60"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
with open(os.environ["FAKE_HELPER"], "a") as f:
    f.write("%d\n" % helper.pid)
result = json.loads(os.environ["FAKE_RESULT"])
for line in sys.stdin:
    message = json.loads(line)
    if "id" not in message:
        continue
    payload = result if message.get("method") == "model/list" else {}
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": payload}) + "\n")
    sys.stdout.flush()
"#;

struct Fake {
    dir: PathBuf,
    result: String,
}

impl Drop for Fake {
    fn drop(&mut self) {
        // Never leave a helper behind, whatever the test found.
        for pid in self.helpers() {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Fake {
    fn new(result: Value) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-catalog-probe-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("server.py"), FAKE_SERVER).unwrap();
        Fake { dir, result: result.to_string() }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("python3");
        command
            .arg(self.dir.join("server.py"))
            .env("FAKE_STARTS", self.dir.join("starts"))
            .env("FAKE_HELPER", self.dir.join("helpers"))
            .env("FAKE_RESULT", &self.result);
        command
    }

    /// How many servers were started.
    fn starts(&self) -> usize {
        std::fs::read_to_string(self.dir.join("starts")).unwrap_or_default().lines().count()
    }

    fn helpers(&self) -> Vec<u32> {
        std::fs::read_to_string(self.dir.join("helpers")).unwrap_or_default().lines().filter_map(|l| l.trim().parse().ok()).collect()
    }

    /// One catalog read the way the app reads Codex's part of it: the probe's models in `Listed`, then the catalog.
    fn read(&self) -> (Listed, bool) {
        let (codex, codex_usage) = codex_session(self.command(), Duration::from_secs(20)).map_or((None, None), |(m, u)| (Some(m), u));
        (Listed { codex, codex_usage, ..Default::default() }, false)
    }
}

/// Whether a process is still running (a zombie waiting to be reaped has stopped).
fn running(pid: u32) -> bool {
    match std::fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat")) {
        Ok(stat) => stat.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().next()).is_some_and(|state| state != "Z" && state != "X"),
        Err(_) => unsafe { libc::kill(pid as i32, 0) == 0 },
    }
}

fn gone_within(pid: u32, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if !running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !running(pid)
}

fn real_result() -> Value {
    json!({"data": [
        {"id": "gpt-5.6-sol", "displayName": "GPT-5.6-Sol", "description": "Latest frontier agentic coding model.", "hidden": false,
         "supportedReasoningEfforts": [{"reasoningEffort": "low"}, {"reasoningEffort": "medium"}, {"reasoningEffort": "high"},
            {"reasoningEffort": "xhigh"}, {"reasoningEffort": "max"}, {"reasoningEffort": "ultra"}]},
        {"id": "gpt-5.4-mini", "displayName": "GPT-5.4-mini", "supportedReasoningEfforts": [{"reasoningEffort": "low"}, {"reasoningEffort": "high"}]}
    ]})
}

// case: survives a shape it does not recognize
#[test]
fn a_server_list_of_a_shape_the_probe_does_not_know_offers_nothing() {
    for result in [Value::Null, json!({"models": [{"id": "x"}]})] {
        let fake = Fake::new(result.clone());
        let cache = Mutex::new(None);
        let models = kept_catalog(&cache, || fake.read());
        assert_eq!(models, Vec::<Value>::new(), "{result}");
        assert_eq!(fake.starts(), 1);
    }
}

// case: reads the list and shuts the server down again
#[test]
fn the_probe_reads_the_list_then_shuts_its_whole_server_down() {
    let fake = Fake::new(real_result());
    let cache = Mutex::new(None);
    let models = kept_catalog(&cache, || fake.read());
    // Two from the protocol, plus the account-default row in front.
    assert_eq!(models.len(), 3);
    assert_eq!(models[0]["id"], "account-default");
    assert_eq!(models[1]["id"], "gpt-5.6-sol");
    let helpers = fake.helpers();
    assert_eq!(helpers.len(), 1);
    // Leaving the server running would be a background process the user never asked for.
    assert!(gone_within(helpers[0], Duration::from_secs(5)), "the server's helper {} is still running", helpers[0]);
}

// case: caches a success instead of starting a server per call
#[test]
fn a_read_catalog_is_kept_rather_than_a_server_started_per_call() {
    let fake = Fake::new(real_result());
    let cache = Mutex::new(None);
    let first = kept_catalog(&cache, || fake.read());
    let second = kept_catalog(&cache, || fake.read());
    assert_eq!(first.len(), 3);
    assert_eq!(second, first);
    assert_eq!(fake.starts(), 1);
}

// case: does not cache a failure
#[test]
fn a_server_that_listed_nothing_is_asked_again_next_time() {
    // A transient failure must not pin the picker to "unavailable" for the rest of the cache window.
    let fake = Fake::new(json!({"data": []}));
    let cache = Mutex::new(None);
    assert_eq!(kept_catalog(&cache, || fake.read()), Vec::<Value>::new());
    assert_eq!(kept_catalog(&cache, || fake.read()), Vec::<Value>::new());
    assert_eq!(fake.starts(), 2);
}
