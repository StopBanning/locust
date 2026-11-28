use super::*;
use locust_core::agents::{Auth, Probe};
use locust_core::discovery_service::{DiscoveryService, Seams};
use rune::{Env, Runtime};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const CURSOR_PAGE: &str = "https://cursor.com/cli";

fn missing(id: &str, name: &str) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: false,
        version: None,
        auth: Auth::Unknown,
        ready: false,
        status: Probe::Missing,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..12 {
        rt.frame(0.05);
    }
}

fn say(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn focus_again(rt: &mut Runtime<State>) {
    rt.window_focus(false);
    rt.window_focus(true);
    settle(rt);
}

fn wait_for(rt: &mut Runtime<State>, f: impl Fn(&State) -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !f(&rt.state) && Instant::now() < until {
        rt.update(|s, _| { poll(s); });
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(rt);
}

// case: a Get it page opened asks about every agent on the next return, the uninstalled too
#[test]
fn agents_vendor_page_rechecks_every_agent_once_on_return() {
    check_vendor_page_return(false);
    check_vendor_page_return(true);
}

fn check_vendor_page_return(overlapping_sweep: bool) {
    let calls = Arc::new(AtomicUsize::new(0));
    let installed = Arc::new(AtomicBool::new(false));
    let cursor = missing("cursor", "Cursor Agent");
    let kilo = missing("kilo", "Kilo");
    let service = use_discovery(DiscoveryService::new(Seams {
        probe: Box::new({
            let (calls, installed, cursor, kilo) = (calls.clone(), installed.clone(), cursor.clone(), kilo.clone());
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                let find = |name: &str| (installed.load(Ordering::SeqCst) && name == "kilo").then(|| "/fixture/kilo".into());
                let run = |_p: &std::path::Path, args: &[&str]| {
                    let stdout = match args {
                        ["--version"] => "0.3.1",
                        ["acp", "--help"] => "Agent Client Protocol",
                        _ => panic!("unexpected Kilo probe: {args:?}"),
                    };
                    Some(agents::Answer { exit: Some(0), stdout: stdout.into(), stderr: String::new(), timed_out: false })
                };
                let seams = agents::Seams {
                    find: &find, run: &run, exists: &|_| false, home: None, xdg_config_home: None, recall: None,
                };
                let checked = agents::discover_one("kilo", &seams).unwrap();
                vec![cursor.clone(), if checked.installed { checked } else { kilo.clone() }]
            }
        }),
        now: Box::new(|| 0),
        ttl_ms: 60_000,
        held: Box::new(|| None),
    }));
    let mut s = State::default();
    s.home.home_open = true;
    s.home.agents_open = true;
    s.home.others_open = true;
    s.settings_panel.agents = Some(service.get().unwrap());
    let mut rt = Runtime::new(s, crate::view, (1420., 1600.), Env::new(true, 1.));
    settle(&mut rt);
    assert!(rt.find("agents-install-kilo").is_some());
    assert!(rt.find("agents-get-it-kilo").is_none(), "npm agents keep Install, not a vendor button");
    focus_again(&mut rt);
    assert!(!rt.state.settings_panel.agents_job.is_pending());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "plain focus does not start a sweep");

    let (x, y) = rt.find("agents-get-it-cursor").expect("Home offers Get it for Cursor");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert_eq!(rt.state.opened_links, [CURSOR_PAGE], "Get it opens only Cursor's approved vendor URL");
    assert!(!rt.state.settings_panel.agents_job.is_pending(), "opening a page does not probe until focus returns");

    let blocked = overlapping_sweep.then(|| {
        let (release, wait) = std::sync::mpsc::channel();
        let stale = rt.state.settings_panel.agents.clone().unwrap();
        rt.update(|s, _| s.settings_panel.agents_job.request((), move |()| {
            let _ = wait.recv();
            stale
        }));
        release
    });
    set_npm_stand_in(Some(false));
    installed.store(true, Ordering::SeqCst);
    focus_again(&mut rt);
    assert!(rt.state.settings_panel.agents_job.is_pending() || calls.load(Ordering::SeqCst) == 2,
        "focus must start or queue the real discovery job");
    if let Some(release) = blocked {
        assert_eq!(calls.load(Ordering::SeqCst), 1, "an overlapping job must finish before the fresh sweep starts");
        release.send(()).unwrap();
    }
    wait_for(&mut rt, |s| s.settings_panel.agents.as_ref().unwrap().iter().any(|f| f.id == "kilo" && f.ready));
    assert!(say(&rt).contains("0.3.1"), "another, previously uninstalled agent is now ready");
    assert!(rt.find("agents-install-kilo").is_none(), "a ready agent no longer offers Install");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "focus invalidates the cached answer");
    assert!(!npm_missing(), "the vendor return forgets the stale npm refusal as well");
    focus_again(&mut rt);
    assert!(!rt.state.settings_panel.agents_job.is_pending(), "only the first return asks again");
}

#[test]
fn agents_vendor_browser_refusal_uses_electrons_words() {
    let mut s = State::default();
    s.home.others_open = true;
    s.settings_panel.agents = Some(vec![missing("cursor", "Cursor Agent")]);
    // The shell's failure is delivered through the same window field as a blocked external link.
    s.link_refusal = Some((CURSOR_PAGE.into(), locust_core::published_links::NO_BROWSER.into()));
    let mut rt = Runtime::new(s, |s, _| home_section(s, &Palette::new(true)), (1100., 1400.), Env::new(true, 1.));
    settle(&mut rt);
    assert!(rt.elements().iter().any(|e| !e.exiting && e.text.as_deref() == Some("Your browser could not be opened.")));
}
