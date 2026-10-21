import { randomUUID } from 'node:crypto'
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { replaceFile } from './replace-file.js'
import { MAX_SKILL_DESCRIPTION, MAX_SKILL_NAME, skillMarkdown, TUNE_SKILL_NAME, tuneSkillOf, type PublicTeammateSkill } from '../shared/tune.js'

/**
 * A TEAMMATE'S OWN SKILLS (0.811).
 *
 * A tune of a Claude Code teammate may draft "What it knows" as a skill (shared/tune.ts); the person keeps the setup
 * and says Use this skill, and it is the teammate's from then on. Each lives as Claude Code's own shape, a folder
 * holding a SKILL.md, under `<root>/teammate-skills/<teammate>/<name>/`, which claude-skills.ts hands to that
 * teammate's Claude Code runs as the `teammate` plugin -- copied for each run, never linked. An index beside the
 * folders says which tune each came from and when. At most 20 a teammate; a skill of the same name is replaced;
 * removing a teammate removes its skills.
 */
export const MAX_SKILLS_PER_TEAMMATE = 20

export interface TeammateSkillStore {
  list(teammateId?: unknown): Promise<readonly PublicTeammateSkill[]>
  add(teammateId: unknown, skill: unknown, compareId?: string): Promise<PublicTeammateSkill>
  remove(teammateId: unknown, name: unknown): Promise<void>
  removeForTeammate(teammateId: unknown): Promise<void>
  /** The folder of one teammate's skills, for claude-skills.ts to read; it may not exist yet. */
  folderOf(teammateId: string): string
}

const SCHEMA_VERSION = 1 as const
const TEAMMATE_ID = /^[A-Za-z0-9_-]{1,80}$/
const UNREADABLE = 'The teammate skills record on disk could not be read.'
const NOT_SAVED = 'That skill could not be saved. The teammate is unchanged.'

interface StoredFile {
  readonly schemaVersion: typeof SCHEMA_VERSION
  readonly skills: readonly PublicTeammateSkill[]
}

const EMPTY: StoredFile = { schemaVersion: SCHEMA_VERSION, skills: [] }

function parsedEntry(value: unknown): PublicTeammateSkill | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const row = value as Record<string, unknown>
  const teammateId = typeof row.teammateId === 'string' && TEAMMATE_ID.test(row.teammateId) ? row.teammateId : undefined
  const name = typeof row.name === 'string' && row.name.length <= MAX_SKILL_NAME && TUNE_SKILL_NAME.test(row.name) ? row.name : undefined
  const description = typeof row.description === 'string' && row.description.length > 0 && row.description.length <= MAX_SKILL_DESCRIPTION ? row.description : undefined
  const createdAt = typeof row.createdAt === 'string' && row.createdAt.length <= 40 ? row.createdAt : undefined
  const compareId = typeof row.compareId === 'string' && row.compareId.length <= 80 ? row.compareId : undefined
  if (teammateId === undefined || name === undefined || description === undefined || createdAt === undefined) return undefined
  return { teammateId, name, description, ...(compareId === undefined ? {} : { compareId }), createdAt }
}

export function createTeammateSkillStore(options: { readonly rootDirectory: string; readonly now?: () => Date }): TeammateSkillStore {
  const root = join(options.rootDirectory, 'teammate-skills')
  const index = join(root, 'skills.json')
  const now = options.now ?? (() => new Date())
  let queue: Promise<unknown> = Promise.resolve()
  const serialize = <T>(work: () => Promise<T>): Promise<T> => {
    const next = queue.then(work, work)
    queue = next.then(() => undefined, () => undefined)
    return next
  }

  const read = async (): Promise<StoredFile> => {
    let raw: string
    try {
      raw = await readFile(index, 'utf8')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return EMPTY
      throw new Error(UNREADABLE)
    }
    try {
      const parsed = JSON.parse(raw) as { readonly schemaVersion?: unknown; readonly skills?: unknown }
      if (parsed.schemaVersion !== SCHEMA_VERSION || !Array.isArray(parsed.skills)) throw new Error(UNREADABLE)
      return { schemaVersion: SCHEMA_VERSION, skills: parsed.skills.flatMap((entry) => parsedEntry(entry) ?? []) }
    } catch {
      throw new Error(UNREADABLE)
    }
  }

  const write = async (file: StoredFile): Promise<void> => {
    await mkdir(root, { recursive: true })
    const temporary = `${index}.${randomUUID()}.tmp`
    await writeFile(temporary, `${JSON.stringify(file, null, 2)}\n`, 'utf8')
    await replaceFile(temporary, index)
  }

  const folderOf = (teammateId: string): string => join(root, teammateId)
  const idOf = (value: unknown): string | undefined => (typeof value === 'string' && TEAMMATE_ID.test(value) ? value : undefined)

  return {
    folderOf,

    list: (teammateId) =>
      serialize(async () => {
        const skills = (await read()).skills
        return typeof teammateId === 'string' ? skills.filter((entry) => entry.teammateId === teammateId) : skills
      }),

    add: (teammateId, skill, compareId) =>
      serialize(async () => {
        const id = idOf(teammateId)
        const given = tuneSkillOf(skill)
        if (id === undefined || given === undefined) throw new Error('That skill could not be saved: it did not read whole. The teammate is unchanged.')
        const file = await read()
        const others = file.skills.filter((entry) => !(entry.teammateId === id && entry.name === given.name))
        if (others.filter((entry) => entry.teammateId === id).length >= MAX_SKILLS_PER_TEAMMATE) {
          throw new Error(`A teammate keeps at most ${String(MAX_SKILLS_PER_TEAMMATE)} skills. Remove one in Edit teammate first.`)
        }
        const kept: PublicTeammateSkill = {
          teammateId: id,
          name: given.name,
          description: given.description,
          ...(typeof compareId === 'string' && compareId.length > 0 && compareId.length <= 80 ? { compareId } : {}),
          createdAt: now().toISOString()
        }
        // Written beside, then moved into place, so a run never copies a half-written skill; the one it replaces is
        // moved aside first and put back if anything fails. A name starting with a dot is one no run copies.
        const folder = join(folderOf(id), given.name)
        const staged = join(folderOf(id), `.${given.name}.${randomUUID()}.tmp`)
        const aside = join(folderOf(id), `.${given.name}.${randomUUID()}.old`)
        let replaced = false
        let placed = false
        try {
          await mkdir(staged, { recursive: true })
          await writeFile(join(staged, 'SKILL.md'), skillMarkdown(given), 'utf8')
          try {
            await rename(folder, aside)
            replaced = true
          } catch (error) {
            if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
          }
          await rename(staged, folder)
          placed = true
          await write({ schemaVersion: SCHEMA_VERSION, skills: [...others, kept] })
        } catch {
          if (placed) await rm(folder, { recursive: true, force: true }).catch(() => undefined)
          if (replaced) await rename(aside, folder).catch(() => undefined)
          await rm(staged, { recursive: true, force: true }).catch(() => undefined)
          throw new Error(NOT_SAVED)
        }
        if (replaced) await rm(aside, { recursive: true, force: true }).catch(() => undefined)
        return kept
      }),

    remove: (teammateId, name) =>
      serialize(async () => {
        const id = idOf(teammateId)
        if (id === undefined || typeof name !== 'string') return
        const file = await read()
        const entry = file.skills.find((one) => one.teammateId === id && one.name === name)
        if (entry === undefined) return
        await rm(join(folderOf(id), entry.name), { recursive: true, force: true })
        await write({ schemaVersion: SCHEMA_VERSION, skills: file.skills.filter((one) => one !== entry) })
      }),

    removeForTeammate: (teammateId) =>
      serialize(async () => {
        const id = idOf(teammateId)
        if (id === undefined) return
        const file = await read()
        await rm(folderOf(id), { recursive: true, force: true })
        if (file.skills.some((one) => one.teammateId === id)) await write({ schemaVersion: SCHEMA_VERSION, skills: file.skills.filter((one) => one.teammateId !== id) })
      })
  }
}

/** The skill a store keeps under `name`, read back from its SKILL.md: what the run is handed, for a test to see. */
export async function keptSkillText(store: TeammateSkillStore, teammateId: string, name: string): Promise<string | undefined> {
  return readFile(join(store.folderOf(teammateId), name, 'SKILL.md'), 'utf8').catch(() => undefined)
}
