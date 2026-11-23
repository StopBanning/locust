//! OpenCode sets itself up as the window finds it, not when the first message starts (Electron `main/index.ts`'s
//! discovery handler, 0.780: `void openCodeFirstStart.ensure()` once OpenCode 1 is found ready).
use super::*;
use locust_core::agents::{Auth, Probe};
use locust_core::discovery_service::{DiscoveryService, Seams};
use locust_core::opencode_first_start::{OpenCodeFirstStart, SetupCommand, SetupLaunch};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn opencode(version: &str, ready: bool) -> Found {
    Found {
        id: "opencode".into(),
        display_name: "OpenCode".into(),
        installed: true,
        version: Some(version.into()),
        auth: Auth::NotApplicable,
        ready,
        status: if ready { Probe::Ready } else { Probe::ProbeFailed },
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

/// The window on a fake sweep that answers `found`, and a fake set-up whose commands are kept, never run.
fn window_finding(found: Vec<Found>) -> (rune::Runtime<State>, Arc<Mutex<Vec<SetupCommand>>>) {
    use_discovery(DiscoveryService::new(Seams {
        probe: Box::new(move || found.clone()),
        now: Box::new(|| 0),
        ttl_ms: 10_000,
        held: Box::new(|| None),
    }));
    let ran = Arc::new(Mutex::new(Vec::new()));
    let kept = ran.clone();
    use_opencode_first_start(OpenCodeFirstStart::new(
        || Some(SetupLaunch { executable_path: "/opt/bin/opencode".into(), prefix_args: Vec::new(), env: BTreeMap::new() }),
        || Some("file:///guard/opencode-command-guard.js".into()),
        move |command| {
            kept.lock().unwrap().push(command);
            None
        },
        || PathBuf::from("/home/.config/opencode"),
        |_| false,
    ));
    let mut s = State::default();
    s.settings_panel.agents = Some(Vec::new());
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    (rt, ran)
}

fn ask_and_settle(rt: &mut rune::Runtime<State>) {
    rt.update(|s, _| request(s));
    let until = Instant::now() + Duration::from_secs(10);
    while rt.state.settings_panel.agents_job.is_pending() && Instant::now() < until {
        rt.update(|s, _| {
            poll(s);
        });
        std::thread::sleep(Duration::from_millis(5));
    }
    rt.update(|s, _| {
        poll(s);
    });
    assert!(!rt.state.settings_panel.agents_job.is_pending(), "the sweep never landed");
}

fn ran_within(ran: &Arc<Mutex<Vec<SetupCommand>>>, wait: Duration) -> Vec<SetupCommand> {
    let until = Instant::now() + wait;
    while ran.lock().unwrap().is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    ran.lock().unwrap().clone()
}

#[test]
fn finding_opencode_1_ready_sets_it_up_once_before_any_message() {
    let (mut rt, ran) = window_finding(vec![opencode("1.19.2", true)]);
    ask_and_settle(&mut rt);
    let ran = ran_within(&ran, Duration::from_secs(10));
    assert_eq!(ran.len(), 1, "the window's discovery started OpenCode's set-up");
    assert_eq!(ran[0].executable_path, "/opt/bin/opencode");
    assert_eq!(ran[0].args, vec!["debug".to_owned(), "config".to_owned()]);
    assert_eq!(
        ran[0].env.get("OPENCODE_CONFIG_CONTENT").map(String::as_str),
        Some(r#"{"plugin":["file:///guard/opencode-command-guard.js"]}"#)
    );
}

#[test]
fn opencode_2_or_one_not_ready_is_not_set_up() {
    for found in [opencode("2.0.1", true), opencode("1.19.2", false)] {
        let (mut rt, ran) = window_finding(vec![found.clone()]);
        ask_and_settle(&mut rt);
        assert!(ran_within(&ran, Duration::from_millis(300)).is_empty(), "{found:?} started a set-up");
    }
}
