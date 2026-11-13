//! Stateful Scout runner tests ported from:
//! - `apps/desktop/src/main/scout-keeps-its-goal-hours-and-word.test.ts` (17 tests)
//! - `apps/desktop/src/main/scout-keeps-watch.test.ts` (16 tests)
//! - `apps/desktop/src/main/scout-runs-the-check-only-when-the-folder-moved.test.ts` (2 tests)

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, TimeZone};
use serde_json::json;

use crate::pull_requests::{FolderPullRequest, PullRequestChecks, PullRequestState};

use super::folder::folder_mark_of;
use super::github::{ScoutGithub, ScoutGithubUnread};
use super::scheduler::FakeScheduler;
use super::store::ScoutFiles;
use super::types::{
    FailedTurn, FolderCheckResult, ReviewAsked, SCOUT_WAIT_MOST_MS, ScoutCard, ScoutChange,
    ScoutFact, ScoutOutcome, ScoutQuietHours, ScoutSettings, ScoutState, ScoutUrgency,
    ScoutWatchesPatch, WaitingCard, check_facts, in_quiet_hours, scout_prompt,
    scout_settings_from,
};
use super::{AnswerResult, AskResult, Scout, ScoutDeps, ScoutRoute, ScoutTeammate, TurnOnOutcome};

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "locust-scout-runner-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn route() -> ScoutRoute {
    ScoutRoute {
        runtime: "opencode".into(),
        model: "opencode/space-bunny-free".into(),
        account: None,
    }
}

fn local_dt(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> DateTime<FixedOffset> {
    let offset = FixedOffset::west_opt(5 * 3600).unwrap();
    offset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
}

fn pr_passing(number: u64, title: &str, url: &str) -> FolderPullRequest {
    FolderPullRequest {
        number,
        title: title.into(),
        url: url.into(),
        state: PullRequestState::Open,
        updated_at: None,
        review: None,
        checks: PullRequestChecks::Passing,
        check_count: 9,
        not_passed: 0,
        conflicts: false,
    }
}

fn pr_failing() -> FolderPullRequest {
    FolderPullRequest {
        checks: PullRequestChecks::Failing,
        not_passed: 2,
        ..pr_passing(
            212,
            "Talk starts from any app",
            "https://github.com/acme/app/pull/212",
        )
    }
}

fn review(number: u64, title: &str, url: &str) -> ReviewAsked {
    ReviewAsked {
        number,
        title: title.into(),
        url: url.into(),
        author: None,
    }
}

fn red_check() -> FolderCheckResult {
    FolderCheckResult {
        command: "npm test".into(),
        passed: false,
        lines: vec!["FAIL cart.test.js".into(), "expected 6, got 0".into()],
    }
}

fn green_check() -> FolderCheckResult {
    FolderCheckResult {
        command: "npm test".into(),
        passed: true,
        lines: vec![],
    }
}

type AskHook = Box<dyn FnMut(&str, &str, Option<&str>) -> AskResult + Send>;
type RunCheckHook = Box<dyn FnMut() -> Option<FolderCheckResult> + Send>;
type MakeTeammateHook = Box<dyn FnMut(&ScoutRoute) -> Result<String, String> + Send>;
type ContinuesHook = Box<dyn FnMut(&str) -> Result<Option<String>, String> + Send>;
type NextFreeModelHook = Box<dyn FnMut(&str, &[String]) -> Option<String> + Send>;

struct MockState {
    files: ScoutFiles,
    clock: DateTime<FixedOffset>,
    github: Option<ScoutGithub>,
    waiting: Vec<WaitingCard>,
    asked: Vec<String>,
    models: Vec<Option<String>>,
    stopped: Vec<String>,
    answers: HashMap<String, AnswerResult>,
    changes: Vec<ScoutState>,
    notified: Vec<String>,
    teammates: HashSet<String>,
    made: usize,
    person_busy: bool,
    folder_busy: bool,
    mark: Option<String>,
    check: Option<FolderCheckResult>,
    checks: usize,
    has_check_support: bool,
    answer_wait_ms: u64,
    ask_hook: Option<AskHook>,
    run_check_hook: Option<RunCheckHook>,
    make_teammate_hook: Option<MakeTeammateHook>,
    continues_hook: Option<ContinuesHook>,
    next_free_model_hook: Option<NextFreeModelHook>,
}

struct MockDeps(Arc<Mutex<MockState>>);

impl ScoutDeps for MockDeps {
    fn files(&self) -> ScoutFiles {
        self.0.lock().unwrap().files.clone()
    }
    fn now(&self) -> DateTime<FixedOffset> {
        self.0.lock().unwrap().clock
    }
    fn github(&self) -> Result<Option<ScoutGithub>, ScoutGithubUnread> {
        Ok(self.0.lock().unwrap().github.clone())
    }
    fn waiting(&self) -> Result<Vec<WaitingCard>, String> {
        Ok(self.0.lock().unwrap().waiting.clone())
    }
    fn failed_turn(&self, mission_id: &str) -> Result<Option<FailedTurn>, String> {
        if mission_id.starts_with("failed_") {
            Ok(Some(FailedTurn {
                mission_id: mission_id.to_owned(),
                teammate_id: Some("tm_wren".into()),
                name: Some("Wren".into()),
                asked: "Fix the save".into(),
                why: Some("The model is overloaded.".into()),
            }))
        } else {
            Ok(None)
        }
    }
    fn continues_from(&self, mission_id: &str) -> Result<Option<String>, String> {
        let mut st = self.0.lock().unwrap();
        if let Some(ref mut hook) = st.continues_hook {
            return hook(mission_id);
        }
        Ok(None)
    }
    fn team(&self) -> Result<Vec<ScoutTeammate>, String> {
        Ok(vec![
            ScoutTeammate {
                teammate_id: "tm_scout_1".into(),
                name: "Scout".into(),
                role: "Keeps watch".into(),
            },
            ScoutTeammate {
                teammate_id: "tm_wren".into(),
                name: "Wren".into(),
                role: "Code & Migrations".into(),
            },
        ])
    }
    fn ask(&self, teammate_id: &str, prompt: &str, model: Option<&str>) -> AskResult {
        let hook = self.0.lock().unwrap().ask_hook.take();
        if let Some(mut h) = hook {
            let res = h(teammate_id, prompt, model);
            let mut st = self.0.lock().unwrap();
            if st.ask_hook.is_none() {
                st.ask_hook = Some(h);
            }
            return res;
        }
        let mut st = self.0.lock().unwrap();
        st.asked.push(prompt.to_owned());
        st.models.push(model.map(str::to_owned));
        let n = st.asked.len();
        AskResult::Started {
            mission_id: format!("look_{n}"),
            run_id: format!("run_{n}"),
        }
    }
    fn stop(&self, run_id: &str) {
        self.0.lock().unwrap().stopped.push(run_id.to_owned());
    }
    fn next_free_model(&self, teammate_id: &str, tried: &[String]) -> Option<String> {
        let hook = self.0.lock().unwrap().next_free_model_hook.take();
        if let Some(mut h) = hook {
            let res = h(teammate_id, tried);
            let mut st = self.0.lock().unwrap();
            if st.next_free_model_hook.is_none() {
                st.next_free_model_hook = Some(h);
            }
            return res;
        }
        [
            "opencode/longcat-2.5-preview-free",
            "opencode/nemotron-3-ultra-free",
        ]
        .into_iter()
        .find(|id| !tried.iter().any(|t| t == id))
        .map(str::to_owned)
    }
    fn answer_of(&self, mission_id: &str) -> AnswerResult {
        self.0
            .lock()
            .unwrap()
            .answers
            .get(mission_id)
            .cloned()
            .unwrap_or_else(|| AnswerResult::Failed("It said nothing.".into()))
    }
    fn make_teammate(&self, r: &ScoutRoute) -> Result<String, String> {
        let mut st = self.0.lock().unwrap();
        if let Some(ref mut hook) = st.make_teammate_hook {
            return hook(r);
        }
        st.made += 1;
        let id = format!("tm_scout_{}", st.made);
        st.teammates.insert(id.clone());
        Ok(id)
    }
    fn teammate_exists(&self, teammate_id: &str) -> Result<bool, String> {
        Ok(self.0.lock().unwrap().teammates.contains(teammate_id))
    }
    fn changed(&self, state: &ScoutState) {
        self.0.lock().unwrap().changes.push(state.clone());
    }
    fn notify(&self, card: &ScoutCard) {
        self.0.lock().unwrap().notified.push(card.fact.said.clone());
    }
    fn answer_wait_ms(&self) -> u64 {
        self.0.lock().unwrap().answer_wait_ms
    }
    fn folder_mark(&self) -> Option<String> {
        self.0.lock().unwrap().mark.clone()
    }
    fn run_check(&self) -> Option<FolderCheckResult> {
        let hook = self.0.lock().unwrap().run_check_hook.take();
        if let Some(mut h) = hook {
            let res = h();
            let mut st = self.0.lock().unwrap();
            st.checks += 1;
            if st.run_check_hook.is_none() {
                st.run_check_hook = Some(h);
            }
            return res;
        }
        let mut st = self.0.lock().unwrap();
        st.checks += 1;
        st.check.clone()
    }
    fn has_check_support(&self) -> bool {
        self.0.lock().unwrap().has_check_support
    }
    fn folder_busy(&self) -> bool {
        self.0.lock().unwrap().folder_busy
    }
    fn person_busy(&self) -> bool {
        self.0.lock().unwrap().person_busy
    }
}

struct Harness {
    _dir: TempDir,
    st: Arc<Mutex<MockState>>,
    sched: Arc<FakeScheduler>,
    scout: Arc<Scout>,
}

impl Harness {
    fn new(tag: &str, clock: DateTime<FixedOffset>) -> Self {
        let dir = TempDir::new(tag);
        let st = Arc::new(Mutex::new(MockState {
            files: ScoutFiles::Paths {
                file: dir.0.join("scout.json"),
                log_file: Some(dir.0.join("scout-looks.json")),
            },
            clock,
            github: Some(ScoutGithub {
                prs: vec![],
                reviews: vec![],
                branch: None,
                issues: vec![],
            }),
            waiting: vec![],
            asked: vec![],
            models: vec![],
            stopped: vec![],
            answers: HashMap::new(),
            changes: vec![],
            notified: vec![],
            teammates: HashSet::new(),
            made: 0,
            person_busy: false,
            folder_busy: false,
            mark: Some("mark_1".into()),
            check: Some(red_check()),
            checks: 0,
            has_check_support: true,
            answer_wait_ms: super::ANSWER_WAIT_MS,
            ask_hook: None,
            run_check_hook: None,
            make_teammate_hook: None,
            continues_hook: None,
            next_free_model_hook: None,
        }));
        let sched = FakeScheduler::new(clock.timestamp_millis());
        let scout = Scout::with_scheduler(Arc::new(MockDeps(st.clone())), sched.clone());
        Self {
            _dir: dir,
            st,
            sched,
            scout,
        }
    }

    fn at(&self, hour: u32, minute: u32) {
        let next = local_dt(2026, 10, 22, hour, minute, 0);
        self.st.lock().unwrap().clock = next;
    }

    fn tick(&self, ms: i64) {
        {
            let mut st = self.st.lock().unwrap();
            st.clock += ChronoDuration::milliseconds(ms);
        }
        if ms > 0 {
            self.sched.advance_ms(ms as u64);
        }
    }

    fn tick_timer(&self) {
        let every_ms = self.scout.state().settings.every_minutes as u64 * 60_000;
        self.sched.advance_ms(every_ms);
    }

    fn asked(&self) -> Vec<String> {
        self.st.lock().unwrap().asked.clone()
    }
    fn models(&self) -> Vec<Option<String>> {
        self.st.lock().unwrap().models.clone()
    }
    fn stopped(&self) -> Vec<String> {
        self.st.lock().unwrap().stopped.clone()
    }
    fn notified(&self) -> Vec<String> {
        self.st.lock().unwrap().notified.clone()
    }
    fn checks(&self) -> usize {
        self.st.lock().unwrap().checks
    }
    fn made(&self) -> usize {
        self.st.lock().unwrap().made
    }
    fn set_github(&self, prs: Vec<FolderPullRequest>, reviews: Vec<ReviewAsked>) {
        self.st.lock().unwrap().github = Some(ScoutGithub {
            prs,
            reviews,
            branch: None,
            issues: vec![],
        });
    }
    fn set_person(&self, busy: bool) {
        self.st.lock().unwrap().person_busy = busy;
    }
    fn set_mark(&self, mark: &str) {
        self.st.lock().unwrap().mark = Some(mark.to_owned());
    }
    fn set_check(&self, check: Option<FolderCheckResult>) {
        self.st.lock().unwrap().check = check;
    }
    fn set_folder_busy(&self, busy: bool) {
        self.st.lock().unwrap().folder_busy = busy;
    }
    fn set_answer(&self, mission_id: &str, text: &str) {
        self.st
            .lock()
            .unwrap()
            .answers
            .insert(mission_id.to_owned(), AnswerResult::Text(text.to_owned()));
    }
    fn set_waiting(&self, waiting: Vec<WaitingCard>) {
        self.st.lock().unwrap().waiting = waiting;
    }
}

// =================================================================================================
// Part A: scout-keeps-its-goal-hours-and-word.test.ts (17 tests)
// =================================================================================================

#[test]
fn goal_is_kept_in_persons_words_trimmed_capped_at_600_and_cleared_by_empty() {
    let base = ScoutSettings::default();
    let kept = scout_settings_from(
        &json!({ "goal": "  Keep main’s tests\n passing.  " }),
        &base,
    );
    assert_eq!(kept.goal.as_deref(), Some("Keep main’s tests passing."));

    let long = "x".repeat(700);
    let capped = scout_settings_from(&json!({ "goal": long }), &kept);
    assert_eq!(capped.goal.as_ref().unwrap().len(), 600);

    let preserved = scout_settings_from(&json!({ "on": true }), &kept);
    assert_eq!(preserved.goal.as_deref(), Some("Keep main’s tests passing."));

    let cleared = scout_settings_from(&json!({ "goal": "   " }), &kept);
    assert_eq!(cleared.goal, None);
}

#[test]
fn goal_is_read_into_looks_prompt_and_only_when_there_is_one() {
    let facts = vec![ScoutFact::simple("k", ScoutUrgency::Now, "A fact.")];
    assert!(
        scout_prompt(&facts, &[], Some("Keep main green."), &[])
            .contains("The person asked you to keep watch for this: Keep main green.")
    );
    assert!(!scout_prompt(&facts, &[], None, &[]).contains("keep watch for this"));

    let h = Harness::new("goal-prompt", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.set(&ScoutChange {
        goal: Some("Keep main green.".into()),
        ..Default::default()
    });
    h.set_github(vec![pr_failing()], vec![]);
    assert!(matches!(h.scout.turn_on(Some(&route())), TurnOnOutcome::Ok(_)));
    assert!(h.asked()[0].contains("The person asked you to keep watch for this: Keep main green."));
}

#[test]
fn quiet_hours_cross_midnight_and_are_none_when_from_equals_to() {
    let quiet = ScoutQuietHours { from: 22, to: 7 };
    assert!(in_quiet_hours(Some(quiet), local_dt(2026, 10, 22, 23, 30, 0)));
    assert!(in_quiet_hours(Some(quiet), local_dt(2026, 10, 22, 6, 59, 0)));
    assert!(!in_quiet_hours(Some(quiet), local_dt(2026, 10, 22, 7, 0, 0)));
    assert!(!in_quiet_hours(
        Some(ScoutQuietHours { from: 0, to: 8 }),
        local_dt(2026, 10, 22, 12, 0, 0)
    ));

    let base = ScoutSettings::default();
    let same = scout_settings_from(&json!({ "quiet": { "from": 9, "to": 9 } }), &base);
    assert_eq!(same.quiet, None);

    let with_q = ScoutSettings {
        quiet: Some(quiet),
        ..base
    };
    let cleared = scout_settings_from(&json!({ "quiet": null }), &with_q);
    assert_eq!(cleared.quiet, None);
}

#[test]
fn quiet_hours_keep_timer_from_looking_while_look_now_looks_with_no_notify() {
    let h = Harness::new("quiet-timer", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.scout.set(&ScoutChange {
        quiet: Some(Some(ScoutQuietHours { from: 22, to: 7 })),
        ..Default::default()
    });
    h.at(23, 0);
    h.set_github(vec![pr_failing()], vec![]);
    h.tick_timer();
    assert!(h.asked().is_empty());

    h.scout.look();
    assert_eq!(h.asked().len(), 1);
    assert!(h.notified().is_empty());
}

#[test]
fn holds_due_look_while_person_works_and_looks_once_it_ends() {
    let h = Harness::new("wait-person", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.set_github(vec![pr_failing()], vec![]);
    h.set_person(true);
    h.tick_timer();
    assert!(h.asked().is_empty());
    assert!(h.sched.pending_count() >= 1);

    h.set_person(false);
    h.tick(60_000);
    assert_eq!(h.asked().len(), 1);
}

#[test]
fn looks_anyway_after_half_an_hour_so_a_long_run_never_silences_it() {
    let h = Harness::new("wait-most", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.set_github(vec![pr_failing()], vec![]);
    h.set_person(true);
    h.tick_timer();
    for _ in 0..31 {
        if !h.asked().is_empty() {
            break;
        }
        assert!(h.asked().is_empty());
        h.tick(60_000);
    }
    assert_eq!(h.asked().len(), 1);
    assert_eq!(SCOUT_WAIT_MOST_MS, 30 * 60_000);
}

#[test]
fn does_not_wait_when_wait_while_working_is_false() {
    let h = Harness::new("no-wait", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.scout.set(&ScoutChange {
        wait_while_working: Some(false),
        ..Default::default()
    });
    h.set_github(vec![pr_failing()], vec![]);
    h.set_person(true);
    h.tick_timer();
    assert_eq!(h.asked().len(), 1);
}

#[test]
fn check_facts_are_told_when_check_turns_and_only_then() {
    let green = green_check();
    let red = red_check();
    assert!(check_facts(None, &green, "a").is_empty());

    let f1 = check_facts(None, &red, "a");
    assert_eq!(f1[0].urgency, ScoutUrgency::Now);
    assert_eq!(f1[0].said, "The check npm test fails: FAIL cart.test.js");

    let f2 = check_facts(Some(true), &red, "b");
    assert_eq!(f2[0].urgency, ScoutUrgency::Now);
    assert_eq!(
        f2[0].said,
        "The check npm test fails since the folder changed: FAIL cart.test.js"
    );

    assert!(check_facts(Some(false), &red, "c").is_empty());

    let f3 = check_facts(Some(false), &green, "d");
    assert_eq!(f3[0].urgency, ScoutUrgency::Later);
    assert_eq!(f3[0].said, "The check npm test passes again.");
}

#[test]
fn folder_check_runs_only_when_chosen_only_when_moved_and_never_while_busy() {
    let h = Harness::new("check-runs", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.turn_on(Some(&route()));
    assert_eq!(h.checks(), 0);
    h.scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            github: Some(true),
            teammates: Some(true),
            check: Some(true),
            comments: None,
        }),
        ..Default::default()
    });
    h.scout.look();
    assert_eq!(h.checks(), 1);
    let c0 = &h.scout.state().cards[0];
    assert_eq!(c0.fact.said, "The check npm test fails: FAIL cart.test.js");
    assert_eq!(c0.fact.urgency, ScoutUrgency::Now);

    // Same folder: not run again.
    h.scout.look();
    assert_eq!(h.checks(), 1);

    // Moved, but a turn is working there: not yet.
    h.set_mark("mark_2");
    h.set_folder_busy(true);
    h.scout.look();
    assert_eq!(h.checks(), 1);

    // Quiet again: run, and it passes -- news.
    h.set_folder_busy(false);
    h.set_check(Some(green_check()));
    h.scout.look();
    assert_eq!(h.checks(), 2);

    let check_cards: Vec<_> = h
        .scout
        .state()
        .cards
        .into_iter()
        .filter(|c| c.fact.key.starts_with("check:"))
        .collect();
    assert_eq!(
        check_cards
            .iter()
            .map(|c| c.fact.said.as_str())
            .collect::<Vec<_>>(),
        vec!["The check npm test passes again."]
    );
    assert_eq!(check_cards[0].fact.urgency, ScoutUrgency::Later);
    assert!(!check_cards[0].reading);
}

#[test]
fn folder_check_is_one_card_in_brief_when_it_turns_on_briefs_own_look() {
    let h = Harness::new("check-brief", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            github: Some(true),
            teammates: Some(true),
            check: Some(true),
            comments: None,
        }),
        ..Default::default()
    });
    h.scout.look();
    assert_eq!(
        h.scout
            .state()
            .cards
            .iter()
            .filter(|c| c.fact.key.starts_with("check:"))
            .count(),
        1
    );
    h.scout.set(&ScoutChange {
        brief_at: Some(Some(7)),
        ..Default::default()
    });
    h.set_mark("mark_2");
    h.set_check(Some(green_check()));
    h.scout.look();
    let brief = h
        .scout
        .state()
        .cards
        .into_iter()
        .find(|c| c.fact.key.starts_with("brief:"))
        .expect("brief card");
    let lines = brief.fact.lines.expect("brief lines");
    assert!(
        lines.contains(&"Scout has 1 more card for you.".to_owned()),
        "{lines:?}"
    );
}

#[test]
fn drops_check_result_when_turn_began_beside_it_and_reruns_when_quiet() {
    let h = Harness::new("check-raced", local_dt(2026, 10, 22, 14, 0, 0));
    let busy_during_check = Arc::new(Mutex::new(true));
    let st_ref = h.st.clone();
    let flag = busy_during_check.clone();
    h.st.lock().unwrap().run_check_hook = Some(Box::new(move || {
        if *flag.lock().unwrap() {
            st_ref.lock().unwrap().folder_busy = true;
        }
        Some(red_check())
    }));
    h.scout.turn_on(Some(&route()));
    h.scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            github: Some(true),
            teammates: Some(true),
            check: Some(true),
            comments: None,
        }),
        ..Default::default()
    });
    h.scout.look();
    assert!(
        !h.scout
            .state()
            .cards
            .iter()
            .any(|c| c.fact.key.starts_with("check:"))
    );

    *busy_during_check.lock().unwrap() = false;
    h.set_folder_busy(false);
    h.scout.look();
    let saids: Vec<_> = h
        .scout
        .state()
        .cards
        .into_iter()
        .filter(|c| c.fact.key.starts_with("check:"))
        .map(|c| c.fact.said)
        .collect();
    assert_eq!(saids, vec!["The check npm test fails: FAIL cart.test.js"]);
}

#[test]
fn off_stops_a_look_already_under_way_and_cards_stay_in_locusts_words() {
    let h = Harness::new("off-stops", local_dt(2026, 10, 22, 14, 0, 0));
    h.set_github(vec![pr_failing()], vec![]);
    h.scout.turn_on(Some(&route()));
    assert!(h.scout.state().cards[0].reading);
    let state = h.scout.set(&ScoutChange {
        on: Some(false),
        ..Default::default()
    });
    assert_eq!(h.stopped(), vec!["run_1"]);
    assert!(!state.cards[0].reading);
}

#[test]
fn off_while_waiting_on_folder_check_keeps_nothing() {
    let h = Harness::new("off-during-check", local_dt(2026, 10, 22, 14, 0, 0));
    let during: Arc<Mutex<Option<Box<dyn Fn() + Send>>>> = Arc::new(Mutex::new(None));
    let during_ref = during.clone();
    h.st.lock().unwrap().run_check_hook = Some(Box::new(move || {
        if let Some(f) = during_ref.lock().unwrap().as_ref() {
            f();
        }
        Some(red_check())
    }));

    h.scout.turn_on(Some(&route()));
    h.scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            github: Some(true),
            teammates: Some(true),
            check: Some(false),
            comments: None,
        }),
        ..Default::default()
    });
    h.set_github(vec![], vec![]);
    h.scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            github: Some(true),
            teammates: Some(true),
            check: Some(true),
            comments: None,
        }),
        ..Default::default()
    });
    h.set_mark("mark_2");
    let asked_before = h.asked().len();

    let scout_ref = h.scout.clone();
    *during.lock().unwrap() = Some(Box::new(move || {
        scout_ref.set(&ScoutChange {
            on: Some(false),
            ..Default::default()
        });
    }));
    h.scout.look();
    assert_eq!(h.asked().len(), asked_before);
    assert!(h.notified().is_empty());
    assert!(
        !h.scout
            .state()
            .cards
            .iter()
            .any(|c| c.fact.key.starts_with("check:"))
    );
}

#[test]
fn quiet_hours_beginning_during_folder_check_keeps_nothing_and_notifies_nothing() {
    let h = Harness::new("quiet-during-check", local_dt(2026, 10, 22, 14, 0, 0));
    let during: Arc<Mutex<Option<Box<dyn Fn() + Send>>>> = Arc::new(Mutex::new(None));
    let during_ref = during.clone();
    h.st.lock().unwrap().run_check_hook = Some(Box::new(move || {
        if let Some(f) = during_ref.lock().unwrap().as_ref() {
            f();
        }
        Some(red_check())
    }));

    h.scout.turn_on(Some(&route()));
    h.scout.set(&ScoutChange {
        quiet: Some(Some(ScoutQuietHours { from: 22, to: 7 })),
        watches: Some(ScoutWatchesPatch {
            github: Some(true),
            teammates: Some(true),
            check: Some(true),
            comments: None,
        }),
        ..Default::default()
    });
    h.at(21, 59);
    h.set_mark("mark_9");
    let asked_before = h.asked().len();

    let st_ref = h.st.clone();
    *during.lock().unwrap() = Some(Box::new(move || {
        st_ref.lock().unwrap().clock = local_dt(2026, 10, 22, 22, 4, 0);
    }));
    h.tick_timer();
    assert_eq!(h.asked().len(), asked_before);
    assert!(h.notified().is_empty());
    assert!(
        !h.scout
            .state()
            .cards
            .iter()
            .any(|c| c.fact.key.starts_with("check:"))
    );
}

#[test]
fn stops_a_looks_turn_that_was_starting_as_scout_was_turned_off() {
    let h = Harness::new("off-during-ask", local_dt(2026, 10, 22, 14, 0, 0));
    let n = Arc::new(Mutex::new(0usize));
    let scout_ref = h.scout.clone();
    let n_ref = n.clone();
    h.st.lock().unwrap().ask_hook = Some(Box::new(move |_, _, _| {
        let mut guard = n_ref.lock().unwrap();
        *guard += 1;
        let idx = *guard;
        scout_ref.set(&ScoutChange {
            on: Some(false),
            ..Default::default()
        });
        AskResult::Started {
            mission_id: format!("look_{idx}"),
            run_id: format!("run_{idx}"),
        }
    }));
    h.set_github(vec![pr_failing()], vec![]);
    h.scout.turn_on(Some(&route()));
    assert_eq!(h.stopped(), vec!["run_1"]);
    assert!(!h.scout.state().cards.iter().any(|c| c.reading));
}

#[test]
fn counts_todays_looks_and_asked_and_resets_next_day() {
    let h = Harness::new("today-count", local_dt(2026, 10, 22, 14, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.scout.look();
    let t1 = h.scout.state().today.unwrap();
    assert_eq!((t1.looks, t1.asked), (2, 0));

    h.set_github(vec![pr_failing()], vec![]);
    h.scout.look();
    let t2 = h.scout.state().today.unwrap();
    assert_eq!((t2.looks, t2.asked), (3, 1));

    // Advance the clock to next day without triggering interval looks in between.
    h.st.lock().unwrap().clock += ChronoDuration::hours(24);
    assert_eq!(h.scout.state().today, None);
    h.scout.look();
    let t3 = h.scout.state().today.unwrap();
    assert_eq!((t3.looks, t3.asked), (1, 0));
}

#[test]
fn counts_looks_model_turn_on_the_day_look_began_across_midnight() {
    let h = Harness::new("today-midnight", local_dt(2026, 10, 22, 14, 0, 0));
    let n = Arc::new(Mutex::new(0usize));
    let past_midnight = Arc::new(Mutex::new(false));
    let st_ref = h.st.clone();
    let pm_ref = past_midnight.clone();
    let n_ref = n.clone();
    h.st.lock().unwrap().ask_hook = Some(Box::new(move |_, _, _| {
        let mut guard = n_ref.lock().unwrap();
        *guard += 1;
        let idx = *guard;
        if *pm_ref.lock().unwrap() {
            st_ref.lock().unwrap().clock += ChronoDuration::milliseconds(20_000);
        }
        AskResult::Started {
            mission_id: format!("look_{idx}"),
            run_id: format!("run_{idx}"),
        }
    }));

    h.scout.turn_on(Some(&route()));
    h.st.lock().unwrap().clock = local_dt(2026, 10, 22, 23, 59, 50);
    h.set_github(vec![pr_failing()], vec![]);
    *past_midnight.lock().unwrap() = true;
    h.scout.look();

    assert_eq!(h.scout.state().today, None);
    *past_midnight.lock().unwrap() = false;
    h.st.lock().unwrap().clock -= ChronoDuration::milliseconds(20_000);
    let oct22 = h.scout.state().today.unwrap();
    assert_eq!(oct22.day, "2026-10-22");
    assert_eq!(oct22.asked, 1);
}

// =================================================================================================
// Part B: scout-keeps-watch.test.ts (16 tests)
// =================================================================================================

#[test]
fn joins_the_team_on_given_route_once_and_looks_at_once() {
    let h = Harness::new("join-once", local_dt(2026, 10, 12, 6, 0, 0));
    h.set_github(vec![pr_failing()], vec![]);
    let out = h.scout.turn_on(Some(&route()));
    let state = match out {
        TurnOnOutcome::Ok(s) => s,
        TurnOnOutcome::Refused(e) => panic!("unexpected refusal: {e}"),
    };
    assert_eq!(h.made(), 1);
    assert!(state.settings.on);
    assert_eq!(state.settings.teammate_id.as_deref(), Some("tm_scout_1"));
    assert_eq!(h.asked().len(), 1);

    h.scout.set(&ScoutChange {
        on: Some(false),
        ..Default::default()
    });
    h.scout.turn_on(Some(&route()));
    assert_eq!(h.made(), 1);
}

#[test]
fn says_why_when_it_cannot_join_the_team() {
    let h = Harness::new("join-refused", local_dt(2026, 10, 12, 6, 0, 0));
    h.st.lock().unwrap().make_teammate_hook = Some(Box::new(|_| {
        Err("A teammate named Scout is already on the team.".into())
    }));
    let out = h.scout.turn_on(Some(&route()));
    assert_eq!(
        out,
        TurnOnOutcome::Refused("A teammate named Scout is already on the team.".into())
    );
}

#[test]
fn makes_cards_in_locusts_words_at_once_then_fills_in_from_models_answer() {
    let h = Harness::new("look-fill", local_dt(2026, 10, 12, 6, 0, 0));
    h.set_github(vec![pr_failing()], vec![]);
    let TurnOnOutcome::Ok(looked) = h.scout.turn_on(Some(&route())) else {
        panic!("turn_on should succeed");
    };
    let card = &looked.cards[0];
    assert_eq!(
        card.fact.said,
        "Your pull request #212, Talk starts from any app: 2 of 9 checks did not pass."
    );
    assert!(card.reading);
    assert_eq!(card.fact.urgency, ScoutUrgency::Now);
    assert!(h.asked()[0].contains("1. Your pull request #212"));
    assert!(h.asked()[0].contains("- Wren, Code & Migrations"));
    assert!(!h.asked()[0].contains("- Scout"));
    assert_eq!(h.scout.state().look_ids, vec!["look_1"]);

    h.set_answer(
        "look_1",
        "ITEM 1\nTAKE: Wren changed that test this morning.\nHAND TO: Wren\nDRAFT: Fix the Mac timeout on #212.",
    );
    h.scout.on_run_ended("look_1");
    let after = &h.scout.state().cards[0];
    assert_eq!(
        after.take.as_deref(),
        Some("Wren changed that test this morning.")
    );
    let draft = after.fact.draft.as_ref().unwrap();
    assert_eq!(draft.teammate_id, "tm_wren");
    assert_eq!(draft.name, "Wren");
    assert_eq!(draft.message, "Fix the Mac timeout on #212.");
    assert!(!after.reading);
}

#[test]
fn asks_no_model_when_nothing_is_new_and_tells_a_fact_once() {
    let h = Harness::new("fact-once", local_dt(2026, 10, 12, 6, 0, 0));
    h.set_github(
        vec![pr_failing()],
        vec![review(
            214,
            "Rename the call log",
            "https://github.com/acme/app/pull/214",
        )],
    );
    h.scout.turn_on(Some(&route()));
    assert_eq!(h.asked().len(), 1);
    h.set_answer("look_1", "ITEM 1\nTAKE: Read it.");
    h.scout.on_run_ended("look_1");
    h.scout.look();
    assert_eq!(h.asked().len(), 1);
    assert_eq!(h.scout.state().cards.len(), 2);

    h.set_github(
        vec![pr_passing(
            212,
            "Talk starts from any app",
            "https://github.com/acme/app/pull/212",
        )],
        vec![],
    );
    h.scout.look();
    assert_eq!(h.asked().len(), 2);
    assert!(
        h.scout.state().cards[0]
            .fact
            .said
            .contains("all 9 checks passed")
    );
}

#[test]
fn tries_next_free_models_then_leaves_cards_in_locusts_words_and_says_why() {
    let h = Harness::new("free-fallbacks", local_dt(2026, 10, 12, 6, 0, 0));
    h.set_github(
        vec![],
        vec![review(
            214,
            "Rename the call log",
            "https://github.com/acme/app/pull/214",
        )],
    );
    h.scout.turn_on(Some(&route()));
    h.scout.on_run_ended("look_1");
    h.scout.on_run_ended("look_2");
    assert!(h.scout.state().cards[0].reading);
    h.scout.on_run_ended("look_3");

    assert_eq!(
        h.models(),
        vec![
            None,
            Some("opencode/longcat-2.5-preview-free".into()),
            Some("opencode/nemotron-3-ultra-free".into()),
        ]
    );
    let state = h.scout.state();
    assert_eq!(
        state.cards[0].fact.said,
        "Someone asked you to review pull request #214, Rename the call log."
    );
    assert!(!state.cards[0].reading);
    assert_eq!(state.cards[0].take, None);
    assert_eq!(
        state.model_said.as_deref(),
        Some("It said nothing. These cards are in Locust’s words.")
    );
}

#[test]
fn non_free_model_never_moves_to_another_and_says_why_at_once() {
    let h = Harness::new("paid-no-fallback", local_dt(2026, 10, 12, 6, 0, 0));
    h.st.lock().unwrap().next_free_model_hook = Some(Box::new(|_, _| None));
    h.set_github(
        vec![],
        vec![review(214, "x", "https://github.com/acme/app/pull/214")],
    );
    h.scout.turn_on(Some(&ScoutRoute {
        runtime: "claude".into(),
        model: "haiku".into(),
        account: None,
    }));
    h.scout.on_run_ended("look_1");
    assert_eq!(h.models(), vec![None]);
    assert_eq!(
        h.scout.state().model_said.as_deref(),
        Some("It said nothing. These cards are in Locust’s words.")
    );
}

#[test]
fn provider_refused_stops_run_and_asks_next_free_model_at_once() {
    let h = Harness::new("provider-refused", local_dt(2026, 10, 12, 6, 0, 0));
    h.set_github(
        vec![],
        vec![review(214, "x", "https://github.com/acme/app/pull/214")],
    );
    h.scout.turn_on(Some(&route()));
    h.scout.provider_refused(
        "look_1",
        "The model's provider answered \"Free usage exceeded, subscribe to Go\".",
    );
    assert_eq!(h.stopped(), vec!["run_1"]);
    assert_eq!(
        h.models(),
        vec![None, Some("opencode/longcat-2.5-preview-free".into())]
    );
    h.set_answer("look_2", "ITEM 1\nTAKE: Worth a look today.");
    h.scout.on_run_ended("look_2");
    let state = h.scout.state();
    assert_eq!(state.cards[0].take.as_deref(), Some("Worth a look today."));
    assert_eq!(state.model_said, None);

    h.scout.provider_refused("look_1", "late");
    assert_eq!(h.stopped(), vec!["run_1"]);
}

#[test]
fn does_not_wait_forever_on_a_model_that_never_ends_its_turn() {
    let h = Harness::new("answer-timeout", local_dt(2026, 10, 12, 6, 0, 0));
    h.st.lock().unwrap().answer_wait_ms = 1_000;
    h.set_github(
        vec![],
        vec![review(214, "x", "https://github.com/acme/app/pull/214")],
    );
    h.scout.turn_on(Some(&route()));
    h.tick(1_000);
    assert_eq!(h.stopped(), vec!["run_1"]);
    h.tick(1_000);
    assert_eq!(h.stopped(), vec!["run_1", "run_2"]);
    h.tick(1_000);
    assert_eq!(h.stopped(), vec!["run_1", "run_2", "run_3"]);
    let state = h.scout.state();
    assert!(!state.cards[0].reading);
    assert!(state.model_said.unwrap().contains("took too long"));
}

#[test]
fn says_why_when_model_could_not_be_asked_and_keeps_cards() {
    let h = Harness::new("ask-refused", local_dt(2026, 10, 12, 6, 0, 0));
    h.st.lock().unwrap().ask_hook =
        Some(Box::new(|_, _, _| AskResult::Refused("OpenCode is not installed.".into())));
    h.set_github(
        vec![],
        vec![review(214, "x", "https://github.com/acme/app/pull/214")],
    );
    h.scout.turn_on(Some(&route()));
    let state = h.scout.state();
    assert_eq!(state.cards.len(), 1);
    assert!(!state.cards[0].reading);
    assert_eq!(state.model_said.as_deref(), Some("OpenCode is not installed."));
    assert!(state.look_ids.is_empty());
}

#[test]
fn tells_windows_only_about_now_cards_and_only_when_notify_is_on() {
    let now = DateTime::parse_from_rfc3339("2026-10-12T06:00:00.000Z").unwrap();
    let h = Harness::new("notify-now", now);
    h.set_waiting(vec![WaitingCard {
        approval_id: "ap_1".into(),
        mission_id: "mis_1".into(),
        teammate_id: Some("tm_atlas".into()),
        name: Some("Atlas".into()),
        summary: "Run pnpm test".into(),
        requested_at: "2026-10-12T05:30:00.000Z".into(),
    }]);
    h.set_github(
        vec![],
        vec![review(214, "x", "https://github.com/acme/app/pull/214")],
    );
    h.scout.turn_on(Some(&route()));
    assert_eq!(
        h.notified(),
        vec!["Atlas has waited 30 minutes for your OK."]
    );

    h.scout.set(&ScoutChange {
        notify: Some(false),
        ..Default::default()
    });
    h.set_waiting(vec![WaitingCard {
        approval_id: "ap_2".into(),
        mission_id: "mis_2".into(),
        teammate_id: None,
        name: Some("Atlas".into()),
        summary: "x".into(),
        requested_at: "2026-10-12T05:30:00.000Z".into(),
    }]);
    h.scout.on_run_ended("look_1");
    h.scout.look();
    assert_eq!(h.notified().len(), 1);
}

#[test]
fn hears_teammates_failed_turn_and_tells_on_next_look_never_its_own_looks() {
    let h = Harness::new("failed-turn", local_dt(2026, 10, 12, 6, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.scout.on_run_ended("failed_9");
    h.scout.on_run_ended("mis_ok");
    h.scout.look();
    let saids: Vec<_> = h
        .scout
        .state()
        .cards
        .into_iter()
        .map(|c| c.fact.said)
        .collect();
    assert_eq!(saids, vec!["Wren's turn failed: The model is overloaded."]);
}

#[test]
fn drops_failed_turn_whose_conversation_went_on_before_and_after_card() {
    let h = Harness::new("failed-continued", local_dt(2026, 10, 12, 6, 0, 0));
    h.st.lock().unwrap().continues_hook = Some(Box::new(|mid| {
        Ok(match mid {
            "carried_on" => Some("failed_9".into()),
            "carried_on_2" => Some("failed_10".into()),
            _ => None,
        })
    }));
    h.scout.turn_on(Some(&route()));
    h.scout.on_run_ended("failed_9");
    h.scout.on_run_ended("carried_on");
    h.scout.look();
    assert!(h.scout.state().cards.is_empty());

    h.scout.on_run_ended("failed_10");
    h.scout.look();
    let keys: Vec<_> = h
        .scout
        .state()
        .cards
        .into_iter()
        .map(|c| c.fact.key)
        .collect();
    assert_eq!(keys, vec!["failed:failed_10"]);
    h.scout.on_run_ended("carried_on_2");
    assert!(h.scout.state().cards.is_empty());
}

#[test]
fn watches_only_what_it_is_set_to_watch() {
    let now = DateTime::parse_from_rfc3339("2026-10-12T06:00:00.000Z").unwrap();
    let h = Harness::new("watches", now);
    h.scout.turn_on(Some(&route()));
    h.scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            github: Some(false),
            teammates: Some(false),
            check: Some(false),
            comments: None,
        }),
        ..Default::default()
    });
    h.set_github(vec![pr_failing()], vec![]);
    h.set_waiting(vec![WaitingCard {
        approval_id: "ap_1".into(),
        mission_id: "mis_1".into(),
        teammate_id: None,
        name: Some("Atlas".into()),
        summary: "x".into(),
        requested_at: "2026-10-12T05:00:00.000Z".into(),
    }]);
    h.scout.on_run_ended("failed_9");
    h.scout.look();
    assert!(h.scout.state().cards.is_empty());
}

#[test]
fn stops_when_scout_is_taken_off_the_team() {
    let h = Harness::new("off-team", local_dt(2026, 10, 12, 6, 0, 0));
    h.scout.turn_on(Some(&route()));
    h.st.lock().unwrap().teammates.clear();
    let state = h.scout.look();
    assert!(!state.settings.on);
}

#[test]
fn lasts_across_restart_and_reading_card_waits_on_nothing() {
    let h = Harness::new("restart", local_dt(2026, 10, 12, 6, 0, 0));
    h.set_github(
        vec![],
        vec![review(214, "x", "https://github.com/acme/app/pull/214")],
    );
    h.scout.turn_on(Some(&route()));

    let sched2 = FakeScheduler::new(local_dt(2026, 10, 12, 6, 0, 0).timestamp_millis());
    let again = Scout::with_scheduler(Arc::new(MockDeps(h.st.clone())), sched2);
    again.start(60_000);
    let state = again.state();
    assert!(state.settings.on);
    assert_eq!(state.cards.len(), 1);
    assert!(!state.cards[0].reading);

    let scout_file = h.st.lock().unwrap().files.scout_path().unwrap();
    let raw: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(scout_file).unwrap()).unwrap();
    assert_eq!(
        raw["seen"],
        serde_json::json!(["review:https://github.com/acme/app/pull/214"])
    );
    again.dispose();
}

#[test]
fn forgets_dismissed_card_and_does_not_bring_it_back() {
    let h = Harness::new("dismiss", local_dt(2026, 10, 12, 6, 0, 0));
    h.set_github(
        vec![],
        vec![review(214, "x", "https://github.com/acme/app/pull/214")],
    );
    h.scout.turn_on(Some(&route()));
    let card_id = h.scout.state().cards[0].card_id.clone();
    h.scout.dismiss(&card_id, Some(ScoutOutcome::Dismissed));
    h.scout.on_run_ended("look_1");
    h.scout.look();
    assert!(h.scout.state().cards.is_empty());
}

// =================================================================================================
// Part C: scout-runs-the-check-only-when-the-folder-moved.test.ts (2 tests)
// =================================================================================================

fn git(cwd: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=Locust test",
            "-c",
            "user.email=locust-test",
        ])
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&status.stderr)
    );
}

fn run_git(args: &[&str], cwd: &std::path::Path) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn folder_mark_moves_on_commit_edited_save_with_new_mtime_and_new_file() {
    let dir = TempDir::new("folder-mark");
    let folder = &dir.0;
    git(folder, &["init", "-q"]);
    fs::write(folder.join("app.js"), "export const total = 1\n").unwrap();
    git(folder, &["add", "-A"]);
    git(folder, &["commit", "-q", "-m", "first"]);

    let clean = folder_mark_of(folder, run_git).unwrap();
    assert_eq!(clean.len(), 32);
    assert!(clean.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(folder_mark_of(folder, run_git).as_deref(), Some(clean.as_str()));

    fs::write(folder.join("app.js"), "export const total = 2\n").unwrap();
    let edited = folder_mark_of(folder, run_git).unwrap();
    assert_ne!(edited, clean);
    assert_eq!(
        folder_mark_of(folder, run_git).as_deref(),
        Some(edited.as_str())
    );

    fs::write(folder.join("app.js"), "export const total = 3\n").unwrap();
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    fs::OpenOptions::new()
        .write(true)
        .open(folder.join("app.js"))
        .unwrap()
        .set_modified(future)
        .unwrap();
    let saved_again = folder_mark_of(folder, run_git).unwrap();
    assert_ne!(saved_again, edited);

    fs::write(folder.join("notes.md"), "new\n").unwrap();
    let added = folder_mark_of(folder, run_git).unwrap();
    assert_ne!(added, saved_again);

    git(folder, &["add", "-A"]);
    git(folder, &["commit", "-q", "-m", "second"]);
    let committed = folder_mark_of(folder, run_git).unwrap();
    assert_ne!(committed, added);
    assert_ne!(committed, clean);
}

#[test]
fn folder_mark_is_none_outside_a_git_checkout() {
    let dir = TempDir::new("loose-folder");
    fs::write(dir.0.join("a.txt"), "x").unwrap();
    assert_eq!(
        folder_mark_of(&dir.0, |_, _| Err("fatal: not a git repository".into())),
        None
    );
}
