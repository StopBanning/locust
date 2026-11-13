//! Electron's runner tests, carried over. Each `mod` is one TypeScript file; the harness here is theirs: an in-memory
//! routine map (`Mem`) and a host whose every answer the test sets (`Fake`), or the native store itself where the
//! TypeScript test used the real store (routine-recovery.test.ts).
use super::*;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};

type PeerFn = Box<dyn Fn(&str) -> Option<Peer> + Send + Sync>;
type RouteFn = Box<dyn Fn(&str) -> Option<Value> + Send + Sync>;
type StartOnce = Box<dyn FnOnce(&StartRequest) -> Result<Begun, StartError> + Send>;

pub(crate) fn peer(id: &str, name: &str) -> Peer {
    Peer { teammate_id: id.into(), name: name.into() }
}

/// Every option of `RoutineRunnerOptions` a test sets.
pub(crate) struct Fake {
    pub workspace: Mutex<String>,
    pub peers: PeerFn,
    pub routes: Option<RouteFn>,
    pub starts: Mutex<Vec<StartRequest>>,
    pub counter: Mutex<usize>,
    pub refuse: Mutex<Option<StartError>>,
    pub once: Mutex<VecDeque<StartOnce>>,
    pub on_start: Option<Box<dyn Fn(&StartRequest) + Send + Sync>>,
    pub phases: Mutex<HashMap<String, Phase>>,
    pub live: Mutex<HashSet<String>>,
    /// None: `askedAQuestion` not given.
    pub asked: Mutex<Option<HashSet<String>>>,
    pub asked_throws: AtomicBool,
    pub replies: Mutex<HashMap<String, String>>,
    /// None: `teammateBusy` not given.
    pub busy: Mutex<Option<HashSet<String>>>,
    pub updates: Mutex<Vec<Update>>,
    pub asks: Mutex<Vec<(Option<String>, String, u64)>>,
    pub copies: Option<RoutineCopies>,
    pub folder: Option<PathBuf>,
    pub goal: Option<Mutex<(Vec<GoalCheck>, usize)>>,
    pub ready: Mutex<Vec<String>>,
    pub fired: Mutex<Vec<Vec<String>>>,
    pub home: Option<Box<dyn Fn(&Value) -> Option<String> + Send + Sync>>,
    pub opened_at: Option<DateTime<Utc>>,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            workspace: Mutex::new("ws_test".into()),
            peers: Box::new(|_| Some(peer("tm_wren", "Wren"))),
            routes: None,
            starts: Mutex::default(),
            counter: Mutex::default(),
            refuse: Mutex::default(),
            once: Mutex::default(),
            on_start: None,
            phases: Mutex::default(),
            live: Mutex::default(),
            asked: Mutex::new(Some(HashSet::new())),
            asked_throws: AtomicBool::new(false),
            replies: Mutex::default(),
            busy: Mutex::default(),
            updates: Mutex::default(),
            asks: Mutex::default(),
            copies: None,
            folder: None,
            goal: None,
            ready: Mutex::default(),
            fired: Mutex::default(),
            home: None,
            opened_at: None,
        }
    }
}

impl Fake {
    pub fn starts(&self) -> Vec<StartRequest> {
        self.starts.lock().unwrap().clone()
    }
    pub fn notices(&self) -> Vec<String> {
        self.updates.lock().unwrap().iter().filter_map(|u| if let Update::Notice { message, .. } = u { Some(message.clone()) } else { None }).collect()
    }
    pub fn last_notice(&self) -> String {
        self.notices().last().cloned().unwrap_or_default()
    }
    pub fn phase(&self, mission: &str, phase: Phase) {
        self.phases.lock().unwrap().insert(mission.into(), phase);
    }
    pub fn reply(&self, mission: &str, reply: &str) {
        self.replies.lock().unwrap().insert(mission.into(), reply.into());
    }
    pub fn blocked(&self) -> Vec<Update> {
        self.updates.lock().unwrap().iter().filter(|u| matches!(u, Update::Blocked { .. })).cloned().collect()
    }
}

impl Host for Fake {
    fn workspace_id(&self, _home: &str) -> String {
        self.workspace.lock().unwrap().clone()
    }
    fn peer(&self, teammate: &str) -> Option<Peer> {
        (self.peers)(teammate)
    }
    fn start(&self, request: StartRequest) -> Result<Begun, StartError> {
        self.starts.lock().unwrap().push(request.clone());
        if let Some(hook) = &self.on_start {
            hook(&request);
        }
        if let Some(once) = self.once.lock().unwrap().pop_front() {
            return once(&request);
        }
        if let Some(refused) = self.refuse.lock().unwrap().clone() {
            return Err(refused);
        }
        let mut n = self.counter.lock().unwrap();
        *n += 1;
        Ok(Begun { mission_id: format!("mission_{n}"), run_id: format!("run_{n}") })
    }
    fn phase_of(&self, mission: &str) -> Option<Phase> {
        self.phases.lock().unwrap().get(mission).copied()
    }
    fn is_live(&self, mission: &str) -> bool {
        self.live.lock().unwrap().contains(mission)
    }
    fn asked_a_question(&self, mission: &str) -> Result<bool, String> {
        if self.asked_throws.load(Ordering::SeqCst) {
            return Err("unreadable".into());
        }
        match &*self.asked.lock().unwrap() {
            None => Err("not given".into()),
            Some(set) => Ok(set.contains(mission)),
        }
    }
    fn route_of(&self, teammate: &str) -> Option<Value> {
        self.routes.as_ref().and_then(|r| r(teammate))
    }
    fn reply_of(&self, mission: &str) -> Option<String> {
        self.replies.lock().unwrap().get(mission).cloned()
    }
    fn notify(&self, update: Update) {
        self.updates.lock().unwrap().push(update);
    }
    fn copies(&self) -> Option<&RoutineCopies> {
        self.copies.as_ref()
    }
    fn folder_now(&self, _home: &str) -> Option<PathBuf> {
        self.folder.clone()
    }
    fn has_goal_check(&self) -> bool {
        self.goal.is_some()
    }
    fn goal_check(&self, _home: &str, _cwd: &Path) -> Option<GoalCheck> {
        let mut goal = self.goal.as_ref()?.lock().unwrap();
        let (checks, at) = &mut *goal;
        let found = checks.get((*at).min(checks.len() - 1)).cloned();
        *at += 1;
        found
    }
    fn teammate_busy(&self, teammate: &str) -> Option<bool> {
        self.busy.lock().unwrap().as_ref().map(|b| b.contains(teammate))
    }
    fn asks_first(&self, name: Option<&str>, routine: &str, step: u64) {
        self.asks.lock().unwrap().push((name.map(str::to_owned), routine.into(), step));
    }
    fn arrivals_ready(&self, _routine: &str, _now: i64) -> Vec<String> {
        self.ready.lock().unwrap().clone()
    }
    fn arrivals_fired(&self, _routine: &str, files: &[String], _now: i64) {
        self.fired.lock().unwrap().push(files.to_vec());
    }
    fn home_of(&self, routine: &Value) -> Option<String> {
        match &self.home {
            Some(f) => f(routine),
            None => routine["workspaceId"].as_str().map(str::to_owned),
        }
    }
    fn opened_at(&self) -> Option<DateTime<Utc>> {
        self.opened_at
    }
}

/// The TypeScript harnesses' `held` map, and what they record.
#[derive(Default)]
pub(super) struct Mem {
    pub held: Mutex<Vec<Value>>,
    pub runs: Mutex<Vec<String>>,
    pub staged: Mutex<Vec<Option<Value>>>,
    pub failures: Mutex<Vec<Option<String>>>,
    pub kept: Mutex<Vec<String>>,
    pub misses: Mutex<Vec<(String, String, String)>>,
    /// `lastRunAt` a counted run is given (the scheduled harness's clock); None: the wall clock.
    pub clock: Mutex<Option<String>>,
}

impl Mem {
    pub fn with(routines: Vec<Value>) -> Arc<Self> {
        Arc::new(Self { held: Mutex::new(routines), ..Self::default() })
    }
    pub fn get_one(&self, id: &str) -> Value {
        self.held.lock().unwrap().iter().find(|r| r["routineId"] == id).cloned().unwrap_or(Value::Null)
    }
    pub fn execution(&self, id: &str) -> Value {
        self.get_one(id)["execution"].clone()
    }
    pub fn edit(&self, id: &str, f: impl FnOnce(&mut Value)) {
        let mut held = self.held.lock().unwrap();
        if let Some(r) = held.iter_mut().find(|r| r["routineId"] == id) {
            f(r);
        }
    }
    pub fn runs(&self) -> Vec<String> {
        self.runs.lock().unwrap().clone()
    }
}

fn remove(r: &mut Value, keys: &[&str]) {
    if let Some(o) = r.as_object_mut() {
        for k in keys {
            o.remove(*k);
        }
    }
}

impl Routines for Mem {
    fn get(&self, id: &str) -> Result<Option<Value>, String> {
        Ok(Some(self.get_one(id)).filter(|r| !r.is_null()))
    }
    fn list(&self) -> Result<Vec<Value>, String> {
        Ok(self.held.lock().unwrap().clone())
    }
    fn record_run(&self, id: &str, _attempt: &str, staged: Option<Value>, failed: Option<String>) -> Result<(), String> {
        self.runs.lock().unwrap().push(id.into());
        self.staged.lock().unwrap().push(staged.clone());
        self.failures.lock().unwrap().push(failed.clone());
        let at = self.clock.lock().unwrap().clone().unwrap_or_else(|| iso(Utc::now()));
        self.edit(id, |r| {
            remove(r, &["execution", "lastFailed"]);
            r["runs"] = json!(r["runs"].as_u64().unwrap_or(0) + 1);
            r["lastRunAt"] = json!(at);
            if let Some(s) = staged {
                r["staged"] = s;
            }
            if let Some(f) = failed {
                r["lastFailed"] = json!(f);
            }
        });
        Ok(())
    }
    fn save_progress(&self, id: &str, execution: &Value, _expected: Option<&str>) -> Result<(), String> {
        self.edit(id, |r| r["execution"] = execution.clone());
        Ok(())
    }
    fn clear_progress(&self, id: &str, _attempt: &str) -> Result<(), String> {
        self.edit(id, |r| remove(r, &["execution"]));
        Ok(())
    }
    fn abandon(&self, id: &str, _attempt: &str) -> Result<(), String> {
        self.edit(id, |r| {
            remove(r, &["schedule"]);
            r["execution"]["status"] = json!("abandoned");
        });
        Ok(())
    }
    fn keep_schedule(&self, id: &str, _attempt: &str, kept_at: &str) -> Result<(), String> {
        self.kept.lock().unwrap().push(id.into());
        self.edit(id, |r| {
            remove(r, &["execution"]);
            r["lastRunAt"] = json!(kept_at);
        });
        Ok(())
    }
    fn record_miss(&self, id: &str, due: &str, recorded: &str) -> Result<(), String> {
        self.misses.lock().unwrap().push((id.into(), due.into(), recorded.into()));
        Ok(())
    }
}

/// A routine: `base` with `over`'s keys laid on top (a null removes one).
pub(super) fn with(base: Value, over: Value) -> Value {
    let mut out = base;
    if let (Some(o), Some(over)) = (out.as_object_mut(), over.as_object()) {
        for (k, v) in over {
            if v.is_null() {
                o.remove(k);
            } else {
                o.insert(k.clone(), v.clone());
            }
        }
    }
    out
}

pub(super) fn runner(routines: &Arc<Mem>, host: &Arc<Fake>) -> RoutineRunner {
    RoutineRunner::new(routines.clone(), host.clone())
}

fn ok<T>(answer: Answer<T>) -> T {
    match answer {
        Ok(Ok(v)) => v,
        Ok(Err(refused)) => panic!("refused: {refused}"),
        Err(thrown) => panic!("threw: {thrown}"),
    }
}

fn refused<T: std::fmt::Debug>(answer: Answer<T>) -> String {
    match answer {
        Ok(Err(refused)) => refused,
        other => panic!("not a refusal: {other:?}"),
    }
}

fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    use chrono::TimeZone;
    chrono::Local.with_ymd_and_hms(y, mo, d, h, mi, s).earliest().unwrap().with_timezone(&Utc)
}

fn decide(routine: &str, attempt: &Value, step: &Value, decision: Decision) -> RecoveryRequest {
    RecoveryRequest { routine_id: routine.into(), attempt_id: text(attempt).into(), step: step.as_u64().unwrap(), decision }
}

mod routine_runner_ts {
    //! routine-runner.test.ts: running a routine, running on its own, a step that ends by asking.
    use super::*;

    pub fn routine(over: Value) -> Value {
        with(
            json!({"routineId": "rt_1", "name": "Nightly tidy", "teammateId": "tm_wren",
                "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"},
                "steps": ["Read status.ts.", "List every file.", "Say done."],
                "learnedFrom": ["mission_a", "mission_b", "mission_c"], "createdAt": "2026-09-05T00:00:00.000Z", "runs": 0}),
            over,
        )
    }

    fn harness(routines: Vec<Value>) -> (Arc<Mem>, Arc<Fake>) {
        (Mem::with(routines), Arc::new(Fake::default()))
    }

    #[test]
    fn starts_step_1_on_the_routine_route_recorded_as_the_routine_but_counts_only_completion() {
        let (mem, host) = harness(vec![routine(json!({}))]);
        let r = runner(&mem, &host);
        assert_eq!(ok(r.run("rt_1", None)), Begun { mission_id: "mission_1".into(), run_id: "run_1".into() });
        let starts = host.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].prompt, "Read status.ts.");
        assert_eq!((starts[0].runtime.as_str(), starts[0].model.as_deref(), starts[0].mode.as_str()), ("cursor", Some("composer-2.5"), "ask"));
        assert_eq!(starts[0].follow_up_of, None);
        assert_eq!(starts[0].started_by, json!({"kind": "routine", "routineId": "rt_1", "step": 1}));
        // Contract changed deliberately: starting step 1 is not a completed routine.
        assert!(mem.runs().is_empty());
        let started = host.updates.lock().unwrap().iter().find(|u| matches!(u, Update::MissionStarted { .. })).cloned();
        assert!(matches!(started, Some(Update::MissionStarted { ref teammate_id, ref routine_id, step: 1, .. }) if teammate_id == "tm_wren" && routine_id == "rt_1"));
        let running = r.running();
        assert_eq!((running.len(), running[0].routine_id.as_str(), running[0].step, running[0].of, running[0].mission_id.as_str()), (1, "rt_1", 1, 3, "mission_1"));
        assert!(host.notices()[0].contains("step 1 of 3"));
    }

    #[test]
    fn each_later_step_follows_up_the_one_before_it_and_only_after_it_completed() {
        let (mem, host) = harness(vec![routine(json!({}))]);
        let r = runner(&mem, &host);
        ok(r.run("rt_1", None));
        // Some other mission ending moves nothing.
        r.on_run_ended("mission_stranger").unwrap();
        assert_eq!(host.starts().len(), 1);
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        let s = host.starts();
        assert_eq!((s.len(), s[1].prompt.as_str(), s[1].follow_up_of.as_deref()), (2, "List every file.", Some("mission_1")));
        assert_eq!(s[1].started_by["step"], 2);
        assert_eq!((r.running()[0].step, r.running()[0].mission_id.as_str()), (2, "mission_2"));
        host.phase("mission_2", Phase::Completed);
        r.on_run_ended("mission_2").unwrap();
        let s = host.starts();
        assert_eq!((s[2].prompt.as_str(), s[2].follow_up_of.as_deref(), &s[2].started_by["step"]), ("Say done.", Some("mission_2"), &json!(3)));
        host.phase("mission_3", Phase::Completed);
        r.on_run_ended("mission_3").unwrap();
        assert_eq!(host.starts().len(), 3);
        assert!(r.running().is_empty());
        assert_eq!(host.last_notice(), "Routine \"Nightly tidy\" finished: 3 steps completed.");
        // One run counted, however many steps.
        assert_eq!(mem.runs(), vec!["rt_1"]);
    }

    #[test]
    fn a_step_that_failed_was_stopped_or_was_interrupted_ends_the_routine_there_and_says_which() {
        for (phase, words) in [(Phase::Failed, "that run failed"), (Phase::Cancelled, "that run was stopped"), (Phase::Interrupted, "that run was interrupted")] {
            let (mem, host) = harness(vec![routine(json!({}))]);
            let r = runner(&mem, &host);
            ok(r.run("rt_1", None));
            host.phase("mission_1", phase);
            r.on_run_ended("mission_1").unwrap();
            assert_eq!(host.starts().len(), 1);
            assert!(r.running().is_empty());
            assert_eq!(host.last_notice(), format!("Routine \"Nightly tidy\" stopped at step 1 of 3: {words}."));
        }
    }

    #[test]
    fn a_record_it_cannot_read_is_not_a_completion() {
        let (mem, host) = harness(vec![routine(json!({}))]);
        let r = runner(&mem, &host);
        ok(r.run("rt_1", None));
        r.on_run_ended("mission_1").unwrap();
        assert_eq!(host.starts().len(), 1);
        assert!(host.last_notice().contains("the record of that run could not be read"));
    }

    #[test]
    fn a_run_keeps_the_steps_it_started_with_and_an_edit_made_while_it_runs_applies_from_the_next_run() {
        let (mem, host) = harness(vec![routine(json!({}))]);
        let r = runner(&mem, &host);
        ok(r.run("rt_1", None));
        let planned = mem.get_one("rt_1")["steps"][1].clone();
        // The store now holds the edited routine.
        mem.edit("rt_1", |r| r["steps"] = json!(["Read status.ts.", "Now list only .ts files."]));
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        assert_eq!(host.starts()[1].prompt, text(&planned));
        assert_ne!(host.starts()[1].prompt, "Now list only .ts files.");
        // The edit cut the routine to two steps; this run still has its three.
        host.phase("mission_2", Phase::Completed);
        r.on_run_ended("mission_2").unwrap();
        assert_eq!(host.starts().len(), 3);
        assert_eq!(host.last_notice(), "Routine \"Nightly tidy\" · step 3 of 3.");
    }

    #[test]
    fn runs_on_the_teammates_model_now_in_the_mode_it_was_saved_with() {
        let saved = routine(json!({"route": {"runtime": "cursor", "model": "cursor-grok-4.6-high", "mode": "ask"}}));
        let mem = Mem::with(vec![saved.clone()]);
        let host = Arc::new(Fake { routes: Some(Box::new(|_| Some(json!({"runtime": "cursor", "model": "grok-4.7-high", "mode": "accept-edits"})))), ..Fake::default() });
        ok(runner(&mem, &host).run("rt_1", None));
        let s = &host.starts()[0];
        assert_eq!((s.runtime.as_str(), s.model.as_deref(), s.mode.as_str()), ("cursor", Some("grok-4.7-high"), "ask"));
        // A teammate with no route read falls back to the one it was saved with.
        let (mem, host) = harness(vec![saved]);
        ok(runner(&mem, &host).run("rt_1", None));
        assert_eq!((host.starts()[0].model.as_deref(), host.starts()[0].mode.as_str()), (Some("cursor-grok-4.6-high"), "ask"));
    }

    #[test]
    fn refuses_what_it_cannot_honestly_run_with_the_reason() {
        let (mem, host) = harness(vec![routine(json!({}))]);
        assert_eq!(refused(runner(&mem, &host).run("rt_nope", None)), "That routine no longer exists.");
        let host = Arc::new(Fake { peers: Box::new(|_| None), ..Fake::default() });
        assert!(refused(runner(&mem, &host).run("rt_1", None)).contains("no longer exists"));
        let (mem, host) = harness(vec![routine(json!({"route": {"runtime": "gemini", "model": "x", "mode": "accept-edits"}}))]);
        assert!(refused(runner(&mem, &host).run("rt_1", None)).contains("gemini"));
        let (mem, host) = harness(vec![routine(json!({}))]);
        *host.refuse.lock().unwrap() = Some(StartError::Refused { code: "CODEX_UNAVAILABLE".into(), message: "no runtime".into() });
        let r = runner(&mem, &host);
        assert_eq!(refused(r.run("rt_1", None)), "no runtime");
        assert!(r.running().is_empty());
        assert!(mem.runs().is_empty());
        // A preflight refusal proves nothing started: no receipt is left behind.
        assert!(mem.execution("rt_1").is_null());
    }

    #[test]
    fn one_routine_at_a_time_per_teammate() {
        let (mem, host) = harness(vec![routine(json!({})), routine(json!({"routineId": "rt_2", "name": "Other"}))]);
        let r = runner(&mem, &host);
        ok(r.run("rt_1", None));
        assert!(refused(r.run("rt_2", None)).contains("already running"));
        assert_eq!(host.starts().len(), 1);
    }

    #[test]
    fn a_routine_that_asks_for_inputs_is_refused_before_anything_starts() {
        // W7's input cases need the input form (its own PARITY row); until then nothing with inputs runs.
        let (mem, host) = harness(vec![routine(json!({"steps": ["Read {{topic}}"], "inputs": [{"key": "topic", "label": "Topic", "kind": "text", "required": true}]}))]);
        assert!(refused(runner(&mem, &host).run("rt_1", None)).contains("Nothing was started"));
        assert!(host.starts().is_empty());
        assert!(mem.execution("rt_1").is_null());
    }

    // --- a routine that runs on its own ---

    const NOON: &str = "2026-09-05T12:00:00.000Z";

    fn scheduled(routines: Vec<Value>, busy: &[&str]) -> (Arc<Mem>, Arc<Fake>) {
        let mem = Mem::with(routines);
        let host = Arc::new(Fake {
            peers: Box::new(|id| Some(peer(id, "Wren"))),
            busy: Mutex::new(Some(busy.iter().map(|b| b.to_string()).collect())),
            ..Fake::default()
        });
        (mem, host)
    }

    fn tick(mem: &Mem, r: &RoutineRunner, now: DateTime<Utc>) -> Vec<String> {
        *mem.clock.lock().unwrap() = Some(iso(now));
        r.tick(now).unwrap()
    }

    #[test]
    fn a_due_routine_starts_on_the_tick_as_if_run_were_pressed_and_an_undue_one_and_one_with_no_schedule_do_not() {
        let (mem, host) = scheduled(
            vec![
                routine(json!({"routineId": "rt_due", "teammateId": "tm_a", "schedule": {"kind": "every", "hours": 2}, "createdAt": "2026-09-05T09:00:00.000Z"})),
                routine(json!({"routineId": "rt_soon", "teammateId": "tm_b", "schedule": {"kind": "every", "hours": 4}, "createdAt": "2026-09-05T09:00:00.000Z"})),
                routine(json!({"routineId": "rt_manual", "teammateId": "tm_c"})),
            ],
            &[],
        );
        let r = runner(&mem, &host);
        assert_eq!(tick(&mem, &r, at(NOON)), vec!["rt_due"]);
        assert_eq!(host.starts().iter().map(|s| s.started_by.clone()).collect::<Vec<_>>(), vec![json!({"kind": "routine", "routineId": "rt_due", "step": 1})]);
        // Only final completion counts.
        assert!(mem.runs().is_empty());
        // The next minute: progress blocks a duplicate.
        assert!(tick(&mem, &r, at("2026-09-05T12:01:00.000Z")).is_empty());
        // Two hours on, rt_soon is due too, and rt_due is still replaying step 1 so it waits.
        assert_eq!(tick(&mem, &r, at("2026-09-05T14:00:00.000Z")), vec!["rt_soon"]);
        assert_eq!(host.starts().len(), 2);
    }

    #[test]
    fn runs_on_its_own_only_in_the_folder_it_was_made_in() {
        let made = "2026-09-05T09:00:00.000Z";
        let (mem, _) = scheduled(
            vec![
                routine(json!({"routineId": "rt_here", "teammateId": "tm_a", "schedule": {"kind": "every", "hours": 1}, "createdAt": made, "workspaceId": "ws_test"})),
                routine(json!({"routineId": "rt_there", "teammateId": "tm_b", "schedule": {"kind": "every", "hours": 1}, "createdAt": made, "workspaceId": "ws_other"})),
                routine(json!({"routineId": "rt_old_there", "teammateId": "tm_c", "schedule": {"kind": "every", "hours": 1}, "createdAt": made, "learnedFrom": ["mission_in_other"]})),
                routine(json!({"routineId": "rt_old_unknown", "teammateId": "tm_d", "schedule": {"kind": "every", "hours": 1}, "createdAt": made, "learnedFrom": ["mission_gone"]})),
            ],
            &[],
        );
        let host = Arc::new(Fake {
            peers: Box::new(|id| Some(peer(id, "Wren"))),
            home: Some(Box::new(|r| r["workspaceId"].as_str().map(str::to_owned).or_else(|| (r["learnedFrom"][0] == "mission_in_other").then(|| "ws_other".into())))),
            ..Fake::default()
        });
        assert_eq!(tick(&mem, &runner(&mem, &host), at(NOON)), vec!["rt_here", "rt_old_unknown"]);
    }

    #[test]
    fn a_teammate_busy_with_a_persons_run_is_skipped_and_tried_next_tick_not_queued_and_not_held_off() {
        let (mem, host) = scheduled(vec![routine(json!({"schedule": {"kind": "every", "hours": 1}, "createdAt": "2026-09-05T09:00:00.000Z"}))], &["tm_wren"]);
        let r = runner(&mem, &host);
        assert!(tick(&mem, &r, at(NOON)).is_empty());
        assert!(host.starts().is_empty());
        *host.busy.lock().unwrap() = Some(HashSet::new());
        assert_eq!(tick(&mem, &r, at("2026-09-05T12:01:00.000Z")), vec!["rt_1"]);
    }

    fn failing(routines: Vec<Value>) -> (Arc<Mem>, Arc<Fake>) {
        let (mem, host) = scheduled(routines, &[]);
        *host.refuse.lock().unwrap() = Some(StartError::Refused { code: "CODEX_UNAVAILABLE".into(), message: "no runtime".into() });
        (mem, host)
    }

    #[test]
    fn a_scheduled_start_that_fails_is_held_off_for_an_hour_rather_than_retried_every_minute() {
        let (mem, host) = failing(vec![routine(json!({"schedule": {"kind": "every", "hours": 1}, "createdAt": "2026-09-05T09:00:00.000Z"}))]);
        let r = runner(&mem, &host);
        let noon = at(NOON);
        assert!(tick(&mem, &r, noon).is_empty());
        assert_eq!(host.starts().len(), 1);
        tick(&mem, &r, noon + chrono::Duration::milliseconds(60_000));
        tick(&mem, &r, noon + chrono::Duration::milliseconds(SCHEDULE_HOLD_OFF_MS - 1));
        assert_eq!(host.starts().len(), 1);
        tick(&mem, &r, noon + chrono::Duration::milliseconds(SCHEDULE_HOLD_OFF_MS));
        assert_eq!(host.starts().len(), 2);
        // Nothing was recorded as a run.
        assert!(mem.runs().is_empty());
    }

    #[test]
    fn daily_at_a_time_runs_once_that_day_whichever_minute_the_tick_lands_on() {
        let (mem, host) = scheduled(vec![routine(json!({"schedule": {"kind": "daily", "at": "09:00"}, "createdAt": "2026-09-01T00:00:00.000Z"}))], &[]);
        let r = runner(&mem, &host);
        assert!(tick(&mem, &r, local(2026, 9, 5, 8, 59, 0)).is_empty());
        assert_eq!(tick(&mem, &r, local(2026, 9, 5, 9, 3, 0)), vec!["rt_1"]);
        for (h, m) in [(9, 4), (13, 0), (23, 59)] {
            assert!(tick(&mem, &r, local(2026, 9, 5, h, m, 0)).is_empty());
        }
    }

    #[test]
    fn a_refused_scheduled_start_tells_the_window_names_the_routine_and_says_when_it_will_try_again() {
        let (mem, host) = failing(vec![routine(json!({"routineId": "rt_due", "teammateId": "tm_a", "schedule": {"kind": "every", "hours": 2}, "createdAt": "2026-09-05T09:00:00.000Z"}))]);
        tick(&mem, &runner(&mem, &host), at(NOON));
        let blocked = host.blocked();
        assert_eq!(blocked.len(), 1);
        let Update::Blocked { name, message, retry_at, .. } = &blocked[0] else { unreachable!() };
        assert_eq!(name, "Nightly tidy");
        assert!(message.contains("no runtime"));
        assert!(at(retry_at) > at(NOON));
    }

    #[test]
    fn a_refused_scheduled_start_is_said_once_per_attempt_not_once_per_tick() {
        let (mem, host) = failing(vec![routine(json!({"routineId": "rt_due", "teammateId": "tm_a", "schedule": {"kind": "every", "hours": 2}, "createdAt": "2026-09-05T09:00:00.000Z"}))]);
        let r = runner(&mem, &host);
        tick(&mem, &r, at(NOON));
        tick(&mem, &r, at(NOON) + chrono::Duration::milliseconds(60_000));
        assert_eq!(host.blocked().len(), 1);
    }

    // --- a routine step that ends by asking something ---

    #[test]
    fn a_step_that_asked_something_stops_there_instead_of_starting_the_next_step() {
        let (mem, host) = harness(vec![routine(json!({}))]);
        let r = runner(&mem, &host);
        ok(r.run("rt_1", None));
        host.phase("mission_1", Phase::Completed);
        host.asked.lock().unwrap().as_mut().unwrap().insert("mission_1".into());
        r.on_run_ended("mission_1").unwrap();
        assert_eq!(host.starts().len(), 1);
        assert!(r.running().is_empty());
        assert!(host.notices().join(" ").to_lowercase().contains("asked you something"));
    }

    #[test]
    fn a_step_that_asked_nothing_still_advances() {
        let (mem, host) = harness(vec![routine(json!({}))]);
        let r = runner(&mem, &host);
        ok(r.run("rt_1", None));
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        assert_eq!(host.starts().len(), 2);
    }
}

mod hands_off {
    //! a-routine-hands-off-to-the-next-teammate.test.ts.
    use super::*;

    fn people(id: &str) -> Option<Peer> {
        match id {
            "tm_wren" => Some(peer("tm_wren", "Wren")),
            "tm_atlas" => Some(peer("tm_atlas", "Atlas")),
            "tm_sable" => Some(peer("tm_sable", "Sable")),
            _ => None,
        }
    }
    fn routes(id: &str) -> Option<Value> {
        match id {
            "tm_wren" => Some(json!({"runtime": "cursor", "model": "composer-2.5", "mode": "ask"})),
            "tm_atlas" => Some(json!({"runtime": "opencode", "model": "opencode/nemotron-3-ultra-free", "mode": "ask"})),
            "tm_sable" => Some(json!({"runtime": "claude", "model": "haiku", "mode": "ask", "effort": "low"})),
            _ => None,
        }
    }
    pub fn chain(over: Value) -> Value {
        with(
            json!({"routineId": "rt_chain", "name": "Intake to review", "teammateId": "tm_wren", "route": routes("tm_wren"),
                "steps": ["Read the bug report in issue.md and say what is wrong.", "Write a plan to fix it.", "Check the plan."],
                "handOffs": [{}, {"teammateId": "tm_atlas"}, {"teammateId": "tm_sable", "check": true}],
                "learnedFrom": [], "createdAt": "2026-09-28T00:00:00.000Z", "runs": 0}),
            over,
        )
    }
    fn harness(routine: Value, busy: &[&str]) -> (Arc<Mem>, Arc<Fake>) {
        let host = Fake {
            peers: Box::new(people),
            routes: Some(Box::new(routes)),
            busy: Mutex::new(Some(busy.iter().map(|b| b.to_string()).collect())),
            ..Fake::default()
        };
        (Mem::with(vec![routine]), Arc::new(host))
    }
    fn finish(host: &Fake, r: &RoutineRunner, mission: &str, reply: &str) {
        host.phase(mission, Phase::Completed);
        host.reply(mission, reply);
        r.on_run_ended(mission).unwrap();
    }

    #[test]
    fn refuses_before_any_step_runs_when_a_later_steps_teammate_was_removed_naming_the_step() {
        let (mem, host) = harness(chain(json!({"handOffs": [{}, {"teammateId": "tm_atlas"}, {"teammateId": "tm_removed", "check": true}]})), &[]);
        assert_eq!(
            refused(runner(&mem, &host).run("rt_chain", None)),
            "Step 3 of 3, the step that checks the work, was handed to a teammate who has since been removed, so nothing was started. Edit the routine to choose who takes it."
        );
        assert!(host.starts().is_empty());
    }

    #[test]
    fn gives_each_step_to_its_teammate_on_that_teammates_route_with_the_answer_of_the_step_before() {
        let (mem, host) = harness(chain(json!({})), &[]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        let s = host.starts();
        assert_eq!((s[0].prompt.as_str(), s[0].runtime.as_str(), &s[0].peer), ("Read the bug report in issue.md and say what is wrong.", "cursor", &people("tm_wren").unwrap()));
        finish(&host, &r, "mission_1", "The total is summed twice in cart.ts line 12.");
        let s = &host.starts()[1];
        assert_eq!((s.runtime.as_str(), s.model.as_deref(), s.mode.as_str(), &s.follow_up_of, &s.peer), ("opencode", Some("opencode/nemotron-3-ultra-free"), "ask", &None, &people("tm_atlas").unwrap()));
        assert!(s.prompt.contains("Wren did the step before this one and answered:"));
        assert!(s.prompt.contains("The total is summed twice in cart.ts line 12."));
        assert!(s.prompt.ends_with(&format!("{}Write a plan to fix it.", hand_off::STEP_MARK)));
        assert!(!s.prompt.contains("VERDICT"));
        assert_eq!(hand_off::step_words_of(&s.prompt), "Write a plan to fix it.");
        assert_eq!(host.last_notice(), "Routine \"Intake to review\" · step 2 of 3, handed to Atlas with Wren's answer.");
        finish(&host, &r, "mission_2", "Plan: remove the second sum.");
        let s = &host.starts()[2];
        assert_eq!((s.runtime.as_str(), s.model.as_deref(), s.effort.as_deref(), &s.peer), ("claude", Some("haiku"), Some("low"), &people("tm_sable").unwrap()));
        assert!(s.prompt.contains("Atlas did the step before this one and answered:"));
        assert!(s.prompt.contains(hand_off::CHECK_RULE));
        assert!(s.prompt.contains("The routine's task, as its first step asked: Read the bug report in issue.md and say what is wrong."));
        assert!(!host.starts()[1].prompt.contains("The routine's task"));
        assert!(s.prompt.ends_with(&format!("{}Check the plan.", hand_off::STEP_MARK)));
        assert_eq!(host.last_notice(), "Routine \"Intake to review\" · step 3 of 3, handed to Sable with Atlas's answer to check.");
        finish(&host, &r, "mission_3", "The plan is right.\nVERDICT: APPROVED");
        assert_eq!(mem.runs(), vec!["rt_chain"]);
        assert_eq!(host.last_notice(), "Routine \"Intake to review\" finished: 3 steps completed, approved by Sable.");
    }

    #[test]
    fn does_not_count_a_run_the_checker_did_not_approve_and_a_restart_does_not_count_it_either() {
        let (mem, host) = harness(chain(json!({})), &[]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        finish(&host, &r, "mission_1", "Found it.");
        finish(&host, &r, "mission_2", "A plan.");
        finish(&host, &r, "mission_3", "The plan misses the tests.\nVERDICT: CHANGES NEEDED -- add a test for the double sum");
        assert!(mem.runs().is_empty());
        let e = mem.execution("rt_chain");
        assert_eq!((&e["status"], &e["canContinue"], &e["settledAtDispatch"]), (&json!("held"), &json!(false), &json!(true)));
        assert_eq!(e["reason"], "Sable, the checker, asked for changes: add a test for the double sum. The run does not count as done. Review the work, then run the routine again.");
        assert!(host.last_notice().contains("stopped at step 3 of 3: Sable, the checker, asked for changes"));
        // A new runner, as after a restart: the hold stands.
        runner(&mem, &host).reconcile().unwrap();
        assert!(mem.runs().is_empty());
        assert_eq!(mem.execution("rt_chain")["status"], "held");
    }

    #[test]
    fn says_what_the_checker_asked_for_with_one_full_stop_however_the_checker_ended_it() {
        let (mem, host) = harness(chain(json!({})), &[]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        finish(&host, &r, "mission_1", "Found it.");
        finish(&host, &r, "mission_2", "A plan.");
        finish(&host, &r, "mission_3", "VERDICT: CHANGES NEEDED -- read the code first.");
        assert_eq!(host.last_notice(), "Routine \"Intake to review\" stopped at step 3 of 3: Sable, the checker, asked for changes: read the code first.");
    }

    #[test]
    fn does_not_count_a_checker_that_gave_no_verdict() {
        let (mem, host) = harness(chain(json!({})), &[]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        finish(&host, &r, "mission_1", "Found it.");
        finish(&host, &r, "mission_2", "A plan.");
        finish(&host, &r, "mission_3", "Looks good to me.");
        assert!(mem.runs().is_empty());
        assert!(text(&mem.execution("rt_chain")["reason"]).starts_with("Sable, the checker, gave no verdict"));
    }

    #[test]
    fn reads_the_verdict_of_a_checker_that_finished_while_the_app_was_closed() {
        for (reply, counted) in [("VERDICT: APPROVED", true), ("VERDICT: CHANGES NEEDED -- redo it", false)] {
            let (mem, host) = harness(chain(json!({})), &[]);
            let r = runner(&mem, &host);
            ok(r.run("rt_chain", None));
            finish(&host, &r, "mission_1", "Found it.");
            finish(&host, &r, "mission_2", "A plan.");
            // The checker's run finishes, but this runner never hears of it.
            host.phase("mission_3", Phase::Completed);
            host.reply("mission_3", reply);
            runner(&mem, &host).reconcile().unwrap();
            assert_eq!(mem.runs().len() == 1, counted, "{reply}");
        }
    }

    #[test]
    fn continues_the_same_teammates_conversation_with_nothing_quoted_when_two_steps_in_a_row_are_theirs() {
        let (mem, host) = harness(chain(json!({"handOffs": [{}, {}, {"teammateId": "tm_atlas"}]})), &[]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        finish(&host, &r, "mission_1", "Found it.");
        let s = &host.starts()[1];
        assert_eq!((s.prompt.as_str(), s.follow_up_of.as_deref(), &s.peer), ("Write a plan to fix it.", Some("mission_1"), &people("tm_wren").unwrap()));
    }

    #[test]
    fn waits_held_when_the_next_teammate_is_busy_and_is_continued_on_them_from_the_card() {
        let (mem, host) = harness(chain(json!({})), &["tm_atlas"]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        finish(&host, &r, "mission_1", "Found it.");
        assert_eq!(host.starts().len(), 1);
        let held = mem.execution("rt_chain");
        assert_eq!((&held["status"], &held["canContinue"], &held["step"]), (&json!("held"), &json!(true), &json!(1)));
        assert_eq!(held["reason"], "Step 2 is Atlas's, and Atlas is busy. Continue it here when they are free.");
        let request = decide("rt_chain", &held["attemptId"], &json!(1), Decision::Continue);
        // Still busy: continuing says so.
        assert_eq!(r.recover(&request).unwrap(), Err("Atlas is busy. Wait for their current mission to finish.".into()));
        *host.busy.lock().unwrap() = Some(HashSet::new());
        ok(r.recover(&request));
        let s = &host.starts()[1];
        assert_eq!((s.runtime.as_str(), &s.peer, &s.follow_up_of), ("opencode", &people("tm_atlas").unwrap(), &None));
        assert!(s.prompt.contains("Wren did the step before this one and answered:\n\nFound it."));
    }

    #[test]
    fn starts_on_the_teammate_of_step_1_when_that_step_is_handed_to_someone_else() {
        let (mem, host) = harness(chain(json!({"handOffs": [{"teammateId": "tm_atlas"}, {}, {}]})), &[]);
        ok(runner(&mem, &host).run("rt_chain", None));
        let s = &host.starts()[0];
        assert_eq!((s.runtime.as_str(), &s.peer, s.prompt.as_str()), ("opencode", &people("tm_atlas").unwrap(), "Read the bug report in issue.md and say what is wrong."));
    }

    fn levels(host: &Fake) -> Vec<String> {
        host.updates
            .lock()
            .unwrap()
            .iter()
            .filter_map(|u| match u {
                Update::Notice { message, level, .. } => {
                    let said = message.strip_prefix("Routine \"Intake to review\" ").unwrap_or(message);
                    Some(format!("{}: {said}", if *level == Level::Info { "info" } else { "warning" }))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn notices_are_information_while_it_goes_well_and_a_warning_only_when_it_stops_or_needs_the_person() {
        let (mem, host) = harness(chain(json!({})), &[]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        finish(&host, &r, "mission_1", "Found it.");
        finish(&host, &r, "mission_2", "A plan.");
        finish(&host, &r, "mission_3", "VERDICT: CHANGES NEEDED -- add a test");
        assert_eq!(
            levels(&host),
            vec![
                "info: · step 1 of 3. Each next step starts when this one completes.",
                "info: · step 2 of 3, handed to Atlas with Wren's answer.",
                "info: · step 3 of 3, handed to Sable with Atlas's answer to check.",
                "warning: stopped at step 3 of 3: Sable, the checker, asked for changes: add a test.",
            ]
        );
    }

    #[test]
    fn notices_say_a_finished_run_as_information() {
        let (mem, host) = harness(chain(json!({})), &[]);
        let r = runner(&mem, &host);
        ok(r.run("rt_chain", None));
        finish(&host, &r, "mission_1", "Found it.");
        finish(&host, &r, "mission_2", "A plan.");
        finish(&host, &r, "mission_3", "VERDICT: APPROVED");
        assert_eq!(levels(&host).last().unwrap(), "info: finished: 3 steps completed, approved by Sable.");
    }
}

mod waits_for_ok {
    //! a-routine-step-waits-for-your-ok.test.ts.
    use super::*;

    fn routine(over: Value) -> Value {
        with(
            json!({"routineId": "rt_ship", "name": "Ship the fix", "teammateId": "tm_wren",
                "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"},
                "steps": ["Fix the failing test in cart.ts.", "Push the branch and open a pull request."],
                "handOffs": [{}, {"askFirst": true}], "learnedFrom": [], "createdAt": "2026-10-28T00:00:00.000Z", "runs": 0}),
            over,
        )
    }
    fn harness(routine: Value) -> (Arc<Mem>, Arc<Fake>) {
        let host = Fake {
            peers: Box::new(|id| (id == "tm_wren").then(|| peer("tm_wren", "Wren"))),
            routes: Some(Box::new(|_| Some(json!({"runtime": "cursor", "model": "composer-2.5", "mode": "ask"})))),
            busy: Mutex::new(Some(HashSet::new())),
            ..Fake::default()
        };
        (Mem::with(vec![routine]), Arc::new(host))
    }
    fn finish(host: &Fake, r: &RoutineRunner, mission: &str, reply: &str) {
        host.phase(mission, Phase::Completed);
        host.reply(mission, reply);
        r.on_run_ended(mission).unwrap();
    }

    #[test]
    fn holds_before_the_step_tells_the_person_and_go_ahead_starts_it_with_the_answer_of_the_step_before() {
        let (mem, host) = harness(routine(json!({})));
        let r = runner(&mem, &host);
        ok(r.run("rt_ship", None));
        finish(&host, &r, "mission_1", "Fixed: the total was summed twice.");
        assert_eq!(host.starts().len(), 1);
        let waiting = mem.execution("rt_ship");
        for (k, v) in [("status", json!("held")), ("step", json!(1)), ("of", json!(2)), ("canContinue", json!(true)), ("settledAtDispatch", json!(true)), ("waitsForOk", json!(true))] {
            assert_eq!(waiting[k], v, "{k}");
        }
        assert_eq!(waiting["reason"], "Step 2 waits for your OK. Read what step 1 did, then go ahead or stop here.");
        assert_eq!(*host.asks.lock().unwrap(), vec![(Some("Wren".to_owned()), "Ship the fix".to_owned(), 2)]);
        assert_eq!(host.last_notice(), "Routine \"Ship the fix\" is waiting for your OK after step 1 of 2. Go ahead with step 2 under Routines.");
        // A restart's reconcile leaves a wait the routine asked for alone.
        r.reconcile().unwrap();
        assert_eq!((&mem.execution("rt_ship")["status"], &mem.execution("rt_ship")["waitsForOk"]), (&json!("held"), &json!(true)));
        ok(r.recover(&decide("rt_ship", &waiting["attemptId"], &json!(1), Decision::Continue)));
        let s = &host.starts()[1];
        assert_eq!((s.prompt.as_str(), s.follow_up_of.as_deref(), &s.peer), ("Push the branch and open a pull request.", Some("mission_1"), &peer("tm_wren", "Wren")));
        // The step it started is running, not still waiting.
        let e = mem.execution("rt_ship");
        assert_eq!((&e["status"], &e["step"]), (&json!("running"), &json!(2)));
        assert!(e.get("waitsForOk").is_none());
        finish(&host, &r, "mission_2", "Opened the pull request.");
        assert_eq!(mem.runs(), vec!["rt_ship"]);
    }

    #[test]
    fn stop_here_ends_the_run_and_keeps_the_routine_as_it_was() {
        let (mem, host) = harness(routine(json!({})));
        let r = runner(&mem, &host);
        ok(r.run("rt_ship", None));
        finish(&host, &r, "mission_1", "Fixed.");
        let waiting = mem.execution("rt_ship");
        ok(r.recover(&decide("rt_ship", &waiting["attemptId"], &json!(1), Decision::Keep)));
        assert_eq!(*mem.kept.lock().unwrap(), vec!["rt_ship"]);
        assert_eq!(host.starts().len(), 1);
    }

    #[test]
    fn does_not_wait_when_no_step_asks_or_when_the_step_that_asks_is_not_the_next_one() {
        let (mem, host) = harness(routine(json!({"steps": ["One.", "Two.", "Three."], "handOffs": [{}, {}, {"askFirst": true}]})));
        let r = runner(&mem, &host);
        ok(r.run("rt_ship", None));
        finish(&host, &r, "mission_1", "Done one.");
        assert_eq!(host.starts().len(), 2);
        assert!(host.asks.lock().unwrap().is_empty());
        finish(&host, &r, "mission_2", "Done two.");
        assert_eq!(host.starts().len(), 2);
        assert_eq!(*host.asks.lock().unwrap(), vec![(Some("Wren".to_owned()), "Ship the fix".to_owned(), 3)]);
    }
}

mod until_check {
    //! a-routine-keeps-going-until-its-check-passes.test.ts.
    use super::*;
    use crate::routine_copy::tests::temp;

    fn failing() -> GoalCheck {
        GoalCheck { command: "npm test".into(), passed: false, tail: vec!["FAIL cart.test.js".into(), "expected 6, got 0".into()] }
    }
    fn passing() -> GoalCheck {
        GoalCheck { command: "npm test".into(), passed: true, tail: vec![] }
    }

    struct Set {
        mem: Arc<Mem>,
        host: Arc<Fake>,
        _dirs: (crate::routine_copy::tests::Temp, crate::routine_copy::tests::Temp),
    }
    fn set_up(checks: Vec<GoalCheck>, tries: u64, in_copy: bool) -> Set {
        let (folder, root) = (temp("locust-goal-folder-"), temp("locust-goal-root-"));
        let mut routine = json!({"routineId": "rt_tests", "name": "Green tests", "teammateId": "tm_wren",
            "route": {"runtime": "codex", "model": "gpt-6-luna", "mode": "accept-edits"},
            "steps": ["Fix the cart total."], "learnedFrom": [], "createdAt": "2026-10-01T00:00:00.000Z", "runs": 0,
            "untilCheck": {"tries": tries}});
        if in_copy {
            routine["inCopy"] = json!(true);
        }
        let host = Fake {
            folder: Some(folder.0.clone()),
            // A copy that changes nothing, for a routine that works in one.
            copies: Some(RoutineCopies::new(&root.0)),
            goal: Some(Mutex::new((checks, 0))),
            ..Fake::default()
        };
        Set { mem: Mem::with(vec![routine]), host: Arc::new(host), _dirs: (folder, root) }
    }
    fn fixes(host: &Fake) -> Vec<String> {
        host.notices().into_iter().filter(|n| n.contains("fix 1 of 3")).collect()
    }

    #[test]
    fn asks_the_same_teammate_to_fix_what_the_check_said_in_the_same_conversation_then_finishes_when_it_passes() {
        let Set { mem, host, _dirs, .. } = set_up(vec![failing(), passing()], 3, false);
        let r = runner(&mem, &host);
        ok(r.run("rt_tests", None));
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        let s = host.starts();
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].follow_up_of.as_deref(), Some("mission_1"));
        assert_eq!(s[1].prompt, goal_fix_prompt(&failing(), 1, 3));
        assert!(s[1].prompt.contains("expected 6, got 0"));
        assert_eq!(mem.execution("rt_tests")["goalTry"], 1);
        // The fix is said by its own line in the thread, never again as a notice (0.814).
        assert!(fixes(&host).is_empty());
        host.phase("mission_2", Phase::Completed);
        r.on_run_ended("mission_2").unwrap();
        assert_eq!(mem.runs(), vec!["rt_tests"]);
        assert_eq!(host.last_notice(), "Routine \"Green tests\" finished: the check `npm test` passed after 1 fix.");
    }

    #[test]
    fn stops_at_its_limit_and_says_what_still_fails_and_the_run_does_not_count_as_done() {
        let Set { mem, host, _dirs, .. } = set_up(vec![failing()], 1, false);
        let r = runner(&mem, &host);
        ok(r.run("rt_tests", None));
        for m in ["mission_1", "mission_2"] {
            host.phase(m, Phase::Completed);
            r.on_run_ended(m).unwrap();
        }
        assert_eq!(host.starts().len(), 2);
        assert!(mem.runs().is_empty());
        assert_eq!(mem.execution("rt_tests")["status"], "held");
        assert!(text(&mem.execution("rt_tests")["reason"]).starts_with(
            "The check `npm test` still fails after 1 fix: FAIL cart.test.js / expected 6, got 0. The run does not count as done."
        ));
        assert!(host.last_notice().contains("stopped: the check `npm test` still fails after 1 fix"));
    }

    #[test]
    fn in_a_copy_when_the_fixes_run_out_the_routine_itself_remembers_the_check_still_failed() {
        let Set { mem, host, _dirs, .. } = set_up(vec![failing()], 1, true);
        let r = runner(&mem, &host);
        ok(r.run("rt_tests", None));
        for m in ["mission_1", "mission_2"] {
            host.phase(m, Phase::Completed);
            r.on_run_ended(m).unwrap();
        }
        let failures = mem.failures.lock().unwrap().clone();
        assert_eq!(failures, vec![Some("The check `npm test` still fails after 1 fix: FAIL cart.test.js / expected 6, got 0.".to_owned())]);
        assert_eq!(mem.get_one("rt_tests")["lastFailed"], json!(failures[0]));
    }

    #[test]
    fn a_run_that_passes_leaves_no_failure_behind() {
        let Set { mem, host, _dirs, .. } = set_up(vec![passing()], 1, true);
        let r = runner(&mem, &host);
        ok(r.run("rt_tests", None));
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        assert_eq!(*mem.failures.lock().unwrap(), vec![None]);
    }

    #[test]
    fn finishes_at_once_when_the_check_passes_the_first_time_and_says_so() {
        let Set { mem, host, _dirs, .. } = set_up(vec![passing()], 3, false);
        let r = runner(&mem, &host);
        ok(r.run("rt_tests", None));
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        assert_eq!(host.starts().len(), 1);
        assert_eq!(mem.runs(), vec!["rt_tests"]);
        assert_eq!(host.last_notice(), "Routine \"Green tests\" finished: the check `npm test` passed.");
    }
}

mod works_in_a_copy {
    //! a-routine-works-in-a-copy.test.ts, "a routine that works in a copy".
    use super::*;
    use crate::routine_copy::tests::{Temp, temp};

    struct Set {
        mem: Arc<Mem>,
        host: Arc<Fake>,
        folder: PathBuf,
        _dirs: (Temp, Temp),
    }
    fn set_up(writes: bool) -> Set {
        let (folder, root) = (temp("locust-rtcopy-folder-"), temp("locust-rtcopy-root-"));
        std::fs::write(folder.0.join("notes.md"), "Old notes.\n").unwrap();
        let routine = json!({"routineId": "rt_inbox", "name": "Inbox receipt", "teammateId": "tm_cedar",
            "route": {"runtime": "codex", "model": "account-default", "mode": "accept-edits"},
            "steps": ["Write a receipt."], "learnedFrom": [], "createdAt": "2026-10-01T00:00:00.000Z", "runs": 0, "inCopy": true});
        let there = folder.0.clone();
        let host = Fake {
            peers: Box::new(|_| Some(peer("tm_cedar", "Cedar"))),
            copies: Some(RoutineCopies::new(&root.0)),
            folder: Some(folder.0.clone()),
            // The teammate: writes a receipt wherever it is told to work.
            on_start: writes.then(|| -> Box<dyn Fn(&StartRequest) + Send + Sync> {
                Box::new(move |r: &StartRequest| std::fs::write(r.cwd.clone().unwrap_or(there.clone()).join("receipt.md"), "Received.\n").unwrap())
            }),
            ..Fake::default()
        };
        let f = folder.0.clone();
        Set { mem: Mem::with(vec![routine]), host: Arc::new(host), folder: f, _dirs: (folder, root) }
    }

    #[test]
    fn runs_its_step_in_the_copy_the_folder_is_untouched_and_the_change_waits_named() {
        let Set { mem, host, folder, _dirs } = set_up(true);
        let r = runner(&mem, &host);
        ok(r.run("rt_inbox", None));
        let copy = host.copies.as_ref().unwrap().path("rt_inbox").unwrap();
        assert_eq!(host.starts().iter().map(|s| s.cwd.clone()).collect::<Vec<_>>(), vec![Some(copy)]);
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        assert!(!folder.join("receipt.md").exists());
        let staged = mem.staged.lock().unwrap()[0].clone().unwrap();
        assert_eq!((&staged["folder"], &staged["changed"], &staged["deleted"]), (&json!(folder.to_string_lossy()), &json!(["receipt.md"]), &json!([])));
        assert_eq!(mem.get_one("rt_inbox")["staged"]["changed"], json!(["receipt.md"]));
        assert!(host.last_notice().ends_with(
            "1 file changed in its copy, waiting for you under Routines: Keep writes it into the folder, Discard throws it away."
        ));
    }

    #[test]
    fn will_not_run_again_while_those_changes_wait_on_run_or_on_its_own() {
        let Set { mem, host, _dirs, .. } = set_up(true);
        let r = runner(&mem, &host);
        ok(r.run("rt_inbox", None));
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        assert!(refused(r.run("rt_inbox", None)).contains("waiting under Routines. Keep or Discard them"));
        mem.edit("rt_inbox", |r| {
            r["schedule"] = json!({"kind": "every", "hours": 1});
            r["lastRunAt"] = json!("2026-09-01T00:00:00.000Z");
        });
        assert!(r.tick(at("2026-10-02T00:00:00Z")).unwrap().is_empty());
    }

    #[test]
    fn a_run_that_changed_nothing_leaves_nothing_waiting_and_no_copy() {
        let Set { mem, host, _dirs, .. } = set_up(false);
        mem.edit("rt_inbox", |r| r["steps"] = json!(["Just read."]));
        let r = runner(&mem, &host);
        ok(r.run("rt_inbox", None));
        host.phase("mission_1", Phase::Completed);
        r.on_run_ended("mission_1").unwrap();
        assert_eq!(*mem.staged.lock().unwrap(), vec![None]);
        assert!(!host.copies.as_ref().unwrap().path("rt_inbox").unwrap().exists());
        assert_eq!(host.last_notice(), "Routine \"Inbox receipt\" finished: 1 step completed. Nothing changed in its copy, so there is nothing to keep.");
    }

    #[test]
    fn abandoning_a_held_copy_run_takes_its_copy_with_it() {
        let Set { mem, host, _dirs, .. } = set_up(true);
        let r = runner(&mem, &host);
        ok(r.run("rt_inbox", None));
        host.phase("mission_1", Phase::Failed);
        r.on_run_ended("mission_1").unwrap();
        let e = mem.execution("rt_inbox");
        assert!(host.copies.as_ref().unwrap().path("rt_inbox").unwrap().exists());
        ok(r.recover(&decide("rt_inbox", &e["attemptId"], &e["step"], Decision::Abandon)));
        assert!(!host.copies.as_ref().unwrap().path("rt_inbox").unwrap().exists());
    }
}

mod file_arriving {
    //! a-file-arriving-starts-its-routine.test.ts, "a routine on a new file".
    use super::*;

    #[test]
    fn starts_on_the_tick_when_files_are_ready_tells_step_1_their_names_and_only_then() {
        let routine = json!({"routineId": "rt_1", "name": "Read new invoices", "teammateId": "tm_wren",
            "route": {"runtime": "codex", "model": "account-default", "mode": "ask"},
            "steps": ["Summarize it in one line."], "learnedFrom": ["mission_a"], "createdAt": "2026-10-01T00:00:00.000Z", "runs": 0,
            "schedule": {"kind": "files", "folder": "inbox"}});
        let mem = Mem::with(vec![routine]);
        let host = Arc::new(Fake { asked: Mutex::new(None), ..Fake::default() });
        let r = runner(&mem, &host);
        assert!(r.tick(at("2026-10-01T09:00:00Z")).unwrap().is_empty());
        let ready = vec!["invoice-1043.pdf".to_owned(), "invoice-1051.pdf".to_owned()];
        *host.ready.lock().unwrap() = ready.clone();
        assert_eq!(r.tick(at("2026-10-01T09:01:00Z")).unwrap(), vec!["rt_1"]);
        let first = &host.starts()[0].prompt;
        assert_eq!(*first, format!("{}\n\nSummarize it in one line.", routine_files::arrival_note("inbox", &ready)));
        assert!(first.contains("2 new files just arrived: inbox/invoice-1043.pdf, inbox/invoice-1051.pdf."));
        assert_eq!(*host.fired.lock().unwrap(), vec![ready]);
        // The thread is told what started it; the step itself is shown as saved.
        assert!(host.notices().contains(&"Started because 2 new files arrived: inbox/invoice-1043.pdf, inbox/invoice-1051.pdf.".to_owned()));
    }
}

mod paused {
    //! a-paused-routine-waits-to-be-resumed.test.ts.
    use super::*;

    fn saved(over: Value) -> Value {
        let eight = local(2026, 10, 2, 8, 0, 0);
        with(
            json!({"routineId": "rt_1", "name": "Morning digest", "teammateId": "tm_wren",
                "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"}, "steps": ["Say good morning."], "learnedFrom": [],
                "createdAt": iso(eight), "lastRunAt": iso(eight), "runs": 0, "schedule": {"kind": "every", "hours": 1}}),
            over,
        )
    }
    fn harness(routine: Value, opened: DateTime<Utc>) -> (Arc<Mem>, Arc<Fake>) {
        (Mem::with(vec![routine]), Arc::new(Fake { opened_at: Some(opened), asked: Mutex::new(None), ..Fake::default() }))
    }

    #[test]
    fn is_not_started_by_its_schedule_and_records_no_miss_however_overdue() {
        let (mem, host) = harness(saved(json!({"paused": true})), local(2026, 10, 2, 7, 0, 0));
        assert!(runner(&mem, &host).tick(local(2026, 10, 3, 9, 0, 0)).unwrap().is_empty());
        assert!(host.starts().is_empty());
        assert!(mem.misses.lock().unwrap().is_empty());
    }

    #[test]
    fn still_starts_when_a_person_presses_run() {
        let (mem, host) = harness(saved(json!({"paused": true})), local(2026, 10, 2, 7, 0, 0));
        ok(runner(&mem, &host).run("rt_1", None));
        assert_eq!(host.starts().len(), 1);
    }

    #[test]
    fn resumed_counts_from_the_resume_so_a_long_overdue_hourly_routine_does_not_fire_on_the_spot_and_does_an_hour_on() {
        let resumed = local(2026, 10, 9, 10, 0, 0);
        let (mem, host) = harness(saved(json!({"resumedAt": iso(resumed)})), local(2026, 10, 9, 9, 0, 0));
        let r = runner(&mem, &host);
        assert!(r.tick(local(2026, 10, 9, 10, 0, 30)).unwrap().is_empty());
        assert!(mem.misses.lock().unwrap().is_empty());
        assert_eq!(r.tick(local(2026, 10, 9, 11, 0, 30)).unwrap(), vec!["rt_1"]);
    }

    #[test]
    fn counts_from_its_last_run_when_that_is_later_than_an_old_resume() {
        let base = |r: Value| iso(crate::routines::schedule::base(&r).unwrap().with_timezone(&Utc));
        assert_eq!(base(json!({"createdAt": "2026-10-01T00:00:00.000Z", "lastRunAt": "2026-10-05T00:00:00.000Z", "resumedAt": "2026-10-03T00:00:00.000Z"})), "2026-10-05T00:00:00.000Z");
        assert_eq!(base(json!({"createdAt": "2026-10-01T00:00:00.000Z", "resumedAt": "2026-10-03T00:00:00.000Z"})), "2026-10-03T00:00:00.000Z");
        assert_eq!(base(json!({"createdAt": "2026-10-01T00:00:00.000Z"})), "2026-10-01T00:00:00.000Z");
    }

    #[test]
    fn a_missed_slot_while_closed_is_recorded_not_started() {
        // The other half of the clock: open after the slot, and it is missed.
        let (mem, host) = harness(saved(json!({})), local(2026, 10, 2, 12, 0, 0));
        assert!(runner(&mem, &host).tick(local(2026, 10, 2, 12, 0, 30)).unwrap().is_empty());
        assert!(host.starts().is_empty());
        assert_eq!(mem.misses.lock().unwrap().len(), 1);
    }
}

#[path = "routine_runner_recovery_tests.rs"]
mod recovery;
