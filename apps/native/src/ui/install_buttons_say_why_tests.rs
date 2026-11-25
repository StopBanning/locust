//! INSTALL BUTTONS SAY WHY (`renderer/src/install-buttons-say-why.test.tsx`):
//! On Home, when Node.js is missing, the panel says so ("Node.js is not on this machine, so the Install buttons cannot
//! run.") and offers a button ("Get Node.js") even after another runtime has connected; while nothing is connected,
//! Home defers uninstalled agents behind a press ("N other(s) Locust can drive...") while keeping OpenCode and any
//! already-installed agent visible; and an uninstalled agent says "ready to install", never "not installed".
//!
//! Note: the 8 Electron cases for `npmIsBundled` (running Electron's own binary with `ELECTRON_RUN_AS_NODE=1` as a
//! bundled Node/npm) do not apply to the native Rust app, which does not ship Node.js or a bundled copy of npm.
use super::*;
use locust_core::agents::{Auth, Found, Probe};

fn runtime(id: &str, name: &str, connected: bool) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: connected,
        version: connected.then(|| "1.0.0".into()),
        auth: if connected { Auth::Authenticated } else { Auth::Unknown },
        ready: connected,
        status: if connected { Probe::Ready } else { Probe::Missing },
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

fn draw_home(found: Vec<Found>, npm_missing: bool) -> rune::Runtime<State> {
    set_npm_stand_in(Some(!npm_missing));
    let mut s = State::default();
    s.home.folder_path = Some("/Users/someone/work".into());
    s.settings_panel.agents = Some(found);
    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::home_panel::home_view(s, &Palette::new(true)),
        (1215.0, 4000.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn panel(connected: usize, npm_missing: bool) -> rune::Runtime<State> {
    let runtimes = vec![
        runtime("codex", "Codex", connected > 0),
        runtime("claude", "Claude Code", false),
        runtime("opencode", "OpenCode", false),
        runtime("cursor", "Cursor", false),
    ];
    draw_home(runtimes, npm_missing)
}

fn words(rt: &rune::Runtime<State>) -> String {
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| e.text.or(e.label))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---- when Node.js is missing ------------------------------------------------------------------------------------

#[test]
fn renders_the_panel_at_all() {
    // case: renders the panel at all
    let rt = panel(0, true);
    assert!(words(&rt).contains("Install"), "{}", words(&rt));
}

#[test]
fn says_so_even_after_a_runtime_has_connected() {
    // case: says so even after a runtime has connected
    let rt = panel(1, true);
    let said = words(&rt);
    assert!(said.contains("Node.js is not on this machine"), "{said}");
    assert!(said.contains("cannot run"), "{said}");
}

#[test]
fn still_says_so_when_nothing_at_all_has_connected() {
    // case: still says so when nothing at all has connected
    let rt = panel(0, true);
    assert!(words(&rt).contains("Node.js is not on this machine"), "{}", words(&rt));
}

#[test]
fn says_nothing_about_node_when_npm_is_there() {
    // case: says nothing about Node when npm is there
    let rt = panel(1, false);
    assert!(!words(&rt).contains("Node.js is not on this machine"), "{}", words(&rt));
}

#[test]
fn offers_a_way_to_get_node_that_this_app_can_actually_open() {
    // case: offers a way to get Node that this app can actually open
    let mut rt = panel(1, true);
    assert!(words(&rt).contains("Get Node.js"), "{}", words(&rt));
    let btn = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("agents-get-nodejs"))
        .expect("Get Node.js button");
    assert_eq!(btn.role, "button");
    let (x, y) = rt.find("agents-get-nodejs").expect("Get Node.js coords");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.opened_links, vec!["https://nodejs.org"]);
}

// ---- what the cold first screen does not say yet ----------------------------------------------------------------

#[test]
fn offers_the_other_agents_behind_a_press_while_none_of_them_can_help() {
    // case: offers the other agents behind a press while none of them can help
    let mut rt = panel(0, false);
    let cold = words(&rt);
    assert!(cold.contains("Locust can drive"), "{cold}");
    assert!(!cold.contains("needs a Cursor account"), "{cold}");
    assert!(cold.contains("OpenCode"), "{cold}");
    assert!(cold.contains("no account needed"), "{cold}");

    // Pressing the deferred button reveals the other agents.
    let (x, y) = rt.find("home-agents-others").expect("deferred others button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert!(words(&rt).contains("needs a Cursor account"), "{}", words(&rt));
}

#[test]
fn offers_an_agent_nobody_has_added_yet_as_ready_to_install_never_as_not_installed() {
    // case: offers an agent nobody has added yet as ready to install, never as "not installed"
    let cold = words(&panel(0, false));
    assert!(cold.contains("ready to install"), "{cold}");
    assert!(!cold.to_ascii_lowercase().contains("not installed"), "{cold}");
    let warm = words(&panel(1, false));
    assert!(!warm.to_ascii_lowercase().contains("not installed"), "{warm}");
}

#[test]
fn shows_every_agent_again_the_moment_one_of_them_works() {
    // case: shows every agent again the moment one of them works
    let warm = words(&panel(1, false));
    assert!(warm.contains("needs a Cursor account"), "{warm}");
    assert!(!warm.contains("Locust can drive"), "{warm}");
}

// ---- the deferral, on a machine that HAS some of them -----------------------------------------------------------

fn with_installed() -> rune::Runtime<State> {
    let mut codex = runtime("codex", "Codex", false);
    codex.installed = true;
    codex.status = Probe::ProbeFailed;
    let runtimes = vec![
        runtime("opencode", "OpenCode", false),
        codex,
        runtime("cursor", "Cursor", false),
    ];
    draw_home(runtimes, false)
}

#[test]
fn draws_the_installed_one_even_while_nothing_is_connected() {
    // case: draws the installed one even while nothing is connected
    let said = words(&with_installed());
    assert!(said.contains("Codex"), "{said}");
}

#[test]
fn still_defers_the_ones_that_are_only_products_you_could_buy() {
    // case: still defers the ones that are only products you could buy
    let said = words(&with_installed());
    assert!(said.contains("Locust can drive"), "{said}");
    assert!(!said.contains("Cursor"), "{said}");
}

#[test]
fn counts_only_what_it_actually_deferred() {
    // case: counts only what it actually deferred
    let said = words(&with_installed());
    assert!(said.contains("1 other Locust can drive"), "{said}");
}
