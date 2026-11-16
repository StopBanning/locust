//! What each teammate's face and row in the sidebar and the rail show: Electron's one status per teammate
//! (`face_status::teammate_status_view`, App.tsx 9264-9300 `viewByTeammate`), from what this window knows.
//!
//! - **Blocked**: the agents discovery found, once it has answered (Settings › AI agents asks it).
//! - **Waiting on you**: the cards the engine holds for the teammate's runs.
//! - **Live**: a run of theirs running (the engine's, or this window's own send); what it is doing is read off its
//!   ledger's events, at most twice a second while anything runs (`LiveFaces`). Electron streams those events; this
//!   window reads the file, so a change shows up to half a second later.
//! - **Just finished**: a run the list last saw running and now sees completed, for `DONE_HOP_MS`.
//! - **Just received**: a handoff to them is held (`handoff_glances`, `RECEIVED_GLANCE_MS`).

use super::*;
use locust_core::bot_rig::{FaceActivity, LiveActivity};
use locust_core::face_status::{runtime_reach, teammate_status_view, RuntimeFacts, StatusInputs, StatusView, DONE_HOP_MS};
use std::collections::HashMap;

/// How often the live runs' events are read again, in seconds.
const READ_EVERY_S: f64 = 0.5;

#[derive(Default)]
pub(super) struct LiveFaces {
    /// What each live run is doing, by its mission.
    pub by_mission: HashMap<String, LiveActivity>,
    pub job: Latest<HashMap<String, LiveActivity>>,
    asked_at: Option<f64>,
    /// Teammates whose run just finished, until when (window clock).
    pub done_until: HashMap<String, f64>,
}

/// In `poll`: read the live runs' events again when due, take a finished read, and let a finished hop end. True when
/// a face may have changed.
pub(super) fn tick(s: &mut State, now: f64) -> bool {
    let mut changed = false;
    if let Some(read) = s.live_faces.job.take() {
        if read != s.live_faces.by_mission {
            s.live_faces.by_mission = read;
            changed = true;
        }
    }
    let before = s.live_faces.done_until.len();
    s.live_faces.done_until.retain(|_, until| now < *until);
    changed |= s.live_faces.done_until.len() != before;
    if s.seen_live.is_empty() {
        if !s.live_faces.by_mission.is_empty() {
            s.live_faces.by_mission.clear();
            changed = true;
        }
        return changed;
    }
    let due = s.live_faces.asked_at.is_none_or(|at| now - at >= READ_EVERY_S || now < at);
    if due && !s.live_faces.job.is_pending() && let Some(store) = s.data.clone() {
        s.live_faces.asked_at = Some(now);
        let live = s.seen_live.clone();
        s.live_faces.job.request(live, move |live| {
            live.into_iter()
                .filter_map(|id| {
                    let path = store.write_path(&id).ok()?;
                    Some((id, locust_core::face_status::live_activity_in_ledger(&path)?))
                })
                .collect()
        });
    }
    changed
}

/// A new list came back: each run it last saw running and now sees completed hops its teammate's face
/// (App.tsx 3315-3320, on `run.completed`).
pub(super) fn note_finished(s: &mut State, next: &[ConversationRow], owners: &HashMap<String, String>, now: f64) {
    for row in next.iter().filter(|row| row.state == "completed") {
        let was_running = s.recent.iter().any(|old| old.id == row.id && old.state == "running");
        if was_running && let Some(owner) = owners.get(&row.id) {
            s.live_faces.done_until.insert(owner.clone(), now + DONE_HOP_MS as f64 / 1000.0);
        }
    }
}

/// The agents discovery found, as the face reads them; None until it has answered.
fn runtimes(s: &State) -> Option<Vec<RuntimeFacts>> {
    s.settings_panel.agents.as_ref().map(|found| {
        found
            .iter()
            .map(|f| RuntimeFacts {
                id: f.id.clone(),
                ready: f.ready,
                status: f.status,
                installed: f.installed,
                checking: f.checking,
            })
            .collect()
    })
}

/// `teammate`'s status now, given the agents discovery found and the cards waiting on the person.
fn view_of(s: &State, teammate: &Teammate, agents: Option<&[RuntimeFacts]>, waiting: &[locust_core::approval::Request]) -> StatusView {
    let own_id = teammate.runtime.as_deref();
    let own = agents.and_then(|all| all.iter().find(|r| Some(r.id.as_str()) == own_id));
    let theirs = |mission: &str| s.mission_owners.get(mission).is_some_and(|owner| owner == &teammate.id);
    let live_missions: Vec<&String> = s.seen_live.iter().filter(|id| theirs(id)).collect();
    let running = !live_missions.is_empty()
        || teammate.busy
        || s.flight.as_ref().is_some_and(|f| f.teammate_id == teammate.id)
        || s.ledger.as_ref().is_some_and(|pane| pane.header.id == teammate.id && pane.snapshot.running);
    let pending_approvals = waiting.iter().filter(|card| theirs(&card.mission_id)).count();
    teammate_status_view(&StatusInputs {
        own_runtime: own,
        reach: agents.map(|all| runtime_reach(all, true)).unwrap_or_default(),
        running,
        pending_approvals,
        role_label: &teammate.role,
        live: live_missions.iter().find_map(|id| s.live_faces.by_mission.get(*id).copied()),
        recently_done: s.live_faces.done_until.contains_key(&teammate.id),
        recently_received: s.handoff_glances.held.iter().any(|h| h.to == teammate.id),
    })
}

/// Every teammate's status, for one line of faces.
pub(super) fn views(s: &State) -> HashMap<String, StatusView> {
    let agents = runtimes(s);
    let waiting = locust_core::engine_api::api().waiting();
    s.teammates.iter().map(|t| (t.id.clone(), view_of(s, t, agents.as_deref(), &waiting))).collect()
}

/// The dot a face wears for its status (status.ts `facePresenceFor`).
pub(super) fn presence_dot(view: &StatusView) -> Option<crate::look::bot_stage::PresenceDot> {
    use crate::look::bot_stage::PresenceDot;
    use locust_core::face_status::{face_presence_for, Presence};
    match face_presence_for(view.status) {
        Presence::Working => Some(PresenceDot::Working),
        Presence::Approval => Some(PresenceDot::Waiting),
        Presence::Blocked => Some(PresenceDot::Blocked),
        Presence::None => None,
    }
}

/// The colour of a status's tone (`lc-tone-*`).
pub(super) fn tone_color(l: &crate::look::controls::Look, tone: locust_core::face_status::Tone) -> rune::Color {
    use locust_core::face_status::Tone;
    match tone {
        Tone::Live => l.c("green"),
        Tone::Amber => l.c("amber"),
        Tone::Red => l.c("red"),
        Tone::Muted => l.c("text-muted"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn mate(id: &str) -> Teammate {
        Teammate {
            id: id.into(),
            name: id.into(),
            role: String::new(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        }
    }

    /// Ticks the faces until `done`, drawing each time; at most 60 s, as the window tests' pump waits (the engine's
    /// `load_waits::UNDER_LOAD`), and straight on once it is true.
    fn run_until(rt: &mut rune::Runtime<State>, now: &mut f64, what: &str, done: impl Fn(&rune::Runtime<State>) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            let at = *now;
            rt.update(move |s, _| {
                tick(s, at);
            });
            rt.frame(0.1);
            if done(rt) {
                return;
            }
            assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
            *now += 0.1;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn activity(s: &State, id: &str) -> FaceActivity {
        views(s).get(id).map(|v| v.activity).expect("a status for every teammate")
    }

    /// The rail face's (name, description), as the accessibility tree tells a screen reader.
    fn rail_words(rt: &rune::Runtime<State>, id: &str) -> (Option<String>, Option<String>) {
        fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
            if node["key"] == key {
                return Some(node);
            }
            node["children"].as_array()?.iter().find_map(|child| find(child, key))
        }
        let tree = rt.accessibility_json();
        let face = find(&tree, &format!("rail-{id}")).expect("the rail face in the accessibility tree");
        let said = |field: &str| face[field].as_str().map(str::to_owned);
        (said("name"), said("description"))
    }

    fn listed(id: &str, state: &str) -> ConversationRow {
        ConversationRow {
            id: id.into(),
            teammate: String::new(),
            title: String::new(),
            started_by: "the person".into(),
            state: state.into(),
            updated: String::new(),
        }
    }

    fn append(path: &std::path::Path, event: serde_json::Value) {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(file, "{}", serde_json::json!({ "event": event })).unwrap();
    }

    #[test]
    fn a_running_teammate_shows_what_its_ledger_says_on_the_roster_face_thinking_then_working() {
        let (_dir, store) = crate::org::tests::temp_store();
        let ledger = store.write_path("m_face_1").unwrap();
        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        append(&ledger, serde_json::json!({"type": "step.started", "payload": {"stepKind": "reasoning"}}));
        let mut s = State::default();
        s.teammates = vec![mate("wren"), mate("sable")];
        s.data = Some(store);
        s.seen_live = vec!["m_face_1".into()];
        s.mission_owners = HashMap::from([("m_face_1".to_string(), "wren".to_string())]);
        let mut rt = rune::Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
        let face = |rt: &rune::Runtime<State>, id: &str| rt.state.bot_stage.desired_activity(&format!("roster-{id}"));
        let mut now = 0.0;
        run_until(&mut rt, &mut now, "wren thinking", |rt| face(rt, "wren") == Some(FaceActivity::Thinking));
        assert_eq!(face(&rt, "sable"), Some(FaceActivity::Idle), "sable runs nothing");

        append(&ledger, serde_json::json!({"type": "tool.started", "payload": {"itemId": "t1", "toolKind": "command_execution", "name": "shell"}}));
        run_until(&mut rt, &mut now, "wren working", |rt| face(rt, "wren") == Some(FaceActivity::Working));
    }

    #[test]
    fn a_finished_run_hops_its_teammates_face_for_its_moment_then_rests() {
        let row = |state: &str| listed("m_face_2", state);
        let mut s = State::default();
        s.teammates = vec![mate("wren")];
        s.recent = vec![row("running")];
        let owners = HashMap::from([("m_face_2".to_string(), "wren".to_string())]);
        note_finished(&mut s, &[row("completed")], &owners, 10.0);
        assert_eq!(activity(&s, "wren"), FaceActivity::Done);
        tick(&mut s, 10.0 + DONE_HOP_MS as f64 / 1000.0 - 0.05);
        assert_eq!(activity(&s, "wren"), FaceActivity::Done, "still within {DONE_HOP_MS} ms");
        tick(&mut s, 10.0 + DONE_HOP_MS as f64 / 1000.0);
        assert_eq!(activity(&s, "wren"), FaceActivity::Idle);
    }

    #[test]
    fn a_run_already_finished_when_first_listed_does_not_hop() {
        let row = listed("m_face_3", "completed");
        let mut s = State::default();
        s.teammates = vec![mate("wren")];
        let owners = HashMap::from([("m_face_3".to_string(), "wren".to_string())]);
        note_finished(&mut s, &[row], &owners, 1.0);
        assert_eq!(activity(&s, "wren"), FaceActivity::Idle);
    }

    #[test]
    fn a_card_waiting_on_the_person_shows_waiting_on_the_rail_face_and_in_its_words() {
        // The card is the whole process's, and the window counts a card's conversation as live: hold the engine, as
        // every test that runs a turn does, so no other test's window sees it.
        let (_dir, store) = crate::org::tests::temp_store();
        let _engine = locust_core::native::hooks::install(store, None);
        let mut s = State::default();
        s.teammates = vec![mate("wren"), mate("sable")];
        s.seen_live = vec!["m_face_4".into()];
        s.mission_owners = HashMap::from([("m_face_4".to_string(), "sable".to_string())]);
        let stop = Arc::new(AtomicBool::new(false));
        let asking = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let _ = locust_core::native::approvals_channel().ask_browser("m_face_4", "Open a page", "example", &|| stop.load(Ordering::SeqCst));
            })
        };
        let mut rt = rune::Runtime::new(s, |s, _| crate::rail::rail(s, &Palette::new(true)), (72.0, 800.0), rune::Env::new(true, 1.0));
        let mut now = 0.0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_until(&mut rt, &mut now, "sable waiting", |rt| rt.state.bot_stage.desired_activity("rail-sable") == Some(FaceActivity::Waiting));
            assert_eq!(rt.state.bot_stage.desired_activity("rail-wren"), Some(FaceActivity::Idle));
            // a-face-says-its-state-in-words.test.tsx, the compact rail: the hover's words, then the state.
            let (label, description) = rail_words(&rt, "sable");
            assert_eq!(label.as_deref(), Some("Message sable · "));
            assert_eq!(description.as_deref(), Some("waiting on you"));
            assert_eq!(rail_words(&rt, "wren").1.as_deref(), Some("idle"));
            assert_eq!(crate::look::faces::drawn_presence("rail-sable"), Some(Some(crate::look::bot_stage::PresenceDot::Waiting)), "the amber dot");
            assert_eq!(crate::look::faces::drawn_presence("rail-wren"), Some(None), "no dot at rest");
        }));
        stop.store(true, Ordering::SeqCst);
        asking.join().unwrap();
        result.unwrap();
    }

    fn found(id: &str, ready: bool, installed: bool, status: locust_core::agents::Probe) -> locust_core::agents::Found {
        locust_core::agents::Found {
            id: id.into(),
            display_name: id.into(),
            installed,
            version: installed.then(|| "1.0.0".into()),
            auth: locust_core::agents::Auth::Unknown,
            ready,
            status,
            unsupported_note: None,
            through_cli: false,
            checking: false,
        }
    }

    #[test]
    fn when_no_agent_on_the_machine_can_run_every_face_is_blocked_and_says_none_is_installed() {
        use locust_core::agents::Probe;
        let mut s = State::default();
        // Wren is set to an agent discovery did not list, so only the machine-wide answer can block her.
        let mut wren = mate("wren");
        wren.runtime = Some("claude".into());
        s.teammates = vec![wren];
        s.settings_panel.agents = Some(vec![found("opencode", false, false, Probe::Missing)]);
        let mut rt = rune::Runtime::new(s, |s, _| crate::rail::rail(s, &Palette::new(true)), (72.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        assert_eq!(rt.state.bot_stage.desired_activity("rail-wren"), Some(FaceActivity::Blocked));
        assert_eq!(rail_words(&rt, "wren").1.as_deref(), Some("No AI agent installed"));
    }

    #[test]
    fn a_teammate_whose_own_agent_is_signed_out_shows_blocked_and_says_why_in_the_rail() {
        use locust_core::agents::{Auth, Found, Probe};
        let agent = |id: &str, ready: bool, status: Probe| Found {
            id: id.into(),
            display_name: id.into(),
            installed: true,
            version: Some("1.0.0".into()),
            auth: Auth::Unknown,
            ready,
            status,
            unsupported_note: None,
            through_cli: false,
            checking: false,
        };
        let mut s = State::default();
        let mut sable = mate("sable");
        sable.runtime = Some("claude".into());
        s.teammates = vec![mate("wren"), sable];
        s.settings_panel.agents = Some(vec![agent("codex", true, Probe::Ready), agent("claude", false, Probe::AuthRequired)]);
        let mut rt = rune::Runtime::new(s, |s, _| crate::rail::rail(s, &Palette::new(true)), (72.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        assert_eq!(rt.state.bot_stage.desired_activity("rail-sable"), Some(FaceActivity::Blocked));
        assert_eq!(rail_words(&rt, "sable").1.as_deref(), Some("Sign-in needed"), "a blocked face says why");
        assert_eq!(crate::look::faces::drawn_presence("rail-sable"), Some(Some(crate::look::bot_stage::PresenceDot::Blocked)));
        assert_eq!(rt.state.bot_stage.desired_activity("rail-wren"), Some(FaceActivity::Idle), "codex is ready");
    }
}
