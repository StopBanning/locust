//! Sequential benchmark rerun runner (`main/compare-reruns.ts`).
//!
//! A scored comparison can be run again once or three times more: each run is a new
//! comparison of the same models (and column instructions) on the same question, from
//! the same commit when it changes files, scored by the same judge(s) on the same points.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crate::compare::{
    Compare, CompareRoute, CompareSlotId, CompareStore, CreateCompareInput, RERUN_TIMES,
    RerunProgress, RerunStep, newest_judge_mission_ids, rerun_refusal_of,
};

pub trait CompareRerunsHost: Send + Sync + 'static {
    fn begin(&self, compare: &Compare);
    fn judge(
        &self,
        compare: &Compare,
        route: &CompareRoute,
        criteria: Option<&str>,
        panel: &[CompareRoute],
    ) -> Option<String>;
    fn live(&self, mission_id: &str) -> bool;
    fn phase_of(&self, mission_id: &str) -> Option<String>;
    fn retry(&self, compare: &Compare, slot: CompareSlotId) -> Option<String>;
    fn stop_mission(&self, mission_id: &str);
    fn tidy(&self, compare: &Compare);
    fn tell(&self, compares: Vec<Compare>, progress: Vec<RerunProgress>);
    fn poll_ms(&self) -> u64 {
        3_000
    }
    fn run_timeout_ms(&self) -> u64 {
        45 * 60_000
    }
}

struct Batch {
    progress: RerunProgress,
    stopped: bool,
    wait_for_checks: bool,
    checked: bool,
}

pub struct CompareReruns {
    store: CompareStore,
    host: Arc<dyn CompareRerunsHost>,
    batches: Arc<Mutex<HashMap<String, Batch>>>,
}

impl CompareReruns {
    pub fn new(store: CompareStore, host: Arc<dyn CompareRerunsHost>) -> Self {
        Self {
            store,
            host,
            batches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn progress(&self) -> Vec<RerunProgress> {
        let guard = self.batches.lock().unwrap_or_else(|e| e.into_inner());
        guard.values().map(|b| b.progress.clone()).collect()
    }

    fn tell(&self, compares: Vec<Compare>) {
        let prog = self.progress();
        self.host.tell(compares, prog);
    }

    pub fn start(
        &self,
        origin_id: &str,
        times: usize,
        wait_for_checks: bool,
    ) -> Result<Compare, String> {
        if !RERUN_TIMES.contains(&times) {
            return Err("Run it again once, or three times.".into());
        }
        let origin = self
            .store
            .get(origin_id)
            .ok()
            .flatten()
            .ok_or_else(|| "That comparison is no longer here.".to_string())?;
        if let Some(why) = rerun_refusal_of(&origin) {
            return Err(why.to_owned());
        }
        {
            let mut guard = self.batches.lock().unwrap_or_else(|e| e.into_inner());
            if guard.contains_key(&origin.compare_id) {
                return Err(
                    "It is running again already. Stop it, or wait for it to finish.".into(),
                );
            }
            guard.insert(
                origin.compare_id.clone(),
                Batch {
                    progress: RerunProgress {
                        origin_id: origin.compare_id.clone(),
                        run: 1,
                        of: times,
                        step: RerunStep::Answering,
                        compare_id: None,
                    },
                    stopped: false,
                    wait_for_checks,
                    checked: false,
                },
            );
        }
        self.tell(Vec::new());

        let store = self.store.clone();
        let host = Arc::clone(&self.host);
        let batches = Arc::clone(&self.batches);
        let origin_clone = origin.clone();
        thread::spawn(move || {
            for run in 1..=times {
                let stopped = batches
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&origin_clone.compare_id)
                    .is_none_or(|b| b.stopped);
                if stopped {
                    break;
                }
                run_once(&store, &*host, &batches, &origin_clone, run, times);
            }
            batches
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&origin_clone.compare_id);
            host.tell(Vec::new(), {
                let guard = batches.lock().unwrap_or_else(|e| e.into_inner());
                guard.values().map(|b| b.progress.clone()).collect()
            });
        });

        Ok(origin)
    }

    pub fn checked(&self, origin_id: &str, compare_id: &str) -> bool {
        let mut guard = self.batches.lock().unwrap_or_else(|e| e.into_inner());
        let Some(batch) = guard.get_mut(origin_id) else {
            return false;
        };
        if batch.stopped
            || batch.progress.step != RerunStep::Checking
            || batch.progress.compare_id.as_deref() != Some(compare_id)
            || batch.checked
        {
            return false;
        }
        batch.checked = true;
        true
    }

    pub fn stop(&self, origin_id: &str) {
        let mut guard = self.batches.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(batch) = guard.get_mut(origin_id) {
            batch.stopped = true;
        }
    }

    pub fn settle_leftover(&self) {
        let running: std::collections::HashSet<String> = {
            let guard = self.batches.lock().unwrap_or_else(|e| e.into_inner());
            guard
                .values()
                .filter_map(|b| b.progress.compare_id.clone())
                .collect()
        };
        let Ok(all) = self.store.list() else { return };
        for compare in all {
            if compare.rerun_of.is_none()
                || compare.settled
                || running.contains(&compare.compare_id)
            {
                continue;
            }
            let mut ids: Vec<String> = compare
                .slots
                .iter()
                .flat_map(|c| c.mission_ids.iter().cloned())
                .collect();
            if let Some(j) = &compare.judge {
                ids.extend(j.mission_ids.iter().cloned());
                ids.extend(j.panel_mission_ids.iter().cloned());
            }
            if ids.iter().any(|id| self.host.live(id)) {
                continue;
            }
            self.host.tidy(&compare);
            if let Ok(over) = self.store.settle(&compare.compare_id) {
                self.tell(vec![over]);
            }
        }
    }
}

fn current_progress(batches: &Arc<Mutex<HashMap<String, Batch>>>) -> Vec<RerunProgress> {
    batches
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .map(|b| b.progress.clone())
        .collect()
}

fn is_stopped(batches: &Arc<Mutex<HashMap<String, Batch>>>, origin_id: &str) -> bool {
    batches
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(origin_id)
        .is_none_or(|b| b.stopped)
}

fn settled(
    host: &dyn CompareRerunsHost,
    batches: &Arc<Mutex<HashMap<String, Batch>>>,
    origin_id: &str,
    ids: impl Fn() -> Vec<String>,
    deadline: Instant,
) -> bool {
    let poll = Duration::from_millis(host.poll_ms().max(1));
    loop {
        let running: Vec<String> = ids().into_iter().filter(|id| host.live(id)).collect();
        if running.is_empty() {
            return true;
        }
        if is_stopped(batches, origin_id) || Instant::now() > deadline {
            for id in running {
                host.stop_mission(&id);
            }
            return false;
        }
        thread::sleep(poll);
    }
}

fn run_once(
    store: &CompareStore,
    host: &dyn CompareRerunsHost,
    batches: &Arc<Mutex<HashMap<String, Batch>>>,
    origin: &Compare,
    run: usize,
    times: usize,
) {
    let Ok(rerun) = store.create(CreateCompareInput {
        teammate_id: origin.teammate_id.clone(),
        prompt: origin.prompt.clone(),
        routes: origin.slots.iter().map(|c| c.route.clone()).collect(),
        changes: origin.changes,
        changes_in: origin.changes_in,
        blind: false,
        base: origin.base.clone(),
        rerun_of: Some(origin.compare_id.clone()),
    }) else {
        return;
    };
    let gathered = store
        .add_rerun(&origin.compare_id, &rerun.compare_id)
        .unwrap_or_else(|_| origin.clone());
    let wait_for_checks = {
        let mut guard = batches.lock().unwrap_or_else(|e| e.into_inner());
        let Some(batch) = guard.get_mut(&origin.compare_id) else {
            return;
        };
        batch.progress = RerunProgress {
            origin_id: origin.compare_id.clone(),
            run,
            of: times,
            step: RerunStep::Answering,
            compare_id: Some(rerun.compare_id.clone()),
        };
        batch.wait_for_checks
    };
    host.tell(vec![gathered, rerun.clone()], current_progress(batches));

    let deadline = Instant::now() + Duration::from_millis(host.run_timeout_ms());
    let current = || {
        store
            .get(&rerun.compare_id)
            .ok()
            .flatten()
            .unwrap_or_else(|| rerun.clone())
    };

    host.begin(&rerun);
    host.tell(vec![current()], current_progress(batches));

    let newest = || {
        current()
            .slots
            .iter()
            .filter_map(|c| c.mission_ids.last().cloned())
            .collect::<Vec<_>>()
    };
    let mut answered = settled(host, batches, &origin.compare_id, newest, deadline);

    if answered && !is_stopped(batches, &origin.compare_id) && !wait_for_checks {
        let cur = current();
        let failed: Vec<CompareSlotId> = cur
            .slots
            .iter()
            .filter_map(|c| {
                let id = c.mission_ids.last()?;
                (host.phase_of(id).as_deref() == Some("failed")).then_some(c.slot)
            })
            .collect();
        for &slot in &failed {
            let cur_now = current();
            let _ = host.retry(&cur_now, slot);
        }
        if !failed.is_empty() {
            answered = settled(host, batches, &origin.compare_id, newest, deadline);
        }
    }

    if answered && !is_stopped(batches, &origin.compare_id) && wait_for_checks {
        {
            let mut guard = batches.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(batch) = guard.get_mut(&origin.compare_id) {
                batch.checked = false;
                batch.progress.step = RerunStep::Checking;
            }
        }
        host.tell(vec![current()], current_progress(batches));
        let poll = Duration::from_millis(host.poll_ms().max(1));
        loop {
            let (is_checked, stopped) = {
                let guard = batches.lock().unwrap_or_else(|e| e.into_inner());
                guard
                    .get(&origin.compare_id)
                    .map_or((false, true), |b| (b.checked, b.stopped))
            };
            if is_checked || stopped || Instant::now() > deadline {
                if !is_checked || stopped || Instant::now() > deadline {
                    answered = false;
                }
                break;
            }
            thread::sleep(poll);
        }
    }

    if answered
        && !is_stopped(batches, &origin.compare_id)
        && let Some(judge) = &origin.judge
    {
        {
            let mut guard = batches.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(batch) = guard.get_mut(&origin.compare_id) {
                batch.progress.step = RerunStep::Judging;
            }
        }
        host.tell(vec![current()], current_progress(batches));
        let panel_routes: Vec<CompareRoute> =
            judge.panel.iter().map(|m| m.route.clone()).collect();
        let cur_now = current();
        let why = host.judge(
            &cur_now,
            &judge.route,
            judge.criteria.as_deref(),
            &panel_routes,
        );
        if why.is_none() {
            host.tell(vec![current()], current_progress(batches));
            let judge_ids = || newest_judge_mission_ids(current().judge.as_ref());
            settled(host, batches, &origin.compare_id, judge_ids, deadline);
        }
    }

    let over = current();
    host.tidy(&over);
    let settled_cmp = store.settle(&rerun.compare_id).unwrap_or(over);
    host.tell(vec![settled_cmp], current_progress(batches));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::JudgePanelMember;
    use std::collections::HashSet;

    fn routes() -> Vec<CompareRoute> {
        vec![
            CompareRoute {
                runtime: "opencode".into(),
                model: "opencode/space-bunny-free".into(),
                label: Some("Space Bunny".into()),
                ..CompareRoute::new("opencode", "opencode/space-bunny-free")
            },
            CompareRoute {
                runtime: "opencode".into(),
                model: "opencode/ling-3.1-flash-free".into(),
                label: Some("Ling 3.1".into()),
                ..CompareRoute::new("opencode", "opencode/ling-3.1-flash-free")
            },
        ]
    }

    fn judge_route() -> CompareRoute {
        CompareRoute {
            runtime: "opencode".into(),
            model: "opencode/muse-spark-1.3-contributor-free".into(),
            label: Some("Muse Spark".into()),
            ..CompareRoute::new("opencode", "opencode/muse-spark-1.3-contributor-free")
        }
    }

    struct FakeHost {
        store: CompareStore,
        calls: Mutex<Vec<String>>,
        live: Mutex<HashSet<String>>,
        looks: Mutex<HashMap<String, usize>>,
        told: Mutex<Vec<(Vec<Compare>, Vec<RerunProgress>)>>,
        missions: Mutex<usize>,
        refuse_judge: Option<String>,
        never_settles: bool,
        fail_first: bool,
        slow_panel: bool,
    }

    impl FakeHost {
        fn next_id(&self) -> usize {
            let mut m = self.missions.lock().unwrap();
            *m += 1;
            *m
        }
    }

    impl CompareRerunsHost for FakeHost {
        fn begin(&self, compare: &Compare) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("begin:{}", compare.compare_id));
            for col in &compare.slots {
                let id = format!("mission_{}", self.next_id());
                self.store.add_turn(&compare.compare_id, col.slot, &id).unwrap();
                if self.never_settles {
                    self.live.lock().unwrap().insert(id);
                }
            }
        }

        fn judge(
            &self,
            compare: &Compare,
            route: &CompareRoute,
            criteria: Option<&str>,
            panel: &[CompareRoute],
        ) -> Option<String> {
            self.calls.lock().unwrap().push(format!(
                "judge:{}:{}:{}",
                compare.compare_id,
                route.model,
                criteria.unwrap_or("")
            ));
            if !panel.is_empty() {
                let models: Vec<&str> = panel.iter().map(|p| p.model.as_str()).collect();
                self.calls
                    .lock()
                    .unwrap()
                    .push(format!("panel:{}:{}", compare.compare_id, models.join(",")));
            }
            if let Some(why) = &self.refuse_judge {
                return Some(why.clone());
            }
            let members: Vec<JudgePanelMember> = panel
                .iter()
                .map(|r| {
                    let mid = format!("mission_panel_{}", self.next_id());
                    if self.slow_panel {
                        self.live.lock().unwrap().insert(mid.clone());
                    }
                    JudgePanelMember {
                        route: r.clone(),
                        mission_id: mid,
                    }
                })
                .collect();
            let jid = format!("mission_judge_{}", self.next_id());
            self.store
                .judge(
                    &compare.compare_id,
                    route.clone(),
                    &jid,
                    criteria,
                    members,
                )
                .unwrap();
            None
        }

        fn live(&self, mission_id: &str) -> bool {
            if self.slow_panel && mission_id.starts_with("mission_panel_") {
                let mut looks = self.looks.lock().unwrap();
                let count = looks.entry(mission_id.to_owned()).or_insert(0);
                *count += 1;
                if *count == 1 {
                    self.calls.lock().unwrap().push(format!("waited:{mission_id}"));
                }
                if *count > 2 {
                    self.live.lock().unwrap().remove(mission_id);
                }
            }
            self.live.lock().unwrap().contains(mission_id)
        }

        fn phase_of(&self, mission_id: &str) -> Option<String> {
            if self.fail_first && mission_id == "mission_1" {
                Some("failed".into())
            } else {
                Some("completed".into())
            }
        }

        fn retry(&self, compare: &Compare, slot: CompareSlotId) -> Option<String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("retry:{}:{}", compare.compare_id, slot.as_str()));
            let mid = format!("mission_retried_{}", self.next_id());
            self.store.retry(&compare.compare_id, slot, &mid).unwrap();
            None
        }

        fn stop_mission(&self, mission_id: &str) {
            self.calls.lock().unwrap().push(format!("stop:{mission_id}"));
            self.live.lock().unwrap().remove(mission_id);
        }

        fn tidy(&self, compare: &Compare) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("tidy:{}", compare.compare_id));
        }

        fn tell(&self, compares: Vec<Compare>, progress: Vec<RerunProgress>) {
            self.told.lock().unwrap().push((compares, progress));
        }

        fn poll_ms(&self) -> u64 {
            1
        }
    }

    struct TempDir(std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct Setup {
        _dir: TempDir,
        store: CompareStore,
        origin: Compare,
        host: Arc<FakeHost>,
        reruns: CompareReruns,
    }

    impl Setup {
        fn new(
            changes: bool,
            refuse_judge: Option<&str>,
            never_settles: bool,
            fail_first: bool,
            panel: Vec<CompareRoute>,
            slow_panel: bool,
        ) -> Self {
            let root = std::env::temp_dir().join(format!(
                "locust-compare-reruns-{}-{}",
                std::process::id(),
                crate::invariants::unique_nanos()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let dir = TempDir(root.clone());
            let store = CompareStore::new(root, None).unwrap();
            let base = "c".repeat(40);
            let origin = store
                .create(CreateCompareInput {
                    teammate_id: Some("tm_wren".into()),
                    prompt: "Explain the date bug.".into(),
                    routes: routes(),
                    changes,
                    changes_in: None,
                    blind: false,
                    base: if changes { Some(base) } else { None },
                    rerun_of: None,
                })
                .unwrap();
            store.add_turn(&origin.compare_id, CompareSlotId::A, "mission_a1").unwrap();
            store.add_turn(&origin.compare_id, CompareSlotId::B, "mission_b1").unwrap();
            let members: Vec<JudgePanelMember> = panel
                .into_iter()
                .enumerate()
                .map(|(i, route)| JudgePanelMember {
                    route,
                    mission_id: format!("mission_p1_{i}"),
                })
                .collect();
            let origin = store
                .judge(
                    &origin.compare_id,
                    judge_route(),
                    "mission_j1",
                    Some("Names the cause\nFixes the test".into()),
                    members,
                )
                .unwrap();
            let host = Arc::new(FakeHost {
                store: store.clone(),
                calls: Mutex::new(Vec::new()),
                live: Mutex::new(HashSet::new()),
                looks: Mutex::new(HashMap::new()),
                told: Mutex::new(Vec::new()),
                missions: Mutex::new(0),
                refuse_judge: refuse_judge.map(str::to_owned),
                never_settles,
                fail_first,
                slow_panel,
            });
            let reruns = CompareReruns::new(store.clone(), host.clone());
            Self {
                _dir: dir,
                store,
                origin,
                host,
                reruns,
            }
        }

        fn finished(&self) {
            for _ in 0..2_000 {
                if self.reruns.progress().is_empty() {
                    return;
                }
                thread::sleep(Duration::from_millis(5));
            }
            panic!("reruns did not finish in time");
        }
    }

    #[test]
    fn holds_measured_run_for_checks_and_skips_auto_retries() {
        let s = Setup::new(false, None, false, true, Vec::new(), false);
        assert!(!s.reruns.checked(&s.origin.compare_id, "cmp_wrong"));
        s.reruns.start(&s.origin.compare_id, 1, true).unwrap();
        for _ in 0..2_000 {
            if s.reruns
                .progress()
                .first()
                .is_some_and(|p| p.step == RerunStep::Checking)
            {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let waiting = s.reruns.progress()[0].clone();
        assert_eq!(waiting.step, RerunStep::Checking);
        {
            let calls = s.host.calls.lock().unwrap();
            assert!(!calls.iter().any(|c| {
                c.starts_with("judge:") || c.starts_with("tidy:") || c.starts_with("retry:")
            }));
        }
        let cid = waiting.compare_id.as_deref().unwrap();
        assert!(!s.reruns.checked("cmp_wrong", cid));
        assert!(!s.reruns.checked(&s.origin.compare_id, "cmp_wrong"));
        assert!(s.reruns.checked(&s.origin.compare_id, cid));
        assert!(!s.reruns.checked(&s.origin.compare_id, cid));
        s.finished();
        let calls = s.host.calls.lock().unwrap();
        assert!(calls.iter().any(|c| c.starts_with("judge:")));
        assert!(calls.iter().any(|c| c.starts_with("tidy:")));
    }

    #[test]
    fn stops_measured_run_waiting_for_checks() {
        let s = Setup::new(false, None, false, false, Vec::new(), false);
        s.reruns.start(&s.origin.compare_id, 3, true).unwrap();
        for _ in 0..2_000 {
            if s.reruns
                .progress()
                .first()
                .is_some_and(|p| p.step == RerunStep::Checking)
            {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let id = s.reruns.progress()[0].compare_id.clone().unwrap();
        s.reruns.stop(&s.origin.compare_id);
        assert!(!s.reruns.checked(&s.origin.compare_id, &id));
        s.finished();
        let calls = s.host.calls.lock().unwrap();
        assert_eq!(calls.iter().filter(|c| c.starts_with("begin:")).count(), 1);
        assert!(!calls.iter().any(|c| c.starts_with("judge:")));
        assert!(calls.contains(&format!("tidy:{id}")));
    }

    #[test]
    fn runs_again_sequentially_with_same_models_prompt_and_judge() {
        let s = Setup::new(false, None, false, false, Vec::new(), false);
        s.reruns.start(&s.origin.compare_id, 3, false).unwrap();
        s.finished();
        let gathered = s.store.get(&s.origin.compare_id).unwrap().unwrap();
        assert_eq!(gathered.reruns.len(), 3);
        let calls = s.host.calls.lock().unwrap();
        for rid in &gathered.reruns {
            let rerun = s.store.get(rid).unwrap().unwrap();
            assert_eq!(rerun.rerun_of.as_deref(), Some(s.origin.compare_id.as_str()));
            assert_eq!(rerun.prompt, "Explain the date bug.");
            assert_eq!(rerun.teammate_id.as_deref(), Some("tm_wren"));
            assert!(rerun.settled);
            assert!(calls.contains(&format!(
                "judge:{rid}:{}:Names the cause\nFixes the test",
                judge_route().model
            )));
            assert!(calls.contains(&format!("tidy:{rid}")));
        }
        let order: Vec<&str> = calls
            .iter()
            .filter(|c| c.starts_with("begin:") || c.starts_with("judge:"))
            .map(|c| c.split(':').next().unwrap())
            .collect();
        assert_eq!(order, vec!["begin", "judge", "begin", "judge", "begin", "judge"]);
    }

    #[test]
    fn judges_each_run_with_three_judges_and_waits_for_all_three() {
        let ling = CompareRoute::new("opencode", "opencode/ling-3.1-flash-free");
        let nemo = CompareRoute::new("opencode", "opencode/nemotron-3-ultra-free");
        let s = Setup::new(false, None, false, false, vec![ling.clone(), nemo.clone()], true);
        s.reruns.start(&s.origin.compare_id, 3, false).unwrap();
        s.finished();
        let ids = s.store.get(&s.origin.compare_id).unwrap().unwrap().reruns;
        assert_eq!(ids.len(), 3);
        let calls = s.host.calls.lock().unwrap();
        for rid in &ids {
            assert!(calls.contains(&format!("panel:{rid}:{},{}", ling.model, nemo.model)));
        }
        let second_begin = calls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.starts_with("begin:"))
            .nth(1)
            .map(|(i, _)| i)
            .unwrap();
        let first_wait = calls.iter().position(|c| c.starts_with("waited:")).unwrap();
        assert!(first_wait < second_begin);
    }

    #[test]
    fn starts_changes_rerun_from_same_commit() {
        let s = Setup::new(true, None, false, false, Vec::new(), false);
        s.reruns.start(&s.origin.compare_id, 1, false).unwrap();
        s.finished();
        let rid = &s.store.get(&s.origin.compare_id).unwrap().unwrap().reruns[0];
        let rerun = s.store.get(rid).unwrap().unwrap();
        assert!(rerun.changes);
        assert_eq!(rerun.base.as_deref(), Some("c".repeat(40).as_str()));
    }

    #[test]
    fn tells_progress_while_running_and_empty_when_done() {
        let s = Setup::new(false, None, false, false, Vec::new(), false);
        s.reruns.start(&s.origin.compare_id, 1, false).unwrap();
        s.finished();
        let told = s.host.told.lock().unwrap();
        assert!(told.iter().any(|(_, prog)| prog
            .iter()
            .any(|p| p.origin_id == s.origin.compare_id && p.step == RerunStep::Judging)));
        assert!(told.last().unwrap().1.is_empty());
    }

    #[test]
    fn stops_current_run_and_begins_no_more() {
        let s = Setup::new(false, None, true, false, Vec::new(), false);
        s.reruns.start(&s.origin.compare_id, 3, false).unwrap();
        for _ in 0..200 {
            if s.host
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| c.starts_with("begin:"))
            {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        s.reruns.stop(&s.origin.compare_id);
        s.finished();
        let calls = s.host.calls.lock().unwrap();
        assert_eq!(calls.iter().filter(|c| c.starts_with("begin:")).count(), 1);
        assert_eq!(calls.iter().filter(|c| c.starts_with("stop:")).count(), 2);
        assert!(!calls.iter().any(|c| c.starts_with("judge:")));
        let rid = &s.store.get(&s.origin.compare_id).unwrap().unwrap().reruns[0];
        assert!(calls.contains(&format!("tidy:{rid}")));
        assert!(s.store.get(rid).unwrap().unwrap().settled);
    }

    #[test]
    fn retries_failed_column_once_before_judging() {
        let s = Setup::new(false, None, false, true, Vec::new(), false);
        s.reruns.start(&s.origin.compare_id, 1, false).unwrap();
        s.finished();
        let rid = s.store.get(&s.origin.compare_id).unwrap().unwrap().reruns[0].clone();
        let calls = s.host.calls.lock().unwrap();
        let retry_pos = calls.iter().position(|c| c == &format!("retry:{rid}:a")).unwrap();
        let judge_pos = calls.iter().position(|c| c.starts_with("judge:")).unwrap();
        assert!(retry_pos < judge_pos);
        assert!(s
            .store
            .get(&rid)
            .unwrap()
            .unwrap()
            .slots[0]
            .mission_ids
            .last()
            .unwrap()
            .starts_with("mission_retried_"));
    }

    #[test]
    fn continues_when_judge_refuses_and_leaves_unscored() {
        let s = Setup::new(
            false,
            Some("A judge needs at least two finished answers to read."),
            false,
            false,
            Vec::new(),
            false,
        );
        s.reruns.start(&s.origin.compare_id, 3, false).unwrap();
        s.finished();
        assert_eq!(
            s.host
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.starts_with("begin:"))
                .count(),
            3
        );
        for rid in &s.store.get(&s.origin.compare_id).unwrap().unwrap().reruns {
            assert!(s.store.get(rid).unwrap().unwrap().judge.is_none());
        }
    }

    #[test]
    fn settles_leftover_runs_except_live_ones() {
        let s = Setup::new(false, None, false, false, Vec::new(), false);
        let left = s
            .store
            .create(CreateCompareInput {
                teammate_id: Some("tm_wren".into()),
                prompt: s.origin.prompt.clone(),
                routes: routes(),
                changes: false,
                changes_in: None,
                blind: false,
                base: None,
                rerun_of: Some(s.origin.compare_id.clone()),
            })
            .unwrap();
        let going = s
            .store
            .create(CreateCompareInput {
                teammate_id: Some("tm_wren".into()),
                prompt: s.origin.prompt.clone(),
                routes: routes(),
                changes: false,
                changes_in: None,
                blind: false,
                base: None,
                rerun_of: Some(s.origin.compare_id.clone()),
            })
            .unwrap();
        s.store
            .add_turn(&going.compare_id, CompareSlotId::A, "mission_still_going")
            .unwrap();
        s.host
            .live
            .lock()
            .unwrap()
            .insert("mission_still_going".into());

        s.reruns.settle_leftover();
        assert!(s.store.get(&left.compare_id).unwrap().unwrap().settled);
        assert!(!s.store.get(&going.compare_id).unwrap().unwrap().settled);
    }

    #[test]
    fn keeps_each_columns_instructions_when_whole_comparison_runs_again() {
        let s = Setup::new(false, None, false, false, Vec::new(), false);
        let mut r0 = CompareRoute::new("codex", "same");
        r0.instructions = Some("Saved.".into());
        let mut r1 = CompareRoute::new("codex", "same");
        r1.instructions = Some(String::new());
        let mut r2 = CompareRoute::new("codex", "same");
        r2.instructions = Some("Check twice.".into());
        let cmp = s
            .store
            .create(CreateCompareInput {
                teammate_id: Some("tm_wren".into()),
                prompt: "Explain the cause.".into(),
                routes: vec![r0, r1, r2],
                changes: false,
                changes_in: None,
                blind: false,
                base: None,
                rerun_of: None,
            })
            .unwrap();
        let cmp = s
            .store
            .judge(&cmp.compare_id, judge_route(), "mj", None, Vec::new())
            .unwrap();
        s.reruns.start(&cmp.compare_id, 1, false).unwrap();
        s.finished();
        let rid = &s.store.get(&cmp.compare_id).unwrap().unwrap().reruns[0];
        let rerun = s.store.get(rid).unwrap().unwrap();
        let got: Vec<Option<&str>> = rerun
            .slots
            .iter()
            .map(|c| c.route.instructions.as_deref())
            .collect();
        assert_eq!(got, vec![Some("Saved."), Some(""), Some("Check twice.")]);
    }
}
