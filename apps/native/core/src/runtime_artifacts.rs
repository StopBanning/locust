//! What a person has already set up inside the coding-agent CLIs themselves — port of `main/runtime-artifacts.ts`.
//!
//! Locust does not create these and cannot run them. They are the agents, commands and automations someone configured
//! in Claude Code, Codex or Cursor directly, and the app was silent about all of it -- so a person with three Codex
//! agents and three Claude commands read "Nothing saved yet" (Colin, 2026-09-07). The brief is to LIST them.
//!
//! Nothing here executes anything, and only the front matter is read -- never the instruction body, which is the
//! person's own prose and none of the app's business.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeArtifact {
    /// Which CLI it belongs to.
    pub runtime: String,
    /// `agent`, `command` or `automation`.
    pub kind: &'static str,
    pub name: String,
    pub description: Option<String>,
    /// Where it lives, so the app can say where to go and edit it.
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub name: String,
    pub description: Option<String>,
}

/// How many of each kind to read, so a large folder cannot stall a screen.
const MAX_PER_LOCATION: usize = 40;
/// Front matter is short; a file bigger than this is read only at its head.
const HEAD_BYTES: usize = 4096;

pub struct Location {
    pub runtime: &'static str,
    pub kind: &'static str,
    /// Relative to the person's home directory.
    pub dir: &'static [&'static str],
    pub extensions: &'static [&'static str],
}

/// Where each CLI keeps them, measured on a real machine 2026-09-07. A fixed list rather than a search: these are the
/// paths the tools document for themselves, and hunting the home directory would read files nobody asked us to read.
pub const ARTIFACT_LOCATIONS: &[Location] = &[
    Location { runtime: "claude", kind: "agent", dir: &[".claude", "agents"], extensions: &["md"] },
    Location { runtime: "claude", kind: "command", dir: &[".claude", "commands"], extensions: &["md"] },
    Location { runtime: "codex", kind: "agent", dir: &[".codex", "agents"], extensions: &["toml"] },
    Location { runtime: "codex", kind: "automation", dir: &[".codex", "automations"], extensions: &["toml", "md", "json"] },
    Location { runtime: "cursor", kind: "agent", dir: &[".cursor", "agents"], extensions: &["md", "json", "toml"] },
];

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    let trimmed = trimmed.strip_suffix(',').unwrap_or(trimmed).trim();
    let bytes = trimmed.as_bytes();
    let quoted = bytes.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') && bytes[bytes.len() - 1] == bytes[0];
    if quoted { trimmed[1..trimmed.len() - 1].trim().to_owned() } else { trimmed.to_owned() }
}

/// The file's name without its last extension (`path.basename(name, path.extname(name))`).
fn stem(file_name: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    match base.rfind('.') {
        Some(dot) if dot > 0 => base[..dot].to_owned(),
        _ => base.to_owned(),
    }
}

/// The extension with its dot dropped, as `path.extname` finds it (`.bashrc` has none).
fn extension(file_name: &str) -> Option<String> {
    let dot = file_name.rfind('.')?;
    (dot > 0).then(|| file_name[dot + 1..].to_lowercase())
}

/// The name and one-line description, from the front matter each format uses: Claude writes YAML between `---` fences,
/// Codex writes TOML key/value at the top of the file. A file that states neither is still listed under its own
/// filename -- it exists, and saying so is the point.
pub fn parse_artifact(file_name: &str, contents: &str) -> Parsed {
    let fallback = stem(file_name);
    let mut cut = contents.len().min(HEAD_BYTES);
    while !contents.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &contents[..cut];
    let fence = regex::Regex::new(r"^---\r?\n([\s\S]*?)\r?\n---").expect("fence pattern");
    let body = fence.captures(head).and_then(|c| c.get(1)).map_or(head, |m| m.as_str());
    let field = |key: &str| -> Option<String> {
        // YAML `key: value` and TOML `key = value`, one line, first wins.
        let found = regex::Regex::new(&format!(r"(?im)^{key}\s*[:=]\s*(.+)$")).expect("field pattern");
        let value = found.captures(body).and_then(|c| c.get(1)).map(|m| unquote(m.as_str())).unwrap_or_default();
        (!value.is_empty()).then_some(value)
    };
    Parsed { name: field("name").unwrap_or(fallback), description: field("description") }
}

/// Read one location. A directory that is not there is not an error: this person has not set any up.
fn read_location(home: &Path, location: &Location) -> Vec<RuntimeArtifact> {
    let dir = location.dir.iter().fold(home.to_path_buf(), |dir, part| dir.join(part));
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut wanted: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| extension(name).is_some_and(|ext| location.extensions.contains(&ext.as_str())))
        .collect();
    wanted.sort();
    wanted.truncate(MAX_PER_LOCATION);
    let mut found = Vec::new();
    for name in wanted {
        let path = dir.join(&name);
        // Unreadable is the same as absent for a list: say nothing rather than report a file the person cannot see.
        if !std::fs::metadata(&path).is_ok_and(|m| m.is_file()) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else { continue };
        let parsed = parse_artifact(&name, &contents);
        found.push(RuntimeArtifact {
            runtime: location.runtime.to_owned(),
            kind: location.kind,
            name: parsed.name,
            description: parsed.description,
            path,
        });
    }
    found
}

/// Everything the installed runtimes have. `installed` gates it so a leftover `.claude/agents` from an uninstall never
/// reads as a working teammate's routine. Signed out still counts as installed (index.ts): the agents they wrote are on
/// this machine whether or not the CLI can run right now.
pub fn read_runtime_artifacts(installed: &[&str], home: &Path) -> Vec<RuntimeArtifact> {
    ARTIFACT_LOCATIONS
        .iter()
        .filter(|location| installed.contains(&location.runtime))
        .flat_map(|location| read_location(home, location))
        .collect()
}

/// Which agents' lists are read, from discovery's answer (index.ts's RUNTIME_ARTIFACTS_CHANNEL): ready or signed out.
pub fn listed_runtimes(found: &[crate::agents::Found]) -> Vec<String> {
    use crate::agents::Probe;
    found.iter().filter(|f| matches!(f.status, Probe::Ready | Probe::AuthRequired)).map(|f| f.id.clone()).collect()
}

/// CliArtifacts.tsx `inventorySummary`: `3 agents · 1 command`, in the order the kinds first appear.
pub fn inventory_summary(artifacts: &[&RuntimeArtifact]) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for entry in artifacts {
        match counts.iter_mut().find(|(kind, _)| *kind == entry.kind) {
            Some((_, count)) => *count += 1,
            None => counts.push((entry.kind, 1)),
        }
    }
    counts.iter().map(|(kind, count)| if *count == 1 { format!("1 {kind}") } else { format!("{count} {kind}s") }).collect::<Vec<_>>().join(" \u{b7} ")
}

/// The fold's words under one agent (CliArtifacts.tsx's summary): `3 agents · 1 command of its own`.
pub fn fold_line(artifacts: &[&RuntimeArtifact]) -> String {
    format!("{} of its own", inventory_summary(artifacts))
}

/// The note inside the fold, Electron's word for word.
pub const LISTS_NOT_RUNS: &str = "Locust lists these, it does not run them";

/// The last two segments of where it lives (`agents/gig-scout.md`): the whole path runs off the row's edge.
pub fn path_tail(path: &Path) -> String {
    let whole = path.to_string_lossy();
    let parts: Vec<&str> = whole.split(['/', '\\']).collect();
    parts[parts.len().saturating_sub(2)..].join("/")
}

#[cfg(test)]
#[path = "runtime_artifacts_tests.rs"]
mod runtime_artifacts_tests;

#[cfg(test)]
#[path = "runtime_artifacts_fold_tests.rs"]
mod runtime_artifacts_fold_tests;
