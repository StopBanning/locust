import { randomUUID } from 'node:crypto'
import { cp, lstat, mkdir, readdir, rm, stat, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { skillMarkdown, type TuneSkill } from '../shared/tune.js'

/**
 * SKILLS FOR A CLAUDE CODE TEAMMATE (0.679).
 *
 * Claude Code runs a skill through its `Skill` tool, and finds skills in the
 * folder's `.claude/skills` and the person's `~/.claude/skills`. Every Locust
 * run outside Auto is started with `--restricted`, and MEASURED 2026-10-04 on
 * 2.1.289 (FINDING-claude-skills-under-restricted.md, eight Haiku runs): that
 * flag drops both of those folders, and `--setting-sources project` and
 * `--add-dir` do not bring them back. What does is `--plugin-dir`: the same
 * SKILL.md inside `<plugin>/skills/<name>/`, beside a
 * `.claude-plugin/plugin.json`, loaded under `--restricted`, was called, and
 * was followed. Its name gains the plugin's: `project:review`.
 *
 * So before a Claude run outside Auto, this builds those plugins from the
 * skill folders, in a folder of the run's own:
 *
 *   - `project`, from `<folder>/.claude/skills` -- always. A project's skills
 *     are part of the project, the way its CLAUDE.md is.
 *   - `personal`, from `~/.claude/skills` -- only when the person has said so
 *     in Settings (`claudeOwnSkills`). Their own skills are theirs, written
 *     for their own sessions; a teammate gets them when asked, not by default.
 *   - `library`, the skills the person kept from a GitHub repository
 *     (0.710, skill-library.ts) -- always: keeping one is the asking.
 *   - `teammate`, the run's teammate's own skills (0.811, teammate-skills.ts),
 *     kept from a tune -- always, Auto included -- and a tune column's one
 *     candidate skill, written into that column's run alone.
 *
 * COPIED, never linked. A junction would always be current, but the run's
 * folder is removed when the run ends, and a removal that followed a link
 * would delete the person's own skills. A copy can only ever delete itself.
 * Only folders holding a SKILL.md are copied, within a size bound, so a stray
 * build folder beside the skills is never carried along.
 *
 * Auto needs none of this for the first two: it runs without `--restricted`,
 * as the person, and Claude Code finds both folders itself. It does need the
 * library, which lives in Locust's profile where nothing else looks, so an
 * Auto run is handed that one alone (`auto`).
 */

/** Claude Code's own rule for a plugin's skill: `skills/<name>/SKILL.md`. */
const SKILL_FILE = 'SKILL.md'
/** One skill folder larger than this is left out: a skill is instructions, not a payload. */
const MOST_BYTES_A_SKILL = 5 * 1024 * 1024
/** And all of them together, per plugin. */
const MOST_BYTES_ALL = 40 * 1024 * 1024
const MOST_SKILLS = 200

export type SkillPluginName = 'project' | 'personal' | 'library' | 'teammate'

const PLUGIN_DESCRIPTION: Record<SkillPluginName, string> = {
  project: "This folder's own skills",
  personal: 'Your own Claude Code skills',
  library: 'Skills you kept from GitHub',
  teammate: "This teammate's own skills"
}

export interface SkillPlugin {
  readonly name: SkillPluginName
  /** The folder to name with `--plugin-dir`. */
  readonly dir: string
  /** The skills it carries, by the name a run calls them: `project:review`. */
  readonly skills: readonly string[]
}

export interface PreparedSkills {
  readonly plugins: readonly SkillPlugin[]
  /** Removes the run's copies. Safe to call twice. */
  readonly dispose: () => Promise<void>
}

/** The size of a folder, giving up past `limit`. Links are not followed and count as nothing. */
async function sizeWithin(path: string, limit: number): Promise<number | undefined> {
  let total = 0
  const pending = [path]
  while (pending.length > 0) {
    const at = pending.pop() as string
    const entries = await readdir(at, { withFileTypes: true })
    for (const entry of entries) {
      const full = join(at, entry.name)
      if (entry.isSymbolicLink()) continue
      if (entry.isDirectory()) pending.push(full)
      else if (entry.isFile()) total += (await stat(full)).size
      if (total > limit) return undefined
    }
  }
  return total
}

/** The skill folders under `root`: each a real folder (not a link) with a SKILL.md, within the bounds. */
export async function skillFoldersIn(root: string): Promise<readonly string[]> {
  let entries
  try {
    entries = await readdir(root, { withFileTypes: true })
  } catch {
    return []
  }
  const found: string[] = []
  let total = 0
  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    if (found.length >= MOST_SKILLS) break
    if (!entry.isDirectory() || entry.name.startsWith('.')) continue
    // A name Claude Code would take: the folder's name is the skill's.
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(entry.name)) continue
    const folder = join(root, entry.name)
    try {
      const file = await lstat(join(folder, SKILL_FILE))
      if (!file.isFile()) continue
      const size = await sizeWithin(folder, MOST_BYTES_A_SKILL)
      if (size === undefined || total + size > MOST_BYTES_ALL) continue
      total += size
      found.push(entry.name)
    } catch {
      continue
    }
  }
  return found
}

async function buildPlugin(
  base: string,
  name: SkillPluginName,
  from: string | undefined,
  { extra, only }: { readonly extra?: TuneSkill; readonly only?: readonly string[] } = {}
): Promise<SkillPlugin | undefined> {
  // A tune column's skill (0.811) stands in for a kept one of its name, for this run alone.
  const names = (from === undefined ? [] : await skillFoldersIn(from)).filter((skill) => skill !== extra?.name && (only === undefined || only.includes(skill)))
  if (names.length === 0 && extra === undefined) return undefined
  const dir = join(base, name)
  await mkdir(join(dir, '.claude-plugin'), { recursive: true })
  await writeFile(
    join(dir, '.claude-plugin', 'plugin.json'),
    `${JSON.stringify({ name, description: PLUGIN_DESCRIPTION[name], version: '1.0.0' })}\n`
  )
  const copied: string[] = []
  if (extra !== undefined) {
    await mkdir(join(dir, 'skills', extra.name), { recursive: true })
    await writeFile(join(dir, 'skills', extra.name, SKILL_FILE), skillMarkdown(extra))
    copied.push(`${name}:${extra.name}`)
  }
  for (const skill of names) {
    try {
      // A link inside a skill is left out, not copied: the copy holds only
      // real files, so removing it can never reach past it.
      await cp(join(from!, skill), join(dir, 'skills', skill), {
        recursive: true,
        filter: async (source) => !(await lstat(source)).isSymbolicLink()
      })
      copied.push(`${name}:${skill}`)
    } catch {
      // A skill that cannot be read is left out; the rest still load.
    }
  }
  return copied.length === 0 ? undefined : { name, dir, skills: copied }
}

/**
 * The plugins one Claude run outside Auto should be started with, built under
 * `scratchRoot` in a folder of its own and removed by `dispose`.
 */
export async function prepareClaudeSkills(options: {
  readonly scratchRoot: string
  readonly workspacePath: string
  readonly ownSkills: boolean
  readonly homeDirectory?: string
  /** The kept skills' folder (skill-library.ts `skillsFolder`). */
  readonly libraryFolder?: string
  /** An Auto run: it finds the folder's skills and the person's itself, so only the library is built. */
  readonly auto?: boolean
  /**
   * The run's teammate's own skills (0.811, teammate-skills.ts), and a tune column's one skill for this run alone:
   * both live where Claude Code never looks, so Auto is handed them as well.
   */
  readonly teammateFolder?: string
  /** Only these of its skills: the ones the teammate's record lists, so what Edit teammate shows is what a run gets. */
  readonly teammateSkills?: readonly string[]
  readonly candidate?: TuneSkill
}): Promise<PreparedSkills> {
  const base = join(options.scratchRoot, randomUUID())
  const dispose = async (): Promise<void> => {
    await rm(base, { recursive: true, force: true }).catch(() => undefined)
  }
  try {
    const plugins: SkillPlugin[] = []
    if (options.auto !== true) {
      const project = await buildPlugin(base, 'project', join(options.workspacePath, '.claude', 'skills'))
      if (project !== undefined) plugins.push(project)
    }
    if (options.ownSkills && options.auto !== true) {
      const personal = await buildPlugin(base, 'personal', join(options.homeDirectory ?? homedir(), '.claude', 'skills'))
      if (personal !== undefined) plugins.push(personal)
    }
    if (options.libraryFolder !== undefined) {
      const library = await buildPlugin(base, 'library', options.libraryFolder)
      if (library !== undefined) plugins.push(library)
    }
    if (options.teammateFolder !== undefined || options.candidate !== undefined) {
      const teammate = await buildPlugin(base, 'teammate', options.teammateFolder, {
        ...(options.candidate === undefined ? {} : { extra: options.candidate }),
        ...(options.teammateSkills === undefined ? {} : { only: options.teammateSkills })
      })
      if (teammate !== undefined) plugins.push(teammate)
    }
    if (plugins.length === 0) await dispose()
    return { plugins, dispose }
  } catch {
    await dispose()
    return { plugins: [], dispose }
  }
}

/** Copies a crash left behind: every run folder under the scratch root, at launch, before any run. */
export async function clearClaudeSkillCopies(scratchRoot: string): Promise<void> {
  await rm(scratchRoot, { recursive: true, force: true }).catch(() => undefined)
}
