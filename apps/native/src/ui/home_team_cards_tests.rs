//! Electron `homeTeamState.test.ts` is the rule (core/src/home_team_state_tests.rs); these draw it on Home's team
//! cards as HomeTeam.tsx does: the state beside the name, what they wait on under it, what they last did on hover.
use super::*;

fn mates(s: &mut State, list: &[(&str, &str, &str, bool)]) {
    crate::team_scene(s);
    s.teammates.truncate(list.len());
    let mut owners = serde_json::Map::new();
    for (mate, (id, name, role, busy)) in s.teammates.iter_mut().zip(list) {
        mate.id = (*id).into();
        mate.name = (*name).into();
        mate.role = (*role).into();
        mate.busy = *busy;
        mate.runtime = Some("codex".into());
        owners.insert(format!("m_{id}"), serde_json::json!(id));
    }
    s.people_panel.team = serde_json::json!({ "missionOwners": owners });
}

fn ask(s: &mut State, owner: &str, kind: &str, summary: &str, command: Option<&str>) {
    let mission = format!("m_{owner}");
    s.home.waiting_for.insert(mission.clone(), summary.into());
    s.home.asks.insert(mission, BoardAsk { approval_id: "a1".into(), kind: kind.into(), command: command.map(str::to_owned), where_: None, shown: std::time::Instant::now() });
}

struct Card { texts: Vec<String>, description: Option<String> }

fn card(s: State, id: &str) -> Card {
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let key = format!("home-card-{id}");
    let outer = all.iter().find(|e| e.key.as_deref() == Some(key.as_str())).map(|e| e.rect).expect("the card");
    let texts = all.iter().filter(|e| {
        let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
        cx >= outer[0] && cx <= outer[0] + outer[2] && cy >= outer[1] && cy <= outer[1] + outer[3]
    }).filter_map(|e| e.text.clone()).collect();
    fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        if node["key"] == key { return Some(node); }
        node["children"].as_array()?.iter().find_map(|child| find(child, key))
    }
    let tree = rt.accessibility_json();
    let description = find(&tree, &key).and_then(|n| n["description"].as_str().map(str::to_owned));
    Card { texts, description }
}

#[test]
fn a_working_teammate_wears_the_live_word_beside_the_name() {
    let mut s = State::default();
    mates(&mut s, &[("tm_codex", "Codex", "Builder", true)]);
    let shown = card(s, "tm_codex").texts;
    assert!(shown.iter().any(|t| t == "working" || t == "thinking" || t == "replying" || t == "typing"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "Builder"), "the role stays under the name: {shown:?}");
}

#[test]
fn a_waiting_teammate_says_waiting_on_you_and_under_the_name_the_command_it_wants_to_run() {
    let mut s = State::default();
    mates(&mut s, &[("tm_casper", "Casper", "Money", true)]);
    ask(&mut s, "tm_casper", "command", "Run tests", Some("pnpm test"));
    let shown = card(s, "tm_casper").texts;
    assert!(shown.iter().any(|t| t == "waiting on you"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "Wants to run pnpm test"), "{shown:?}");
    assert!(!shown.iter().any(|t| t == "Money"), "the waiting line takes the role's place: {shown:?}");
}

#[test]
fn a_waiting_question_is_said_as_asked_and_another_approval_plainly() {
    let both = || {
        let mut s = State::default();
        mates(&mut s, &[("tm_a", "Ann", "Docs", true), ("tm_b", "Bo", "QA", true)]);
        ask(&mut s, "tm_a", "question", "Which folder?", None);
        ask(&mut s, "tm_b", "file-change", "edit FirstLaunch.tsx", None);
        s
    };
    let first = card(both(), "tm_a").texts;
    assert!(first.iter().any(|t| t == "Which folder?"), "{first:?}");
    let second = card(both(), "tm_b").texts;
    assert!(second.iter().any(|t| t == "Wants your approval: edit FirstLaunch.tsx"), "{second:?}");
}

#[test]
fn an_idle_teammate_says_nothing_beside_the_name_and_keeps_what_they_last_did_for_the_hover() {
    let wren = || {
        let mut s = State::default();
        mates(&mut s, &[("tm_wren", "Wren", "Docs & QA", false)]);
        s
    };
    let quiet = card(wren(), "tm_wren");
    let mut s = wren();
    assert_eq!(quiet.description.as_deref(), Some("no work yet"));
    assert!(!quiet.texts.iter().any(|t| t.contains("idle") || t == "working"), "{:?}", quiet.texts);
    let two_hours = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    s.recent = vec![locust_core::engine::ConversationRow { id: "m_tm_wren".into(), teammate: "Wren".into(), title: "t".into(), started_by: "the person".into(), state: "completed".into(), updated: two_hours }];
    assert_eq!(card(s, "tm_wren").description.as_deref(), Some("last worked 2 hours ago"));
}
