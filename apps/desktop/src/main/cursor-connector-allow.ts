import { mkdir, readFile, rmdir, stat, unlink, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'

/**
 * Let a Cursor run outside Auto call the connectors the person configured.
 *
 * MEASURED 2026-09-15/16 in Colin's own ledger: 17, 5, 3 and 2 `user
 * rejected MCP` per run in Accept edits, 0 in Auto. `--approve-mcps` approves
 * the SERVER; each CALL still asks, and a print-mode run has nobody to answer,
 * so every call resolved to "no". The effective policy was not "safer", it
 * was "always no" -- for servers the person set up themselves and that
 * Cursor's own app uses freely.
 *
 * Making Accept edits pass `--force` would be wrong: that is Auto for shell
 * commands under another name. Cursor's CLI reads permissions from
 * `<workspace>/.cursor/cli.json` -- `Mcp(<server>:<tool>)` patterns with `*`
 * wildcards, deny winning over allow (cursor.com/docs/cli/reference/
 * permissions) -- the same shape Colin's own `~/.cursor/cli-config.json`
 * already holds four Robinhood tools in. So at the start of a Cursor run
 * outside Auto, `Mcp(<server>:*)` for each configured server is merged into
 * that file's allow list. `deny` is never touched, and a file that does not
 * parse is left alone rather than written over.
 *
 * TAKEN BACK WHEN THE RUN ENDS. The rules used to stay for good, so a later
 * Ask run in the same folder inherited connector access it was never given,
 * and the folder kept a file the person did not write. Now a grant comes with
 * a `release`: it removes exactly the rules THIS run added -- not ones that
 * were there before, not ones the person added meanwhile, not `deny` -- and
 * deletes the file (and `.cursor/`) when the run created them and nothing
 * else is left in them. Two runs in one folder hold a rule together: it goes
 * when the last holder ends.
 *
 * Colin, 2026-09-10: "just let them have access to the mcp tools if the
 * client has access to it." The mode governs this machine; a connector the
 * person set up in Cursor themselves is not this machine. Decided 2026-09-17
 * (`docs/HARNESS-COMMUNICATION-2026-09-17.md`).
 */

export const CURSOR_PROJECT_CONFIG = join('.cursor', 'cli.json')

export function cursorAllowRule(server: string): string {
  return `Mcp(${server}:*)`
}

export interface MergedCursorConfig {
  /** The file to write, or undefined when nothing needs writing. */
  readonly text: string | undefined
  readonly added: readonly string[]
}

/**
 * The file with the missing rules added, or `undefined` when the file cannot
 * be read as the config it is supposed to be. An absent file is an empty one.
 */
export function mergedCursorConfig(text: string | undefined, servers: readonly string[]): MergedCursorConfig | undefined {
  let value: unknown = {}
  if (text !== undefined && text.trim().length > 0) {
    try {
      value = JSON.parse(text) as unknown
    } catch {
      return undefined
    }
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  const permissions = record.permissions
  if (permissions !== undefined && (typeof permissions !== 'object' || permissions === null || Array.isArray(permissions))) return undefined
  const held = (permissions ?? {}) as Record<string, unknown>
  if (held.allow !== undefined && !Array.isArray(held.allow)) return undefined
  const allow = [...((held.allow ?? []) as unknown[])]
  const added: string[] = []
  for (const server of servers) {
    const rule = cursorAllowRule(server)
    if (!allow.includes(rule) && !allow.includes('Mcp(*:*)')) {
      allow.push(rule)
      added.push(rule)
    }
  }
  if (added.length === 0) return { text: undefined, added }
  const next = { ...record, permissions: { ...held, allow, ...(held.deny === undefined ? { deny: [] } : {}) } }
  return { text: `${JSON.stringify(next, null, 2)}\n`, added }
}

export interface AllowIo {
  readonly readFile: (path: string) => Promise<string>
  readonly writeFile: (path: string, text: string) => Promise<void>
  readonly mkdir: (path: string) => Promise<unknown>
  /** Whether the path exists at all. */
  readonly exists: (path: string) => Promise<boolean>
  readonly removeFile: (path: string) => Promise<void>
  /** Removes a directory only if it is empty; throws otherwise. */
  readonly removeDir: (path: string) => Promise<void>
}

const nodeIo: AllowIo = {
  readFile: (path) => readFile(path, 'utf8'),
  writeFile: (path, text) => writeFile(path, text, 'utf8'),
  mkdir: (path) => mkdir(path, { recursive: true }),
  exists: (path) => stat(path).then(() => true, () => false),
  removeFile: (path) => unlink(path),
  removeDir: (path) => rmdir(path)
}

/** What a run was given, and how it gives it back. */
export interface CursorConnectorGrant {
  /** The rules this run added to the file (not the ones it merely relies on). */
  readonly added: readonly string[]
  /** Idempotent. Never throws. Call it when the run ends, however it ends. */
  readonly release: () => Promise<void>
}

const NO_GRANT: CursorConnectorGrant = { added: [], release: async () => undefined }

/** What the file looked like before the first run in this folder touched it. */
interface Original {
  readonly createdFile: boolean
  readonly createdDir: boolean
  readonly hadPermissions: boolean
  readonly hadAllow: boolean
  readonly hadDeny: boolean
}

interface Folder {
  readonly original: Original
  /** Rules this process put in the file, and how many live runs hold each. */
  readonly holds: Map<string, number>
}

export interface CursorConnectorKeeper {
  readonly allow: (workspace: string, servers: readonly string[]) => Promise<CursorConnectorGrant>
  /**
   * Take back the rules a run of an EARLIER Locust left behind (0.588): one
   * that was live when Locust quit or crashed never reached its `release`, so
   * its rules stayed and the next run in that folder -- in Ask, even -- had
   * connector access it was never given. Called once at start, before any
   * run; resolves to the files it changed.
   */
  readonly recover: () => Promise<readonly string[]>
  /**
   * Whether a live run of ours holds connector rules in this folder now (0.825). Cursor reads them from the folder,
   * not from the run, so a turn in Ask or Plan started there meanwhile would have them too: it is not started.
   */
  readonly holds: (workspace: string) => boolean
}

/**
 * Where the keeper writes what it holds, so a Locust that quits mid-run can
 * take the rules back when it next starts. A file in the profile; its
 * failure never fails a run.
 */
export interface HoldStore {
  readonly read: () => Promise<string | undefined>
  readonly write: (text: string) => Promise<void>
}

const HOLD_STORE_VERSION = 1

/** The store's shape: each file the keeper's rules are in, and how the file looked before. */
interface StoredHolds {
  readonly version: typeof HOLD_STORE_VERSION
  readonly folders: Readonly<Record<string, { readonly original: Original; readonly rules: readonly string[] }>>
}

function parseStoredHolds(text: string | undefined): StoredHolds | undefined {
  if (text === undefined || text.trim().length === 0) return { version: HOLD_STORE_VERSION, folders: {} }
  let value: unknown
  try {
    value = JSON.parse(text) as unknown
  } catch {
    return undefined
  }
  if (!isRecord(value) || value.version !== HOLD_STORE_VERSION || !isRecord(value.folders)) return undefined
  const folders: Record<string, { readonly original: Original; readonly rules: readonly string[] }> = {}
  for (const [path, held] of Object.entries(value.folders)) {
    if (!isRecord(held) || !Array.isArray(held.rules) || !isRecord(held.original)) continue
    const rules = held.rules.filter((rule): rule is string => typeof rule === 'string')
    if (rules.length === 0) continue
    const original = held.original
    const flag = (name: string): boolean => original[name] === true
    folders[path] = {
      original: { createdFile: flag('createdFile'), createdDir: flag('createdDir'), hadPermissions: flag('hadPermissions'), hadAllow: flag('hadAllow'), hadDeny: flag('hadDeny') },
      rules
    }
  }
  return { version: HOLD_STORE_VERSION, folders }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/**
 * The file with `rules` taken out of its allow list; `text` is `undefined`
 * when the file should be deleted (it only ever held what this run put
 * there). `'unreadable'` when the file no longer parses as the config.
 */
function withoutRules(text: string, rules: readonly string[], original: Original): { readonly text: string | undefined } | 'unreadable' {
  let value: unknown
  try {
    value = JSON.parse(text) as unknown
  } catch {
    return 'unreadable'
  }
  if (!isRecord(value)) return 'unreadable'
  const record: Record<string, unknown> = { ...value }
  const permissionsValue = record.permissions
  if (permissionsValue !== undefined && !isRecord(permissionsValue)) return 'unreadable'
  const permissions: Record<string, unknown> = { ...(permissionsValue ?? {}) }
  if (permissions.allow !== undefined && !Array.isArray(permissions.allow)) return 'unreadable'
  if (Array.isArray(permissions.allow)) {
    const gone = new Set(rules)
    permissions.allow = (permissions.allow as unknown[]).filter((rule) => typeof rule !== 'string' || !gone.has(rule))
  }
  // Give back the empty containers this run made, and only those.
  if (Array.isArray(permissions.allow) && permissions.allow.length === 0 && !original.hadAllow) delete permissions.allow
  if (Array.isArray(permissions.deny) && permissions.deny.length === 0 && !original.hadDeny) delete permissions.deny
  if (Object.keys(permissions).length === 0 && !original.hadPermissions) delete record.permissions
  else record.permissions = permissions
  if (Object.keys(record).length === 0 && original.createdFile) return { text: undefined }
  return { text: `${JSON.stringify(record, null, 2)}\n` }
}

export function createCursorConnectorKeeper(io: AllowIo = nodeIo, store?: HoldStore): CursorConnectorKeeper {
  const folders = new Map<string, Folder>()
  // What is held, written down after every change (0.588); a store that cannot be written is not a run's problem.
  const remember = async (): Promise<void> => {
    if (store === undefined) return
    const held: StoredHolds = {
      version: HOLD_STORE_VERSION,
      folders: Object.fromEntries([...folders].map(([path, folder]) => [path, { original: folder.original, rules: [...folder.holds.keys()] }]))
    }
    await store.write(`${JSON.stringify(held, null, 2)}\n`).catch(() => undefined)
  }
  // One read-modify-write at a time per file, so two runs starting (or
  // ending) together cannot overwrite each other's change.
  const queues = new Map<string, Promise<unknown>>()
  const serial = <T>(path: string, work: () => Promise<T>): Promise<T> => {
    const next = (queues.get(path) ?? Promise.resolve()).then(work, work)
    queues.set(path, next.catch(() => undefined))
    return next
  }

  async function take(path: string, servers: readonly string[]): Promise<CursorConnectorGrant> {
    let text: string | undefined
    try {
      text = await io.readFile(path)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') return NO_GRANT
    }
    const held = folders.get(path)
    // Rules a live run of ours already put there: this run relies on them too.
    const relied = held === undefined ? [] : servers.map(cursorAllowRule).filter((rule) => held.holds.has(rule))
    const merged = mergedCursorConfig(text, servers)
    if (merged === undefined) return NO_GRANT
    let folder = held
    if (merged.added.length > 0) {
      if (folder === undefined) {
        const parsed = text === undefined || text.trim().length === 0 ? {} : (JSON.parse(text) as Record<string, unknown>)
        const permissions = isRecord(parsed.permissions) ? parsed.permissions : undefined
        const dir = dirname(path)
        folder = {
          original: {
            createdFile: text === undefined,
            createdDir: text === undefined && !(await io.exists(dir)),
            hadPermissions: permissions !== undefined,
            hadAllow: permissions?.allow !== undefined,
            hadDeny: permissions?.deny !== undefined
          },
          holds: new Map()
        }
      }
      await io.mkdir(dirname(path))
      await io.writeFile(path, merged.text!)
      folders.set(path, folder)
    }
    if (folder === undefined) return NO_GRANT
    const mine = [...merged.added, ...relied]
    if (mine.length === 0) return NO_GRANT
    for (const rule of mine) folder.holds.set(rule, (folder.holds.get(rule) ?? 0) + 1)
    await remember()
    let released = false
    return {
      added: merged.added,
      release: () => {
        if (released) return Promise.resolve()
        released = true
        return serial(path, () => giveBack(path, mine)).catch(() => undefined)
      }
    }
  }

  async function giveBack(path: string, mine: readonly string[]): Promise<void> {
    const folder = folders.get(path)
    if (folder === undefined) return
    const last: string[] = []
    for (const rule of mine) {
      const count = (folder.holds.get(rule) ?? 0) - 1
      if (count > 0) folder.holds.set(rule, count)
      else {
        folder.holds.delete(rule)
        last.push(rule)
      }
    }
    if (folder.holds.size === 0) folders.delete(path)
    await remember()
    if (last.length === 0) return
    let text: string
    try {
      text = await io.readFile(path)
    } catch {
      return // already gone: nothing to take back
    }
    const taken = withoutRules(text, last, folder.original)
    if (taken === 'unreadable') return // never written over
    if (taken.text === undefined) {
      await io.removeFile(path)
      if (folder.original.createdDir) await io.removeDir(dirname(path)).catch(() => undefined)
      return
    }
    if (taken.text !== text) await io.writeFile(path, taken.text)
  }

  async function recover(): Promise<readonly string[]> {
    if (store === undefined) return []
    const stored = parseStoredHolds(await store.read().catch(() => undefined))
    // A store that does not read as itself is left as it is: never written over, never acted on.
    if (stored === undefined) return []
    const changed: string[] = []
    for (const [path, held] of Object.entries(stored.folders)) {
      await serial(path, async () => {
        // A live run of THIS process holds the file: its own release will see to it.
        if (folders.has(path)) return
        let text: string
        try {
          text = await io.readFile(path)
        } catch {
          return // already gone
        }
        const taken = withoutRules(text, held.rules, held.original)
        if (taken === 'unreadable') return
        if (taken.text === undefined) {
          await io.removeFile(path)
          if (held.original.createdDir) await io.removeDir(dirname(path)).catch(() => undefined)
          changed.push(path)
          return
        }
        if (taken.text !== text) {
          await io.writeFile(path, taken.text)
          changed.push(path)
        }
      }).catch(() => undefined)
    }
    if (Object.keys(stored.folders).length > 0) await remember()
    return changed
  }

  return {
    allow: (workspace, servers) => {
      if (servers.length === 0) return Promise.resolve(NO_GRANT)
      const path = join(workspace, CURSOR_PROJECT_CONFIG)
      return serial(path, () => take(path, servers)).catch(() => NO_GRANT)
    },
    recover,
    holds: (workspace) => (folders.get(join(workspace, CURSOR_PROJECT_CONFIG))?.holds.size ?? 0) > 0
  }
}

const keepers = new WeakMap<AllowIo, CursorConnectorKeeper>()

/**
 * Merge the rules into the workspace's Cursor config. `added` is what was
 * added this time; empty means they were already there, there were no
 * servers, or the file could not be read (never written over). Call
 * `release` when the run ends.
 */
export async function allowCursorConnectors(
  workspace: string,
  servers: readonly string[],
  io: AllowIo = nodeIo
): Promise<CursorConnectorGrant> {
  let keeper = keepers.get(io)
  if (keeper === undefined) {
    keeper = createCursorConnectorKeeper(io)
    keepers.set(io, keeper)
  }
  return keeper.allow(workspace, servers)
}
