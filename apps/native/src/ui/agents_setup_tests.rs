//! Settings › AI agents says what each agent has set up for itself under its row (Electron's Screens.tsx
//! `<RuntimeSetupLine>`, fed by App.tsx's `readRuntimeSetup` with the chosen project folder): its MCP servers, hooks,
//! skills and agents, by name only, read from the agent's own files.
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

/// A home folder where Claude Code has a connector of its own and one for a project, a hook and a skill, and a
/// project folder with its own .mcp.json.
fn home() -> Home {
    let home = std::env::temp_dir().join(format!("locust-window-setup-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    let _ = std::fs::remove_dir_all(&home);
    let put = |rel: &str, body: &str| {
        let path = home.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    };
    let shop = home.join("shop");
    put(".claude.json", &format!(r#"{{"mcpServers":{{"github":{{"command":"npx","args":["--token","SECRET"]}}}},"projects":{{"{}":{{"mcpServers":{{"robinhood":{{}}}}}}}}}}"#, shop.display()));
    put(".claude/settings.json", r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"secret.sh"}]}]}}"#);
    put(".claude/skills/reskin/SKILL.md", "---\nname: reskin\n---\n");
    put("shop/.mcp.json", r#"{"mcpServers":{"playwright":{}}}"#);
    put(".cursor/mcp.json", "{not json");
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

/// Ask, then poll until the sweep and the setup read it starts have both landed.
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
        if !p.agents_job.is_pending() && !p.setup_job.is_pending() && !p.artifacts_job.is_pending() {
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

#[test]
fn an_agents_row_names_the_mcp_servers_hooks_and_skills_it_has_set_up_from_its_own_files() {
    let home = home();
    let answer = Arc::new(Mutex::new(vec![agent("claude", "Claude Code", Probe::Ready), agent("cursor", "Cursor Agent", Probe::Ready)]));
    let mut rt = window(&home, answer);
    rt.update(|s, _| s.home.folder_path = Some(home.0.join("shop").display().to_string()));
    sweep(&mut rt);

    let claude = row_words(&rt, "claude");
    assert!(claude.contains("Connectors (MCP): github, robinhood, playwright · Hooks: Stop (1) · Skills: reskin"), "{claude}");
    // Names only: never a server's command line or a hook's script.
    assert!(!claude.contains("SECRET") && !claude.contains("secret.sh"), "{claude}");
    // A file that cannot be parsed is counted on its own agent's row, never guessed at.
    let cursor = row_words(&rt, "cursor");
    assert!(cursor.contains("1 config file could not be read"), "{cursor}");
    assert!(!claude.contains("could not be read"), "{claude}");
}

#[test]
fn the_project_folders_own_connectors_are_read_again_when_the_folder_changes() {
    let home = home();
    let answer = Arc::new(Mutex::new(vec![agent("claude", "Claude Code", Probe::Ready)]));
    let mut rt = window(&home, answer);
    sweep(&mut rt);
    // No folder chosen: the user's own connector only.
    let claude = row_words(&rt, "claude");
    assert!(claude.contains("Connectors (MCP): github ·"), "{claude}");
    assert!(!claude.contains("playwright"), "{claude}");

    rt.update(|s, _| s.home.folder_path = Some(home.0.join("shop").display().to_string()));
    sweep(&mut rt);
    assert!(row_words(&rt, "claude").contains("Connectors (MCP): github, robinhood, playwright"), "{}", row_words(&rt, "claude"));
}
