//! home-leads-with-the-team.test.tsx (8 cases): Home leads with the team, a card per teammate and who is working, and
//! folds the agent list to one line only when it has nothing to ask of the person (`home_agents`, by
//! `home_order::agents_fold`). Drawn headless through `home_view`. Expected words are Electron's (FirstLaunch.tsx,
//! HomeTeam.tsx, AgentMark.tsx and the test); a route is the one Electron's App hands the card (`homeRouteOf`).
use super::*;
use locust_core::agents::{Auth, Found, Probe};

fn ready(id: &str, name: &str) -> Found {
    Found { id: id.into(), display_name: name.into(), installed: true, version: Some("1.0.0".into()), auth: Auth::Authenticated, ready: true, status: Probe::Ready, unsupported_note: None, through_cli: false, checking: false }
}

fn signed_out(id: &str, name: &str) -> Found {
    Found { auth: Auth::Unauthenticated, ready: false, status: Probe::AuthRequired, ..ready(id, name) }
}

fn absent(id: &str, name: &str) -> Found {
    Found { installed: false, version: None, auth: Auth::Unknown, ready: false, status: Probe::Missing, ..ready(id, name) }
}

/// Electron's OpenCode on its first start: installed, still being checked.
fn checking(id: &str, name: &str) -> Found {
    Found { version: None, auth: Auth::NotApplicable, ready: false, status: Probe::ProbeFailed, checking: true, ..ready(id, name) }
}

fn mate(id: &str, name: &str, role: &str, runtime: Option<&str>, busy: bool) -> Teammate {
    Teammate { id: id.into(), name: name.into(), role: role.into(), runtime: runtime.map(Into::into), model: None, account: None, busy, runs_in: "ask".into(), folder: None }
}

/// Electron's TEAM: Wren on Codex's account default, idle; Penny, working.
fn team() -> Vec<Teammate> {
    vec![mate("tm_wren", "Wren", "Code & Migrations", Some("codex"), false), mate("tm_penny", "Penny", "Money", None, true)]
}

fn home_with(found: Vec<Found>, team: Vec<Teammate>, more: impl FnOnce(&mut State)) -> rune::Runtime<State> {
    let mut s = State::default();
    crate::team_scene(&mut s);
    s.teammates = team;
    s.people_panel.team = serde_json::json!({ "missionOwners": {} });
    s.settings_panel.agents = Some(found);
    more(&mut s);
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn home(found: Vec<Found>, team: Vec<Teammate>) -> rune::Runtime<State> {
    home_with(found, team, |_| {})
}

fn texts(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

fn labels(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.label).collect()
}

/// The agent list is folded to its one line (FirstLaunch's `lc-agenthead is-folded`): Show all is offered.
fn folded(rt: &rune::Runtime<State>) -> bool {
    rt.find("home-agents-show-all").is_some()
}

/// The whole list is open (`lc-runtimepanel`): its lede is drawn.
fn whole_list(rt: &rune::Runtime<State>) -> bool {
    let lede = format!("Each AI agent starts with this {}", locust_core::machine_words::this_machine().computer);
    texts(rt).iter().any(|t| t.contains(&lede))
}

#[test]
fn leads_with_the_team_a_card_per_teammate_and_who_is_working() {
    let rt = home(vec![ready("codex", "Codex CLI"), ready("claude", "Claude Code")], team());
    let t = texts(&rt);
    assert!(t.iter().any(|t| t == "Your team"), "{t:?}");
    let l = labels(&rt);
    assert!(l.iter().any(|l| l.starts_with("Message Wren, Code & Migrations")), "{l:?}");
    // Codex's account default, as homeRouteOf says it (0.848): the runtime's name.
    assert!(l.iter().any(|l| l == "Message Wren, Code & Migrations, on Codex"), "{l:?}");
    // Penny is working and has no agent of her own: no route, and where she stands in the card's name.
    assert!(l.iter().any(|l| l == "Message Penny, Money, working now"), "{l:?}");
    assert!(t.iter().any(|t| t.contains("New teammate")), "{t:?}");
}

#[test]
fn folds_the_agent_list_to_one_line_when_every_installed_agent_is_ready() {
    let rt = home(vec![ready("codex", "Codex CLI"), ready("claude", "Claude Code"), absent("gemini", "Gemini CLI")], team());
    assert!(folded(&rt));
    let t = texts(&rt);
    assert!(t.iter().any(|t| t == "Connected accounts"), "{t:?}");
    assert!(!t.iter().any(|t| t == "AI agents"), "{t:?}");
    // The ready agents as their marks, each naming itself and its state, not a sentence of names.
    let l = labels(&rt);
    assert!(l.iter().any(|l| l == "Codex CLI. Ready · 1.0.0"), "{l:?}");
    assert!(l.iter().any(|l| l == "Claude Code. Ready · 1.0.0"), "{l:?}");
    assert!(!t.iter().any(|t| t.contains("Codex CLI · Claude Code")), "{t:?}");
    assert!(t.iter().any(|t| t == "Show all"), "{t:?}");
    // An agent not installed is an offer, not a problem: it does not hold the list open.
    assert!(!whole_list(&rt));
}

#[test]
fn keeps_the_whole_list_when_an_installed_agent_needs_the_person() {
    let rt = home(vec![ready("codex", "Codex CLI"), signed_out("claude", "Claude Code")], team());
    assert!(!folded(&rt));
    assert!(whole_list(&rt));
}

#[test]
fn keeps_the_whole_list_on_a_first_run_while_another_agent_could_still_be_added() {
    let rt = home(vec![ready("codex", "Codex CLI"), ready("claude", "Claude Code"), absent("cursor", "Cursor Agent")], vec![]);
    assert!(!texts(&rt).iter().any(|t| t == "Your team"));
    assert!(!folded(&rt));
    assert!(whole_list(&rt));
}

#[test]
fn folds_on_a_first_run_too_once_every_agent_is_ready() {
    let rt = home(vec![ready("codex", "Codex CLI"), ready("claude", "Claude Code")], vec![]);
    assert!(folded(&rt));
    assert!(!whole_list(&rt));
    // And a signed-out one opens it again, team or not.
    assert!(whole_list(&home(vec![ready("codex", "Codex CLI"), signed_out("claude", "Claude Code")], vec![])));
}

#[test]
fn with_a_team_folds_while_an_agent_is_still_being_checked_and_says_which() {
    let rt = home(vec![ready("codex", "Codex CLI"), ready("claude", "Claude Code"), checking("opencode", "OpenCode")], team());
    assert!(folded(&rt));
    assert!(!whole_list(&rt));
    let t = texts(&rt);
    assert!(t.iter().any(|t| t == "2 ready"), "{t:?}");
    assert!(t.iter().any(|t| t == "· checking OpenCode"), "{t:?}");
    // Two at once are counted, not listed.
    let two = texts(&home(vec![ready("codex", "Codex CLI"), checking("opencode", "OpenCode"), checking("cursor", "Cursor Agent")], team()));
    assert!(two.iter().any(|t| t == "· checking 2 more"), "{two:?}");
    // Before a team, the list still waits for every answer.
    assert!(whole_list(&home(vec![ready("codex", "Codex CLI"), ready("claude", "Claude Code"), checking("opencode", "OpenCode")], vec![])));
    // And an answer that needs the person opens it again.
    assert!(whole_list(&home(vec![ready("codex", "Codex CLI"), checking("opencode", "OpenCode"), signed_out("claude", "Claude Code")], team())));
}

#[test]
fn says_nothing_of_checking_once_every_answer_is_in() {
    let rt = home(vec![ready("codex", "Codex CLI"), ready("claude", "Claude Code")], team());
    let t = texts(&rt);
    assert!(t.iter().any(|t| t == "2 ready"), "{t:?}");
    assert!(!t.iter().any(|t| t.contains("checking")), "{t:?}");
}

#[test]
fn says_where_each_teammate_stands_beside_the_name_and_reads_it_aloud_with_the_card() {
    let two_hours = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    let rt = home_with(
        vec![ready("codex", "Codex CLI")],
        vec![
            mate("tm_wren", "Wren", "Code & Migrations", Some("codex"), false),
            mate("tm_penny", "Penny", "Money", Some("codex"), true),
            mate("tm_sable", "Sable", "Data", Some("codex"), true),
        ],
        |s| {
            s.people_panel.team = serde_json::json!({ "missionOwners": { "m_tm_wren": "tm_wren", "m_tm_penny": "tm_penny", "m_tm_sable": "tm_sable" } });
            s.recent = vec![locust_core::engine::ConversationRow { id: "m_tm_wren".into(), teammate: "Wren".into(), title: "t".into(), started_by: "the person".into(), state: "completed".into(), updated: two_hours }];
            s.home.waiting_for.insert("m_tm_sable".into(), "Which folder?".into());
            s.home.asks.insert("m_tm_sable".into(), BoardAsk { approval_id: "a1".into(), kind: "question".into(), command: None, where_: None, shown: std::time::Instant::now() });
        },
    );
    let card = |id: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("home-card-{id}"))).expect("a card");
    // Idle says nothing beside the name; what they last did is the card's hover.
    assert_eq!(card("tm_wren").help.as_deref(), Some("last worked 2 hours ago"));
    let t = texts(&rt);
    assert!(t.iter().any(|t| t == "waiting on you"), "{t:?}");
    assert!(t.iter().any(|t| ["working", "thinking", "replying", "typing"].contains(&t.as_str())), "{t:?}");
    assert_eq!(card("tm_wren").label.as_deref(), Some("Message Wren, Code & Migrations, on Codex"));
    // Her live word read aloud as Electron's state.spoken is ("<word> now"); with no face drawn it is working.
    assert_eq!(card("tm_penny").label.as_deref(), Some("Message Penny, Money, working now, on Codex"));
    assert_eq!(card("tm_sable").label.as_deref(), Some("Message Sable, Data, waiting on you, on Codex"));
    // The old "working" tag is gone: the state says it, once.
    assert!(!t.iter().any(|t| t == "working now"), "{t:?}");
}
