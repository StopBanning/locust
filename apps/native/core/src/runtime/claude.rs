type ApprovalLifecycle = Arc<Mutex<Option<(Arc<Channel>, String)>>>;
// Claude Code print/stream-json driver. Ports createClaudePrintCommand + process-runner stream-json path.
use super::process::{Control, interrupt_process};
use super::records::{self, BoundedStderr, Limits, Lines, Piece, Stopping, Termination};
use super::{Event, Runtime, Turn, claude_events::ClaudeNormalizer};
use crate::approval::{Channel, Mode};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

/// Ask-mode tools from createClaudePrintCommand when sandbox is read-only.
const ASK_TOOLS: &str = "Read,Glob,Grep,Task,Skill,WebSearch,WebFetch,mcp__*";

enum StdinMsg {
    Bytes(Vec<u8>),
    Close,
}

pub struct Claude {
    child: Child,
    input: SyncSender<StdinMsg>,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    normal: ClaudeNormalizer,
    queued: VecDeque<Event>,
    /// Ending the process: SIGINT, then SIGKILL past the grace period (process-runner.ts requestTermination).
    stopping: Option<Stopping>,
    /// The person stopped it (as against the host ending it after its result, or for input it could not take).
    user_stopped: bool,
    cancel: Option<Arc<AtomicBool>>,
    limits: Limits,
    stderr: Arc<Mutex<BoundedStderr>>,
    dropped: Arc<AtomicUsize>,
    output_limit: Arc<AtomicBool>,
    input_failed: Arc<AtomicBool>,
    /// The prompt, kept only to redact it from stderr.
    prompt: String,
    /// When the turn's result closed input; past `linger_after_result` a run still alive is ended.
    result_at: Option<Instant>,
    ended_after_result: bool,
    /// stdin stays open until the turn's `result` (process-runner.ts A2.10).
    stdin_open: bool,
    /// Process exited; finish() still owes a terminal event.
    exited: bool,
    exit_code: Option<i32>,
    approvals: Option<Arc<Channel>>,
    mission_id: String,
    run_id: String,
    folder: PathBuf,
    mode: Mode,
    approval_lifecycle: ApprovalLifecycle,
}

fn err(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

/// User turn line for `--input-format stream-json` (process-runner.ts:streamJsonUserLine).
pub fn user_line(prompt: &str) -> Vec<u8> {
    let mut bytes = json!({
        "type": "user",
        "message": {"role": "user", "content": prompt}
    })
    .to_string()
    .into_bytes();
    bytes.push(b'\n');
    bytes
}

/// The editing tool list, for every mode that may write.
const EDIT_TOOLS: &str = "Read,Glob,Grep,Edit,Write,NotebookEdit,Bash,Task,Skill,WebSearch,WebFetch,mcp__*";
/// The permission bridge's tool, as `permission_bridge` serves it.
pub const BRIDGE_TOOL: &str = "mcp__locust__approve";

/// Everything a Claude Code turn's command line can carry. Paths are the files and folders this run made or was given.
#[derive(Debug, Clone, Default)]
pub struct PrintOptions<'a> {
    pub mode: Mode,
    /// The permission bridge's MCP config; Electron's host passes it in every mode but Auto.
    pub bridge: Option<&'a str>,
    /// Locust's browser MCP config, when the run has the browser.
    pub browser: Option<&'a str>,
    /// The command guard's `--settings` file (every mode, Auto included).
    pub guard_settings: Option<&'a str>,
    pub skills: &'a [&'a str],
    pub references: &'a [&'a str],
    /// The person's connectors this teammate was given; one allow rule each outside Auto.
    pub connectors: &'a [&'a str],
    /// False keeps the person's account MCP servers out of the run (`--strict-mcp-config`).
    pub account_connectors: bool,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub resume: Option<&'a str>,
    pub fork: bool,
}

fn require_text<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(format!("{label} must be non-empty"));
    }
    Ok(value)
}

/// commands.ts requireEffort: one plain lowercase word.
fn require_effort(value: &str) -> Result<&str, String> {
    if !(1..=16).contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err("Effort is invalid".into());
    }
    Ok(value)
}

/// connectors.ts toolPrefixFor: Claude Code's own rule, read from 2.1.281. Every character outside `[A-Za-z0-9_-]`
/// becomes an underscore; only for a `claude.ai ` name are runs collapsed and the ends trimmed.
pub fn tool_prefix_for(name: &str) -> String {
    let trimmed = name.trim();
    let mut server: String =
        trimmed.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    if trimmed.starts_with("claude.ai ") {
        let mut collapsed = String::new();
        for c in server.chars() {
            if !(c == '_' && collapsed.ends_with('_')) {
                collapsed.push(c);
            }
        }
        server = collapsed.strip_prefix('_').unwrap_or(&collapsed).to_owned();
        if server.ends_with('_') {
            server.pop();
        }
    }
    format!("mcp__{server}__")
}

/// connectors.ts allowRuleFor: fails closed on a name with no letter or digit, which would widen `mcp____*`.
pub fn allow_rule_for(name: &str) -> Option<String> {
    let prefix = tool_prefix_for(name);
    let server = &prefix["mcp__".len()..prefix.len() - "__".len()];
    server.bytes().any(|b| b.is_ascii_alphanumeric()).then(|| format!("{prefix}*"))
}

/// codex-mission.ts claudeConnectorRules (ask-asks-before-a-connector): which of the person's connectors a Claude turn
/// may call without asking. None in Ask or Plan (read-only: every call goes to the card), none when the person asked to
/// be asked about every one (or the turn is Approve each), otherwise the named ones. `print_args` builds whatever it is
/// given, as Electron's command builder does; this is the gate in front of it.
pub fn claude_connector_rules<'a>(mode: Mode, ask_every: bool, named: &'a [&'a str]) -> &'a [&'a str] {
    let read_only = matches!(mode, Mode::Ask | Mode::Plan);
    if ask_every || mode == Mode::ApproveEach || read_only || named.is_empty() { &[] } else { named }
}

fn connector_rules(names: &[&str], browser: bool) -> Vec<String> {
    let mut rules: Vec<String> = Vec::new();
    for name in names {
        if let Some(rule) = allow_rule_for(name)
            && !rules.contains(&rule)
        {
            rules.push(rule);
        }
    }
    if browser {
        rules.push("mcp__locust_browser__*".into());
    }
    if rules.is_empty() { Vec::new() } else { vec!["--allowedTools".into(), rules.join(",")] }
}

/// commands.ts claudeReferenceDenyRule: Claude's file rules use Edit for every editing tool and `//` for an absolute
/// path. A path the rules cannot name literally is refused, so a reference is never left unprotected.
pub fn reference_deny_rule(folder: &str) -> Result<String, String> {
    let mut path = require_text(folder, "Reference folder")?.replace('\\', "/");
    while path.ends_with('/') {
        path.pop();
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        path = format!("/{}{}", (bytes[0] as char).to_ascii_lowercase(), &path[2..]);
    }
    if !path.starts_with('/') || path.starts_with("//") || path.contains(['\r', '\n', ',', '(', ')', '*', '?', '[', ']']) {
        return Err("This reference path cannot be protected by Claude Code's file rules. Nothing was sent; remove it or choose a local folder without pattern characters.".into());
    }
    Ok(format!("Edit(/{path}/**)"))
}

/// commands.ts createClaudePrintCommand, argument for argument. The reasons for each flag, and the measurements
/// behind them, are kept beside the TypeScript; `command_grid_tests` holds the two to the same output.
///
/// The mode decides the sandbox as Electron's host does: Ask and Plan read only, Accept edits and Approve each write
/// in the folder, Auto runs as the person (`bypassPermissions`, which `--restricted` refuses, so Auto drops it).
pub fn print_args(o: &PrintOptions) -> Result<Vec<String>, String> {
    let auto = o.mode == Mode::Auto;
    let editing = !matches!(o.mode, Mode::Ask | Mode::Plan);
    let ask_each = o.mode == Mode::ApproveEach;
    if ask_each && o.bridge.is_none() {
        return Err("Approve each on Claude Code asks through Locust's permission host, which this run does not have.".into());
    }
    let mut args: Vec<String> = Vec::new();
    if !auto {
        args.push("--restricted".into());
    }
    args.extend(
        ["--print", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--permission-mode"]
            .map(String::from),
    );
    args.push(if auto { "bypassPermissions" } else if ask_each { "default" } else if editing { "acceptEdits" } else { "default" }.into());
    args.push("--tools".into());
    args.push(if editing { EDIT_TOOLS } else { ASK_TOOLS }.into());
    if !auto {
        args.extend(connector_rules(o.connectors, o.browser.is_some()));
    }
    if let Some(settings) = o.guard_settings {
        args.extend(["--settings".into(), require_text(settings, "Command guard settings")?.to_owned()]);
    }
    // One --mcp-config naming every server file, so the bridge is never lost to a later one.
    let bridge = if auto { None } else { o.bridge };
    let mut files: Vec<String> = Vec::new();
    if let Some(b) = bridge {
        files.push(require_text(b, "Permission bridge config")?.to_owned());
    }
    if let Some(b) = o.browser {
        files.push(require_text(b, "Browser MCP config")?.to_owned());
    }
    if !files.is_empty() {
        args.push("--mcp-config".into());
        args.extend(files);
    }
    if bridge.is_some() {
        args.extend(["--permission-prompt-tool".into(), BRIDGE_TOOL.into()]);
    }
    if !o.account_connectors {
        args.push("--strict-mcp-config".into());
    }
    for dir in o.skills {
        let folder = require_text(dir, "Skill folder")?;
        args.extend(["--plugin-dir".into(), folder.to_owned()]);
        if !auto {
            args.extend(["--add-dir".into(), folder.to_owned()]);
        }
    }
    if !auto {
        for folder in o.references {
            args.extend(["--add-dir".into(), require_text(folder, "Reference folder")?.to_owned()]);
        }
        if editing && !o.references.is_empty() {
            args.push("--disallowedTools".into());
            for folder in o.references {
                args.push(reference_deny_rule(folder)?);
            }
        }
    }
    if let Some(model) = o.model {
        args.extend(["--model".into(), require_text(model, "Model")?.to_owned()]);
    }
    if let Some(effort) = o.effort {
        args.extend(["--effort".into(), require_effort(effort)?.to_owned()]);
    }
    if let Some(session) = o.resume {
        args.extend(["--resume".into(), require_text(session, "Session id")?.to_owned()]);
        if o.fork {
            args.push("--fork-session".into());
        }
    }
    super::safety::assert_safe_runtime_command("claude", &args, Some(o.mode.sandbox()))?;
    Ok(args)
}

/// commands.ts createClaudeContextCommand: Claude Code's free `/context` for a turn's session, on that turn's own
/// command line, so the breakdown counts the tools, connectors, settings, plugins, references, model and approval bridge
/// the turn had. Only the transport and session persistence differ; a launch that is lost, cut short, not Claude's, or
/// has slash commands turned off is refused rather than guessed at.
pub fn context_args(runtime: &str, turn_args: &[String], session_id: &str) -> Result<Vec<String>, String> {
    if runtime != "claude"
        || !turn_args.iter().any(|a| a == "--print")
        || turn_args.iter().any(|a| a == "--disable-slash-commands")
        || turn_args.iter().any(|a| a.contains('…'))
    {
        return Err("The Claude launch flags are unavailable.".into());
    }
    if !(1..=128).contains(&session_id.len())
        || !session_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("The Claude session is unavailable.".into());
    }
    let mut args = Vec::new();
    let mut each = turn_args.iter();
    while let Some(arg) = each.next() {
        if matches!(arg.as_str(), "--input-format" | "--output-format" | "--resume") {
            each.next();
            continue;
        }
        if matches!(arg.as_str(), "--verbose" | "--include-partial-messages" | "--fork-session" | "--no-session-persistence") {
            continue;
        }
        args.push(arg.clone());
    }
    args.extend(["--output-format", "json", "--no-session-persistence", "--resume", session_id, "--", "/context"].map(String::from));
    Ok(args)
}

/// The `/context` run as a command: the turn's program, folder and variables, with `context_args`. It takes nothing on
/// stdin (the spec's `stdin: "none"`); the caller spawns it with a closed input.
pub fn context_command(turn: &Command, session_id: &str) -> Result<Command, String> {
    let program = Path::new(turn.get_program());
    let runtime = program.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let args: Vec<String> = turn.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    let args = context_args(runtime, &args, session_id)?;
    let mut out = Command::new(turn.get_program());
    out.args(args);
    if let Some(dir) = turn.get_current_dir() {
        out.current_dir(dir);
    }
    for (k, v) in turn.get_envs() {
        match v {
            Some(v) => out.env(k, v),
            None => out.env_remove(k),
        };
    }
    out.stdin(Stdio::null());
    Ok(out)
}

/// Ask's command line with nothing else: read only, no bridge, no guard. The context meter's probe and the few
/// callers that start Claude outside a turn use it.
pub fn ask_args(model: Option<&str>) -> Vec<String> {
    let model = model.map(str::trim).filter(|m| !m.is_empty());
    print_args(&PrintOptions { mode: Mode::Ask, model, ..Default::default() })
        .expect("Ask without a bridge always builds")
}

/// Claude Code with no arguments yet: a turn adds `print_args` once the bridge, browser and guard files exist.
pub fn bare_command(folder: &Path) -> io::Result<Command> {
    let mut c = Command::new(super::discovery::find_named("claude")?);
    c.current_dir(folder);
    // Never pass Locust API keys; Claude Code reads its own login.
    c.env_remove("ANTHROPIC_API_KEY");
    c.env_remove("CLAUDE_API_KEY");
    Ok(c)
}

pub fn command(folder: &Path, model: Option<&str>) -> io::Result<Command> {
    Ok(command_at(super::discovery::find_named("claude")?, folder, model))
}

/// [`command`] for a known program, so a test can read what it sets without Claude Code installed.
pub(crate) fn command_at(program: std::path::PathBuf, folder: &Path, model: Option<&str>) -> Command {
    let mut c = Command::new(program);
    c.args(ask_args(model));
    c.current_dir(folder);
    // Never pass Locust API keys; Claude Code reads its own login.
    c.env_remove("ANTHROPIC_API_KEY");
    c.env_remove("CLAUDE_API_KEY");
    set_run_env(&mut c);
    c
}

/// What every Claude Code run Locust starts sets, as Electron's `CLAUDE_RUN_ENV` (packages/runtime-adapters/src/commands.ts,
/// 15 Nov 2026): background tasks off. A background job dies with the run, so an agent that waits for one has lost it,
/// and a resumed session is handed the dead job's notice, which cut two Opus runs off on 15 Nov. With the switch, a
/// helper or a command runs in the foreground, where the run can see it finish. Claude Code's own `--bg` sessions
/// ([`bare_command`]'s other callers) are a separate path and do not get it.
pub const RUN_ENV: &[(&str, &str)] = &[("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1")];

/// Sets [`RUN_ENV`] on a Claude Code run.
pub fn set_run_env(c: &mut Command) {
    for (key, value) in RUN_ENV {
        c.env(key, value);
    }
}

/// Parent Claude Code session variables never handed on to a `--bg` child.
const PARENT_SESSION_VARIABLES: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_BRIDGE_SESSION_ID",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_EXECPATH",
];

/// Output of a one-shot `claude --bg` / `claude agents --json` / `claude stop` command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundStartResult {
    pub id: String,
    pub sentence: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundStartError {
    pub message: String,
    pub needs_trust: bool,
}

pub fn prepare_background_command(cmd: &mut Command, folder: &Path, args: &[String]) {
    cmd.args(args)
        .current_dir(folder)
        .stdin(Stdio::null())
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("CLAUDE_API_KEY");
    for key in PARENT_SESSION_VARIABLES {
        cmd.env_remove(key);
    }
}

/// Runs a one-shot Claude Code CLI command (`--bg`, `agents --json --all`, `stop <id>`) with parent session vars stripped.
pub fn run_background_cli(
    mut cmd: Command,
    folder: &Path,
    args: &[String],
) -> crate::claude_background::ClaudeCliOutput {
    prepare_background_command(&mut cmd, folder, args);
    match cmd.output() {
        Ok(out) => crate::claude_background::ClaudeCliOutput {
            code: out.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Err(err) => crate::claude_background::ClaudeCliOutput {
            code: 127,
            stdout: String::new(),
            stderr: err.to_string(),
        },
    }
}

pub fn start_background_with(
    mut cmd: Command,
    folder: &Path,
    options: &crate::claude_background::BackgroundArgsOptions<'_>,
) -> Result<BackgroundStartResult, BackgroundStartError> {
    let args = crate::claude_background::background_args(options).map_err(|message| {
        BackgroundStartError {
            message,
            needs_trust: false,
        }
    })?;
    prepare_background_command(&mut cmd, folder, &args);
    let output = cmd.output().map_err(|e| BackgroundStartError {
        message: e.to_string(),
        needs_trust: false,
    })?;
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if crate::claude_background::needs_trust(&said) {
        return Err(BackgroundStartError {
            message: "Claude Code has not been told to trust this folder, so it will not work here in the background yet. Open Claude Code in this folder once, answer its question, then send again.".to_string(),
            needs_trust: true,
        });
    }
    let id = crate::claude_background::background_id_from(&said);
    if !output.status.success() || id.is_none() {
        let line = said
            .lines()
            .map(str::trim)
            .find(|part| !part.is_empty())
            .map(|part| &part[..part.len().min(300)]);
        let message = match line {
            Some(l) => format!("Claude Code did not start it in the background: {l}"),
            None => "Claude Code did not start it in the background.".to_string(),
        };
        return Err(BackgroundStartError {
            message,
            needs_trust: false,
        });
    }
    Ok(BackgroundStartResult {
        id: id.unwrap(),
        sentence: crate::claude_background::background_sentence(
            crate::claude_background::BackgroundState::Working,
            None,
        ),
    })
}

pub fn poll_background_with(
    watch: &mut crate::claude_background::BackgroundWatch,
    mut cmd: Command,
    folder: &Path,
) -> Result<crate::claude_background::WatchTickOutcome, String> {
    watch.begin_tick();
    let args = vec![
        "agents".to_string(),
        "--json".to_string(),
        "--all".to_string(),
    ];
    prepare_background_command(&mut cmd, folder, &args);
    let parsed = cmd.output().ok().and_then(|out| {
        if out.status.success() {
            crate::claude_background::parse_background_agents(&String::from_utf8_lossy(&out.stdout))
        } else {
            None
        }
    });
    let outcome = watch.finish_tick(parsed.as_deref());
    if parsed.is_none() {
        return Err("claude agents gave no answer".to_string());
    }
    Ok(outcome)
}

impl Claude {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Self::spawn_with(command, Limits::default())
    }

    /// process-runner.ts start, for Claude Code's stream-json transport: the allowlisted environment with the
    /// command's own variables on top, stderr captured and bounded, stdout read as records within their caps (a record
    /// too large is skipped for a stand-in and the run carries on), and at most `max_queued_records` waiting unread.
    pub fn spawn_with(command: &mut Command, limits: Limits) -> io::Result<Self> {
        records::keep_allowlisted_environment(command);
        let mut child = super::process::spawn(command, Stdio::piped(), Stdio::piped())
            .map_err(|e| records::launch_error(&e))?;
        // spawn configured a piped stdin; no prior take occurs.
        let mut stdin = child
            .stdin
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let stderr = records::capture_stderr(
            child
                .stderr
                .take()
                // Invariant: spawn configured this pipe and it has not been taken.
                .expect("spawn configured this pipe and it has not been taken"),
            limits.max_stderr_bytes,
        );
        let control = Arc::new(Control::default());
        let pid = child.id();
        control.watch(pid);
        let (input, writes) = mpsc::sync_channel::<StdinMsg>(256);
        let input_failed = Arc::new(AtomicBool::new(false));
        let writer_failed = input_failed.clone();
        std::thread::spawn(move || {
            for msg in writes {
                match msg {
                    StdinMsg::Bytes(bytes) => {
                        // inputDeliveryFailed: the run is ended, and not as the person's cancellation.
                        if stdin.write_all(&bytes).and_then(|_| stdin.flush()).is_err() {
                            writer_failed.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                    StdinMsg::Close => break,
                }
            }
            // Dropping stdin closes the pipe (turn result / stop).
        });
        let approval_lifecycle: ApprovalLifecycle = Arc::new(Mutex::new(None));
        let lifecycle = approval_lifecycle.clone();
        // spawn configured a piped stdout; no prior take occurs.
        let mut output = child
            .stdout
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let (tx, lines) = mpsc::sync_channel(limits.max_queued_records);
        let reader_control = control.clone();
        let dropped = Arc::new(AtomicUsize::new(0));
        let output_limit = Arc::new(AtomicBool::new(false));
        let (reader_dropped, reader_limit) = (dropped.clone(), output_limit.clone());
        std::thread::spawn(move || {
            let mut split = Lines::new(&limits);
            let mut bytes = vec![0u8; 64 * 1024];
            loop {
                let (pieces, ended) = match output.read(&mut bytes) {
                    Ok(0) => (split.end(), true),
                    Ok(n) => (split.feed(&bytes[..n]), false),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        let _ = tx.try_send(Err(e));
                        break;
                    }
                };
                reader_dropped.store(split.dropped, Ordering::Relaxed);
                let mut values: Vec<io::Result<Value>> = pieces
                    .into_iter()
                    .map(|piece| match piece {
                        // A line that is not a record is said and read past, as claude-events.ts does, not the end of the run.
                        Piece::Record(raw) => Ok(records::claude_record(&raw)),
                        // A stand-in in the dropped record's place: its size, type and calls, none of its content.
                        Piece::Dropped { head, bytes, .. } => Ok(records::stand_in(&head, bytes)),
                    })
                    .collect();
                if ended {
                    values.push(Err(err("Claude stdout closed before the turn completed")));
                }
                for value in values {
                    let terminal = value.is_err();
                    if (terminal || value.as_ref().is_ok_and(ends_turn))
                        && let Some((channel, mission)) = crate::invariants::lock(&lifecycle).as_ref()
                    {
                        channel.release(mission);
                    }
                    match tx.try_send(value) {
                        Ok(()) => {}
                        Err(mpsc::TrySendError::Full(_)) => {
                            // exceedOutputLimit: an unknown number of records would be lost, so the run stops and says so.
                            reader_limit.store(true, Ordering::Relaxed);
                            reader_control.fail(records::OUTPUT_LIMIT_MESSAGE);
                            return;
                        }
                        Err(mpsc::TrySendError::Disconnected(_)) => return,
                    }
                    if terminal {
                        return;
                    }
                }
            }
        });
        Ok(Self {
            child,
            input,
            control,
            lines,
            normal: ClaudeNormalizer::default(),
            queued: VecDeque::new(),
            stopping: None,
            user_stopped: false,
            cancel: None,
            limits,
            stderr,
            dropped,
            output_limit,
            input_failed,
            prompt: String::new(),
            result_at: None,
            ended_after_result: false,
            stdin_open: true,
            exited: false,
            exit_code: None,
            approvals: None,
            mission_id: String::new(),
            run_id: String::new(),
            folder: PathBuf::new(),
            mode: Mode::Ask,
            approval_lifecycle,
        })
    }

    /// A turn's start: one already stopped is never launched (process-runner.ts abortBeforeLaunchError), and the
    /// stop flag rides with the run.
    pub fn launch(command: &mut Command, cancel: &Arc<AtomicBool>) -> io::Result<Self> {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Runtime process was cancelled before launch"));
        }
        Ok(Self::spawn(command)?.with_cancel(cancel.clone()))
    }

    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    fn cancelled(&self) -> bool {
        crate::run_cli::interrupted()
            || self
                .cancel
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
    }

    fn send(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        if !self.stdin_open {
            return Err(err("Claude stdin is closed"));
        }
        match self.input.try_send(StdinMsg::Bytes(bytes)) {
            Ok(()) => Ok(()),
            Err(e) => {
                let message = format!("Claude stdin queue unavailable; terminating run: {e}");
                self.control.fail(&message);
                Err(err(message))
            }
        }
    }

    fn close_stdin(&mut self) {
        if !self.stdin_open {
            return;
        }
        self.stdin_open = false;
        let _ = self.input.try_send(StdinMsg::Close);
    }

    fn receive(&self) -> io::Result<Option<Value>> {
        if let Some(message) = self.control.failure() {
            return Err(err(message));
        }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Claude transport disconnected")),
        }
    }

    fn transport_failure(&mut self, message: &str) -> Event {
        self.queued.clear();
        let diagnostic = self.normal.diagnostic("native.transport", message);
        let terminal = self.settle(message);
        self.queued.push_back(terminal);
        diagnostic
    }

    /// The run's terminal event, with what the process itself came to (RuntimeProcessCompletion).
    fn settle(&mut self, message: &str) -> Event {
        let (stderr, stderr_truncated) = crate::invariants::lock(&self.stderr).text(&self.prompt);
        let e = &mut self.normal.evidence;
        e.exit_code = self.exit_code;
        e.stderr = stderr;
        e.stderr_truncated = stderr_truncated;
        e.oversized_records_dropped = self.dropped.load(Ordering::Relaxed);
        e.output_limit_exceeded = self.output_limit.load(Ordering::Relaxed);
        e.input_delivery_failed = self.input_failed.load(Ordering::Relaxed);
        e.ended_after_result = self.ended_after_result;
        e.forced_termination_attempted =
            Some(self.stopping.is_some_and(|s| s.forced.is_some()) || (self.stopping.is_some() && self.control.forced()));
        let cancelled = self.user_stopped || self.cancelled();
        self.normal.finish(cancelled, message)
    }

    /// requestTermination: the group's SIGINT first, and the grace period for it; the person's stop is a cancellation,
    /// the host's (after a result, or for input it could not deliver) is not.
    fn terminate(&mut self, by_person: bool) {
        self.user_stopped |= by_person;
        if self.stopping.is_some() || self.exited {
            return;
        }
        let now = Instant::now();
        self.stopping = Some(Stopping::new(now));
        self.control.cancel_at(now + self.limits.cancellation_grace);
        interrupt_process(self.child.id());
        self.close_stdin();
    }

    fn handle_record(&mut self, v: Value) -> io::Result<()> {
        if v["type"].as_str() == Some("control_request") {
            if let Some(request_id) = v["request_id"].as_str() {
                let behavior = self.answer_control(&v, request_id)?;
                let reply = json!({
                    "type": "control_response",
                    "response": {
                        "subtype": "success",
                        "request_id": request_id,
                        "response": behavior
                    }
                });
                let mut bytes = serde_json::to_vec(&reply)?;
                bytes.push(b'\n');
                let _ = self.send(bytes);
            }
            self.queued.extend(self.normal.accept(&v));
            return Ok(());
        }
        let is_result = ends_turn(&v);
        self.queued.extend(self.normal.accept(&v));
        if is_result && self.stdin_open {
            // process-runner.ts closeInputAfterResult: input closes on the turn's result (even one too large to carry)
            // so Claude exits, and a run still alive `linger_after_result` later is ended.
            self.close_stdin();
            self.result_at = Some(Instant::now());
        }
        Ok(())
    }

    fn answer_control(&mut self, v: &Value, request_id: &str) -> io::Result<Value> {
        let request = &v["request"];
        let subtype = request["subtype"].as_str().unwrap_or("");
        if subtype != "can_use_tool" {
            return Ok(json!({
                "behavior": "deny",
                "message": "Native declined an unsupported Claude control request."
            }));
        }
        let tool_name = request["tool_name"].as_str().unwrap_or("tool");
        let input = if request["input"].is_null() {
            json!({})
        } else {
            request["input"].clone()
        };
        if self.mode != Mode::ApproveEach || self.approvals.is_none() {
            let message = "Native declines approvals without an interactive Approve each host.";
            self.queued
                .push_back(self.normal.diagnostic("claude.approval_declined", message));
            return Ok(json!({
                "behavior": "deny",
                "message": message
            }));
        }
        let channel = self
            .approvals
            .clone()
            .ok_or_else(|| err("Approval host is unavailable"))?;
        let mission = self.mission_id.clone();
        let run = self.run_id.clone();
        let cwd = self.folder.to_string_lossy().into_owned();
        let cancel = self.cancel.clone();
        let is_cancelled = move || {
            crate::run_cli::interrupted()
                || cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
        };
        match channel.ask_claude(
            &run,
            &mission,
            request_id,
            tool_name,
            &input,
            &cwd,
            &is_cancelled,
        ) {
            Ok(behavior) => Ok(behavior),
            Err(message) => {
                self.queued
                    .push_back(self.normal.diagnostic("claude.approval", &message));
                Ok(json!({"behavior":"deny","message": message}))
            }
        }
    }

    fn poll_exit(&mut self) {
        if self.exited {
            return;
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.exited = true;
            self.exit_code = status.code();
            self.close_stdin();
        }
    }
}

/// The turn's result, or the stand-in for one too large to carry: either ends the turn.
fn ends_turn(v: &Value) -> bool {
    v["type"] == "result" || (v["type"] == "locust.oversized" && v["recordType"] == "result")
}

impl Runtime for Claude {
    fn learned_commands(&mut self) -> Option<Vec<crate::runtime_commands::CommandInfo>> {
        self.normal.take_commands()
    }
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        if turn.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        *crate::invariants::lock(&self.approval_lifecycle) = turn
            .approvals
            .as_ref()
            .map(|ch| (ch.clone(), turn.mission_id.clone()));
        self.approvals = turn.approvals.clone();
        self.mission_id = turn.mission_id.clone();
        self.run_id = turn.run_id.clone();
        self.folder = turn.folder.clone();
        self.mode = turn.mode;
        self.prompt = turn.prompt.clone();
        self.send(user_line(&turn.prompt))
    }

    /// A stream-json run takes another user message while its turn runs (process-runner.ts `send`), until the turn's
    /// result closes its input.
    fn can_steer(&self) -> bool {
        true
    }

    fn steer(&mut self, text: &str) -> bool {
        self.stdin_open && self.stopping.is_none() && !text.trim().is_empty() && self.send(user_line(text)).is_ok()
    }

    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended {
            return Ok(self.queued.pop_front());
        }
        if let Some(message) = self.control.failure() {
            // The queue overflowed: what reached it before is still the run's, and is read before the run stops.
            if self.output_limit.load(Ordering::Relaxed)
                && let Ok(Ok(v)) = self.lines.try_recv()
            {
                if let Err(e) = self.handle_record(v) {
                    return Ok(Some(self.transport_failure(&e.to_string())));
                }
                if let Some(e) = self.queued.pop_front() {
                    return Ok(Some(e));
                }
                return Ok(None);
            }
            return Ok(Some(self.transport_failure(&message)));
        }
        self.poll_exit();
        if !self.exited {
            if self.input_failed.load(Ordering::Relaxed) {
                self.terminate(false);
            }
            if self.stopping.is_none()
                && self.result_at.is_some_and(|at| at.elapsed() >= self.limits.linger_after_result)
            {
                self.ended_after_result = true;
                self.terminate(false);
            }
            let limits = self.limits;
            match self.stopping.as_mut().map(|s| s.due(Instant::now(), &limits)) {
                // Force needs nothing here: the watchdog `terminate` armed (`control.cancel_at`) ends the tree at that
                // same moment, polled or not; `due` has marked it forced for the record.
                Some(Termination::Unconfirmed) => {
                    self.queued.clear();
                    self.normal.evidence.termination_unconfirmed = true;
                    return Ok(Some(self.settle("Claude Code did not confirm it had ended.")));
                }
                _ => {}
            }
        }
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        if self.exited && self.queued.is_empty() && !self.normal.ended {
            // What the process wrote before it ended is read first; its pipe closing (or staying held by something it
            // left behind, past a moment) is the end.
            if let Ok(Ok(v)) = self.lines.recv_timeout(Duration::from_millis(200)) {
                if let Err(e) = self.handle_record(v) {
                    return Ok(Some(self.transport_failure(&e.to_string())));
                }
                return Ok(self.queued.pop_front());
            }
            let message = match self.exit_code {
                Some(0) => String::new(),
                Some(code) => format!("Claude Code exited with code {code}."),
                None => "Claude Code ended without an exit code.".into(),
            };
            return Ok(Some(self.settle(&message)));
        }
        match self.receive() {
            Ok(Some(v)) => {
                if let Err(e) = self.handle_record(v) {
                    return Ok(Some(self.transport_failure(&e.to_string())));
                }
                Ok(self.queued.pop_front())
            }
            Ok(None) => Ok(None),
            Err(e) => {
                // Claude Code closes stdout as it exits. After the turn's result that is the end, not a transport
                // failure (seen live on 2.1.322: a diagnostic row on a turn that completed); give the exit a moment
                // to be seen so the run keeps its code.
                let until = Instant::now() + Duration::from_secs(2);
                self.poll_exit();
                while !self.exited && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(10));
                    self.poll_exit();
                }
                if self.exited || self.normal.saw_result() {
                    Ok(Some(self.settle(&e.to_string())))
                } else {
                    Ok(Some(self.transport_failure(&e.to_string())))
                }
            }
        }
    }

    fn stop(&mut self) -> io::Result<()> {
        // process-runner.ts:killProcessTree SIGINT, then SIGKILL after grace.
        self.terminate(true);
        Ok(())
    }
}

impl Drop for Claude {
    fn drop(&mut self) {
        self.control.close_and_kill(self.child.id());
        super::process::kill_reap(&mut self.child);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn ask_args_match_read_only_print_command() {
        let args = ask_args(Some("haiku"));
        assert!(args.contains(&"--restricted".into()));
        assert!(args.windows(2).any(|w| w == ["--input-format", "stream-json"]));
        assert!(args.windows(2).any(|w| w == ["--output-format", "stream-json"]));
        // Claude Code 2.1.322 lists `manual` and still takes `default`, which Electron sends.
        assert!(args.windows(2).any(|w| w == ["--permission-mode", "default"]));
        assert!(args.windows(2).any(|w| w == ["--tools", ASK_TOOLS]));
        assert!(args.windows(2).any(|w| w == ["--model", "haiku"]));
        assert!(args.contains(&"--strict-mcp-config".into()));
        assert!(!args.iter().any(|a| a.contains("bypassPermissions")));
    }

    #[test]
    fn the_mode_decides_tools_and_permissions_and_keeps_the_guard() {
        let guard_settings = "/some folder/command-guard.json";
        let pair = |a: &[String], flag: &str| a[a.iter().position(|s| s == flag).unwrap() + 1].clone();
        for (mode, permission, tools) in [
            (Mode::Ask, "default", ASK_TOOLS),
            (Mode::Plan, "default", ASK_TOOLS),
            (Mode::AcceptEdits, "acceptEdits", EDIT_TOOLS),
            (Mode::ApproveEach, "default", EDIT_TOOLS),
            (Mode::Auto, "bypassPermissions", EDIT_TOOLS),
        ] {
            let a = print_args(&PrintOptions {
                mode,
                bridge: Some("/b/mcp.json"),
                guard_settings: Some(guard_settings),
                ..Default::default()
            })
            .unwrap();
            assert_eq!(pair(&a, "--permission-mode"), permission, "{mode:?}");
            assert_eq!(pair(&a, "--tools"), tools, "{mode:?}");
            assert_eq!(pair(&a, "--settings"), guard_settings, "{mode:?}");
            // bypassPermissions cannot be combined with --restricted, and Auto asks nothing, so it has no bridge.
            assert_eq!(a.contains(&"--restricted".into()), mode != Mode::Auto, "{mode:?}");
            assert_eq!(a.contains(&"--permission-prompt-tool".into()), mode != Mode::Auto, "{mode:?}");
        }
    }

    #[test]
    fn approve_each_needs_the_bridge() {
        assert!(print_args(&PrintOptions { mode: Mode::ApproveEach, ..Default::default() }).is_err());
    }

    #[test]
    fn one_mcp_config_names_bridge_and_browser() {
        let a = print_args(&PrintOptions { mode: Mode::AcceptEdits, bridge: Some("/b"), browser: Some("/w"), ..Default::default() }).unwrap();
        assert_eq!(a.iter().filter(|s| *s == "--mcp-config").count(), 1);
        assert!(a.windows(3).any(|w| w == ["--mcp-config", "/b", "/w"]));
        assert!(a.windows(2).any(|w| w == ["--allowedTools", "mcp__locust_browser__*"]));
    }

    // connectors.test.ts's live tool names
    #[test]
    fn connector_prefixes_follow_claude_code() {
        assert_eq!(tool_prefix_for("claude.ai Anthropic Economic Index"), "mcp__claude_ai_Anthropic_Economic_Index__");
        assert_eq!(tool_prefix_for("robinhood-trading"), "mcp__robinhood-trading__");
        assert_eq!(tool_prefix_for("Robinhood"), "mcp__Robinhood__");
        assert_eq!(allow_rule_for("..."), None);
        assert_eq!(allow_rule_for("claude.ai Notion").as_deref(), Some("mcp__claude_ai_Notion__*"));
    }

    #[test]
    fn reference_rules_name_one_literal_folder() {
        assert_eq!(reference_deny_rule("/work/reference/").unwrap(), "Edit(//work/reference/**)");
        assert_eq!(reference_deny_rule("C:\\Users\\me\\ref").unwrap(), "Edit(//c/Users/me/ref/**)");
        assert!(reference_deny_rule("/work/a*b").is_err());
        assert!(reference_deny_rule("relative").is_err());
    }

    #[test]
    fn a_result_with_an_unanswered_request_withdraws_the_card() {
        let mut command = Command::new("python3");
        command.args(["-c", r#"import json,time
print(json.dumps({"type":"control_request","request_id":"dead","request":{"subtype":"can_use_tool","tool_name":"Write","input":{}}}),flush=True)
time.sleep(.1)
print(json.dumps({"type":"result","subtype":"success","result":"ended"}),flush=True)
time.sleep(.1)
"#]);
        let channel = Channel::new();
        let mut runtime = Claude::spawn(&mut command).unwrap();
        let turn = Turn {
            prompt: "hello".into(),
            folder: PathBuf::from("/tmp"),
            model: None,
            effort: None,
            resume: None,
            mode: Mode::ApproveEach,
            approvals: Some(channel.clone()),
            mission_id: "dead".into(),
            run_id: "r".into(),
        };
        runtime.start(&turn).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !runtime.normal.ended {
            runtime.next_event().unwrap();
            assert!(Instant::now() < deadline);
        }
        assert!(channel.waiting().is_empty());
        assert!(channel.take_recorded().is_empty());
    }

    #[test]
    fn stdout_closing_after_the_result_is_no_failure() {
        let mut command = Command::new("python3");
        command.args(["-c", r#"import json,os,sys,time
print(json.dumps({"type":"system","subtype":"init","session_id":"s"}),flush=True)
print(json.dumps({"type":"result","subtype":"success","result":"done"}),flush=True)
os.close(1)
time.sleep(.3)
"#]);
        let mut runtime = Claude::spawn(&mut command).unwrap();
        let turn = Turn {
            prompt: "hello".into(),
            folder: PathBuf::from("/tmp"),
            model: None,
            effort: None,
            resume: None,
            mode: Mode::Ask,
            approvals: None,
            mission_id: "eof".into(),
            run_id: "r".into(),
        };
        runtime.start(&turn).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut kinds = Vec::new();
        while !runtime.normal.ended || !runtime.queued.is_empty() {
            if let Some(e) = runtime.next_event().unwrap() {
                kinds.push(e.kind.clone());
            }
            assert!(Instant::now() < deadline);
        }
        assert!(!kinds.iter().any(|k| k == "adapter.diagnostic"), "{kinds:?}");
        assert_eq!(kinds.last().map(String::as_str), Some("run.completed"), "{kinds:?}");
    }

    #[test]
    fn stop_kills_a_child_that_ignores_stdin() {
        let mut command = Command::new("python3");
        command.args(["-c", "import time; time.sleep(60)"]);
        let mut runtime = Claude::spawn(&mut command).unwrap();
        let start = Instant::now();
        runtime.stop().unwrap();
        assert!(start.elapsed() < Duration::from_millis(250));
        loop {
            if runtime.child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(6));
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
#[path = "references_never_widen_tests.rs"]
mod references_never_widen_tests;

#[cfg(test)]
#[path = "a_context_inspection_tests.rs"]
mod a_context_inspection_tests;

#[cfg(test)]
#[path = "permission_bridge_argv_tests.rs"]
mod permission_bridge_argv_tests;

#[cfg(test)]
#[path = "connector_allow_rules_tests.rs"]
mod connector_allow_rules_tests;

#[cfg(all(test, unix))]
#[path = "claude_run_skill_lifetime_tests.rs"]
mod run_skill_lifetime_tests;
