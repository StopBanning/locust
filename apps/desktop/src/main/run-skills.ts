import { randomUUID } from 'node:crypto'
import { cp, lstat, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { skillMarkdown, type TuneSkill } from '../shared/tune.js'
import { skillFoldersIn } from './claude-skills.js'
import { skillNameIn, skillNameKey } from './skill-discovery.js'

/**
 * SKILLS FOR CODEX, OPENCODE, COPILOT AND ANTIGRAVITY (0.813).
 *
 * Claude Code is handed a teammate's skills as plugins (claude-skills.ts). Codex and OpenCode read the same SKILL.md
 * shape from folders of their own -- the folder's `.agents/skills`, the person's `~/.agents/skills` and, for
 * OpenCode, `.opencode/skills` and `.claude/skills` -- and find those themselves, as they do in the person's own
 * sessions. What they cannot find is what lives in Locust's profile: the skills kept from GitHub (skill-library.ts)
 * and a teammate's own, kept from a tune (teammate-skills.ts). MEASURED 2026-10-22 on this machine, no model called:
 *
 *   - Codex 0.163.0's app-server lists a skill from any folder named in `skills/extraRoots/set`, for that process
 *     only (`skills/list`, scope user), beside every skill it found itself;
 *   - OpenCode 1.18.27 scans every folder in its config's `skills.paths` for each SKILL.md beneath it (its own code), and
 *     OpenCode 2 takes the same folder in its config's `skills` list. Locust's per-run config carries it. (Not
 *     `OPENCODE_CONFIG_DIR`, which lists the skill too but makes the folder a config folder that OpenCode installs a
 *     61 MB plugin package into before every run.)
 *
 * So a run on either is handed one folder, copied for the run and removed when it ends, laid out as both read it:
 * `<run>/skills/<name>/SKILL.md`. COPIED, never linked, for claude-skills.ts's reason: removing the copy can only
 * ever remove itself. A name in more than one place is the most particular one's -- a tune column's candidate, then
 * the teammate's own, then the library's -- since the folder is flat where Claude Code's plugins name each.
 *
 * The same folder, MEASURED 2026-10-22, also serves two more, and nothing of the person's is written:
 *
 *   - Copilot CLI 1.0.101 takes it as one `--plugin-dir`. `<run>/.plugin/plugin.json` points `skills` at `skills/`,
 *     and `copilot skill list` shows those skills beside the person's own. Nothing is installed. (`copilot skill add`
 *     would keep the directory, so it is never run.) A prompt on this machine was refused by Copilot's policy before
 *     a model answered; the listing is what shows the skill is seen.
 *   - Antigravity CLI 1.4.4 takes `<run>` as an absolute `--add-dir` and reads `<run>/.agents/skills/<name>/SKILL.md`.
 *     Print mode listed the skill, and a one-line prompt answered with the word only that skill held, in 15 seconds.
 *     The folder was unchanged. A `.agents/skills.json` entry can point at `skills/` too; the copy is what the prompt
 *     read. A relative `--add-dir` is not seen.
 */
export interface PreparedRunSkills {
  /** `<run>`: Copilot's `--plugin-dir` and Antigravity's `--add-dir`. Holds `skills/`, `.plugin/` and `.agents/`. */
  readonly dir: string
  /** `<run>/skills`: Codex's extra root, OpenCode 1's `skills.paths` entry and OpenCode 2's `skills` entry. */
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
  /** The target machine's path semantics; explicit in portable collision tests. */
  readonly platform?: NodeJS.Platform
  /** The real copy by default; a failing disk copy can be exercised without a platform-specific file lock. */
  readonly copyFolder?: typeof cp
}): Promise<PreparedRunSkills | undefined> {
  const dir = join(options.scratchRoot, randomUUID())
  const skillsDir = join(dir, 'skills')
  const dispose = async (): Promise<void> => {
    await rm(dir, { recursive: true, force: true }).catch(() => undefined)
  }
  try {
    const taken: string[] = []
    const names = new Set<string>()
    const destinations = new Set<string>()
    const key = (name: string): string => skillNameKey(name, options.platform)
    if (options.candidate !== undefined) {
      await mkdir(join(skillsDir, options.candidate.name), { recursive: true })
      await writeFile(join(skillsDir, options.candidate.name, 'SKILL.md'), skillMarkdown(options.candidate))
      taken.push(options.candidate.name)
      names.add(key(options.candidate.name))
      destinations.add(key(options.candidate.name))
    }
    const copy = async (from: string | undefined, only?: readonly string[]): Promise<void> => {
      if (from === undefined) return
      for (const name of await skillFoldersIn(from)) {
        if (destinations.has(key(name)) || (only !== undefined && !only.includes(name))) continue
        const staging = join(dir, `.copy-${randomUUID()}`)
        try {
          // A link inside a skill is left out, not copied: the copy holds only real files.
          await (options.copyFolder ?? cp)(join(from, name), staging, { recursive: true, filter: async (source) => !(await lstat(source)).isSymbolicLink() })
          // Read the identity from the completed snapshot, not a source that could change while copying.
          const identity = skillNameIn(await readFile(join(staging, 'SKILL.md'), 'utf8')) ?? name
          if (names.has(key(identity))) continue
          await mkdir(skillsDir, { recursive: true })
          await rename(staging, join(skillsDir, name))
          taken.push(name)
          destinations.add(key(name))
          names.add(key(identity))
        } catch {
          // A skill that cannot be read is left out; the rest still load.
        } finally {
          await rm(staging, { recursive: true, force: true }).catch(() => undefined)
        }
      }
    }
    await copy(options.teammateFolder, options.teammateSkills ?? [])
    await copy(options.libraryFolder)
    if (taken.length === 0) {
      await dispose()
      return undefined
    }
    // Copilot's plugin manifest points at the folder Codex and OpenCode already read.
    await mkdir(join(dir, '.plugin'), { recursive: true })
    await writeFile(join(dir, '.plugin', 'plugin.json'), `${JSON.stringify({ name: 'locust-run-skills', description: 'Skills for this run.', version: '0.0.1', skills: 'skills' })}\n`)
    // Antigravity reads this copy. It is the same files, not a link.
    await mkdir(join(dir, '.agents'), { recursive: true })
    await cp(skillsDir, join(dir, '.agents', 'skills'), { recursive: true })
    return { dir, skillsDir, skills: taken, dispose }
  } catch {
    await dispose()
    return undefined
  }
}
