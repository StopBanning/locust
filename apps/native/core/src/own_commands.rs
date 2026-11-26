//! The person's own slash commands: Markdown files that become prompts in the `/` menu (Electron
//! `main/own-command-host.ts` and `shared/own-commands.ts`). Two folders hold them: `<profile>/commands`, on this
//! computer, and `<folder>/.locust/commands`, in the folder a conversation works in. A file's path is its name
//! (`team/review.md` is `/team/review`), and an optional top-of-file block between `---` lines gives its description,
//! the teammate it is for and the mode it starts in.
use std::path::{Path, PathBuf};

/// At most this many commands are read from one folder (Electron `MAX_COMMANDS`).
pub const MAX_COMMANDS: usize = 200;
/// A file longer than this, in UTF-16 units as Electron counts `text.length`, is left out (`MAX_FILE`).
pub const MAX_FILE: usize = 100_000;
/// Folders deeper than this are not read (`relative.split('/').length <= 8`).
pub const MAX_DEPTH: usize = 8;

/// Which folder a command came from: the person's own on this computer, or the conversation's folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Global,
    Project,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Global => "global",
            Source::Project => "project",
        }
    }
}

/// One command file, read (Electron `ParsedCommand` with the `source` readTree adds).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCommand {
    pub name: String,
    pub description: Option<String>,
    pub teammate: Option<String>,
    /// A mode Locust has: `ask`, `plan`, `accept-edits`, `approve-each` or `auto`.
    pub mode: Option<&'static str>,
    pub body: String,
    pub source: Source,
}

/// The words a command file may give as its mode, and the mode each means (Electron `MODE_WORDS`).
fn mode_of(word: &str) -> Option<&'static str> {
    match word.to_lowercase().as_str() {
        "ask" => Some("ask"),
        "plan" => Some("plan"),
        "edit" | "accept-edits" => Some("accept-edits"),
        "approve" | "approve-each" => Some("approve-each"),
        "auto" => Some("auto"),
        _ => None,
    }
}

/// `team/review.md` is the command `/team/review` (Electron `commandNameFromPath`).
pub fn command_name_from_path(relative_path: &str) -> String {
    let forward = relative_path.replace('\\', "/");
    crate::invariants::regex(r"(?i)\.md$").replace(&forward, "").into_owned()
}

/// One field of the top-of-file block (Electron `fieldOf`): the first line that starts with the key and a colon, its
/// value trimmed and taken out of one pair of matching quotes. An empty value is no value.
fn field_of(meta: &str, key: &str) -> Option<String> {
    let line = crate::invariants::regex(r"\r?\n").split(meta).find(|one| {
        one.strip_prefix(key).is_some_and(|rest| rest.trim_start().starts_with(':'))
    })?;
    let mut value = line[line.find(':').map_or(0, |at| at + 1)..].trim();
    let mut chars = value.chars();
    if let (Some(first), Some(last)) = (chars.next(), chars.next_back())
        && (first == '\'' || first == '"')
        && first == last
    {
        value = value[1..value.len() - 1].trim();
    }
    (!value.is_empty()).then(|| value.to_owned())
}

/// A command file read (Electron `parseOwnCommand`). The top-of-file block is optional and unknown keys are ignored; so
/// is a mode Locust does not have. Without the block, the whole file is the prompt.
pub fn parse_own_command(markdown: &str, name: &str, source: Source) -> StoredCommand {
    let wrapped = crate::invariants::regex(r"^---\r?\n((?s:.)*?)\r?\n---\r?\n?((?s:.)*)$").captures(markdown);
    let meta = wrapped.as_ref().and_then(|c| c.get(1)).map_or("", |m| m.as_str());
    let body = match &wrapped {
        Some(c) => c.get(2).map_or("", |m| m.as_str()),
        None => markdown,
    };
    let body = body.strip_prefix('\n').unwrap_or(body).to_owned();
    StoredCommand {
        name: name.to_owned(),
        description: field_of(meta, "description"),
        teammate: field_of(meta, "teammate"),
        mode: field_of(meta, "mode").and_then(|word| mode_of(&word)),
        body,
        source,
    }
}

/// Every command under `root` (Electron `readTree`): Markdown files, nested folders giving nested names, at most
/// [`MAX_COMMANDS`]. A name starting with `.` and a link are passed over, as is a folder deeper than [`MAX_DEPTH`]. A
/// file that cannot be read, or is longer than [`MAX_FILE`], is left out and the rest are still read; a folder that is
/// missing or cannot be read gives none. Entries are read in name order, so the menu keeps one order on every disk.
pub fn read_tree(root: &Path, source: Source) -> Vec<StoredCommand> {
    let mut found = Vec::new();
    walk(root, "", source, &mut found);
    found
}

fn walk(dir: &Path, prefix: &str, source: Source, found: &mut Vec<StoredCommand>) {
    if found.len() >= MAX_COMMANDS {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if found.len() >= MAX_COMMANDS || name.starts_with('.') || kind.is_symlink() {
            continue;
        }
        let relative = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        if kind.is_dir() {
            if relative.split('/').count() <= MAX_DEPTH {
                walk(&entry.path(), &relative, source, found);
            }
        } else if kind.is_file() && name.to_lowercase().ends_with(".md") {
            // A file of more than three bytes a unit is longer than MAX_FILE units however it reads; not read at all.
            if entry.metadata().is_ok_and(|m| m.len() > 3 * MAX_FILE as u64) {
                continue;
            }
            let Ok(bytes) = std::fs::read(entry.path()) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            if text.encode_utf16().count() > MAX_FILE {
                continue;
            }
            found.push(parse_own_command(&text, &command_name_from_path(&relative), source));
        }
    }
}

/// The person's commands folder on this computer: `<profile>/commands`.
pub fn global_dir(profile: &Path) -> PathBuf {
    profile.join("commands")
}

/// A folder's commands folder: `<folder>/.locust/commands`.
pub fn project_dir(folder: &Path) -> PathBuf {
    folder.join(".locust").join("commands")
}

/// The commands a conversation can use (Electron `commandsIn`): the person's own first, then its folder's, when it has
/// one.
pub fn commands_in(profile: &Path, folder: Option<&Path>) -> Vec<StoredCommand> {
    let mut all = read_tree(&global_dir(profile), Source::Global);
    if let Some(folder) = folder {
        all.extend(read_tree(&project_dir(folder), Source::Project));
    }
    all
}

/// Words, with quotes grouping a phrase; a backslash is not an escape (Electron `splitCommandWords`).
pub fn split_command_words(input: &str) -> Vec<String> {
    crate::invariants::regex(r#""([^"]*)"|'([^']*)'|(\S+)"#)
        .captures_iter(input)
        .map(|c| c.get(1).or_else(|| c.get(2)).or_else(|| c.get(3)).map_or("", |m| m.as_str()).to_owned())
        .collect()
}

/// A command's prompt with what was typed after its name (Electron `expandTemplate`). `$ARGUMENTS` is everything typed;
/// `$1`, `$2` are the words, the highest number taking the rest. With neither, what was typed follows a blank line.
pub fn expand_template(body: &str, args: &str) -> String {
    let words = split_command_words(args);
    const TOKEN: &str = "\u{0}";
    let has_arguments = body.contains("$ARGUMENTS");
    let mut text = if has_arguments { body.replace("$ARGUMENTS", TOKEN) } else { body.to_owned() };
    let mut numbers: Vec<usize> = crate::invariants::regex(r"\$(\d+)")
        .captures_iter(&text)
        .filter_map(|c| c.get(1)?.as_str().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .collect();
    numbers.sort_unstable_by(|a, b| b.cmp(a));
    numbers.dedup();
    let last = numbers.first().copied();
    for n in &numbers {
        let value = if Some(*n) == last {
            words.get(n - 1..).map(|rest| rest.join(" ")).unwrap_or_default()
        } else {
            words.get(n - 1).cloned().unwrap_or_default()
        };
        text = text.replace(&format!("${n}"), &value);
    }
    if has_arguments {
        text = text.replace(TOKEN, args);
    }
    if !has_arguments && numbers.is_empty() && !args.trim().is_empty() {
        text = format!("{}\n\n{args}", text.trim_end());
    }
    text
}

/// A `!` followed by a command in backticks (Electron `bangBlocks`): the command inside, and the whole mark it replaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BangBlock {
    pub command: String,
    pub token: String,
}

pub fn bang_blocks(text: &str) -> Vec<BangBlock> {
    crate::invariants::regex(r"!`([^`\n]+)`")
        .captures_iter(text)
        .filter_map(|c| {
            let command = c.get(1)?.as_str().trim();
            let token = c.get(0)?.as_str();
            (!command.is_empty()).then(|| BangBlock { command: command.to_owned(), token: token.to_owned() })
        })
        .collect()
}

/// `/name the rest` when `name` is one of the person's commands (Electron `argumentsOfOwnCommand`): the name as the file
/// gives it and what was typed after it. A bare `/name` is the menu's, still being filtered.
pub fn arguments_of_own_command(text: &str, names: &[&str]) -> Option<(String, String)> {
    let trimmed = text.trim();
    let c = crate::invariants::regex(r"^/(\S+)(?:\s+((?s:.)*))?$").captures(trimmed)?;
    let typed = c.get(1)?.as_str();
    if !trimmed[1 + typed.len()..].contains(' ') {
        return None;
    }
    let name = names.iter().find(|one| one.to_lowercase() == typed.to_lowercase())?;
    Some(((*name).to_owned(), c.get(2).map_or("", |m| m.as_str()).trim().to_owned()))
}

/// What choosing a command gives (Electron `expand`'s three answers): a refusal in words, the folder's `!` lines to ask
/// about before any runs, or the prompt for the box with the mode and teammate the file names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expanded {
    Refused(String),
    Ask(Vec<String>),
    Text { text: String, mode: Option<&'static str>, teammate: Option<String> },
}

/// The words for a command no longer in either folder.
pub const NOT_IN_FOLDER: &str = "That command is not in this folder. Nothing was changed.";
/// The words for a `!` line with no folder to run in.
pub const CHOOSE_A_FOLDER: &str = "Choose a folder first. Nothing was run.";
/// The words for Allow with no folder.
pub const NOTHING_ALLOWED: &str = "Choose a folder first. Nothing was allowed.";

/// The command named, its prompt filled in (Electron `expand`). A prompt with no `!` line is the answer at once. One
/// with `!` lines is refused when the organization has turned the terminal off; a folder's own asks first until the
/// folder is allowed; then each line runs in the folder, through `run`, and its output takes the line's place.
pub fn expand(
    profile: &Path,
    folder: Option<&Path>,
    name: &str,
    source: Source,
    args: &str,
    refusal: Option<String>,
    run: &mut dyn FnMut(&str, &Path) -> crate::own_command::ResultRow,
) -> Expanded {
    let commands = commands_in(profile, folder);
    let Some(command) = commands.iter().find(|one| one.source == source && one.name.to_lowercase() == name.to_lowercase()) else {
        return Expanded::Refused(NOT_IN_FOLDER.into());
    };
    let text = expand_template(&command.body, args);
    let blocks = bang_blocks(&text);
    let done = |text: String| Expanded::Text { text, mode: command.mode, teammate: command.teammate.clone() };
    if blocks.is_empty() {
        return done(text);
    }
    if let Some(refused) = refusal {
        return Expanded::Refused(refused);
    }
    let Some(folder) = folder else {
        return Expanded::Refused(CHOOSE_A_FOLDER.into());
    };
    if source == Source::Project && !allowed(profile, folder) {
        return Expanded::Ask(blocks.into_iter().map(|block| block.command).collect());
    }
    let mut filled = text;
    for block in blocks {
        let ran = run(&block.command, folder);
        let body = if ran.output.is_empty() { "(no output)" } else { ran.output.as_str() };
        let how = if ran.stopped {
            "stopped before it finished".to_owned()
        } else {
            ran.exit.map_or_else(|| "ended".to_owned(), |code| format!("exit {code}"))
        };
        let note = ran.omitted.map(|note| format!("\n{note}")).unwrap_or_default();
        if let Some(at) = filled.find(&block.token) {
            let insertion = format!("output of `{}` ({how}):\n{body}{note}", block.command);
            filled.replace_range(at..at + block.token.len(), &insertion);
        }
    }
    done(filled)
}

/// [`expand`] as the window runs it: the organization's terminal policy, and each `!` line through the person's shell
/// in the folder, as their own `!` command runs.
pub fn expand_here(profile: &Path, folder: Option<&Path>, name: &str, source: Source, args: &str) -> Expanded {
    expand(profile, folder, name, source, args, crate::terminal::check_policy().err(), &mut run_line)
}

/// One `!` line, run to its end in `folder` (own-command-host.ts `runIn`): one that cannot start says why, as exit 1.
pub fn run_line(command: &str, folder: &Path) -> crate::own_command::ResultRow {
    let failed = |output: String| crate::own_command::ResultRow { output, omitted: None, exit: Some(1), stopped: false, elapsed: 0 };
    let mut running = match crate::own_command::Running::start(command, folder) {
        Ok(running) => running,
        Err(message) => return failed(message),
    };
    loop {
        match running.poll() {
            Ok(Some(done)) => return done,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(message) => return failed(message),
        }
    }
}

/// Where the folders whose commands may run their `!` lines are kept (Electron `command-allows.json`).
pub fn allows_path(profile: &Path) -> PathBuf {
    profile.join("command-allows.json")
}

/// A folder as Node's `resolve` gives it: absolute, with `.` and `..` taken out by their names alone.
fn resolved(folder: &Path) -> PathBuf {
    let absolute = std::path::absolute(folder).unwrap_or_else(|_| folder.to_path_buf());
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn read_allows(profile: &Path) -> Vec<String> {
    std::fs::read(allows_path(profile))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value.as_array().cloned())
        .map(|list| list.into_iter().filter_map(|one| one.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// Whether the person allowed this folder's commands to run their `!` lines.
pub fn allowed(profile: &Path, folder: &Path) -> bool {
    let folder = resolved(folder);
    read_allows(profile).iter().any(|one| resolved(Path::new(one)) == folder)
}

/// Allow a folder's `!` lines, once, for good (Electron `allow`): the folder is kept in `command-allows.json`.
pub fn allow(profile: &Path, folder: Option<&Path>) -> Result<(), String> {
    let Some(folder) = folder else {
        return Err(NOTHING_ALLOWED.into());
    };
    let folder = resolved(folder);
    let mut kept: Vec<String> = read_allows(profile).into_iter().filter(|one| resolved(Path::new(one)) != folder).collect();
    kept.push(folder.to_string_lossy().into_owned());
    std::fs::create_dir_all(profile).map_err(|e| e.to_string())?;
    std::fs::write(allows_path(profile), serde_json::Value::from(kept).to_string()).map_err(|e| e.to_string())
}

/// The words when a folder's commands folder is asked for with no folder open (own-command-ipc.ts).
pub const NOWHERE_TO_OPEN: &str = "Choose a folder first. There is nowhere to open.";
/// The words when the commands folder could not be made (ChatCommands.tsx's `failureSaid` fallback).
pub const COULD_NOT_OPEN: &str = "That folder could not be opened. Your commands are where they were.";

/// Where a source's commands live (own-command-host.ts `directory`): the person's own folder on this computer, or the
/// conversation folder's, when there is one. Locust names the folder; nothing the window sends does.
pub fn directory(profile: &Path, which: Source, folder: Option<&Path>) -> Option<PathBuf> {
    match which {
        Source::Global => Some(global_dir(profile)),
        Source::Project => folder.map(project_dir),
    }
}

/// Open folder (own-command-ipc.ts `OWN_COMMAND_OPEN`): the commands folder is made if it is missing and handed to
/// `open`, the file manager; with no folder open, a folder's own is refused in Electron's words and nothing opens.
/// What `open` says when it fails is the answer, as Electron returns `shell.openPath`'s error.
pub fn open_folder(profile: &Path, which: Source, folder: Option<&Path>, open: &mut dyn FnMut(&Path) -> Result<(), String>) -> Result<(), String> {
    let dir = directory(profile, which, folder).ok_or_else(|| NOWHERE_TO_OPEN.to_owned())?;
    std::fs::create_dir_all(&dir).map_err(|_| COULD_NOT_OPEN.to_owned())?;
    open(&dir)
}

/// [`open_folder`] in the computer's file manager (Electron `shell.openPath`): Finder on a Mac, the desktop's opener on
/// Linux.
pub fn open_folder_here(profile: &Path, which: Source, folder: Option<&Path>) -> Result<(), String> {
    open_folder(profile, which, folder, &mut |dir| {
        let opener = if cfg!(target_os = "macos") { "/usr/bin/open" } else { "xdg-open" };
        let mut cmd = std::process::Command::new(opener);
        cmd.arg(dir);
        crate::runtime::process::own_group(&mut cmd);
        match cmd.status() {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err(COULD_NOT_OPEN.to_owned()),
            Err(e) => Err(e.to_string()),
        }
    })
}

#[cfg(test)]
#[path = "own_commands_tests.rs"]
mod own_commands_tests;
