import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { MissionLedger, Workroom } from '@teammate/mission-store'
import type { RuntimeDiscovery, RuntimeProcessCompletion, RuntimeProcessRecordStream, RuntimeProcessRun } from '@teammate/runtime-adapters'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { createBriefSessions } from './brief-sessions.js'
import { createCodexMissionService } from './codex-mission.js'
import type { MissionPeerContext } from './workroom-briefing.js'

/**
 * A TEAMMATE'S KEY REACHES ONLY ITS RUNS (0.833, shared/teammate-keys.ts). A key the person kept for a teammate is in
 * that teammate's run's environment, under its name -- and in no other run: not another teammate's, not one with no
 * teammate, not a sealed one, not one on an agent that drops keys from its commands. And whatever the run says or does
 * with it, the value is never in what Locust records: its name in brackets is.
 */
const NOW = '2026-10-24T05:00:00.000Z'
const KEY = { name: 'STRIPE_KEY', value: 'locust-made-up-key-value-0042' }
const PEER: MissionPeerContext = { self: { teammateId: 'tm_mira', name: 'Mira', role: 'Payments' }, others: [] }
const OTHER: MissionPeerContext = { self: { teammateId: 'tm_ada', name: 'Ada', role: 'Research' }, others: [] }

const discovery = (id: 'claude' | 'codex'): RuntimeDiscovery => ({
  id,
  kind: 'agent-runtime',
  displayName: id === 'claude' ? 'Claude Code' : 'Codex CLI',
  optional: false,
  availability: 'available',
  readiness: 'ready',
  executable: {
    commandName: id,
    discoveredPath: process.platform === 'win32' ? `C:\\tools\\${id}.exe` : `/tools/${id}`,
    executablePath: process.platform === 'win32' ? `C:\\tools\\${id}.exe` : `/tools/${id}`,
    prefixArgs: [],
    kind: 'native',
    env: { EXISTING: 'kept' }
  },
  version: id === 'claude' ? { raw: '2.1.283 (Claude Code)', version: '2.1.283', major: 2, minor: 1, patch: 283 } : { raw: 'codex-cli 0.151.0', version: '0.151.0', major: 0, minor: 151, patch: 0 },
  supportedFeatures: ['non-interactive', 'jsonl-events', 'stdin-prompt', 'workspace-selection', 'read-only-sandbox'],
  requiredFeatures: ['non-interactive', 'jsonl-events', 'stdin-prompt', 'workspace-selection', 'read-only-sandbox'],
  diagnostics: []
})

describe('a teammate’s key', () => {
  let folder: string
  beforeEach(async () => { folder = await mkdtemp(join(tmpdir(), 'locust-keys-run-')) })
  afterEach(async () => { await rm(folder, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }) })

  const workroom: Workroom = {
    post: async () => { throw new Error('not used') },
    unread: async () => ({ messages: [], remaining: 0 }),
    markDelivered: async () => undefined,
    read: async () => ({ messages: [], deliveries: [], issues: [] }),
    flush: async () => undefined
  }

  /** A run that prints the key, as a careless command would, then says it did. */
  const said = (): RuntimeProcessRun => {
    const records = [
      { type: 'system', subtype: 'init', session_id: 'sess-1' },
      { type: 'assistant', message: { id: 'msg_1', content: [{ type: 'tool_use', id: 'tool_1', name: 'Bash', input: { command: 'echo $STRIPE_KEY' } }] } },
      { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 'tool_1', content: `${KEY.value}\n` }] } },
      { type: 'assistant', message: { id: 'msg_2', content: [{ type: 'text', text: `The key is ${KEY.value}, and it works.` }] } },
      { type: 'result', subtype: 'success', is_error: false, result: `The key is ${KEY.value}, and it works.`, session_id: 'sess-1' }
    ]
    return {
      records: {
        async *[Symbol.asyncIterator]() { for (const [index, record] of records.entries()) yield { sequence: index + 1, raw: JSON.stringify(record) } },
        drainAvailable: () => []
      } as RuntimeProcessRecordStream,
      completion: Promise.resolve({ exitCode: 0, signal: null, stderr: '', stderrTail: '', startedAt: NOW, endedAt: NOW } as unknown as RuntimeProcessCompletion)
    }
  }

  const setUp = (runtime: 'claude' | 'codex') => {
    const envs: Record<string, string>[] = []
    const kept: string[] = []
    let nextId = 0
    const missions = createCodexMissionService({
      workspacePath: folder,
      discover: async () => [discovery(runtime)],
      runner: {
        start: (spec: { readonly env?: Readonly<Record<string, string>> }) => {
          envs.push({ ...(spec.env ?? {}) })
          return said()
        }
      },
      ledger: {
        createMission: async () => undefined,
        appendEvents: async (_missionId: string, events: unknown) => { kept.push(JSON.stringify(events)) },
        appendHostFailure: async (_missionId: string, failure: unknown) => { kept.push(JSON.stringify(failure)) },
        appendPeerLinks: async () => undefined,
        appendEditCheck: async () => undefined, appendApproval: async () => undefined,
        getMission: async () => undefined,
        listMissions: async () => ({ missions: [], issues: [], unreadableCount: 0 }),
        flush: async () => undefined
      } as unknown as MissionLedger,
      workroom,
      briefSessions: createBriefSessions({ rootDirectory: folder }),
      createId: () => String(++nextId),
      now: () => new Date(NOW),
      // Only Mira has a key.
      teammateKeys: async (teammateId) => (teammateId === 'tm_mira' ? [KEY] : [])
    })
    return { missions, envs, kept }
  }
  const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 150))

  it('is in its own teammate’s run, under its name, beside what the launch had', async () => {
    const { missions, envs } = setUp('claude')
    const started = await missions.start('Charge a test card', 'claude', 'accept-edits', {}, () => undefined, undefined, PEER)
    expect(started.ok).toBe(true)
    expect(envs[0]?.STRIPE_KEY).toBe(KEY.value)
    expect(envs[0]?.EXISTING).toBe('kept')
  })

  it('is never in what Locust keeps of the run, whatever printed it', async () => {
    const { missions, kept } = setUp('claude')
    await missions.start('Charge a test card', 'claude', 'accept-edits', {}, () => undefined, undefined, PEER)
    await settle()
    const all = kept.join('\n')
    expect(all.length).toBeGreaterThan(0)
    expect(all).not.toContain(KEY.value)
    expect(all).toContain('[key STRIPE_KEY]')
  })

  it('is in no other run: another teammate’s, one with no teammate, a sealed one, or one on Codex', async () => {
    const claude = setUp('claude')
    await claude.missions.start('Look', 'claude', 'ask', {}, () => undefined, undefined, OTHER)
    await claude.missions.start('Look', 'claude', 'ask', {}, () => undefined)
    await claude.missions.start('Look', 'claude', 'ask', {}, () => undefined, undefined, PEER, undefined, undefined, undefined, undefined, { key: 'scout', sealed: true })
    expect(claude.envs).toHaveLength(3)
    for (const env of claude.envs) expect(env.STRIPE_KEY).toBeUndefined()
    const codex = setUp('codex')
    await codex.missions.start('Look', 'codex', 'ask', {}, () => undefined, undefined, PEER).catch(() => undefined)
    for (const env of codex.envs) expect(env.STRIPE_KEY).toBeUndefined()
  })
})
