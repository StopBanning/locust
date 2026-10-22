import { randomUUID } from 'node:crypto'
import { cp, lstat, mkdir, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { skillMarkdown, type TuneSkill } from '../shared/tune.js'
import { skillFoldersIn } from './claude-skills.js'

/**
 * SKILLS FOR CODEX AND OPENCODE TEAMMATES (0.813).
 *
 * Claude Code is handed a teammate's skills as plugins (claude-skills.ts). Codex and OpenCode read the same SKILL.md
 * shape from folders of their own -- the folder's `.agents/skills`, the person's `~/.agents/skills` and, for
 * OpenCode, `.opencode/skills` and `.claude/skills` -- and find those themselves, as they do in the person's own
 * sessions. What they cannot find is what lives in Locust's profile: the skills kept from GitHub (skill-library.ts)
 * and a teammate's own, kept from a tune (teammate-skills.ts). MEASURED 2026-10-22 on this machine, no model called:
 *
 *   - Codex 0.163.0's app-server lists a skill from any folder named in `skills/extraRoots/set`, for that process
 *     only (`skills/list`, scope user), beside every skill it found itself;
 *   - OpenCode 1.18.27 lists one from `$OPENCODE_CONFIG_DIR/skills/<name>/SKILL.md`, beside the person's own
 *     (`opencode debug skill`); OpenCode 2 takes the same folder in its config's `skills` list.
 *
 * So a run on either is handed one folder, copied for the run and removed when it ends, laid out as both read it:
 * `<run>/skills/<name>/SKILL.md`. COPIED, never linked, for claude-skills.ts's reason: removing the copy can only
 * ever remove itself. A name in more than one place is the most particular one's -- a tune column's candidate, then
 * the teammate's own, then the library's -- since the folder is flat where Claude Code's plugins name each.
 */
export interface PreparedRunSkills {
  /** `<run>`: what OpenCode 1 is given as its config folder, holding nothing but `skills/`. */
  readonly dir: string
  /** `<run>/skills`: Codex's extra root, and OpenCode 2's `skills` entry. */
  readonly skillsDir: string
  /** The skills it holds, by name. */
  readonly skills: readonly string[]
  /** Removes the run's copy. Safe to call twice. */
  readonly dispose: () => Promise<void>
}

export async function prepareRunSkills(options: {
  readonly scratchRoot: string
  /** The kept skills' folder (skill-library.ts `skillsFolder`). */
  readonly libraryFolder?: string
  /** The run's teammate's own skills (teammate-skills.ts `folderOf`). */
  readonly teammateFolder?: string
  /** Only these of them: the ones its record lists, so what Edit teammate shows is what a run gets. */
  readonly teammateSkills?: readonly string[]
  /** A tune column's one skill, for its run alone. */
  readonly candidate?: TuneSkill
}): Promise<PreparedRunSkills | undefined> {
  const dir = join(options.scratchRoot, randomUUID())
  const skillsDir = join(dir, 'skills')
  const dispose = async (): Promise<void> => {
    await rm(dir, { recursive: true, force: true }).catch(() => undefined)
  }
  try {
    const taken: string[] = []
    if (options.candidate !== undefined) {
      await mkdir(join(skillsDir, options.candidate.name), { recursive: true })
      await writeFile(join(skillsDir, options.candidate.name, 'SKILL.md'), skillMarkdown(options.candidate))
      taken.push(options.candidate.name)
    }
    const copy = async (from: string | undefined, only?: readonly string[]): Promise<void> => {
      if (from === undefined) return
      for (const name of await skillFoldersIn(from)) {
        if (taken.includes(name) || (only !== undefined && !only.includes(name))) continue
        try {
          // A link inside a skill is left out, not copied: the copy holds only real files.
          await cp(join(from, name), join(skillsDir, name), { recursive: true, filter: async (source) => !(await lstat(source)).isSymbolicLink() })
          taken.push(name)
        } catch {
          // A skill that cannot be read is left out; the rest still load.
        }
      }
    }
    await copy(options.teammateFolder, options.teammateSkills ?? [])
    await copy(options.libraryFolder)
    if (taken.length === 0) {
      await dispose()
      return undefined
    }
    return { dir, skillsDir, skills: taken, dispose }
  } catch {
    await dispose()
    return undefined
  }
}
