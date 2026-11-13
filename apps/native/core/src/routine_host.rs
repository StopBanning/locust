//! THE RUNNER'S HOST IN THE APP (main/index.ts's `createRoutineRunner({...})` options), for the native engine.
//!
//! Every turn a routine starts goes through `engine_api::api()` -- the engine may be a separate process -- with an
//! `Origin` whose `started_by` is `{kind: 'routine', routineId, step}`. Endings, answers and questions are read from
//! the turn's ledger; whether a turn is still running, from the engine. A native teammate works in a folder of its
//! own, so "the folder the window is in" is the routine's teammate's folder: a step runs there (or in the routine's
//! copy), the goal check is that folder's check command, and the watched folder is inside it.
//!
//! The folder watcher lives here too. What a teammate's turn wrote is read from Undo's own checkpoint records in the
//! native data folder, which a turn in the engine process writes as it ends, so the watcher learns of it whichever
//! process ran the turn (Electron's `turnWrote`).
use crate::door_tools::Turn;
use crate::engine::Teammate;
use crate::native::{Origin, Send};
use crate::routine_copy::RoutineCopies;
use crate::routine_files::{FileArrivals, Watch};
use crate::routine_runner::{Begun, GoalCheck, Host, Peer, Phase, StartError, StartRequest, Update};
use crate::store::Store;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

pub struct LiveHost {
    store: Store,
    copies: RoutineCopies,
    arrivals: Mutex<FileArrivals>,
    opened_at: DateTime<Utc>,
    updates: Mutex<Vec<Update>>,
    /// Added to the wall clock: a test moves time on rather than sleeping through a settle.
    skew_ms: AtomicI64,
    /// Checkpoint records already read, per project folder; the first look is a baseline.
    turns_seen: Mutex<HashMap<PathBuf, HashSet<String>>>,
}

/// A turn's record, read from its ledger.
pub fn read_turn(mission: &str) -> Option<Turn> {
    let path = crate::native::find_ledger(mission)?;
    Turn::from_records(&crate::ledger::Tail::new(&path, 0).poll())
}

impl LiveHost {
    pub fn new(store: Store, opened_at: DateTime<Utc>) -> Self {
        Self {
            copies: RoutineCopies::in_store(&store),
            store,
            arrivals: Mutex::new(FileArrivals::new()),
            opened_at,
            updates: Mutex::default(),
            skew_ms: AtomicI64::new(0),
            turns_seen: Mutex::default(),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn routine_copies(&self) -> &RoutineCopies {
        &self.copies
    }

    /// Move this host's clock on by `ms` (tests).
    pub fn skew(&self, ms: i64) {
        self.skew_ms.fetch_add(ms, Ordering::SeqCst);
    }

    /// What the runner said since the last look, oldest first.
    pub fn take_updates(&self) -> Vec<Update> {
        std::mem::take(&mut *self.updates.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn teammate(&self, id: &str) -> Option<Teammate> {
        let team = crate::people::team(&self.store).ok()?;
        crate::native::parse_roster(&team.to_string()).ok()?.into_iter().find(|t| t.id == id)
    }

    /// The folder a teammate works in.
    pub fn folder(&self, id: &str) -> Option<PathBuf> {
        crate::native::folder_for(&self.teammate(id)?, self.store.electron()).ok()
    }

    /// Look at every watched folder once (`pollWatchedFolders`). True when some routine has new files ready.
    pub fn watch(&self, routines: &[Value]) -> bool {
        let watching: Vec<Watch> = routines
            .iter()
            .filter(|r| r["schedule"]["kind"] == "files")
            .filter_map(|r| {
                let home = r["teammateId"].as_str()?;
                let project = self.folder(home)?;
                let ws = crate::people::workspace_id(&project);
                if r["workspaceId"].as_str().is_some_and(|w| w != ws) {
                    return None;
                }
                Some(Watch { routine_id: r["routineId"].as_str()?.to_owned(), project, folder: r["schedule"]["folder"].as_str()?.to_owned() })
            })
            .collect();
        if watching.is_empty() {
            return false;
        }
        let now = self.now().timestamp_millis();
        let mut arrivals = self.arrivals.lock().unwrap_or_else(|e| e.into_inner());
        // What each turn wrote, from its checkpoint record: none of it is an arrival.
        let mut seen = self.turns_seen.lock().unwrap_or_else(|e| e.into_inner());
        let projects: HashSet<&PathBuf> = watching.iter().map(|w| &w.project).collect();
        for project in projects {
            let records = crate::checkpoints::records(&self.store, project).unwrap_or_default();
            let first = !seen.contains_key(project);
            let known = seen.entry(project.clone()).or_default();
            for (id, record) in records {
                if known.insert(id) && !first {
                    let workspace = record["workspace"].as_str().map(PathBuf::from).unwrap_or_else(|| project.clone());
                    arrivals.wrote(&workspace, &crate::checkpoints::changed(&record));
                }
            }
        }
        // A teammate at work: what settles now may be its own, held until its turn says what it wrote.
        let writing = !crate::engine_api::api().running().is_empty();
        arrivals.poll(&watching, now, writing);
        watching.iter().any(|w| !arrivals.ready(&w.routine_id, now).is_empty())
    }
}

/// Electron's spend refusal is SPEND_LIMIT_REACHED; native says it in `spend::limit_refusal`'s words.
fn refusal_code(message: &str) -> &'static str {
    if message.contains("has reached this month's limit") { "SPEND_LIMIT_REACHED" } else { "POLICY_REFUSED" }
}

impl Host for LiveHost {
    fn workspace_id(&self, home: &str) -> String {
        self.folder(home).map_or_else(|| "unknown".into(), |f| crate::people::workspace_id(&f))
    }

    fn peer(&self, teammate: &str) -> Option<Peer> {
        self.teammate(teammate).map(|t| Peer { teammate_id: t.id, name: t.name })
    }

    /// A step's turn, through the engine. `send` answers Err only before the agent's process is started -- the
    /// process is spawned on the turn's own thread, and a failure there is written in the turn's ledger -- so an Err
    /// proves nothing ran: a preflight refusal in Electron's terms.
    fn start(&self, request: StartRequest) -> Result<Begun, StartError> {
        let Some(mut teammate) = self.teammate(&request.peer.teammate_id) else {
            return Err(StartError::Refused { code: "POLICY_REFUSED".into(), message: "That teammate no longer exists.".into() });
        };
        teammate.runs_in = request.mode.clone();
        let folder = request.cwd.clone().or_else(|| self.folder(&request.home));
        let follow_up = request.follow_up_of.clone();
        let job = Send { teammate, conversation: follow_up.clone(), prompt: request.prompt, model: request.model, effort: request.effort };
        let origin = Origin {
            mode: crate::approval::Mode::parse(&request.mode),
            folder,
            started_by: Some(request.started_by),
            runtime: Some(request.runtime),
            ..Default::default()
        };
        match crate::engine_api::api().send(job, origin) {
            Ok(started) => {
                // A follow-up's own id: the conversation's newest turn now.
                let mission = match follow_up {
                    Some(_) => crate::native::real_tip(&started.conversation_id).map_err(StartError::Threw)?,
                    None => started.conversation_id,
                };
                Ok(Begun { run_id: format!("run_{mission}"), mission_id: mission })
            }
            Err(message) => Err(StartError::Refused { code: refusal_code(&message).into(), message }),
        }
    }

    fn phase_of(&self, mission: &str) -> Option<Phase> {
        Some(match read_turn(mission)?.phase() {
            "completed" => Phase::Completed,
            "failed" => Phase::Failed,
            "cancelled" => Phase::Cancelled,
            _ => Phase::Interrupted,
        })
    }

    fn is_live(&self, mission: &str) -> bool {
        crate::engine_api::api().owns(mission)
    }

    /// The same reading the attention card makes: the last final message, and a decision block in it.
    fn asked_a_question(&self, mission: &str) -> Result<bool, String> {
        let turn = read_turn(mission).ok_or("Routine mission receipt unavailable")?;
        Ok(crate::door_tools::latest_final(&turn).is_some_and(|text| crate::protocol_tags::parse_decision(&text).is_some()))
    }

    fn route_of(&self, teammate: &str) -> Option<Value> {
        let team = crate::people::team(&self.store).ok()?;
        let row = team["teammates"].as_array()?.iter().find(|t| t["teammateId"] == teammate)?.clone();
        let t = self.teammate(teammate)?;
        let mut route = json!({
            "runtime": row["route"]["runtime"].as_str().map(str::to_owned).or(t.runtime).unwrap_or_else(|| "codex".into()),
            "model": row["route"]["model"].as_str().map(str::to_owned).or(t.model).unwrap_or_else(|| "account-default".into()),
            "mode": row["route"]["mode"].as_str().unwrap_or(&t.runs_in),
        });
        if let Some(effort) = row["route"]["effort"].as_str() {
            route["effort"] = json!(effort);
        }
        Some(route)
    }

    fn reply_of(&self, mission: &str) -> Option<String> {
        crate::door_tools::latest_final(&read_turn(mission)?)
    }

    fn notify(&self, update: Update) {
        self.updates.lock().unwrap_or_else(|e| e.into_inner()).push(update);
    }

    fn copies(&self) -> Option<&RoutineCopies> {
        Some(&self.copies)
    }

    fn folder_now(&self, home: &str) -> Option<PathBuf> {
        self.folder(home)
    }

    fn has_goal_check(&self) -> bool {
        true
    }

    /// `runFolderCheck`: the folder's own check command (Settings > Project folder), run in `cwd`.
    fn goal_check(&self, home: &str, cwd: &Path) -> Option<GoalCheck> {
        let ws = crate::workspace_settings::workspace_id_for(&self.folder(home)?);
        let command = crate::edit_check::check_command_for(&self.store, Some(&ws)).filter(|c| !c.is_empty())?;
        let run = crate::edit_check::run_check_command(&command, cwd, crate::edit_check::CHECK_TIMEOUT_MS);
        let tail = if let Some(error) = &run.error {
            vec![error.clone()]
        } else if run.timed_out {
            vec!["The check ran past its time limit and was stopped.".into()]
        } else {
            let lines = crate::edit_check::comparable_lines(&run.output);
            let skip = lines.len().saturating_sub(25);
            lines[skip..]
                .iter()
                .map(|l| if crate::hand_off::len16(l) > 300 { format!("{}…", crate::hand_off::head16(l, 299)) } else { l.clone() })
                .collect()
        };
        Some(GoalCheck { command, passed: run.error.is_none() && !run.timed_out && run.exit_code == Some(0), tail })
    }

    /// A scheduled routine waits for any live run of the teammate's, whoever started it.
    fn teammate_busy(&self, teammate: &str) -> Option<bool> {
        Some(crate::engine_api::api().running().iter().any(|m| crate::people::owner_of(&self.store, m).as_deref() == Some(teammate)))
    }

    fn arrivals_ready(&self, routine: &str, now: i64) -> Vec<String> {
        self.arrivals.lock().unwrap_or_else(|e| e.into_inner()).ready(routine, now)
    }

    fn arrivals_fired(&self, routine: &str, files: &[String], now: i64) {
        self.arrivals.lock().unwrap_or_else(|e| e.into_inner()).fired(routine, files, now);
    }

    fn opened_at(&self) -> Option<DateTime<Utc>> {
        Some(self.opened_at)
    }

    fn now(&self) -> DateTime<Utc> {
        Utc::now() + chrono::Duration::milliseconds(self.skew_ms.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routine_runner::{RoutineRunner, Routines};
    use crate::routines::progress::StoreRoutines;
    use std::sync::Arc;

    fn fixture() -> (PathBuf, Store) {
        let root = std::env::temp_dir().join(format!("locust-routine-host-{}", crate::native::uuid_v4().unwrap()));
        let electron = root.join("electron");
        std::fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::fs::write(electron.join("routines.json"), json!({"schemaVersion": 1, "routines": []}).to_string()).unwrap();
        std::fs::write(
            electron.join("teammates.json"),
            json!({"schemaVersion": 1, "teammates": [{"teammateId": "codec", "name": "Codec", "hue": "blue", "role": "Custom",
                "createdAt": "2026-11-01T00:00:00Z", "folder": root.join("work").to_string_lossy(),
                "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}}]})
            .to_string(),
        )
        .unwrap();
        (root.clone(), Store::new(root.join("native"), electron).unwrap())
    }

    fn wait(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !done() {
            assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// The live host over the native engine and the fake Codex: two steps run as one conversation, recorded as the
    /// routine, and the run counts once.
    #[test]
    fn two_steps_run_through_the_engine_as_one_conversation_and_count_once() {
        let (root, store) = fixture();
        let _hook = crate::native::hooks::install(
            store.clone(),
            Some(("python3".into(), vec![concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py").into(), "cards-routine".into()])),
        );
        crate::routines::save(&store, json!({"name": "Two steps", "teammateId": "codec", "steps": ["First.", "Second."]})).unwrap();
        let id = crate::routines::progress::all(&store).unwrap()[0]["routineId"].as_str().unwrap().to_owned();
        let host = Arc::new(LiveHost::new(store.clone(), Utc::now()));
        let routines = Arc::new(StoreRoutines(store.clone()));
        let runner = RoutineRunner::new(routines.clone(), host.clone());
        let begun = runner.run(&id, None).unwrap().unwrap();
        let first = read_turn(&begun.mission_id).unwrap();
        assert_eq!(first.started_by, json!({"kind": "routine", "routineId": id, "step": 1}));
        assert_eq!(first.prompt, "First.");
        wait("step 2 to start", || {
            runner.ended().unwrap();
            runner.running().first().is_some_and(|p| p.step == 2)
        });
        let second = runner.running()[0].mission_id.clone();
        assert_ne!(second, begun.mission_id);
        assert_eq!(crate::native::real_tip(&begun.mission_id).unwrap(), second, "step 2 follows up step 1's conversation");
        wait("the run to finish", || {
            runner.ended().unwrap();
            routines.get(&id).unwrap().unwrap()["runs"] == 1
        });
        let r = routines.get(&id).unwrap().unwrap();
        assert!(r.get("execution").is_none());
        let said: Vec<String> = host.take_updates().into_iter().filter_map(|u| if let Update::Notice { message, .. } = u { Some(message) } else { None }).collect();
        assert_eq!(said.last().unwrap(), "Routine \"Two steps\" finished: 2 steps completed.");
        assert_eq!(host.reply_of(&second).as_deref(), Some("Unknown cards scenario"));
        assert_eq!(host.asked_a_question(&second), Ok(false));
        drop(_hook);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_turn_that_wrote_a_file_into_the_watched_folder_does_not_start_its_routine() {
        let (root, store) = fixture();
        let work = root.join("work").canonicalize().unwrap();
        std::fs::create_dir_all(work.join("inbox")).unwrap();
        crate::routines::save(&store, json!({"name": "Inbox", "teammateId": "codec", "steps": ["Read it."], "schedule": {"kind": "files", "folder": "inbox"}})).unwrap();
        let routines = crate::routines::progress::all(&store).unwrap();
        // No other test's turn may be running: a live turn holds what settles until it says what it wrote.
        let _hook = crate::native::hooks::install(store.clone(), None);
        let host = LiveHost::new(store.clone(), Utc::now());
        assert!(!host.watch(&routines));
        // A turn writes inbox/summary.md, and Undo records it as the turn ends.
        let turn = crate::checkpoints::Turn::begin(&store, &work);
        std::fs::write(work.join("inbox/summary.md"), "mine\n").unwrap();
        turn.finish_reporting_edits(&store, "mission_writer").unwrap();
        // A person's file lands too.
        std::fs::write(work.join("inbox/invoice.pdf"), "theirs\n").unwrap();
        assert!(!host.watch(&routines));
        host.skew(crate::routine_files::SETTLE_MS + 1);
        assert!(host.watch(&routines));
        let id = routines[0]["routineId"].as_str().unwrap();
        assert_eq!(host.arrivals_ready(id, host.now().timestamp_millis()), vec!["invoice.pdf"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
