//! 1:1 ports of `main/scout-prepares-the-fix.test.ts` (7 cases) plus Scout fix special-case coverage.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, TimeZone};

use crate::command_guard::{GuardContext, hook_reply, in_fix_copy, publishes_from_copy, stopped_because};

use super::fix::{
    FixApplyResult, FixCheckResult, FixPrepareInput, FixStartRequest, FixStarted, FixTurnEnd,
    FixTurnPhase, SCOUT_FIX_MS, SCOUT_FIX_TURNS, ScoutFixDeps, ScoutFixOutcome, ScoutFixes,
    default_fix_git, new_fix_id, scout_fix_root_from,
};
use super::github::{ScoutGithub, ScoutGithubUnread};
use super::scheduler::FakeScheduler;
use super::store::ScoutFiles;
use super::types::{
    FailedTurn, FolderCheckResult, ScoutChange, ScoutFix, ScoutFixSettingsPatch, ScoutSettings,
    ScoutUrgency, ScoutWatchesPatch, WaitingCard,
};
use super::{
    AnswerResult, AskResult, FixPrepareReq, Scout, ScoutDeps, ScoutRoute, ScoutTeammate,
};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "locust-scout-fix-test-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn project() -> (TempDir, PathBuf, PathBuf) {
    let top = TempDir::new();
    let folder = top.path().join("project");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("answer.txt"), "41\n").unwrap();
    fs::write(folder.join("notes.md"), "mine\n").unwrap();
    default_fix_git(&["init", "-q"], &folder).unwrap();
    default_fix_git(
        &["remote", "add", "origin", "https://github.com/acme/app.git"],
        &folder,
    )
    .unwrap();
    let root = top.path().join("scout-fixes");
    (top, folder, root)
}

fn git_raw(args: &[&str], cwd: &Path) -> (i32, String) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (code, combined)
}

fn check_of(_command: &str, cwd: &Path) -> FixCheckResult {
    let answer = fs::read_to_string(cwd.join("answer.txt"))
        .unwrap_or_default()
        .trim()
        .to_owned();
    if answer == "42" {
        FixCheckResult {
            passed: true,
            lines: vec![],
        }
    } else {
        FixCheckResult {
            passed: false,
            lines: vec![format!("expected 42, got {answer}")],
        }
    }
}

#[derive(Clone, Debug)]
struct RecordedStart {
    prompt: String,
    follow_up_of: Option<String>,
}

enum WorkAction {
    End(FixTurnEnd),
    Hang,
}

type WorkFn = Arc<dyn Fn(&Path, usize) -> WorkAction + Send + Sync>;

struct TestFixDeps {
    root: PathBuf,
    scheduler: Arc<FakeScheduler>,
    work: WorkFn,
    fixes_slot: Mutex<Option<Arc<ScoutFixes>>>,
    started: Mutex<Vec<RecordedStart>>,
    stopped: Mutex<Vec<String>>,
    ends: Mutex<HashMap<String, FixTurnEnd>>,
}

impl TestFixDeps {
    fn new(root: PathBuf, scheduler: Arc<FakeScheduler>, work: WorkFn) -> Arc<Self> {
        Arc::new(Self {
            root,
            scheduler,
            work,
            fixes_slot: Mutex::new(None),
            started: Mutex::new(Vec::new()),
            stopped: Mutex::new(Vec::new()),
            ends: Mutex::new(HashMap::new()),
        })
    }
}

impl ScoutFixDeps for TestFixDeps {
    fn root(&self) -> PathBuf {
        self.root.clone()
    }

    fn run_check(&self, command: &str, cwd: &Path) -> FixCheckResult {
        check_of(command, cwd)
    }

    fn start(&self, input: FixStartRequest<'_>) -> Result<FixStarted, String> {
        let turn = {
            let mut st = self.started.lock().unwrap();
            st.push(RecordedStart {
                prompt: input.prompt.to_owned(),
                follow_up_of: input.follow_up_of.map(str::to_owned),
            });
            st.len()
        };
        let mission_id = format!("mission_{turn}");
        let run_id = format!("run_{turn}");
        let action = (self.work)(input.cwd, turn);
        if let WorkAction::End(end) = action {
            self.ends.lock().unwrap().insert(mission_id.clone(), end);
            let fixes = self.fixes_slot.lock().unwrap().clone();
            if let Some(f) = fixes {
                let mid = mission_id.clone();
                std::thread::spawn(move || {
                    f.on_run_ended(&mid);
                });
            }
        }
        Ok(FixStarted { mission_id, run_id })
    }

    fn stop(&self, run_id: &str) {
        self.stopped.lock().unwrap().push(run_id.to_owned());
    }

    fn end_of(&self, mission_id: &str) -> FixTurnEnd {
        self.ends
            .lock()
            .unwrap()
            .get(mission_id)
            .cloned()
            .unwrap_or(FixTurnEnd {
                phase: FixTurnPhase::Cancelled,
                said: None,
                failed: None,
                asked: false,
            })
    }

    fn now_ms(&self) -> i64 {
        self.scheduler.now_ms()
    }
}

fn fixes_with(root: &Path, work: WorkFn) -> (Arc<TestFixDeps>, Arc<FakeScheduler>, Arc<ScoutFixes>) {
    let sched = FakeScheduler::new(1_000_000);
    let deps = TestFixDeps::new(root.to_path_buf(), sched.clone(), work);
    let fixes = ScoutFixes::with_scheduler(deps.clone(), sched.clone());
    *deps.fixes_slot.lock().unwrap() = Some(fixes.clone());
    (deps, sched, fixes)
}

#[test]
fn works_in_a_copy_with_no_remote_where_a_push_fails_and_the_guard_refuses_push_remote_and_gh() {
    let (_top, folder, root) = project();
    let seen_copy = Arc::new(Mutex::new(PathBuf::new()));
    let seen_copy_inner = seen_copy.clone();
    let (_deps, _sched, fixes) = fixes_with(
        &root,
        Arc::new(move |cwd, _| {
            *seen_copy_inner.lock().unwrap() = cwd.to_path_buf();
            fs::write(cwd.join("answer.txt"), "42\n").unwrap();
            WorkAction::End(FixTurnEnd {
                phase: FixTurnPhase::Completed,
                said: Some("Changed the answer to 42.".into()),
                failed: None,
                asked: false,
            })
        }),
    );

    let fix_id = new_fix_id();
    let outcome = fixes.prepare(&FixPrepareInput {
        fix_id: fix_id.clone(),
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        command: "npm test".into(),
        folder: folder.clone(),
    });
    assert!(matches!(
        &outcome,
        ScoutFixOutcome::Ready { files, mission_id, .. }
            if files == &vec!["answer.txt".to_owned()] && mission_id == "mission_1"
    ));

    let copy = seen_copy.lock().unwrap().clone();
    assert_eq!(copy, root.join(&fix_id));
    assert_eq!(git_raw(&["remote"], &copy).1.trim(), "");
    let (_, toplevel) = git_raw(&["rev-parse", "--show-toplevel"], &copy);
    assert_eq!(
        PathBuf::from(toplevel.trim()).canonicalize().unwrap(),
        copy.canonicalize().unwrap()
    );
    let (push_code, _) = git_raw(&["push", "origin", "HEAD"], &copy);
    assert_ne!(push_code, 0);

    let copy_s = copy.to_string_lossy().into_owned();
    let folder_s = folder.to_string_lossy().into_owned();
    assert!(in_fix_copy(&[&copy_s]));
    assert!(!in_fix_copy(&[&folder_s]));

    let copy_ctx = GuardContext {
        fix_copy: in_fix_copy(&[&copy_s]),
        started_pids: vec![],
    };
    let folder_ctx = GuardContext {
        fix_copy: in_fix_copy(&[&folder_s]),
        started_pids: vec![],
    };
    for command in [
        "git push origin main",
        "git -C . push",
        "git remote add origin https://github.com/acme/app.git",
        "gh pr create --fill",
        "npm test && gh api repos/acme/app",
        "\"C:\\Program Files\\GitHub CLI\\gh.exe\" pr create",
    ] {
        assert!(publishes_from_copy(command), "{command}");
        let refused = stopped_because(command, &copy_ctx).expect(command);
        assert!(
            refused.starts_with("Refused: this is a copy Scout made to prepare a fix"),
            "{command}: {refused}"
        );
        assert_eq!(stopped_because(command, &folder_ctx), None, "{command}");
    }
    for command in [
        "npm test",
        "git status",
        "git diff",
        "git log --grep push",
        "cat ghost.txt",
    ] {
        assert_eq!(stopped_because(command, &copy_ctx), None, "{command}");
    }
}

#[test]
fn refuses_a_push_in_the_copy_through_the_hook_itself_from_the_folder_it_runs_in() {
    let (top, _folder, root) = project();
    let copy = root.join(new_fix_id());
    fs::create_dir_all(&copy).unwrap();
    let copy_s = copy.to_string_lossy().into_owned();
    let other_s = top.path().to_string_lossy().into_owned();

    let event = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git push origin main" }
    });
    let reply_copy: serde_json::Value =
        serde_json::from_str(&hook_reply(&event.to_string(), &copy_s).unwrap()).unwrap();
    assert_eq!(reply_copy["hookSpecificOutput"]["permissionDecision"], "deny");

    let event_with_cwd = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git push origin main" },
        "cwd": copy_s,
    });
    let reply_event_cwd: serde_json::Value =
        serde_json::from_str(&hook_reply(&event_with_cwd.to_string(), &other_s).unwrap()).unwrap();
    assert_eq!(
        reply_event_cwd["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
    assert_eq!(hook_reply(&event.to_string(), &other_s), None);

    assert_eq!(
        scout_fix_root_from(Some(Path::new("/home/u")), Some("/custom/scout-fixes")),
        PathBuf::from("/custom/scout-fixes")
    );
    assert_eq!(
        scout_fix_root_from(Some(Path::new("/home/u")), Some("  ")),
        PathBuf::from("/home/u/.locust/scout-fixes")
    );
}

#[test]
fn stops_at_the_turn_limit_and_the_time_limit_and_says_why_a_turn_that_changes_nothing_says_what_it_said() {
    let (_top, folder, root) = project();
    let (stubborn_deps, _sched1, stubborn_fixes) = fixes_with(
        &root,
        Arc::new(|cwd, turn| {
            fs::write(cwd.join("answer.txt"), format!("{}\n", 40 - turn)).unwrap();
            WorkAction::End(FixTurnEnd {
                phase: FixTurnPhase::Completed,
                said: Some("Tried.".into()),
                failed: None,
                asked: false,
            })
        }),
    );
    let tries = stubborn_fixes.prepare(&FixPrepareInput {
        fix_id: new_fix_id(),
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        command: "npm test".into(),
        folder: folder.clone(),
    });
    assert_eq!(
        tries,
        ScoutFixOutcome::Unfixed {
            why: format!(
                "The check still fails after {SCOUT_FIX_TURNS} tries: expected 42, got {}",
                40 - SCOUT_FIX_TURNS
            ),
            mission_id: Some(format!("mission_{SCOUT_FIX_TURNS}")),
        }
    );
    {
        let st = stubborn_deps.started.lock().unwrap();
        assert_eq!(st.len(), SCOUT_FIX_TURNS);
        assert_eq!(st[1].follow_up_of.as_deref(), Some("mission_1"));
        assert!(st[1].prompt.contains("still fails after your change (try 2 of 2)"));
    }

    let (slow_deps, slow_sched, slow_fixes) = fixes_with(&root, Arc::new(|_, _| WorkAction::Hang));
    let folder_bg = folder.clone();
    let fixes_bg = slow_fixes.clone();
    let handle = std::thread::spawn(move || {
        fixes_bg.prepare(&FixPrepareInput {
            fix_id: new_fix_id(),
            teammate_id: "tm_wren".into(),
            name: "Wren".into(),
            command: "npm test".into(),
            folder: folder_bg,
        })
    });
    for _ in 0..200 {
        if !slow_deps.started.lock().unwrap().is_empty() && slow_sched.pending_count() > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    slow_sched.advance_ms(SCOUT_FIX_MS);
    let slow_outcome = handle.join().unwrap();
    assert_eq!(
        slow_outcome,
        ScoutFixOutcome::Unfixed {
            why: "It ran past 30 minutes, so Locust stopped it.".into(),
            mission_id: Some("mission_1".into()),
        }
    );
    assert_eq!(*slow_deps.stopped.lock().unwrap(), vec!["run_1".to_owned()]);

    let (idle_deps, _sched3, idle_fixes) = fixes_with(
        &root,
        Arc::new(|_, _| {
            WorkAction::End(FixTurnEnd {
                phase: FixTurnPhase::Completed,
                said: Some("The test needs a database I cannot reach.".into()),
                failed: None,
                asked: false,
            })
        }),
    );
    let idle_outcome = idle_fixes.prepare(&FixPrepareInput {
        fix_id: new_fix_id(),
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        command: "npm test".into(),
        folder,
    });
    assert_eq!(
        idle_outcome,
        ScoutFixOutcome::Unfixed {
            why: "Wren changed nothing, and said: “The test needs a database I cannot reach.”".into(),
            mission_id: Some("mission_1".into()),
        }
    );
    let first_prompt = &idle_deps.started.lock().unwrap()[0].prompt;
    assert!(first_prompt.contains("You are working in a copy of their folder"));
    assert!(first_prompt.contains("Do not push, add a remote, or use gh"));
    assert!(first_prompt.contains("    expected 42, got 41"));
}

#[test]
fn applies_beside_the_persons_own_edits_and_refuses_when_they_changed_the_same_file_since() {
    let (_top, folder, root) = project();
    let (_deps, _sched, fixes) = fixes_with(
        &root,
        Arc::new(|cwd, _| {
            fs::write(cwd.join("answer.txt"), "42\n").unwrap();
            WorkAction::End(FixTurnEnd {
                phase: FixTurnPhase::Completed,
                said: Some("Fixed.".into()),
                failed: None,
                asked: false,
            })
        }),
    );

    let first = new_fix_id();
    let ready1 = fixes.prepare(&FixPrepareInput {
        fix_id: first.clone(),
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        command: "npm test".into(),
        folder: folder.clone(),
    });
    assert!(matches!(ready1, ScoutFixOutcome::Ready { .. }));

    // Person keeps working on another file meanwhile.
    fs::write(folder.join("notes.md"), "mine, changed while the fix was made\n").unwrap();
    assert_eq!(
        fixes.apply(&first),
        FixApplyResult::Brought(vec!["answer.txt".into()])
    );
    assert_eq!(fs::read_to_string(folder.join("answer.txt")).unwrap(), "42\n");
    assert_eq!(
        fs::read_to_string(folder.join("notes.md")).unwrap(),
        "mine, changed while the fix was made\n"
    );
    // Special case (`scout-fix.ts:205`): copy discarded right after bringing in cleanly.
    assert_eq!(fixes.apply(&first), FixApplyResult::Gone);

    fs::write(folder.join("answer.txt"), "41\n").unwrap();
    let second = new_fix_id();
    let ready2 = fixes.prepare(&FixPrepareInput {
        fix_id: second.clone(),
        teammate_id: "tm_wren".into(),
        name: "Wren".into(),
        command: "npm test".into(),
        folder: folder.clone(),
    });
    assert!(matches!(ready2, ScoutFixOutcome::Ready { .. }));
    fs::write(folder.join("answer.txt"), "40\n").unwrap();
    assert_eq!(
        fixes.apply(&second),
        FixApplyResult::YourChanges(vec!["answer.txt".into()])
    );
    assert_eq!(fs::read_to_string(folder.join("answer.txt")).unwrap(), "40\n");
}

struct PendingFixOutcome {
    prepared: Mutex<Vec<String>>,
    stops: Mutex<Vec<String>>,
    discarded: Mutex<Vec<String>>,
    apply_result: Mutex<FixApplyResult>,
    outcome: Mutex<Option<ScoutFixOutcome>>,
    cv: Condvar,
}

impl PendingFixOutcome {
    fn new() -> Self {
        Self {
            prepared: Mutex::new(Vec::new()),
            stops: Mutex::new(Vec::new()),
            discarded: Mutex::new(Vec::new()),
            apply_result: Mutex::new(FixApplyResult::Brought(vec!["answer.txt".into()])),
            outcome: Mutex::new(None),
            cv: Condvar::new(),
        }
    }

    fn finish(&self, outcome: ScoutFixOutcome) {
        *self.outcome.lock().unwrap() = Some(outcome);
        self.cv.notify_all();
    }

    fn reset(&self) {
        *self.outcome.lock().unwrap() = None;
    }
}

struct CardHarnessDeps {
    file: PathBuf,
    now: DateTime<FixedOffset>,
    check_passes: Mutex<bool>,
    mark: Mutex<u32>,
    teammates: Mutex<HashSet<String>>,
    fix_ctrl: Arc<PendingFixOutcome>,
}

impl ScoutDeps for CardHarnessDeps {
    fn files(&self) -> ScoutFiles {
        ScoutFiles::Paths {
            file: self.file.clone(),
            log_file: None,
        }
    }
    fn now(&self) -> DateTime<FixedOffset> {
        self.now
    }
    fn github(&self) -> Result<Option<ScoutGithub>, ScoutGithubUnread> {
        Ok(Some(ScoutGithub {
            prs: vec![],
            reviews: vec![],
            issues: vec![],
            branch: None,
        }))
    }
    fn waiting(&self) -> Result<Vec<WaitingCard>, String> {
        Ok(vec![])
    }
    fn failed_turn(&self, _: &str) -> Result<Option<FailedTurn>, String> {
        Ok(None)
    }
    fn team(&self) -> Result<Vec<ScoutTeammate>, String> {
        Ok(vec![ScoutTeammate {
            teammate_id: "tm_wren".into(),
            name: "Wren".into(),
            role: "Code & Migrations".into(),
        }])
    }
    fn ask(&self, _: &str, _: &str, _: Option<&str>) -> AskResult {
        AskResult::Refused("No model in this test.".into())
    }
    fn stop(&self, _: &str) {}
    fn next_free_model(&self, _: &str, _: &[String]) -> Option<String> {
        None
    }
    fn answer_of(&self, _: &str) -> AnswerResult {
        AnswerResult::Failed("It said nothing.".into())
    }
    fn make_teammate(&self, _: &ScoutRoute) -> Result<String, String> {
        self.teammates.lock().unwrap().insert("tm_scout_1".into());
        Ok("tm_scout_1".into())
    }
    fn teammate_exists(&self, id: &str) -> Result<bool, String> {
        Ok(self.teammates.lock().unwrap().contains(id))
    }
    fn folder_mark(&self) -> Option<String> {
        Some(format!("mark-{}", *self.mark.lock().unwrap()))
    }
    fn run_check(&self) -> Option<FolderCheckResult> {
        let passed = *self.check_passes.lock().unwrap();
        Some(FolderCheckResult {
            command: "npm test".into(),
            passed,
            lines: if passed {
                vec![]
            } else {
                vec!["1 test failed".into()]
            },
        })
    }
    fn has_check_support(&self) -> bool {
        true
    }
    fn has_fixes(&self) -> bool {
        true
    }
    fn fix_prepare(&self, req: &FixPrepareReq) -> ScoutFixOutcome {
        self.fix_ctrl.prepared.lock().unwrap().push(req.fix_id.clone());
        self.fix_ctrl.cv.notify_all();
        let mut guard = self.fix_ctrl.outcome.lock().unwrap();
        while guard.is_none() {
            guard = self.fix_ctrl.cv.wait(guard).unwrap();
        }
        guard.take().unwrap()
    }
    fn fix_apply(&self, _fix_id: &str) -> FixApplyResult {
        self.fix_ctrl.apply_result.lock().unwrap().clone()
    }
    fn fix_discard(&self, fix_id: &str) {
        self.fix_ctrl.discarded.lock().unwrap().push(fix_id.to_owned());
    }
    fn fix_stop(&self, why: &str) {
        self.fix_ctrl.stops.lock().unwrap().push(why.to_owned());
        self.fix_ctrl.finish(ScoutFixOutcome::Unfixed {
            why: why.to_owned(),
            mission_id: None,
        });
    }
}

fn scout_harness() -> (TempDir, Arc<CardHarnessDeps>, Arc<Scout>) {
    let top = TempDir::new();
    let offset = FixedOffset::east_opt(0).unwrap();
    let deps = Arc::new(CardHarnessDeps {
        file: top.path().join("scout.json"),
        now: offset.with_ymd_and_hms(2026, 10, 23, 11, 0, 0).unwrap(),
        check_passes: Mutex::new(false),
        mark: Mutex::new(1),
        teammates: Mutex::new(HashSet::new()),
        fix_ctrl: Arc::new(PendingFixOutcome::new()),
    });
    let sched = FakeScheduler::new(1_000_000);
    let scout = Scout::with_scheduler(deps.clone(), sched);
    (top, deps, scout)
}

fn check_card(scout: &Scout) -> Option<super::types::ScoutCard> {
    scout
        .state()
        .cards
        .into_iter()
        .find(|card| card.fact.key.ends_with(":fails"))
}

fn wait_until(pred: impl Fn() -> bool) {
    for _ in 0..200 {
        if pred() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("condition not reached in time");
}

fn route() -> ScoutRoute {
    ScoutRoute {
        runtime: "opencode".into(),
        model: "opencode/deepseek-v4.1-flash-free".into(),
        account: None,
    }
}

#[test]
fn prepares_nothing_until_fixes_are_on_then_says_who_is_preparing_it_then_that_it_is_ready_and_apply_brings_it_in() {
    let (_top, deps, scout) = scout_harness();
    scout.turn_on(Some(&route()));
    scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            check: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    });
    scout.look();
    assert_eq!(check_card(&scout).and_then(|c| c.fix), None);
    assert!(deps.fix_ctrl.prepared.lock().unwrap().is_empty());

    scout.set(&ScoutChange {
        fixes: Some(ScoutFixSettingsPatch {
            on: Some(true),
            teammate_id: Some(Some("tm_wren".into())),
            per_day: Some(2),
        }),
        ..Default::default()
    });
    *deps.check_passes.lock().unwrap() = true;
    *deps.mark.lock().unwrap() += 1;
    scout.look();
    *deps.check_passes.lock().unwrap() = false;
    *deps.mark.lock().unwrap() += 1;
    scout.look();

    wait_until(|| !deps.fix_ctrl.prepared.lock().unwrap().is_empty());
    let card = check_card(&scout).unwrap();
    assert!(matches!(
        &card.fix,
        Some(ScoutFix::Preparing { name, .. }) if name == "Wren"
    ));
    assert_eq!(scout.state().today.and_then(|t| t.fixes), Some(1));

    deps.fix_ctrl.finish(ScoutFixOutcome::Ready {
        files: vec!["answer.txt".into(), "test/answer.test.ts".into()],
        added: Some(2),
        removed: Some(1),
        mission_id: "mission_fix".into(),
    });
    wait_until(|| matches!(check_card(&scout).and_then(|c| c.fix), Some(ScoutFix::Ready { .. })));
    let ready_card = check_card(&scout).unwrap();
    assert!(matches!(
        ready_card.fix,
        Some(ScoutFix::Ready {
            ref files,
            ref mission_id,
            added: Some(2),
            removed: Some(1),
            ..
        }) if files == &vec!["answer.txt".to_owned(), "test/answer.test.ts".to_owned()]
            && mission_id == "mission_fix"
    ));

    scout.apply_fix(&card.card_id);
    let applied = check_card(&scout).unwrap();
    assert_eq!(
        applied.fix,
        Some(ScoutFix::Applied {
            files: vec!["answer.txt".into()]
        })
    );
    assert_eq!(applied.fact.urgency, ScoutUrgency::Later);

    // Special cases: YourChanges -> Stale; Nothing / Gone -> Unfixed; SkipFix clears; FixAgain restarts.
    *deps.check_passes.lock().unwrap() = true;
    *deps.mark.lock().unwrap() += 1;
    scout.look();
    *deps.check_passes.lock().unwrap() = false;
    *deps.mark.lock().unwrap() += 1;
    deps.fix_ctrl.reset();
    scout.look();
    wait_until(|| deps.fix_ctrl.prepared.lock().unwrap().len() == 2);
    deps.fix_ctrl.finish(ScoutFixOutcome::Ready {
        files: vec!["answer.txt".into()],
        added: None,
        removed: None,
        mission_id: "mission_fix_2".into(),
    });
    wait_until(|| matches!(check_card(&scout).and_then(|c| c.fix), Some(ScoutFix::Ready { .. })));

    let card2 = check_card(&scout).unwrap();
    *deps.fix_ctrl.apply_result.lock().unwrap() =
        FixApplyResult::YourChanges(vec!["answer.txt".into()]);
    scout.apply_fix(&card2.card_id);
    assert!(matches!(
        check_card(&scout).and_then(|c| c.fix),
        Some(ScoutFix::Stale { ref files, .. }) if files == &vec!["answer.txt".to_owned()]
    ));

    // Raise per-day cap so preparing again starts a third fix turn, then finish Unfixed -> discard.
    scout.set(&ScoutChange {
        fixes: Some(ScoutFixSettingsPatch {
            per_day: Some(3),
            ..Default::default()
        }),
        ..Default::default()
    });
    deps.fix_ctrl.reset();
    scout.fix_again(&card2.card_id);
    wait_until(|| deps.fix_ctrl.prepared.lock().unwrap().len() == 3);
    deps.fix_ctrl.finish(ScoutFixOutcome::Unfixed {
        why: "Could not fix.".into(),
        mission_id: Some("mission_fix_3".into()),
    });
    wait_until(|| matches!(check_card(&scout).and_then(|c| c.fix), Some(ScoutFix::Unfixed { .. })));
    assert!(!deps.fix_ctrl.discarded.lock().unwrap().is_empty());

    // SkipFix clears fix back to None while keeping card.
    scout.skip_fix(&card2.card_id);
    assert_eq!(check_card(&scout).unwrap().fix, None);
}

#[test]
fn stops_at_the_days_limit_and_says_why_turned_off_the_fix_under_way_stops() {
    let (_top, deps, scout) = scout_harness();
    scout.turn_on(Some(&route()));
    scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            check: Some(true),
            ..Default::default()
        }),
        fixes: Some(ScoutFixSettingsPatch {
            on: Some(true),
            teammate_id: Some(Some("tm_wren".into())),
            per_day: Some(1),
        }),
        ..Default::default()
    });
    scout.look();
    wait_until(|| deps.fix_ctrl.prepared.lock().unwrap().len() == 1);
    deps.fix_ctrl.finish(ScoutFixOutcome::Ready {
        files: vec!["answer.txt".into()],
        added: None,
        removed: None,
        mission_id: "mission_fix".into(),
    });
    wait_until(|| matches!(check_card(&scout).and_then(|c| c.fix), Some(ScoutFix::Ready { .. })));

    *deps.check_passes.lock().unwrap() = true;
    *deps.mark.lock().unwrap() += 1;
    scout.look();
    *deps.check_passes.lock().unwrap() = false;
    *deps.mark.lock().unwrap() += 1;
    scout.look();

    assert_eq!(deps.fix_ctrl.prepared.lock().unwrap().len(), 1);
    assert_eq!(
        check_card(&scout).and_then(|c| c.fix),
        Some(ScoutFix::Held {
            why: "Today’s fix is used, so Scout did not prepare one. It can again tomorrow, or raise the limit in Settings > General, under Scout.".into()
        })
    );

    let (_top2, off_deps, off_scout) = scout_harness();
    off_scout.turn_on(Some(&route()));
    off_scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            check: Some(true),
            ..Default::default()
        }),
        fixes: Some(ScoutFixSettingsPatch {
            on: Some(true),
            teammate_id: Some(Some("tm_wren".into())),
            per_day: Some(2),
        }),
        ..Default::default()
    });
    off_scout.look();
    wait_until(|| !off_deps.fix_ctrl.prepared.lock().unwrap().is_empty());
    off_scout.set(&ScoutChange {
        on: Some(false),
        ..Default::default()
    });
    assert_eq!(
        *off_deps.fix_ctrl.stops.lock().unwrap(),
        vec!["Scout was turned off, so the fix stopped.".to_owned()]
    );
}

#[test]
fn says_when_nobody_is_picked_to_prepare_it() {
    let (_top, _deps, scout) = scout_harness();
    scout.turn_on(Some(&route()));
    scout.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            check: Some(true),
            ..Default::default()
        }),
        fixes: Some(ScoutFixSettingsPatch {
            on: Some(true),
            per_day: Some(2),
            ..Default::default()
        }),
        ..Default::default()
    });
    scout.look();
    assert_eq!(
        check_card(&scout).and_then(|c| c.fix),
        Some(ScoutFix::Held {
            why: "No teammate is picked to prepare fixes, so Scout did not prepare one. Pick one in Settings > General, under Scout.".into()
        })
    );

    // Turning fixes.on off while a fix is under way also stops it cleanly.
    let (_top2, deps2, scout2) = scout_harness();
    scout2.turn_on(Some(&route()));
    scout2.set(&ScoutChange {
        watches: Some(ScoutWatchesPatch {
            check: Some(true),
            ..Default::default()
        }),
        fixes: Some(ScoutFixSettingsPatch {
            on: Some(true),
            teammate_id: Some(Some("tm_wren".into())),
            per_day: Some(2),
        }),
        ..Default::default()
    });
    scout2.look();
    wait_until(|| !deps2.fix_ctrl.prepared.lock().unwrap().is_empty());
    scout2.set(&ScoutChange {
        fixes: Some(ScoutFixSettingsPatch {
            on: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert_eq!(deps2.fix_ctrl.stops.lock().unwrap().len(), 1);
    scout2.dispose();
}

#[test]
fn recovers_preparing_fix_cards_when_scout_starts_after_restart() {
    let (_top, deps, scout) = scout_harness();
    // Write a pre-existing scout.json with a leftover `preparing` fix.
    let state_json = serde_json::json!({
        "settings": ScoutSettings {
            on: true,
            teammate_id: Some("tm_scout_1".into()),
            ..Default::default()
        }.to_json(),
        "cards": [{
            "cardId": "card_prev",
            "key": "check:npm:fails",
            "urgency": "now",
            "said": "npm test failed",
            "at": "2026-10-23T10:50:00.000Z",
            "fix": {
                "state": "preparing",
                "fixId": "rt_scoutfix0123456789ab",
                "name": "Wren",
                "since": "2026-10-23T10:50:00.000Z"
            }
        }]
    });
    fs::write(&deps.file, serde_json::to_string_pretty(&state_json).unwrap()).unwrap();
    scout.start(30_000);
    assert_eq!(
        *deps.fix_ctrl.discarded.lock().unwrap(),
        vec!["rt_scoutfix0123456789ab".to_owned()]
    );
    let card = check_card(&scout).unwrap();
    assert_eq!(
        card.fix,
        Some(ScoutFix::Unfixed {
            name: "Wren".into(),
            why: "Locust closed while it was being prepared.".into(),
            mission_id: None,
        })
    );
    scout.dispose();
}
