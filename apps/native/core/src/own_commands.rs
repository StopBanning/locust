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

#[cfg(test)]
#[path = "own_commands_tests.rs"]
mod own_commands_tests;
