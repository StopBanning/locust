import { randomUUID } from 'node:crypto'
import { constants as fsConstants } from 'node:fs'
import { mkdir, open, readFile, unlink } from 'node:fs/promises'
import { isAbsolute, join } from 'node:path'

import type { ApprovalRule, RuleEffect, RuleKind } from '../shared/approval-rules.js'
import { replaceFile } from './replace-file.js'
import { secretIn } from '../shared/secrets.js'

/**
 * SAVED APPROVAL RULES, remembered (0.521, shared/approval-rules.ts).
 *
 * `userData/approval-rules.json`, kept the way compares.json is: writes
 * serialized, a temporary file synced and renamed into place, and a file
 * that cannot be read is UNREADABLE -- never empty, which would be written
 * back over the person's rules, and never "no rules", which would be read
 * as nothing to deny. While it is unreadable every card asks.
 */
const SCHEMA_VERSION = 1 as const
const MAX_FILE_BYTES = 1024 * 1024
export const MAX_RULES = 200
const UNREADABLE = 'The saved approval rules could not be read. Every card asks until they can.'

export interface StoredRule extends ApprovalRule {
  /** How many cards this rule answered, and when it last did: shown in Settings. */
  readonly uses?: number
  readonly lastUsedAt?: string
}

interface StoredFile {
  readonly schemaVersion: typeof SCHEMA_VERSION
  readonly rules: readonly StoredRule[]
}

export interface ApprovalRuleStore {
  list(): Promise<readonly StoredRule[]>
  add(rule: Omit<ApprovalRule, 'ruleId' | 'createdAt'>): Promise<StoredRule>
  remove(ruleId: unknown): Promise<void>
  /** Every rule at once (0.587): the file is left readable and empty, and every card asks again. Resolves to how many went. */
  removeAll(): Promise<number>
  /** A card was answered by this rule. */
  used(ruleId: string): Promise<void>
}

const EFFECTS: readonly RuleEffect[] = ['allow', 'deny']
const KINDS: readonly RuleKind[] = ['command', 'edit', 'read', 'connector']
const text = (value: unknown, max: number): string | undefined => (typeof value === 'string' && value.length > 0 && value.length <= max ? value : undefined)

function parsedRule(value: unknown): StoredRule | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  const ruleId = text(record.ruleId, 80)
  const pattern = text(record.pattern, 400)
  const createdAt = text(record.createdAt, 40)
  if (ruleId === undefined || pattern === undefined || createdAt === undefined) return undefined
  if (!EFFECTS.includes(record.effect as RuleEffect) || !KINDS.includes(record.kind as RuleKind)) return undefined
  const teammateId = text(record.teammateId, 200)
  const folder = text(record.folder, 1_024)
  if ((record.teammateId !== undefined && teammateId === undefined) || (record.folder !== undefined && (folder === undefined || !isAbsolute(folder)))) return undefined
  const uses = typeof record.uses === 'number' && Number.isSafeInteger(record.uses) && record.uses >= 0 ? record.uses : undefined
  const lastUsedAt = text(record.lastUsedAt, 40)
  return {
    ruleId,
    effect: record.effect as RuleEffect,
    kind: record.kind as RuleKind,
    pattern,
    createdAt,
    ...(teammateId === undefined ? {} : { teammateId }),
    ...(folder === undefined ? {} : { folder }),
    ...(uses === undefined ? {} : { uses }),
    ...(lastUsedAt === undefined ? {} : { lastUsedAt })
  }
}

export function parsedRuleFile(raw: string): StoredFile {
  let value: unknown
  try {
    value = JSON.parse(raw)
  } catch {
    throw new Error(UNREADABLE)
  }
  if (typeof value !== 'object' || value === null) throw new Error(UNREADABLE)
  const record = value as Record<string, unknown>
  if (record.schemaVersion !== SCHEMA_VERSION || !Array.isArray(record.rules)) throw new Error(UNREADABLE)
  // A rule that does not read is dropped, never half-applied: a deny with a
  // missing field must not become an allow with a default.
  const rules = record.rules.map(parsedRule)
  if (rules.some((rule) => rule === undefined)) throw new Error(UNREADABLE)
  return { schemaVersion: SCHEMA_VERSION, rules: rules as StoredRule[] }
}

export function createApprovalRuleStore(options: { readonly rootDirectory: string; readonly now?: () => Date }): ApprovalRuleStore {
  if (!isAbsolute(options.rootDirectory)) throw new Error('Approval rule store directory is invalid')
  const path = join(options.rootDirectory, 'approval-rules.json')
  const now = options.now ?? (() => new Date())
  let queue: Promise<unknown> = Promise.resolve()
  const serialize = <T>(task: () => Promise<T>): Promise<T> => {
    const next = queue.then(task, task)
    queue = next.then(() => undefined, () => undefined)
    return next
  }
  const read = async (): Promise<StoredFile> => {
    let raw: string
    try {
      raw = await readFile(path, 'utf8')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { schemaVersion: SCHEMA_VERSION, rules: [] }
      throw new Error(UNREADABLE)
    }
    if (Buffer.byteLength(raw, 'utf8') > MAX_FILE_BYTES) throw new Error(UNREADABLE)
    return parsedRuleFile(raw)
  }
  const write = async (rules: readonly StoredRule[]): Promise<void> => {
    await mkdir(options.rootDirectory, { recursive: true, mode: 0o700 })
    const temporary = `${path}.${randomUUID()}.tmp`
    const handle = await open(temporary, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600)
    try {
      await handle.writeFile(`${JSON.stringify({ schemaVersion: SCHEMA_VERSION, rules }, null, 2)}\n`, 'utf8')
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
  return {
    list: () => serialize(async () => (await read()).rules),
    add: (input) =>
      serialize(async () => {
        const candidate = parsedRule({ ...input, ruleId: `rule_${randomUUID().replace(/-/g, '').slice(0, 16)}`, createdAt: now().toISOString() })
        if (candidate === undefined) throw new Error('That rule could not be saved.')
        // A rule is the command as written, kept for good (0.870): one carrying a key would keep the key too.
        const secret = secretIn(candidate.pattern)
        if (secret !== undefined) throw new Error(`That command holds ${secret}, so it isn't saved as a rule: a rule keeps the command as written. Allow it once instead.`)
        const file = await read()
        // The same rule twice is one rule.
        const same = file.rules.find((rule) => rule.effect === candidate.effect && rule.kind === candidate.kind && rule.pattern === candidate.pattern && rule.teammateId === candidate.teammateId && rule.folder === candidate.folder)
        if (same !== undefined) return same
        if (file.rules.length >= MAX_RULES) throw new Error(`${MAX_RULES} rules is the most Locust keeps; remove one first.`)
        await write([...file.rules, candidate])
        return candidate
      }),
    remove: (ruleId) =>
      serialize(async () => {
        const file = await read()
        await write(file.rules.filter((rule) => rule.ruleId !== ruleId))
      }),
    removeAll: () =>
      serialize(async () => {
        const file = await read()
        await write([])
        return file.rules.length
      }),
    used: (ruleId) =>
      serialize(async () => {
        const file = await read()
        await write(file.rules.map((rule) => (rule.ruleId === ruleId ? { ...rule, uses: (rule.uses ?? 0) + 1, lastUsedAt: now().toISOString() } : rule)))
      })
  }
}
