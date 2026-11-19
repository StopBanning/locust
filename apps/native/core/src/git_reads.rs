//! Locust's git tool for a run with no shell (Electron `resources/locust-git-reads.mjs`, 0.818): a per-run stdio MCP
//! server that runs git with no shell and checks each argument. OpenCode starts it for one Ask or Plan run, in that
//! run's folder (`LOCUST_GIT_FOLDER`), as `locust-native --git-reads`; it is never written to an agent's own config.
//!
//! Why not the shell: OpenCode matched a `git status*` allow with the redirect taken off, so `git status > notes.txt`
//! wrote the file (measured 2026-10-22). Ask and Plan refuse every shell command, and git reaches the run here.
//!
//! Nothing here writes: `GIT_OPTIONAL_LOCKS=0` keeps `git status` from refreshing the index, a diff reads a copy of
//! the index, every git variable the run inherited is dropped, and git is told `core.fsmonitor=false` and plain diffs
//! (no external diff program, no textconv filter).
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs,
    io::{self, BufRead, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_CHARS: usize = 60_000;
const TIME_LIMIT: Duration = Duration::from_secs(30);
const MAX_REQUEST_BYTES: usize = 512 * 1024;
const OWN_CONFIG: [&str; 3] = ["--no-pager", "-c", "core.fsmonitor=false"];
const PLAIN: [&str; 3] = ["--no-color", "--no-ext-diff", "--no-textconv"];

/// The six tools, as the Electron server lists them.
pub fn tools() -> Value {
    let read_only = json!({ "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false });
    let revision = |description: &str| json!({ "type": "string", "minLength": 1, "maxLength": 300, "description": description });
    let paths = json!({
        "type": "array",
        "items": { "type": "string", "minLength": 1, "maxLength": 400 },
        "maxItems": 50,
        "description": "Only these files or folders, relative to this folder."
    });
    let schema = |properties: Value, required: Value| {
        json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
    };
    let boolean = |description: &str| json!({ "type": "boolean", "description": description });
    json!([
        {
            "name": "status",
            "description": "git status for this folder: its branch, and each changed, staged, deleted and untracked file. Read only.",
            "inputSchema": schema(json!({}), json!([])),
            "annotations": read_only
        },
        {
            "name": "diff",
            "description": "git diff: the uncommitted change by default. staged: true shows what is staged; against compares with a commit, branch or range such as main...HEAD. Untracked files are not in a diff: status lists them, and your read tool shows them. Read only.",
            "inputSchema": schema(json!({
                "staged": boolean("What is staged for the next commit, rather than what is not."),
                "against": revision("A commit, branch or range to compare with, such as HEAD~1 or main...HEAD."),
                "paths": paths,
                "stat": boolean("Only which files changed, and by how many lines.")
            }), json!([])),
            "annotations": read_only
        },
        {
            "name": "log",
            "description": "git log: the latest commits, newest first, on this branch or on revision, and only those touching paths if given. Read only.",
            "inputSchema": schema(json!({
                "revision": revision("A commit, branch or range such as main..HEAD. The current branch if left out."),
                "paths": paths,
                "count": { "type": "integer", "minimum": 1, "maximum": 200, "description": "How many commits, 20 if left out." },
                "oneline": boolean("One line per commit."),
                "patch": boolean("Each commit with its change."),
                "stat": boolean("Each commit with the files it changed.")
            }), json!([])),
            "annotations": read_only
        },
        {
            "name": "show",
            "description": "git show: one commit with its change, or commit:path for a file as it was in that commit. Read only.",
            "inputSchema": schema(json!({
                "object": revision("A commit such as HEAD or a1b2c3d, or commit:path such as HEAD~2:src/app.ts."),
                "stat": boolean("Only the files the commit changed, not the change itself.")
            }), json!(["object"])),
            "annotations": read_only
        },
        {
            "name": "blame",
            "description": "git blame: who last changed each line of a file, and in which commit. Read only.",
            "inputSchema": schema(json!({
                "path": { "type": "string", "minLength": 1, "maxLength": 400, "description": "The file, relative to this folder." },
                "revision": revision("As of this commit; the working file if left out.")
            }), json!(["path"])),
            "annotations": read_only
        },
        {
            "name": "branches",
            "description": "git branch: every local and remote branch, with the commit it is on and what it tracks. Read only.",
            "inputSchema": schema(json!({}), json!([])),
            "annotations": read_only
        }
    ])
}

fn known_properties(name: &str) -> Option<Vec<String>> {
    let tools = tools();
    let tool = tools.as_array()?.iter().find(|t| t["name"] == name)?;
    Some(tool["inputSchema"]["properties"].as_object()?.keys().cloned().collect())
}

/// A revision, range or commit:path, as one argument git cannot take for an option.
fn revision(value: Option<&Value>, what: &str) -> Result<Option<String>, String> {
    let Some(value) = value else { return Ok(None) };
    let text = value.as_str().filter(|t| {
        !t.trim().is_empty() && t.encode_utf16().count() <= 300 && !t.contains(['\0', '\r', '\n'])
    });
    let Some(text) = text else {
        return Err(format!("{what} must be a commit, branch or range, such as HEAD~1 or main...HEAD. Nothing was read."));
    };
    if text.starts_with('-') {
        return Err(format!("{what} cannot start with \"-\": options are not taken here. Nothing was read."));
    }
    Ok(Some(text.to_owned()))
}

/// A path made whole without asking the disk, as Node's `path.resolve` makes it.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A path inside this folder, as git takes it after `--`.
fn inside(value: &Value, folder: &Path) -> Result<String, String> {
    let text = value.as_str().filter(|t| {
        !t.trim().is_empty() && t.encode_utf16().count() <= 400 && !t.contains(['\0', '\r', '\n'])
    });
    let Some(text) = text else {
        return Err("Each path must be a file or folder in this folder. Nothing was read.".into());
    };
    // Git's own pathspec syntax (`:(top)`, `:/`) reaches past the folder; a plain path is all that is needed here.
    if text.starts_with(':') {
        return Err(format!("{text} is not a plain path. Name the file or folder itself. Nothing was read."));
    }
    let root = lexical(folder);
    let Ok(within) = lexical(&root.join(text)).strip_prefix(&root).map(Path::to_path_buf) else {
        return Err(format!("{text} is outside this folder. Nothing was read."));
    };
    let parts: Vec<String> = within.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    Ok(if parts.is_empty() { ".".into() } else { parts.join("/") })
}

fn paths(value: Option<&Value>, folder: &Path) -> Result<Vec<String>, String> {
    let Some(value) = value else { return Ok(Vec::new()) };
    let Some(list) = value.as_array().filter(|l| l.len() <= 50) else {
        return Err("paths must be a list of at most 50 files or folders. Nothing was read.".into());
    };
    list.iter().map(|one| inside(one, folder)).collect()
}

fn flag(value: Option<&Value>, name: &str) -> Result<bool, String> {
    match value {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{name} must be true or false. Nothing was read.")),
    }
}

fn whole_number(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 1e15).map(|f| f as i64))
}

/// The git arguments a tool call stands for; the refusal when one of them is not what it should be.
pub fn git_args(name: &str, input: &Value, folder: &Path) -> Result<Vec<String>, String> {
    let empty = serde_json::Map::new();
    let args = input.as_object().unwrap_or(&empty);
    let Some(known) = known_properties(name) else {
        return Err(format!("There is no git tool called {name}. Nothing was read."));
    };
    let extra: Vec<&str> = args.keys().filter(|k| !known.contains(k)).map(String::as_str).collect();
    if !extra.is_empty() {
        return Err(format!("git {name} takes no {}. Nothing was read.", extra.join(", ")));
    }
    let own = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let mut out = Vec::new();
    match name {
        "status" => out = own(&["status", "--short", "--branch"]),
        "diff" => {
            let against = revision(args.get("against"), "against")?;
            out.push("diff".into());
            out.extend(own(&PLAIN));
            if flag(args.get("staged"), "staged")? {
                out.push("--cached".into());
            }
            if flag(args.get("stat"), "stat")? {
                out.push("--stat".into());
            }
            out.extend(against);
            out.push("--".into());
            out.extend(paths(args.get("paths"), folder)?);
        }
        "log" => {
            let count = match args.get("count") {
                None => 20,
                Some(v) => whole_number(v)
                    .filter(|n| (1..=200).contains(n))
                    .ok_or("count must be a whole number from 1 to 200. Nothing was read.")?,
            };
            let at = revision(args.get("revision"), "revision")?;
            out.push("log".into());
            out.extend(own(&PLAIN));
            out.push(format!("--max-count={count}"));
            for (key, arg) in [("oneline", "--oneline"), ("patch", "--patch"), ("stat", "--stat")] {
                if flag(args.get(key), key)? {
                    out.push(arg.into());
                }
            }
            out.extend(at);
            out.push("--".into());
            out.extend(paths(args.get("paths"), folder)?);
        }
        "show" => {
            let object = revision(args.get("object"), "object")?
                .ok_or("show needs an object: a commit, or commit:path. Nothing was read.")?;
            out.push("show".into());
            out.extend(own(&PLAIN));
            if flag(args.get("stat"), "stat")? {
                out.push("--stat".into());
            }
            out.push(object);
            out.push("--".into());
        }
        "blame" => {
            let at = revision(args.get("revision"), "revision")?;
            let path = args.get("path").ok_or("blame needs a path: the file to read. Nothing was read.")?;
            out = own(&["blame", "--no-textconv"]);
            out.extend(at);
            out.push("--".into());
            out.push(inside(path, folder)?);
        }
        "branches" => out = own(&["branch", "--all", "-vv", "--no-color"]),
        _ => return Err(format!("There is no git tool called {name}. Nothing was read.")),
    }
    Ok(out)
}

fn text_result(text: impl Into<String>, is_error: bool) -> Value {
    let mut out = json!({ "content": [{ "type": "text", "text": text.into() }] });
    if is_error {
        out["isError"] = json!(true);
    }
    out
}

/// What a git call ended with.
struct Ran {
    stdout: String,
    stderr: String,
    /// Exited zero.
    ok: bool,
    timed_out: bool,
    not_found: bool,
}

/// The server for one run: its folder, where a diff's copy of the index goes, and the environment it inherited.
pub struct GitReads {
    folder: Option<PathBuf>,
    scratch: PathBuf,
    inherited: Vec<(OsString, OsString)>,
    git: String,
}

impl GitReads {
    /// As OpenCode starts it: the folder from `LOCUST_GIT_FOLDER`, this process's own environment.
    pub fn from_process() -> Self {
        Self::new(
            std::env::var_os("LOCUST_GIT_FOLDER").filter(|f| !f.is_empty()).map(PathBuf::from),
            std::env::temp_dir(),
            std::env::vars_os().collect(),
        )
    }

    pub fn new(folder: Option<PathBuf>, scratch: PathBuf, inherited: Vec<(OsString, OsString)>) -> Self {
        Self { folder, scratch, inherited, git: "git".into() }
    }

    /// Git's environment: none of the run's git variables, no locks taken, no prompt, no pager.
    fn git_env(&self) -> Vec<(OsString, OsString)> {
        let mut env: Vec<(OsString, OsString)> = self
            .inherited
            .iter()
            .filter(|(k, _)| !k.to_string_lossy().to_ascii_uppercase().starts_with("GIT_"))
            .cloned()
            .collect();
        for (k, v) in [("GIT_OPTIONAL_LOCKS", "0"), ("GIT_TERMINAL_PROMPT", "0"), ("GIT_PAGER", "cat"), ("PAGER", "cat")] {
            env.retain(|(key, _)| key != k);
            env.push((k.into(), v.into()));
        }
        env
    }

    fn git(&self, folder: &Path, args: &[String], env: &[(OsString, OsString)]) -> Ran {
        let mut command = Command::new(&self.git);
        command
            .args(OWN_CONFIG)
            .args(args)
            .current_dir(folder)
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Ran { stdout: String::new(), stderr: e.to_string(), ok: false, timed_out: false, not_found: e.kind() == io::ErrorKind::NotFound };
            }
        };
        let read = |pipe: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                if let Some(mut p) = pipe {
                    let _ = p.take(32 * 1024 * 1024).read_to_end(&mut bytes);
                }
                String::from_utf8_lossy(&bytes).into_owned()
            })
        };
        let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let started = Instant::now();
        let (ok, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status.success(), false),
                Ok(None) if started.elapsed() >= TIME_LIMIT => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break (false, true);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => break (false, false),
            }
        };
        Ran { stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default(), ok, timed_out, not_found: false }
    }

    /// `git diff` writes the index back with what it refreshed, which takes the lock a commit the person makes at the
    /// same moment needs. So a diff reads a copy of the index, keeping the index's own times: git checks the contents
    /// of a file changed in the same second as the index was written ("racy git"), and a copy dated later made git
    /// trust that entry instead (measured 2026-11-01). None when there is no index to copy.
    fn index_copy(&self, folder: &Path, env: &[(OsString, OsString)]) -> Option<PathBuf> {
        let found = self.git(folder, &["rev-parse".into(), "--git-path".into(), "index".into()], env);
        if !found.ok {
            return None;
        }
        let index = folder.join(found.stdout.trim());
        let copy = self.scratch.join(format!("locust-git-index-{}", crate::invariants::unique_nanos()));
        let meta = fs::metadata(&index).ok()?;
        if fs::copy(&index, &copy).is_err() {
            let _ = fs::remove_file(&copy);
            return None;
        }
        let mut times = fs::FileTimes::new();
        if let Ok(at) = meta.accessed() {
            times = times.set_accessed(at);
        }
        if let Ok(at) = meta.modified() {
            times = times.set_modified(at);
        }
        if fs::File::options().write(true).open(&copy).and_then(|f| f.set_times(times)).is_err() {
            let _ = fs::remove_file(&copy);
            return None;
        }
        Some(copy)
    }

    /// One tool call, answered as the tool's result.
    pub fn call(&self, name: &str, input: &Value) -> Value {
        let Some(folder) = self.folder.as_deref() else {
            return text_result("Locust did not say which folder this run is in. Nothing was read.", true);
        };
        let args = match git_args(name, input, folder) {
            Ok(a) => a,
            Err(why) => return text_result(why, true),
        };
        let env = self.git_env();
        let copy = if name == "diff" { self.index_copy(folder, &env) } else { None };
        let ran = match &copy {
            Some(c) => {
                let mut with = env.clone();
                with.push(("GIT_INDEX_FILE".into(), c.clone().into_os_string()));
                self.git(folder, &args, &with)
            }
            None => self.git(folder, &args, &env),
        };
        if let Some(c) = copy {
            let _ = fs::remove_file(&c);
            let mut lock = c.into_os_string();
            lock.push(".lock");
            let _ = fs::remove_file(lock);
        }
        said(name, ran)
    }

    /// One line of the protocol, and the answer to send, if any.
    pub fn answer(&self, line: &str) -> Option<Value> {
        let error = |id: Value, code: i64, message: &str| json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } });
        let Ok(rpc) = serde_json::from_str::<Value>(line) else {
            return Some(error(Value::Null, -32700, "Invalid JSON"));
        };
        if !rpc.is_object() || rpc["jsonrpc"] != "2.0" {
            return Some(error(Value::Null, -32600, "Invalid request"));
        }
        let id = rpc.get("id")?.clone();
        let result = |result: Value| json!({ "jsonrpc": "2.0", "id": id.clone(), "result": result });
        match rpc["method"].as_str() {
            Some("initialize") => {
                let asked = rpc["params"]["protocolVersion"].as_str().unwrap_or_default();
                let version = if ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&asked) { asked } else { "2025-06-18" };
                Some(result(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "locust_git", "version": "1.0.0" }
                })))
            }
            Some("ping") => Some(result(json!({}))),
            Some("tools/list") => Some(result(json!({ "tools": tools() }))),
            Some("tools/call") => {
                let name = rpc["params"]["name"].as_str().unwrap_or_default();
                if known_properties(name).is_none() {
                    let shown = match &rpc["params"]["name"] {
                        Value::String(s) => s.clone(),
                        Value::Null => "undefined".into(),
                        other => other.to_string(),
                    };
                    return Some(error(id, -32602, &format!("Unknown git tool: {shown}")));
                }
                let input = rpc["params"].get("arguments").cloned().unwrap_or_else(|| json!({}));
                Some(result(self.call(name, &input)))
            }
            _ => Some(error(id, -32601, "Method not found")),
        }
    }

    /// Answers each line of `input` on `output` until the input ends.
    pub fn serve(&self, mut input: impl BufRead, output: &mut impl Write) -> io::Result<()> {
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = (&mut input).take(MAX_REQUEST_BYTES as u64 + 1).read_until(b'\n', &mut line)?;
            if read == 0 {
                return Ok(());
            }
            if line.len() > MAX_REQUEST_BYTES {
                let refused = json!({ "jsonrpc": "2.0", "id": Value::Null, "error": { "code": -32600, "message": "MCP request is too large. Nothing was run." } });
                writeln!(output, "{refused}")?;
                output.flush()?;
                return Err(io::Error::other("MCP request is too large"));
            }
            let text = String::from_utf8_lossy(&line);
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            if let Some(answer) = self.answer(text) {
                writeln!(output, "{answer}")?;
                output.flush()?;
            }
        }
    }
}

/// What a git call printed, as the tool's answer.
fn said(name: &str, ran: Ran) -> Value {
    let why = ran.stderr.trim();
    if ran.timed_out {
        return text_result(format!("git {name} took longer than {} seconds and was stopped. Ask for less, with paths or stat.", TIME_LIMIT.as_secs()), true);
    }
    if ran.not_found {
        return text_result("git is not installed on this computer, so there is no history to read.", true);
    }
    if !ran.ok {
        if why.to_ascii_lowercase().contains("not a git repository") {
            return text_result("This folder is not a git repository, so there is no history to read. Your read, grep, glob and list tools still work.", true);
        }
        let cut: String = why.chars().take(2000).collect();
        return text_result(format!("git {name} failed: {}", if cut.is_empty() { "it printed nothing to say why" } else { &cut }), true);
    }
    let text = ran.stdout;
    if text.trim().is_empty() {
        return text_result(if name == "diff" { "No difference.".to_owned() } else { format!("git {name} printed nothing.") }, false);
    }
    if text.chars().count() <= MAX_CHARS {
        return text_result(text, false);
    }
    let head: String = text.chars().take(MAX_CHARS).collect();
    text_result(
        format!("{head}\n\n[Cut at 60,000 characters. Ask for fewer files with paths, or stat: true for the list of files.]"),
        false,
    )
}

/// `locust-native --git-reads`: the server on this process's own input and output.
pub fn serve_stdio() -> i32 {
    let reads = GitReads::from_process();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    match reads.serve(stdin.lock(), &mut stdout) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

#[cfg(test)]
#[path = "git_reads_tests.rs"]
mod tests;
