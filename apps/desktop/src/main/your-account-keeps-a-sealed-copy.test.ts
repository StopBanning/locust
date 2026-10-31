import { randomBytes } from 'node:crypto'
import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { syncCore, type SyncObject } from '../../../relay/src/sync.js'
import type { YourAccountStatus } from '../shared/phone.js'
import { applyPendingRestore, LEFT_OUT, readBackup, requestRestore } from './profile-backup.js'
import { ASK_ID, grantIdOf, newAskKeys, newProfileKey, profileKeyFromCode, recoveryCode } from './profile-sync-crypto.js'
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
// A sync a test set off may still be writing as it ends: the clearing tries again for a moment rather than racing it.
afterEach(async () => {
  for (const folder of folders.splice(0)) {
    for (let tries = 0; ; tries += 1) {
      try { await rm(folder, { recursive: true, force: true }); break } catch (error) { if (tries >= 20) throw error; await new Promise((resolve) => setTimeout(resolve, 50)) }
    }
  }
})

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
  /** Every id read back, in order: who read what is the relay's to see, and a test's to check. */
  const gets: string[] = []
  const remote = (link: AccountLink): SyncAccountRemote => ({
    async put(id, box) {
      if (full) throw new SyncRefused(507, 'Your copy on Locust’s relay is full (1 GB).')
      const answer = await core.put({ user, room: link.room, id, size: box.length, body: box.slice().buffer })
      if (answer.status !== 204) throw new SyncRefused(answer.status, String(answer.body?.error))
    },
    async get(from, id) {
      gets.push(id)
      const answer = await core.get({ user, from, id })
      return answer.status === 200 ? new Uint8Array(answer.bytes as ArrayBuffer) : undefined
    },
    async remove(id) { await core.remove({ user, room: link.room, id }) },
    async usage() {
      const body = (await core.list({ user })).body as { used: number; quota: number }
      return { used: body.used, quota: body.quota }
    },
    async copies() {
      const body = (await core.list({ user })).body as { computers: { room: string; bytes: number; files: { id: string; at: number }[] }[] }
      return body.computers.map((one) => ({ room: one.room, bytes: one.bytes, at: Math.max(0, ...one.files.map((file) => file.at)), ids: one.files.map((file) => file.id) }))
    },
    async removeAll() { removedAll += 1; await core.removeAll({ user }) }
  })
  return { held, remote, gets, removedAll: () => removedAll }
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

function anAccount(profile: string, options: { link?: AccountLink; relay?: ReturnType<typeof aRelay>; available?: boolean; computer?: string; restore?: (folder: string) => Promise<{ ok: true } | { ok: false; reason: string }>; askForMs?: number } = {}) {
  let link = options.link
  const relay = options.relay ?? aRelay(link?.user ?? 7)
  const told: YourAccountStatus[] = []
  const account = createYourAccount({
    profile,
    link: async () => link,
    secrets: lock(options.available),
    computer: () => options.computer ?? "Colin's Mac",
    appVersion: '0.845.0',
    busy: () => false,
    remote: relay.remote,
    quietMs: 0,
    setInterval: () => 'look',
    clearInterval: () => undefined,
    changed: (status) => { told.push(status) },
    ...(options.restore === undefined ? {} : { restore: options.restore }),
    ...(options.askForMs === undefined ? {} : { askForMs: options.askForMs }),
    copiesFreshMs: 0
  })
  return { account, relay, told, signIn: (next: AccountLink | undefined) => { link = next } }
}

const pause = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))
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

describe('your Locust on a new computer, with the recovery code (slice 4)', () => {
  /** The PC keeps a copy; the Mac signs in as the same GitHub account, with a room of its own. */
  async function twoComputers() {
    const relay = aRelay(7)
    const pc = anAccount(await aProfile(), { link: linkAs(7), relay, computer: "Colin's PC" })
    const first = await pc.account.turnOn()
    await until(async () => (await pc.account.status()).lastSynced !== undefined)
    pc.account.stop()
    const macProfile = await fresh()
    await writeFile(join(macProfile, 'teammates.json'), `${JSON.stringify({ schemaVersion: 15, teammates: [{ name: 'Atlas' }] })}\n`)
    const restores: string[] = []
    const mac = anAccount(macProfile, {
      link: linkAs(7), relay,
      restore: async (folder) => {
        restores.push(folder)
        return requestRestore(folder, macProfile, { appVersion: '0.845.0', liveRuns: 0, now: () => new Date() }).then((asked) => (asked.ok ? { ok: true as const } : asked))
      }
    })
    return { relay, code: first.code!, mac, macProfile, restores }
  }

  it('a new computer is told another computer keeps a copy', async () => {
    const { mac } = await twoComputers()
    await mac.account.status()
    await until(async () => (await mac.account.status()).elsewhere !== undefined)
    expect((await mac.account.status()).elsewhere?.computers).toBe(1)
  })

  it('the code opens the newest copy, restores it at the next start, and keeps the key for sync', async () => {
    const { code, mac, macProfile, restores } = await twoComputers()
    const brought = await mac.account.bringHere(code.toLowerCase().replace(/-/g, ' '))
    expect(brought).toEqual({ ok: true, computer: "Colin's PC" })
    expect(restores).toHaveLength(1)
    expect((await readBackup(restores[0]!, { appVersion: '0.845.0' })).ok).toBe(true)
    // The next start applies it: the PC's teammates replace the Mac's, and what they replaced is kept aside.
    const applied = await applyPendingRestore(macProfile, { appVersion: '0.845.0', now: () => new Date() })
    expect(applied?.ok).toBe(true)
    expect(await readFile(join(macProfile, 'teammates.json'), 'utf8')).toContain('Wren')
    // Sync on afterwards uses the same key: no new code, and the PC's code still opens this computer's copy.
    expect(await mac.account.status()).toMatchObject({ sync: 'off', hasKey: true })
    expect((await mac.account.turnOn()).code).toBeUndefined()
    // That turn-on syncs: let it finish before the folder is cleared, or the clearing races its writes.
    await until(async () => (await mac.account.status()).lastSynced !== undefined)
    mac.account.stop()
  })

  it('a code that does not open the copy changes nothing', async () => {
    const { mac, macProfile, restores } = await twoComputers()
    const wrong = await recoveryCode(newProfileKey())
    const brought = await mac.account.bringHere(wrong)
    expect(brought.ok).toBe(false)
    expect(brought.ok ? '' : brought.reason).toContain('did not open with this key')
    expect(restores).toEqual([])
    await expect(stat(join(macProfile, ACCOUNT_SYNC_FILE))).rejects.toThrow()
    expect(await readFile(join(macProfile, 'teammates.json'), 'utf8')).toContain('Atlas')
  })

  it('a code copied wrong is caught before the relay is asked', async () => {
    const { code, mac } = await twoComputers()
    // One character in the middle copied wrong: the two check bytes catch it.
    const brought = await mac.account.bringHere(`${code.slice(0, 2)}${code[2] === '0' ? '1' : '0'}${code.slice(3)}`)
    expect(brought).toEqual({ ok: false, reason: 'That code does not read right. It is 55 letters and digits in groups of five; check each group.' })
  })

  it('with no other computer keeping a copy, it says to turn Keep a copy on there first', async () => {
    // This computer keeps its own copy: that one is never offered back to it, even with its own code.
    const alone = anAccount(await aProfile(), { link: linkAs(7) })
    const own = await alone.account.turnOn()
    await until(async () => (await alone.account.status()).lastSynced !== undefined)
    const brought = await alone.account.bringHere(own.code!)
    expect(brought).toEqual({ ok: false, reason: 'No other computer keeps a copy for colin yet. Turn on Keep a copy there first.' })
    alone.account.stop()
  })
})

describe('the relay over HTTP', () => {
  it('reads the relay’s own list into copies and usage, and refuses a room name it should not', async () => {
    const relay = aRelay(7)
    const pc = anAccount(await aProfile(), { link: linkAs(7), relay, computer: "Colin's PC" })
    await pc.account.turnOn()
    await until(async () => (await pc.account.status()).lastSynced !== undefined)
    pc.account.stop()
    const { httpSyncRemote } = await import('./profile-sync.js')
    // The relay's list answer as its room route sends it, plus one entry that is not a room.
    const listed = (await relay.remote(linkAs(7)).copies())
    const body = JSON.stringify({ used: 1234, quota: 1 << 30, computers: [...listed.map((copy) => ({ room: copy.room, bytes: copy.bytes, files: [...copy.ids.map((id) => ({ id, size: 1, at: copy.at })), { id: '../not-an-id', size: 1, at: 0 }] })), { room: '../other', bytes: 5, files: [] }] })
    const asked: { url: string; method: string; auth: string | null }[] = []
    const remote = httpSyncRemote({
      relay: 'https://relay.test/', room: 'R'.repeat(22), token: 'secret-token',
      fetch: (async (url: string, init: RequestInit) => {
        asked.push({ url, method: String(init.method), auth: new Headers(init.headers).get('authorization') })
        return new Response(body, { status: 200, headers: { 'content-type': 'application/json' } })
      }) as typeof fetch
    })
    expect(await remote.copies()).toEqual(listed)
    expect(await remote.usage()).toEqual({ used: 1234, quota: 1 << 30 })
    expect(asked[0]).toEqual({ url: `https://relay.test/v1/room/${'R'.repeat(22)}/sync`, method: 'GET', auth: 'Bearer secret-token' })
  })
})

describe('approving a new computer from the one that has your Locust (slice 4b)', () => {
  async function pcAndMac(options: { askForMs?: number } = {}) {
    const relay = aRelay(7)
    const pcLink = linkAs(7)
    const pc = anAccount(await aProfile(), { link: pcLink, relay, computer: "Colin's PC" })
    await pc.account.turnOn()
    await until(async () => (await pc.account.status()).lastSynced !== undefined)
    const macProfile = await fresh()
    await writeFile(join(macProfile, 'teammates.json'), `${JSON.stringify({ schemaVersion: 15, teammates: [{ name: 'Atlas' }] })}\n`)
    const restores: string[] = []
    const mac = anAccount(macProfile, {
      link: linkAs(7), relay, computer: "Colin's Mac",
      restore: async (folder) => { restores.push(folder); return requestRestore(folder, macProfile, { appVersion: '0.845.0', liveRuns: 0, now: () => new Date() }).then((asked) => (asked.ok ? { ok: true as const } : asked)) },
      ...(options.askForMs === undefined ? {} : { askForMs: options.askForMs })
    })
    return { relay, pc, pcLink, mac, macProfile, restores }
  }
  /** The PC's view once it has read the Mac's ask. */
  const askSeen = async (pc: ReturnType<typeof anAccount>): Promise<YourAccountStatus> => {
    let seen: YourAccountStatus | undefined
    await until(async () => { seen = await pc.account.status(); return (seen.asks?.length ?? 0) > 0 })
    return seen!
  }

  it('the Mac asks, both screens show one code, the PC approves, and the Mac brings the copy in with no recovery code', async () => {
    const { pc, mac, macProfile, restores } = await pcAndMac()
    const asked = await mac.account.ask()
    expect(asked.asking?.match).toMatch(/^[0-9A-Z]{4}-[0-9A-Z]{4}$/)
    const onPc = await askSeen(pc)
    expect(onPc.asks).toEqual([{ room: expect.any(String), name: "Colin's Mac", match: asked.asking!.match }])
    // The Mac's waiting ask is not mistaken for a copy on either computer.
    expect(onPc.elsewhere).toBeUndefined()
    const approved = await pc.account.approve(onPc.asks![0]!.room, asked.asking!.match)
    expect(approved).toMatchObject({ good: true, message: "Approved. Colin's Mac brings your Locust in and restarts." })
    expect(approved.asks).toBeUndefined()
    await mac.account.checkAsk()
    expect(restores).toHaveLength(1)
    const status = await mac.account.status()
    expect(status.asking).toBeUndefined()
    expect(status.message).toBe("Your Locust from Colin's PC is ready. Restarting to bring it in…")
    expect((await applyPendingRestore(macProfile, { appVersion: '0.845.0', now: () => new Date() }))?.ok).toBe(true)
    expect(await readFile(join(macProfile, 'teammates.json'), 'utf8')).toContain('Wren')
    // The Mac now holds the same key: Keep a copy there shows no new code.
    expect((await mac.account.turnOn()).code).toBeUndefined()
    await until(async () => (await mac.account.status()).lastSynced !== undefined)
    pc.account.stop()
    mac.account.stop()
  })

  it('the ask leaves no trace: the Mac takes it down, and the PC then removes its grant', async () => {
    const { relay, pc, mac } = await pcAndMac()
    const asked = await mac.account.ask()
    const onPc = await askSeen(pc)
    await pc.account.approve(onPc.asks![0]!.room, asked.asking!.match)
    await mac.account.checkAsk()
    const idsAt = (room: string) => [...relay.held.keys()].filter((key) => key.includes(`/${room}/`)).map((key) => key.split('/').pop()!)
    expect(idsAt(onPc.asks![0]!.room)).not.toContain(ASK_ID)
    await until(async () => { await pc.account.status(); return [...relay.held.keys()].every((key) => !key.endsWith(grantIdOf(onPc.asks![0]!.room))) })
    pc.account.stop()
    mac.account.stop()
  })

  it('a code that is not the one the PC was shown sends nothing', async () => {
    const { relay, pc, mac, restores } = await pcAndMac()
    await mac.account.ask()
    const onPc = await askSeen(pc)
    const refused = await pc.account.approve(onPc.asks![0]!.room, 'AAAA-AAAA')
    expect(refused).toMatchObject({ good: false, message: 'The code changed. Ask again on the new computer, and approve only if both screens show the same code.' })
    expect([...relay.held.keys()].some((key) => key.endsWith(grantIdOf(onPc.asks![0]!.room)))).toBe(false)
    await mac.account.checkAsk()
    expect(restores).toEqual([])
    pc.account.stop()
    mac.account.stop()
  })

  it('a key swapped at the relay after the PC showed the code is refused: the PC reads the ask again before it seals', async () => {
    const { relay, pc, mac } = await pcAndMac()
    const asked = await mac.account.ask()
    const onPc = await askSeen(pc)
    // The relay puts its own key in the ask, keeping the Mac's name: the code it makes differs from the Mac's.
    const room = onPc.asks![0]!.room
    const forged = new TextEncoder().encode(JSON.stringify({ v: 1, name: "Colin's Mac", key: (await newAskKeys()).publicKey, at: Date.now() }))
    await relay.remote({ ...linkAs(7), room }).put(ASK_ID, forged)
    const refused = await pc.account.approve(room, asked.asking!.match)
    expect(refused.good).toBe(false)
    expect([...relay.held.keys()].some((key) => key.endsWith(grantIdOf(room)))).toBe(false)
    pc.account.stop()
    mac.account.stop()
  })

  it('Not mine puts the ask away on that computer and gives nothing', async () => {
    const { relay, pc, mac } = await pcAndMac()
    await mac.account.ask()
    const onPc = await askSeen(pc)
    const after = await pc.account.ignore(onPc.asks![0]!.room)
    expect(after.asks).toBeUndefined()
    expect((await pc.account.status()).asks).toBeUndefined()
    expect([...relay.held.keys()].some((key) => key.endsWith(grantIdOf(onPc.asks![0]!.room)))).toBe(false)
    pc.account.stop()
    mac.account.stop()
  })

  it('a computer without the key is never shown an ask to approve', async () => {
    const { relay, pc, mac } = await pcAndMac()
    await mac.account.ask()
    await askSeen(pc)
    const third = anAccount(await aProfile(), { link: linkAs(7), relay, computer: 'A third computer' })
    const before = relay.gets.length
    await third.account.status()
    await pause(50)
    expect((await third.account.status()).asks).toBeUndefined()
    // Not shown, and not read either: a computer that cannot approve has no business opening another's ask.
    expect(relay.gets.slice(before).filter((id) => id === ASK_ID)).toEqual([])
    pc.account.stop()
    mac.account.stop()
  })

  it('after ten minutes with no approval the ask is given up and taken down', async () => {
    const { relay, pc, mac } = await pcAndMac({ askForMs: 0 })
    await mac.account.ask()
    await pause(5)
    await mac.account.checkAsk()
    const status = await mac.account.status()
    expect(status.asking).toBeUndefined()
    expect(status.message).toBe('No computer approved this one in ten minutes. Ask again, or use your recovery code.')
    expect([...relay.held.keys()].some((key) => key.endsWith(ASK_ID))).toBe(false)
    pc.account.stop()
    mac.account.stop()
  })
})
