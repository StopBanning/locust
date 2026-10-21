import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import type { MissionLedger } from '@teammate/mission-store'
import type { AppServerRunProcess, NormalizedRuntimeEvent, RuntimeDiscovery, RuntimeProcessCompletion, RuntimeProcessRunner } from '@teammate/runtime-adapters'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { CodexMissionUpdate, MissionApprovalRequest, MissionMode } from '../shared/ipc.js'
import { createApprovalChannel } from './approval-channel.js'
import { createCodexMissionService } from './codex-mission.js'

const NOW = '2026-10-20T12:00:00.000Z'
const WORKSPACE = join(tmpdir(), 'locust-v2-mission-fixture')
const GUARD = pathToFileURL(join(tmpdir(), 'locust-v2-guard-fixture')).href
const V1_GUARD = pathToFileURL(join(tmpdir(), 'locust-v1-guard-fixture.mjs')).href
const MODEL = 'opencode/zen-free'
const SESSION = 'session-v2-desktop'
const services = new Set<ReturnType<typeof createCodexMissionService>>()
const OPTIONS = [
  { optionId: 'once', kind: 'allow_once', name: 'Allow once' },
  { optionId: 'always', kind: 'allow_always', name: 'Always allow' },
  { optionId: 'deny', kind: 'reject_once', name: 'Deny' }
]
type Message = { id?: number; method?: string; params?: Record<string, unknown>; result?: Record<string, unknown> }
type Agent = { say(message: Message): void; answer(message: Message, result: Record<string, unknown>): void; sent: Message[]; process: AppServerRunProcess; kill: ReturnType<typeof vi.fn> }
const runtime = (major = 2): RuntimeDiscovery => ({
  id: 'opencode', kind: 'agent-runtime', displayName: 'OpenCode', optional: true,
  availability: 'available', readiness: 'ready', diagnostics: [], supportedFeatures: [], requiredFeatures: [],
  executable: { commandName: 'opencode', executablePath: '/fixture/opencode', discoveredPath: '/fixture/opencode', prefixArgs: ['shim'], kind: 'native', env: { FIXTURE: 'inherited' } },
  version: { raw: `opencode ${major}.0.0`, version: `${major}.0.0`, major, minor: 0, patch: 0 }
})
const receipt = (): RuntimeProcessCompletion => ({
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 0,
  cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false,
  inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: NOW, finishedAt: NOW
})
function ledger() {
  const missions = new Map<string, { metadata: Parameters<MissionLedger['createMission']>[0]; events: NormalizedRuntimeEvent[] }>()
  const appendEvents = vi.fn<MissionLedger['appendEvents']>(async (id, events) => { missions.get(id)?.events.push(...events) })
  const createMission = vi.fn<MissionLedger['createMission']>(async (metadata) => { missions.set(metadata.missionId, { metadata, events: [] }) })
  const value = {
    createMission, appendEvents,
    getMission: async (id: string) => {
      const mission = missions.get(id)
      return mission === undefined ? undefined : { ...mission, hostFailures: [], checkpoints: [], peerLinks: [], editChecks: [], approvals: [], phase: 'completed', lastUpdatedAt: NOW, ledgerSequence: mission.events.length + 1, issues: [] }
    },
    appendHostFailure: vi.fn(async () => undefined), appendPeerLinks: async () => undefined,
    appendEditCheck: async () => undefined, appendApproval: async () => undefined,
    listMissions: async () => ({ missions: [], issues: [], unreadableCount: 0 }), flush: async () => undefined
  } as unknown as MissionLedger
  return { value, missions, createMission, appendEvents }
}
function fakeAgent(script: (message: Message, agent: Agent) => void): Agent {
  let data: (chunk: string) => void = () => undefined
  let closed = false
  const exits: Array<() => void> = []
  const kill = vi.fn(() => {
    if (closed) return
    closed = true
    queueMicrotask(() => { for (const listener of exits) listener() })
  })
  const agent: Agent = {
    sent: [], kill,
    say: (message) => queueMicrotask(() => data(`${JSON.stringify({ jsonrpc: '2.0', ...message })}\n`)),
    answer: (message, result) => agent.say({ id: message.id!, result }),
    process: {
      write: (line) => { const message = JSON.parse(line) as Message; agent.sent.push(message); script(message, agent) },
      kill, onData: (listener) => { data = listener },
      onExit: (listener) => { exits.push(listener); if (closed) queueMicrotask(listener) },
      onStdoutClose: () => undefined
    }
  }
  return agent
}
const text = (agent: Agent, value: string): void => agent.say({ method: 'session/update', params: { sessionId: SESSION, update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: value } } } })
function scriptFor(turn?: (message: Message, agent: Agent) => void, history = false) {
  const current: Record<string, string> = { model: 'provider/previous', mode: 'build', effort: 'low' }
  const config = () => Object.entries(current).map(([id, currentValue]) => ({ id, type: 'select', currentValue, options: [] }))
  return (message: Message, agent: Agent): void => {
    if (message.method === 'initialize') agent.answer(message, { protocolVersion: 1, agentCapabilities: { loadSession: true } })
    else if (message.method === 'session/new' || message.method === 'session/load') {
      if (history && message.method === 'session/load') text(agent, 'Historical reply must not be duplicated.')
      agent.answer(message, { sessionId: SESSION, configOptions: config() })
    } else if (message.method === 'session/set_config_option') {
      current[String(message.params?.configId)] = String(message.params?.value)
      agent.answer(message, { configOptions: config() })
    } else if (message.method === 'session/prompt') {
      if (turn !== undefined) turn(message, agent)
      else { text(agent, 'Current answer.'); agent.answer(message, { stopReason: 'end_turn' }) }
    } else turn?.(message, agent)
  }
}
function fixture(options: Partial<Parameters<typeof createCodexMissionService>[0]> = {}, script = scriptFor()) {
  const stored = ledger()
  const agents: Agent[] = []
  const scheduled: Array<() => void> = []
  const updates: CodexMissionUpdate[] = []
  const cards: MissionApprovalRequest[] = []
  const approvals = createApprovalChannel({ emitApproval: (card) => cards.push(card), createId: () => `card-${cards.length + 1}` })
  const runnerStart = vi.fn<RuntimeProcessRunner['start']>(() => ({ records: { async *[Symbol.asyncIterator]() {}, drainAvailable: () => [] }, completion: Promise.resolve(receipt()) }))
  const acpSpawn = vi.fn((_file: string, _args: readonly string[], _env?: Readonly<Record<string, string>>, _cwd?: string) => {
    const agent = fakeAgent(script)
    agents.push(agent)
    return agent.process
  })
  const serveSpawn = vi.fn(() => fakeAgent(() => undefined).process)
  let id = 0
  const service = createCodexMissionService({
    workspacePath: WORKSPACE, discover: async () => [runtime()], ledger: stored.value,
    runner: { start: runnerStart }, acpSpawn, opencodeServeSpawn: serveSpawn,
    openCode2CommandGuard: async () => GUARD, openCodeCommandGuard: async () => V1_GUARD,
    autoModeAllowed: async () => true,
    approvals, createId: () => String(++id), now: () => new Date(NOW),
    schedule: (task) => scheduled.push(task), ...options
  })
  services.add(service)
  const dispatch = () => { while (scheduled.length > 0) scheduled.shift()!() }
  const start = async (mode: MissionMode = 'ask', followUpOf?: string, route: { model?: string; effort?: string } = { model: MODEL, effort: 'high' }) => {
    const result = await service.start('Inspect the project.', 'opencode', mode, route, (update) => updates.push(update), undefined, undefined, followUpOf)
    dispatch()
    return result
  }
  const settled = async (): Promise<void> => { await vi.waitFor(() => expect(service.liveMissionIds()).toEqual([])) }
  return { service, start, dispatch, settled, stored, agents, acpSpawn, serveSpawn, runnerStart, updates, cards, approvals }
}
async function flush(): Promise<void> { for (let i = 0; i < 80; i += 1) await Promise.resolve() }

afterEach(async () => {
  await Promise.all([...services].map((service) => service.dispose()))
  services.clear()
  vi.clearAllTimers()
  vi.useRealTimers()
})

describe('OpenCode 2 runs over ACP with its permissions', () => {
  it('starts ACP instead of serve or run, passes the native guard and sets mode and model before its prompt', async () => {
    const f = fixture()
    expect(await f.start()).toMatchObject({ ok: true })
    expect(f.acpSpawn).toHaveBeenCalledOnce()
    await f.settled()
    expect(f.acpSpawn.mock.calls[0]?.slice(0, 2)).toEqual(['/fixture/opencode', ['shim', 'acp']])
    expect(f.acpSpawn.mock.calls[0]?.[3]).toBe(WORKSPACE)
    const env = f.acpSpawn.mock.calls[0]?.[2]
    expect(env?.FIXTURE).toBe('inherited')
    const config = JSON.parse(env!.OPENCODE_CONFIG_CONTENT!) as Record<string, unknown>
    expect(config).toMatchObject({ default_agent: 'locust', plugins: [GUARD], agents: { locust: { mode: 'primary', permissions: expect.any(Array) } } })
    expect(config.permission).toBeUndefined()
    expect(config.experimental).toBeUndefined()
    const sent = f.agents[0]!.sent
    const promptAt = sent.findIndex((message) => message.method === 'session/prompt')
    expect(sent.slice(0, promptAt).filter((message) => message.method === 'session/set_config_option').map((message) => message.params)).toEqual([
      { sessionId: SESSION, configId: 'mode', value: 'locust' }, { sessionId: SESSION, configId: 'model', value: MODEL }, { sessionId: SESSION, configId: 'effort', value: 'high' }
    ])
    expect(f.serveSpawn).not.toHaveBeenCalled()
    expect(f.runnerStart).not.toHaveBeenCalled()
    expect(f.stored.createMission.mock.calls[0]?.[0]).toMatchObject({ runtime: 'opencode', cliVersion: '2.0.0', model: MODEL })
    expect(JSON.stringify(f.updates)).toContain('Current answer.')
    await f.service.dispose()
  })

  it.each(['approve-once', 'deny'] as const)('answers an Approve each card with the person\'s %s decision', async (decision) => {
    let prompt: Message | undefined
    const f = fixture({}, scriptFor((message, agent) => {
      if (message.method === 'session/prompt') {
        prompt = message
        agent.say({ id: 90, method: 'session/request_permission', params: { sessionId: SESSION, toolCall: { toolCallId: 'cmd-1', kind: 'execute', title: 'Check files', rawInput: { command: 'echo fixture' } }, options: OPTIONS } })
      } else if (message.id === 90 && message.result !== undefined) agent.answer(prompt!, { stopReason: 'end_turn' })
    }))
    expect(await f.start('approve-each')).toMatchObject({ ok: true })
    await vi.waitFor(() => expect(f.cards).toHaveLength(1))
    expect(f.cards[0]).toMatchObject({ runtime: 'opencode', kind: 'command', detail: 'echo fixture' })
    expect(f.agents[0]!.sent.some((message) => message.id === 90)).toBe(false)
    f.approvals.decide({ approvalId: f.cards[0]!.approvalId, decision })
    await f.settled()
    expect(f.agents[0]!.sent.find((message) => message.id === 90)?.result).toEqual({ outcome: { outcome: 'selected', optionId: decision === 'approve-once' ? 'once' : 'deny' } })
    const config = JSON.parse(f.acpSpawn.mock.calls[0]![2]!.OPENCODE_CONFIG_CONTENT!) as { agents: { locust: { permissions: unknown[] } } }
    expect(config.agents.locust.permissions).toContainEqual({ action: '*', resource: '*', effect: 'ask' })
    await f.service.dispose()
  })

  it.each([['auto', 'once'], ['ask', 'deny']] as const)('uses %s permission policy without raising a card', async (mode, answer) => {
    let prompt: Message | undefined
    const f = fixture({}, scriptFor((message, agent) => {
      if (message.method === 'session/prompt') {
        prompt = message
        agent.say({ id: 91, method: 'session/request_permission', params: { sessionId: SESSION, toolCall: { toolCallId: 'unexpected-tool', kind: 'execute', rawInput: { command: 'echo fixture' } }, options: OPTIONS } })
      } else if (message.id === 91 && message.result !== undefined) agent.answer(prompt!, { stopReason: 'end_turn' })
    }))
    expect(await f.start(mode)).toMatchObject({ ok: true })
    await f.settled()
    expect(f.cards).toEqual([])
    expect(f.agents[0]!.sent.find((message) => message.id === 91)?.result).toEqual({ outcome: { outcome: 'selected', optionId: answer } })
    await f.service.dispose()
  })

  it('loads a follow-up ACP session while keeping its replayed history out of the new ledger', async () => {
    const f = fixture({}, scriptFor(undefined, true))
    const first = await f.start()
    expect(first.ok).toBe(true)
    if (!first.ok) return
    await f.settled()
    const second = await f.start('ask', first.data.missionId)
    expect(second.ok).toBe(true)
    if (!second.ok) return
    await f.settled()
    expect(f.agents[1]!.sent.some((message) => message.method === 'session/new')).toBe(false)
    expect(f.agents[1]!.sent.find((message) => message.method === 'session/load')?.params).toEqual({ sessionId: SESSION, cwd: WORKSPACE, mcpServers: [] })
    const recorded = f.stored.missions.get(second.data.missionId)?.events ?? []
    expect(JSON.stringify(recorded)).toContain('Current answer.')
    expect(JSON.stringify(recorded)).not.toContain('Historical reply')
    await f.service.dispose()
  })

  it.each(['version-change', 'imported-permissions'] as const)('briefs a new session for %s instead of loading a foreign permission history', async (reason) => {
    const f = fixture()
    const first = await f.start()
    expect(first.ok).toBe(true)
    if (!first.ok) return
    await f.settled()
    const prior = f.stored.missions.get(first.data.missionId)!
    prior.metadata = { ...prior.metadata, ...(reason === 'version-change' ? { cliVersion: '1.19.3' } : { command: undefined }) }
    const second = await f.start('ask', first.data.missionId)
    expect(second.ok).toBe(true)
    if (!second.ok) return
    await f.settled()
    const sent = f.agents[1]!.sent
    expect(sent.some((message) => message.method === 'session/load')).toBe(false)
    expect(sent.some((message) => message.method === 'session/new')).toBe(true)
    expect(JSON.stringify(sent.find((message) => message.method === 'session/prompt')?.params)).toContain('Current answer.')
    await f.service.dispose()
  })

  it('asks a side question in a freshly briefed ACP session without appending to the parent', async () => {
    const f = fixture()
    const first = await f.start()
    expect(first.ok).toBe(true)
    if (!first.ok) return
    await f.settled()
    const parentCount = f.stored.missions.get(first.data.missionId)!.events.length
    const side = await f.service.start('What did that mean?', 'opencode', 'ask', { model: MODEL }, (update) => f.updates.push(update), undefined, undefined, undefined, undefined, undefined, undefined, undefined, { of: first.data.missionId, question: 0 })
    expect(side.ok).toBe(true)
    f.dispatch()
    await f.settled()
    expect(f.agents[1]!.sent.some((message) => message.method === 'session/load')).toBe(false)
    expect(f.agents[1]!.sent.some((message) => message.method === 'session/new')).toBe(true)
    expect(JSON.stringify(f.agents[1]!.sent.find((message) => message.method === 'session/prompt')?.params)).toContain('Current answer.')
    expect(f.stored.missions.get(first.data.missionId)!.events).toHaveLength(parentCount)
    await f.service.dispose()
  })

  it('refuses a V2 cloud start before admitting a local ACP or legacy cloud process', async () => {
    const f = fixture()
    const args: Parameters<typeof f.service.start> = ['Inspect the project.', 'opencode', 'ask', { model: MODEL }, () => undefined]
    args[15] = {}
    expect(await f.service.start(...args)).toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED', message: expect.stringContaining('Turn Cloud off') } })
    expect(f.stored.createMission).not.toHaveBeenCalled()
    expect(f.acpSpawn).not.toHaveBeenCalled()
    expect(f.serveSpawn).not.toHaveBeenCalled()
    await f.service.dispose()
  })

  it('cancels a prompt over ACP and bounds an agent that never acknowledges cancellation', async () => {
    vi.useFakeTimers()
    const f = fixture({}, scriptFor(() => undefined))
    const started = await f.start()
    expect(started.ok).toBe(true)
    if (!started.ok) return
    await flush()
    expect(f.agents).toHaveLength(1)
    expect(f.agents[0]!.sent.some((message) => message.method === 'session/prompt')).toBe(true)
    expect(f.service.cancel(started.data.runId)).toMatchObject({ ok: true })
    await flush()
    expect(f.agents[0]!.sent.find((message) => message.method === 'session/cancel')?.params).toEqual({ sessionId: SESSION })
    expect(f.agents[0]!.kill).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(3_000)
    await flush()
    expect(f.agents[0]!.kill).toHaveBeenCalledOnce()
    expect(f.service.liveMissionIds()).toEqual([])
    expect(f.stored.appendEvents.mock.calls.flatMap(([, events]) => events).some((event) => event.type === 'run.cancelled')).toBe(true)
    await f.service.dispose()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('refuses V2 without an ACP spawn before creating a mission or falling back to V1', async () => {
    const f = fixture({ acpSpawn: undefined })
    expect(await f.start()).toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED', message: expect.stringContaining('ACP') } })
    expect(f.stored.createMission).not.toHaveBeenCalled()
    expect(f.acpSpawn).not.toHaveBeenCalled()
    expect(f.serveSpawn).not.toHaveBeenCalled()
    expect(f.runnerStart).not.toHaveBeenCalled()
    await f.service.dispose()
  })

  it('refuses V2 without its command guard instead of starting an unguarded process', async () => {
    const f = fixture({ openCode2CommandGuard: async () => undefined })
    expect(await f.start()).toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED', message: expect.stringContaining('command guard') } })
    expect(f.stored.createMission).not.toHaveBeenCalled()
    expect(f.acpSpawn).not.toHaveBeenCalled()
    expect(f.serveSpawn).not.toHaveBeenCalled()
    expect(f.runnerStart).not.toHaveBeenCalled()
    await f.service.dispose()
  })

  it('retains the V1 run route and its original hook when no serve bridge is available', async () => {
    const f = fixture({ discover: async () => [runtime(1)], opencodeServeSpawn: undefined })
    // V1 rejects arbitrary effort values at command construction, as before.
    const started = await f.start('ask', undefined, { model: MODEL })
    expect(started.ok).toBe(true)
    expect(f.runnerStart).toHaveBeenCalledOnce()
    const spec = f.runnerStart.mock.calls[0]![0]
    expect(spec.args).toContain('run')
    expect(spec.args).not.toContain('acp')
    const config = JSON.parse(spec.env!.OPENCODE_CONFIG_CONTENT!) as { plugin: string[] }
    expect(config.plugin).toContain(V1_GUARD)
    expect(f.acpSpawn).not.toHaveBeenCalled()
    expect(f.serveSpawn).not.toHaveBeenCalled()
    await f.service.dispose()
  })

  it('retains the V1 serve route and its original hook when the serve bridge is available', async () => {
    const f = fixture({ discover: async () => [runtime(1)] })
    const started = await f.start('ask', undefined, { model: MODEL })
    expect(started.ok).toBe(true)
    if (!started.ok) return
    expect(f.serveSpawn).toHaveBeenCalledOnce()
    const called = f.serveSpawn.mock.calls[0] as unknown as [string, readonly string[], Readonly<Record<string, string>>]
    expect(called[1]).toContain('serve')
    expect(JSON.parse(called[2].OPENCODE_CONFIG_CONTENT!).plugin).toContain(V1_GUARD)
    expect(f.acpSpawn).not.toHaveBeenCalled()
    expect(f.runnerStart).not.toHaveBeenCalled()
    expect(f.service.cancel(started.data.runId)).toMatchObject({ ok: true })
    await f.service.dispose()
  })
})
