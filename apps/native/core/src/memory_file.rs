//! Team memory as a FILE the teammate can read, beside the lines it is handed (Electron `main/memory-file.ts`), and
//! whether a memory's named files moved on without it (`main/memory-provenance.ts`, `shared/memory.ts` citedPaths).
//!
//! The brief pastes the newest few memories and names `.locust/memory.md` for the rest; the file lists every memory
//! with its id, so a tidy pass can name one exactly. The block is still how memory CHANGES: the file is read-only to
//! the teammate. With memory switched off, a file written while it was on is rewritten to say so (A1.7).
//!
//! A memory that names a file -- "the retry logic lives in src/net.ts" -- is a claim about that file as it was. Once
//! the file changes, the brief, the file and the Memory screen say "may be out of date: src/net.ts changed since"
//! (A1.3). The memory is still briefed: a file changes for many reasons.

use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// Where the file lives, relative to the folder the run stands in.
pub const MEMORY_FILE_IN_FOLDER: &str = ".locust/memory.md";

/// What the file says while memory is switched off.
pub const MEMORY_OFF_TEXT: &str = "# Team memory\n\nTeam memory is switched off in Locust for this folder. Nothing here is current, and nothing is remembered for this run.\n";

/// A file this much newer than the memory counts as changed after it: a save a second later is the same moment.
const GRACE_MS: i64 = 2000;
/// At most this many cited files are checked per memory.
const MAX_CITED: usize = 5;

const FILE_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "md", "mdx", "py", "rb", "go", "rs", "java", "kt", "swift", "c", "h",
    "cpp", "hpp", "cs", "css", "scss", "html", "yml", "yaml", "toml", "ini", "env", "lock", "sh", "ps1", "bat", "sql",
    "txt", "csv", "xml", "gradle", "vue", "svelte", "php", "lua", "dart", "ex", "exs", "tf", "proto", "graphql",
];

/// The files a memory names, as written in it: paths, or file names with an extension a project file has. Not URLs,
/// not version numbers, not "e.g.". Only what it says -- whether a named file exists is `changed_since`'s to check.
pub fn cited_paths(text: &str) -> Vec<String> {
    let split = crate::invariants::regex(r#"[\s,;()\[\]{}<>"'`]+"#);
    let scheme = crate::invariants::regex(r"(?i)^[a-z]{2,}:");
    let extension = crate::invariants::regex(r"\.([A-Za-z0-9]{1,10})$");
    let dotfile = crate::invariants::regex(r"^\.[A-Za-z][A-Za-z0-9._-]*$");
    let mut found: Vec<String> = Vec::new();
    for raw in split.split(text) {
        // Only trailing punctuation: a leading dot is a dotfile's name (.env).
        let token = raw.trim_start_matches([':', '!', '?']).trim_end_matches(['.', ':', '!', '?']);
        // A scheme is two letters or more: `C:\...` is a Windows path, not a scheme.
        if token.is_empty() || token.encode_utf16().count() > 200 || token.contains("://") || scheme.is_match(token) {
            continue;
        }
        let ext = extension.captures(token).map(|c| c[1].to_lowercase());
        let has_letter = token.chars().any(|c| c.is_ascii_alphabetic());
        let path_like = (token.contains('/') || token.contains('\\')) && has_letter && !token.chars().all(|c| c == '/');
        let named = ext.as_deref().is_some_and(|e| {
            FILE_EXTENSIONS.contains(&e) && token[..token.len() - e.len() - 1].chars().any(|c| c.is_ascii_alphabetic() || c == '_')
        });
        if (path_like || named || dotfile.is_match(token)) && !found.iter().any(|f| f == token) {
            found.push(token.to_owned());
        }
    }
    found
}

/// "src/net.ts changed since", "a.ts, b.ts and c.ts changed since".
pub fn out_of_date(files: &[String]) -> String {
    let named = match files {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    format!("{named} changed since")
}

/// `cited` under `folder`, resolved without touching the disk; `None` when it leaves the folder or is the folder.
fn inside(folder: &Path, cited: &str) -> Option<PathBuf> {
    let joined = folder.join(cited.replace('\\', "/"));
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    (out.starts_with(folder) && out != folder).then_some(out)
}

/// The files `text` names, inside `folder`, saved after `written_at`. A path that leaves the folder, or does not exist
/// there, says nothing; nor does a date that does not read.
pub fn changed_since(folder: &Path, text: &str, written_at: &str) -> Vec<String> {
    let Ok(written) = chrono::DateTime::parse_from_rfc3339(written_at) else {
        return Vec::new();
    };
    let after = written.timestamp_millis() + GRACE_MS;
    cited_paths(text)
        .into_iter()
        .take(MAX_CITED)
        .filter(|cited| {
            inside(folder, cited)
                .and_then(|full| std::fs::metadata(full).ok())
                .and_then(|m| m.modified().ok())
                .map(chrono::DateTime::<chrono::Utc>::from)
                .is_some_and(|saved| saved.timestamp_millis() > after)
        })
        .collect()
}

/// When a memory's words were last written: a rewrite is dated by the rewrite.
pub fn last_written(memory: &Value) -> Option<&str> {
    memory.get("updatedAt").and_then(Value::as_str).or_else(|| memory["createdAt"].as_str())
}

/// Oldest first by last write (`byLastWritten`), so "the end of the list is the newest" holds for a memory rewritten in
/// place too. Stable: memories written at the same moment keep the store's own order. A date that does not read sorts
/// as the oldest.
pub fn by_last_written(mut rows: Vec<Value>) -> Vec<Value> {
    rows.sort_by_key(|m| {
        last_written(m)
            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
            .map_or(i64::MIN, |at| at.timestamp_millis())
    });
    rows
}

/// Who wrote the words a teammate will read (A1.6): the rewrite's writer, else who first kept it.
pub fn writer(memory: &Value) -> &str {
    memory["updatedBy"]["name"]
        .as_str()
        .filter(|n| !n.trim().is_empty())
        .or_else(|| memory["by"]["name"].as_str())
        .or_else(|| memory["by"].as_str())
        .unwrap_or("a conversation")
}

fn strings(value: &Value) -> Vec<String> {
    value.as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect()
}

/// The whole list, as the file a teammate reads: this folder's, then everywhere's, newest last, each with its id.
pub fn memory_file_text(rows: &[Value], now: chrono::DateTime<chrono::Utc>) -> String {
    let line = |memory: &Value| -> String {
        let by = match writer(memory) {
            "you" => "the person",
            other => other,
        };
        let when = last_written(memory)
            .filter(|at| at.len() >= 10)
            .map(|at| match crate::people::memory_age(at, now) {
                Some(age) => format!(" -- {} ({age})", &at[..10]),
                None => format!(" -- {}", &at[..10]),
            })
            .unwrap_or_default();
        let changed = strings(&memory["changedSince"]);
        let stale = if changed.is_empty() { String::new() } else { format!(" -- may be out of date: {}", out_of_date(&changed)) };
        let unused = memory["unusedDays"].as_i64().map(|d| format!(" -- not given to a teammate in {d} days")).unwrap_or_default();
        let id = memory["memoryId"].as_str().map(|id| format!(" [{id}]")).unwrap_or_default();
        format!(
            "- {} (by {by}{when}{stale}{unused}){id}",
            crate::protocol_tags::defang_protocol_blocks(memory["text"].as_str().unwrap_or(""))
        )
    };
    let listed = |group: Vec<&Value>| {
        if group.is_empty() { "(nothing yet)".to_owned() } else { group.into_iter().map(line).collect::<Vec<_>>().join("\n") }
    };
    let here: Vec<&Value> = rows.iter().filter(|m| m["scope"] != "global").collect();
    let everywhere: Vec<&Value> = rows.iter().filter(|m| m["scope"] == "global").collect();
    let (h, e) = (here.len(), everywhere.len());
    [
        "# Team memory".to_owned(),
        String::new(),
        "Written by Locust before each run, newest last. Read-only: to change memory, use the <locust-memory> block in your reply. The id in square brackets names each memory exactly.".to_owned(),
        String::new(),
        format!("## This folder ({h})"),
        String::new(),
        listed(here),
        String::new(),
        format!("## Everywhere ({e})"),
        String::new(),
        listed(everywhere),
        String::new(),
    ]
    .join("\n")
}

/// Write the file under `folder` when it changed, and keep `.locust/` out of git there (best effort, and only where
/// there is a repository). Answers the path the brief should name, or `None` when the folder could not be written --
/// the brief then pastes as before and names no file.
pub fn write_memory_file(folder: &Path, text: &str) -> Option<&'static str> {
    let path = folder.join(MEMORY_FILE_IN_FOLDER);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text) {
        std::fs::create_dir_all(folder.join(".locust")).ok()?;
        std::fs::write(&path, text).ok()?;
    }
    if folder.join(".git").join("HEAD").is_file() {
        let info = folder.join(".git").join("info");
        let exclude = info.join("exclude");
        let current = std::fs::read_to_string(&exclude).unwrap_or_default();
        if !current.lines().any(|l| matches!(l.trim(), ".locust/" | "/.locust/" | ".locust")) {
            let sep = if !current.is_empty() && !current.ends_with('\n') { "\n" } else { "" };
            let _ = std::fs::create_dir_all(&info).and_then(|_| std::fs::write(&exclude, format!("{current}{sep}.locust/\n")));
        }
    }
    Some(MEMORY_FILE_IN_FOLDER)
}

/// Whether the file under `folder` is exactly `text`: the brief names the file only when it is there and current.
pub fn memory_file_is(folder: &Path, text: &str) -> bool {
    std::fs::read_to_string(folder.join(MEMORY_FILE_IN_FOLDER)).is_ok_and(|held| held == text)
}

/// Memory switched off: a file written while it was on is rewritten to say so, so a teammate cannot read the old list
/// there. Only one that exists -- a folder that never had the file gets none. Answers whether it changed anything.
pub fn retire_memory_file(folder: &Path) -> bool {
    let path = folder.join(MEMORY_FILE_IN_FOLDER);
    match std::fs::read_to_string(&path) {
        Ok(current) if current != MEMORY_OFF_TEXT => std::fs::write(&path, MEMORY_OFF_TEXT).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "memory_rewritten_case_tests.rs"]
mod memory_rewritten_case_tests;

#[cfg(test)]
#[path = "memory_file_case_tests.rs"]
mod memory_file_case_tests;
