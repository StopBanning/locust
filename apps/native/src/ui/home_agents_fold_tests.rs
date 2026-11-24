//! FirstLaunch.tsx's folded accounts line, drawn headless: with a team and every installed agent ready, Home's accounts
//! are one line, and Show all opens the list again.
use super::*;
use locust_core::agents::{Auth, Found, Probe};

fn agent(id: &str, name: &str, installed: bool, ready: bool) -> Found {
    Found { id: id.into(), display_name: name.into(), installed, version: None, auth: if ready { Auth::Authenticated } else { Auth::Unknown }, ready,
        status: if ready { Probe::Ready } else if installed { Probe::AuthRequired } else { Probe::Missing }, unsupported_note: None, through_cli: false, checking: false }
}

fn home(found: Vec<Found>) -> rune::Runtime<State> {
    let mut s = State::default();
    crate::team_scene(&mut s);
    s.settings_panel.agents = Some(found);
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 1600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn shown(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn with_a_team_and_every_installed_agent_ready_the_accounts_are_one_line() {
    let rt = home(vec![agent("codex", "Codex CLI", true, true), agent("claude", "Claude Code", true, true), agent("cursor", "Cursor Agent", false, false)]);
    let said = shown(&rt);
    assert!(said.contains("Connected accounts"), "{said}");
    assert!(said.contains("2 ready"), "{said}");
    assert!(!said.contains(&format!("Each AI agent starts with this {}", locust_core::machine_words::this_machine().computer)), "the whole list is folded: {said}");
    assert!(rt.find("home-agents-show-all").is_some());
}

#[test]
fn show_all_opens_the_whole_list_again() {
    let mut rt = home(vec![agent("codex", "Codex CLI", true, true)]);
    let (x, y) = rt.find("home-agents-show-all").expect("Show all");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert!(rt.state.home.agents_open);
    assert!(shown(&rt).contains(&format!("Each AI agent starts with this {}", locust_core::machine_words::this_machine().computer)));
}

#[test]
fn an_installed_agent_that_needs_its_sign_in_keeps_the_whole_list_open() {
    let said = shown(&home(vec![agent("codex", "Codex CLI", true, true), agent("claude", "Claude Code", true, false)]));
    assert!(!said.contains("Connected accounts"), "{said}");
    assert!(said.contains(&format!("Each AI agent starts with this {}", locust_core::machine_words::this_machine().computer)), "{said}");
}
