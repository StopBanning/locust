use super::*;
use crate::engine_api::{EngineApi, InProcess};
use crate::relay::{EndedMission, PeerRosterEntry, Relay, RelayHost, RelayedMessage, RelayUpdate, SharingMission, StartRequest, StartResult, TurnRoute};

fn profile(t: &Temp, model: &str, effort: &str) {
    t.store()
        .save_json("teammates.json", &json!({
            "schemaVersion": 1,
            "teammates": [
                {"teammateId": "tm_codec", "name": "Codec", "role": "Custom", "roleTitle": "Code",
                 "hue": "blue", "createdAt": "2026-11-01T00:00:00.000Z", "avatar": {},
                 "folder": t.0.join("work"), "route": {"runtime": "codex", "model": model, "mode": "ask", "effort": effort}},
                {"teammateId": "tm_penny", "name": "Penny", "role": "Custom", "roleTitle": "Money",
                 "hue": "blue", "createdAt": "2026-11-01T00:00:00.000Z", "avatar": {},
                 "folder": t.0.join("work"), "route": {"runtime": "codex", "model": "gpt-5.5", "mode": "ask"}}
            ],
            "missionOwners": {},
            "settings": {"relay": true}
        }))
        .unwrap();
}

fn from_composer() -> Origin {
    Origin { reference_ids: Some(Vec::new()), ..Default::default() }
}

fn start_picked(t: &Temp, model: &str, effort: Option<&str>, account: Option<String>) -> String {
    let first = InProcess.send(
        Send { model: Some(model.into()), effort: effort.map(str::to_owned), ..job(t, None, "Do work.") },
        Origin { runtime: Some("codex".into()), account: Some(account), mode: Some(Mode::Ask), ..from_composer() },
    ).unwrap();
    finished(&first.conversation_id);
    first.conversation_id
}

fn last_start(t: &Temp) -> Value {
    codex_requests(t).into_iter().filter(|r| r["method"] == "turn/start").last().unwrap()["params"].clone()
}

// case: a conversation shows what it ran on, not the saved model of its teammate
#[test]
fn b07_manual_reply_keeps_the_picked_model_and_level_not_the_profile() {
    let t = Temp::new();
    profile(&t, "gpt-5.5", "high");
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let first = start_picked(&t, "gpt-5.6-luna", Some("low"), None);
    let reply = InProcess.send(
        Send { model: None, ..job(&t, Some(&first), "Continue.") },
        from_composer(),
    ).unwrap();
    finished(&reply.conversation_id);
    let tip = real_tip(&first).unwrap();
    assert_ne!(tip, first);
    let req = last_start(&t);
    assert_eq!(req["model"], "gpt-5.6-luna");
    assert_eq!(req["effort"], "low");
    assert_eq!(records(&path_of(&t, &tip))[0]["metadata"]["model"], "gpt-5.6-luna");
}

/// Penny's answer is supplied by the fixture, as in Electron. Codec's return goes through the native host,
/// all the way to a fake app-server and the real mission ledger.
struct ReturnedAnswer {
    starts: Mutex<Vec<String>>,
}
impl RelayHost for ReturnedAnswer {
    fn enabled(&self) -> bool { true }
    fn peer_context_for(&self, id: &str) -> Option<PeerRosterEntry> { NativeRelayHost.peer_context_for(id) }
    fn final_reply_of(&self, _: &str) -> Option<String> {
        Some("A high-yield savings account pays about 4.4% this month.".into())
    }
    fn post_returned_answer(&self, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str) -> Option<String> {
        Some("wm_returned".into())
    }
    fn route_of(&self, id: &str) -> Option<TurnRoute> { NativeRelayHost.route_of(id) }
    fn start(&self, request: StartRequest) -> Result<StartResult, String> {
        if request.peer.teammate_id == "tm_penny" {
            return Ok(StartResult::Started { run_id: "run_penny".into(), mission_id: "mission_penny".into() });
        }
        let started = NativeRelayHost.start(request)?;
        match &started {
            StartResult::Started { mission_id, .. } => self.starts.lock().unwrap().push(mission_id.clone()),
            _ => panic!("the reply did not start: {started:?}"),
        }
        Ok(started)
    }
    fn assign_owner(&self, _: &str, _: &str) {}
    fn notify(&self, _: RelayUpdate) {}
}

fn bring_the_answer_back(t: &Temp, first: &str) -> String {
    let relay = Relay::new(ReturnedAnswer { starts: Mutex::new(Vec::new()) });
    let host = relay.host();
    let peer = host.peer_context_for("tm_codec").expect("Codec on the roster");
    relay.on_shared(&SharingMission {
        run_id: format!("run_{first}"), mission_id: first.into(),
        runtime: "codex".into(), sandbox: "read-only".into(), model: None,
        peer, relay: None, started_by: None,
    }, &[RelayedMessage {
        message_id: "msg_codec".into(), from_teammate_id: "tm_codec".into(),
        from_name: "Codec".into(), from_mission_id: first.into(), to_teammate_id: "tm_penny".into(),
        to_name: "Penny".into(), text: "What do the savings rates look like this month?".into(),
        urgent: false, defer: false, wants_answer: true,
    }]);
    relay.on_run_ended(&EndedMission {
        mission_id: "mission_penny".into(), peer: host.peer_context_for("tm_penny"), relay: None,
    });
    let started = host.starts.lock().unwrap();
    assert_eq!(started.len(), 1, "the relay returned Penny's answer to Codec");
    let tip = started[0].clone();
    drop(started);
    finished(&tip);
    assert_eq!(records(&path_of(t, &tip))[0]["metadata"]["startedBy"]["kind"], "relay");
    tip
}

// case: its level and account go with it
#[test]
fn b07_automatic_reply_keeps_its_level_and_named_account() {
    let t = Temp::new();
    profile(&t, "gpt-5.5", "high");
    let store = t.store();
    let _hook = hooks::install(store.clone(), codex_commands_hook(&t));
    let work = accounts_for(&store).add("codex", "Work").unwrap().id;
    let first = start_picked(&t, "gpt-5.6-luna", Some("low"), Some(work.clone()));
    let back = bring_the_answer_back(&t, &first);
    let rows = records(&path_of(&t, &back));
    assert_eq!(rows[0]["metadata"]["account"], work);
    assert_eq!(rows[0]["metadata"]["mode"], "ask");
    let req = last_start(&t);
    assert_eq!(req["effort"], "low");
    assert_eq!(req["model"], "gpt-5.6-luna");
}

// case: a turn on the account default starts on the account default
#[test]
fn b07_automatic_reply_on_account_default_does_not_take_the_profiles_level() {
    let t = Temp::new();
    profile(&t, "gpt-5.5", "high");
    let _hook = hooks::install(t.store(), codex_commands_hook(&t));
    let first = start_picked(&t, "account-default", None, None);
    let initial = last_start(&t);
    let back = bring_the_answer_back(&t, &first);
    let rows = records(&path_of(&t, &back));
    assert_eq!(rows[0]["metadata"]["model"], "account-default");
    assert!(rows[0]["metadata"].get("account").is_none());
    assert!(initial.get("model").is_none() && initial.get("effort").is_none(), "{initial}");
    let req = last_start(&t);
    assert!(req.get("model").is_none() && req.get("effort").is_none(), "{req}");
}
