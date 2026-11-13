//! THE ROUTINE RUNNER (main/routine-runner.ts), ported line by line.
//!
//! Step 1 starts as a new conversation for the teammate, and each later step starts as a follow-up of the same
//! conversation once the step before it COMPLETED -- or, when the step is handed to another teammate, as a
//! conversation of theirs, given the step before's answer. A step that failed, was stopped, was interrupted or asked
//! the person something ends the routine there. A checker step counts only if it approves. A step marked "wait for my
//! OK" holds before it starts until the person goes ahead. A standing goal runs the folder's check when the steps are
//! done and sends the same teammate back to fix it, a set number of times. A routine that works in a copy runs every
//! step there, and what it changed waits for Keep or Discard. A routine on a folder starts once per settled batch of
//! new files.
//!
//! Everything durable goes through [`Routines`] and every turn through [`Host::start`]; the live host
//! (`routine_host.rs`) starts turns with `engine_api::api()`. The runner keeps only what Electron keeps in RAM: the
//! steps it is following and the hour-long hold-off of a scheduled start that failed. One transition at a time: every
//! public call takes one lock, as Electron's `serial` queue does.
use crate::hand_off::{self, Verdict};
use crate::routine_copy::RoutineCopies;
use crate::routine_files;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// How long a scheduled routine waits after a start that failed before it is tried again.
pub const SCHEDULE_HOLD_OFF_MS: i64 = 3_600_000;

/// The routine store, as the runner uses it (`RoutineStore`'s run methods). Each write compares attempt ids.
pub trait Routines: Send + Sync {
    fn get(&self, id: &str) -> Result<Option<Value>, String>;
    fn list(&self) -> Result<Vec<Value>, String>;
    fn record_run(&self, id: &str, attempt: &str, staged: Option<Value>, failed: Option<String>) -> Result<(), String>;
    fn save_progress(&self, id: &str, execution: &Value, expected: Option<&str>) -> Result<(), String>;
    fn clear_progress(&self, id: &str, attempt: &str) -> Result<(), String>;
    fn abandon(&self, id: &str, attempt: &str) -> Result<(), String>;
    fn keep_schedule(&self, id: &str, attempt: &str, kept_at: &str) -> Result<(), String>;
    /// A slot that passed before this process opened.
    fn record_miss(&self, _id: &str, _due: &str, _recorded: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Who takes a step: the teammate's id and name (`MissionPeerContext.self`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub teammate_id: String,
    pub name: String,
}

/// How a finished turn ended, from its ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

/// One turn to start.
#[derive(Clone, Debug, PartialEq)]
pub struct StartRequest {
    pub prompt: String,
    pub runtime: String,
    pub mode: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
    pub peer: Peer,
    pub follow_up_of: Option<String>,
    /// `{kind: 'routine', routineId, step}`.
    pub started_by: Value,
    /// The routine's copy, when it works in one: the step runs there.
    pub cwd: Option<PathBuf>,
    /// The routine's own teammate, whose folder a step works in when there is no copy (native: one folder each).
    pub home: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Begun {
    pub mission_id: String,
    pub run_id: String,
}

/// A start that did not begin: refused with a code, or failed so that nobody can say whether it began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartError {
    Refused { code: String, message: String },
    Threw(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
}

/// What the runner tells the window (`CodexMissionUpdate`'s routine kinds).
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    MissionStarted { run_id: String, mission_id: String, teammate_id: String, prompt: String, routine_id: String, step: u64 },
    Notice { run_id: String, mission_id: String, message: String, level: Level },
    RecoveryChanged,
    Blocked { routine_id: String, name: String, message: String, retry_at: String },
}

/// What a standing goal's check said (0.534).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalCheck {
    pub command: String,
    pub passed: bool,
    /// The end of its output, for the teammate asked to fix it and for the person.
    pub tail: Vec<String>,
}

/// The new files a watching routine runs for (0.522), named to step 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrival {
    pub folder: String,
    pub files: Vec<String>,
}

/// A step the runner is following.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub routine_id: String,
    pub name: String,
    pub teammate_id: String,
    pub step: u64,
    pub of: u64,
    pub mission_id: String,
    pub run_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Continue,
    Abandon,
    Keep,
}

/// A person's decision on a held attempt (`RoutineRecoveryRequest`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryRequest {
    pub routine_id: String,
    pub attempt_id: String,
    pub step: u64,
    pub decision: Decision,
}

/// Everything else the runner asks of the app (`RoutineRunnerOptions`). A method left at its default is an option
/// Electron's runner was not given.
pub trait Host: Send + Sync {
    /// The folder the routine's teammate works in, as a workspace id. Electron has one per window; a native teammate
    /// has its own folder, so it is asked per routine (`home`: the routine's own teammate).
    fn workspace_id(&self, home: &str) -> String;
    fn peer(&self, teammate: &str) -> Option<Peer>;
    fn start(&self, request: StartRequest) -> Result<Begun, StartError>;
    fn assign_owner(&self, _teammate: &str, _mission: &str) {}
    /// How a finished turn ended. None when the ledger cannot say.
    fn phase_of(&self, mission: &str) -> Option<Phase>;
    /// Whether that turn is still RUNNING: the ledger reads "interrupted" for a turn with no ending yet.
    fn is_live(&self, _mission: &str) -> bool {
        false
    }
    /// Whether that turn ended by asking the person something. An error means "cannot tell", which holds.
    fn asked_a_question(&self, _mission: &str) -> Result<bool, String> {
        Err("cannot tell".into())
    }
    /// A teammate's route now: `{runtime, model, mode, effort?}`.
    fn route_of(&self, _teammate: &str) -> Option<Value> {
        None
    }
    /// A finished turn's answer.
    fn reply_of(&self, _mission: &str) -> Option<String> {
        None
    }
    fn notify(&self, update: Update);
    fn copies(&self) -> Option<&RoutineCopies> {
        None
    }
    /// The folder a copy is made from: the routine's teammate's.
    fn folder_now(&self, _home: &str) -> Option<PathBuf> {
        None
    }
    /// Whether a standing goal's check can be asked for at all (`goalCheck` given).
    fn has_goal_check(&self) -> bool {
        false
    }
    /// The folder's check command, run in `cwd`. None when the folder has no command set.
    fn goal_check(&self, _home: &str, _cwd: &Path) -> Option<GoalCheck> {
        None
    }
    /// Whether the teammate has a live run of anyone's. None: not asked (`teammateBusy` absent).
    fn teammate_busy(&self, _teammate: &str) -> Option<bool> {
        None
    }
    /// A run now waits for the person's OK before `step`.
    fn asks_first(&self, _name: Option<&str>, _routine: &str, _step: u64) {}
    fn arrivals_ready(&self, _routine: &str, _now: i64) -> Vec<String> {
        Vec::new()
    }
    fn arrivals_fired(&self, _routine: &str, _files: &[String], _now: i64) {}
    /// Where a routine was made (M15).
    fn home_of(&self, routine: &Value) -> Option<String> {
        routine["workspaceId"].as_str().map(str::to_owned)
    }
    /// When this process opened: a slot at or before it is missed, not started.
    fn opened_at(&self) -> Option<DateTime<Utc>> {
        None
    }
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Outer `Err`: what Electron throws (a store write that failed, a start that crashed). Inner `Err`: a refusal, said.
pub type Answer<T> = Result<Result<T, String>, String>;

fn iso(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn steps_of(r: &Value) -> Vec<String> {
    r["steps"].as_array().map(|s| s.iter().map(|s| text(s).to_owned()).collect()).unwrap_or_default()
}

fn execution_of(r: &Value) -> Option<Value> {
    r.get("execution").filter(|e| e.is_object()).cloned()
}

fn abandoned(e: &Option<Value>) -> bool {
    e.as_ref().is_some_and(|e| e["status"] == "abandoned")
}

/// Runtimes the runner can start: the ones whose events the host reads (`hostReadsEventsOf`).
pub fn can_start_routine(runtime: &str) -> bool {
    matches!(runtime, "kilo" | "acp" | "antigravity" | "codex" | "claude" | "cursor" | "opencode" | "copilot" | "muse")
}

fn phase_words(phase: Option<Phase>) -> &'static str {
    match phase {
        Some(Phase::Failed) => "that run failed",
        Some(Phase::Cancelled) => "that run was stopped",
        Some(Phase::Interrupted) => "that run was interrupted",
        Some(Phase::Completed) => "that run completed",
        None => "the record of that run could not be read",
    }
}

/// Who takes a step of a hand-off chain: its named teammate, or the routine's own.
fn owner_of(teammate: &str, hand_offs: &Value, step: u64) -> String {
    hand_offs[(step as usize).wrapping_sub(1)]["teammateId"].as_str().unwrap_or(teammate).to_owned()
}

/// Whether that step is the checker, whose approval the run needs.
fn checks(hand_offs: &Value, step: u64) -> bool {
    hand_offs[(step as usize).wrapping_sub(1)]["check"] == true
}

fn hand_offs(r: &Value) -> &Value {
    r.get("handOffs").unwrap_or(&Value::Null)
}

/// What a step's notice adds when it was handed on.
fn handed_to(to: &str, from: Option<&str>, check: bool) -> String {
    match (from, check) {
        (None, true) => format!(", {to} checks it"),
        (None, false) => String::new(),
        (Some(from), true) => format!(", handed to {to} with {from}'s answer to check"),
        (Some(from), false) => format!(", handed to {to} with {from}'s answer"),
    }
}

/// Why a step cannot run: its teammate was removed. Nothing has started when this is said.
fn removed_step_saying(routine: &Value, step: u64) -> String {
    let role = if checks(hand_offs(routine), step) { ", the step that checks the work," } else { "" };
    format!(
        "Step {step} of {}{role} was handed to a teammate who has since been removed, so nothing was started. Edit the routine to choose who takes it.",
        steps_of(routine).len()
    )
}

/// The fix a standing goal asks for, quoting what the check said (`goalFixPrompt`).
pub fn goal_fix_prompt(check: &GoalCheck, fix: u64, of: u64) -> String {
    let tail = if check.tail.is_empty() { "(it printed nothing)".to_owned() } else { check.tail.join("\n") };
    format!(
        "The check `{}` failed after the work above (fix {fix} of {of}). The end of its output:\n\n{tail}\n\nFix what it reports in this folder, then say in one or two sentences what you changed.",
        check.command
    )
}

/// `fillInputs`: resolve inputs, enforce folder-picker selection, substitute into steps, and check step length.
pub fn fill_inputs_with(
    routine: &Value,
    given: Option<&Value>,
    folder_was_chosen: Option<&dyn Fn(&str) -> bool>,
) -> Result<Value, String> {
    let Some(raw_inputs) = routine.get("inputs").and_then(Value::as_array) else {
        return Ok(routine.clone());
    };
    if raw_inputs.is_empty() {
        return Ok(routine.clone());
    }
    let inputs = crate::routine_inputs::parse_inputs_array(&Value::Array(raw_inputs.clone()))
        .ok_or_else(|| "The inputs for this routine could not be read.".to_string())?;
    let settled = crate::routine_inputs::resolve_values(&inputs, given)?;
    for input in &inputs {
        if input.kind == crate::routine_inputs::RoutineInputKind::Folder
            && let Some(path) = settled.get(&input.key)
            && !path.is_empty()
            && !folder_was_chosen.is_some_and(|check| check(path))
        {
            return Err(format!(
                "Choose \"{}\" with the folder picker before running.",
                input.label
            ));
        }
    }
    let steps: Vec<String> = routine["steps"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let filled = crate::routine_inputs::substitute_steps(&steps, &settled);
    for (i, step) in filled.iter().enumerate() {
        if step.encode_utf16().count() > 8000 {
            return Err(format!(
                "Step {} is too long once its inputs are filled in; shorten an answer or the step.",
                i + 1
            ));
        }
    }
    let mut out = routine.clone();
    out["steps"] = json!(filled);
    Ok(out)
}

/// `fillInputs` with no entered values (used by scheduled/automatic runs).
pub fn fill_inputs(routine: &Value) -> Result<Value, String> {
    fill_inputs_with(routine, None, None)
}

/// "2 files changed in its copy, waiting for you under Routines ..." (`waitingWords`).
fn waiting_words(staged: &Value) -> String {
    let count = staged["changed"].as_array().map_or(0, Vec::len) + staged["deleted"].as_array().map_or(0, Vec::len);
    let (s, them) = if count == 1 { ("", "it") } else { ("s", "them") };
    format!("{count} file{s} changed in its copy, waiting for you under Routines: Keep writes {them} into the folder, Discard throws {them} away")
}

/// The plan a run started with, over the routine as it is now (`startedPlan`): its steps, route, hand-offs, copy and
/// goal are the attempt's, so an edit made meanwhile applies from the next run.
fn started_plan(held: &Value, execution: &Value) -> Value {
    let mut plan = held.clone();
    if let Some(o) = plan.as_object_mut() {
        for key in ["handOffs", "inCopy", "untilCheck"] {
            o.remove(key);
        }
        o.insert("steps".into(), execution["steps"].clone());
        o.insert("route".into(), execution["route"].clone());
        for key in ["handOffs", "untilCheck"] {
            if let Some(v) = execution.get(key) {
                o.insert(key.into(), v.clone());
            }
        }
        if execution["inCopy"] == true {
            o.insert("inCopy".into(), json!(true));
        }
    }
    plan
}

/// A refusal that proves no dispatch: only these clear a first step's receipt.
const PREFLIGHT: &[&str] = &["INVALID_PROMPT", "RUN_ALREADY_ACTIVE", "CODEX_UNAVAILABLE", "SPEND_LIMIT_REACHED", "POLICY_REFUSED"];

struct Refusal {
    message: String,
}

struct Inner {
    routines: Arc<dyn Routines>,
    host: Arc<dyn Host>,
    /// The steps this process follows, in the order they started.
    active: Vec<Progress>,
    /// routineId -> ms before which a scheduled start is not tried again.
    held_off: HashMap<String, i64>,
}

/// `createRoutineRunner`.
pub struct RoutineRunner {
    inner: Mutex<Inner>,
}

impl RoutineRunner {
    pub fn new(routines: Arc<dyn Routines>, host: Arc<dyn Host>) -> Self {
        Self { inner: Mutex::new(Inner { routines, host, active: Vec::new(), held_off: HashMap::new() }) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The steps being followed now.
    pub fn running(&self) -> Vec<Progress> {
        self.lock().active.clone()
    }

    /// Whether this runner follows a step of any routine (the window then looks for its end more often).
    pub fn following(&self) -> bool {
        !self.lock().active.is_empty()
    }

    pub fn reconcile(&self) -> Result<(), String> {
        self.lock().reconcile()
    }

    pub fn recover(&self, request: &RecoveryRequest) -> Answer<()> {
        self.lock().recover(request)
    }

    /// `arrived`: the new files a watching routine runs for, named to step 1.
    pub fn run(&self, routine_id: &str, arrived: Option<&Arrival>) -> Answer<Begun> {
        self.lock().run_with_inputs(routine_id, arrived, None, None)
    }

    /// Run a routine with entered input values and optional folder-picker check.
    pub fn run_with_inputs(
        &self,
        routine_id: &str,
        arrived: Option<&Arrival>,
        given: Option<&Value>,
        folder_was_chosen: Option<&dyn Fn(&str) -> bool>,
    ) -> Answer<Begun> {
        self.lock()
            .run_with_inputs(routine_id, arrived, given, folder_was_chosen)
    }

    /// Any turn ending, however it ended. Only a routine's own current step moves it.
    pub fn on_run_ended(&self, mission_id: &str) -> Result<(), String> {
        self.lock().on_run_ended(mission_id)
    }

    /// The ends of the steps being followed, without the schedule: what `tick` does first, for a window that looks
    /// often. A live turn is not one that ended.
    pub fn ended(&self) -> Result<(), String> {
        self.lock().ended()
    }

    /// Start every scheduled routine whose next run has passed, as of `now`. Returns the ids started.
    pub fn tick(&self, now: DateTime<Utc>) -> Result<Vec<String>, String> {
        self.lock().tick(now)
    }
}

impl Inner {
    fn changed(&self) {
        self.host.notify(Update::RecoveryChanged);
    }

    fn forget(&mut self, routine_id: &str) {
        self.active.retain(|p| p.routine_id != routine_id);
    }

    fn name_of(&self, teammate: &str) -> Option<String> {
        self.host.peer(teammate).map(|p| p.name)
    }

    fn hold(&mut self, routine: &Value, execution: &Value, reason: &str, can_continue: bool, settled: bool, waits: bool) -> Result<(), String> {
        if execution["status"] == "held" && execution["reason"] == reason && execution.get("canContinue") == Some(&json!(can_continue)) {
            return Ok(());
        }
        let mut next = execution.clone();
        next["status"] = json!("held");
        next["reason"] = json!(reason);
        next["canContinue"] = json!(can_continue);
        if settled {
            next["settledAtDispatch"] = json!(true);
        }
        if waits {
            next["waitsForOk"] = json!(true);
        }
        let id = text(&routine["routineId"]).to_owned();
        self.routines.save_progress(&id, &next, Some(text(&execution["attemptId"])))?;
        self.forget(&id);
        self.changed();
        Ok(())
    }

    /// Whether a CHECKER step's answer approves, and why not when it does not.
    fn checker_said(&self, mission: &str, teammate: &str) -> (bool, String) {
        let verdict = hand_off::verdict_of(self.host.reply_of(mission).as_deref());
        let name = self.name_of(teammate).unwrap_or_else(|| "The checker".into());
        match verdict {
            Some(Verdict::Approved) => (true, format!("approved by {name}")),
            None => (false, format!("{name}, the checker, gave no verdict")),
            Some(Verdict::ChangesNeeded(changes)) => {
                let trimmed = changes.trim_end_matches(|c: char| c == '.' || c == '!' || c.is_whitespace());
                let said = if changes.is_empty() { String::new() } else { format!(": {trimmed}") };
                (false, format!("{name}, the checker, asked for changes{said}"))
            }
        }
    }

    /// A finished run: counted, and in a copy what it changed waits for the person in the same write. A copy that
    /// changed nothing is removed. Returns what is waiting, if anything.
    fn complete(&mut self, routine_id: &str, execution: &Value, failed: Option<String>) -> Result<Option<Value>, String> {
        let mut staged = None;
        let in_copy = execution["inCopy"] == true;
        if in_copy && let Some(copies) = self.host.copies() {
            if let (Ok(changes), Some(folder)) = (copies.changes(routine_id), copies.source(routine_id))
                && changes.changed.len() + changes.deleted.len() > 0
            {
                staged = Some(json!({"attemptId": execution["attemptId"], "finishedAt": iso(self.host.now()),
                    "folder": folder.to_string_lossy(), "changed": changes.changed, "deleted": changes.deleted}));
            }
        }
        self.routines.record_run(routine_id, text(&execution["attemptId"]), staged.clone(), failed)?;
        if in_copy && staged.is_none() && let Some(copies) = self.host.copies() {
            let _ = copies.discard(routine_id);
        }
        self.forget(routine_id);
        self.changed();
        Ok(staged)
    }

    // Policy (c): ask. A missing receipt is not proof of no side effects, so an attempt that cannot be confirmed is
    // held for the person on its card. Reconciliation is retried; the dispatch never is.
    fn reconcile(&mut self) -> Result<(), String> {
        for routine in self.routines.list()? {
            let id = text(&routine["routineId"]).to_owned();
            let Some(execution) = execution_of(&routine) else { continue };
            if execution["status"] == "abandoned" || self.active.iter().any(|p| p.routine_id == id) {
                continue;
            }
            // A refusal the dispatch already explained is not re-decided (`settledAtDispatch`).
            if execution["status"] == "held" && execution["settledAtDispatch"] == true {
                continue;
            }
            let home = text(&routine["teammateId"]).to_owned();
            if execution["workspaceId"] != self.host.workspace_id(&home).as_str() {
                self.hold(&routine, &execution, "Open the original workspace to reconcile this attempt. Nothing will be replayed.", false, false, false)?;
                continue;
            }
            let mission = execution["missionId"].as_str().map(str::to_owned);
            // A mission that is STILL RUNNING is not a mission to review.
            if mission.as_deref().is_some_and(|m| self.host.is_live(m)) {
                continue;
            }
            let phase = mission.as_deref().and_then(|m| self.host.phase_of(m));
            let question = mission.as_deref().is_none_or(|m| self.host.asked_a_question(m).unwrap_or(true));
            let confirmed = phase == Some(Phase::Completed) && !question;
            let step = execution["step"].as_u64().unwrap_or(1);
            let verdict = match &mission {
                Some(m) if confirmed && checks(hand_offs(&execution), step) => {
                    Some(self.checker_said(m, &owner_of(&home, hand_offs(&execution), step)))
                }
                _ => None,
            };
            if let Some((false, why)) = verdict {
                self.hold(&routine, &execution, &format!("{why}. The run does not count as done. Review the work, then run the routine again."), false, true, false)?;
            } else if confirmed && execution["step"] == execution["of"] {
                self.complete(&id, &execution, None)?;
            } else {
                let reason = if confirmed {
                    "The saved step completed. Review the remaining steps before continuing.".to_owned()
                } else if mission.is_none() {
                    "Dispatch outcome is uncertain: the app stopped before saving a mission receipt. Review external work; nothing will be replayed.".to_owned()
                } else {
                    let asked = if question && phase == Some(Phase::Completed) { ", but it asked a question or its answer could not be checked" } else { "" };
                    format!("Review required: {}{asked}. Nothing will be replayed.", phase_words(phase))
                };
                self.hold(&routine, &execution, &reason, confirmed, false, false)?;
            }
        }
        Ok(())
    }

    /// `info` while a routine goes well, `warning` when it stopped or waits for the person.
    fn notice(&self, progress: &Progress, message: String, level: Level) {
        self.host.notify(Update::Notice { run_id: progress.run_id.clone(), mission_id: progress.mission_id.clone(), message, level });
    }

    #[allow(clippy::too_many_arguments)]
    fn start_step(
        &mut self,
        routine: &Value,
        peer: &Peer,
        step: u64,
        follow_up_of: Option<String>,
        handed_from: Option<(String, String)>,
        arrived: Option<&Arrival>,
        fix: Option<(String, u64)>,
    ) -> Result<Result<Begun, Refusal>, String> {
        let id = text(&routine["routineId"]).to_owned();
        let steps = steps_of(routine);
        let saved = steps.get((step as usize).wrapping_sub(1)).cloned();
        let hook = routine["execution"]["hook"] == true;
        let prompt = match (&fix, &saved, arrived) {
            (Some((p, _)), _, _) => Some(p.clone()),
            (None, Some(s), Some(a)) if step == 1 => Some(format!("{}\n\n{s}", routine_files::arrival_note(&a.folder, &a.files))),
            (None, s, _) => s.clone(),
        };
        let Some(prompt) = prompt else {
            return Ok(Err(Refusal { message: format!("Routine has no step {step}.") }));
        };
        let prior = execution_of(routine);
        // A STEP TOO LONG TO SEND IS REFUSED HERE, BEFORE ANYTHING IS RECORDED (A5.1).
        if let Some(too_long) = hand_off::step_too_long_notice(&prompt) {
            let message = format!("Step {step} is too long to send: {too_long} Shorten it in Edit, then run the routine again. Nothing was started.");
            if step > 1 && let Some(prior) = prior.as_ref().filter(|p| p["status"] != "abandoned") {
                self.hold(routine, prior, &message, false, true, false)?;
            }
            return Ok(Err(Refusal { message }));
        }
        // A step handed to another teammate runs on THEIR route; the routine's own teammate runs on theirs now, in the
        // mode the routine was saved with (0.493).
        let home = text(&routine["teammateId"]).to_owned();
        let owner = owner_of(&home, hand_offs(routine), step);
        let route = if owner == home {
            match self.host.route_of(&owner) {
                Some(mut current) => {
                    current["mode"] = routine["route"]["mode"].clone();
                    Some(current)
                }
                None => Some(routine["route"].clone()),
            }
        } else {
            self.host.route_of(&owner)
        };
        let route = match route {
            Some(r) if can_start_routine(text(&r["runtime"])) => r,
            other => {
                let message = if other.is_none() {
                    format!("Step {step} goes to {}, whose route could not be read. Nothing was started.", peer.name)
                } else {
                    format!("Step {step} goes to {}, whose runtime routines cannot run on yet. Nothing was started.", peer.name)
                };
                if step > 1 && let Some(prior) = prior.as_ref().filter(|p| p["status"] != "abandoned") {
                    self.hold(routine, prior, &message, false, true, false)?;
                }
                return Ok(Err(Refusal { message }));
            }
        };
        let check = checks(hand_offs(routine), step);
        let sent = if fix.is_some() || (handed_from.is_none() && !check) {
            prompt.clone()
        } else {
            let answer = handed_from.as_ref().and_then(|(m, _)| self.host.reply_of(m));
            let from = handed_from.as_ref().map(|(_, name)| hand_off::From { name, answer: answer.as_deref() });
            let task = if step > 1 { steps.first().map(String::as_str) } else { None };
            hand_off::hand_off_prompt(&prompt, from, check, task)
        };
        let fresh = prior.is_none() || abandoned(&prior);
        let now = iso(self.host.now());
        let mut intent = serde_json::Map::new();
        if hook {
            intent.insert("hook".into(), json!(true));
        }
        let attempt = match &prior {
            Some(p) if !fresh => text(&p["attemptId"]).to_owned(),
            _ => crate::native::uuid_v4().map_err(|e| e.to_string())?,
        };
        intent.insert("attemptId".into(), json!(attempt));
        intent.insert("status".into(), json!("dispatching"));
        intent.insert("step".into(), json!(step));
        intent.insert("of".into(), json!(steps.len()));
        intent.insert("steps".into(), json!(steps));
        intent.insert("route".into(), routine["route"].clone());
        if let Some(h) = routine.get("handOffs") {
            intent.insert("handOffs".into(), h.clone());
        }
        if routine["inCopy"] == true {
            intent.insert("inCopy".into(), json!(true));
        }
        if let Some(tries) = routine["untilCheck"]["tries"].as_u64() {
            intent.insert("untilCheck".into(), json!({"tries": tries}));
        }
        if let Some((_, goal_try)) = &fix {
            intent.insert("goalTry".into(), json!(goal_try));
        }
        intent.insert("workspaceId".into(), json!(self.host.workspace_id(&home)));
        if prior.as_ref().is_some_and(|p| p["recovered"] == true) {
            intent.insert("recovered".into(), json!(true));
        }
        let started_at = match &prior {
            Some(p) if !fresh => p["startedAt"].clone(),
            _ => json!(now),
        };
        intent.insert("startedAt".into(), started_at);
        intent.insert("updatedAt".into(), json!(now));
        if let Some(f) = &follow_up_of {
            intent.insert("followUpOf".into(), json!(f));
        }
        let intent = Value::Object(intent);
        // Persist BEFORE spawn: a spawn whose id was not recorded stays uncertain and is never replayed.
        let expected = if abandoned(&prior) { None } else { prior.as_ref().map(|p| text(&p["attemptId"]).to_owned()) };
        self.routines.save_progress(&id, &intent, expected.as_deref())?;
        let cwd = if routine["inCopy"] == true { self.host.copies().and_then(|c| c.path(&id).ok()) } else { None };
        let request = StartRequest {
            prompt: sent,
            runtime: text(&route["runtime"]).to_owned(),
            mode: if hook { "ask".into() } else { text(&route["mode"]).to_owned() },
            model: route["model"].as_str().filter(|m| *m != "account-default").map(str::to_owned),
            effort: route["effort"].as_str().map(str::to_owned),
            account: route["account"].as_str().map(str::to_owned),
            peer: peer.clone(),
            follow_up_of,
            started_by: json!({"kind": "routine", "routineId": id, "step": step}),
            cwd,
            home,
        };
        match self.dispatch(routine, &intent, step, fresh, request) {
            Err(error) => {
                self.forget(&id);
                self.changed();
                Err(error)
            }
            ok => ok,
        }
    }

    /// The start itself and what it leaves on record (`startStep`'s try block).
    fn dispatch(&mut self, routine: &Value, intent: &Value, step: u64, fresh: bool, request: StartRequest) -> Result<Result<Begun, Refusal>, String> {
        let id = text(&routine["routineId"]).to_owned();
        let attempt = text(&intent["attemptId"]).to_owned();
        match self.host.start(request) {
            Err(StartError::Threw(error)) => Err(error),
            Ok(begun) => {
                let mut running = intent.clone();
                running["status"] = json!("running");
                running["missionId"] = json!(begun.mission_id);
                running["runId"] = json!(begun.run_id);
                self.routines.save_progress(&id, &running, Some(&attempt))?;
                Ok(Ok(begun))
            }
            Err(StartError::Refused { code, message }) => {
                if step == 1 && fresh && PREFLIGHT.contains(&code.as_str()) {
                    // Only preflight refusals prove no dispatch.
                    self.routines.clear_progress(&id, &attempt)?;
                } else if code == "SPEND_LIMIT_REACHED" {
                    self.hold(routine, intent, &format!("Step {step} was not started: {message}"), true, true, false)?;
                } else {
                    self.hold(routine, intent, &format!("Dispatch not confirmed for step {step}: {message}. Review external work before proceeding."), false, true, false)?;
                }
                Ok(Err(Refusal { message }))
            }
        }
    }

    /// `said`: what the turn was actually asked, when it is not the saved step -- a goal's fix.
    fn announce(&mut self, routine: &Value, peer: &Peer, step: u64, begun: &Begun, said: Option<String>) -> Progress {
        let id = text(&routine["routineId"]).to_owned();
        let steps = steps_of(routine);
        let progress = Progress {
            routine_id: id.clone(),
            name: text(&routine["name"]).to_owned(),
            teammate_id: peer.teammate_id.clone(),
            step,
            of: steps.len() as u64,
            mission_id: begun.mission_id.clone(),
            run_id: begun.run_id.clone(),
        };
        self.forget(&id);
        self.active.push(progress.clone());
        self.host.notify(Update::MissionStarted {
            run_id: begun.run_id.clone(),
            mission_id: begun.mission_id.clone(),
            teammate_id: peer.teammate_id.clone(),
            prompt: said.unwrap_or_else(|| steps.get(step as usize - 1).cloned().unwrap_or_default()),
            routine_id: id,
            step,
        });
        progress
    }

    fn recover(&mut self, request: &RecoveryRequest) -> Answer<()> {
        self.reconcile()?;
        let routine = self.routines.get(&request.routine_id)?;
        let execution = routine.as_ref().and_then(execution_of);
        let (Some(routine), Some(execution)) = (routine, execution) else {
            return Ok(Err(STALE.into()));
        };
        if execution["status"] != "held" || execution["attemptId"] != request.attempt_id.as_str() || execution["step"] != request.step {
            return Ok(Err(STALE.into()));
        }
        let id = text(&routine["routineId"]).to_owned();
        let attempt = text(&execution["attemptId"]).to_owned();
        match request.decision {
            Decision::Keep => {
                self.routines.keep_schedule(&id, &attempt, &iso(self.host.now()))?;
                self.changed();
                return Ok(Ok(()));
            }
            Decision::Abandon => {
                self.routines.abandon(&id, &attempt)?;
                // Put down: its copy goes with it. Nothing of it reached the folder.
                if execution["inCopy"] == true && let Some(copies) = self.host.copies() {
                    let _ = copies.discard(&id);
                }
                self.changed();
                return Ok(Ok(()));
            }
            Decision::Continue => {}
        }
        let Some(mission) = execution["missionId"].as_str().filter(|_| execution["canContinue"] == true).map(str::to_owned) else {
            return Ok(Err("The saved step is not confirmed complete. Review its mission and external work; it cannot be safely continued.".into()));
        };
        // Recovery uses the saved definition, not edits made four days later -- who takes each step included.
        let mut saved = started_plan(&routine, &execution);
        let mut recovered = execution.clone();
        recovered["recovered"] = json!(true);
        saved["execution"] = recovered;
        let step = request.step;
        let next = step + 1;
        let home = text(&routine["teammateId"]).to_owned();
        let owner = owner_of(&home, hand_offs(&saved), next);
        let before = owner_of(&home, hand_offs(&saved), step);
        if self.host.teammate_busy(&owner) == Some(true) {
            let who = self.name_of(&owner).unwrap_or_else(|| "This teammate".into());
            return Ok(Err(format!("{who} is busy. Wait for their current mission to finish.")));
        }
        let Some(peer) = self.host.peer(&owner) else {
            return Ok(Err(removed_step_saying(&saved, next)));
        };
        let check = checks(hand_offs(&saved), next);
        let handed_from = (owner != before || check)
            .then(|| (mission.clone(), self.name_of(&before).unwrap_or_else(|| "The teammate before".into())));
        let follow = (owner == before).then(|| mission.clone());
        let begun = match self.start_step(&saved, &peer, next, follow, handed_from.clone(), None, None)? {
            Ok(begun) => begun,
            Err(refusal) => return Ok(Err(refusal.message)),
        };
        self.host.assign_owner(&peer.teammate_id, &begun.mission_id);
        let started = self.announce(&saved, &peer, next, &begun, None);
        if let Some((_, from)) = &handed_from {
            let of = steps_of(&saved).len();
            self.notice(&started, format!("Routine \"{}\" · step {next} of {of}{}.", text(&saved["name"]), handed_to(&peer.name, Some(from), check)), Level::Info);
        }
        self.changed();
        Ok(Ok(()))
    }

    fn run(&mut self, routine_id: &str, arrived: Option<&Arrival>) -> Answer<Begun> {
        self.run_with_inputs(routine_id, arrived, None, None)
    }

    fn run_with_inputs(
        &mut self,
        routine_id: &str,
        arrived: Option<&Arrival>,
        given: Option<&Value>,
        folder_was_chosen: Option<&dyn Fn(&str) -> bool>,
    ) -> Answer<Begun> {
        self.reconcile()?;
        let Some(stored) = self.routines.get(routine_id)? else {
            return Ok(Err("That routine no longer exists.".into()));
        };
        let routine = match fill_inputs_with(&stored, given, folder_was_chosen) {
            Ok(r) => r,
            Err(message) => return Ok(Err(message)),
        };
        let id = text(&routine["routineId"]).to_owned();
        let name = text(&routine["name"]).to_owned();
        let execution = execution_of(&routine);
        if execution.is_some() && !abandoned(&execution) {
            return Ok(Err("This routine is already running or waiting for review. Open its routine card before starting more work.".into()));
        }
        let runtime = text(&routine["route"]["runtime"]);
        if !can_start_routine(runtime) {
            return Ok(Err(format!("Routines cannot run on {runtime} yet.")));
        }
        let home = text(&routine["teammateId"]).to_owned();
        let first = owner_of(&home, hand_offs(&routine), 1);
        if let Some(already) = self.active.iter().find(|p| p.teammate_id == first) {
            return Ok(Err(format!(
                "{} is already running for this teammate (step {} of {}). One routine at a time.",
                already.name, already.step, already.of
            )));
        }
        let Some(peer) = self.host.peer(&first) else {
            return Ok(Err("The teammate this routine belongs to no longer exists.".into()));
        };
        // EVERY STEP'S TEAMMATE, BEFORE ANY STEP RUNS (R12): a removed checker is refused before anything spends.
        let count = steps_of(&routine).len() as u64;
        for step in 2..=count {
            let owner = owner_of(&home, hand_offs(&routine), step);
            if owner == first || self.host.peer(&owner).is_some() {
                continue;
            }
            return Ok(Err(removed_step_saying(&routine, step)));
        }
        // IN A COPY: a fresh copy for this run, refused while the last run's changes still wait.
        let in_copy = routine["inCopy"] == true;
        if in_copy {
            if routine.get("staged").is_some() {
                return Ok(Err(format!("The last run's changes are waiting under Routines. Keep or Discard them, then run \"{name}\" again.")));
            }
            let (Some(copies), Some(folder)) = (self.host.copies(), self.host.folder_now(&home)) else {
                return Ok(Err("This routine works in a copy of the folder, which this window cannot make. Nothing was started.".into()));
            };
            if let Err(error) = copies.make(&id, &folder) {
                let said = match error.strip_prefix("It answers in a copy of the folder, and") {
                    Some(rest) => format!("It works in a copy of the folder, and{rest}"),
                    None => error,
                };
                return Ok(Err(format!("{said} Nothing was started.")));
            }
        }
        let begun = match self.start_step(&routine, &peer, 1, None, None, arrived, None)? {
            Ok(begun) => begun,
            Err(refusal) => {
                if in_copy && let Some(copies) = self.host.copies() {
                    let _ = copies.discard(&id);
                }
                return Ok(Err(refusal.message));
            }
        };
        self.host.assign_owner(&peer.teammate_id, &begun.mission_id);
        let progress = self.announce(&routine, &peer, 1, &begun, None);
        // What started it, said: the bubble shows the saved step, not the note the run was given.
        if let Some(arrived) = arrived {
            let which = if arrived.files.len() == 1 { "a new file".to_owned() } else { format!("{} new files", arrived.files.len()) };
            self.notice(&progress, format!("Started because {which} arrived: {}.", routine_files::named(&arrived.folder, &arrived.files)), Level::Info);
        }
        if count > 1 {
            self.notice(&progress, format!("Routine \"{name}\" · step 1 of {count}. Each next step starts when this one completes."), Level::Info);
        }
        Ok(Ok(begun))
    }

    fn on_run_ended(&mut self, mission_id: &str) -> Result<(), String> {
        let Some(progress) = self.active.iter().find(|p| p.mission_id == mission_id).cloned() else { return Ok(()) };
        let phase = self.host.phase_of(mission_id);
        let Some(stored) = self.routines.get(&progress.routine_id)? else {
            self.forget(&progress.routine_id);
            return Ok(());
        };
        let Some(execution) = execution_of(&stored).filter(|e| e["missionId"] == mission_id) else { return Ok(()) };
        let (name, step, of) = (progress.name.clone(), progress.step, progress.of);
        if phase != Some(Phase::Completed) {
            self.hold(&stored, &execution, phase_words(phase), false, false, false)?;
            self.notice(&progress, format!("Routine \"{name}\" stopped at step {step} of {of}: {}.", phase_words(phase)), Level::Warning);
            return Ok(());
        }
        // A step that ended by asking something did not finish, whatever its exit code says.
        if self.host.asked_a_question(mission_id).unwrap_or(true) {
            self.hold(&stored, &execution, "The step asked you something. Review its mission before proceeding.", false, false, false)?;
            self.notice(&progress, format!("Routine \"{name}\" stopped at step {step} of {of}: it asked you something. Answer it in that mission, then deal with the held attempt under Routines — this attempt cannot be continued, so running the routine again starts from step 1."), Level::Warning);
            return Ok(());
        }
        // A CHECKER step counts only if it approves; the hold is final.
        let mut approval = String::new();
        if execution.get("goalTry").is_none() && checks(hand_offs(&execution), step) {
            let (approved, why) = self.checker_said(mission_id, &progress.teammate_id);
            if !approved {
                self.hold(&stored, &execution, &format!("{why}. The run does not count as done. Review the work, then run the routine again."), false, true, false)?;
                self.notice(&progress, format!("Routine \"{name}\" stopped at step {step} of {of}: {why}."), Level::Warning);
                return Ok(());
            }
            approval = format!(", {why}");
        }
        let in_copy = execution["inCopy"] == true;
        let home = text(&stored["teammateId"]).to_owned();
        // A STANDING GOAL (0.534): with its steps done, the folder's check runs -- in the copy, for a routine in one.
        if step >= of && execution["hook"] != true && execution.get("untilCheck").is_some() && self.host.has_goal_check() {
            let cwd = if in_copy { self.host.copies().and_then(|c| c.path(&progress.routine_id).ok()) } else { self.host.folder_now(&home) };
            let checked = cwd.and_then(|cwd| self.host.goal_check(&home, &cwd));
            let fixes = execution["goalTry"].as_u64().unwrap_or(0);
            let allowed = execution["untilCheck"]["tries"].as_u64().unwrap_or(0);
            let after = if fixes == 0 { String::new() } else { format!(" after {fixes} fix{}", if fixes == 1 { "" } else { "es" }) };
            let Some(checked) = checked else {
                let staged = self.complete(&progress.routine_id, &execution, None)?;
                let waiting = staged.map(|s| format!(" {}.", waiting_words(&s))).unwrap_or_default();
                self.notice(&progress, format!("Routine \"{name}\" finished its steps, but this folder has no check command, so it was not checked. Set one in Settings > Project folder.{waiting}"), Level::Warning);
                return Ok(());
            };
            if checked.passed {
                let staged = self.complete(&progress.routine_id, &execution, None)?;
                let waiting = staged.map(|s| format!(" {}.", waiting_words(&s))).unwrap_or_default();
                self.notice(&progress, format!("Routine \"{name}\" finished: the check `{}` passed{after}.{waiting}", checked.command), Level::Info);
                return Ok(());
            }
            if fixes < allowed {
                let current = self.routines.get(&progress.routine_id)?;
                let mut plan = started_plan(current.as_ref().unwrap_or(&stored), &execution);
                plan["untilCheck"] = execution["untilCheck"].clone();
                let Some(peer) = self.host.peer(&progress.teammate_id) else {
                    self.hold(&stored, &execution, "The check failed, and the teammate who would fix it is gone.", false, true, false)?;
                    return Ok(());
                };
                // The fix turn is shown as what it was asked -- the check's own words -- not as the step again.
                let fix_prompt = goal_fix_prompt(&checked, fixes + 1, allowed);
                match self.start_step(&plan, &peer, of, Some(mission_id.to_owned()), None, None, Some((fix_prompt.clone(), fixes + 1)))? {
                    Err(refusal) => {
                        self.forget(&progress.routine_id);
                        self.notice(&progress, format!("Routine \"{name}\": the check failed, and the fix could not start: {}", refusal.message), Level::Warning);
                    }
                    Ok(begun) => {
                        self.host.assign_owner(&peer.teammate_id, &begun.mission_id);
                        // Said once, by the fix turn's own words, never again as a notice (0.814).
                        self.announce(&plan, &peer, of, &begun, Some(fix_prompt));
                    }
                }
                return Ok(());
            }
            let last = checked.tail.iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" / ");
            let said = format!(
                "the check `{}` still fails after {fixes} fix{}{}",
                checked.command,
                if fixes == 1 { "" } else { "es" },
                if checked.tail.is_empty() { String::new() } else { format!(": {last}") }
            );
            let capital = {
                let mut c = said.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            };
            if in_copy {
                let failed: String = hand_off::head16(&format!("{capital}."), 600).to_owned();
                let staged = self.complete(&progress.routine_id, &execution, Some(failed))?;
                let waiting = staged
                    .map(|s| format!(" {} -- whether work that does not pass is worth keeping is yours to judge.", waiting_words(&s)))
                    .unwrap_or_default();
                self.notice(&progress, format!("Routine \"{name}\" stopped: {said}.{waiting}"), Level::Warning);
                return Ok(());
            }
            self.hold(&stored, &execution, &format!("{capital}. The run does not count as done. Review the work, then run the routine again."), false, true, false)?;
            self.notice(&progress, format!("Routine \"{name}\" stopped: {said}. It does not count as done."), Level::Warning);
            return Ok(());
        }
        if step >= of {
            let staged = self.complete(&progress.routine_id, &execution, None)?;
            let tail = match staged {
                Some(s) => format!(" {}.", waiting_words(&s)),
                None if in_copy => " Nothing changed in its copy, so there is nothing to keep.".to_owned(),
                None => String::new(),
            };
            self.notice(&progress, format!("Routine \"{name}\" finished: {of} step{} completed{approval}.{tail}", if of == 1 { "" } else { "s" }), Level::Info);
            return Ok(());
        }
        // A RUN KEEPS THE STEPS IT STARTED WITH (0.493).
        let current = self.routines.get(&progress.routine_id)?;
        let routine = current.map(|c| started_plan(&c, &execution));
        let next = step + 1;
        let owner = routine.as_ref().map(|r| owner_of(&home, hand_offs(r), next));
        // WAITS FOR YOUR OK (0.840): held before it starts; Go ahead on the card starts it.
        if let (Some(r), Some(owner)) = (&routine, &owner)
            && r["handOffs"][(next - 1) as usize]["askFirst"] == true
        {
            let who = self.name_of(owner);
            self.hold(&stored, &execution, &format!("Step {next} waits for your OK. Read what step {step} did, then go ahead or stop here."), true, true, true)?;
            self.notice(&progress, format!("Routine \"{name}\" is waiting for your OK after step {step} of {of}. Go ahead with step {next} under Routines."), Level::Warning);
            self.host.asks_first(who.as_deref(), &name, next);
            return Ok(());
        }
        // HANDED TO A TEAMMATE WHO IS BUSY: held, not queued and not refused.
        if let (Some(_), Some(owner)) = (&routine, &owner)
            && *owner != progress.teammate_id
            && let Some(busy) = self.host.teammate_busy(owner)
            && busy
        {
            let who = self.name_of(owner).unwrap_or_else(|| "The next teammate".into());
            self.hold(&stored, &execution, &format!("Step {next} is {who}'s, and {who} is busy. Continue it here when they are free."), true, true, false)?;
            self.notice(&progress, format!("Routine \"{name}\" is waiting after step {step} of {of}: step {next} is {who}'s, who is busy. Continue it under Routines when they are free."), Level::Warning);
            return Ok(());
        }
        let peer = owner.as_deref().and_then(|o| self.host.peer(o));
        let (Some(routine), Some(owner), Some(peer)) = (routine, owner, peer) else {
            self.hold(&stored, &execution, "The routine or teammate changed while it was running.", false, false, false)?;
            self.notice(&progress, format!("Routine \"{name}\" stopped after step {step}: the routine changed while it was running."), Level::Warning);
            return Ok(());
        };
        let total = steps_of(&routine).len() as u64;
        if step >= total {
            self.hold(&stored, &execution, "The routine or teammate changed while it was running.", false, false, false)?;
            self.notice(&progress, format!("Routine \"{name}\" stopped after step {step}: the routine changed while it was running."), Level::Warning);
            return Ok(());
        }
        // Another teammate's step starts a conversation of its own, given what the step before answered.
        let check = checks(hand_offs(&routine), next);
        let handed_from = (owner != progress.teammate_id || check)
            .then(|| (mission_id.to_owned(), self.name_of(&progress.teammate_id).unwrap_or_else(|| "The teammate before".into())));
        let follow = (owner == progress.teammate_id).then(|| mission_id.to_owned());
        match self.start_step(&routine, &peer, next, follow, handed_from.clone(), None, None)? {
            Err(refusal) => {
                self.forget(&progress.routine_id);
                self.notice(&progress, format!("Routine \"{name}\" stopped before step {next} of {total}: {}", refusal.message), Level::Warning);
            }
            Ok(begun) => {
                self.host.assign_owner(&peer.teammate_id, &begun.mission_id);
                let started = self.announce(&routine, &peer, next, &begun, None);
                let from = handed_from.as_ref().map(|(_, n)| n.as_str());
                self.notice(&started, format!("Routine \"{}\" · step {next} of {total}{}.", text(&routine["name"]), handed_to(&peer.name, from, check)), Level::Info);
            }
        }
        Ok(())
    }

    fn ended(&mut self) -> Result<(), String> {
        // An end callback can fail on a transient disk error: retried here rather than left stuck in RAM.
        for progress in self.active.clone() {
            if self.host.is_live(&progress.mission_id) {
                continue;
            }
            if self.host.phase_of(&progress.mission_id).is_some() {
                self.on_run_ended(&progress.mission_id)?;
            }
        }
        Ok(())
    }

    fn tick(&mut self, now: DateTime<Utc>) -> Result<Vec<String>, String> {
        self.ended()?;
        self.reconcile()?;
        let mut started = Vec::new();
        let local = now.with_timezone(&chrono::Local);
        for routine in self.routines.list().unwrap_or_default() {
            let id = text(&routine["routineId"]).to_owned();
            let execution = execution_of(&routine);
            if execution.is_some() && !abandoned(&execution) {
                continue;
            }
            let Some(schedule) = routine.get("schedule").filter(|s| s.is_object()) else { continue };
            // Paused (0.705): not started, and not missed either.
            if routine["paused"] == true {
                continue;
            }
            // Its last run's changes still wait for Keep or Discard (0.533).
            if routine.get("staged").is_some() {
                continue;
            }
            let kind = text(&schedule["kind"]);
            // On a new file (0.522): due when the watcher has settled files for it and its hour is not full.
            let arrived = if kind == "files" { self.host.arrivals_ready(&id, now.timestamp_millis()) } else { Vec::new() };
            // A private link (`hook`) needs the relay native does not have: never due here.
            let due = match kind {
                "hook" => false,
                "files" => !arrived.is_empty(),
                _ => crate::routines::schedule::base(&routine)
                    .and_then(|base| crate::routines::schedule::next(schedule, base, local))
                    .is_some_and(|next| next <= local),
            };
            if !due {
                continue;
            }
            // M15: on its own, only in the folder it was made in.
            let home = text(&routine["teammateId"]).to_owned();
            if let Some(made_in) = self.host.home_of(&routine)
                && made_in != self.host.workspace_id(&home)
            {
                continue;
            }
            // No catch-up: a time that passed while Locust was closed is missed, recorded, and not started.
            if kind != "files" {
                let opened = self.host.opened_at().unwrap_or(DateTime::UNIX_EPOCH).with_timezone(&chrono::Local);
                if let Some(slot) = crate::routines::schedule::base(&routine)
                    .and_then(|base| crate::routines::schedule::missed(schedule, base, opened, local))
                {
                    self.routines.record_miss(&id, &iso(slot.with_timezone(&Utc)), &iso(now))?;
                    continue;
                }
            }
            if self.held_off.get(&id).is_some_and(|until| *until > now.timestamp_millis()) {
                continue;
            }
            self.held_off.remove(&id);
            // Busy is a wait, not a failure: tried again next tick, no hold-off.
            let first = owner_of(&home, hand_offs(&routine), 1);
            if self.active.iter().any(|p| p.teammate_id == first) || self.host.teammate_busy(&first) == Some(true) {
                continue;
            }
            let arrival = (kind == "files").then(|| Arrival { folder: text(&schedule["folder"]).to_owned(), files: arrived.clone() });
            match self.run(&id, arrival.as_ref())? {
                Ok(_) => {
                    started.push(id.clone());
                    if kind == "files" {
                        self.host.arrivals_fired(&id, &arrived, now.timestamp_millis());
                    }
                }
                // The workspace being full is a WAIT, like the teammate being busy, and is not announced.
                Err(message) if message.to_lowercase().contains("missions can run at once") => continue,
                Err(message) => {
                    let latest = self.routines.get(&id)?.as_ref().and_then(execution_of);
                    if latest.is_some() && !abandoned(&latest) {
                        continue;
                    }
                    let retry_at = now.timestamp_millis() + SCHEDULE_HOLD_OFF_MS;
                    self.held_off.insert(id.clone(), retry_at);
                    // Said, not just recorded: a scheduled run is the one nobody is watching start.
                    self.host.notify(Update::Blocked {
                        routine_id: id.clone(),
                        name: text(&routine["name"]).to_owned(),
                        message,
                        retry_at: iso(DateTime::from_timestamp_millis(retry_at).unwrap_or(now)),
                    });
                }
            }
        }
        Ok(started)
    }
}

const STALE: &str = "This recovery decision is stale. Reload routines and review the current attempt.";

#[cfg(test)]
#[path = "routine_runner_tests.rs"]
pub(crate) mod tests;
