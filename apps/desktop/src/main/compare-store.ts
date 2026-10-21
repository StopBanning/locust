import { isAccountId } from '../shared/agent-accounts.js'
import { randomUUID } from 'node:crypto'
import { constants as fsConstants } from 'node:fs'
import { mkdir, open, readFile, unlink } from 'node:fs/promises'
import { isAbsolute, join } from 'node:path'

import { COMPARE_SLOTS, compareTreeId, MAX_COMPARE_SLOTS, MAX_JUDGE_CRITERIA, MIN_COMPARE_SLOTS, PANEL_JUDGES } from '../shared/compare.js'
import type { CompareRoute, CompareSlotId, PublicCompare, PublicCompareSlot } from '../shared/compare.js'
import { instructionsText } from '../shared/teammate-instructions.js'
import { replaceFile } from './replace-file.js'

/**
 * COMPARISONS, remembered (0.441, shared/compare.ts).
 *
 * `userData/compares.json`, kept the way rooms.json is: writes serialized, a
 * temporary file synced and renamed into place, and a file that cannot be
 * read is UNREADABLE rather than empty -- every write reads first, so
 * "empty" would be written back over whatever the file held (the groups
 * store, 2026-09-16; the rooms store, 2026-09-14). A missing file is empty.
 */
const SCHEMA_VERSION = 1 as const
const MAX_FILE_BYTES = 4 * 1024 * 1024
export const MAX_COMPARES = 200
const MAX_PROMPT = 8_000
const MAX_TURNS_PER_SLOT = 200
/** Tries-again a column remembers, so their missions stay folded into it. */
const MAX_RETRIED_PER_SLOT = 50
/** Files a kept column's changes named; the rest are still in the folder, just not listed. */
const MAX_BROUGHT = 200
/** Judges a comparison remembers; older ones' runs stay the comparison's, unlisted. */
const MAX_JUDGES = 20
const UNREADABLE = 'The saved comparisons could not be read. Nothing was changed.'
/** Runs again a comparison remembers (0.763); older ones are still comparisons, just not counted. */
const MAX_RERUNS = 30
const COMMIT = /^[0-9a-f]{40}$/

interface StoredFile {
  readonly schemaVersion: typeof SCHEMA_VERSION
  readonly compares: readonly PublicCompare[]
}
const EMPTY: StoredFile = { schemaVersion: SCHEMA_VERSION, compares: [] }

export interface CompareStore {
  list(): Promise<readonly PublicCompare[]>
  get(compareId: unknown): Promise<PublicCompare | undefined>
  /** A new comparison, its columns named in order; the oldest past `MAX_COMPARES` is forgotten. */
  create(input: {
    readonly teammateId?: string
    readonly prompt: string
    readonly routes: readonly CompareRoute[]
    readonly changes?: boolean
    readonly changesIn?: 'copy' | 'folder'
    readonly blind?: boolean
    /** The commit its copies are cut from (0.763). */
    readonly base?: string
    /** It runs another comparison again (0.763). */
    readonly rerunOf?: string
  }): Promise<PublicCompare>
  /** A column's next turn started. */
  addTurn(compareId: string, slot: CompareSlotId, missionId: string): Promise<PublicCompare>
  /**
   * A column's newest answer tried again (0.444): the new mission takes its
   * place, and the one it replaces is kept as the comparison's, undrawn. A
   * column that could not start takes its first turn and loses its refusal.
   */
  retry(compareId: string, slot: CompareSlotId, missionId: string): Promise<PublicCompare>
  /** A column could not start, and why. */
  refuse(compareId: string, slot: CompareSlotId, why: string): Promise<PublicCompare>
  keep(compareId: string, slot: CompareSlotId, brought?: readonly string[]): Promise<PublicCompare>
  /** Another model joins (0.523): a new column, the next letter, nothing asked yet. */
  addColumn(compareId: string, route: CompareRoute): Promise<PublicCompare>
  /**
   * A judge was asked (0.520): its run, newest last, on this route and with these words -- and, for three judges
   * voting (0.765), the other two's runs, which replace the last ask's panel.
   */
  judge(compareId: string, route: CompareRoute, missionId: string, criteria?: string, panel?: readonly { readonly route: CompareRoute; readonly missionId: string }[]): Promise<PublicCompare>
  /** A model drafted its points (0.766): that run, kept so it is never listed as a conversation. */
  addPointDraft(compareId: string, missionId: string): Promise<PublicCompare>
  /** A run of it again began (0.763): that comparison's id, newest last. */
  addRerun(compareId: string, rerunId: string): Promise<PublicCompare>
  /** A run again is over (0.763): its models and judge have finished, and its copies are gone. */
  settle(compareId: string): Promise<PublicCompare>
  remove(compareId: unknown): Promise<void>
}

const isSlot = (value: unknown): value is CompareSlotId => typeof value === 'string' && (COMPARE_SLOTS as readonly string[]).includes(value)
const text = (value: unknown, max: number): string | undefined => (typeof value === 'string' && value.length > 0 && value.length <= max ? value : undefined)

function parsedSlot(value: unknown): PublicCompareSlot | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  const route = (typeof record.route === 'object' && record.route !== null ? record.route : {}) as Record<string, unknown>
  const runtime = text(route.runtime, 40)
  const model = text(route.model, 200)
  if (!isSlot(record.slot) || runtime === undefined || model === undefined) return undefined
  if (route.account !== undefined && !isAccountId(route.account)) return undefined
  const account = route.account as string | undefined
  const effort = text(route.effort, 40)
  const label = text(route.label, 120)
  const mode = route.mode === 'auto' ? ('auto' as const) : undefined
  const instructions = instructionsText(route.instructions)
  if (route.instructions !== undefined && instructions === undefined) return undefined
  const why = text(route.why, 400)
  const missionIds = Array.isArray(record.missionIds) ? record.missionIds.filter((id): id is string => typeof id === 'string' && id.length > 0 && id.length <= 200).slice(-MAX_TURNS_PER_SLOT) : []
  const refused = text(record.refused, 1_000)
  const retried = Array.isArray(record.retried) ? record.retried.filter((id): id is string => typeof id === 'string' && id.length > 0 && id.length <= 200).slice(-MAX_RETRIED_PER_SLOT) : []
  return {
    slot: record.slot,
    route: { runtime, model, ...(account === undefined ? {} : { account }), ...(effort === undefined ? {} : { effort }), ...(label === undefined ? {} : { label }), ...(mode === undefined ? {} : { mode }), ...(instructions === undefined ? {} : { instructions }), ...(why === undefined ? {} : { why }) },
    missionIds,
    ...(refused === undefined ? {} : { refused }),
    ...(retried.length === 0 ? {} : { retried })
  }
}

function parsedCompare(value: unknown): PublicCompare | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  const compareId = text(record.compareId, 80)
  const teammateId = text(record.teammateId, 200)
  const prompt = text(record.prompt, MAX_PROMPT)
  const createdAt = text(record.createdAt, 40)
  if (compareId === undefined || (record.teammateId !== undefined && teammateId === undefined) || prompt === undefined || createdAt === undefined || !Array.isArray(record.slots)) return undefined
  const slots = record.slots.map(parsedSlot).filter((slot): slot is PublicCompareSlot => slot !== undefined)
  if (teammateId === undefined && slots.some((column) => column.route.instructions !== undefined)) return undefined
  if (slots.length < MIN_COMPARE_SLOTS || slots.length > MAX_COMPARE_SLOTS || new Set(slots.map((slot) => slot.slot)).size !== slots.length) return undefined
  const kept = typeof record.kept === 'object' && record.kept !== null ? (record.kept as Record<string, unknown>) : undefined
  const keptAt = text(kept?.at, 40)
  const brought = Array.isArray(kept?.brought) ? kept.brought.filter((file): file is string => typeof file === 'string' && file.length > 0 && file.length <= 500).slice(0, MAX_BROUGHT) : undefined
  // A judge (0.520): its route, its runs and the person's words, each checked like a column's.
  const judgeRecord = typeof record.judge === 'object' && record.judge !== null ? (record.judge as Record<string, unknown>) : undefined
  const judgeRoute = judgeRecord === undefined ? undefined : parsedSlot({ slot: 'a', route: judgeRecord.route, missionIds: judgeRecord.missionIds })
  const judgeCriteria = text(judgeRecord?.criteria, MAX_JUDGE_CRITERIA)
  // Its panel (0.765): each member checked as a judge is, its one run a mission id.
  const panel = Array.isArray(judgeRecord?.panel)
    ? judgeRecord.panel.flatMap((member: unknown) => {
        const held = (typeof member === 'object' && member !== null ? member : {}) as Record<string, unknown>
        const parsed = parsedSlot({ slot: 'a', route: held.route, missionIds: [held.missionId] })
        const missionId = parsed?.missionIds[0]
        return parsed === undefined || missionId === undefined ? [] : [{ route: parsed.route, missionId }]
      }).slice(0, PANEL_JUDGES)
    : []
  const panelMissionIds = Array.isArray(judgeRecord?.panelMissionIds)
    ? judgeRecord.panelMissionIds.filter((id): id is string => typeof id === 'string' && id.length > 0 && id.length <= 200).slice(-MAX_JUDGES * PANEL_JUDGES)
    : []
  const judge = judgeRoute === undefined || judgeRoute.missionIds.length === 0
    ? undefined
    : {
        route: judgeRoute.route,
        missionIds: judgeRoute.missionIds.slice(-MAX_JUDGES),
        ...(judgeCriteria === undefined ? {} : { criteria: judgeCriteria }),
        ...(panel.length === 0 ? {} : { panel }),
        ...(panelMissionIds.length === 0 ? {} : { panelMissionIds })
      }
  // Your own benchmark (0.763): the commit it started from, the comparison it runs again, its own runs again.
  const base = typeof record.base === 'string' && COMMIT.test(record.base) ? record.base : undefined
  const rerunOf = text(record.rerunOf, 80)
  const reruns = Array.isArray(record.reruns) ? record.reruns.filter((id): id is string => typeof id === 'string' && id.length > 0 && id.length <= 80).slice(-MAX_RERUNS) : []
  const pointDrafts = Array.isArray(record.pointDrafts) ? record.pointDrafts.filter((id): id is string => typeof id === 'string' && id.length > 0 && id.length <= 200).slice(-MAX_JUDGES) : []
  const keptSlot = kept !== undefined && isSlot(kept.slot) && keptAt !== undefined && slots.some((slot) => slot.slot === kept.slot)
    ? { slot: kept.slot, at: keptAt, ...(brought === undefined ? {} : { brought }) }
    : undefined
  return {
    compareId,
    ...(teammateId === undefined ? {} : { teammateId }),
    prompt,
    createdAt,
    slots,
    ...(record.changes === true ? { changes: true as const } : {}),
    ...(record.changes === true && (record.changesIn === 'copy' || record.changesIn === 'folder') ? { changesIn: record.changesIn } : {}),
    ...(record.blind === true ? { blind: true as const } : {}),
    ...(keptSlot === undefined ? {} : { kept: keptSlot }),
    ...(judge === undefined ? {} : { judge }),
    ...(base === undefined ? {} : { base }),
    ...(rerunOf === undefined ? {} : { rerunOf }),
    ...(reruns.length === 0 ? {} : { reruns }),
    ...(record.settled === true ? { settled: true as const } : {}),
    ...(pointDrafts.length === 0 ? {} : { pointDrafts })
  }
}

export function parsedCompareFile(raw: string): StoredFile {
  let value: unknown
  try {
    value = JSON.parse(raw)
  } catch {
    throw new Error(UNREADABLE)
  }
  if (typeof value !== 'object' || value === null) throw new Error(UNREADABLE)
  const record = value as Record<string, unknown>
  if (record.schemaVersion !== SCHEMA_VERSION || !Array.isArray(record.compares)) throw new Error(UNREADABLE)
  const compares: PublicCompare[] = []
  const seen = new Set<string>()
  for (const entry of record.compares) {
    const compare = parsedCompare(entry)
    if (compare === undefined || seen.has(compare.compareId)) continue
    seen.add(compare.compareId)
    compares.push(compare)
  }
  return { schemaVersion: SCHEMA_VERSION, compares: compares.slice(-MAX_COMPARES) }
}

export function createCompareStore(options: {
  readonly rootDirectory: string
  readonly now?: () => Date
  readonly createId?: () => string
  /** Where columns' copies live (compare-copies.ts COPY_ROOT): each column is told its folder. */
  readonly copyRoot?: string
}): CompareStore {
  const rootDirectory = options.rootDirectory
  if (!isAbsolute(rootDirectory)) throw new Error('Compare store directory is invalid')
  const path = join(rootDirectory, 'compares.json')
  const now = options.now ?? (() => new Date())
  const createId = options.createId ?? (() => `cmp_${randomUUID().replace(/-/g, '').slice(0, 16)}`)

  let queue: Promise<unknown> = Promise.resolve()
  const serialize = <T>(task: () => Promise<T>): Promise<T> => {
    const next = queue.then(task, task)
    queue = next.then(
      () => undefined,
      () => undefined
    )
    return next
  }

  const read = async (): Promise<StoredFile> => {
    let raw: string
    try {
      raw = await readFile(path, 'utf8')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return EMPTY
      throw new Error(UNREADABLE)
    }
    if (Buffer.byteLength(raw, 'utf8') > MAX_FILE_BYTES) throw new Error(UNREADABLE)
    return parsedCompareFile(raw)
  }

  const write = async (file: StoredFile): Promise<void> => {
    await mkdir(rootDirectory, { recursive: true, mode: 0o700 })
    const temporary = `${path}.${randomUUID()}.tmp`
    const handle = await open(temporary, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600)
    try {
      await handle.writeFile(`${JSON.stringify(file, null, 2)}\n`, 'utf8')
      await handle.sync()
    } finally {
      await handle.close()
    }
    try {
      await replaceFile(temporary, path)
    } catch (error) {
      await unlink(temporary).catch(() => undefined)
      throw error
    }
  }

  /** Change one comparison, or say it is gone. */
  const change = (compareId: string, edit: (compare: PublicCompare) => PublicCompare): Promise<PublicCompare> =>
    serialize(async () => {
      const file = await read()
      const found = file.compares.find((compare) => compare.compareId === compareId)
      if (found === undefined) throw new Error('That comparison is no longer here.')
      const changed = edit(found)
      await write({ schemaVersion: SCHEMA_VERSION, compares: file.compares.map((compare) => (compare.compareId === compareId ? changed : compare)) })
      return changed
    })

  const inSlot = (compare: PublicCompare, slot: CompareSlotId, edit: (column: PublicCompareSlot) => PublicCompareSlot): PublicCompare => {
    if (!compare.slots.some((column) => column.slot === slot)) throw new Error('That comparison has no such column.')
    return { ...compare, slots: compare.slots.map((column) => (column.slot === slot ? edit(column) : column)) }
  }

  const inner: CompareStore = {
    list: () => serialize(async () => (await read()).compares),

    get: (compareId) => serialize(async () => (await read()).compares.find((compare) => compare.compareId === compareId)),

    create: (input) =>
      serialize(async () => {
        const prompt = input.prompt.trim()
        for (const route of input.routes) {
          if (route.instructions !== undefined && (input.teammateId === undefined || instructionsText(route.instructions) === undefined)) throw new Error('Choose a teammate and valid column instructions. Nothing was started.')
        }
        if (prompt.length === 0 || prompt.length > MAX_PROMPT) throw new Error('There is nothing to compare.')
        if (input.routes.length < MIN_COMPARE_SLOTS || input.routes.length > MAX_COMPARE_SLOTS) {
          throw new Error(`A comparison is ${String(MIN_COMPARE_SLOTS)} or ${String(MAX_COMPARE_SLOTS)} models.`)
        }
        const file = await read()
        const compare: PublicCompare = {
          compareId: createId(),
          ...(input.teammateId === undefined ? {} : { teammateId: input.teammateId }),
          prompt,
          createdAt: now().toISOString(),
          ...(input.changes === true ? { changes: true as const } : {}),
          ...(input.changes === true && (input.changesIn === 'copy' || input.changesIn === 'folder') ? { changesIn: input.changesIn } : {}),
          ...(input.blind === true ? { blind: true as const } : {}),
          ...(input.base !== undefined && COMMIT.test(input.base) ? { base: input.base } : {}),
          ...(input.rerunOf === undefined ? {} : { rerunOf: input.rerunOf }),
          slots: input.routes.map((route, index) => ({
            slot: COMPARE_SLOTS[index]!,
            route: {
              runtime: route.runtime,
              model: route.model, ...(route.account === undefined ? {} : { account: route.account }),
              ...(route.effort === undefined ? {} : { effort: route.effort }),
              ...(route.instructions === undefined ? {} : { instructions: instructionsText(route.instructions)! }),
              ...(typeof route.why === 'string' && route.why.trim().length > 0 ? { why: route.why.trim().slice(0, 400) } : {}),
              ...(route.label === undefined ? {} : { label: route.label.slice(0, 120) }),
              ...(route.mode === 'auto' && input.changes === true ? { mode: 'auto' as const } : {})
            },
            missionIds: []
          }))
        }
        await write({ schemaVersion: SCHEMA_VERSION, compares: [...file.compares, compare].slice(-MAX_COMPARES) })
        return compare
      }),

    addTurn: (compareId, slot, missionId) =>
      change(compareId, (compare) =>
        inSlot(compare, slot, (column) => ({ ...column, missionIds: [...column.missionIds, missionId].slice(-MAX_TURNS_PER_SLOT) }))
      ),

    retry: (compareId, slot, missionId) =>
      change(compareId, (compare) =>
        inSlot(compare, slot, (column) => {
          const replaced = column.missionIds.at(-1)
          const { refused: _refused, ...rest } = column
          void _refused
          return replaced === undefined
            ? { ...rest, missionIds: [missionId] }
            : { ...rest, missionIds: [...column.missionIds.slice(0, -1), missionId], retried: [...(column.retried ?? []), replaced].slice(-MAX_RETRIED_PER_SLOT) }
        })
      ),

    refuse: (compareId, slot, why) => change(compareId, (compare) => inSlot(compare, slot, (column) => ({ ...column, refused: why.slice(0, 1_000) }))),

    keep: (compareId, slot, brought) =>
      change(compareId, (compare) => {
        const column = compare.slots.find((one) => one.slot === slot)
        if (column === undefined || column.missionIds.length === 0) throw new Error('That column has nothing to keep yet.')
        return { ...compare, kept: { slot, at: now().toISOString(), ...(brought === undefined ? {} : { brought: brought.slice(0, MAX_BROUGHT) }) } }
      }),

    addColumn: (compareId, route) =>
      change(compareId, (compare) => {
        if (compare.slots.length >= MAX_COMPARE_SLOTS) throw new Error(`A comparison is at most ${String(MAX_COMPARE_SLOTS)} models.`)
        if (compare.slots.some((column) => column.route.runtime === route.runtime && column.route.model === route.model && column.route.account === route.account)) throw new Error('That model is already in this comparison.')
        const slot = COMPARE_SLOTS.find((letter) => !compare.slots.some((column) => column.slot === letter))
        if (slot === undefined) throw new Error(`A comparison is at most ${String(MAX_COMPARE_SLOTS)} models.`)
        return {
          ...compare,
          slots: [...compare.slots, {
            slot,
            route: { runtime: route.runtime, model: route.model, ...(route.account === undefined ? {} : { account: route.account }), ...(route.effort === undefined ? {} : { effort: route.effort }), ...(route.label === undefined ? {} : { label: route.label.slice(0, 120) }) },
            missionIds: []
          }]
        }
      }),

    judge: (compareId, route, missionId, criteria, panel = []) =>
      change(compareId, (compare) => {
        const routeOf = (one: CompareRoute): CompareRoute => ({ runtime: one.runtime, model: one.model, ...(one.account === undefined ? {} : { account: one.account }), ...(one.effort === undefined ? {} : { effort: one.effort }), ...(one.label === undefined ? {} : { label: one.label.slice(0, 120) }) })
        const members = panel.slice(0, PANEL_JUDGES).map((member) => ({ route: routeOf(member.route), missionId: member.missionId }))
        // Every panel run stays the comparison's, so one asked over never turns up as a conversation.
        const heard = [...(compare.judge?.panelMissionIds ?? []), ...members.map((member) => member.missionId)].slice(-MAX_JUDGES * PANEL_JUDGES)
        return {
          ...compare,
          judge: {
            route: routeOf(route),
            missionIds: [...(compare.judge?.missionIds ?? []), missionId].slice(-MAX_JUDGES),
            ...(criteria === undefined || criteria.trim().length === 0 ? {} : { criteria: criteria.trim().slice(0, MAX_JUDGE_CRITERIA) }),
            ...(members.length === 0 ? {} : { panel: members }),
            ...(heard.length === 0 ? {} : { panelMissionIds: heard })
          }
        }
      }),

    addPointDraft: (compareId, missionId) =>
      change(compareId, (compare) => ({ ...compare, pointDrafts: [...(compare.pointDrafts ?? []), missionId].slice(-MAX_JUDGES) })),

    addRerun: (compareId, rerunId) =>
      change(compareId, (compare) => ({ ...compare, reruns: [...(compare.reruns ?? []).filter((id) => id !== rerunId), rerunId].slice(-MAX_RERUNS) })),

    settle: (compareId) => change(compareId, (compare) => ({ ...compare, settled: true as const })),

    remove: (compareId) =>
      serialize(async () => {
        const file = await read()
        await write({ schemaVersion: SCHEMA_VERSION, compares: file.compares.filter((compare) => compare.compareId !== compareId) })
      })
  }
  const copyRoot = options.copyRoot
  if (copyRoot === undefined) return inner
  const shown = (compare: PublicCompare): PublicCompare => withColumnFolders(compare, copyRoot)
  return {
    ...inner,
    list: async () => (await inner.list()).map(shown),
    get: async (compareId) => {
      const found = await inner.get(compareId)
      return found === undefined ? undefined : shown(found)
    },
    create: async (input) => shown(await inner.create(input)),
    addTurn: async (...args) => shown(await inner.addTurn(...args)),
    retry: async (...args) => shown(await inner.retry(...args)),
    refuse: async (...args) => shown(await inner.refuse(...args)),
    keep: async (...args) => shown(await inner.keep(...args)),
    addColumn: async (...args) => shown(await inner.addColumn(...args)),
    judge: async (...args) => shown(await inner.judge(...args)),
    addRerun: async (...args) => shown(await inner.addRerun(...args)),
    settle: async (...args) => shown(await inner.settle(...args))
  }
}

/**
 * WHERE EACH COLUMN WORKS (0.555), worked out, never stored. Colin, on a
 * comparison on Auto: Antigravity wrote optimization-review.md in its copy,
 * and the file's chip, looking in his folder, said "That file is not there.
 * The teammate named it but did not write it." A column that edits works in
 * its copy (or worktree) until one is kept; then the kept one's files are in
 * the folder and the copies are gone.
 */
export function withColumnFolders(compare: PublicCompare, copyRoot: string): PublicCompare {
  // A run again that is over has no copies left (0.763): its files are read nowhere, as a kept comparison's others are.
  if (compare.changes !== true || compare.changesIn === 'folder' || compare.kept !== undefined || compare.settled === true) return compare
  return { ...compare, slots: compare.slots.map((column) => ({ ...column, folder: join(copyRoot, compareTreeId(compare.compareId, column.slot)) })) }
}
