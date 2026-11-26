//! `apps/desktop/src/main/relay.test.ts`, one test per case, against Electron's own fixtures: Wren (Code & Migrations,
//! no route of their own) shares from a Cursor / composer-2.5 run that may edit; Booty (Custom) answers on Claude Code /
//! sonnet, read-only. Every expected word and number below is the one Electron's test expects.

use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub(super) fn tm(id: &str, name: &str, role: &str, route: Option<TurnRoute>) -> PeerRosterEntry {
    PeerRosterEntry {
        teammate_id: id.to_owned(),
        name: name.to_owned(),
        role: role.to_owned(),
        route,
    }
}

pub(super) fn route(runtime: &str, model: &str, mode: &str) -> TurnRoute {
    TurnRoute {
        runtime: runtime.to_owned(),
        model: Some(model.to_owned()),
        effort: None,
        account: None,
        mode: mode.to_owned(),
    }
}

fn wren() -> PeerRosterEntry {
    tm("tm_wren", "Wren", "Code & Migrations", None)
}

fn booty() -> PeerRosterEntry {
    tm("tm_booty", "Booty", "Custom", Some(route("claude", "sonnet", "ask")))
}

/// Booty before anyone has run them: no route of their own.
fn new_booty() -> PeerRosterEntry {
    tm("tm_booty", "Booty", "Custom", None)
}

fn atlas() -> PeerRosterEntry {
    tm("tm_atlas", "Atlas", "Research & Briefs", Some(route("codex", "account-default", "ask")))
}

fn juno() -> PeerRosterEntry {
    tm("tm_juno", "Juno", "Custom", None)
}

fn busy() -> Option<Result<StartResult, String>> {
    Some(Ok(StartResult::Busy { pool: false }))
}

fn pool_full() -> Option<Result<StartResult, String>> {
    Some(Ok(StartResult::Busy { pool: true }))
}

fn refused(message: &str) -> Option<Result<StartResult, String>> {
    Some(Ok(StartResult::Refused {
        message: message.to_owned(),
    }))
}

pub(super) struct HostState {
    pub enabled: bool,
    pub cap: usize,
    pub peers: HashMap<String, PeerRosterEntry>,
    pub roster_error: Option<String>,
    pub exchange_of: HashMap<String, String>,
    /// Answers by attempt, so a recipient can be busy on the first and free on the next.
    pub start_results: Vec<Option<Result<StartResult, String>>>,
    pub start_result: Option<Result<StartResult, String>>,
    /// The meeting harness's mission ids: `mission_<name>_<count>`.
    pub named_missions: bool,
    /// A start takes time: the window every overlap lived in.
    pub start_delay_ms: u64,
    pub may_interrupt: bool,
    pub stop_works: bool,
    /// Whether the recipient's running turn takes a message at its next step; None: no steering at all.
    pub steers: Option<bool>,
    pub cursor_holds_read_only: bool,
    pub out_of_plan: Option<String>,
    pub out_of_plan_asked: Vec<(String, Option<String>)>,
    pub still_waiting: Option<bool>,
    pub asked: Vec<String>,
    pub final_reply: Option<String>,
    pub returned: usize,
    pub starts: Vec<StartRequest>,
    pub owners: Vec<(String, String)>,
    pub updates: Vec<RelayUpdate>,
    pub stopped: Vec<String>,
    pub steered: Vec<(String, String)>,
    pub kept: Vec<(String, String)>,
}

impl Default for HostState {
    fn default() -> Self {
        let mut peers = HashMap::new();
        peers.insert("tm_wren".to_owned(), wren());
        peers.insert("tm_booty".to_owned(), booty());
        Self {
            enabled: true,
            cap: MAX_RELAY_HOPS,
            peers,
            roster_error: None,
            exchange_of: HashMap::new(),
            start_results: Vec::new(),
            start_result: None,
            named_missions: false,
            start_delay_ms: 0,
            may_interrupt: false,
            stop_works: true,
            steers: None,
            cursor_holds_read_only: true,
            out_of_plan: None,
            out_of_plan_asked: Vec::new(),
            still_waiting: None,
            asked: Vec::new(),
            final_reply: None,
            returned: 0,
            starts: Vec::new(),
            owners: Vec::new(),
            updates: Vec::new(),
            stopped: Vec::new(),
            steered: Vec::new(),
            kept: Vec::new(),
        }
    }
}

pub(super) struct Host {
    pub s: Mutex<HostState>,
}

impl RelayHost for Host {
    fn enabled(&self) -> bool {
        self.s.lock().unwrap().enabled
    }
    fn hop_cap(&self) -> usize {
        self.s.lock().unwrap().cap
    }
    fn exchange_of(&self, mission_id: &str) -> Option<String> {
        self.s.lock().unwrap().exchange_of.get(mission_id).cloned()
    }
    fn peer_context_for(&self, teammate_id: &str) -> Option<PeerRosterEntry> {
        self.s.lock().unwrap().peers.get(teammate_id).cloned()
    }
    fn peer_context_read(&self, teammate_id: &str) -> Result<Option<PeerRosterEntry>, String> {
        if let Some(why) = self.s.lock().unwrap().roster_error.clone() {
            return Err(why);
        }
        Ok(self.peer_context_for(teammate_id))
    }
    fn final_reply_of(&self, _mission_id: &str) -> Option<String> {
        self.s.lock().unwrap().final_reply.clone()
    }
    fn post_returned_answer(&self, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str) -> Option<String> {
        let mut s = self.s.lock().unwrap();
        s.returned += 1;
        Some("wm_returned".to_owned())
    }
    fn cursor_holds_read_only(&self) -> bool {
        self.s.lock().unwrap().cursor_holds_read_only
    }
    fn out_of_plan(&self, runtime: &str, account: Option<&str>) -> Option<String> {
        let mut s = self.s.lock().unwrap();
        s.out_of_plan_asked.push((runtime.to_owned(), account.map(str::to_owned)));
        s.out_of_plan.clone()
    }
    fn start(&self, req: StartRequest) -> Result<StartResult, String> {
        let (n, delay, name) = {
            let mut s = self.s.lock().unwrap();
            let name = req.peer.name.to_lowercase();
            s.starts.push(req);
            (s.starts.len(), s.start_delay_ms, name)
        };
        if delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
        let s = self.s.lock().unwrap();
        if let Some(Some(answer)) = s.start_results.get(n - 1) {
            return answer.clone();
        }
        if let Some(answer) = &s.start_result {
            return answer.clone();
        }
        let mission_id = if s.named_missions {
            format!("mission_{name}_{n}")
        } else {
            format!("mission_{n}")
        };
        Ok(StartResult::Started {
            run_id: format!("run_{n}"),
            mission_id,
        })
    }
    fn assign_owner(&self, teammate_id: &str, mission_id: &str) {
        self.s
            .lock()
            .unwrap()
            .owners
            .push((teammate_id.to_owned(), mission_id.to_owned()));
    }
    fn still_waiting(&self, teammate_id: &str) -> Option<bool> {
        let mut s = self.s.lock().unwrap();
        let answer = s.still_waiting?;
        s.asked.push(teammate_id.to_owned());
        Some(answer)
    }
    fn may_interrupt(&self) -> bool {
        self.s.lock().unwrap().may_interrupt
    }
    fn stop_work_of(&self, teammate_id: &str) -> bool {
        let mut s = self.s.lock().unwrap();
        s.stopped.push(teammate_id.to_owned());
        s.stop_works
    }
    fn steer_work_of(&self, teammate_id: &str, text: &str) -> bool {
        let mut s = self.s.lock().unwrap();
        let Some(steers) = s.steers else { return false };
        s.steered.push((teammate_id.to_owned(), text.to_owned()));
        steers
    }
    fn notify(&self, update: RelayUpdate) {
        self.s.lock().unwrap().updates.push(update);
    }
    fn note(&self, mission_id: &str, message: &str) {
        self.s
            .lock()
            .unwrap()
            .kept
            .push((mission_id.to_owned(), message.to_owned()));
    }
}

pub(super) fn harness(set: impl FnOnce(&mut HostState)) -> Relay<Host> {
    let mut s = HostState::default();
    set(&mut s);
    Relay::new(Host { s: Mutex::new(s) })
}

pub(super) fn st<H: RelayHost>(relay: &Relay<H>) -> std::sync::MutexGuard<'_, HostState>
where
    H: AsHost,
{
    relay.host().state()
}

pub(super) trait AsHost {
    fn state(&self) -> std::sync::MutexGuard<'_, HostState>;
}

impl AsHost for Host {
    fn state(&self) -> std::sync::MutexGuard<'_, HostState> {
        self.s.lock().unwrap()
    }
}

/// What the thread that shared was told, in order.
pub(super) fn said(relay: &Relay<Host>) -> Vec<String> {
    st(relay)
        .updates
        .iter()
        .filter_map(|u| match u {
            RelayUpdate::Notice { message, .. } => Some(message.clone()),
            _ => None,
        })
        .collect()
}

fn notices(relay: &Relay<Host>) -> Vec<(String, String, String, RelayNoticeLevel)> {
    st(relay)
        .updates
        .iter()
        .filter_map(|u| match u {
            RelayUpdate::Notice {
                run_id,
                mission_id,
                message,
                level,
            } => Some((run_id.clone(), mission_id.clone(), message.clone(), *level)),
            _ => None,
        })
        .collect()
}

fn starts(relay: &Relay<Host>) -> Vec<StartRequest> {
    st(relay).starts.clone()
}

fn last_of(origin: &RelayOrigin) -> Vec<(&str, &str)> {
    origin
        .last_mission_of
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect()
}

fn message(to: &PeerRosterEntry, text: &str) -> RelayedMessage {
    RelayedMessage {
        message_id: format!("msg_{}", to.teammate_id),
        from_teammate_id: "tm_wren".to_owned(),
        from_name: "Wren".to_owned(),
        from_mission_id: "mission_wren1".to_owned(),
        to_teammate_id: to.teammate_id.clone(),
        to_name: to.name.clone(),
        text: text.to_owned(),
        urgent: false,
        defer: false,
        wants_answer: false,
    }
}

fn ask(to: &PeerRosterEntry) -> RelayedMessage {
    message(to, "What is the build command?")
}

fn urgent(to: &PeerRosterEntry) -> RelayedMessage {
    RelayedMessage {
        urgent: true,
        ..ask(to)
    }
}

fn from(sender: &PeerRosterEntry, mission_id: &str, to: &PeerRosterEntry, text: &str) -> RelayedMessage {
    RelayedMessage {
        from_teammate_id: sender.teammate_id.clone(),
        from_name: sender.name.clone(),
        from_mission_id: mission_id.to_owned(),
        ..message(to, text)
    }
}

fn sharing() -> SharingMission {
    SharingMission {
        run_id: "run_wren1".to_owned(),
        mission_id: "mission_wren1".to_owned(),
        runtime: "cursor".to_owned(),
        sandbox: "workspace-write".to_owned(),
        model: Some("composer-2.5".to_owned()),
        peer: wren(),
        relay: None,
        started_by: None,
    }
}

fn origin(hop: usize, last: &[(&str, &str)]) -> RelayOrigin {
    RelayOrigin {
        hop,
        root_mission_id: None,
        last_mission_of: last.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect(),
        relayed_to: Vec::new(),
        answering: Vec::new(),
    }
}

/// A run ending, as the service reports it.
fn ended(mission_id: &str, peer: Option<PeerRosterEntry>) -> EndedMission {
    EndedMission {
        mission_id: mission_id.to_owned(),
        peer,
        relay: None,
    }
}

fn decide(enabled: bool, hop: usize, cap: Option<usize>, spent: Option<usize>) -> RelayDecision {
    decide_relay(DecideRelayInput {
        enabled,
        defer: false,
        closes: false,
        hop,
        recipient_name: "Booty",
        cap,
        spent,
    })
}

fn reason(decision: RelayDecision) -> String {
    match decision {
        RelayDecision::Skip { reason } => reason,
        RelayDecision::Start { .. } => String::new(),
    }
}

fn brief(sender: &PeerRosterEntry, recipient: &PeerRosterEntry, hop: usize, cap: Option<usize>, read_by_person: bool) -> String {
    relay_prompt(&RelayPromptInput {
        sender,
        recipient,
        hop,
        cap,
        read_by_person,
        reply_number: None,
        briefed_before: false,
    })
}

fn minutes(repliers: &[&str], silent: &[&str]) -> String {
    let r: Vec<String> = repliers.iter().map(|s| (*s).to_owned()).collect();
    let q: Vec<String> = silent.iter().map(|s| (*s).to_owned()).collect();
    meeting_prompt(&r, &q, false)
}

// ---------------------------------------------------------------------------------------------------------------------
// deciding whether a teammate replies on their own
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn when_switched_off_says_the_message_waits() {
    let decision = decide(false, 0, None, None);
    assert!(matches!(decision, RelayDecision::Skip { .. }));
    assert!(reason(decision).contains("switched off in Settings"));
}

#[test]
fn starts_the_first_hop_when_switched_on() {
    assert_eq!(decide(true, 0, None, None), RelayDecision::Start { hop: 1 });
}

#[test]
fn stops_at_the_cap_naming_who_will_see_the_message_next() {
    let decision = decide(true, MAX_RELAY_HOPS, None, None);
    assert!(matches!(decision, RelayDecision::Skip { .. }));
    let why = reason(decision);
    assert!(why.contains("Booty"));
    assert!(why.contains(&MAX_RELAY_HOPS.to_string()));
}

#[test]
fn the_cap_is_a_backstop_not_a_conversation_length() {
    assert_eq!(MAX_RELAY_HOPS, 12);
}

// case: takes the personu2019s own budget over the constant, and says the number it stopped at
#[test]
fn takes_the_persons_own_budget_over_the_constant_and_says_the_number_it_stopped_at() {
    assert!(matches!(decide(true, 2, Some(2), None), RelayDecision::Skip { .. }));
    assert!(reason(decide(true, 2, Some(2), None)).contains("Stopped after 2 automatic replies"));
    assert_eq!(decide(true, 1, Some(2), None), RelayDecision::Start { hop: 2 });
    assert!(reason(decide(true, 1, Some(1), None)).contains("after 1 automatic reply."));
}

// ---------------------------------------------------------------------------------------------------------------------
// the brief a relayed run is started with
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn names_the_sender_shows_how_to_write_back_and_forbids_unrelated_work() {
    let prompt = brief(&wren(), &booty(), 1, None, false);
    assert!(prompt.contains("Wren (Code & Migrations) sent you a message"));
    assert!(prompt.contains("<locust-share to=\"Wren\">"));
    assert!(prompt.contains("Stay on what was asked"));
}

#[test]
fn lets_the_shape_the_sender_asked_for_win_over_complete_sentences() {
    let prompt = brief(&wren(), &booty(), 1, None, false);
    let ordinary = prompt.find("in complete sentences").unwrap();
    let shape = prompt.find("If Wren said what shape the reply should take").unwrap();
    assert!(ordinary > 0);
    assert!(shape > ordinary);
    assert!(prompt.contains(
        "one word, a number, a list -- give it in that shape inside the share block; that shape wins"
    ));
}

#[test]
fn every_hop_is_told_that_silence_is_how_an_exchange_finishes() {
    for hop in [1, 2, 5] {
        let prompt = brief(&booty(), &wren(), hop, None, false);
        assert!(prompt.contains("When a reply is needed, end with one"), "hop {hop}");
        assert!(prompt.contains("end with no share block"), "hop {hop}");
    }
    assert!(brief(&booty(), &wren(), 2, None, false).contains("Booty (Custom) replied to you"));
}

#[test]
fn tells_a_relayed_run_to_ask_the_teammate_not_a_person() {
    let prompt = brief(&wren(), &booty(), 1, None, false);
    assert!(prompt.contains("The person is not in this exchange"));
    assert!(prompt.contains("ask Wren inside that share block"));
    assert!(prompt.contains("A <locust-ask> block reaches only a person"));
}

#[test]
fn tells_the_reply_that_lands_in_the_persons_own_conversation_to_say_what_it_means_for_them() {
    let prompt = brief(&booty(), &wren(), 2, None, true);
    assert!(prompt.contains("the person who started this conversation reads it"));
    assert!(prompt.contains("say in a line or two what Booty's reply means"));
    assert!(!prompt.contains("reach nobody and cost a run each"));
    assert!(!prompt.contains("nobody is watching this run"));
    assert!(prompt.contains("a <locust-ask> block reaches them here"));
    // The peer's side of the same exchange is still nobody's conversation.
    let peer = brief(&wren(), &booty(), 3, None, false);
    assert!(peer.contains("reach nobody and cost a run each"));
    // And a first message is never "read by the person", whoever it is for.
    assert!(!brief(&wren(), &booty(), 1, None, true).contains("reads it"));
}

#[test]
fn says_the_same_after_a_meeting_where_several_teammates_could_be_asked() {
    let prompt = minutes(&["Atlas", "Juno"], &[]);
    assert!(prompt.contains("There is no person in this exchange"));
    assert!(prompt.contains("never in a <locust-ask> block"));
}

// ---------------------------------------------------------------------------------------------------------------------
// relaying a share
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn keeps_mcp_shares_visible_without_starts_steering_interruptions_or_deferred_handoffs_j() {
    for answer in [None, busy()] {
        let relay = harness(|s| {
            s.peers.insert(
                "tm_booty".into(),
                tm("tm_booty", "Booty", "Custom", Some(route("claude", "sonnet", "auto"))),
            );
            s.may_interrupt = true;
            s.steers = Some(true);
            s.start_result = answer.clone();
        });
        let posted = vec![RelayedMessage {
            urgent: true,
            wants_answer: true,
            ..ask(&booty())
        }];
        let before = posted.clone();
        relay.on_shared(
            &SharingMission {
                sandbox: "read-only".into(),
                started_by: Some(MissionStarter::Mcp),
                ..sharing()
            },
            &posted,
        );
        relay.on_run_ended(&ended("mission_wren1", Some(wren())));
        relay.on_run_ended(&ended("mission_booty", Some(booty())));
        let s = st(&relay);
        assert!(s.starts.is_empty());
        assert!(s.owners.is_empty());
        assert!(s.stopped.is_empty());
        assert!(s.steered.is_empty());
        assert_eq!(posted, before);
        let text = "Shares from another app are visible in Locust. No teammate was started automatically.";
        assert_eq!(s.kept, vec![("mission_wren1".to_owned(), text.to_owned())]);
        assert_eq!(s.updates.len(), 1);
        assert!(matches!(&s.updates[0], RelayUpdate::Notice { message, .. } if message == text));
    }
}

#[test]
fn starts_nothing_when_off_and_says_so_in_the_thread() {
    let relay = harness(|s| s.enabled = false);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let s = st(&relay);
    assert!(s.starts.is_empty());
    assert_eq!(s.updates.len(), 1);
    assert!(matches!(&s.updates[0], RelayUpdate::Notice { message, .. } if message.contains("switched off in Settings")));
}

#[test]
fn turns_a_failure_between_the_decision_and_the_start_into_a_notice_never_silence() {
    let relay = harness(|s| s.roster_error = Some("the roster could not be read.".into()));
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let s = st(&relay);
    assert!(s.starts.is_empty());
    assert_eq!(s.updates.len(), 1);
    assert!(matches!(&s.updates[0], RelayUpdate::Notice { message, .. }
        if message.contains("Booty could not reply on their own: the roster could not be read")));
}

#[test]
fn starts_no_reply_on_a_plan_that_is_used_up_and_says_so_where_the_share_was_made_0_862() {
    let relay = harness(|s| s.out_of_plan = Some("their Claude Code plan is used up until Mon, Nov 9, 6:52 PM.".into()));
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let told = said(&relay);
    let s = st(&relay);
    assert!(s.starts.is_empty());
    assert!(s.owners.is_empty());
    assert_eq!(s.out_of_plan_asked, vec![("claude".to_owned(), None)]);
    assert_eq!(
        told,
        vec!["Booty did not reply on their own: their Claude Code plan is used up until Mon, Nov 9, 6:52 PM. The message waits for their next run. To have it answered sooner, give Booty another model on the Team screen.".to_owned()]
    );
    // Kept in the sender's own record too, so reopening the thread later still says why.
    assert_eq!(s.kept.iter().map(|(_, m)| m.clone()).collect::<Vec<_>>(), told);
    assert!(!s.updates.iter().any(|u| matches!(u, RelayUpdate::Starting { .. })));
}

/// The Rust host answers a plan check with a reason or nothing: a check that could not be made is nothing
/// (`native.rs` `out_of_plan` reads `limits::limited_runtimes`, which leaves out what it cannot read).
#[test]
fn a_plan_that_can_take_the_run_is_started_as_before_and_a_check_that_fails_never_blocks_it() {
    let fine = harness(|s| s.out_of_plan = None);
    fine.on_shared(&sharing(), &[ask(&booty())]);
    assert_eq!(starts(&fine).len(), 1);
    assert_eq!(st(&fine).out_of_plan_asked.len(), 1);
    let dir = std::env::temp_dir().join(format!("locust-relay-plan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mission_broken.jsonl"), "{ not a ledger\n").unwrap();
    let broken = harness(|s| {
        s.out_of_plan = crate::limits::limited_runtimes_in_dir(&dir, 0).get("claude").cloned();
    });
    broken.on_shared(&sharing(), &[ask(&booty())]);
    assert_eq!(starts(&broken).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

// case: starts the recipient's run on the recipient's OWN route, owned by the recipient
#[test]
fn starts_the_recipients_run_on_the_recipients_own_route_owned_by_the_recipient() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let s = st(&relay);
    assert_eq!(s.starts.len(), 1);
    let start = &s.starts[0];
    assert_eq!(start.runtime, "claude");
    assert_eq!(start.mode, "ask");
    assert_eq!(start.model.as_deref(), Some("sonnet"));
    assert_eq!(start.peer, booty());
    assert_eq!(start.follow_up_of, None);
    assert_eq!(start.relay.hop, 1);
    assert_eq!(last_of(&start.relay), vec![("tm_wren", "mission_wren1")]);
    assert!(!s.updates.iter().any(|u| matches!(u, RelayUpdate::Notice { .. })));
    assert!(start.prompt.contains("Wren (Code & Migrations) sent you a message"));
    assert_eq!(s.owners, vec![("tm_booty".to_owned(), "mission_1".to_owned())]);
    assert!(s.updates.iter().any(|u| matches!(u,
        RelayUpdate::MissionStarted { teammate_id, started_by_hop: 1, .. } if teammate_id == "tm_booty")));
}

#[test]
fn says_so_when_a_one_to_one_reply_never_comes_back_and_where_it_went() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert_eq!(starts(&relay).len(), 1);
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_1".into(),
        peer: Some(booty()),
        relay: Some(origin(1, &[])),
    });
    let last = notices(&relay).pop().unwrap();
    assert_eq!(
        last.2,
        "Booty finished without writing back. Anything they said is in their own conversation."
    );
    // Addressed to the thread that ASKED, not to the run that stayed quiet.
    assert_eq!(last.1, "mission_wren1");
}

// case: stays quiet when the reply DID come back
#[test]
fn stays_quiet_when_the_reply_did_come_back() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&booty())]);
    relay.on_shared(
        &SharingMission {
            run_id: "run_booty".into(),
            mission_id: "mission_1".into(),
            peer: booty(),
            relay: Some(origin(1, &[])),
            runtime: "claude".into(),
            sandbox: "read-only".into(),
            model: Some("sonnet".into()),
            started_by: None,
        },
        &[ask(&wren())],
    );
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_1".into(),
        peer: Some(booty()),
        relay: Some(origin(1, &[])),
    });
    assert!(!said(&relay).contains(
        &"Booty finished without writing back. Anything they said is in their own conversation.".to_owned()
    ));
}

#[test]
fn a_teammate_who_has_never_run_borrows_the_senders_route_and_the_thread_says_so() {
    let relay = harness(|s| {
        s.peers.insert("tm_booty".into(), new_booty());
    });
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let start = starts(&relay).remove(0);
    assert_eq!(start.runtime, "cursor");
    assert_eq!(start.model.as_deref(), Some("composer-2.5"));
    assert_eq!(start.mode, "accept-edits");
    let (_, _, said, level) = notices(&relay).remove(0);
    assert!(said.contains("has no model of their own yet"));
    // The runtime by name, never the raw model id, and as information (0.366).
    assert!(said.contains("(Cursor Agent)"));
    assert!(!said.contains("composer-2.5"));
    assert_eq!(level, RelayNoticeLevel::Info);
    assert!(said.contains("may edit this folder"));
}

#[test]
fn says_read_only_when_that_is_what_the_senders_route_lends() {
    let relay = harness(|s| {
        s.peers.insert("tm_booty".into(), new_booty());
    });
    relay.on_shared(
        &SharingMission {
            sandbox: "full-access".into(),
            ..sharing()
        },
        &[ask(&booty())],
    );
    assert_eq!(starts(&relay)[0].mode, "ask");
    assert!(said(&relay)[0].contains("read-only"));
}

#[test]
fn tells_a_read_only_reply_that_a_command_is_refused_and_the_message_is_quoted_0_592_and_says_nothing_of_it_to_one_that_may_edit() {
    let ro = harness(|_| {});
    ro.on_shared(&sharing(), &[ask(&booty())]);
    let start = starts(&ro).remove(0);
    assert_eq!(start.mode, "ask");
    assert!(start.prompt.contains(
        "You are replying in a read-only mode: a shell command is refused here and ends this run with nothing written."
    ));
    assert!(start.prompt.contains("answer from it and from your read tools"));
    let rw = harness(|s| {
        s.peers.insert(
            "tm_booty".into(),
            tm("tm_booty", "Booty", "Custom", Some(route("claude", "sonnet", "accept-edits"))),
        );
    });
    rw.on_shared(&sharing(), &[ask(&booty())]);
    let start = starts(&rw).remove(0);
    assert_eq!(start.mode, "accept-edits");
    assert!(!start.prompt.contains("read-only mode"));
}

#[test]
fn lends_accept_edits_to_a_cursor_recipient_where_read_only_cannot_be_held_and_says_so() {
    let relay = harness(|s| {
        s.peers.insert("tm_booty".into(), new_booty());
        s.cursor_holds_read_only = false;
    });
    relay.on_shared(
        &SharingMission {
            sandbox: "full-access".into(),
            ..sharing()
        },
        &[ask(&booty())],
    );
    let start = starts(&relay).remove(0);
    assert_eq!((start.runtime.as_str(), start.mode.as_str()), ("cursor", "accept-edits"));
    assert!(said(&relay).iter().any(|line| line
        .contains("replies in Edit mode rather than read-only: Cursor Agent cannot be held read-only")));
    // A remembered Cursor route in ask is widened the same way; a Codex one is not.
    let remembered = harness(|s| {
        s.peers.insert(
            "tm_booty".into(),
            tm("tm_booty", "Booty", "Custom", Some(route("cursor", "composer-2.5", "ask"))),
        );
        s.cursor_holds_read_only = false;
    });
    remembered.on_shared(&sharing(), &[ask(&booty())]);
    let start = starts(&remembered).remove(0);
    assert_eq!((start.runtime.as_str(), start.mode.as_str()), ("cursor", "accept-edits"));
    let codex = harness(|s| {
        s.peers.insert(
            "tm_booty".into(),
            tm("tm_booty", "Booty", "Custom", Some(route("codex", "account-default", "ask"))),
        );
        s.cursor_holds_read_only = false;
    });
    codex.on_shared(&sharing(), &[ask(&booty())]);
    let start = starts(&codex).remove(0);
    assert_eq!((start.runtime.as_str(), start.mode.as_str()), ("codex", "ask"));
}

#[test]
fn a_read_only_sender_gets_a_read_only_reply_when_the_recipient_has_no_route() {
    let relay = harness(|s| {
        s.peers.insert("tm_booty".into(), new_booty());
    });
    relay.on_shared(
        &SharingMission {
            sandbox: "read-only".into(),
            ..sharing()
        },
        &[ask(&booty())],
    );
    assert_eq!(starts(&relay)[0].mode, "ask");
}

#[test]
fn an_account_default_route_is_sent_as_no_model_the_way_the_composer_sends_it() {
    let relay = harness(|s| {
        s.peers.insert(
            "tm_booty".into(),
            tm("tm_booty", "Booty", "Custom", Some(route("codex", "account-default", "ask"))),
        );
    });
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let start = starts(&relay).remove(0);
    assert_eq!(start.runtime, "codex");
    assert_eq!(start.model, None);
}

#[test]
fn a_reply_runs_at_the_recipients_own_saved_effort() {
    let relay = harness(|s| {
        let mut r = route("codex", "gpt-5.5", "ask");
        r.effort = Some("high".into());
        s.peers.insert("tm_booty".into(), tm("tm_booty", "Booty", "Custom", Some(r)));
    });
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let start = starts(&relay).remove(0);
    assert_eq!(start.runtime, "codex");
    assert_eq!(start.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(start.effort.as_deref(), Some("high"));
}

#[test]
fn a_borrowed_route_borrows_no_effort() {
    let relay = harness(|s| {
        s.peers.insert("tm_booty".into(), new_booty());
    });
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert_eq!(starts(&relay)[0].effort, None);
}

#[test]
fn the_reply_back_follows_up_the_mission_that_asked_so_it_lands_in_that_thread() {
    let relay = harness(|_| {});
    let booty_run = SharingMission {
        run_id: "run_booty".into(),
        mission_id: "mission_booty".into(),
        peer: booty(),
        relay: Some(origin(1, &[("tm_wren", "mission_wren1")])),
        ..sharing()
    };
    relay.on_shared(&booty_run, &[from(&booty(), "mission_booty", &wren(), "What is the build command?")]);
    let all = starts(&relay);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].peer, wren());
    assert_eq!(all[0].follow_up_of.as_deref(), Some("mission_wren1"));
    assert_eq!(all[0].relay.hop, 2);
    assert_eq!(
        last_of(&all[0].relay),
        vec![("tm_wren", "mission_wren1"), ("tm_booty", "mission_booty")]
    );
}

// case: a third hop continues the recipient's OWN earlier turn, so their side reads as one thread too
#[test]
fn a_third_hop_continues_the_recipients_own_earlier_turn_so_their_side_reads_as_one_thread_too() {
    let relay = harness(|_| {});
    let wren_again = SharingMission {
        run_id: "run_wren2".into(),
        mission_id: "mission_wren2".into(),
        relay: Some(origin(2, &[("tm_wren", "mission_wren1"), ("tm_booty", "mission_booty")])),
        ..sharing()
    };
    relay.on_shared(&wren_again, &[message(&booty(), "One more thing.")]);
    let start = starts(&relay).remove(0);
    assert_eq!(start.peer, booty());
    assert_eq!(start.follow_up_of.as_deref(), Some("mission_booty"));
    assert_eq!(start.relay.hop, 3);
    assert_eq!(
        last_of(&start.relay),
        vec![("tm_wren", "mission_wren2"), ("tm_booty", "mission_booty")]
    );
}

#[test]
fn stops_after_the_cap_and_says_so_in_the_thread_that_shared() {
    let relay = harness(|_| {});
    let capped = SharingMission {
        run_id: "run_wren2".into(),
        mission_id: "mission_wren2".into(),
        relay: Some(origin(MAX_RELAY_HOPS, &[("tm_wren", "mission_wren1")])),
        ..sharing()
    };
    relay.on_shared(&capped, &[ask(&booty())]);
    assert!(starts(&relay).is_empty());
    let s = st(&relay);
    assert_eq!(s.updates.len(), 1);
    assert!(matches!(&s.updates[0], RelayUpdate::Notice { run_id, mission_id, message, .. }
        if run_id == "run_wren2" && mission_id == "mission_wren2"
            && message.contains(&format!("Stopped after {MAX_RELAY_HOPS}"))));
}

#[test]
fn says_why_when_the_recipients_run_could_not_start_and_lets_the_message_wait() {
    let relay = harness(|s| s.start_result = refused("Claude is not installed."));
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert!(st(&relay).owners.is_empty());
    let first = said(&relay).remove(0);
    assert!(first.contains("Claude is not installed"));
    assert!(first.contains("waits for their next run"));
}

// a reply held because every run slot was taken

#[test]
fn says_so_not_that_the_recipient_is_mid_run() {
    let relay = harness(|s| s.start_results = vec![pool_full()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let first = said(&relay).remove(0);
    assert!(first.contains("Every run slot is in use"));
    assert!(!first.contains("part-way"));
}

// case: starts when ANY run ends, not only the recipient’s own
#[test]
fn starts_when_any_run_ends_not_only_the_recipients_own() {
    let relay = harness(|s| s.start_results = vec![pool_full()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    // Someone else's run ends -- Juno's, a stranger to this exchange -- and a slot frees.
    relay.on_run_ended(&ended("mission_juno", Some(juno())));
    let s = st(&relay);
    assert_eq!(s.starts.len(), 2);
    assert_eq!(s.owners, vec![("tm_booty".to_owned(), "mission_2".to_owned())]);
}

#[test]
fn is_held_again_quietly_while_every_slot_is_still_taken() {
    let relay = harness(|s| s.start_results = vec![pool_full(), pool_full()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    relay.on_run_ended(&ended("mission_juno", Some(juno())));
    assert_eq!(starts(&relay).len(), 2);
    assert_eq!(said(&relay).iter().filter(|l| l.contains("Every run slot")).count(), 1);
}

// a reply whose recipient is mid-run

#[test]
fn is_held_rather_than_dropped_and_the_thread_that_shared_is_told() {
    let relay = harness(|s| s.start_results = vec![busy()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert!(st(&relay).owners.is_empty());
    let first = said(&relay).remove(0);
    assert!(first.contains("part-way through another mission"));
    assert!(first.contains("starts when it ends"));
}

#[test]
fn starts_the_moment_that_run_ends() {
    let relay = harness(|s| s.start_results = vec![busy()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    let s = st(&relay);
    assert_eq!(s.starts.len(), 2);
    // The same brief, on Booty's own route, as the first attempt.
    let second = &s.starts[1];
    assert_eq!(second.runtime, "claude");
    assert_eq!(second.model.as_deref(), Some("sonnet"));
    assert_eq!(second.mode, "ask");
    assert_eq!(second.relay.hop, 1);
    assert_eq!(s.owners, vec![("tm_booty".to_owned(), "mission_2".to_owned())]);
}

#[test]
fn is_held_once_however_many_messages_pile_up_for_one_teammate() {
    let relay = harness(|s| s.start_results = vec![busy(), busy()]);
    relay.on_shared(&sharing(), &[message(&booty(), "first")]);
    relay.on_shared(
        &SharingMission {
            run_id: "run_wren2".into(),
            mission_id: "mission_wren2".into(),
            ..sharing()
        },
        &[message(&booty(), "second")],
    );
    assert_eq!(said(&relay).iter().filter(|l| l.contains("part-way")).count(), 1);
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    assert_eq!(starts(&relay).len(), 3);
}

#[test]
fn tells_every_thread_that_waited_on_the_held_reply_how_it_ended_2026_10_10_sweep() {
    let relay = harness(|s| s.start_results = vec![busy(), busy()]);
    relay.on_shared(&sharing(), &[message(&booty(), "first")]);
    relay.on_shared(
        &SharingMission {
            run_id: "run_wren2".into(),
            mission_id: "mission_wren2".into(),
            ..sharing()
        },
        &[message(&booty(), "second")],
    );
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    assert_eq!(starts(&relay).len(), 3);
    // The held reply ran, as mission_3, and wrote to nobody.
    relay.on_run_ended(&ended("mission_3", Some(booty())));
    let told: Vec<String> = notices(&relay)
        .into_iter()
        .filter(|(_, _, m, _)| m.contains("finished without writing back"))
        .map(|(_, mission, _, _)| mission)
        .collect();
    assert!(told.contains(&"mission_wren2".to_owned()));
    assert_eq!(told.len(), 2);
}

#[test]
fn waits_again_when_the_teammate_was_taken_by_something_else_in_between() {
    let relay = harness(|s| s.start_results = vec![busy(), busy()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    assert_eq!(starts(&relay).len(), 2);
    // Still held: the next ending is another chance, not a lost message.
    relay.on_run_ended(&ended("mission_booty2", Some(booty())));
    assert_eq!(starts(&relay).len(), 3);
}

#[test]
fn does_not_start_when_a_later_run_already_carried_the_message() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.still_waiting = Some(false);
    });
    relay.on_shared(&sharing(), &[ask(&booty())]);
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    let s = st(&relay);
    assert_eq!(s.asked, vec!["tm_booty".to_owned()]);
    assert_eq!(s.starts.len(), 1);
}

#[test]
fn does_not_start_once_replies_have_been_switched_off() {
    let relay = harness(|s| s.start_results = vec![busy()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    st(&relay).enabled = false;
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    assert_eq!(starts(&relay).len(), 1);
    assert!(said(&relay).last().unwrap().contains("switched off in Settings"));
}

#[test]
fn still_tells_the_asker_when_the_held_reply_finishes_without_writing_back() {
    let relay = harness(|s| s.start_results = vec![busy()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    // mission_2 is the run the held reply started.
    relay.on_run_ended(&ended("mission_2", Some(booty())));
    assert!(said(&relay).last().unwrap().contains("Booty finished without writing back"));
    let s = st(&relay);
    assert!(matches!(s.updates.last(), Some(RelayUpdate::Notice { run_id, mission_id, .. })
        if run_id == "run_wren1" && mission_id == "mission_wren1"));
}

// saying a hop is coming before it exists

#[test]
fn names_the_teammate_and_what_they_are_answering_before_the_start() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&booty())]);
    let s = st(&relay);
    let starting = s
        .updates
        .iter()
        .position(|u| matches!(u, RelayUpdate::Starting { .. }))
        .unwrap();
    assert_eq!(
        s.updates[starting],
        RelayUpdate::Starting {
            teammate_id: "tm_booty".into(),
            name: "Booty".into(),
            answering: "mission_wren1".into(),
            hop: 1,
        }
    );
    // Before, so the window is never behind the host.
    let started = s
        .updates
        .iter()
        .position(|u| matches!(u, RelayUpdate::MissionStarted { .. }))
        .unwrap();
    assert!(starting < started);
}

#[test]
fn clears_it_however_the_start_ended() {
    for answer in [None, busy(), refused("no")] {
        let relay = harness(|s| s.start_result = answer.clone());
        relay.on_shared(&sharing(), &[ask(&booty())]);
        let s = st(&relay);
        assert_eq!(s.updates.iter().filter(|u| matches!(u, RelayUpdate::Starting { .. })).count(), 1);
        assert_eq!(s.updates.iter().filter(|u| matches!(u, RelayUpdate::StartSettled { .. })).count(), 1);
    }
}

#[test]
fn clears_it_when_the_start_throws_so_a_row_is_never_stuck_working() {
    let relay = harness(|s| s.start_result = Some(Err("the runtime is not there".into())));
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert_eq!(
        st(&relay).updates.iter().filter(|u| matches!(u, RelayUpdate::StartSettled { .. })).count(),
        1
    );
}

// a message that asks to be taken now

#[test]
fn stops_the_recipient_when_the_person_has_allowed_it() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.may_interrupt = true;
    });
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    assert_eq!(st(&relay).stopped, vec!["tm_booty".to_owned()]);
    assert!(said(&relay).last().unwrap().contains("stopped part-way"));
}

#[test]
fn stops_nobody_when_it_is_switched_off_and_says_so_rather_than_going_quiet() {
    let relay = harness(|s| s.start_results = vec![busy()]);
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    assert!(st(&relay).stopped.is_empty());
    assert!(said(&relay).last().unwrap().contains("switched off in Settings"));
}

#[test]
fn stops_nobody_when_the_recipient_was_free_anyway() {
    let relay = harness(|s| s.may_interrupt = true);
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    let s = st(&relay);
    assert_eq!(s.starts.len(), 1);
    assert!(s.stopped.is_empty());
}

#[test]
fn leaves_an_ordinary_message_waiting_however_busy_the_recipient_is() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.may_interrupt = true;
    });
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert!(st(&relay).stopped.is_empty());
}

#[test]
fn still_only_holds_the_message_once_stopping_is_not_a_second_way_to_start() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.may_interrupt = true;
    });
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    assert_eq!(starts(&relay).len(), 1);
    assert_eq!(st(&relay).stopped, vec!["tm_booty".to_owned()]);
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    assert_eq!(starts(&relay).len(), 2);
}

#[test]
fn says_nothing_about_stopping_when_there_was_nothing_to_stop() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.may_interrupt = true;
        s.stop_works = false;
    });
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    assert!(!said(&relay).iter().any(|l| l.contains("stopped part-way")));
}

#[test]
fn cannot_outrun_the_budget_a_capped_exchange_never_reaches_the_stop() {
    let relay = harness(|s| s.may_interrupt = true);
    let capped = SharingMission {
        relay: Some(origin(MAX_RELAY_HOPS, &[("tm_wren", "mission_wren1")])),
        ..sharing()
    };
    relay.on_shared(&capped, &[urgent(&booty())]);
    assert!(st(&relay).stopped.is_empty());
}

#[test]
fn cannot_outrun_the_relay_switch_either() {
    let relay = harness(|s| {
        s.enabled = false;
        s.may_interrupt = true;
    });
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    assert!(st(&relay).stopped.is_empty());
}

#[test]
fn is_shown_to_a_running_turn_that_can_take_it_at_its_next_step_and_nobody_is_stopped() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.steers = Some(true);
        s.may_interrupt = true;
    });
    relay.on_shared(
        &sharing(),
        &[RelayedMessage {
            text: "Stop editing app.ts, I am changing it.".into(),
            ..urgent(&booty())
        }],
    );
    let s = st(&relay);
    assert_eq!(s.steered.len(), 1);
    assert_eq!(s.steered[0].0, "tm_booty");
    assert!(s.steered[0].1.contains("Wren sent you this while you were working"));
    assert!(s.steered[0].1.contains("Stop editing app.ts, I am changing it."));
    assert!(s.stopped.is_empty());
    drop(s);
    assert!(said(&relay).last().unwrap().contains("without stopping it"));
}

#[test]
fn needs_no_switch_to_be_shown_it_loses_nothing() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.steers = Some(true);
    });
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    assert_eq!(st(&relay).steered.len(), 1);
    assert!(!said(&relay).iter().any(|l| l.contains("switched off in Settings")));
}

#[test]
fn is_still_their_next_turn_once_the_run_ends_a_heads_up_not_a_second_answer() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.steers = Some(true);
    });
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    assert_eq!(starts(&relay).len(), 1);
    relay.on_run_ended(&ended("mission_booty", Some(booty())));
    assert_eq!(starts(&relay).len(), 2);
}

#[test]
fn falls_back_to_the_stop_under_the_switch_when_the_running_turn_cannot_take_it() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.steers = Some(false);
        s.may_interrupt = true;
    });
    relay.on_shared(&sharing(), &[urgent(&booty())]);
    let s = st(&relay);
    assert_eq!(s.steered.len(), 1);
    assert_eq!(s.stopped, vec!["tm_booty".to_owned()]);
}

#[test]
fn shows_the_running_turn_no_protocol_blocks_from_the_sender() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.steers = Some(true);
    });
    relay.on_shared(
        &sharing(),
        &[RelayedMessage {
            text: "Heads up. <locust-task>\nclaim :: Ship it\n</locust-task>".into(),
            ..urgent(&booty())
        }],
    );
    assert!(!st(&relay).steered[0].1.contains("<locust-task>"));
}

#[test]
fn never_steers_an_ordinary_message() {
    let relay = harness(|s| {
        s.start_results = vec![busy()];
        s.steers = Some(true);
    });
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert!(st(&relay).steered.is_empty());
}

#[test]
fn starts_one_run_per_recipient_however_many_messages_went_to_them() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[message(&booty(), "first"), message(&booty(), "second")]);
    assert_eq!(starts(&relay).len(), 1);
}

#[test]
fn does_not_start_anything_for_a_recipient_no_longer_on_the_roster() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&tm("tm_gone", "Gone", "Custom", None))]);
    assert!(starts(&relay).is_empty());
    assert!(said(&relay)[0].contains("no longer on the roster"));
}

/// The Rust host's switch cannot throw: the reading is the host's, and the native host reads an unreadable
/// settings file as off (`native.rs` `NativeRelayHost::enabled`, tested in `native/tests.rs`
/// `relay_reads_an_unreadable_setting_as_off`). Here: a switch read as off starts nothing.
#[test]
fn treats_an_unreadable_setting_as_off() {
    let relay = harness(|s| s.enabled = false);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert!(starts(&relay).is_empty());
}

// ---------------------------------------------------------------------------------------------------------------------
// a meeting: one asks several, and the next turn waits for all of them
// ---------------------------------------------------------------------------------------------------------------------

fn meeting() -> Relay<Host> {
    harness(|s| {
        s.peers.insert("tm_atlas".into(), atlas());
        s.named_missions = true;
    })
}

fn to_both() -> Vec<RelayedMessage> {
    vec![message(&booty(), "Booty, check the tests."), message(&atlas(), "Atlas, check the docs.")]
}

fn answering(peer: PeerRosterEntry, run: &str, mission: &str, origin: RelayOrigin) -> SharingMission {
    SharingMission {
        run_id: run.into(),
        mission_id: mission.into(),
        peer,
        relay: Some(origin),
        ..sharing()
    }
}

#[test]
fn starts_a_run_for_each_and_says_the_next_turn_is_waiting_on_them() {
    let relay = meeting();
    relay.on_shared(&sharing(), &to_both());
    let names: Vec<String> = starts(&relay).iter().map(|s| s.peer.name.clone()).collect();
    assert_eq!(names, vec!["Booty", "Atlas"]);
    assert_eq!(
        said(&relay).last().unwrap(),
        "Waiting on Booty, Atlas to reply before your next turn."
    );
}

// case: holds the first reply, and starts the asker's turn once after the last, briefed with everyone
#[test]
fn holds_the_first_reply_and_starts_the_askers_turn_once_after_the_last_briefed_with_everyone() {
    let relay = meeting();
    relay.on_shared(&sharing(), &to_both());
    let first = starts(&relay);
    relay.on_shared(
        &answering(booty(), "run_1", "mission_booty_1", first[0].relay.clone()),
        &[from(&booty(), "mission_booty_1", &wren(), "Tests pass.")],
    );
    assert_eq!(starts(&relay).len(), 2);
    assert_eq!(said(&relay).last().unwrap(), "Booty replied. Still waiting on Atlas.");
    relay.on_shared(
        &answering(atlas(), "run_2", "mission_atlas_2", first[1].relay.clone()),
        &[from(&atlas(), "mission_atlas_2", &wren(), "Docs are stale.")],
    );
    let all = starts(&relay);
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].peer, wren());
    assert_eq!(all[2].follow_up_of.as_deref(), Some("mission_wren1"));
    assert!(all[2].prompt.contains("Booty and Atlas replied to your message"));
}

#[test]
fn counts_a_teammate_who_finishes_without_replying_and_still_convenes_the_rest() {
    let relay = meeting();
    relay.on_shared(&sharing(), &to_both());
    let first = starts(&relay);
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_booty_1".into(),
        peer: Some(new_booty()),
        relay: Some(first[0].relay.clone()),
    });
    assert_eq!(
        said(&relay).last().unwrap(),
        "Booty finished without replying. Still waiting on Atlas."
    );
    relay.on_shared(
        &answering(atlas(), "run_2", "mission_atlas_2", first[1].relay.clone()),
        &[from(&atlas(), "mission_atlas_2", &wren(), "Docs are stale.")],
    );
    let all = starts(&relay);
    assert_eq!(all.len(), 3);
    assert!(all[2].prompt.contains("Atlas replied to your message"));
    assert!(all[2].prompt.contains("Booty finished without replying"));
}

#[test]
fn says_so_when_nobody_replied_and_starts_nothing() {
    let relay = meeting();
    relay.on_shared(&sharing(), &to_both());
    let first = starts(&relay);
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_booty_1".into(),
        peer: Some(new_booty()),
        relay: Some(first[0].relay.clone()),
    });
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_atlas_2".into(),
        peer: Some(tm("tm_atlas", "Atlas", "Research & Briefs", None)),
        relay: Some(first[1].relay.clone()),
    });
    assert_eq!(starts(&relay).len(), 2);
    assert_eq!(
        said(&relay).last().unwrap(),
        "Nobody replied. Your message waits with them for their next run."
    );
}

#[test]
fn one_recipient_is_an_ordinary_exchange_not_a_meeting() {
    let relay = meeting();
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert!(!said(&relay).iter().any(|n| n.starts_with("Waiting on")));
}

#[test]
fn names_everyone_in_the_minutes() {
    assert!(minutes(&["Booty", "Atlas", "Juno"], &[]).contains("Booty, Atlas and Juno replied"));
    assert!(minutes(&["Booty"], &["Atlas"]).contains("Atlas finished without replying"));
}

// ---------------------------------------------------------------------------------------------------------------------
// an ending is written down, not only announced
// ---------------------------------------------------------------------------------------------------------------------

fn capped() -> SharingMission {
    SharingMission {
        relay: Some(origin(MAX_RELAY_HOPS, &[("tm_wren", "mission_wren1")])),
        ..sharing()
    }
}

#[test]
fn keeps_the_reason_an_exchange_stopped_at_its_budget() {
    let relay = harness(|_| {});
    relay.on_shared(&capped(), &[ask(&booty())]);
    let s = st(&relay);
    assert_eq!(s.kept.len(), 1);
    assert!(s.kept[0].1.contains("Stopped after"));
    // Against the mission a person would reopen: the one that shared.
    assert_eq!(s.kept[0].0, "mission_wren1");
}

#[test]
fn keeps_that_a_teammate_finished_without_writing_back() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&booty())]);
    relay.on_run_ended(&ended("mission_1", Some(booty())));
    assert!(st(&relay).kept.iter().any(|(_, m)| m.contains("finished without writing back")));
}

#[test]
fn keeps_nothing_for_a_reply_that_is_merely_being_waited_on() {
    let relay = harness(|s| s.start_results = vec![busy()]);
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert!(said(&relay).iter().any(|l| l.contains("part-way through")));
    assert!(st(&relay).kept.is_empty());
}

#[test]
fn says_it_as_well_as_keeping_it_the_live_update_is_unchanged() {
    let relay = harness(|_| {});
    relay.on_shared(&capped(), &[ask(&booty())]);
    assert!(said(&relay).iter().any(|l| l.contains("Stopped after")));
    assert_eq!(st(&relay).kept.len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------------
// a budget spent per exchange, not per chain
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn refuses_once_the_exchange_has_spent_its_budget_whatever_this_chain_says() {
    assert_eq!(
        decide(true, 1, Some(6), Some(6)),
        RelayDecision::Skip {
            reason: "Stopped after 6 automatic replies. Booty will see this on their next run.".into()
        }
    );
}

#[test]
fn allows_a_deep_chain_that_has_not_spent_the_budget() {
    assert_eq!(decide(true, 2, Some(6), Some(2)), RelayDecision::Start { hop: 3 });
}

#[test]
fn falls_back_to_the_chain_depth_when_nothing_counted() {
    assert!(matches!(decide(true, 6, Some(6), None), RelayDecision::Skip { .. }));
    assert!(matches!(decide(true, 0, Some(6), None), RelayDecision::Start { .. }));
}

#[test]
fn counts_every_relayed_run_against_one_exchange_across_separate_chains() {
    let relay = harness(|_| {});
    relay.on_shared(
        &SharingMission {
            run_id: "run_a".into(),
            mission_id: "mission_root".into(),
            ..sharing()
        },
        &[ask(&booty())],
    );
    relay.on_shared(
        &SharingMission {
            run_id: "run_b".into(),
            mission_id: "mission_b".into(),
            relay: Some(RelayOrigin {
                root_mission_id: Some("mission_root".into()),
                ..origin(1, &[])
            }),
            ..sharing()
        },
        &[ask(&booty())],
    );
    let all = starts(&relay);
    assert_eq!(all.len(), 2);
    for start in &all {
        assert_eq!(start.relay.root_mission_id.as_deref(), Some("mission_root"));
    }
}

#[test]
fn gives_a_person_started_mission_its_own_exchange() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert_eq!(starts(&relay)[0].relay.root_mission_id.as_deref(), Some("mission_wren1"));
}

// ---------------------------------------------------------------------------------------------------------------------
// telling a reply what a reply costs
// ---------------------------------------------------------------------------------------------------------------------

fn costs(hop: usize) -> String {
    brief(&wren(), &tm("tm_booty", "Booty", "Custom", None), hop, Some(6), false)
}

// case: says nothing about it on the FIRST hop, where there is work to do
#[test]
fn says_nothing_about_it_on_the_first_hop_where_there_is_work_to_do() {
    assert!(!costs(1).contains("costs money"));
}

#[test]
fn tells_a_reply_that_the_other_end_is_a_model_and_a_reply_costs_a_run() {
    assert!(costs(2).contains("is a MODEL, not a person"));
    assert!(costs(2).contains("costs money"));
}

#[test]
fn names_the_acknowledgements_by_name_because_those_are_the_ones_that_happen() {
    let said = costs(3);
    assert!(said.contains("END HERE with no share block"));
    assert!(said.contains("Thanks, receipts and recaps reach nobody"));
}

// case: says what a reply IS, before what it is not
#[test]
fn says_what_a_reply_is_before_what_it_is_not() {
    let said = costs(2);
    let worth = said.find("A reply is worth that only when it moves the work").unwrap();
    let nots = said.find("Thanks, receipts and recaps").unwrap();
    assert!(nots > worth);
    assert!(said.contains("written for a capable colleague who has not seen your turn"));
    assert!(said.contains(
        "lead with the answer, said as what is true rather than as a negation, then the evidence by name"
    ));
    assert!(!said.contains("nobody is watching"));
    assert!(!said.contains("Do not thank"));
}

// ---------------------------------------------------------------------------------------------------------------------
// telling a teammate where in the budget it is
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn names_the_reply_and_the_budget_plainly_in_the_middle_of_an_exchange() {
    assert_eq!(budget_sentence(1, Some(6)), "This is automatic reply 1 of 6.");
    assert_eq!(budget_sentence(3, Some(6)), "This is automatic reply 3 of 6.");
}

#[test]
fn warns_on_the_one_before_the_last_where_the_warning_can_still_change_something() {
    assert!(budget_sentence(5, Some(6)).contains("One more would be the last"));
    assert!(budget_sentence(5, Some(6)).contains("5 of 6"));
}

#[test]
fn says_what_is_actually_true_on_the_last_a_reply_written_here_reaches_nobody() {
    let last = budget_sentence(6, Some(6));
    assert!(last.contains("LAST automatic reply"));
    assert!(last.contains("waits for a person"));
}

#[test]
fn treats_anything_past_the_cap_as_the_last_never_as_a_negative_count() {
    assert!(budget_sentence(9, Some(6)).contains("LAST automatic reply"));
}

#[test]
fn falls_back_to_the_default_budget_when_none_was_given() {
    assert_eq!(budget_sentence(1, None), format!("This is automatic reply 1 of {MAX_RELAY_HOPS}."));
}

#[test]
fn handles_a_budget_of_one_where_the_first_reply_is_also_the_last() {
    assert!(budget_sentence(1, Some(1)).contains("LAST automatic reply"));
}

#[test]
fn is_in_the_brief_a_relayed_run_is_actually_started_with() {
    assert!(costs(5).contains("One more would be the last"));
}

// ---------------------------------------------------------------------------------------------------------------------
// the reply that lands back in the person's conversation
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn is_briefed_as_read_by_the_person_and_the_hop_after_it_is_not() {
    let relay = harness(|_| {});
    relay.on_shared(&sharing(), &[ask(&booty())]);
    assert_eq!(starts(&relay).len(), 1);
    let booty_origin = starts(&relay)[0].relay.clone();
    assert_eq!(last_of(&booty_origin), vec![("tm_wren", "mission_wren1")]);
    // Booty answers Wren: hop 2, into the conversation the person started.
    relay.on_shared(
        &SharingMission {
            run_id: "run_1".into(),
            mission_id: "mission_1".into(),
            peer: booty(),
            relay: Some(booty_origin),
            runtime: "claude".into(),
            sandbox: "read-only".into(),
            model: Some("sonnet".into()),
            started_by: None,
        },
        &[ask(&wren())],
    );
    let all = starts(&relay);
    assert_eq!(all.len(), 2);
    assert!(all[1].prompt.contains("the person who started this conversation reads it"));
    assert!(!all[1].prompt.contains("nobody is watching this run"));
    // Wren writes back to Booty: hop 3, Booty's own conversation, nobody's.
    relay.on_shared(
        &SharingMission {
            run_id: "run_2".into(),
            mission_id: "mission_2".into(),
            peer: wren(),
            relay: Some(all[1].relay.clone()),
            runtime: "cursor".into(),
            sandbox: "workspace-write".into(),
            model: Some("composer-2.5".into()),
            started_by: None,
        },
        &[ask(&booty())],
    );
    let all = starts(&relay);
    assert_eq!(all.len(), 3);
    assert!(all[2].prompt.contains("reach nobody and cost a run each"));
    assert!(!all[2].prompt.contains("reads it"));
}
