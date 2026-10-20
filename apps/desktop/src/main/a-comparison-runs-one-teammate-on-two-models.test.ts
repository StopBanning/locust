import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { MissionLedger, Workroom } from '@teammate/mission-store'
import type { RuntimeDiscovery, RuntimeProcessCompletion, RuntimeProcessRecordStream, RuntimeProcessRun } from '@teammate/runtime-adapters'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { compareSlotKey } from '../shared/compare.js'
import { createBriefSessions } from './brief-sessions.js'
import { createCodexMissionService } from './codex-mission.js'
import type { MissionPeerContext } from './workroom-briefing.js'
import type { MemoryBriefing } from './peer-exchange.js'
import { columnInstructions } from './compare-instructions.js'

/**
 * ONE TEAMMATE, TWO MODELS AT ONCE -- ONLY IN A COMPARISON (0.441).
 *
 * The host runs one mission per teammate at a time (RUN_ALREADY_ACTIVE),
 * and that rule stands. A comparison's column carries its own run slot, so
 * Wren can answer on Fable and on Astra at once -- and a second ordinary run
 * of Wren's is still refused while one is going.
 */
const NOW = '2026-09-28T15:00:00.000Z'
const WREN: MissionPeerContext = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [] }
const slotOf = (slot: 'a' | 'b', teammateId: string | undefined = 'tm_wren'): { key: string } => ({ key: compareSlotKey(teammateId, 'cmp_1', slot) })

describe('a comparison', () => {
  let folder: string
  beforeEach(async () => {
    folder = await mkdtemp(join(tmpdir(), 'locust-compare-slots-'))
  })
  afterEach(async () => {
    await rm(folder, { recursive: true, force: true })
  })

  const runtime: RuntimeDiscovery = {
    id: 'codex',
    kind: 'agent-runtime',
    displayName: 'Codex CLI',
    optional: false,
    availability: 'available',
    readiness: 'ready',
    executable: {
      commandName: 'codex',
      discoveredPath: process.platform === 'win32' ? 'C:\\tools\\codex.exe' : '/tools/codex',
      executablePath: process.platform === 'win32' ? 'C:\\tools\\codex.exe' : '/tools/codex',
      prefixArgs: [],
      kind: 'native'
    },
    version: { raw: 'codex-cli 0.151.0', version: '0.151.0', major: 0, minor: 151, patch: 0 },
    supportedFeatures: ['non-interactive', 'jsonl-events', 'stdin-prompt', 'workspace-selection', 'read-only-sandbox'],
    requiredFeatures: ['non-interactive', 'jsonl-events', 'stdin-prompt', 'workspace-selection', 'read-only-sandbox'],
    diagnostics: []
  }
  /** A run that has started and does not end while the test looks. */
  const open = (): RuntimeProcessRun => ({
    records: {
      async *[Symbol.asyncIterator]() {
        yield { sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread-1' }) }
      },
      drainAvailable: () => []
    } as RuntimeProcessRecordStream,
    completion: new Promise<RuntimeProcessCompletion>(() => undefined)
  })
  const workroom: Workroom = {
    post: async () => {
      throw new Error('not used')
    },
    unread: async () => ({ messages: [], remaining: 0 }),
    markDelivered: async () => undefined,
    read: async () => ({ messages: [], deliveries: [], issues: [] }),
    flush: async () => undefined
  }
  const service = (memory?: MemoryBriefing) => {
    let nextId = 0
    return createCodexMissionService({
      workspacePath: process.platform === 'win32' ? 'C:\\safe-workspace' : '/safe-workspace',
      discover: async () => [runtime],
      runner: { start: () => open() },
      ledger: {
        createMission: async () => undefined,
        appendEvents: async () => undefined,
        appendHostFailure: async () => undefined,
        appendPeerLinks: async () => undefined,
        appendEditCheck: async () => undefined, appendApproval: async () => undefined,
        getMission: async () => undefined,
        listMissions: async () => ({ missions: [], issues: [], unreadableCount: 0 }),
        flush: async () => undefined
      } as unknown as MissionLedger,
      workroom,
      ...(memory === undefined ? {} : { memory }),
      briefSessions: createBriefSessions({ rootDirectory: folder }),
      createId: () => String(++nextId),
      now: () => new Date(NOW),
      schedule: () => undefined
    })
  }

  it('runs one teammate on two models at once, a column each', async () => {
    const missions = service()
    const first = await missions.start('Add a discount code field.', 'codex', 'ask', { model: 'gpt-6-luna' }, () => undefined, undefined, WREN, undefined, undefined, undefined, undefined, slotOf('a'))
    const second = await missions.start('Add a discount code field.', 'codex', 'ask', { model: 'gpt-6-astra' }, () => undefined, undefined, WREN, undefined, undefined, undefined, undefined, slotOf('b'))
    expect(first.ok).toBe(true)
    expect(second.ok).toBe(true)
  })

  it('passes each column’s own instructions to its brief, including empty, without changing an ordinary turn', async () => {
    const seen: (string | undefined)[] = []
    const missions = service({ section: async (_peer, conversation) => {
      const instructions = columnInstructions('Saved standing words.', conversation?.instructions)
      seen.push(instructions)
      return instructions
    } })
    for (const [index, instructions] of ['Check the result.', '', undefined].entries()) {
      const answer = await missions.start('One ask.', 'codex', 'ask', {}, () => undefined, undefined, WREN, undefined, undefined, undefined, undefined,
        { key: `compare:trial:${index}`, ...(instructions === undefined ? {} : { instructions }) })
      expect(answer.ok).toBe(true)
    }
    const normal = await missions.start('An ordinary turn.', 'codex', 'ask', {}, () => undefined, undefined, WREN)
    expect(normal.ok).toBe(true)
    expect(seen).toEqual(['Check the result.', '', 'Saved standing words.', 'Saved standing words.'])
  })

  it('starts a second conversation with the same teammate while one is going (0.551)', async () => {
    const missions = service()
    expect((await missions.start('One', 'codex', 'ask', {}, () => undefined, undefined, WREN)).ok).toBe(true)
    expect((await missions.start('Two', 'codex', 'ask', {}, () => undefined, undefined, WREN)).ok).toBe(true)
  })

  it('runs on two models with no teammate at all -- none is needed to compare', async () => {
    const missions = service()
    // Two conversations of nobody's run side by side too (0.551).
    expect((await missions.start('One', 'codex', 'ask', {}, () => undefined)).ok).toBe(true)
    expect((await missions.start('Two', 'codex', 'ask', {}, () => undefined)).ok).toBe(true)
    const a = await missions.start('Go', 'codex', 'ask', {}, () => undefined, undefined, undefined, undefined, undefined, undefined, undefined, slotOf('a', undefined))
    const b = await missions.start('Go', 'codex', 'ask', {}, () => undefined, undefined, undefined, undefined, undefined, undefined, undefined, slotOf('b', undefined))
    expect(a.ok).toBe(true)
    expect(b.ok).toBe(true)
  })

  it('refuses a second run in the same column while its first is going', async () => {
    const missions = service()
    expect((await missions.start('One', 'codex', 'ask', {}, () => undefined, undefined, WREN, undefined, undefined, undefined, undefined, slotOf('a'))).ok).toBe(true)
    const again = await missions.start('Two', 'codex', 'ask', {}, () => undefined, undefined, WREN, undefined, undefined, undefined, undefined, slotOf('a'))
    expect(again.ok).toBe(false)
  })
})
