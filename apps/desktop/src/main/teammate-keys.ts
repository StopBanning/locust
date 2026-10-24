import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { keyNameRefusal, keyValueRefusal, MAX_KEYS_PER_TEAMMATE, type RunKey, type TeammateKeysResult } from '../shared/teammate-keys.js'
import { privateMcpFile } from './locust-mcp-host.js'
import type { SecretBox } from './own-models.js'
import { replaceFile } from './replace-file.js'

/*
 * KEYS A TEAMMATE NEEDS (0.833, shared/teammate-keys.ts). Each teammate's keys, by name, as the operating system
 * encrypted them, in one file that is the account's alone. Names come back to the window; values only to a run of
 * that teammate's own (runKeys), and to nothing else.
 */
export const TEAMMATE_KEYS_FILE = 'teammate-keys.json'

interface Saved {
  readonly schemaVersion: 1
  /** Each teammate's keys: the name, and the value as the operating system encrypted it, base64. */
  readonly teammates: Readonly<Record<string, Readonly<Record<string, string>>>>
}

export interface TeammateKeys {
  names(teammateId: string): Promise<readonly string[]>
  set(teammateId: unknown, name: unknown, value: unknown): Promise<TeammateKeysResult>
  remove(teammateId: unknown, name: unknown): Promise<TeammateKeysResult>
  /** A teammate's keys, decrypted, for one of its runs. One that will not decrypt here is left out. */
  runKeys(teammateId: string): Promise<readonly RunKey[]>
  /** A removed teammate's keys go with it. */
  forget(teammateId: string): Promise<void>
}

export function createTeammateKeys(options: {
  readonly directory: string
  readonly secrets: SecretBox
  readonly protectFile?: (path: string) => Promise<void>
}): TeammateKeys {
  const file = join(options.directory, TEAMMATE_KEYS_FILE)
  let writing: Promise<unknown> = Promise.resolve()

  const load = async (): Promise<Saved> => {
    const text = await readFile(file, 'utf8').catch(() => undefined)
    if (text === undefined) return { schemaVersion: 1, teammates: {} }
    try {
      const parsed = JSON.parse(text) as Partial<Saved>
      return parsed.schemaVersion === 1 && typeof parsed.teammates === 'object' && parsed.teammates !== null ? { schemaVersion: 1, teammates: parsed.teammates } : { schemaVersion: 1, teammates: {} }
    } catch { return { schemaVersion: 1, teammates: {} } }
  }
  const save = async (saved: Saved): Promise<void> => {
    await mkdir(options.directory, { recursive: true })
    const temp = `${file}.tmp`
    await rm(temp, { force: true })
    await writeFile(temp, '', { mode: 0o600, flag: 'wx' })
    // The encrypted keys are written only once the file is the account's alone.
    await (options.protectFile ?? privateMcpFile)(temp)
    await writeFile(temp, JSON.stringify(saved), { mode: 0o600 })
    await replaceFile(temp, file)
  }
  /** One change at a time, each reading what the last one wrote. */
  const serial = <T>(work: () => Promise<T>): Promise<T> => {
    const next = writing.then(work, work)
    writing = next.catch(() => undefined)
    return next
  }
  const namesOf = (saved: Saved, teammateId: string): readonly string[] => Object.keys(saved.teammates[teammateId] ?? {}).sort()

  return {
    async names(teammateId) {
      return namesOf(await load(), teammateId)
    },
    set(teammateId, name, value) {
      return serial(async () => {
        const saved = await load()
        const id = typeof teammateId === 'string' ? teammateId : ''
        const named = typeof name === 'string' ? name.trim() : ''
        const given = typeof value === 'string' ? value.trim() : ''
        const fail = (message: string): TeammateKeysResult => ({ ok: false, message, names: namesOf(saved, id) })
        if (id.length === 0) return fail('Choose a teammate first.')
        const refused = keyNameRefusal(named) ?? keyValueRefusal(given)
        if (refused !== undefined) return fail(refused)
        const kept = saved.teammates[id] ?? {}
        if (kept[named] === undefined && Object.keys(kept).length >= MAX_KEYS_PER_TEAMMATE) return fail(`A teammate keeps ${String(MAX_KEYS_PER_TEAMMATE)} keys at most. Remove one first.`)
        if (!options.secrets.available()) return fail('This computer cannot encrypt the key, so it was not kept.')
        const next: Saved = { schemaVersion: 1, teammates: { ...saved.teammates, [id]: { ...kept, [named]: options.secrets.encrypt(given).toString('base64') } } }
        try { await save(next) } catch { return fail('The key could not be kept on this computer. Nothing changed.') }
        return { ok: true, names: namesOf(next, id) }
      })
    },
    remove(teammateId, name) {
      return serial(async () => {
        const saved = await load()
        const id = typeof teammateId === 'string' ? teammateId : ''
        const kept = { ...(saved.teammates[id] ?? {}) }
        if (typeof name !== 'string' || kept[name] === undefined) return { ok: true, names: namesOf(saved, id) }
        delete kept[name]
        const teammates = { ...saved.teammates }
        if (Object.keys(kept).length === 0) delete teammates[id]
        else teammates[id] = kept
        const next: Saved = { schemaVersion: 1, teammates }
        try { await save(next) } catch { return { ok: false, message: 'The key could not be removed. It is unchanged.', names: namesOf(saved, id) } }
        return { ok: true, names: namesOf(next, id) }
      })
    },
    async runKeys(teammateId) {
      const kept = (await load()).teammates[teammateId] ?? {}
      if (!options.secrets.available()) return []
      const keys: RunKey[] = []
      for (const [name, cipher] of Object.entries(kept)) {
        if (keyNameRefusal(name) !== undefined) continue
        try { keys.push({ name, value: options.secrets.decrypt(Buffer.from(cipher, 'base64')) }) } catch { /* One that will not decrypt here is left out. */ }
      }
      return keys
    },
    forget(teammateId) {
      return serial(async () => {
        const saved = await load()
        if (saved.teammates[teammateId] === undefined) return
        const teammates = { ...saved.teammates }
        delete teammates[teammateId]
        await save({ schemaVersion: 1, teammates })
      })
    }
  }
}
