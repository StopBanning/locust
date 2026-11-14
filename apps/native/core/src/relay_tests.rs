use super::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn peer(id: &str, name: &str, role: &str, route: Option<TurnRoute>) -> PeerRosterEntry {
    PeerRosterEntry {
        teammate_id: id.to_owned(),
        name: name.to_owned(),
        role: role.to_owned(),
        route,
    }
}

fn route(runtime: &str, model: Option<&str>, mode: &str) -> TurnRoute {
    TurnRoute {
        runtime: runtime.to_owned(),
        model: model.map(str::to_owned),
        effort: None,
        account: None,
        mode: mode.to_owned(),
    }
}

fn msg(
    id: &str,
    from: (&str, &str, &str),
    to: (&str, &str),
    text: &str,
    urgent: bool,
    defer: bool,
    wants_answer: bool,
) -> RelayedMessage {
    RelayedMessage {
        message_id: id.to_owned(),
        from_teammate_id: from.0.to_owned(),
        from_name: from.1.to_owned(),
        from_mission_id: from.2.to_owned(),
        to_teammate_id: to.0.to_owned(),
        to_name: to.1.to_owned(),
        text: text.to_owned(),
        urgent,
        defer,
        wants_answer,
    }
}

struct MockState {
    enabled: bool,
    hop_cap: usize,
    peers: HashMap<String, PeerRosterEntry>,
    exchange_map: HashMap<String, String>,
    final_replies: HashMap<String, String>,
    how_ended_map: HashMap<String, String>,
    cursor_holds_read_only: bool,
    routes: HashMap<String, TurnRoute>,
    out_of_plan_map: HashMap<String, String>,
    hubs: HashMap<String, String>,
    remembered_hubs: Vec<(String, String, bool)>,
    owners: Vec<(String, String)>,
    still_waiting_map: HashMap<String, bool>,
    may_interrupt: bool,
    stop_returns: HashMap<String, bool>,
    stopped_calls: Vec<String>,
    steer_returns: HashMap<String, bool>,
    steered_calls: Vec<(String, String)>,
    starts: Vec<StartRequest>,
    next_start_results: VecDeque<Result<StartResult, String>>,
    posted_returns: Vec<(String, String, String, String, String, String)>,
    updates: Vec<RelayUpdate>,
    notes: Vec<(String, String)>,
    seq: usize,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            enabled: true,
            hop_cap: MAX_RELAY_HOPS,
            peers: HashMap::new(),
            exchange_map: HashMap::new(),
            final_replies: HashMap::new(),
            how_ended_map: HashMap::new(),
            cursor_holds_read_only: true,
            routes: HashMap::new(),
            out_of_plan_map: HashMap::new(),
            hubs: HashMap::new(),
            remembered_hubs: Vec::new(),
            owners: Vec::new(),
            still_waiting_map: HashMap::new(),
            may_interrupt: false,
            stop_returns: HashMap::new(),
            stopped_calls: Vec::new(),
            steer_returns: HashMap::new(),
            steered_calls: Vec::new(),
            starts: Vec::new(),
            next_start_results: VecDeque::new(),
            posted_returns: Vec::new(),
            updates: Vec::new(),
            notes: Vec::new(),
            seq: 0,
        }
    }
}

struct MockHost {
    st: Arc<Mutex<MockState>>,
}

impl MockHost {
    fn new() -> Self {
        let mut st = MockState::default();
        st.peers.insert(
            "tm_wren".into(),
            peer(
                "tm_wren",
                "Wren",
                "Backend",
                Some(route("codex", Some("gpt-5.4"), "accept-edits")),
            ),
        );
        st.peers.insert(
            "tm_juno".into(),
            peer(
                "tm_juno",
                "Juno",
                "Frontend",
                Some(route("claude", Some("claude-sonnet-4-6"), "ask")),
            ),
        );
        st.peers.insert(
            "tm_pip".into(),
            peer(
                "tm_pip",
                "Pip",
                "Reviewer",
                Some(route("codex", Some("gpt-5.4"), "ask")),
            ),
        );
        Self {
            st: Arc::new(Mutex::new(st)),
        }
    }
}

impl RelayHost for MockHost {
    fn enabled(&self) -> bool {
        self.st.lock().unwrap().enabled
    }

    fn hop_cap(&self) -> usize {
        self.st.lock().unwrap().hop_cap
    }

    fn exchange_of(&self, mission_id: &str) -> Option<String> {
        self.st.lock().unwrap().exchange_map.get(mission_id).cloned()
    }

    fn peer_context_for(&self, teammate_id: &str) -> Option<PeerRosterEntry> {
        self.st.lock().unwrap().peers.get(teammate_id).cloned()
    }

    fn final_reply_of(&self, mission_id: &str) -> Option<String> {
        self.st.lock().unwrap().final_replies.get(mission_id).cloned()
    }

    fn how_ended(&self, mission_id: &str) -> Option<String> {
        self.st.lock().unwrap().how_ended_map.get(mission_id).cloned()
    }

    fn post_returned_answer(
        &self,
        from_id: &str,
        from_name: &str,
        from_mission: &str,
        to_id: &str,
        to_name: &str,
        text: &str,
    ) -> Option<String> {
        let mut st = self.st.lock().unwrap();
        st.seq += 1;
        let id = format!("wm_ret_{}", st.seq);
        st.posted_returns.push((
            from_id.to_owned(),
            from_name.to_owned(),
            from_mission.to_owned(),
            to_id.to_owned(),
            to_name.to_owned(),
            text.to_owned(),
        ));
        Some(id)
    }

    fn cursor_holds_read_only(&self) -> bool {
        self.st.lock().unwrap().cursor_holds_read_only
    }

    fn route_of(&self, mission_id: &str) -> Option<TurnRoute> {
        self.st.lock().unwrap().routes.get(mission_id).cloned()
    }

    fn out_of_plan(&self, runtime: &str, account: Option<&str>) -> Option<String> {
        let key = match account {
            Some(a) => format!("{runtime}:{a}"),
            None => runtime.to_owned(),
        };
        let st = self.st.lock().unwrap();
        st.out_of_plan_map
            .get(&key)
            .or_else(|| st.out_of_plan_map.get(runtime))
            .cloned()
    }

    fn start(&self, req: StartRequest) -> Result<StartResult, String> {
        let mut st = self.st.lock().unwrap();
        st.starts.push(req);
        if let Some(res) = st.next_start_results.pop_front() {
            return res;
        }
        st.seq += 1;
        let n = st.seq;
        Ok(StartResult::Started {
            run_id: format!("run_{n}"),
            mission_id: format!("m_{n}"),
        })
    }

    fn assign_owner(&self, teammate_id: &str, mission_id: &str) {
        self.st
            .lock()
            .unwrap()
            .owners
            .push((teammate_id.to_owned(), mission_id.to_owned()));
    }

    fn hub_of(&self, teammate_id: &str) -> Option<String> {
        self.st.lock().unwrap().hubs.get(teammate_id).cloned()
    }

    fn remember_hub(&self, teammate_id: &str, mission_id: &str, began: bool) {
        let mut st = self.st.lock().unwrap();
        st.hubs.insert(teammate_id.to_owned(), mission_id.to_owned());
        st.remembered_hubs
            .push((teammate_id.to_owned(), mission_id.to_owned(), began));
    }

    fn still_waiting(&self, teammate_id: &str) -> Option<bool> {
        self.st
            .lock()
            .unwrap()
            .still_waiting_map
            .get(teammate_id)
            .copied()
    }

    fn may_interrupt(&self) -> bool {
        self.st.lock().unwrap().may_interrupt
    }

    fn stop_work_of(&self, teammate_id: &str) -> bool {
        let mut st = self.st.lock().unwrap();
        st.stopped_calls.push(teammate_id.to_owned());
        st.stop_returns.get(teammate_id).copied().unwrap_or(true)
    }

    fn steer_work_of(&self, teammate_id: &str, text: &str) -> bool {
        let mut st = self.st.lock().unwrap();
        st.steered_calls
            .push((teammate_id.to_owned(), text.to_owned()));
        st.steer_returns.get(teammate_id).copied().unwrap_or(false)
    }

    fn notify(&self, update: RelayUpdate) {
        self.st.lock().unwrap().updates.push(update);
    }

    fn note(&self, mission_id: &str, message: &str) {
        self.st
            .lock()
            .unwrap()
            .notes
            .push((mission_id.to_owned(), message.to_owned()));
    }
}

fn notices(st: &MockState) -> Vec<String> {
    st.updates
        .iter()
        .filter_map(|u| match u {
            RelayUpdate::Notice { message, .. } => Some(message.clone()),
            _ => None,
        })
        .collect()
}

fn sharing_wren(mission_id: &str, relay: Option<RelayOrigin>) -> SharingMission {
    SharingMission {
        run_id: format!("r_{mission_id}"),
        mission_id: mission_id.to_owned(),
        runtime: "codex".into(),
        sandbox: "workspace-write".into(),
        model: Some("gpt-5.4".into()),
        peer: peer(
            "tm_wren",
            "Wren",
            "Backend",
            Some(route("codex", Some("gpt-5.4"), "accept-edits")),
        ),
        relay,
        started_by: None,
    }
}

#[test]
fn decide_relay_respects_defer_closes_disabled_and_cap() {
    assert_eq!(
        decide_relay(DecideRelayInput {
            enabled: true,
            defer: true,
            closes: true,
            hop: 0,
            recipient_name: "Juno",
            cap: Some(4),
            spent: Some(10),
        }),
        RelayDecision::Skip {
            reason: "Sent to Juno to read on their next run, rather than starting one.".into()
        }
    );
    assert_eq!(
        decide_relay(DecideRelayInput {
            enabled: false,
            defer: false,
            closes: true,
            hop: 0,
            recipient_name: "Juno",
            cap: Some(4),
            spent: None,
        }),
        RelayDecision::Skip {
            reason: "Sent to Juno to read on their next run: the message says nothing more is needed from them.".into()
        }
    );
    assert_eq!(
        decide_relay(DecideRelayInput {
            enabled: false,
            defer: false,
            closes: false,
            hop: 0,
            recipient_name: "Juno",
            cap: Some(4),
            spent: None,
        }),
        RelayDecision::Skip {
            reason: "Teammate replies are switched off in Settings; the message waits for their next run.".into()
        }
    );
    assert_eq!(
        decide_relay(DecideRelayInput {
            enabled: true,
            defer: false,
            closes: false,
            hop: 0,
            recipient_name: "Juno",
            cap: Some(1),
            spent: Some(1),
        }),
        RelayDecision::Skip {
            reason: "Stopped after 1 automatic reply. Juno will see this on their next run.".into()
        }
    );
    assert_eq!(
        decide_relay(DecideRelayInput {
            enabled: true,
            defer: false,
            closes: false,
            hop: 2,
            recipient_name: "Juno",
            cap: Some(5),
            spent: Some(5),
        }),
        RelayDecision::Skip {
            reason: "Stopped after 5 automatic replies. Juno will see this on their next run.".into()
        }
    );
    assert_eq!(
        decide_relay(DecideRelayInput {
            enabled: true,
            defer: false,
            closes: false,
            hop: 11,
            recipient_name: "Juno",
            cap: None,
            spent: None,
        }),
        RelayDecision::Start { hop: 12 }
    );
    assert_eq!(
        decide_relay(DecideRelayInput {
            enabled: true,
            defer: false,
            closes: false,
            hop: 12,
            recipient_name: "Juno",
            cap: None,
            spent: None,
        }),
        RelayDecision::Skip {
            reason: "Stopped after 12 automatic replies. Juno will see this on their next run.".into()
        }
    );
}

#[test]
fn says_nothing_is_needed_catches_closings_but_keeps_requests() {
    assert!(says_nothing_is_needed("Confirmed, got it. No further action is needed from you."));
    assert!(says_nothing_is_needed("Thanks! Nothing further is required on your part."));
    assert!(says_nothing_is_needed("You do not need to take any action."));
    assert!(says_nothing_is_needed("No need for you to act on this."));

    assert!(!says_nothing_is_needed("Nothing further is needed from me."));
    assert!(!says_nothing_is_needed("No further action is needed from you, right?"));
    assert!(!says_nothing_is_needed("No further action is needed from you, please double check."));
    assert!(!says_nothing_is_needed("You don't need to act on this, can you archive it?"));
}

#[test]
fn prompt_helpers_and_budget_sentences_match_electron() {
    assert_eq!(
        budget_sentence(12, None),
        "This is the LAST automatic reply in this exchange: anything you send back now waits for a person rather than reaching them, so finish what you can say here."
    );
    assert_eq!(
        budget_sentence(3, Some(4)),
        "This is automatic reply 3 of 4. One more would be the last, so do not write back unless it genuinely needs saying."
    );
    assert_eq!(budget_sentence(2, Some(5)), "This is automatic reply 2 of 5.");

    let wren = peer("tm_wren", "Wren", "Backend", None);
    let juno = peer("tm_juno", "Juno", "Frontend", None);

    let first = relay_prompt(&RelayPromptInput {
        sender: &wren,
        recipient: &juno,
        hop: 1,
        cap: Some(4),
        read_by_person: false,
        reply_number: Some(1),
        briefed_before: false,
    });
    assert!(first.starts_with("Wren (Backend) sent you a message; it is quoted below with anything else waiting for you."));
    assert!(first.contains("<locust-share to=\"Wren\">"));
    assert!(first.contains("The person is not in this exchange: Wren is a teammate"));
    assert!(first.contains("This is automatic reply 1 of 4."));

    let reply_to_person = relay_prompt(&RelayPromptInput {
        sender: &juno,
        recipient: &wren,
        hop: 2,
        cap: Some(4),
        read_by_person: true,
        reply_number: Some(2),
        briefed_before: false,
    });
    assert!(reply_to_person.contains("the person who started this conversation reads it"));
    assert!(reply_to_person.contains("a <locust-ask> block reaches them here"));
    assert!(!reply_to_person.contains("The person is not in this exchange"));

    let briefed = relay_prompt(&RelayPromptInput {
        sender: &juno,
        recipient: &wren,
        hop: 3,
        cap: Some(4),
        read_by_person: true,
        reply_number: Some(3),
        briefed_before: true,
    });
    assert!(briefed.contains("The rules for replies from your earlier turn in this exchange still hold"));
    assert!(briefed.contains("The person who started this conversation reads it: say in a line or two what Juno's reply means"));
    assert!(briefed.contains("This is automatic reply 3 of 4. One more would be the last"));

    let mp = meeting_prompt(&["Juno".into(), "Pip".into()], &["Sam".into()], true);
    assert!(mp.starts_with("Juno and Pip replied to your message"));
    assert!(mp.contains("Sam finished without replying."));
    assert!(mp.contains("The person who started this conversation reads it and is waiting on this"));

    let rap = returned_answer_prompt("Juno", true);
    assert!(rap.starts_with("Juno answered your message in their own conversation"));
    assert!(rap.contains("The person who started this conversation reads it and is waiting on this"));
}

#[test]
fn without_protocol_blocks_strips_every_tag_and_defangs_strays() {
    let input = "Answer here.\n<locust-share to=\"Wren\">dup</locust-share>\n<locust-memory>note</locust-memory>\n<locust-task>t</locust-task>\n<locust-file>a.md :: note</locust-file>\n<locust-next>n :: d</locust-next>\nAnd `<locust-share>` in code stays, plus <locust-share dangling";
    let cleaned = without_protocol_blocks(input);
    assert!(cleaned.contains("Answer here."));
    assert!(!cleaned.contains("dup"));
    assert!(!cleaned.contains("a.md"));
    assert!(!cleaned.contains("note</locust-memory>"));
    assert!(cleaned.contains("`‹locust-share>`"));
    assert!(cleaned.contains("‹locust-share dangling"));
}

#[test]
fn relay_skips_mcp_and_deduplicates_recipients_and_tracks_hub() {
    let relay = Relay::new(MockHost::new());
    let mut mcp_mission = sharing_wren("m_mcp", None);
    mcp_mission.started_by = Some(MissionStarter::Mcp);
    relay.on_shared(
        &mcp_mission,
        &[msg("wm_1", ("tm_wren", "Wren", "m_mcp"), ("tm_juno", "Juno"), "hi", false, false, true)],
    );
    {
        let st = relay.host().st.lock().unwrap();
        assert!(st.starts.is_empty());
        assert_eq!(st.notes.len(), 1);
        assert!(st.notes[0].1.contains("Shares from another app are visible in Locust"));
    }

    let wren1 = sharing_wren("m_1", None);
    relay.on_shared(
        &wren1,
        &[
            msg("wm_2", ("tm_wren", "Wren", "m_1"), ("tm_juno", "Juno"), "one?", false, false, true),
            msg("wm_3", ("tm_wren", "Wren", "m_1"), ("tm_juno", "Juno"), "two?", false, false, true),
        ],
    );
    {
        let st = relay.host().st.lock().unwrap();
        assert_eq!(st.starts.len(), 1);
        assert_eq!(st.starts[0].relay.answering, vec!["wm_2", "wm_3"]);
        assert_eq!(st.starts[0].follow_up_of, None);
        assert_eq!(st.remembered_hubs, vec![("tm_juno".into(), "m_1".into(), true)]);
    }

    // Outside exchange -> continues Juno's hub
    let wren2 = sharing_wren("m_outside", None);
    relay.on_shared(
        &wren2,
        &[msg("wm_4", ("tm_wren", "Wren", "m_outside"), ("tm_juno", "Juno"), "next job?", false, false, true)],
    );
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.starts[1].follow_up_of.as_deref(), Some("m_1"));
    assert_eq!(st.remembered_hubs[1], ("tm_juno".into(), "m_2".into(), false));
}

#[test]
fn conversation_model_is_kept_when_replied_to_and_borrows_when_none() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        // Recipient without route borrows sender's (workspace-write -> accept-edits, no effort/account)
        st.peers.insert("tm_blank".into(), peer("tm_blank", "Blank", "Helper", None));
        // Inside exchange, m_juno_custom picked a different model and None (account-default)
        st.routes.insert(
            "m_juno_custom".into(),
            route("codex", None, "accept-edits"),
        );
    }

    relay.on_shared(
        &sharing_wren("m_root", None),
        &[msg("wm_1", ("tm_wren", "Wren", "m_root"), ("tm_blank", "Blank"), "go?", false, false, true)],
    );
    {
        let st = relay.host().st.lock().unwrap();
        assert_eq!(st.starts[0].runtime, "codex");
        assert_eq!(st.starts[0].mode, "accept-edits");
        assert_eq!(st.starts[0].model.as_deref(), Some("gpt-5.4"));
        assert!(notices(&st).iter().any(|n| {
            n.contains("Blank has no model of their own yet, so this reply uses Wren's (")
                && n.contains(") and may edit this folder.")
        }));
    }

    // Now Juno replies inside an exchange where lastMissionOf has Juno -> m_juno_custom
    let reply_origin = RelayOrigin {
        hop: 1,
        root_mission_id: Some("m_root".into()),
        last_mission_of: vec![
            ("tm_juno".into(), "m_juno_custom".into()),
            ("tm_wren".into(), "m_wren_mid".into()),
        ],
        relayed_to: vec![],
        answering: vec![],
    };
    relay.on_shared(
        &sharing_wren("m_wren_mid", Some(reply_origin)),
        &[msg("wm_2", ("tm_wren", "Wren", "m_wren_mid"), ("tm_juno", "Juno"), "here is the answer", false, false, false)],
    );
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.starts[1].runtime, "codex");
    assert_eq!(st.starts[1].model, None);
    assert_eq!(st.starts[1].follow_up_of.as_deref(), Some("m_juno_custom"));
}

#[test]
fn cursor_widens_only_when_machine_cannot_hold_read_only_and_appends_read_only_sentence() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.cursor_holds_read_only = false;
        st.peers.insert(
            "tm_cur".into(),
            peer("tm_cur", "Cur", "Review", Some(route("cursor", Some("auto"), "ask"))),
        );
        st.peers.insert(
            "tm_ask".into(),
            peer("tm_ask", "Asker", "Review", Some(route("codex", Some("gpt-5.4"), "ask"))),
        );
    }

    relay.on_shared(
        &sharing_wren("m_1", None),
        &[
            msg("wm_1", ("tm_wren", "Wren", "m_1"), ("tm_cur", "Cur"), "check?", false, false, true),
            msg("wm_2", ("tm_wren", "Wren", "m_1"), ("tm_ask", "Asker"), "check?", false, false, true),
        ],
    );
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.starts[0].mode, "accept-edits");
    assert!(notices(&st).iter().any(|n| n.contains("replies in Edit mode rather than read-only: Cursor Agent cannot be held read-only on this system.")));
    assert_eq!(st.starts[1].mode, "ask");
    assert!(st.starts[1].prompt.contains(read_only_reply_sentence()));
}

#[test]
fn budget_counts_across_branches_room_posts_and_held_replies() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.hop_cap = 2;
        // Room post unites m_branch_a and m_branch_b into one exchange root "room_1".
        st.exchange_map.insert("m_branch_a".into(), "room_1".into());
        st.exchange_map.insert("m_branch_b".into(), "room_1".into());
    }

    // Branch A starts hop 1 -> spend = 1
    relay.on_shared(
        &sharing_wren("m_branch_a", None),
        &[msg("wm_a", ("tm_wren", "Wren", "m_branch_a"), ("tm_juno", "Juno"), "a?", false, false, true)],
    );
    // Branch B starts hop 1 (but busy -> deferred! releases slot, then later cap check sees spend)
    // First test non-busy parallel spend: Branch B starts Pip -> spend = 2 (equals cap = 2)
    relay.on_shared(
        &sharing_wren("m_branch_b", None),
        &[msg("wm_b", ("tm_wren", "Wren", "m_branch_b"), ("tm_pip", "Pip"), "b?", false, false, true)],
    );
    // Third share under same room_1 at hop 0 is STILL stopped by spent = 2 >= cap = 2!
    relay.on_shared(
        &sharing_wren("m_branch_b", None),
        &[msg("wm_c", ("tm_wren", "Wren", "m_branch_b"), ("tm_juno", "Juno"), "c?", false, false, true)],
    );
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.starts.len(), 2);
    assert!(notices(&st).iter().any(|n| n.contains("Stopped after 2 automatic replies.")));

    // Held reply recomputes budget when delivered after spend moved past cap!
    drop(st);
    let relay2 = Relay::new(MockHost::new());
    {
        let mut st = relay2.host().st.lock().unwrap();
        st.hop_cap = 1;
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }
    relay2.on_shared(
        &sharing_wren("m_parent", None),
        &[msg("wm_h", ("tm_wren", "Wren", "m_parent"), ("tm_juno", "Juno"), "hold me?", false, false, true)],
    );
    // While held, another message under m_parent spends the 1 slot (to Pip)
    relay2.on_shared(
        &sharing_wren("m_parent", None),
        &[msg("wm_p", ("tm_wren", "Wren", "m_parent"), ("tm_pip", "Pip"), "take slot?", false, false, true)],
    );
    // Now Juno's running mission ends -> held reply sees spent = 1 >= cap = 1 and does NOT overshoot!
    relay2.on_run_ended(&EndedMission {
        mission_id: "m_juno_prev".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    let st2 = relay2.host().st.lock().unwrap();
    // 1 attempted start to Juno (Busy), 1 start to Pip, 0 delivered for held Juno
    assert_eq!(st2.starts.len(), 2);
    assert!(notices(&st2).iter().any(|n| n.contains("Stopped after 1 automatic reply.")));
}

#[test]
fn busy_teammate_defers_steers_or_interrupts_and_tells_every_held_thread_on_refusal() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.steer_returns.insert("tm_juno".into(), true);
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }

    // Urgent message when runtime steers -> shows notice, does NOT stop
    relay.on_shared(
        &sharing_wren("m_a", None),
        &[msg("wm_1", ("tm_wren", "Wren", "m_a"), ("tm_juno", "Juno"), "stop!<locust-task>x</locust-task>", true, false, true)],
    );
    // Second thread also shares to busy Juno -> goes to held_also
    let mut pip_sender = sharing_wren("m_b", None);
    pip_sender.peer = peer("tm_pip", "Pip", "Reviewer", None);
    relay.on_shared(
        &pip_sender,
        &[msg("wm_2", ("tm_pip", "Pip", "m_b"), ("tm_juno", "Juno"), "also waiting?", false, false, true)],
    );
    {
        let mut st = relay.host().st.lock().unwrap();
        assert_eq!(st.steered_calls.len(), 1);
        assert!(!st.steered_calls[0].1.contains("<locust-task>"));
        assert!(st.stopped_calls.is_empty());
        assert!(notices(&st).iter().any(|n| n.contains("was shown this part-way through their run, without stopping it")));
        // When Juno finishes, start is refused -> BOTH m_a and m_b hear!
        st.next_start_results.push_back(Ok(StartResult::Refused {
            message: "quota empty".into(),
        }));
    }

    relay.on_run_ended(&EndedMission {
        mission_id: "m_juno_running".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    let st = relay.host().st.lock().unwrap();
    assert!(notices(&st).iter().any(|n| n.contains("could not reply on their own: quota empty")));
    assert!(st.notes.iter().any(|(m, msg)| m == "m_b" && msg.contains("could not reply on their own")));
}

#[test]
fn urgent_interrupt_falls_back_to_stop_only_when_allowed() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.may_interrupt = false;
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }
    relay.on_shared(
        &sharing_wren("m_1", None),
        &[msg("wm_1", ("tm_wren", "Wren", "m_1"), ("tm_juno", "Juno"), "now!", true, false, true)],
    );
    {
        let mut st = relay.host().st.lock().unwrap();
        assert!(st.stopped_calls.is_empty());
        assert!(notices(&st).iter().any(|n| n.contains("Teammates interrupting each other is switched off in Settings")));
        st.may_interrupt = true;
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }
    relay.on_shared(
        &sharing_wren("m_3", None),
        &[msg("wm_3", ("tm_wren", "Wren", "m_3"), ("tm_pip", "Pip"), "urgent!", true, false, true)],
    );
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.stopped_calls, vec!["tm_pip"]);
    assert!(notices(&st).iter().any(|n| n.contains("Pip was stopped part-way so they can take this next.")));
}

#[test]
fn pool_busy_defers_and_drains_when_any_other_teammate_ends() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: true }));
    }
    relay.on_shared(
        &sharing_wren("m_1", None),
        &[msg("wm_1", ("tm_wren", "Wren", "m_1"), ("tm_juno", "Juno"), "help?", false, false, true)],
    );
    assert!(notices(&relay.host().st.lock().unwrap())
        .iter()
        .any(|n| n.contains("Every run slot is in use, so Juno's reply starts when one frees up.")));

    // Unrelated Pip ends -> Juno starts!
    relay.on_run_ended(&EndedMission {
        mission_id: "m_other".into(),
        peer: Some(peer("tm_pip", "Pip", "Reviewer", None)),
        relay: None,
    });
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.starts.len(), 2);
    assert_eq!(st.starts[1].peer.teammate_id, "tm_juno");
}

#[test]
fn silent_question_brings_answer_back_unless_cut_off_or_fyi() {
    // Case 1: Question + completed with prose -> brought back
    let relay = Relay::new(MockHost::new());
    relay.on_shared(
        &sharing_wren("m_ask", None),
        &[msg("wm_q", ("tm_wren", "Wren", "m_ask"), ("tm_juno", "Juno"), "What port?", false, false, true)],
    );
    let started_mission = relay.host().st.lock().unwrap().starts[0].relay.clone();
    assert_eq!(started_mission.hop, 1);
    relay
        .host()
        .st
        .lock()
        .unwrap()
        .final_replies
        .insert("m_1".into(), "Port 8080.\n<locust-memory>x</locust-memory>".into());
    relay.on_run_ended(&EndedMission {
        mission_id: "m_1".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: Some(started_mission),
    });
    {
        let st = relay.host().st.lock().unwrap();
        assert_eq!(st.posted_returns.len(), 1);
        assert!(st.posted_returns[0].5.starts_with(RETURNED_ANSWER));
        assert!(st.posted_returns[0].5.contains("Port 8080."));
        assert!(!st.posted_returns[0].5.contains("<locust-memory>"));
        assert_eq!(st.starts.len(), 2);
        assert_eq!(st.starts[1].peer.teammate_id, "tm_wren");
        assert!(st.starts[1].prompt.contains("Juno answered your message in their own conversation"));
    }

    // Case 2: Cut off (`how_ended` is Some) -> NOT brought back, says stopped before writing back
    let relay_cut = Relay::new(MockHost::new());
    relay_cut.on_shared(
        &sharing_wren("m_ask2", None),
        &[msg("wm_q2", ("tm_wren", "Wren", "m_ask2"), ("tm_juno", "Juno"), "What port?", false, false, true)],
    );
    {
        let mut st = relay_cut.host().st.lock().unwrap();
        st.final_replies.insert("m_1".into(), "Partial output".into());
        st.how_ended_map.insert("m_1".into(), "usage limit reached".into());
    }
    relay_cut.on_run_ended(&EndedMission {
        mission_id: "m_1".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    {
        let st = relay_cut.host().st.lock().unwrap();
        assert!(st.posted_returns.is_empty());
        assert_eq!(st.starts.len(), 1);
        assert!(notices(&st).iter().any(|n| n.contains("Juno stopped before writing back: usage limit reached.")));
    }

    // Case 3: FYI (`wants_answer = false`) -> NOT brought back even when prose exists
    let relay_fyi = Relay::new(MockHost::new());
    relay_fyi.on_shared(
        &sharing_wren("m_fyi", None),
        &[msg("wm_fyi", ("tm_wren", "Wren", "m_fyi"), ("tm_juno", "Juno"), "FYI deploy done.", false, false, false)],
    );
    relay_fyi
        .host()
        .st
        .lock()
        .unwrap()
        .final_replies
        .insert("m_1".into(), "Noted!".into());
    relay_fyi.on_run_ended(&EndedMission {
        mission_id: "m_1".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    let st_fyi = relay_fyi.host().st.lock().unwrap();
    assert!(st_fyi.posted_returns.is_empty());
    assert!(notices(&st_fyi).iter().any(|n| n.contains("Juno finished without writing back.")));
}

#[test]
fn meeting_waits_for_all_recipients_and_closes_once() {
    let relay = Relay::new(MockHost::new());
    relay.on_shared(
        &sharing_wren("m_wren", None),
        &[
            msg("wm_j", ("tm_wren", "Wren", "m_wren"), ("tm_juno", "Juno"), "thoughts?", false, false, true),
            msg("wm_p", ("tm_wren", "Wren", "m_wren"), ("tm_pip", "Pip"), "thoughts?", false, false, true),
        ],
    );
    assert!(notices(&relay.host().st.lock().unwrap())
        .iter()
        .any(|n| n == "Waiting on Juno, Pip to reply before your next turn."));

    // Juno shares back to Wren -> held, waiting on Pip
    let juno_mission = SharingMission {
        run_id: "run_1".into(),
        mission_id: "m_1".into(),
        runtime: "claude".into(),
        sandbox: "read-only".into(),
        model: Some("claude-sonnet-4-6".into()),
        peer: peer("tm_juno", "Juno", "Frontend", None),
        relay: Some(relay.host().st.lock().unwrap().starts[0].relay.clone()),
        started_by: None,
    };
    relay.on_shared(
        &juno_mission,
        &[msg("wm_back", ("tm_juno", "Juno", "m_1"), ("tm_wren", "Wren"), "LGTM", false, false, false)],
    );
    {
        let st = relay.host().st.lock().unwrap();
        assert_eq!(st.starts.len(), 2);
        assert!(notices(&st).iter().any(|n| n == "Juno replied. Still waiting on Pip."));
    }

    // Pip ends without replying -> meeting closes once for Wren mentioning Pip silent!
    relay.on_run_ended(&EndedMission {
        mission_id: "m_2".into(),
        peer: Some(peer("tm_pip", "Pip", "Reviewer", None)),
        relay: None,
    });
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.starts.len(), 3);
    assert_eq!(st.starts[2].peer.teammate_id, "tm_wren");
    assert!(st.starts[2].prompt.contains("Juno replied to your message"));
    assert!(st.starts[2].prompt.contains("Pip finished without replying."));
    assert!(st.starts[2].prompt.contains("The person who started this conversation reads it and is waiting on this"));
}

#[test]
fn only_the_face_you_talk_to_hops_and_nested_under_sits_one_level_deep() {
    let rows = vec![
        NestedRow { mission_id: "person".into(), member_ids: vec!["person".into(), "person_followup".into()] },
        NestedRow { mission_id: "juno_reply".into(), member_ids: vec!["juno_reply".into()] },
        NestedRow { mission_id: "pip_reply".into(), member_ids: vec!["pip_reply".into()] },
        NestedRow { mission_id: "orphan".into(), member_ids: vec!["orphan".into()] },
        // Cycle A <-> B
        NestedRow { mission_id: "cyc_a".into(), member_ids: vec!["cyc_a".into()] },
        NestedRow { mission_id: "cyc_b".into(), member_ids: vec!["cyc_b".into()] },
    ];
    let map = nested_under(&rows, |row| match row.mission_id.as_str() {
        "juno_reply" => Some("person_followup".into()),
        "pip_reply" => Some("juno_reply".into()), // second level -> stays unnested
        "orphan" => Some("deleted_mission".into()),
        "cyc_a" => Some("cyc_b".into()),
        "cyc_b" => Some("cyc_a".into()),
        _ => None,
    });
    assert_eq!(map.get("juno_reply").map(String::as_str), Some("person"));
    assert_eq!(map.get("pip_reply"), None);
    assert_eq!(map.get("orphan"), None);
    assert_eq!(map.get("cyc_a"), None);
    assert_eq!(map.get("cyc_b"), None);
}

#[test]
fn exchange_of_and_exchange_across_walk_component_and_sum_cost_and_hops() {
    let m1 = ExchangeMission {
        mission_id: "m1".into(),
        runtime: "codex".into(),
        model: "gpt-5.4".into(),
        cost: Some(RunCost { usd: Some(0.12), ..Default::default() }),
        peer_messages: vec![ExchangePeerMessage { message_id: "wm1".into() }],
        started_by_hop: None,
        teammate_id: Some("tm_wren".into()),
        teammate_name: Some("Wren".into()),
        live: false,
        run_id: Some("r1".into()),
    };
    let m2 = ExchangeMission {
        mission_id: "m2".into(),
        runtime: "claude".into(),
        model: "claude-sonnet-4-6".into(),
        cost: Some(RunCost { usd: Some(0.08), ..Default::default() }),
        peer_messages: vec![
            ExchangePeerMessage { message_id: "wm1".into() },
            ExchangePeerMessage { message_id: "wm2".into() },
        ],
        started_by_hop: Some(1),
        teammate_id: Some("tm_juno".into()),
        teammate_name: Some("Juno".into()),
        live: true,
        run_id: Some("r2".into()),
    };
    let m3_room_other = ExchangeMission {
        mission_id: "m3".into(),
        runtime: "codex".into(),
        model: "o3".into(),
        cost: None,
        peer_messages: vec![ExchangePeerMessage { message_id: "wm2".into() }],
        started_by_hop: Some(2),
        teammate_id: Some("tm_wren".into()),
        teammate_name: Some("Wren".into()),
        live: false,
        run_id: None,
    };
    let solo = ExchangeMission {
        mission_id: "solo".into(),
        runtime: "codex".into(),
        model: "gpt-5.4".into(),
        cost: None,
        peer_messages: vec![],
        started_by_hop: None,
        teammate_id: None,
        teammate_name: None,
        live: false,
        run_id: None,
    };
    let all = vec![m1, m2, m3_room_other, solo];
    assert_eq!(exchange_of("solo", &all), None);

    let ov = exchange_of("m1", &all).unwrap();
    assert_eq!(ov.hops, 2);
    assert_eq!(ov.live_run_ids, vec!["r2"]);
    assert_eq!(ov.participants.len(), 2);
    // Wren's latest turn in walk is m3 (o3) or updated along walk:
    let wren = ov.participants.iter().find(|p| p.teammate_id == "tm_wren").unwrap();
    assert_eq!(wren.mission_ids.len(), 2);
    assert!((ov.cost.unwrap().usd.unwrap() - 0.20).abs() < 1e-6);

    // Separate room roots joined via exchange_across
    let r_a = ExchangeMission {
        mission_id: "ra".into(),
        runtime: "codex".into(),
        model: "m1".into(),
        cost: None,
        peer_messages: vec![ExchangePeerMessage { message_id: "w_room".into() }],
        started_by_hop: Some(1),
        teammate_id: Some("tm_a".into()),
        teammate_name: Some("A".into()),
        live: false,
        run_id: None,
    };
    let r_b = ExchangeMission {
        mission_id: "rb".into(),
        runtime: "claude".into(),
        model: "m2".into(),
        cost: None,
        peer_messages: vec![],
        started_by_hop: None,
        teammate_id: Some("tm_b".into()),
        teammate_name: Some("B".into()),
        live: false,
        run_id: None,
    };
    let merged = exchange_across(&["ra", "rb"], &[r_a, r_b]).unwrap();
    assert_eq!(merged.participants.len(), 2);
}

#[test]
fn out_of_plan_refuses_notes_and_skips_start_while_missing_peer_releases_budget() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.hop_cap = 1;
        st.out_of_plan_map
            .insert("claude".into(), "Out until 20 Nov.".into());
    }
    // First share hits missing peer -> releases spend so next valid share can still use the budget
    relay.on_shared(
        &sharing_wren("m_1", None),
        &[
            msg("wm_missing", ("tm_wren", "Wren", "m_1"), ("tm_ghost", "Ghost"), "hi?", false, false, true),
            msg("wm_juno", ("tm_wren", "Wren", "m_1"), ("tm_juno", "Juno"), "go?", false, false, true),
        ],
    );
    let st = relay.host().st.lock().unwrap();
    assert!(st.starts.is_empty());
    assert!(notices(&st).iter().any(|n| n.contains("Ghost is no longer on the roster; nothing was started.")));
    assert!(notices(&st).iter().any(|n| n.contains("Juno did not reply on their own: Out until 20 Nov.")));
    assert!(st.notes.iter().any(|(m, txt)| m == "m_1" && txt.contains("Out until 20 Nov.")));
}

#[test]
fn still_waiting_false_skips_deferred_delivery_and_eviction_keeps_live_exchanges() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
        st.still_waiting_map.insert("tm_juno".into(), false);
    }
    relay.on_shared(
        &sharing_wren("m_busy", None),
        &[msg("wm_1", ("tm_wren", "Wren", "m_busy"), ("tm_juno", "Juno"), "hold", false, false, true)],
    );
    relay.on_run_ended(&EndedMission {
        mission_id: "m_juno_done".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    assert_eq!(relay.host().st.lock().unwrap().starts.len(), 1);

    // Eviction test across > EXCHANGES_REMEMBERED exchanges: recent exchange retains accurate count
    let relay2 = Relay::new(MockHost::new());
    {
        let mut st = relay2.host().st.lock().unwrap();
        st.hop_cap = 2;
    }
    for i in 0..405 {
        relay2.on_shared(
            &sharing_wren(&format!("old_{i}"), None),
            &[msg(&format!("w_{i}"), ("tm_wren", "Wren", &format!("old_{i}")), ("tm_juno", "Juno"), "q?", false, false, true)],
        );
    }
    relay2.on_shared(
        &sharing_wren("live_root", None),
        &[
            msg("wl_1", ("tm_wren", "Wren", "live_root"), ("tm_juno", "Juno"), "q1?", false, false, true),
            msg("wl_2", ("tm_wren", "Wren", "live_root"), ("tm_pip", "Pip"), "q2?", false, false, true),
        ],
    );
    // Third under live_root MUST be stopped because spend=2 is retained!
    relay2.on_shared(
        &sharing_wren("live_root", None),
        &[msg("wl_3", ("tm_wren", "Wren", "live_root"), ("tm_juno", "Juno"), "q3?", false, false, true)],
    );
    assert!(notices(&relay2.host().st.lock().unwrap())
        .iter()
        .any(|n| n.contains("Stopped after 2 automatic replies.")));
}

#[test]
fn deliver_deferred_rereads_enabled_and_briefs_each_teammate_rules_once() {
    let relay = Relay::new(MockHost::new());
    {
        let mut st = relay.host().st.lock().unwrap();
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }
    relay.on_shared(
        &sharing_wren("m_1", None),
        &[msg("wm_1", ("tm_wren", "Wren", "m_1"), ("tm_juno", "Juno"), "q?", false, false, true)],
    );
    // Person toggles replies OFF while Juno was busy
    relay.host().st.lock().unwrap().enabled = false;
    relay.on_run_ended(&EndedMission {
        mission_id: "m_juno_was_running".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    {
        let st = relay.host().st.lock().unwrap();
        assert_eq!(st.starts.len(), 1);
        assert!(notices(&st).iter().any(|n| n.contains("Teammate replies are switched off in Settings")));
    }

    // Re-enable and verify briefedBefore + relayedTo chain
    relay.host().st.lock().unwrap().enabled = true;
    let hop1_origin = RelayOrigin {
        hop: 1,
        root_mission_id: Some("m_wren_0".into()),
        last_mission_of: vec![
            ("tm_wren".into(), "m_wren_0".into()),
            ("tm_juno".into(), "m_juno_1".into()),
        ],
        relayed_to: vec![],
        answering: vec![],
    };
    let juno_sender = SharingMission {
        run_id: "r_j".into(),
        mission_id: "m_juno_1".into(),
        runtime: "claude".into(),
        sandbox: "read-only".into(),
        model: None,
        peer: peer("tm_juno", "Juno", "Frontend", None),
        relay: Some(hop1_origin),
        started_by: None,
    };
    // Hop 2 -> Wren is added to relayed_to
    relay.on_shared(
        &juno_sender,
        &[msg("wm_h2", ("tm_juno", "Juno", "m_juno_1"), ("tm_wren", "Wren"), "question?", false, false, true)],
    );
    let hop2_origin = relay.host().st.lock().unwrap().starts.last().unwrap().relay.clone();
    assert!(hop2_origin.relayed_to.contains(&"tm_wren".to_owned()));
    // Hop 3 -> Wren replies, then hop 4 back to Wren sees briefed_before = true!
    relay.on_shared(
        &sharing_wren("m_wren_2", Some(hop2_origin)),
        &[msg("wm_h3", ("tm_wren", "Wren", "m_wren_2"), ("tm_juno", "Juno"), "next?", false, false, true)],
    );
    let hop3_origin = relay.host().st.lock().unwrap().starts.last().unwrap().relay.clone();
    assert!(hop3_origin.relayed_to.contains(&"tm_wren".to_owned()));
    assert!(hop3_origin.relayed_to.contains(&"tm_juno".to_owned()));
    let juno_sender2 = SharingMission {
        run_id: "r_j2".into(),
        mission_id: "m_juno_3".into(),
        runtime: "claude".into(),
        sandbox: "read-only".into(),
        model: None,
        peer: peer("tm_juno", "Juno", "Frontend", None),
        relay: Some(hop3_origin),
        started_by: None,
    };
    relay.on_shared(
        &juno_sender2,
        &[msg("wm_h4", ("tm_juno", "Juno", "m_juno_3"), ("tm_wren", "Wren"), "reply?", false, false, true)],
    );
    let st = relay.host().st.lock().unwrap();
    let final_prompt = &st.starts.last().unwrap().prompt;
    assert!(final_prompt.contains("The rules for replies from your earlier turn in this exchange still hold"));
}

#[test]
fn return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy() {
    // 1) return_answer preserves relayed_to and releases budget / defers when asker is Busy.
    let relay = Relay::new(MockHost::new());
    let origin = RelayOrigin {
        hop: 1,
        root_mission_id: Some("m_ask".into()),
        last_mission_of: vec![("tm_wren", "m_ask"), ("tm_juno", "m_1")]
            .into_iter()
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect(),
        relayed_to: vec!["tm_pip".into()],
        answering: vec![],
    };
    relay.on_shared(
        &sharing_wren("m_ask", Some(origin)),
        &[msg("wm_q", ("tm_wren", "Wren", "m_ask"), ("tm_juno", "Juno"), "Port?", false, false, true)],
    );
    {
        let mut st = relay.host().st.lock().unwrap();
        st.hop_cap = 3;
        st.final_replies.insert("m_1".into(), "Port 4000.".into());
        // Asker Wren is busy when Juno ends without writing back:
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }
    relay.on_run_ended(&EndedMission {
        mission_id: "m_1".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    // Wren's busy defer releases the slot (hop 1 spent only) and holds reply; when Wren ends, it starts with relayed_to=["tm_pip"]!
    relay.on_run_ended(&EndedMission {
        mission_id: "m_wren_busy".into(),
        peer: Some(peer("tm_wren", "Wren", "Code & Migrations", None)),
        relay: None,
    });
    {
        let st = relay.host().st.lock().unwrap();
        assert_eq!(st.starts.len(), 3);
        let ret_start = st.starts.last().unwrap();
        assert_eq!(ret_start.peer.teammate_id, "tm_wren");
        assert!(ret_start.relay.relayed_to.contains(&"tm_pip".to_owned()));
        assert!(notices(&st).iter().any(|n| n.contains("Wren is part-way through another mission")));
    }

    // 2) return_answer Refused says "to read on their next run" and releases budget.
    let relay_ref = Relay::new(MockHost::new());
    relay_ref.on_shared(
        &sharing_wren("m_ask", None),
        &[msg("wm_q2", ("tm_wren", "Wren", "m_ask"), ("tm_juno", "Juno"), "Port?", false, false, true)],
    );
    {
        let mut st = relay_ref.host().st.lock().unwrap();
        st.hop_cap = 2;
        st.final_replies.insert("m_1".into(), "Port 4001.".into());
        st.next_start_results.push_back(Ok(StartResult::Refused {
            message: "quota empty".into(),
        }));
    }
    relay_ref.on_run_ended(&EndedMission {
        mission_id: "m_1".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    {
        let st = relay_ref.host().st.lock().unwrap();
        assert!(notices(&st).iter().any(|n| n.contains("to read on their next run.")));
    }
    // Refused return_answer MUST have released its reserved hop back to 1 (< cap 2), so another share starts:
    relay_ref.on_shared(
        &sharing_wren("m_ask", None),
        &[msg("wm_q2_follow", ("tm_wren", "Wren", "m_ask"), ("tm_pip", "Pip"), "Follow-up?", false, false, false)],
    );
    assert_eq!(relay_ref.host().st.lock().unwrap().starts.len(), 3);

    // 3) close_meeting preserves relayed_to, releases budget & defers on Busy.
    let relay_meet = Relay::new(MockHost::new());
    let meet_origin = RelayOrigin {
        hop: 1,
        root_mission_id: Some("m_meet".into()),
        last_mission_of: vec![("tm_wren".into(), "m_meet".into())],
        relayed_to: vec!["tm_juno".into()],
        answering: vec![],
    };
    relay_meet.on_shared(
        &sharing_wren("m_meet", Some(meet_origin)),
        &[
            msg("wm_j", ("tm_wren", "Wren", "m_meet"), ("tm_juno", "Juno"), "q1?", false, false, true),
            msg("wm_p", ("tm_wren", "Wren", "m_meet"), ("tm_pip", "Pip"), "q2?", false, false, true),
        ],
    );
    let r_juno = relay_meet.host().st.lock().unwrap().starts[0].relay.clone();
    relay_meet.on_shared(
        &SharingMission {
            run_id: "run_1".into(),
            mission_id: "m_1".into(),
            runtime: "claude".into(),
            sandbox: "read-only".into(),
            model: None,
            peer: peer("tm_juno", "Juno", "Frontend", None),
            relay: Some(r_juno),
            started_by: None,
        },
        &[msg("wm_b1", ("tm_juno", "Juno", "m_1"), ("tm_wren", "Wren"), "a1", false, false, false)],
    );
    {
        let mut st = relay_meet.host().st.lock().unwrap();
        st.hop_cap = 3;
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }
    relay_meet.on_run_ended(&EndedMission {
        mission_id: "m_2".into(),
        peer: Some(peer("tm_pip", "Pip", "Reviewer", None)),
        relay: None,
    });
    // Wren was busy when meeting closed -> held, then started on Wren end with preserved relayed_to:
    relay_meet.on_run_ended(&EndedMission {
        mission_id: "m_wren_m".into(),
        peer: Some(peer("tm_wren", "Wren", "Code & Migrations", None)),
        relay: None,
    });
    let st_m = relay_meet.host().st.lock().unwrap();
    assert_eq!(st_m.starts.len(), 4);
    let meet_start = st_m.starts.last().unwrap();
    assert_eq!(meet_start.peer.teammate_id, "tm_wren");
    assert!(meet_start.relay.relayed_to.contains(&"tm_juno".to_owned()));
}

#[test]
fn deliver_deferred_requeues_restores_exchange_releases_budget_and_notifies_also_waiting() {
    // 1) deliver_deferred releases budget on non-Started (Refused) so a follow-up share can use cap=1.
    let r_rel = Relay::new(MockHost::new());
    {
        let mut st = r_rel.host().st.lock().unwrap();
        st.hop_cap = 1;
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
        st.next_start_results.push_back(Ok(StartResult::Refused {
            message: "down".into(),
        }));
    }
    r_rel.on_shared(
        &sharing_wren("m_rel", None),
        &[msg("w_r1", ("tm_wren", "Wren", "m_rel"), ("tm_juno", "Juno"), "q1?", false, false, true)],
    );
    r_rel.on_run_ended(&EndedMission {
        mission_id: "m_j0".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    r_rel.on_shared(
        &sharing_wren("m_rel", None),
        &[msg("w_r2", ("tm_wren", "Wren", "m_rel"), ("tm_pip", "Pip"), "q2?", false, false, false)],
    );
    assert_eq!(r_rel.host().st.lock().unwrap().starts.len(), 3);

    // 2) deliver_deferred re-queues on Busy, restores exchanges & moves held_also to also_waiting on Started.
    let r_def = Relay::new(MockHost::new());
    {
        let mut st = r_def.host().st.lock().unwrap();
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
        st.next_start_results.push_back(Ok(StartResult::Busy { pool: false }));
    }
    r_def.on_shared(
        &sharing_wren("m_w", None),
        &[msg("w_d1", ("tm_wren", "Wren", "m_w"), ("tm_juno", "Juno"), "help?", false, false, true)],
    );
    let mut pip_m = sharing_wren("m_p", None);
    pip_m.peer = peer("tm_pip", "Pip", "Reviewer", None);
    r_def.on_shared(
        &pip_m,
        &[msg("w_d2", ("tm_pip", "Pip", "m_p"), ("tm_juno", "Juno"), "also?", false, false, true)],
    );
    // 1st end -> deliver_deferred gets Busy again -> MUST re-queue.
    r_def.on_run_ended(&EndedMission {
        mission_id: "m_busy_1".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    assert_eq!(r_def.host().st.lock().unwrap().starts.len(), 3);
    // 2nd end -> deliver_deferred now starts m_1 and restores exchange + also_waiting.
    r_def.on_run_ended(&EndedMission {
        mission_id: "m_busy_2".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    {
        let mut st = r_def.host().st.lock().unwrap();
        assert_eq!(st.starts.len(), 4);
        st.final_replies.insert("m_1".into(), "All fixed.".into());
    }
    r_def.on_run_ended(&EndedMission {
        mission_id: "m_1".into(),
        peer: Some(peer("tm_juno", "Juno", "Frontend", None)),
        relay: None,
    });
    let st = r_def.host().st.lock().unwrap();
    assert!(st.posted_returns.iter().any(|(_, _, _, to, _, txt)| to == "tm_wren" && txt.contains("All fixed.")));
    assert!(st.posted_returns.iter().any(|(_, _, _, to, _, txt)| to == "tm_pip" && txt.contains("All fixed.")));
}
