//! The three read-only tools offered through a call's private server.

use crate::{door_tools::ToolResult, page_preview, workspace_files};
use serde_json::Value;
use std::{
    fs::File,
    io::{self, Read},
    path::{Component, Path},
};

const SEARCH_FILES: usize = 3_000;
const SEARCH_BYTES: u64 = 1024 * 1024;
const READ_BYTES: u64 = 256 * 1024;
const MATCHES: usize = 60;

pub const TOOLS: &[&str] = &["find_files", "read_file", "search_files"];

fn detail<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn excluded(path: &Path) -> bool {
    page_preview::never_served(path)
        || path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("credentials"))
}

pub fn call(root: Option<&Path>, name: &str, args: &Value) -> ToolResult {
    let Some(root) = root else {
        return ToolResult::refused("No folder is open in Locust.");
    };
    let result = match name {
        "find_files" => Ok(find(root, detail(args, "query"))),
        "read_file" => read_file(root, args),
        "search_files" => search(root, args),
        _ => Err("A voice call cannot do that.".into()),
    };
    match result {
        Ok(text) => ToolResult::ok(text),
        Err(refusal) => ToolResult::refused(refusal),
    }
}

fn find(root: &Path, query: Option<&str>) -> String {
    let (paths, partial) = workspace_files::list_without_git(root);
    let query = query.map(str::to_lowercase);
    let found: Vec<_> = paths
        .into_iter()
        .filter(|path| {
            !excluded(Path::new(path))
                && query
                    .as_ref()
                    .is_none_or(|query| path.to_lowercase().contains(query))
        })
        .collect();
    if found.is_empty() {
        return query.map_or_else(
            || "The folder has no files.".into(),
            |query| format!("No file's path has \"{query}\" in it."),
        );
    }
    let shown = found.len().min(200);
    let mut words = found[..shown].join("\n");
    if shown < found.len() || partial {
        let more = if partial { "more than " } else { "" };
        words.push_str(&format!(
            "\n(The first {shown} of {more}{}; ask with a query to narrow it.)",
            found.len()
        ));
    }
    words
}

fn open(root: &File, path: &Path) -> io::Result<File> {
    let file = page_preview::open_beneath(root, path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Not a file"));
    }
    Ok(file)
}

fn head(file: File, bytes: u64) -> Option<String> {
    let mut data = Vec::new();
    file.take(bytes).read_to_end(&mut data).ok()?;
    if data.iter().take(8_192).any(|&byte| byte == 0) {
        return None;
    }
    // Lossy conversion would read arbitrary binary bytes aloud. Allow a cut in the last UTF-8 code point.
    match std::str::from_utf8(&data) {
        Ok(text) => Some(text.into()),
        Err(error) if data.len() as u64 == bytes && error.error_len().is_none() => {
            data.truncate(error.valid_up_to());
            String::from_utf8(data).ok()
        }
        Err(_) => None,
    }
}

fn read_file(root: &Path, args: &Value) -> Result<String, String> {
    let asked = detail(args, "path").ok_or("Say which file, as its path in the folder.")?;
    let path = Path::new(asked);
    if path.is_absolute() || asked.starts_with('\\') || asked.as_bytes().get(1) == Some(&b':') {
        return Err("Give the path inside the folder, like src/app.ts.".into());
    }
    if !path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("That is outside the folder, so a call does not read it.".into());
    }
    if excluded(path) {
        return Err("That file holds secrets, so a call never reads it.".into());
    }
    let folder = File::open(root).map_err(|_| "No folder is open in Locust.")?;
    let file = open(&folder, path).map_err(|_| format!("There is no {asked} in the folder."))?;
    let size = file
        .metadata()
        .map_err(|_| format!("{asked} is not a file."))?
        .len();
    let body = head(file, READ_BYTES)
        .ok_or_else(|| format!("{asked} is not text, so a call cannot read it out."))?;
    let lines: Vec<_> = body
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let from = args["from_line"].as_f64().unwrap_or(1.0).max(1.0) as usize;
    if from > lines.len() {
        return Err(format!("{asked} has {} lines.", lines.len()));
    }
    let to = lines.len().min(from.saturating_add(399));
    let cut = if size > READ_BYTES {
        format!(
            " (only the first 256 KB of {} KB are read)",
            (size + 512) / 1024
        )
    } else {
        String::new()
    };
    let rest = if to < lines.len() {
        format!(" Ask again with from_line {} for more.", to + 1)
    } else {
        String::new()
    };
    Ok(format!(
        "{asked}, lines {from}-{to} of {}{cut}.{rest}\n\n{}",
        lines.len(),
        lines[from - 1..to].join("\n")
    ))
}

fn search(root: &Path, args: &Value) -> Result<String, String> {
    let wanted = detail(args, "text").ok_or("Say what to look for.")?;
    let folder = File::open(root).map_err(|_| "No folder is open in Locust.")?;
    let needle = wanted.to_lowercase();
    let (paths, _) = workspace_files::list_without_git(root);
    let mut found = Vec::new();
    let mut searched = 0;
    for path in paths {
        if found.len() == MATCHES || searched == SEARCH_FILES {
            break;
        }
        if excluded(Path::new(&path)) {
            continue;
        }
        let Ok(file) = open(&folder, Path::new(&path)) else {
            continue;
        };
        if file.metadata().is_ok_and(|info| info.len() > SEARCH_BYTES) {
            continue;
        }
        searched += 1;
        if let Some(body) = head(file, SEARCH_BYTES) {
            for (index, line) in body.lines().enumerate() {
                if line.to_lowercase().contains(&needle) {
                    let excerpt: String = line.trim().chars().take(200).collect();
                    found.push(format!("{path}:{}: {excerpt}", index + 1));
                    if found.len() == MATCHES {
                        break;
                    }
                }
            }
        }
    }
    if found.is_empty() {
        return Ok(format!(
            "\"{wanted}\" is in none of the {searched} files searched."
        ));
    }
    if found.len() == MATCHES {
        found.push(format!("(The first {MATCHES} matches.)"));
    }
    Ok(found.join("\n"))
}

#[cfg(all(test, unix))]
#[path = "voice_call_folder_tests.rs"]
mod tests;
