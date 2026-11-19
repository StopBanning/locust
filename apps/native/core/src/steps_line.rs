//! A steps line in words (missionView.ts `stepsLine`): what a stretch of steps did, counted by kind ("ran 3 commands,
//! read 2 files"), a thought's headline in place of all of it (0.492, 0.610), and the amber outcomes behind it.
//!
//! Electron's `ActivityDetail` is read off a ledger item: a thought is `reasoning`; an edit (`activity_summary::is_edit`)
//! is `edit`; a shell call is `shell`; a helper is `helper`; anything else is a tool. The detail's `name` is the call's
//! arguments (else its summary), its `tool` the item's name. The ledger keeps no separate title: it writes the title into
//! `summary` when the runtime sent one and the command otherwise (ledger.rs, "Prefer title"), so a shell call has a title
//! only when its summary is not its command.
use crate::ledger::{Item, Status};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub key: &'static str,
    pub text: String,
    pub amber: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepsLine {
    pub segments: Vec<Segment>,
    /// The sentence in shorter forms, each shorter than the last, for a row too narrow for it (0.604).
    pub shorter: Vec<String>,
    /// The whole sentence with every name whole, for the hover.
    pub title: String,
}

impl StepsLine {
    /// The segments as one line, joined as the window draws them.
    pub fn text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" · ")
    }
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("a fixed pattern"))
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() > max { format!("{}\u{2026}", text.chars().take(max - 1).collect::<String>()) } else { text.to_owned() }
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn pluralize(count: usize, singular: &str) -> String {
    format!("{count} {singular}{}", if count == 1 { "" } else { "s" })
}

fn times(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// "Run the tests" -> "run the tests", keeping a word that is capitalised on its own (`README`).
fn lower_first(text: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    if re(&R, r"^[A-Z][a-z]").is_match(text) {
        let mut chars = text.chars();
        chars.next().map(|c| c.to_lowercase().chain(chars).collect()).unwrap_or_default()
    } else {
        text.to_owned()
    }
}

fn last_part(path: &str) -> Option<String> {
    path.split(['/', '\\']).rfind(|p| !p.is_empty()).map(str::to_owned)
}

fn first_line(command: &str) -> String {
    crate::shell_words::shell_command_text(command).lines().next().unwrap_or("").trim().to_owned()
}

/// The word that says what a command is: its first, past any `cd <folder> &&` in front of it (`commandHead`).
pub fn command_head(name: &str) -> String {
    static HOP: OnceLock<Regex> = OnceLock::new();
    let hop = re(&HOP, r#"(?i)^(?:cd|pushd|chdir|set-location|sl)\s+(?:"[^"]*"|'[^']*'|\S+)\s*(?:&&|;)\s*"#);
    let mut line = first_line(name);
    while let Some(found) = hop.find(&line) {
        line = line[found.end()..].to_owned();
    }
    line.split_whitespace().next().unwrap_or("").to_owned()
}

/// What a command only looks at, from its first word (`commandLooksAt`).
pub fn command_looks_at(command: &str) -> Option<&'static str> {
    static FILES: OnceLock<Regex> = OnceLock::new();
    let head = command_head(command).to_lowercase();
    let head = head.strip_suffix(".exe").unwrap_or(&head);
    if ["cat", "head", "tail", "less", "more", "type", "get-content", "gc", "bat", "nl", "sed"].contains(&head) {
        return Some("read");
    }
    if head == "rg" && re(&FILES, r"\s--files\b").is_match(command) {
        return Some("list");
    }
    if ["rg", "grep", "egrep", "fgrep", "findstr", "select-string", "sls", "ag", "ack"].contains(&head) {
        return Some("search");
    }
    ["ls", "dir", "tree", "find", "fd", "get-childitem", "gci", "ll", "la"].contains(&head).then_some("list")
}

/// What a tool other than a command did, by the runtime's own name for it (`toolLooksAt`).
fn tool_looks_at(tool: &str) -> Option<&'static str> {
    static WEB: OnceLock<Regex> = OnceLock::new();
    static FETCH: OnceLock<Regex> = OnceLock::new();
    static PLAN: OnceLock<Regex> = OnceLock::new();
    static WAIT: OnceLock<Regex> = OnceLock::new();
    static LIST: OnceLock<Regex> = OnceLock::new();
    static GLOB: OnceLock<Regex> = OnceLock::new();
    static SEARCH: OnceLock<Regex> = OnceLock::new();
    static READ: OnceLock<Regex> = OnceLock::new();
    let name = tool.to_lowercase();
    let name = name.as_str();
    if re(&WEB, r"web_?search|search_?web").is_match(name) {
        Some("web")
    } else if re(&FETCH, r"fetch|read_?url|url_?content|browser").is_match(name) {
        Some("fetch")
    } else if re(&PLAN, r"todo|update_?plan|task_?boundary").is_match(name) {
        Some("plan")
    } else if name == "manage_task" || name == "schedule" {
        Some("wait")
    } else if name.contains("repl") {
        Some("code")
    } else if re(&WAIT, r"^(await|wait|await_?shell)$").is_match(name) {
        Some("wait")
    } else if re(&LIST, r"^(ls|list|list_?dir|list_?directory|listdir|dir)$").is_match(name) {
        Some("list")
    } else if re(&GLOB, r"^glob$|glob_?tool|find_?by_?name|file_?search").is_match(name) {
        Some("glob")
    } else if re(&SEARCH, r"grep|glob|search|find|codebase|semsearch|ripgrep").is_match(name) {
        Some("search")
    } else if re(&READ, r"^(read|read_?file|readfile|view|view_?file|open_?file|notebook_?read|cat|readtoolcall)$").is_match(name) {
        Some("read")
    } else {
        None
    }
}

fn path_word(word: &str) -> String {
    word.trim_start_matches(['"', '\'']).trim_end_matches(['"', '\'', ',', ';']).to_owned()
}

fn is_line_range(word: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"^\d+,\d+p?").is_match(word)
}

fn segments(line: &str, pipe_too: bool) -> Vec<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    let split = re(&R, r"\|\||&&|;");
    split
        .split(line)
        .map(|part| if pipe_too { part.split('|').next().unwrap_or("").to_owned() } else { part.to_owned() })
        .collect()
}

/// The files a reading command names, by their own names: `cat README.md LOCUST.md` reads both (`commandTargets`).
fn command_targets(command: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for segment in segments(&first_line(command), true) {
        if command_looks_at(segment.trim()) != Some("read") {
            continue;
        }
        for word in segment.split_whitespace().skip(1).map(path_word) {
            let keep = word.chars().count() > 1
                && !word.starts_with('-')
                && word.contains(['.', '/', '\\'])
                && !word.ends_with(['/', '\\'])
                && !is_line_range(&word);
            if let Some(name) = keep.then(|| last_part(&word)).flatten()
                && !names.contains(&name)
            {
                names.push(name);
            }
        }
    }
    names
}

fn first_command(command: &str) -> String {
    segments(&first_line(command), true).into_iter().next().unwrap_or_default()
}

/// The last path-looking word of the first command, as its file name (`commandTarget`).
fn command_target(command: &str) -> Option<String> {
    let first = first_command(command);
    let last = first
        .split_whitespace()
        .skip(1)
        .map(path_word)
        .filter(|w| w.chars().count() > 1 && !w.starts_with('-') && w.contains(['.', '/', '\\']) && !is_line_range(w))
        .last()?;
    last_part(&last)
}

/// What a search command looked for: its first word that is not a flag (`commandPattern`).
fn command_pattern(command: &str) -> Option<String> {
    let first = first_command(command);
    let word = first.split_whitespace().skip(1).find(|w| !w.starts_with('-') && w.chars().any(|c| c.is_ascii_alphanumeric()))?;
    let bare = word.trim_matches(['"', '\'']);
    (!bare.is_empty()).then(|| bare.to_owned())
}

/// A file name longer than the row can hold, cut in the middle with its extension kept (`shortName`, 0.604).
pub fn short_name(name: &str) -> String {
    static EXT: OnceLock<Regex> = OnceLock::new();
    let max = 34;
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max {
        return name.to_owned();
    }
    let extension = re(&EXT, r"(?:\.[A-Za-z0-9]{1,8}){1,2}$").find(name).map_or("", |m| m.as_str());
    let stem: Vec<char> = chars[..chars.len() - extension.chars().count()].to_vec();
    let room = 6usize.max(max.saturating_sub(extension.chars().count() + 1));
    let head = (room as f64 * 0.64).ceil() as usize;
    let tail = room - head;
    let start: String = stem.iter().take(head).collect();
    let end: String = if tail > 0 { stem[stem.len().saturating_sub(tail)..].iter().collect() } else { String::new() };
    format!("{start}\u{2026}{end}{extension}")
}

/// Locust's git tool, for a run with no shell (`gitToolCommand`): `locust_git_diff` is `git diff`.
fn git_tool_command(tool: &str) -> Option<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    let found = re(&R, r"(?i)^locust_git_(status|diff|log|show|blame|branches)$").captures(tool.trim())?;
    let command = found[1].to_lowercase();
    Some(format!("git {}", if command == "branches" { "branch" } else { &command }))
}

/// A runtime's own tool names said as words (`TOOL_WORDS`' `did`).
fn tool_did(tool: &str, count: usize) -> Option<String> {
    static IMAGE: OnceLock<Regex> = OnceLock::new();
    let tool = tool.trim();
    if re(&IMAGE, r"(?i)^(image_?generation|image_?gen|generate_?image)$").is_match(tool) {
        return Some(if count == 1 { "made an image".into() } else { format!("made {count} images") });
    }
    if tool.eq_ignore_ascii_case("invalid") {
        return Some(if count == 1 { "made a tool call that did not work".into() } else { format!("made {count} tool calls that did not work") });
    }
    git_tool_command(tool).map(|git| if count == 1 { format!("ran {git}") } else { format!("ran {git} {count} times") })
}

/// The skill a Skill call ran (`skillCalled`): None for any other tool, "" when the call named none.
fn skill_called(tool: &str, target: &str) -> Option<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    (tool.trim().eq_ignore_ascii_case("skill")).then(|| re(&R, r"^(project|personal):").replace(target.trim(), "").into_owned())
}

/// Codex's Windows sandbox never started the command (shared/codex-sandbox.ts).
fn sandbox_did_not_start(output: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)helper_unknown_error|setup refresh had errors").is_match(output)
}

#[derive(PartialEq)]
enum Kind {
    Reasoning,
    Shell,
    Helper,
    Edit,
    Tool,
}

struct Detail<'a> {
    item: &'a Item,
    kind: Kind,
    name: String,
    tool: &'a str,
    title: Option<String>,
    settled: bool,
    failed: bool,
    exit_code: Option<i64>,
    output: &'a str,
    background: bool,
    background_ended: &'a str,
    status: &'a str,
    duration_ms: u64,
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn detail_of(item: &Item) -> Option<Detail<'_>> {
    match item {
        Item::Thought { text, duration_ms, .. } => Some(Detail {
            item,
            kind: Kind::Reasoning,
            name: String::new(),
            tool: "",
            title: None,
            settled: true,
            failed: false,
            exit_code: None,
            output: text,
            background: false,
            background_ended: "",
            status: "",
            duration_ms: duration_ms.unwrap_or(0),
        }),
        Item::Tool { name, status, summary, detail } => {
            let arguments = (!detail.arguments.is_empty() && !detail.arguments.starts_with(['{', '['])).then_some(detail.arguments.as_str());
            let target = arguments.map_or_else(|| if summary.is_empty() { name.clone() } else { summary.clone() }, str::to_owned);
            let kind = if crate::activity_summary::is_edit(item) {
                Kind::Edit
            } else if crate::command_outcomes::is_shell(name) {
                Kind::Shell
            } else if detail.helper {
                Kind::Helper
            } else {
                Kind::Tool
            };
            let title = (kind == Kind::Shell && !summary.is_empty() && arguments.is_some_and(|a| one_line(a) != one_line(summary)))
                .then(|| summary.clone());
            Some(Detail {
                item,
                kind,
                name: target,
                tool: name,
                title,
                settled: *status != Status::Running,
                failed: *status == Status::Failed,
                exit_code: detail.exit_code,
                output: &detail.output,
                background: detail.background,
                background_ended: &detail.background_ended,
                status: &detail.runtime_status,
                duration_ms: detail.duration_ms.unwrap_or(0),
            })
        }
        _ => None,
    }
}

#[derive(Default)]
struct Held {
    count: usize,
    names: Vec<String>,
}

/// A steps line's words for its thoughts and calls (`stepsLine`). `open` drops a lone thought's preview: its words are
/// underneath (0.594).
pub fn steps_line(items: &[Item], finished: bool, workspace: Option<&str>, open: bool) -> StepsLine {
    static ING: OnceLock<Regex> = OnceLock::new();
    static INLINE: OnceLock<Regex> = OnceLock::new();
    static NAMED_SEARCH: OnceLock<Regex> = OnceLock::new();
    static EVERY: OnceLock<Regex> = OnceLock::new();
    let ing = re(&ING, r"^[A-Z][a-z]+ing\b");
    let details: Vec<Detail> = items.iter().filter_map(detail_of).collect();
    let mut phrases: Vec<(&'static str, Held)> = Vec::new();
    let mut note = |kind: &'static str, name: Option<String>| {
        let at = phrases.iter().position(|(k, _)| *k == kind).unwrap_or_else(|| {
            phrases.push((kind, Held::default()));
            phrases.len() - 1
        });
        let held = &mut phrases[at].1;
        held.count += 1;
        if let Some(name) = name.filter(|n| !n.is_empty())
            && !held.names.contains(&name)
        {
            held.names.push(name);
        }
    };
    let file_name = |path: &str| -> Option<String> {
        let relative = crate::path_words::relative_path(path, workspace);
        let bare = relative.trim().trim_matches(['"', '\'']);
        if bare.is_empty() || (bare.contains(char::is_whitespace) && !bare.contains(['/', '\\'])) || !bare.contains(['.', '/', '\\']) {
            return None;
        }
        last_part(bare)
    };
    let (mut failed, mut sandbox_failed, mut refused, mut declined, mut silent, mut working) = (0, 0, 0, 0, 0, 0);
    let (mut thought_ms, mut thoughts) = (0u64, 0usize);
    let mut headline: Option<String> = None;
    let mut thought_text: Option<&str> = None;
    for detail in &details {
        if detail.status == "refused" {
            refused += 1;
            continue;
        }
        if detail.status == "declined" {
            declined += 1;
            continue;
        }
        if finished && !detail.settled && !detail.failed {
            silent += 1;
        }
        if detail.background && detail.settled && !detail.failed {
            if detail.background_ended == "failed" {
                failed += 1;
            } else if detail.background_ended.is_empty() {
                if finished { silent += 1 } else { working += 1 }
            }
        }
        match detail.kind {
            Kind::Reasoning => {
                thoughts += 1;
                thought_ms += detail.duration_ms;
                if headline.is_none() {
                    headline = crate::thread_words::thought_headline(detail.output);
                }
                if !detail.output.trim().is_empty() && thought_text.is_none() {
                    thought_text = Some(detail.output);
                }
            }
            Kind::Shell => {
                if detail.failed || detail.exit_code.is_some_and(|c| c != 0) {
                    failed += 1;
                }
                if detail.failed && sandbox_did_not_start(detail.output) {
                    sandbox_failed += 1;
                }
                let looked = command_looks_at(&detail.name);
                let targets = command_targets(&detail.name);
                let pattern = command_pattern(&detail.name);
                if looked == Some("read") && targets.len() > 1 {
                    for name in targets {
                        note("read", Some(name));
                    }
                } else if looked == Some("search") && pattern.as_deref().is_some_and(|p| p.contains(['/', '\\'])) {
                    note("searchedIn", pattern.as_deref().and_then(last_part));
                } else if let Some(looked) = looked {
                    let name = match looked {
                        "read" => command_target(&detail.name),
                        "search" => pattern,
                        _ => None,
                    };
                    note(looked, name);
                } else {
                    note("command", Some(detail.title.clone().unwrap_or_else(|| command_head(&detail.name))));
                }
            }
            Kind::Helper => {
                if detail.failed {
                    failed += 1;
                }
                note("helper", Some(detail.name.clone()));
            }
            Kind::Edit => {
                if detail.failed {
                    failed += 1;
                    continue;
                }
                let files = crate::activity_entries::files(detail.item);
                if files.is_empty() {
                    let paths = crate::activity_summary::unreported_paths(detail.item);
                    let paths = if paths.is_empty() { vec![detail.name.clone()] } else { paths };
                    for path in paths {
                        note("edited", Some(file_name(&path).unwrap_or(path)));
                    }
                } else {
                    for entry in files {
                        let kind = match entry.file.status.as_str() {
                            "ADDED" => "created",
                            "DELETED" => "deleted",
                            _ => "edited",
                        };
                        note(kind, Some(file_name(&entry.file.path).unwrap_or(entry.file.path.clone())));
                    }
                }
            }
            Kind::Tool => {
                if detail.failed {
                    failed += 1;
                }
                let skill_target = if detail.tool == detail.name { "" } else { detail.name.as_str() };
                if let Some(skill) = skill_called(detail.tool, skill_target) {
                    note("skill", Some(skill));
                    continue;
                }
                if let Some(git) = git_tool_command(detail.tool) {
                    note("git", Some(git));
                    continue;
                }
                match tool_looks_at(detail.tool) {
                    Some(looked) => {
                        let searched_in = (looked == "search" && detail.name.contains(['/', '\\'])).then(|| file_name(&detail.name)).flatten();
                        let kind = if searched_in.is_some() { "searchedIn" } else { looked };
                        let name = match looked {
                            "read" | "list" => file_name(&detail.name),
                            "glob" => Some(detail.name.clone()),
                            "code" => Some(detail.tool.to_owned()),
                            _ => searched_in.or_else(|| {
                                (looked == "search" && !re(&NAMED_SEARCH, r"\s{2}|^[A-Z][a-z]+ing\b").is_match(&detail.name)).then(|| detail.name.clone())
                            }),
                        };
                        note(kind, name);
                    }
                    None => note("tool", Some(detail.tool.to_owned())),
                }
            }
        }
    }
    let one = |held: &Held| (held.count == 1).then(|| held.names.first().cloned()).flatten();
    let (mut words, mut briefs, mut longs): (Vec<String>, Vec<String>, Vec<String>) = (vec![], vec![], vec![]);
    let mut word = |full: String, brief: Option<String>, long: Option<String>| {
        briefs.push(brief.unwrap_or_else(|| full.clone()));
        longs.push(long.unwrap_or_else(|| full.clone()));
        words.push(full);
    };
    let files = |held: &Held| {
        if held.names.len() > 1 {
            format!("read {}", pluralize(held.names.len(), "file"))
        } else if held.count == 1 {
            "read a file".to_owned()
        } else {
            format!("read {}", pluralize(held.count, "file"))
        }
    };
    let searches = |held: &Held| if held.count == 1 { "ran a search".to_owned() } else { format!("ran {}", times(held.count, "search", "searches")) };
    for (kind, held) in &phrases {
        let named = one(held);
        let n = held.count;
        match *kind {
            "read" => word(
                named.as_ref().map_or_else(|| files(held), |n| format!("read {}", short_name(n))),
                Some(files(held)),
                Some(named.as_ref().map_or_else(|| files(held), |n| format!("read {n}"))),
            ),
            "search" => word(
                if n == 1 { named.as_ref().map_or_else(|| "searched the files".into(), |p| format!("searched for {}", clip(p, 32))) } else { format!("ran {}", times(n, "search", "searches")) },
                Some(searches(held)),
                None,
            ),
            "code" => {
                let language = if held.names.iter().all(|n| n.to_lowercase().contains("node") || n.to_lowercase().contains("js")) {
                    "JavaScript"
                } else if { let all = held.names.join(" ").to_lowercase(); all.contains("python") || all.contains("py") } {
                    "Python"
                } else {
                    "code"
                };
                word(if n == 1 { format!("ran {language}") } else { format!("ran {language} {n} times") }, None, None);
            }
            "glob" => word(
                if n > 1 {
                    format!("listed files {n} times")
                } else {
                    match &named {
                        Some(p) if !re(&EVERY, r"^(\*\*[\\/])?\*(\.\*)?$").is_match(p) => format!("listed files matching {}", clip(p, 28)),
                        _ => "listed every file".into(),
                    }
                },
                Some(if n > 1 { format!("listed files {n} times") } else { "listed files".into() }),
                None,
            ),
            "searchedIn" => {
                let many = format!("ran {}", times(n, "search", "searches"));
                word(
                    named.as_ref().filter(|_| n == 1).map_or_else(|| many.clone(), |p| format!("searched {}", short_name(p))),
                    Some(searches(held)),
                    Some(named.as_ref().filter(|_| n == 1).map_or(many.clone(), |p| format!("searched {p}"))),
                );
            }
            "wait" => word(if n == 1 { "waited for a command".into() } else { format!("waited {n} times") }, None, None),
            "list" => {
                let many = format!("listed {}", pluralize(n, "folder"));
                let brief = if n == 1 { "listed a folder".to_owned() } else { many.clone() };
                word(
                    if n == 1 { named.as_ref().map_or_else(|| "listed a folder".into(), |p| format!("listed {}", short_name(p))) } else { many.clone() },
                    Some(brief),
                    Some(if n == 1 { named.as_ref().map_or_else(|| "listed a folder".into(), |p| format!("listed {p}")) } else { many }),
                );
            }
            "web" => word(if n == 1 { "searched the web".into() } else { format!("searched the web {n} times") }, None, None),
            "fetch" => word(if n == 1 { "fetched a page".into() } else { format!("fetched {}", pluralize(n, "page")) }, None, None),
            "plan" => word("updated the plan".into(), None, None),
            "skill" => {
                let only = (held.names.len() == 1).then(|| held.names[0].clone());
                let again = if n == 1 { String::new() } else { format!(" {n} times") };
                let many = format!("used {}", pluralize(held.names.len().max(1), "skill"));
                word(
                    only.as_ref().map_or_else(|| many.clone(), |s| format!("used the {} skill{again}", clip(s, 32))),
                    Some(if n == 1 { "used a skill".into() } else if only.is_some() { format!("used a skill{again}") } else { many.clone() }),
                    Some(only.as_ref().map_or(many.clone(), |s| format!("used the {s} skill{again}"))),
                );
            }
            "helper" => word(
                if n == 1 { format!("asked a helper{}", named.as_ref().map_or_else(String::new, |h| format!(": {}", clip(h, 48)))) } else { format!("asked {}", pluralize(n, "helper")) },
                Some(if n == 1 { "asked a helper".into() } else { format!("asked {}", pluralize(n, "helper")) }),
                None,
            ),
            "git" => {
                let commands: Vec<String> = held.names.iter().map(|n| n.strip_prefix("git ").unwrap_or(n).to_owned()).collect();
                let brief = if n == 1 { "ran git".to_owned() } else { format!("ran git {n} times") };
                let full = match commands.len() {
                    1 if n == 1 => format!("ran git {}", commands[0]),
                    1 => format!("ran git {} {n} times", commands[0]),
                    2 | 3 => format!("ran git {} and {}", commands[..commands.len() - 1].join(", "), commands[commands.len() - 1]),
                    _ => brief.clone(),
                };
                word(full, Some(brief), None);
            }
            "created" | "edited" | "deleted" => {
                let many = format!("{kind} {}", pluralize(held.names.len().max(1), "file"));
                let lone = held.names.len() == 1;
                word(
                    if lone { format!("{kind} {}", short_name(&held.names[0])) } else { many.clone() },
                    Some(if lone { format!("{kind} a file") } else { many.clone() }),
                    Some(if lone { format!("{kind} {}", held.names[0]) } else { many }),
                );
            }
            "tool" => {
                if let Some(known) = (held.names.len() == 1).then(|| tool_did(&held.names[0], n)).flatten() {
                    word(known, None, None);
                } else if held.names.len() == 1 {
                    let tool = &held.names[0];
                    word(
                        if n == 1 { format!("used {tool}") } else { format!("used {tool} {n} times") },
                        Some(if n == 1 { "used a tool".into() } else { format!("used a tool {n} times") }),
                        None,
                    );
                } else {
                    word(pluralize(n, "other tool call"), None, None);
                }
            }
            "command" => {
                let titled: Vec<&Detail> = details.iter().filter(|d| d.kind == Kind::Shell && command_looks_at(&d.name).is_none()).collect();
                let only = (titled.len() == 1).then(|| titled[0]);
                let first = first_line(only.map_or("", |d| d.name.as_str()));
                let full = if n > 1 {
                    format!("ran {}", pluralize(n, "command"))
                } else if let Some(title) = only.and_then(|d| d.title.as_deref()) {
                    lower_first(title)
                } else if ing.is_match(&first) {
                    lower_first(&clip(&first, 48))
                } else if first.chars().filter(char::is_ascii_alphabetic).count() < 3 {
                    "ran a script".into()
                } else if first.chars().count() > 40 && re(&INLINE, r#"\s-(?:e|c|-eval|-command|Command)\s+["'`]"#).is_match(&first) {
                    let head = command_head(&first);
                    let head = head.strip_suffix(".exe").or_else(|| head.strip_suffix(".EXE")).unwrap_or(&head).to_owned();
                    format!("ran a {head} script")
                } else {
                    format!("ran {}", clip(&first, 40))
                };
                word(full, Some(if n == 1 { "ran a command".into() } else { format!("ran {}", pluralize(n, "command")) }), None);
            }
            _ => {}
        }
    }
    if let Some(headline) = headline {
        words = vec![headline.clone()];
        briefs = vec![headline.clone()];
        longs = vec![headline];
    } else if words.is_empty() && thoughts > 0 {
        let thought = if thought_ms >= 1000 { format!("thought for {}", crate::ledger::duration_text(thought_ms)) } else { "thought".into() };
        let said = (!open).then(|| thought_text.map(one_line)).flatten().filter(|s| !s.is_empty());
        words.push(said.map_or_else(|| thought.clone(), |s| format!("{thought}: {}", clip(&s, 70))));
        briefs.push(thought.clone());
        longs.push(words[0].clone());
    }
    let lone_failed = details.len() == 1 && details[0].kind == Kind::Shell && details[0].title.is_some() && failed == 1 && refused == 0 && declined == 0;
    let sentence = if lone_failed {
        format!("Failed to {}", lower_first(details[0].title.as_deref().unwrap_or("")))
    } else if words.is_empty() {
        pluralize(details.len(), "step")
    } else if words.len() > 1 {
        briefs.join(", ")
    } else {
        words[0].clone()
    };
    let mut segments = vec![Segment { key: "what", text: capital(&sentence), amber: lone_failed }];
    let mut push = |key: &'static str, text: String, amber: bool| segments.push(Segment { key, text, amber });
    if failed > 0 && !lone_failed {
        push("failed", format!("{failed} failed"), true);
    }
    if sandbox_failed > 0 && sandbox_failed == failed {
        push("sandbox", "Codex\u{2019}s sandbox did not start".into(), true);
    }
    if working > 0 {
        push("working", format!("{working} working in the background"), false);
    }
    if silent > 0 {
        push("silent", format!("{silent} did not report"), true);
    }
    if refused > 0 {
        push("refused", format!("{refused} refused"), true);
    }
    if declined > 0 {
        push("declined", format!("{declined} declined"), true);
    }
    let whole = if lone_failed || words.is_empty() { sentence.clone() } else { longs.join(", ") };
    StepsLine { shorter: shorter_forms(&words, &briefs, &sentence, lone_failed), segments, title: capital(&whole) }
}

/// The shorter forms of a steps sentence (`shorterForms`, 0.604): the phrases without their names, then with the last
/// phrases given way to "and N more".
fn shorter_forms(words: &[String], briefs: &[String], sentence: &str, lone_failed: bool) -> Vec<String> {
    if lone_failed || words.is_empty() {
        return Vec::new();
    }
    let len = |s: &str| s.encode_utf16().count();
    let mut shorter: Vec<String> = Vec::new();
    let brief = briefs.join(", ");
    if len(&brief) < len(sentence) {
        shorter.push(capital(&brief));
    }
    for keep in (1..briefs.len()).rev() {
        let text = format!("{} and {} more", briefs[..keep].join(", "), briefs.len() - keep);
        if len(&text) < len(shorter.last().map_or(sentence, String::as_str)) {
            shorter.push(capital(&text));
        }
    }
    shorter
}
