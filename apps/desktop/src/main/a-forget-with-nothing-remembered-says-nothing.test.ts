import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { CodexMissionUpdate, PublicTeammate, WorkspaceSettings } from '../shared/ipc.js'
import { createMemoryReader } from './memory-reader.js'
import { createMemoryStore } from './memory-store.js'

/*
 * A FORGET WITH NOTHING REMEMBERED SAYS NOTHING (0.785).
 *
 * Clean-machine drive, packaged 0.784, 2026-10-17: told its last turn was undone, a teammate on a new computer tried
 * to forget the file it had handed over. Nothing had ever been remembered there, and the thread said in amber "a
 * conversation tried to forget something and could not. ... It is still remembered." -- wrong twice, and the first
 * thing a new person saw after their first Undo. A forget where nothing is remembered has nothing it could mean and
 * nothing for the person to settle, so it says nothing. A forget that misses while memories are there still says so,
 * now opening with a capital.
 */

const SETTINGS: WorkspaceSettings = { relay: true, relayHopCap: 6, interrupt: false, memoryMode: 'auto', autoMode: false, askConnectors: false, keepATodoList: false, layout: 'auto', tube: 'full', replySize: 'standard' }
const SHOP = { workspaceId: 'ws_shop', workspaceName: 'shop' }
const REPLY = 'Done.\n<locust-memory>\nforget :: laptop-setup.md, the checklist the person undid\n</locust-memory>'

let root: string | undefined
afterEach(async () => {
  if (root !== undefined) await rm(root, { recursive: true, force: true })
  root = undefined
})

async function setUp() {
  root = await mkdtemp(join(tmpdir(), 'locust-forget-nothing-'))
  let ids = 0
  const memories = createMemoryStore({ rootDirectory: root, now: () => new Date('2026-10-17T20:00:00.000Z'), createId: () => `id${String(++ids)}` })
  const updates: CodexMissionUpdate[] = []
  const reader = createMemoryReader({
    memories,
    ledger: {
      getMission: async () =>
        ({
          metadata: { missionId: 'mission_1', runId: 'run_1', workspaceId: SHOP.workspaceId },
          phase: 'completed',
          events: [{ type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text: REPLY, final: true } }]
        }) as never
    },
    teammates: {
      list: async (): Promise<readonly PublicTeammate[]> => [],
      missionOwners: async (): Promise<Readonly<Record<string, string>>> => ({}),
      readSettings: async () => SETTINGS
    },
    workspaceName: SHOP.workspaceName,
    notify: (update) => {
      updates.push(update)
    }
  })
  const notices = (): string[] =>
    updates.filter((update) => update.kind === 'relay-notice').map((update) => (update as { readonly message: string }).message)
  return { memories, reader, notices }
}

describe('a forget with nothing remembered', () => {
  it('is answered as nothing remembered, by the store', async () => {
    const { memories } = await setUp()
    expect((await memories.forget('anything at all', SHOP.workspaceId)).refusal).toBe('nothing-remembered')
  })

  it('says nothing in the thread', async () => {
    const { reader, notices } = await setUp()
    await reader.onRunEnded({ missionId: 'mission_1' })
    expect(notices()).toEqual([])
  })

  it('still says a miss when something is remembered, opening with a capital', async () => {
    const { memories, reader, notices } = await setUp()
    await memories.add({ text: 'Tests run with pnpm test.', scope: 'workspace', ...SHOP, by: { name: 'you' }, status: 'kept' })
    expect((await memories.forget('anything at all', SHOP.workspaceId)).refusal).toBe('nothing-matched')
    await reader.onRunEnded({ missionId: 'mission_1' })
    expect(notices()).toHaveLength(1)
    expect(notices()[0]).toMatch(/^A conversation tried to forget something and could not\./)
    expect(notices()[0]).toContain('It is still remembered.')
  })
})
