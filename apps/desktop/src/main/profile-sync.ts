/**
 * YOUR LOCUST ON EVERY COMPUTER YOU SIGN IN TO (docs/PLAN-2026-10-30-ACCOUNTS.md, slice 2). Colin: "we need accounts,
 * because I don't have any of my chats ... a user is not going to have that same interaction."
 *
 * A signed-in computer keeps a current copy of its profile at Locust's relay (apps/relay/src/sync.ts). What it carries
 * is exactly what a backup carries (profile-backup.ts carriedFiles): named files and folders, never "everything but",
 * so nothing in LEFT_OUT -- agent sign-ins, phone keys, own-model keys, checkpoints -- can reach the relay without a
 * decision there first. Every piece is sealed on this computer (profile-sync-crypto.ts) before it leaves.
 *
 * PUSH: each carried file is cut into CHUNK_BYTES pieces at fixed offsets, and only a piece whose hash changed since
 * the last push goes up. A conversation is only ever appended to, so a new turn sends its last piece and nothing else.
 * The manifest -- every file's size and hash, and each piece's -- is sealed and sent LAST, as a backup's is written
 * last, so a copy is never described before its pieces are there. Pieces no longer named are removed after it.
 *
 * PULL: the manifest, then every piece, each checked against its hash, written out as a BACKUP FOLDER (a profile/ and
 * a locust-backup.json). The restore that already exists then reads it with the same reader as a backup chosen from
 * disk, refuses what it refuses, and applies it at the next start with what it replaces moved aside. Sync adds no
 * second way to change a profile.
 *
 * v1 is each computer keeping its own copy, not two computers merged: a new computer starts from one copy.
 */
import { createHash } from 'node:crypto'
import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { BACKUP_MANIFEST, BACKUP_SCHEMA, carriedFiles, carries, countProfile, LEFT_OUT, type BackupCounts, type BackupManifest } from './profile-backup.js'
import { chunkIdOf, chunksOf, MANIFEST_ID, openFile, sealFile, type SyncKeys } from './profile-sync-crypto.js'

/** What this computer last sent: the next push sends only what differs from it. */
export const PROFILE_SYNC_FILE = 'profile-sync.json'
export const SYNC_SCHEMA = 1
/** A sync waits for this long after the last change, so a run's burst of writes goes up once. */
export const SYNC_QUIET_MS = 60_000

export interface SyncPiece { readonly bytes: number; readonly sha256: string }
export interface SyncedFile { readonly path: string; readonly bytes: number; readonly sha256: string; readonly pieces: readonly SyncPiece[] }

/** The copy's own list, sealed under MANIFEST_ID. */
export interface SyncManifest {
  readonly schema: number
  readonly app: 'Locust'
  readonly appVersion: string
  readonly createdAt: string
  /** The computer the copy is of, in the person's words: "Colin's PC". */
  readonly computer: string
  readonly counts: BackupCounts
  readonly files: readonly SyncedFile[]
}

interface SyncState {
  readonly schema: number
  readonly account: number
  readonly room: string
  readonly pushedAt: string
  readonly files: readonly SyncedFile[]
}

/** Why the relay would not take or give a piece, in words the window can show. */
export class SyncRefused extends Error {
  constructor(readonly status: number, message: string) {
    super(message)
  }
}

/** The few things sync needs of the relay; httpSyncRemote is the real one. */
export interface SyncRemote {
  put(id: string, box: Uint8Array): Promise<void>
  get(from: string, id: string): Promise<Uint8Array | undefined>
  remove(id: string): Promise<void>
}

/** What Settings › Your account asks of the relay besides: the account's storage, and deleting every copy of it. */
export interface SyncAccountRemote extends SyncRemote {
  usage(): Promise<{ readonly used: number; readonly quota: number }>
  /** Every computer of the account that keeps a copy: its room, its bytes, and when it last changed. */
  copies(): Promise<readonly { readonly room: string; readonly bytes: number; readonly at: number }[]>
  removeAll(): Promise<void>
}

export type SyncPushed =
  | { readonly ok: true; readonly sent: number; readonly bytes: number; readonly removed: number; readonly files: number; readonly at: string }
  | { readonly ok: false; readonly reason: string }

export type SyncPulled =
  | { readonly ok: true; readonly folder: string; readonly computer: string; readonly createdAt: string; readonly counts: BackupCounts; readonly files: number; readonly bytes: number }
  | { readonly ok: false; readonly reason: string }

const hash = (bytes: Uint8Array): string => createHash('sha256').update(bytes).digest('hex')

/** A path sync may carry: one a backup carries, and nothing a backup leaves out, however it came to be listed. */
export function syncable(path: string): boolean {
  const [first] = path.split('/')
  return carries(path) && !Object.hasOwn(LEFT_OUT, first ?? '') && !path.split('/').some((part) => part === '..' || part === '')
}

async function readState(profile: string, account: number, room: string): Promise<SyncState | undefined> {
  try {
    const state = JSON.parse(await readFile(join(profile, PROFILE_SYNC_FILE), 'utf8')) as SyncState
    // Signed in as another account, or a new room: the copy at the relay is not the one this state describes.
    return state.schema === SYNC_SCHEMA && state.account === account && state.room === room && Array.isArray(state.files) ? state : undefined
  } catch {
    return undefined
  }
}

/** What this computer last sent, for "Last synced": when, how many files and their bytes; undefined before the first sync. */
export async function lastSync(profile: string, account: number, room: string): Promise<{ readonly at: string; readonly files: number; readonly bytes: number } | undefined> {
  const state = await readState(profile, account, room)
  return state === undefined ? undefined : { at: state.pushedAt, files: state.files.length, bytes: state.files.reduce((sum, file) => sum + file.bytes, 0) }
}

/** Forgets what was sent, so the next sync sends everything: after the copy at the relay is deleted. */
export async function forgetSync(profile: string): Promise<void> {
  await rm(join(profile, PROFILE_SYNC_FILE), { force: true })
}

const said = (error: unknown): string => (error instanceof SyncRefused ? error.message : 'Locust’s relay could not be reached. Your Locust is safe here; the next sync tries again.')

/**
 * Bring this computer's copy at the relay up to date. Only changed pieces go up; nothing is written to the state
 * until the manifest naming them is up, so a push that stops halfway is simply done again.
 */
export async function pushProfile(profile: string, options: {
  readonly keys: SyncKeys
  readonly remote: SyncRemote
  readonly room: string
  readonly computer: string
  readonly appVersion: string
  readonly now: () => Date
}): Promise<SyncPushed> {
  const { keys, remote } = options
  const before = await readState(profile, keys.account, options.room)
  const sentBefore = new Map((before?.files ?? []).map((file) => [file.path, file] as const))
  const files: SyncedFile[] = []
  let sent = 0
  let bytes = 0
  try {
    for (const path of (await carriedFiles(profile)).filter(syncable)) {
      // A file removed between the listing and the read (a conversation deleted) is simply not in this copy.
      const read = await readFile(join(profile, ...path.split('/'))).catch(() => undefined)
      if (read === undefined) continue
      const content = new Uint8Array(read)
      const pieces: SyncPiece[] = []
      const old = sentBefore.get(path)
      for (const [index, [start, end]] of chunksOf(content.length).entries()) {
        const piece = content.subarray(start, end)
        const entry = { bytes: piece.length, sha256: hash(piece) }
        pieces.push(entry)
        const was = old?.pieces[index]
        if (was !== undefined && was.sha256 === entry.sha256 && was.bytes === entry.bytes) continue
        const id = await chunkIdOf(keys, path, index)
        await remote.put(id, await sealFile(keys, id, piece))
        sent += 1
        bytes += piece.length
      }
      files.push({ path, bytes: content.length, sha256: hash(content), pieces })
    }
    const at = options.now().toISOString()
    const manifest: SyncManifest = { schema: SYNC_SCHEMA, app: 'Locust', appVersion: options.appVersion, createdAt: at, computer: options.computer, counts: await countProfile(profile), files }
    const manifestId = MANIFEST_ID
    await remote.put(manifestId, await sealFile(keys, manifestId, new TextEncoder().encode(JSON.stringify(manifest))))
    // Only once the manifest no longer names them: a file gone, or a file now in fewer pieces.
    let removed = 0
    const kept = new Map(files.map((file) => [file.path, file.pieces.length] as const))
    for (const file of before?.files ?? []) {
      for (let index = kept.get(file.path) ?? 0; index < file.pieces.length; index += 1) {
        await remote.remove(await chunkIdOf(keys, file.path, index))
        removed += 1
      }
    }
    const state: SyncState = { schema: SYNC_SCHEMA, account: keys.account, room: options.room, pushedAt: at, files }
    await writeFile(join(profile, PROFILE_SYNC_FILE), `${JSON.stringify(state)}\n`, 'utf8')
    return { ok: true, sent, bytes, removed, files: files.length, at }
  } catch (error) {
    return { ok: false, reason: said(error) }
  }
}

/**
 * Another computer's copy (or this one's), opened into `folder` as a backup folder for the existing restore to read.
 * The folder must be new or empty. Every piece is checked against the manifest; a piece that changed while it was
 * read (that computer synced in between) refuses the whole copy rather than restoring half of two.
 */
export async function pullProfile(options: {
  readonly keys: SyncKeys
  readonly remote: SyncRemote
  readonly from: string
  readonly folder: string
}): Promise<SyncPulled> {
  const { keys, remote, folder } = options
  try {
    if ((await readdir(folder).catch(() => [])).length > 0) return { ok: false, reason: 'That folder already has files in it.' }
    const manifestId = MANIFEST_ID
    const box = await remote.get(options.from, manifestId)
    if (box === undefined) return { ok: false, reason: 'That computer has not kept a copy yet.' }
    let manifest: SyncManifest
    try {
      manifest = JSON.parse(new TextDecoder().decode(await openFile(keys, manifestId, box))) as SyncManifest
    } catch {
      return { ok: false, reason: 'That copy did not open with this key. Check the recovery code, or approve this computer from one that has your Locust.' }
    }
    if (manifest.app !== 'Locust' || typeof manifest.schema !== 'number' || manifest.schema > SYNC_SCHEMA || !Array.isArray(manifest.files)) {
      return { ok: false, reason: `That copy was made by a newer Locust (${String(manifest.appVersion)}). Update Locust, then try again.` }
    }
    const entries: BackupManifest['files'][number][] = []
    let bytes = 0
    for (const file of manifest.files) {
      if (typeof file.path !== 'string' || !syncable(file.path)) return { ok: false, reason: `That copy names a file it should not carry: ${String(file.path)}.` }
      const parts: Uint8Array[] = []
      for (const [index, piece] of file.pieces.entries()) {
        const id = await chunkIdOf(keys, file.path, index)
        const held = await remote.get(options.from, id)
        const opened = held === undefined ? undefined : await openFile(keys, id, held).catch(() => undefined)
        if (opened === undefined || opened.length !== piece.bytes || hash(opened) !== piece.sha256) {
          return { ok: false, reason: `${manifest.computer} was syncing while this was read. Try again in a minute.` }
        }
        parts.push(opened)
      }
      const content = Buffer.concat(parts)
      if (content.length !== file.bytes || hash(content) !== file.sha256) return { ok: false, reason: `${file.path} in that copy does not match its own list.` }
      const to = join(folder, 'profile', ...file.path.split('/'))
      await mkdir(join(to, '..'), { recursive: true })
      await writeFile(to, content)
      entries.push({ path: file.path, bytes: file.bytes, sha256: file.sha256 })
      bytes += file.bytes
    }
    const backup: BackupManifest = {
      schema: BACKUP_SCHEMA,
      app: 'Locust',
      appVersion: manifest.appVersion,
      createdAt: manifest.createdAt,
      counts: manifest.counts,
      files: entries,
      leftOut: Object.keys(LEFT_OUT)
    }
    await mkdir(folder, { recursive: true })
    await writeFile(join(folder, BACKUP_MANIFEST), `${JSON.stringify(backup, null, 2)}\n`, 'utf8')
    return { ok: true, folder, computer: manifest.computer, createdAt: manifest.createdAt, counts: manifest.counts, files: entries.length, bytes }
  } catch (error) {
    return { ok: false, reason: said(error) }
  }
}

/** The relay's sync routes (apps/relay/src/room.ts syncFetch), with this computer's own room token as the bearer. */
export function httpSyncRemote(config: { readonly relay: string; readonly room: string; readonly token: string; readonly fetch?: typeof fetch }): SyncAccountRemote {
  const go = config.fetch ?? fetch
  const base = `${config.relay.replace(/\/+$/, '')}/v1/room/${config.room}/sync`
  const authorization = `Bearer ${config.token}`
  const refused = async (answer: Response): Promise<SyncRefused> => {
    const error = ((await answer.json().catch(() => ({}))) as { error?: unknown }).error
    if (answer.status === 507) return new SyncRefused(507, 'Your copy on Locust’s relay is full (1 GB). Your Locust is safe here; nothing more is sent until there is room.')
    if (answer.status === 429) return new SyncRefused(429, 'Locust synced a lot this hour. The next sync carries on.')
    if (answer.status === 401 || answer.status === 403) return new SyncRefused(answer.status, 'Locust’s relay did not know this computer. Sign in again in Settings.')
    if (answer.status === 501) return new SyncRefused(501, 'Locust’s relay does not keep copies yet.')
    return new SyncRefused(answer.status, `Locust’s relay answered ${String(answer.status)}${typeof error === 'string' ? ` (${error})` : ''}. The next sync tries again.`)
  }
  return {
    async put(id, box) {
      const answer = await go(`${base}/${id}`, { method: 'PUT', headers: { authorization, 'content-type': 'application/octet-stream' }, body: box })
      if (!answer.ok) throw await refused(answer)
    },
    async get(from, id) {
      const answer = await go(`${base}/${from}/${id}`, { method: 'GET', headers: { authorization } })
      if (answer.status === 404) return undefined
      if (!answer.ok) throw await refused(answer)
      return new Uint8Array(await answer.arrayBuffer())
    },
    async remove(id) {
      const answer = await go(`${base}/${id}`, { method: 'DELETE', headers: { authorization } })
      if (!answer.ok) throw await refused(answer)
    },
    async usage() {
      const answer = await go(base, { method: 'GET', headers: { authorization } })
      if (!answer.ok) throw await refused(answer)
      const body = (await answer.json().catch(() => ({}))) as { used?: unknown; quota?: unknown }
      const count = (value: unknown): number => (typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0)
      return { used: count(body.used), quota: count(body.quota) }
    },
    async copies() {
      const answer = await go(base, { method: 'GET', headers: { authorization } })
      if (!answer.ok) throw await refused(answer)
      const body = (await answer.json().catch(() => ({}))) as { computers?: unknown }
      const out: { room: string; bytes: number; at: number }[] = []
      for (const one of Array.isArray(body.computers) ? body.computers as unknown[] : []) {
        if (typeof one !== 'object' || one === null) continue
        const said = one as { room?: unknown; bytes?: unknown; files?: unknown }
        if (typeof said.room !== 'string' || !/^[A-Za-z0-9_-]{22}$/.test(said.room)) continue
        const times = (Array.isArray(said.files) ? said.files as { at?: unknown }[] : []).map((file) => (typeof file?.at === 'number' ? file.at : 0))
        out.push({ room: said.room, bytes: typeof said.bytes === 'number' && said.bytes >= 0 ? said.bytes : 0, at: Math.max(0, ...times) })
      }
      return out
    },
    async removeAll() {
      const answer = await go(base, { method: 'DELETE', headers: { authorization } })
      if (!answer.ok) throw await refused(answer)
    }
  }
}

/**
 * WHEN A SYNC RUNS: SYNC_QUIET_MS after the last change, never while a run is going (it waits another quiet spell),
 * one at a time, and once more if something changed while one was running.
 */
export function createSyncSchedule(options: {
  readonly push: () => Promise<unknown>
  readonly busy: () => boolean
  readonly quietMs?: number
  readonly setTimer?: (run: () => void, ms: number) => unknown
  readonly clearTimer?: (timer: unknown) => void
}): { touched(): void; stop(): void } {
  const quiet = options.quietMs ?? SYNC_QUIET_MS
  const set = options.setTimer ?? ((run, ms) => setTimeout(run, ms))
  const clear = options.clearTimer ?? ((timer) => clearTimeout(timer as ReturnType<typeof setTimeout>))
  let timer: unknown
  let running = false
  let again = false
  let stopped = false
  const arm = (): void => {
    if (stopped) return
    if (timer !== undefined) clear(timer)
    timer = set(fire, quiet)
  }
  const fire = (): void => {
    timer = undefined
    if (options.busy()) { arm(); return }
    running = true
    again = false
    void options.push().catch(() => undefined).finally(() => {
      running = false
      if (again) arm()
    })
  }
  return {
    touched() {
      if (running) { again = true; return }
      arm()
    },
    stop() {
      stopped = true
      if (timer !== undefined) clear(timer)
      timer = undefined
    }
  }
}
