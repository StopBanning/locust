//! Electron's `main/an-app-reads-usage-and-sets-a-teammates-model.test.ts` (0.882):
//! `read_usage` gives the window's own readings and agents at their limit; `set_teammate_model` saves an offered model
//! only with both switches on, keeping agent/account/mode and compatible effort, and reporting org policy refusals;
//! short conversation ids (8+ hex/hyphen chars, with or without `mission_`) resolve when matching exactly one turn.
use super::*;
use std::collections::HashMap;

struct UsageDoor {
    see_all: bool,
    own_mode: bool,
    refuse: Option<String>,
    usage_fails: bool,
    turns: HashMap<String, Turn>,
    saved: Mutex<Vec<(String, Value)>>,
}

fn codec_mate() -> (Teammate, Option<String>) {
    (
        Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Custom".into(),
            runtime: Some("codex".into()),
            model: Some("gpt-6.1-sol".into()),
            account: Some("work".into()),
            busy: false,
            runs_in: "auto".into(),
            folder: None,
        },
        Some("high".into()),
    )
}

fn completed_turn(id: &str) -> Turn {
    let mut t = turn(id, true, vec![ev("run.completed", json!({}))]);
    t.runtime = Some("codex".into());
    t.updated = "2026-11-22T12:00:00.000Z".into();
    t
}

fn door(see_all: bool, own_mode: bool, refuse: Option<&str>) -> Tools<UsageDoor> {
    let id1 = "mission_7a173829-9d97-48a1-92ba-708f65d0421d";
    let id2 = "mission_7a17ffff-0000-4000-8000-000000000000";
    let id3 = "mission_deadbeef-1111-4000-8000-000000000000";
    let id4 = "mission_deadbeef-2222-4000-8000-000000000000";
    let id5 = "mission_nothexid-0000-4000-8000-000000000000";
    let turns = HashMap::from([
        (id1.to_owned(), completed_turn(id1)),
        (id2.to_owned(), completed_turn(id2)),
        (id3.to_owned(), completed_turn(id3)),
        (id4.to_owned(), completed_turn(id4)),
        (id5.to_owned(), completed_turn(id5)),
    ]);
    Tools::new(UsageDoor {
        see_all,
        own_mode,
        refuse: refuse.map(str::to_owned),
        usage_fails: false,
        turns,
        saved: Mutex::new(Vec::new()),
    })
}

impl Locust for UsageDoor {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        Ok(vec![codec_mate()])
    }
    fn busy(&self, _: &str) -> bool {
        false
    }
    fn start(&self, _: StartRequest) -> Result<Begun, String> {
        Err("not here".into())
    }
    fn read(&self, mission: &str) -> Option<Turn> {
        self.turns.get(mission).cloned()
    }
    fn newest(&self, mission: &str) -> Option<String> {
        Some(mission.to_owned())
    }
    fn owner(&self, _: &str) -> Option<String> {
        None
    }
    fn live(&self, _: &str) -> bool {
        false
    }
    fn waiting(&self, _: &str) -> bool {
        false
    }
    fn cancel(&self, _: &str) -> bool {
        false
    }
    fn conversations(&self) -> Vec<Turn> {
        self.turns.values().cloned().collect()
    }
    fn check_folder(&self, path: &str) -> Result<PathBuf, String> {
        Ok(PathBuf::from(path))
    }
    fn folder(&self) -> Option<String> {
        None
    }
    fn own_mode(&self) -> bool {
        self.own_mode
    }
    fn see_all(&self) -> bool {
        self.see_all
    }
    fn models(&self) -> Vec<Value> {
        let m = |id: &str, efforts: &[&str]| {
            json!({"id": id, "runtime": "codex", "displayName": id.to_uppercase(), "description": "", "supportedEfforts": efforts})
        };
        vec![
            m("gpt-6.1-sol", &["high", "max"]),
            m("gpt-6.2-sol", &["high", "ultra"]),
            m("gpt-6-luna", &["low"]),
        ]
    }
    fn carry_on(&self, _: &str, _: &str, _: Option<&str>, _: &crate::carry_on::CarryOnAlso) -> Result<(String, Option<String>), String> {
        Err("not here".into())
    }
    fn usage(&self) -> Result<(HashMap<String, String>, HashMap<String, String>), String> {
        if self.usage_fails {
            return Err("disk".into());
        }
        Ok((
            HashMap::from([("codex".into(), "5-hour window 78% used · resets 09:56 · from a run at 2026-11-22T05:13:36.175Z".into())]),
            HashMap::from([("codex:work".into(), "Codex is at its limit until 09:56.".into())]),
        ))
    }
    fn save_model(&self, teammate_id: &str, route: &Value) -> Result<(), String> {
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        self.saved.lock().unwrap().push((teammate_id.to_owned(), route.clone()));
        Ok(())
    }
}

// case: gives the window’s own readings, dated by their run, and the agents at their limit
#[test]
fn gives_the_windows_own_readings_dated_by_their_run_and_the_agents_at_their_limit() {
    let f = door(true, true, None);
    let r = f.call("read_usage", &json!({}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(
        reply(&r),
        json!({
            "usage": {"codex": "5-hour window 78% used · resets 09:56 · from a run at 2026-11-22T05:13:36.175Z"},
            "at_limit": {"codex:work": "Codex is at its limit until 09:56."}
        })
    );
    let mut failing = UsageDoor { see_all: true, own_mode: true, refuse: None, usage_fails: true, turns: HashMap::new(), saved: Mutex::new(Vec::new()) };
    failing.usage_fails = true;
    let err = Tools::new(failing).call("read_usage", &json!({}));
    assert!(err.is_error && err.text.contains("could not read its usage records"), "{}", err.text);
}

// case: saves a model its agent offers, keeping its agent, account, mode and an effort the new model takes
#[test]
fn saves_a_model_its_agent_offers_keeping_its_agent_account_mode_and_an_effort_the_new_model_takes() {
    let f = door(true, true, None);
    let r = f.call("set_teammate_model", &json!({"teammate": "codec", "model": "GPT-6.2-SOL"}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(reply(&r), json!({"teammate": "Codec", "runtime": "codex", "model": "gpt-6.2-sol", "effort": "high"}));
    assert_eq!(
        f.locust.saved.lock().unwrap().as_slice(),
        [("tm_codec".to_owned(), json!({"runtime": "codex", "model": "gpt-6.2-sol", "mode": "auto", "effort": "high", "account": "work"}))]
    );

    // An effort the new model does not take is dropped, and one asked for must be offered.
    let r2 = f.call("set_teammate_model", &json!({"teammate": "tm_codec", "model": "gpt-6-luna"}));
    assert!(!r2.is_error, "{}", r2.text);
    assert_eq!(
        f.locust.saved.lock().unwrap().last().unwrap(),
        &("tm_codec".to_owned(), json!({"runtime": "codex", "model": "gpt-6-luna", "mode": "auto", "account": "work"}))
    );
    let bad_effort = f.call("set_teammate_model", &json!({"teammate": "Codec", "model": "gpt-6.2-sol", "effort": "low"}));
    assert!(bad_effort.is_error);
    assert!(bad_effort.text.ends_with("takes these efforts here: high, ultra. Nothing was changed."), "{}", bad_effort.text);
    assert_eq!(f.locust.saved.lock().unwrap().len(), 2);
}

// case: changes nothing without both of the person’s switches, a known teammate and an offered model
#[test]
fn changes_nothing_without_both_of_the_persons_switches_a_known_teammate_and_an_offered_model() {
    for (see_all, own_mode) in [(false, true), (true, false)] {
        let f = door(see_all, own_mode, None);
        let r = f.call("set_teammate_model", &json!({"teammate": "Codec", "model": "gpt-6.2-sol"}));
        assert!(r.is_error);
        assert!(r.text.starts_with("Off: "), "{}", r.text);
        assert!(r.text.contains("Let apps see your conversations") && r.text.contains("Use each teammate's own mode"), "{}", r.text);
        assert!(f.locust.saved.lock().unwrap().is_empty());
    }
    let f = door(true, true, None);
    let nobody = f.call("set_teammate_model", &json!({"teammate": "Nobody", "model": "gpt-6.2-sol"}));
    assert!(nobody.is_error && nobody.text.starts_with("No teammate"), "{}", nobody.text);
    let wrong = f.call("set_teammate_model", &json!({"teammate": "Codec", "model": "claude-opus-5-5"}));
    assert!(wrong.is_error && wrong.text.contains("not one codex offers"), "{}", wrong.text);
    assert!(f.locust.saved.lock().unwrap().is_empty());
}

// case: says the host’s refusal, from the organization’s policy, as it is
#[test]
fn says_the_hosts_refusal_from_the_organizations_policy_as_it_is() {
    let f = door(true, true, Some("Your organization does not allow gpt-6.2-sol."));
    let r = f.call("set_teammate_model", &json!({"teammate": "Codec", "model": "gpt-6.2-sol"}));
    assert!(r.is_error);
    assert_eq!(r.text, "Your organization does not allow gpt-6.2-sol. Nothing was changed.");
}

// case: reads one by eight or more of its characters, with or without mission_
#[test]
fn reads_one_by_eight_or_more_of_its_characters_with_or_without_mission() {
    let f = door(true, true, None);
    assert!(!f.call("read_reply", &json!({"conversation_id": "7a173829"})).is_error);
    assert!(!f.call("read_reply", &json!({"conversation_id": "mission_7a173829-9d97"})).is_error);
}

// case: reads none when the start matches two, is shorter than eight, or is not an id
#[test]
fn reads_none_when_the_start_matches_two_is_shorter_than_eight_or_is_not_an_id() {
    let f = door(true, true, None);
    for asked in ["7a17", "7a17fff", "mission_7a17", "zzzzzzzzzz", "deadbeef", "nothexid"] {
        assert!(f.call("read_reply", &json!({"conversation_id": asked})).is_error, "{asked}");
    }
    // Two share "7a17"; eight characters that only one starts with name that one.
    assert!(!f.call("read_reply", &json!({"conversation_id": "7a17ffff"})).is_error);
    assert!(!f.call("read_reply", &json!({"conversation_id": "deadbeef-1111"})).is_error);
}
