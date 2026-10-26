import { randomBytes } from 'node:crypto'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { createRoutineHooks, ROUTINE_HOOKS_FILE, type HookRelay } from './routine-hooks.js'
import { createRoutineStore } from './routine-store.js'

/**
 * A CALLED LINK IS CLOSED WHEN ITS RECORD CANNOT BE USED (2026-10-25). The relay keeps accepting calls until this
 * computer tells it to stop. A record that cannot be decrypted, or a computer that cannot encrypt, must still close
 * the link it already registered. Leaving it open is the failure these tests name.
 */

const box = {
  available: () => true,
  encrypt: (text: string) => Buffer.from([...Buffer.from(text)].map((byte) => byte ^ 0x5a)),
  decrypt: (cipher: Buffer) => Buffer.from([...cipher].map((byte) => byte ^ 0x5a)).toString('utf8')
}
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })

async function registered() {
  const root = await mkdtemp(join(tmpdir(), 'locust-hook-close-'))
  roots.push(root)
  const store = createRoutineStore({ rootDirectory: root })
  const routine = await store.create({ name: 'Event summary', teammateId: 'tm_a', route: { runtime: 'codex', model: 'account-default', mode: 'ask' }, steps: ['Summarize the event.'], learnedFrom: [], schedule: { kind: 'hook' }, workspaceId: 'ws_a' })
  const room = randomBytes(16).toString('base64url')
  const hashes = new Map<string, string>()
  const relay: HookRelay = {
    room: () => ({ relay: 'https://example.test', room }),
    connected: () => true,
    hookOpen: async (id, hash) => { hashes.set(id, hash); return true },
    hookClose: async (id) => { hashes.delete(id); return true },
    hookLetters: async () => ({ items: [], lost: 0 }),
    hookCollected: async () => true
  }
  const options = { file: join(root, ROUTINE_HOOKS_FILE), secrets: box, relay: () => relay, routines: () => store.list(), lost: async () => undefined, tick: () => undefined, protectFile: async () => undefined }
  const hooks = createRoutineHooks(options)
  await hooks.sync()
  expect(hashes.size).toBe(1)
  return { options, hashes }
}

describe('a called link is closed when its record cannot be used', () => {
  it('closes the relay link when the kept record cannot be read', async () => {
    const { options, hashes } = await registered()
    await writeFile(options.file, '{')
    const again = createRoutineHooks(options)
    await again.sync().catch(() => undefined)
    expect(hashes.size).toBe(0)
  })

  it('closes the relay link when this computer cannot encrypt', async () => {
    const { options, hashes } = await registered()
    const locked = createRoutineHooks({ ...options, secrets: { ...box, available: () => false } })
    await locked.sync()
    expect(hashes.size).toBe(0)
  })
})
