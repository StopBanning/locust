import { randomBytes } from 'node:crypto'
import { appendFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { syncCore, type SyncObject } from '../../../relay/src/sync.js'
import { readBackup } from './profile-backup.js'
import { CHUNK_BYTES, MANIFEST_ID, newProfileKey, openFile, syncKeys } from './profile-sync-crypto.js'
import { createSyncSchedule, httpSyncRemote, PROFILE_SYNC_FILE, pullProfile, pushProfile, SyncRefused, type SyncRemote } from './profile-sync.js'

/**
 * YOUR LOCUST IS KEPT CURRENT AT THE RELAY (docs/PLAN-2026-10-30-ACCOUNTS.md, slice 2). Colin: "we need accounts,
 * because I don't have any of my chats." A signed-in computer sends its profile's backup set, sealed, to the relay; a
 * second push sends only what changed; nothing a backup leaves out is ever sent; and another computer opens the copy
 * into a backup folder that the existing restore reads, refusing a copy caught halfway through a sync.
 *
 * The relay here is the relay's own rules (apps/relay/src/sync.ts) over an in-memory bucket.
 */
const SECRET = 'NEVER-LEAVES-THIS-COMPUTER-q7'
const json = (value: unknown): string => `${JSON.stringify(value)}\n`
const room = (): string => randomBytes(16).toString('base64url')
const folders: string[] = []
const fresh = async (): Promise<string> => { const folder = await mkdtemp(join(tmpdir(), 'locust-sync-')); folders.push(folder); return folder }
afterEach(async () => { for (const folder of folders.splice(0)) await rm(folder, { recursive: true, force: true }) })

function aRelay(user: number) {
  const held = new Map<string, { bytes: Uint8Array; uploaded: number }>()
  const ledger = new Map<string, unknown>()
  const core = syncCore({
    objects: {
      head: async (key) => { const one = held.get(key); return one === undefined ? undefined : { key, size: one.bytes.length, uploaded: one.uploaded } },
      get: async (key) => { const one = held.get(key); return one === undefined ? undefined : { body: one.bytes.slice().buffer, size: one.bytes.length } },
      put: async (key, body, _size) => { held.set(key, { bytes: new Uint8Array(body as ArrayBuffer), uploaded: Date.now() }) },
      list: async (prefix) => [...held].filter(([key]) => key.startsWith(prefix)).map(([key, one]): SyncObject => ({ key, size: one.bytes.length, uploaded: one.uploaded })),
      delete: async (keys) => { for (const key of keys) held.delete(key) }
    },
    ledger: { get: async <T>(key: string) => ledger.get(key) as T | undefined, put: async (key, value) => { ledger.set(key, value) } },
    now: () => Date.now()
  })
  const puts: { id: string; box: Uint8Array }[] = []
  const removes: string[] = []
  const remoteFor = (mine: string): SyncRemote => ({
    async put(id, box) {
      const answer = await core.put({ user, room: mine, id, size: box.length, body: box.slice().buffer })
      if (answer.status !== 204) throw new SyncRefused(answer.status, String(answer.body?.error))
      puts.push({ id, box })
    },
    async get(from, id) {
      const answer = await core.get({ user, from, id })
      return answer.status === 200 ? new Uint8Array(answer.bytes as ArrayBuffer) : undefined
    },
    async remove(id) {
      await core.remove({ user, room: mine, id })
      removes.push(id)
    }
  })
  return { held, puts, removes, remoteFor }
}

async function aProfile(): Promise<string> {
  const root = await fresh()
  await writeFile(join(root, 'teammates.json'), json({ schemaVersion: 15, teammates: [{ name: 'Wren' }, { name: 'Atlas' }] }))
  await writeFile(join(root, 'memories.json'), json({ schemaVersion: 2, memories: [{ text: 'Colin ships on Mondays' }] }))
  await mkdir(join(root, 'mission-ledger'))
  await writeFile(join(root, 'mission-ledger', 'mission_a.jsonl'), json({ type: 'started', text: 'first turn' }))
  await writeFile(join(root, 'mission-ledger', 'mission_b.jsonl'), json({ type: 'started', text: 'other turn' }))
  await writeFile(join(root, 'mission-ledger', '.writable'), '')
  // What a backup leaves out, each holding the secret: none of it may reach the relay, sealed or not.
  await writeFile(join(root, 'own-models.json'), json({ keys: { openai: SECRET } }))
  await writeFile(join(root, 'phone-relay.json'), json({ token: SECRET }))
  await mkdir(join(root, 'agent-accounts'))
  await writeFile(join(root, 'agent-accounts', 'auth.json'), json({ token: SECRET }))
  await mkdir(join(root, 'checkpoints'))
  await writeFile(join(root, 'checkpoints', '.env'), `KEY=${SECRET}\n`)
  return root
}

const keysFor = async (key = newProfileKey()) => ({ key, keys: await syncKeys(key, 7) })
const options = (keys: Awaited<ReturnType<typeof syncKeys>>, remote: SyncRemote, mine: string) =>
  ({ keys, remote, room: mine, computer: "Colin's PC", appVersion: '0.845.0', now: () => new Date('2026-10-31T18:00:00Z') })

describe('a push', () => {
  it('sends the backup set sealed, and nothing a backup leaves out', async () => {
    const profile = await aProfile()
    const relay = aRelay(7)
    const mine = room()
    const { keys } = await keysFor()
    const pushed = await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))
    expect(pushed).toMatchObject({ ok: true, files: 4, sent: 4 })
    const manifestId = MANIFEST_ID
    const manifest = JSON.parse(new TextDecoder().decode(await openFile(keys, manifestId, relay.puts.at(-1)!.box))) as { files: { path: string }[]; computer: string }
    // The manifest goes up last, so a copy is never described before its pieces are there.
    expect(relay.puts.at(-1)!.id).toBe(manifestId)
    expect(manifest.files.map((file) => file.path).sort()).toEqual(['memories.json', 'mission-ledger/mission_a.jsonl', 'mission-ledger/mission_b.jsonl', 'teammates.json'])
    expect(manifest.computer).toBe("Colin's PC")
    for (const { id, box } of relay.puts) {
      const opened = new TextDecoder().decode(await openFile(keys, id, box))
      expect(opened).not.toContain(SECRET)
      // And sealed: the relay holds no word of what is in it, nor any file's name.
      expect(Buffer.from(box).toString('latin1')).not.toMatch(/Wren|first turn|teammates|mission_a/)
    }
    expect([...relay.held.keys()].join(' ')).not.toMatch(/teammates|mission|\.json/)
  })

  it('sends only what changed: a new turn sends its conversation’s last piece and the manifest', async () => {
    const profile = await aProfile()
    const relay = aRelay(7)
    const mine = room()
    const { keys } = await keysFor()
    await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))
    relay.puts.length = 0
    await appendFile(join(profile, 'mission-ledger', 'mission_a.jsonl'), json({ type: 'turn', text: 'second turn' }))
    expect(await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))).toMatchObject({ ok: true, sent: 1, removed: 0 })
    expect(relay.puts).toHaveLength(2)
    // Nothing changed: the manifest alone.
    relay.puts.length = 0
    expect(await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))).toMatchObject({ ok: true, sent: 0 })
    expect(relay.puts).toHaveLength(1)
  })

  it('removes the pieces of a conversation deleted since, once the manifest no longer names them', async () => {
    const profile = await aProfile()
    const relay = aRelay(7)
    const mine = room()
    const { keys } = await keysFor()
    await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))
    const before = relay.held.size
    await rm(join(profile, 'mission-ledger', 'mission_b.jsonl'))
    expect(await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))).toMatchObject({ ok: true, removed: 1, files: 3 })
    expect(relay.held.size).toBe(before - 1)
  })

  it('cuts a big conversation into pieces, and sends again only the piece that grew', async () => {
    const profile = await aProfile()
    await writeFile(join(profile, 'mission-ledger', 'mission_a.jsonl'), 'x'.repeat(CHUNK_BYTES + 10))
    const relay = aRelay(7)
    const mine = room()
    const { keys } = await keysFor()
    expect(await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))).toMatchObject({ ok: true, sent: 5 })
    await appendFile(join(profile, 'mission-ledger', 'mission_a.jsonl'), 'y')
    expect(await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))).toMatchObject({ ok: true, sent: 1, bytes: 11 })
  })

  it('sends everything again for another account or a new room, and says so plainly when the relay refuses', async () => {
    const profile = await aProfile()
    const relay = aRelay(7)
    const mine = room()
    const { keys } = await keysFor()
    await pushProfile(profile, options(keys, relay.remoteFor(mine), mine))
    const other = room()
    expect(await pushProfile(profile, options(keys, relay.remoteFor(other), other))).toMatchObject({ ok: true, sent: 4 })
    const state = await readFile(join(profile, PROFILE_SYNC_FILE), 'utf8')
    const refusing: SyncRemote = { put: async () => { throw new SyncRefused(507, 'Your copy is full.') }, get: async () => undefined, remove: async () => undefined }
    expect(await pushProfile(profile, options(keys, refusing, other))).toEqual({ ok: false, reason: 'Your copy is full.' })
    // A push that failed leaves the state as it was, so the next one sends what this one could not.
    expect(await readFile(join(profile, PROFILE_SYNC_FILE), 'utf8')).toBe(state)
  })
})

describe('a pull', () => {
  it('opens another computer’s copy as a backup the existing restore reads', async () => {
    const profile = await aProfile()
    const relay = aRelay(7)
    const pc = room()
    const mac = room()
    const { key, keys } = await keysFor()
    await pushProfile(profile, options(keys, relay.remoteFor(pc), pc))
    const folder = join(await fresh(), 'copy')
    const pulled = await pullProfile({ keys: await syncKeys(key, 7), remote: relay.remoteFor(mac), from: pc, folder })
    expect(pulled).toMatchObject({ ok: true, computer: "Colin's PC", files: 4 })
    expect(await readFile(join(folder, 'profile', 'mission-ledger', 'mission_a.jsonl'), 'utf8')).toContain('first turn')
    const reading = await readBackup(folder, { appVersion: '0.845.0' })
    expect(reading).toMatchObject({ ok: true, manifest: { counts: { teammates: 2, memories: 1, conversations: 2 } } })
  })

  it('does not open with another key, and refuses a copy caught halfway through a sync', async () => {
    const profile = await aProfile()
    const relay = aRelay(7)
    const pc = room()
    const { keys } = await keysFor()
    await pushProfile(profile, options(keys, relay.remoteFor(pc), pc))
    const wrong = await pullProfile({ keys: (await keysFor()).keys, remote: relay.remoteFor(room()), from: pc, folder: join(await fresh(), 'a') })
    expect(wrong).toMatchObject({ ok: false, reason: expect.stringContaining('did not open with this key') })

    // The manifest of one push, with a piece of the next: the piece no longer matches what the manifest says.
    const manifestId = MANIFEST_ID
    const manifestKey = [...relay.held.keys()].find((name) => name.endsWith(`/${pc}/${manifestId}`))!
    const oldManifest = relay.held.get(manifestKey)!
    await appendFile(join(profile, 'mission-ledger', 'mission_a.jsonl'), json({ type: 'turn', text: 'second turn' }))
    await pushProfile(profile, options(keys, relay.remoteFor(pc), pc))
    relay.held.set(manifestKey, oldManifest)
    const torn = await pullProfile({ keys, remote: relay.remoteFor(room()), from: pc, folder: join(await fresh(), 'b') })
    expect(torn).toEqual({ ok: false, reason: "Colin's PC was syncing while this was read. Try again in a minute." })
  })
})

describe('the relay’s routes', () => {
  it('are called with this computer’s token, and their refusals said in words', async () => {
    const calls: { url: string; method: string; auth: string }[] = []
    const answers = [new Response(null, { status: 204 }), new Response(new Uint8Array([1, 2]), { status: 200 }), new Response('{}', { status: 404 }), new Response('{"error":"quota"}', { status: 507 })]
    const remote = httpSyncRemote({
      relay: 'https://app.locust.lol/', room: 'ROOM', token: 'TOKEN',
      fetch: (async (url: string, init: RequestInit) => { calls.push({ url, method: init.method!, auth: (init.headers as Record<string, string>).authorization! }); return answers.shift()! }) as typeof fetch
    })
    await remote.put('ID', new Uint8Array([9]))
    expect(await remote.get('PC', 'ID')).toEqual(new Uint8Array([1, 2]))
    expect(await remote.get('PC', 'GONE')).toBeUndefined()
    await expect(remote.put('ID', new Uint8Array([9]))).rejects.toThrow('full (1 GB)')
    expect(calls.map((call) => `${call.method} ${call.url}`)).toEqual([
      'PUT https://app.locust.lol/v1/room/ROOM/sync/ID', 'GET https://app.locust.lol/v1/room/ROOM/sync/PC/ID',
      'GET https://app.locust.lol/v1/room/ROOM/sync/PC/GONE', 'PUT https://app.locust.lol/v1/room/ROOM/sync/ID'
    ])
    expect(new Set(calls.map((call) => call.auth))).toEqual(new Set(['Bearer TOKEN']))
  })
})

describe('when a sync runs', () => {
  it('a quiet minute after the last change, never during a run, and once more for a change made while it ran', async () => {
    vi.useFakeTimers()
    try {
      let busy = false
      let finish: () => void = () => undefined
      const push = vi.fn(() => new Promise<void>((resolve) => { finish = resolve }))
      const schedule = createSyncSchedule({ push, busy: () => busy, quietMs: 60_000 })
      schedule.touched()
      await vi.advanceTimersByTimeAsync(50_000)
      schedule.touched()
      await vi.advanceTimersByTimeAsync(50_000)
      expect(push).not.toHaveBeenCalled()
      busy = true
      await vi.advanceTimersByTimeAsync(10_000)
      expect(push).not.toHaveBeenCalled()
      busy = false
      await vi.advanceTimersByTimeAsync(60_000)
      expect(push).toHaveBeenCalledTimes(1)
      schedule.touched()
      finish()
      await vi.advanceTimersByTimeAsync(60_000)
      expect(push).toHaveBeenCalledTimes(2)
      schedule.stop()
      schedule.touched()
      finish()
      await vi.advanceTimersByTimeAsync(120_000)
      expect(push).toHaveBeenCalledTimes(2)
    } finally {
      vi.useRealTimers()
    }
  })
})
