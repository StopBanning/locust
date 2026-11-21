//! Settings > AI agents says, in plain words, what an ACP agent told Locust it can do, as the agent's own answer with
//! when (Electron's renderer/src/an-acp-agent-says-what-it-can-do.test.ts), and the agent's row reads what was kept
//! as it is drawn, with no new probe of the CLI (main/what-an-acp-agent-can-do-is-kept.test.ts, its Settings cases).
use super::*;
use locust_core::agents::{Auth, Probe};
use serde_json::json;

fn told() -> serde_json::Value {
    json!({"continuesSessions": true, "images": false, "audio": false, "embeddedContext": true, "mcpOverHttp": true, "mcpOverSse": false, "toldAt": "2026-10-01T09:00:00.000Z"})
}
fn at(when: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(when).unwrap().with_timezone(&chrono::Utc)
}

// case: says each thing as a plain yes or no
#[test]
fn says_each_thing_as_a_plain_yes_or_no() {
    assert_eq!(
        ability_lines(&told()),
        [
            "Can continue an earlier session: yes",
            "Takes pictures in a message: no",
            "Takes sound in a message: no",
            "Takes a file\u{2019}s contents in a message: yes",
            "Connects to connectors over the web (HTTP): yes",
            "Connects to connectors over SSE: no",
        ]
    );
    let mut all = told();
    for key in ["images", "audio", "mcpOverSse"] {
        all[key] = json!(true);
    }
    assert!(ability_lines(&all).iter().all(|line| line.ends_with(": yes")), "{:?}", ability_lines(&all));
}

// case: says it is the agent’s own word, and when it said it
#[test]
fn says_it_is_the_agents_own_word_and_when_it_said_it() {
    let heading = |now: &str| ability_heading("GitHub Copilot", &told(), at(now));
    assert_eq!(heading("2026-10-01T18:00:00.000Z"), "What GitHub Copilot said it can do (today):");
    assert_eq!(heading("2026-10-02T10:00:00.000Z"), "What GitHub Copilot said it can do (yesterday):");
    assert_eq!(heading("2026-10-04T10:00:00.000Z"), "What GitHub Copilot said it can do (3 days ago):");
    // A whole day is 24 hours from the telling, not a calendar day: 08:59 two days on is still yesterday.
    assert_eq!(heading("2026-10-03T08:59:00.000Z"), "What GitHub Copilot said it can do (yesterday):");
    // A bare date is midnight UTC, as `Date.parse` reads it; a time that is not one is "at its last run".
    let mut bare = told();
    bare["toldAt"] = json!("2026-10-01");
    assert_eq!(ability_heading("Kilo", &bare, at("2026-10-03T00:00:00.000Z")), "What Kilo said it can do (2 days ago):");
    bare["toldAt"] = json!("whenever");
    assert_eq!(ability_heading("Kilo", &bare, at("2026-10-03T00:00:00.000Z")), "What Kilo said it can do (at its last run):");
}

fn agent(id: &str, name: &str) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: true,
        version: Some("1.0.0".into()),
        auth: Auth::Authenticated,
        ready: true,
        status: Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}
fn window(agents: Vec<Found>) -> rune::Runtime<State> {
    let mut s = State::default();
    s.settings_panel.agents = Some(agents);
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}
/// Every word drawn inside one agent's row, top to bottom.
fn row_words(rt: &rune::Runtime<State>, id: &str) -> Vec<String> {
    let all = rt.elements();
    let key = format!("agents-row-{id}");
    let Some(row) = all.iter().position(|e| e.key.as_deref() == Some(key.as_str())) else { return Vec::new() };
    let inside = |mut i: usize| loop {
        if i == row {
            return true;
        }
        match all[i].parent {
            Some(p) => i = p,
            None => return false,
        }
    };
    (0..all.len()).filter(|&i| !all[i].exiting && inside(i)).filter_map(|i| all[i].text.clone()).collect()
}

// case: reaches Settings on the runtime it was said by, and no other
#[test]
fn the_agents_row_says_what_it_said_and_no_other_row_does() {
    locust_core::acp_capabilities::record_said(
        "copilot",
        &json!({"loadSession": true, "promptCapabilities": {"image": true, "audio": false, "embeddedContext": true}, "mcpCapabilities": {"http": false, "sse": false}}),
    );
    let rt = window(vec![agent("copilot", "GitHub Copilot"), agent("codex", "Codex CLI")]);
    let copilot = row_words(&rt, "copilot");
    let from = copilot.iter().position(|w| w == "What GitHub Copilot said it can do (today):").unwrap_or_else(|| panic!("{copilot:?}"));
    assert_eq!(
        copilot[from + 1..from + 7],
        [
            "Can continue an earlier session: yes",
            "Takes pictures in a message: yes",
            "Takes sound in a message: no",
            "Takes a file\u{2019}s contents in a message: yes",
            "Connects to connectors over the web (HTTP): no",
            "Connects to connectors over SSE: no",
        ]
    );
    // Under the agent's name, not above it.
    assert_eq!(copilot.first().map(String::as_str), Some("GitHub Copilot"), "{copilot:?}");
    let codex = row_words(&rt, "codex");
    assert!(!codex.iter().any(|w| w.contains("said it can do") || w.ends_with(": yes") || w.ends_with(": no")), "{codex:?}");
}

// case: reaches an answer discovery already cached, without probing a CLI again (measured on 0.566)
#[test]
fn a_run_that_says_what_it_can_do_reaches_the_row_already_drawn_without_asking_again() {
    let mut rt = window(vec![agent("kilo", "Kilo Code")]);
    assert!(!row_words(&rt, "kilo").iter().any(|w| w.contains("said it can do")), "{:?}", row_words(&rt, "kilo"));
    // A Kilo run starts and says what it can do; the row drawn next says it, from what discovery already has.
    // The run's own events change the window's state, which draws it again: nothing here asks discovery.
    locust_core::acp_capabilities::record_said("kilo", &json!({"loadSession": true}));
    rt.update(|_, _| {});
    rt.frame(0.05);
    let kilo = row_words(&rt, "kilo");
    assert!(kilo.iter().any(|w| w == "What Kilo Code said it can do (today):"), "{kilo:?}");
    assert!(kilo.iter().any(|w| w == "Can continue an earlier session: yes"), "{kilo:?}");
    rt.update(|s, _| {
        assert!(!s.settings_panel.agents_job.is_pending());
        assert!(!s.settings_panel.agents_recheck.is_pending());
    });
}
