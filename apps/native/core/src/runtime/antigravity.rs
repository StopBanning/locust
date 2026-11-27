//! Antigravity CLI (agy) driver. Ports createAgyAcpCommand + agy-acp-run.ts (27 Nov 2026), and createAgyPrintCommand +
//! agy-json stdin, the route kept for an agy without `acp`.
//!
//! `agy acp` (agy 1.5.18, MEASURED 2026-11-27 on this Mac; ~/work/scratch/agy-acp-probe/) is an ACP stdio server. In
//! its `default` mode it asks for every command and edit with `session/request_permission` (options `allow`/`deny`),
//! so an approval becomes a Locust card instead of `-p` deciding by itself; its `ask_question` reaches the client as an
//! `elicitation/create` form once forms are offered, and the same turn goes on with the answer (`-p` 1.4.2 answered
//! "User Skipped" itself). Modes are `default`, `accept-edits`, `auto`, `plan`; the model and effort are its `model` and
//! `thought_level` settings, the effort's choices listed per model. `session/load` continues an ACP session and a
//! conversation `-p` began alike.
use super::acp::{Acp, Profile};
use super::process::{Control, interrupt_process, kill_process};
use super::{Event, Runtime, Turn, agy_events::AgyNormalizer};
use crate::approval::Mode;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

const MAX_LINE: u64 = 8 * 1024 * 1024;

enum StdinMsg {
    Bytes(Vec<u8>),
    Close,
}

/// Antigravity's `-p` route: one stream-json turn on stdin (createAgyPrintCommand).
pub struct PrintAntigravity {
    child: Child,
    input: SyncSender<StdinMsg>,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    normal: AgyNormalizer,
    queued: VecDeque<Event>,
    stopping: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    stdin_open: bool,
    exited: bool,
    exit_code: Option<i32>,
}

fn err(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

pub fn sandbox_for(mode: Mode) -> &'static str {
    match mode {
        Mode::Auto => "full-access",
        Mode::AcceptEdits | Mode::ApproveEach => "workspace-write",
        Mode::Ask | Mode::Plan => "read-only",
    }
}

pub fn user_line(prompt: &str) -> Vec<u8> {
    let mut bytes = json!({
        "event": "user",
        "message": {"role":"user","content":[{"type":"text","text": prompt}]}
    })
    .to_string()
    .into_bytes();
    bytes.push(b'\n');
    bytes
}

pub const ANTIGRAVITY_TIER_NAMES: [(&str, &str); 3] = [
    ("flash", "Gemini 3.8 Flash"),
    ("pro", "Gemini Pro"),
    ("flash_lite", "Gemini Flash Lite"),
];

pub fn antigravity_tier_name(tier: &str) -> Option<&'static str> {
    ANTIGRAVITY_TIER_NAMES
        .iter()
        .find(|(id, _)| *id == tier)
        .map(|(_, name)| *name)
}

/// Maps an app tier (`flash`, `pro`, `flash_lite`) or a model id with an optional
/// `-(low|medium|high|max)` suffix to `(model, effort)` for `agy`.
/// A chosen effort wins; a bare `gemini-*` id defaults to `medium`.
pub fn agy_route(model: Option<&str>, effort: Option<&str>) -> (Option<String>, Option<String>) {
    let model = model.map(str::trim).filter(|m| !m.is_empty());
    let effort = effort.map(str::trim).filter(|e| !e.is_empty());
    let Some(model) = model else {
        return (None, effort.map(str::to_owned));
    };
    let (base, named) = match model {
        "flash" => ("gemini-3.8-flash", Some("medium")),
        "pro" => ("gemini-3.1-pro", Some("high")),
        "flash_lite" => ("gemini-3.8-flash", Some("low")),
        _ => {
            let split = ["low", "medium", "high", "max"]
                .into_iter()
                .find_map(|suffix| {
                    model
                        .strip_suffix(&format!("-{suffix}"))
                        .map(|b| (b, Some(suffix)))
                });
            split.unwrap_or((model, None))
        }
    };
    let chosen = effort
        .or(named)
        .or_else(|| base.starts_with("gemini-").then_some("medium"));
    (Some(base.to_owned()), chosen.map(str::to_owned))
}

pub fn print_args(
    mode: Mode,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
    references: &[&str],
    skills: Option<&str>,
) -> Vec<String> {
    let sandbox = sandbox_for(mode);
    let mut args = vec![
        "--output-format".into(),
        "stream-json".into(),
        "--input-format".into(),
        "stream-json".into(),
    ];
    // The run's skills, as a workspace folder agy reads `.agents/skills` from (run-skills.ts), in every mode.
    if let Some(dir) = skills {
        args.extend(["--add-dir".into(), dir.into()]);
    }
    if sandbox == "workspace-write" {
        args.push("--mode".into());
        args.push("accept-edits".into());
    }
    if sandbox == "full-access" {
        args.push("--dangerously-skip-permissions".into());
    }
    // Reference folders, read only (createAgyPrintCommand): elsewhere an extra root would also be writable.
    if sandbox == "read-only" {
        for folder in references {
            args.extend(["--add-dir".into(), (*folder).into()]);
        }
    }
    let (resolved_model, resolved_effort) = agy_route(model, effort);
    if let Some(model) = resolved_model {
        args.push("--model".into());
        args.push(model);
    }
    if let Some(effort) = resolved_effort {
        args.push("--effort".into());
        args.push(effort);
    }
    if let Some(resume) = resume.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--conversation".into());
        args.push(resume.into());
    }
    args.push("-p=".into());
    args
}

/// Whether `agy acp --help` names the Agent Client Protocol (commands.ts `detectSupportedFeatures`, antigravity).
pub fn speaks_acp(help: &str) -> bool {
    let words = help.to_lowercase();
    ["agent client protocol", "agent-client-protocol", "agent client-protocol", "agent-client protocol"].iter().any(|w| words.contains(w))
}

/// The binary as last looked at: its path, size and time. A changed agy is asked again.
type Stamp = (PathBuf, u64, Option<std::time::SystemTime>);
/// A few binaries at most (a person has one agy; tests make their own), each with what its help said.
static ACP_SEEN: std::sync::Mutex<Vec<(Stamp, bool)>> = std::sync::Mutex::new(Vec::new());

fn keep_acp_answer(stamp: Stamp, says: bool) {
    let mut seen = crate::invariants::lock(&ACP_SEEN);
    seen.retain(|(kept, _)| kept.0 != stamp.0);
    seen.push((stamp, says));
    let extra = seen.len().saturating_sub(16);
    seen.drain(..extra);
}

fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((path.to_path_buf(), meta.len(), meta.modified().ok()))
}

/// What a sweep read off `agy acp --help` (agents.rs), kept so a turn need not ask again: the answer takes three
/// seconds on this Mac.
pub fn remember_acp_help(path: &Path, help: &str) {
    if let Some(stamp) = stamp_of(path) {
        keep_acp_answer(stamp, speaks_acp(help));
    }
}

/// `agy acp --help`'s words, stdout and stderr together (agy prints its usage to stderr), within ten seconds.
fn acp_help(path: &Path) -> Option<String> {
    let mut child = Command::new(path).args(["acp", "--help"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let (mut out, mut errs) = (child.stdout.take()?, child.stderr.take()?);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut a, mut b) = (String::new(), String::new());
        let _ = out.read_to_string(&mut a);
        let _ = errs.read_to_string(&mut b);
        let _ = tx.send(format!("{a}\n{b}"));
    });
    let said = rx.recv_timeout(Duration::from_secs(10)).ok();
    if said.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    said
}

/// Whether this agy runs a turn over `agy acp`: what the last sweep read off this same binary, else asked now.
pub fn agy_speaks_acp(path: &Path) -> bool {
    let stamp = stamp_of(path);
    if let Some(stamp) = &stamp
        && let Some((_, says)) = crate::invariants::lock(&ACP_SEEN).iter().find(|(kept, _)| kept == stamp)
    {
        return *says;
    }
    let says = acp_help(path).is_some_and(|help| speaks_acp(&help));
    if let Some(stamp) = stamp {
        keep_acp_answer(stamp, says);
    }
    says
}

/// `agy acp`, with the folders `-p` was given: the skills folder in every mode, the reference folders only in a
/// read-only run (createAgyAcpCommand).
pub fn acp_args(mode: Mode, references: &[&str], skills: Option<&str>) -> Vec<String> {
    let mut args = vec!["acp".to_owned()];
    if let Some(dir) = skills {
        args.extend(["--add-dir".into(), dir.into()]);
    }
    if sandbox_for(mode) == "read-only" {
        for folder in references {
            args.extend(["--add-dir".into(), (*folder).into()]);
        }
    }
    args
}

/// Each Locust mode as agy's (agy-acp-run.ts `agyAcpSession`). Who answers is acp.rs's own rule for a named runtime:
/// Approve each a card, Auto allowed, the rest refused -- read-only for Ask and Plan, commands refused in Accept edits
/// as `-p --mode accept-edits` did.
pub fn acp_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Ask | Mode::ApproveEach => "default",
        Mode::Plan => "plan",
        Mode::AcceptEdits => "accept-edits",
        Mode::Auto => "auto",
    }
}

/// agy's id for a model kept as a name and an effort (agy-acp-run.ts `agyAcpModelId`): the bare id where agy lists it,
/// else the id with the effort on the end (`gpt-oss-120b-medium`), else what was asked.
pub fn acp_model_id(wanted: &str, effort: Option<&str>, offered: &[String]) -> String {
    if offered.is_empty() || offered.iter().any(|o| o == wanted) {
        return wanted.to_owned();
    }
    match effort.map(|e| format!("{wanted}-{e}")) {
        Some(whole) if offered.contains(&whole) => whole,
        _ => wanted.to_owned(),
    }
}

/// The settings one turn sets over `agy acp`: the model, then the mode (both must hold), then the effort where the model
/// lists it.
pub fn acp_profile(turn: &Turn) -> Profile {
    let (model, effort) = agy_route(turn.model.as_deref().filter(|m| *m != "account-default"), turn.effort.as_deref());
    let mut required = vec![];
    if let Some(model) = model {
        required.push(("model".to_owned(), model));
    }
    required.push(("mode".to_owned(), acp_mode(turn.mode).to_owned()));
    let kept = effort.clone();
    Profile {
        runtime: "antigravity".into(),
        required,
        strict: true,
        named: true,
        host_permissions: true,
        preferred: effort.map(|e| vec![("thought_level".to_owned(), e)]).unwrap_or_default(),
        resolve: Some(std::sync::Arc::new(move |config: &str, wanted: &str, offered: &[String]| {
            if config == "model" { acp_model_id(wanted, kept.as_deref(), offered) } else { wanted.to_owned() }
        }) as super::acp::ResolveConfig),
        ..Default::default()
    }
}

pub fn command(
    folder: &Path,
    mode: Mode,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
    references: &[&str],
    skills: Option<&str>,
) -> io::Result<Command> {
    // agy reads `--add-dir` against its own folder; the run's skills are elsewhere, so only an absolute one will do.
    if let Some(dir) = skills
        && !Path::new(dir).is_absolute()
    {
        return Err(err(format!("Skills folder must be an absolute path: {dir}")));
    }
    command_for(super::discovery::find_named("agy")?, folder, mode, model, effort, resume, references, skills)
}

// Tests take the `-p` route unless one opens the protocol's on its own thread: whether this machine's agy speaks it
// must never decide a test (CI has no agy; this Mac has 1.5.18).
#[cfg(any(test, feature = "test-hooks"))]
thread_local! {
    static ACP_ROUTE_IN_TESTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(any(test, feature = "test-hooks"))]
pub fn open_acp_route_in_tests(open: bool) {
    ACP_ROUTE_IN_TESTS.with(|c| c.set(open));
}
fn acp_route_open() -> bool {
    #[cfg(any(test, feature = "test-hooks"))]
    {
        ACP_ROUTE_IN_TESTS.with(std::cell::Cell::get)
    }
    #[cfg(not(any(test, feature = "test-hooks")))]
    {
        true
    }
}

/// `-p` cannot stop and ask: it would run Approve each as Edit without a word. Refused in words instead, as
/// codex-mission.ts does (27 Nov 2026, agy 1.5.18).
pub const NO_APPROVE_EACH_WITHOUT_ACP: &str =
    "Antigravity asks before each action only through agy acp, which this Antigravity CLI does not have. Run agy update in a terminal, or choose Edit.";

/// The command for this agy: over its protocol where it has one (the model, effort, mode and conversation then go there,
/// not on argv), else `-p`, which takes no Approve each.
#[allow(clippy::too_many_arguments)]
pub fn command_for(
    agy: PathBuf,
    folder: &Path,
    mode: Mode,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
    references: &[&str],
    skills: Option<&str>,
) -> io::Result<Command> {
    if acp_route_open() && agy_speaks_acp(&agy) {
        let mut c = Command::new(agy);
        c.args(acp_args(mode, references, skills)).current_dir(folder);
        return Ok(c);
    }
    if mode == Mode::ApproveEach {
        return Err(err(NO_APPROVE_EACH_WITHOUT_ACP));
    }
    let mut c = Command::new(agy);
    c.args(print_args(mode, model, effort, resume, references, skills))
        .current_dir(folder);
    Ok(c)
}

impl PrintAntigravity {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let mut child = super::process::spawn(command, Stdio::piped(), Stdio::null())?;
        // spawn configured a piped stdin; no prior take occurs.
        let mut stdin = child
            .stdin
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let control = Arc::new(Control::default());
        let pid = child.id();
        control.watch(pid);
        let (input, writes) = mpsc::sync_channel::<StdinMsg>(256);
        let writer_control = control.clone();
        std::thread::spawn(move || {
            for msg in writes {
                match msg {
                    StdinMsg::Bytes(bytes) => {
                        if let Err(e) = stdin.write_all(&bytes).and_then(|_| stdin.flush()) {
                            writer_control.fail(format!("Antigravity stdin write failed: {e}"));
                            break;
                        }
                    }
                    StdinMsg::Close => break,
                }
            }
        });
        // spawn configured a piped stdout; no prior take occurs.
        let output = child
            .stdout
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let (tx, lines) = mpsc::sync_channel(super::records::MAX_QUEUED_RECORDS);
        let reader_control = control.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                match super::process::read_line(&mut reader, &mut line) {
                    Ok(0) => break,
                    Ok(_) if line.len() as u64 > MAX_LINE => {
                        let _ = tx.try_send(Err(err("Antigravity stream-json line exceeds 8 MiB")));
                        break;
                    }
                    Ok(_) => {
                        let trimmed = if line.last() == Some(&b'\n') {
                            &line[..line.len() - 1]
                        } else {
                            &line[..]
                        };
                        let trimmed = if trimmed.last() == Some(&b'\r') {
                            &trimmed[..trimmed.len() - 1]
                        } else {
                            trimmed
                        };
                        if trimmed.is_empty() {
                            continue;
                        }
                        let value = serde_json::from_slice::<Value>(trimmed)
                            .map_err(|_| err("Antigravity sent malformed json"));
                        match tx.try_send(value) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                reader_control.fail("Antigravity notification queue overflow");
                                break;
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                    Err(e) => {
                        let _ = tx.try_send(Err(e));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            input,
            control,
            lines,
            normal: AgyNormalizer::default(),
            queued: VecDeque::new(),
            stopping: None,
            cancel: None,
            stdin_open: true,
            exited: false,
            exit_code: None,
        })
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
            return Err(err("Antigravity stdin is closed"));
        }
        self.input.try_send(StdinMsg::Bytes(bytes)).map_err(|e| {
            let m = format!("Antigravity stdin queue unavailable: {e}");
            self.control.fail(&m);
            err(m)
        })
    }
    fn close_stdin(&mut self) {
        if self.stdin_open {
            self.stdin_open = false;
            let _ = self.input.try_send(StdinMsg::Close);
        }
    }
    fn receive(&self) -> io::Result<Option<Value>> {
        if let Some(message) = self.control.failure() {
            return Err(err(message));
        }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Antigravity transport disconnected")),
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
    fn await_exit(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        while !self.exited && Instant::now() < deadline {
            self.poll_exit();
            if self.exited {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.poll_exit();
    }
}

impl Runtime for PrintAntigravity {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        if turn.mode == Mode::ApproveEach {
            return Err(err(NO_APPROVE_EACH_WITHOUT_ACP));
        }
        if turn.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        self.normal = AgyNormalizer::default().with_sandbox(sandbox_for(turn.mode));
        self.send(user_line(&turn.prompt))?;
        self.close_stdin();
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended {
            return Ok(self.queued.pop_front());
        }
        if let Some(message) = self.control.failure() {
            self.queued.clear();
            self.queued.extend(
                self.normal
                    .finish(self.stopping.is_some() || self.cancelled(), self.exit_code),
            );
            return Ok(Some(self.normal.diagnostic(
                "error",
                "native.transport",
                &message,
            )));
        }
        if self
            .stopping
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(5))
        {
            self.queued.clear();
            kill_process(self.child.id());
            self.queued.extend(self.normal.finish(true, self.exit_code));
            return Ok(self.queued.pop_front());
        }
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        self.poll_exit();
        match self.receive() {
            Ok(Some(v)) => {
                if v["event"].as_str() == Some("result") {
                    self.close_stdin();
                }
                self.queued.extend(self.normal.accept(&v));
                Ok(self.queued.pop_front())
            }
            Ok(None) => Ok(None),
            Err(_) => {
                while let Ok(Ok(v)) = self.lines.try_recv() {
                    self.queued.extend(self.normal.accept(&v));
                }
                if let Some(e) = self.queued.pop_front() {
                    return Ok(Some(e));
                }
                self.await_exit(Duration::from_millis(500));
                self.queued.extend(
                    self.normal
                        .finish(self.stopping.is_some() || self.cancelled(), self.exit_code),
                );
                Ok(self.queued.pop_front())
            }
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_some() {
            return Ok(());
        }
        let now = Instant::now();
        self.stopping = Some(now);
        self.control.cancel_at(now + Duration::from_secs(5));
        interrupt_process(self.child.id());
        self.close_stdin();
        Ok(())
    }
}
impl Drop for PrintAntigravity {
    fn drop(&mut self) {
        self.control.close_and_kill(self.child.id());
        super::process::kill_reap(&mut self.child);
    }
}

/// A turn over `agy acp`: argv begins with `acp` (command decided it).
fn uses_acp(command: &Command) -> bool {
    command.get_args().next().is_some_and(|a| a == "acp")
}

/// Antigravity over `agy acp`, or over `-p` where agy has no `acp` (as copilot.rs holds its two routes).
pub enum Antigravity {
    Print(Box<PrintAntigravity>),
    Acp(Box<Acp>),
}
impl Antigravity {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        if uses_acp(command) {
            // The turn's own settings are set at start, when the turn is known (acp_profile).
            Ok(Self::Acp(Box::new(Acp::spawn(command)?)))
        } else {
            Ok(Self::Print(Box::new(PrintAntigravity::spawn(command)?)))
        }
    }
    pub fn with_cancel(self, flag: Arc<AtomicBool>) -> Self {
        match self {
            Self::Print(p) => Self::Print(Box::new(p.with_cancel(flag))),
            Self::Acp(a) => Self::Acp(Box::new(a.with_cancel(flag))),
        }
    }
}
impl Runtime for Antigravity {
    /// Over ACP a message reaches it as its next prompt; `-p` takes none.
    fn can_steer(&self) -> bool {
        matches!(self, Self::Acp(_))
    }
    fn steer(&mut self, text: &str) -> bool {
        match self {
            Self::Print(_) => false,
            Self::Acp(a) => a.steer(text),
        }
    }
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        match self {
            Self::Print(p) => p.start(turn),
            Self::Acp(a) => {
                a.set_profile(acp_profile(turn));
                a.start(turn)
            }
        }
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        match self {
            Self::Print(p) => p.next_event(),
            Self::Acp(a) => a.next_event(),
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        match self {
            Self::Print(p) => p.stop(),
            Self::Acp(a) => a.stop(),
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    #[test]
    fn ask_is_read_only_without_skip() {
        let args = print_args(Mode::Ask, Some("gemini"), None, None, &[], None);
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
        assert!(args.windows(2).any(|w| w == ["--model", "gemini"]));
        assert!(args.contains(&"-p=".into()));
    }
    #[test]
    fn auto_skips_permissions() {
        let args = print_args(Mode::Auto, None, Some("low"), Some("c1"), &[], None);
        assert!(args.contains(&"--dangerously-skip-permissions".into()));
        assert!(args.windows(2).any(|w| w == ["--effort", "low"]));
        assert!(args.windows(2).any(|w| w == ["--conversation", "c1"]));
    }
}

#[cfg(test)]
#[path = "agy_events_case_tests.rs"]
mod agy_events_case_tests;

#[cfg(test)]
#[path = "agy_waiting_case_tests.rs"]
mod agy_waiting_case_tests;
