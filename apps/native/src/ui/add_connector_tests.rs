//! The form on Settings › Connectors (Electron `AddConnector.tsx`), drawn and pressed: what it offers, what it sends
//! to the agents, what each said, and Undo. The agents are a test's own: no real CLI runs.
use super::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use rune::Runtime;

/// Claude Code and Codex installed; each has nothing yet, and every add and remove succeeds. Every run is kept.
struct Fake {
    calls: Mutex<Vec<Vec<String>>>,
    /// Agents that already have one by the name asked about.
    had: Vec<&'static str>,
}

impl connector_add::Runner for Fake {
    fn run(&self, exe: &Path, args: &[String], _limit: std::time::Duration) -> Result<connector_add::Ran, ()> {
        let mut words = vec![exe.to_string_lossy().into_owned()];
        words.extend(args.iter().cloned());
        self.calls.lock().unwrap().push(words);
        let had = self.had.iter().any(|agent| exe.ends_with(agent));
        let exit = if args.iter().any(|a| a == "get") && !had { 1 } else { 0 };
        Ok(connector_add::Ran { exit: Some(exit), ..Default::default() })
    }
}

impl Agents for Fake {
    fn launch(&self, agent: &str) -> Option<Launch> {
        matches!(agent, "claude" | "codex").then(|| Launch { executable: PathBuf::from(format!("/bin/{agent}")), prefix_args: Vec::new() })
    }
    fn runner(&self) -> &dyn connector_add::Runner {
        self
    }
}

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn drawn(installed: &[&str]) -> (Arc<Fake>, Runtime<State>) {
    drawn_with(installed, Vec::new())
}

fn drawn_with(installed: &[&str], had: Vec<&'static str>) -> (Arc<Fake>, Runtime<State>) {
    let fake = Arc::new(Fake { calls: Mutex::new(Vec::new()), had });
    use_agents(fake.clone());
    let mut s = State::default();
    s.connectors.add.installed = Some(installed.iter().map(|a| (*a).to_owned()).collect());
    let mut rt = Runtime::new(s, |s, _| section(s, &Palette::new(true)), (720.0, 900.0), rune::Env::new(true, 1.0));
    settle(&mut rt);
    (fake, rt)
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

/// Until the agents' answer lands, as the window's poll takes it.
fn answered(rt: &mut Runtime<State>) {
    for _ in 0..1000 {
        let mut landed = false;
        rt.update(|s, _| landed = poll(s) && s.connectors.add.busy.is_none());
        if landed {
            settle(rt);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the agents never answered");
}

fn label_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    let (x, y) = rt.find(key)?;
    rt.elements().iter().find(|e| e.key.as_deref().is_some_and(|k| k.ends_with(key)) && e.rect[0] <= x && x <= e.rect[0] + e.rect[2] && e.rect[1] <= y && y <= e.rect[1] + e.rect[3]).and_then(|e| e.label.clone())
}

#[test]
fn offers_nothing_when_no_agent_that_can_take_one_is_installed() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (fake, mut rt) = drawn(&[]);
    assert!(rt.find("add-connector-none").is_some());
    click(&mut rt, "add-connector-start");
    assert!(!rt.state.connectors.add.open, "a button with nothing to offer opens nothing");
    assert!(fake.calls.lock().unwrap().is_empty());
}

#[test]
fn adds_to_every_agent_chosen_says_what_each_said_and_undo_takes_it_back() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (fake, mut rt) = drawn(&["claude", "codex"]);
    assert!(rt.find("add-connector-none").is_none());
    click(&mut rt, "add-connector-start");
    assert!(rt.find("add-connector-form").is_some());
    // Every installed agent is chosen until one is turned off.
    assert_eq!(label_of(&rt, "add-connector-add").as_deref(), Some("Add to 2 agents"));
    rt.update(|s, _| s.connectors.add.name = TextEdit::new("rea"));
    rt.state.connectors.add.value = TextEdit::new("npx -y rea-agents@6.3.0 mcp");
    click(&mut rt, "add-connector-add");
    answered(&mut rt);
    // Each agent was asked first, then given it in its own words.
    let calls = fake.calls.lock().unwrap().clone();
    assert!(calls.contains(&vec!["/bin/claude".into(), "mcp".into(), "get".into(), "rea".into()]), "{calls:?}");
    assert!(calls.iter().any(|c| c[0] == "/bin/codex" && c[1..].join(" ") == "mcp add rea -- npx -y rea-agents@6.3.0 mcp"), "{calls:?}");
    // All of it done: the form goes, and what each said stays.
    assert!(rt.find("add-connector-form").is_none());
    assert_eq!(label_of(&rt, "add-connector-result-claude").as_deref(), Some("Added"));
    assert_eq!(label_of(&rt, "add-connector-result-codex").as_deref(), Some("Added"));
    assert_eq!(label_of(&rt, "add-connector-start").as_deref(), Some("Add another…"));
    click(&mut rt, "add-connector-undo");
    answered(&mut rt);
    assert_eq!(label_of(&rt, "add-connector-result-claude").as_deref(), Some("Taken back"));
    assert!(fake.calls.lock().unwrap().iter().any(|c| c[0] == "/bin/claude" && c[1..].join(" ") == "mcp remove --scope user rea"));
    // Nothing is left to take back.
    assert!(rt.find("add-connector-undo").is_none());
}

#[test]
fn an_agent_turned_off_is_not_given_it_and_a_name_that_is_not_one_reaches_no_agent() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (fake, mut rt) = drawn(&["claude", "codex"]);
    click(&mut rt, "add-connector-start");
    click(&mut rt, "add-connector-agent-codex");
    assert_eq!(label_of(&rt, "add-connector-add").as_deref(), Some("Add to 1 agent"));
    rt.state.connectors.add.name = TextEdit::new("two words");
    rt.state.connectors.add.value = TextEdit::new("npx x");
    click(&mut rt, "add-connector-add");
    assert_eq!(rt.state.connectors.add.problem.as_deref(), Some("A name is letters, digits, - and _, up to 64, starting with a letter or digit."));
    assert!(rt.find("add-connector-problem").is_some());
    assert!(fake.calls.lock().unwrap().is_empty());
    rt.state.connectors.add.name = TextEdit::new("rea");
    click(&mut rt, "add-connector-add");
    answered(&mut rt);
    let calls = fake.calls.lock().unwrap().clone();
    assert!(calls.iter().all(|c| c[0] == "/bin/claude"), "{calls:?}");
    // With every agent turned off, it asks for one, and nobody is run.
    click(&mut rt, "add-connector-start");
    click(&mut rt, "add-connector-agent-claude");
    assert_eq!(label_of(&rt, "add-connector-add").as_deref(), Some("Add to 0 agents"));
    let before = fake.calls.lock().unwrap().len();
    rt.state.connectors.add.name = TextEdit::new("rea");
    rt.state.connectors.add.value = TextEdit::new("npx x");
    rt.update(|s, _| add(s));
    assert_eq!(rt.state.connectors.add.problem.as_deref(), Some("Choose at least one agent."));
    assert_eq!(rt.state.connectors.add.busy, None);
    assert_eq!(fake.calls.lock().unwrap().len(), before);
}

#[test]
fn undo_takes_it_back_only_from_the_agents_just_given_it() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (fake, mut rt) = drawn_with(&["claude", "codex"], vec!["codex"]);
    click(&mut rt, "add-connector-start");
    rt.update(|s, _| s.connectors.add.name = TextEdit::new("rea"));
    rt.state.connectors.add.value = TextEdit::new("npx -y rea-agents@6.3.0 mcp");
    click(&mut rt, "add-connector-add");
    answered(&mut rt);
    assert_eq!(label_of(&rt, "add-connector-result-codex").as_deref(), Some("Had one"));
    click(&mut rt, "add-connector-undo");
    answered(&mut rt);
    let removes: Vec<_> = fake.calls.lock().unwrap().iter().filter(|c| c.iter().any(|w| w == "remove")).cloned().collect();
    // Codex keeps the one it had: only Claude Code is asked to remove.
    assert_eq!(removes.len(), 1, "{removes:?}");
    assert_eq!(removes[0][0], "/bin/claude");
    assert_eq!(label_of(&rt, "add-connector-result-codex").as_deref(), Some("Had one"));
}

#[test]
fn a_result_row_says_its_state_and_why() {
    let row = |outcome, said: Option<&str>, undo| Row { result: AgentResult { agent: "claude".into(), outcome, said: said.map(str::to_owned) }, undo };
    assert_eq!(row_words(&row(Outcome::Added, None, false)), ("Added", "good", None));
    assert_eq!(row_words(&row(Outcome::Had, None, false)), ("Had one", "muted", Some("It already has a connector by this name, and keeps its own.".into())));
    assert_eq!(row_words(&row(Outcome::Failed, Some("Error: x"), false)), ("Not added", "amber", Some("Error: x".into())));
    assert_eq!(row_words(&row(Outcome::Failed, None, true)).0, "Not taken back");
    assert_eq!(row_words(&row(Outcome::Removed, None, true)), ("Taken back", "muted", None));
    assert_eq!(row_words(&row(Outcome::Kept, Some("OpenCode has no command"), true)), ("Still has it", "amber", Some("OpenCode has no command".into())));
}
