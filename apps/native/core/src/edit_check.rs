//! Post-edit check runner (`main/edit-check.ts`) and fix-until-check-passes loop (`main/check-fix.ts`, `shared/check-fix.ts`).

use crate::store::Store;
use regex::{Regex, RegexBuilder};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const CHECK_TIMEOUT_MS: u64 = 180_000;
pub const CHECK_OUTPUT_BYTES: usize = 256 * 1024;
pub const MAX_NEW_LINES: usize = 30;
pub const MAX_LINE: usize = 300;
pub const MAX_CHECK_COMMAND_LEN: usize = 500;
pub const MAX_CHECK_FIXES: u32 = 5;
pub const DEFAULT_CHECK_FIXES: u32 = 2;

const REMEMBERED: usize = 500;
const CHAINS: usize = 100;
const TOLD_LINES: usize = 30;
const TOLD_LINE: usize = 300;
pub const MAX_EDIT_CHECK_LINES: usize = 60;
pub const MAX_EDIT_CHECK_LINE_LENGTH: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCheckOutcome {
    Passed,
    Failed,
    TimedOut,
    CouldNotRun,
}

impl EditCheckOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::TimedOut => "timed-out",
            Self::CouldNotRun => "could-not-run",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "timed-out" => Some(Self::TimedOut),
            "could-not-run" => Some(Self::CouldNotRun),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditCheckResult {
    pub command: String,
    pub outcome: EditCheckOutcome,
    /// Lines of output that were not in the last check in this folder.
    pub new_lines: Vec<String>,
    /// Failed, and nothing in the failure is new since the last check.
    pub unchanged: bool,
    /// No earlier check in this folder to compare with.
    pub first: bool,
    /// Every line this failed check printed (for `check-fix`, never shown or recorded).
    pub lines: Option<Vec<String>>,
    /// Every line the check before it printed (absent when there was none).
    pub before: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckRun {
    pub exit_code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
    pub error: Option<String>,
}

/// One check command, as the person wrote it -- one line, bounded, or nothing (`teammate-store.ts:parsedCheckCommand`).
pub fn parsed_check_command(value: &str) -> Option<String> {
    let command = value.trim();
    if command.is_empty()
        || command.encode_utf16().count() > MAX_CHECK_COMMAND_LEN
        || command.chars().any(|c| c == '\r' || c == '\n' || c == '\0')
    {
        return None;
    }
    Some(command.to_owned())
}

/// A folder's most fixes, read defensively: a whole number from 1 to 5, or none (`shared/check-fix.ts`).
pub fn parsed_check_fix_tries(value: &Value) -> Option<u32> {
    let n = value.as_u64()?;
    if (1..=u64::from(MAX_CHECK_FIXES)).contains(&n) {
        Some(n as u32)
    } else {
        None
    }
}

/// Every folder's most fixes, by workspace id (`shared/check-fix.ts`).
pub fn parsed_check_fixes(value: &Value) -> BTreeMap<String, u32> {
    let Some(obj) = value.as_object() else {
        return BTreeMap::new();
    };
    let mut kept = BTreeMap::new();
    for (folder, tries) in obj.iter().take(200) {
        if folder.is_empty() || folder.len() > 200 {
            continue;
        }
        if let Some(most) = parsed_check_fix_tries(tries) {
            kept.insert(folder.clone(), most);
        }
    }
    kept
}

/// Run a check command through the shell in `cwd`, bounded by `timeout_ms` and `CHECK_OUTPUT_BYTES`.
pub fn run_check_command(command: &str, cwd: &Path, timeout_ms: u64) -> CheckRun {
    if command.contains('\0') {
        return CheckRun {
            exit_code: None,
            output: String::new(),
            timed_out: false,
            error: Some("The check command contains a NUL byte.".into()),
        };
    }

    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CheckRun {
                exit_code: None,
                output: String::new(),
                timed_out: false,
                error: Some(e.to_string()),
            };
        }
    };

    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(spawn_bounded_reader(stdout, buf.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_bounded_reader(stderr, buf.clone()));
    }

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut timed_out = false;
    let mut exit_code = None;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break;
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    kill_process_tree(&mut child);
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                for r in readers {
                    let _ = r.join();
                }
                let output = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
                return CheckRun {
                    exit_code: None,
                    output,
                    timed_out,
                    error: Some(e.to_string()),
                };
            }
        }
    }

    for r in readers {
        let _ = r.join();
    }
    let output = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    CheckRun {
        exit_code,
        output,
        timed_out,
        error: None,
    }
}

fn spawn_bounded_reader(
    mut stream: impl Read + Send + 'static,
    buf: Arc<Mutex<Vec<u8>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut guard = buf.lock().unwrap();
                    if guard.len() < CHECK_OUTPUT_BYTES {
                        let take = (CHECK_OUTPUT_BYTES - guard.len()).min(n);
                        guard.extend_from_slice(&chunk[..take]);
                    }
                }
            }
        }
    })
}

fn kill_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as libc::pid_t;
        if pid > 0 {
            unsafe {
                libc::killpg(pid, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill();
}

fn encode_uri(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let ch = *byte as char;
        if byte.is_ascii_alphanumeric()
            || matches!(
                ch,
                ';' | ',' | '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')' | '#'
            )
        {
            out.push(ch);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

fn is_word_or_sep_before(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '\\' | '/' | '-')
}

fn is_word_or_sep_after(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')
}

/// Replace `cwd` (in all its path/file-URL/JSON spellings) with `.` (`edit-check.ts:withoutFolder`).
pub fn without_folder(output: &str, cwd: &str, is_windows: bool) -> String {
    let root = cwd.trim_end_matches(['/', '\\']);
    if root.len() < 3 {
        return output.to_owned();
    }
    let forward = root.replace('\\', "/");
    let slash = if forward.starts_with('/') { "" } else { "/" };
    let mut spellings = HashSet::new();
    spellings.insert(format!("file://{slash}{}", encode_uri(&forward)));
    spellings.insert(format!("file://{slash}{forward}"));
    spellings.insert(root.to_owned());
    spellings.insert(root.replace('\\', "\\\\"));
    spellings.insert(forward.clone());
    if forward.len() >= 3
        && forward.as_bytes()[0].is_ascii_alphabetic()
        && forward.as_bytes()[1] == b':'
        && forward.as_bytes()[2] == b'/'
    {
        let drive_lower = (forward.as_bytes()[0] as char).to_ascii_lowercase();
        spellings.insert(format!("/{drive_lower}{}", &forward[2..]));
    }
    let mut list: Vec<String> = spellings.into_iter().collect();
    list.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    let escaped: Vec<String> = list.iter().map(|s| regex::escape(s)).collect();
    let pattern = escaped.join("|");
    let Ok(re) = RegexBuilder::new(&pattern)
        .case_insensitive(is_windows)
        .build()
    else {
        return output.to_owned();
    };

    let mut result = String::with_capacity(output.len());
    let mut last_end = 0;
    for m in re.find_iter(output) {
        let before_ok = output[..m.start()]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_or_sep_before(c));
        let after_ok = output[m.end()..]
            .chars()
            .next()
            .is_none_or(|c| !is_word_or_sep_after(c));
        if before_ok && after_ok {
            result.push_str(&output[last_end..m.start()]);
            result.push('.');
            last_end = m.end();
        }
    }
    result.push_str(&output[last_end..]);
    result
}

/// Strip ANSI codes, timings, and duplicate lines (`edit-check.ts:comparableLines`).
pub fn comparable_lines(output: &str) -> Vec<String> {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    static TRAIL_TIME_RE: OnceLock<Regex> = OnceLock::new();
    static ONLY_TIME_WORD_RE: OnceLock<Regex> = OnceLock::new();
    static TIME_HEADER_RE: OnceLock<Regex> = OnceLock::new();
    static DURATION_MS_RE: OnceLock<Regex> = OnceLock::new();

    let ansi = ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").unwrap());
    let trail_time = TRAIL_TIME_RE.get_or_init(|| {
        RegexBuilder::new(r"\s*\(?\b\d+(?:\.\d+)?\s?(?:ms|s|sec|seconds)\)?$")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    let only_time_word = ONLY_TIME_WORD_RE.get_or_init(|| {
        RegexBuilder::new(r"^(?:time|took|duration|elapsed|done in|finished in|in)\s*:?$")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    let time_header = TIME_HEADER_RE.get_or_init(|| {
        RegexBuilder::new(r"^(?:start at|duration|time:|finished in|ran \d+ tests? in)\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    let duration_ms = DURATION_MS_RE.get_or_init(|| {
        RegexBuilder::new(r"^[^A-Za-z0-9_]*duration_ms\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });

    let stripped = ansi.replace_all(output, "");
    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    for raw in stripped.split('\n').map(|s| s.strip_suffix('\r').unwrap_or(s)) {
        let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let line = trail_time.replace(&collapsed, "").trim().to_owned();
        if line.is_empty()
            || only_time_word.is_match(&line)
            || time_header.is_match(&line)
            || duration_ms.is_match(&line)
        {
            continue;
        }
        if seen.insert(line.clone()) {
            lines.push(line);
        }
    }
    lines
}

fn bounded_lines(list: &[String], max_lines: usize, max_len: usize) -> Vec<String> {
    let start = list.len().saturating_sub(max_lines);
    list[start..]
        .iter()
        .map(|line| {
            let count = line.chars().count();
            if count > max_len {
                let prefix: String = line.chars().take(max_len - 1).collect();
                format!("{prefix}…")
            } else {
                line.clone()
            }
        })
        .collect()
}

/// JSON record for `mission.edit_check`, bounded as `packages/mission-store/src/index.ts:boundedEditCheck`.
pub fn edit_check_record_json(result: &EditCheckResult, occurred_at: &str) -> Value {
    let new_lines: Vec<String> = result
        .new_lines
        .iter()
        .take(MAX_EDIT_CHECK_LINES)
        .map(|l| l.chars().take(MAX_EDIT_CHECK_LINE_LENGTH).collect())
        .collect();
    let command: String = result.command.chars().take(MAX_CHECK_COMMAND_LEN).collect();
    json!({
        "command": command,
        "outcome": result.outcome.as_str(),
        "newLines": new_lines,
        "unchanged": result.unchanged,
        "first": result.first,
        "occurredAt": occurred_at,
    })
}

/// Stateful per-folder check runner (`edit-check.ts:createEditCheck`).
#[derive(Default)]
pub struct EditCheck {
    state: Mutex<EditCheckState>,
}

#[derive(Default)]
struct EditCheckState {
    last: HashMap<String, HashSet<String>>,
    busy: HashSet<String>,
    running: HashMap<String, String>,
}

impl EditCheck {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn running_checks(&self) -> Vec<(String, String)> {
        let guard = self.state.lock().unwrap();
        guard
            .running
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn after<F, R, S>(
        &self,
        cwd: &Path,
        mission_id: Option<&str>,
        workspace_id: Option<&str>,
        command_for: F,
        on_start: S,
        run: R,
    ) -> Option<EditCheckResult>
    where
        F: FnOnce(Option<&str>) -> Option<String>,
        S: FnOnce(&str),
        R: FnOnce(&str, &Path) -> CheckRun,
    {
        let command = command_for(workspace_id)?;
        if command.is_empty() {
            return None;
        }
        let cwd_key = cwd.to_string_lossy().into_owned();
        {
            let mut guard = self.state.lock().unwrap();
            if guard.busy.contains(&cwd_key) {
                return None;
            }
            guard.busy.insert(cwd_key.clone());
            if let Some(id) = mission_id {
                guard.running.insert(id.to_owned(), command.clone());
            }
        }

        let _cleanup = BusyGuard {
            owner: self,
            cwd_key: cwd_key.clone(),
            mission_id: mission_id.map(str::to_owned),
        };

        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            on_start(&command);
        }));

        let result = run(&command, cwd);
        let lines = comparable_lines(&without_folder(&result.output, &cwd_key, cfg!(windows)));
        let before = {
            let mut guard = self.state.lock().unwrap();
            let prev = guard.last.get(&cwd_key).cloned();
            guard.last.insert(cwd_key.clone(), lines.iter().cloned().collect());
            prev
        };
        let first = before.is_none();

        if let Some(err) = result.error {
            let clean = without_folder(&err, &cwd_key, cfg!(windows));
            return Some(EditCheckResult {
                command,
                outcome: EditCheckOutcome::CouldNotRun,
                new_lines: bounded_lines(&[clean], MAX_NEW_LINES, MAX_LINE),
                unchanged: false,
                first,
                lines: None,
                before: None,
            });
        }
        if result.timed_out {
            return Some(EditCheckResult {
                command,
                outcome: EditCheckOutcome::TimedOut,
                new_lines: Vec::new(),
                unchanged: false,
                first,
                lines: None,
                before: None,
            });
        }
        if result.exit_code == Some(0) {
            return Some(EditCheckResult {
                command,
                outcome: EditCheckOutcome::Passed,
                new_lines: Vec::new(),
                unchanged: false,
                first,
                lines: None,
                before: None,
            });
        }
        let fresh: Vec<String> = if let Some(prev) = &before {
            lines.iter().filter(|l| !prev.contains(*l)).cloned().collect()
        } else {
            lines.clone()
        };
        let unchanged = !first && fresh.is_empty();
        Some(EditCheckResult {
            command,
            outcome: EditCheckOutcome::Failed,
            new_lines: bounded_lines(&fresh, MAX_NEW_LINES, MAX_LINE),
            unchanged,
            first,
            lines: Some(lines),
            before: before.map(|b| b.into_iter().collect()),
        })
    }
}

struct BusyGuard<'a> {
    owner: &'a EditCheck,
    cwd_key: String,
    mission_id: Option<String>,
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        let mut guard = self.owner.state.lock().unwrap();
        guard.busy.remove(&self.cwd_key);
        if let Some(id) = &self.mission_id {
            guard.running.remove(id);
        }
    }
}

/// Prompt sent to the teammate when `Send to <teammate>` is clicked on the card (`EditCheckCard.tsx:editCheckFollowUp`).
pub fn edit_check_follow_up(command: &str, new_lines: &[String], first: bool) -> String {
    let mut out = Vec::with_capacity(new_lines.len() + 4);
    out.push(format!(
        "After your last turn, the project's check `{command}` failed{}:",
        if first { "" } else { " with problems it did not have before" }
    ));
    out.push(String::new());
    for line in new_lines {
        out.push(format!("    {line}"));
    }
    out.push(String::new());
    out.push("Fix what your change caused and say what you changed. Locust runs the check again after your turn.".into());
    out.join("\n")
}

/// Automatic fix-turn prompt (`shared/check-fix.ts:checkFixPrompt`).
pub fn check_fix_prompt(command: &str, lines: &[String], first: bool, fix: u32, of: u32) -> String {
    let count = format!("(fix {fix} of {of})");
    let header = if fix == 1 {
        format!(
            "The check `{command}` failed after your last turn{} {count}. What it printed:",
            if first { "" } else { ", with problems it did not have before" }
        )
    } else {
        format!(
            "The check `{command}` still fails after your fix {count}. What it printed{}:",
            if first { "" } else { " that was not failing before your change" }
        )
    };
    let mut out = Vec::new();
    out.push(header);
    out.push(String::new());
    if lines.is_empty() {
        out.push("    (it printed nothing)".into());
    } else {
        for line in lines {
            out.push(format!("    {line}"));
        }
    }
    out.push(String::new());
    out.push("Fix what your change caused, then say in one or two sentences what you changed. Do not weaken, skip or delete tests, or change the check itself, to make it pass; change a test only where the task asked for the behavior it checks, and say so. Locust runs the check again after your turn.".into());
    out.join("\n")
}

pub mod stopped_reason {
    pub fn out_of_fixes(fixes: u32) -> String {
        format!(
            "The check still fails after {fixes} {}, so Locust stopped sending it back. Send it yourself from the card, or raise the most in Settings > Project folder.",
            if fixes == 1 { "fix" } else { "fixes" }
        )
    }
    pub fn no_progress(fix: u32) -> String {
        format!(
            "The check still fails the same way after fix {fix}, so Locust stopped sending it back: another fix would likely end the same way."
        )
    }
    pub fn fixed_what_it_broke(fix: u32) -> String {
        format!(
            "After fix {fix} the check fails only the way it did before this conversation's change, so Locust stopped there: what is left was failing already."
        )
    }
    pub fn changed_nothing(fix: u32) -> String {
        format!(
            "Fix {fix} changed no files, so the check was not run again and Locust stopped sending it back."
        )
    }
    pub fn busy(name: &str) -> String {
        format!("You were already talking to {name}, so Locust did not send the failing check back.")
    }
    pub fn could_not_start(why: &str) -> String {
        format!("Locust could not send the failing check back: {why}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckFixStarter {
    pub fix: u32,
    pub of: u32,
    pub command: String,
}

impl CheckFixStarter {
    pub fn to_json(&self) -> Value {
        json!({
            "kind": "check-fix",
            "fix": self.fix,
            "of": self.of,
            "command": self.command,
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        if v.get("kind").and_then(Value::as_str) != Some("check-fix") {
            return None;
        }
        let fix = v.get("fix").and_then(Value::as_u64)? as u32;
        let of = v.get("of").and_then(Value::as_u64)? as u32;
        let command = v.get("command").and_then(Value::as_str)?.to_owned();
        if fix < 1 || of < fix || of > MAX_CHECK_FIXES || command.is_empty() {
            return None;
        }
        Some(Self { fix, of, command })
    }
}

#[derive(Clone, Debug)]
pub struct CheckedTurn {
    pub mission_id: String,
    pub workspace_id: Option<String>,
    pub teammate_id: Option<String>,
    pub completed: bool,
    pub apart: bool,
    pub started_by_kind: Option<String>,
    pub started_by_fix: Option<u32>,
    pub sent_by_person: bool,
}

#[derive(Clone, Debug)]
pub enum CheckFixResultOrNoEdits {
    Result(EditCheckResult),
    NoEdits,
}

#[derive(Clone, Debug)]
pub struct FixStartRequest {
    pub follow_up_of: String,
    pub teammate_id: Option<String>,
    pub prompt: String,
    pub started_by: CheckFixStarter,
}

#[derive(Clone, Debug)]
pub enum FixStartOutcome {
    Ok { mission_id: String },
    Busy,
    Error { message: String },
}

#[derive(Clone, Debug)]
struct Chain {
    baseline: HashSet<String>,
    caused: HashSet<String>,
    first: bool,
}

#[derive(Default)]
struct CheckFixState {
    sent: HashSet<String>,
    sent_order: VecDeque<String>,
    chains: HashMap<String, Chain>,
    chain_order: VecDeque<String>,
    deciding: HashSet<String>,
}

impl CheckFixState {
    fn mark_sent(&mut self, mission_id: &str) {
        if self.sent.insert(mission_id.to_owned()) {
            self.sent_order.push_back(mission_id.to_owned());
            while self.sent_order.len() > REMEMBERED {
                if let Some(oldest) = self.sent_order.pop_front() {
                    self.sent.remove(&oldest);
                }
            }
        }
    }

    fn unmark_sent(&mut self, mission_id: &str) {
        self.sent.remove(mission_id);
    }

    fn insert_chain(&mut self, mission_id: String, chain: Chain) {
        if !self.chains.contains_key(&mission_id) {
            self.chain_order.push_back(mission_id.clone());
        }
        self.chains.insert(mission_id, chain);
        while self.chains.len() > CHAINS {
            if let Some(oldest) = self.chain_order.pop_front() {
                self.chains.remove(&oldest);
            }
        }
    }
}

/// Stateful fix-until-check-passes controller (`check-fix.ts:createCheckFix`).
#[derive(Default)]
pub struct CheckFix {
    state: Mutex<CheckFixState>,
}

impl CheckFix {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_checked<MF, B, N, S, NT>(
        &self,
        turn: &CheckedTurn,
        result: CheckFixResultOrNoEdits,
        most_fixes: MF,
        busy: B,
        name_of: N,
        start: S,
        note: NT,
    ) where
        MF: FnOnce(Option<&str>) -> Option<u32>,
        B: FnOnce(&CheckedTurn) -> bool,
        N: FnOnce(Option<&str>) -> String,
        S: FnOnce(FixStartRequest) -> FixStartOutcome,
        NT: FnOnce(&str, &str, &'static str),
    {
        {
            let mut guard = self.state.lock().unwrap();
            if guard.deciding.contains(&turn.mission_id) || guard.sent.contains(&turn.mission_id) {
                return;
            }
            guard.deciding.insert(turn.mission_id.clone());
        }
        let _deciding_guard = DecidingGuard {
            owner: self,
            mission_id: turn.mission_id.clone(),
        };

        if !turn.completed || turn.apart {
            return;
        }
        let done = if turn.started_by_kind.as_deref() == Some("check-fix") {
            match turn.started_by_fix {
                Some(f) => f,
                None => return,
            }
        } else if turn.started_by_kind.is_none() && turn.sent_by_person {
            0
        } else {
            return;
        };

        let chain = {
            let mut guard = self.state.lock().unwrap();
            let c = if done == 0 {
                None
            } else {
                guard.chains.get(&turn.mission_id).cloned()
            };
            guard.chains.remove(&turn.mission_id);
            c
        };

        if let CheckFixResultOrNoEdits::Result(r) = &result
            && r.outcome != EditCheckOutcome::Failed
        {
            return;
        }
        let Some(most) = most_fixes(turn.workspace_id.as_deref()).filter(|&m| m >= 1) else {
            return;
        };

        let result = match result {
            CheckFixResultOrNoEdits::NoEdits => {
                if done > 0 {
                    note(&turn.mission_id, &stopped_reason::changed_nothing(done), "warning");
                }
                return;
            }
            CheckFixResultOrNoEdits::Result(r) => r,
        };

        let baseline: HashSet<String> = match &chain {
            Some(c) => c.baseline.clone(),
            None => result
                .before
                .as_ref()
                .map(|b| b.iter().cloned().collect())
                .unwrap_or_default(),
        };
        let source_lines = result.lines.as_deref().unwrap_or(&result.new_lines);
        let caused: Vec<String> = source_lines
            .iter()
            .filter(|l| !baseline.contains(*l))
            .cloned()
            .collect();

        if done == 0 || chain.is_none() {
            if result.unchanged {
                if done > 0 {
                    note(&turn.mission_id, &stopped_reason::no_progress(done), "warning");
                }
                return;
            }
        } else if let Some(c) = &chain {
            if caused.is_empty() {
                note(&turn.mission_id, &stopped_reason::fixed_what_it_broke(done), "info");
                return;
            }
            let same_lines = caused.len() == c.caused.len() && caused.iter().all(|l| c.caused.contains(l));
            if same_lines {
                note(&turn.mission_id, &stopped_reason::no_progress(done), "warning");
                return;
            }
        }

        if done >= most {
            note(&turn.mission_id, &stopped_reason::out_of_fixes(done), "warning");
            return;
        }
        if busy(turn) {
            let name = name_of(turn.teammate_id.as_deref());
            note(&turn.mission_id, &stopped_reason::busy(&name), "info");
            return;
        }

        {
            let mut guard = self.state.lock().unwrap();
            guard.mark_sent(&turn.mission_id);
        }

        let fix = done + 1;
        let first = chain.as_ref().map(|c| c.first).unwrap_or(result.first);
        let started_by = CheckFixStarter {
            fix,
            of: most,
            command: result.command.clone(),
        };
        let prompt = check_fix_prompt(
            &result.command,
            &bounded_lines(&caused, TOLD_LINES, TOLD_LINE),
            first,
            fix,
            most,
        );
        match start(FixStartRequest {
            follow_up_of: turn.mission_id.clone(),
            teammate_id: turn.teammate_id.clone(),
            prompt,
            started_by,
        }) {
            FixStartOutcome::Ok { mission_id } => {
                let mut guard = self.state.lock().unwrap();
                guard.insert_chain(
                    mission_id,
                    Chain {
                        baseline,
                        caused: caused.into_iter().collect(),
                        first,
                    },
                );
            }
            FixStartOutcome::Busy => {
                let name = name_of(turn.teammate_id.as_deref());
                note(&turn.mission_id, &stopped_reason::busy(&name), "info");
            }
            FixStartOutcome::Error { message } => {
                {
                    let mut guard = self.state.lock().unwrap();
                    guard.unmark_sent(&turn.mission_id);
                }
                note(&turn.mission_id, &stopped_reason::could_not_start(&message), "warning");
            }
        }
    }
}

struct DecidingGuard<'a> {
    owner: &'a CheckFix,
    mission_id: String,
}

impl Drop for DecidingGuard<'_> {
    fn drop(&mut self) {
        let mut guard = self.owner.state.lock().unwrap();
        guard.deciding.remove(&self.mission_id);
    }
}

pub fn check_command_for(store: &Store, workspace_id: Option<&str>) -> Option<String> {
    let ws = workspace_id?;
    let settings = crate::workspace_settings::load(store).ok()?;
    settings.check_commands.get(ws).cloned()
}

pub fn most_fixes_for(store: &Store, workspace_id: Option<&str>) -> Option<u32> {
    let ws = workspace_id?;
    let settings = crate::workspace_settings::load(store).ok()?;
    settings.check_fixes.get(ws).copied()
}

#[cfg(test)]
#[path = "edit_check_tests.rs"]
mod tests;
