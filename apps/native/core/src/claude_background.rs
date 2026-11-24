//! Claude Code work that keeps going when Locust closes (`claude --bg`, `claude agents --json`,
//! `claude stop`).
//!
//! Ports `apps/desktop/src/main/claude-background.ts` (Row 1) and
//! `apps/desktop/src/main/claude-background-runs.ts` (Row 2).
//! Locust only watches these sessions; it never answers approval prompts for them.

use chrono::{DateTime, SecondsFormat};
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;

/// State of a Claude Code background session as reported by `claude agents --json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackgroundState {
    Working,
    Blocked,
    Done,
    Failed,
    Stopped,
    Unknown,
}

impl BackgroundState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Unknown => "unknown",
        }
    }
}

/// One background session entry parsed from `claude agents --json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundAgent {
    pub id: String,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub name: Option<String>,
    pub started_at: Option<String>,
    pub state: BackgroundState,
    /// What a blocked session waits on, in Claude Code's own words when it gives any.
    pub waiting_for: Option<String>,
}

/// Locust's sandbox mode mapped to Claude Code's `--permission-mode` for a `--bg` session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundSandbox {
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}

/// Options for building `claude --bg ...` arguments.
#[derive(Clone, Debug)]
pub struct BackgroundArgsOptions<'a> {
    pub prompt: &'a str,
    pub sandbox: BackgroundSandbox,
    pub ask_each: bool,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub resume_session_id: Option<&'a str>,
}

fn trimmed_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Claude Code's state word, or `Unknown` for one this build has not met (never a guess).
pub fn background_state_of(value: Option<&str>) -> BackgroundState {
    match value.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("working") => BackgroundState::Working,
        Some("blocked") => BackgroundState::Blocked,
        Some("done") => BackgroundState::Done,
        Some("failed") => BackgroundState::Failed,
        Some("stopped") => BackgroundState::Stopped,
        _ => BackgroundState::Unknown,
    }
}

/// What a blocked session says it waits for: a string, or an object's own words.
fn waiting_words(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    let obj = value.as_object()?;
    for key in ["message", "description", "title", "tool", "kind", "type"] {
        if let Some(said) = trimmed_text(obj.get(key)) {
            return Some(said);
        }
    }
    None
}

fn format_started_at(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(ms) = value.as_i64() {
        return DateTime::from_timestamp_millis(ms)
            .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true));
    }
    if let Some(ms) = value.as_f64().filter(|n| n.is_finite()) {
        return DateTime::from_timestamp_millis(ms as i64)
            .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true));
    }
    trimmed_text(Some(value))
}

/// The background sessions in `claude agents --json`; interactive ones and malformed entries are left out.
/// `None` when the answer is not a list at all: reading an empty/malformed answer as "no sessions"
/// would wrongly mark every live session as stopped.
pub fn parse_background_agents(json: &str) -> Option<Vec<BackgroundAgent>> {
    let parsed: Value = serde_json::from_str(json).ok()?;
    let list = parsed.as_array()?;
    let mut agents = Vec::new();
    for entry in list {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        if obj.get("kind").and_then(Value::as_str) != Some("background") {
            continue;
        }
        let Some(id) = trimmed_text(obj.get("id")) else {
            continue;
        };
        agents.push(BackgroundAgent {
            id,
            session_id: trimmed_text(obj.get("sessionId")),
            cwd: trimmed_text(obj.get("cwd")),
            name: trimmed_text(obj.get("name")),
            started_at: format_started_at(obj.get("startedAt")),
            state: background_state_of(obj.get("state").and_then(Value::as_str)),
            waiting_for: waiting_words(obj.get("waitingFor")),
        });
    }
    Some(agents)
}

/// Still going: Locust keeps watching. Done, failed and stopped are over; so is one Claude Code no longer lists.
pub fn background_is_live(state: BackgroundState) -> bool {
    matches!(
        state,
        BackgroundState::Working | BackgroundState::Blocked | BackgroundState::Unknown
    )
}

/// The card's words for a state.
pub fn background_sentence(state: BackgroundState, waiting_for: Option<&str>) -> String {
    match state {
        BackgroundState::Working => "Working in the background.".to_string(),
        BackgroundState::Blocked => match waiting_for {
            Some(what) => format!(
                "Waiting for you: {what}. Open it to answer; Locust does not answer for you."
            ),
            None => "Waiting for you. Open it to answer; Locust does not answer for you.".to_string(),
        },
        BackgroundState::Done => "Finished in the background.".to_string(),
        BackgroundState::Failed => {
            "Stopped with an error in the background. Open it to see what happened.".to_string()
        }
        BackgroundState::Stopped => "Stopped.".to_string(),
        BackgroundState::Unknown => "Running in the background.".to_string(),
    }
}

/// Locust's mode, said as Claude Code's own permission mode for a session Locust does not run.
pub fn background_permission_mode(sandbox: BackgroundSandbox, ask_each: bool) -> &'static str {
    if ask_each && sandbox == BackgroundSandbox::WorkspaceWrite {
        return "default";
    }
    match sandbox {
        BackgroundSandbox::ReadOnly => "plan",
        BackgroundSandbox::WorkspaceWrite => "acceptEdits",
        BackgroundSandbox::FullAccess => "bypassPermissions",
    }
}

/// `claude --bg ...`: the conversation's own session when it has one, in the folder, on the chosen model and mode.
pub fn background_args(options: &BackgroundArgsOptions<'_>) -> Result<Vec<String>, String> {
    let prompt = options.prompt.trim();
    if prompt.is_empty() {
        return Err("A background turn needs a message.".to_string());
    }
    let mut args = vec!["--bg".to_string()];
    if let Some(resume) = options.resume_session_id {
        args.push("--resume".to_string());
        args.push(resume.to_string());
    }
    args.push("--permission-mode".to_string());
    args.push(background_permission_mode(options.sandbox, options.ask_each).to_string());
    if let Some(model) = options.model {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    if let Some(effort) = options.effort {
        args.push("--effort".to_string());
        args.push(effort.to_string());
    }
    // After `--`, so a message that begins with a dash is a message, never a flag.
    args.push("--".to_string());
    args.push(prompt.to_string());
    Ok(args)
}

/// `Workspace not trusted. ...`: the folder needs the person, once, in Claude Code itself.
pub fn needs_trust(said: &str) -> bool {
    let lower = said.to_ascii_lowercase();
    lower.contains("workspace not trusted") || lower.contains("accept the trust prompt")
}

/// The id `claude --bg` prints, from what it printed ("backgrounded · ad39859a").
pub fn background_id_from(said: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)backgrounded\s*\S\s*([0-9a-f]{6,})").expect("valid background id regex")
    });
    re.captures(said)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Result of completing one poll tick of `BackgroundWatch`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchTickOutcome {
    /// Emitted only when the observed set of watched agents changed since the previous notice.
    pub changed: Option<Vec<BackgroundAgent>>,
    /// True when at least one watched id is still live and another poll should be scheduled.
    pub schedule_next: bool,
}

/// Poll state machine for `claude agents --json`:
/// polls only while at least one of Locust's own ids is live, marks missing ids as `Stopped`,
/// suppresses duplicate notices, and keeps a single poll loop when `watch()` arrives mid-tick.
#[derive(Clone, Debug, Default)]
pub struct BackgroundWatch {
    watched: Vec<String>,
    watched_set: HashSet<String>,
    timer_scheduled: bool,
    ticking: bool,
    last_snapshot: Option<Vec<BackgroundAgent>>,
}

impl BackgroundWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start (or keep) watching these ids. Returns `true` if the caller should schedule a poll now.
    pub fn watch(&mut self, ids: &[&str]) -> bool {
        for &id in ids {
            if self.watched_set.insert(id.to_string()) {
                self.watched.push(id.to_string());
            }
        }
        if !self.watched.is_empty() && !self.timer_scheduled && !self.ticking {
            self.timer_scheduled = true;
            true
        } else {
            false
        }
    }

    /// Currently watched live ids.
    pub fn watching(&self) -> Vec<String> {
        self.watched.clone()
    }

    /// True if a timer is currently armed.
    pub fn timer_scheduled(&self) -> bool {
        self.timer_scheduled
    }

    /// Mark the start of an in-flight `claude agents --json` look.
    pub fn begin_tick(&mut self) {
        self.timer_scheduled = false;
        self.ticking = true;
    }

    /// Finish an in-flight look with the parsed agent list (or `None` when the list call failed).
    pub fn finish_tick(&mut self, all: Option<&[BackgroundAgent]>) -> WatchTickOutcome {
        self.ticking = false;
        let mut changed = None;
        if let Some(all) = all {
            let mut seen: Vec<BackgroundAgent> = all
                .iter()
                .filter(|agent| self.watched_set.contains(&agent.id))
                .cloned()
                .collect();
            let seen_ids: HashSet<&str> = seen.iter().map(|a| a.id.as_str()).collect();
            let missing: Vec<BackgroundAgent> = self
                .watched
                .iter()
                .filter(|id| !seen_ids.contains(id.as_str()))
                .map(|id| BackgroundAgent {
                    id: id.clone(),
                    session_id: None,
                    cwd: None,
                    name: None,
                    started_at: None,
                    state: BackgroundState::Stopped,
                    waiting_for: None,
                })
                .collect();
            seen.extend(missing);

            for agent in &seen {
                if !background_is_live(agent.state) {
                    self.watched_set.remove(&agent.id);
                    self.watched.retain(|id| id != &agent.id);
                }
            }

            if self.last_snapshot.as_ref() != Some(&seen) {
                self.last_snapshot = Some(seen.clone());
                changed = Some(seen);
            }
        }
        let schedule_next = !self.watched.is_empty();
        if schedule_next {
            self.timer_scheduled = true;
        }
        WatchTickOutcome {
            changed,
            schedule_next,
        }
    }

    /// Stop watching everything and disarm any pending timer.
    pub fn stop(&mut self) {
        self.timer_scheduled = false;
        self.watched.clear();
        self.watched_set.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::claude;
    use std::path::PathBuf;
    use std::process::Command;

    fn fixture_script() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("scripts")
            .join("fixtures")
            .join("claude-bg.py")
    }

    #[test]
    // case: reads only background sessions from claude agents --json, and nothing from a malformed answer
    fn reads_only_background_sessions_from_claude_agents_json_and_nothing_from_a_malformed_answer() {
        let json = serde_json::json!([
            {
                "id": "bg-1a2b3c",
                "cwd": "C:/work/pebble",
                "kind": "background",
                "startedAt": "2026-10-06T10:00:00Z",
                "sessionId": "s-1",
                "name": "fix the cart",
                "state": "working"
            },
            {
                "pid": 4321,
                "cwd": "C:/work/pebble",
                "kind": "interactive",
                "startedAt": "2026-10-06T09:00:00Z",
                "sessionId": "s-2",
                "name": "mine",
                "status": "idle"
            },
            {
                "id": "interactive-1",
                "kind": "interactive",
                "state": "working"
            },
            {
                "id": "bg-blocked",
                "kind": "background",
                "state": "blocked",
                "waitingFor": { "tool": "Bash" }
            },
            {
                "id": "bg-new",
                "kind": "background",
                "state": "paused"
            },
            {
                "kind": "background",
                "state": "working"
            }
        ])
        .to_string();

        assert_eq!(
            parse_background_agents(&json),
            Some(vec![
                BackgroundAgent {
                    id: "bg-1a2b3c".into(),
                    cwd: Some("C:/work/pebble".into()),
                    started_at: Some("2026-10-06T10:00:00Z".into()),
                    session_id: Some("s-1".into()),
                    name: Some("fix the cart".into()),
                    state: BackgroundState::Working,
                    waiting_for: None,
                },
                BackgroundAgent {
                    id: "bg-blocked".into(),
                    cwd: None,
                    started_at: None,
                    session_id: None,
                    name: None,
                    state: BackgroundState::Blocked,
                    waiting_for: Some("Bash".into()),
                },
                // A state this build has not met is said as unknown, never guessed.
                BackgroundAgent {
                    id: "bg-new".into(),
                    cwd: None,
                    started_at: None,
                    session_id: None,
                    name: None,
                    state: BackgroundState::Unknown,
                    waiting_for: None,
                },
            ])
        );

        // Not a list: no answer, never "none are running".
        assert_eq!(parse_background_agents(""), None);
        assert_eq!(parse_background_agents(r#"{"id":"x"}"#), None);

        // As measured: epoch milliseconds, and what a blocked one waits for.
        let measured = serde_json::json!([{
            "pid": 21492,
            "id": "ad39859a",
            "kind": "background",
            "startedAt": 1791339613341_i64,
            "sessionId": "ad39859a-5d0f",
            "name": "create hello.txt file",
            "status": "waiting",
            "waitingFor": "permission prompt",
            "state": "blocked"
        }])
        .to_string();
        assert_eq!(
            parse_background_agents(&measured),
            Some(vec![BackgroundAgent {
                id: "ad39859a".into(),
                session_id: Some("ad39859a-5d0f".into()),
                cwd: None,
                name: Some("create hello.txt file".into()),
                started_at: Some("2026-10-07T02:20:13.341Z".into()),
                state: BackgroundState::Blocked,
                waiting_for: Some("permission prompt".into()),
            }])
        );

        assert_eq!(
            background_id_from(
                "Starting background service…\nbackgrounded · ad39859a\n  claude agents  list sessions"
            ),
            Some("ad39859a".into())
        );
    }

    #[test]
    // case: says a blocked session waits for the person, and that Locust does not answer for them
    fn says_a_blocked_session_waits_for_the_person_and_that_locust_does_not_answer_for_them() {
        assert_eq!(
            background_sentence(BackgroundState::Blocked, Some("Bash")),
            "Waiting for you: Bash. Open it to answer; Locust does not answer for you."
        );
        assert!(background_is_live(BackgroundState::Blocked));
        assert!(!background_is_live(BackgroundState::Done));
    }

    #[test]
    // case: starts claude --bg on the conversation, the mode said as Claude Code's, the message never taken for a flag
    fn starts_claude_bg_on_the_conversation_the_mode_said_as_claude_codes_the_message_never_taken_for_a_flag() {
        assert_eq!(
            background_args(&BackgroundArgsOptions {
                prompt: "--help me",
                sandbox: BackgroundSandbox::WorkspaceWrite,
                ask_each: false,
                model: Some("haiku"),
                effort: None,
                resume_session_id: Some("s-1"),
            }),
            Ok(vec![
                "--bg".into(),
                "--resume".into(),
                "s-1".into(),
                "--permission-mode".into(),
                "acceptEdits".into(),
                "--model".into(),
                "haiku".into(),
                "--".into(),
                "--help me".into(),
            ])
        );
        assert_eq!(
            background_args(&BackgroundArgsOptions {
                prompt: "Read it.",
                sandbox: BackgroundSandbox::ReadOnly,
                ask_each: false,
                model: None,
                effort: None,
                resume_session_id: None,
            }),
            Ok(vec![
                "--bg".into(),
                "--permission-mode".into(),
                "plan".into(),
                "--".into(),
                "Read it.".into(),
            ])
        );
        assert_eq!(
            background_permission_mode(BackgroundSandbox::WorkspaceWrite, true),
            "default"
        );
        assert_eq!(
            background_permission_mode(BackgroundSandbox::FullAccess, false),
            "bypassPermissions"
        );
        assert!(background_args(&BackgroundArgsOptions {
            prompt: "  ",
            sandbox: BackgroundSandbox::ReadOnly,
            ask_each: false,
            model: None,
            effort: None,
            resume_session_id: None,
        })
        .is_err());
    }

    #[test]
    // case: knows the folder must be trusted in Claude Code first
    fn knows_the_folder_must_be_trusted_in_claude_code_first() {
        assert!(needs_trust(
            "Workspace not trusted. Run `claude` in C:\\x once and accept the trust prompt, then retry."
        ));
        assert!(!needs_trust("bg-1a2b3c"));
    }

    #[test]
    // case: polls only while one of its own is live, and says one Claude Code no longer lists is over
    fn polls_only_while_one_of_its_own_is_live_and_says_one_claude_code_no_longer_lists_is_over() {
        let mut watch = BackgroundWatch::new();
        let mut changes: Vec<Vec<BackgroundAgent>> = Vec::new();
        assert!(!watch.timer_scheduled());

        assert!(watch.watch(&["a"]));
        assert!(watch.timer_scheduled());

        let listed = vec![
            BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
            BackgroundAgent {
                id: "someone-else".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
        ];
        watch.begin_tick();
        let out = watch.finish_tick(Some(&listed));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(
            changes.last().unwrap(),
            &vec![BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            }]
        );
        assert!(out.schedule_next);
        assert!(watch.timer_scheduled());

        let listed = vec![BackgroundAgent {
            id: "a".into(),
            session_id: None,
            cwd: None,
            name: None,
            started_at: None,
            state: BackgroundState::Blocked,
            waiting_for: Some("Bash".into()),
        }];
        watch.begin_tick();
        let out = watch.finish_tick(Some(&listed));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(
            changes.last().unwrap(),
            &vec![BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Blocked,
                waiting_for: Some("Bash".into()),
            }]
        );

        // Unchanged: no second notice.
        watch.begin_tick();
        let out = watch.finish_tick(Some(&listed));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(changes.len(), 2);

        // Missing from claude agents --json: marked stopped, nothing left live, no timer left.
        watch.begin_tick();
        let out = watch.finish_tick(Some(&[]));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(
            changes.last().unwrap(),
            &vec![BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Stopped,
                waiting_for: None,
            }]
        );
        assert!(!out.schedule_next);
        assert!(!watch.timer_scheduled());
        assert!(watch.watching().is_empty());
    }

    #[test]
    // case: keeps one poll loop when a second watch comes while a look is in flight (2026-10-10 sweep)
    fn keeps_one_poll_loop_when_a_second_watch_comes_while_a_look_is_in_flight() {
        let mut watch = BackgroundWatch::new();
        assert!(watch.watch(&["a"]));
        watch.begin_tick();
        // The look is out; a second id arrives and must NOT schedule a second poll loop.
        assert!(!watch.watch(&["b"]));
        assert!(!watch.timer_scheduled());

        let out = watch.finish_tick(Some(&[
            BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
            BackgroundAgent {
                id: "b".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
        ]));
        assert!(out.schedule_next);
        assert!(watch.timer_scheduled());
        assert_eq!(watch.watching(), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn runtime_claude_background_caller_runs_fake_script_and_never_answers_blocked_session() {
        let script = fixture_script();
        assert!(script.exists(), "fixture script missing: {}", script.display());
        let tmp = std::env::temp_dir().join(format!(
            "locust-bg-row1-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&tmp);
        let calls_file = tmp.join("calls.jsonl");
        let _ = std::fs::remove_file(&calls_file);

        // 1. Start via runtime::claude::background_command_with_program (caller path).
        let opts = BackgroundArgsOptions {
            prompt: "--help me",
            sandbox: BackgroundSandbox::WorkspaceWrite,
            ask_each: false,
            model: Some("haiku"),
            effort: None,
            resume_session_id: Some("s-1"),
        };
        let mut cmd = Command::new("python3");
        cmd.arg(&script);
        cmd.env("CLAUDE_BG_CALLS_FILE", &calls_file);
        let started = claude::start_background_with(cmd, &tmp, &opts).expect("start ok");
        assert_eq!(started.id, "ad39859a");
        assert_eq!(
            started.sentence,
            "Working in the background."
        );

        // 2. Untrusted folder refusal through the same caller.
        let mut untrusted_cmd = Command::new("python3");
        untrusted_cmd.arg(&script);
        untrusted_cmd.env("CLAUDE_BG_UNTRUSTED", "1");
        let err = claude::start_background_with(untrusted_cmd, &tmp, &opts).unwrap_err();
        assert!(err.needs_trust, "expected needs_trust refusal, got: {err:?}");

        // 3. Watch & poll a blocked session via runtime::claude::poll_background_with.
        let mut watch = BackgroundWatch::new();
        assert!(watch.watch(&["ad39859a"]));
        let mut list_cmd = Command::new("python3");
        list_cmd.arg(&script);
        list_cmd.env(
            "CLAUDE_BG_AGENTS_JSON",
            r#"[{"id":"ad39859a","kind":"background","state":"blocked","waitingFor":"permission prompt"}]"#,
        );
        let outcome = claude::poll_background_with(&mut watch, list_cmd, &tmp).expect("poll ok");
        let seen = outcome.changed.expect("changed");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].state, BackgroundState::Blocked);
        assert_eq!(
            background_sentence(seen[0].state, seen[0].waiting_for.as_deref()),
            "Waiting for you: permission prompt. Open it to answer; Locust does not answer for you."
        );
        assert!(outcome.schedule_next);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
