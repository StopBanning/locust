//! runtime-setup.test.ts: every case by name, against a fake disk. Expected values are Electron's own, written out.

use super::*;
use std::collections::HashMap;

const HOME: &str = "/Users/colin";
const WS: &str = "/Users/colin/shop";

fn normal(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// A fake disk: paths that exist map to text; "<<EACCES>>" cannot be read; anything else is absent.
fn disk(files: &[(String, String)]) -> impl Fn(&Path) -> std::io::Result<String> + use<> {
    let held: HashMap<String, String> = files.iter().map(|(p, t)| (normal(Path::new(p)), t.clone())).collect();
    move |path: &Path| match held.get(&normal(path)) {
        None => Err(ErrorKind::NotFound.into()),
        Some(t) if t == "<<EACCES>>" => Err(ErrorKind::PermissionDenied.into()),
        Some(t) => Ok(t.clone()),
    }
}

fn folders(tree: &[(String, &[&str])]) -> impl Fn(&Path) -> std::io::Result<Vec<String>> + use<> {
    let held: HashMap<String, Vec<String>> = tree.iter().map(|(p, e)| (normal(Path::new(p)), e.iter().map(|s| s.to_string()).collect())).collect();
    move |dir: &Path| held.get(&normal(dir)).cloned().ok_or_else(|| ErrorKind::NotFound.into())
}

fn none(_: &Path) -> std::io::Result<Vec<String>> {
    Err(ErrorKind::NotFound.into())
}

fn at(rel: &str) -> String {
    format!("{HOME}/{rel}")
}
fn in_ws(rel: &str) -> String {
    format!("{WS}/{rel}")
}

fn read(workspace: Option<&str>, read: ReadFile, list: ListFolder) -> BTreeMap<String, RuntimeSetup> {
    read_runtime_setup(&SetupOptions { workspace: workspace.map(PathBuf::from), home: PathBuf::from(HOME), read, list })
}

fn names_of(list: &[&str]) -> Vec<String> {
    list.iter().map(|x| x.to_string()).collect()
}

// what each runtime has set up for itself

#[test]
fn reads_claude_code_hooks_from_user_and_project_settings_and_mcp_servers_from_three_places_names_only() {
    let files = disk(&[
        (at(".claude/settings.json"), r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"secret.sh"}]}],"PreToolUse":[{},{}]}}"#.into()),
        (in_ws(".claude/settings.json"), r#"{"hooks":{"PostToolUse":[{}]}}"#.into()),
        (
            at(".claude.json"),
            format!(r#"{{"mcpServers":{{"github":{{"command":"npx","args":["--token","SECRET"]}}}},"projects":{{"{WS}":{{"mcpServers":{{"robinhood":{{}}}}}},"C:/elsewhere":{{"mcpServers":{{"other":{{}}}}}}}}}}"#),
        ),
        (in_ws(".mcp.json"), r#"{"mcpServers":{"playwright":{}}}"#.into()),
    ]);
    let setup = read(Some(WS), &files, &none);
    let claude = &setup["claude"];
    assert_eq!(claude.hooks, names_of(&["Stop (1)", "PreToolUse (2)", "PostToolUse (1)"]));
    assert_eq!(claude.mcp_servers, names_of(&["github", "robinhood", "playwright"]));
    assert_eq!(claude.unreadable, Vec::<String>::new());
    let all = format!("{setup:?}");
    assert!(!all.contains("SECRET") && !all.contains("secret.sh"), "{all}");
}

#[test]
fn reads_codex_mcp_servers_from_toml_table_headers_and_its_notify_hook_without_a_toml_parser() {
    let toml = [
        "model = \"gpt-5\"",
        "notify = [\"node\", \"C:\\\\scripts\\\\notify.mjs\"]",
        "",
        "[mcp_servers.node_repl]",
        "command = \"node\"",
        "",
        "[mcp_servers.node_repl.env]",
        "TOKEN = \"SECRET\"",
        "",
        "[mcp_servers.robinhood]",
        "url = \"https://example\"",
    ]
    .join("\n");
    let files = disk(&[(at(".codex/config.toml"), toml)]);
    let setup = read(None, &files, &none);
    assert_eq!(setup["codex"].mcp_servers, names_of(&["node_repl", "robinhood"]));
    assert_eq!(setup["codex"].hooks, names_of(&["notify (1)"]));
    assert!(!format!("{setup:?}").contains("SECRET"));
}

#[test]
fn reads_cursor_opencode_and_copilot_from_their_own_files_in_the_folder_and_at_home() {
    let files = disk(&[
        (at(".cursor/mcp.json"), r#"{"mcpServers":{"figma":{}}}"#.into()),
        (in_ws(".cursor/hooks.json"), r#"{"hooks":{"afterFileEdit":[{"command":"fmt"}]}}"#.into()),
        (in_ws("opencode.json"), r#"{"mcp":{"context7":{}}}"#.into()),
        (at(".copilot/mcp-config.json"), r#"{"mcpServers":{"github":{}}}"#.into()),
    ]);
    let setup = read(Some(WS), &files, &none);
    assert_eq!(setup["cursor"].mcp_servers, names_of(&["figma"]));
    assert_eq!(setup["cursor"].hooks, names_of(&["afterFileEdit (1)"]));
    assert_eq!(setup["opencode"].mcp_servers, names_of(&["context7"]));
    assert_eq!(setup["copilot"].mcp_servers, names_of(&["github"]));
}

#[test]
fn an_absent_file_is_nothing_a_file_that_cannot_be_read_or_parsed_is_named_never_guessed() {
    let files = disk(&[(at(".claude/settings.json"), "{not json".into()), (at(".cursor/mcp.json"), "<<EACCES>>".into())]);
    let setup = read(Some(WS), &files, &none);
    assert_eq!(setup["claude"].hooks, Vec::<String>::new());
    let slashes = |v: &[String]| v.iter().map(|p| p.replace('\\', "/")).collect::<Vec<_>>();
    assert_eq!(slashes(&setup["claude"].unreadable), vec![at(".claude/settings.json")]);
    assert_eq!(slashes(&setup["cursor"].unreadable), vec![at(".cursor/mcp.json")]);
    assert_eq!(setup["codex"], RuntimeSetup { mcp_servers: vec![], hooks: vec![], skills: vec![], agents: vec![], sources: vec![], unreadable: vec![] });
}

#[test]
fn names_the_mcp_servers_in_the_persons_own_copilot_settings_and_none_when_there_are_none_to_read() {
    let home = Path::new(HOME);
    let files = disk(&[(at(".copilot/mcp-config.json"), r#"{"mcpServers":{"notes":{"command":"n"},"robinhood-trading":{"url":"https://x"}}}"#.into())]);
    assert_eq!(copilot_mcp_server_names(home, &files), names_of(&["notes", "robinhood-trading"]));
    assert_eq!(copilot_mcp_server_names(home, &disk(&[])), Vec::<String>::new());
    assert_eq!(copilot_mcp_server_names(home, &disk(&[(at(".copilot/mcp-config.json"), "{not json".into())])), Vec::<String>::new());
}

// the skills each runtime finds

fn tree() -> impl Fn(&Path) -> std::io::Result<Vec<String>> + use<> {
    folders(&[
        (at(".claude/skills"), &["reskin/", "new-lead/"]),
        (at(".agents/skills"), &["shared-one/"]),
        (at(".codex/skills"), &[".system/", "codex-own/"]),
        (in_ws(".agents/skills"), &["project-agents/"]),
        (in_ws(".opencode/skills"), &["opencode-own/"]),
        (at(".copilot/skills"), &["copilot-own/"]),
        (in_ws(".github/skills"), &["github-project/"]),
        (at(".cursor/skills"), &["cursor-own/"]),
        (at(".cursor/skills-cursor"), &["canvas/"]),
    ])
}

#[test]
fn lists_codex_skills_from_codex_skills_and_the_agents_folders_never_the_claude_folder_or_its_built_ins() {
    let setup = read(Some(WS), &disk(&[]), &tree());
    assert_eq!(setup["codex"].skills, names_of(&["codex-own", "shared-one", "project-agents"]));
}

#[test]
fn lists_opencode_skills_from_the_claude_and_agents_folders_and_its_own() {
    let setup = read(Some(WS), &disk(&[]), &tree());
    assert_eq!(setup["opencode"].skills, names_of(&["reskin", "new-lead", "shared-one", "project-agents", "opencode-own"]));
    // Claude Code's own list is unchanged: its two folders.
    assert_eq!(setup["claude"].skills, names_of(&["reskin", "new-lead"]));
}

#[test]
fn lists_cursor_skills_from_its_own_two_roots_never_its_built_ins_or_the_third_party_folders() {
    let setup = read(Some(WS), &disk(&[]), &tree());
    assert_eq!(setup["cursor"].skills, names_of(&["cursor-own", "shared-one", "project-agents"]));
}

#[test]
fn lists_copilot_skills_from_where_copilot_skill_help_says_it_looks_the_claude_folder_only_in_the_project() {
    let setup = read(Some(WS), &disk(&[]), &tree());
    assert_eq!(setup["copilot"].skills, names_of(&["copilot-own", "shared-one", "github-project", "project-agents"]));
}

// Screens.tsx RuntimeSetupLine, in its own words.

#[test]
fn the_agents_screen_says_each_kind_of_setup_and_counts_what_it_could_not_read() {
    let setup = RuntimeSetup {
        mcp_servers: names_of(&["github", "figma"]),
        hooks: names_of(&["Stop (1)"]),
        skills: names_of(&["a", "b", "c", "d", "e", "f", "g"]),
        agents: names_of(&["seo-scout"]),
        sources: names_of(&["/h/.claude.json"]),
        unreadable: names_of(&["/h/.cursor/mcp.json", "/h/x.json"]),
    };
    assert_eq!(
        setup_line(&setup),
        "Connectors (MCP): github, figma · Hooks: Stop (1) · Skills: a, b, c, d, e +2 · Agents: seo-scout · 2 config files could not be read"
    );
    assert_eq!(setup_title(&setup), "read: /h/.claude.json\nunreadable: /h/.cursor/mcp.json\nunreadable: /h/x.json");
    assert_eq!(setup_line(&RuntimeSetup::default()), "No connectors, hooks, skills or agents set up");
    assert_eq!(setup_title(&RuntimeSetup::default()), "No configuration files found");
    assert_eq!(setup_line(&RuntimeSetup { unreadable: names_of(&["/x"]), ..Default::default() }), "1 config file could not be read");
}

/// Beside Electron's case: Screens.tsx `fewNames` keeps a list of exactly five whole and counts from the sixth, so
/// six skills read "a, b, c, d, e +1" and five read all five, through the line the agents screen draws.
#[test]
fn the_agents_screen_counts_the_sixth_name_and_keeps_exactly_five_whole() {
    let six = RuntimeSetup { skills: names_of(&["a", "b", "c", "d", "e", "f"]), ..Default::default() };
    assert_eq!(setup_line(&six), "Skills: a, b, c, d, e +1");
    let five = RuntimeSetup { agents: names_of(&["a", "b", "c", "d", "e"]), ..Default::default() };
    assert_eq!(setup_line(&five), "Agents: a, b, c, d, e");
}

/// Beside Electron's case: the project's Claude folder is one Copilot reads, as `copilot skill --help` says.
#[test]
fn copilot_reads_the_claude_skills_folder_in_the_project_but_not_at_home() {
    let list = folders(&[(at(".claude/skills"), &["home-claude/"]), (in_ws(".claude/skills"), &["project-claude/"])]);
    let setup = read(Some(WS), &disk(&[]), &list);
    assert_eq!(setup["copilot"].skills, names_of(&["project-claude"]));
}
