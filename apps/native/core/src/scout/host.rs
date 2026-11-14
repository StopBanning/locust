//! Native engine host for Scout (`main/index.ts` `createScout({...})` wiring).
//!
//! Connects `crate::scout::Scout` to the native `Store`, `engine_api::api()`, `people`,
//! `ledger`, `edit_check`, `gh`, and `git` without Electron or React.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, FixedOffset, Local};
use serde_json::{Value, json};

use crate::approval::Mode;
use crate::connectors_off::FREE_MODELS_BEST_FIRST;
use crate::invariants::regex;
use crate::ledger::{self, Item};
use crate::native::{Origin, Send, SlotOrigin};
use crate::store::Store;

use super::folder::folder_mark_of;
use super::github::{GhError, ScoutGithub, ScoutGithubUnread, read_scout_github};
use super::scheduler::ScoutScheduler;
use super::store::ScoutFiles;
use super::types::{FailedTurn, FolderCheckResult, WaitingCard};
use super::{AnswerResult, AskResult, BriefRuns, Scout, ScoutDeps, ScoutRoute, ScoutTeammate};

pub const NO_WORKSPACE_MESSAGE: &str = "Choose the folder your teammates work in first. Locust was opened from its own install folder, and no teammate should work in there.";
const GH_TIMEOUT: Duration = Duration::from_secs(30);
const GIT_TIMEOUT: Duration = Duration::from_secs(10);
const CHECK_TAIL_LINES: usize = 25;
const CHECK_LINE_MAX_CHARS: usize = 300;
const BRIEF_MAX_MISSIONS: usize = 80;
const ANSWER_ERR_MAX_CHARS: usize = 200;
const SCOUT_NAMES: &[&str] = &["Scout", "Scout 2", "Scout 3", "Lookout"];

type GhHook = Arc<dyn Fn(&[&str], &Path) -> Result<String, GhError> + std::marker::Send + Sync>;
type GitHook = Arc<dyn Fn(&[&str], &Path) -> Result<String, String> + std::marker::Send + Sync>;

static HOSTS: OnceLock<Mutex<HashMap<PathBuf, Vec<Weak<Scout>>>>> = OnceLock::new();

fn hosts() -> &'static Mutex<HashMap<PathBuf, Vec<Weak<Scout>>>> {
    HOSTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn store_key(store: &Store) -> PathBuf {
    std::fs::canonicalize(store.root()).unwrap_or_else(|_| store.root().to_path_buf())
}

/// Registers a Scout instance for native turn-ending and diagnostic callbacks under `store`.
pub fn register_host(store: &Store, scout: &Arc<Scout>) {
    let key = store_key(store);
    let mut map = hosts().lock().unwrap_or_else(|e| e.into_inner());
    let list = map.entry(key).or_default();
    list.retain(|w| w.strong_count() > 0);
    if !list.iter().any(|w| {
        w.upgrade()
            .is_some_and(|existing| Arc::ptr_eq(&existing, scout))
    }) {
        list.push(Arc::downgrade(scout));
    }
}

/// Unregisters a Scout instance from `store`.
pub fn unregister_host(store: &Store, scout: &Arc<Scout>) {
    let key = store_key(store);
    let mut map = hosts().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(list) = map.get_mut(&key) {
        list.retain(|w| {
            w.upgrade()
                .is_some_and(|existing| !Arc::ptr_eq(&existing, scout))
        });
        if list.is_empty() {
            map.remove(&key);
        }
    }
}

fn active_hosts(store: &Store) -> Vec<Arc<Scout>> {
    let key = store_key(store);
    let mut map = hosts().lock().unwrap_or_else(|e| e.into_inner());
    let Some(list) = map.get_mut(&key) else {
        return Vec::new();
    };
    let mut live = Vec::new();
    list.retain(|w| match w.upgrade() {
        Some(arc) => {
            live.push(arc);
            true
        }
        None => false,
    });
    if list.is_empty() {
        map.remove(&key);
    }
    live
}

/// Called by `native.rs` when any turn in `store` finishes cleanly, fails, or is stopped.
pub fn on_run_ended(store: &Store, mission_id: &str) {
    for scout in active_hosts(store) {
        scout.on_run_ended(mission_id);
    }
}

/// Called by `native.rs` when a running turn emits `adapter.diagnostic`.
pub fn on_diagnostic(store: &Store, mission_id: &str, payload: &Value) {
    let code = payload.get("code").and_then(Value::as_str).unwrap_or("");
    if code.is_empty()
        || !regex(r"^opencode\.runtime_error$|\.provider_busy\.runtime_error$").is_match(code)
    {
        return;
    }
    let why = payload
        .get("detail")
        .or_else(|| payload.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(code);
    for scout in active_hosts(store) {
        scout.provider_refused(mission_id, why);
    }
}

fn parsed_acp_route_model(model: &str) -> Option<&str> {
    if !model.starts_with("acp_") {
        return None;
    }
    let (_, picked) = model.split_once("::")?;
    (!picked.is_empty()).then_some(picked)
}

/// Port of `free-routes.ts` `isFreeRoute`: true when `runtime` + `model` is a known free route.
pub fn is_free_route(runtime: &str, model: Option<&str>) -> bool {
    let Some(model) = model else {
        return false;
    };
    if runtime == "acp" {
        return parsed_acp_route_model(model)
            .is_some_and(|picked| regex(r"(?i)^opencode/[a-z0-9.-]+-free$").is_match(picked));
    }
    if runtime == "kilo" {
        return regex(r"(?i)(?:^|[-/:])free$").is_match(model);
    }
    runtime == "opencode" && regex(r"(?i)(?:^|[-/])free$").is_match(model)
}

fn clip_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        head
    } else {
        text.to_owned()
    }
}

fn clip_ellipsis(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let keep = max_chars.saturating_sub(1);
    let head: String = text.chars().take(keep).collect();
    format!("{head}…")
}

fn workspace_in(file: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(file).ok()?;
    let mut text = String::new();
    file.take(64 * 1024).read_to_string(&mut text).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let path = PathBuf::from(value["path"].as_str()?);
    path.is_absolute().then_some(path).filter(|p| p.is_dir())
}

fn open_workspace_in(store: &Store) -> Option<PathBuf> {
    workspace_in(&store.root().join("workspace.json"))
        .or_else(|| workspace_in(&store.electron().join("workspace.json")))
}

fn run_cli_with_timeout(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
    extra_env: &[(&str, &str)],
) -> Result<(Option<i32>, String, String), io::Error> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for &(k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn()?;
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = out_pipe {
            let _ = p.take(4 * 1024 * 1024).read_to_end(&mut buf);
        }
        String::from_utf8_lossy(&buf).into_owned()
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = err_pipe {
            let _ = p.take(4 * 1024 * 1024).read_to_end(&mut buf);
        }
        String::from_utf8_lossy(&buf).into_owned()
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Ok(Some(st.code())),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("{program} timed out"),
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => break Err(e),
        }
    };
    let stdout = out_handle.join().unwrap_or_default();
    let stderr = err_handle.join().unwrap_or_default();
    status.map(|code| (code.flatten(), stdout, stderr))
}

fn default_gh(args: &[&str], cwd: &Path) -> Result<String, GhError> {
    match run_cli_with_timeout(
        "gh",
        args,
        cwd,
        GH_TIMEOUT,
        &[("GH_PROMPT_DISABLED", "1"), ("GH_NO_UPDATE_NOTIFIER", "1")],
    ) {
        Ok((Some(0), stdout, _)) => Ok(stdout),
        Ok((_, _stdout, stderr)) => Err(GhError::Failed(stderr)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(GhError::NotFound(e.to_string())),
        Err(e) => Err(GhError::Failed(e.to_string())),
    }
}

fn default_git(args: &[&str], cwd: &Path) -> Result<String, String> {
    match run_cli_with_timeout("git", args, cwd, GIT_TIMEOUT, &[]) {
        Ok((Some(0), stdout, _)) => Ok(stdout),
        Ok((_, _, stderr)) => Err(stderr),
        Err(e) => Err(e.to_string()),
    }
}

/// Native `ScoutDeps` implementation backed by `Store`, `engine_api`, `people`, `ledger`,
/// `edit_check`, `gh`, and `git`.
pub struct ScoutHostDeps {
    store: Store,
    folder_override: Mutex<Option<PathBuf>>,
    skew_ms: AtomicI64,
    gh_hook: Mutex<Option<GhHook>>,
    git_hook: Mutex<Option<GitHook>>,
}

impl ScoutHostDeps {
    pub fn new(store: Store) -> Arc<Self> {
        Arc::new(Self {
            store,
            folder_override: Mutex::new(None),
            skew_ms: AtomicI64::new(0),
            gh_hook: Mutex::new(None),
            git_hook: Mutex::new(None),
        })
    }

    pub fn set_folder(&self, folder: Option<PathBuf>) {
        *self
            .folder_override
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = folder;
    }

    pub fn advance_clock_ms(&self, ms: i64) {
        self.skew_ms.fetch_add(ms, Ordering::Relaxed);
    }

    pub fn set_gh_hook(&self, hook: Option<GhHook>) {
        *self.gh_hook.lock().unwrap_or_else(|e| e.into_inner()) = hook;
    }

    pub fn set_git_hook(&self, hook: Option<GitHook>) {
        *self.git_hook.lock().unwrap_or_else(|e| e.into_inner()) = hook;
    }

    pub fn workspace_folder(&self) -> Option<PathBuf> {
        if let Some(over) = self
            .folder_override
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return Some(over);
        }
        open_workspace_in(&self.store)
    }

    fn mission_path(&self, mission_id: &str) -> Option<PathBuf> {
        if !ledger::safe_id(mission_id) {
            return None;
        }
        if let Ok(dir) = self.store.ledger_dir() {
            let p = dir.join(format!("{mission_id}.jsonl"));
            if p.is_file() {
                return Some(p);
            }
        }
        let electron_p = self
            .store
            .electron()
            .join("mission-ledger")
            .join(format!("{mission_id}.jsonl"));
        electron_p.is_file().then_some(electron_p)
    }

    fn teammate_name(&self, teammate_id: &str) -> Option<String> {
        let team = crate::people::team(&self.store).ok()?;
        team["teammates"]
            .as_array()?
            .iter()
            .find(|t| t["teammateId"].as_str() == Some(teammate_id))
            .and_then(|t| t["name"].as_str())
            .map(str::to_owned)
    }

    fn teammate_entry(&self, teammate_id: &str) -> Option<Value> {
        let team = crate::people::team(&self.store).ok()?;
        team["teammates"]
            .as_array()?
            .iter()
            .find(|t| t["teammateId"].as_str() == Some(teammate_id))
            .cloned()
    }

    fn list_mission_files(&self) -> Vec<(String, SystemTime, PathBuf)> {
        let mut by_id: HashMap<String, (SystemTime, PathBuf)> = HashMap::new();
        let dirs = [
            self.store.electron().join("mission-ledger"),
            self.store.root().join("mission-ledger"),
        ];
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if !ledger::safe_id(stem) {
                    continue;
                }
                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                by_id.insert(stem.to_owned(), (modified, path));
            }
        }
        by_id
            .into_iter()
            .map(|(id, (mtime, path))| (id, mtime, path))
            .collect()
    }
}

impl ScoutDeps for ScoutHostDeps {
    fn files(&self) -> ScoutFiles {
        ScoutFiles::Store(self.store.clone())
    }

    fn now(&self) -> DateTime<FixedOffset> {
        let now = Local::now().fixed_offset();
        let skew = self.skew_ms.load(Ordering::Relaxed);
        if skew == 0 {
            now
        } else {
            now + chrono::Duration::milliseconds(skew)
        }
    }

    fn github(&self) -> Result<Option<ScoutGithub>, ScoutGithubUnread> {
        let Some(folder) = self.workspace_folder() else {
            return Ok(None);
        };
        let hook = self
            .gh_hook
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let mut unread_why: Option<ScoutGithubUnread> = None;
        let gh_fn = |args: &[&str], cwd: &Path| -> Result<String, GhError> {
            match &hook {
                Some(cb) => cb(args, cwd),
                None => default_gh(args, cwd),
            }
        };
        let res = read_scout_github(
            &folder,
            gh_fn,
            Some("@me"),
            Some(&mut |why| {
                unread_why = Some(why);
            }),
        );
        if let Some(gh) = res {
            Ok(Some(gh))
        } else if let Some(why) = unread_why {
            Err(why)
        } else {
            Ok(None)
        }
    }

    fn waiting(&self) -> Result<Vec<WaitingCard>, String> {
        let list = crate::engine_api::api().waiting();
        Ok(list
            .into_iter()
            .map(|req| {
                let teammate_id = crate::people::owner_of(&self.store, &req.mission_id);
                let name = teammate_id
                    .as_deref()
                    .and_then(|id| self.teammate_name(id));
                WaitingCard {
                    approval_id: req.approval_id,
                    mission_id: req.mission_id,
                    summary: req.summary,
                    requested_at: req.requested_at,
                    teammate_id,
                    name,
                }
            })
            .collect())
    }

    fn failed_turn(&self, mission_id: &str) -> Result<Option<FailedTurn>, String> {
        let Some(path) = self.mission_path(mission_id) else {
            return Ok(None);
        };
        let transcript = ledger::read_transcript(&path);
        if !transcript.failed {
            return Ok(None);
        }
        let meta = ledger::read_metadata(&path);
        let owner = crate::people::owner_of(&self.store, mission_id);
        let name = owner.as_deref().and_then(|id| self.teammate_name(id));
        let why = transcript.items.iter().rev().find_map(|item| match item {
            Item::Note { level, text } if level == "error" && !text.trim().is_empty() => {
                Some(text.clone())
            }
            _ => None,
        });
        Ok(Some(FailedTurn {
            mission_id: mission_id.to_owned(),
            asked: meta.map(|m| m.prompt).unwrap_or_default(),
            teammate_id: owner,
            name,
            why,
        }))
    }

    fn continues_from(&self, mission_id: &str) -> Result<Option<String>, String> {
        let Some(path) = self.mission_path(mission_id) else {
            return Ok(None);
        };
        Ok(ledger::read_metadata(&path).and_then(|m| m.parent))
    }

    fn team(&self) -> Result<Vec<ScoutTeammate>, String> {
        let file = crate::people::team(&self.store)?;
        let rows = file["teammates"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        Ok(rows
            .iter()
            .filter_map(|row| {
                let teammate_id = row["teammateId"].as_str()?.to_owned();
                let name = row["name"].as_str()?.to_owned();
                let role = row["roleTitle"]
                    .as_str()
                    .or_else(|| row["role"].as_str())
                    .unwrap_or("Custom")
                    .to_owned();
                Some(ScoutTeammate {
                    teammate_id,
                    name,
                    role,
                })
            })
            .collect())
    }

    fn ask(&self, teammate_id: &str, prompt: &str, model: Option<&str>) -> AskResult {
        let Some(folder) = self.workspace_folder() else {
            return AskResult::Refused(NO_WORKSPACE_MESSAGE.into());
        };
        let Some(entry) = self.teammate_entry(teammate_id) else {
            return AskResult::Refused(
                "Scout has no model to read with. Pick one for it on the Team screen.".into(),
            );
        };
        let route = &entry["route"];
        let Some(route_runtime) = route.get("runtime").and_then(Value::as_str) else {
            return AskResult::Refused(
                "Scout has no model to read with. Pick one for it on the Team screen.".into(),
            );
        };
        let route_model = route
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("account-default");
        let route_account = route.get("account").and_then(Value::as_str).map(str::to_owned);
        let route_effort = route.get("effort").and_then(Value::as_str).map(str::to_owned);

        let runtime = if model.is_some() {
            "opencode".to_owned()
        } else {
            route_runtime.to_owned()
        };
        let chosen = model.unwrap_or(route_model).to_owned();

        let roster = crate::native::roster_in(&self.store).unwrap_or_default();
        let Some(mut teammate) = roster.into_iter().find(|m| m.id == teammate_id) else {
            return AskResult::Refused(
                "Scout has no model to read with. Pick one for it on the Team screen.".into(),
            );
        };
        teammate.runtime = Some(runtime.clone());
        teammate.model = (chosen != "account-default").then(|| chosen.clone());
        teammate.account = if model.is_none() {
            route_account.clone()
        } else {
            None
        };

        let job = Send {
            teammate,
            conversation: None,
            prompt: prompt.to_owned(),
            model: (chosen != "account-default").then(|| chosen.clone()),
            effort: if model.is_none() { route_effort } else { None },
        };
        let origin = Origin {
            mode: Some(Mode::Ask),
            folder: Some(folder),
            started_by: Some(json!({ "kind": "scout" })),
            runtime: Some(runtime),
            account: if model.is_none() {
                route_account.map(Some)
            } else {
                Some(None)
            },
            slot: Some(SlotOrigin {
                key: format!("scout:{teammate_id}:{chosen}"),
                sealed: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        match crate::engine_api::api().send(job, origin) {
            Ok(started) => AskResult::Started {
                mission_id: started.conversation_id.clone(),
                run_id: started.conversation_id,
            },
            Err(why) => {
                if why.contains("already running") {
                    AskResult::Refused("Scout is still reading its last look.".into())
                } else {
                    AskResult::Refused(why)
                }
            }
        }
    }

    fn stop(&self, run_id: &str) {
        let _ = crate::engine_api::api().stop(run_id);
    }

    fn next_free_model(&self, teammate_id: &str, tried: &[String]) -> Option<String> {
        let entry = self.teammate_entry(teammate_id)?;
        let route = entry.get("route")?;
        let runtime = route.get("runtime").and_then(Value::as_str)?;
        let model = route.get("model").and_then(Value::as_str);
        if !is_free_route(runtime, model) {
            return None;
        }
        FREE_MODELS_BEST_FIRST
            .iter()
            .find(|&&id| Some(id) != model && !tried.iter().any(|t| t == id))
            .map(|&id| id.to_owned())
    }

    fn answer_of(&self, mission_id: &str) -> AnswerResult {
        let Some(path) = self.mission_path(mission_id) else {
            return AnswerResult::Failed("Scout’s look left no record to read.".into());
        };
        let transcript = ledger::read_transcript(&path);
        let stopped = transcript
            .items
            .iter()
            .any(|item| matches!(item, Item::Note { text, .. } if text == "Stopped"));
        let completed = !transcript.failed && !transcript.running && !stopped;
        let last_text = transcript
            .items
            .iter()
            .rev()
            .find_map(|item| match item {
                Item::Assistant { text, .. } if !text.trim().is_empty() => Some(text.trim()),
                _ => None,
            })
            .map(str::to_owned);
        if completed && let Some(text) = last_text {
            return AnswerResult::Text(text);
        }
        if completed {
            return AnswerResult::Failed("Scout’s model said nothing.".into());
        }
        let why = transcript.items.iter().rev().find_map(|item| match item {
            Item::Note { level, text } if level == "error" && !text.trim().is_empty() => {
                Some(clip_chars(text, ANSWER_ERR_MAX_CHARS))
            }
            _ => None,
        });
        match why {
            Some(reason) => AnswerResult::Failed(format!("Scout’s model did not answer: {reason}")),
            None => AnswerResult::Failed("Scout’s model did not answer.".into()),
        }
    }

    fn make_teammate(&self, route: &ScoutRoute) -> Result<String, String> {
        let file = crate::people::team(&self.store)?;
        let rows = file["teammates"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if let Some(before) = rows.iter().find(|mate| {
            mate["role"].as_str() == Some("Custom")
                && mate["roleTitle"].as_str() == Some("Keeps watch")
        }) && let Some(id) = before["teammateId"].as_str()
        {
            return Ok(id.to_owned());
        }
        let taken: HashSet<String> = rows
            .iter()
            .filter_map(|mate| mate["name"].as_str())
            .map(|n| n.to_lowercase())
            .collect();
        let name = SCOUT_NAMES
            .iter()
            .find(|&&candidate| !taken.contains(&candidate.to_lowercase()))
            .map(|&s| s.to_owned())
            .unwrap_or_else(|| format!("Scout {}", rows.len() + 1));

        let mut route_json = json!({
            "runtime": route.runtime,
            "model": route.model,
            "mode": "ask"
        });
        if let Some(account) = &route.account {
            route_json["account"] = json!(account);
        }
        crate::people::save_teammate_id(
            &self.store,
            json!({
                "name": name,
                "hue": "teal",
                "role": "Custom",
                "roleTitle": "Keeps watch",
                "route": route_json
            }),
        )
    }

    fn teammate_exists(&self, teammate_id: &str) -> Result<bool, String> {
        let file = crate::people::team(&self.store)?;
        let rows = file["teammates"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        Ok(rows
            .iter()
            .any(|mate| mate["teammateId"].as_str() == Some(teammate_id)))
    }

    fn folder_mark(&self) -> Option<String> {
        let folder = self.workspace_folder()?;
        let hook = self
            .git_hook
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        folder_mark_of(&folder, |args, cwd| match &hook {
            Some(cb) => cb(args, cwd),
            None => default_git(args, cwd),
        })
    }

    fn run_check(&self) -> Option<FolderCheckResult> {
        let folder = self.workspace_folder()?;
        let ws_id = crate::workspace_settings::workspace_id_for(&folder);
        let command = crate::edit_check::check_command_for(&self.store, Some(&ws_id))?;
        let run = crate::edit_check::run_check_command(
            &command,
            &folder,
            crate::edit_check::CHECK_TIMEOUT_MS,
        );
        let lines = if let Some(err) = &run.error {
            vec![err.clone()]
        } else if run.timed_out {
            vec!["The check ran past its time limit and was stopped.".to_owned()]
        } else {
            let all = crate::edit_check::comparable_lines(&run.output);
            let skip = all.len().saturating_sub(CHECK_TAIL_LINES);
            all.into_iter()
                .skip(skip)
                .map(|line| clip_ellipsis(&line, CHECK_LINE_MAX_CHARS))
                .collect()
        };
        let passed = run.error.is_none() && !run.timed_out && run.exit_code == Some(0);
        Some(FolderCheckResult {
            command,
            passed,
            lines,
        })
    }

    fn has_check_support(&self) -> bool {
        let Some(folder) = self.workspace_folder() else {
            return false;
        };
        let ws_id = crate::workspace_settings::workspace_id_for(&folder);
        crate::edit_check::check_command_for(&self.store, Some(&ws_id)).is_some()
    }

    fn folder_busy(&self) -> bool {
        !crate::engine_api::api().running().is_empty()
    }

    fn person_busy(&self) -> bool {
        for mission_id in crate::engine_api::api().running() {
            let Some(path) = self.mission_path(&mission_id) else {
                continue;
            };
            if let Some(meta) = ledger::read_metadata(&path)
                && meta.started_by_kind.is_none()
            {
                return true;
            }
        }
        false
    }

    fn brief_facts(&self, since: &str, looks: &[String]) -> Option<BriefRuns> {
        let from_dt = DateTime::parse_from_rfc3339(since).ok()?;
        let from_ts: SystemTime = from_dt.into();
        let own: HashSet<&str> = looks.iter().map(String::as_str).collect();
        let mut ended: Vec<(String, SystemTime, PathBuf)> = self
            .list_mission_files()
            .into_iter()
            .filter(|(id, mtime, _)| *mtime >= from_ts && !own.contains(id.as_str()))
            .collect();
        ended.sort_by(|a, b| b.1.cmp(&a.1));
        let more = ended.len() > BRIEF_MAX_MISSIONS;
        let mut finished = 0u32;
        let mut failed = 0u32;
        for (_, _, path) in ended.into_iter().take(BRIEF_MAX_MISSIONS) {
            let transcript = ledger::read_transcript(&path);
            let stopped = transcript
                .items
                .iter()
                .any(|item| matches!(item, Item::Note { text, .. } if text == "Stopped"));
            if transcript.failed {
                failed += 1;
            } else if !transcript.running && !stopped {
                finished += 1;
            }
        }
        Some(BriefRuns {
            finished,
            failed,
            more,
        })
    }
}

/// Live native Scout host owning a `Scout` engine + `ScoutHostDeps` registration.
pub struct ScoutHost {
    store: Store,
    pub deps: Arc<ScoutHostDeps>,
    pub scout: Arc<Scout>,
}

impl ScoutHost {
    pub fn start(store: Store) -> Self {
        let deps = ScoutHostDeps::new(store.clone());
        let scout = Scout::new(deps.clone());
        register_host(&store, &scout);
        Self { store, deps, scout }
    }

    pub fn start_with_scheduler(
        store: Store,
        scheduler: Arc<dyn ScoutScheduler>,
    ) -> Self {
        let deps = ScoutHostDeps::new(store.clone());
        let scout = Scout::with_scheduler(deps.clone(), scheduler);
        register_host(&store, &scout);
        Self { store, deps, scout }
    }
}

impl Drop for ScoutHost {
    fn drop(&mut self) {
        self.scout.dispose();
        unregister_host(&self.store, &self.scout);
    }
}
