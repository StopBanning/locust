//! Electron's `main/signing-in-is-a-button.test.ts` and its `SignInButton.tsx`: Sign in, and on a signed-in row Sign in
//! again, which says first that it can leave the agent signed out, then opens the CLI's own account-changing command
//! and says what to do in the window that opened, until Locust's window is back in front.
use super::*;
use locust_core::agents::{Auth, Probe, sign_in_again_warning, sign_in_args, sign_in_command, sign_in_opened_line};

fn agent(id: &str, name: &str, auth: Auth, status: Probe) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: true,
        version: Some("1.0.0".into()),
        auth,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: id == "antigravity",
        checking: false,
    }
}
fn window(agents: Vec<Found>) -> rune::Runtime<State> {
    let mut s = State::default();
    s.settings_panel.agents = Some(agents);
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    settle(&mut rt);
    rt
}
fn settle(rt: &mut rune::Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.016);
    }
}
fn press(rt: &mut rune::Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}
fn words(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join(" | ")
}

// case: signing in is a button > runs the words the install facts name, and nothing for OpenCode
#[test]
fn runs_the_words_the_install_facts_name_and_nothing_for_opencode() {
    let none: Vec<String> = Vec::new();
    assert_eq!(sign_in_args("muse", false), Some(vec!["login".to_owned()]));
    assert_eq!(sign_in_args("cursor", false), Some(vec!["login".to_owned()]));
    // Claude Code and Codex sign in from their own first screen.
    assert_eq!(sign_in_args("claude", false), Some(none.clone()));
    assert_eq!(sign_in_args("codex", false), Some(none));
    // The one runtime with no account has no sign-in to open.
    assert_eq!(sign_in_args("opencode", false), None);
    assert_eq!(sign_in_args("not-a-runtime", false), None);
}

// case: signing in is a button > signs in again with each CLI’s own command, read from its --help (Colin, 2026-10-02: change accounts)
#[test]
fn signs_in_again_with_each_clis_own_command_read_from_its_help() {
    let words = |w: &[&str]| Some(w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
    assert_eq!(sign_in_args("claude", true), words(&["auth", "login"]));
    assert_eq!(sign_in_args("codex", true), words(&["login"]));
    assert_eq!(sign_in_args("cursor", true), words(&["login"]));
    assert_eq!(sign_in_args("muse", true), words(&["login"]));
    // Copilot's is its own /login, inside the window; Antigravity's too: /logout and /login inside `agy`.
    assert_eq!(sign_in_args("copilot", true), words(&[]));
    assert_eq!(sign_in_args("antigravity", true), words(&[]));
    assert_eq!(sign_in_command("antigravity", true).as_deref(), Some("run agy, then type /logout, then /login, and choose the account"));
    // Once its window is open, the row says the steps (0.656): `agy` opens signed in as it was.
    assert_eq!(sign_in_opened_line("antigravity", true), "In the window that opened, type /logout, then /login, and choose the account.");
    assert_eq!(sign_in_opened_line("antigravity", false), "Finish in the window that opened.");
    assert_eq!(sign_in_opened_line("codex", true), "Finish in the window that opened.");
    // No account at all: nothing, rather than a guess.
    assert_eq!(sign_in_args("opencode", true), None);
}

// case: signing in is a button > refuses a runtime discovery did not find, rather than guessing a path
#[test]
fn refuses_a_runtime_discovery_did_not_find_rather_than_guessing_a_path() {
    let mut opened = Vec::new();
    let said = open_sign_in("muse", |_| None, |path| {
        opened.push(path.to_owned());
        Ok(())
    });
    assert_eq!(said, Err("Locust could not find this agent on the machine. Install it first, then sign in.".to_owned()));
    assert!(opened.is_empty(), "nothing is opened without the path discovery found");
    // Found: the window opens against that very path.
    let mut at = None;
    assert_eq!(open_sign_in("muse", |p| (p == "muse").then(|| "/opt/muse/bin/muse".into()), |path| {
        at = Some(path.to_owned());
        Ok(())
    }), Ok(()));
    assert_eq!(at, Some(std::path::PathBuf::from("/opt/muse/bin/muse")));
}

#[test]
fn says_codex_signs_out_at_once_and_any_other_may_stay_signed_out() {
    assert_eq!(sign_in_again_warning("codex", "Codex CLI"), "Codex CLI signs out as soon as this opens, and stays signed out until you finish there.");
    assert_eq!(sign_in_again_warning("claude", "Claude Code"), "Until you finish signing in there, Claude Code may stay signed out.");
}

#[test]
fn a_signed_in_row_signs_in_again_only_after_saying_it_can_sign_you_out() {
    let mut rt = window(vec![
        agent("codex", "Codex CLI", Auth::Authenticated, Probe::Ready),
        agent("claude", "Claude Code", Auth::Unauthenticated, Probe::AuthRequired),
        // Signed in to nothing: OpenCode's free model has no account, so nothing to sign in to again.
        agent("opencode", "OpenCode", Auth::NotApplicable, Probe::Ready),
        // Ready, but its sign-in not read: Electron offers Sign in again only on `auth === 'authenticated'`.
        agent("cursor", "Cursor Agent", Auth::Unknown, Probe::Ready),
    ]);
    assert!(rt.find("agents-sign-in-again-codex").is_some(), "a ready, signed-in row offers Sign in again");
    assert!(rt.find("agents-sign-in-again-claude").is_none() && rt.find("agents-sign-in-claude").is_some(), "a signed-out row offers Sign in");
    assert!(rt.find("agents-sign-in-again-opencode").is_none(), "no account, no Sign in again");
    assert!(rt.find("agents-sign-in-again-cursor").is_none(), "a sign-in not read is not offered again");

    // Pressed: the warning and a choice, nothing opened yet.
    press(&mut rt, "agents-sign-in-again-codex");
    assert!(rt.state.agents_signed_in.is_empty());
    assert!(words(&rt).contains("Codex CLI signs out as soon as this opens, and stays signed out until you finish there."), "{}", words(&rt));
    // Cancel leaves it as it was.
    press(&mut rt, "agents-sign-in-again-cancel-codex");
    assert!(rt.state.agents_signed_in.is_empty() && rt.find("agents-sign-in-again-codex").is_some());
    assert!(!words(&rt).contains("signs out as soon as this opens"));

    // Open it runs the CLI's own command for changing accounts, and the row says to finish there.
    press(&mut rt, "agents-sign-in-again-codex");
    press(&mut rt, "agents-sign-in-again-open-codex");
    assert_eq!(rt.state.agents_signed_in, vec!["codex login".to_owned()]);
    assert!(rt.find("agents-sign-in-opened-codex").is_some() && rt.find("agents-sign-in-again-codex").is_none());
    assert!(words(&rt).contains("Finish in the window that opened."));
}

#[test]
fn a_first_sign_in_says_to_finish_in_the_window_until_locust_is_back_in_front() {
    let mut rt = window(vec![agent("claude", "Claude Code", Auth::Unauthenticated, Probe::AuthRequired)]);
    press(&mut rt, "agents-sign-in-claude");
    assert_eq!(rt.state.agents_signed_in, vec!["claude".to_owned()]);
    assert!(rt.find("agents-sign-in-opened-claude").is_some() && rt.find("agents-sign-in-claude").is_none());
    let at = match rt.state.agents_sign_in_rows.get("claude") {
        Some(SignInRow::Opened { at, again: false }) => *at,
        other => panic!("{other:?}"),
    };
    // Focus straight back, within the second the window takes to open, is the window opening, not the person returning.
    rt.update(|s, _| window_focused(s, at + std::time::Duration::from_millis(500)));
    assert!(rt.state.agents_sign_in_rows.contains_key("claude"));
    let sweeps = rt.state.settings_panel.agent_sweeps;
    rt.state.settings_panel.agents_gave_up = true;
    // Back later: the row offers its button again, and every agent is asked again with the budget back in full.
    rt.update(|s, _| window_focused(s, at + std::time::Duration::from_secs(1)));
    settle(&mut rt);
    assert!(rt.state.agents_sign_in_rows.is_empty());
    assert!(!rt.state.settings_panel.agents_gave_up, "asked again, budget back");
    assert!(rt.state.settings_panel.agent_sweeps <= sweeps);
    assert!(rt.find("agents-sign-in-claude").is_some());
}

#[test]
fn antigravity_signing_in_again_says_the_steps_inside_its_window() {
    let mut rt = window(vec![agent("antigravity", "Antigravity", Auth::Authenticated, Probe::Ready)]);
    press(&mut rt, "agents-sign-in-again-antigravity");
    assert!(words(&rt).contains("Until you finish signing in there, Antigravity may stay signed out."));
    press(&mut rt, "agents-sign-in-again-open-antigravity");
    assert_eq!(rt.state.agents_signed_in, vec!["agy".to_owned()]);
    assert!(words(&rt).contains("In the window that opened, type /logout, then /login, and choose the account."), "{}", words(&rt));
    // A focus with nothing opened asks nobody again.
    rt.update(|s, _| s.agents_sign_in_rows.clear());
    rt.state.settings_panel.agents_gave_up = true;
    rt.update(|s, _| window_focused(s, std::time::Instant::now() + std::time::Duration::from_secs(5)));
    assert!(rt.state.settings_panel.agents_gave_up);
}

#[test]
fn a_row_signed_in_before_locust_is_back_in_front_offers_sign_in_again_not_the_first_windows_line() {
    let mut rt = window(vec![agent("claude", "Claude Code", Auth::Unauthenticated, Probe::AuthRequired)]);
    press(&mut rt, "agents-sign-in-claude");
    assert!(rt.find("agents-sign-in-opened-claude").is_some());
    // Discovery answers signed in while the sign-in window still has the focus: the row is ready, and its line about
    // the first sign-in's window is not carried onto Sign in again.
    rt.update(|s, _| s.settings_panel.agents = Some(vec![agent("claude", "Claude Code", Auth::Authenticated, Probe::Ready)]));
    settle(&mut rt);
    assert!(rt.find("agents-sign-in-opened-claude").is_none(), "{}", words(&rt));
    assert!(rt.find("agents-sign-in-again-claude").is_some());
}

#[test]
fn only_a_ready_or_active_row_signs_in_again() {
    let mut s = State::default();
    s.limited_runtimes.insert("codex".into(), "resets at 5".into());
    s.settings_panel.agents = Some(vec![
        agent("codex", "Codex CLI", Auth::Authenticated, Probe::Ready),
        // Kilo is a preview row; Antigravity through its app is an experimental one.
        agent("kilo", "Kilo", Auth::Authenticated, Probe::Ready),
        Found { through_cli: false, ..agent("antigravity", "Antigravity", Auth::Authenticated, Probe::Ready) },
        agent("claude", "Claude Code", Auth::Authenticated, Probe::Ready),
    ]);
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    settle(&mut rt);
    let all = words(&rt);
    assert!(all.contains("AT LIMIT") && all.contains("PREVIEW") && all.contains("EXPERIMENTAL"), "{all}");
    for id in ["codex", "kilo", "antigravity"] {
        assert!(rt.find(&format!("agents-sign-in-again-{id}")).is_none(), "{id}: {all}");
    }
    assert!(rt.find("agents-sign-in-again-claude").is_some());
}
