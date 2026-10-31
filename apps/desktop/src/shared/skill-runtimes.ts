/**
 * THE AGENTS A SKILL REACHES (0.813).
 *
 * A skill is a folder holding a SKILL.md, read only when the work matches when to use it. Claude Code is handed a
 * run's skills as plugins (main/claude-skills.ts). Codex, OpenCode, Copilot, Antigravity and Cursor are handed one folder
 * for the run (main/run-skills.ts), MEASURED on Codex 0.163.0, OpenCode 1.18.27, Copilot CLI 1.0.101 and Antigravity
 * CLI 1.4.4 and Cursor Agent 2026.10.22. Those six take a teammate's own skills, a tune column's one and the ones kept from GitHub. The other
 * agents are not handed one, so a tune does not draft one for them and Edit teammate says their kept skills wait.
 */
export const SKILL_RUNTIMES = ['claude', 'codex', 'opencode', 'copilot', 'antigravity', 'cursor'] as const

export function takesSkills(runtime: string | undefined): boolean {
  return runtime !== undefined && (SKILL_RUNTIMES as readonly string[]).includes(runtime)
}

/** How the window names them, in a sentence: "Claude Code, Codex, OpenCode, Copilot, Antigravity and Cursor teammates". */
export const SKILL_RUNTIME_NAMES = 'Claude Code, Codex, OpenCode, Copilot, Antigravity and Cursor'
/** And one of them: "a Claude Code, Codex, OpenCode, Copilot, Antigravity or Cursor teammate". */
export const SKILL_RUNTIME_ANY = 'Claude Code, Codex, OpenCode, Copilot, Antigravity or Cursor'

/**
 * Where each finds skills itself, besides the ones it is handed: the folder's, relative to it, and the person's,
 * relative to their home. Claude Code's `.claude/skills` are handed to it as plugins (claude-skills.ts) and the
 * person's only when Settings lends them. Codex, OpenCode, Copilot, Antigravity and Cursor look in theirs whatever Locust
 * says, as they do in the person's own sessions.
 */
export const SKILL_FOLDERS: Readonly<Record<(typeof SKILL_RUNTIMES)[number], { readonly project: readonly string[]; readonly personal: readonly string[] }>> = {
  // Cursor's installed loader lists these; Claude/Codex/Grok folders depend on its third-party extensibility setting.
  cursor: { project: ['.cursor/skills', '.agents/skills', '.claude/skills', '.codex/skills', '.grok/skills'], personal: ['.cursor/skills', '.agents/skills', '.claude/skills', '.codex/skills', '.grok/skills'] },
  claude: { project: ['.claude/skills'], personal: ['.claude/skills'] },
  codex: { project: ['.agents/skills'], personal: ['.agents/skills', '.codex/skills'] },
  opencode: { project: ['.opencode/skills', '.claude/skills', '.agents/skills'], personal: ['.config/opencode/skills', '.claude/skills', '.agents/skills'] },
  // Copilot 1.0.101 also lists these from the working directory, and ~/.agents/skills as "inherited".
  copilot: { project: ['.github/skills', '.agents/skills', '.claude/skills'], personal: ['.copilot/skills', '.agents/skills'] },
  // Antigravity CLI 1.4.4. Plugins under ~/.gemini/antigravity-cli are its own installs, not these folders.
  antigravity: { project: ['.agents/skills'], personal: ['.gemini/antigravity-cli/skills', '.gemini/config/skills'] }
}
