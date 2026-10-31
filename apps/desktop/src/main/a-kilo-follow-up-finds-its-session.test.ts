import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { MissionLedger } from '@teammate/mission-store'
import type { AppServerRunProcess, NormalizedRuntimeEvent, RuntimeDiscovery, RuntimeProcessCompletion, RuntimeProcessRunner } from '@teammate/runtime-adapters'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { CodexMissionUpdate, MissionMode } from '../shared/ipc.js'
import { createCodexMissionService } from './codex-mission.js'

/**
 * A KILO FOLLOW-UP FINDS ITS SESSION (0.845).
 *
 * Kilo keeps its sessions in its own data folder, and every Kilo run was given a new one in the temp folder. So a
 * follow-up asked Kilo to load a session from a folder that never held it, and Kilo answered "Internal error:
 * OpenCode service failure": every second message to a Kilo teammate failed (found filming the launch clip,
 * 2026-10-31). The turns of a conversation now share one folder, named for its first turn; a conversation whose
 * folder is not there starts a new session, told what was said before.
 */
const NOW = '2026-10-31T22:00:00.000Z'
const WORKSPACE = join(tmpdir(), 'locust-kilo-follow-up-fixture')
const HOMES = join(tmpdir(), 'locust-kilo-homes-fixture')
const MODEL = 'kilo/kilo-auto/free'
const SESSION = 'ses_kilo_fixture'
const services = new Set<ReturnType<typeof createCodexMissionService>>()
type Message = { id?: number; method?: string; params?: Record<string, unknown>; result?: Record<string, unknown> }
type Agent = { say(message: Message): void; answer(message: Message, result: Record<string, unknown>): void; sent: Message[]; process: AppServerRunProcess }
const kilo: RuntimeDiscovery = {
  id: 'kilo', kind: 'agent-runtime', displayName: 'Kilo', optional: true,
  availability: 'available', readiness: 'ready', diagnostics: [], supportedFeatures: [], requiredFeatures: [],
  executable: { commandName: 'kilo', executablePath: '/fixture/kilo', discoveredPath: '/fixture/kilo', prefixArgs: [], kind: 'native' },
  version: { raw: 'kilo 7.8.18', version: '7.8.18', major: 7, minor: 8, patch: 18 }
}
const receipt = (): RuntimeProcessCompletion => ({
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 0,
  cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false,
  inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: NOW, finishedAt: NOW
})
function ledger() {
  const missions = new Map<string, { metadata: Parameters<MissionLedger['createMission']>[0]; events: NormalizedRuntimeEvent[] }>()
  const value = {
    createMission: vi.fn<MissionLedger['createMission']>(async (metadata) => { missions.set(metadata.missionId, { metadata, events: [] }) }),
    appendEvents: vi.fn<MissionLedger['appendEvents']>(async (id, events) => { missions.get(id)?.events.push(...events) }),
    getMission: async (id: string) => {
      const mission = missions.get(id)
      return mission === undefined ? undefined : { ...mission, hostFailures: [], checkpoints: [], peerLinks: [], editChecks: [], approvals: [], phase: 'completed', lastUpdatedAt: NOW, ledgerSequence: mission.events.length + 1, issues: [] }
    },
    appendHostFailure: vi.fn(async () => undefined), appendPeerLinks: async () => undefined,
    appendEditCheck: async () => undefined, appendApproval: async () => undefined,
    listMissions: async () => ({ missions: [], issues: [], unreadableCount: 0 }), flush: async () => undefined
  } as unknown as MissionLedger
  return { value, missions }
}
/** Kilo over ACP, as far as a turn needs: it can load a session, takes its model and mode, and answers. */
function fakeKilo(): Agent {
  let data: (chunk: string) => void = () => undefined
  let closed = false
  const exits: Array<() => void> = []
  const current: Record<string, string> = { model: 'kilo/google/gemini-3-pro-image', mode: 'code' }
  const config = () => Object.entries(current).map(([id, currentValue]) => ({ id, type: 'select', currentValue, options: [] }))
  const agent: Agent = {
    sent: [],
    say: (message) => queueMicrotask(() => data(`${JSON.stringify({ jsonrpc: '2.0', ...message })}\n`)),
    answer: (message, result) => agent.say({ id: message.id!, result }),
    process: {
      write: (line) => {
        const message = JSON.parse(line) as Message
        agent.sent.push(message)
        if (message.method === 'initialize') agent.answer(message, { protocolVersion: 1, agentCapabilities: { loadSession: true } })
        else if (message.method === 'session/new' || message.method === 'session/load') agent.answer(message, { sessionId: SESSION, configOptions: config() })
        else if (message.method === 'session/set_config_option') {
          current[String(message.params?.configId)] = String(message.params?.value)
          agent.answer(message, { configOptions: config() })
        } else if (message.method === 'session/prompt') {
          agent.say({ method: 'session/update', params: { sessionId: SESSION, update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'Kilo answered.' } } } })
          agent.answer(message, { stopReason: 'end_turn' })
        }
      },
      kill: () => {
        if (closed) return
        closed = true
        queueMicrotask(() => { for (const listener of exits) listener() })
      },
      onData: (listener) => { data = listener },
      onExit: (listener) => { exits.push(listener); if (closed) queueMicrotask(listener) },
      onStdoutClose: () => undefined
    }
  }
  return agent
}
function fixture() {
  const stored = ledger()
  const agents: Agent[] = []
  const scheduled: Array<() => void> = []
  const updates: CodexMissionUpdate[] = []
  const acpSpawn = vi.fn((_file: string, _args: readonly string[], _env?: Readonly<Record<string, string>>, _cwd?: string) => {
    const agent = fakeKilo()
    agents.push(agent)
    return agent.process
  })
  // A folder per conversation, as index.ts keeps them in the profile; `made` the first time it is asked for.
  const there = new Set<string>()
  const kiloHome = vi.fn(async (conversation: string) => {
    const made = !there.has(conversation)
    there.add(conversation)
    return { folder: join(HOMES, conversation), made }
  })
  const runnerStart = vi.fn<RuntimeProcessRunner['start']>(() => ({ records: { async *[Symbol.asyncIterator]() {}, drainAvailable: () => [] }, completion: Promise.resolve(receipt()) }))
  let id = 0
  const open = () => {
    const opened = createCodexMissionService({
      workspacePath: WORKSPACE, discover: async () => [kilo], ledger: stored.value,
      runner: { start: runnerStart }, acpSpawn, kiloHome,
      createId: () => String(++id), now: () => new Date(NOW), schedule: (task) => scheduled.push(task)
    })
    services.add(opened)
    return opened
  }
  let service = open()
  // Locust closed and opened again: the same record and folders, nothing remembered in between
  const restart = async () => {
    await service.dispose()
    service = open()
  }
  const start = async (followUpOf?: string, mode: MissionMode = 'ask') => {
    const result = await service.start('What should we build next?', 'kilo', mode, { model: MODEL }, (update) => updates.push(update), undefined, undefined, followUpOf)
    while (scheduled.length > 0) scheduled.shift()!()
    if (!result.ok) throw new Error(JSON.stringify(result.error))
    await vi.waitFor(() => expect(service.liveMissionIds()).toEqual([]))
    return result.data.missionId
  }
  const homeOf = (run: number) => acpSpawn.mock.calls[run]?.[2]?.XDG_DATA_HOME
  const sent = (run: number, method: string) => agents[run]?.sent.find((message) => message.method === method)
  return { start, restart, stored, agents, acpSpawn, kiloHome, homeOf, sent, forget: (conversation: string) => there.delete(conversation) }
}

afterEach(async () => {
  await Promise.all([...services].map((service) => service.dispose()))
  services.clear()
})

describe('a Kilo follow-up finds its session', () => {
  it('runs every turn of a conversation in its first turn\'s Kilo folder, and the follow-ups load its session', async () => {
    const f = fixture()
    const first = await f.start()
    const second = await f.start(first)
    await f.start(second)
    expect(f.kiloHome.mock.calls.map(([conversation]) => conversation)).toEqual([first, first, first])
    expect([f.homeOf(0), f.homeOf(1), f.homeOf(2)]).toEqual([join(HOMES, first), join(HOMES, first), join(HOMES, first)])
    // Kilo's own config and data are the conversation's folder, never the person's
    expect(f.acpSpawn.mock.calls[1]?.[2]).toMatchObject({ KILO_CONFIG_DIR: join(HOMES, first), XDG_CONFIG_HOME: join(HOMES, first) })
    expect(f.sent(0, 'session/new')).toBeDefined()
    expect(f.sent(1, 'session/new')).toBeUndefined()
    expect(f.sent(1, 'session/load')?.params).toMatchObject({ sessionId: SESSION, cwd: WORKSPACE })
    expect(f.sent(2, 'session/load')?.params).toMatchObject({ sessionId: SESSION })
    expect(f.stored.missions.get(second)?.metadata.continuesFrom).toMatchObject({ missionId: first, runtimeThreadId: SESSION })
  })

  it('finds the first turn after Locust was reopened, from the record', async () => {
    const f = fixture()
    const first = await f.start()
    const second = await f.start(first)
    const third = await f.start(second)
    await f.restart()
    // three turns back, through the record alone
    await f.start(third)
    expect(f.kiloHome.mock.calls.at(-1)?.[0]).toBe(first)
    expect(f.homeOf(3)).toBe(join(HOMES, first))
    expect(f.sent(3, 'session/load')?.params).toMatchObject({ sessionId: SESSION })
  })

  it('gives two conversations two folders', async () => {
    const f = fixture()
    const one = await f.start()
    const two = await f.start()
    expect(one).not.toBe(two)
    expect([f.homeOf(0), f.homeOf(1)]).toEqual([join(HOMES, one), join(HOMES, two)])
  })

  it('starts a conversation whose folder is gone in a new session, told what was said before', async () => {
    const f = fixture()
    const first = await f.start()
    // as on a restored profile, or a conversation begun before 0.845: its Kilo never held this session
    f.forget(first)
    const second = await f.start(first)
    expect(f.homeOf(1)).toBe(join(HOMES, first))
    expect(f.sent(1, 'session/load')).toBeUndefined()
    expect(f.sent(1, 'session/new')).toBeDefined()
    expect(JSON.stringify(f.sent(1, 'session/prompt')?.params)).toContain('Kilo answered.')
    // the record says this followed that, without claiming a resume that did not happen
    const continued = f.stored.missions.get(second)?.metadata.continuesFrom
    expect(continued).toMatchObject({ missionId: first })
    expect(continued).not.toHaveProperty('runtimeThreadId')
  })
})
