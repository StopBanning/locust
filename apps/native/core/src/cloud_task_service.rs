//! Codex cloud task service (`apps/desktop/src/main/cloud-task-service.ts`):
//! persists cloud tasks started from Locust and drives start, refresh, diff,
//! and apply through injected `codex` and `git` runners.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::Utc;
use regex::Regex;
use serde_json::{Value, json};

use crate::cloud_tasks::{
    CloudEnvironment, CloudStartProblem, CloudState, CloudStatus, CloudTask, FolderOnGitHub, Ran,
    StartAnswer,
    environment_of, folder_on_github, leaving_no_log, refusal_for, start_answer, status_answer,
};

const KEEP: usize = 200;

fn valid_task_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^task_[A-Za-z0-9_]+$").expect("task id regex"))
}

fn config_warn_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)is ignored|unrecognized configuration").expect("config warn regex")
    })
}

pub struct CloudStartInput {
    pub folder: String,
    pub prompt: String,
    pub teammate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudStartResult {
    Ok { task: CloudTask, notes: Vec<String> },
    /// `no_environment` (27 Nov): the repository Codex Cloud has no environment for, so the window can say so where
    /// the person picks Cloud, before the next send (Electron `CloudStartResult.noEnvironment`).
    Err { message: String, no_environment: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudApplyResult {
    Ok { task: CloudTask },
    Err { message: String },
}

pub struct CloudTaskService<'a> {
    file: PathBuf,
    codex: Box<dyn Fn(&[&str], &Path) -> Result<Ran, String> + 'a>,
    git: Box<dyn Fn(&[&str], &Path) -> Result<Ran, String> + 'a>,
    ready: Box<dyn Fn() -> bool + 'a>,
    now: Box<dyn Fn() -> String + 'a>,
}

/// Where the window keeps its Codex cloud tasks (Electron's `userData/cloud-tasks.json`).
pub fn store_path_of(store: &crate::store::Store) -> PathBuf {
    store.root().join("cloud-tasks.json")
}

/// This computer's Codex cloud task service (Electron's `createCloudTaskService` in `index.ts`):
/// keeps tasks in `store_path_of(store)`, runs `codex` wrapped in `leaving_no_log`, and runs `git`.
pub fn on_this_computer(store: &crate::store::Store) -> CloudTaskService<'static> {
    let codex = leaving_no_log(|args: &[&str], cwd: &Path| {
        let Ok(path) = crate::runtime::discovery::find() else {
            return Ok(Ran::new(127, "", "Codex CLI is not installed or not signed in."));
        };
        let out = std::process::Command::new(path)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|e| e.to_string())?;
        Ok(Ran::new(
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        ))
    });
    let git = |args: &[&str], cwd: &Path| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|e| e.to_string())?;
        Ok(Ran::new(
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        ))
    };
    CloudTaskService::from_runners(store_path_of(store), codex, git)
        .with_ready(|| crate::runtime::discovery::find().is_ok())
}

impl<'a> CloudTaskService<'a> {
    pub fn new(
        file: impl Into<PathBuf>,
        codex: &'a dyn Fn(&[&str], &Path) -> Result<Ran, String>,
        git: &'a dyn Fn(&[&str], &Path) -> Result<Ran, String>,
    ) -> Self {
        Self::from_runners(
            file,
            move |args, cwd| codex(args, cwd),
            move |args, cwd| git(args, cwd),
        )
    }

    pub fn from_runners(
        file: impl Into<PathBuf>,
        codex: impl Fn(&[&str], &Path) -> Result<Ran, String> + 'a,
        git: impl Fn(&[&str], &Path) -> Result<Ran, String> + 'a,
    ) -> Self {
        Self {
            file: file.into(),
            codex: Box::new(codex),
            git: Box::new(git),
            ready: Box::new(|| true),
            now: Box::new(|| Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        }
    }

    pub fn with_ready(mut self, ready: impl Fn() -> bool + 'a) -> Self {
        self.ready = Box::new(ready);
        self
    }

    pub fn with_now(mut self, now: impl Fn() -> String + 'a) -> Self {
        self.now = Box::new(now);
        self
    }

    pub fn codex_ready(&self) -> bool {
        (self.ready)()
    }

    pub fn environment(&self, repo: &str, folder: &Path) -> CloudEnvironment {
        environment_of(&*self.codex, repo, folder)
    }

    fn read(&self) -> Vec<CloudTask> {
        let Ok(text) = fs::read_to_string(&self.file) else {
            return Vec::new();
        };
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        items.iter().filter_map(task_from_json).collect()
    }

    fn write(&self, tasks: &[CloudTask]) -> Result<(), String> {
        let start = tasks.len().saturating_sub(KEEP);
        let kept: Vec<Value> = tasks[start..].iter().map(task_to_json).collect();
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = serde_json::to_string(&kept).map_err(|e| e.to_string())?;
        fs::write(&self.file, body).map_err(|e| e.to_string())
    }

    fn replace(&self, task: &CloudTask) -> Result<(), String> {
        let mut tasks = self.read();
        for entry in &mut tasks {
            if entry.task_id == task.task_id {
                *entry = task.clone();
            }
        }
        self.write(&tasks)
    }

    fn find(&self, task_id: &str) -> Option<CloudTask> {
        self.read().into_iter().find(|t| t.task_id == task_id)
    }

    pub fn list(&self, folder: Option<&str>) -> Vec<CloudTask> {
        let tasks = self.read();
        match folder {
            None => tasks,
            Some(target) => {
                let want = target.to_ascii_lowercase();
                tasks
                    .into_iter()
                    .filter(|t| t.folder.to_ascii_lowercase() == want)
                    .collect()
            }
        }
    }

    pub fn where_folder(&self, folder: &Path) -> FolderOnGitHub {
        folder_on_github(folder, &*self.git)
    }

    pub fn start(&self, input: CloudStartInput) -> CloudStartResult {
        let prompt = input.prompt.trim().to_string();
        if prompt.is_empty() {
            return CloudStartResult::Err {
                message: "Say what the cloud task should do.".into(),
                no_environment: None,
            };
        }
        let folder_path = Path::new(&input.folder);
        let where_gh = folder_on_github(folder_path, &*self.git);
        let Some(repo) = where_gh.repo else {
            return CloudStartResult::Err {
                message: "A cloud task works on a GitHub repository, and this folder has none. Push it to GitHub first, or send it here instead.".into(),
                no_environment: None,
            };
        };

        let mut notes = Vec::new();
        if let Some(unpushed) = where_gh.unpushed
            && unpushed > 0
        {
            let branch = where_gh.branch.as_deref().unwrap_or("this branch");
            let (noun, verb, pron) = if unpushed == 1 {
                ("commit", "is", "it")
            } else {
                ("commits", "are", "them")
            };
            notes.push(format!(
                "{unpushed} {noun} on {branch} {verb} not on GitHub, so the cloud works without {pron}."
            ));
        }
        if where_gh.dirty {
            notes.push(
                "Uncommitted changes in this folder stay here: the cloud works from what GitHub has."
                    .into(),
            );
        }

        let mut args: Vec<&str> = vec!["cloud", "exec", "--env", &repo];
        if let Some(branch) = where_gh.branch.as_deref() {
            args.push("--branch");
            args.push(branch);
        }
        args.push(&prompt);

        let ran = (self.codex)(&args, folder_path).unwrap_or_else(|err| Ran::new(1, "", err));
        let (task_id, url) = match start_answer(&ran, &repo) {
            StartAnswer::Ok { task_id, url } => (task_id, url),
            StartAnswer::Err(problem) => {
                let no_environment = match &problem {
                    CloudStartProblem::NoEnvironment { repo } => Some(repo.clone()),
                    CloudStartProblem::Other { .. } => None,
                };
                return CloudStartResult::Err {
                    message: refusal_for(&problem),
                    no_environment,
                };
            }
        };

        let task = CloudTask {
            task_id,
            url,
            runtime: "codex".into(),
            repo,
            branch: where_gh.branch,
            prompt,
            folder: input.folder,
            created_at: (self.now)(),
            teammate_id: input.teammate_id,
            status: CloudStatus {
                state: CloudState::Pending,
                title: None,
                added: None,
                removed: None,
                files: None,
            },
            applied_at: None,
        };
        let mut tasks = self.read();
        tasks.push(task.clone());
        let _ = self.write(&tasks);
        CloudStartResult::Ok { task, notes }
    }

    pub fn refresh(&self, task_id: &str) -> Option<CloudTask> {
        let task = self.find(task_id)?;
        if task.applied_at.is_some() {
            return Some(task);
        }
        let Ok(ran) = (self.codex)(&["cloud", "status", task_id], Path::new(&task.folder)) else {
            return Some(task);
        };
        let status = status_answer(&format!("{}\n{}", ran.stdout, ran.stderr));
        if status.state == CloudState::Unknown {
            return Some(task);
        }
        let next = CloudTask { status, ..task };
        let _ = self.replace(&next);
        Some(next)
    }

    pub fn diff(&self, task_id: &str) -> Option<String> {
        let task = self.find(task_id)?;
        let ran = (self.codex)(&["cloud", "diff", task_id], Path::new(&task.folder)).ok()?;
        let has_diff = ran.stdout.lines().any(|l| l.starts_with("diff --git "));
        if ran.code == 0 && has_diff {
            Some(ran.stdout)
        } else {
            None
        }
    }

    pub fn apply(&self, task_id: &str) -> CloudApplyResult {
        let Some(task) = self.find(task_id) else {
            return CloudApplyResult::Err {
                message: "That cloud task is not one Locust started.".into(),
            };
        };
        if task.applied_at.is_some() {
            return CloudApplyResult::Err {
                message: "Its change is already in this folder.".into(),
            };
        }
        let ran = (self.codex)(&["cloud", "apply", task_id], Path::new(&task.folder))
            .unwrap_or_else(|err| Ran::new(1, "", err));
        if ran.code != 0 {
            let combined = format!("{}\n{}", ran.stderr, ran.stdout);
            let said = combined
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !config_warn_re().is_match(line))
                .last()
                .unwrap_or("Codex did not say why");
            return CloudApplyResult::Err {
                message: format!(
                    "The change could not be applied here: {said}. Nothing in the folder was changed by Locust."
                ),
            };
        }
        let mut status = task.status.clone();
        status.state = CloudState::Applied;
        let next = CloudTask {
            applied_at: Some((self.now)()),
            status,
            ..task
        };
        let _ = self.replace(&next);
        CloudApplyResult::Ok { task: next }
    }
}

fn task_to_json(task: &CloudTask) -> Value {
    let mut status = serde_json::Map::new();
    status.insert("state".into(), json!(task.status.state.as_str()));
    if let Some(title) = &task.status.title {
        status.insert("title".into(), json!(title));
    }
    if let Some(added) = task.status.added {
        status.insert("added".into(), json!(added));
    }
    if let Some(removed) = task.status.removed {
        status.insert("removed".into(), json!(removed));
    }
    if let Some(files) = task.status.files {
        status.insert("files".into(), json!(files));
    }

    let mut obj = serde_json::Map::new();
    obj.insert("taskId".into(), json!(task.task_id));
    obj.insert("url".into(), json!(task.url));
    obj.insert("runtime".into(), json!(task.runtime));
    obj.insert("repo".into(), json!(task.repo));
    if let Some(branch) = &task.branch {
        obj.insert("branch".into(), json!(branch));
    }
    obj.insert("prompt".into(), json!(task.prompt));
    obj.insert("folder".into(), json!(task.folder));
    obj.insert("createdAt".into(), json!(task.created_at));
    if let Some(tm) = &task.teammate_id {
        obj.insert("teammateId".into(), json!(tm));
    }
    obj.insert("status".into(), Value::Object(status));
    if let Some(applied_at) = &task.applied_at {
        obj.insert("appliedAt".into(), json!(applied_at));
    }
    Value::Object(obj)
}

fn task_from_json(value: &Value) -> Option<CloudTask> {
    let obj = value.as_object()?;
    let task_id = obj.get("taskId")?.as_str()?;
    if !valid_task_id_re().is_match(task_id) {
        return None;
    }
    let status_obj = obj.get("status").and_then(Value::as_object);
    let state = status_obj
        .and_then(|s| s.get("state"))
        .and_then(Value::as_str)
        .and_then(CloudState::from_str_opt)
        .unwrap_or(CloudState::Unknown);
    let status = CloudStatus {
        state,
        title: status_obj
            .and_then(|s| s.get("title"))
            .and_then(Value::as_str)
            .map(str::to_string),
        added: status_obj.and_then(|s| s.get("added")).and_then(Value::as_u64),
        removed: status_obj
            .and_then(|s| s.get("removed"))
            .and_then(Value::as_u64),
        files: status_obj.and_then(|s| s.get("files")).and_then(Value::as_u64),
    };
    Some(CloudTask {
        task_id: task_id.to_string(),
        url: obj
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        runtime: obj
            .get("runtime")
            .and_then(Value::as_str)
            .unwrap_or("codex")
            .to_string(),
        repo: obj
            .get("repo")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        branch: obj
            .get("branch")
            .and_then(Value::as_str)
            .map(str::to_string),
        prompt: obj
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        folder: obj
            .get("folder")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        created_at: obj
            .get("createdAt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        teammate_id: obj
            .get("teammateId")
            .and_then(Value::as_str)
            .map(str::to_string),
        status,
        applied_at: obj
            .get("appliedAt")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

#[cfg(test)]
#[path = "cloud_task_service_tests.rs"]
mod tests;
