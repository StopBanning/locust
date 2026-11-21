//! ONE CONNECTOR, ADDED TO EVERY AGENT YOU CHOOSE (Electron 0.716, `shared/connector-add.ts` and
//! `main/connector-add.ts`). Colin, 2026-10-09, of REA, an MCP server: "sure you can run connectors, or however you see
//! fit." Each agent keeps its own list of MCP servers in its own file and format; Locust never edits those files. It
//! runs each agent's own `mcp add`, the way a person would type it (the words were measured on each CLI in a throwaway
//! home on 2026-10-09), and says what each answered. Cursor Agent has no such command, so it is not offered.
//!
//! An agent that already has a connector of that name keeps it untouched: four of the six overwrite a name silently,
//! and the one there may hold a key Locust never saw. An agent whose answer cannot be read is not written to at all.
//! Nothing secret goes through here: no environment values, no headers.

use std::path::PathBuf;
use std::time::Duration;

/// The agents a connector can be added to, in the order they are offered.
pub const AGENTS: [&str; 6] = ["claude", "codex", "gemini", "copilot", "opencode", "antigravity"];

/// A listing can be slow (OpenCode's connects to every server) but never unbounded.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(50);
pub const CHANGE_TIMEOUT: Duration = Duration::from_secs(40);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunsAs {
    Command { command: String, args: Vec<String> },
    Url(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connector {
    pub name: String,
    pub runs_as: RunsAs,
}

/// A name every one of the agents takes: letters, digits, `-` and `_`, starting with a letter or digit, up to 64.
pub fn name_problem(name: &str) -> Option<&'static str> {
    let name = name.trim();
    if name.is_empty() {
        return Some("Give it a name.");
    }
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !first_ok || !rest_ok || name.chars().count() > 64 {
        return Some("A name is letters, digits, - and _, up to 64, starting with a letter or digit.");
    }
    None
}

/// A command line split as a shell would for this: spaces between words, a word in "double" or 'single' quotes kept
/// whole. Nothing else is interpreted (no variables, globs or pipes): nothing runs it through a shell. None when a
/// quote is left open.
pub fn split_command_line(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for ch in line.trim().chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                word.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            started = true;
            continue;
        }
        if ch.is_whitespace() {
            if started {
                words.push(std::mem::take(&mut word));
            }
            word.clear();
            started = false;
            continue;
        }
        word.push(ch);
        started = true;
    }
    if quote.is_some() {
        return None;
    }
    if started {
        words.push(word);
    }
    Some(words)
}

/// How the person said it runs: a command on this computer or a web address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    Url,
}

/// What the person typed, as a connector, or why it is not one yet.
pub fn connector_from(name: &str, kind: Kind, value: &str) -> Result<Connector, String> {
    if let Some(problem) = name_problem(name) {
        return Err(problem.into());
    }
    let name = name.trim().to_owned();
    match kind {
        Kind::Url => {
            let Ok(url) = url::Url::parse(value.trim()) else {
                return Err("That is not a web address. It starts with https://.".into());
            };
            if url.scheme() != "https" && url.scheme() != "http" {
                return Err("A web address here starts with https:// or http://.".into());
            }
            Ok(Connector { name, runs_as: RunsAs::Url(url.to_string()) })
        }
        Kind::Command => {
            let Some(words) = split_command_line(value) else {
                return Err("A quote is left open in the command.".into());
            };
            let mut words = words.into_iter();
            match words.next() {
                Some(command) if !command.is_empty() => Ok(Connector { name, runs_as: RunsAs::Command { command, args: words.collect() } }),
                _ => Err("Write the command that starts it, such as npx -y rea-agents@6.3.0 mcp.".into()),
            }
        }
    }
}

/// Gemini CLI's own flags for `mcp add` (its help, 2026-10-09): a server word spelling one is read as Gemini's.
const GEMINI_FLAGS: [&str; 18] = [
    "-d", "--debug", "-s", "--scope", "-t", "--transport", "--type", "-e", "--env", "-H", "--header", "--timeout", "--trust",
    "--description", "--include-tools", "--exclude-tools", "-h", "--help",
];

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| (*w).to_owned()).collect()
}

/// The words each agent's own `mcp add` takes, for everywhere it runs (its user scope, never one folder's); `--` keeps
/// a server's own flags (`npx -y ...`) from being read as the agent's. None for one that cannot take it as written.
pub fn add_args(agent: &str, connector: &Connector) -> Option<Vec<String>> {
    let name = connector.name.as_str();
    let command: Vec<String> = match &connector.runs_as {
        RunsAs::Command { command, args } => std::iter::once(command.clone()).chain(args.iter().cloned()).collect(),
        RunsAs::Url(_) => Vec::new(),
    };
    let with_command = |head: &[&str]| {
        let mut out = words(head);
        out.push("--".into());
        out.extend(command.iter().cloned());
        out
    };
    let url = match &connector.runs_as {
        RunsAs::Url(url) => Some(url.as_str()),
        RunsAs::Command { .. } => None,
    };
    Some(match (agent, url) {
        ("claude", Some(url)) => words(&["mcp", "add", "--scope", "user", "--transport", "http", name, url]),
        ("claude", None) => with_command(&["mcp", "add", "--scope", "user", name]),
        ("codex" | "opencode", Some(url)) => words(&["mcp", "add", name, "--url", url]),
        ("codex" | "opencode" | "copilot" | "antigravity", None) => with_command(&["mcp", "add", name]),
        ("gemini", Some(url)) => words(&["mcp", "add", "--scope", "user", "--transport", "http", name, url]),
        // Gemini CLI refuses `--` (measured: it printed its help), so the server's words follow bare, and one that
        // spells a Gemini flag would be taken as Gemini's: it is not offered a wrong entry.
        ("gemini", None) => {
            if command.iter().any(|word| GEMINI_FLAGS.contains(&word.split('=').next().unwrap_or(word))) {
                return None;
            }
            let mut out = words(&["mcp", "add", "--scope", "user", name]);
            out.extend(command.iter().cloned());
            out
        }
        ("copilot", Some(url)) => words(&["mcp", "add", "--transport", "http", name, url]),
        // Antigravity's flags must come before the name.
        ("antigravity", Some(url)) => words(&["mcp", "add", "--type", "http", name, url]),
        _ => return None,
    })
}

/// Why an agent cannot be given a connector as written, when [`add_args`] declines it.
pub fn declined_why(agent: &str) -> &'static str {
    if agent == "gemini" {
        "Gemini CLI would read one of the command\u{2019}s own flags as its own. Add it in Gemini CLI itself."
    } else {
        "It cannot take this connector as written."
    }
}

/// The words that take it out again, or None for an agent with no such command (OpenCode).
pub fn remove_args(agent: &str, name: &str) -> Option<Vec<String>> {
    match agent {
        "claude" | "gemini" => Some(words(&["mcp", "remove", "--scope", "user", name])),
        "codex" | "copilot" | "antigravity" => Some(words(&["mcp", "remove", name])),
        _ => None,
    }
}

/// What happened at one agent, in a word the card draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Added,
    Had,
    Failed,
    Removed,
    Kept,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentResult {
    pub agent: String,
    pub outcome: Outcome,
    /// The agent's own words, when it refused or failed; never a secret, since none was sent.
    pub said: Option<String>,
}

fn result(agent: &str, outcome: Outcome, said: Option<String>) -> AgentResult {
    AgentResult { agent: agent.to_owned(), outcome, said }
}

/// How to start one agent's CLI, as discovery found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub executable: PathBuf,
    pub prefix_args: Vec<String>,
}

/// What one run of an agent's CLI came to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ran {
    pub exit: Option<i64>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Runs an agent's CLI with its words, within a time limit. Err when it could not be started.
pub trait Runner: Sync {
    fn run(&self, executable: &std::path::Path, args: &[String], limit: Duration) -> Result<Ran, ()>;
}

fn run_with(runner: &dyn Runner, launch: &Launch, args: &[String], limit: Duration) -> Result<Ran, ()> {
    let mut all = launch.prefix_args.clone();
    all.extend(args.iter().cloned());
    runner.run(&launch.executable, &all, limit)
}

fn plain(text: &str) -> String {
    crate::invariants::regex(r"\x1b\[[0-9;?]*[A-Za-z]").replace_all(text, "").into_owned()
}

/// The agent's last meaningful line, for a card: never more than a sentence, without colours, frames or Codex's PATH
/// warning.
pub fn last_words(stdout: &str, stderr: &str) -> Option<String> {
    let text = plain(&format!("{stderr}\n{stdout}"));
    let frame = |c: char| c.is_whitespace() || "│┌└◆●○✓✗|".contains(c);
    let warning = crate::invariants::regex(r"(?i)^WARNING: proceeding, even though");
    let last = text
        .lines()
        .map(|line| line.trim_start_matches(frame).trim().to_owned())
        .filter(|line| !line.is_empty() && !warning.is_match(line))
        .last()?;
    Some(if last.chars().count() > 220 { format!("{}…", last.chars().take(219).collect::<String>()) } else { last })
}

/// The names an agent's own listing shows, for the three with no `get`: Antigravity's table, Gemini CLI's
/// `○ name: ...` lines, OpenCode's `✓ name` / `✗ name` lines. Formats measured 2026-10-09.
pub fn listed_names(agent: &str, output: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let table = crate::invariants::regex(r"(?i)^(\S+)\s+(stdio|http|sse)\s");
    let gemini = crate::invariants::regex(r"^[○✓✗●◐⊘✔✘•-]\s+([^:\s]+):");
    let opencode = crate::invariants::regex(r"(?:^|\s)[✓✗]\s+(\S+)");
    for raw in plain(output).lines() {
        let line = raw.trim();
        let found = match agent {
            // Its header (`NAME TYPE STATUS ...`) never matches: no transport stands second there.
            "antigravity" => table.captures(line).map(|c| c[1].to_owned()),
            "gemini" => gemini.captures(line).map(|c| c[1].to_owned()),
            "opencode" => opencode.captures(line.trim_start_matches(['│', '●', ' ', '\t'])).map(|c| c[1].to_owned()),
            _ => None,
        };
        if let Some(name) = found
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    names
}

/// Whether the agent already has one by this name: Some(true), Some(false), or None when it could not be told.
pub fn agent_has(agent: &str, name: &str, launch: &Launch, runner: &dyn Runner) -> Option<bool> {
    if matches!(agent, "claude" | "codex" | "copilot") {
        let answer = run_with(runner, launch, &words(&["mcp", "get", name]), LIST_TIMEOUT).ok()?;
        if answer.timed_out {
            return None;
        }
        return match answer.exit {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        };
    }
    let answer = run_with(runner, launch, &words(&["mcp", "list"]), LIST_TIMEOUT).ok()?;
    // A listing that failed names nothing, which read as "has none", and the agent's own `mcp add` then replaced the
    // person's entry (Electron 0.753, CN1). A failed listing cannot tell.
    if answer.timed_out || answer.exit != Some(0) {
        return None;
    }
    Some(listed_names(agent, &format!("{}\n{}", answer.stdout, answer.stderr)).iter().any(|n| n == name))
}

fn distinct(agents: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for agent in agents {
        if !out.contains(agent) {
            out.push(agent.clone());
        }
    }
    out
}

fn not_installed(agent: &str) -> String {
    format!("{} is not installed here.", crate::inspector::runtime_display_name(agent))
}

/// For each agent chosen, at once: whether it has one of that name, asked of the agent itself, and only if it has
/// none, its own `mcp add`. Err with why, when what was typed is not a connector yet (then no agent is run).
pub fn add(
    name: &str,
    kind: Kind,
    value: &str,
    agents: &[String],
    launch_of: &(dyn Fn(&str) -> Option<Launch> + Sync),
    runner: &dyn Runner,
) -> Result<Vec<AgentResult>, String> {
    let connector = connector_from(name, kind, value)?;
    let one = |agent: &str| -> AgentResult {
        let Some(launch) = launch_of(agent) else { return result(agent, Outcome::Failed, Some(not_installed(agent))) };
        let Some(args) = add_args(agent, &connector) else { return result(agent, Outcome::Failed, Some(declined_why(agent).into())) };
        match agent_has(agent, &connector.name, &launch, runner) {
            Some(true) => return result(agent, Outcome::Had, None),
            None => {
                let said = format!("It did not say whether it already has one named {}, so nothing was changed.", connector.name);
                return result(agent, Outcome::Failed, Some(said));
            }
            Some(false) => {}
        }
        match run_with(runner, &launch, &args, CHANGE_TIMEOUT) {
            Ok(answer) if answer.exit == Some(0) && !answer.timed_out => result(agent, Outcome::Added, None),
            Ok(answer) if answer.timed_out => {
                result(agent, Outcome::Failed, Some("It did not answer in time; check its connectors before trying again.".into()))
            }
            Ok(answer) => result(agent, Outcome::Failed, last_words(&answer.stdout, &answer.stderr)),
            Err(()) => result(agent, Outcome::Failed, Some("It could not be started.".into())),
        }
    };
    Ok(each(&distinct(agents), &one))
}

/// Undo, right after an add: each agent's own `mcp remove`, for the agents just given it. OpenCode has no remove
/// command (measured), so it keeps its copy and the card says where it lives.
pub fn remove(
    name: &str,
    agents: &[String],
    launch_of: &(dyn Fn(&str) -> Option<Launch> + Sync),
    runner: &dyn Runner,
) -> Vec<AgentResult> {
    let one = |agent: &str| -> AgentResult {
        let Some(args) = remove_args(agent, name) else {
            let said = "OpenCode has no command to remove one: delete it from the \"mcp\" section of its opencode.json.";
            return result(agent, Outcome::Kept, Some(said.into()));
        };
        let Some(launch) = launch_of(agent) else { return result(agent, Outcome::Failed, Some(not_installed(agent))) };
        match run_with(runner, &launch, &args, CHANGE_TIMEOUT) {
            Ok(answer) if answer.exit == Some(0) && !answer.timed_out => result(agent, Outcome::Removed, None),
            Ok(answer) => result(agent, Outcome::Failed, last_words(&answer.stdout, &answer.stderr)),
            Err(()) => result(agent, Outcome::Failed, Some("It could not be started.".into())),
        }
    };
    each(&distinct(agents), &one)
}

/// Each agent at once, the answers in the order the agents were given.
fn each(agents: &[String], one: &(dyn Fn(&str) -> AgentResult + Sync)) -> Vec<AgentResult> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = agents.iter().map(|agent| scope.spawn(move || one(agent))).collect();
        handles
            .into_iter()
            .zip(agents)
            .map(|(handle, agent)| handle.join().unwrap_or_else(|_| result(agent, Outcome::Failed, Some("It could not be started.".into()))))
            .collect()
    })
}

/// The command each agent is started by, as discovery finds it.
fn command_name(agent: &str) -> Option<&'static str> {
    Some(match agent {
        "claude" => "claude",
        "codex" => "codex",
        "gemini" => "gemini",
        "copilot" => "copilot",
        "opencode" => "opencode",
        "antigravity" => "agy",
        _ => return None,
    })
}

/// How this machine starts an agent's CLI, or None when it is not installed here.
pub fn real_launch(agent: &str) -> Option<Launch> {
    let path = crate::runtime::discovery::find_named(command_name(agent)?).ok()?;
    Some(Launch { executable: path, prefix_args: Vec::new() })
}

/// The agents that can take a connector and are installed here, in the order they are offered.
pub fn installed() -> Vec<String> {
    AGENTS.iter().filter(|agent| real_launch(agent).is_some()).map(|agent| (*agent).to_owned()).collect()
}

/// The real CLI, bounded by the time limit.
pub struct RealRunner;

impl Runner for RealRunner {
    fn run(&self, executable: &std::path::Path, args: &[String], limit: Duration) -> Result<Ran, ()> {
        let answer = crate::github_account::run_bounded(std::process::Command::new(executable).args(args), limit);
        match answer.exit {
            crate::github_account::Exit::Missing => Err(()),
            crate::github_account::Exit::Code(_) if answer.stderr == "It took too long." && answer.stdout.is_empty() => {
                Ok(Ran { exit: None, timed_out: true, ..Ran::default() })
            }
            crate::github_account::Exit::Code(code) => Ok(Ran { exit: Some(code), stdout: answer.stdout, stderr: answer.stderr, timed_out: false }),
        }
    }
}

#[cfg(test)]
#[path = "connector_add_tests.rs"]
mod connector_add_tests;
