//! the-composer-counts-what-it-is-about.test.tsx (5 cases), through the window's own wiring: the box reads what
//! discovery found (fixed 20 Nov: the box never read discovery at all, so it said "Message Codec…" over a hung agent
//! and let the message go). Plus the readiness the send gate now takes from discovery, and what it leaves alone.
use super::*;
use locust_core::agents::{Auth, Found, Probe};

#[derive(Clone, Copy)]
enum Is { Ready, Hung, Absent, SignedOut, Asking }

fn agent(id: &str, name: &str, is: Is) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: !matches!(is, Is::Absent),
        version: matches!(is, Is::Ready).then(|| "1.18.27".into()),
        auth: match is { Is::Ready => Auth::Authenticated, Is::SignedOut => Auth::Unauthenticated, _ => Auth::Unknown },
        ready: matches!(is, Is::Ready),
        status: match is {
            Is::Ready => Probe::Ready,
            Is::Hung | Is::Asking => Probe::ProbeFailed,
            Is::Absent => Probe::Missing,
            Is::SignedOut => Probe::AuthRequired,
        },
        unsupported_note: None,
        through_cli: false,
        checking: matches!(is, Is::Asking),
    }
}

/// Codec's box (codex), after discovery answered with these.
fn found(agents: Vec<Found>) -> State {
    let mut s = State::default();
    crate::scenes::composer_base_scene(&mut s, false);
    s.settings_panel.agents = Some(agents);
    s
}

fn hung(n: usize) -> Vec<Found> {
    let mut all = vec![agent("codex", "Codex CLI", Is::Hung)];
    all.extend((1..n).map(|i| agent(&format!("hung{i}"), &format!("Agent {i}"), Is::Hung)));
    all
}

// case: says one in the singular
#[test]
fn says_one_in_the_singular() {
    let said = composer_placeholder(&found(hung(1)));
    assert!(said.contains("An AI agent is installed but not answering"), "{said}");
    assert!(!said.chars().next().unwrap().is_ascii_digit());
}

// case: counts five as five, not as one
#[test]
fn counts_five_as_five() {
    let said = composer_placeholder(&found(hung(5)));
    assert!(said.contains("5 AI agents are installed but not answering"), "{said}");
    assert!(!said.contains("An AI agent is installed"));
}

// case: counts only what is installed, not every runtime it knows about
#[test]
fn counts_only_the_installed() {
    let said = composer_placeholder(&found(vec![
        agent("codex", "Codex CLI", Is::Hung),
        agent("claude", "Claude Code", Is::Hung),
        agent("cursor", "Cursor Agent", Is::Absent),
        agent("copilot", "Copilot CLI", Is::Absent),
    ]));
    assert!(said.contains("2 AI agents are installed but not answering"), "{said}");
    assert!(!said.contains("3 AI agents") && !said.contains("4 AI agents"));
}

// case: does not complain about hung agents when one of them works
#[test]
fn a_working_agent_is_named_instead_of_the_hung_ones() {
    let said = composer_placeholder(&found(vec![
        agent("opencode", "OpenCode", Is::Ready),
        agent("codex", "Codex CLI", Is::Hung),
        agent("claude", "Claude Code", Is::Hung),
    ]));
    assert!(!said.contains("not answering"), "{said}");
    assert!(said.contains("OpenCode is ready"), "{said}");
}

// case: says nothing of the kind while discovery is still asking
#[test]
fn nothing_of_the_kind_while_discovery_asks() {
    let mut asking = hung(3);
    asking[1] = agent("hung1", "Agent 1", Is::Asking);
    let s = found(asking);
    let said = composer_placeholder(&s);
    assert!(!said.contains("not answering"), "{said}");
    assert_eq!(said, "Checking the AI agents on this machine…");
}

#[test]
fn a_hung_agent_holds_the_message_back_and_a_ready_one_takes_it() {
    let mut s = found(hung(1));
    s.draft.replace_text("Fix the build.");
    assert!(!selected_ready_now(&s));
    assert!(send_blocked_now(&s).is_some(), "a hung agent cannot take the message");
    // Ready alone is not usable: its check has to have answered ready too (runtimeIsUsable).
    let mut half = agent("codex", "Codex CLI", Is::Ready);
    half.status = Probe::Offline;
    let mut s = found(vec![half]);
    s.draft.replace_text("Fix the build.");
    assert!(!selected_ready_now(&s));
    assert!(send_blocked_now(&s).is_some());
    let s = found(vec![agent("codex", "Codex CLI", Is::Ready)]);
    assert!(selected_ready_now(&s));
    assert_eq!(composer_placeholder(&s), "Message Codec…");
}

#[test]
fn asks_for_a_sign_in_only_when_an_agent_said_it_wants_one() {
    let said = composer_placeholder(&found(vec![agent("codex", "Codex CLI", Is::SignedOut), agent("claude", "Claude Code", Is::Hung)]));
    assert_eq!(said, "Install an AI agent and sign in to start…");
    let mut wants = agent("codex", "Codex CLI", Is::SignedOut);
    wants.auth = Auth::Unknown;
    assert_eq!(composer_placeholder(&found(vec![wants])), "Install an AI agent and sign in to start…", "its check said so");
    let said = composer_placeholder(&found(hung(2)));
    assert!(!said.contains("sign in"), "{said}");
}

#[test]
fn discovery_cannot_say_and_the_window_keeps_its_answer() {
    // No answer yet, the box's agent still being asked, or an account route: the window's own readiness stands.
    let mut s = found(vec![]);
    s.settings_panel.agents = None;
    assert_eq!(discovered_ready(&s), None);
    assert!(selected_ready_now(&s));
    let s = found(vec![agent("codex", "Codex CLI", Is::Asking)]);
    assert_eq!(discovered_ready(&s), None);
    let mut s = found(hung(1));
    s.picks.insert("codec".into(), TurnPick { model: "account-default".into(), effort: None, runtime: None, account: Some(Some("work".into())) });
    assert_eq!(discovered_ready(&s), None, "an account route is the account's to answer");
}
