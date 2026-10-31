import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { YourAccountStatus } from '../shared/phone.js'
import type { SecretBox } from './own-models.js'
import { ASK_ID, grantIdOf, MANIFEST_ID, matchOf, newAskKeys, newProfileKey, openGrant, PROFILE_KEY_BYTES, profileKeyFromCode, recoveryCode, sealGrant, syncKeys, type AskKeys } from './profile-sync-crypto.js'
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
export const COPIES_FRESH_MS = 10_000
/** A new computer asking to be approved looks for the grant this often, for this long (accounts slice 4b). */
export const ASK_EVERY_MS = 5_000
export const ASK_FOR_MS = 10 * 60_000
const ASK_NAME = 60
const ASK_KEY = /^[A-Za-z0-9_-]{43}$/

/** One computer's place at the relay, as listed: its room, its bytes, when it last changed, and its files' ids. */
interface Listed { readonly room: string; readonly bytes: number; readonly at: number; readonly ids: readonly string[] }
/** A new computer of the account asking for the profile key, as this computer read its ask. */
interface Ask { readonly room: string; readonly name: string; readonly match: string; readonly key: string }

/** An ask as the relay hands it back: the asking computer's name and its key's public half, or undefined. */
function askOf(bytes: Uint8Array | undefined): { readonly name: string; readonly key: string } | undefined {
  if (bytes === undefined || bytes.length > 2048) return undefined
  try {
    const said = JSON.parse(new TextDecoder().decode(bytes)) as { v?: unknown; name?: unknown; key?: unknown }
    if (said.v !== 1 || typeof said.key !== 'string' || !ASK_KEY.test(said.key) || typeof said.name !== 'string') return undefined
    const name = said.name.replace(/[^\p{L}\p{N} '.,()_-]/gu, '').replace(/\s+/g, ' ').trim().slice(0, ASK_NAME)
    return { name: name === '' ? 'A computer' : name, key: said.key }
  } catch {
    return undefined
  }
}

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
  readonly copiesFreshMs?: number
  readonly askEveryMs?: number
  readonly askForMs?: number
}) {
  const file = join(options.profile, ACCOUNT_SYNC_FILE)
  const remoteOf = options.remote ?? ((link: AccountLink) => httpSyncRemote(link))
  const now = options.now ?? (() => new Date())
  let message: { text: string; good: boolean } | undefined
  let storage: { used: number; quota: number } | undefined
  let syncing = false
  /** The account's other computers as the relay last listed them, and the asks this computer could approve. */
  let elsewhere: { account: number; room: string; copies: readonly Listed[]; asks: readonly Ask[]; lookedAt: number } | undefined
  let looking = false
  /** Asks the person said are not theirs, and rooms this computer granted, whose grant goes once their ask does. */
  const ignored = new Set<string>()
  const grantedTo = new Set<string>()
  /** This computer asking to be approved: its fresh key, the code both screens show, and the look for the grant. */
  let asking: { keys: AskKeys; match: string; since: number; room: string; account: number; timer: unknown } | undefined
  let checking = false
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
    const listed = link !== undefined && elsewhere?.account === link.user && elsewhere.room === link.room ? elsewhere : undefined
    // A place with the copy's list is a copy; one holding only an ask is a computer waiting to be approved.
    const others = (listed?.copies ?? []).filter((copy) => copy.ids.includes(MANIFEST_ID))
    const asks = ours ? (listed?.asks ?? []).filter((ask) => !ignored.has(ask.room)) : []
    return {
      ...(link === undefined ? {} : { signedIn: { login: link.login } }),
      sync: ours && saved.on ? 'on' : 'off',
      hasKey: ours,
      syncing,
      ...(last === undefined ? {} : { lastSynced: last }),
      ...(ours && storage !== undefined ? { storage } : {}),
      ...(others.length === 0 ? {} : { elsewhere: { computers: others.length, at: new Date(Math.max(...others.map((copy) => copy.at))).toISOString() } }),
      ...(asks.length === 0 ? {} : { asks: asks.map((ask) => ({ room: ask.room, name: ask.name, match: ask.match })) }),
      ...(asking !== undefined && link !== undefined && asking.room === link.room ? { asking: { match: asking.match, since: new Date(asking.since).toISOString() } } : {}),
      ...(message === undefined ? {} : { message: message.text, good: message.good })
    }
  }
  const tell = (): void => { if (options.changed !== undefined) void status().then(options.changed, () => undefined) }
  /**
   * Asks the relay, at most every few seconds, what the account's other computers keep: their copies, and their asks to
   * be approved, which this computer reads only when it holds the key to give. Says so when anything changes.
   */
  function lookElsewhere(link: AccountLink): void {
    const fresh = elsewhere?.account === link.user && elsewhere.room === link.room && now().getTime() - elsewhere.lookedAt < (options.copiesFreshMs ?? COPIES_FRESH_MS)
    if (fresh || looking) return
    looking = true
    void (async () => {
      const remote = remoteOf(link)
      const listed = (await remote.copies()).filter((copy) => copy.bytes > 0)
      const others = listed.filter((copy) => copy.room !== link.room)
      const mine = listed.find((copy) => copy.room === link.room)
      const saved = await read()
      const asks: Ask[] = []
      if (saved !== undefined && saved.account === link.user) {
        for (const copy of others) {
          if (!copy.ids.includes(ASK_ID) || ignored.has(copy.room) || mine?.ids.includes(grantIdOf(copy.room)) === true) continue
          const ask = askOf(await remote.get(copy.room, ASK_ID).catch(() => undefined))
          if (ask !== undefined) asks.push({ room: copy.room, name: ask.name, key: ask.key, match: await matchOf(ask.key) })
        }
      }
      // A grant whose ask is gone was taken, or given up on: it is not left lying at the relay.
      for (const room of [...grantedTo]) {
        if (others.some((copy) => copy.room === room && copy.ids.includes(ASK_ID))) continue
        await remote.remove(grantIdOf(room)).then(() => { grantedTo.delete(room) }, () => undefined)
      }
      const changed = JSON.stringify([others, asks]) !== JSON.stringify([elsewhere?.copies ?? [], elsewhere?.asks ?? []])
      elsewhere = { account: link.user, room: link.room, copies: others, asks, lookedAt: now().getTime() }
      if (changed) tell()
    })().catch(() => undefined).finally(() => { looking = false })
  }

  /** The newest copy another computer keeps, opened with this key, written as a backup folder and restored. */
  const bringWithKey = async (link: AccountLink, key: Uint8Array): Promise<{ readonly ok: true; readonly computer: string } | { readonly ok: false; readonly reason: string }> => {
    let copies: readonly Listed[]
    try {
      copies = (await remoteOf(link).copies()).filter((copy) => copy.room !== link.room && copy.ids.includes(MANIFEST_ID)).sort((a, b) => b.at - a.at)
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
  }

  const clearAsk = (): void => {
    if (asking !== undefined) (options.clearInterval ?? ((timer) => clearInterval(timer as ReturnType<typeof setInterval>)))(asking.timer)
    asking = undefined
  }

  /**
   * THE NEW COMPUTER'S LOOK FOR ITS GRANT, every few seconds while it asks. A grant that opens with this ask's key is
   * the profile key: the ask is taken down and the copy brought here as with the recovery code. One that does not open
   * (sealed to another key, or swapped on the way) is passed over. After ten minutes the ask is given up.
   */
  const checkAsk = async (): Promise<void> => {
    if (asking === undefined || checking) return
    checking = true
    try {
      const current = asking
      const link = await options.link().catch(() => undefined)
      if (link === undefined || link.room !== current.room || link.user !== current.account) { clearAsk(); tell(); return }
      const remote = remoteOf(link)
      if (now().getTime() - current.since > (options.askForMs ?? ASK_FOR_MS)) {
        clearAsk()
        await remote.remove(ASK_ID).catch(() => undefined)
        message = { text: 'No computer approved this one in ten minutes. Ask again, or use your recovery code.', good: false }
        tell()
        return
      }
      for (const copy of await remote.copies()) {
        if (copy.room === link.room || !copy.ids.includes(grantIdOf(link.room))) continue
        const grant = await remote.get(copy.room, grantIdOf(link.room)).catch(() => undefined)
        const opened = grant === undefined ? undefined : await openGrant(grant, current.keys, link.room)
        if (opened === undefined) continue
        clearAsk()
        await remote.remove(ASK_ID).catch(() => undefined)
        message = { text: 'Approved. Bringing your Locust here…', good: true }
        tell()
        const brought = await serial(() => bringWithKey(link, opened.profileKey))
        message = brought.ok ? { text: `Your Locust from ${brought.computer} is ready. Restarting to bring it in…`, good: true } : { text: brought.reason, good: false }
        tell()
        return
      }
    } catch {
      // The relay did not answer this time: the next look tries again.
    } finally {
      checking = false
    }
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
      return bringWithKey(link, key)
    }),
    /**
     * ASK THE COMPUTER THAT HAS YOUR LOCUST (accounts slice 4b). A fresh key is made here and its ask left in this
     * computer's own place at the relay; the screen shows the match code, and a look for the grant runs until one
     * comes or ten minutes pass.
     */
    ask: (): Promise<YourAccountStatus> => serial(async () => {
      const link = await options.link().catch(() => undefined)
      if (link === undefined) { message = { text: 'Sign in with GitHub first, under Use from anywhere.', good: false }; return status() }
      if (!options.secrets.available()) { message = { text: 'This computer cannot lock a key, so your copy cannot be brought here safely.', good: false }; return status() }
      const remote = remoteOf(link)
      try {
        const copies = (await remote.copies()).filter((copy) => copy.room !== link.room && copy.ids.includes(MANIFEST_ID))
        if (copies.length === 0) { message = { text: `No other computer keeps a copy for ${link.login} yet. Turn on Keep a copy there first.`, good: false }; return status() }
        clearAsk()
        const keys = await newAskKeys()
        await remote.put(ASK_ID, new TextEncoder().encode(JSON.stringify({ v: 1, name: options.computer().slice(0, ASK_NAME), key: keys.publicKey, at: now().getTime() })))
        asking = { keys, match: await matchOf(keys.publicKey), since: now().getTime(), room: link.room, account: link.user, timer: (options.setInterval ?? setInterval)(() => { void checkAsk() }, options.askEveryMs ?? ASK_EVERY_MS) }
        message = undefined
      } catch (error) {
        message = { text: refusal(error), good: false }
      }
      return status()
    }),
    /** The look for the grant, once: the timer runs it, and a test can. */
    checkAsk,
    stopAsking: (): Promise<YourAccountStatus> => serial(async () => {
      const link = await options.link().catch(() => undefined)
      clearAsk()
      if (link !== undefined) await remoteOf(link).remove(ASK_ID).catch(() => undefined)
      message = undefined
      return status()
    }),
    /**
     * Approve a computer's ask, from a computer that holds the key. The ask is read again, and its match code must be the
     * one the person was shown: if the relay changed the key since, nothing is sent.
     */
    approve: (room: string, match: string): Promise<YourAccountStatus> => serial(async () => {
      const link = await options.link().catch(() => undefined)
      const saved = await read()
      const key = link !== undefined && saved !== undefined && saved.account === link.user ? keyOf(saved) : undefined
      if (link === undefined || key === undefined) {
        message = { text: 'This computer does not hold your Locust’s key. Approve from the computer that keeps your copy.', good: false }
        return status()
      }
      const shown = elsewhere?.asks.find((ask) => ask.room === room)
      const remote = remoteOf(link)
      try {
        const ask = askOf(await remote.get(room, ASK_ID))
        if (ask === undefined) { message = { text: 'That computer is not asking any more.', good: false }; return status() }
        if (shown === undefined || shown.match !== match || (await matchOf(ask.key)) !== match) {
          message = { text: 'The code changed. Ask again on the new computer, and approve only if both screens show the same code.', good: false }
          return status()
        }
        await remote.put(grantIdOf(room), await sealGrant(key, ask.key, link.room, room))
        grantedTo.add(room)
        if (elsewhere !== undefined) elsewhere = { ...elsewhere, asks: elsewhere.asks.filter((one) => one.room !== room) }
        message = { text: `Approved. ${ask.name} brings your Locust in and restarts.`, good: true }
      } catch (error) {
        message = { text: refusal(error), good: false }
      }
      return status()
    }),
    /** Not this person's computer: its ask is not shown again here. */
    ignore: (room: string): Promise<YourAccountStatus> => serial(async () => {
      ignored.add(room)
      message = { text: 'Not approved. That computer gets nothing from this one.', good: true }
      return status()
    }),
    stop(): void {
      schedule.stop()
      watch(false)
      clearAsk()
    }
  }
}

export type YourAccount = ReturnType<typeof createYourAccount>
export type { YourAccountStatus }
