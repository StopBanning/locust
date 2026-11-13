//! A TEAMMATE'S SETUP (`shared/teammate-setup.ts`, `main/teammate-setup.ts`).
//!
//! Gathered fresh from the same sources a teammate's brief and run read:
//! `LOCUST.md`, the runtime's own file, group instructions, the teammate's instructions,
//! About you, memory, skills, connectors, tune lessons, and what the last turn loaded.
//! Nothing here is written, and nothing is started.

use crate::people;
use crate::skills;
use crate::store::Store;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const WORKSPACE_BRIEF: &str = "LOCUST.md";
pub const MAX_BRIEF_LINES: usize = 200;
pub const MAX_BRIEF_BYTES: usize = 24 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderKind {
    Project,
    Own,
    Worktree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSetup {
    pub name: String,
    pub kind: FolderKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocustMdSetup {
    pub lines: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileSetup {
    pub name: String,
    pub lines: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSetup {
    pub name: String,
    pub characters: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySetup {
    pub on: bool,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalSkillsSetup {
    pub count: usize,
    pub lent: bool,
    pub found_itself: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsSetup {
    pub own: Vec<String>,
    pub project: usize,
    pub project_folders: Vec<String>,
    pub personal: PersonalSkillsSetup,
    pub library: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastTurnSetup {
    pub version: Option<String>,
    pub tools: Option<u64>,
    pub skills: Option<u64>,
    pub mcp_servers: Option<u64>,
    pub agents: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeammateSetup {
    pub teammate_id: String,
    pub folder: FolderSetup,
    pub locust_md: Option<LocustMdSetup>,
    pub runtime_file: Option<RuntimeFileSetup>,
    pub group: Option<GroupSetup>,
    pub instructions: usize,
    pub about_you: bool,
    pub memory: MemorySetup,
    pub skills: Option<SkillsSetup>,
    pub connectors: Option<Vec<String>>,
    pub lessons: usize,
    pub last_turn: Option<LastTurnSetup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupRow {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupSection {
    pub title: String,
    pub note: String,
    pub rows: Vec<SetupRow>,
}

pub fn runtime_file(runtime: &str) -> Option<&'static str> {
    match runtime {
        "claude" => Some("CLAUDE.md"),
        "codex" | "opencode" => Some("AGENTS.md"),
        "gemini" => Some("GEMINI.md"),
        _ => None,
    }
}

pub fn takes_skills(runtime: Option<&str>) -> bool {
    matches!(
        runtime,
        Some("claude" | "codex" | "opencode" | "copilot" | "antigravity" | "cursor")
    )
}

pub fn skill_folders(runtime: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match runtime {
        "cursor" => Some((
            &[".cursor/skills", ".agents/skills", ".claude/skills", ".codex/skills", ".grok/skills"],
            &[".cursor/skills", ".agents/skills", ".claude/skills", ".codex/skills", ".grok/skills"],
        )),
        "claude" => Some((&[".claude/skills"], &[".claude/skills"])),
        "codex" => Some((&[".agents/skills"], &[".agents/skills", ".codex/skills"])),
        "opencode" => Some((
            &[".opencode/skills", ".claude/skills", ".agents/skills"],
            &[".config/opencode/skills", ".claude/skills", ".agents/skills"],
        )),
        "copilot" => Some((
            &[".github/skills", ".agents/skills", ".claude/skills"],
            &[".copilot/skills", ".agents/skills"],
        )),
        "antigravity" => Some((
            &[".agents/skills"],
            &[".gemini/antigravity-cli/skills", ".gemini/config/skills"],
        )),
        _ => None,
    }
}

fn line_count(text: &str) -> usize {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.len()
}

fn is_runtime_tag(line: &str) -> Option<(String, bool)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('<') || !trimmed.ends_with('>') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let (closing, name) = if let Some(rest) = inner.strip_prefix('/') {
        (true, rest)
    } else {
        (false, inner)
    };
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "codex" | "claude" | "cursor" | "gemini" | "opencode" | "copilot" | "muse" | "antigravity"
    ) {
        Some((lower, closing))
    } else {
        None
    }
}

/// Filters `LOCUST.md` sections addressed to other runtimes (`briefForRuntime`).
pub fn brief_for_runtime(raw: &str, runtime: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut fenced = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        }
        let tag = if fenced { None } else { is_runtime_tag(line) };
        let Some((tag_name, closing)) = tag else {
            kept.push(line);
            i += 1;
            continue;
        };
        if closing {
            kept.push(line);
            i += 1;
            continue;
        }
        let mut close_at = None;
        let mut inner_fenced = false;
        for (ahead, candidate) in lines.iter().enumerate().skip(i + 1) {
            let ct = candidate.trim_start();
            if ct.starts_with("```") || ct.starts_with("~~~") {
                inner_fenced = !inner_fenced;
            }
            if !inner_fenced
                && let Some((name, is_close)) = is_runtime_tag(candidate)
                && is_close
                && name == tag_name
            {
                close_at = Some(ahead);
                break;
            }
        }
        let Some(close) = close_at else {
            kept.push(line);
            i += 1;
            continue;
        };
        let section = &lines[i + 1..close];
        if tag_name == runtime {
            kept.extend_from_slice(section);
        }
        for inside in section {
            let it = inside.trim_start();
            if it.starts_with("```") || it.starts_with("~~~") {
                fenced = !fenced;
            }
        }
        i = close + 1;
    }
    kept.join("\n")
}

/// Reads and bounds `LOCUST.md` (`readWorkspaceBrief`).
pub fn read_workspace_brief(folder: &Path, runtime: Option<&str>) -> Option<LocustMdSetup> {
    let raw = std::fs::read_to_string(folder.join(WORKSPACE_BRIEF)).ok()?;
    let filtered = match runtime {
        Some(r) => brief_for_runtime(&raw, r),
        None => raw,
    };
    if filtered.trim().is_empty() {
        return None;
    }
    let normalized = filtered.replace("\r\n", "\n").replace('\r', "\n");
    let mut all: Vec<&str> = normalized.split('\n').collect();
    while all.last().is_some_and(|l| l.is_empty()) {
        all.pop();
    }
    let mut truncated = all.len() > MAX_BRIEF_LINES;
    let mut kept = &all[..all.len().min(MAX_BRIEF_LINES)];
    let mut text = kept.join("\n");
    if text.len() > MAX_BRIEF_BYTES {
        truncated = true;
        while !kept.is_empty() && kept.join("\n").len() > MAX_BRIEF_BYTES {
            kept = &kept[..kept.len() - 1];
        }
        text = kept.join("\n");
    }
    let _ = text;
    Some(LocustMdSetup {
        lines: kept.len(),
        truncated,
    })
}

/// CWD first, stopping at its `.git` file or directory (`skillProjectFolders`).
pub fn skill_project_folders(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let start = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    let mut at = start.as_path();
    loop {
        out.push(at.to_path_buf());
        if std::fs::symlink_metadata(at.join(".git")).is_ok() {
            break;
        }
        let Some(parent) = at.parent() else { break };
        if parent == at {
            break;
        }
        at = parent;
    }
    out
}

/// Counts unique skill names under `roots`, following symlinks/junctions while avoiding cycles.
pub fn counted_skill_names(roots: &[PathBuf]) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut pending: Vec<(PathBuf, usize)> = roots.iter().map(|p| (p.clone(), 0)).collect();
    while let Some((next_path, depth)) = pending.pop() {
        let Ok(canon) = std::fs::canonicalize(&next_path) else {
            continue;
        };
        if !seen.insert(canon.clone()) {
            continue;
        }
        if let Ok(skill_md) = std::fs::read_to_string(canon.join("SKILL.md")) {
            let fallback = next_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_owned();
            let name = skills::skill_name_in(&skill_md).unwrap_or(fallback);
            if !name.is_empty() {
                names.insert(name);
            }
            continue;
        }
        if depth >= 6 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&canon) else {
            continue;
        };
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().into_owned();
            if fname.starts_with('.') || fname == "node_modules" {
                continue;
            }
            let child = entry.path();
            if std::fs::metadata(&child).is_ok_and(|m| m.is_dir()) {
                pending.push((child, depth + 1));
            }
        }
    }
    names
}

/// Parses `run.started` `evidence.raw` counts (`lastTurnOf`).
pub fn last_turn_of(events: &[Value]) -> Option<LastTurnSetup> {
    let started = events
        .iter()
        .find(|e| e.get("type").and_then(Value::as_str) == Some("run.started"))?;
    let raw = started
        .get("payload")
        .and_then(|p| p.get("evidence"))
        .and_then(|e| e.get("raw"))
        .and_then(Value::as_object)?;
    let num = |key: &str| -> Option<u64> {
        let v = raw.get(key)?;
        v.as_u64()
    };
    let version_re = crate::invariants::regex(r"^[\w.+-]{1,40}$");
    let version = raw
        .get("claude_code_version")
        .and_then(Value::as_str)
        .filter(|v| version_re.is_match(v))
        .map(str::to_owned);
    let tools = num("toolCount");
    let skills = num("skillCount");
    let mcp_servers = num("mcpServerCount");
    let agents = num("agentCount");
    if version.is_none()
        && tools.is_none()
        && skills.is_none()
        && mcp_servers.is_none()
        && agents.is_none()
    {
        return None;
    }
    Some(LastTurnSetup {
        version,
        tools,
        skills,
        mcp_servers,
        agents,
    })
}

pub struct SetupSources<'a> {
    pub teammate_id: &'a str,
    pub runtime: Option<&'a str>,
    pub mode: Option<&'a str>,
    pub instructions: Option<&'a str>,
    pub teammate_folder: Option<&'a Path>,
    pub worktree: bool,
    pub connectors: Vec<String>,
    pub project_path: &'a Path,
    pub group: Option<(String, String)>,
    pub about_you: Option<&'a str>,
    pub memory_mode: &'a str,
    pub claude_own_skills: bool,
    pub memory_count: usize,
    pub own_skills: Vec<String>,
    pub lessons: usize,
    pub library_folder: &'a Path,
    pub home_directory: &'a Path,
    pub last_turn_events: &'a [Value],
}

pub fn gather_teammate_setup(sources: &SetupSources<'_>) -> TeammateSetup {
    let runtime = sources.runtime;
    let folder_path = sources.teammate_folder.unwrap_or(sources.project_path);
    let folder_name = folder_path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| folder_path.to_string_lossy().into_owned());
    let folder_kind = if sources.worktree {
        FolderKind::Worktree
    } else if sources.teammate_folder.is_some() {
        FolderKind::Own
    } else {
        FolderKind::Project
    };
    let locust_md = read_workspace_brief(folder_path, runtime);
    let file_name = runtime.and_then(runtime_file);
    let runtime_file_setup = file_name.map(|name| {
        let lines = std::fs::read_to_string(folder_path.join(name))
            .ok()
            .map(|text| line_count(&text));
        RuntimeFileSetup {
            name: name.to_owned(),
            lines,
        }
    });
    let memory_on = sources.memory_mode != "off";
    let claude = runtime == Some("claude");
    let folders = runtime.filter(|r| takes_skills(Some(r))).and_then(skill_folders);
    let skills_setup = folders.map(|(project_rel, personal_rel)| {
        // Special case: restricted Claude gets copies from CWD only; Codex and OpenCode always search ancestors,
        // Claude only in Auto.
        let project_roots = if matches!(runtime, Some("codex" | "opencode"))
            || (claude && sources.mode == Some("auto"))
        {
            skill_project_folders(folder_path)
        } else {
            vec![folder_path.to_path_buf()]
        };
        let project_dirs: Vec<PathBuf> = project_roots
            .iter()
            .flat_map(|root| {
                project_rel
                    .iter()
                    .map(move |rel| rel.split('/').fold(root.clone(), |acc, p| acc.join(p)))
            })
            .collect();
        let personal_dirs: Vec<PathBuf> = personal_rel
            .iter()
            .map(|rel| {
                rel.split('/')
                    .fold(sources.home_directory.to_path_buf(), |acc, p| acc.join(p))
            })
            .collect();
        let personal_count = counted_skill_names(&personal_dirs).len();
        // Special case: Auto runs as the person, and Claude Code finds their own skills itself;
        // Codex and OpenCode always do.
        let lent = !claude || sources.claude_own_skills || sources.mode == Some("auto");
        SkillsSetup {
            own: sources.own_skills.clone(),
            project: counted_skill_names(&project_dirs).len(),
            project_folders: project_rel.iter().map(|s| (*s).to_owned()).collect(),
            personal: PersonalSkillsSetup {
                count: personal_count,
                lent,
                found_itself: !claude,
            },
            library: skills::skill_folders_in(sources.library_folder).len(),
        }
    });

    TeammateSetup {
        teammate_id: sources.teammate_id.to_owned(),
        folder: FolderSetup {
            name: folder_name,
            kind: folder_kind,
        },
        locust_md,
        runtime_file: runtime_file_setup,
        group: sources
            .group
            .as_ref()
            .filter(|(_, inst)| !inst.is_empty())
            .map(|(name, inst)| GroupSetup {
                name: name.clone(),
                characters: inst.encode_utf16().count(),
            }),
        instructions: sources
            .instructions
            .map(|s| s.encode_utf16().count())
            .unwrap_or(0),
        about_you: sources
            .about_you
            .is_some_and(|s| !s.trim().is_empty()),
        memory: MemorySetup {
            on: memory_on,
            count: if memory_on { sources.memory_count } else { 0 },
        },
        skills: skills_setup,
        connectors: claude.then(|| sources.connectors.clone()),
        lessons: sources.lessons,
        last_turn: last_turn_of(sources.last_turn_events),
    }
}

fn count_word(n: usize, one: &str, many: Option<&str>) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else if let Some(m) = many {
        format!("{n} {m}")
    } else {
        format!("{n} {one}s")
    }
}

fn listed(folders: &[String]) -> (String, String) {
    let join = |word: &str| -> String {
        if folders.len() < 2 {
            folders.first().cloned().unwrap_or_default()
        } else {
            let head = folders[..folders.len() - 1].join(", ");
            let tail = folders.last().map(String::as_str).unwrap_or("");
            format!("{head} {word} {tail}")
        }
    };
    (join("and"), join("or"))
}

/// The card's words, in the order a turn is given them (`setupSections`).
pub fn setup_sections(setup: &TeammateSetup, name: &str, runtime: &str) -> Vec<SetupSection> {
    let folder = match setup.folder.kind {
        FolderKind::Worktree => format!("A worktree of its own, from {}", setup.folder.name),
        FolderKind::Own => format!("Its own folder, {}", setup.folder.name),
        FolderKind::Project => format!("The project folder, {}", setup.folder.name),
    };
    let mut every = vec![
        SetupRow {
            label: "Folder".into(),
            value: folder,
        },
        SetupRow {
            label: "LOCUST.md".into(),
            value: match &setup.locust_md {
                None => "None in this folder".into(),
                Some(md) if md.truncated => {
                    format!("{} read; the rest is cut", count_word(md.lines, "line", None))
                }
                Some(md) => count_word(md.lines, "line", None),
            },
        },
    ];
    if let Some(rf) = &setup.runtime_file {
        every.push(SetupRow {
            label: rf.name.clone(),
            value: match rf.lines {
                None => "None in this folder".into(),
                Some(lines) => format!(
                    "{}, read by {runtime} itself",
                    count_word(lines, "line", None)
                ),
            },
        });
    }
    if let Some(group) = &setup.group {
        every.push(SetupRow {
            label: "Group".into(),
            value: format!(
                "{}’s instructions, {}",
                group.name,
                count_word(group.characters, "character", None)
            ),
        });
    }
    every.push(SetupRow {
        label: "Its instructions".into(),
        value: if setup.instructions == 0 {
            "None".into()
        } else {
            count_word(setup.instructions, "character", None)
        },
    });
    every.push(SetupRow {
        label: "About you".into(),
        value: if setup.about_you {
            "Your note, given to every teammate".into()
        } else {
            "None".into()
        },
    });
    every.push(SetupRow {
        label: "Team memory".into(),
        value: if setup.memory.on {
            format!(
                "On, {} in this folder",
                count_word(setup.memory.count, "memory", Some("memories"))
            )
        } else {
            "Off".into()
        },
    });

    let mut sections = vec![SetupSection {
        title: "Every turn".into(),
        note: format!("What {name} is given on every turn, in this order."),
        rows: every,
    }];

    if setup.skills.is_some() || setup.connectors.is_some() {
        let mut matched = Vec::new();
        if let Some(sk) = &setup.skills {
            matched.push(SetupRow {
                label: "Its own skills".into(),
                value: if sk.own.is_empty() {
                    "None yet. A tune can give it one.".into()
                } else {
                    sk.own.join(", ")
                },
            });
            let default_folders = vec![".claude/skills".to_owned()];
            let pf = if sk.project_folders.is_empty() {
                &default_folders
            } else {
                &sk.project_folders
            };
            let (and_list, or_list) = listed(pf);
            matched.push(SetupRow {
                label: "Project skills".into(),
                value: if sk.project == 0 {
                    format!("None in {or_list}")
                } else {
                    format!("{} in {and_list}", count_word(sk.project, "skill", None))
                },
            });
            matched.push(SetupRow {
                label: "Your skills".into(),
                value: if sk.personal.found_itself {
                    format!(
                        "{} in your skill folders, read by {runtime} itself",
                        count_word(sk.personal.count, "skill", None)
                    )
                } else if sk.personal.lent {
                    count_word(sk.personal.count, "skill", None)
                } else {
                    "Not lent; Settings can lend them".into()
                },
            });
            matched.push(SetupRow {
                label: "Kept from GitHub".into(),
                value: if sk.library == 0 {
                    "None".into()
                } else {
                    count_word(sk.library, "skill", None)
                },
            });
        }
        if let Some(connectors) = &setup.connectors {
            matched.push(SetupRow {
                label: "Connectors".into(),
                value: if connectors.is_empty() {
                    "Off".into()
                } else {
                    format!("On: {}", connectors.join(", "))
                },
            });
        }
        sections.push(SetupSection {
            title: "When the work matches".into(),
            note: format!("{runtime} loads a skill only when the work matches when to use it."),
            rows: matched,
        });
    }

    sections.push(SetupSection {
        title: "Only when you tune it".into(),
        note: "Read by the next /tune of this teammate, never by its everyday turns.".into(),
        rows: vec![SetupRow {
            label: "Tune lessons".into(),
            value: if setup.lessons == 0 {
                "None".into()
            } else {
                count_word(setup.lessons, "lesson", None)
            },
        }],
    });

    if let Some(last) = &setup.last_turn {
        let mut loaded = Vec::new();
        if let Some(tools) = last.tools {
            loaded.push(count_word(tools as usize, "tool", None));
        }
        if let Some(sk) = last.skills {
            loaded.push(count_word(sk as usize, "skill", None));
        }
        if let Some(mcp) = last.mcp_servers {
            loaded.push(count_word(mcp as usize, "connector server", None));
        }
        if let Some(agents) = last.agents {
            loaded.push(count_word(agents as usize, "agent", None));
        }
        let mut rows = Vec::new();
        if let Some(version) = &last.version {
            rows.push(SetupRow {
                label: "Version".into(),
                value: format!("{runtime} {version}"),
            });
        }
        if !loaded.is_empty() {
            rows.push(SetupRow {
                label: "Loaded".into(),
                value: loaded.join(", "),
            });
        }
        if !rows.is_empty() {
            sections.push(SetupSection {
                title: "Its last turn here".into(),
                note: format!(
                    "As {runtime} counted them when the turn started, its own built-in ones included."
                ),
                rows,
            });
        }
    }

    sections
}

/// High-level fresh reader for a teammate's setup from the native store and Electron's shared folders.
pub fn read_teammate_setup(
    store: &Store,
    electron_root: Option<&Path>,
    home_dir: &Path,
    project_path: &Path,
    teammate_id: &str,
    last_turn_events: &[Value],
) -> Result<TeammateSetup, String> {
    let team_json = people::team(store)?;
    let entry = team_json["teammates"]
        .as_array()
        .and_then(|arr| arr.iter().find(|t| t["teammateId"] == teammate_id))
        .ok_or_else(|| "This teammate’s setup could not be read. Nothing was changed.".to_owned())?;

    let runtime = entry["route"]["runtime"].as_str();
    let mode = entry["route"]["mode"].as_str();
    let instructions = entry["instructions"].as_str();
    let teammate_folder = entry["folder"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(Path::new);
    let worktree = entry["worktree"] == true;
    let connectors: Vec<String> = entry["connectors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();

    let about_you = team_json["settings"]["aboutYou"].as_str();
    let memory_mode = team_json["settings"]["memoryMode"]
        .as_str()
        .unwrap_or(crate::workspace_settings::DEFAULT_MEMORY_MODE);
    let claude_own_skills = team_json["settings"]["claudeOwnSkills"] == true;

    let effective_folder = teammate_folder.unwrap_or(project_path);
    let memory_count = if memory_mode == "off" {
        0
    } else {
        let ws_id = people::workspace_id(effective_folder);
        people::memories(store)
            .ok()
            .and_then(|m| {
                m["memories"].as_array().map(|arr| {
                    arr.iter()
                        .filter(|row| {
                            row["status"] == "kept"
                                && row["enabled"] == true
                                && (row["scope"] == "global" || row["workspaceId"] == ws_id)
                        })
                        .count()
                })
            })
            .unwrap_or(0)
    };

    let sources_skills = electron_root.map(|e| skills::sources_for(e, teammate_id));
    let own_skills = sources_skills
        .as_ref()
        .map(|s| s.teammate_skills.clone())
        .unwrap_or_default();
    let library_folder = sources_skills
        .as_ref()
        .and_then(|s| s.library.clone())
        .unwrap_or_else(|| store.root().join("skill-library").join("skills"));

    let lessons_path = store.root().join("tune-lessons.json");
    let lessons = std::fs::read_to_string(&lessons_path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| {
            v["lessons"].as_array().map(|arr| {
                arr.iter()
                    .filter(|l| l["teammateId"] == teammate_id)
                    .count()
            })
        })
        .unwrap_or(0);

    Ok(gather_teammate_setup(&SetupSources {
        teammate_id,
        runtime,
        mode,
        instructions,
        teammate_folder,
        worktree,
        connectors,
        project_path,
        group: None,
        about_you,
        memory_mode,
        claude_own_skills,
        memory_count,
        own_skills,
        lessons,
        library_folder: &library_folder,
        home_directory: home_dir,
        last_turn_events,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new(prefix: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "{prefix}{}-{}",
                std::process::id(),
                crate::invariants::unique_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(".git"), "Fixture boundary.").unwrap();
            Self(dir)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_skill(root: &Path, name: &str) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\nSay hi.\n"),
        )
        .unwrap();
    }

    #[test]
    fn last_turn_of_parses_valid_counts_and_rejects_bad_values() {
        let valid = vec![json!({
            "type": "run.started",
            "payload": {
                "runtimeThreadId": "s",
                "evidence": {
                    "raw": {
                        "claude_code_version": "2.1.310",
                        "toolCount": 21,
                        "skillCount": 80,
                        "mcpServerCount": 0,
                        "agentCount": 3,
                        "cwd": "C:/x"
                    },
                    "redacted": true
                }
            }
        })];
        assert_eq!(
            last_turn_of(&valid),
            Some(LastTurnSetup {
                version: Some("2.1.310".into()),
                tools: Some(21),
                skills: Some(80),
                mcp_servers: Some(0),
                agents: Some(3),
            })
        );
        let invalid = vec![json!({
            "type": "run.started",
            "payload": {
                "evidence": {
                    "raw": {
                        "toolCount": -1,
                        "skillCount": 1.5,
                        "claude_code_version": "bad version!"
                    }
                }
            }
        })];
        assert_eq!(last_turn_of(&invalid), None);
        assert_eq!(last_turn_of(&[]), None);
    }

    #[test]
    fn claude_code_setup_reads_each_layer_fresh() {
        let folder = Scratch::new("locust-setup-ws-");
        let home = Scratch::new("locust-setup-home-");
        let library = Scratch::new("locust-setup-lib-");
        std::fs::write(
            folder.0.join("LOCUST.md"),
            "Run the tests with npm test.\nKeep answers short.\n",
        )
        .unwrap();
        std::fs::write(folder.0.join("CLAUDE.md"), "one\ntwo\nthree\n").unwrap();
        write_skill(&folder.0.join(".claude/skills"), "review");
        write_skill(&home.0.join(".claude/skills"), "mine");
        write_skill(&library.0, "kept");
        let events = vec![json!({
            "type": "run.started",
            "payload": { "evidence": { "raw": { "claude_code_version": "2.1.310", "toolCount": 21, "skillCount": 4 } } }
        })];
        let setup = gather_teammate_setup(&SetupSources {
            teammate_id: "tm_ada",
            runtime: Some("claude"),
            mode: Some("accept-edits"),
            instructions: Some("Separate what you observed from what you assumed."),
            teammate_folder: None,
            worktree: false,
            connectors: vec!["Gmail".into()],
            project_path: &folder.0,
            group: Some(("Release".into(), "Ship once a day.".into())),
            about_you: Some("I am Colin."),
            memory_mode: "project",
            claude_own_skills: false,
            memory_count: 4,
            own_skills: vec!["evidence-first".into()],
            lessons: 1,
            library_folder: &library.0,
            home_directory: &home.0,
            last_turn_events: &events,
        });
        assert_eq!(setup.teammate_id, "tm_ada");
        assert_eq!(setup.folder.kind, FolderKind::Project);
        assert_eq!(
            setup.locust_md,
            Some(LocustMdSetup {
                lines: 2,
                truncated: false
            })
        );
        assert_eq!(
            setup.runtime_file,
            Some(RuntimeFileSetup {
                name: "CLAUDE.md".into(),
                lines: Some(3)
            })
        );
        assert_eq!(
            setup.group,
            Some(GroupSetup {
                name: "Release".into(),
                characters: 16
            })
        );
        assert_eq!(setup.instructions, 49);
        assert!(setup.about_you);
        assert_eq!(setup.memory, MemorySetup { on: true, count: 4 });
        assert_eq!(
            setup.skills,
            Some(SkillsSetup {
                own: vec!["evidence-first".into()],
                project: 1,
                project_folders: vec![".claude/skills".into()],
                personal: PersonalSkillsSetup {
                    count: 1,
                    lent: false,
                    found_itself: false,
                },
                library: 1,
            })
        );
        assert_eq!(setup.connectors, Some(vec!["Gmail".into()]));
        assert_eq!(setup.lessons, 1);
    }

    #[test]
    fn other_runtimes_and_codex_opencode_skills_match_electron() {
        let folder = Scratch::new("locust-setup-ws-");
        let home = Scratch::new("locust-setup-home-");
        let library = Scratch::new("locust-setup-lib-");
        write_skill(&folder.0.join(".agents/skills"), "shared");
        write_skill(&folder.0.join(".opencode/skills"), "oc-only");
        write_skill(&folder.0.join(".claude/skills"), "shared");
        write_skill(&home.0.join(".agents/skills"), "mine");
        write_skill(&home.0.join(".codex/skills"), "installed");
        write_skill(&home.0.join(".claude/skills"), "claude-own");
        write_skill(&library.0, "kept");

        let gemini = gather_teammate_setup(&SetupSources {
            teammate_id: "tm_ada",
            runtime: Some("gemini"),
            mode: Some("accept-edits"),
            instructions: None,
            teammate_folder: None,
            worktree: false,
            connectors: vec![],
            project_path: &folder.0,
            group: None,
            about_you: None,
            memory_mode: "project",
            claude_own_skills: false,
            memory_count: 0,
            own_skills: vec!["never-read".into()],
            lessons: 0,
            library_folder: &library.0,
            home_directory: &home.0,
            last_turn_events: &[],
        });
        assert_eq!(
            gemini.runtime_file,
            Some(RuntimeFileSetup {
                name: "GEMINI.md".into(),
                lines: None
            })
        );
        assert_eq!(gemini.skills, None);
        assert_eq!(gemini.connectors, None);

        let codex = gather_teammate_setup(&SetupSources {
            teammate_id: "tm_ada",
            runtime: Some("codex"),
            mode: Some("accept-edits"),
            instructions: None,
            teammate_folder: None,
            worktree: false,
            connectors: vec![],
            project_path: &folder.0,
            group: None,
            about_you: None,
            memory_mode: "off",
            claude_own_skills: false,
            memory_count: 0,
            own_skills: vec!["evidence-first".into()],
            lessons: 0,
            library_folder: &library.0,
            home_directory: &home.0,
            last_turn_events: &[],
        });
        assert_eq!(codex.skills.as_ref().unwrap().project, 1);
        assert_eq!(codex.skills.as_ref().unwrap().personal.count, 2);
        assert!(codex.skills.as_ref().unwrap().personal.found_itself);

        let opencode = gather_teammate_setup(&SetupSources {
            teammate_id: "tm_ada",
            runtime: Some("opencode"),
            mode: Some("accept-edits"),
            instructions: None,
            teammate_folder: None,
            worktree: false,
            connectors: vec![],
            project_path: &folder.0,
            group: None,
            about_you: None,
            memory_mode: "off",
            claude_own_skills: false,
            memory_count: 0,
            own_skills: vec!["evidence-first".into()],
            lessons: 0,
            library_folder: &library.0,
            home_directory: &home.0,
            last_turn_events: &[],
        });
        assert_eq!(opencode.skills.as_ref().unwrap().project, 2);
        let rows = &setup_sections(&opencode, "Ada", "OpenCode")[1].rows;
        assert_eq!(
            rows[1],
            SetupRow {
                label: "Project skills".into(),
                value: "2 skills in .opencode/skills, .claude/skills and .agents/skills".into()
            }
        );
        assert_eq!(
            rows[2],
            SetupRow {
                label: "Your skills".into(),
                value: "2 skills in your skill folders, read by OpenCode itself".into()
            }
        );
    }

    #[test]
    fn card_words_match_electron_in_order() {
        let setup = TeammateSetup {
            teammate_id: "tm_ada".into(),
            folder: FolderSetup {
                name: "locust".into(),
                kind: FolderKind::Project,
            },
            locust_md: Some(LocustMdSetup {
                lines: 210,
                truncated: true,
            }),
            runtime_file: Some(RuntimeFileSetup {
                name: "CLAUDE.md".into(),
                lines: Some(1),
            }),
            group: None,
            instructions: 49,
            about_you: false,
            memory: MemorySetup { on: true, count: 1 },
            skills: Some(SkillsSetup {
                own: vec![],
                project: 0,
                project_folders: vec![".claude/skills".into()],
                personal: PersonalSkillsSetup {
                    count: 3,
                    lent: false,
                    found_itself: false,
                },
                library: 2,
            }),
            connectors: Some(vec![]),
            lessons: 2,
            last_turn: Some(LastTurnSetup {
                version: Some("2.1.310".into()),
                tools: Some(21),
                skills: Some(1),
                mcp_servers: Some(0),
                agents: Some(3),
            }),
        };
        let sections = setup_sections(&setup, "Ada", "Claude Code");
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Every turn", "When the work matches", "Only when you tune it", "Its last turn here"]
        );
        assert_eq!(sections[0].rows[1].value, "210 lines read; the rest is cut");
        assert_eq!(sections[0].rows[2].value, "1 line, read by Claude Code itself");
        assert_eq!(sections[0].rows[5].value, "On, 1 memory in this folder");
        assert_eq!(sections[1].rows[0].value, "None yet. A tune can give it one.");
        assert_eq!(sections[1].rows[1].value, "None in .claude/skills");
        assert_eq!(sections[1].rows[2].value, "Not lent; Settings can lend them");
        assert_eq!(sections[1].rows[4].value, "Off");
        assert_eq!(
            sections[3].rows[1].value,
            "21 tools, 1 skill, 0 connector servers, 3 agents"
        );
    }

    #[cfg(unix)]
    #[test]
    fn skill_census_follows_symlinks_ignores_broken_and_stops_cycles() {
        let root = Scratch::new("locust-setup-linked-");
        let home = root.0.join("home");
        let project = root.0.join("project");
        let cwd = project.join("child");
        let personal = home.join(".agents/skills");
        let shared = root.0.join("shared");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&personal).unwrap();
        std::fs::write(project.join(".git"), "Only its existence matters.").unwrap();
        write_skill(&shared, "linked-review");
        write_skill(&project.join(".agents/skills"), "ancestor-review");
        write_skill(&project.join(".claude/skills"), "not-lent-to-restricted-claude");
        std::os::unix::fs::symlink(shared.join("linked-review"), personal.join("linked-review")).unwrap();
        std::os::unix::fs::symlink(&personal, personal.join("loop")).unwrap();
        std::os::unix::fs::symlink(root.0.join("missing"), personal.join("broken")).unwrap();

        let setup = gather_teammate_setup(&SetupSources {
            teammate_id: "tm_ada",
            runtime: Some("codex"),
            mode: Some("ask"),
            instructions: None,
            teammate_folder: None,
            worktree: false,
            connectors: vec![],
            project_path: &cwd,
            group: None,
            about_you: None,
            memory_mode: "off",
            claude_own_skills: false,
            memory_count: 0,
            own_skills: vec![],
            lessons: 0,
            library_folder: &root.0.join("no-library"),
            home_directory: &home,
            last_turn_events: &[],
        });
        assert_eq!(setup.skills.as_ref().unwrap().project, 1);
        assert_eq!(setup.skills.as_ref().unwrap().personal.count, 1);

        let restricted = gather_teammate_setup(&SetupSources {
            teammate_id: "tm_ada",
            runtime: Some("claude"),
            mode: Some("ask"),
            instructions: None,
            teammate_folder: None,
            worktree: false,
            connectors: vec![],
            project_path: &cwd,
            group: None,
            about_you: None,
            memory_mode: "off",
            claude_own_skills: false,
            memory_count: 0,
            own_skills: vec![],
            lessons: 0,
            library_folder: &root.0.join("no-library"),
            home_directory: &home,
            last_turn_events: &[],
        });
        assert_eq!(restricted.skills.as_ref().unwrap().project, 0);
    }
}
