//! Electron's `pathTitle.ts`: a first message that is only a path (0.849) is titled by the file it names.
//! Colin sends a handoff as its file's path and nothing else, so the sidebar rows all read "/Users/locust/work/..." and
//! the column cut off the one part that told them apart. Any other first line is the title as typed.

use regex::Regex;
use std::sync::LazyLock;

static PATH_ONLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:~|[A-Za-z]:)?[\\/]\S+$").unwrap());

pub fn path_only_title(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !PATH_ONLY.is_match(trimmed) {
        return None;
    }
    let parts: Vec<&str> = trimmed.split(['\\', '/']).filter(|part| !part.is_empty()).collect();
    if parts.len() >= 2 { parts.last().map(|part| (*part).to_owned()) } else { None }
}

#[cfg(test)]
#[path = "path_title_tests.rs"]
mod path_title_tests;
