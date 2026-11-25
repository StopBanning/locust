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
    /// Newest event timestamp for each live run, by its mission (`live_run_last_at`).
    pub last_at_by_mission: HashMap<String, String>,
    pub job: Latest<HashMap<String, (LiveActivity, Option<String>)>>,
    asked_at: Option<f64>,
    /// Teammates whose run just finished, until when (window clock).
    pub done_until: HashMap<String, f64>,
}

/// In `poll`: read the live runs' events again when due, take a finished read, and let a finished hop end. True when
/// a face may have changed.
pub(super) fn tick(s: &mut State, now: f64) -> bool {
    let mut changed = false;
    if let Some(read) = s.live_faces.job.take() {
        let mut next_activity = HashMap::with_capacity(read.len());
        for (id, (activity, last_at)) in read {
            next_activity.insert(id.clone(), activity);
            if let Some(ts) = last_at {
                s.live_faces.last_at_by_mission.insert(id, ts);
            }
        }
        if next_activity != s.live_faces.by_mission {
            s.live_faces.by_mission = next_activity;
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
                    let activity = locust_core::face_status::live_activity_in_ledger(&path)?;
                    let last_at = locust_core::face_status::live_last_at_in_ledger(&path, None, None);
                    Some((id, (activity, last_at)))
                })
                .collect()
        });
    }
    changed
}

/// When `teammate_id` last worked: the newest event timestamp recorded from their live runs, falling back to `recent`.
#[allow(dead_code)]
pub(super) fn last_worked_for(s: &State, teammate_id: &str) -> Option<String> {
    let from_live = s
        .live_faces
        .last_at_by_mission
        .iter()
        .filter(|(mission, _)| s.mission_owners.get(*mission).is_some_and(|owner| owner == teammate_id))
        .map(|(_, ts)| ts.clone())
        .max();
    let from_recent = s
        .recent
        .iter()
        .filter(|r| s.mission_owners.get(&r.id).is_some_and(|owner| owner == teammate_id))
        .map(|r| r.updated.clone())
        .max();
    from_live.into_iter().chain(from_recent).max()
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

/// `teammate`'s status now, given the agents discovery found and the cards/questions waiting on the person.
fn view_of(
    s: &State,
    teammate: &Teammate,
    agents: Option<&[RuntimeFacts]>,
    waiting: &[locust_core::approval::Request],
    needs_you: &HashMap<String, usize>,
) -> StatusView {
    let own_id = teammate.runtime.as_deref();
    let own = agents.and_then(|all| all.iter().find(|r| Some(r.id.as_str()) == own_id));
    let theirs = |mission: &str| s.mission_owners.get(mission).is_some_and(|owner| owner == &teammate.id);
    let live_missions: Vec<&String> = s.seen_live.iter().filter(|id| theirs(id)).collect();
    let running = !live_missions.is_empty()
        || teammate.busy
        || s.flight.as_ref().is_some_and(|f| f.teammate_id == teammate.id)
        || s.ledger.as_ref().is_some_and(|pane| pane.header.id == teammate.id && pane.snapshot.running);
    let engine_waiting = waiting.iter().filter(|card| theirs(&card.mission_id)).count();
    let pending_approvals = engine_waiting.max(needs_you.get(&teammate.id).copied().unwrap_or(0));
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
    let needs_you = locust_core::face_status::waiting_by_teammate(&crate::rail::needs_you_items(s));
    s.teammates.iter().map(|t| (t.id.clone(), view_of(s, t, agents.as_deref(), &waiting, &needs_you))).collect()
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

    /// The face's (name, description) by its accessibility node key.
    fn face_words(rt: &rune::Runtime<State>, key: &str) -> (Option<String>, Option<String>) {
        fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
            if node["key"] == key {
                return Some(node);
            }
            node["children"].as_array()?.iter().find_map(|child| find(child, key))
        }
        let tree = rt.accessibility_json();
        let face = find(&tree, key).unwrap_or_else(|| panic!("face {key:?} in the accessibility tree"));
        let said = |field: &str| face[field].as_str().map(str::to_owned);
        (said("name"), said("description"))
    }

    /// The rail face's (name, description), as the accessibility tree tells a screen reader.
    fn rail_words(rt: &rune::Runtime<State>, id: &str) -> (Option<String>, Option<String>) {
        face_words(rt, &format!("rail-{id}"))
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

    // case: done hops once; it is not a loop
    #[test]
    fn done_hops_once_in_the_drawn_rail_and_is_not_a_loop() {
        let mut s = State::default();
        s.teammates = vec![mate("wren")];
        s.recent = vec![listed("m_hop_1", "running")];
        s.seen_live = vec!["m_hop_1".into()];
        s.mission_owners = HashMap::from([("m_hop_1".to_string(), "wren".to_string())]);
        let mut rt = rune::Runtime::new(s, |s, _| crate::rail::rail(s, &Palette::new(true)), (72.0, 800.0), rune::Env::new(true, 1.0));
        let mut tx = rune::Tx::default();
        rt.frame(0.0);
        rt.state.bot_stage.poll(0.0, false, true, &mut tx);
        // Run finishes: rail face switches from Working to Done.
        let owners = HashMap::from([("m_hop_1".to_string(), "wren".to_string())]);
        rt.update(move |s, _| {
            s.seen_live.clear();
            note_finished(s, &[listed("m_hop_1", "completed")], &owners, 0.05);
        });
        let mut hops = 0usize;
        let mut t = 0.05;
        while t < 1.5 {
            rt.state.bot_stage.begin_frame();
            rt.frame(0.05);
            rt.state.bot_stage.poll(t, false, true, &mut tx);
            if rt.state.bot_stage.last_hop("rail-wren") == Some(true) {
                hops += 1;
            }
            t += 0.05;
        }
        assert_eq!(hops, 1, "done hops once in the subtle rail, not zero and not a loop");
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

    // case: tells a screen reader what each face is doing, and keeps the name the drives find it by
    #[test]
    fn tells_a_screen_reader_what_each_face_is_doing_and_keeps_the_name_the_drives_find_it_by() {
        use locust_core::agents::Probe;
        let mut wren = mate("tm_wren");
        wren.name = "Wren".into();
        wren.role = "Code & Migrations".into();
        wren.busy = true;
        let mut sable = mate("tm_sable");
        sable.name = "Sable".into();
        sable.role = "Ops & Scheduling".into();
        sable.runtime = Some("claude".into());
        let mut s = State::default();
        s.teammates = vec![wren, sable];
        s.settings_panel.agents = Some(vec![
            found("codex", true, true, Probe::Ready),
            found("claude", false, true, Probe::AuthRequired),
        ]);
        let mut rt = rune::Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        assert_eq!(
            face_words(&rt, "teammate-tm_wren"),
            (Some("Wren — open their conversation".into()), Some("working".into()))
        );
        assert_eq!(
            face_words(&rt, "teammate-tm_sable"),
            (Some("Sable — open their conversation".into()), Some("Sign-in needed".into()))
        );
    }

    // case: says the state its hidden row would have shown
    #[test]
    fn says_the_state_its_hidden_row_would_have_shown() {
        let (_dir, store) = crate::org::tests::temp_store();
        let _engine = locust_core::native::hooks::install(store, None);
        let mut wren = mate("tm_wren");
        wren.name = "Wren".into();
        wren.role = "Code & Migrations".into();
        let mut sable = mate("tm_sable");
        sable.name = "Sable".into();
        sable.role = "Ops & Scheduling".into();
        sable.busy = true;
        let mut s = State::default();
        s.teammates = vec![wren, sable];
        s.seen_live = vec!["m_rail_words_1".into()];
        s.mission_owners = HashMap::from([("m_rail_words_1".to_string(), "tm_wren".to_string())]);
        let stop = Arc::new(AtomicBool::new(false));
        let asking = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let _ = locust_core::native::approvals_channel().ask_browser("m_rail_words_1", "Open a page", "example", &|| stop.load(Ordering::SeqCst));
            })
        };
        let mut rt = rune::Runtime::new(s, |s, _| crate::rail::rail(s, &Palette::new(true)), (72.0, 800.0), rune::Env::new(true, 1.0));
        let mut now = 0.0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_until(&mut rt, &mut now, "wren waiting", |rt| rt.state.bot_stage.desired_activity("rail-tm_wren") == Some(FaceActivity::Waiting));
            assert_eq!(
                rail_words(&rt, "tm_wren"),
                (Some("Message Wren · Code & Migrations".into()), Some("waiting on you".into()))
            );
            assert_eq!(
                rail_words(&rt, "tm_sable"),
                (Some("Message Sable · Ops & Scheduling".into()), Some("working".into()))
            );
        }));
        stop.store(true, Ordering::SeqCst);
        asking.join().unwrap();
        result.unwrap();
    }

    // case: a question alone makes the face wait, in amber, with nothing running
    #[test]
    fn a_question_a_teammate_stopped_on_makes_its_rail_face_wait_in_amber() {
        let mut ada = mate("tm_ada");
        ada.name = "Ada".into();
        ada.role = "Builder".into();
        let mut bo = mate("tm_bo");
        bo.name = "Bo".into();
        bo.role = "Builder".into();
        let mut s = State::default();
        s.teammates = vec![ada.clone(), bo];
        s.conversations = HashMap::from([("tm_ada".to_string(), Conversation { conversation_id: Some("m_q_1".into()), turns: vec![] })]);
        s.mission_owners = HashMap::from([("m_q_1".to_string(), "tm_ada".to_string())]);
        s.ledger = Some(LedgerPane {
            cold_tip: None,
            header: ada,
            conversation: Conversation { conversation_id: Some("m_q_1".into()), turns: vec![] },
            items: vec![ledger::Item::Assistant {
                item_id: "a1".into(),
                text: "<locust-ask>\nCyan or amber?\n- Cyan :: cool\n- Amber :: warm\n</locust-ask>".into(),
                r#final: true,
            }],
            dir: std::path::PathBuf::new(),
            worker: None,
            snapshot: ledger::Snapshot {
                tip: "m_q_1".into(),
                running: false,
                ..Default::default()
            },
            due: 0.0,
            loading: false,
            after_tip: None,
            after_tip_deadline: None,
            backoff: 0.0,
        });
        let mut rt = rune::Runtime::new(s, |s, _| crate::rail::rail(s, &Palette::new(true)), (72.0, 800.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        assert_eq!(rt.state.bot_stage.desired_activity("rail-tm_ada"), Some(FaceActivity::Waiting));
        assert_eq!(crate::look::faces::drawn_presence("rail-tm_ada"), Some(Some(crate::look::bot_stage::PresenceDot::Waiting)));
        assert_eq!(rail_words(&rt, "tm_ada").1.as_deref(), Some("waiting on you"));
        assert_eq!(rt.state.bot_stage.desired_activity("rail-tm_bo"), Some(FaceActivity::Idle));
    }

    // case: last worked just now: her conversation is dated by its newest event, not by when it was sent
    #[test]
    fn a_teammate_who_just_finished_is_dated_by_her_newest_ledger_event_not_when_sent() {
        let (_dir, store) = crate::org::tests::temp_store();
        let ledger = store.write_path("m_last_1").unwrap();
        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        let sent = "2026-10-12T15:00:00.000Z";
        let ended = "2026-10-12T15:02:10.000Z";
        append(&ledger, serde_json::json!({"type": "step.started", "occurredAt": "2026-10-12T15:00:02.000Z", "payload": {"stepKind": "reasoning"}}));
        append(&ledger, serde_json::json!({"type": "tool.started", "occurredAt": "2026-10-12T15:01:30.000Z", "payload": {"itemId": "t1", "name": "shell"}}));
        append(&ledger, serde_json::json!({"type": "run.completed", "occurredAt": ended, "payload": {}}));
        let mut s = State::default();
        s.teammates = vec![mate("wren")];
        s.data = Some(store);
        s.seen_live = vec!["m_last_1".into()];
        s.recent = vec![ConversationRow { updated: sent.into(), ..listed("m_last_1", "running") }];
        s.mission_owners = HashMap::from([("m_last_1".to_string(), "wren".to_string())]);
        let mut rt = rune::Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
        let mut now = 0.0;
        run_until(&mut rt, &mut now, "wren last_at", |rt| last_worked_for(&rt.state, "wren").as_deref() == Some(ended));
        let check_now: chrono::DateTime<chrono::Utc> = "2026-10-12T15:02:20.000Z".parse().unwrap();
        assert_eq!(
            locust_core::home_team_state::last_worked_phrase(last_worked_for(&rt.state, "wren").as_deref(), check_now),
            "last worked just now"
        );
    }
}
