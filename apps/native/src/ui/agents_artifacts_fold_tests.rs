//! Settings › AI agents lists what each agent has of its own, folded under its row (Electron's Screens.tsx
//! `<CliArtifacts>` fed by App.tsx's `listRuntimeArtifacts` once discovery settles): read as the window's discovery
//! lands, for the agents that are ready or signed out, and read again when that set changes.
use super::*;
use locust_core::agents::{Auth, Probe};
use locust_core::discovery_service::{DiscoveryService, Seams};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Home(std::path::PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A home folder with three Codex agents and an automation, a Claude agent and command, and a Cursor agent.
fn home() -> Home {
    let home = std::env::temp_dir().join(format!("locust-window-artifacts-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    let _ = std::fs::remove_dir_all(&home);
    let put = |rel: &str, body: &str| {
        let path = home.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    };
    put(".codex/agents/gig-scout.toml", "name = \"gig-scout\"\ndescription = \"Finds gigs\"\n");
    put(".codex/agents/reviewer.toml", "name = 'reviewer'\n");
    put(".codex/agents/tidy.toml", "");
    put(".codex/automations/nightly.toml", "name = \"nightly\"\n");
    put(".claude/agents/seo-scout.md", "---\nname: seo-scout\ndescription: Audits pages\n---\nbody\n");
    put(".claude/commands/standup.md", "---\ndescription: Daily standup\n---\n");
    put(".cursor/agents/ghost.md", "---\nname: ghost\n---\n");
    Home(home)
}

fn agent(id: &str, name: &str, status: Probe) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: status != Probe::Missing,
        version: (status != Probe::Missing).then(|| "1.0.0".into()),
        auth: if status == Probe::AuthRequired { Auth::Unauthenticated } else { Auth::Authenticated },
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

/// The window on a fake sweep whose answer the test can change, reading `home` as the person's home folder.
fn window(home: &Home, answer: Arc<Mutex<Vec<Found>>>) -> rune::Runtime<State> {
    use_home(home.0.clone());
    use_discovery(DiscoveryService::new(Seams {
        probe: Box::new(move || answer.lock().unwrap().clone()),
        now: Box::new(|| 0),
        ttl_ms: 0,
        held: Box::new(|| None),
    }));
    let mut s = State::default();
    s.settings_panel.agents = Some(Vec::new());
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// Ask, then poll until the sweep and the read it starts have both landed.
fn sweep(rt: &mut rune::Runtime<State>) {
    rt.update(|s, _| {
        invalidate();
        request(s);
    });
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        rt.update(|s, _| {
            poll(s);
        });
        let p = &rt.state.settings_panel;
        if !p.agents_job.is_pending() && !p.artifacts_job.is_pending() {
            break;
        }
        assert!(Instant::now() < until, "the sweep or the read never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
    rt.update(|s, _| {
        poll(s);
    });
    settle(rt);
}

fn settle(rt: &mut rune::Runtime<State>) {
    for _ in 0..20 {
        rt.frame(0.016);
    }
}

fn press(rt: &mut rune::Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

/// Every word drawn inside one agent's row.
fn row_words(rt: &rune::Runtime<State>, id: &str) -> String {
    let all = rt.elements();
    let key = format!("agents-row-{id}");
    let Some(row) = all.iter().position(|e| e.key.as_deref() == Some(key.as_str())) else { return String::new() };
    let inside = |mut i: usize| loop {
        if i == row {
            return true;
        }
        match all[i].parent {
            Some(p) => i = p,
            None => return false,
        }
    };
    (0..all.len()).filter(|&i| !all[i].exiting && inside(i)).filter_map(|i| all[i].text.clone()).collect::<Vec<_>>().join(" | ")
}

// case: is listed as a fact about that runtime, with no apology needed
// case: draws nothing at all for a runtime that has none
#[test]
fn each_agent_row_carries_its_own_list_folded_with_the_count_on_the_fold() {
    let home = home();
    let answer = Arc::new(Mutex::new(vec![
        agent("codex", "Codex CLI", Probe::Ready),
        agent("claude", "Claude Code", Probe::AuthRequired),
        agent("cursor", "Cursor Agent", Probe::Missing),
    ]));
    let mut rt = window(&home, answer);
    sweep(&mut rt);

    let codex = row_words(&rt, "codex");
    assert!(codex.contains("3 agents \u{b7} 1 automation of its own"), "{codex}");
    // Signed out still counts: what they wrote is on this machine.
    let claude = row_words(&rt, "claude");
    assert!(claude.contains("1 agent \u{b7} 1 command of its own"), "{claude}");
    // Each row counts only its own agent's.
    assert!(!codex.contains("command"), "{codex}");
    // An agent that is not installed lists nothing, whatever its folder holds.
    assert!(!row_words(&rt, "cursor").contains("of its own"), "{}", row_words(&rt, "cursor"));
    // Folded: the names wait for a press.
    assert!(!codex.contains("gig-scout"), "{codex}");
    assert!(!codex.contains(locust_core::runtime_artifacts::LISTS_NOT_RUNS), "{codex}");

    press(&mut rt, "agents-artifacts-fold-codex");
    let codex = row_words(&rt, "codex");
    assert!(codex.contains(locust_core::runtime_artifacts::LISTS_NOT_RUNS), "{codex}");
    for said in ["agent | gig-scout | Finds gigs | agents/gig-scout.toml", "reviewer", "tidy", "automation | nightly | automations/nightly.toml"] {
        assert!(codex.contains(said), "{said} missing from {codex}");
    }
    // Opening one agent's fold leaves the others folded. (Its name is on the row's setup line, "Agents: seo-scout";
    // what the fold adds is the description and where it lives.)
    assert!(!row_words(&rt, "claude").contains("Audits pages"), "{}", row_words(&rt, "claude"));

    press(&mut rt, "agents-artifacts-fold-codex");
    assert!(!row_words(&rt, "codex").contains("gig-scout"), "{}", row_words(&rt, "codex"));
}

#[test]
fn the_list_is_read_again_when_which_agents_are_there_changes() {
    let home = home();
    let answer = Arc::new(Mutex::new(vec![agent("codex", "Codex CLI", Probe::Ready), agent("claude", "Claude Code", Probe::Missing)]));
    let mut rt = window(&home, answer.clone());
    sweep(&mut rt);
    assert!(row_words(&rt, "codex").contains("of its own"), "{}", row_words(&rt, "codex"));
    assert!(!row_words(&rt, "claude").contains("of its own"), "{}", row_words(&rt, "claude"));

    // Claude Code is installed and Codex uninstalled: the next sweep reads the lists again.
    *answer.lock().unwrap() = vec![agent("codex", "Codex CLI", Probe::Missing), agent("claude", "Claude Code", Probe::Ready)];
    sweep(&mut rt);
    assert!(!row_words(&rt, "codex").contains("of its own"), "{}", row_words(&rt, "codex"));
    assert!(row_words(&rt, "claude").contains("1 agent \u{b7} 1 command of its own"), "{}", row_words(&rt, "claude"));
}
