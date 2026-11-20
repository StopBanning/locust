//! Electron `with-nothing-installed-install-comes-first.test.tsx`, drawn headless at Locust's own 1215x800 window.
use super::*;
use locust_core::agents::{Auth, Found, Probe};

fn missing(id: &str, name: &str) -> Found {
    Found { id: id.into(), display_name: name.into(), installed: false, version: None, auth: Auth::Unknown, ready: false,
        status: Probe::Missing, unsupported_note: None, through_cli: false, checking: false }
}
fn ready(id: &str, name: &str) -> Found {
    Found { installed: true, version: Some("1.0.0".into()), auth: Auth::Authenticated, ready: true, status: Probe::Ready, ..missing(id, name) }
}
fn checking(id: &str, name: &str) -> Found {
    Found { ready: false, status: Probe::ProbeFailed, checking: true, ..ready(id, name) }
}
fn others() -> Vec<Found> {
    vec![missing("codex", "Codex CLI"), missing("claude", "Claude Code"), missing("cursor", "Cursor Agent")]
}

/// Where Home draws the agents, the team templates and the made-folder card, top to bottom (None: not drawn).
struct At { install: Option<f32>, team: Option<f32>, folder: Option<f32>, shown: String }

fn home(found: Vec<Found>) -> At {
    let mut s = State::default();
    s.home.folder_path = Some("/Users/someone/Documents/Locust".into());
    s.home.folder_invented = true;
    s.settings_panel.agents = Some(found);
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 4000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let top = |hit: &dyn Fn(&rune::ElementInfo) -> bool| all.iter().find(|e| hit(e)).map(|e| e.rect[1]);
    At {
        install: top(&|e| e.key.as_deref() == Some("home-agents")),
        team: top(&|e| e.text.as_deref() == Some(crate::starters::START_WITH_A_TEAM)),
        folder: top(&|e| e.text.as_deref() == Some(locust_core::folders::MADE_FOLDER)),
        shown: all.iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n"),
    }
}

#[test]
fn nothing_installed_puts_the_install_row_before_the_team_templates_and_the_folder_card() {
    let mut found = vec![missing("opencode", "OpenCode")];
    found.extend(others());
    let at = home(found);
    let (install, team, folder) = (at.install.expect("install"), at.team.expect("team"), at.folder.expect("folder"));
    assert!(team > install && folder > install, "install {install}, team {team}, folder {folder}");
    assert!(at.shown.contains("Install OpenCode"), "{}", at.shown);
}

#[test]
fn nothing_installed_still_offers_the_teams_and_the_folder_after_it() {
    let mut found = vec![missing("opencode", "OpenCode")];
    found.extend(others());
    let at = home(found);
    assert!(at.shown.contains("Start with a team"), "{}", at.shown);
    assert!(at.shown.contains("Choose folder"), "{}", at.shown);
}

#[test]
fn once_anything_is_installed_home_leads_with_the_team_templates_again() {
    let mut found = vec![ready("opencode", "OpenCode")];
    found.extend(others());
    let at = home(found);
    let (install, team, folder) = (at.install.expect("install"), at.team.expect("team"), at.folder.expect("folder"));
    assert!(team < install && folder < install, "install {install}, team {team}, folder {folder}");
}

#[test]
fn home_does_not_reorder_while_an_installed_agent_is_still_being_checked() {
    let at = home(vec![checking("codex", "Codex CLI"), missing("opencode", "OpenCode")]);
    let (install, team) = (at.install.expect("install"), at.team.expect("team"));
    assert!(team < install, "install {install}, team {team}");
}

// Electron `the-first-screen-line-is-centred.test.ts`: Electron reads its stylesheet; here the drawn frame is measured,
// which is what its probe on the built app measures.
fn centre_x(all: &[rune::ElementInfo], hit: impl Fn(&rune::ElementInfo) -> bool) -> Option<f32> {
    all.iter().find(|e| hit(e)).map(|e| e.rect[0] + e.rect[2] / 2.0)
}

fn first_screen(found: Vec<Found>) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.settings_panel.agents = Some(found);
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 1600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

#[test]
fn the_line_under_the_machine_is_centred_in_the_column_as_the_machine_is_box_and_words_both() {
    let mut found = vec![missing("opencode", "OpenCode")];
    found.extend(others());
    let all = first_screen(found);
    let line = all.iter().find(|e| e.key.as_deref() == Some("home-intro")).expect("the intro line");
    assert_eq!(line.text.as_deref(), Some(crate::boot_view::INTRO_WITH_FREE));
    let machine = centre_x(&all, |e| e.label.as_deref().is_some_and(|l| l.starts_with("Locust. "))).expect("the lockup");
    let words = centre_x(&all, |e| e.key.as_deref() == Some("home-intro")).unwrap();
    assert!((words - machine).abs() <= 2.0, "line centre {words}, machine centre {machine}");
    // One line at the column's width: the words are centred because the box that holds them is.
    assert!(line.rect[3] < 30.0, "the line wrapped: {:?}", line.rect);
}

#[test]
fn the_line_sits_under_a_machine_centred_in_the_same_column() {
    let all = first_screen(vec![missing("opencode", "OpenCode")]);
    let machine = all.iter().find(|e| e.label.as_deref().is_some_and(|l| l.starts_with("Locust. "))).expect("the lockup");
    let line = all.iter().find(|e| e.key.as_deref() == Some("home-intro")).expect("the intro line");
    assert!(line.rect[1] > machine.rect[1], "line {:?} above machine {:?}", line.rect, machine.rect);
    let column = (1215.0f32 - 760.0) / 2.0 + 380.0;
    assert!((machine.rect[0] + machine.rect[2] / 2.0 - column).abs() <= 2.0, "machine {:?}", machine.rect);
}

#[test]
fn the_line_is_for_a_first_run_only_and_waits_for_the_agents_to_answer() {
    let mut s = State::default();
    assert_eq!(intro_line(&s), None);
    s.settings_panel.agents = Some(vec![missing("opencode", "OpenCode")]);
    assert!(intro_line(&s).is_some());
}

#[test]
fn the_line_drops_the_free_promise_only_when_opencode_lists_nothing_free() {
    let model = |id: &str| locust_core::engine::Model { id: id.into(), name: id.into(), efforts: Vec::new() };
    let mut s = State::default();
    s.settings_panel.agents = Some(vec![ready("opencode", "OpenCode")]);
    assert_eq!(intro_line(&s).as_deref(), Some(crate::boot_view::INTRO_WITH_FREE), "a list not read yet keeps the promise");
    s.models.insert("all".into(), vec![locust_core::engine::Models { teammate: None, runtime: "opencode".into(), model: None, effort: None, models: vec![model("opencode/big-pickle")] }]);
    assert_eq!(intro_line(&s).as_deref(), Some(crate::boot_view::INTRO));
}

#[test]
fn with_nothing_installed_the_agents_say_to_press_install_as_the_row_has_it() {
    let mut found = vec![missing("opencode", "OpenCode")];
    found.extend(others());
    let at = home(found);
    assert!(at.shown.contains("The quickest start is OpenCode: press its Install, no account"), "{}", at.shown);
}
