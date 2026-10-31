import { randomBytes } from 'node:crypto'
import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { syncCore, type SyncObject } from '../../../relay/src/sync.js'
import type { YourAccountStatus } from '../shared/phone.js'
import { LEFT_OUT } from './profile-backup.js'
import { profileKeyFromCode, recoveryCode } from './profile-sync-crypto.js'
import { PROFILE_SYNC_FILE, SyncRefused, type SyncAccountRemote } from './profile-sync.js'
import { ACCOUNT_SYNC_FILE, createYourAccount, type AccountLink } from './your-account.js'

/**
 * SETTINGS › YOUR ACCOUNT (accounts slice 3). Signed in with GitHub through Use from anywhere, a person turns sync on:
 * the key is made here and locked by the operating system, the recovery code comes back once, and the copy goes up.
 * Off keeps the key; on again shows no new code. Delete my copy empties the relay, turns sync off and forgets what was
 * sent, so the next turn-on sends everything again. The relay is the relay's own rules over an in-memory bucket.
 */
const folders: string[] = []
const fresh = async (): Promise<string> => { const folder = await mkdtemp(join(tmpdir(), 'locust-account-')); folders.push(folder); return folder }
afterEach(async () => { for (const folder of folders.splice(0)) await rm(folder, { recursive: true, force: true }) })

function aRelay(user: number, full = false) {
  const held = new Map<string, { bytes: Uint8Array; uploaded: number }>()
  const ledger = new Map<string, unknown>()
  const core = syncCore({
    objects: {
      head: async (key) => { const one = held.get(key); return one === undefined ? undefined : { key, size: one.bytes.length, uploaded: one.uploaded } },
      get: async (key) => { const one = held.get(key); return one === undefined ? undefined : { body: one.bytes.slice().buffer, size: one.bytes.length } },
      put: async (key, body) => { held.set(key, { bytes: new Uint8Array(body as ArrayBuffer), uploaded: Date.now() }) },
      list: async (prefix) => [...held].filter(([key]) => key.startsWith(prefix)).map(([key, one]): SyncObject => ({ key, size: one.bytes.length, uploaded: one.uploaded })),
      delete: async (keys) => { for (const key of keys) held.delete(key) }
    },
    ledger: { get: async <T>(key: string) => ledger.get(key) as T | undefined, put: async (key, value) => { ledger.set(key, value) } },
    now: () => Date.now()
  })
  let removedAll = 0
  const remote = (link: AccountLink): SyncAccountRemote => ({
    async put(id, box) {
      if (full) throw new SyncRefused(507, 'Your copy on Locust’s relay is full (1 GB).')
      const answer = await core.put({ user, room: link.room, id, size: box.length, body: box.slice().buffer })
      if (answer.status !== 204) throw new SyncRefused(answer.status, String(answer.body?.error))
    },
    async get(from, id) {
      const answer = await core.get({ user, from, id })
      return answer.status === 200 ? new Uint8Array(answer.bytes as ArrayBuffer) : undefined
    },
    async remove(id) { await core.remove({ user, room: link.room, id }) },
    async usage() {
      const body = (await core.list({ user })).body as { used: number; quota: number }
      return { used: body.used, quota: body.quota }
    },
    async removeAll() { removedAll += 1; await core.removeAll({ user }) }
  })
  return { held, remote, removedAll: () => removedAll }
}

async function aProfile(): Promise<string> {
  const root = await fresh()
  await writeFile(join(root, 'teammates.json'), `${JSON.stringify({ schemaVersion: 15, teammates: [{ name: 'Wren' }] })}\n`)
  await writeFile(join(root, 'memories.json'), `${JSON.stringify({ schemaVersion: 2, memories: [{ text: 'Colin ships on Mondays' }] })}\n`)
  return root
}

/** The operating system's lock, as a test stands it in: reversible, and never the clear text. */
const lock = (available = true) => ({
  available: () => available,
  encrypt: (text: string) => Buffer.from(`locked:${Buffer.from(text).toString('hex')}`),
  decrypt: (cipher: Buffer) => Buffer.from(cipher.toString().replace(/^locked:/, ''), 'hex').toString()
})

const linkAs = (user: number, login = 'colin'): AccountLink => ({ relay: 'https://relay.test', room: randomBytes(16).toString('base64url'), token: 't'.repeat(43), user, login })

function anAccount(profile: string, options: { link?: AccountLink; relay?: ReturnType<typeof aRelay>; available?: boolean } = {}) {
  let link = options.link
  const relay = options.relay ?? aRelay(link?.user ?? 7)
  const told: YourAccountStatus[] = []
  const account = createYourAccount({
    profile,
    link: async () => link,
    secrets: lock(options.available),
    computer: () => "Colin's Mac",
    appVersion: '0.845.0',
    busy: () => false,
    remote: relay.remote,
    quietMs: 0,
    setInterval: () => 'look',
    clearInterval: () => undefined,
    changed: (status) => { told.push(status) }
  })
  return { account, relay, told, signIn: (next: AccountLink | undefined) => { link = next } }
}

const until = async (check: () => Promise<boolean>): Promise<void> => {
  for (let n = 0; n < 200; n += 1) {
    if (await check()) return
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
  throw new Error('timed out')
}

describe('Your account', () => {
  it('without a GitHub sign-in it says where to sign in and keeps nothing', async () => {
    const profile = await aProfile()
    const { account } = anAccount(profile)
    expect(await account.status()).toEqual({ sync: 'off', hasKey: false, syncing: false })
    const answer = await account.turnOn()
    expect(answer.code).toBeUndefined()
    expect(answer.status.message).toBe('Sign in with GitHub first, under Use from anywhere.')
    await expect(stat(join(profile, ACCOUNT_SYNC_FILE))).rejects.toThrow()
  })

  it('turning sync on shows the recovery code once, locks the key, and sends the copy', async () => {
    const profile = await aProfile()
    const { account, relay } = anAccount(profile, { link: linkAs(7) })
    const answer = await account.turnOn()
    expect(answer.code).toMatch(/^([0-9A-Z]{5}-){10}[0-9A-Z]{5}$/)
    expect(answer.status).toMatchObject({ signedIn: { login: 'colin' }, sync: 'on', hasKey: true })
    const key = await profileKeyFromCode(answer.code!)
    const saved = await readFile(join(profile, ACCOUNT_SYNC_FILE), 'utf8')
    expect(saved).not.toContain(Buffer.from(key!).toString('base64url'))
    expect(saved).not.toContain(answer.code!)
    expect(LEFT_OUT[ACCOUNT_SYNC_FILE]).toBeDefined()
    await until(async () => (await account.status()).lastSynced !== undefined)
    const status = await account.status()
    expect(status.lastSynced?.files).toBe(2)
    expect(status.storage?.used).toBeGreaterThan(0)
    expect(status.storage?.quota).toBeGreaterThan(status.storage!.used)
    expect(relay.held.size).toBeGreaterThan(0)
    for (const [, one] of relay.held) expect(Buffer.from(one.bytes).toString('utf8')).not.toContain('Colin ships on Mondays')
    account.stop()
  })

  it('off keeps the key, and on again shows no new code and seals with the same key', async () => {
    const profile = await aProfile()
    const { account } = anAccount(profile, { link: linkAs(7) })
    const first = await account.turnOn()
    const before = await readFile(join(profile, ACCOUNT_SYNC_FILE), 'utf8')
    expect((await account.turnOff()).sync).toBe('off')
    expect(await account.status()).toMatchObject({ sync: 'off', hasKey: true })
    const again = await account.turnOn()
    expect(again.code).toBeUndefined()
    expect(again.status.sync).toBe('on')
    expect(JSON.parse(await readFile(join(profile, ACCOUNT_SYNC_FILE), 'utf8')).key).toBe(JSON.parse(before).key)
    expect(await recoveryCode((await profileKeyFromCode(first.code!))!)).toBe(first.code)
    account.stop()
  })

  it('a computer that cannot lock a key keeps sync off and says why', async () => {
    const profile = await aProfile()
    const { account } = anAccount(profile, { link: linkAs(7), available: false })
    const answer = await account.turnOn()
    expect(answer.code).toBeUndefined()
    expect(answer.status).toMatchObject({ sync: 'off', hasKey: false, message: 'This computer cannot lock a key, so sync stays off. Your Locust is safe here.' })
  })

  it('a key made for another GitHub account is not used: a new sign-in starts over with its own code', async () => {
    const profile = await aProfile()
    const relay = aRelay(7)
    const { account, signIn } = anAccount(profile, { link: linkAs(7), relay })
    await account.turnOn()
    signIn(linkAs(8, 'someone'))
    expect(await account.status()).toMatchObject({ signedIn: { login: 'someone' }, sync: 'off', hasKey: false })
    expect((await account.turnOn()).code).toBeDefined()
    account.stop()
  })

  it('Sync now says when the relay is full, and nothing on the computer changes', async () => {
    const profile = await aProfile()
    const { account } = anAccount(profile, { link: linkAs(7), relay: aRelay(7, true) })
    await account.turnOn()
    const status = await account.syncNow()
    expect(status.message).toContain('full (1 GB)')
    expect(status.good).toBe(false)
    expect(await readFile(join(profile, 'teammates.json'), 'utf8')).toContain('Wren')
    account.stop()
  })

  it('Delete my copy empties the relay, turns sync off and forgets what was sent', async () => {
    const profile = await aProfile()
    const { account, relay } = anAccount(profile, { link: linkAs(7) })
    await account.turnOn()
    await until(async () => (await account.status()).lastSynced !== undefined)
    const status = await account.deleteCopy()
    expect(relay.removedAll()).toBe(1)
    expect(relay.held.size).toBe(0)
    expect(status).toMatchObject({ sync: 'off', hasKey: true, good: true, message: 'Your copy on Locust’s relay is deleted. Everything on this computer stays.' })
    expect(status.lastSynced).toBeUndefined()
    await expect(stat(join(profile, PROFILE_SYNC_FILE))).rejects.toThrow()
    expect(await readFile(join(profile, 'teammates.json'), 'utf8')).toContain('Wren')
    // On again, everything goes up again: the forgotten list does not skip a piece the relay no longer has.
    await account.turnOn()
    await until(async () => (await account.status()).lastSynced !== undefined)
    expect(relay.held.size).toBeGreaterThan(0)
    account.stop()
  })
})
