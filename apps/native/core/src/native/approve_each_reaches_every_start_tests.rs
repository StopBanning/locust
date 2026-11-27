//! Approve each reaches every start (Electron `main/approve-each-reaches-every-start.test.ts`).
//!
//! Electron's file reads its own source; here each case is the behavior that source promises. The mode is NAMED, never
//! fallen through to read-only; nothing refuses it for want of a card; the live-turn cap is one pool; and Copilot runs
//! it over ACP in Agent mode with allow-all off, a denial's reason reaching it as its next prompt
//! (`scripts/fixtures/m4-copilot-approval.py reason`).
use super::*;
use crate::approval::{Channel, Decision, Mode, codex_app_server_policy};
use crate::runtime::{Event, Runtime};

/// A runtime that has done its asking: it takes a steer when `takes`, and ends on its first event.
struct Ended {
    takes: bool,
    steered: Arc<Mutex<Vec<String>>>,
}
impl Runtime for Ended {
    fn start(&mut self, _turn: &crate::runtime::Turn) -> io::Result<()> {
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        Ok(Some(Event { kind: "run.completed".into(), payload: json!({}), thread: None }))
    }
    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn can_steer(&self) -> bool {
        self.takes
    }
    fn steer(&mut self, text: &str) -> bool {
        self.steered.lock().unwrap().push(text.to_owned());
        self.takes
    }
}

/// What Copilot asks over ACP, as the fixture asks it.
fn copilot_asks() -> Value {
    json!({"toolCall": {"toolCallId": "t1", "kind": "execute", "title": "Run echo", "rawInput": {"command": "echo hi"}},
        "options": [{"kind": "reject_once", "optionId": "no"}, {"kind": "allow_once", "optionId": "yes"}]})
}

/// A Copilot card in `channel` for `mission`, asked on its own thread; the thread gives what the run was answered.
fn copilot_card(channel: &Arc<Channel>, mission: &str) -> std::thread::JoinHandle<Result<bool, String>> {
    let (ch, m) = (channel.clone(), mission.to_owned());
    let asking = std::thread::spawn(move || ch.ask_acp_runtime("copilot", "run_copilot", &m, &copilot_asks(), "/tmp", &|| false));
    wait("the card", || !channel.waiting_for_mission(mission).is_empty());
    asking
}

/// Drive `runtime` as a turn of `mission` is driven, to its end.
fn drive(t: &Temp, mission: &str, channel: &Channel, runtime: &mut dyn Runtime) {
    let store = t.store();
    let mut writer = LedgerWriter::create_runtime(&store, mission, "Run echo.", "account-default", "copilot", "copilot", &[], None).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let mut registered = Some(Registered(Vec::new(), None, None));
    drive_runtime(runtime, &mut writer, &flag, &mut registered, channel, mission, &store);
}

/// The person's answer to the only card waiting for `mission`.
fn deny_saying(channel: &Channel, mission: &str, reason: Option<&str>) {
    let card = channel.waiting_for_mission(mission).remove(0);
    channel
        .decide(Answer::Decision { approval_id: card.approval_id, decision: Decision::Deny, reason: reason.map(str::to_owned) })
        .unwrap();
}

// ---- approve-each is a mode the mission loop runs, not one it falls through ----

// case: the mode map names it, so it can never land on read-only again
#[test]
fn the_mode_map_names_it_so_it_can_never_land_on_read_only_again() {
    assert_eq!(Mode::parse("approve-each"), Some(Mode::ApproveEach));
    // Named beside accept-edits: approvals only mean something when the run could otherwise act.
    assert_eq!(Mode::ApproveEach.sandbox(), Mode::AcceptEdits.sandbox());
    assert_eq!(Mode::ApproveEach.sandbox(), "workspace-write");
    assert_eq!(codex_app_server_policy(Mode::ApproveEach, None).sandbox, "workspace-write");
    // And never widened past the folder either.
    assert_eq!(codex_app_server_policy(Mode::ApproveEach, Some("full-access")).sandbox, "workspace-write");
}

// case: the policy asks the server to stop, which is what makes a card appear
#[test]
fn the_policy_asks_the_server_to_stop_which_is_what_makes_a_card_appear() {
    assert_eq!(codex_app_server_policy(Mode::ApproveEach, None).approval_policy, "untrusted");
    for mode in [Mode::Ask, Mode::AcceptEdits, Mode::Plan, Mode::Auto] {
        assert_eq!(codex_app_server_policy(mode, None).approval_policy, "never", "{mode:?}");
    }
}

// case: no start path refuses it any more
#[test]
fn no_start_path_refuses_it_any_more() {
    // Every way in names it as a mode to run: the engine's own list, and every runtime that can stop and ask.
    assert!(MODES.contains(&"approve-each"));
    for runtime in ["codex", "claude", "opencode", "copilot"] {
        assert_eq!(crate::mode_facts::unavailable_reason("approve-each", runtime), None, "{runtime}");
    }
    // A scheduled routine runs it, and says where it is set that it waits on its first card; it is not refused.
    let routine = json!({"schedule": {"every": "day"}, "route": {"mode": "approve-each"}});
    assert_eq!(
        crate::routines::waits_for_you(&routine),
        Some("Runs in Approve each: fired while you are away, it waits for you on its first card. Ask or Accept edits runs through.")
    );
}

// case: a run that stops to ask is answered by the channel, and released when it ends
#[test]
fn a_run_that_stops_to_ask_is_answered_by_the_channel_and_released_when_it_ends() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), None);
    let channel = Channel::new();
    // Answered by the channel: a denial reaches the run as a denial.
    let asking = copilot_card(&channel, "mission_answered");
    deny_saying(&channel, "mission_answered", None);
    assert_eq!(asking.join().unwrap(), Ok(false));
    // Released when it ends: a card still waiting is refused, and none is left waiting on a run that has gone.
    let asking = copilot_card(&channel, "mission_released");
    let steered = Arc::new(Mutex::new(Vec::new()));
    drive(&t, "mission_released", &channel, &mut Ended { takes: true, steered });
    assert_ne!(asking.join().unwrap(), Ok(true));
    assert!(channel.waiting_for_mission("mission_released").is_empty());
}

// case: one host answers a card, whichever asked for it
#[test]
fn one_host_answers_a_card_whichever_asked_for_it() {
    // One channel for the process: the window answers every card through it.
    assert!(Arc::ptr_eq(&approvals_channel(), &approvals_channel()));
    let channel = Channel::new();
    let copilot = copilot_card(&channel, "mission_copilot");
    let codex = {
        let ch = channel.clone();
        std::thread::spawn(move || {
            ch.ask_codex_with_changes("run_codex", "mission_codex", "codex", "item/commandExecution/requestApproval",
                &json!({"itemId": "c1", "command": "echo hi", "cwd": "/tmp"}), json!(1), None, &|| false)
        })
    };
    wait("both cards", || channel.waiting().len() == 2);
    let ids: Vec<String> = channel.waiting().into_iter().map(|c| c.approval_id).collect();
    assert_ne!(ids[0], ids[1], "an id is minted by exactly one ask");
    for id in &ids {
        channel.decide(Answer::Decision { approval_id: id.clone(), decision: Decision::ApproveOnce, reason: None }).unwrap();
        // Claimed once: the same answer again finds nothing waiting.
        assert!(channel.decide(Answer::Decision { approval_id: id.clone(), decision: Decision::ApproveOnce, reason: None }).is_err());
    }
    assert_eq!(copilot.join().unwrap(), Ok(true));
    assert_eq!(codex.join().unwrap().unwrap()["decision"], "accept");
}

// ---- the live-mission cap is one pool across the transports ----

// case: both transports carry the same bound
#[test]
fn both_transports_carry_the_same_bound() {
    // shared/live-missions.ts MAX_LIVE_MISSIONS, and a room is held under the same number.
    assert_eq!(MAX_LIVE_TURNS, 8);
    assert_eq!(crate::room_store::MAX_ROOM_TEAMMATES, MAX_LIVE_TURNS);
    assert_eq!(too_many_live(), "Up to 8 missions can run at once. Wait for one to finish or stop it first.");
}

// case: each one counts the other before admitting a start
#[test]
fn each_one_counts_the_other_before_admitting_a_start() {
    // One map holds every turn, whichever runtime runs it; a turn registered under its conversation and its own id
    // counts once.
    let (codex, antigravity, copilot) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let mut running = HashMap::new();
    running.insert("conv_codex".to_owned(), codex.clone());
    running.insert("mission_codex".to_owned(), codex);
    running.insert("mission_antigravity".to_owned(), antigravity);
    running.insert("mission_copilot".to_owned(), copilot);
    assert_eq!(live_turns(&running), 3);
}

// case: the host wires each service to the other
#[test]
fn the_host_wires_each_service_to_the_other() {
    // Eight turns live on any runtime refuse the ninth, whichever runtime it is for, before anything is recorded.
    let t = Temp::new();
    let _hook = hooks::install(t.store(), fake("success"));
    let flags: Vec<_> = (0..MAX_LIVE_TURNS).map(|_| Arc::new(AtomicBool::new(false))).collect();
    with_running(|m| {
        for (n, flag) in flags.iter().enumerate() {
            m.insert(format!("mission_live_{n}"), flag.clone());
        }
    });
    let copilot = Teammate { id: "tm_cope".into(), name: "Cope".into(), runtime: Some("copilot".into()), runs_in: "approve-each".into(), ..codex(&t) };
    let refused = send(Send { teammate: copilot, ..job(&t, None, "Run echo.") });
    let codex_refused = send(job(&t, None, "Run echo."));
    with_running(|m| (0..MAX_LIVE_TURNS).for_each(|n| drop(m.remove(&format!("mission_live_{n}")))));
    assert_eq!(refused.unwrap_err(), too_many_live());
    assert_eq!(codex_refused.unwrap_err(), too_many_live());
    assert!(ledgers(&t).is_empty(), "nothing was started");
}

// ---- Copilot runs Approve-each over ACP, held to the mode that asks ----

/// Copilot in Approve each, on the fake that checks Agent mode and allow-all off before it is prompted.
fn copilot_run(t: &Temp, scenario: &str, record: &Path) -> String {
    *crate::invariants::lock(&hooks::STATE) = (
        Some(t.store()),
        Some((
            PathBuf::from("python3"),
            vec![
                concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m4-copilot-approval.py").into(),
                scenario.into(),
                "--acp".into(),
                record.to_string_lossy().into_owned(),
            ],
        )),
    );
    let copilot = Teammate { id: "tm_cope".into(), name: "Cope".into(), runtime: Some("copilot".into()), runs_in: "approve-each".into(), model: None, ..codex(t) };
    send(Send { teammate: copilot, model: None, ..job(t, None, "Run echo.") }).unwrap().conversation_id
}

/// The card the run is waiting on, once it is.
fn card_for(id: &str) -> crate::approval::Request {
    wait("Copilot's card", || !waiting_approvals_for(id).is_empty());
    waiting_approvals_for(id).remove(0)
}

// case: rides ACP in Approve-each only, with Agent mode and allow-all off
#[test]
fn rides_acp_in_approve_each_only_with_agent_mode_and_allow_all_off() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), None);
    // The fake answers a prompt only once Agent mode is set and allow-all is off, and only over ACP.
    let id = copilot_run(&t, "deny", &t.0.join("unused.json"));
    let card = card_for(&id);
    decide_approval(&card.approval_id, Decision::Deny, None).unwrap();
    assert!(ended(&t, &id).contains(&"run.completed".to_owned()), "{:?}", kinds(&path_of(&t, &id)));
    // Over ACP it takes a message as its next prompt; print mode takes none.
    let acp = crate::runtime::copilot::Copilot::spawn(Command::new("true").arg("--acp")).unwrap();
    assert!(acp.can_steer());
    let mut print = crate::runtime::copilot::Copilot::spawn(&mut Command::new("true")).unwrap();
    assert!(!print.can_steer());
    assert!(!print.steer("I declined that. Not in this folder."));
    // Approve each alone rides it: Accept edits is Copilot's print mode, which cannot stop and ask.
    assert!(crate::runtime::copilot::prompt_args("Run echo.", Mode::AcceptEdits, None, None, None, None, &[], None)
        .iter()
        .all(|a| a != "--acp"));
}

// case: what it asks is the card every runtime uses, named as Copilot, and a reason reaches it as its next prompt
#[test]
fn what_it_asks_is_the_card_every_runtime_uses_named_as_copilot_and_a_reason_reaches_it_as_its_next_prompt() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), None);
    let record = t.0.join("next-prompt.json");
    let id = copilot_run(&t, "reason", &record);
    let card = card_for(&id);
    assert_eq!(card.runtime, "copilot");
    assert_eq!(card.kind, crate::approval::Kind::Command);
    decide_approval(&card.approval_id, Decision::Deny, Some("Not in this folder.".into())).unwrap();
    assert!(ended(&t, &id).contains(&"run.completed".to_owned()), "{:?}", kinds(&path_of(&t, &id)));
    let next: Value = serde_json::from_str(&fs::read_to_string(&record).expect("Copilot was prompted again")).unwrap();
    assert_eq!(next["prompt"], json!([{"type": "text", "text": "I declined that. Not in this folder."}]));
}

#[test]
fn a_reason_the_run_can_no_longer_take_is_said_so_and_a_plain_denial_says_nothing() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), None);
    let channel = Channel::new();
    // Taken: steered once, in Electron's words.
    let asking = copilot_card(&channel, "mission_takes");
    deny_saying(&channel, "mission_takes", Some("Not in this folder."));
    assert_eq!(asking.join().unwrap(), Ok(false));
    let steered = Arc::new(Mutex::new(Vec::new()));
    drive(&t, "mission_takes", &channel, &mut Ended { takes: true, steered: steered.clone() });
    assert_eq!(*steered.lock().unwrap(), ["I declined that. Not in this folder."]);
    assert!(relay_live_state(&["mission_takes".into()]).notices.is_empty());
    // Not taken: the person is told, as a warning.
    let asking = copilot_card(&channel, "mission_gone");
    deny_saying(&channel, "mission_gone", Some("Not in this folder."));
    assert_eq!(asking.join().unwrap(), Ok(false));
    drive(&t, "mission_gone", &channel, &mut Ended { takes: false, steered: Arc::new(Mutex::new(Vec::new())) });
    let notices = relay_live_state(&["mission_gone".into()]).notices;
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].message,
        "Your reason was not passed on: the run could not take a message just then. Say it in your next message."
    );
    assert_eq!(notices[0].level, "warning");
    // A denial with no reason steers nothing and says nothing.
    let asking = copilot_card(&channel, "mission_plain");
    deny_saying(&channel, "mission_plain", None);
    assert_eq!(asking.join().unwrap(), Ok(false));
    let steered = Arc::new(Mutex::new(Vec::new()));
    drive(&t, "mission_plain", &channel, &mut Ended { takes: false, steered: steered.clone() });
    assert!(steered.lock().unwrap().is_empty());
    assert!(relay_live_state(&["mission_plain".into()]).notices.is_empty());
}

// case: the host starts the agent IN its folder, and admits the mode for Copilot
#[test]
fn the_host_starts_the_agent_in_its_folder_and_admits_the_mode_for_copilot() {
    let t = Temp::new();
    let _hook = hooks::install(t.store(), None);
    let record = t.0.join("next-prompt.json");
    let id = copilot_run(&t, "reason", &record);
    decide_approval(&card_for(&id).approval_id, Decision::Deny, Some("Somewhere else.".into())).unwrap();
    finished(&id);
    let next: Value = serde_json::from_str(&fs::read_to_string(&record).unwrap()).unwrap();
    assert_eq!(fs::canonicalize(next["cwd"].as_str().unwrap()).unwrap(), fs::canonicalize(t.0.join("work")).unwrap());
    // Codex, OpenCode and Copilot run it; an agent that cannot stop and ask is told so.
    for runtime in ["codex", "opencode", "copilot"] {
        assert_eq!(crate::mode_facts::unavailable_reason("approve-each", runtime), None, "{runtime}");
    }
    assert_eq!(
        crate::mode_facts::unavailable_reason("approve-each", "cursor").as_deref(),
        Some("Claude Code, Codex CLI, OpenCode, Copilot CLI and Antigravity only. Cursor Agent cannot stop and ask yet.")
    );
}

fn ended(t: &Temp, id: &str) -> Vec<String> {
    finished(id);
    kinds(&path_of(t, id))
}
