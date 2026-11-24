//! a-connector-can-be-added-to-every-agent.test.tsx (0.716): every case by name, drawn through Settings › Connectors
//! itself. Expected words are Electron's (AddConnector.tsx, settingsPages.ts), written out. The agents are a test's
//! own: each keeps what it was given and answers `mcp get` from that, so a second add reads it back from each agent.
use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use rune::Runtime;

/// Claude Code, Codex and Copilot, each keeping the connectors it is given by name, with the words it was given.
#[derive(Default)]
struct Kept {
    given: Mutex<HashMap<String, HashMap<String, Vec<String>>>>,
    installed: Vec<&'static str>,
}

impl Kept {
    fn agent_of(exe: &Path) -> String {
        exe.file_name().unwrap().to_string_lossy().into_owned()
    }
    fn has(&self, agent: &str, name: &str) -> Option<Vec<String>> {
        self.given.lock().unwrap().get(agent).and_then(|all| all.get(name)).cloned()
    }
}

impl connector_add::Runner for Kept {
    fn run(&self, exe: &Path, args: &[String], _limit: std::time::Duration) -> Result<connector_add::Ran, ()> {
        let agent = Self::agent_of(exe);
        let mut given = self.given.lock().unwrap();
        let mine = given.entry(agent).or_default();
        let at = |verb: &str| args.iter().position(|a| a == verb);
        let exit = if let Some(i) = at("get") {
            if mine.contains_key(&args[i + 1]) { 0 } else { 1 }
        } else if let Some(i) = at("add") {
            // The name is the first word after `add` that is not a flag or a flag's value.
            let mut j = i + 1;
            while args[j].starts_with("--") {
                j += 2;
            }
            mine.insert(args[j].clone(), args.to_vec());
            0
        } else if let Some(i) = at("remove") {
            mine.remove(args.last().unwrap_or(&args[i]));
            0
        } else {
            1
        };
        Ok(connector_add::Ran { exit: Some(exit), ..Default::default() })
    }
}

impl Agents for Kept {
    fn launch(&self, agent: &str) -> Option<Launch> {
        self.installed.contains(&agent).then(|| Launch { executable: PathBuf::from(format!("/bin/{agent}")), prefix_args: Vec::new() })
    }
    fn runner(&self) -> &dyn connector_add::Runner {
        self
    }
}

fn view(s: &State, _cx: &mut rune::Cx<State>) -> El {
    crate::settings_panel::view(s, &Palette::new(true))
}

/// Settings open on Connectors, with these agents installed, found the way the page finds them.
fn connectors_page(installed: Vec<&'static str>) -> (Arc<Kept>, Runtime<State>) {
    let kept = Arc::new(Kept { installed, ..Default::default() });
    use_agents(kept.clone());
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Connectors);
    let mut rt = Runtime::new(s, view, (1000.0, 2400.0), rune::Env::new(true, 1.0));
    rt.update(|s, _| crate::connectors_section::request(s, false));
    until(&mut rt, |s| s.connectors.add.installed.is_some());
    (kept, rt)
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

fn until(rt: &mut Runtime<State>, done: impl Fn(&State) -> bool) {
    for _ in 0..1000 {
        rt.update(|s, _| {
            crate::connectors_section::poll(s);
        });
        if done(&rt.state) {
            settle(rt);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the agents never answered");
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text.or(e.label)).collect()
}

fn state_of(rt: &Runtime<State>, agent: &str) -> Option<String> {
    let key = format!("add-connector-result-{agent}");
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key.as_str())).and_then(|e| e.label)
}

#[test]
fn starts_as_one_button_and_says_so_when_no_agent_here_can_take_one() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_kept, mut rt) = connectors_page(vec!["claude", "codex"]);
    let shown = words(&rt);
    assert!(shown.iter().any(|w| w == "Add a connector…"), "{shown:?}");
    assert!(rt.find("add-connector-form").is_none(), "one button, no form");
    assert!(!shown.iter().any(|w| w == "None of the agents that can take one is installed here."), "{shown:?}");
    // Not disabled: pressing it opens the form.
    click(&mut rt, "add-connector-start");
    assert!(rt.find("add-connector-form").is_some());

    let (_kept, mut rt) = connectors_page(vec![]);
    assert!(words(&rt).iter().any(|w| w == "None of the agents that can take one is installed here."), "{:?}", words(&rt));
    click(&mut rt, "add-connector-start");
    assert!(rt.find("add-connector-form").is_none(), "disabled: pressing it opens nothing");
}

#[test]
fn says_what_each_agent_did_in_the_connector_lists_own_words_and_tones() {
    let row = |agent: &str, outcome, said: Option<&str>, undo| Row { result: AgentResult { agent: agent.into(), outcome, said: said.map(str::to_owned) }, undo };
    assert_eq!(row_words(&row("claude", Outcome::Added, None, false)), ("Added", "good", None));
    assert_eq!(
        row_words(&row("codex", Outcome::Had, None, false)),
        ("Had one", "muted", Some("It already has a connector by this name, and keeps its own.".into()))
    );
    assert_eq!(row_words(&row("copilot", Outcome::Failed, Some("Error: bad name"), false)), ("Not added", "amber", Some("Error: bad name".into())));
    assert_eq!(
        row_words(&row("opencode", Outcome::Kept, Some("OpenCode has no command to remove one."), false)),
        ("Still has it", "amber", Some("OpenCode has no command to remove one.".into()))
    );
    // An Undo that failed is not "not added": it was added, and is still there.
    assert_eq!(row_words(&row("claude", Outcome::Failed, None, true)).0, "Not taken back");
    assert_eq!(row_words(&row("claude", Outcome::Removed, None, true)).0, "Taken back");
}

#[test]
fn is_found_by_the_words_a_person_types_for_it() {
    use crate::settings_pages::{SETTINGS_PAGES, SettingsPageId, matched_headings};
    let connectors = SETTINGS_PAGES.iter().find(|page| page.id == SettingsPageId::Connectors).unwrap();
    assert!(connectors.headings.contains(&"Add a connector"));
    for typed in ["mcp server", "add mcp", "every agent"] {
        assert!(matched_headings(connectors, typed).contains(&"Add a connector"), "{typed}");
    }
}

/// Beside Electron's cases, its drive (`drive-a-connector-goes-to-every-agent.mjs`) in the window: fill the form on
/// the Connectors page, then read each agent's own answer back.
#[test]
fn a_connector_added_through_the_form_is_kept_by_every_agent_chosen_and_each_reads_it_back() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (kept, mut rt) = connectors_page(vec!["claude", "codex", "copilot"]);
    let fill = |rt: &mut Runtime<State>| {
        click(rt, "add-connector-start");
        rt.update(|s, _| {
            s.connectors.add.name = TextEdit::new("rea");
            s.connectors.add.value = TextEdit::new("npx -y rea-agents@6.3.0 mcp");
        });
        settle(rt);
        assert!(words(rt).iter().any(|w| w == "Add to 3 agents"), "{:?}", words(rt));
        click(rt, "add-connector-add");
        until(rt, |s| s.connectors.add.busy.is_none() && s.connectors.add.results.is_some());
    };
    fill(&mut rt);
    for agent in ["claude", "codex", "copilot"] {
        let given = kept.has(agent, "rea").unwrap_or_else(|| panic!("{agent} was not given rea"));
        assert!(given.join(" ").ends_with("rea -- npx -y rea-agents@6.3.0 mcp"), "{agent}: {given:?}");
        assert_eq!(state_of(&rt, agent).as_deref(), Some("Added"), "{agent}");
    }
    assert!(words(&rt).iter().any(|w| w == "Those agents have it from their next turn."), "{:?}", words(&rt));

    // Added again, each agent's own `mcp get` finds it: every row says it had one, and none is given a second.
    let adds = |kept: &Kept| kept.given.lock().unwrap().values().map(HashMap::len).sum::<usize>();
    let before = adds(&kept);
    fill(&mut rt);
    for agent in ["claude", "codex", "copilot"] {
        assert_eq!(state_of(&rt, agent).as_deref(), Some("Had one"), "{agent}");
    }
    assert_eq!(adds(&kept), before);
}
