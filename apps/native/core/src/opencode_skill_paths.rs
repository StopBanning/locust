//! THE PERSON'S OWN OPENCODE SKILL PATHS (main/opencode-skill-paths.ts, 0.816).
//!
//! A run's OpenCode 1 config (`OPENCODE_CONFIG_CONTENT`) names Locust's skills folder, and OpenCode keeps only the last
//! config's `skills.paths` (measured on 1.18.27), so on its own the run dropped the person's. The run now carries the
//! paths OpenCode itself would have used, then Locust's folder (`opencode_config::with_skills`).
//!
//! OpenCode's order, the last config that names `skills.paths` winning: the global folder's `config.json`,
//! `opencode.json`, `opencode.jsonc` (read once, first); `OPENCODE_CONFIG`; each folder's `opencode.json` then
//! `opencode.jsonc` from the repository's root down to the work; then the `.opencode` folders (the work's outward),
//! `~/.opencode` and `OPENCODE_CONFIG_DIR`; then `OPENCODE_CONFIG_CONTENT`. `OPENCODE_DISABLE_PROJECT_CONFIG` (`1` or
//! `true`) leaves out every folder's own. Configuration is read, never auth and never a `{file:...}` it names; no file
//! is written. The strings go as written: OpenCode resolves `~/`, a relative path and `{env:...}` the same from the
//! run's config as from a file.
//!
//! HOME and the environment are passed in, never read here: the run's own `Command` points `XDG_CONFIG_HOME` at
//! Locust's own OpenCode home, so the caller reads Locust's process environment, as Electron reads `process.env`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// JSONC as OpenCode reads it (jsonc-parser with trailing commas and empty content): `//` and `/* */` comments outside
/// strings, and a comma before `}` or `]`, taken out; then plain JSON. None when the text is not that.
pub fn parse_jsonc(text: &str) -> Option<serde_json::Value> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(bytes.len());
            out.push_str(text.get(start..i)?);
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let end = text[i + 2..].find("*/")?;
            i += 2 + end + 2;
            out.push(' ');
        } else {
            let next = text[i..].chars().next()?;
            out.push(next);
            i += next.len_utf8();
        }
    }
    // A trailing comma: a comma whose next non-space character closes the object or list. Strings are kept whole.
    let chars: Vec<char> = out.chars().collect();
    let mut clean = String::with_capacity(out.len());
    let mut in_string = false;
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if in_string {
            clean.push(c);
            if c == '\\' && k + 1 < chars.len() {
                clean.push(chars[k + 1]);
                k += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            clean.push(c);
        } else if c == ',' && chars[k + 1..].iter().find(|c| !c.is_whitespace()).is_some_and(|n| matches!(n, '}' | ']')) {
            // dropped
        } else {
            clean.push(c);
        }
        k += 1;
    }
    if clean.trim().is_empty() {
        return Some(serde_json::Value::Null);
    }
    serde_json::from_str(&clean).ok()
}

fn invalid(from: &str) -> String {
    format!("OpenCode’s settings in {from} aren’t valid JSON, so nothing was started. Fix them, then send again.")
}

/// The folders from `cwd` outward to the first that holds a `.git` (file or folder), CWD first (skillProjectFolders).
fn project_folders(cwd: &Path) -> Vec<PathBuf> {
    crate::teammate_setup::skill_project_folders(cwd)
}

/// openCodeSkillPaths: the `skills.paths` OpenCode would use in `cwd`, or the sentence that says which settings file is
/// not JSON (or cannot be read). Only non-empty strings, each once, in the list's own order.
pub fn opencode_skill_paths(cwd: &Path, home: &Path, env: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let var = |k: &str| env.get(k).filter(|v| !v.is_empty()).cloned();
    let global = var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config")).join("opencode");
    let folders = project_folders(cwd);
    let project_disabled = var("OPENCODE_DISABLE_PROJECT_CONFIG").is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let mut directories: Vec<PathBuf> = Vec::new();
    let dot_folders: Vec<PathBuf> = if project_disabled { Vec::new() } else { folders.iter().map(|f| f.join(".opencode")).collect() };
    for d in dot_folders.into_iter().chain([home.join(".opencode")]).chain(var("OPENCODE_CONFIG_DIR").map(PathBuf::from)) {
        if !directories.contains(&d) {
            directories.push(d);
        }
    }
    let mut files: Vec<PathBuf> = ["config.json", "opencode.json", "opencode.jsonc"].iter().map(|n| global.join(n)).collect();
    if let Some(explicit) = var("OPENCODE_CONFIG") {
        files.push(cwd.join(explicit));
    }
    if !project_disabled {
        for folder in folders.iter().rev() {
            files.extend(["opencode.json", "opencode.jsonc"].iter().map(|n| folder.join(n)));
        }
    }
    for folder in &directories {
        files.extend(["opencode.json", "opencode.jsonc"].iter().map(|n| folder.join(n)));
    }
    let mut paths: Vec<String> = Vec::new();
    let mut add = |text: &str, from: &str| -> Result<(), String> {
        let config = parse_jsonc(text).ok_or_else(|| invalid(from))?;
        // A later config's list takes the earlier one's place; a config that names none leaves it alone.
        if let Some(list) = config.get("skills").and_then(|s| s.get("paths")).and_then(|p| p.as_array()) {
            paths.clear();
            for path in list.iter().filter_map(|p| p.as_str()).filter(|p| !p.is_empty()) {
                if !paths.iter().any(|p| p == path) {
                    paths.push(path.to_owned());
                }
            }
        }
        Ok(())
    };
    for file in &files {
        match std::fs::read_to_string(file) {
            Ok(text) => add(&text, &file.display().to_string())?,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => {}
            Err(_) => return Err(format!("OpenCode’s settings file {} couldn’t be read, so nothing was started.", file.display())),
        }
    }
    if let Some(content) = var("OPENCODE_CONFIG_CONTENT") {
        add(&content, "OPENCODE_CONFIG_CONTENT")?;
    }
    Ok(paths)
}

/// The paths for a run in `cwd`, read from Locust's own HOME and environment.
pub fn for_this_process(cwd: &Path) -> Result<Vec<String>, String> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Ok(Vec::new()) };
    opencode_skill_paths(cwd, &home, &std::env::vars().collect())
}

#[cfg(test)]
#[path = "opencode_skill_paths_tests.rs"]
mod tests;
