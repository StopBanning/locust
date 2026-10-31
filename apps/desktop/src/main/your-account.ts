import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { YourAccountStatus } from '../shared/phone.js'
import type { SecretBox } from './own-models.js'
import { newProfileKey, PROFILE_KEY_BYTES, profileKeyFromCode, recoveryCode, syncKeys } from './profile-sync-crypto.js'
import { createSyncSchedule, forgetSync, httpSyncRemote, lastSync, pullProfile, pushProfile, SyncRefused, type SyncAccountRemote } from './profile-sync.js'

/**
 * SETTINGS › YOUR ACCOUNT (accounts slice 3, docs/PLAN-2026-10-30-ACCOUNTS.md).
 *
 * The account is the GitHub sign-in "Use from anywhere" already ties this computer's room to (0.838): one sign-in
 * does both. Turning sync on makes the profile key on this computer, locks it with the operating system (safeStorage,
 * as own-model keys are), and shows the recovery code ONCE: without it, losing every computer loses the copy, because
 * the relay cannot open it. After that the copy is kept current a quiet minute after a change, never during a run
 * (profile-sync.ts). Off stops syncing and keeps the key, so turning it on again needs no new code and the old copy
 * still opens. "Delete my copy" deletes every computer's copy of the account at the relay, then turns sync off, so
 * nothing is sent straight back.
 */

export const ACCOUNT_SYNC_FILE = 'account-sync.json'
/** Changes are noticed by a look every few minutes as well as when a run ends. */
export const ACCOUNT_LOOK_MS = 10 * 60_000
/** Where a copy brought from another computer is opened before the restore applies it (left out of backups). */
export const ACCOUNT_COPIES_FOLDER = 'account-copies'
/** How long the list of the account's other copies is trusted before the relay is asked again. */
export const COPIES_FRESH_MS = 60_000

/** The room and the GitHub account this computer is tied to, from "Use from anywhere". */
export interface AccountLink {
  readonly relay: string
  readonly room: string
  readonly token: string
  readonly user: number
  readonly login: string
}

interface Saved {
  readonly v: 1
  readonly on: boolean
  /** The GitHub account the key was made for. */
  readonly account: number
  /** The profile key, base64url, locked by the operating system, base64. */
  readonly key: string
}

const savedOf = (value: unknown): Saved | undefined => {
  if (typeof value !== 'object' || value === null) return undefined
  const said = value as Record<string, unknown>
  return said.v === 1 && typeof said.on === 'boolean' && typeof said.account === 'number' && Number.isSafeInteger(said.account) && said.account > 0 && typeof said.key === 'string' && said.key.length > 0
    ? { v: 1, on: said.on, account: said.account, key: said.key }
    : undefined
}

export function createYourAccount(options: {
  readonly profile: string
  readonly link: () => Promise<AccountLink | undefined>
  readonly secrets: SecretBox
  readonly computer: () => string
  readonly appVersion: string
  /** A run is going: a sync waits for a quiet spell. */
  readonly busy: () => boolean
  readonly remote?: (link: AccountLink) => SyncAccountRemote
  readonly now?: () => Date
  readonly quietMs?: number
  readonly lookMs?: number
  readonly setInterval?: (run: () => void, ms: number) => unknown
  readonly clearInterval?: (timer: unknown) => void
  readonly changed?: (status: YourAccountStatus) => void
  /** Asks for the restore of a backup-shaped folder at the next start, and restarts: the restore Settings already has. */
  readonly restore?: (folder: string) => Promise<{ readonly ok: true } | { readonly ok: false; readonly reason: string }>
}) {
  const file = join(options.profile, ACCOUNT_SYNC_FILE)
  const remoteOf = options.remote ?? ((link: AccountLink) => httpSyncRemote(link))
  const now = options.now ?? (() => new Date())
  let message: { text: string; good: boolean } | undefined
  let storage: { used: number; quota: number } | undefined
  let syncing = false
  /** The account's copies kept by other computers, as the relay last listed them. */
  let elsewhere: { account: number; room: string; copies: readonly { room: string; bytes: number; at: number }[]; lookedAt: number } | undefined
  let looking = false
  let look: unknown
  let queue: Promise<unknown> = Promise.resolve()
  const serial = <T>(work: () => Promise<T>): Promise<T> => {
    const next = queue.then(work, work)
    queue = next.catch(() => undefined)
    return next
  }

  const read = async (): Promise<Saved | undefined> => savedOf(JSON.parse(await readFile(file, 'utf8').catch(() => 'null')) as unknown)
  const write = (saved: Saved): Promise<void> => writeFile(file, `${JSON.stringify(saved)}\n`, 'utf8')
  const keyOf = (saved: Saved): Uint8Array | undefined => {
    try {
      const key = Buffer.from(options.secrets.decrypt(Buffer.from(saved.key, 'base64')), 'base64url')
      return key.length === PROFILE_KEY_BYTES ? new Uint8Array(key) : undefined
    } catch {
      return undefined
    }
  }
  const refusal = (error: unknown): string => (error instanceof SyncRefused ? error.message : 'Locust’s relay could not be reached. Your Locust is safe here; try again in a minute.')

  const status = async (): Promise<YourAccountStatus> => {
    const link = await options.link().catch(() => undefined)
    const saved = await read()
    const ours = saved !== undefined && link !== undefined && saved.account === link.user
    const last = ours ? await lastSync(options.profile, link.user, link.room) : undefined
    if (link !== undefined) lookElsewhere(link)
    const others = link !== undefined && elsewhere?.account === link.user && elsewhere.room === link.room ? elsewhere.copies : []
    return {
      ...(link === undefined ? {} : { signedIn: { login: link.login } }),
      sync: ours && saved.on ? 'on' : 'off',
      hasKey: ours,
      syncing,
      ...(last === undefined ? {} : { lastSynced: last }),
      ...(ours && storage !== undefined ? { storage } : {}),
      ...(others.length === 0 ? {} : { elsewhere: { computers: others.length, at: new Date(Math.max(...others.map((copy) => copy.at))).toISOString() } }),
      ...(message === undefined ? {} : { message: message.text, good: message.good })
    }
  }
  const tell = (): void => { if (options.changed !== undefined) void status().then(options.changed, () => undefined) }
  /** Asks the relay, at most once a minute, which other computers of the account keep a copy; says so when it changes. */
  function lookElsewhere(link: AccountLink): void {
    const fresh = elsewhere?.account === link.user && elsewhere.room === link.room && now().getTime() - elsewhere.lookedAt < COPIES_FRESH_MS
    if (fresh || looking) return
    looking = true
    void remoteOf(link).copies().then((copies) => {
      const others = copies.filter((copy) => copy.room !== link.room && copy.bytes > 0)
      const changed = JSON.stringify(others) !== JSON.stringify(elsewhere?.copies ?? [])
      elsewhere = { account: link.user, room: link.room, copies: others, lookedAt: now().getTime() }
      if (changed) tell()
    }, () => undefined).finally(() => { looking = false })
  }

  /** One sync now: the copy, then how much of the account's storage it takes. */
  const push = (): Promise<boolean> => serial(async () => {
    const link = await options.link().catch(() => undefined)
    const saved = await read()
    if (link === undefined || saved === undefined || !saved.on) return false
    if (saved.account !== link.user) {
      message = { text: `Sync was turned on as another GitHub account. Turn it off and on again as ${link.login}.`, good: false }
      tell()
      return false
    }
    const key = keyOf(saved)
    if (key === undefined) {
      message = { text: 'This computer could not unlock the key your copy is sealed with. Turn sync off and on again.', good: false }
      tell()
      return false
    }
    syncing = true
    tell()
    try {
      const remote = remoteOf(link)
      const pushed = await pushProfile(options.profile, { keys: await syncKeys(key, link.user), remote, room: link.room, computer: options.computer(), appVersion: options.appVersion, now })
      message = pushed.ok ? undefined : { text: pushed.reason, good: false }
      if (pushed.ok) storage = await remote.usage().catch(() => storage)
      return pushed.ok
    } finally {
      syncing = false
      tell()
    }
  })
  const schedule = createSyncSchedule({ push, busy: options.busy, ...(options.quietMs === undefined ? {} : { quietMs: options.quietMs }) })
  const watch = (on: boolean): void => {
    if (on && look === undefined) look = (options.setInterval ?? setInterval)(() => schedule.touched(), options.lookMs ?? ACCOUNT_LOOK_MS)
    if (!on && look !== undefined) { (options.clearInterval ?? ((timer) => clearInterval(timer as ReturnType<typeof setInterval>)))(look); look = undefined }
  }

  return {
    status,
    /** At start: a computer with sync on syncs after a quiet spell, and keeps looking. */
    async start(): Promise<void> {
      if ((await read())?.on === true) { watch(true); schedule.touched() }
    },
    /** Something changed, or a run ended. */
    touched(): void { schedule.touched() },
    /**
     * Sync on. The first time for this account it makes and locks the key and gives back the recovery code, which is
     * never shown again; after that it turns on with the key it already has.
     */
    turnOn: (): Promise<{ readonly status: YourAccountStatus; readonly code?: string }> => serial(async () => {
      const link = await options.link().catch(() => undefined)
      if (link === undefined) {
        message = { text: 'Sign in with GitHub first, under Use from anywhere.', good: false }
        return { status: await status() }
      }
      const saved = await read()
      if (saved !== undefined && saved.account === link.user && keyOf(saved) !== undefined) {
        await write({ ...saved, on: true })
        message = undefined
        watch(true)
        schedule.touched()
        return { status: await status() }
      }
      if (!options.secrets.available()) {
        message = { text: 'This computer cannot lock a key, so sync stays off. Your Locust is safe here.', good: false }
        return { status: await status() }
      }
      const key = newProfileKey()
      await write({ v: 1, on: true, account: link.user, key: options.secrets.encrypt(Buffer.from(key).toString('base64url')).toString('base64') })
      storage = undefined
      message = undefined
      watch(true)
      schedule.touched()
      return { status: await status(), code: await recoveryCode(key) }
    }),
    turnOff: (): Promise<YourAccountStatus> => serial(async () => {
      const saved = await read()
      if (saved !== undefined) await write({ ...saved, on: false })
      watch(false)
      message = undefined
      return status()
    }),
    /** Sync now, from the button: says how it went. */
    syncNow: async (): Promise<YourAccountStatus> => {
      const pushed = await push().catch((error: unknown) => { message = { text: refusal(error), good: false }; return false })
      if (pushed) message = { text: 'Synced.', good: true }
      return status()
    },
    /** Every copy of the account at the relay deleted, sync off, and what was sent forgotten. Nothing here changes. */
    deleteCopy: (): Promise<YourAccountStatus> => serial(async () => {
      const link = await options.link().catch(() => undefined)
      if (link === undefined) {
        message = { text: 'Sign in with GitHub first, under Use from anywhere.', good: false }
        return status()
      }
      try {
        await remoteOf(link).removeAll()
      } catch (error) {
        message = { text: refusal(error), good: false }
        return status()
      }
      const saved = await read()
      if (saved !== undefined) await write({ ...saved, on: false })
      await forgetSync(options.profile)
      watch(false)
      storage = undefined
      message = { text: 'Your copy on Locust’s relay is deleted. Everything on this computer stays.', good: true }
      return status()
    }),
    /**
     * YOUR LOCUST ON THIS COMPUTER, WITH THE RECOVERY CODE (accounts slice 4). The newest copy another computer keeps is
     * opened with the key the code holds, written as a backup folder, and restored through the restore Settings already
     * has: what it replaces is moved aside, and Locust restarts once. The key is kept here, locked, so turning sync on
     * afterwards shows no new code. A code that does not open the copy changes nothing.
     */
    bringHere: (code: string): Promise<{ readonly ok: true; readonly computer: string } | { readonly ok: false; readonly reason: string }> => serial(async () => {
      const link = await options.link().catch(() => undefined)
      if (link === undefined) return { ok: false, reason: 'Sign in with GitHub first, under Use from anywhere.' }
      const key = await profileKeyFromCode(code)
      if (key === undefined) return { ok: false, reason: 'That code does not read right. It is 55 letters and digits in groups of five; check each group.' }
      if (!options.secrets.available()) return { ok: false, reason: 'This computer cannot lock a key, so your copy cannot be brought here safely.' }
      let copies: readonly { room: string; bytes: number; at: number }[]
      try {
        copies = (await remoteOf(link).copies()).filter((copy) => copy.room !== link.room && copy.bytes > 0).sort((a, b) => b.at - a.at)
      } catch (error) {
        return { ok: false, reason: refusal(error) }
      }
      if (copies.length === 0) return { ok: false, reason: `No other computer keeps a copy for ${link.login} yet. Turn on Keep a copy there first.` }
      const keys = await syncKeys(key, link.user)
      const folder = join(options.profile, ACCOUNT_COPIES_FOLDER, now().toISOString().replace(/[:.]/g, '-'))
      const pulled = await pullProfile({ keys, remote: remoteOf(link), from: copies[0]!.room, folder })
      if (!pulled.ok) return pulled
      await write({ v: 1, on: false, account: link.user, key: options.secrets.encrypt(Buffer.from(key).toString('base64url')).toString('base64') })
      if (options.restore === undefined) return { ok: true, computer: pulled.computer }
      const asked = await options.restore(folder)
      return asked.ok ? { ok: true, computer: pulled.computer } : asked
    }),
    stop(): void {
      schedule.stop()
      watch(false)
    }
  }
}

export type YourAccount = ReturnType<typeof createYourAccount>
export type { YourAccountStatus }
