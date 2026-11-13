//! Runtimes that accept a per-run skill directory or plugin (Electron `shared/skill-runtimes.ts`).

pub const SKILL_RUNTIMES: &[&str] = &[
    "claude",
    "codex",
    "opencode",
    "copilot",
    "antigravity",
    "cursor",
];

pub fn takes_skills(runtime: Option<&str>) -> bool {
    runtime.is_some_and(|r| SKILL_RUNTIMES.contains(&r))
}

pub const SKILL_RUNTIME_NAMES: &str =
    "Claude Code, Codex, OpenCode, Copilot, Antigravity or Cursor";

pub const SKILL_RUNTIME_ANY: &str =
    "a Claude Code, Codex, OpenCode, Copilot, Antigravity or Cursor";
