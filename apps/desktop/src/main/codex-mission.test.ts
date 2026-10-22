import { workspaceIdFor } from './workspace.js'
import type {
  GitReadsSpec,
  RuntimeCommandSpec,
  RuntimeDiscovery,
  RuntimeJsonlRecord,
  RuntimeProcessRecordStream,
  RuntimeProcessCompletion,
  RuntimeProcessRun,
  RuntimeProcessRunner,
  NormalizedRuntimeEvent
} from '@teammate/runtime-adapters'
import type { MissionLedger, MissionPeerLink, Workroom, WorkroomMessage } from '@teammate/mission-store'
import { describe, expect, it, vi } from 'vitest'

import { openCodeReadOnlySection, planSection } from './workroom-briefing.js'
import type { CodexMissionUpdate, MissionApprovalRequest } from '../shared/ipc.js'
import { MAX_LIVE_MISSIONS } from '../shared/live-missions.js'
import { createRecentEdits } from './recent-edits.js'
import { createCodexMissionService } from './codex-mission.js'
import { FREE_ONLY_REFUSAL } from './free-routes.js'
import { createApprovalChannel } from './approval-channel.js'
import { mkdtemp, readdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { prepareRunSkills } from './run-skills.js'
import type { MissionPeerContext } from './workroom-briefing.js'

const NOW = '2026-08-31T15:00:00.000Z'
const WORKSPACE = process.platform === 'win32' ? 'C:\\safe-workspace' : '/safe-workspace'

function codexRuntime(readiness: RuntimeDiscovery['readiness'] = 'ready'): RuntimeDiscovery {
  return {
    id: 'codex',
    kind: 'agent-runtime',
    displayName: 'Codex CLI',
    optional: false,
    availability: 'available',
    readiness,
    executable: {
      commandName: 'codex',
      discoveredPath: process.platform === 'win32' ? 'C:\\tools\\codex.exe' : '/tools/codex',
      executablePath: process.platform === 'win32' ? 'C:\\tools\\codex.exe' : '/tools/codex',
      prefixArgs: [],
      kind: 'native'
    },
    version: {
      raw: 'codex-cli 0.151.0-alpha.7.2',
      version: '0.151.0-alpha.7.2',
      major: 0,
      minor: 151,
      patch: 0,
      prerelease: 'alpha.7.2'
    },
    supportedFeatures: [
      'non-interactive',
      'jsonl-events',
      'stdin-prompt',
      'workspace-selection',
      'read-only-sandbox'
    ],
    requiredFeatures: [
      'non-interactive',
      'jsonl-events',
      'stdin-prompt',
      'workspace-selection',
      'read-only-sandbox'
    ],
    diagnostics: []
  }
}

function completion(overrides: Partial<RuntimeProcessCompletion> = {}): RuntimeProcessCompletion {
  return {
    exitCode: 0,
    signal: null,
    stderr: '',
    stderrTruncated: false,
    recordCount: 4,
    cancelled: false,
    forcedTerminationAttempted: false,
    terminationUnconfirmed: false,
    inputDeliveryFailed: false,
    outputLimitExceeded: false,
    oversizedRecordsDropped: 0,
    startedAt: NOW,
    finishedAt: NOW,
    ...overrides
  }
}

function records(values: readonly Record<string, unknown>[]): RuntimeProcessRecordStream {
  return {
    async *[Symbol.asyncIterator]() {
      for (const [index, value] of values.entries()) {
        yield { sequence: index + 1, raw: JSON.stringify(value) }
      }
    },
    // Nothing is buffered ahead in this fake, so the consumer's batching path
    // degrades to one record per append -- which is what the ordering tests
    // below want to observe.
    drainAvailable: () => []
  }
}

/**
 * A stream that hands over the first record and reports the rest as already
 * buffered, the way a real burst arrives while the consumer is awaiting a
 * durable write.
 */
function burst(values: readonly Record<string, unknown>[]): RuntimeProcessRecordStream {
  const all = values.map((value, index) => ({ sequence: index + 1, raw: JSON.stringify(value) }))
  const buffered = all.slice(1)
  return {
    async *[Symbol.asyncIterator]() {
      yield all[0]!
    },
    drainAvailable: () => buffered.splice(0)
  }
}

function fakeLedger(overrides: Partial<MissionLedger> = {}): MissionLedger {
  return {
    createMission: async () => undefined,
    appendEvents: async () => undefined,
    appendHostFailure: async () => undefined,
    createCheckpoint: async () => { throw new Error('not used in this test') },
    appendPeerLinks: async () => undefined, appendEditCheck: async () => undefined, appendApproval: async () => undefined,
    deleteMission: async () => true,
    listTrashedMissions: async () => [],
    restoreMission: async () => true,
    emptyTrash: async () => 0,
    storageReport: async () => ({ missionCount: 0, byteTotal: 0, unreadableCount: 0 }),
    pruneMissions: async () => ({ deleted: [], failed: [], unreadable: [], keptForContinuity: [], keptAsRunning: [] }),
    getMission: async () => undefined,
    listMissions: async () => ({ missions: [], issues: [], unreadableCount: 0 }),
    flush: async () => undefined,
    ...overrides
  }
}

describe('a Cursor run whose folder .cursorignore hides', () => {
  /*
   * THE REGRESSION FROM 0.87.0, which broke every Cursor run in a hidden
   * folder: the notice was APPENDED to the ledger, `appendEvents` requires
   * strictly contiguous event sequences, and the adapter numbers its own from
   * zero -- so the notice took a number the runtime was going to use and
   * every later event was refused. Two teammates did real work and were
   * reported as having written nothing back.
   *
   * AND THE NOISE, which is why it is no longer raised up front. Warning at
   * the start of every run in a hidden folder is true and unavoidable, and
   * Colin's teammates there never open a file: an amber line above every
   * exchange, forever, about something inapplicable. It waits for an actual
   * refusal now -- Cursor's failed tool carries `Permission denied`
   * unredacted -- so it arrives beside the failure it explains and a run that
   * reads nothing never hears it.
   */
  function cursorRuntime(): RuntimeDiscovery {
    return {
      ...codexRuntime(),
      id: 'cursor',
      displayName: 'Cursor Agent',
      executable: {
        commandName: 'cursor-agent',
        discoveredPath: process.platform === 'win32' ? 'C:\tools\cursor-agent.exe' : '/tools/cursor-agent',
        executablePath: process.platform === 'win32' ? 'C:\tools\cursor-agent.exe' : '/tools/cursor-agent',
        prefixArgs: [],
        kind: 'native'
      }
    }
  }

  const denied = {
    type: 'tool_call',
    subtype: 'completed',
    call_id: 'call-1',
    tool_call: { readToolCall: { result: { error: { errorMessage: 'Permission denied' } } }, toolCallId: 'call-1' }
  }

  const drive = async (stream: readonly Record<string, unknown>[]) => {
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async () => undefined)
    const { service, scheduled } = scheduledService(
      { start: () => ({ records: records(stream), completion: Promise.resolve(completion()) }) } as unknown as RuntimeProcessRunner,
      fakeLedger({ appendEvents }),
      { discover: async () => [cursorRuntime()], readCursorIgnore: async () => 'safe-workspace/' }
    )
    const updates: CodexMissionUpdate[] = []
    const response = await service.start('Read notes.txt.', 'cursor', 'accept-edits', {}, (update) => updates.push(update))
    if (!response.ok) throw new Error('start refused')
    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'event')).toBe(true)
    })
    const notices = updates.filter(
      (update) => update.kind === 'event' && update.event.type === 'adapter.diagnostic'
        && JSON.stringify(update.event).includes('Cursor cannot read files here')
    )
    const written = appendEvents.mock.calls.flatMap(([, events]) => events)
    return { notices, written }
  }

  it('says nothing when nothing was refused', async () => {
    // The whole point. A run that never opens a file has no reason to hear
    // about a rule that hides files.
    const { notices } = await drive([{ type: 'tool_call', subtype: 'started', call_id: 'c', tool_call: { readToolCall: {}, toolCallId: 'c' } }])
    expect(notices).toHaveLength(0)
  })

  it('explains a refusal when one actually happens', async () => {
    const { notices } = await drive([denied])
    expect(notices).toHaveLength(1)
    expect(JSON.stringify(notices[0])).toContain('.cursorignore')
  })

  it('explains it once, however many reads are refused', async () => {
    const { notices } = await drive([denied, denied, denied])
    expect(notices).toHaveLength(1)
  })

  it('never writes it to the ledger, which once broke every run in such a folder', async () => {
    const { written } = await drive([denied])
    expect(written.some((event) => JSON.stringify(event).includes('cursorignore'))).toBe(false)
  })
})

function scheduledService(
  runner: RuntimeProcessRunner,
  ledger = fakeLedger(),
  extra: Partial<Parameters<typeof createCodexMissionService>[0]> = {}
) {
  const scheduled: Array<() => void> = []
  let nextId = 0
  const service = createCodexMissionService({
    workspacePath: WORKSPACE,
    discover: async () => [codexRuntime()],
    runner,
    ledger,
    createId: () => String(++nextId),
    now: () => new Date(NOW),
    schedule: (task) => scheduled.push(task),
    ...extra
  })
  return { service, scheduled, ledger }
}

describe('Codex mission service', () => {
  it('refuses a forbidden Cursor effort variant before recording or dispatching a turn', async () => {
    const start = vi.fn(() => { throw new Error('must not run') })
    const createMission = vi.fn(async () => undefined)
    const cursor: RuntimeDiscovery = { ...codexRuntime(), id: 'cursor', displayName: 'Cursor Agent', modelHints: { aliases: [], efforts: ['low', 'high'], models: [{ id: 'family-low', displayName: 'Family Low' }, { id: 'family-high', displayName: 'Family High' }] } }
    const { service } = scheduledService({ start }, fakeLedger({ createMission }), { discover: async () => [cursor], organizationPolicy: { present: true, path: '/admin/policy.json', policy: { allowedModels: { cursor: ['family-low'] } } } })
    expect(await service.start('Work', 'cursor', 'accept-edits', { model: 'family-low', effort: 'high' }, () => undefined)).toMatchObject({ ok: false, error: { code: 'POLICY_REFUSED', message: expect.stringContaining('allowedModels') } })
    expect(start).not.toHaveBeenCalled()
    expect(createMission).not.toHaveBeenCalled()
    await service.dispose()
  })
  it('keeps a running mission untouched when organization policy refuses the handoff target', async () => {
    const checkpoint = vi.fn(async () => { throw new Error('This handoff must not stop or reconcile the live run') })
    const ledger = fakeLedger({ createCheckpoint: checkpoint })
    const organizationPolicy = { present: true, path: '/admin/policy.json', policy: { allowedAgents: ['codex'] as const } }
    let signal: AbortSignal | undefined
    const { service, scheduled } = scheduledService({ start: (_spec, _prompt, options) => {
      signal = options?.signal
      return { records: records([]), completion: new Promise((resolve) => {
        signal?.addEventListener('abort', () => resolve(completion({ cancelled: true })), { once: true })
      }) }
    } }, ledger, { organizationPolicy, discover: async () => [codexRuntime(), { ...codexRuntime(), id: 'claude', displayName: 'Claude Code' }] })
    const started = await service.start('Work', 'codex', 'ask', {}, () => undefined)
    if (!started.ok) throw new Error(started.error.message)
    for (const run of scheduled) run()
    const refused = await service.handOff(started.data.runId, 'claude', 'ask', {}, () => undefined)
    expect(refused).toMatchObject({ ok: false, error: { code: 'HANDOFF_REFUSED', untouched: true, message: expect.stringContaining('allowedAgents') } })
    expect(signal?.aborted).toBe(false)
    expect(checkpoint).not.toHaveBeenCalled()
    expect(service.hasMission(started.data.missionId)).toBe(true)
    await service.dispose()
  })
  // L5 (the code review): a start still in its pre-spawn look at the tree
  // spawned anyway after the window closed or the app quit -- nothing checked
  // again once that await returned.
  it('does not spawn a run the app stopped while it looked at the tree', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    let release!: () => void
    const looking = new Promise<void>((resolve) => { release = resolve })
    const { service } = scheduledService({ start }, fakeLedger(), {
      observeDisk: async () => {
        await looking
        return new Map()
      }
    })
    const response = service.start('Fix the typo.', 'codex', 'accept-edits', {}, () => undefined)
    await new Promise((resolve) => setTimeout(resolve, 20))
    service.interrupt()
    release()
    await expect(response).resolves.toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED', message: expect.stringMatching(/stopped before launch/) } })
    expect(start).not.toHaveBeenCalled()
  })

  it('runs one fixed read-only Codex route and streams normalized events', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([
        { type: 'thread.started', thread_id: 'thread-live' },
        { type: 'turn.started' },
        { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Safe result' } },
        { type: 'turn.completed', usage: { output_tokens: 2 } }
      ]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async () => undefined)
    const { service, scheduled } = scheduledService({ start }, fakeLedger({
      createMission,
      appendEvents
    }))
    const updates: CodexMissionUpdate[] = []

    const response = await service.start('Inspect the workspace without changing it.', 'codex', 'ask', {}, (update) => {
      updates.push(update)
    })
    expect(response).toMatchObject({
      ok: true,
      data: {
        runId: 'run_1',
        missionId: 'mission_2',
        runtime: 'codex',
        model: 'account-default',
        cliVersion: '0.151.0-alpha.7.2'
      }
    })
    expect(start).toHaveBeenCalledTimes(1)
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({
      missionId: 'mission_2',
      runId: 'run_1',
      prompt: 'Inspect the workspace without changing it.',
      resolvedRouteId: 'codex-account:default'
    }))
    const [spec, prompt] = start.mock.calls[0] ?? []
    expect(spec).toMatchObject({ runtime: 'codex', cwd: WORKSPACE, stdin: 'prompt', stdout: 'jsonl' })
    expect(spec?.args).toEqual(expect.arrayContaining(['exec', '--json', '--sandbox', 'read-only', '-C', WORKSPACE, '-']))
    expect(spec?.args).not.toContain(prompt)

    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.completed')).toBe(true)
    })
    const eventUpdates = updates.flatMap((update) => update.kind === 'event' ? [{ event: update.event }]
      : update.kind === 'message-deltas' ? update.events.map((event) => ({ event })) : [])
    // The host's own notes (the start-timing note, 0.602) carry no CLI version; the runtime's events all do.
    expect(eventUpdates.filter(({ event }) => (event.payload as { code?: string }).code !== 'host.start-timing').every(({ event }) => event.cliVersion === '0.151.0-alpha.7.2')).toBe(true)
    expect(JSON.stringify(updates)).toContain('Safe result')
    expect(appendEvents.mock.calls.flatMap(([, events]) => events).map(({ type }) => type)).toEqual([
      'run.started',
      'step.started',
      'message.delta',
      'step.completed',
      'run.completed',
      // The host's start-timing note, appended after the run's own events (0.602).
      'adapter.diagnostic'
    ])
  })

  it('rejects invalid prompts before discovery or process launch', async () => {
    const discover = vi.fn(async () => [codexRuntime()])
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover,
      runner: { start },
      ledger: fakeLedger()
    })

    await expect(service.start('   ', 'codex', 'ask', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'INVALID_PROMPT' }
    })
    expect(discover).not.toHaveBeenCalled()
    expect(start).not.toHaveBeenCalled()
  })

  it('never launches Codex when the durable mission header cannot be written', async () => {
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const ledger = fakeLedger({
      createMission: async () => {
        throw new Error('private storage path')
      }
    })
    const { service } = scheduledService({ start }, ledger)

    await expect(service.start('Do safe work.', 'codex', 'ask', {}, () => undefined)).resolves.toEqual({
      ok: false,
      error: {
        code: 'PERSISTENCE_FAILED',
        message: 'The mission could not be created in the durable local ledger.'
      }
    })
    expect(start).not.toHaveBeenCalled()
  })

  it('removes the actual skill copy before returning a failed ledger start', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-start-skills-'))
    try {
      let copied = false
      const spawn = vi.fn(() => { throw new Error('No process should start.') })
      const { service } = scheduledService({ start: vi.fn() }, fakeLedger({ createMission: async () => { throw new Error('Disk full.') } }), {
        appServerSpawn: spawn,
        runSkills: async () => {
          const prepared = await prepareRunSkills({ scratchRoot: root, candidate: { name: 'review', description: 'Review.', body: 'Read twice.' } })
          expect(await readdir(prepared!.skillsDir)).toEqual(['review'])
          copied = true
          return prepared
        }
      })
      const response = await service.start('Read safely.', 'codex', 'ask', {}, () => undefined)
      expect(copied).toBe(true)
      expect(response).toMatchObject({ ok: false, error: { code: 'PERSISTENCE_FAILED' } })
      expect(await readdir(root)).toEqual([])
      expect(spawn).not.toHaveBeenCalled()
    } finally { await rm(root, { recursive: true, force: true }) }
  })

  it('removes a skill copy if interrupted after preparation but before launch', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-stop-skills-'))
    try {
      const spawn = vi.fn(() => { throw new Error('No process should start.') })
      const { service } = scheduledService({ start: vi.fn() }, fakeLedger(), {
        appServerSpawn: spawn,
        runSkills: async () => {
          const prepared = await prepareRunSkills({ scratchRoot: root, candidate: { name: 'review', description: 'Review.', body: 'Read twice.' } })
          service.interrupt()
          return prepared
        }
      })
      expect(await service.start('Read safely.', 'codex', 'ask', {}, () => undefined)).toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED' } })
      expect(await readdir(root)).toEqual([])
      expect(spawn).not.toHaveBeenCalled()
    } finally { await rm(root, { recursive: true, force: true }) }
  })

  it('removes prepared skills when the command builder refuses its options', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-refused-skills-'))
    try {
      const runSkills = vi.fn(async () => prepareRunSkills({ scratchRoot: root, candidate: { name: 'review', description: 'Review.', body: 'Read twice.' } }))
      const start = vi.fn() satisfies RuntimeProcessRunner['start']
      const { service } = scheduledService({ start }, fakeLedger(), {
        discover: async () => [{ ...codexRuntime(), id: 'copilot' as const }],
        copilotCommandGuard: async () => '\0',
        runSkills
      })
      expect(await service.start('Read safely.', 'copilot', 'ask', {}, () => undefined)).toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED' } })
      expect(runSkills).toHaveBeenCalledOnce()
      expect(await readdir(root)).toEqual([])
      expect(start).not.toHaveBeenCalled()
    } finally { await rm(root, { recursive: true, force: true }) }
  })

  it('leaves a registered run its skills until process completion owns their cleanup', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-owned-skills-'))
    try {
      let finish!: (value: RuntimeProcessCompletion) => void
      const ended = new Promise<RuntimeProcessCompletion>((resolve) => { finish = resolve })
      const { service, scheduled } = scheduledService({ start: () => ({ records: records([]), completion: ended }) }, fakeLedger(), {
        discover: async () => [{ ...codexRuntime(), id: 'copilot' as const }],
        runSkills: async () => prepareRunSkills({ scratchRoot: root, candidate: { name: 'review', description: 'Review.', body: 'Read twice.' } })
      })
      const response = await service.start('Read safely.', 'copilot', 'ask', {}, () => undefined)
      expect(response.ok).toBe(true)
      expect(await readdir(root)).toHaveLength(1)
      finish(completion())
      scheduled[0]?.()
      await vi.waitFor(async () => { expect(await readdir(root)).toEqual([]) })
    } finally { await rm(root, { recursive: true, force: true }) }
  })

  it('records that a plan was a plan, while still holding it read-only', async () => {
    // Two facts that have to hold together. `plan` and `ask` are both
    // read-only, which is why the mode used to collapse into `ask` on its way
    // in -- harmless for permission, and it meant the record never knew a
    // plan had been asked for, so a plan reopened after a restart lost its
    // "Build this plan" offer (QA, 2026-09-06).
    let created: { readonly sandbox?: string; readonly mode?: string } | undefined
    const start = (() => {
      throw new Error('stop here')
    }) as unknown as RuntimeProcessRunner['start']
    const ledger = fakeLedger({
      createMission: async (metadata: { readonly sandbox?: string; readonly mode?: string }) => {
        created = metadata
        return undefined as never
      }
    })
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: { start },
      ledger
    })

    await service.start('Plan how you would add a LICENSE file.', 'codex', 'plan', {}, () => undefined)
    expect(created?.mode).toBe('plan')
    expect(created?.sandbox).toBe('read-only')
  })

  it('refuses an Auto mission when the workspace has Auto switched off, and records nothing', async () => {
    // The renderer does not offer Auto with the switch off, but a window left
    // open across a change of mind, or a routine recorded while it was on,
    // both send the mode anyway. The switch is what decides, and it is asked
    // on the path every run takes.
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const ledger = fakeLedger()
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: { start },
      ledger,
      autoModeAllowed: async () => false
    })

    await expect(service.start('Tidy my whole machine.', 'codex', 'auto', {}, () => undefined)).resolves.toEqual({
      ok: false,
      error: {
        code: 'RUNTIME_START_FAILED',
        message:
          'Auto mode is switched off for this workspace. Turn it on in Settings to let a run work outside the workspace folder. Nothing was recorded.'
      }
    })
    expect(start).not.toHaveBeenCalled()
  })

  it('refuses a paid route in a window held to free routes, before anything is asked or recorded', async () => {
    /*
     * 2026-09-22: a drive whose header said it "spends nothing" sent a turn
     * on Cursor's Grok. The refusal lives here, on the path every run takes,
     * so a drive that picks a paid route by accident is stopped by the app.
     */
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const discover = vi.fn(async () => [codexRuntime()])
    const createMission = vi.fn(async () => undefined as never)
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover,
      runner: { start },
      ledger: fakeLedger({ createMission }),
      freeRoutesOnly: true
    })

    for (const [runtime, model] of [
      ['cursor', 'cursor-grok-4.6-medium'],
      ['codex', undefined],
      ['claude', 'sonnet'],
      // OpenCode is not free by being OpenCode: only its free models are.
      ['opencode', 'anthropic/claude-sonnet-5']
    ] as const) {
      await expect(
        service.start('Reply with one word.', runtime, 'ask', model === undefined ? {} : { model }, () => undefined)
      ).resolves.toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED', message: FREE_ONLY_REFUSAL } })
    }
    expect(discover).not.toHaveBeenCalled()
    expect(createMission).not.toHaveBeenCalled()
    expect(start).not.toHaveBeenCalled()
  })

  it('lets a free route through a window held to free routes', async () => {
    // The control: the guard that refuses everything would pass the test above.
    const discover = vi.fn(async () => [codexRuntime()])
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover,
      runner: { start: vi.fn() satisfies RuntimeProcessRunner['start'] },
      ledger: fakeLedger(),
      freeRoutesOnly: true
    })

    const answer = await service.start(
      'Reply with one word.',
      'opencode',
      'ask',
      { model: 'opencode/muse-spark-1.3-contributor-free' },
      () => undefined
    )
    expect(JSON.stringify(answer)).not.toContain(FREE_ONLY_REFUSAL)
    expect(discover).toHaveBeenCalled()
  })

  it('refuses an Auto mission when nobody wired the switch at all', async () => {
    // Absent means off. A caller that forgets to pass the seam cannot get the
    // widest sandbox by omission.
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: { start },
      ledger: fakeLedger()
    })

    await expect(service.start('Tidy my whole machine.', 'codex', 'auto', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED' }
    })
    expect(start).not.toHaveBeenCalled()
  })

  it('refuses the NEXT Auto run once the switch is taken back, on the same service', async () => {
    // The 0.35.0 targeted QA asked for exactly this and could not test it:
    // "same-session mode changes and revocation before scheduled/relayed
    // starts". A relay hop and a routine step both start through this
    // service without a window in the loop, so the switch has to be read as
    // each run starts rather than once when the service is built.
    let allowed = true
    const started: string[] = []
    const start = ((spec: { readonly args: readonly string[] }) => {
      started.push(spec.args.join(' '))
      throw new Error('stop here')
    }) as unknown as RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: { start },
      ledger: fakeLedger(),
      autoModeAllowed: async () => allowed
    })

    await service.start('First, while it is on.', 'codex', 'auto', {}, () => undefined)
    expect(started).toHaveLength(1)
    expect(started[0]).toContain('--sandbox danger-full-access')

    // The person switches it off in Settings. Nothing else changes.
    allowed = false

    await expect(service.start('Second, after taking it back.', 'codex', 'auto', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED' }
    })
    // Not merely refused: nothing was launched, so nothing can have run wide.
    expect(started).toHaveLength(1)
  })

  it('runs an Auto mission on the widest sandbox once the workspace allows it', async () => {
    // The argv is what this case is about, so the spec is captured and the
    // spawn is stopped right after it.
    let launched: { readonly args: readonly string[]; readonly sandbox?: string } | undefined
    const start = ((spec: { readonly args: readonly string[]; readonly sandbox?: string }) => {
      launched = spec
      throw new Error('stop here')
    }) as unknown as RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: { start },
      ledger: fakeLedger(),
      autoModeAllowed: async () => true
    })

    await service.start('Tidy my whole machine.', 'codex', 'auto', {}, () => undefined)
    expect(launched?.args.join(' ')).toContain('--sandbox danger-full-access')
    // And the spec says so itself, which is what the runner re-reads at spawn.
    expect(launched?.sandbox).toBe('full-access')
  })

  it("says a runtime whose own service did not answer could not be reached, not that it is signed out (0.495)", async () => {
    // Colin, 2026-09-30: Cursor's servers were resetting connections and the
    // refusal told him to install or sign in.
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime('unhealthy')],
      runner: { start: vi.fn() },
      ledger: fakeLedger()
    })
    const response = await service.start('Do safe work.', 'codex', 'ask', {}, () => undefined)
    expect(response.ok ? '' : response.error.message).toContain('could not be reached just now')
    expect(response.ok ? '' : response.error.message).not.toContain('sign in')
  })

  it('returns an actionable generic error when Codex is not ready', async () => {
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime('authentication-required')],
      runner: { start },
      ledger: fakeLedger()
    })

    await expect(service.start('Do safe work.', 'codex', 'ask', {}, () => undefined)).resolves.toEqual({
      ok: false,
      error: {
        code: 'CODEX_UNAVAILABLE',
        message: 'Codex CLI is signed out. Sign in to it, then retry discovery.'
      }
    })
    expect(start).not.toHaveBeenCalled()
  })

  it('does not launch a process when its owning window closes during discovery', async () => {
    let finishDiscovery!: (value: readonly RuntimeDiscovery[]) => void
    const discovery = new Promise<readonly RuntimeDiscovery[]>((resolve) => {
      finishDiscovery = resolve
    })
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: () => discovery,
      runner: { start },
      ledger: fakeLedger()
    })

    const response = service.start('Do safe work.', 'codex', 'ask', {}, () => undefined)
    service.dispose()
    finishDiscovery([codexRuntime()])

    await expect(response).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED' }
    })
    expect(start).not.toHaveBeenCalled()
  })

  it('allows one active run and synthesizes cancellation from the host signal', async () => {
    let releaseRecords!: () => void
    const recordsReleased = new Promise<void>((resolve) => {
      releaseRecords = resolve
    })
    let resolveCompletion!: (value: RuntimeProcessCompletion) => void
    const processCompletion = new Promise<RuntimeProcessCompletion>((resolve) => {
      resolveCompletion = resolve
    })
    const start = vi.fn((_spec, _prompt, options): RuntimeProcessRun => {
      options?.signal?.addEventListener('abort', () => {
        releaseRecords()
        resolveCompletion(completion({
          exitCode: null,
          signal: 'SIGINT',
          recordCount: 1,
          cancelled: true
        }))
      }, { once: true })
      return {
        records: {
          async *[Symbol.asyncIterator]() {
            yield { sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread-cancel' }) }
            await recordsReleased
          }
          ,
          drainAvailable: () => []
        },
        completion: processCompletion
      }
    }) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start })
    const updates: CodexMissionUpdate[] = []
    const first = await service.start('Wait safely.', 'codex', 'ask', {}, (update) => updates.push(update))
    expect(first.ok).toBe(true)
    // A reply to the turn still running waits (0.551: a new conversation would start).
    const firstMission = first.ok ? first.data.missionId : undefined
    await expect(service.start('A second run.', 'codex', 'ask', {}, () => undefined, undefined, undefined, firstMission)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUN_ALREADY_ACTIVE' }
    })
    if (!first.ok) return

    scheduled[0]?.()
    expect(service.cancel(first.data.runId)).toEqual({
      ok: true,
      data: { runId: first.data.runId, state: 'cancellation-requested' }
    })
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.cancelled')).toBe(true)
    })
    expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.failed')).toBe(false)
  })

  it('never forwards process rejection details to the renderer', async () => {
    const secret = 'C:\\private\\token.txt sk-should-never-leak'
    const process: RuntimeProcessRun = {
      records: records([]),
      completion: Promise.reject(new Error(secret))
    }
    const { service, scheduled } = scheduledService({ start: () => process })
    const updates: CodexMissionUpdate[] = []
    await service.start('Safe prompt.', 'codex', 'ask', {}, (update) => updates.push(update))
    scheduled[0]?.()

    await vi.waitFor(() => expect(updates.filter((update) => update.kind !== 'start-phase')).toHaveLength(1))
    expect(updates.filter((update) => update.kind !== 'start-phase')[0]).toMatchObject({
      kind: 'transport-error',
      error: { code: 'RUNTIME_TRANSPORT_FAILED' }
    })
    expect(JSON.stringify(updates)).not.toContain(secret)
    expect(JSON.stringify(updates)).not.toContain('private')
  })
})

describe('Codex mission durability and lifecycle boundaries', () => {
  it('persists before emitting, aborts on a failed durable write, and frees the service afterwards', async () => {
    const timeline: string[] = []
    let aborted = false
    let resolveCompletion!: (value: RuntimeProcessCompletion) => void
    const processCompletion = new Promise<RuntimeProcessCompletion>((resolve) => {
      resolveCompletion = resolve
    })
    const start = vi.fn((_spec, _prompt, options): RuntimeProcessRun => {
      options?.signal?.addEventListener('abort', () => {
        aborted = true
        resolveCompletion(completion({ exitCode: null, signal: 'SIGINT', recordCount: 2, cancelled: true }))
      }, { once: true })
      return {
        records: records([
          { type: 'thread.started', thread_id: 'thread-durable' },
          { type: 'turn.started' },
          { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'never shown' } }
        ]),
        completion: processCompletion
      }
    }) satisfies RuntimeProcessRunner['start']
    let appendCalls = 0
    const ledger = fakeLedger({
      appendEvents: async (_missionId, events) => {
        appendCalls += 1
        if (appendCalls >= 2) throw new Error('disk full')
        timeline.push(`append:${events.map(({ type }) => type).join(',')}`)
      }
    })
    const { service, scheduled } = scheduledService({ start }, ledger)
    const updates: CodexMissionUpdate[] = []
    const response = await service.start('Persist safely.', 'codex', 'ask', {}, (update) => {
      if (update.kind !== 'start-phase') timeline.push(update.kind === 'event' ? `emit:${update.event.type}` : `emit:${update.kind}`)
      updates.push(update)
    })
    expect(response.ok).toBe(true)
    scheduled[0]?.()

    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'persistence-error')).toBe(true)
    })
    expect(aborted).toBe(true)
    expect(timeline).toEqual(['append:run.started', 'emit:run.started', 'emit:persistence-error'])
    expect(updates.filter((update) => update.kind === 'persistence-error')).toHaveLength(1)
    // The card says WHY. It used to say only that the ledger could not be
    // written, and nothing on the machine kept the error (Wembley's run,
    // 2026-09-17, cause unrecoverable).
    const failed = updates.find((update) => update.kind === 'persistence-error')
    expect(failed?.kind === 'persistence-error' ? failed.error.message : '').toContain('disk full')
    // The service is free again as soon as the renderer hears about the failure.
    await expect(service.start('Try again.', 'codex', 'ask', {}, () => undefined)).resolves.toMatchObject({ ok: true })
  })

  it('records host launch failures durably and reports a persistence failure when even that write fails', async () => {
    const failingStart = vi.fn(() => {
      throw new Error('spawn blocked')
    }) satisfies RuntimeProcessRunner['start']
    const appendHostFailure = vi.fn<MissionLedger['appendHostFailure']>(async () => undefined)
    const { service } = scheduledService({ start: failingStart }, fakeLedger({ appendHostFailure }))
    await expect(service.start('Do safe work.', 'codex', 'ask', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED' }
    })
    expect(appendHostFailure).toHaveBeenCalledWith('mission_2', expect.objectContaining({
      code: 'runtime-start-failed'
    }))

    const { service: doomed } = scheduledService({ start: failingStart }, fakeLedger({
      appendHostFailure: async () => {
        throw new Error('disk full')
      }
    }))
    await expect(doomed.start('Do safe work.', 'codex', 'ask', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'PERSISTENCE_FAILED' }
    })
  })

  it('persists a transport failure and downgrades to a persistence error when the write fails', async () => {
    const brokenProcess = (): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.reject(new Error('transport died'))
    })
    const appendHostFailure = vi.fn<MissionLedger['appendHostFailure']>(async () => undefined)
    const { service, scheduled } = scheduledService({ start: () => brokenProcess() }, fakeLedger({ appendHostFailure }))
    const updates: CodexMissionUpdate[] = []
    await service.start('Safe prompt.', 'codex', 'ask', {}, (update) => updates.push(update))
    scheduled[0]?.()
    await vi.waitFor(() => expect(updates.filter((update) => update.kind !== 'start-phase')).toHaveLength(1))
    expect(updates.filter((update) => update.kind !== 'start-phase')[0]).toMatchObject({ kind: 'transport-error' })
    expect(appendHostFailure).toHaveBeenCalledWith('mission_2', expect.objectContaining({
      code: 'runtime-transport-failed'
    }))

    const { service: doomed, scheduled: doomedScheduled } = scheduledService({ start: () => brokenProcess() }, fakeLedger({
      appendHostFailure: async () => {
        throw new Error('disk full')
      }
    }))
    const doomedUpdates: CodexMissionUpdate[] = []
    await doomed.start('Safe prompt.', 'codex', 'ask', {}, (update) => doomedUpdates.push(update))
    doomedScheduled[0]?.()
    const doomedSaid = () => doomedUpdates.filter((update) => update.kind !== 'start-phase')
    await vi.waitFor(() => expect(doomedSaid()).toHaveLength(1))
    expect(doomedSaid()[0]).toMatchObject({ kind: 'persistence-error' })
  })

  it('rejects a second start while the first is still resolving discovery', async () => {
    let finishDiscovery!: (value: readonly RuntimeDiscovery[]) => void
    const discovery = new Promise<readonly RuntimeDiscovery[]>((resolve) => {
      finishDiscovery = resolve
    })
    const start = vi.fn((): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: () => discovery,
      runner: { start },
      ledger: fakeLedger(),
      schedule: () => undefined
    })

    // Two replies to one turn (0.551: the claim is the conversation's, not the teammate's).
    const first = service.start('First mission.', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_earlier')
    await expect(service.start('Second mission.', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_earlier')).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUN_ALREADY_ACTIVE' }
    })
    finishDiscovery([codexRuntime()])
    // The earlier turn is not in this fake ledger; what is checked is the claim during discovery.
    await first
  })

  // 0.813, Sol's review of 0.812: the person's reply started, and Locust's fix of the same turn would run beside it.
  it('refuses Locust\'s fix of a turn once another turn continues it, and lets a person\'s own reply through', async () => {
    let finishRun!: (value: RuntimeProcessCompletion) => void
    const running = new Promise<RuntimeProcessCompletion>((resolve) => {
      finishRun = resolve
    })
    const start = vi.fn((): RuntimeProcessRun => ({ records: records([]), completion: running })) satisfies RuntimeProcessRunner['start']
    // The turn both continue: finished, in this folder.
    const earlier = {
      metadata: {
        missionId: 'mission_earlier', runId: 'run_earlier', prompt: 'Add sum.', runtime: 'codex', model: 'account-default',
        requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null, workspaceId: workspaceIdFor(WORKSPACE),
        sandbox: 'read-only', executionPolicyVersion: 1, createdAt: NOW
      },
      events: [{ id: 'e1', runId: 'run_earlier', missionId: 'mission_earlier', sequence: 1, type: 'run.started', occurredAt: NOW, sourceAdapter: 'codex', payload: { runtimeThreadId: 'thread-earlier', evidence: { redacted: true } } }],
      hostFailures: [], checkpoints: [], peerLinks: [], editChecks: [], approvals: [], phase: 'completed', lastUpdatedAt: NOW, ledgerSequence: 2, issues: []
    } as never
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: { start },
      ledger: fakeLedger({ getMission: async (missionId) => (missionId === 'mission_earlier' ? earlier : undefined) }),
      schedule: () => undefined
    })
    const reply = await service.start('My own reply.', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_earlier')
    expect(reply).toMatchObject({ ok: true })
    const fix = { kind: 'check-fix', fix: 1, of: 2, command: 'npm test' } as const
    await expect(service.start('The check failed.', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_earlier', undefined, fix)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUN_ALREADY_ACTIVE', message: 'This conversation went on before the fix could start, so Locust did not send the failing check back.' }
    })
    // A fix of another turn is not held back by it.
    expect(start).toHaveBeenCalledTimes(1)
    finishRun(completion())
  })

  it('drains in-flight work on dispose, latches the service closed, and keeps interrupt reusable', async () => {
    let resolveCompletion!: (value: RuntimeProcessCompletion) => void
    const processCompletion = new Promise<RuntimeProcessCompletion>((resolve) => {
      resolveCompletion = resolve
    })
    const start = vi.fn((_spec, _prompt, options): RuntimeProcessRun => {
      options?.signal?.addEventListener('abort', () => {
        resolveCompletion(completion({ exitCode: null, signal: 'SIGINT', recordCount: 1, cancelled: true }))
      }, { once: true })
      return {
        records: {
          async *[Symbol.asyncIterator]() {
            yield { sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread-drain' }) }
            await processCompletion
          }
          ,
          drainAvailable: () => []
        },
        completion: processCompletion
      }
    }) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start })
    const updates: CodexMissionUpdate[] = []
    await service.start('Wait safely.', 'codex', 'ask', {}, (update) => updates.push(update))
    scheduled[0]?.()

    await service.dispose()
    // dispose resolved only after consume settled, so the terminal receipt is already emitted.
    expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.cancelled')).toBe(true)
    await expect(service.start('After shutdown.', 'codex', 'ask', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED' }
    })

    const { service: reusable } = scheduledService({
      start: () => ({ records: records([]), completion: Promise.resolve(completion()) })
    })
    reusable.interrupt()
    await expect(reusable.start('After interrupt.', 'codex', 'ask', {}, () => undefined)).resolves.toMatchObject({ ok: true })
  })
  it('checkpoints the mission it cut short, after that run has finished settling', async () => {
    const calls: Array<[string, string]> = []
    const ledger = fakeLedger({
      createCheckpoint: async (missionId, reason) => {
        calls.push([missionId, reason])
        return { missionId } as never
      }
    })
    const start = vi.fn((_spec, _prompt, options): RuntimeProcessRun => {
      const signal = options?.signal
      return {
        records: records([{ type: 'thread.started', thread_id: 'thread-live' }]),
        completion: new Promise((resolve) => {
          signal?.addEventListener('abort', () => {
            resolve(completion({ cancelled: true }))
          })
        })
      }
    }) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, ledger)
    const updates: CodexMissionUpdate[] = []
    await service.start('Wait safely.', 'codex', 'ask', {}, (update) => updates.push(update))
    scheduled[0]?.()

    // The real sequence: the window closes, the run settles and the service
    // forgets it, and only THEN does the app quit. Calling dispose() straight
    // after interrupt() leaves the mission still active, so dispose reads the
    // id itself and the test passes whether or not interrupt() recorded it --
    // which is exactly how this test first passed against a service that had
    // the line removed.
    service.interrupt()
    for (let tick = 0; tick < 20; tick += 1) {
      if (updates.some((update) => update.kind === 'event' && update.event.type === 'run.cancelled')) break
      await new Promise((resolve) => setImmediate(resolve))
    }
    expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.cancelled')).toBe(true)

    await service.dispose()

    expect(calls).toEqual([['mission_2', 'shutdown']])
  })

  it('does not checkpoint when no mission was running', async () => {
    const calls: string[] = []
    const ledger = fakeLedger({
      createCheckpoint: async (missionId) => {
        calls.push(missionId)
        return {} as never
      }
    })
    const { service } = scheduledService(
      { start: () => ({ records: records([]), completion: Promise.resolve(completion()) }) },
      ledger
    )

    await service.dispose()

    expect(calls).toEqual([])
  })

  it('still shuts down when the checkpoint write fails', async () => {
    const ledger = fakeLedger({
      createCheckpoint: async () => {
        throw new Error('disk full')
      }
    })
    const start = vi.fn((_spec, _prompt, options): RuntimeProcessRun => {
      const signal = options?.signal
      return {
        records: records([{ type: 'thread.started', thread_id: 'thread-live' }]),
        completion: new Promise((resolve) => {
          signal?.addEventListener('abort', () => {
            resolve(completion({ cancelled: true }))
          })
        })
      }
    }) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, ledger)
    await service.start('Wait safely.', 'codex', 'ask', {}, () => undefined)
    scheduled[0]?.()
    service.interrupt()

    // A bookkeeping write must never be able to hang or crash shutdown.
    await expect(service.dispose()).resolves.toBeUndefined()
  })
})

describe('mission sandbox', () => {
  function specFor(mode: 'ask' | 'accept-edits') {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'thread.started', thread_id: 't' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    return { start, service: scheduledService({ start }).service }
  }

  /*
   * M11 (the code review): a teammate set to work on its own branch, whose
   * branch could not be made, is refused on EVERY start -- not only a direct
   * message. A room post, a relayed share, a routine step or a resume passed
   * the same context here, and ran in the shared folder, in the teammate's
   * write mode, which is exactly what Own branch was turned on to prevent.
   */
  it('refuses a teammate whose own branch could not be made, whoever started it', async () => {
    const { start, service } = specFor('accept-edits')
    const peer: MissionPeerContext = {
      self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' },
      others: [],
      worktreeRefused: 'Wren is set to work on its own branch, but this folder is not a git repository.'
    }
    const response = await service.start('Fix the typo.', 'codex', 'accept-edits', {}, () => undefined, undefined, peer)
    expect(response).toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED', message: 'Wren is set to work on its own branch, but this folder is not a git repository.' }
    })
    expect(start).not.toHaveBeenCalled()
  })

  /*
   * A teammate's MONTHLY LIMIT (0.353), on the same shared path as the branch
   * above: whoever starts the run -- a message, a relayed reply, a routine
   * step, a room post -- it is refused before anything is recorded, with its
   * own code, so a routine can tell it from a dispatch that may have run.
   */
  it.each([undefined, { kind: 'mcp' as const }])('refuses a teammate at the monthly limit, whoever started it, before anything is recorded (%j)', async (origin) => {
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const said = "Wren has reached this month's limit: $5.02 of $5.00. Raise the limit by editing Wren, or it starts again on October 1."
    const asked: string[] = []
    const { service } = scheduledService({ start }, fakeLedger({ createMission }), {
      spendRefusal: async (teammateId) => {
        asked.push(teammateId)
        return teammateId === 'tm_wren' ? said : undefined
      }
    })
    const wren: MissionPeerContext = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [] }
    const response = await service.start('Fix the typo.', 'codex', origin === undefined ? 'accept-edits' : 'ask', {}, () => undefined, undefined, wren, undefined, undefined, origin)
    expect(response).toMatchObject({ ok: false, error: { code: 'SPEND_LIMIT_REACHED', message: said } })
    expect(start).not.toHaveBeenCalled()
    expect(createMission).not.toHaveBeenCalled()

    // Another teammate, and a run that belongs to nobody, start as before.
    const juno: MissionPeerContext = { self: { teammateId: 'tm_juno', name: 'Juno', role: 'Reviewer' }, others: [] }
    expect(await service.start('Look around.', 'codex', 'ask', {}, () => undefined, undefined, juno)).toMatchObject({ ok: true })
    expect(asked).toEqual(['tm_wren', 'tm_juno'])
  })

  it('lets the run start when the limit cannot be checked, and says why in the log', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const notes: string[] = []
    const { service } = scheduledService({ start }, fakeLedger(), {
      spendRefusal: async () => {
        throw new Error('the ledger would not read')
      },
      note: (label, detail) => notes.push(`${label}: ${detail}`)
    })
    const wren: MissionPeerContext = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [] }
    expect(await service.start('Look around.', 'codex', 'ask', {}, () => undefined, undefined, wren)).toMatchObject({ ok: true })
    expect(notes).toEqual(["spend-limit: could not check tm_wren's monthly limit: the ledger would not read"])
  })

  it('runs read-only when the mode is ask', async () => {
    const { start, service } = specFor('ask')
    await service.start('Look around.', 'codex', 'ask', {}, () => undefined)
    expect(start.mock.calls[0]?.[0]?.args).toEqual(
      expect.arrayContaining(['--sandbox', 'read-only'])
    )
    expect(start.mock.calls[0]?.[0]?.args).not.toContain('workspace-write')
  })

  it('records an MCP-origin Ask run with the actual read-only sandbox', async () => {
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({ records: records([]), completion: Promise.resolve(completion()) })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({ createMission }))
    const result = await service.start('Review this.', 'codex', 'ask', {}, () => undefined, undefined, undefined, undefined, undefined, { kind: 'mcp' })
    expect(result).toMatchObject({ ok: true, data: { sandbox: 'read-only' } })
    expect(start.mock.calls[0]?.[0]?.args).toEqual(expect.arrayContaining(['--sandbox', 'read-only']))
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({ sandbox: 'read-only', mode: 'ask', startedBy: { kind: 'mcp' } }))
  })

  it('runs workspace-write only when edits were explicitly accepted', async () => {
    const { start, service } = specFor('accept-edits')
    const response = await service.start('Fix the typo.', 'codex', 'accept-edits', {}, () => undefined)
    expect(start.mock.calls[0]?.[0]?.args).toEqual(
      expect.arrayContaining(['--sandbox', 'workspace-write'])
    )
    // What the run was allowed to do is reported back, so the UI states the
    // real posture rather than assuming one.
    expect(response).toMatchObject({ ok: true, data: { sandbox: 'workspace-write' } })
  })

  it('records the sandbox it actually used in the durable header', async () => {
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({ createMission }))

    await service.start('Fix the typo.', 'codex', 'accept-edits', {}, () => undefined)

    expect(createMission).toHaveBeenCalledWith(
      expect.objectContaining({ sandbox: 'workspace-write' })
    )
  })

  it('never widens the sandbox for an unrecognized mode', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start })

    // The IPC layer normalizes this, but the service must not depend on that:
    // a mode it does not recognise is read-only, never write.
    await service.start('Do something.', 'codex', 'whatever' as never, {}, () => undefined)

    expect(start.mock.calls[0]?.[0]?.args).toEqual(expect.arrayContaining(['--sandbox', 'read-only']))
  })
})

describe('runtime selection', () => {
  function claudeRuntime(): RuntimeDiscovery {
    return {
      ...codexRuntime(),
      id: 'claude',
      displayName: 'Claude Code',
      executable: {
        commandName: 'claude',
        discoveredPath: process.platform === 'win32' ? 'C:\tools\claude.exe' : '/tools/claude',
        executablePath: process.platform === 'win32' ? 'C:\tools\claude.exe' : '/tools/claude',
        prefixArgs: [],
        kind: 'native'
      }
    }
  }

  function serviceWith(runtimes: RuntimeDiscovery[], ledger = fakeLedger()) {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => runtimes,
      runner: { start },
      ledger,
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: () => undefined
    })
    return { start, service }
  }

  it('hands a running Claude turn a message through its open input (A2.10)', async () => {
    const sent: string[] = []
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      // Still running: the turn has not reached its result.
      completion: new Promise<RuntimeProcessCompletion>(() => undefined),
      send: (text: string) => {
        sent.push(text)
        return true
      }
    })) satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime(), claudeRuntime()],
      runner: { start },
      ledger: fakeLedger(),
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: () => undefined
    })
    const response = await service.start('Count to 200.', 'claude', 'accept-edits', {}, () => undefined)
    const runId = response.ok ? response.data.runId : ''
    expect(start.mock.calls[0]?.[0]?.stdin).toBe('stream-json')
    expect(await service.steer(runId, 'Stop at 100.')).toBe(true)
    expect(sent).toEqual(['Stop at 100.'])
    // A run whose transport cannot take one says so.
    const { service: plain } = serviceWith([codexRuntime(), claudeRuntime()])
    const other = await plain.start('Count to 200.', 'claude', 'accept-edits', {}, () => undefined)
    expect(await plain.steer(other.ok ? other.data.runId : '', 'Stop at 100.')).toBe(false)
  }, 15_000)

  it('launches Claude with its own restricted argv when Claude is chosen', async () => {
    const { start, service } = serviceWith([codexRuntime(), claudeRuntime()])

    const response = await service.start('Summarize this repo.', 'claude', 'ask', {}, () => undefined)

    expect(response).toMatchObject({ ok: true, data: { runtime: 'claude' } })
    const args = start.mock.calls[0]?.[0]?.args ?? []
    expect(start.mock.calls[0]?.[0]?.runtime).toBe('claude')
    expect(args).toEqual(expect.arrayContaining(['--print', '--output-format', 'stream-json']))
    // Claude's command takes no sandbox flag; the Codex one must not leak in.
    expect(args).not.toContain('--sandbox')
  })

  it('records a Claude run that may edit as one that may edit', async () => {
    // This asserted the opposite until 2026-09-03, on the reading that only
    // Codex takes a sandbox. Claude Code takes a different one -- the
    // permission mode and tool list, set from this very value -- and forcing
    // read-only here meant a mission started in Accept edits ran in plan mode
    // and had its Write refused with "Write is disabled for this session".
    // The receipt has to say what the run was actually allowed to do.
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service } = serviceWith([codexRuntime(), claudeRuntime()], fakeLedger({ createMission }))

    const response = await service.start('Edit something.', 'claude', 'accept-edits', {}, () => undefined)

    expect(response).toMatchObject({ ok: true, data: { sandbox: 'workspace-write' } })
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({
      runtime: 'claude',
      sandbox: 'workspace-write'
    }))
  })

  it('sends Claude the mode it was started in, not a default', async () => {
    // The receipt said workspace-write while the argv still said plan, because
    // the Claude branch built its command without a sandbox at all. The run
    // answered "I don't have a Write tool available in this session" while the
    // ledger claimed it could edit -- the ledger and the process disagreeing
    // is the one thing a receipt must never do.
    const { service, start } = serviceWith([codexRuntime(), claudeRuntime()], fakeLedger())

    await service.start('Edit something.', 'claude', 'accept-edits', {}, () => undefined)

    const spec = start.mock.calls[0]?.[0] as { args: string[] }
    expect(spec.args.join(' ')).toContain('--permission-mode acceptEdits')
    expect(spec.args.join(' ')).toContain('Edit,Write,NotebookEdit,Bash')
  })

  it('still records a Claude run asked to read as read-only', async () => {
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service } = serviceWith([codexRuntime(), claudeRuntime()], fakeLedger({ createMission }))

    const response = await service.start('Read something.', 'claude', 'ask', {}, () => undefined)

    expect(response).toMatchObject({ ok: true, data: { sandbox: 'read-only' } })
  })

  it('refuses a runtime whose event stream it cannot read yet, by name, recording nothing', async () => {
    const createMission = vi.fn(async () => undefined)
    const { service, start } = serviceWith(
      [{ ...codexRuntime(), id: 'gemini', displayName: 'Gemini CLI', optional: true }],
      fakeLedger({ createMission })
    )

    await expect(service.start('Do work.', 'gemini', 'ask', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED', message: expect.stringContaining('Gemini CLI') }
    })
    expect(start).not.toHaveBeenCalled()
    expect(createMission).not.toHaveBeenCalled()
  })

  it('runs a Cursor mission under its own command and its own normalizer', async () => {
    // Two records as Cursor prints them, so the normalizer's signature on the
    // events is the fact under test -- a Codex normalizer would sign them as
    // Codex, or read nothing it recognises.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([
        { type: 'system', subtype: 'init', session_id: 'cursor-session-1', model: 'Composer 2.5', permissionMode: 'default' },
        { type: 'assistant', message: { role: 'assistant', content: [{ type: 'text', text: 'pebble' }] } },
        { type: 'result', subtype: 'success', is_error: false, result: 'pebble' }
      ]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async () => undefined)
    const cursor = { ...codexRuntime(), id: 'cursor' as const, displayName: 'Cursor Agent', optional: true }
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      // Where Cursor's sandbox exists, so a read-only mission is allowed to
      // start; the refusal on other platforms has its own test.
      platform: 'darwin',
      discover: async () => [cursor],
      runner: { start },
      ledger: fakeLedger({ createMission, appendEvents }),
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: (task) => scheduledTasks.push(task)
    })
    const scheduledTasks: Array<() => void> = []
    const updates: CodexMissionUpdate[] = []

    const response = await service.start('Do work.', 'cursor', 'ask', {}, (update) => {
      updates.push(update)
    })
    expect(response).toMatchObject({ ok: true, data: { runtime: 'cursor', sandbox: 'read-only' } })
    const spec = start.mock.calls[0]?.[0]
    expect(spec?.runtime).toBe('cursor')
    expect(spec?.args).toEqual(expect.arrayContaining(['--print', '--trust', '--mode', 'ask', '--sandbox', 'enabled']))
    expect(spec?.args).not.toContain('--force')
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({ runtime: 'cursor' }))

    scheduledTasks[0]?.()
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.completed')).toBe(true)
    })
    const recorded = appendEvents.mock.calls.flatMap(([, events]) => events)
    expect(recorded.map(({ type }) => type)).toEqual(['run.started', 'message.delta', 'run.completed', 'adapter.diagnostic'])
    expect((recorded.at(-1)?.payload as { code?: string }).code).toBe('host.start-timing')
    expect(recorded.every((event) => event.sourceAdapter === 'cursor')).toBe(true)
    expect(recorded[0]?.runtimeThreadId).toBe('cursor-session-1')
  })

  it('sends a Copilot teammate the effort it was given (the 2026-10-10 sweep: neither route sent it)', async () => {
    const { service, start } = serviceWith([{ ...codexRuntime(), id: 'copilot', displayName: 'Copilot CLI', optional: true }])
    await service.start('Say hi.', 'copilot', 'ask', { effort: 'high' }, () => undefined)
    const spec = start.mock.calls[0]?.[0]
    const at = spec?.args.indexOf('--effort') ?? -1
    expect(spec?.args.slice(at, at + 2)).toEqual(['--effort', 'high'])
  })

  it('gives every Copilot run Locust’s command guard as a plugin, and no other runtime that plugin (0.721)', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({ records: records([]), completion: Promise.resolve(completion()) })) satisfies RuntimeProcessRunner['start']
    const guard = vi.fn(async () => 'C:\\Users\\Jane\\AppData\\Roaming\\Locust\\copilot-command-guard')
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [{ ...codexRuntime(), id: 'copilot', displayName: 'Copilot CLI', optional: true }, { ...codexRuntime(), id: 'cursor', displayName: 'Cursor Agent', optional: true }],
      runner: { start },
      ledger: fakeLedger(),
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: () => undefined,
      copilotCommandGuard: guard
    })
    await service.start('Say hi.', 'copilot', 'ask', {}, () => undefined)
    const copilot = start.mock.calls[0]?.[0]
    const at = copilot?.args.indexOf('--plugin-dir') ?? -1
    expect(copilot?.args.slice(at, at + 2)).toEqual(['--plugin-dir', 'C:\\Users\\Jane\\AppData\\Roaming\\Locust\\copilot-command-guard'])
    await service.start('Say hi.', 'cursor', 'ask', {}, () => undefined)
    expect(start.mock.calls[1]?.[0]?.args).not.toContain('C:\\Users\\Jane\\AppData\\Roaming\\Locust\\copilot-command-guard')
    expect(guard).toHaveBeenCalledTimes(1)
  })

  it('starts none of the MCP servers in the person’s Copilot settings in Ask, and asks for them only there (0.824)', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({ records: records([]), completion: Promise.resolve(completion()) })) satisfies RuntimeProcessRunner['start']
    const servers = vi.fn(async () => ['notes', 'robinhood-trading'] as const)
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [{ ...codexRuntime(), id: 'copilot', displayName: 'Copilot CLI', optional: true }],
      runner: { start },
      ledger: fakeLedger(),
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: () => undefined,
      copilotMcpServers: servers
    })
    await service.start('Say hi.', 'copilot', 'ask', {}, () => undefined)
    expect(start.mock.calls[0]?.[0]?.args.join(' ')).toContain('--disable-mcp-server notes --disable-mcp-server robinhood-trading')
    await service.start('Change it.', 'copilot', 'accept-edits', {}, () => undefined)
    expect(start.mock.calls[1]?.[0]?.args).not.toContain('--disable-mcp-server')
    expect(servers).toHaveBeenCalledTimes(1)
  })

  it('lets a Cursor mission edit when asked, and never forces its commands', async () => {
    const { service, start } = serviceWith([{ ...codexRuntime(), id: 'cursor', displayName: 'Cursor Agent', optional: true }])
    await service.start('Do work.', 'cursor', 'accept-edits', {}, () => undefined)
    const spec = start.mock.calls[0]?.[0]
    expect(spec?.args).not.toContain('--mode')
    expect(spec?.args).not.toContain('--force')
  })

  it('records nothing when the chosen options cannot be turned into a command', async () => {
    // Cursor takes no effort level, so the builder refuses this argv. The
    // mission file used to be written first, leaving a permanent record of a
    // run that never existed -- and nothing created today can be pruned
    // today, so it could not be cleared either.
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const appendHostFailure = vi.fn<MissionLedger['appendHostFailure']>(async () => undefined)
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      platform: 'darwin',
      discover: async () => [{ ...codexRuntime(), id: 'cursor', displayName: 'Cursor Agent', optional: true }],
      runner: { start },
      ledger: fakeLedger({ createMission, appendHostFailure })
    })

    await expect(
      service.start('Do work.', 'cursor', 'ask', { effort: 'high' }, () => undefined)
    ).resolves.toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED' } })
    expect(start).not.toHaveBeenCalled()
    expect(createMission).not.toHaveBeenCalled()
    expect(appendHostFailure).not.toHaveBeenCalled()
  })

  it('runs a read-only Cursor mission on Windows in its ask mode, without the sandbox Windows cannot run', async () => {
    // 0.485: ask mode held against three pushed writes on Windows (2026-09-30),
    // where plan mode had not (2026-09-02). Refused until then.
    // `--sandbox enabled` is still refused outright on Windows, so it is not asked for.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      platform: 'win32',
      discover: async () => [{ ...codexRuntime(), id: 'cursor', displayName: 'Cursor Agent', optional: true }],
      runner: { start },
      ledger: fakeLedger(),
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: () => undefined
    })

    await expect(service.start('Do work.', 'cursor', 'ask', {}, () => undefined)).resolves.toMatchObject({ ok: true })
    expect(start.mock.calls[0]?.[0]?.args).toEqual(expect.arrayContaining(['--mode', 'ask']))
    expect(start.mock.calls[0]?.[0]?.args).not.toContain('--sandbox')
    expect(start.mock.calls[0]?.[0]?.args).not.toContain('plan')
  })

  it('runs a read-only Cursor mission where the sandbox is real, and asks for it', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      platform: 'darwin',
      discover: async () => [{ ...codexRuntime(), id: 'cursor', displayName: 'Cursor Agent', optional: true }],
      runner: { start },
      ledger: fakeLedger(),
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: () => undefined
    })

    await expect(service.start('Do work.', 'cursor', 'ask', {}, () => undefined)).resolves.toMatchObject({ ok: true })
    expect(start.mock.calls[0]?.[0]?.args).toEqual(expect.arrayContaining(['--mode', 'ask', '--sandbox', 'enabled']))
  })

  it('names the runtime the user actually chose when it is unavailable', async () => {
    const { service } = serviceWith([codexRuntime()])

    await expect(service.start('Do work.', 'claude', 'ask', {}, () => undefined)).resolves.toMatchObject({
      ok: false,
      error: { code: 'CODEX_UNAVAILABLE', message: expect.stringContaining('Claude Code') }
    })
  })
})

describe('durable write batching', () => {
  it('collapses a burst of records into one durable append', async () => {
    // Each append costs an fsync. Paying it per provider record lets the
    // runner's bounded queue fill while the consumer waits, and a full queue
    // ends the run as an output-limit breach -- a verbose mission killed for
    // being verbose.
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async () => undefined)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: burst([
        { type: 'thread.started', thread_id: 'thread-live' },
        { type: 'turn.started' },
        { type: 'item.completed', item: { id: 'a', type: 'agent_message', text: 'one' } },
        { type: 'turn.completed' }
      ]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, fakeLedger({ appendEvents }))

    await service.start('Do work.', 'codex', 'ask', {}, () => undefined)
    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(appendEvents.mock.calls.length).toBeGreaterThan(0)
    })

    // Four provider records, ONE durable append for the burst -- then a second
    // for the terminal event that finish() produces.
    expect(appendEvents.mock.calls[0]?.[1].map((event) => event.type)).toEqual([
      'run.started',
      'step.started',
      'message.delta',
      'step.completed'
    ])
    await vi.waitFor(() => {
      expect(appendEvents.mock.calls).toHaveLength(3)
    })
    expect(appendEvents.mock.calls[1]?.[1].map((event) => event.type)).toEqual(['run.completed'])
    // The third append is the host's start-timing note, after the run's own events (0.602).
    expect(appendEvents.mock.calls[2]?.[1].map((event) => (event.payload as { code?: string }).code)).toEqual(['host.start-timing'])
  })

  it('still persists every event before any of them is emitted', async () => {
    const order: string[] = []
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async (_id, events) => {
      order.push(`persist:${events.length}`)
    })
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: burst([
        { type: 'thread.started', thread_id: 't' },
        { type: 'item.completed', item: { id: 'a', type: 'agent_message', text: 'one' } }
      ]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, fakeLedger({ appendEvents }))

    await service.start('Do work.', 'codex', 'ask', {}, (update) => {
      if (update.kind === 'event') order.push('emit')
    })
    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(order).toContain('emit')
    })

    // Batching must not reorder the guarantee: the write lands first.
    expect(order[0]).toMatch(/^persist:/)
  })
})

describe('model selection', () => {
  function harness() {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service } = scheduledService({ start }, fakeLedger({ createMission }))
    return { start, createMission, service }
  }

  it('sends a chosen model to the runtime and records it', async () => {
    const { start, createMission, service } = harness()
    const response = await service.start('Do work.', 'codex', 'ask', { model: 'gpt-5.6-luna' }, () => undefined)

    expect(start.mock.calls[0]?.[0]?.args).toEqual(expect.arrayContaining(['--model', 'gpt-5.6-luna']))
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({ model: 'gpt-5.6-luna' }))
    expect(response).toMatchObject({ ok: true, data: { model: 'gpt-5.6-luna' } })
  })

  it('treats account-default as "send no model", not as a model id', async () => {
    // It is the shell's word for the absence of a choice. Passing it through
    // would make the CLI look for a model that does not exist.
    const { start, createMission, service } = harness()
    await service.start('Do work.', 'codex', 'ask', { model: 'account-default' }, () => undefined)

    expect(start.mock.calls[0]?.[0]?.args).not.toContain('--model')
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({ model: 'account-default' }))
  })

  it('sends no model when none was chosen', async () => {
    const { start, service } = harness()
    await service.start('Do work.', 'codex', 'ask', {}, () => undefined)
    expect(start.mock.calls[0]?.[0]?.args).not.toContain('--model')
  })
})

describe('mid-mission handoff', () => {
  function checkpoint(overrides: Record<string, unknown> = {}) {
    return {
      schemaVersion: 1,
      missionId: 'mission_2',
      runId: 'run_1',
      epoch: 3,
      reason: 'route-switch',
      reconciledThroughSequence: 9,
      settledActions: [],
      unsettledActions: [],
      resumeSafety: 'safe',
      safetyReason: 'Nothing was in flight.',
      transcriptDigest: 'digest',
      assistantSummary: 'Renamed the parser module.',
      createdAt: NOW,
      ...overrides
    } as Awaited<ReturnType<MissionLedger['createCheckpoint']>>
  }

  function handoffClaudeRuntime(): RuntimeDiscovery {
    return {
      ...codexRuntime(),
      id: 'claude',
      displayName: 'Claude Code',
      executable: {
        commandName: 'claude',
        discoveredPath: process.platform === 'win32' ? 'C:\\tools\\claude.exe' : '/tools/claude',
        executablePath: process.platform === 'win32' ? 'C:\\tools\\claude.exe' : '/tools/claude',
        prefixArgs: [],
        kind: 'native'
      }
    }
  }

  function liveService(
    ledger: MissionLedger,
    runtimes: RuntimeDiscovery[] = [codexRuntime(), handoffClaudeRuntime()],
    extra: Partial<Parameters<typeof createCodexMissionService>[0]> = {}
  ) {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed', usage: { output_tokens: 1 } }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => runtimes,
      runner: { start },
      ledger,
      createId: (() => { let n = 0; return () => String(++n) })(),
      now: () => new Date(NOW),
      schedule: (task) => { setImmediate(task) },
      ...extra
    })
    return { start, service }
  }

  it('starts a NEW mission that records what it continues from', async () => {
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () => checkpoint())
    const { service, start } = liveService(fakeLedger({ createMission, createCheckpoint }))

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    const response = await service.handOff('run_1', 'claude', 'ask', {}, () => undefined)

    expect(response).toMatchObject({
      ok: true,
      data: {
        runtime: 'claude',
        continuesFrom: { missionId: 'mission_2', checkpointEpoch: 3 },
        resumeSafety: 'safe'
      }
    })
    // A mission records ONE runtime, so the continuation is a separate mission
    // pointing back at the checkpoint -- not the first mission mutated.
    expect(createMission).toHaveBeenCalledTimes(2)
    expect(createMission.mock.calls[1]?.[0]).toMatchObject({
      runtime: 'claude',
      continuesFrom: { missionId: 'mission_2', checkpointEpoch: 3, reason: 'route-switch' }
    })
    expect(createMission.mock.calls[0]?.[0].continuesFrom).toBeUndefined()
    // The new runtime is launched with the briefing, not the bare original.
    expect(start.mock.calls[1]?.[1]).toContain('Refactor the parser.')
    expect(start.mock.calls[1]?.[1]).toContain('Renamed the parser module.')
  })

  it('reconciles only after the stopped run has settled', async () => {
    const order: string[] = []
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () => {
      order.push('checkpoint')
      return checkpoint()
    })
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async () => {
      order.push('append')
    })
    const { service } = liveService(fakeLedger({ createCheckpoint, appendEvents }))

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    await service.handOff('run_1', 'claude', 'ask', {}, () => undefined)

    // Checkpointing a still-draining mission would report actions as unsettled
    // that were about to report back, and the briefing would then tell the next
    // runtime to re-verify work that had already finished.
    expect(order.indexOf('append')).toBeGreaterThanOrEqual(0)
    expect(order.indexOf('checkpoint')).toBeGreaterThan(order.indexOf('append'))
  })

  it('refuses a handoff to the runtime already running it, without stopping anything', async () => {
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () => checkpoint())
    const { service } = liveService(fakeLedger({ createCheckpoint }))

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    const response = await service.handOff('run_1', 'codex', 'ask', {}, () => undefined)

    expect(response).toMatchObject({ ok: false, error: { code: 'HANDOFF_REFUSED' } })
    expect(response.ok === false && response.error.message).toContain('Nothing was changed')
    // Stopping a run to restart it on the same route costs progress and buys
    // nothing, so the run must still be running.
    expect(createCheckpoint).not.toHaveBeenCalled()
  })

  it('refuses when the ledger cannot be reconciled, and says the run is stopped', async () => {
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () =>
      checkpoint({ resumeSafety: 'unsafe', safetyReason: 'The ledger stops at sequence 4.' })
    )
    const { service } = liveService(fakeLedger({ createCheckpoint }))

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    const response = await service.handOff('run_1', 'claude', 'ask', {}, () => undefined)

    expect(response).toMatchObject({ ok: false, error: { code: 'HANDOFF_REFUSED' } })
    // The stop is real and cannot be undone, so the message must not imply the
    // mission is still running.
    expect(response.ok === false && response.error.message).toContain('was stopped')
    expect(response.ok === false && response.error.message).toContain('The ledger stops at sequence 4.')
  })

  it('says both things when the stop worked and the new run did not start', async () => {
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () => checkpoint())
    // The continuation's record cannot be written -- a failure nothing could
    // know before the run was stopped (a signed-out target is refused first).
    let missions = 0
    const createMission = vi.fn<MissionLedger['createMission']>(async () => {
      missions += 1
      if (missions > 1) throw new Error('disk full')
    })
    const { service } = liveService(fakeLedger({ createCheckpoint, createMission }))

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    const response = await service.handOff('run_1', 'claude', 'ask', {}, () => undefined)

    expect(response.ok).toBe(false)
    expect(response.ok === false && response.error.message).toContain('was stopped for the handoff')
    expect(createCheckpoint).toHaveBeenCalledTimes(1)
  })

  /*
   * M9 (the code review): a refusal the target runtime would give no matter
   * what the run did is asked BEFORE the run is stopped. It used to be asked
   * after, by the new start, so a handoff to Cursor read-only on Windows, to
   * a runtime Locust cannot read, or to one that is signed out stopped the
   * run and started nothing: the work in flight was lost for a refusal that
   * was known before anything was touched.
   */
  function handoffRuntime(id: 'cursor' | 'gemini', displayName: string): RuntimeDiscovery {
    return {
      ...codexRuntime(),
      id,
      displayName,
      executable: {
        commandName: id,
        discoveredPath: process.platform === 'win32' ? `C:${String.fromCharCode(92)}tools${String.fromCharCode(92)}${id}.exe` : `/tools/${id}`,
        executablePath: process.platform === 'win32' ? `C:${String.fromCharCode(92)}tools${String.fromCharCode(92)}${id}.exe` : `/tools/${id}`,
        prefixArgs: [],
        kind: 'native'
      }
    }
  }

  // Cursor read-only on Windows left this list in 0.485: its ask mode holds.
  it.each([
    ['a runtime whose events Locust cannot read', 'gemini', 'accept-edits', /cannot read its event stream/],
    ['a runtime that is signed out', 'claude', 'ask', /is signed out/]
  ] as const)('refuses a handoff to %s before stopping anything', async (_what, runtime, mode, reason) => {
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () => checkpoint())
    const runtimes = [
      codexRuntime(),
      { ...handoffClaudeRuntime(), readiness: 'authentication-required' as const },
      handoffRuntime('cursor', 'Cursor Agent'),
      handoffRuntime('gemini', 'Gemini CLI')
    ]
    let release!: () => void
    const held = new Promise<void>((resolve) => { release = resolve })
    // A run still working: it ends when released or when it is stopped.
    const start = vi.fn((_spec, _prompt, options): RuntimeProcessRun => {
      options?.signal?.addEventListener('abort', () => { release() })
      return {
        records: { async *[Symbol.asyncIterator]() { await held }, drainAvailable: () => [] },
        completion: held.then(() => completion())
      }
    }) satisfies RuntimeProcessRunner['start']
    const { service } = liveService(fakeLedger({ createCheckpoint }), runtimes, { platform: 'win32', runner: { start } })

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    const response = await service.handOff('run_1', runtime, mode, {}, () => undefined)

    expect(response).toMatchObject({ ok: false, error: { code: 'HANDOFF_REFUSED' } })
    expect(response.ok === false && response.error.message).toMatch(reason)
    expect(response.ok === false && response.error.message).toContain('Nothing was stopped')
    expect(response.ok === false && response.error.untouched).toBe(true)
    expect(createCheckpoint).not.toHaveBeenCalled()
    expect(service.liveMissionIds()).toEqual(['mission_2'])
    release()
  })

  it.each(['ask', 'plan'] as const)('hands off to Cursor read-only on Windows too, in %s (0.485: its ask mode holds)', async (mode) => {
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () => checkpoint())
    const { service } = liveService(fakeLedger({ createCheckpoint }), [codexRuntime(), handoffRuntime('cursor', 'Cursor Agent')], { platform: 'win32' })

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    const response = await service.handOff('run_1', 'cursor', mode, {}, () => undefined)

    expect(response).toMatchObject({ ok: true, data: { runtime: 'cursor' } })
  })

  it('still hands off to Cursor on Windows when the run may edit', async () => {
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async () => checkpoint())
    const { service } = liveService(fakeLedger({ createCheckpoint }), [codexRuntime(), handoffRuntime('cursor', 'Cursor Agent')], { platform: 'win32' })

    await service.start('Refactor the parser.', 'codex', 'ask', {}, () => undefined)
    const response = await service.handOff('run_1', 'cursor', 'accept-edits', {}, () => undefined)

    expect(response).toMatchObject({ ok: true, data: { runtime: 'cursor' } })
    expect(createCheckpoint).toHaveBeenCalledTimes(1)
  })

  it('refuses when there is no active run to hand off', async () => {
    const { service } = liveService(fakeLedger())

    const response = await service.handOff('run_nope', 'claude', 'ask', {}, () => undefined)

    expect(response).toMatchObject({ ok: false, error: { code: 'RUN_NOT_ACTIVE' } })
  })
})

describe('the workroom around a mission', () => {
  const WREN = { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }
  const ATLAS = { teammateId: 'tm_atlas', name: 'Atlas', role: 'Research & Briefs' }
  const PEER: MissionPeerContext = { self: WREN, others: [ATLAS] }

  function waiting(text: string, messageId = 'wm_1'): WorkroomMessage {
    return {
      messageId,
      sequence: 1,
      from: { teammateId: 'tm_atlas', name: 'Atlas', missionId: 'mission_a' },
      to: { teammateId: 'tm_wren', name: 'Wren' },
      text,
      postedAt: NOW
    }
  }

  function fakeWorkroom(overrides: Partial<Workroom> = {}) {
    const posted: { from: unknown; to: unknown; text: string }[] = []
    const delivered: { messageIds: readonly string[]; missionId: string }[] = []
    let nextId = 0
    const workroom: Workroom = {
      post: async (input) => {
        posted.push(input)
        return {
          messageId: `wm_out_${++nextId}`,
          sequence: nextId,
          from: input.from,
          to: input.to,
          text: input.text,
          postedAt: NOW
        }
      },
      unread: async () => ({ messages: [], remaining: 0 }),
      markDelivered: async (messageIds, missionId) => {
        delivered.push({ messageIds, missionId })
      },
      read: async () => ({ messages: [], deliveries: [], issues: [] }),
      flush: async () => undefined,
      ...overrides
    }
    return { workroom, posted, delivered }
  }

  function transcript(finalText: string): RuntimeProcessRun {
    return {
      records: records([
        { type: 'thread.started', thread_id: 'thread-live' },
        { type: 'turn.started' },
        { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: finalText } },
        { type: 'turn.completed', usage: { output_tokens: 2 } }
      ]),
      completion: Promise.resolve(completion())
    }
  }

  function peerService(input: {
    readonly run: RuntimeProcessRun
    readonly workroom: Workroom
    readonly ledger?: Partial<MissionLedger>
    readonly runtime?: RuntimeDiscovery
    readonly openCodeGitReads?: () => Promise<GitReadsSpec | undefined>
  }) {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => input.run)
    const links: { missionId: string; links: readonly MissionPeerLink[] }[] = []
    const ledger = fakeLedger({
      appendPeerLinks: async (missionId, appended) => {
        links.push({ missionId, links: appended })
      },
      ...input.ledger
    })
    const scheduled: Array<() => void> = []
    let nextId = 0
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [input.runtime ?? codexRuntime()],
      runner: { start },
      ledger,
      workroom: input.workroom,
      createId: () => String(++nextId),
      now: () => new Date(NOW),
      schedule: (task) => scheduled.push(task),
      ...(input.openCodeGitReads === undefined ? {} : { openCodeGitReads: input.openCodeGitReads })
    })
    return { service, start, links, scheduled }
  }

  async function drain(scheduled: Array<() => void>): Promise<void> {
    while (scheduled.length > 0) scheduled.shift()!()
    // The consume loop awaits several durable writes; let them all settle.
    for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))
  }

  it('quotes a waiting message into the prompt as a claim, records it, and marks it delivered only once the runtime starts', async () => {
    const { workroom, delivered } = fakeWorkroom({
      unread: async (teammateId) =>
        teammateId === 'tm_wren' ? { messages: [waiting('pnpm check runs everything.')], remaining: 0 } : { messages: [], remaining: 0 }
    })
    const { service, start, links, scheduled } = peerService({ run: transcript('Done.'), workroom })

    const response = await service.start('Which command runs the checks?', 'codex', 'ask', {}, () => undefined, undefined, PEER)

    expect(response.ok).toBe(true)
    if (!response.ok) return
    const sentPrompt = start.mock.calls[0]?.[1] as string
    // LAST, not first. The brief leads with the standing things -- roster,
    // memory, block formats -- and ends with what was actually asked, so the
    // question is the nearest thing to the answer and the stable prefix can
    // be cached. `endsWith` is the same claim the old `startsWith` made:
    // the person's words go in whole and are not rewritten.
    expect(sentPrompt.endsWith('Which command runs the checks?')).toBe(true)
    expect(sentPrompt).toContain('CLAIMS from other agents')
    expect(sentPrompt).toContain('pnpm check runs everything.')
    expect(sentPrompt).toContain('<locust-share to="Atlas">')
    expect(links).toEqual([
      {
        missionId: 'mission_2',
        links: [{ direction: 'received', messageId: 'wm_1', peerTeammateId: 'tm_atlas', occurredAt: NOW }]
      }
    ])
    expect(delivered).toEqual([])
    await drain(scheduled)
    expect(delivered).toEqual([{ messageIds: ['wm_1'], missionId: 'mission_2' }])
    expect(response.data.peerMessages).toEqual([
      {
        messageId: 'wm_1',
        direction: 'received',
        from: { teammateId: 'tm_atlas', name: 'Atlas', missionId: 'mission_a' },
        to: { teammateId: 'tm_wren', name: 'Wren' },
        text: 'pnpm check runs everything.',
        at: NOW
      }
    ])
    expect(response.data.peerDeliveryFailed).toBe(false)
  })

  it('plans in the plan MODE, and only where the sandbox already refuses writes', async () => {
    const { workroom } = fakeWorkroom()
    const planning = peerService({ run: transcript('Here is the plan.'), workroom })
    const planned = await planning.service.start(
      'Add retries to the fetch helper.',
      'codex',
      'plan',
      {},
      () => undefined,
      undefined,
      PEER
    )
    expect(planned.ok).toBe(true)
    // The invariant Plan-as-a-mode rests on: choosing it CANNOT leave the run
    // able to write. The sandbox is what the runtime is actually launched
    // with, so it is asserted here rather than trusted from the mode name.
    expect(planned.ok && planned.data.sandbox).toBe('read-only')
    const withPlan = planning.start.mock.calls[0]?.[1] as string
    /*
     * The brief now leads with the standing things and ends with what was
     * asked -- except here, where PLAN FIRST is deliberately appended after
     * everything ("so it is the instruction closest to the model's answer",
     * codex-mission.ts). That is the same reasoning and it outranks: a
     * read-only promise the app has to keep beats a general ordering rule.
     *
     * So the person's words are LAST OF THE BRIEF, with only the plan
     * instruction after them -- and still whole and unaltered, which is the
     * claim the old `startsWith` was really making.
     */
    expect(withPlan).toContain('Add retries to the fetch helper.')
    expect(withPlan.endsWith(planSection())).toBe(true)
    expect(withPlan.indexOf('Add retries to the fetch helper.')).toBeGreaterThan(withPlan.indexOf('<locust-ask>'))
    expect(withPlan).toContain('PLAN FIRST')
    expect(withPlan).toContain('numbered steps')

    // Accept edits CAN change the workspace. Plan is a mode now, so the two
    // cannot be chosen together at all -- and the host still checks the
    // sandbox rather than trusting the mode name it was handed.
    const editing = peerService({ run: transcript('Done.'), workroom })
    const edited = await editing.service.start(
      'Add retries to the fetch helper.',
      'codex',
      'accept-edits',
      {},
      () => undefined,
      undefined,
      PEER
    )
    expect(edited.ok).toBe(true)
    expect(editing.start.mock.calls[0]?.[1] as string).not.toContain('PLAN FIRST')

    // And an ordinary read-only run is unchanged.
    const plain = peerService({ run: transcript('Done.'), workroom })
    await plain.service.start('Add retries to the fetch helper.', 'codex', 'ask', {}, () => undefined, undefined, PEER)
    expect(plain.start.mock.calls[0]?.[1] as string).not.toContain('PLAN FIRST')
  })

  it('tells a read-only OpenCode run it has no shell, and no other run (A2.20)', async () => {
    /*
     * In Ask or Plan mode OpenCode rejects every shell call and the run ENDS
     * there; the free models reach for `cat` first. The run is read-only
     * either way -- the line is what lets it finish (workroom-briefing.ts).
     */
    const { workroom } = fakeWorkroom()
    const codex = codexRuntime()
    const opencode: RuntimeDiscovery = {
      ...codex,
      id: 'opencode',
      displayName: 'OpenCode',
      executable: { ...codex.executable!, commandName: 'opencode' }
    }
    const sent = async (runtime: RuntimeDiscovery, mode: 'ask' | 'plan' | 'accept-edits'): Promise<string> => {
      const one = peerService({ run: transcript('Done.'), workroom, runtime })
      const started = await one.service.start(
        'What does LOCUST.md say?',
        runtime.id as 'codex' | 'opencode',
        mode,
        runtime.id === 'opencode' ? { model: 'opencode/muse-spark-1.3-contributor-free' } : {},
        () => undefined,
        undefined,
        PEER
      )
      expect(started.ok).toBe(true)
      return one.start.mock.calls[0]?.[1] as string
    }

    expect(await sent(opencode, 'ask')).toContain(openCodeReadOnlySection())
    // Plan mode too -- and the plan instruction still comes last.
    const planned = await sent(opencode, 'plan')
    expect(planned).toContain(openCodeReadOnlySection())
    expect(planned.endsWith(planSection())).toBe(true)
    // A run that may edit has its shell; a read-only Codex run can run
    // read-only commands in its sandbox, so neither is told otherwise.
    expect(await sent(opencode, 'accept-edits')).not.toContain(openCodeReadOnlySection())
    expect(await sent(codex, 'ask')).not.toContain(openCodeReadOnlySection())
  })

  it('gives a read-only OpenCode run Locust’s git tool, told of in OpenCode’s own instructions rather than the prompt (0.818)', async () => {
    /*
     * Ask's shell refuses every command again (commands.ts: OpenCode matched `git status*` with the redirect taken
     * off, so `git status > status.txt` wrote the file). Git comes through Locust's git tool, and OpenCode's
     * `instructions` say so to every agent of the run, a /review subagent included.
     */
    const { workroom } = fakeWorkroom()
    const codex = codexRuntime()
    const opencode: RuntimeDiscovery = { ...codex, id: 'opencode', displayName: 'OpenCode', executable: { ...codex.executable!, commandName: 'opencode' } }
    const gitReads: GitReadsSpec = { command: 'C:\\Locust\\Locust.exe', args: ['C:\\Locust\\resources\\locust-git-reads.mjs'], env: { ELECTRON_RUN_AS_NODE: '1' }, instructions: 'C:\\profile\\opencode-git-tool.md' }
    const launched = async (mode: 'ask' | 'plan' | 'accept-edits') => {
      const one = peerService({ run: transcript('Done.'), workroom, runtime: opencode, openCodeGitReads: async () => gitReads })
      expect((await one.service.start('What changed?', 'opencode', mode, { model: 'opencode/deepseek-v4.1-flash-free' }, () => undefined, undefined, PEER)).ok).toBe(true)
      const spec = one.start.mock.calls[0]![0] as RuntimeCommandSpec
      const prompt = one.start.mock.calls[0]![1] as string
      return { spec, config: JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? '{}') as { mcp?: Record<string, { environment: Record<string, string> }>; permission: Record<string, unknown>; instructions?: string[] }, prompt }
    }
    for (const mode of ['ask', 'plan'] as const) {
      const run = await launched(mode)
      expect(run.config.mcp?.locust_git?.environment.LOCUST_GIT_FOLDER, mode).toBe(run.spec.cwd)
      expect(run.config.permission.bash, mode).toBe('ask')
      expect(run.config.instructions, mode).toContain(gitReads.instructions)
      // Said once, where every agent reads it: not again in the prompt.
      expect(run.prompt, mode).not.toContain(openCodeReadOnlySection())
    }
    // A run that may edit has a shell that runs git: no git tool beside it.
    expect((await launched('accept-edits')).config.mcp?.locust_git).toBeUndefined()
  })

  it('keeps the person\'s own words as the recorded prompt, not the briefing', async () => {
    const { workroom } = fakeWorkroom()
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service } = peerService({ run: transcript('Done.'), workroom, ledger: { createMission } })

    await service.start('Look around.', 'codex', 'ask', {}, () => undefined, undefined, PEER)

    expect(createMission.mock.calls[0]?.[0]?.prompt).toBe('Look around.')
  })

  it('refuses to run on messages the ledger cannot record, and says so', async () => {
    const { workroom, delivered } = fakeWorkroom({
      unread: async () => ({ messages: [waiting('claim')], remaining: 0 })
    })
    const { service, start } = peerService({
      run: transcript('Done.'),
      workroom,
      ledger: { appendPeerLinks: async () => { throw new Error('disk full') } }
    })

    const response = await service.start('Task.', 'codex', 'ask', {}, () => undefined, undefined, PEER)

    expect(response).toEqual({
      ok: false,
      error: {
        code: 'PERSISTENCE_FAILED',
        message: 'The messages this mission was shown could not be recorded in the durable local ledger.'
      }
    })
    expect(start).not.toHaveBeenCalled()
    // Nothing was consumed: the message waits for a mission that can record it.
    expect(delivered).toEqual([])
  })

  it('runs without the channel when it cannot be read, and reports that on the receipt', async () => {
    const { workroom } = fakeWorkroom({ unread: async () => { throw new Error('channel damaged') } })
    const { service, start } = peerService({ run: transcript('Done.'), workroom })

    const response = await service.start('Task.', 'codex', 'ask', {}, () => undefined, undefined, PEER)

    expect(response.ok).toBe(true)
    if (!response.ok) return
    expect(response.data.peerDeliveryFailed).toBe(true)
    expect(response.data.peerMessages).toEqual([])
    // The share form still goes: the run can post even when it was shown nothing.
    expect(start.mock.calls[0]?.[1]).toContain('<locust-share to="Atlas">')
  })

  it('posts a share block to the named teammate, attributed to this mission, and links it', async () => {
    const { workroom, posted } = fakeWorkroom()
    const updates: CodexMissionUpdate[] = []
    const { service, links, scheduled } = peerService({
      run: transcript('The gate is pnpm check.\n\n<locust-share to="Atlas">\npnpm check runs build, typecheck and tests.\n</locust-share>'),
      workroom
    })

    const response = await service.start('Find the check command.', 'codex', 'ask', {}, (update) => updates.push(update), undefined, PEER)
    expect(response.ok).toBe(true)
    await drain(scheduled)

    expect(posted).toEqual([
      {
        from: { teammateId: 'tm_wren', name: 'Wren', missionId: 'mission_2' },
        to: { teammateId: 'tm_atlas', name: 'Atlas' },
        text: 'pnpm check runs build, typecheck and tests.'
      }
    ])
    expect(links).toEqual([
      {
        missionId: 'mission_2',
        links: [{ direction: 'posted', messageId: 'wm_out_1', peerTeammateId: 'tm_atlas', occurredAt: NOW }]
      }
    ])
    const peerUpdates = updates.filter((update) => update.kind === 'peer-message')
    expect(peerUpdates).toEqual([
      {
        kind: 'peer-message',
        runId: 'run_1',
        missionId: 'mission_2',
        message: {
          messageId: 'wm_out_1',
          direction: 'posted',
          from: { teammateId: 'tm_wren', name: 'Wren', missionId: 'mission_2' },
          to: { teammateId: 'tm_atlas', name: 'Atlas' },
          text: 'pnpm check runs build, typecheck and tests.',
          at: NOW
        }
      }
    ])
  })

  it('refuses a share addressed to someone who is not on the roster, out loud', async () => {
    const { workroom, posted } = fakeWorkroom()
    const updates: CodexMissionUpdate[] = []
    const { service, scheduled } = peerService({
      run: transcript('<locust-share to="Mallory">\nsecret\n</locust-share>'),
      workroom
    })

    await service.start('Task.', 'codex', 'ask', {}, (update) => updates.push(update), undefined, PEER)
    await drain(scheduled)

    expect(posted).toEqual([])
    expect(updates.filter((update) => update.kind === 'peer-share-failed')).toEqual([
      {
        kind: 'peer-share-failed',
        runId: 'run_1',
        missionId: 'mission_2',
        message: 'Wren addressed a message to "Mallory", who is not on the roster. Nothing was sent.'
      }
    ])
  })

  it('shares nothing from a run that did not complete', async () => {
    const { workroom, posted } = fakeWorkroom()
    const run: RuntimeProcessRun = {
      records: records([
        { type: 'thread.started', thread_id: 'thread-live' },
        { type: 'turn.started' },
        { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: '<locust-share to="Atlas">\nhalf a claim\n</locust-share>' } }
      ]),
      completion: Promise.resolve(completion({ exitCode: 1 }))
    }
    const { service, scheduled } = peerService({ run, workroom })

    await service.start('Task.', 'codex', 'ask', {}, () => undefined, undefined, PEER)
    await drain(scheduled)

    expect(posted).toEqual([])
  })

  it('shares nothing from a mission that belongs to nobody', async () => {
    const { workroom, posted } = fakeWorkroom()
    const { service, start, scheduled } = peerService({
      run: transcript('<locust-share to="Atlas">\nfinding\n</locust-share>'),
      workroom
    })

    await service.start('Task.', 'codex', 'ask', {}, () => undefined)
    await drain(scheduled)

    expect(posted).toEqual([])
    /*
     * It is briefed, and it is not briefed as anybody.
     *
     * This asserted the prompt was the person's words and NOTHING else,
     * which is what the app did and what Grok's pass 14 ranked second: a run
     * started from Home with nobody picked did not know the folder and could
     * not quote a memory the person had just typed. It gets the standing
     * formats and (where one exists) the folder and the project's memory;
     * what it must never get is a roster, a role or a share block, because
     * there is nobody to be and nobody to write to.
     */
    const briefed = start.mock.calls[0]?.[1] as string
    expect(briefed.endsWith('Task.')).toBe(true)
    expect(briefed).toContain('<locust-ask>')
    expect(briefed).not.toContain('locust-share')
    expect(briefed).not.toContain('Teammates in this workspace besides you')
  })
})

describe('missions side by side', () => {
  const ATLAS = { self: { teammateId: 'tm_atlas', name: 'Atlas', role: 'Research & Briefs' }, others: [] }
  const WREN = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [] }
  const NOVA = { self: { teammateId: 'tm_nova', name: 'Nova', role: 'Docs & QA' }, others: [] }
  const KAI = { self: { teammateId: 'tm_kai', name: 'Kai', role: 'Custom' }, others: [] }
  const ORION = { self: { teammateId: 'tm_orion', name: 'Orion', role: 'Custom' }, others: [] }

  /** A run that lasts until its signal aborts, so several can be live at once. */
  function openEnded() {
    const signals: AbortSignal[] = []
    const start = vi.fn((_spec, _prompt, options): RuntimeProcessRun => {
      const signal = options?.signal
      if (signal) signals.push(signal)
      let release!: () => void
      const released = new Promise<void>((resolve) => {
        release = resolve
      })
      let finish!: (value: RuntimeProcessCompletion) => void
      const done = new Promise<RuntimeProcessCompletion>((resolve) => {
        finish = resolve
      })
      signal?.addEventListener('abort', () => {
        release()
        finish(completion({ exitCode: null, signal: 'SIGINT', recordCount: 1, cancelled: true }))
      }, { once: true })
      return {
        records: {
          async *[Symbol.asyncIterator]() {
            yield { sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread-open' }) }
            await released
          },
          drainAvailable: () => []
        },
        completion: done
      }
    }) satisfies RuntimeProcessRunner['start']
    return { start, signals }
  }

  it('runs two teammates\u2019 missions at once, and stops each by its own run id', async () => {
    const { start, signals } = openEnded()
    const { service, scheduled } = scheduledService({ start })

    const atlas = await service.start('Atlas works.', 'codex', 'ask', {}, () => undefined, undefined, ATLAS)
    const wren = await service.start('Wren works.', 'codex', 'ask', {}, () => undefined, undefined, WREN)
    expect(atlas.ok && wren.ok).toBe(true)
    if (!atlas.ok || !wren.ok) return
    expect(atlas.data.runId).not.toBe(wren.data.runId)
    while (scheduled.length > 0) scheduled.shift()!()

    expect(service.cancel(atlas.data.runId)).toMatchObject({ ok: true })
    expect(signals[0]?.aborted).toBe(true)
    // Stopping one is not stopping the other.
    expect(signals[1]?.aborted).toBe(false)
    expect(service.cancel(wren.data.runId)).toMatchObject({ ok: true })
    expect(signals[1]?.aborted).toBe(true)
  })

  it('starts a second conversation with the same teammate, and holds a reply to a turn still running (0.551)', async () => {
    const { start } = openEnded()
    const { service } = scheduledService({ start })
    const first = await service.start('Atlas works.', 'codex', 'ask', {}, () => undefined, undefined, ATLAS)
    expect((await service.start('Atlas, something else.', 'codex', 'ask', {}, () => undefined, undefined, ATLAS)).ok).toBe(true)
    await expect(
      service.start('Atlas again.', 'codex', 'ask', {}, () => undefined, undefined, ATLAS, first.ok ? first.data.missionId : undefined)
    ).resolves.toEqual({
      ok: false,
      error: { code: 'RUN_ALREADY_ACTIVE', message: 'Atlas is still answering in this conversation. Your message goes when that turn finishes.' }
    })
  })

  it('caps how many missions can be live at once, and says the number', async () => {
    const { start } = openEnded()
    const { service } = scheduledService({ start })
    /*
     * Derived from the constant, not written out.
     *
     * This said "Up to 4 missions" and filled the pool with four named
     * peers. Both broke the day the cap was measured and raised to 8 -- a
     * failing test that only meant the number moved, which is noise, and
     * which cannot tell a deliberate change from a mistaken one.
     *
     * A teammate may hold one mission, so the pool is filled with a distinct
     * teammate per slot, however many slots there are.
     */
    const slot = (i: number) => ({ self: { teammateId: `tm_slot_${String(i)}`, name: `Slot${String(i)}`, role: 'Custom' }, others: [] })
    for (let i = 0; i < MAX_LIVE_MISSIONS; i += 1) {
      await expect(service.start('Work.', 'codex', 'ask', {}, () => undefined, undefined, slot(i))).resolves.toMatchObject({ ok: true })
    }
    await expect(service.start('One too many.', 'codex', 'ask', {}, () => undefined, undefined, ORION)).resolves.toMatchObject({
      ok: false,
      error: { code: 'RUN_ALREADY_ACTIVE', message: expect.stringContaining(`Up to ${String(MAX_LIVE_MISSIONS)} missions`) }
    })
  })

  it('hands off one teammate\u2019s run without waiting for another teammate\u2019s to finish', async () => {
    const { start } = openEnded()
    const createCheckpoint = vi.fn<MissionLedger['createCheckpoint']>(async (missionId) => ({
      schemaVersion: 1,
      missionId,
      runId: 'run_x',
      epoch: 1,
      reason: 'route-switch',
      reconciledThroughSequence: 1,
      settledActions: [],
      unsettledActions: [],
      resumeSafety: 'safe',
      safetyReason: 'nothing in flight',
      transcriptDigest: 'digest',
      assistantSummary: '',
      createdAt: NOW
    }))
    const scheduledTasks: Array<() => void> = []
    let nextId = 0
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [
        codexRuntime(),
        {
          ...codexRuntime(),
          id: 'claude',
          displayName: 'Claude Code',
          executable: {
            commandName: 'claude',
            discoveredPath: process.platform === 'win32' ? 'C:\tools\claude.exe' : '/tools/claude',
            executablePath: process.platform === 'win32' ? 'C:\tools\claude.exe' : '/tools/claude',
            prefixArgs: [],
            kind: 'native'
          }
        }
      ],
      runner: { start },
      ledger: fakeLedger({ createCheckpoint }),
      createId: () => String(++nextId),
      now: () => new Date(NOW),
      schedule: (task) => scheduledTasks.push(task)
    })

    const atlas = await service.start('Atlas works.', 'codex', 'ask', {}, () => undefined, undefined, ATLAS)
    const wren = await service.start('Wren works.', 'codex', 'ask', {}, () => undefined, undefined, WREN)
    expect(atlas.ok && wren.ok).toBe(true)
    if (!atlas.ok) return
    while (scheduledTasks.length > 0) scheduledTasks.shift()!()

    // Wren's run is still open-ended. A handoff that waited on every live
    // loop would never return here.
    const handed = await service.handOff(atlas.data.runId, 'claude', 'ask', {}, () => undefined)
    expect(handed).toMatchObject({ ok: true })
    expect(createCheckpoint).toHaveBeenCalledWith(atlas.data.missionId, 'route-switch')
  })
})

describe('continuing a conversation', () => {
  function finished(overrides: Record<string, unknown> = {}) {
    return {
      metadata: {
        missionId: 'mission_prior',
        runId: 'run_prior',
        prompt: 'check the google stock price',
        runtime: 'codex',
        model: 'account-default',
        requestedRouteId: 'codex',
        resolvedRouteId: 'codex-account:default',
        cliVersion: null,
        workspaceId: 'ws_test',
        sandbox: 'read-only',
        executionPolicyVersion: 1,
        createdAt: NOW,
        ...overrides
      },
      events: [
        {
          id: 'e1',
          runId: 'run_prior',
          missionId: 'mission_prior',
          sequence: 1,
          type: 'run.started',
          occurredAt: NOW,
          sourceAdapter: 'codex',
          payload: { runtimeThreadId: 'thread-prior', evidence: { redacted: true } }
        }
      ],
      hostFailures: [],
      checkpoints: [],
      peerLinks: [],
    editChecks: [], approvals: [],
      phase: 'completed',
      lastUpdatedAt: NOW,
      ledgerSequence: 2,
      issues: []
    } as never
  }

  it('resumes the earlier mission\u2019s own session, and records what it continued', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'thread.started', thread_id: 'thread-prior' }, { type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service } = scheduledService({ start }, fakeLedger({
      createMission,
      getMission: async () => finished()
    }))

    const response = await service.start(
      'cant you look it up for me?', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior'
    )

    expect(response.ok).toBe(true)
    if (!response.ok) return
    // The runtime is told to resume that session rather than start blank.
    const spec = start.mock.calls[0]?.[0] as { args: readonly string[] }
    expect(spec.args.slice(0, 3)).toEqual(['exec', 'resume', 'thread-prior'])
    // And the record says so, so a reopened thread can rebuild the exchange.
    expect(createMission.mock.calls[0]?.[0]?.continuesFrom).toEqual({
      missionId: 'mission_prior',
      checkpointEpoch: 1,
      reason: 'follow-up',
      runtimeThreadId: 'thread-prior'
    })
    expect(response.data.followsUp).toEqual({ missionId: 'mission_prior', runtimeThreadId: 'thread-prior' })
  })

  it('starts cold when the mode changed, so the chip is not lying about the run', async () => {
    // An outside tester, 0.38.7 finding 5: "Composer mode does not apply to
    // follow-ups in the open thread. Chip set to Accept edits; the follow-up
    // still ran with no write tools because the thread started in Ask. The
    // chip lies about the session that will actually run."
    //
    // A resumed session keeps the tools it was BUILT with, so the mode a
    // person changed since cannot reach it. Losing the earlier messages is
    // the smaller cost -- and the thread says that happened -- next to the
    // mode on screen not being the mode that runs.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const created: Record<string, unknown>[] = []
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => finished({ mode: 'ask' }) as never,
      createMission: async (metadata) => {
        created.push(metadata as unknown as Record<string, unknown>)
      }
    }))

    const response = await service.start(
      'now actually write it', 'codex', 'accept-edits', {}, () => undefined, undefined, undefined, 'mission_prior'
    )

    expect(response.ok).toBe(true)
    const spec = start.mock.calls[0]?.[0] as { args: readonly string[] }
    // Not `exec resume thread-prior`: that session cannot be given write tools.
    expect(spec.args.slice(0, 3)).not.toEqual(['exec', 'resume', 'thread-prior'])
    expect(spec.args).not.toContain('thread-prior')
    // And the run it did start is the one the chip promised.
    expect(created[0]?.sandbox).toBe('workspace-write')
  })

  it('tells a reply it cannot resume what was said before (0.495)', async () => {
    // Grok's 0.489 pass: a cold reply's thread said "Started without the
    // earlier messages", and the runtime was told nothing of them. It is now
    // given the conversation so far, the way a runtime switch is.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => finished({ mode: 'ask' }) as never
    }))
    await service.start('now actually write it', 'codex', 'accept-edits', {}, () => undefined, undefined, undefined, 'mission_prior')
    const sent = String(start.mock.calls[0]?.[1] ?? '')
    expect(sent).toContain('This conversation has earlier turns, but not in your session')
    expect(sent).toContain('The person now asks:')
    expect(sent.trimEnd().endsWith('now actually write it')).toBe(true)
  })

  it('still resumes when the mode is the same one the conversation started in', async () => {
    // The guard above must not cost every follow-up its context: an unchanged
    // mode resumes exactly as before.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'thread.started', thread_id: 'thread-prior' }, { type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => finished({ mode: 'ask' }) as never
    }))

    const response = await service.start(
      'and what did that say?', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior'
    )

    expect(response.ok).toBe(true)
    const spec = start.mock.calls[0]?.[0] as { args: readonly string[] }
    expect(spec.args.slice(0, 3)).toEqual(['exec', 'resume', 'thread-prior'])
  })

  it('continues a conversation whose earlier turn recorded no session, cold rather than not at all', async () => {
    // A turn that failed before its runtime started leaves nothing to resume.
    // Refusing the reply is what made a follow-up after a failure open a
    // second sidebar row and drop the turn above it (Colin, 2026-09-03). The
    // conversation is still that conversation; only the model starts blank.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const created: Record<string, unknown>[] = []
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => ({ ...(finished() as never as Record<string, unknown>), events: [] }) as never,
      createMission: async (metadata) => {
        created.push(metadata as unknown as Record<string, unknown>)
      }
    }))

    const response = await service.start(
      'go on', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior'
    )

    expect(response).toMatchObject({ ok: true })
    // The record links the turns and claims no resume that did not happen.
    expect(created.at(-1)?.continuesFrom).toEqual({
      missionId: 'mission_prior',
      checkpointEpoch: 1,
      reason: 'follow-up'
    })
    expect((response as { data: { followsUp?: unknown } }).data.followsUp).toEqual({ missionId: 'mission_prior' })
    // And the process starts fresh: `exec resume` would need a session id.
    expect(start).toHaveBeenCalled()
    const spec = start.mock.calls[0]?.[0] as { args: string[] }
    expect(spec.args).not.toContain('resume')
  })

  it('picks up the session a turn stopped before naming its own was resuming (0.496)', async () => {
    // Stop pressed half a second into a reply: the turn resumed thread-older
    // and was stopped before it said so. The next reply is still that
    // conversation, not "A fresh session".
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const created: Record<string, unknown>[] = []
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => ({
        ...(finished({ continuesFrom: { missionId: 'mission_older', checkpointEpoch: 1, reason: 'follow-up', runtimeThreadId: 'thread-older' } }) as never as Record<string, unknown>),
        events: []
      }) as never,
      createMission: async (metadata) => {
        created.push(metadata as unknown as Record<string, unknown>)
      }
    }))

    await service.start('go on', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior')

    const spec = start.mock.calls[0]?.[0] as { args: readonly string[] }
    expect(spec.args.slice(0, 3)).toEqual(['exec', 'resume', 'thread-older'])
    expect(String(start.mock.calls[0]?.[1] ?? '')).not.toContain('not in your session')
    expect(created.at(-1)?.continuesFrom).toMatchObject({ missionId: 'mission_prior', runtimeThreadId: 'thread-older' })
  })

  it('starts again from an edited message cold, never in the session that holds the turns set aside (0.498)', async () => {
    // `mission_prior` is the turn BEFORE the edited one. Its session also
    // holds every later turn, so resuming it would give the model exactly
    // what the person went back to change.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const created: Record<string, unknown>[] = []
    const catchUpTerminal = vi.fn(async () => ({ latestMissionId: 'mission_old_tip' }))
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => finished({ mode: 'ask' }) as never,
      createMission: async (metadata) => {
        created.push(metadata as unknown as Record<string, unknown>)
      }
    }), { catchUpTerminal } as never)

    const response = await service.start(
      'check the apple stock price instead', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior',
      undefined, undefined, undefined, undefined, undefined, true
    )

    expect(response.ok).toBe(true)
    const spec = start.mock.calls[0]?.[0] as { args: readonly string[] }
    expect(spec.args).not.toContain('thread-prior')
    const sent = String(start.mock.calls[0]?.[1] ?? '')
    expect(sent).toContain('went back to an earlier point')
    expect(sent).toContain('check the google stock price')
    expect(sent.trimEnd().endsWith('check the apple stock price instead')).toBe(true)
    // The terminal's latest turn is the branch being set aside: never jumped to.
    expect(catchUpTerminal).not.toHaveBeenCalled()
    // Marked as an edit, so the version before can be found again after a restart.
    expect(created.at(-1)?.continuesFrom).toEqual({ missionId: 'mission_prior', checkpointEpoch: 1, reason: 'follow-up', edited: true })
  })

  it('sends again as the same words, not as an earlier turn quoted back (0.496)', async () => {
    // The first turn of a conversation, stopped before any reply, then Send
    // again: there is no session to resume, and "Asked: <the same words> --
    // no reply was recorded" above those same words told the model nothing.
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => ({ ...(finished() as never as Record<string, unknown>), events: [] }) as never
    }))

    await service.start('check the google stock price', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior')

    const sent = String(start.mock.calls[0]?.[1] ?? '')
    expect(sent).not.toContain('This conversation has earlier turns')
    expect(sent).toContain('check the google stock price')
  })

  /*
   * H6: Resume from checkpoint resumed as NOBODY -- in the project folder
   * rather than the teammate's own branch, outside their one-run guard, and
   * a runtime change recorded as a follow-up. Driven through the real
   * service, as the review's verifier did.
   */
  describe('H6: resuming an interrupted mission', () => {
    const resumed = async (runtime: 'codex' | 'claude') => {
      const starts: { cwd?: string }[] = []
      const created: Record<string, unknown>[] = []
      const checkpoints: string[] = []
      const start = vi.fn((spec: { cwd?: string }, _prompt, _options): RuntimeProcessRun => {
        starts.push(spec)
        // A run that stays live, so its owner can be asked about.
        return { records: records([{ type: 'thread.started', thread_id: 'thread-resumed' }]), completion: new Promise(() => undefined) }
      }) satisfies RuntimeProcessRunner['start']
      const { service } = scheduledService({ start }, fakeLedger({
        getMission: async () => finished({ runtime: 'codex' }),
        createMission: async (metadata) => {
          created.push(metadata as unknown as Record<string, unknown>)
        },
        createCheckpoint: async (missionId, reason) => {
          checkpoints.push(reason)
          return { missionId, epoch: 2, reason, resumeSafety: 'safe', safetyReason: 'settled', createdAt: NOW, unsettledActions: [], settledActions: [], assistantSummary: 'Half done.' } as never
        }
      }), {
        discover: async () => [codexRuntime(), { ...codexRuntime(), id: 'claude' as const, displayName: 'Claude Code', executable: { ...codexRuntime().executable!, commandName: 'claude' } }]
      })
      const WREN_TREE = process.platform === 'win32' ? 'C:\\project\\.locust\\worktrees\\tm_wren' : '/project/.locust/worktrees/tm_wren'
      const peer: MissionPeerContext = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [], cwd: WREN_TREE } as MissionPeerContext
      const response = await service.resume('mission_prior', runtime, 'accept-edits', {}, () => undefined, peer)
      return { service, response, starts, created, checkpoints, WREN_TREE }
    }

    it('runs in the teammate\u2019s own branch, as theirs, under their one-run guard', async () => {
      const { service, response, starts, WREN_TREE } = await resumed('codex')
      expect(response.ok).toBe(true)
      expect(starts[0]?.cwd).toBe(WREN_TREE)
      expect(service.runIdOwnedBy('tm_wren')).toBeDefined()
    })

    it('records a resume on another runtime as a route switch, not a follow-up', async () => {
      const { created, checkpoints } = await resumed('claude')
      expect(checkpoints).toEqual(['route-switch'])
      expect(created.at(-1)?.continuesFrom).toMatchObject({ missionId: 'mission_prior', reason: 'route-switch' })
    })

    it('and on the same runtime, as a follow-up still', async () => {
      const { created, checkpoints } = await resumed('codex')
      expect(checkpoints).toEqual(['manual'])
      expect(created.at(-1)?.continuesFrom).toMatchObject({ reason: 'follow-up' })
    })
  })

  it('A2.11: carries the conversation\u2019s earlier turns into the switch, oldest first, and stops at a loop', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'thread.started', thread_id: 'thread-new' }, { type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const byId: Record<string, unknown> = {
      mission_prior: finished({ runtime: 'claude', continuesFrom: { missionId: 'mission_middle', checkpointEpoch: 1, reason: 'follow-up' } }),
      mission_middle: { ...(finished({ missionId: 'mission_middle', prompt: 'then make it weekly', continuesFrom: { missionId: 'mission_first', checkpointEpoch: 1, reason: 'follow-up' } }) as Record<string, unknown>), events: [] },
      // A record that points back at itself must not walk forever.
      mission_first: { ...(finished({ missionId: 'mission_first', prompt: 'build a price report', continuesFrom: { missionId: 'mission_first', checkpointEpoch: 1, reason: 'follow-up' } }) as Record<string, unknown>), events: [] }
    }
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async (id) => byId[id as string] as never,
      createCheckpoint: async (missionId, reason) => ({
        missionId, epoch: 2, reason, resumeSafety: 'safe', safetyReason: 'settled', createdAt: NOW,
        unsettledActions: [], settledActions: [], assistantSummary: 'The price was 181.'
      }) as never
    }))
    const response = await service.start('now in euros', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior')
    expect(response).toMatchObject({ ok: true })
    const briefed = start.mock.calls[0]?.[1] as string
    expect(briefed).toContain('Earlier in this conversation, oldest first:\n- Asked: "build a price report" -- no reply was recorded.\n- Asked: "then make it weekly" -- no reply was recorded.')
  })

  it('continues another runtime\u2019s conversation from a checkpoint, with the reply as the latest word', async () => {
    // This used to be refused ("switch the route back"), and the person's
    // workaround was a fresh mission with the task retyped -- the 0.21.2 QA
    // pass did exactly that after a quota failure. Now it is a handoff
    // without the stop.
    const checkpoints: Array<[string, string]> = []
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'thread.started', thread_id: 'thread-new' }, { type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => finished({ runtime: 'claude' }),
      createMission,
      createCheckpoint: async (missionId, reason) => {
        checkpoints.push([missionId, reason])
        return {
          missionId,
          epoch: 2,
          reason,
          resumeSafety: 'safe',
          safetyReason: 'settled',
          createdAt: NOW,
          unsettledActions: [],
          settledActions: ['read README.md'],
          assistantSummary: 'The price was 181.'
        } as never
      }
    }))

    const response = await service.start(
      'now in euros', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior'
    )

    expect(response).toMatchObject({ ok: true })
    expect(checkpoints).toEqual([['mission_prior', 'route-switch']])
    expect(start).toHaveBeenCalledTimes(1)
    const briefed = start.mock.calls[0]?.[1] as string
    expect(briefed).toContain('another agent (Claude Code)')
    expect(briefed).toContain('check the google stock price')
    expect(briefed).toContain('The price was 181.')
    expect(briefed.endsWith('The person now asks:\n\nnow in euros')).toBe(true)
    // Recorded the way the live handoff records, so the thread draws the
    // same divider and a reopened conversation stitches the same way.
    expect(createMission.mock.calls[0]?.[0]?.continuesFrom).toEqual({
      missionId: 'mission_prior',
      checkpointEpoch: 2,
      reason: 'route-switch'
    })
    // No `exec resume`: the other runtime's session is not this one's.
    const spec = start.mock.calls[0]?.[0] as { args: readonly string[] }
    expect(spec.args.includes('resume')).toBe(false)
    // And the receipt SAYS it was a switch, so the window draws the seam at
    // once. It came back like any other reply, and the thread showed no
    // divider until the conversation was reopened (drive-runtime-switch,
    // packaged 0.309).
    expect(response.ok && response.data.switchedFrom).toEqual({
      missionId: 'mission_prior',
      runtime: 'claude',
      // The model the turn before ran on, for the divider to name (0.779).
      model: 'account-default',
      unsettledCount: 0,
      omittedBriefing: []
    })
  })

  it('names no switch on an ordinary reply on the same runtime', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'thread.started', thread_id: 'thread-new' }, { type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => finished({ runtime: 'codex' })
    }))

    const response = await service.start(
      'now in euros', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior'
    )

    expect(response).toMatchObject({ ok: true })
    expect(response.ok && response.data.switchedFrom).toBeUndefined()
  })

  it('refuses the cross-runtime continuation when the record cannot be trusted', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService({ start }, fakeLedger({
      getMission: async () => finished({ runtime: 'claude' }),
      createCheckpoint: async (missionId, reason) =>
        ({ missionId, epoch: 2, reason, resumeSafety: 'unsafe', safetyReason: 'the ledger tail is damaged' }) as never
    }))
    const response = await service.start(
      'go on', 'codex', 'ask', {}, () => undefined, undefined, undefined, 'mission_prior'
    )
    expect(response).toMatchObject({
      ok: false,
      error: { code: 'RUNTIME_START_FAILED', message: expect.stringContaining('the ledger tail is damaged') }
    })
    expect(start).not.toHaveBeenCalled()
  })

  it('starts a fresh conversation when nothing is being continued', async () => {
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service } = scheduledService({ start }, fakeLedger({ createMission }))

    await service.start('a first question', 'codex', 'ask', {}, () => undefined)

    const spec = start.mock.calls[0]?.[0] as { args: readonly string[] }
    expect(spec.args.includes('resume')).toBe(false)
    expect(createMission.mock.calls[0]?.[0]?.continuesFrom).toBeUndefined()
  })
})

describe('what a completed share hands to the relay', () => {
  const WREN = { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }
  const ATLAS = { teammateId: 'tm_atlas', name: 'Atlas', role: 'Research & Briefs' }
  const PEER: MissionPeerContext = { self: WREN, others: [ATLAS] }

  it.each([undefined, { kind: 'mcp' as const }])('calls onShared with the origin and exactly the messages it posted, after they are recorded (%j)', async origin => {
    let nextId = 0
    const workroom: Workroom = {
      post: async (input) => ({
        messageId: `wm_out_${String(++nextId)}`,
        sequence: nextId,
        from: input.from,
        to: input.to,
        text: input.text,
        postedAt: NOW
      }),
      unread: async () => ({ messages: [], remaining: 0 }),
      markDelivered: async () => undefined,
      read: async () => ({ messages: [], deliveries: [], issues: [] }),
      flush: async () => undefined
    }
    const scheduled: Array<() => void> = []
    const shared: { mission: unknown; posted: readonly WorkroomMessage[] }[] = []
    let ids = 0
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: {
        start: () => ({
          records: records([
            { type: 'thread.started', thread_id: 'thread-live' },
            { type: 'turn.started' },
            {
              type: 'item.completed',
              item: {
                id: 'answer',
                type: 'agent_message',
                text: 'Found it.\n<locust-share to="Atlas">The build runs with pnpm check.</locust-share>'
              }
            },
            { type: 'turn.completed', usage: { output_tokens: 2 } }
          ]),
          completion: Promise.resolve(completion())
        })
      },
      ledger: fakeLedger(),
      workroom,
      createId: () => String(++ids),
      now: () => new Date(NOW),
      schedule: (task) => scheduled.push(task),
      onShared: async (mission, posted) => {
        shared.push({ mission, posted })
      }
    })

    const response = await service.start('Where do the checks run?', 'codex', 'ask', { model: 'gpt-5-codex' }, () => undefined, undefined, PEER, undefined, undefined, origin)
    expect(response.ok).toBe(true)
    while (scheduled.length > 0) scheduled.shift()!()
    for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))

    expect(shared).toHaveLength(1)
    expect(shared[0]?.mission).toMatchObject({
      runtime: 'codex',
      sandbox: 'read-only',
      model: 'gpt-5-codex',
      peer: PEER,
      relay: undefined,
      ...(origin === undefined ? {} : { startedBy: origin })
    })
    expect(shared[0]?.posted.map((message) => [message.to.name, message.text])).toEqual([
      ['Atlas', 'The build runs with pnpm check.']
    ])
  })

  /*
   * A2.17: PROOF FROM THE HOST. A message a writing run sends carries the
   * paths the host itself saw that run change -- its own before-and-after
   * reading of the folder -- so the recipient reads the claim beside the
   * fact. A read-only run was never looked at, and claims nothing.
   */
  describe('what the host saw, on the messages a run sends', () => {
    const NUL = String.fromCharCode(0)
    const run = async (mode: 'accept-edits' | 'ask') => {
      const posts: { observed?: readonly string[] }[] = []
      const scheduled: Array<() => void> = []
      let looks = 0
      let ids = 0
      const service = createCodexMissionService({
        workspacePath: WORKSPACE,
        discover: async () => [codexRuntime()],
        runner: {
          start: () => ({
            records: records([
              { type: 'thread.started', thread_id: 'thread-live' },
              { type: 'turn.started' },
              { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Done.\n<locust-share to="Atlas">I fixed app.ts.</locust-share>' } },
              { type: 'turn.completed', usage: { output_tokens: 2 } }
            ]),
            completion: Promise.resolve(completion())
          })
        },
        ledger: fakeLedger(),
        observeDisk: async () => {
          looks += 1
          const map = new Map<string, string>()
          if (looks > 1) for (const entry of ` M src/app.ts${NUL}?? src/new.ts${NUL}`.split(NUL)) if (entry.length >= 4) map.set(entry.slice(3), entry.slice(0, 2))
          return map
        },
        observePatches: async () => new Map(),
        workroom: {
          post: async (input) => {
            posts.push(input)
            return { messageId: `wm_${String(posts.length)}`, sequence: posts.length, from: input.from, to: input.to, text: input.text, postedAt: NOW }
          },
          unread: async () => ({ messages: [], remaining: 0 }),
          markDelivered: async () => undefined,
          read: async () => ({ messages: [], deliveries: [], issues: [] }),
          flush: async () => undefined
        },
        createId: () => String(++ids),
        now: () => new Date(NOW),
        schedule: (task) => scheduled.push(task)
      })
      const response = await service.start('Fix app.ts and tell Atlas.', 'codex', mode, {}, () => undefined, undefined, PEER)
      expect(response.ok).toBe(true)
      while (scheduled.length > 0) scheduled.shift()!()
      for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))
      return posts
    }

    it('attaches the paths the host saw a writing run change', async () => {
      const posts = await run('accept-edits')
      expect(posts).toHaveLength(1)
      expect([...(posts[0]!.observed ?? [])].sort()).toEqual(['src/app.ts', 'src/new.ts'])
    })

    it('A2.9: tells a teammate, in their next brief, that another changed a file they changed too', async () => {
      const prompts: string[] = []
      const scheduled: Array<() => void> = []
      let looks = 0
      let ids = 0
      const service = createCodexMissionService({
        workspacePath: WORKSPACE,
        discover: async () => [codexRuntime()],
        runner: {
          start: (_spec, prompt) => {
            prompts.push(prompt)
            return {
              records: records([
                { type: 'thread.started', thread_id: `thread-${String(prompts.length)}` },
                { type: 'turn.started' },
                { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Done.' } },
                { type: 'turn.completed', usage: { output_tokens: 2 } }
              ]),
              completion: Promise.resolve(completion())
            }
          }
        },
        ledger: fakeLedger(),
        // Every run finds src/app.ts changed by the time it ends.
        observeDisk: async () => {
          looks += 1
          return looks % 2 === 0 ? new Map([['src/app.ts', ' M']]) : new Map<string, string>()
        },
        observePatches: async () => new Map(),
        recentEdits: createRecentEdits(),
        workroom: {
          post: async (input) => ({ messageId: 'wm_1', sequence: 1, from: input.from, to: input.to, text: input.text, postedAt: NOW }),
          unread: async () => ({ messages: [], remaining: 0 }),
          markDelivered: async () => undefined,
          read: async () => ({ messages: [], deliveries: [], issues: [] }),
          flush: async () => undefined
        },
        createId: () => String(++ids),
        now: () => new Date(NOW),
        schedule: (task) => scheduled.push(task)
      })
      const runAs = async (peer: MissionPeerContext) => {
        const response = await service.start('Tidy app.ts.', 'codex', 'accept-edits', {}, () => undefined, undefined, peer)
        expect(response.ok).toBe(true)
        while (scheduled.length > 0) scheduled.shift()!()
        for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))
      }
      await runAs(PEER)
      await runAs({ self: ATLAS, others: [WREN] })
      await runAs(PEER)
      expect(prompts[0]).not.toContain('also changed by another teammate')
      expect(prompts[2]).toContain('- Atlas changed src/app.ts')
    })

    it('attaches nothing from a read-only run, which the host never looked at', async () => {
      const posts = await run('ask')
      expect(posts).toHaveLength(1)
      expect(posts[0]!.observed).toBeUndefined()
    })
  })

  it('a relayed run carries its hop through to what it shares', async () => {
    const scheduled: Array<() => void> = []
    const shared: { mission: { relay?: unknown } }[] = []
    let ids = 0
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: {
        start: () => ({
          records: records([
            { type: 'thread.started', thread_id: 'thread-live' },
            { type: 'turn.started' },
            {
              type: 'item.completed',
              item: { id: 'answer', type: 'agent_message', text: '<locust-share to="Wren">Yes.</locust-share>' }
            },
            { type: 'turn.completed', usage: { output_tokens: 1 } }
          ]),
          completion: Promise.resolve(completion())
        })
      },
      ledger: fakeLedger(),
      workroom: {
        post: async (input) => ({ messageId: 'wm_1', sequence: 1, from: input.from, to: input.to, text: input.text, postedAt: NOW }),
        unread: async () => ({ messages: [], remaining: 0 }),
        markDelivered: async () => undefined,
        read: async () => ({ messages: [], deliveries: [], issues: [] }),
        flush: async () => undefined
      },
      createId: () => String(++ids),
      now: () => new Date(NOW),
      schedule: (task) => scheduled.push(task),
      onShared: async (mission) => {
        shared.push({ mission })
      }
    })
    const origin = { hop: 1, lastMissionOf: { tm_wren: 'mission_origin' } }
    const response = await service.start('Reply to Wren.', 'codex', 'ask', {}, () => undefined, undefined, { self: ATLAS, others: [WREN] }, undefined, origin)
    expect(response.ok).toBe(true)
    while (scheduled.length > 0) scheduled.shift()!()
    for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))
    expect(shared[0]?.mission.relay).toEqual(origin)
  })
})

describe('what a run changed on disk that it never said', () => {
  // OpenCode, 2026-09-05: a free Muse Spark run in write mode changed
  // notes.ts through its `task` sub-agent; the stream said `task` and `read`,
  // the activity card said "2 tool calls", and the file on disk disagreed
  // with the receipt. The host now looks for itself.
  const NUL = String.fromCharCode(0)
  const snapshots = (before: string, after: string) => {
    let looks = 0
    const observeDisk = vi.fn(async () => {
      looks += 1
      const output = looks === 1 ? before : after
      const map = new Map<string, string>()
      for (const entry of output.split(NUL)) if (entry.length >= 4) map.set(entry.slice(3), entry.slice(0, 2))
      return map
    })
    return observeDisk
  }

  it('adds an observed edit row, after the terminal event, for a file no tool named', async () => {
    const appended: NormalizedRuntimeEvent[] = []
    const ledger = fakeLedger({ appendEvents: async (_id, events) => { appended.push(...events) } })
    const observeDisk = snapshots('', ` M src/notes.ts${NUL}`)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([
        { type: 'thread.started', thread_id: 'thread-live' },
        { type: 'turn.started' },
        { type: 'item.started', item: { id: 'i1', type: 'command_execution', command: 'cat README.md' } },
        { type: 'item.completed', item: { id: 'i1', type: 'command_execution', command: 'cat README.md', exit_code: 0 } },
        { type: 'turn.completed', usage: { output_tokens: 2 } }
      ]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, ledger, { observeDisk })
    await service.start('polish the notes', 'codex', 'accept-edits', {}, () => undefined)
    scheduled[0]?.()
    await new Promise((resolve) => setTimeout(resolve, 20))

    expect(observeDisk).toHaveBeenCalledTimes(2)
    const completedAt = appended.findIndex((event) => event.type === 'run.completed')
    expect(completedAt).toBeGreaterThan(-1)
    const observed = appended.slice(completedAt + 1).filter((event) => (event.payload as { code?: string }).code !== 'host.start-timing')
    expect(observed.map((event) => event.type)).toEqual(['tool.started', 'tool.completed'])
    expect(observed[0]?.payload).toMatchObject({ name: 'edit', command: 'src/notes.ts', status: 'observed on disk' })
    // Contiguous with the record it follows: the ledger refuses a gap.
    expect(observed[0]?.sequence).toBe(appended[completedAt]!.sequence + 1)
    expect(observed[1]?.sequence).toBe(appended[completedAt]!.sequence + 2)
  })

  it('claims nothing from the tree when another writing run shared the folder', async () => {
    // Three teammates started at once in one folder, each writing one file.
    // Every run's card then read "3 files, +124" -- the sum of all three --
    // because the observation compares the whole working tree and cannot see
    // who wrote what (drive, 2026-09-06).
    const appended: NormalizedRuntimeEvent[] = []
    const ledger = fakeLedger({ appendEvents: async (_id, events) => { appended.push(...events) } })
    // Both runs start on a clean tree; both end looking at the same changed
    // file. Which of them wrote it is exactly what cannot be known.
    let looks = 0
    const observeDisk = vi.fn(async () => {
      looks += 1
      const map = new Map<string, string>()
      if (looks > 2) map.set('src/notes.ts', ' M')
      return map
    })
    const runtimeRecords = () => records([
      { type: 'thread.started', thread_id: 'thread-live' },
      { type: 'turn.started' },
      { type: 'item.started', item: { id: 'i1', type: 'command_execution', command: 'cat README.md' } },
      { type: 'item.completed', item: { id: 'i1', type: 'command_execution', command: 'cat README.md', exit_code: 0 } },
      { type: 'turn.completed', usage: { output_tokens: 2 } }
    ])
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: runtimeRecords(),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, ledger, { observeDisk })

    // Two teammates, because the service allows one live run per teammate
    // and counts "nobody" as one of them. The first draft of this test gave
    // neither a teammate, so the second start was refused, only one run ever
    // happened, and it passed with the fix REMOVED -- a green that could not
    // have gone red. Both start before either finishes: that is the point.
    const wren = { self: { teammateId: 'tm_wren', name: 'Wren', role: 'Code & Migrations' }, others: [] }
    const gem = { self: { teammateId: 'tm_gem', name: 'Gem', role: 'Research & Briefs' }, others: [] }
    const first = await service.start('write one poem', 'codex', 'accept-edits', {}, () => undefined, undefined, wren)
    const second = await service.start('write another poem', 'codex', 'accept-edits', {}, () => undefined, undefined, gem)
    // Both really started, or there is no concurrency here to test.
    expect([first.ok, second.ok]).toEqual([true, true])
    scheduled[0]?.()
    scheduled[1]?.()
    await new Promise((resolve) => setTimeout(resolve, 30))

    const observed = appended.filter((event) => (event.payload as { status?: string } | undefined)?.status === 'observed on disk')
    expect(observed).toHaveLength(0)

    // Silence would be its own lie: a run really did change something, and a
    // card reading only "5 tool calls" invites the reader to think nothing
    // was written. The folder change is reported as the folder is.
    const notices = appended.filter((event) => event.type === 'adapter.diagnostic' && (event.payload as { code?: string }).code === 'host.shared_workspace')
    expect(notices).toHaveLength(2)
    expect((notices[0]?.payload as { message?: string }).message).toMatch(/Another teammate was working in this folder/)
    expect((notices[0]?.payload as { message?: string }).message).toMatch(/1 file in the folder is different/)
    // One file, so it must read "it is not counted", never "none of them are".
    expect((notices[0]?.payload as { message?: string }).message).toMatch(/it is not counted as this run's work/)

    // The negative control, and it matters: "no observed rows" is also what a
    // broken harness produces. One run alone, same records, same snapshots --
    // this one MUST still get its row, or the assertion above proves nothing.
    const soloAppended: NormalizedRuntimeEvent[] = []
    const soloLedger = fakeLedger({ appendEvents: async (_id, events) => { soloAppended.push(...events) } })
    const solo = scheduledService({ start }, soloLedger, { observeDisk: snapshots('', ` M src/notes.ts${NUL}`) })
    await solo.service.start('write one poem', 'codex', 'accept-edits', {}, () => undefined)
    solo.scheduled[0]?.()
    await new Promise((resolve) => setTimeout(resolve, 30))
    expect(
      soloAppended.filter((event) => (event.payload as { status?: string } | undefined)?.status === 'observed on disk').length
    ).toBeGreaterThan(0)
  })

  it('does not repeat a file the runtime already named, and never looks on a read-only run', async () => {
    const appended: NormalizedRuntimeEvent[] = []
    const ledger = fakeLedger({ appendEvents: async (_id, events) => { appended.push(...events) } })
    const observeDisk = snapshots('', ` M src/notes.ts${NUL}`)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([
        { type: 'thread.started', thread_id: 'thread-live' },
        { type: 'turn.started' },
        { type: 'item.started', item: { id: 'i1', type: 'command_execution', command: 'apply_patch src/notes.ts' } },
        { type: 'item.completed', item: { id: 'i1', type: 'command_execution', command: 'apply_patch src/notes.ts', exit_code: 0 } },
        { type: 'turn.completed', usage: { output_tokens: 2 } }
      ]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, ledger, { observeDisk })
    await service.start('polish the notes', 'codex', 'accept-edits', {}, () => undefined)
    scheduled[0]?.()
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(appended.filter((event) => event.payload && (event.payload as { status?: string }).status === 'observed on disk')).toHaveLength(0)

    const readOnly = snapshots('', ` M src/notes.ts${NUL}`)
    const again = scheduledService({ start }, fakeLedger(), { observeDisk: readOnly })
    await again.service.start('just read', 'codex', 'ask', {}, () => undefined)
    again.scheduled[0]?.()
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(readOnly).not.toHaveBeenCalled()
  })
})

/**
 * Codex on the transport that STREAMS.
 *
 * `codex exec --json` sends an agent message once and whole -- measured
 * 2026-09-10 by running it -- so a Codex teammate outside Approve-each said
 * nothing and then dropped the entire reply in one paint. Every mode rides
 * `codex app-server` instead, which sends the same reply as deltas. These
 * hold the switch in place: that Codex asks for a server rather than an exec,
 * that the deltas become message events, and that the two dangerous knobs on
 * this transport -- which are JSON rather than argv, and so out of reach of
 * the argv guard -- still cannot be widened by a mode that was not granted it.
 */
/** A fake `codex app-server` that answers the handshake and then streams. */
function fakeAppServer(script: ReadonlyArray<readonly [string, unknown]>, settings: unknown = {}) {
  const written: Array<Record<string, unknown>> = []
  let emit: (chunk: string) => void = () => undefined
  const spawn = vi.fn((_executablePath: string, _args: readonly string[]) => ({
    write: (line: string) => {
      for (const part of line.split('\n')) {
        if (part.trim().length === 0) continue
        const message = JSON.parse(part) as Record<string, unknown>
        written.push(message)
        if (typeof message.id !== 'number') continue
        // Answer each request the way the real server does, then -- once the
        // turn is accepted -- play the script out. The person's settings, when
        // asked for, are `settings` (0.824: Ask and Plan read them first).
        const answer = message.method === 'thread/start' || message.method === 'thread/resume'
          ? { thread: { id: 'thread-live' } }
          : message.method === 'turn/start'
            ? { turn: { id: 'turn-1' } }
            : message.method === 'config/read'
              ? { config: settings }
              : { userAgent: 'codex' }
        queueMicrotask(() => {
          emit(`${JSON.stringify({ jsonrpc: '2.0', id: message.id, result: answer })}\n`)
          if (message.method !== 'turn/start') return
          queueMicrotask(() => {
            for (const [method, params] of script) {
              emit(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`)
            }
          })
        })
      }
    },
    kill: () => undefined,
    onData: (listener: (chunk: string) => void) => {
      emit = listener
    },
    onExit: () => undefined
  }))
  return {
    spawn,
    written,
    /** The server ASKS -- the approval channel's whole reason to exist. */
    ask: (id: string, method: string, params: unknown) =>
      emit(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}
`),
    /** What was written back to that request, once something answers it. */
    replyTo: (id: string) => written.find((message) => message.id === id)
  }
}

describe('Codex over app-server', () => {
  const REPLY = [
    ['item/started', { item: { id: 'm1', type: 'agentMessage', text: '' } }],
    ['item/agentMessage/delta', { itemId: 'm1', delta: 'Two ' }],
    ['item/agentMessage/delta', { itemId: 'm1', delta: 'files.' }],
    ['item/completed', { item: { id: 'm1', type: 'agentMessage', text: 'Two files.' } }],
    ['turn/completed', { threadId: 'thread-live' }]
  ] as const

  /*
   * Colin, 2026-09-23: "planui looks like its failing in here in a codex
   * chart" -- a Codex reply that typed its TODO list, because Codex 0.153
   * offers its plan tool only when told to and Locust never told it. From
   * 0.153 the server is started with the tool on; the fake runtime below the
   * other tests is 0.151.0-alpha, which keeps plain `app-server`.
   */
  it("switches Codex's plan tool on from 0.153, so its plan reaches the panel rather than the reply", async () => {
    const { spawn } = fakeAppServer(REPLY)
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const older = codexRuntime()
    const { service } = scheduledService({ start }, fakeLedger({ createMission }), {
      appServerSpawn: spawn,
      discover: async () => [{ ...older, version: { ...older.version!, raw: 'codex-cli 0.153.0', version: '0.153.0', minor: 153 } }]
    })

    const response = await service.start('How many files?', 'codex', 'accept-edits', {}, () => undefined)
    expect(response.ok).toBe(true)
    const withPlanTool = ['app-server', '-c', 'tools.update_plan.enabled=true']
    expect(spawn.mock.calls[0]?.[1]).toEqual(withPlanTool)
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({
      command: expect.objectContaining({ args: withPlanTool })
    }))
  })

  it('launches a server rather than an exec, and turns its deltas into message events', async () => {
    const { spawn, written } = fakeAppServer(REPLY)
    const start = vi.fn() satisfies RuntimeProcessRunner['start']
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async () => undefined)
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service, scheduled } = scheduledService(
      { start },
      fakeLedger({ appendEvents, createMission }),
      { appServerSpawn: spawn }
    )
    const updates: CodexMissionUpdate[] = []

    const response = await service.start('How many files?', 'codex', 'accept-edits', {}, (update) => {
      updates.push(update)
    })
    expect(response.ok).toBe(true)
    // The exec transport is not reached at all.
    expect(start).not.toHaveBeenCalled()
    expect(spawn).toHaveBeenCalledTimes(1)
    expect(spawn.mock.calls[0]?.[1]).toEqual(['app-server'])
    // And what ran is what the record says ran.
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({
      command: expect.objectContaining({ args: ['app-server'] })
    }))

    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.completed')).toBe(true)
    })
    const types = appendEvents.mock.calls.flatMap(([, events]) => events).map(({ type }) => type)
    expect(types).toContain('message.delta')
    expect(types.filter((type) => type === 'message.delta').length).toBeGreaterThan(1)
    expect(JSON.stringify(updates)).toContain('Two ')
    // The folder and the policy the mode earns, on the calls that take them.
    const threadStart = written.find((message) => message.method === 'thread/start')
    expect(threadStart?.params).toMatchObject({
      cwd: WORKSPACE,
      sandbox: 'workspace-write',
      approvalPolicy: 'never'
    })
  })

  it('holds a read-only mode read-only, and never asks for full access on its behalf', async () => {
    const { spawn, written } = fakeAppServer(REPLY)
    const { service } = scheduledService({ start: vi.fn() }, fakeLedger(), { appServerSpawn: spawn })
    await service.start('Just look.', 'codex', 'ask', {}, () => undefined)
    // The handshake runs on its own microtasks after the start resolves.
    await vi.waitFor(() => {
      expect(written.some((message) => message.method === 'thread/start')).toBe(true)
    })
    const threadStart = written.find((message) => message.method === 'thread/start')
    expect(threadStart?.params).toMatchObject({ sandbox: 'read-only' })
    expect(JSON.stringify(written)).not.toContain('danger-full-access')
  })

  it('starts none of the MCP servers or plugins the person’s Codex runs as programs, in Ask and Plan only (0.824)', async () => {
    const settings = { mcp_servers: { node_repl: { command: 'node', args: ['repl.mjs'] }, docs: { url: 'https://docs.example.com/mcp' } } }
    for (const mode of ['ask', 'plan'] as const) {
      const { spawn, written } = fakeAppServer(REPLY, settings)
      const { service } = scheduledService({ start: vi.fn() }, fakeLedger(), { appServerSpawn: spawn })
      await service.start('Just look.', 'codex', mode, {}, () => undefined)
      await vi.waitFor(() => {
        expect(written.some((message) => message.method === 'thread/start')).toBe(true)
      })
      // Read first, which starts nothing; then the thread, with those servers off and the one at an address kept.
      expect(written.findIndex((message) => message.method === 'config/read')).toBeLessThan(written.findIndex((message) => message.method === 'thread/start'))
      const config = (written.find((message) => message.method === 'thread/start')?.params as { config?: Record<string, unknown> } | undefined)?.config
      expect(config).toMatchObject({ 'features.plugins': false, 'mcp_servers.node_repl.enabled': false })
      expect(config).not.toHaveProperty('mcp_servers.docs.enabled')
    }
    // Edit leaves the person's Codex as it is, and does not ask for its settings.
    const { spawn, written } = fakeAppServer(REPLY, settings)
    const { service } = scheduledService({ start: vi.fn() }, fakeLedger(), { appServerSpawn: spawn })
    await service.start('Change it.', 'codex', 'accept-edits', {}, () => undefined)
    await vi.waitFor(() => {
      expect(written.some((message) => message.method === 'thread/start')).toBe(true)
    })
    expect(written.some((message) => message.method === 'config/read')).toBe(false)
    expect(JSON.stringify(written)).not.toContain('features.plugins')
  })

  it('starts every Codex thread with Locust’s command guard, for its own folder, and without it when the guard says so (0.720)', async () => {
    const guard = { bypass_hook_trust: true, hooks: { PreToolUse: [{ matcher: 'Bash', hooks: [{ type: 'command', command: 'guard', timeout: 15 }] }] } }
    const askedFor: string[] = []
    const guarded = fakeAppServer(REPLY)
    const { service } = scheduledService({ start: vi.fn() }, fakeLedger(), {
      appServerSpawn: guarded.spawn,
      codexCommandGuard: async (folder: string) => {
        askedFor.push(folder)
        return guard
      }
    })
    await service.start('Just look.', 'codex', 'ask', {}, () => undefined)
    await vi.waitFor(() => {
      expect(guarded.written.some((message) => message.method === 'thread/start')).toBe(true)
    })
    expect(guarded.written.find((message) => message.method === 'thread/start')?.params).toMatchObject({ config: guard })
    expect(askedFor).toHaveLength(1)
    // Another hook Codex would load: no guard, and so no bypass either.
    const unguarded = fakeAppServer(REPLY)
    const again = scheduledService({ start: vi.fn() }, fakeLedger(), { appServerSpawn: unguarded.spawn, codexCommandGuard: async () => undefined })
    await again.service.start('Just look.', 'codex', 'ask', {}, () => undefined)
    await vi.waitFor(() => {
      expect(unguarded.written.some((message) => message.method === 'thread/start')).toBe(true)
    })
    expect(JSON.stringify(unguarded.written)).not.toContain('bypass_hook_trust')
  })

  it('leaves every other runtime on the transport it already had', async () => {
    const { spawn } = fakeAppServer(REPLY)
    const start = vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([{ type: 'turn.completed' }]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']
    const { service } = scheduledService(
      { start },
      fakeLedger(),
      {
        appServerSpawn: spawn,
        discover: async () => [
          codexRuntime(),
          {
            ...codexRuntime(),
            id: 'claude' as const,
            displayName: 'Claude Code',
            executable: {
              commandName: 'claude',
              discoveredPath: process.platform === 'win32' ? 'C:\tools\claude.exe' : '/tools/claude',
              executablePath: process.platform === 'win32' ? 'C:\tools\claude.exe' : '/tools/claude',
              prefixArgs: [],
              kind: 'native' as const
            }
          }
        ]
      }
    )
    await service.start('Read this.', 'claude', 'ask', {}, () => undefined)
    expect(spawn).not.toHaveBeenCalled()
    expect(start).toHaveBeenCalledTimes(1)
  })
})

describe('stopping a Codex run over app-server', () => {
  it('ends the run when the person presses stop, and says it was cancelled', async () => {
    let emit: (chunk: string) => void = () => undefined
    let killed = false
    const spawn = vi.fn(() => ({
      write: (line: string) => {
        for (const part of line.split('\n')) {
          if (part.trim().length === 0) continue
          const message = JSON.parse(part) as Record<string, unknown>
          if (typeof message.id !== 'number') continue
          const answer = message.method === 'thread/start'
            ? { thread: { id: 'thread-live' } }
            : message.method === 'turn/start'
              ? { turn: { id: 'turn-1' } }
              : { userAgent: 'codex' }
          queueMicrotask(() => {
            emit(`${JSON.stringify({ jsonrpc: '2.0', id: message.id, result: answer })}\n`)
            if (message.method !== 'turn/start') return
            // A turn that keeps talking and never completes: the case the
            // stop button exists for.
            queueMicrotask(() => {
              emit(`${JSON.stringify({ jsonrpc: '2.0', method: 'item/agentMessage/delta', params: { itemId: 'm1', delta: '1. ' } })}\n`)
            })
          })
        }
      },
      kill: () => {
        killed = true
      },
      onData: (listener: (chunk: string) => void) => {
        emit = listener
      },
      onExit: () => undefined
    }))
    const { service, scheduled } = scheduledService({ start: vi.fn() }, fakeLedger(), {
      appServerSpawn: spawn
    })
    const updates: CodexMissionUpdate[] = []
    const response = await service.start('Count to 200.', 'codex', 'accept-edits', {}, (update) => {
      updates.push(update)
    })
    expect(response.ok).toBe(true)
    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'message-deltas' && update.events.some((event) => event.type === 'message.delta'))).toBe(true)
    })

    const runId = response.ok ? response.data.runId : ''
    expect(service.cancel(runId)).toMatchObject({ ok: true })
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.cancelled')).toBe(true)
    })
    expect(killed).toBe(true)
    expect(service.liveMissionIds()).toEqual([])
  })
})

/*
 * A2.10: a running Codex turn is shown a message at its next step, not
 * stopped. The service steers the run it holds, on that run's own thread and
 * turn; a run on any other transport cannot be steered and says so.
 */
describe('steering a Codex run over app-server', () => {
  it('sends the message into the running turn, and the run keeps going', async () => {
    let emit: (chunk: string) => void = () => undefined
    const written: Record<string, unknown>[] = []
    const spawn = vi.fn(() => ({
      write: (line: string) => {
        for (const part of line.split('\n')) {
          if (part.trim().length === 0) continue
          const message = JSON.parse(part) as Record<string, unknown>
          written.push(message)
          if (typeof message.id !== 'number') continue
          const answer = message.method === 'thread/start'
            ? { thread: { id: 'thread-live' } }
            : message.method === 'turn/start'
              ? { turn: { id: 'turn-1' } }
              : message.method === 'turn/steer'
                ? { turnId: 'turn-1' }
                : { userAgent: 'codex' }
          queueMicrotask(() => {
            emit(`${JSON.stringify({ jsonrpc: '2.0', id: message.id, result: answer })}\n`)
            if (message.method !== 'turn/start') return
            queueMicrotask(() => {
              emit(`${JSON.stringify({ jsonrpc: '2.0', method: 'item/agentMessage/delta', params: { itemId: 'm1', delta: '1. ' } })}\n`)
            })
          })
        }
      },
      kill: () => undefined,
      onData: (listener: (chunk: string) => void) => {
        emit = listener
      },
      onExit: () => undefined
    }))
    const { service, scheduled } = scheduledService({ start: vi.fn() }, fakeLedger(), { appServerSpawn: spawn })
    const updates: CodexMissionUpdate[] = []
    const response = await service.start('Count to 200.', 'codex', 'accept-edits', {}, (update) => {
      updates.push(update)
    })
    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(updates.some((update) => update.kind === 'message-deltas' && update.events.some((event) => event.type === 'message.delta'))).toBe(true)
    })
    const runId = response.ok ? response.data.runId : ''

    expect(await service.steer(runId, 'Stop editing app.ts.')).toBe(true)
    expect(written.find((message) => message.method === 'turn/steer')?.params).toEqual({
      threadId: 'thread-live',
      expectedTurnId: 'turn-1',
      input: [{ type: 'text', text: 'Stop editing app.ts.' }]
    })
    // Not stopped: still live, nothing cancelled.
    expect(service.liveMissionIds()).toHaveLength(1)
    expect(updates.some((update) => update.kind === 'event' && update.event.type === 'run.cancelled')).toBe(false)
    expect(await service.steer('run_nobody', 'anyone?')).toBe(false)
  })
})

/**
 * Approve-each, on the same loop as every other mode.
 *
 * It used to have a mission service of its own -- 771 lines duplicating this
 * one -- because it was the only mode whose transport could stop and ask.
 * Every Codex mode rides that transport now, so the duplicate is gone and the
 * mode is ordinary: the same start, the same ledger, the same follow-up
 * rules, plus a channel that answers when the server asks.
 *
 * The channel's own behaviour is in `approval-channel.test.ts`. What is held
 * here is the wiring: that a card is raised from inside a live mission, that
 * the person's answer reaches the runtime, and that a run which ends with a
 * question outstanding does not leave it outstanding.
 */
describe('Approve-each is a mode of this service', () => {
  it('shows pending text durably before an approval card without waiting for the text deadline', async () => {
    vi.useFakeTimers()
    try {
      const order: string[] = []
      const approvals = createApprovalChannel({ emitApproval: () => { order.push('card') } })
      const server = fakeAppServer([['item/agentMessage/delta', { itemId: 'answer', delta: 'About to ask.' }]])
      const { service, scheduled } = scheduledService({ start: vi.fn() }, fakeLedger({
        appendEvents: async (_id, events) => {
          if (events.some((event) => event.type === 'message.delta')) order.push('disk')
        }
      }), { appServerSpawn: server.spawn, approvals })
      await service.start('Ask before running a command.', 'codex', 'approve-each', {}, (update) => {
        if (update.kind === 'message-deltas') order.push('text')
      })
      scheduled[0]?.()
      await vi.advanceTimersByTimeAsync(0)
      expect(order).toEqual([])
      server.ask('approval', 'item/commandExecution/requestApproval', { command: 'pnpm build' })
      await vi.advanceTimersByTimeAsync(0)
      expect(order).toEqual(['disk', 'text', 'card'])
      expect(approvals.pendingCount).toBe(1)
    } finally { vi.useRealTimers() }
  })

  function withApprovals() {
    const raised: MissionApprovalRequest[] = []
    const approvals = createApprovalChannel({
      emitApproval: (request) => raised.push(request),
      createId: (() => {
        let n = 0
        return () => String(++n)
      })()
    })
    return { approvals, raised }
  }

  it('runs write-capable and stops to ask, rather than falling through to read-only', async () => {
    const { approvals } = withApprovals()
    const server = fakeAppServer([])
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service, scheduled } = scheduledService({ start: vi.fn() }, fakeLedger({ createMission }), {
      appServerSpawn: server.spawn,
      approvals
    })
    const response = await service.start('Do something consequential.', 'codex', 'approve-each', {}, () => undefined)
    expect(response).toMatchObject({ ok: true, data: { sandbox: 'workspace-write' } })
    // The record says what it was allowed, and approvals only mean something
    // when the run could otherwise act.
    expect(createMission).toHaveBeenCalledWith(expect.objectContaining({ sandbox: 'workspace-write', mode: 'approve-each' }))
    scheduled[0]?.()

    // The policy that makes the server ask at all.
    await vi.waitFor(() => {
      expect(server.written.some((message) => message.method === 'turn/start')).toBe(true)
    })
    const thread = server.written.find((message) => message.method === 'thread/start')
    expect(thread?.params).toMatchObject({ sandbox: 'workspace-write', approvalPolicy: 'untrusted' })
  })

  it('raises a card mid-run and sends the person\u2019s answer back to the runtime', async () => {
    const { approvals, raised } = withApprovals()
    const server = fakeAppServer([])
    const { service, scheduled } = scheduledService({ start: vi.fn() }, fakeLedger(), {
      appServerSpawn: server.spawn,
      approvals
    })
    const response = await service.start('Build it.', 'codex', 'approve-each', {}, () => undefined)
    expect(response.ok).toBe(true)
    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(server.written.some((message) => message.method === 'turn/start')).toBe(true)
    })

    server.ask('srv-1', 'item/commandExecution/requestApproval', { command: 'pnpm build', cwd: 'C:\work' })
    await vi.waitFor(() => {
      expect(raised).toHaveLength(1)
    })
    expect(raised[0]).toMatchObject({ kind: 'command', detail: 'pnpm build', runtime: 'codex' })
    // The run it belongs to, so the card can be filed under the conversation.
    expect(raised[0]?.runId).toBe(response.ok ? response.data.runId : '')

    // Answered through the service, the way the IPC handler does it.
    expect(service.decide({ approvalId: raised[0]!.approvalId, decision: 'approve-once' })).toBe(true)
    await vi.waitFor(() => {
      expect(server.replyTo('srv-1')).toMatchObject({ result: { decision: 'accept' } })
    })
  })

  it('refuses what a stopped run was still asking, so no card waits on a run that has gone', async () => {
    const { approvals, raised } = withApprovals()
    const server = fakeAppServer([])
    const { service, scheduled } = scheduledService({ start: vi.fn() }, fakeLedger(), {
      appServerSpawn: server.spawn,
      approvals
    })
    const response = await service.start('Build it.', 'codex', 'approve-each', {}, () => undefined)
    scheduled[0]?.()
    await vi.waitFor(() => {
      expect(server.written.some((message) => message.method === 'turn/start')).toBe(true)
    })
    server.ask('srv-9', 'item/fileChange/requestApproval', { changes: [1] })
    await vi.waitFor(() => {
      expect(raised).toHaveLength(1)
    })

    expect(approvals.pendingCount).toBe(1)

    service.cancel(response.ok ? response.data.runId : '')

    /*
     * What is asserted is that nothing is LEFT WAITING, not that a refusal
     * reached the wire.
     *
     * Stopping kills the server, so whether the `reject` wins the race to a
     * dying pipe is immaterial -- and it is a real race: the channel's handler
     * is async, so resolving its promise takes two microtask hops to reach the
     * client's send, while disposing that client takes one. The old approval
     * service had the same ordering and asserted the same two things.
     *
     * What would be a defect is a promise nobody ever settles (the run hangs)
     * or a card that outlives its run (the person answers into nothing).
     */
    expect(approvals.pendingCount).toBe(0)
    expect(service.decide({ approvalId: raised[0]!.approvalId, decision: 'approve-once' })).toBe(false)
  })

  it('answers nothing when no channel is wired, rather than pretending it did', () => {
    const { service } = scheduledService({ start: vi.fn() }, fakeLedger())
    expect(service.decide({ approvalId: 'ap_1', decision: 'approve-once' })).toBe(false)
  })
})

describe('a model of your own (0.357)', () => {
  const opencodeRuntime = (): RuntimeDiscovery => {
    const codex = codexRuntime()
    return { ...codex, id: 'opencode', displayName: 'OpenCode', executable: { ...codex.executable!, commandName: 'opencode' } }
  }
  const run = () =>
    vi.fn((_spec, _prompt, _options): RuntimeProcessRun => ({
      records: records([]),
      completion: Promise.resolve(completion())
    })) satisfies RuntimeProcessRunner['start']

  it('is declared to OpenCode for the run, key and all, in the child environment only', async () => {
    const start = run()
    const { service } = scheduledService({ start }, fakeLedger(), {
      discover: async () => [opencodeRuntime()],
      ownProvider: async (model) =>
        model === 'own-a1b2c3d4/acme-70b'
          ? { id: 'own-a1b2c3d4', provider: { name: 'Acme Chat', baseUrl: 'https://llm.acme.example/v1', apiKey: 'sk-acme-secret', models: ['acme-70b'] } }
          : undefined
    })
    const started = await service.start('Say hello.', 'opencode', 'accept-edits', { model: 'own-a1b2c3d4/acme-70b' }, () => undefined)
    expect(started.ok).toBe(true)
    const spec = start.mock.calls[0]?.[0] as { readonly args: readonly string[]; readonly env?: Record<string, string> }
    const config = JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? '{}')
    expect(config.provider['own-a1b2c3d4'].options).toEqual({ baseURL: 'https://llm.acme.example/v1', apiKey: 'sk-acme-secret' })
    expect(spec.args).toContain('own-a1b2c3d4/acme-70b')
    expect(spec.args.join(' ')).not.toContain('sk-acme-secret')
  })

  it('is refused in words, before anything is written, once it has been removed', async () => {
    const start = run()
    const createMission = vi.fn<MissionLedger['createMission']>(async () => undefined)
    const { service } = scheduledService({ start }, fakeLedger({ createMission }), {
      discover: async () => [opencodeRuntime()],
      ownProvider: async () => undefined
    })
    const refused = await service.start('Say hello.', 'opencode', 'accept-edits', { model: 'own-a1b2c3d4/acme-70b' }, () => undefined)
    expect(refused).toMatchObject({ ok: false, error: { code: 'RUNTIME_START_FAILED', message: expect.stringContaining('no longer in Your own models') } })
    expect(start).not.toHaveBeenCalled()
    expect(createMission).not.toHaveBeenCalled()
  })

  it('changes nothing for any other OpenCode model', async () => {
    const start = run()
    const asked: string[] = []
    const { service } = scheduledService({ start }, fakeLedger(), {
      discover: async () => [opencodeRuntime()],
      ownProvider: async (model) => {
        asked.push(model)
        return undefined
      }
    })
    expect((await service.start('Say hello.', 'opencode', 'accept-edits', { model: 'opencode/ling-3.0-flash-fin-free' }, () => undefined)).ok).toBe(true)
    expect(asked).toEqual([])
    const spec = start.mock.calls[0]?.[0] as { readonly env?: Record<string, string> }
    expect(JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? '{}').provider).toBeUndefined()
  })
})

describe('a file the runtime named but the host could not read (0.597)', () => {
  it("is still said to have changed on disk, on the runtime's own row, with no patch", async () => {
    const appended: { type: string; payload: Record<string, unknown> }[] = []
    const scheduled: Array<() => void> = []
    let looks = 0
    let ids = 0
    const service = createCodexMissionService({
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: {
        start: () => ({
          records: records([
            { type: 'thread.started', thread_id: 'thread-live' },
            { type: 'turn.started' },
            { type: 'item.completed', item: { id: 'fc', type: 'file_change', status: 'completed', changes: [{ path: `${WORKSPACE}/src/app.ts`, kind: 'update' }] } },
            { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Done.' } },
            { type: 'turn.completed', usage: { output_tokens: 2 } }
          ]),
          completion: Promise.resolve(completion())
        })
      },
      ledger: fakeLedger({
        appendEvents: async (_missionId, events) => {
          appended.push(...(events as unknown as { type: string; payload: Record<string, unknown> }[]))
        }
      }),
      observeDisk: async () => {
        looks += 1
        return looks > 1 ? new Map([['src/app.ts', ' M']]) : new Map<string, string>()
      },
      // Nothing readable behind it: a dot-folder, a binary, a file past the size cap.
      observePatches: async () => new Map(),
      createId: () => String(++ids),
      now: () => new Date(NOW),
      schedule: (task) => scheduled.push(task)
    })
    const response = await service.start('Fix app.ts.', 'codex', 'accept-edits', {}, () => undefined)
    expect(response.ok).toBe(true)
    while (scheduled.length > 0) scheduled.shift()!()
    for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))
    const observed = appended.filter((event) => event.payload.toolKind === 'observed_edit')
    expect(observed.map((event) => [event.type, event.payload.command, event.payload.status])).toEqual([
      ['tool.started', 'src/app.ts', 'reported by the runtime, changed on disk'],
      ['tool.completed', 'src/app.ts', 'reported by the runtime, changed on disk']
    ])
    expect(observed[1]!.payload.patch).toBeUndefined()
  })
})

/*
 * 0.602. Between Send and the runtime's "started" the window said one word for
 * a median 4.5 to 10.5 s (10/04, 183 missions). The start now says its phase
 * as each begins, and the run's end writes where the seconds went.
 */
describe('the start says its phase, and the run writes where its seconds went', () => {
  const build = (extra: { readonly setsItselfUp?: (runtime: string) => boolean } = {}) => {
    const appended: { type: string; payload: Record<string, unknown> }[] = []
    const updates: CodexMissionUpdate[] = []
    const scheduled: Array<() => void> = []
    let ids = 0
    const service = createCodexMissionService({
      ...extra,
      workspacePath: WORKSPACE,
      discover: async () => [codexRuntime()],
      runner: {
        start: () => ({
          records: records([
            { type: 'thread.started', thread_id: 'thread-live' },
            { type: 'turn.started' },
            { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Done.' } },
            { type: 'turn.completed', usage: { output_tokens: 2 } }
          ]),
          completion: Promise.resolve(completion())
        })
      },
      ledger: fakeLedger({
        appendEvents: async (_missionId, events) => {
          appended.push(...(events as unknown as { type: string; payload: Record<string, unknown> }[]))
        }
      }),
      observeDisk: async () => new Map<string, string>(),
      observePatches: async () => new Map(),
      createId: () => String(++ids),
      now: () => new Date(NOW),
      schedule: (task) => scheduled.push(task)
    })
    return { service, appended, updates, scheduled }
  }
  const phasesOf = (updates: readonly CodexMissionUpdate[]): string[] => updates.filter((update) => update.kind === 'start-phase').map((update) => (update as { phase: string }).phase)

  it('says looking, briefing, reading the folder and starting, in that order, and the end writes the note', async () => {
    const { service, appended, updates, scheduled } = build()
    const response = await service.start('Say done.', 'codex', 'accept-edits', {}, (update) => updates.push(update))
    expect(response.ok).toBe(true)
    expect(phasesOf(updates)).toEqual(['looking', 'briefing', 'reading-folder', 'starting-runtime'])
    while (scheduled.length > 0) scheduled.shift()!()
    for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))
    const completed = appended.findIndex((event) => event.type === 'run.completed')
    expect(completed).toBeGreaterThan(0)
    const note = appended.slice(completed + 1).find((event) => event.payload.code === 'host.start-timing')
    expect(note?.type).toBe('adapter.diagnostic')
    expect(String(note?.payload.message)).toMatch(/^Started in \d+\.\d s: looked for Codex CLI \d+\.\d s, briefed \d+\.\d s, read the folder \d+\.\d s; Codex CLI took \d+\.\d s to say it had started\.$/)
  })

  // 0.780: OpenCode's first start installs its plugin package first; the line says that, the note still times the start.
  it('says the runtime is setting itself up when its start will, and times it as the start', async () => {
    const { service, appended, updates, scheduled } = build({ setsItselfUp: (runtime) => runtime === 'codex' })
    const response = await service.start('Say done.', 'codex', 'accept-edits', {}, (update) => updates.push(update))
    expect(response.ok).toBe(true)
    // From the look for it on (0.813): that look waits on the same install, so "Looking for" was all a person read.
    expect(phasesOf(updates)).toEqual(['runtime-first-setup', 'briefing', 'reading-folder', 'runtime-first-setup'])
    while (scheduled.length > 0) scheduled.shift()!()
    for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve))
    const completed = appended.findIndex((event) => event.type === 'run.completed')
    const note = appended.slice(completed + 1).find((event) => event.payload.code === 'host.start-timing')
    expect(String(note?.payload.message)).toMatch(/; Codex CLI took \d+\.\d s to say it had started\.$/)
  })

  it('reads no folder for a read-only start, and says so by leaving the phase out', async () => {
    const { service, updates } = build()
    const response = await service.start('What is here?', 'codex', 'ask', {}, (update) => updates.push(update))
    expect(response.ok).toBe(true)
    expect(phasesOf(updates)).toEqual(['looking', 'briefing', 'starting-runtime'])
  })
})

/*
 * DURABLE WRITE PIPELINING (0.603). Every append is an fsync, and the loop
 * used to await each before reading the next record -- one fsync a token,
 * each token waiting on its own. Records are now read and normalized while an
 * append is on the disk, and everything that arrived meanwhile goes in the
 * next append. Persist-before-emit still holds per batch.
 */
describe('durable write pipelining', () => {
  const record = (sequence: number, value: Record<string, unknown>): RuntimeJsonlRecord => ({ sequence, raw: JSON.stringify(value) })
  const deferred = (): { readonly promise: Promise<void>; readonly resolve: () => void } => {
    let resolve!: () => void
    const promise = new Promise<void>((settle) => { resolve = settle })
    return { promise, resolve }
  }
  /** A stream the test paces: it hands over what it is given, when it is given, and counts what the consumer took. */
  const pacedStream = () => {
    const queue: Array<{ readonly record: RuntimeJsonlRecord } | { readonly end: true }> = []
    const waiters: Array<() => void> = []
    const push = (entry: { readonly record: RuntimeJsonlRecord } | { readonly end: true }): void => { queue.push(entry); waiters.shift()?.() }
    const taken: number[] = []
    const stream: RuntimeProcessRecordStream = {
      async *[Symbol.asyncIterator]() {
        for (;;) {
          if (queue.length === 0) await new Promise<void>((resolve) => waiters.push(resolve))
          const next = queue.shift()!
          if ('end' in next) return
          taken.push(next.record.sequence)
          yield next.record
        }
      },
      drainAvailable: () => []
    }
    return { stream, taken, give: (entry: RuntimeJsonlRecord) => push({ record: entry }), end: () => push({ end: true }) }
  }

  it('replays the same paced answer with fewer durable appends and sends while keeping every fragment', async () => {
    vi.useFakeTimers()
    try {
      const appends: NormalizedRuntimeEvent[][] = []
      const updates: CodexMissionUpdate[] = []
      const paced = pacedStream()
      const start = vi.fn((): RuntimeProcessRun => ({ records: paced.stream, completion: Promise.resolve(completion()) })) satisfies RuntimeProcessRunner['start']
      const { service, scheduled } = scheduledService({ start }, fakeLedger({
        appendEvents: async (_id, events) => { appends.push([...events]) }
      }))
      await service.start('Say a thousand words.', 'codex', 'ask', {}, (update) => { updates.push(update) })
      scheduled[0]?.()
      paced.give(record(1, { type: 'thread.started', thread_id: 'thread-stream' }))
      await vi.advanceTimersByTimeAsync(2)
      paced.give(record(2, { type: 'turn.started' }))
      await vi.advanceTimersByTimeAsync(2)
      const words = Array.from({ length: 1_000 }, (_, index) => `word${String(index)} `)
      for (const [index, word] of words.entries()) {
        paced.give(record(index + 3, { type: 'item.updated', item: { id: 'answer', type: 'agent_message', delta: word } }))
        await vi.advanceTimersByTimeAsync(2)
      }
      paced.give(record(1003, { type: 'turn.completed' }))
      paced.end()
      await vi.advanceTimersByTimeAsync(0)
      await service.dispose()
      const recorded = appends.flat().filter((event) => event.type === 'message.delta')
      const sent = updates.flatMap((update) => update.kind === 'message-deltas' ? [...update.events] : update.kind === 'event' && update.event.type === 'message.delta' ? [update.event] : [])
      console.log(`STREAM TURN: ${JSON.stringify({ appends: appends.length, sends: updates.filter((update) => update.kind === 'event' || update.kind === 'message-deltas').length, fragments: recorded.length, characters: words.join('').length })}`)
      expect(recorded).toHaveLength(1_000)
      expect(sent).toEqual(recorded)
      expect(recorded.map((event) => event.payload.text).join('')).toBe(words.join(''))
      expect(appends.length).toBeLessThan(100)
      expect(updates.filter((update) => update.kind === 'event' || update.kind === 'message-deltas').length).toBeLessThan(100)
    } finally { vi.useRealTimers() }
  })

  it('reads and normalizes while an append is on the disk, and writes what arrived in one append when it lands', async () => {
    const appends: NormalizedRuntimeEvent[][] = []
    const emittedAtAppend: string[][] = []
    const emitted: string[] = []
    const firstAppend = deferred()
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async (_missionId, events) => {
      appends.push([...events])
      emittedAtAppend.push([...emitted])
      if (appends.length === 1) await firstAppend.promise
    })
    const paced = pacedStream()
    const start = vi.fn((): RuntimeProcessRun => ({ records: paced.stream, completion: Promise.resolve(completion()) })) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, fakeLedger({ appendEvents }))
    await service.start('Do work.', 'codex', 'ask', {}, (update) => { if (update.kind === 'event') emitted.push(update.event.type); if (update.kind === 'message-deltas') emitted.push(...update.events.map((event) => event.type)) })
    scheduled[0]?.()

    paced.give(record(1, { type: 'thread.started', thread_id: 'thread-live' }))
    await vi.waitFor(() => expect(appends).toHaveLength(1))
    expect(appends[0]!.map(({ type }) => type)).toEqual(['run.started'])
    // Three more records while the first append is still on the disk.
    paced.give(record(2, { type: 'turn.started' }))
    paced.give(record(3, { type: 'item.completed', item: { id: 'a', type: 'agent_message', text: 'one' } }))
    paced.give(record(4, { type: 'turn.completed' }))
    await vi.waitFor(() => expect(paced.taken).toEqual([1, 2, 3, 4]))
    // Read and normalized, not yet written or shown: the disk still holds the first append.
    expect(appends).toHaveLength(1)
    expect(emitted).toEqual([])
    firstAppend.resolve()
    await vi.waitFor(() => expect(appends).toHaveLength(2))
    // One append for everything that arrived meanwhile, in order.
    expect(appends[1]!.map(({ type }) => type)).toEqual(['step.started', 'message.delta', 'step.completed'])
    // Persist-before-emit, per batch: when the second append began, only the first batch had been shown.
    expect(emittedAtAppend[1]).toEqual(['run.started'])
    paced.end()
    await vi.waitFor(() => expect(appends.length).toBeGreaterThanOrEqual(3))
    expect(appends[2]!.map(({ type }) => type)).toEqual(['run.completed'])
    // The host's start-timing note (0.602) follows the run's own events; the order of those is what this checks.
    await vi.waitFor(() => expect(emitted.filter((type) => type !== 'adapter.diagnostic')).toEqual(['run.started', 'step.started', 'message.delta', 'step.completed', 'run.completed']))
  })

  it('stops reading at the bound and lets a flood back up into the runner, as it always did', async () => {
    const appends: NormalizedRuntimeEvent[][] = []
    const firstAppend = deferred()
    const appendEvents = vi.fn<MissionLedger['appendEvents']>(async (_missionId, events) => {
      appends.push([...events])
      if (appends.length === 1) await firstAppend.promise
    })
    const paced = pacedStream()
    const start = vi.fn((): RuntimeProcessRun => ({ records: paced.stream, completion: Promise.resolve(completion()) })) satisfies RuntimeProcessRunner['start']
    const { service, scheduled } = scheduledService({ start }, fakeLedger({ appendEvents }), { maxWaitingEvents: 2 })
    await service.start('Do work.', 'codex', 'ask', {}, () => undefined)
    scheduled[0]?.()
    paced.give(record(1, { type: 'thread.started', thread_id: 'thread-live' }))
    await vi.waitFor(() => expect(appends).toHaveLength(1))
    // Two events waiting is the bound: the third record is not taken while the disk holds the first append.
    paced.give(record(2, { type: 'turn.started' }))
    paced.give(record(3, { type: 'item.completed', item: { id: 'a', type: 'agent_message', text: 'one' } }))
    paced.give(record(4, { type: 'turn.completed' }))
    await vi.waitFor(() => expect(paced.taken).toEqual([1, 2, 3]))
    await new Promise((resolve) => setTimeout(resolve, 30))
    expect(paced.taken).toEqual([1, 2, 3])
    firstAppend.resolve()
    await vi.waitFor(() => expect(paced.taken).toEqual([1, 2, 3, 4]))
    await vi.waitFor(() => expect(appends.length).toBeGreaterThanOrEqual(2))
    expect(appends[1]!.map(({ type }) => type)).toEqual(['step.started', 'message.delta'])
    paced.end()
    await vi.waitFor(() => expect(appends.flat().map(({ type }) => type)).toEqual(expect.arrayContaining(['step.completed', 'run.completed'])))
  })
})
