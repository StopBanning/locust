//! Electron's `a-connector-goes-to-every-agent.test.ts` (16 cases): the words each agent is given, what the person
//! typed, whether an agent already has one, adding to every agent chosen, Undo, and an agent's last words.
use super::*;
use std::path::Path;
use std::sync::Mutex;

fn rea() -> Connector {
    Connector { name: "rea".into(), runs_as: RunsAs::Command { command: "npx".into(), args: vec!["-y".into(), "rea-agents@6.3.0".into(), "mcp".into()] } }
}

fn web() -> Connector {
    Connector { name: "docs".into(), runs_as: RunsAs::Url("https://example.com/mcp".into()) }
}

fn w(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn some(list: &[&str]) -> Option<Vec<String>> {
    Some(w(list))
}

// case: a command, after `--` wherever the agent takes one, so the server keeps its own flags
#[test]
fn a_command_after_the_double_dash_wherever_the_agent_takes_one() {
    assert_eq!(add_args("claude", &rea()), some(&["mcp", "add", "--scope", "user", "rea", "--", "npx", "-y", "rea-agents@6.3.0", "mcp"]));
    for agent in ["codex", "copilot", "opencode", "antigravity"] {
        assert_eq!(add_args(agent, &rea()), some(&["mcp", "add", "rea", "--", "npx", "-y", "rea-agents@6.3.0", "mcp"]), "{agent}");
    }
    // Gemini CLI printed its help at `--` (measured): its words follow bare.
    assert_eq!(add_args("gemini", &rea()), some(&["mcp", "add", "--scope", "user", "rea", "npx", "-y", "rea-agents@6.3.0", "mcp"]));
}

// case: a web address, as each one spells it
#[test]
fn a_web_address_as_each_one_spells_it() {
    assert_eq!(add_args("claude", &web()), some(&["mcp", "add", "--scope", "user", "--transport", "http", "docs", "https://example.com/mcp"]));
    assert_eq!(add_args("codex", &web()), some(&["mcp", "add", "docs", "--url", "https://example.com/mcp"]));
    assert_eq!(add_args("gemini", &web()), some(&["mcp", "add", "--scope", "user", "--transport", "http", "docs", "https://example.com/mcp"]));
    assert_eq!(add_args("copilot", &web()), some(&["mcp", "add", "--transport", "http", "docs", "https://example.com/mcp"]));
    assert_eq!(add_args("opencode", &web()), some(&["mcp", "add", "docs", "--url", "https://example.com/mcp"]));
    // Antigravity's flags come before the name.
    assert_eq!(add_args("antigravity", &web()), some(&["mcp", "add", "--type", "http", "docs", "https://example.com/mcp"]));
}

// case: Gemini CLI is not handed a command whose words spell one of its own flags
#[test]
fn gemini_cli_is_not_handed_a_command_whose_words_spell_one_of_its_own_flags() {
    let command = |args: &[&str]| Connector { name: "x".into(), runs_as: RunsAs::Command { command: "node".into(), args: w(args) } };
    assert_eq!(add_args("gemini", &command(&["server.js", "-e", "A=1"])), None);
    assert_eq!(add_args("gemini", &command(&["server.js", "--timeout=5"])), None);
    // The others take it whole, after `--`.
    assert_eq!(add_args("codex", &command(&["server.js", "-e", "A=1"])), some(&["mcp", "add", "x", "--", "node", "server.js", "-e", "A=1"]));
}

// case: takes it out with each one’s own remove, and OpenCode, which has none, is not pretended to
#[test]
fn takes_it_out_with_each_ones_own_remove_and_opencode_which_has_none_is_not_pretended_to() {
    assert_eq!(remove_args("claude", "rea"), some(&["mcp", "remove", "--scope", "user", "rea"]));
    assert_eq!(remove_args("gemini", "rea"), some(&["mcp", "remove", "--scope", "user", "rea"]));
    assert_eq!(remove_args("codex", "rea"), some(&["mcp", "remove", "rea"]));
    assert_eq!(remove_args("copilot", "rea"), some(&["mcp", "remove", "rea"]));
    assert_eq!(remove_args("antigravity", "rea"), some(&["mcp", "remove", "rea"]));
    assert_eq!(remove_args("opencode", "rea"), None);
}

// case: a command line splits on spaces, keeps quoted words whole, and interprets nothing else
#[test]
fn a_command_line_splits_on_spaces_keeps_quoted_words_whole_and_interprets_nothing_else() {
    assert_eq!(split_command_line("npx -y rea-agents@6.3.0 mcp"), some(&["npx", "-y", "rea-agents@6.3.0", "mcp"]));
    assert_eq!(split_command_line("  node \"C:/My Tools/server.js\"  --root 'a b' "), some(&["node", "C:/My Tools/server.js", "--root", "a b"]));
    assert_eq!(split_command_line("echo \"\" $HOME"), some(&["echo", "", "$HOME"]));
    assert_eq!(split_command_line("node \"unclosed"), None);
    assert_eq!(split_command_line("   "), Some(Vec::new()));
}

// case: a name every agent takes, or why not
#[test]
fn a_name_every_agent_takes_or_why_not() {
    assert_eq!(name_problem("rea"), None);
    assert_eq!(name_problem("my_server-2"), None);
    assert_eq!(name_problem(""), Some("Give it a name."));
    assert!(name_problem("two words").unwrap().contains("letters, digits, - and _"));
    assert!(name_problem("-flag").unwrap().contains("starting with a letter or digit"));
    assert!(name_problem(&"a".repeat(65)).unwrap().contains("up to 64"));
    assert_eq!(name_problem(&"a".repeat(64)), None);
}

// case: becomes a connector, or says what is missing
#[test]
fn becomes_a_connector_or_says_what_is_missing() {
    assert_eq!(connector_from(" rea ", Kind::Command, "npx -y rea-agents@6.3.0 mcp"), Ok(rea()));
    assert_eq!(connector_from("docs", Kind::Url, "https://example.com/mcp"), Ok(web()));
    assert_eq!(connector_from("rea", Kind::Command, ""), Err("Write the command that starts it, such as npx -y rea-agents@6.3.0 mcp.".into()));
    assert_eq!(connector_from("rea", Kind::Command, "node \"x"), Err("A quote is left open in the command.".into()));
    // A command that is an empty quoted word is no command.
    assert_eq!(connector_from("rea", Kind::Command, "\"\" server.js"), Err("Write the command that starts it, such as npx -y rea-agents@6.3.0 mcp.".into()));
    assert!(connector_from("docs", Kind::Url, "example.com/mcp").is_err());
    assert_eq!(connector_from("docs", Kind::Url, "file:///C:/server"), Err("A web address here starts with https:// or http://.".into()));
}

/* The listings as printed in the throwaway home, 2026-10-09 (OpenCode's with its colours). */
const GEMINI_LIST: &str = "Warning: MCP servers are configured but disabled because this folder is untrusted.\n\
User-level servers are also suppressed in untrusted folders to prevent accidental side-effects.\n\n\
Configured MCP servers:\n\n\
\u{25cb} probe-web: https://example.com/mcp (http) - Disabled\n";
const ANTIGRAVITY_LIST: &str = "NAME        TYPE   STATUS   COMMAND/URL\n\
probe-dash  stdio  enabled  npx -y rea-agents@6.3.0 mcp\n\
probe-web   http   enabled  https://example.com/mcp\n";
const OPENCODE_LIST: &str = "\u{1b}[0m\r\n\
\u{1b}[90m\u{250c}\u{1b}[39m  MCP Servers\n\
\u{1b}[90m\u{2502}\u{1b}[39m\n\
\u{1b}[34m\u{25cf}\u{1b}[39m  \u{2717} probe-web \u{1b}[90mfailed\n\
\u{1b}[90m\u{2502}\u{1b}[39m      SSE error: Non-200 status code (404)\n\
\u{1b}[90m\u{2502}\u{1b}[39m      \u{1b}[90mhttps://example.com/mcp\n\
\u{1b}[90m\u{2502}\u{1b}[39m\n\
\u{1b}[34m\u{25cf}\u{1b}[39m  \u{2713} probe-local \u{1b}[90mconnected\n\
\u{1b}[90m\u{2514}\u{1b}[39m  2 server(s)\n";

// case: reads the names from the listings of the three with no `get`
#[test]
fn reads_the_names_from_the_listings_of_the_three_with_no_get() {
    assert_eq!(listed_names("gemini", GEMINI_LIST), w(&["probe-web"]));
    assert_eq!(listed_names("antigravity", ANTIGRAVITY_LIST), w(&["probe-dash", "probe-web"]));
    assert_eq!(listed_names("opencode", OPENCODE_LIST), w(&["probe-web", "probe-local"]));
    // A URL or an error line is never a name.
    assert!(!listed_names("opencode", OPENCODE_LIST).iter().any(|n| n == "https://example.com/mcp"));
}

/// A runner that answers from a closure and remembers every command it was given.
struct Fake<F: Fn(&Path, &[String]) -> Result<Ran, ()> + Sync> {
    calls: Mutex<Vec<(PathBuf, Vec<String>)>>,
    answer: F,
}

impl<F: Fn(&Path, &[String]) -> Result<Ran, ()> + Sync> Runner for Fake<F> {
    fn run(&self, executable: &Path, args: &[String], _limit: Duration) -> Result<Ran, ()> {
        self.calls.lock().unwrap().push((executable.to_path_buf(), args.to_vec()));
        (self.answer)(executable, args)
    }
}

impl<F: Fn(&Path, &[String]) -> Result<Ran, ()> + Sync> Fake<F> {
    fn calls(&self) -> Vec<(PathBuf, Vec<String>)> {
        self.calls.lock().unwrap().clone()
    }
}

fn fake<F: Fn(&Path, &[String]) -> Result<Ran, ()> + Sync>(answer: F) -> Fake<F> {
    Fake { calls: Mutex::new(Vec::new()), answer }
}

fn exits(code: Option<i64>, out: &str, err: &str) -> Result<Ran, ()> {
    Ok(Ran { exit: code, stdout: out.into(), stderr: err.into(), timed_out: false })
}

fn launch(agent: &str) -> Option<Launch> {
    let (exe, prefix): (&str, &[&str]) = match agent {
        "claude" => ("C:/bin/claude.exe", &[]),
        "codex" => ("C:/bin/codex.exe", &[]),
        "gemini" => ("C:/bin/node.exe", &["C:/bin/gemini.js"]),
        "copilot" => ("C:/bin/copilot.exe", &[]),
        "opencode" => ("C:/bin/opencode.exe", &[]),
        "antigravity" => ("C:/bin/agy.exe", &[]),
        _ => return None,
    };
    Some(Launch { executable: PathBuf::from(exe), prefix_args: w(prefix) })
}

/// Which agent a command went to, by the launch it was given.
fn who(exe: &Path, args: &[String]) -> String {
    AGENTS
        .iter()
        .find(|agent| {
            let l = launch(agent).unwrap();
            l.executable == exe && l.prefix_args.iter().enumerate().all(|(i, word)| args.get(i) == Some(word))
        })
        .map(|agent| (*agent).to_owned())
        .unwrap_or_else(|| "?".into())
}

fn agents(list: &[&str]) -> Vec<String> {
    w(list)
}

// case: asks the three with a `get` by its exit code, and an answer it cannot read is not a no
#[test]
fn asks_the_three_with_a_get_by_its_exit_code_and_an_answer_it_cannot_read_is_not_a_no() {
    let claude = launch("claude").unwrap();
    let by = |code: Option<i64>, timed_out: bool| fake(move |_, _| Ok(Ran { exit: code, timed_out, ..Ran::default() }));
    assert_eq!(agent_has("claude", "rea", &claude, &by(Some(0), false)), Some(true));
    assert_eq!(agent_has("claude", "rea", &claude, &by(Some(1), false)), Some(false));
    assert_eq!(agent_has("claude", "rea", &claude, &by(Some(2), false)), None);
    assert_eq!(agent_has("claude", "rea", &claude, &by(None, true)), None);
    let asked = by(Some(1), false);
    let node = Launch { executable: PathBuf::from("C:/node.exe"), prefix_args: w(&["C:/codex.js"]) };
    agent_has("codex", "rea", &node, &asked);
    assert_eq!(asked.calls()[0].1, w(&["C:/codex.js", "mcp", "get", "rea"]));
}

// case: a listing that failed cannot tell, and nothing is added over it
#[test]
fn a_listing_that_failed_cannot_tell_and_nothing_is_added_over_it() {
    for agent in ["gemini", "opencode", "antigravity"] {
        let failed = fake(|_, args: &[String]| {
            if args.iter().any(|a| a == "list") {
                exits(Some(1), "", "Error: could not read settings.json")
            } else {
                exits(Some(0), "Added", "")
            }
        });
        assert_eq!(agent_has(agent, "rea", &launch(agent).unwrap(), &failed), None);
        let answer = add("rea", Kind::Url, "https://example.com/mcp", &agents(&[agent]), &launch, &failed).unwrap();
        assert_eq!(answer[0].outcome, Outcome::Failed, "{agent}");
        assert!(!failed.calls().iter().any(|(_, args)| args.iter().any(|a| a == "add")), "{agent}");
    }
}

// case: adds where there is none, keeps one already there, and writes nothing where it cannot tell
#[test]
fn adds_where_there_is_none_keeps_one_already_there_and_writes_nothing_where_it_cannot_tell() {
    let runner = fake(|exe: &Path, args: &[String]| {
        let agent = who(exe, args);
        let words = args.join(" ");
        if words.contains("mcp get") {
            return exits(Some(match agent.as_str() { "codex" => 0, "copilot" => 2, _ => 1 }), "", "");
        }
        if words.contains("mcp list") {
            return if agent == "antigravity" {
                exits(Some(0), &ANTIGRAVITY_LIST.replace("probe-dash", "rea"), "")
            } else {
                Ok(Ran { exit: None, timed_out: true, ..Ran::default() })
            };
        }
        exits(Some(0), "Added stdio MCP server rea", "")
    });
    let answer = add("rea", Kind::Command, "npx -y rea-agents@6.3.0 mcp", &agents(&["claude", "codex", "copilot", "opencode", "antigravity"]), &launch, &runner).unwrap();
    let outcomes: Vec<(String, Outcome)> = answer.iter().map(|one| (one.agent.clone(), one.outcome)).collect();
    assert_eq!(
        outcomes,
        vec![
            ("claude".into(), Outcome::Added),
            ("codex".into(), Outcome::Had),
            ("copilot".into(), Outcome::Failed),
            ("opencode".into(), Outcome::Failed),
            ("antigravity".into(), Outcome::Had),
        ]
    );
    let opencode = answer.iter().find(|one| one.agent == "opencode").unwrap();
    assert_eq!(opencode.said.as_deref(), Some("It did not say whether it already has one named rea, so nothing was changed."));
    // The one add that ran is Claude Code's, in exactly its words; nobody else was written to.
    let adds: Vec<_> = runner.calls().into_iter().filter(|(_, args)| args.iter().any(|a| a == "add")).collect();
    assert_eq!(adds.iter().map(|(exe, args)| who(exe, args)).collect::<Vec<_>>(), w(&["claude"]));
    assert_eq!(adds[0].1, w(&["mcp", "add", "--scope", "user", "rea", "--", "npx", "-y", "rea-agents@6.3.0", "mcp"]));
}

// case: an agent that is not installed, or cannot take it as written, is said and never run
#[test]
fn an_agent_that_is_not_installed_or_cannot_take_it_as_written_is_said_and_never_run() {
    let runner = fake(|_, _| exits(Some(1), "", ""));
    let launch_of = |agent: &str| if agent == "copilot" { None } else { launch(agent) };
    let answer = add("x", Kind::Command, "node server.js -e A=1", &agents(&["gemini", "copilot"]), &launch_of, &runner).unwrap();
    assert_eq!(
        answer,
        vec![
            AgentResult {
                agent: "gemini".into(),
                outcome: Outcome::Failed,
                said: Some("Gemini CLI would read one of the command\u{2019}s own flags as its own. Add it in Gemini CLI itself.".into()),
            },
            AgentResult { agent: "copilot".into(), outcome: Outcome::Failed, said: Some("Copilot CLI is not installed here.".into()) },
        ]
    );
    assert!(runner.calls().is_empty());
}

// case: a refusal comes back in the agent’s own last words
#[test]
fn a_refusal_comes_back_in_the_agents_own_last_words() {
    let runner = fake(|_, args: &[String]| {
        if args.iter().any(|a| a == "get") {
            exits(Some(1), "", "")
        } else {
            exits(Some(1), "", "\u{1b}[31mError: MCP server rea already exists in user config\u{1b}[39m\n")
        }
    });
    let answer = add("rea", Kind::Command, "npx -y rea-agents@6.3.0 mcp", &agents(&["claude"]), &launch, &runner).unwrap();
    assert_eq!(
        answer,
        vec![AgentResult { agent: "claude".into(), outcome: Outcome::Failed, said: Some("Error: MCP server rea already exists in user config".into()) }]
    );
}

// case: something that is not a connector yet goes to no agent at all
#[test]
fn something_that_is_not_a_connector_yet_goes_to_no_agent_at_all() {
    let runner = fake(|_, _| exits(Some(0), "", ""));
    assert_eq!(
        add("bad name", Kind::Command, "npx x", &agents(&["claude"]), &launch, &runner),
        Err("A name is letters, digits, - and _, up to 64, starting with a letter or digit.".into())
    );
    assert!(runner.calls().is_empty());
}

// case: runs each agent’s own remove, and says where OpenCode keeps its copy
#[test]
fn runs_each_agents_own_remove_and_says_where_opencode_keeps_its_copy() {
    let runner = fake(|_, _| exits(Some(0), "", ""));
    let results = remove("rea", &agents(&["claude", "opencode"]), &launch, &runner);
    assert_eq!(
        results,
        vec![
            AgentResult { agent: "claude".into(), outcome: Outcome::Removed, said: None },
            AgentResult {
                agent: "opencode".into(),
                outcome: Outcome::Kept,
                said: Some("OpenCode has no command to remove one: delete it from the \"mcp\" section of its opencode.json.".into()),
            },
        ]
    );
    assert_eq!(runner.calls().into_iter().map(|(_, args)| args).collect::<Vec<_>>(), vec![w(&["mcp", "remove", "--scope", "user", "rea"])]);
}

// case: the last line that says something, without colours, frames or Codex’s PATH warning
#[test]
fn the_last_line_that_says_something_without_colours_frames_or_codexs_path_warning() {
    assert_eq!(
        last_words("", "\u{1b}[33mWARNING: proceeding, even though we could not update PATH: Access is denied.\u{1b}[39m\nError: invalid server name\n").as_deref(),
        Some("Error: invalid server name")
    );
    assert_eq!(last_words("│  ✗ could not write config\n│\n", "").as_deref(), Some("could not write config"));
    // Codex's PATH warning printed last is still not its last words.
    assert_eq!(last_words("WARNING: proceeding, even though we could not update PATH", "Error: x").as_deref(), Some("Error: x"));
    assert_eq!(last_words("", ""), None);
    assert_eq!(last_words(&"x".repeat(400), "").unwrap().chars().count(), 220);
}
