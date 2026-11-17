//! APPLE INTELLIGENCE, ON THIS MAC (17 Nov 2026). Colin, passing on a friend's question: "could you add Siri AI to
//! Locust?" Siri itself takes no messages from another app, but the model behind Apple Intelligence does: since macOS
//! 26 any app may ask it through Foundation Models, on the Mac itself, free, with no account and nothing sent away.
//!
//! The framework is Swift's alone, so a small helper (`helpers/apple-model/main.swift`, built by `build.rs` and
//! embedded here) asks it: one JSON request on stdin, the answer as JSON lines on stdout. The model reads about 8,000
//! tokens at a time, too few for a coding agent's instructions and tools (OpenCode's alone are ~13,000), so this runtime
//! only talks: no tools, Ask (and Plan) only. It keeps no session: every follow-up starts cold and is told what was said
//! (`carry_on::compose_cold_follow_up`), which is what a runtime without a thread id gets.
use super::process::{Control, interrupt_process, kill_process};
use super::{Event, Runtime, Turn};
use crate::approval::Mode;
use crate::runtime::events::{evidence_of, text};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

pub const APPLE_RUNTIME: &str = "apple";
pub const APPLE_DISPLAY_NAME: &str = "Apple Intelligence";
pub const APPLE_MODEL: &str = "apple-on-device";
pub const APPLE_MODEL_NAME: &str = "On-device model";
const MAX_LINE: u64 = 1024 * 1024;

/// What a person reads when the model is not there, by the reason Foundation Models gives.
pub fn unavailable_words(reason: &str) -> &'static str {
    match reason {
        "appleIntelligenceNotEnabled" => {
            "Apple Intelligence is turned off on this Mac. Turn it on in System Settings, under Apple Intelligence & Siri."
        }
        "modelNotReady" => "Apple's on-device model is still downloading. Try again once it is ready.",
        "deviceNotEligible" => "This Mac cannot run Apple Intelligence.",
        _ => "Apple's on-device model is not available on this Mac.",
    }
}

/// What a person reads when the model would not or could not answer, by the kind the helper names.
pub fn failure_words(kind: &str, reason: Option<&str>) -> String {
    match kind {
        "contextSizeExceeded" => "This is more than Apple's on-device model can read at once (about 8,000 tokens, with \
            what was said before). Start a new conversation, or carry on with another teammate."
            .into(),
        "guardrailViolation" => "Apple's on-device model would not answer this: its safety rules stopped it.".into(),
        "refusal" => "Apple's on-device model declined to answer this.".into(),
        "unsupportedLanguageOrLocale" => "Apple's on-device model does not read this language yet.".into(),
        "rateLimited" => "Apple's on-device model is busy. Try again in a moment.".into(),
        "assetsUnavailable" => unavailable_words("modelNotReady").into(),
        "unavailable" => unavailable_words(reason.unwrap_or_default()).into(),
        _ => "Apple's on-device model could not answer.".into(),
    }
}

/// What Apple's model is told before the conversation. Not Locust's usual brief: its rules for the tags an agent writes
/// (`<locust-ask>`, `<locust-memory>`, `<locust-next>`, shares, files) and its folder and roster sections cost much of
/// the 8,000 tokens this model reads, and a model this small copies the tags into its answers as words (measured
/// 17 Nov: asked what a git branch is, it answered and then wrote out a `<locust-ask>` with the template's own
/// placeholders). So: who it is, what it cannot do, the teammate's own instructions, then what was said.
pub fn brief(name: &str, role: &str, instructions: Option<&str>, words: &str) -> String {
    let mut out = format!(
        "You are {name}, a teammate in Locust{}. You run on Apple's on-device model, on this Mac. You can only talk: you \
         cannot read or change files, run commands, or open web pages, so never say you did. Answer plainly and briefly, \
         in the person's language.",
        if role.trim().is_empty() { String::new() } else { format!(" ({})", role.trim()) }
    );
    if let Some(own) = instructions.map(str::trim).filter(|i| !i.is_empty()) {
        out.push_str("\n\nYour instructions from the person:\n");
        out.push_str(own);
    }
    out.push_str("\n\n");
    out.push_str(words);
    out
}

/// The request the helper reads: the whole prompt Locust composed, which already carries the brief and what was said.
pub fn request(prompt: &str) -> Value {
    json!({ "prompt": prompt })
}

// ---- the helper, embedded -----------------------------------------------------------------------------------------

#[cfg(apple_helper)]
static HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/locust-apple-model"));

/// The helper on disk, written from the copy this build carries the first time it is needed (and again if what is
/// there is not byte for byte the same). None where this build carries none: not a Mac, or built without Swift.
#[cfg(apple_helper)]
pub fn helper_path() -> io::Result<std::path::PathBuf> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    let digest = Sha256::digest(HELPER);
    let short: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    let path = std::env::temp_dir().join(format!("locust-apple-model-{short}"));
    if std::fs::read(&path).is_ok_and(|on_disk| on_disk == HELPER) {
        return Ok(path);
    }
    let staged = path.with_extension(format!("{}.part", std::process::id()));
    {
        let mut file = std::fs::File::create(&staged)?;
        file.write_all(HELPER)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&staged, &path)?;
    Ok(path)
}

#[cfg(not(apple_helper))]
pub fn helper_path() -> io::Result<std::path::PathBuf> {
    Err(io::Error::other(unavailable_words("")))
}

/// What the helper's `--probe` said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    Available { context: u64 },
    Unavailable { reason: String },
}

/// Reads a `--probe` answer. Anything else (a helper that would not start, on a Mac too old for the framework, says
/// nothing JSON) is unavailable.
pub fn read_probe(stdout: &str) -> Probe {
    let value: Value = stdout.lines().find_map(|l| serde_json::from_str(l).ok()).unwrap_or(Value::Null);
    if value["available"] == true {
        Probe::Available { context: value["contextSize"].as_u64().unwrap_or(0) }
    } else {
        Probe::Unavailable { reason: value["reason"].as_str().unwrap_or("unknown").to_owned() }
    }
}

/// Whether Apple's model can answer on this Mac, asked once per launch (turning Apple Intelligence on needs no
/// restart of Locust to be noticed in a new launch, and a probe costs a process start).
pub fn probe() -> Probe {
    static ONCE: std::sync::OnceLock<Probe> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let Ok(path) = helper_path() else { return Probe::Unavailable { reason: "unknown".into() } };
        match Command::new(path).arg("--probe").stdin(Stdio::null()).stderr(Stdio::null()).output() {
            Ok(out) => read_probe(&String::from_utf8_lossy(&out.stdout)),
            Err(_) => Probe::Unavailable { reason: "unknown".into() },
        }
    })
    .clone()
}

pub fn available() -> bool {
    matches!(probe(), Probe::Available { .. })
}

/// The modes this runtime runs in: it has no tools, so Ask, and Plan, which runs wherever Ask does (`modeRunsOn`).
pub fn refuse_mode(mode: Mode) -> Option<&'static str> {
    (!matches!(mode, Mode::Ask | Mode::Plan)).then_some(
        "Apple's on-device model only talks: it cannot read or change files, so it runs in Ask mode only.",
    )
}

pub fn command(mode: Mode) -> io::Result<Command> {
    if let Some(words) = refuse_mode(mode) {
        return Err(io::Error::other(words));
    }
    Ok(Command::new(helper_path()?))
}

// ---- reading the helper's lines -------------------------------------------------------------------------------------

#[derive(Default)]
pub struct AppleNormalizer {
    pub ended: bool,
    said: String,
    step_open: bool,
    done: bool,
    failure: Option<(String, Option<String>)>,
    records: u64,
}

impl AppleNormalizer {
    fn event(kind: &str, payload: Value) -> Event {
        Event { kind: kind.into(), payload, thread: None }
    }

    fn open_step(&mut self, out: &mut Vec<Event>, kind: &str) {
        if !self.step_open {
            self.step_open = true;
            out.push(Self::event("step.started", json!({"stepKind": "turn", "evidence": evidence_of(kind, None)})));
        }
    }

    pub fn accept(&mut self, line: &Value) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        self.records += 1;
        let kind = line["type"].as_str().unwrap_or_default();
        let words = line["text"].as_str().unwrap_or_default();
        let mut out = Vec::new();
        match kind {
            "delta" if !words.is_empty() => {
                self.open_step(&mut out, kind);
                self.said.push_str(words);
                out.push(Self::event(
                    "message.delta",
                    json!({"itemId": "msg_0", "operation": "append", "text": text(words, 131_072), "final": false, "evidence": evidence_of(kind, None)}),
                ));
            }
            "replace" => {
                self.open_step(&mut out, kind);
                self.said = words.to_owned();
                out.push(Self::event(
                    "message.delta",
                    json!({"itemId": "msg_0", "operation": "replace", "text": text(words, 131_072), "final": false, "evidence": evidence_of(kind, None)}),
                ));
            }
            "done" => {
                self.open_step(&mut out, kind);
                self.done = true;
                let whole = if words.is_empty() { self.said.clone() } else { words.to_owned() };
                if !whole.is_empty() {
                    out.push(Self::event(
                        "message.delta",
                        json!({"itemId": "msg_0", "operation": "replace", "text": text(&whole, 131_072), "final": true, "evidence": evidence_of(kind, None)}),
                    ));
                }
                out.push(Self::event("step.completed", json!({"stepKind": "turn", "evidence": evidence_of(kind, None)})));
                self.step_open = false;
            }
            "error" => {
                self.failure = Some((
                    line["kind"].as_str().unwrap_or("other").to_owned(),
                    line["reason"].as_str().map(str::to_owned),
                ));
            }
            _ => {}
        }
        out
    }

    /// The run's last event, once the helper has exited (or was stopped).
    pub fn finish(&mut self, cancelled: bool, exit_code: Option<i32>) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        self.ended = true;
        let process = json!({
            "exitCode": exit_code.map(Value::from).unwrap_or(Value::Null),
            "signal": null, "stderr": "", "stderrTruncated": false,
            "recordCount": self.records, "inputDeliveryFailed": false,
        });
        if cancelled {
            return vec![Self::event("run.cancelled", json!({"process": process}))];
        }
        if self.done && self.failure.is_none() && exit_code == Some(0) {
            return vec![Self::event("run.completed", json!({"process": process}))];
        }
        let message = match &self.failure {
            Some((kind, reason)) => failure_words(kind, reason.as_deref()),
            None if self.done => format!(
                "Apple's on-device model answered, then its helper exited with code {}.",
                exit_code.map(|c| c.to_string()).unwrap_or_else(|| "null".into())
            ),
            None => "Apple's on-device model stopped before it answered.".into(),
        };
        vec![Self::event(
            "run.failed",
            json!({"kind": "process-failed", "message": text(&message, 16_384), "runtimeTerminal": if self.done {"completed"} else {"failed"}, "process": process}),
        )]
    }
}

// ---- the process ----------------------------------------------------------------------------------------------------

pub struct Apple {
    child: Child,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    normal: AppleNormalizer,
    queued: VecDeque<Event>,
    stopping: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    exited: bool,
    exit_code: Option<i32>,
}

fn err(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

impl Apple {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let mut child = super::process::spawn(command, Stdio::piped(), Stdio::null())?;
        let control = Arc::new(Control::default());
        control.watch(child.id());
        let output = child.stdout.take().expect("spawn configured this pipe and it has not been taken");
        let (tx, lines) = mpsc::sync_channel(1024);
        let reader_control = control.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                match super::process::read_line(&mut reader, &mut line) {
                    Ok(0) => break,
                    Ok(_) if line.len() as u64 > MAX_LINE => {
                        let _ = tx.try_send(Err(err("Apple's on-device model sent a line longer than 1 MiB")));
                        break;
                    }
                    Ok(_) => {
                        let trimmed = String::from_utf8_lossy(&line);
                        let trimmed = trimmed.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let value = serde_json::from_str::<Value>(trimmed)
                            .map_err(|_| err("Apple's on-device model helper sent malformed json"));
                        match tx.try_send(value) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                reader_control.fail("Apple's on-device model sent more than Locust could record");
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
            control,
            lines,
            normal: AppleNormalizer::default(),
            queued: VecDeque::new(),
            stopping: None,
            cancel: None,
            exited: false,
            exit_code: None,
        })
    }

    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    fn cancelled(&self) -> bool {
        crate::run_cli::interrupted() || self.cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
    }

    fn poll_exit(&mut self) {
        if !self.exited
            && let Ok(Some(status)) = self.child.try_wait()
        {
            self.exited = true;
            self.exit_code = status.code();
        }
    }

    fn await_exit(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        while !self.exited && Instant::now() < deadline {
            self.poll_exit();
            std::thread::sleep(Duration::from_millis(5));
        }
        self.poll_exit();
    }

    fn finish(&mut self) {
        let cancelled = self.stopping.is_some() || self.cancelled();
        self.queued.extend(self.normal.finish(cancelled, self.exit_code));
    }
}

impl Runtime for Apple {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        if let Some(words) = refuse_mode(turn.mode) {
            return Err(err(words));
        }
        if turn.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        // The request goes in whole, then stdin closes: the helper reads to its end.
        let mut stdin = self.child.stdin.take().ok_or_else(|| err("Apple's on-device model helper has no input"))?;
        stdin.write_all(request(&turn.prompt).to_string().as_bytes())?;
        drop(stdin);
        Ok(())
    }

    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended {
            return Ok(self.queued.pop_front());
        }
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        if self.stopping.is_some_and(|t| t.elapsed() >= Duration::from_secs(5)) {
            kill_process(self.child.id());
            self.finish();
            return Ok(self.queued.pop_front());
        }
        if self.control.failure().is_some() {
            self.await_exit(Duration::from_millis(500));
            self.finish();
            return Ok(self.queued.pop_front());
        }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(v)) => {
                self.queued.extend(self.normal.accept(&v));
                Ok(self.queued.pop_front())
            }
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => {
                while let Ok(Ok(v)) = self.lines.try_recv() {
                    self.queued.extend(self.normal.accept(&v));
                }
                self.await_exit(Duration::from_millis(500));
                self.finish();
                Ok(self.queued.pop_front())
            }
        }
    }

    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_none() {
            let now = Instant::now();
            self.stopping = Some(now);
            self.control.cancel_at(now + Duration::from_secs(5));
            interrupt_process(self.child.id());
        }
        Ok(())
    }
}

impl Drop for Apple {
    fn drop(&mut self) {
        self.control.close_and_kill(self.child.id());
        super::process::kill_reap(&mut self.child);
    }
}
