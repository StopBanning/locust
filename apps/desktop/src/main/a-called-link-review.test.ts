import { createHash, randomBytes } from 'node:crypto'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { roomCore, type Attachment, type Peer } from '../../../relay/src/room-core.js'
import type { HookLetter } from '../../../relay/src/protocol.js'
import { deskAccess } from '../shared/desk-calls.js'
import type { CodexMissionStartResponse } from '../shared/ipc.js'
import { ROUTINE_HOOK_NEW, ROUTINE_HOOK_READ } from '../shared/routine-hook.js'
import { createRoutineHooks, hookNote, ROUTINE_HOOKS_FILE, type HookRelay } from './routine-hooks.js'
import { createRoutineRunner, type RoutineRunnerOptions } from './routine-runner.js'
import { createRoutineStore } from './routine-store.js'

/**
 * A CALLED LINK GIVES NOTHING AWAY (2026-10-25). The password stays on this computer, a replaced password is redacted
 * for the week a call can still arrive, an unknown room costs the same hash as a known one, and the caller's content
 * type stays inside the untrusted block.
 */

const password = (): string => randomBytes(32).toString('base64url')
const box = { available: () => true, encrypt: (text: string) => Buffer.from([...Buffer.from(text)].map((byte) => byte ^ 0x5a)), decrypt: (cipher: Buffer) => Buffer.from([...cipher].map((byte) => byte ^ 0x5a)).toString('utf8') }
const digest = (text: string): string => createHash('sha256').update(text).digest('base64url')

describe('a called link gives its room away before it hashes', () => {
  async function room(owned: boolean) {
    let now = 1_800_000_000_000
    const stored = new Map<string, unknown>()
    let hashes = 0
    let attachment: Attachment = { as: 'new', at: now }
    const peer: Peer = {
      get attachment() { return attachment },
      attach: (next) => { attachment = next },
      send: () => undefined,
      close: () => undefined
    }
    const core = roomCore({
      store: {
        get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
        put: async (key, value) => { stored.set(key, structuredClone(value)) },
        delete: async (keys) => { for (const key of keys) stored.delete(key) },
        deleteAll: async () => { stored.clear() }
      },
      peers: () => [peer],
      now: () => now,
      setAlarm: async () => undefined,
      random: (size) => randomBytes(size).toString('base64url'),
      sha256: async (text) => { hashes += 1; return digest(text) }
    })
    const say = (frame: unknown) => core.message(peer, JSON.stringify(frame))
    if (owned) {
      await say({ t: 'auth', as: 'pc', token: password() })
      await say({ t: 'hook-open', hook: 'rt_a', hash: digest(password()) })
    }
    hashes = 0
    const guess = password()
    return { answer: () => core.hook({ method: 'POST', secret: guess, body: 'hello', contentType: 'text/plain' }), hashes: () => hashes }
  }

  it('hashes a well-formed wrong password for an unknown room the same as for a room that has a link', async () => {
    const unknown = await room(false)
    const known = await room(true)
    const unseen = await unknown.answer()
    const missed = await known.answer()
    expect(unseen).toEqual(missed)
    expect(unknown.hashes()).toBe(known.hashes())
    expect(unknown.hashes()).toBeGreaterThan(0)
  })
})

describe('a called link is not a browser read', () => {
  it('is not given to a browser that is only reading', () => {
    expect(deskAccess(ROUTINE_HOOK_READ, ['rt_a'])).toBeUndefined()
  })

  it('is not opened or replaced from a browser', () => {
    expect(deskAccess(ROUTINE_HOOK_NEW, ['rt_a'])).toBeUndefined()
  })
})

describe('a retired link password still reaches the run', () => {
  const roots: string[] = []
  afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })

  it('redacts the password a call already sent once the link has been replaced', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-hook-review-'))
    roots.push(root)
    const store = createRoutineStore({ rootDirectory: root })
    const routine = await store.create({ name: 'Event summary', teammateId: 'tm_a', route: { runtime: 'codex', model: 'account-default', mode: 'auto' }, steps: ['Summarize the event.'], learnedFrom: [], schedule: { kind: 'hook' }, workspaceId: 'ws_a' })
    const now = 1_800_000_000_000
    const boxes: HookLetter[] = []
    const room = randomBytes(16).toString('base64url')
    const relay: HookRelay = {
      room: () => ({ relay: 'https://example.test', room }),
      connected: () => true,
      hookOpen: async () => true,
      hookClose: async () => true,
      hookLetters: async (id) => ({ items: boxes.filter((box) => box.hook === id), lost: 0 }),
      hookCollected: async (_id, ids) => { for (let index = boxes.length - 1; index >= 0; index -= 1) if (ids.includes(boxes[index]!.id)) boxes.splice(index, 1); return true }
    }
    const hooks = createRoutineHooks({ file: join(root, ROUTINE_HOOKS_FILE), secrets: box, relay: () => relay, routines: () => store.list(), lost: async () => undefined, tick: () => undefined, protectFile: async () => undefined, now: () => now })
    const starts: Parameters<RoutineRunnerOptions['start']>[0][] = []
    const runner = createRoutineRunner({ workspaceId: 'ws_a', routines: store, hooks,
      peerContextFor: async (id) => ({ self: { teammateId: id, name: 'Reader', role: 'Code' }, others: [] }),
      routeOf: async () => ({ runtime: 'codex', model: 'account-default', mode: 'auto' }),
      start: async (input) => {
        starts.push(input)
        return { ok: true, data: { missionId: 'mission_1', runId: 'run_1', runtime: 'codex', sandbox: 'read-only', model: '', resolvedRouteId: 'test', cliVersion: null, peerMessages: [], peerDeliveryFailed: false } } as CodexMissionStartResponse
      },
      assignOwner: async () => undefined,
      phaseOf: async () => 'interrupted',
      isLive: () => false,
      askedAQuestion: async () => false,
      notify: () => undefined
    })
    await hooks.sync()
    const old = (await hooks.state(routine.routineId)).url!.split('/').at(-1)!
    boxes.push({ id: `${String(now).padStart(13, '0')}-${randomBytes(6).toString('base64url')}`, hook: routine.routineId, at: now, body: old, contentType: 'text/plain' })
    await hooks.state(routine.routineId, true)
    await hooks.sync()
    await runner.tick(new Date(now))
    expect((starts[0]?.prompt ?? '').includes(old)).toBe(false)
  })

  it('drops a retired password from the file once it is older than a week', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-hook-review-'))
    roots.push(root)
    const store = createRoutineStore({ rootDirectory: root })
    const routine = await store.create({ name: 'Event summary', teammateId: 'tm_a', route: { runtime: 'codex', model: 'account-default', mode: 'auto' }, steps: ['Summarize the event.'], learnedFrom: [], schedule: { kind: 'hook' }, workspaceId: 'ws_a' })
    let now = 1_800_000_000_000
    const room = randomBytes(16).toString('base64url')
    const relay: HookRelay = {
      room: () => ({ relay: 'https://example.test', room }),
      connected: () => true,
      hookOpen: async () => true,
      hookClose: async () => true,
      hookLetters: async () => ({ items: [], lost: 0 }),
      hookCollected: async () => true
    }
    const hooks = createRoutineHooks({ file: join(root, ROUTINE_HOOKS_FILE), secrets: box, relay: () => relay, routines: () => store.list(), lost: async () => undefined, tick: () => undefined, protectFile: async () => undefined, now: () => now })
    const open = async (): Promise<string[]> => {
      const disk = JSON.parse(await readFile(join(root, ROUTINE_HOOKS_FILE), 'utf8')) as { readonly entries: Readonly<Record<string, { readonly secret: string; readonly retired?: readonly { readonly secret: string }[] }>> }
      return Object.values(disk.entries).flatMap((entry) => [entry.secret, ...(entry.retired ?? []).map((item) => item.secret)]).map((secret) => box.decrypt(Buffer.from(secret, 'base64')))
    }
    await hooks.sync()
    const old = (await hooks.state(routine.routineId)).url!.split('/').at(-1)!
    await store.update({ routineId: routine.routineId, name: routine.name, steps: routine.steps, schedule: null })
    await hooks.sync()
    expect((await open()).includes(old)).toBe(true)
    now += 7 * 24 * 60 * 60_000 + 1
    await hooks.sync()
    const later = await readFile(join(root, ROUTINE_HOOKS_FILE), 'utf8')
    expect(later.includes(old)).toBe(false)
    expect((await open()).includes(old)).toBe(false)
  })
})

describe('a caller\'s content type stays inside the fence', () => {
  it('does not leave the caller\'s content type outside the untrusted block', () => {
    const type = 'caller-label'
    const note = hookNote({ ids: [], count: 1, at: 1_800_000_000_000, body: 'hello', contentType: type }, 'Read this.')
    const fenced = note.split('webhook-data\n')[1] ?? ''
    const inside = fenced.slice(0, fenced.lastIndexOf('\n`'))
    expect(inside).toContain(type)
  })
})
