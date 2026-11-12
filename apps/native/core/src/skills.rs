//! SKILLS FOR A RUN (main/claude-skills.ts and main/run-skills.ts).
//!
//! A turn's agent gets the skills it is meant to have, copied for that run alone, never linked (a removal that
//! followed a link would delete the person's own):
//! - Claude Code, as up to four plugins: the folder's own `.claude/skills` (not in Auto), the person's own
//!   `~/.claude/skills` (only with "Lend Claude Code your own skills" on, not in Auto), the skills kept from GitHub,
//!   and the teammate's own. `--restricted` drops Claude Code's usual skill folders, and only `--plugin-dir` puts them
//!   back.
//! - Codex, OpenCode, Copilot, Antigravity and Cursor, as one folder: the teammate's own, then the library's, a
//!   repeated name dropped. Each finds them its own way (`skills/extraRoots/set`, `skills.paths`, `--plugin-dir`,
//!   `--add-dir`).
//!
//! The teammate's skills and the library are read from Electron's data folder, read only: the native app does not
//! keep its own yet.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SKILL_FILE: &str = "SKILL.md";
const MOST_BYTES_A_SKILL: u64 = 10 * 1024 * 1024;
const MOST_BYTES_ALL: u64 = 40 * 1024 * 1024;
const MOST_SKILLS: usize = 200;

/// The bytes under a folder, symlinks not followed, or None past `limit`.
fn size_within(path: &Path, limit: u64) -> Option<u64> {
    let mut total = 0;
    let mut pending = vec![path.to_path_buf()];
    while let Some(at) = pending.pop() {
        for entry in std::fs::read_dir(&at).ok()?.flatten() {
            let kind = entry.file_type().ok()?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata().ok()?.len();
            }
            if total > limit {
                return None;
            }
        }
    }
    Some(total)
}

/// skillFoldersIn: the skill folders under `root`, by name: each a folder with a `SKILL.md`, within the size limits.
pub fn skill_folders_in(root: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(root) else { return Vec::new() };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
    let mut found = Vec::new();
    let mut total = 0;
    for entry in entries {
        if found.len() >= MOST_SKILLS {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type().is_ok_and(|t| t.is_dir()) || name.starts_with('.') {
            continue;
        }
        if !crate::invariants::regex(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$").is_match(&name) {
            continue;
        }
        let folder = entry.path();
        if !std::fs::symlink_metadata(folder.join(SKILL_FILE)).is_ok_and(|m| m.is_file()) {
            continue;
        }
        let Some(size) = size_within(&folder, MOST_BYTES_A_SKILL) else { continue };
        if total + size > MOST_BYTES_ALL {
            continue;
        }
        total += size;
        found.push(name);
    }
    found
}

/// skillNameIn: the `name:` a skill's front matter gives itself.
pub fn skill_name_in(text: &str) -> Option<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let front = crate::invariants::regex(r"(?s)^---\r?\n(.*?)\r?\n---(?:\r?\n|$)").captures(text)?;
    for line in front[1].lines() {
        if let Some(rest) = line.strip_prefix("name:") {
            let value = rest.trim().trim_matches(['"', '\'']).trim();
            return (!value.is_empty()).then(|| value.to_owned());
        }
    }
    None
}

/// Copies a folder, skipping symlinks anywhere inside it.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn fresh(scratch: &Path) -> PathBuf {
    scratch.join(format!("{}-{}", std::process::id(), crate::invariants::unique_nanos()))
}

/// One Claude Code plugin made for a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Plugin {
    pub name: &'static str,
    pub dir: PathBuf,
    /// `plugin:skill`, as Claude Code names them.
    pub skills: Vec<String>,
}

fn build_plugin(base: &Path, name: &'static str, from: Option<&Path>, only: Option<&[String]>) -> Option<Plugin> {
    let names: Vec<String> = from.map(skill_folders_in).unwrap_or_default().into_iter().filter(|s| only.is_none_or(|o| o.contains(s))).collect();
    if names.is_empty() {
        return None;
    }
    let description = match name {
        "project" => "This folder's own skills",
        "personal" => "Your own Claude Code skills",
        "library" => "Skills you kept from GitHub",
        _ => "This teammate's own skills",
    };
    let dir = base.join(name);
    std::fs::create_dir_all(dir.join(".claude-plugin")).ok()?;
    let manifest = serde_json::json!({"name": name, "description": description, "version": "1.0.0"});
    std::fs::write(dir.join(".claude-plugin/plugin.json"), format!("{manifest}\n")).ok()?;
    let mut copied = Vec::new();
    for skill in names {
        if from.is_some_and(|f| copy_dir(&f.join(&skill), &dir.join("skills").join(&skill)).is_ok()) {
            copied.push(format!("{name}:{skill}"));
        }
    }
    (!copied.is_empty()).then_some(Plugin { name, dir, skills: copied })
}

/// Where a run's skills come from.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    pub library: Option<PathBuf>,
    pub teammate_folder: Option<PathBuf>,
    /// The names the teammate's record keeps; a folder's other skills are not its.
    pub teammate_skills: Vec<String>,
}

/// prepareClaudeSkills: the plugins a Claude Code run gets, in Electron's order. Empty when there are none.
pub fn prepare_claude(scratch: &Path, workspace: &Path, own_skills: bool, home: Option<&Path>, auto: bool, sources: &Sources) -> Vec<Plugin> {
    let base = fresh(scratch);
    let mut plugins = Vec::new();
    if !auto && let Some(p) = build_plugin(&base, "project", Some(&workspace.join(".claude/skills")), None) {
        plugins.push(p);
    }
    if own_skills
        && !auto
        && let Some(home) = home
        && let Some(p) = build_plugin(&base, "personal", Some(&home.join(".claude/skills")), None)
    {
        plugins.push(p);
    }
    if let Some(library) = &sources.library
        && let Some(p) = build_plugin(&base, "library", Some(library), None)
    {
        plugins.push(p);
    }
    if let Some(folder) = &sources.teammate_folder
        && let Some(p) = build_plugin(&base, "teammate", Some(folder), Some(&sources.teammate_skills))
    {
        plugins.push(p);
    }
    if plugins.is_empty() {
        let _ = std::fs::remove_dir_all(&base);
    }
    plugins
}

/// The folder a Codex, OpenCode, Copilot, Antigravity or Cursor run gets.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSkills {
    pub dir: PathBuf,
    pub skills_dir: PathBuf,
    pub skills: Vec<String>,
}

/// prepareRunSkills: the teammate's own skills, then the library's, a name already taken (by folder or by the name its
/// front matter gives) dropped; with the manifests Copilot and Cursor read and the copy Antigravity reads. None when
/// there are none.
pub fn prepare_run(scratch: &Path, sources: &Sources) -> Option<RunSkills> {
    let dir = fresh(scratch);
    let skills_dir = dir.join("skills");
    std::fs::create_dir_all(&skills_dir).ok()?;
    // A disk that folds case folds skill names too.
    let folds = std::fs::metadata(dir.join("SKILLS")).is_ok();
    let key = |n: &str| if folds { n.to_lowercase() } else { n.to_owned() };
    let mut taken: Vec<String> = Vec::new();
    let mut names: HashSet<String> = HashSet::new();
    let mut destinations: HashSet<String> = HashSet::new();
    let mut copy = |from: Option<&Path>, only: Option<&[String]>| {
        let Some(from) = from else { return };
        for name in skill_folders_in(from) {
            if destinations.contains(&key(&name)) || only.is_some_and(|o| !o.contains(&name)) {
                continue;
            }
            let staging = dir.join(format!(".copy-{}", crate::invariants::unique_nanos()));
            if copy_dir(&from.join(&name), &staging).is_ok() {
                let identity = std::fs::read_to_string(staging.join(SKILL_FILE)).ok().and_then(|t| skill_name_in(&t)).unwrap_or_else(|| name.clone());
                if !names.contains(&key(&identity)) && std::fs::rename(&staging, skills_dir.join(&name)).is_ok() {
                    taken.push(name.clone());
                    destinations.insert(key(&name));
                    names.insert(key(&identity));
                }
            }
            let _ = std::fs::remove_dir_all(&staging);
        }
    };
    copy(sources.teammate_folder.as_deref(), Some(&sources.teammate_skills));
    copy(sources.library.as_deref(), None);
    if taken.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        return None;
    }
    let manifest = serde_json::json!({"name": "locust-run-skills", "description": "Skills for this run.", "version": "0.0.1", "skills": "skills"});
    let wrote = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(dir.join(".plugin"))?;
        std::fs::write(dir.join(".plugin/plugin.json"), format!("{manifest}\n"))?;
        std::fs::create_dir_all(dir.join(".cursor-plugin"))?;
        std::fs::copy(dir.join(".plugin/plugin.json"), dir.join(".cursor-plugin/plugin.json"))?;
        copy_dir(&skills_dir, &dir.join(".agents/skills"))
    })();
    if wrote.is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        return None;
    }
    Some(RunSkills { dir, skills_dir, skills: taken })
}

/// The skills Electron keeps for a teammate (`teammate-skills/<id>`, the names in `teammate-skills/skills.json`) and
/// the library (`skill-library/skills`), read from Electron's data folder.
pub fn sources_for(electron: &Path, teammate: &str) -> Sources {
    let root = electron.join("teammate-skills");
    let names: Vec<String> = std::fs::read_to_string(root.join("skills.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .map(|v| {
            v["skills"].as_array().into_iter().flatten().filter(|s| s["teammateId"] == teammate).filter_map(|s| s["name"].as_str().map(str::to_owned)).collect()
        })
        .unwrap_or_default();
    let library = electron.join("skill-library/skills");
    Sources {
        library: library.is_dir().then_some(library),
        teammate_folder: (!names.is_empty()).then(|| root.join(teammate)),
        teammate_skills: names,
    }
}

/// Codex is told its run's skill folder at the handshake (`skills/extraRoots/set`), by the turn's mission id.
static ROOTS: Mutex<Option<HashMap<String, Vec<String>>>> = Mutex::new(None);

pub fn set_roots(mission: &str, roots: Vec<String>) {
    crate::invariants::lock(&ROOTS).get_or_insert_with(HashMap::new).insert(mission.to_owned(), roots);
}

pub fn roots(mission: &str) -> Vec<String> {
    crate::invariants::lock(&ROOTS).as_ref().and_then(|m| m.get(mission).cloned()).unwrap_or_default()
}

pub fn forget_roots(mission: &str) {
    if let Some(m) = crate::invariants::lock(&ROOTS).as_mut() {
        m.remove(mission);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("locust-skills-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn skill(root: &Path, folder: &str, name: &str) {
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::write(root.join(folder).join("SKILL.md"), format!("---\nname: {name}\ndescription: x\n---\nDo it.\n")).unwrap();
    }

    #[test]
    fn a_run_gets_the_teammates_skills_then_the_librarys_without_repeats() {
        let d = temp();
        let (mate, library) = (d.join("mate"), d.join("library"));
        skill(&mate, "review", "review");
        skill(&mate, "unlisted", "unlisted");
        skill(&library, "review", "review");
        skill(&library, "deploy", "deploy");
        skill(&library, "renamed", "deploy");
        std::fs::create_dir_all(library.join("not-a-skill")).unwrap();
        let sources = Sources { library: Some(library), teammate_folder: Some(mate), teammate_skills: vec!["review".into()] };
        let run = prepare_run(&d.join("scratch"), &sources).unwrap();
        assert_eq!(run.skills, ["review", "deploy"], "the teammate's listed skill first, the library's after, a repeat name dropped");
        assert!(run.dir.join(".plugin/plugin.json").is_file() && run.dir.join(".cursor-plugin/plugin.json").is_file());
        assert!(run.dir.join(".agents/skills/deploy/SKILL.md").is_file());
        assert_eq!(prepare_run(&d.join("scratch"), &Sources::default()), None);
    }

    #[test]
    fn claude_gets_plugins_in_electrons_order_and_none_from_the_folder_in_auto() {
        let d = temp();
        let work = d.join("work");
        skill(&work.join(".claude/skills"), "lint", "lint");
        let home = d.join("home");
        skill(&home.join(".claude/skills"), "mine", "mine");
        let plugins = prepare_claude(&d.join("scratch"), &work, true, Some(&home), false, &Sources::default());
        assert_eq!(plugins.iter().map(|p| p.name).collect::<Vec<_>>(), ["project", "personal"]);
        assert_eq!(plugins[0].skills, ["project:lint"]);
        assert!(plugins[0].dir.join(".claude-plugin/plugin.json").is_file());
        assert!(prepare_claude(&d.join("scratch"), &work, true, Some(&home), true, &Sources::default()).is_empty());
        assert_eq!(prepare_claude(&d.join("scratch"), &work, false, Some(&home), false, &Sources::default()).len(), 1, "the person's own only when lent");
    }

    #[test]
    fn a_skill_names_itself_in_its_front_matter() {
        assert_eq!(skill_name_in("---\nname: 'Deploy'\n---\nx").as_deref(), Some("Deploy"));
        assert_eq!(skill_name_in("no front matter"), None);
    }
}
