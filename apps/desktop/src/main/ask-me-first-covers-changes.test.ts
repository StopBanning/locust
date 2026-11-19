import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { CodexMissionUpdate, PublicTeammate, WorkspaceSettings } from '../shared/ipc.js'
import { createMemoryReader } from './memory-reader.js'
import { createMemoryStore } from './memory-store.js'

/*
 * "ASK ME FIRST" COVERS A REWRITE AND A FORGET (0.315).
 *
 * In ask mode only a NEW memory waited for the person. A named rewrite of a
 * kept memory, or a forget, applied at once -- the one promise of that mode
 * it did not keep (harness review, 2026-09-24, reported #3). Now each waits
 * beside the kept memory as a proposal, and the kept memory stays as it was,
 * and briefed, until the person answers.
 */

const NOW = '2026-09-24T12:00:00.000Z'
let root: string | undefined

afterEach(async () => {
  if (root !== undefined) await rm(root, { recursive: true, force: true })
  root = undefined
})

async function store() {
  root = await mkdtemp(join(tmpdir(), 'locust-ask-'))
  let ids = 0
  return createMemoryStore({ rootDirectory: root, now: () => new Date(NOW), createId: () => `id${String(++ids)}` })
}

const YURT = { teammateId: 'tm_yurt', name: 'Yurt' }
const SHOP = { workspaceId: 'ws_shop', workspaceName: 'shop' }

async function withKept(text: string, name?: string) {
  const memories = await store()
  const { memory } = await memories.add({ text, scope: 'workspace', ...SHOP, by: YURT, status: 'kept', ...(name === undefined ? {} : { name }) })
  return { memories, kept: memory }
}

describe('a named rewrite in ask mode', () => {
  it('waits beside the kept memory, which stays as it was and briefed', async () => {
    const { memories, kept } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    const result = await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed', name: 'deploy-day', missionId: 'mission_2' })
    expect(result.proposedChange).toBe(true)
    expect(result.memory).toMatchObject({ status: 'proposed', replaces: kept.memoryId, text: 'Deploys go out on Thursdays.', missionId: 'mission_2' })
    expect((await memories.briefed('ws_shop')).map((memory) => memory.text)).toEqual(['Deploys go out on Fridays.'])
    const held = (await memories.list()).find((memory) => memory.memoryId === kept.memoryId)
    expect(held?.text).toBe('Deploys go out on Fridays.')
    expect(held?.previousText).toBeUndefined()
  })

  it('a second rewrite before the person answers updates the same proposal', async () => {
    const { memories } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    const first = await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed', name: 'deploy-day' })
    const second = await memories.add({ text: 'Deploys go out on Wednesdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed', name: 'deploy-day' })
    expect(second.memory.memoryId).toBe(first.memory.memoryId)
    expect((await memories.list()).filter((memory) => memory.status === 'proposed').map((memory) => memory.text)).toEqual(['Deploys go out on Wednesdays.'])
  })

  it('Keep the change rewrites the kept memory, one step back kept, and the proposal goes', async () => {
    const { memories, kept } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    const { memory: proposal } = await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed', name: 'deploy-day', missionId: 'mission_2' })
    const changed = await memories.update({ memoryId: proposal.memoryId, keep: true })
    expect(changed).toMatchObject({ memoryId: kept.memoryId, text: 'Deploys go out on Thursdays.', previousText: 'Deploys go out on Fridays.', updatedAt: NOW, missionId: 'mission_2', name: 'deploy-day' })
    expect((await memories.list()).map((memory) => memory.memoryId)).toEqual([kept.memoryId])
    expect((await memories.briefed('ws_shop')).map((memory) => memory.text)).toEqual(['Deploys go out on Thursdays.'])
  })

  it('Keep the old one drops only the proposal', async () => {
    const { memories, kept } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    const { memory: proposal } = await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed', name: 'deploy-day' })
    await memories.remove(proposal.memoryId)
    expect(await memories.list()).toEqual([kept])
  })

  it('auto mode still rewrites at once, as it always did', async () => {
    const { memories, kept } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    const result = await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'kept', name: 'deploy-day' })
    expect(result.rewritten).toBe(true)
    expect(result.memory).toMatchObject({ memoryId: kept.memoryId, text: 'Deploys go out on Thursdays.', previousText: 'Deploys go out on Fridays.' })
  })
})

describe('a forget in ask mode', () => {
  const ASK = { by: YURT, missionId: 'mission_3', ask: true }

  it('waits beside the kept memory, which stays briefed', async () => {
    const { memories, kept } = await withKept('The API is on port 3000.')
    const result = await memories.forget('The API is on port 3000', 'ws_shop', ASK)
    expect(result.removed).toEqual([])
    expect(result.proposed).toEqual(['The API is on port 3000.'])
    expect((await memories.briefed('ws_shop')).map((memory) => memory.text)).toEqual(['The API is on port 3000.'])
    expect((await memories.list()).find((memory) => memory.status === 'proposed')).toMatchObject({ forgets: kept.memoryId, text: 'The API is on port 3000.', missionId: 'mission_3', by: YURT })
  })

  it('asked twice, still one proposal -- and the proposal itself never makes the quote ambiguous', async () => {
    const { memories } = await withKept('The API is on port 3000.')
    await memories.forget('The API is on port 3000', 'ws_shop', ASK)
    const again = await memories.forget('The API is on port 3000', 'ws_shop', ASK)
    expect(again.refusal).toBeUndefined()
    expect((await memories.list()).filter((memory) => memory.status === 'proposed')).toHaveLength(1)
  })

  it('Forget it removes the kept memory and the proposal', async () => {
    const { memories } = await withKept('The API is on port 3000.')
    await memories.forget('The API is on port 3000', 'ws_shop', ASK)
    const proposal = (await memories.list()).find((memory) => memory.status === 'proposed')!
    await memories.update({ memoryId: proposal.memoryId, keep: true })
    expect(await memories.list()).toEqual([])
  })

  it('Keep it drops only the proposal', async () => {
    const { memories } = await withKept('The API is on port 3000.')
    await memories.forget('The API is on port 3000', 'ws_shop', ASK)
    const proposal = (await memories.list()).find((memory) => memory.status === 'proposed')!
    await memories.remove(proposal.memoryId)
    expect((await memories.list()).map((memory) => memory.text)).toEqual(['The API is on port 3000.'])
  })

  it('a memory nobody kept yet simply goes: there is nothing of the person’s to protect', async () => {
    const memories = await store()
    await memories.add({ text: 'The API is on port 3000.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed' })
    const result = await memories.forget('The API is on port 3000', 'ws_shop', ASK)
    expect(result.removed).toEqual(['The API is on port 3000.'])
    expect(await memories.list()).toEqual([])
  })

  it('without ask, a forget applies at once, as it always did', async () => {
    const { memories } = await withKept('The API is on port 3000.')
    const result = await memories.forget('The API is on port 3000', 'ws_shop')
    expect(result.removed).toEqual(['The API is on port 3000.'])
    expect(await memories.list()).toEqual([])
  })
})

describe('a proposal whose memory is gone', () => {
  it('goes with it, whichever way the memory went', async () => {
    const { memories, kept } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed', name: 'deploy-day' })
    await memories.forget('Deploys go out on Fridays', 'ws_shop', { by: YURT, ask: true })
    expect((await memories.list()).filter((memory) => memory.status === 'proposed')).toHaveLength(2)
    // The person removes the memory itself: both questions about it are moot.
    await memories.remove(kept.memoryId)
    expect(await memories.list()).toEqual([])
  })

})

describe('words already waiting as a change', () => {
  it('are not asked a second time by a plain remember', async () => {
    const { memories } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed', name: 'deploy-day' })
    const again = await memories.add({ text: 'Deploys go out on Thursdays.', scope: 'workspace', ...SHOP, by: YURT, status: 'proposed' })
    expect(again.created).toBe(false)
    expect((await memories.list()).filter((memory) => memory.status === 'proposed')).toHaveLength(1)
  })
})

describe('the reader, in ask mode, on a real store', () => {
  const SETTINGS: WorkspaceSettings = { relay: true, relayHopCap: 6, interrupt: false, memoryMode: 'ask', autoMode: false, askConnectors: false, keepATodoList: false, layout: 'auto', tube: 'full', replySize: 'standard' }

  async function run(reply: string) {
    const { memories } = await withKept('Deploys go out on Fridays.', 'deploy-day')
    await memories.add({ text: 'The API is on port 3000.', scope: 'workspace', ...SHOP, by: YURT, status: 'kept' })
    const updates: CodexMissionUpdate[] = []
    const reader = createMemoryReader({
      memories,
      ledger: {
        getMission: async () =>
          ({
            metadata: { missionId: 'mission_9', runId: 'run_9', workspaceId: 'ws_shop' },
            phase: 'completed',
            events: [{ type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text: reply, final: true } }]
          }) as never
      },
      teammates: {
        list: async () => [{ teammateId: 'tm_yurt', name: 'Yurt' } as PublicTeammate],
        missionOwners: async (): Promise<Readonly<Record<string, string>>> => ({ mission_9: 'tm_yurt' }),
        readSettings: async () => SETTINGS
      },
      workspaceName: 'shop',
      notify: (update) => {
        updates.push(update)
      }
    })
    await reader.onRunEnded({ missionId: 'mission_9' })
    return { memories, updates }
  }

  it('proposes the rewrite and the forget, changes nothing kept, and names each for what it is', async () => {
    const { memories, updates } = await run(
      'Done.\n<locust-memory>\nremember as deploy-day :: Deploys go out on Thursdays.\nforget :: The API is on port 3000\n</locust-memory>'
    )
    expect((await memories.briefed('ws_shop')).map((memory) => memory.text)).toEqual(['Deploys go out on Fridays.', 'The API is on port 3000.'])
    expect(updates.find((update) => update.kind === 'memory-changed')).toEqual({
      kind: 'memory-changed',
      by: 'Yurt',
      kept: [],
      proposed: [],
      forgotten: [],
      proposedChanges: ['Deploys go out on Thursdays.'],
      proposedForgets: ['The API is on port 3000.']
    })
    // Proposed is not missed: nothing amber, nothing for the person to fix.
    expect(updates.filter((update) => update.kind === 'relay-notice')).toEqual([])
  })
})
