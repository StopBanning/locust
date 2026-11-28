use super::*;

const CHOSEN: &str = "opencode/nemotron-3.5-lightning-free";
const SAVED: &str = "opencode/deepseek-v4.1-flash-free";

fn relay(known: bool) -> Relay<MockHost> {
    let host = MockHost::new();
    {
        let mut st = host.st.lock().unwrap();
        st.peers.insert("tm_wren".into(), peer("tm_wren", "Wren", "Backend", Some(route("opencode", Some(SAVED), "accept-edits"))));
        if known {
            st.routes.insert("m_asker".into(), route("opencode", Some(CHOSEN), "accept-edits"));
        }
        // This must never influence a recipient who has no earlier turn in the exchange.
        st.routes.insert("m_juno_unrelated".into(), route("codex", Some("account-default"), "plan"));
    }
    Relay::new(host)
}

fn ask(relay: &Relay<MockHost>, meeting: bool) {
    let mut questions = vec![msg("wm_j", ("tm_wren", "Wren", "m_asker"), ("tm_juno", "Juno"), "What did you find?", false, false, true)];
    if meeting {
        questions.push(msg("wm_p", ("tm_wren", "Wren", "m_asker"), ("tm_pip", "Pip"), "What did you find?", false, false, true));
    }
    relay.on_shared(&sharing_wren("m_asker", None), &questions);
}

fn reply(relay: &Relay<MockHost>, index: usize, id: &str, name: &str) {
    let origin = relay.host().st.lock().unwrap().starts[index].relay.clone();
    let mission = format!("m_{}", index + 1);
    let sender = SharingMission {
        run_id: format!("run_{}", index + 1), mission_id: mission.clone(),
        runtime: "claude".into(), sandbox: "read-only".into(), model: Some("sonnet".into()),
        peer: peer(id, name, "Reviewer", None), relay: Some(origin), started_by: None,
    };
    let outgoing_id = format!("reply_{index}");
    relay.on_shared(&sender, &[msg(&outgoing_id, (id, name, &mission), ("tm_wren", "Wren"), "Here is what I found.", false, false, false)]);
}

fn return_answer(relay: &Relay<MockHost>) {
    let origin = {
        let mut st = relay.host().st.lock().unwrap();
        st.final_replies.insert("m_1".into(), "Port 8080.".into());
        st.starts[0].relay.clone()
    };
    relay.on_run_ended(&EndedMission {
        mission_id: "m_1".into(), peer: Some(peer("tm_juno", "Juno", "Reviewer", None)), relay: Some(origin),
    });
}

fn resumed(relay: &Relay<MockHost>, index: usize, model: &str) -> String {
    let st = relay.host().st.lock().unwrap();
    assert_eq!(st.starts.len(), index + 1);
    let run = &st.starts[index];
    assert_eq!(run.peer.teammate_id, "tm_wren");
    assert_eq!(run.follow_up_of.as_deref(), Some("m_asker"));
    assert_eq!((&*run.runtime, run.model.as_deref(), &*run.mode), ("opencode", Some(model), "accept-edits"));
    run.prompt.clone()
}

// case: the answer brought back starts the asker on the model picked there, not the one on their profile
#[test]
fn sweep_returned_answer_uses_the_askers_selected_model() {
    let relay = relay(true);
    ask(&relay, false);
    return_answer(&relay);
    resumed(&relay, 1, CHOSEN);
}

// case: so does a reply written back
#[test]
fn sweep_written_reply_uses_the_askers_selected_model() {
    let relay = relay(true);
    ask(&relay, false);
    reply(&relay, 0, "tm_juno", "Juno");
    resumed(&relay, 1, CHOSEN);
}

// case: and so does the turn after a meeting, when several were asked at once
#[test]
fn sweep_meeting_return_uses_the_askers_selected_model() {
    let relay = relay(true);
    ask(&relay, true);
    reply(&relay, 0, "tm_juno", "Juno");
    assert_eq!(relay.host().st.lock().unwrap().starts.len(), 2);
    reply(&relay, 1, "tm_pip", "Pip");
    assert!(resumed(&relay, 2, CHOSEN).contains("Juno and Pip replied to your message"));
}

// case: a teammate with no turn in the exchange answers on their own saved model
#[test]
fn sweep_new_recipient_uses_their_saved_model_not_the_askers_route() {
    let relay = relay(true);
    ask(&relay, false);
    let st = relay.host().st.lock().unwrap();
    let run = &st.starts[0];
    assert_eq!((&*run.runtime, run.model.as_deref(), &*run.mode), ("claude", Some("claude-sonnet-4-6"), "ask"));
    assert_eq!(run.follow_up_of, None);
}

// case: a turn whose route is not known, or cannot be read, continues on the saved one
#[test]
fn sweep_unavailable_earlier_route_uses_the_askers_saved_model() {
    // RelayHost::route_of represents both missing and unreadable ledger data as None.
    let relay = relay(false);
    ask(&relay, false);
    return_answer(&relay);
    resumed(&relay, 1, SAVED);
}
