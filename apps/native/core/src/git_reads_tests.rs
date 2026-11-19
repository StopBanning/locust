//! Electron apps/desktop/src/main/git-reaches-a-run-with-no-shell.test.ts, against the Rust server in-process. Git
//! runs only in temporary folders these tests make.
use super::*;
use std::time::SystemTime;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp(tag: &str) -> Temp {
    let dir = std::env::temp_dir().join(format!("locust-git-reads-{tag}-{}", crate::invariants::unique_nanos()));
    fs::create_dir_all(&dir).unwrap();
    Temp(fs::canonicalize(&dir).unwrap())
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=Locust test", "-c", "user.email=test@locust.test"])
        .args(args)
        .current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A repository with one commit and an uncommitted change that breaks it, inside a folder that has a sibling.
fn repository() -> (Temp, PathBuf) {
    let root = temp("repo");
    git(&root.0, &["init", "-q", "project"]);
    let folder = root.0.join("project");
    git(&folder, &["config", "core.autocrlf", "false"]);
    fs::write(folder.join("calc.py"), "def add(a, b):\n    return a + b\n").unwrap();
    git(&folder, &["add", "calc.py"]);
    git(&folder, &["commit", "-q", "-m", "add calc"]);
    fs::write(folder.join("calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
    (root, folder)
}

/// The server as OpenCode would start it in `folder`, with its index copies in `scratch`.
struct Tool {
    reads: GitReads,
}

impl Tool {
    fn start(folder: &Path, scratch: &Path, env: &[(&str, &str)]) -> Self {
        let mut inherited: Vec<(OsString, OsString)> = std::env::vars_os().collect();
        for (k, v) in env {
            inherited.push(((*k).into(), (*v).into()));
        }
        Self { reads: GitReads::new(Some(folder.to_path_buf()), scratch.to_path_buf(), inherited) }
    }

    /// One request through the protocol, as a line in and a line out.
    fn rpc(&self, method: &str, params: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string();
        let mut out = Vec::new();
        self.reads.serve(io::Cursor::new(format!("{line}\n")), &mut out).unwrap();
        serde_json::from_slice(out.split(|b| *b == b'\n').next().unwrap()).unwrap()
    }

    fn call(&self, name: &str, args: Value) -> (String, bool) {
        let answer = self.rpc("tools/call", json!({ "name": name, "arguments": args }));
        let text = answer["result"]["content"].as_array().unwrap().iter().map(|p| p["text"].as_str().unwrap()).collect::<String>();
        (text, answer["result"]["isError"] == true)
    }
}

// case: lists six tools, every one marked read only
#[test]
fn lists_six_tools_every_one_marked_read_only() {
    let (root, folder) = repository();
    let tool = Tool::start(&folder, &root.0, &[]);
    assert_eq!(tool.rpc("initialize", json!({ "protocolVersion": "2025-06-18" }))["result"]["serverInfo"]["name"], "locust_git");
    let listed = tool.rpc("tools/list", json!(null));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["status", "diff", "log", "show", "blame", "branches"]);
    for one in tools {
        assert_eq!(one["annotations"]["readOnlyHint"], true, "{}", one["name"]);
    }
}

// case: reads what a review needs: the change, the history, a file as it was
#[test]
fn reads_what_a_review_needs_the_change_the_history_a_file_as_it_was() {
    let (root, folder) = repository();
    let tool = Tool::start(&folder, &root.0, &[]);
    let (status, _) = tool.call("status", json!({}));
    assert!(status.starts_with("## ") && status.contains("\n M calc.py"), "{status}");
    let (diff, error) = tool.call("diff", json!({}));
    assert!(!error);
    assert!(diff.contains("-    return a + b\n+    return a - b"), "{diff}");
    assert!(tool.call("diff", json!({ "stat": true, "paths": ["calc.py"] })).0.contains("1 file changed"));
    let (log, _) = tool.call("log", json!({ "oneline": true, "count": 5 }));
    let (sha, rest) = log.split_once(' ').unwrap();
    assert!(!sha.is_empty() && sha.chars().all(|c| c.is_ascii_hexdigit()) && rest == "add calc\n", "{log}");
    assert_eq!(tool.call("show", json!({ "object": "HEAD:calc.py" })).0, "def add(a, b):\n    return a + b\n");
    assert!(tool.call("blame", json!({ "path": "calc.py" })).0.contains("Not Committed Yet"));
    assert!(tool.call("branches", json!({})).0.contains("add calc"));
    assert_eq!(tool.call("diff", json!({ "staged": true })).0, "No difference.");
}

fn set_times(path: &Path, at: SystemTime) {
    let times = fs::FileTimes::new().set_accessed(at).set_modified(at);
    fs::File::options().write(true).open(path).unwrap().set_times(times).unwrap();
}

// case: sees a change of the same size made in the same second as the index was written
#[test]
fn sees_a_change_of_the_same_size_made_in_the_same_second_as_the_index_was_written() {
    let root = temp("racy");
    git(&root.0, &["init", "-q", "project"]);
    let folder = root.0.join("project");
    git(&folder, &["config", "core.autocrlf", "false"]);
    // A file's change time cannot be set; in the measured race it fell in the same second too.
    git(&folder, &["config", "core.trustctime", "false"]);
    let file = folder.join("calc.py");
    // 2026-01-01T12:00:00Z
    let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_767_268_800);
    fs::write(&file, "def add(a, b):\n    return a + b\n").unwrap();
    // Dated before the commit, so git keeps the entry's times as they are rather than marking it for a content check.
    set_times(&file, then);
    git(&folder, &["add", "calc.py"]);
    git(&folder, &["commit", "-q", "-m", "add calc"]);
    fs::write(&file, "def add(a, b):\n    return a - b\n").unwrap();
    set_times(&file, then);
    set_times(&folder.join(".git").join("index"), then);
    // The index itself makes git look: the change is there.
    assert!(git(&folder, &["diff", "--stat"]).contains("1 file changed"));
    let (diff, _) = Tool::start(&folder, &root.0, &[]).call("diff", json!({}));
    assert!(diff.contains("+    return a - b"), "{diff}");
}

// case: takes no option, no path outside the folder, and nothing it does not name
#[test]
fn takes_no_option_no_path_outside_the_folder_and_nothing_it_does_not_name() {
    let (root, folder) = repository();
    let tool = Tool::start(&folder, &root.0, &[]);
    let elsewhere = root.0.join("elsewhere.txt").to_string_lossy().into_owned();
    for (name, args) in [
        ("diff", json!({ "against": "--output=../written.txt" })),
        ("log", json!({ "revision": "--output=written.txt" })),
        ("show", json!({ "object": "--ext-diff" })),
        ("log", json!({ "paths": ["../outside"] })),
        ("diff", json!({ "paths": [elsewhere] })),
        ("diff", json!({ "paths": [":(top)"] })),
        ("blame", json!({ "path": "../calc.py" })),
        ("status", json!({ "porcelain": "--output=written.txt" })),
        ("log", json!({ "count": 0 })),
        ("diff", json!({ "staged": "yes" })),
    ] {
        let (text, error) = tool.call(name, args.clone());
        assert!(error, "{name} {args}");
        assert!(text.ends_with("Nothing was read."), "{name} {args}: {text}");
    }
    assert!(!folder.join("written.txt").exists() && !root.0.join("written.txt").exists());
    // An unknown tool is the protocol's error, not a git call.
    assert_eq!(tool.rpc("tools/call", json!({ "name": "push", "arguments": {} }))["error"]["code"], -32602);
}

// case: writes nothing: no index refresh, no lock, and no program a variable names
#[test]
fn writes_nothing_no_index_refresh_no_lock_and_no_program_a_variable_names() {
    let (root, folder) = repository();
    // A committed file touched but not changed: `git status` would refresh the index for it, and write the index.
    fs::write(folder.join("notes.txt"), "Status: draft\n").unwrap();
    git(&folder, &["add", "notes.txt"]);
    git(&folder, &["commit", "-q", "-m", "add notes"]);
    set_times(&folder.join("notes.txt"), SystemTime::now() + Duration::from_secs(60));
    let index = folder.join(".git").join("index");
    let before = fs::metadata(&index).unwrap().modified().unwrap();
    let listed = |dir: &Path| {
        let mut names: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    };
    let in_folder = listed(&folder);
    // `touch` as git's external diff would make files named after its arguments, were it run.
    let elsewhere = folder.join("elsewhere").to_string_lossy().into_owned();
    let scratch = temp("scratch");
    let tool = Tool::start(&folder, &scratch.0, &[("GIT_EXTERNAL_DIFF", "touch"), ("GIT_DIR", &elsewhere)]);
    std::thread::sleep(Duration::from_millis(50));
    assert!(!tool.call("status", json!({})).1);
    assert!(tool.call("diff", json!({})).0.contains("+    return a - b"));
    assert!(!tool.call("log", json!({ "patch": true })).1);
    assert_eq!(fs::metadata(&index).unwrap().modified().unwrap(), before);
    assert_eq!(listed(&folder), in_folder);
    assert!(listed(&folder.join(".git")).iter().all(|n| !n.ends_with(".lock")));
    // The copy of the index a diff read is gone with it.
    assert!(listed(&scratch.0).iter().all(|n| !n.starts_with("locust-git-index-")), "{:?}", listed(&scratch.0));
    drop(root);
}

// case: says so when the folder is not a repository
#[test]
fn says_so_when_the_folder_is_not_a_repository() {
    let root = temp("none");
    let ceiling = root.0.to_string_lossy().into_owned();
    let said = Tool::start(&root.0, &root.0, &[("GIT_CEILING_DIRECTORIES", &ceiling)]).call("status", json!({}));
    assert_eq!(
        said,
        ("This folder is not a git repository, so there is no history to read. Your read, grep, glob and list tools still work.".to_owned(), true)
    );
}

#[test]
fn the_protocol_answers_ping_skips_notifications_and_refuses_what_it_cannot_read() {
    let scratch = temp("protocol");
    let reads = GitReads::new(None, scratch.0.clone(), Vec::new());
    let mut out = Vec::new();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n",
        "not json\n",
        "{\"jsonrpc\":\"1.0\",\"id\":8,\"method\":\"ping\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"resources/list\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"tools/call\",\"params\":{\"name\":\"status\"}}\n",
    );
    reads.serve(io::Cursor::new(input), &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], json!({ "jsonrpc": "2.0", "id": 7, "result": {} }));
    assert_eq!(lines[1]["error"]["code"], -32700);
    assert_eq!(lines[2]["error"]["code"], -32600);
    assert_eq!(lines[3]["error"]["code"], -32601);
    assert_eq!(lines[4]["result"]["content"][0]["text"], "Locust did not say which folder this run is in. Nothing was read.");
    assert_eq!(
        git_args("diff", &json!({ "against": "main...HEAD", "paths": ["src", "./"] }), Path::new("/work/p")).unwrap(),
        ["diff", "--no-color", "--no-ext-diff", "--no-textconv", "main...HEAD", "--", "src", "."]
    );
    assert_eq!(
        git_args("log", &json!({}), Path::new("/work/p")).unwrap(),
        ["log", "--no-color", "--no-ext-diff", "--no-textconv", "--max-count=20", "--"]
    );
}
