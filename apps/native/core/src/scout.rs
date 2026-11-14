//! Scout: a teammate that keeps watch, brings morning briefs, weekly tune-ups, and keeps notes.
//!
//! Port of Electron's `main/scout.ts`, `main/scout-folder.ts`, `main/scout-github.ts`,
//! `shared/scout.ts`, and `shared/scout-log.ts`.

pub mod folder;
pub mod github;
pub mod host;
pub mod log;
pub mod scheduler;
pub mod store;
pub mod types;

#[cfg(test)]
mod unit_tests;
#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod brief_tests;
#[cfg(test)]
mod host_tests;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Timelike};
use serde_json::Value;

use crate::pull_requests::{FolderPullRequest, PullRequestChecks, PullRequestReview, PullRequestState};

use self::github::{ScoutGithub, ScoutGithubUnread};
use self::log::{
    LogGithubReason, LookEntryInput, SCOUT_LOG_DAYS, ScoutAsideWhy, ScoutHeldAlert, ScoutLogAside,
    ScoutLogEntry, ScoutLogItem, ScoutLogWatch, look_entry, scout_log_brief, with_look,
};
use self::scheduler::{RealScheduler, ScoutScheduler};
use self::store::{
    Kept, MAX_LOOK_IDS, MAX_REPEATED, SCOUT_FILE, SCOUT_LOG_FILE, ScoutFiles, SinceBrief,
    load_kept_with, load_log_with,
};
use self::types::{
    AnswerTeammate, BriefCards, BriefCheck, BriefGithubMissed, BriefHeld, BriefPrs, BriefWaiting,
    FailedTurn, FolderCheckResult, LastCheck, MAX_SCOUT_NOTES, MAX_SCOUT_SEEN, PromptTeammate,
    SCOUT_WAIT_MOST_MS, ScoutAnswer, ScoutCard, ScoutChange, ScoutFact, ScoutOutcome,
    ScoutSettings, ScoutState, ScoutToday, ScoutWatchKey, ScoutWeek, WaitingCard,
    apply_settings_change, brief_fact, brief_span, card_of, check_facts, days_between, empty_week,
    failed_facts, in_quiet_hours, iso_string, notes_from, pull_request_facts, review_facts,
    scout_answers, scout_day, scout_note_of, scout_prompt, waiting_facts, watch_of_key, week_fact,
    with_new_cards, new_issues, branch_facts,
};

/// How often a look waiting for the person's own turn asks again whether it has ended.
pub const WAIT_STEP_MS: u64 = 60_000;
/// A look's turn, past this, no longer keeps its cards waiting.
pub const ANSWER_WAIT_MS: u64 = 3 * 60_000;
/// Free models a look tries after Scout's own, each when the one before it would not answer.
pub const MAX_FALLBACKS: usize = 2;
const WEEK_DAYS: i64 = 7;
const STALE_WEEK_DAYS: i64 = 14;
const OFF_SAYS: &str = "Scout was turned off, so these cards stay in Locust’s words.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoutTeammate {
    pub teammate_id: String,
    pub name: String,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoutRoute {
    pub runtime: String,
    pub model: String,
    pub account: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskResult {
    Started { mission_id: String, run_id: String },
    Refused(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnswerResult {
    Text(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefRuns {
    pub finished: u32,
    pub failed: u32,
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnOnOutcome {
    Ok(ScoutState),
    Refused(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoutLogSnapshot {
    pub entries: Vec<ScoutLogEntry>,
    pub held: Vec<ScoutHeldAlert>,
}

pub trait ScoutDeps: Send + Sync {
    fn files(&self) -> ScoutFiles;
    fn now(&self) -> DateTime<FixedOffset>;
    fn github(&self) -> Result<Option<ScoutGithub>, ScoutGithubUnread>;
    fn waiting(&self) -> Result<Vec<WaitingCard>, String>;
    fn failed_turn(&self, mission_id: &str) -> Result<Option<FailedTurn>, String>;
    fn continues_from(&self, _mission_id: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn team(&self) -> Result<Vec<ScoutTeammate>, String>;
    fn ask(&self, teammate_id: &str, prompt: &str, model: Option<&str>) -> AskResult;
    fn stop(&self, run_id: &str);
    fn next_free_model(&self, teammate_id: &str, tried: &[String]) -> Option<String>;
    fn answer_of(&self, mission_id: &str) -> AnswerResult;
    fn make_teammate(&self, route: &ScoutRoute) -> Result<String, String>;
    fn teammate_exists(&self, teammate_id: &str) -> Result<bool, String>;
    fn changed(&self, _state: &ScoutState) {}
    fn notify(&self, _card: &ScoutCard) {}
    fn raised(&self, _cards: &[ScoutCard]) {}
    fn answer_wait_ms(&self) -> u64 {
        ANSWER_WAIT_MS
    }
    fn folder_mark(&self) -> Option<String> {
        None
    }
    fn run_check(&self) -> Option<FolderCheckResult> {
        None
    }
    fn has_check_support(&self) -> bool {
        false
    }
    fn folder_busy(&self) -> bool {
        false
    }
    fn person_busy(&self) -> bool {
        false
    }
    fn brief_facts(&self, _since: &str, _looks: &[String]) -> Option<BriefRuns> {
        None
    }
    fn held_alerts(&self, _since_ms: i64) -> Vec<ScoutHeldAlert> {
        Vec::new()
    }
}

struct PendingLook {
    teammate_id: String,
    prompt: String,
    card_ids: Vec<String>,
    team: Vec<ScoutTeammate>,
    run_id: String,
    tried: Vec<String>,
    at: DateTime<FixedOffset>,
    expires_handle: Option<u64>,
}

struct EngineState {
    kept: Option<Kept>,
    save_to_disk: bool,
    save_gen: u64,
    last_written_gen: u64,
    look_log: Option<Vec<ScoutLogEntry>>,
    log_to_disk: bool,
    log_gen: u64,
    last_written_log_gen: u64,
    looking: bool,
    failed_since: Vec<FailedTurn>,
    pending: HashMap<String, PendingLook>,
    timer_handle: Option<u64>,
    first_look_handle: Option<u64>,
    deferred_handle: Option<u64>,
}

impl EngineState {
    fn new() -> Self {
        Self {
            kept: None,
            save_to_disk: true,
            save_gen: 0,
            last_written_gen: 0,
            look_log: None,
            log_to_disk: true,
            log_gen: 0,
            last_written_log_gen: 0,
            looking: false,
            failed_since: Vec::new(),
            pending: HashMap::new(),
            timer_handle: None,
            first_look_handle: None,
            deferred_handle: None,
        }
    }
}

struct GatherResult {
    facts: Vec<ScoutFact>,
    prs: Vec<FolderPullRequest>,
    folder_mark: Option<String>,
    last_check: Option<LastCheck>,
    github_read: bool,
    github_why: Option<ScoutGithubUnread>,
    check_stale: bool,
    repeated: Vec<String>,
    aside: Vec<ScoutLogAside>,
}

pub struct Scout {
    deps: Arc<dyn ScoutDeps>,
    scheduler: Arc<dyn ScoutScheduler>,
    state: Mutex<EngineState>,
    write_lock: Mutex<()>,
    log_write_lock: Mutex<()>,
    disposed: AtomicBool,
}

fn counted(was: Option<&ScoutToday>, at: DateTime<FixedOffset>, asked: bool) -> ScoutToday {
    let day = scout_day(at);
    let mut next = match was {
        Some(prev) if prev.day == day => prev.clone(),
        _ => ScoutToday {
            day,
            looks: 0,
            asked: 0,
            fixes: None,
        },
    };
    if asked {
        next.asked += 1;
    } else {
        next.looks += 1;
    }
    next
}

fn week_counted(was: Option<&ScoutWeek>, at: DateTime<FixedOffset>, asked: bool) -> ScoutWeek {
    let mut next = was.cloned().unwrap_or_else(|| empty_week(&scout_day(at)));
    if asked {
        next.asked += 1;
    } else {
        next.looks += 1;
    }
    next
}

fn week_raised(mut week: ScoutWeek, facts: &[ScoutFact]) -> ScoutWeek {
    for fact in facts {
        if let Some(watch) = watch_of_key(&fact.key) {
            week.watch_mut(watch).raised += 1;
        }
    }
    week
}

fn look_tallied(
    was: Option<&SinceBrief>,
    github_missed: bool,
    raised_facts: &[ScoutFact],
    repeated_keys: &[String],
) -> SinceBrief {
    let mut next = was.cloned().unwrap_or_default();
    for fact in raised_facts {
        match watch_of_key(&fact.key) {
            Some(ScoutWatchKey::Github) => next.raised.github += 1,
            Some(ScoutWatchKey::Teammates) => next.raised.teammates += 1,
            _ if fact.key.starts_with("comment") => next.raised.comments += 1,
            _ => {}
        }
    }
    for key in repeated_keys {
        if !next.repeated.contains(key) {
            next.repeated.push(key.clone());
        }
    }
    if next.repeated.len() > MAX_REPEATED {
        let start = next.repeated.len() - MAX_REPEATED;
        next.repeated.drain(..start);
    }
    next.looks += 1;
    if github_missed {
        next.github_missed += 1;
    }
    next
}

fn off_of(settings: &ScoutSettings) -> Vec<ScoutLogWatch> {
    let mut out = Vec::new();
    if !settings.watches.github {
        out.push(ScoutLogWatch::Github);
    }
    if !settings.watches.comments {
        out.push(ScoutLogWatch::Comments);
    }
    if !settings.watches.teammates {
        out.push(ScoutLogWatch::Teammates);
    }
    if !settings.watches.check {
        out.push(ScoutLogWatch::Check);
    }
    out
}

fn new_card_id() -> String {
    format!("card_{}", crate::invariants::unique_nanos())
}

struct LookingGuard<'a> {
    scout: &'a Scout,
}

impl Drop for LookingGuard<'_> {
    fn drop(&mut self) {
        self.scout
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .looking = false;
    }
}

impl Scout {
    pub fn new(deps: Arc<dyn ScoutDeps>) -> Arc<Self> {
        let scheduler = RealScheduler::new();
        Self::with_scheduler(deps, scheduler)
    }

    pub fn with_scheduler(deps: Arc<dyn ScoutDeps>, scheduler: Arc<dyn ScoutScheduler>) -> Arc<Self> {
        Arc::new(Self {
            deps,
            scheduler,
            state: Mutex::new(EngineState::new()),
            write_lock: Mutex::new(()),
            log_write_lock: Mutex::new(()),
            disposed: AtomicBool::new(false),
        })
    }

    fn load(&self) -> Kept {
        if let Some(kept) = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .kept
            .clone()
        {
            return kept;
        }
        let files = self.deps.files();
        let now = self.deps.now();
        let sched = &self.scheduler;
        let loaded = load_kept_with(&files, now, |p| fs::read_to_string(p), |d| sched.retry_sleep(d));
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = &st.kept {
            return existing.clone();
        }
        st.save_to_disk = loaded.save_to_disk;
        st.kept = Some(loaded.kept.clone());
        loaded.kept
    }

    fn persist_kept(&self) {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let (target_gen, value, save_to_disk) = {
            let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.last_written_gen >= st.save_gen {
                return;
            }
            let Some(kept) = &st.kept else { return };
            (st.save_gen, kept.to_json(), st.save_to_disk)
        };
        if save_to_disk {
            let files = self.deps.files();
            if let Ok(path) = files.scout_path() {
                let _ = files.save_json_atomic(&path, SCOUT_FILE, &value);
            }
        }
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.last_written_gen = st.last_written_gen.max(target_gen);
    }

    fn mutate<F>(&self, change: F) -> ScoutState
    where
        F: FnOnce(&Kept) -> Kept,
    {
        let _ = self.load();
        let state = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let current = st.kept.take().unwrap_or_default();
            let next = change(&current);
            st.save_gen += 1;
            let state = next.state(self.deps.now());
            st.kept = Some(next);
            state
        };
        self.persist_kept();
        self.deps.changed(&state);
        state
    }

    fn load_log(&self) -> Vec<ScoutLogEntry> {
        if let Some(log) = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .look_log
            .clone()
        {
            return log;
        }
        let files = self.deps.files();
        let (entries, disk_ok) = load_log_with(&files, |p| fs::read_to_string(p));
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = &st.look_log {
            return existing.clone();
        }
        st.log_to_disk = disk_ok;
        st.look_log = Some(entries.clone());
        entries
    }

    fn log_look(&self, entry: ScoutLogEntry) {
        let _ = self.load_log();
        let now = self.deps.now();
        let (target_gen, value, log_to_disk) = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let current = st.look_log.take().unwrap_or_default();
            let next = with_look(&current, entry, now);
            st.log_gen += 1;
            let val = Value::Array(next.iter().map(ScoutLogEntry::to_json).collect());
            st.look_log = Some(next);
            (st.log_gen, val, st.log_to_disk)
        };
        let _guard = self.log_write_lock.lock().unwrap_or_else(|e| e.into_inner());
        if log_to_disk {
            let files = self.deps.files();
            if let Ok(Some(path)) = files.log_path() {
                let _ = files.save_json_atomic(&path, SCOUT_LOG_FILE, &value);
            }
        }
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.last_written_log_gen = st.last_written_log_gen.max(target_gen);
    }

    fn held_in_log(&self) -> Vec<ScoutHeldAlert> {
        let since_ms = self.deps.now().timestamp_millis() - SCOUT_LOG_DAYS * 86_400_000;
        self.deps.held_alerts(since_ms)
    }

    fn restart(self: &Arc<Self>, settings: &ScoutSettings) {
        let prev_timer = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.timer_handle.take()
        };
        if let Some(h) = prev_timer {
            self.scheduler.clear(h);
        }
        if !self.disposed.load(Ordering::SeqCst)
            && settings.on
            && settings.teammate_id.is_some()
        {
            let weak: Weak<Self> = Arc::downgrade(self);
            let h = self.scheduler.schedule(
                u64::from(settings.every_minutes) * 60_000,
                Box::new(move || {
                    if let Some(me) = weak.upgrade() {
                        me.due(None);
                    }
                }),
            );
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if self.disposed.load(Ordering::SeqCst) {
                self.scheduler.clear(h);
            } else {
                st.timer_handle = Some(h);
            }
        }
    }

    fn due(self: &Arc<Self>, since_ms: Option<i64>) {
        if self.disposed.load(Ordering::SeqCst) {
            return;
        }
        let now_ms = self.deps.now().timestamp_millis();
        let since = since_ms.unwrap_or(now_ms);
        let already_deferred = {
            let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.deferred_handle.is_some()
        };
        if already_deferred {
            return;
        }
        let waits = {
            let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.kept
                .as_ref()
                .map_or(ScoutSettings::default().wait_while_working, |k| {
                    k.settings.wait_while_working
                })
        };
        if waits
            && self.deps.person_busy()
            && (now_ms - since) < SCOUT_WAIT_MOST_MS as i64
        {
            let weak: Weak<Self> = Arc::downgrade(self);
            let h = self.scheduler.later(
                WAIT_STEP_MS,
                Box::new(move || {
                    if let Some(me) = weak.upgrade() {
                        me.state
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .deferred_handle = None;
                        me.due(Some(since));
                    }
                }),
            );
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.deferred_handle = Some(h);
            return;
        }
        let _ = self.look_mode(true);
    }

    fn still_looking(&self, scheduled: bool) -> bool {
        let now = self.deps.now();
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let settings = st
            .kept
            .as_ref()
            .map(|k| &k.settings);
        let Some(settings) = settings else { return false };
        settings.on && !(scheduled && in_quiet_hours(settings.quiet, now))
    }

    fn gather(&self, from: &Kept, look_stamp: &str) -> GatherResult {
        let mut facts = Vec::new();
        let mut aside = Vec::new();
        let seen: HashSet<&str> = from.seen.iter().map(String::as_str).collect();
        let mut prs = from.prs.clone();
        let mut github_read = false;
        let mut github_why = None;
        let mut check_stale = false;
        let mut folder_mark = from.folder_mark.clone();
        let mut last_check = from.last_check.clone();

        if from.settings.watches.check && self.deps.has_check_support() && !self.deps.folder_busy()
        {
            let mark = self.deps.folder_mark();
            if let Some(m) = &mark
                && mark.as_deref() != from.folder_mark.as_deref()
                && !self.deps.folder_busy()
                && let Some(result) = self.deps.run_check()
                && !self.deps.folder_busy()
            {
                let before = from.last_check.as_ref().map(|c| c.passed);
                facts.extend(check_facts(before, &result, m));
                folder_mark = Some(m.clone());
                last_check = Some(LastCheck {
                    passed: result.passed,
                    command: Some(result.command),
                });
            }
            check_stale = mark.is_some() && mark != folder_mark;
        }

        if from.settings.watches.github {
            match self.deps.github() {
                Ok(Some(github)) => {
                    github_read = true;
                    let now = self.deps.now();
                    let pr_map: HashMap<String, FolderPullRequest> =
                        from.prs.iter().map(|p| (p.url.clone(), p.clone())).collect();
                    facts.extend(pull_request_facts(&pr_map, &github.prs, look_stamp, now));
                    facts.extend(review_facts(&github.reviews));
                    if let Some(branch) = &github.branch {
                        facts.extend(branch_facts(&branch.name, &branch.runs));
                    }
                    let split = new_issues(&github.issues, now, &seen.iter().map(|s| (*s).to_owned()).collect());
                    facts.extend(split.facts);
                    for fact in split.more {
                        aside.push(ScoutLogAside::new(
                            fact.key,
                            fact.said,
                            ScoutAsideWhy::Many,
                        ));
                    }
                    prs = github.prs;
                }
                Ok(None) => {}
                Err(why) => {
                    github_why = Some(why);
                }
            }
        }

        let recent_failed: Vec<FailedTurn> = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut st.failed_since)
        };
        if from.settings.watches.teammates {
            let waiting = self.deps.waiting().unwrap_or_default();
            facts.extend(waiting_facts(
                &waiting,
                self.deps.now(),
                types::SCOUT_WAITING_MS,
            ));
            facts.extend(failed_facts(&recent_failed));
        }

        let mut repeated = Vec::new();
        for fact in &facts {
            if seen.contains(fact.key.as_str()) && !repeated.contains(&fact.key) {
                repeated.push(fact.key.clone());
                aside.push(ScoutLogAside::new(
                    fact.key.clone(),
                    fact.said.clone(),
                    ScoutAsideWhy::Told,
                ));
            }
        }
        let mut unique_keys = HashSet::new();
        let fresh_facts: Vec<ScoutFact> = facts
            .into_iter()
            .filter(|fact| !seen.contains(fact.key.as_str()) && unique_keys.insert(fact.key.clone()))
            .collect();

        GatherResult {
            facts: fresh_facts,
            prs,
            folder_mark,
            last_check,
            github_read,
            github_why,
            check_stale,
            repeated,
            aside,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn brief_of(
        &self,
        from: &Kept,
        news: &[ScoutFact],
        prs: &[FolderPullRequest],
        last_check: Option<&LastCheck>,
        at: DateTime<FixedOffset>,
        github_read: bool,
        check_stale: bool,
        outdated: &dyn Fn(&ScoutCard) -> bool,
        looks: &SinceBrief,
    ) -> ScoutFact {
        let since = from
            .last_brief_at
            .clone()
            .unwrap_or_else(|| iso_string(at - ChronoDuration::hours(24)));
        let ran = self.deps.brief_facts(&since, &from.look_ids);
        let waiting = if from.settings.watches.teammates {
            self.deps.waiting().unwrap_or_default()
        } else {
            Vec::new()
        };
        let counts = |key: &str| !key.starts_with("brief:") && !key.starts_with("waiting:");
        let others: Vec<&ScoutCard> = from
            .cards
            .iter()
            .filter(|c| counts(&c.fact.key) && !outdated(c))
            .collect();
        let told: Vec<&ScoutFact> = news.iter().filter(|f| counts(&f.key)).collect();
        let needs_work = |pr: &FolderPullRequest| {
            pr.checks == PullRequestChecks::Failing
                || pr.review == Some(PullRequestReview::ChangesRequested)
                || pr.conflicts
        };
        let open: Vec<&FolderPullRequest> = prs
            .iter()
            .filter(|pr| pr.state == PullRequestState::Open)
            .collect();
        let mut quiet = Vec::new();
        if from.settings.watches.github
            && looks.looks > looks.github_missed
            && looks.raised.github == 0
            && !open.iter().any(|pr| needs_work(pr))
        {
            quiet.push("GitHub".to_owned());
        }
        if from.settings.watches.teammates
            && looks.raised.teammates == 0
            && waiting.is_empty()
            && ran.as_ref().is_none_or(|r| r.failed == 0)
        {
            quiet.push("your teammates".to_owned());
        }
        let since_ms = DateTime::parse_from_rfc3339(&since)
            .map(|d| d.timestamp_millis())
            .unwrap_or_else(|_| at.timestamp_millis() - 86_400_000);
        let held = self
            .deps
            .held_alerts(since_ms)
            .into_iter()
            .map(|one| BriefHeld {
                title: one.title,
                why: one.why,
            })
            .collect();
        let since_dt = DateTime::parse_from_rfc3339(&since)
            .map(|d| d.with_timezone(at.offset()))
            .unwrap_or(at - ChronoDuration::hours(24));
        let now_cards = others
            .iter()
            .filter(|c| c.fact.urgency == types::ScoutUrgency::Now)
            .count()
            + told
                .iter()
                .filter(|f| f.urgency == types::ScoutUrgency::Now)
                .count();
        let brief = types::ScoutBrief {
            waiting: waiting
                .into_iter()
                .map(|card| {
                    let req_ms = DateTime::parse_from_rfc3339(&card.requested_at)
                        .map(|d| d.timestamp_millis())
                        .unwrap_or_else(|_| at.timestamp_millis());
                    let mins = (((at.timestamp_millis() - req_ms) as f64) / 60_000.0)
                        .round()
                        .max(0.0) as i64;
                    BriefWaiting {
                        name: card.name,
                        minutes: mins,
                    }
                })
                .collect(),
            finished: ran.as_ref().map_or(0, |r| r.finished),
            failed: ran.as_ref().map_or(0, |r| r.failed),
            more: ran.as_ref().is_some_and(|r| r.more),
            span: Some(brief_span(since_dt, at)),
            check: if from.settings.watches.check
                && let Some(c) = last_check
                && let Some(cmd) = &c.command
            {
                Some(BriefCheck {
                    command: cmd.clone(),
                    passed: c.passed,
                    stale: check_stale,
                })
            } else {
                None
            },
            prs: if from.settings.watches.github && github_read && !open.is_empty() {
                Some(BriefPrs {
                    open: open.len(),
                    needs_work: open.iter().filter(|pr| needs_work(pr)).count(),
                })
            } else {
                None
            },
            cards: BriefCards {
                total: others.len() + told.len(),
                now: now_cards,
            },
            quiet,
            github_missed: if from.settings.watches.github && looks.github_missed > 0 {
                Some(BriefGithubMissed {
                    missed: looks.github_missed,
                    looks: looks.looks,
                })
            } else {
                None
            },
            set_aside: if looks.repeated.is_empty() {
                None
            } else {
                Some(looks.repeated.len())
            },
            held,
        };
        brief_fact(&brief, &scout_day(at))
    }

    pub fn look(self: &Arc<Self>) -> ScoutState {
        self.look_mode(false)
    }

    pub fn look_scheduled(self: &Arc<Self>) -> ScoutState {
        self.look_mode(true)
    }

    fn look_mode(self: &Arc<Self>, scheduled: bool) -> ScoutState {
        let from = self.load();
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.looking || !from.settings.on || from.settings.teammate_id.is_none() {
                return from.state(self.deps.now());
            }
            if scheduled && in_quiet_hours(from.settings.quiet, self.deps.now()) {
                drop(st);
                let now = self.deps.now();
                self.log_look(look_entry(&LookEntryInput {
                    at: now,
                    raised: &[],
                    aside: &[],
                    off: &[],
                    github_unread: false,
                    github: None,
                    skipped_quiet_hours: true,
                    away: false,
                }));
                return from.state(self.deps.now());
            }
            st.looking = true;
        }
        let _guard = LookingGuard { scout: self };
        let Some(teammate_id) = from.settings.teammate_id.clone() else {
            return from.state(self.deps.now());
        };
        if self.deps.teammate_exists(&teammate_id).unwrap_or(true) == false {
            let state = self.mutate(|latest| {
                let mut next = latest.clone();
                next.settings.on = false;
                next
            });
            self.restart(&state.settings);
            return state;
        }

        let at = self.deps.now();
        let stamp = iso_string(at);
        let gathered = self.gather(&from, &stamp);
        if !self.still_looking(scheduled) {
            return self.load().state(self.deps.now());
        }

        let mut facts = gathered.facts;
        let day = scout_day(at);
        let age = from
            .week
            .as_ref()
            .map_or(0, |w| days_between(&w.from, &day));
        let week_due = age >= WEEK_DAYS;
        let week = if week_due
            && age < STALE_WEEK_DAYS
            && from.week.as_ref().is_some_and(|w| w.looks > 0)
        {
            from.week.as_ref().map(|w| {
                week_fact(w, from.settings.every_minutes, from.settings.watches)
            })
        } else {
            None
        };
        let check_turned = facts.iter().any(|f| f.key.starts_with("check:"));
        let brief_due = from.settings.brief_at.is_some_and(|h| at.hour() >= h)
            && from.last_brief_day.as_deref() != Some(&day)
            && !in_quiet_hours(from.settings.quiet, at);
        let outdated = |card: &ScoutCard| -> bool {
            (check_turned && card.fact.key.starts_with("check:"))
                || (brief_due && card.fact.key.starts_with("brief:"))
                || (week_due && card.fact.key.starts_with("week:"))
        };
        let github_missed = from.settings.watches.github && !gathered.github_read;
        let tallied_now = look_tallied(
            from.since_brief.as_ref(),
            github_missed,
            &facts,
            &gathered.repeated,
        );
        if brief_due {
            let mut for_brief = facts.clone();
            if let Some(w) = &week {
                for_brief.push(w.clone());
            }
            let bf = self.brief_of(
                &from,
                &for_brief,
                &gathered.prs,
                gathered.last_check.as_ref(),
                at,
                gathered.github_read,
                gathered.check_stale,
                &outdated,
                &tallied_now,
            );
            facts.push(bf);
            if !self.still_looking(scheduled) {
                return self.load().state(self.deps.now());
            }
        }
        if let Some(w) = week {
            facts.push(w);
        }

        let log_reason = if from.settings.watches.github && !gathered.github_read {
            match gathered.github_why {
                Some(ScoutGithubUnread::NotOnGithub) => Some(LogGithubReason::NotOnGithub),
                Some(ScoutGithubUnread::SignedOut) => Some(LogGithubReason::SignedOut),
                Some(ScoutGithubUnread::NoCli) => Some(LogGithubReason::NoCli),
                _ => None,
            }
        } else {
            None
        };
        let github_unread_flag = from.settings.watches.github
            && !gathered.github_read
            && gathered.github_why != Some(ScoutGithubUnread::NotOnGithub);
        let write_log = |raised: &[ScoutFact]| {
            let raised_items: Vec<ScoutLogItem> = raised
                .iter()
                .map(|f| ScoutLogItem::new(f.key.clone(), f.said.clone()))
                .collect();
            self.log_look(look_entry(&LookEntryInput {
                at,
                raised: &raised_items,
                aside: &gathered.aside,
                off: &off_of(&from.settings),
                github_unread: github_unread_flag,
                github: log_reason,
                skipped_quiet_hours: false,
                away: false,
            }));
        };

        if facts.is_empty() {
            let next_prs = gathered.prs;
            let next_mark = gathered.folder_mark;
            let next_check = gathered.last_check;
            let quiet = self.mutate(|latest| {
                let mut next = latest.clone();
                next.folder_mark = next_mark;
                next.last_check = next_check;
                next.prs = next_prs;
                next.last_look_at = Some(stamp.clone());
                next.today = Some(counted(latest.today.as_ref(), at, false));
                let base_week = if week_due {
                    Some(empty_week(&day))
                } else {
                    latest.week.clone()
                };
                next.week = Some(week_counted(base_week.as_ref(), at, false));
                next.since_brief = Some(if brief_due {
                    SinceBrief::default()
                } else {
                    look_tallied(
                        latest.since_brief.as_ref(),
                        github_missed,
                        &[],
                        &gathered.repeated,
                    )
                });
                next
            });
            write_log(&[]);
            return quiet;
        }

        let team: Vec<ScoutTeammate> = self
            .deps
            .team()
            .unwrap_or_default()
            .into_iter()
            .filter(|mate| mate.teammate_id != teammate_id)
            .collect();
        let check_passes_re = crate::invariants::regex(r"^check:.*:passes$");
        let for_model = |fact: &ScoutFact| -> bool {
            !fact.key.starts_with("comment-answer:")
                && !fact.key.starts_with("week:")
                && !fact.key.starts_with("brief:")
                && !check_passes_re.is_match(&fact.key)
        };
        let cards: Vec<ScoutCard> = facts
            .iter()
            .map(|fact| card_of(fact.clone(), new_card_id(), at, for_model(fact)))
            .collect();

        let next_prs = gathered.prs;
        let next_mark = gathered.folder_mark;
        let next_check = gathered.last_check;
        let facts_for_log = facts.clone();
        let state = self.mutate(|latest| {
            let mut next = latest.clone();
            next.folder_mark = next_mark;
            next.last_check = next_check;
            if brief_due {
                next.last_brief_day = Some(day.clone());
                next.last_brief_at = Some(stamp.clone());
            }
            let filtered_existing: Vec<ScoutCard> = latest
                .cards
                .iter()
                .filter(|c| !outdated(c))
                .cloned()
                .collect();
            next.cards = with_new_cards(&filtered_existing, &cards);
            next.seen.extend(facts.iter().map(|f| f.key.clone()));
            if next.seen.len() > MAX_SCOUT_SEEN {
                let start = next.seen.len() - MAX_SCOUT_SEEN;
                next.seen.drain(..start);
            }
            next.prs = next_prs;
            next.last_look_at = Some(stamp.clone());
            next.today = Some(counted(latest.today.as_ref(), at, false));
            let base_week = if week_due {
                Some(empty_week(&day))
            } else {
                latest.week.clone()
            };
            let w_counted = week_counted(base_week.as_ref(), at, false);
            next.week = Some(week_raised(w_counted, &facts));
            next.since_brief = Some(if brief_due {
                SinceBrief::default()
            } else {
                look_tallied(
                    latest.since_brief.as_ref(),
                    github_missed,
                    &facts,
                    &gathered.repeated,
                )
            });
            next
        });
        write_log(&facts_for_log);
        if state.settings.notify
            && state.settings.on
            && !in_quiet_hours(state.settings.quiet, self.deps.now())
        {
            for card in &cards {
                if card.fact.urgency == types::ScoutUrgency::Now {
                    self.deps.notify(card);
                }
            }
        }
        if state.settings.on && !cards.is_empty() {
            self.deps.raised(&cards);
        }

        let asked_cards: Vec<&ScoutCard> = cards.iter().filter(|c| for_model(&c.fact)).collect();
        if asked_cards.is_empty() {
            return state;
        }
        let asked_ids: Vec<String> = asked_cards.iter().map(|c| c.card_id.clone()).collect();
        if !self.still_looking(scheduled) {
            let reason = if state.settings.on {
                "Scout’s quiet hours began, so these cards stay in Locust’s words."
            } else {
                OFF_SAYS
            };
            return self.settle(&asked_ids, None, Some(reason));
        }

        let prompt_facts: Vec<ScoutFact> = facts.into_iter().filter(|f| for_model(f)).collect();
        let prompt_team: Vec<PromptTeammate<'_>> = team
            .iter()
            .map(|m| PromptTeammate {
                name: &m.name,
                role: &m.role,
            })
            .collect();
        let prompt = scout_prompt(
            &prompt_facts,
            &prompt_team,
            state.settings.goal.as_deref(),
            &state.notes,
        );
        self.ask_for(
            teammate_id,
            prompt,
            asked_ids,
            team,
            Vec::new(),
            at,
            None,
        )
        .unwrap_or(state)
    }

    #[allow(clippy::too_many_arguments)]
    fn ask_for(
        self: &Arc<Self>,
        teammate_id: String,
        prompt: String,
        card_ids: Vec<String>,
        team: Vec<ScoutTeammate>,
        tried: Vec<String>,
        at: DateTime<FixedOffset>,
        model: Option<&str>,
    ) -> Option<ScoutState> {
        let asked = self.deps.ask(&teammate_id, &prompt, model);
        let (mission_id, run_id) = match asked {
            AskResult::Refused(why) => return Some(self.settle(&card_ids, None, Some(&why))),
            AskResult::Started { mission_id, run_id } => (mission_id, run_id),
        };
        let is_on = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .kept
            .as_ref()
            .is_some_and(|k| k.settings.on);
        if !is_on {
            self.deps.stop(&run_id);
            return Some(self.settle(&card_ids, None, Some(OFF_SAYS)));
        }
        let weak: Weak<Self> = Arc::downgrade(self);
        let mid_for_timer = mission_id.clone();
        let expires = self.scheduler.later(
            self.deps.answer_wait_ms(),
            Box::new(move || {
                if let Some(me) = weak.upgrade() {
                    me.gave_up(&mid_for_timer, "Scout’s model took too long to answer.");
                }
            }),
        );
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if self.disposed.load(Ordering::SeqCst) {
                self.scheduler.cancel(expires);
                self.deps.stop(&run_id);
                return None;
            }
            st.pending.insert(
                mission_id.clone(),
                PendingLook {
                    teammate_id,
                    prompt,
                    card_ids,
                    team,
                    run_id,
                    tried,
                    at,
                    expires_handle: Some(expires),
                },
            );
        }
        Some(self.mutate(|latest| {
            let mut next = latest.clone();
            next.look_ids.push(mission_id.clone());
            if next.look_ids.len() > MAX_LOOK_IDS {
                let start = next.look_ids.len() - MAX_LOOK_IDS;
                next.look_ids.drain(..start);
            }
            next.today = Some(counted(latest.today.as_ref(), at, true));
            next.week = Some(week_counted(latest.week.as_ref(), at, true));
            next
        }))
    }

    fn stop_looks(&self, why: &str) {
        let (handles, runs, card_ids) = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let mut handles = Vec::new();
            if let Some(h) = st.first_look_handle.take() {
                handles.push(h);
            }
            if let Some(h) = st.deferred_handle.take() {
                handles.push(h);
            }
            let mut runs = Vec::new();
            let mut card_ids = Vec::new();
            for (_, look) in st.pending.drain() {
                if let Some(h) = look.expires_handle {
                    handles.push(h);
                }
                runs.push(look.run_id);
                card_ids.extend(look.card_ids);
            }
            (handles, runs, card_ids)
        };
        for h in handles {
            self.scheduler.cancel(h);
        }
        for run_id in runs {
            self.deps.stop(&run_id);
        }
        if !card_ids.is_empty() {
            let _ = self.settle(&card_ids, None, Some(why));
        }
    }

    fn gave_up(self: &Arc<Self>, mission_id: &str, why: &str) {
        let removed = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.pending.remove(mission_id)
        };
        let Some(look) = removed else { return };
        if let Some(h) = look.expires_handle {
            self.scheduler.cancel(h);
        }
        self.deps.stop(&look.run_id);
        let is_on = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .kept
            .as_ref()
            .is_some_and(|k| k.settings.on);
        let next = if look.tried.len() < MAX_FALLBACKS && is_on {
            self.deps.next_free_model(&look.teammate_id, &look.tried)
        } else {
            None
        };
        match next {
            None => {
                let _ = self.settle(
                    &look.card_ids,
                    None,
                    Some(&format!("{why} These cards are in Locust’s words.")),
                );
            }
            Some(model) => {
                let mut tried = look.tried;
                tried.push(model.clone());
                let _ = self.ask_for(
                    look.teammate_id,
                    look.prompt,
                    look.card_ids,
                    look.team,
                    tried,
                    look.at,
                    Some(&model),
                );
            }
        }
    }

    fn settle(
        &self,
        card_ids: &[String],
        answers: Option<&HashMap<usize, ScoutAnswer>>,
        why: Option<&str>,
    ) -> ScoutState {
        self.mutate(|from| {
            let mut next = from.clone();
            next.cards = from
                .cards
                .iter()
                .map(|card| {
                    let Some(at) = card_ids.iter().position(|id| id == &card.card_id) else {
                        return card.clone();
                    };
                    let mut updated = card.clone();
                    updated.reading = false;
                    if let Some(ans) = answers.and_then(|m| m.get(&(at + 1))) {
                        if let Some(take) = &ans.take {
                            updated.fact.take_set(take.clone(), &mut updated.take);
                        }
                        if let Some(draft) = &ans.draft {
                            updated.fact.draft = Some(draft.clone());
                        }
                    }
                    updated
                })
                .collect();
            next.model_said = why.map(str::to_owned);
            next
        })
    }

    fn ended(self: &Arc<Self>, mission_id: &str) {
        let exists = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .contains_key(mission_id);
        if !exists {
            return;
        }
        match self.deps.answer_of(mission_id) {
            AnswerResult::Failed(why) => {
                self.gave_up(mission_id, &why);
            }
            AnswerResult::Text(text) => {
                let removed = {
                    let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
                    st.pending.remove(mission_id)
                };
                let Some(look) = removed else { return };
                if let Some(h) = look.expires_handle {
                    self.scheduler.cancel(h);
                }
                let answer_team: Vec<AnswerTeammate<'_>> = look
                    .team
                    .iter()
                    .map(|m| AnswerTeammate {
                        teammate_id: &m.teammate_id,
                        name: &m.name,
                    })
                    .collect();
                let parsed = scout_answers(&text, look.card_ids.len(), &answer_team);
                let _ = self.settle(&look.card_ids, Some(&parsed), None);
                if let Some(note) = scout_note_of(&text) {
                    let day = scout_day(self.deps.now());
                    self.mutate(|latest| {
                        if latest.notes.iter().any(|k| k.ends_with(&note)) {
                            return latest.clone();
                        }
                        let mut next = latest.clone();
                        next.notes.push(format!("{day}: {note}"));
                        if next.notes.len() > MAX_SCOUT_NOTES {
                            let start = next.notes.len() - MAX_SCOUT_NOTES;
                            next.notes.drain(..start);
                        }
                        next
                    });
                }
            }
        }
    }

    pub fn state(&self) -> ScoutState {
        self.load().state(self.deps.now())
    }

    pub fn log(&self) -> ScoutLogSnapshot {
        ScoutLogSnapshot {
            entries: self.load_log(),
            held: self.held_in_log(),
        }
    }

    pub fn why_brief(&self) -> String {
        let log = self.load_log();
        let held = self.held_in_log();
        scout_log_brief(&log, &held, self.deps.now())
    }

    pub fn teammate_id(&self) -> Option<String> {
        self.load().settings.teammate_id
    }

    pub fn start(self: &Arc<Self>, first_look_ms: u64) {
        let from = self.load();
        if from.cards.iter().any(|c| c.reading) {
            self.mutate(|latest| {
                let mut next = latest.clone();
                for card in &mut next.cards {
                    card.reading = false;
                }
                next
            });
        }
        self.restart(&from.settings);
        if from.settings.on && !self.disposed.load(Ordering::SeqCst) {
            let weak: Weak<Self> = Arc::downgrade(self);
            let h = self.scheduler.later(
                first_look_ms,
                Box::new(move || {
                    if let Some(me) = weak.upgrade() {
                        me.state
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .first_look_handle = None;
                        me.due(None);
                    }
                }),
            );
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.first_look_handle = Some(h);
        }
    }

    pub fn turn_on(self: &Arc<Self>, route: Option<&ScoutRoute>) -> TurnOnOutcome {
        let from = self.load();
        let mut teammate_id = from.settings.teammate_id.clone();
        let exists = teammate_id
            .as_deref()
            .is_some_and(|id| self.deps.teammate_exists(id).unwrap_or(false));
        if !exists {
            let Some(route) = route else {
                return TurnOnOutcome::Refused(
                    "Scout needs a model to read with: install OpenCode for a free one, then turn it on."
                        .to_owned(),
                );
            };
            match self.deps.make_teammate(route) {
                Ok(id) => teammate_id = Some(id),
                Err(err) => return TurnOnOutcome::Refused(err),
            }
        }
        let state = self.mutate(|latest| {
            let mut next = latest.clone();
            next.settings.on = true;
            next.settings.teammate_id = teammate_id.clone();
            next
        });
        self.restart(&state.settings);
        TurnOnOutcome::Ok(self.look())
    }

    pub fn set(self: &Arc<Self>, change: &ScoutChange) -> ScoutState {
        let before = self.load().settings;
        let state = self.mutate(|latest| {
            let mut next = latest.clone();
            let preserved_id = latest.settings.teammate_id.clone();
            next.settings = apply_settings_change(&latest.settings, change);
            next.settings.teammate_id = preserved_id;
            next
        });
        if state.settings.every_minutes != before.every_minutes
            || state.settings.on != before.on
            || state.settings.watches.comments != before.watches.comments
        {
            self.restart(&state.settings);
        }
        if before.on && !state.settings.on {
            self.stop_looks(OFF_SAYS);
            return self.state();
        }
        state
    }

    pub fn dismiss(&self, card_id: &str, how: Option<ScoutOutcome>) -> ScoutState {
        self.mutate(|latest| {
            let card = latest.cards.iter().find(|one| one.card_id == card_id);
            let mut next = latest.clone();
            next.cards.retain(|one| one.card_id != card_id);
            let Some(card) = card else { return next };
            let Some(watch) = watch_of_key(&card.fact.key) else {
                return next;
            };
            let Some(week) = next.week.as_mut() else {
                return next;
            };
            let tz = *self.deps.now().offset();
            let card_day = DateTime::parse_from_rfc3339(&card.at)
                .map(|d| scout_day(d.with_timezone(&tz)))
                .unwrap_or_default();
            if card_day < week.from {
                return next;
            }
            let outcome = how.unwrap_or(ScoutOutcome::Dismissed);
            match outcome {
                ScoutOutcome::Dismissed => {
                    week.watch_mut(watch).dismissed += 1;
                    week.dismissed += 1;
                }
                ScoutOutcome::Sent => {
                    week.watch_mut(watch).used += 1;
                    week.sent += 1;
                }
                ScoutOutcome::Changed => {
                    week.watch_mut(watch).used += 1;
                    week.changed += 1;
                }
            }
            next
        })
    }

    pub fn tune(self: &Arc<Self>, card_id: &str) -> ScoutState {
        let suggested = self
            .load()
            .cards
            .into_iter()
            .find(|c| c.card_id == card_id)
            .and_then(|c| c.fact.tune);
        if let Some(tune) = suggested {
            if let Some(watch_off) = tune.watch_off {
                let patch = match watch_off {
                    ScoutWatchKey::Github => types::ScoutWatchesPatch {
                        github: Some(false),
                        ..Default::default()
                    },
                    ScoutWatchKey::Teammates => types::ScoutWatchesPatch {
                        teammates: Some(false),
                        ..Default::default()
                    },
                    ScoutWatchKey::Check => types::ScoutWatchesPatch {
                        check: Some(false),
                        ..Default::default()
                    },
                };
                self.set(&ScoutChange {
                    watches: Some(patch),
                    ..Default::default()
                });
            } else if let Some(every) = tune.every_minutes {
                self.set(&ScoutChange {
                    every_minutes: Some(every),
                    ..Default::default()
                });
            }
        }
        self.mutate(|latest| {
            let mut next = latest.clone();
            next.cards.retain(|c| c.card_id != card_id);
            next
        })
    }

    pub fn save_notes(&self, text: &str) -> ScoutState {
        self.mutate(|latest| {
            let mut next = latest.clone();
            next.notes = notes_from(text);
            next
        })
    }

    pub fn provider_refused(self: &Arc<Self>, mission_id: &str, why: &str) {
        let has = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .contains_key(mission_id);
        if has {
            self.gave_up(mission_id, why);
        }
    }

    pub fn on_run_ended(self: &Arc<Self>, mission_id: &str) {
        let is_pending = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .contains_key(mission_id);
        if is_pending {
            self.ended(mission_id);
            return;
        }
        let from = self.load();
        if from.look_ids.iter().any(|id| id == mission_id) {
            return;
        }
        if let Ok(Some(earlier)) = self.deps.continues_from(mission_id) {
            {
                let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
                st.failed_since.retain(|f| f.mission_id != earlier);
            }
            let key = format!("failed:{earlier}");
            if from.cards.iter().any(|c| c.fact.key == key) {
                self.mutate(|latest| {
                    let mut next = latest.clone();
                    next.cards.retain(|c| c.fact.key != key);
                    next
                });
            }
        }
        if !from.settings.on || !from.settings.watches.teammates {
            return;
        }
        if let Ok(Some(failed)) = self.deps.failed_turn(mission_id)
            && failed.teammate_id.as_deref() != from.settings.teammate_id.as_deref()
        {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .failed_since
                .push(failed);
        }
    }

    pub fn dispose(&self) {
        self.disposed.store(true, Ordering::SeqCst);
        let (timer, first, deferred, pending_items) = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            (
                st.timer_handle.take(),
                st.first_look_handle.take(),
                st.deferred_handle.take(),
                st.pending.drain().collect::<Vec<_>>(),
            )
        };
        if let Some(h) = timer {
            self.scheduler.clear(h);
        }
        if let Some(h) = first {
            self.scheduler.cancel(h);
        }
        if let Some(h) = deferred {
            self.scheduler.cancel(h);
        }
        for (_, look) in pending_items {
            if let Some(h) = look.expires_handle {
                self.scheduler.cancel(h);
            }
            self.deps.stop(&look.run_id);
        }
    }
}

trait FactExt {
    fn take_set(&self, take: String, target: &mut Option<String>);
}

impl FactExt for ScoutFact {
    fn take_set(&self, take: String, target: &mut Option<String>) {
        *target = Some(take);
    }
}
